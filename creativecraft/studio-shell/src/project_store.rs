//! One folder per project: everything the tabs made, in a place the user can
//! find again.
//!
//! The shell saves here when the window closes and reopens the same folder on
//! the next launch. This is not the apps' own document format — each app keeps
//! writing its own file (`cut.fcproj`, `cut.ecproj`, `cut.scraft`) next to the
//! others, so any of them can still be opened by its own app alone.

use std::path::{Path, PathBuf};

/// The one place CreativeCraft keeps its projects.
pub fn root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("CreativeCraft")
        .join("projects")
}

/// A project name that is also a legal Windows folder name: the characters
/// Windows forbids become `-`, trailing dots go, and an empty name becomes
/// "Untitled". The name is capped so a folder stays well under the path limit.
pub fn sanitize(name: &str) -> String {
    let mut clean = String::with_capacity(name.len());
    for ch in name.chars() {
        let forbidden = matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if forbidden || ch.is_control() {
            clean.push('-');
        } else {
            clean.push(ch);
        }
        if clean.chars().count() >= 60 {
            break;
        }
    }
    let clean = clean.trim().trim_end_matches('.').trim().to_string();
    if clean.is_empty() {
        "Untitled".to_string()
    } else {
        clean
    }
}

/// The folder for a project, named after it.
pub fn folder_for(name: &str) -> PathBuf {
    root().join(sanitize(name))
}

/// Remember the folder, so the next launch opens the same project.
pub fn remember(folder: &Path) {
    let base = root();
    if std::fs::create_dir_all(&base).is_err() {
        return;
    }
    let _ = std::fs::write(
        base.join("last-project.txt"),
        folder.to_string_lossy().as_bytes(),
    );
}

/// The folder the last close saved, if it is still there.
pub fn remembered() -> Option<PathBuf> {
    let text = std::fs::read_to_string(root().join("last-project.txt")).ok()?;
    let path = PathBuf::from(text.trim());
    path.is_dir().then_some(path)
}

/// The one file in `folder` with this extension. The shell writes exactly one
/// of each, so the first sorted match is the project's file.
pub fn find(folder: &Path, extension: &str) -> Option<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
        })
        .collect();
    files.sort();
    files.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_becomes_a_legal_folder_name() {
        assert_eq!(sanitize("FilmCraft Demo"), "FilmCraft Demo");
        assert_eq!(sanitize("Cut 1/2: final?"), "Cut 1-2- final-");
        assert_eq!(sanitize("   "), "Untitled");
        assert_eq!(sanitize("end."), "end");
        assert!(sanitize(&"long".repeat(40)).chars().count() <= 60);
    }

    #[test]
    fn the_project_folder_lives_under_one_root() {
        let folder = folder_for("FilmCraft Demo");
        assert!(folder.starts_with(root()));
        assert_eq!(
            folder.file_name().and_then(|name| name.to_str()),
            Some("FilmCraft Demo")
        );
    }
}
