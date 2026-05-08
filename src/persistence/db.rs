use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    domain::audiobook::Audiobook,
    domain::audiobook_file::AudiobookFile,
    domain::bookmark::Bookmark,
    library::scanner::{scan_library_folder, ScannedAudiobook},
};

pub struct Database {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct MinimalPlaybackState {
    pub audiobook_id: u64,
    pub position_ms: i64,
    pub last_played_at: String,
    pub completed: bool,
}

#[derive(Debug, Clone)]
pub struct BookmarkWithTitle {
    pub audiobook_id: u64,
    pub book_title: String,
    pub note: String,
    pub position_ms: i64,
}

#[derive(Debug, Clone)]
pub struct LibraryFolderWithCount {
    pub path: String,
    pub book_count: u32,
}

impl Database {
    pub fn open_default() -> Result<Self, String> {
        let path = default_database_path()?;
        let conn =
            Connection::open(path).map_err(|error| format!("failed to open sqlite db: {error}"))?;
        Ok(Self { conn })
    }

    pub fn initialize(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "
                PRAGMA foreign_keys = ON;

                CREATE TABLE IF NOT EXISTS audiobooks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    title TEXT NOT NULL,
                    author TEXT NOT NULL,
                    narrator TEXT NOT NULL,
                    description TEXT NOT NULL,
                    cover_path TEXT NOT NULL,
                    total_duration_ms INTEGER NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    source_folder_path TEXT NOT NULL UNIQUE
                );

