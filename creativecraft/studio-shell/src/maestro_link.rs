//! The Maestro link: files go in to Maestro, and its newest output comes back.
//!
//! Maestro is a separate application with its own library, so the link is files:
//! what CreativeCraft exports (a frame, the cut's audio, the design) is written
//! into one folder the user can drop onto Maestro's page, and the newest file
//! Maestro produced is placed back on the Edit timeline.

use std::path::{Path, PathBuf};

/// What a file is, by its extension — enough to place it on the right track.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Video,
    Image,
    Audio,
    Other,
}

pub fn kind(path: &Path) -> Kind {
    let ext = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" => Kind::Video,
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" => Kind::Image,
        "wav" | "mp3" | "flac" | "aiff" | "aif" | "m4a" | "ogg" | "opus" => Kind::Audio,
        _ => Kind::Other,
    }
}

/// The one folder CreativeCraft writes for Maestro to pick up.
pub fn inbox() -> PathBuf {
    base().join("in")
}

fn base() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("CreativeCraft")
        .join("maestro")
}

/// The newest media file Maestro wrote under `root`, looking at the places a
/// Maestro install keeps its results.
pub fn newest_export(root: &Path) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for folder in [
        root.to_path_buf(),
        root.join("outputs"),
        root.join("app").join("outputs"),
        root.join("app").join("gallery"),
        root.join("output"),
    ] {
        collect_newest(&folder, 0, &mut best);
    }
    best.map(|(_, path)| path)
}

/// Walk one folder and its subfolders (two levels deep is plenty for a gallery)
/// for the newest media file.
fn collect_newest(
    folder: &Path,
    depth: usize,
    best: &mut Option<(std::time::SystemTime, PathBuf)>,
) {
    if depth > 2 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_newest(&path, depth + 1, best);
            continue;
        }
        if kind(&path) == Kind::Other {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
            continue;
        };
        if best.as_ref().is_none_or(|(when, _)| modified > *when) {
            *best = Some((modified, path));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_classified_by_extension() {
        assert_eq!(kind(Path::new("shot.mp4")), Kind::Video);
        assert_eq!(kind(Path::new("art.PNG")), Kind::Image);
        assert_eq!(kind(Path::new("mix.wav")), Kind::Audio);
        assert_eq!(kind(Path::new("notes.txt")), Kind::Other);
    }

    #[test]
    fn the_newest_export_is_found() {
        let dir = std::env::temp_dir().join(format!("cc-maestro-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("outputs")).expect("a test folder is created");
        std::fs::write(dir.join("outputs").join("old.mp4"), b"old").expect("old file");
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(dir.join("outputs").join("new.png"), b"new").expect("new file");
        std::fs::write(dir.join("outputs").join("notes.txt"), b"x").expect("text file");

        let newest = newest_export(&dir).expect("an export is found");
        assert_eq!(
            newest.file_name().and_then(|name| name.to_str()),
            Some("new.png"),
            "the newest media file, not the text file"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_inbox_lives_under_the_maestro_folder() {
        assert!(inbox().ends_with(Path::new("CreativeCraft").join("maestro").join("in")));
    }
}
