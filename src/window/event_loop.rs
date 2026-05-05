use std::{
    num::NonZeroU32,
    time::{Duration, Instant},
};

use glutin::config::{Config, ConfigTemplateBuilder, GlConfig};
use glutin::context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext, Version};
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::surface::{GlSurface, Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::playback::{engine::PlaybackEngine, rodio_engine::RodioPlaybackEngine};
use crate::persistence::db::Database;
use crate::ui::action::UiAction;
use crate::ui::state::{AppPage, AppState, PlaybackStatus};
use crate::window::renderer::Renderer;

const PLAYBACK_TICK_INTERVAL: Duration = Duration::from_secs(1);
const PLAYBACK_PERSIST_INTERVAL: Duration = Duration::from_secs(7);
const RESUME_REWIND_SECONDS: i64 = 10;
const BOOKMARK_TOAST_SECONDS: u64 = 2;

pub fn run(initial_app_state: AppState, database: Database) -> Result<(), String> {
    let event_loop =
        EventLoop::new().map_err(|error| format!("failed to create event loop: {error}"))?;
    let mut app = EchoverApp::new(initial_app_state, database);
    event_loop
        .run_app(&mut app)
        .map_err(|error| format!("event loop terminated with error: {error}"))
}

struct EchoverApp {
    window_state: Option<WindowState>,
    initial_app_state: Option<AppState>,
    database: Option<Database>,
}

impl EchoverApp {
    fn new(initial_app_state: AppState, database: Database) -> Self {
        Self {
            window_state: None,
            initial_app_state: Some(initial_app_state),
            database: Some(database),
        }
    }
}

impl ApplicationHandler for EchoverApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window_state.is_some() {
            return;
        }

        let app_state = self
            .initial_app_state
            .take()
            .unwrap_or_else(AppState::default);
        let database = self.database.take().expect("database should exist once");

        match WindowState::new(event_loop, app_state, database) {
            Ok(mut state) => {
                state.request_redraw("ui state change");
                self.window_state = Some(state);
            }
            Err(error) => {
                tracing::error!("{error}");
                event_loop.exit();
            }
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
            WindowEvent::CloseRequested => {
                state.persist_before_exit();
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                state.resize(size);
                state.request_redraw("window resize");
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = state.window.inner_size();
                state.resize(size);
                state.request_redraw("window resize");
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = state.render() {
                    tracing::error!("{error}");
                    event_loop.exit();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.app_state.mouse_position = (position.x as f32, position.y as f32);
                state.app_state.interaction.hovered_action =
                    state.renderer.action_at(state.app_state.mouse_position);
                state.request_redraw("mouse move");
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                state.app_state.interaction.mouse_down = true;
                let mut redraw_reason = "mouse click";

                if let Some(action) = state.renderer.handle_click(state.app_state.mouse_position) {
                    tracing::info!(?action, "ui action triggered");
                    if state.dispatch_action(action) {
                        redraw_reason = "ui state change";
                    }
                }

                state.request_redraw(redraw_reason);
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                state.app_state.interaction.mouse_down = false;
                state.request_redraw("mouse click");
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * 48.0,
                    MouseScrollDelta::PixelDelta(pos) => -pos.y as f32,
                };

                if state.app_state.scroll_current_page_by(delta) {
                    state.request_redraw("mouse scroll");
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                let mut changed = false;
                match &event.logical_key {
                    Key::Named(NamedKey::Space) => {
                        changed = state.dispatch_action(UiAction::PlayPause);
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        changed = state.dispatch_action(UiAction::SeekBackward);
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        changed = state.dispatch_action(UiAction::SeekForward);
                    }
                    Key::Named(NamedKey::Backspace) => {
                        changed = state.app_state.pop_search_char();
                    }
                    Key::Named(NamedKey::Escape) => {
                        changed = state.app_state.clear_transient_ui();
                        tracing::info!("escape pressed: clear focus/overlay placeholder");
                    }
                    Key::Character(text) => {
                        if text.eq_ignore_ascii_case("b")
                            && matches!(
                                state.app_state.current_page,
                                crate::ui::state::AppPage::Player
                            )
                        {
                            changed = state.dispatch_action(UiAction::AddBookmark);
                            tracing::info!("bookmark action triggered");
                        } else {
                            for ch in text.chars() {
                                if !ch.is_control() {
                                    changed |= state.app_state.append_search_char(ch);
                                }
                            }
                        }
                    }
                    _ => {}
                }

                if changed {
                    state.request_redraw("ui state change");
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(state) = self.window_state.as_mut() {
            state.handle_periodic_playback_tick(Instant::now());
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + PLAYBACK_TICK_INTERVAL,
            ));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

struct WindowState {
    window: Window,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    renderer: Renderer,
    database: Database,
    playback_engine: Box<dyn PlaybackEngine>,
    loaded_file_path: Option<String>,
    resume_load_requested: bool,
    last_playback_persist_at: Instant,
    app_state: AppState,
    pending_redraw_reason: Option<&'static str>,
    render_count: u64,
}

impl WindowState {
    fn new(event_loop: &ActiveEventLoop, app_state: AppState, database: Database) -> Result<Self, String> {
        let window_attributes = WindowAttributes::default()
            .with_title("Echover")
            .with_inner_size(PhysicalSize::new(1200, 800))
            .with_resizable(true);

        let config_template = ConfigTemplateBuilder::new().with_alpha_size(8);
        let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes));

        let (window, gl_config) = display_builder
            .build(event_loop, config_template, choose_gl_config)
            .map_err(|error| format!("failed to create window/display: {error}"))?;

        let window = window.ok_or_else(|| "glutin returned no window".to_owned())?;
        let raw_window_handle = window
            .window_handle()
            .map_err(|error| format!("failed to fetch raw window handle: {error}"))?
            .as_raw();

        let context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
            .build(Some(raw_window_handle));
        let fallback_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(None))
            .build(Some(raw_window_handle));

        let gl_display = gl_config.display();

        let not_current_context = unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .or_else(|_| gl_display.create_context(&gl_config, &fallback_context_attributes))
        }
        .map_err(|error| format!("failed to create OpenGL context: {error}"))?;

        let size = window.inner_size();
        let gl_surface_attributes = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw_window_handle,
            non_zero(size.width),
            non_zero(size.height),
        );

        let gl_surface = unsafe {
            gl_config
                .display()
                .create_window_surface(&gl_config, &gl_surface_attributes)
        }
        .map_err(|error| format!("failed to create window surface: {error}"))?;

        let gl_context = not_current_context
            .make_current(&gl_surface)
            .map_err(|error| format!("failed to activate OpenGL context: {error}"))?;

        if let Err(error) = gl_surface.set_swap_interval(
            &gl_context,
            SwapInterval::Wait(NonZeroU32::new(1).expect("1 is non-zero")),
        ) {
            tracing::warn!("failed to set vsync: {error}");
        }

        let renderer = Renderer::new(
            &gl_config,
            size.width.max(1),
            size.height.max(1),
            window.scale_factor() as f32,
        )?;
        let playback_engine = Box::new(RodioPlaybackEngine::new()?);
        let resume_load_requested = app_state.position_ms > 0 && app_state.current_file_path.is_some();
        let mut state = Self {
            window,
            gl_context,
            gl_surface,
            renderer,
            database,
            playback_engine,
            loaded_file_path: None,
            resume_load_requested,
            last_playback_persist_at: Instant::now(),
            app_state,
            pending_redraw_reason: None,
            render_count: 0,
        };

        state.load_bookmarks_for_current_book();
        Ok(state)
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }

        self.gl_surface.resize(
            &self.gl_context,
            non_zero(size.width),
            non_zero(size.height),
        );
        self.renderer
            .resize(size.width, size.height, self.window.scale_factor() as f32);
    }

    fn render(&mut self) -> Result<(), String> {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }

        self.render_count = self.render_count.wrapping_add(1);
        tracing::info!(
            render_count = self.render_count,
            reason = self.pending_redraw_reason.take().unwrap_or("window/system"),
            "render"
        );

        self.renderer
            .render(&mut self.app_state, size.width, size.height)?;
        self.window.pre_present_notify();
        self.gl_surface
            .swap_buffers(&self.gl_context)
            .map_err(|error| format!("failed to swap buffers: {error}"))
    }

    fn request_redraw(&mut self, reason: &'static str) {
        self.pending_redraw_reason = Some(reason);
        self.window.request_redraw();
    }

    fn dispatch_action(&mut self, action: UiAction) -> bool {
        let state_changed = match action {
            UiAction::ContinueListening
            | UiAction::PlayPause
            | UiAction::SeekBackward
            | UiAction::SeekForward
            | UiAction::AddBookmark => {
                self.app_state.interaction.last_action = Some(action);
                false
            }
            _ => self.app_state.apply_action(action),
        };

        match action {
            UiAction::SelectBook(book_id) => {
                self.app_state.current_book_id = Some(book_id);
                self.app_state.current_file_path = self
                    .database
                    .first_file_path_for_audiobook(book_id)
                    .ok()
                    .flatten();
                self.loaded_file_path = None;
                self.resume_load_requested = false;
                self.app_state.is_playing = false;
                self.app_state.playback_status = PlaybackStatus::Idle;

                if let Ok(Some(playback)) = self.database.load_playback_state_for_audiobook(book_id) {
                    self.app_state.position_ms = playback.position_ms;
                    self.app_state.playback_last_played_at = Some(playback.last_played_at);
                    self.app_state.playback_completed = playback.completed;
                    self.resume_load_requested = true;
                } else {
                    self.app_state.position_ms = 0;
                    self.app_state.playback_last_played_at = None;
                    self.app_state.playback_completed = false;
                }
                self.load_bookmarks_for_current_book();

                if let Some(path) = &self.app_state.current_file_path {
                    tracing::info!(book_id, path = %path, "selected audio path");
                } else {
                    tracing::warn!(book_id, "selected book has no audio file path");
                }
                state_changed || self.app_state.current_file_path.is_some()
            }
            UiAction::ContinueListening => {
                let mut changed = false;
                let book_id = if let Ok(Some(latest)) = self.database.load_latest_playback_state() {
                    if self.app_state.current_book_id != Some(latest.audiobook_id) {
                        self.app_state.current_book_id = Some(latest.audiobook_id);
                        changed = true;
                    }
                    if self.app_state.selected_book_id != Some(latest.audiobook_id) {
                        self.app_state.selected_book_id = Some(latest.audiobook_id);
                        changed = true;
                    }
                    self.app_state.position_ms = latest.position_ms;
                    self.app_state.playback_last_played_at = Some(latest.last_played_at);
                    self.app_state.playback_completed = latest.completed;
                    Some(latest.audiobook_id)
                } else {
                    self.app_state.current_book_id.or(self.app_state.selected_book_id)
                };

                if let Some(book_id) = book_id {
                    self.app_state.current_file_path = self
                        .database
                        .first_file_path_for_audiobook(book_id)
                        .ok()
                        .flatten();
                    self.resume_load_requested = true;
                    if let Some(path) = &self.app_state.current_file_path {
                        tracing::info!(book_id, path = %path, "continue listening audio path");
                    }
                }

                if self.app_state.current_page != AppPage::Player {
                    self.app_state.current_page = AppPage::Player;
                    changed = true;
                }

                self.loaded_file_path = None;
                self.app_state.is_playing = false;
                self.app_state.playback_status = PlaybackStatus::Idle;
                self.load_bookmarks_for_current_book();
                state_changed || changed
            }
            UiAction::PlayPause => {
                let playback_changed = self.toggle_playback();
                self.sync_playback_state_to_db("playback toggle");
                state_changed || playback_changed
            }
            UiAction::SeekBackward => {
                let changed = self.seek_playback_backward(15);
                self.sync_playback_state_to_db("seek backward");
                state_changed || changed
            }
            UiAction::SeekForward => {
                let changed = self.seek_playback_forward(30);
                self.sync_playback_state_to_db("seek forward");
                state_changed || changed
            }
            UiAction::AddBookmark => {
                if self.add_bookmark_at_current_position() {
                    true
                } else {
                    state_changed
                }
            }
            _ => state_changed,
        }
    }

    fn ensure_loaded_current_file(&mut self) -> bool {
        let Some(path) = self.app_state.current_file_path.clone() else {
            let message = "No audio file path is available for the selected book.".to_owned();
            tracing::warn!("{message}");
            self.app_state.playback_status = PlaybackStatus::Error(message);
            self.app_state.is_playing = false;
            return false;
        };

        if self.loaded_file_path.as_deref() != Some(path.as_str()) || self.resume_load_requested {
            self.app_state.playback_status = PlaybackStatus::Loading;
            tracing::info!(path = %path, "playback load");
            if let Err(error) = self.playback_engine.load(&path) {
                tracing::warn!(path = %path, "{error}");
                self.loaded_file_path = None;
                self.resume_load_requested = false;
                self.app_state.playback_status = PlaybackStatus::Error(error);
                self.app_state.is_playing = false;
                return false;
            }
            self.loaded_file_path = Some(path.clone());

            if self.app_state.position_ms > 0 {
                let mut resume_position_ms = self.app_state.position_ms.max(0);
                if self.resume_load_requested {
                    resume_position_ms =
                        (resume_position_ms - RESUME_REWIND_SECONDS * 1000).max(0);
                }
                let seconds = (resume_position_ms / 1000) as u64;
                if seconds > 0 {
                    if let Err(error) = self.playback_engine.seek_forward(seconds) {
                        tracing::warn!(seconds, "{error}");
                        self.app_state.playback_status =
                            PlaybackStatus::Error(format!("Failed to resume from saved position: {error}"));
                        self.app_state.position_ms = 0;
                    } else {
                        self.app_state.position_ms = resume_position_ms;
                    }
                }
            }
            self.resume_load_requested = false;
            if !matches!(self.app_state.playback_status, PlaybackStatus::Error(_)) {
                self.app_state.playback_status = PlaybackStatus::Paused;
            }
        }

        true
    }

    fn toggle_playback(&mut self) -> bool {
        if self.app_state.current_file_path.is_none() {
            if let Some(book_id) = self.app_state.current_book_id.or(self.app_state.selected_book_id) {
                self.app_state.current_file_path = self
                    .database
                    .first_file_path_for_audiobook(book_id)
                    .ok()
                    .flatten();
            }
        }

        if !self.ensure_loaded_current_file() {
            self.app_state.is_playing = false;
            return false;
        }

        let was_playing = self.playback_engine.is_playing();
        if let Err(error) = self.playback_engine.toggle() {
            tracing::warn!("{error}");
            self.app_state.playback_status = PlaybackStatus::Error(error);
            self.app_state.is_playing = false;
            return false;
        }

        self.app_state.is_playing = self.playback_engine.is_playing();
        self.app_state.position_ms = self.playback_engine.current_position_ms();
        self.app_state.playback_status = if self.app_state.is_playing {
            tracing::info!("playback play");
            PlaybackStatus::Playing
        } else {
            if was_playing {
                tracing::info!("playback pause");
            }
            PlaybackStatus::Paused
        };
        true
    }

    fn seek_playback_forward(&mut self, seconds: u64) -> bool {
        if !self.ensure_loaded_current_file() {
            return false;
        }
        if let Err(error) = self.playback_engine.seek_forward(seconds) {
            tracing::warn!("{error}");
            self.app_state.playback_status = PlaybackStatus::Error(error);
            return false;
        }
        self.app_state.position_ms = self.playback_engine.current_position_ms();
        tracing::info!(seconds, position_ms = self.app_state.position_ms, "playback seek forward");
        true
    }

    fn seek_playback_backward(&mut self, seconds: u64) -> bool {
        if !self.ensure_loaded_current_file() {
            return false;
        }
        if let Err(error) = self.playback_engine.seek_backward(seconds) {
            tracing::warn!("{error}");
            self.app_state.playback_status = PlaybackStatus::Error(error);
            return false;
        }
        self.app_state.position_ms = self.playback_engine.current_position_ms();
        tracing::info!(
            seconds,
            position_ms = self.app_state.position_ms,
            "playback seek backward"
        );
        true
    }

    fn sync_playback_state_to_db(&mut self, reason: &'static str) {
        self.update_playback_completed_placeholder();
        if let Some(book_id) = self.app_state.current_book_id.or(self.app_state.selected_book_id) {
            if let Err(error) = self
                .database
                .upsert_playback_state_minimal(
                    book_id,
                    self.app_state.position_ms,
                    self.app_state.playback_completed,
                )
            {
                tracing::warn!("{error}");
            } else {
                self.last_playback_persist_at = Instant::now();
                tracing::info!(
                    reason,
                    book_id,
                    position_ms = self.app_state.position_ms,
                    "playback state saved"
                );
            }
        }
    }

    fn update_playback_completed_placeholder(&mut self) {
        let Some(book_id) = self.app_state.current_book_id.or(self.app_state.selected_book_id) else {
            self.app_state.playback_completed = false;
            return;
        };

        let Some(book) = self.app_state.books.iter().find(|book| book.id == book_id) else {
            self.app_state.playback_completed = false;
            return;
        };

        if book.total_duration_ms <= 0 {
            // Placeholder: if we cannot trust duration, we keep completion false for now.
            self.app_state.playback_completed = false;
            return;
        }

        let remaining_ms = (book.total_duration_ms - self.app_state.position_ms).max(0);
        self.app_state.playback_completed = remaining_ms <= 15_000;
    }

    fn persist_before_exit(&mut self) {
        if self.loaded_file_path.is_some() {
            self.app_state.position_ms = self.playback_engine.current_position_ms();
            self.app_state.is_playing = self.playback_engine.is_playing();
            self.app_state.playback_status = if self.app_state.is_playing {
                PlaybackStatus::Playing
            } else {
                PlaybackStatus::Paused
            };
        }
        self.sync_playback_state_to_db("app close");
        tracing::info!(
            book_id = ?self.app_state.current_book_id.or(self.app_state.selected_book_id),
            position_ms = self.app_state.position_ms,
            "playback state persisted on close"
        );
    }

    fn handle_periodic_playback_tick(&mut self, now: Instant) {
        if self.app_state.expire_transient_message_if_needed(now) {
            self.request_redraw("ui state change");
        }

        if !matches!(self.app_state.playback_status, PlaybackStatus::Playing) {
            return;
        }

        if self.loaded_file_path.is_none() {
            return;
        }

        let latest_position = self.playback_engine.current_position_ms().max(0);
        self.app_state.position_ms = latest_position;

        if now.duration_since(self.last_playback_persist_at) >= PLAYBACK_PERSIST_INTERVAL {
            self.sync_playback_state_to_db("periodic");
            tracing::info!(
                position_ms = self.app_state.position_ms,
                "periodic playback position save"
            );
            self.request_redraw("ui state change");
        }
    }

    fn add_bookmark_at_current_position(&mut self) -> bool {
        let Some(book_id) = self.app_state.current_book_id.or(self.app_state.selected_book_id) else {
            return false;
        };

        let position_ms = if self.loaded_file_path.is_some() {
            self.playback_engine.current_position_ms().max(0)
        } else {
            self.app_state.position_ms.max(0)
        };
        self.app_state.position_ms = position_ms;

        let note = format!("Bookmark at {}s", position_ms / 1000);
        if let Err(error) = self.database.create_bookmark(book_id, position_ms, &note) {
            tracing::warn!(book_id, position_ms, "{error}");
            self.app_state.playback_status = PlaybackStatus::Error(error);
            return true;
        }

        self.load_bookmarks_for_current_book();
        self.app_state
            .set_transient_message("Bookmark saved", Duration::from_secs(BOOKMARK_TOAST_SECONDS));
        tracing::info!(book_id, position_ms, "bookmark saved");
        true
    }

    fn load_bookmarks_for_current_book(&mut self) {
        let Some(book_id) = self.app_state.current_book_id.or(self.app_state.selected_book_id) else {
            self.app_state.bookmarks.clear();
            self.app_state.bookmark_count = 0;
            return;
        };

        match self.database.list_bookmarks(book_id) {
            Ok(bookmarks) => {
                self.app_state.bookmark_count = bookmarks.len() as u32;
                self.app_state.bookmarks = bookmarks;
            }
            Err(error) => {
                tracing::warn!(book_id, "{error}");
                self.app_state.bookmarks.clear();
                self.app_state.bookmark_count = 0;
            }
        }
    }
}

fn choose_gl_config(configs: Box<dyn Iterator<Item = Config> + '_>) -> Config {
    configs
        .max_by_key(|config| config.num_samples())
        .expect("at least one OpenGL config should be available")
}

fn non_zero(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value.max(1)).expect("value is forced to be non-zero")
}
