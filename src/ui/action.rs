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
    NavigateBookmarks,
    NavigateSettings,
    ContinueListening,
    SelectBook(u64),
    SetFilter(LibraryFilter),
    FocusSearch,
    ClearSearch,
    PlayPause,
    SeekBackward,
    SeekForward,
    AddBookmark,
    /// Jump playback to the start of a 1-based chapter inside `book_id`.
    SelectChapter(u64, u32),
    /// Jump playback to a saved bookmark position inside `book_id`.
    JumpToBookmark(u64, i64),
    /// Open a native folder picker, scan the chosen folder, re-hydrate library.
    AddLibraryFolder,
}
