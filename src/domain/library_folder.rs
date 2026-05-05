#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LibraryFolder {
    pub id: u64,
    pub path: String,
    pub is_watched: bool,
    pub last_scanned_at: String,
}
