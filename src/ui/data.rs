use crate::persistence::db::Database;

/// Read-only snapshot of the library that the UI renders against. Built once
/// at startup from the SQLite DB; rebuilt when a user-visible mutation lands
/// (Phase 7+). Holds owned strings — pages just borrow from it.
#[derive(Debug, Default)]
pub struct Library {
    pub books: Vec<DisplayBook>,
    pub bookmarks: Vec<DisplayBookmark>,
    pub folders: Vec<DisplayFolder>,
    pub current_listening_id: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct DisplayBook {
    pub id: u64,
    pub title: String,
    pub author: String,
    pub narrator: String,
    pub current_chapter: String,
    pub duration_text: String,
    pub remaining_text: String,
    pub progress: f32,
    pub total_duration_ms: i64,
    pub completed: bool,
    pub chapters: Vec<DisplayChapter>,
}

#[derive(Debug, Clone)]
pub struct DisplayChapter {
    pub index: u32,
    pub title: String,
    pub duration_text: String,
}

#[derive(Debug, Clone)]
pub struct DisplayBookmark {
    pub book_id: u64,
    pub book_title: String,
    pub note: String,
    pub timestamp: String,
    pub position_ms: i64,
}

#[derive(Debug, Clone)]
pub struct DisplayFolder {
    pub label: String,
    pub path: String,
    pub book_count: u32,
}

impl Library {
    pub fn from_db(db: &Database) -> Result<Self, String> {
        let books_raw = db.load_audiobooks()?;
        let latest = db.load_latest_playback_state()?;
        let current_listening_id = latest.as_ref().map(|p| p.audiobook_id).or_else(|| {
            // Fall back to the first audiobook so Home/Player have something
            // to render when no playback has been recorded yet.
            books_raw.first().map(|b| b.id)
        });

        let mut books = Vec::with_capacity(books_raw.len());
        for raw in &books_raw {
            let chapters_raw = db.load_files_for_audiobook(raw.id)?;
            let chapters: Vec<DisplayChapter> = chapters_raw
                .into_iter()
                .enumerate()
                .map(|(i, file)| DisplayChapter {
                    index: (i + 1) as u32,
                    title: chapter_title_from_path(&file.path),
                    duration_text: format_duration_short(file.duration_ms),
                })
                .collect();

            let position_ms = if Some(raw.id) == latest.as_ref().map(|p| p.audiobook_id) {
                latest.as_ref().map(|p| p.position_ms).unwrap_or(0)
            } else {
                0
            };

            books.push(DisplayBook {
                id: raw.id,
                title: raw.title.clone(),
                author: raw.author.clone(),
                narrator: raw.narrator.clone(),
                current_chapter: raw.current_chapter.clone(),
                duration_text: raw.duration_text.clone(),
                remaining_text: format_remaining(raw.total_duration_ms, position_ms),
                progress: raw.progress,
                total_duration_ms: raw.total_duration_ms,
                completed: raw.completed,
                chapters,
            });
        }

        let bookmarks = db
            .load_all_bookmarks_with_titles()?
            .into_iter()
            .map(|b| DisplayBookmark {
                book_id: b.audiobook_id,
                book_title: b.book_title,
                note: b.note,
                timestamp: format_position(b.position_ms),
                position_ms: b.position_ms,
            })
            .collect();

        let folders = db
            .load_library_folders_with_counts()?
            .into_iter()
            .map(|f| DisplayFolder {
                label: folder_label(&f.path),
                path: f.path,
                book_count: f.book_count,
            })
            .collect();

        Ok(Self {
            books,
            bookmarks,
            folders,
            current_listening_id,
        })
    }

    pub fn find_book(&self, id: u64) -> Option<&DisplayBook> {
        self.books.iter().find(|b| b.id == id)
    }

    pub fn current_listening(&self) -> Option<&DisplayBook> {
        self.current_listening_id
            .and_then(|id| self.find_book(id))
            .or_else(|| self.books.first())
    }

    #[cfg(test)]
    pub fn sample_for_tests() -> Self {
        let chapters = vec![
            DisplayChapter {
                index: 1,
                title: "Opening".into(),
                duration_text: "12m".into(),
            },
            DisplayChapter {
                index: 2,
                title: "Riddles".into(),
                duration_text: "34m".into(),
            },
        ];
        let books = vec![
            DisplayBook {
                id: 1,
                title: "Sample Book".into(),
                author: "Sample Author".into(),
                narrator: "Sample Narrator".into(),
                current_chapter: "Chapter 1".into(),
                duration_text: "5h 00m".into(),
                remaining_text: "3h 00m left".into(),
                progress: 0.4,
                total_duration_ms: 5 * 60 * 60 * 1000,
                completed: false,
                chapters,
            },
            DisplayBook {
                id: 2,
                title: "Another Book".into(),
                author: "Another Author".into(),
                narrator: "Another Narrator".into(),
                current_chapter: "Chapter 1".into(),
                duration_text: "4h 30m".into(),
                remaining_text: "4h 30m left".into(),
                progress: 0.0,
                total_duration_ms: 4 * 60 * 60 * 1000 + 30 * 60 * 1000,
                completed: false,
                chapters: Vec::new(),
            },
        ];
        Self {
            books,
            bookmarks: vec![DisplayBookmark {
                book_id: 1,
                book_title: "Sample Book".into(),
                note: "Sample bookmark".into(),
                timestamp: "0:12:34".into(),
                position_ms: 754_000,
            }],
            folders: vec![DisplayFolder {
                label: "library".into(),
                path: "library".into(),
                book_count: 2,
            }],
            current_listening_id: Some(1),
        }
    }
}

fn chapter_title_from_path(path: &str) -> String {
    let stem = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path);
    stem.replace('_', " ").replace('-', " ")
}

fn folder_label(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| path.to_string())
}

fn format_duration_short(ms: i64) -> String {
    let total_minutes = (ms / 60_000).max(0);
    if total_minutes >= 60 {
        let h = total_minutes / 60;
        let m = total_minutes % 60;
        format!("{h}h {m:02}m")
    } else {
        format!("{total_minutes}m")
    }
}

fn format_remaining(total_ms: i64, position_ms: i64) -> String {
    if total_ms <= 0 {
        return String::new();
    }
    let remaining_ms = (total_ms - position_ms).max(0);
    if position_ms > 0 && remaining_ms == 0 {
        return "Finished".to_owned();
    }
    let total_minutes = remaining_ms / 60_000;
    let h = total_minutes / 60;
    let m = total_minutes % 60;
    if h > 0 {
        format!("{h}h {m:02}m left")
    } else {
        format!("{m}m left")
    }
}

pub fn format_position(ms: i64) -> String {
    let total_seconds = (ms / 1000).max(0);
    let h = total_seconds / 3600;
    let m = (total_seconds % 3600) / 60;
    let s = total_seconds % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}
