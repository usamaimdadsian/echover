use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

const AUDIO_EXTS: &[&str] = &["mp3", "m4a", "m4b", "flac", "ogg", "opus", "wav", "aac"];

/// Heuristic shape produced by the scanner: one folder = one audiobook,
/// containing some audio files. The DB layer turns this into rows.
#[derive(Debug, Clone)]
pub struct ScannedAudiobook {
    pub title: String,
    pub folder_path: String,
    pub files: Vec<PathBuf>,
}

/// Walk `root` recursively and group audio files by their immediate parent
/// directory. Each directory that contains at least one audio file becomes a
/// `ScannedAudiobook`. Files within a book are sorted by path so track order
/// follows filesystem ordering.
pub fn scan_library_folder(root: &Path) -> Vec<ScannedAudiobook> {
    let mut grouped: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !is_audio_file(path) {
            continue;
        }
        let parent = match path.parent() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };
        grouped.entry(parent).or_default().push(path.to_path_buf());
    }

    grouped
        .into_iter()
        .map(|(folder, mut files)| {
            files.sort();
            let title = folder
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
                .unwrap_or_else(|| folder.to_string_lossy().to_string());
            ScannedAudiobook {
                title,
                folder_path: folder.to_string_lossy().to_string(),
                files,
            }
        })
        .collect()
}

fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let lower = ext.to_ascii_lowercase();
            AUDIO_EXTS.iter().any(|&candidate| candidate == lower)
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn groups_audio_files_by_parent_folder() {
        let root = tempdir().unwrap();
        let book_a = root.path().join("Book A");
        let book_b = root.path().join("nested").join("Book B");
        fs::create_dir_all(&book_a).unwrap();
        fs::create_dir_all(&book_b).unwrap();
        fs::write(book_a.join("01.mp3"), b"").unwrap();
        fs::write(book_a.join("02.mp3"), b"").unwrap();
        fs::write(book_a.join("cover.jpg"), b"").unwrap(); // non-audio, ignored
        fs::write(book_b.join("track.flac"), b"").unwrap();

        let mut scanned = scan_library_folder(root.path());
        scanned.sort_by(|a, b| a.title.cmp(&b.title));

        assert_eq!(scanned.len(), 2);
        assert_eq!(scanned[0].title, "Book A");
        assert_eq!(scanned[0].files.len(), 2);
        assert_eq!(scanned[1].title, "Book B");
        assert_eq!(scanned[1].files.len(), 1);
    }

    #[test]
    fn missing_root_returns_empty() {
        let root = tempdir().unwrap();
        let nope = root.path().join("does-not-exist");
        assert!(scan_library_folder(&nope).is_empty());
    }
}
