use crate::window::event_loop;
use crate::{
    persistence::db::Database,
    ui::state::{AppState, PlaybackStatus},
};

pub fn run() -> Result<(), String> {
    let db = Database::open_default()?;
    db.initialize()?;
    db.scan_and_ingest_from_env_or_default()?;
    db.seed_mock_if_empty()?;

    let books = db.load_audiobooks()?;
    let mut app_state = AppState::from_books(books);

    if let Some(playback) = db.load_latest_playback_state()? {
        app_state.current_book_id = Some(playback.audiobook_id);
        app_state.selected_book_id = Some(playback.audiobook_id);
        app_state.position_ms = playback.position_ms;
        app_state.is_playing = false;
        app_state.playback_status = PlaybackStatus::Paused;
        app_state.playback_last_played_at = Some(playback.last_played_at.clone());
        app_state.playback_completed = playback.completed;
        app_state.current_file_path = db.first_file_path_for_audiobook(playback.audiobook_id)?;
        tracing::info!(
            audiobook_id = playback.audiobook_id,
            position_ms = playback.position_ms,
            last_played_at = playback.last_played_at,
            completed = playback.completed,
            "loaded latest playback state"
        );
    }

    event_loop::run(app_state, db)
}
