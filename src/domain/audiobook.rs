#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Audiobook {
    pub id: u64,
    pub title: String,
    pub author: String,
    pub narrator: String,
    pub description: String,
    pub cover_path: String,
    pub total_duration_ms: i64,
    pub created_at: String,
    pub updated_at: String,
    pub progress: f32,
    pub duration_text: String,
    pub current_chapter: String,
    pub completed: bool,
}
