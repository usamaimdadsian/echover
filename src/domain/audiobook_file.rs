#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AudiobookFile {
    pub id: u64,
    pub audiobook_id: u64,
    pub path: String,
    pub file_index: i64,
    pub duration_ms: i64,
    pub format: String,
    pub disc_number: i64,
    pub track_number: i64,
}
