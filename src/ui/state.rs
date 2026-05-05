use std::time::{Duration, Instant};

use crate::domain::audiobook::Audiobook;
use crate::domain::bookmark::Bookmark;
use crate::ui::action::{LibraryFilter, UiAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppPage {
    Home,
    Library,
    Player,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackStatus {
    Idle,
    Loading,
    Playing,
    Paused,
    Error(String),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UiInteractionState {
    pub hovered_action: Option<UiAction>,
    pub last_action: Option<UiAction>,
    pub mouse_down: bool,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub current_page: AppPage,
    pub selected_book_id: Option<u64>,
    pub current_book_id: Option<u64>,
    pub current_file_path: Option<String>,
    pub position_ms: i64,
    pub mouse_position: (f32, f32),
    pub interaction: UiInteractionState,
    pub books: Vec<Audiobook>,
    pub library_filter: LibraryFilter,
    pub search_query: String,
    pub is_playing: bool,
    pub playback_status: PlaybackStatus,
    pub playback_last_played_at: Option<String>,
    pub playback_completed: bool,
    pub bookmarks: Vec<Bookmark>,
    pub transient_message: Option<String>,
    pub transient_message_until: Option<Instant>,
    pub home_scroll: f32,
    pub library_scroll: f32,
    pub home_scroll_max: f32,
    pub library_scroll_max: f32,
    pub bookmark_count: u32,
}

impl Default for AppState {
    fn default() -> Self {
        Self::from_books(Vec::new())
    }
}

impl AppState {
    pub fn from_books(books: Vec<Audiobook>) -> Self {
        let selected_book_id = books.first().map(|book| book.id);
        Self {
            current_page: AppPage::Home,
            selected_book_id,
            current_book_id: None,
            current_file_path: None,
            position_ms: 0,
            mouse_position: (0.0, 0.0),
            interaction: UiInteractionState::default(),
            books,
            library_filter: LibraryFilter::All,
            search_query: String::new(),
            is_playing: false,
            playback_status: PlaybackStatus::Idle,
            playback_last_played_at: None,
            playback_completed: false,
            bookmarks: Vec::new(),
            transient_message: None,
            transient_message_until: None,
            home_scroll: 0.0,
            library_scroll: 0.0,
            home_scroll_max: 0.0,
            library_scroll_max: 0.0,
            bookmark_count: 0,
        }
    }
    pub fn apply_action(&mut self, action: UiAction) -> bool {
        self.interaction.last_action = Some(action);

        match action {
            UiAction::NavigateHome => self.navigate_to(AppPage::Home),
            UiAction::NavigateLibrary => self.navigate_to(AppPage::Library),
            UiAction::NavigatePlayer => self.navigate_to(AppPage::Player),
            UiAction::NavigateSettings => self.navigate_to(AppPage::Settings),
            UiAction::ContinueListening => {
                let continue_id = self.continue_book().map(|book| book.id);
                let mut changed = false;

                if self.selected_book_id != continue_id {
                    self.selected_book_id = continue_id;
                    changed = true;
                }
                if self.current_book_id != continue_id {
                    self.current_book_id = continue_id;
                    changed = true;
                }

                changed | self.navigate_to(AppPage::Player)
            }
            UiAction::SelectBook(book_id) => {
                let mut changed = false;
                if self.selected_book_id != Some(book_id) {
                    self.selected_book_id = Some(book_id);
                    changed = true;
                }
                if self.current_book_id != Some(book_id) {
                    self.current_book_id = Some(book_id);
                    changed = true;
                }

                changed | self.navigate_to(AppPage::Player)
            }
            UiAction::SetFilter(filter) => {
                if self.library_filter != filter {
                    self.library_filter = filter;
                    return true;
                }
                false
            }
            UiAction::PlayPause => {
                self.is_playing = !self.is_playing;
                true
            }
            UiAction::SeekBackward => self.bump_selected_progress(-0.02),
            UiAction::SeekForward => self.bump_selected_progress(0.02),
            UiAction::AddBookmark => {
                self.bookmark_count = self.bookmark_count.saturating_add(1);
                true
            }
            UiAction::DummyClick => false,
        }
    }

    pub fn selected_book(&self) -> Option<&Audiobook> {
        self.selected_book_id
            .and_then(|id| self.books.iter().find(|book| book.id == id))
            .or_else(|| self.books.first())
    }

    pub fn continue_book(&self) -> Option<&Audiobook> {
        if let Some(id) = self.current_book_id {
            if let Some(book) = self.books.iter().find(|book| book.id == id) {
                return Some(book);
            }
        }

        self.books
            .iter()
            .find(|book| book.progress > 0.0 && book.progress < 1.0)
            .or_else(|| self.books.first())
    }

    pub fn filtered_books(&self) -> Vec<&Audiobook> {
        let query = self.search_query.trim().to_ascii_lowercase();

        self.books
            .iter()
            .filter(|book| match self.library_filter {
                LibraryFilter::All => true,
                LibraryFilter::InProgress => book.progress > 0.0 && book.progress < 1.0,
                LibraryFilter::NotStarted => book.progress <= 0.0,
                LibraryFilter::Finished => book.progress >= 1.0,
            })
            .filter(|book| {
                if query.is_empty() {
                    return true;
                }

                book.title.to_ascii_lowercase().contains(&query)
                    || book.author.to_ascii_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn append_search_char(&mut self, ch: char) -> bool {
        if self.current_page != AppPage::Library || ch.is_control() {
            return false;
        }

        self.search_query.push(ch);
        self.library_scroll = 0.0;
        true
    }

    pub fn pop_search_char(&mut self) -> bool {
        if self.current_page != AppPage::Library || self.search_query.is_empty() {
            return false;
        }

        self.search_query.pop();
        self.library_scroll = 0.0;
        true
    }

    pub fn clear_transient_ui(&mut self) -> bool {
        let mut changed = false;

        if self.current_page == AppPage::Library && !self.search_query.is_empty() {
            self.search_query.clear();
            self.library_scroll = 0.0;
            changed = true;
        }

        if self.interaction.hovered_action.take().is_some() {
            changed = true;
        }

        self.interaction.mouse_down = false;
        changed
    }

    pub fn set_transient_message(&mut self, message: impl Into<String>, duration: Duration) {
        self.transient_message = Some(message.into());
        self.transient_message_until = Some(Instant::now() + duration);
    }

    pub fn expire_transient_message_if_needed(&mut self, now: Instant) -> bool {
        let Some(until) = self.transient_message_until else {
            return false;
        };

        if now >= until {
            self.transient_message = None;
            self.transient_message_until = None;
            return true;
        }

        false
    }

    pub fn scroll_current_page_by(&mut self, delta: f32) -> bool {
        let before = match self.current_page {
            AppPage::Home => self.home_scroll,
            AppPage::Library => self.library_scroll,
            _ => return false,
        };

        match self.current_page {
            AppPage::Home => {
                self.home_scroll = (self.home_scroll + delta).clamp(0.0, self.home_scroll_max);
            }
            AppPage::Library => {
                self.library_scroll =
                    (self.library_scroll + delta).clamp(0.0, self.library_scroll_max);
            }
            _ => {}
        }

        let after = match self.current_page {
            AppPage::Home => self.home_scroll,
            AppPage::Library => self.library_scroll,
            _ => before,
        };

        (after - before).abs() > f32::EPSILON
    }

    pub fn set_home_scroll_max(&mut self, max: f32) {
        self.home_scroll_max = max.max(0.0);
        self.home_scroll = self.home_scroll.clamp(0.0, self.home_scroll_max);
    }

    pub fn set_library_scroll_max(&mut self, max: f32) {
        self.library_scroll_max = max.max(0.0);
        self.library_scroll = self.library_scroll.clamp(0.0, self.library_scroll_max);
    }

    fn navigate_to(&mut self, page: AppPage) -> bool {
        if self.current_page != page {
            self.current_page = page;
            return true;
        }
        false
    }

    fn bump_selected_progress(&mut self, delta: f32) -> bool {
        let Some(selected_id) = self.selected_book_id else {
            return false;
        };

        let Some(book) = self.books.iter_mut().find(|book| book.id == selected_id) else {
            return false;
        };

        let next = (book.progress + delta).clamp(0.0, 1.0);
        if (next - book.progress).abs() > f32::EPSILON {
            book.progress = next;
            return true;
        }
        false
    }
}
