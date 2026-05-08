use crate::ui::action::{LibraryFilter, UiAction};
use crate::ui::data::Library;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppPage {
    Home,
    Library,
    Player,
    Bookmarks,
    Settings,
    BookDetail(u64),
}

pub struct AppState {
    pub current_page: AppPage,
    pub library_filter: LibraryFilter,
    pub search_query: String,
    pub search_focused: bool,
    pub library: Library,
    /// Audiobook the playback engine has loaded (if any). Drives "live"
    /// progress display on the Player page.
    pub loaded_audiobook_id: Option<u64>,
    pub is_playing: bool,
    pub playback_position_ms: i64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_page: AppPage::Home,
            library_filter: LibraryFilter::All,
            search_query: String::new(),
            search_focused: false,
            library: Library::default(),
            loaded_audiobook_id: None,
            is_playing: false,
            playback_position_ms: 0,
        }
    }
}

impl AppState {
    pub fn with_library(library: Library) -> Self {
        Self {
            library,
            ..Self::default()
        }
    }

    pub fn apply_action(&mut self, action: UiAction) -> bool {
        if let Some(new_page) = page_for_action(action) {
            if self.current_page == new_page {
                return false;
            }
            self.current_page = new_page;
            tracing::info!(?new_page, "navigated");
            return true;
        }

        match action {
            UiAction::SetFilter(filter) => {
                if self.library_filter == filter {
                    return false;
                }
                self.library_filter = filter;
                tracing::info!(?filter, "library filter changed");
                true
            }
            UiAction::FocusSearch => {
                if self.search_focused {
                    return false;
                }
                self.search_focused = true;
                true
            }
            UiAction::ClearSearch => {
                let had_query = !self.search_query.is_empty();
                let was_focused = self.search_focused;
                self.search_query.clear();
                self.search_focused = false;
                had_query || was_focused
            }
            _ => false,
        }
    }

    pub fn search_input(&mut self, ch: char) -> bool {
        if !self.search_focused || ch.is_control() {
            return false;
        }
        self.search_query.push(ch);
        true
    }

    pub fn search_backspace(&mut self) -> bool {
        if !self.search_focused || self.search_query.is_empty() {
            return false;
        }
        self.search_query.pop();
        true
    }

    pub fn unfocus_search(&mut self) -> bool {
        if !self.search_focused {
            return false;
        }
        self.search_focused = false;
        true
    }
}

fn page_for_action(action: UiAction) -> Option<AppPage> {
    Some(match action {
        UiAction::NavigateHome => AppPage::Home,
        UiAction::NavigateLibrary => AppPage::Library,
        UiAction::NavigatePlayer => AppPage::Player,
        UiAction::NavigateBookmarks => AppPage::Bookmarks,
        UiAction::NavigateSettings => AppPage::Settings,
        UiAction::SelectBook(id) => AppPage::BookDetail(id),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_actions_switch_page_and_signal_change() {
        let mut state = AppState::default();
        assert!(state.apply_action(UiAction::NavigateLibrary));
        assert_eq!(state.current_page, AppPage::Library);
        assert!(!state.apply_action(UiAction::NavigateLibrary));
    }

    #[test]
    fn select_book_routes_to_book_detail() {
        let mut state = AppState::default();
        assert!(state.apply_action(UiAction::SelectBook(7)));
        assert_eq!(state.current_page, AppPage::BookDetail(7));
        assert!(state.apply_action(UiAction::SelectBook(3)));
        assert_eq!(state.current_page, AppPage::BookDetail(3));
        assert!(!state.apply_action(UiAction::SelectBook(3)));
    }

    #[test]
    fn search_focus_input_and_clear() {
        let mut state = AppState::default();
        assert!(state.apply_action(UiAction::FocusSearch));
        assert!(state.search_input('h'));
        assert!(state.search_input('i'));
        assert_eq!(state.search_query, "hi");
        assert!(state.search_backspace());
        assert_eq!(state.search_query, "h");
        assert!(state.apply_action(UiAction::ClearSearch));
        assert!(!state.search_focused);
        assert!(state.search_query.is_empty());
    }
}