                CREATE TABLE IF NOT EXISTS audiobook_files (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    audiobook_id INTEGER NOT NULL,
                    path TEXT NOT NULL UNIQUE,
                    file_index INTEGER NOT NULL,
                    duration_ms INTEGER NOT NULL,
                    format TEXT NOT NULL,
                    disc_number INTEGER NOT NULL,
                    track_number INTEGER NOT NULL,
                    FOREIGN KEY (audiobook_id) REFERENCES audiobooks(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS playback_states (
                    audiobook_id INTEGER PRIMARY KEY,
                    position_ms INTEGER NOT NULL,
                    chapter_id INTEGER NOT NULL,
                    playback_speed REAL NOT NULL,
                    volume REAL NOT NULL,
                    last_played_at TEXT NOT NULL,
                    completed INTEGER NOT NULL,
                    FOREIGN KEY (audiobook_id) REFERENCES audiobooks(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS bookmarks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    audiobook_id INTEGER NOT NULL,
                    position_ms INTEGER NOT NULL,
                    note TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (audiobook_id) REFERENCES audiobooks(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS library_folders (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    path TEXT NOT NULL UNIQUE,
                    is_watched INTEGER NOT NULL,
                    last_scanned_at TEXT NOT NULL
                );
                ",
            )
            .map_err(|error| format!("failed to initialize schema: {error}"))
    }

    pub fn scan_and_ingest_from_env_or_default(&self) -> Result<(), String> {
        let default_path = PathBuf::from("library");
        let scan_root = env::var("ECHOVER_LIBRARY_PATH")
            .map(PathBuf::from)
            .unwrap_or(default_path);

        if !scan_root.exists() {
            return Ok(());
        }
        self.scan_and_ingest_from(&scan_root)
    }

    pub fn scan_and_ingest_from(&self, scan_root: &Path) -> Result<(), String> {
        self.upsert_library_folder(scan_root)?;
        let scanned = scan_library_folder(scan_root);
        for audiobook in scanned {
            self.upsert_scanned_audiobook(audiobook)?;
        }
        Ok(())
    }

    pub fn seed_mock_if_empty(&self) -> Result<(), String> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM audiobooks", [], |row| row.get(0))
            .map_err(|error| format!("failed to check audiobook count: {error}"))?;

        if count > 0 {
            return Ok(());
        }

        let now = now_string();
        let seed_books = [
            ("The Fellowship of the Ring", "J.R.R. Tolkien", 19 * 60 + 7),
            ("Project Hail Mary", "Andy Weir", 16 * 60 + 10),
            ("Atomic Habits", "James Clear", 5 * 60 + 35),
            ("Dune", "Frank Herbert", 21 * 60 + 2),
            ("Sapiens", "Yuval Noah Harari", 15 * 60 + 18),
        ];

        for (index, (title, author, total_minutes)) in seed_books.into_iter().enumerate() {
            let folder_key = format!("__seed__/book-{}", index + 1);
            self.conn
                .execute(
                    "
                    INSERT INTO audiobooks (
                        title, author, narrator, description, cover_path,
                        total_duration_ms, created_at, updated_at, source_folder_path
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                    ",
                    params![
                        title,
                        author,
                        "Unknown Narrator",
                        "Seeded fallback audiobook.",
                        "",
                        (total_minutes as i64) * 60_000,
                        now,
                        now,
                        folder_key
                    ],
                )
                .map_err(|error| format!("failed to seed audiobook: {error}"))?;

            let audiobook_id = self.conn.last_insert_rowid();
            let fake_file = format!("{folder_key}/track-01.mp3");
            self.conn
                .execute(
                    "
                    INSERT INTO audiobook_files (
                        audiobook_id, path, file_index, duration_ms, format, disc_number, track_number
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    ",
                    params![
                        audiobook_id,
                        fake_file,
                        0_i64,
                        (total_minutes as i64) * 60_000,
                        "mp3",
                        1_i64,
                        1_i64
                    ],
                )
                .map_err(|error| format!("failed to seed audiobook_file: {error}"))?;
        }

        Ok(())
    }

    pub fn load_audiobooks(&self) -> Result<Vec<Audiobook>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "
                SELECT
                    a.id, a.title, a.author, a.narrator, a.description, a.cover_path,
                    a.total_duration_ms, a.created_at, a.updated_at,
                    COALESCE(p.position_ms, 0) AS position_ms,
                    COALESCE(p.completed, 0) AS completed
                FROM audiobooks a
                LEFT JOIN playback_states p ON p.audiobook_id = a.id
                ORDER BY COALESCE(p.last_played_at, a.updated_at) DESC, a.id DESC
                ",
            )
            .map_err(|error| format!("failed to prepare audiobook query: {error}"))?;

        let rows = stmt
            .query_map([], |row| {
                let total_duration_ms: i64 = row.get(6)?;
                let position_ms: i64 = row.get(9)?;
                let completed: i64 = row.get(10)?;
                let progress = if total_duration_ms > 0 {
                    (position_ms as f32 / total_duration_ms as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                Ok(Audiobook {
                    id: row.get::<_, i64>(0)? as u64,
                    title: row.get(1)?,
                    author: row.get(2)?,
                    narrator: row.get(3)?,
                    description: row.get(4)?,
                    cover_path: row.get(5)?,
                    total_duration_ms,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    progress,
                    duration_text: format_duration_text(total_duration_ms),
                    current_chapter: "Chapter 1".to_owned(),
                    completed: completed != 0,
                })
            })
            .map_err(|error| format!("failed to query audiobooks: {error}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to decode audiobooks: {error}"))
    }

    /// Return the file path for a given 1-based chapter index. Chapters map
    /// directly to `audiobook_files` rows ordered by `(disc, file_index)`.
    pub fn file_path_for_chapter(
        &self,
        audiobook_id: u64,
        chapter_index_1_based: u32,
    ) -> Result<Option<String>, String> {
        if chapter_index_1_based == 0 {
            return Ok(None);
        }
        let offset = (chapter_index_1_based - 1) as i64;
        self.conn
            .query_row(
                "
                SELECT path
                FROM audiobook_files
                WHERE audiobook_id = ?1
                ORDER BY disc_number ASC, file_index ASC, track_number ASC, id ASC
                LIMIT 1 OFFSET ?2
                ",
                params![audiobook_id as i64, offset],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("failed to query chapter file path: {error}"))
    }

    pub fn load_files_for_audiobook(&self, audiobook_id: u64) -> Result<Vec<AudiobookFile>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "
                SELECT id, audiobook_id, path, file_index, duration_ms, format,
                       disc_number, track_number
                FROM audiobook_files
                WHERE audiobook_id = ?1
                ORDER BY disc_number ASC, file_index ASC, track_number ASC, id ASC
                ",
            )
            .map_err(|error| format!("failed to prepare audiobook_files query: {error}"))?;

        let rows = stmt
            .query_map(params![audiobook_id as i64], |row| {
                Ok(AudiobookFile {
                    id: row.get::<_, i64>(0)? as u64,
                    audiobook_id: row.get::<_, i64>(1)? as u64,
                    path: row.get(2)?,
                    file_index: row.get(3)?,
                    duration_ms: row.get(4)?,
                    format: row.get(5)?,
                    disc_number: row.get(6)?,
                    track_number: row.get(7)?,
                })
            })
            .map_err(|error| format!("failed to query audiobook_files: {error}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to decode audiobook_files: {error}"))
    }

    pub fn load_all_bookmarks_with_titles(&self) -> Result<Vec<BookmarkWithTitle>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "
                SELECT b.audiobook_id, a.title, b.note, b.position_ms
                FROM bookmarks b
                JOIN audiobooks a ON a.id = b.audiobook_id
                ORDER BY b.created_at DESC, b.id DESC
                ",
            )
            .map_err(|error| format!("failed to prepare bookmarks-with-titles query: {error}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(BookmarkWithTitle {
                    audiobook_id: row.get::<_, i64>(0)? as u64,
                    book_title: row.get(1)?,
                    note: row.get(2)?,
                    position_ms: row.get(3)?,
                })
            })
            .map_err(|error| format!("failed to query bookmarks-with-titles: {error}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to decode bookmarks-with-titles: {error}"))
    }

    pub fn load_library_folders_with_counts(
        &self,
    ) -> Result<Vec<LibraryFolderWithCount>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "
                SELECT lf.path,
                       (SELECT COUNT(*)
                          FROM audiobooks a
                         WHERE a.source_folder_path = lf.path
                            OR a.source_folder_path LIKE lf.path || '/%') AS book_count
                FROM library_folders lf
                ORDER BY lf.path ASC
                ",
            )
            .map_err(|error| format!("failed to prepare library_folders query: {error}"))?;
        let rows = stmt
            .query_map([], |row| {
                let count: i64 = row.get(1)?;
                Ok(LibraryFolderWithCount {
                    path: row.get(0)?,
                    book_count: count.max(0) as u32,
                })
            })
            .map_err(|error| format!("failed to query library_folders: {error}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to decode library_folders: {error}"))
    }

    pub fn first_file_path_for_audiobook(&self, audiobook_id: u64) -> Result<Option<String>, String> {
        self.conn
            .query_row(
                "
                SELECT path
                FROM audiobook_files
                WHERE audiobook_id = ?1
                ORDER BY file_index ASC, id ASC
                LIMIT 1
                ",
                params![audiobook_id as i64],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("failed to query first audiobook file path: {error}"))
    }

    pub fn upsert_playback_state_minimal(
        &self,
        audiobook_id: u64,
        position_ms: i64,
        completed: bool,
    ) -> Result<(), String> {
        let now = now_string();
        self.conn
            .execute(
                "
                INSERT INTO playback_states (
                    audiobook_id, position_ms, chapter_id, playback_speed, volume, last_played_at, completed
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ON CONFLICT(audiobook_id) DO UPDATE SET
                    position_ms = excluded.position_ms,
                    playback_speed = excluded.playback_speed,
                    volume = excluded.volume,
                    last_played_at = excluded.last_played_at,
                    completed = excluded.completed
                ",
                params![
                    audiobook_id as i64,
                    position_ms,
                    0_i64,
                    1.0_f32,
                    1.0_f32,
                    now,
                    if completed { 1_i64 } else { 0_i64 },
                ],
            )
            .map_err(|error| format!("failed to upsert playback state: {error}"))?;
        Ok(())
    }

    pub fn load_latest_playback_state(&self) -> Result<Option<MinimalPlaybackState>, String> {
        self.conn
            .query_row(
                "
                SELECT audiobook_id, position_ms, last_played_at, completed
                FROM playback_states
                ORDER BY last_played_at DESC
                LIMIT 1
                ",
                [],
                |row| {
                    Ok(MinimalPlaybackState {
                        audiobook_id: row.get::<_, i64>(0)? as u64,
                        position_ms: row.get(1)?,
                        last_played_at: row.get(2)?,
                        completed: row.get::<_, i64>(3)? != 0,
                    })
                },
            )
            .optional()
            .map_err(|error| format!("failed to load latest playback state: {error}"))
    }

    pub fn load_playback_state_for_audiobook(
        &self,
        audiobook_id: u64,
    ) -> Result<Option<MinimalPlaybackState>, String> {
        self.conn
            .query_row(
                "
                SELECT audiobook_id, position_ms, last_played_at, completed
                FROM playback_states
                WHERE audiobook_id = ?1
                LIMIT 1
                ",
                params![audiobook_id as i64],
                |row| {
                    Ok(MinimalPlaybackState {
                        audiobook_id: row.get::<_, i64>(0)? as u64,
                        position_ms: row.get(1)?,
                        last_played_at: row.get(2)?,
                        completed: row.get::<_, i64>(3)? != 0,
                    })
                },
            )
            .optional()
            .map_err(|error| format!("failed to load playback state for audiobook: {error}"))
    }

    pub fn create_bookmark(
        &self,
        audiobook_id: u64,
        position_ms: i64,
        note: &str,
    ) -> Result<(), String> {
        let now = now_string();
        self.conn
            .execute(
                "
                INSERT INTO bookmarks (audiobook_id, position_ms, note, created_at)
                VALUES (?1, ?2, ?3, ?4)
                ",
                params![audiobook_id as i64, position_ms, note, now],
            )
            .map_err(|error| format!("failed to create bookmark: {error}"))?;
        Ok(())
    }

    pub fn list_bookmarks(&self, audiobook_id: u64) -> Result<Vec<Bookmark>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "
                SELECT id, audiobook_id, position_ms, note, created_at
                FROM bookmarks
                WHERE audiobook_id = ?1
                ORDER BY position_ms ASC, id ASC
                ",
            )
            .map_err(|error| format!("failed to prepare bookmarks query: {error}"))?;

        let rows = stmt
            .query_map(params![audiobook_id as i64], |row| {
                Ok(Bookmark {
                    id: row.get::<_, i64>(0)? as u64,
                    audiobook_id: row.get::<_, i64>(1)? as u64,
                    position_ms: row.get(2)?,
                    note: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .map_err(|error| format!("failed to query bookmarks: {error}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to decode bookmarks: {error}"))
    }

    fn upsert_library_folder(&self, root: &Path) -> Result<(), String> {
        let now = now_string();
        self.conn
            .execute(
                "
                INSERT INTO library_folders (path, is_watched, last_scanned_at)
                VALUES (?1, 1, ?2)
                ON CONFLICT(path) DO UPDATE SET
                    is_watched=excluded.is_watched,
                    last_scanned_at=excluded.last_scanned_at
                ",
                params![root.to_string_lossy().to_string(), now],
            )
            .map_err(|error| format!("failed to upsert library folder: {error}"))?;
        Ok(())
    }

    fn upsert_scanned_audiobook(&self, scanned: ScannedAudiobook) -> Result<(), String> {
        if scanned.files.is_empty() {
            return Ok(());
        }

        let now = now_string();
        let audiobook_id = self.find_or_create_audiobook(&scanned, &now)?;

        for (index, file_path) in scanned.files.iter().enumerate() {
            let file_path_str = file_path.to_string_lossy().to_string();
            let exists: Option<i64> = self
                .conn
                .query_row(
                    "SELECT id FROM audiobook_files WHERE path = ?1",
                    params![file_path_str],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|error| format!("failed to check audiobook file dup: {error}"))?;

            if exists.is_some() {
                continue;
            }

            let extension = file_path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("audio")
                .to_ascii_lowercase();

            let estimated_duration = 45_i64 * 60_000;
            self.conn
                .execute(
                    "
                    INSERT INTO audiobook_files (
                        audiobook_id, path, file_index, duration_ms, format, disc_number, track_number
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    ",
                    params![
                        audiobook_id,
                        file_path_str,
                        index as i64,
                        estimated_duration,
                        extension,
                        1_i64,
                        index as i64 + 1
                    ],
                )
                .map_err(|error| format!("failed to insert audiobook file: {error}"))?;
        }

        self.refresh_total_duration(audiobook_id, &now)?;
        Ok(())
    }

    fn find_or_create_audiobook(
        &self,
        scanned: &ScannedAudiobook,
        now: &str,
    ) -> Result<i64, String> {
        let existing_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM audiobooks WHERE source_folder_path = ?1",
                params![scanned.folder_path],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("failed to query audiobook by folder: {error}"))?;

        if let Some(id) = existing_id {
            self.conn
                .execute(
                    "UPDATE audiobooks SET updated_at = ?1 WHERE id = ?2",
                    params![now, id],
                )
                .map_err(|error| format!("failed to update audiobook timestamp: {error}"))?;
            return Ok(id);
        }

        self.conn
            .execute(
                "
                INSERT INTO audiobooks (
                    title, author, narrator, description, cover_path,
                    total_duration_ms, created_at, updated_at, source_folder_path
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ",
                params![
                    scanned.title,
                    "Unknown Author",
                    "Unknown Narrator",
                    "Imported from local folder scan.",
                    "",
                    0_i64,
                    now,
                    now,
                    scanned.folder_path
                ],
            )
            .map_err(|error| format!("failed to insert audiobook: {error}"))?;

        Ok(self.conn.last_insert_rowid())
    }

    fn refresh_total_duration(&self, audiobook_id: i64, now: &str) -> Result<(), String> {
        let total_duration_ms: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(duration_ms), 0) FROM audiobook_files WHERE audiobook_id = ?1",
                params![audiobook_id],
                |row| row.get(0),
            )
            .map_err(|error| format!("failed to sum audiobook duration: {error}"))?;

        self.conn
            .execute(
                "
                UPDATE audiobooks
                SET total_duration_ms = ?1, updated_at = ?2
                WHERE id = ?3
                ",
                params![total_duration_ms, now, audiobook_id],
            )
            .map_err(|error| format!("failed to update audiobook duration: {error}"))?;
        Ok(())
    }
}

fn default_database_path() -> Result<PathBuf, String> {
    let mut dir =
        env::current_dir().map_err(|error| format!("failed to get current dir: {error}"))?;
    dir.push("data");
    fs::create_dir_all(&dir).map_err(|error| format!("failed to create data dir: {error}"))?;
    dir.push("echover.sqlite3");
    Ok(dir)
}

fn now_string() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn format_duration_text(total_duration_ms: i64) -> String {
    let total_minutes = (total_duration_ms / 60_000).max(0);
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    format!("{hours}h {minutes:02}m")
}
