use crate::persistence::db::Database;
use crate::ui::data::Library;
use crate::ui::state::AppState;
use crate::window::event_loop;

pub fn run() -> Result<(), String> {
    let db = Database::open_default()?;
    db.initialize()?;
    db.scan_and_ingest_from_env_or_default()?;
    db.seed_mock_if_empty()?;

    let library = Library::from_db(&db)?;
    tracing::info!(
        books = library.books.len(),
        bookmarks = library.bookmarks.len(),
        folders = library.folders.len(),
        "library hydrated from sqlite"
    );

    let state = AppState::with_library(library);
    event_loop::run(state, db)
}
