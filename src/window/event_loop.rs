use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, Modifiers, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::persistence::db::Database;
use crate::playback::engine::PlaybackEngine;
use crate::playback::rodio_engine::RodioPlaybackEngine;
use crate::ui::action::UiAction;
use crate::ui::data::Library;
use crate::ui::font::Font;
use crate::ui::hit::{hit_test, HitRegion, Interaction};
use crate::ui::primitives::DrawList;
use crate::ui::shell;
use crate::ui::state::AppState;
use crate::ui::theme::Theme;
use crate::window::renderer::Renderer;

/// Smart-resume rewind on startup: jump back this far so the user re-hears
/// some context. 
const SMART_RESUME_REWIND_MS: i64 = 5_000;
const SEEK_BACKWARD_SECS: u64 = 15;
const SEEK_FORWARD_SECS: u64 = 30;
/// While playing we redraw at this cadence so the progress bar advances.
const PLAYING_TICK: Duration = Duration::from_millis(500);

pub fn run(initial_state: AppState, db: Database) -> Result<(), String> {
    let event_loop =
        EventLoop::new().map_err(|error| format!("failed to create event loop: {error}"))?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = EchoverApp {
        pending_state: Some(initial_state),
        pending_db: Some(db),
        window_state: None,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|error| format!("event loop terminated with error: {error}"))
}

struct EchoverApp {
    pending_state: Option<AppState>,
    pending_db: Option<Database>,
    window_state: Option<WindowState>,
}

