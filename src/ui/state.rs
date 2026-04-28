use crate::ui::action::UiAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppPage {
    Home,
    Library,
    Player,
    Settings,
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
    pub mouse_position: (f32, f32),
    pub interaction: UiInteractionState,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_page: AppPage::Home,
            selected_book_id: None,
            mouse_position: (0.0, 0.0),
            interaction: UiInteractionState::default(),
        }
    }
}

impl AppState {
    pub fn apply_action(&mut self, action: UiAction) -> bool {
        self.interaction.last_action = Some(action);

        let next_page = match action {
            UiAction::NavigateHome => Some(AppPage::Home),
            UiAction::NavigateLibrary => Some(AppPage::Library),
            UiAction::NavigatePlayer => Some(AppPage::Player),
            UiAction::NavigateSettings => Some(AppPage::Settings),
            UiAction::DummyClick => None,
        };

        if let Some(next_page) = next_page {
            if self.current_page != next_page {
                self.current_page = next_page;
                self.selected_book_id = None;
                return true;
            }
        }

        false
    }
}
