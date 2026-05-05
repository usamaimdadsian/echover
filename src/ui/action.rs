#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryFilter {
    All,
    InProgress,
    NotStarted,
    Finished,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    NavigateHome,
    NavigateLibrary,
    NavigatePlayer,
    NavigateSettings,
    ContinueListening,
    SelectBook(u64),
    SetFilter(LibraryFilter),
    PlayPause,
    SeekBackward,
    SeekForward,
    AddBookmark,
    DummyClick,
}
