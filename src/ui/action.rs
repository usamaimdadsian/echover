#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    NavigateHome,
    NavigateLibrary,
    NavigatePlayer,
    NavigateSettings,
    DummyClick,
}
