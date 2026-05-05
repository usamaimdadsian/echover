#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Bookmark {
    pub id: u64,
    pub audiobook_id: u64,
    pub position_ms: i64,
    pub note: String,
    pub created_at: String,
}