impl ApplicationHandler for EchoverApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window_state.is_some() {
            return;
        }
        tracing::info!("event loop resumed, creating window");
        let app_state = self.pending_state.take().unwrap_or_default();
        let Some(db) = self.pending_db.take() else {
            tracing::error!("missing database handle on resume");
            event_loop.exit();
            return;
        };
        match WindowState::new(event_loop, app_state, db) {
            Ok(state) => {
                tracing::info!("window + renderer initialized");
                state.window.request_redraw();
                self.window_state = Some(state);
            }
            Err(error) => {
                tracing::error!("{error}");
                event_loop.exit();
            }
        }
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. }) {
            if let Some(state) = self.window_state.as_ref() {
                if state.app_state.is_playing {
                    state.window.request_redraw();
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(state) = self.window_state.as_ref() else {
            return;
        };
        if state.app_state.is_playing {
            event_loop
                .set_control_flow(ControlFlow::WaitUntil(Instant::now() + PLAYING_TICK));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = self.window_state.as_mut() {
            state.persist_position();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.window_state.as_mut() else {
            return;
        };
        if state.window.id() != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.resize(size);
                state.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                state.resize(state.window.inner_size());
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = state.render() {
                    tracing::error!("{error}");
                    event_loop.exit();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor_moved(position);
            }
            WindowEvent::CursorLeft { .. } => {
                state.cursor_left();
            }
            WindowEvent::ModifiersChanged(mods) => {
                state.modifiers = mods;
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                state.mouse_pressed();
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                state.mouse_released();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        logical_key,
                        text,
                        ..
                    },
                ..
            } => {
                state.handle_key(event_loop, logical_key, text);
            }
            _ => {}
        }
    }
}

struct WindowState {
    window: Window,
    renderer: Renderer,
    theme: Theme,
    app_state: AppState,
    cursor: (f32, f32),
    hit_regions: Vec<HitRegion>,
    draw_list: DrawList,
    font: Font,
    modifiers: Modifiers,
    interaction: Interaction,
    engine: Option<RodioPlaybackEngine>,
    db: Database,
}

impl WindowState {
    fn new(
        event_loop: &ActiveEventLoop,
        mut app_state: AppState,
        db: Database,
    ) -> Result<Self, String> {
        let window = event_loop
            .create_window(
                WindowAttributes::default()
                    .with_title("Echover")
                    .with_inner_size(PhysicalSize::new(1200, 800))
                    .with_resizable(true),
            )
            .map_err(|error| format!("failed to create window: {error}"))?;

        let size = window.inner_size();
        let font = Font::load_default()?;
        let renderer =
            Renderer::new(&window, size.width.max(1), size.height.max(1), &font.atlas)?;

        let engine = match RodioPlaybackEngine::new() {
            Ok(engine) => Some(engine),
            Err(error) => {
                tracing::warn!(%error, "audio engine unavailable; UI runs read-only");
                None
            }
        };

        // Smart-resume: pre-seed playback_position_ms for the current book.
        if let Some(book) = app_state.library.current_listening() {
            if let Ok(Some(saved)) = db.load_playback_state_for_audiobook(book.id) {
                let resumed = (saved.position_ms - SMART_RESUME_REWIND_MS).max(0);
                app_state.playback_position_ms = resumed;
                tracing::info!(
                    audiobook = book.id,
                    saved_ms = saved.position_ms,
                    resume_ms = resumed,
                    "restored playback position"
                );
            }
        }

        Ok(Self {
            window,
            renderer,
            theme: Theme::default(),
            app_state,
            cursor: (-1.0, -1.0),
            hit_regions: Vec::new(),
            draw_list: DrawList::default(),
            font,
            modifiers: Modifiers::default(),
            interaction: Interaction::default(),
            engine,
            db,
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        if let Err(error) = self.renderer.resize(size.width, size.height) {
            tracing::error!("{error}");
        }
    }

    fn cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        self.cursor = (position.x as f32, position.y as f32);
        self.refresh_hover();
    }

    fn cursor_left(&mut self) {
        self.cursor = (-1.0, -1.0);
        self.refresh_hover();
    }

    fn refresh_hover(&mut self) {
        let new_hover = hit_test(&self.hit_regions, self.cursor);
        if new_hover != self.interaction.hover {
            self.interaction.hover = new_hover;
            self.window.request_redraw();
        }
    }

    fn mouse_pressed(&mut self) {
        let hit = hit_test(&self.hit_regions, self.cursor);
        let mut redraw = false;
        if hit != Some(UiAction::FocusSearch) && self.app_state.unfocus_search() {
            redraw = true;
        }
        if self.interaction.pressed != hit {
            self.interaction.pressed = hit;
            redraw = true;
        }
        if redraw {
            self.window.request_redraw();
        }
    }

    fn mouse_released(&mut self) {
        let pressed = self.interaction.pressed.take();
        let release_target = hit_test(&self.hit_regions, self.cursor);
        let mut redraw = true;

        if let (Some(p), Some(r)) = (pressed, release_target) {
            if p == r {
                tracing::info!(action = ?p, "ui action triggered");
                if self.dispatch_action(p) {
                    redraw = true;
                }
            }
        }

        if redraw {
            self.window.request_redraw();
        }
    }

    fn handle_key(
        &mut self,
        event_loop: &ActiveEventLoop,
        key: Key,
        text: Option<winit::keyboard::SmolStr>,
    ) {
        if self.app_state.search_focused {
            match &key {
                Key::Named(NamedKey::Escape) => {
                    if self.app_state.apply_action(UiAction::ClearSearch) {
                        self.window.request_redraw();
                    }
                    return;
                }
                Key::Named(NamedKey::Enter) => {
                    if self.app_state.unfocus_search() {
                        self.window.request_redraw();
                    }
                    return;
                }
                Key::Named(NamedKey::Backspace) => {
                    if self.app_state.search_backspace() {
                        self.window.request_redraw();
                    }
                    return;
                }
                _ => {}
            }
            if let Some(text) = text.as_deref() {
                let mut changed = false;
                for ch in text.chars() {
                    if self.app_state.search_input(ch) {
                        changed = true;
                    }
                }
                if changed {
                    self.window.request_redraw();
                    return;
                }
            }
        }

        let action = match &key {
            Key::Named(NamedKey::Escape) => {
                event_loop.exit();
                return;
            }
            Key::Named(NamedKey::Space) => Some(UiAction::PlayPause),
            Key::Named(NamedKey::ArrowLeft) => Some(UiAction::SeekBackward),
            Key::Named(NamedKey::ArrowRight) => Some(UiAction::SeekForward),
            Key::Character(s) if s.as_ref().eq_ignore_ascii_case("b") => {
                Some(UiAction::AddBookmark)
            }
            Key::Character(s)
                if s.as_ref().eq_ignore_ascii_case("f")
                    && (self.modifiers.state().control_key()
                        || self.modifiers.state().super_key()) =>
            {
                if self.app_state.apply_action(UiAction::NavigateLibrary) {
                    self.window.request_redraw();
                }
                Some(UiAction::FocusSearch)
            }
            _ => None,
        };

        if let Some(action) = action {
            tracing::info!(?action, "shortcut triggered");
            if self.dispatch_action(action) {
                self.window.request_redraw();
            }
        }
    }

    fn ensure_loaded(&mut self) -> Result<(), String> {
        let Some(engine) = self.engine.as_mut() else {
            return Err("audio engine unavailable".to_owned());
        };
        let Some(book) = self.app_state.library.current_listening() else {
            return Err("no audiobook to play".to_owned());
        };
        let book_id = book.id;
        if self.app_state.loaded_audiobook_id == Some(book_id) {
            return Ok(());
        }
        let path = match self.db.first_file_path_for_audiobook(book_id)? {
            Some(p) => p,
            None => return Err(format!("audiobook {book_id} has no playable file")),
        };
        engine.load(&path)?;
        let resume_ms = self.app_state.playback_position_ms;
        if resume_ms > 0 {
            engine.seek_forward((resume_ms / 1000) as u64)?;
        }
        self.app_state.loaded_audiobook_id = Some(book_id);
        tracing::info!(audiobook = book_id, file = %path, "loaded audio");
        Ok(())
    }

    fn dispatch_action(&mut self, action: UiAction) -> bool {
        match action {
            UiAction::PlayPause => {
                if let Err(error) = self.ensure_loaded() {
                    tracing::warn!(%error, "play/pause skipped");
                    return false;
                }
                if let Some(engine) = self.engine.as_mut() {
                    if let Err(error) = engine.toggle() {
                        tracing::error!(%error, "engine toggle failed");
                        return false;
                    }
                    self.app_state.is_playing = engine.is_playing();
                    self.app_state.playback_position_ms = engine.current_position_ms();
                    self.persist_position();
                    return true;
                }
                false
            }
            UiAction::ContinueListening => {
                if let Err(error) = self.ensure_loaded() {
                    tracing::warn!(%error, "continue skipped");
                    return false;
                }
                if let Some(engine) = self.engine.as_mut() {
                    if let Err(error) = engine.play() {
                        tracing::error!(%error, "engine play failed");
                        return false;
                    }
                    self.app_state.is_playing = engine.is_playing();
                }
                let _ = self.app_state.apply_action(UiAction::NavigatePlayer);
                true
            }
            UiAction::SeekBackward => {
                if let Err(error) = self.ensure_loaded() {
                    tracing::warn!(%error, "seek skipped");
                    return false;
                }
                if let Some(engine) = self.engine.as_mut() {
                    if let Err(error) = engine.seek_backward(SEEK_BACKWARD_SECS) {
                        tracing::error!(%error, "seek backward failed");
                        return false;
                    }
                    self.app_state.playback_position_ms = engine.current_position_ms();
                    self.persist_position();
                    return true;
                }
                false
            }
            UiAction::SeekForward => {
                if let Err(error) = self.ensure_loaded() {
                    tracing::warn!(%error, "seek skipped");
                    return false;
                }
                if let Some(engine) = self.engine.as_mut() {
                    if let Err(error) = engine.seek_forward(SEEK_FORWARD_SECS) {
                        tracing::error!(%error, "seek forward failed");
                        return false;
                    }
                    self.app_state.playback_position_ms = engine.current_position_ms();
                    self.persist_position();
                    return true;
                }
                false
            }
            UiAction::SelectChapter(book_id, chapter) => {
                let path = match self.db.file_path_for_chapter(book_id, chapter) {
                    Ok(Some(p)) => p,
                    Ok(None) => {
                        tracing::warn!(audiobook = book_id, chapter, "no file for chapter");
                        return false;
                    }
                    Err(error) => {
                        tracing::error!(%error, "chapter lookup failed");
                        return false;
                    }
                };
                let Some(engine) = self.engine.as_mut() else {
                    tracing::warn!("audio engine unavailable; chapter not played");
                    return false;
                };
                if let Err(error) = engine.load(&path) {
                    tracing::error!(%error, "engine load failed");
                    return false;
                }
                if let Err(error) = engine.play() {
                    tracing::error!(%error, "engine play failed");
                    return false;
                }
                self.app_state.loaded_audiobook_id = Some(book_id);
                self.app_state.is_playing = engine.is_playing();
                self.app_state.playback_position_ms = engine.current_position_ms();
                self.persist_position();
                let _ = self.app_state.apply_action(UiAction::NavigatePlayer);
                tracing::info!(audiobook = book_id, chapter, "playing chapter");
                true
            }
            UiAction::JumpToBookmark(book_id, position_ms) => {
                if self.app_state.loaded_audiobook_id != Some(book_id) {
                    let path = match self.db.first_file_path_for_audiobook(book_id) {
                        Ok(Some(p)) => p,
                        Ok(None) => {
                            tracing::warn!(audiobook = book_id, "no playable file");
                            return false;
                        }
                        Err(error) => {
                            tracing::error!(%error, "book lookup failed");
                            return false;
                        }
                    };
                    if let Some(engine) = self.engine.as_mut() {
                        if let Err(error) = engine.load(&path) {
                            tracing::error!(%error, "engine load failed");
                            return false;
                        }
                    }
                    self.app_state.loaded_audiobook_id = Some(book_id);
                }
                if let Some(engine) = self.engine.as_mut() {
                    if let Err(error) = engine.seek_to_ms(position_ms) {
                        tracing::error!(%error, "engine seek failed");
                        return false;
                    }
                    if let Err(error) = engine.play() {
                        tracing::error!(%error, "engine play failed");
                        return false;
                    }
                    self.app_state.is_playing = engine.is_playing();
                    self.app_state.playback_position_ms = engine.current_position_ms();
                    self.persist_position();
                }
                let _ = self.app_state.apply_action(UiAction::NavigatePlayer);
                tracing::info!(audiobook = book_id, position_ms, "jumped to bookmark");
                true
            }
            UiAction::AddLibraryFolder => {
                let chosen = rfd::FileDialog::new()
                    .set_title("Pick an audiobook folder")
                    .pick_folder();
                let Some(path) = chosen else {
                    tracing::info!("add-folder cancelled");
                    return false;
                };
                tracing::info!(folder = %path.display(), "scanning new folder");
                if let Err(error) = self.db.scan_and_ingest_from(&path) {
                    tracing::error!(%error, "scan failed");
                    return false;
                }
                match Library::from_db(&self.db) {
                    Ok(library) => {
                        self.app_state.library = library;
                        tracing::info!(
                            books = self.app_state.library.books.len(),
                            folders = self.app_state.library.folders.len(),
                            "library re-hydrated after scan"
                        );
                        true
                    }
                    Err(error) => {
                        tracing::error!(%error, "library re-hydrate failed");
                        false
                    }
                }
            }
            UiAction::AddBookmark => {
                if let Some(book) = self.app_state.library.current_listening() {
                    let book_id = book.id;
                    let position = self.app_state.playback_position_ms;
                    if let Err(error) =
                        self.db.create_bookmark(book_id, position, "Quick bookmark")
                    {
                        tracing::error!(%error, "bookmark insert failed");
                        return false;
                    }
                    tracing::info!(audiobook = book_id, position, "bookmark added");
                    if let Ok(updated) = self.db.load_all_bookmarks_with_titles() {
                        self.app_state.library.bookmarks = updated
                            .into_iter()
                            .map(|b| crate::ui::data::DisplayBookmark {
                                book_id: b.audiobook_id,
                                book_title: b.book_title,
                                note: b.note,
                                timestamp: crate::ui::data::format_position(b.position_ms),
                                position_ms: b.position_ms,
                            })
                            .collect();
                    }
                    return true;
                }
                false
            }
            _ => self.app_state.apply_action(action),
        }
    }

    fn persist_position(&self) {
        let Some(book_id) = self.app_state.loaded_audiobook_id else {
            return;
        };
        let position_ms = self.app_state.playback_position_ms;
        let total_ms = self
            .app_state
            .library
            .find_book(book_id)
            .map(|b| b.total_duration_ms)
            .unwrap_or(0);
        let completed = total_ms > 0 && position_ms >= total_ms;
        if let Err(error) = self
            .db
            .upsert_playback_state_minimal(book_id, position_ms, completed)
        {
            tracing::error!(%error, "failed to persist playback position");
        }
    }

    fn render(&mut self) -> Result<(), String> {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }

        if let (Some(engine), Some(_)) = (self.engine.as_ref(), self.app_state.loaded_audiobook_id) {
            self.app_state.playback_position_ms = engine.current_position_ms();
            self.app_state.is_playing = engine.is_playing();
        }

        self.draw_list.clear();
        self.hit_regions.clear();
        shell::layout(
            size.width as f32,
            size.height as f32,
            &self.theme,
            &self.app_state,
            &self.interaction,
            &self.font,
            &mut self.draw_list,
            &mut self.hit_regions,
        );

        let new_hover = hit_test(&self.hit_regions, self.cursor);
        if new_hover != self.interaction.hover {
            self.interaction.hover = new_hover;
        }

        self.window.pre_present_notify();
        self.renderer
            .render(size.width, size.height, &self.draw_list.commands)
    }
}
