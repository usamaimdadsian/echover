#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub audiobook_id: u64,
    pub position_ms: i64,
    pub chapter_id: i64,
    pub playback_speed: f32,
    pub volume: f32,
    pub last_played_at: String,
    pub completed: bool,
}
