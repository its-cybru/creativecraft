//! Removing Maestro again: the install folder, its Python environment and the
//! models Maestro downloaded into it.
//!
//! Deleting can take a while when models were downloaded (tens or hundreds of
//! gigabytes), so it runs on its own thread and the tab reports when it is done.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

/// One removal in flight.
pub struct MaestroUninstall {
    done: Arc<Mutex<Option<Result<(), String>>>>,
}

impl MaestroUninstall {
    /// Start deleting `folder` in the background.
    pub fn start(folder: &Path) -> Self {
        let done = Arc::new(Mutex::new(None));
        let signal = Arc::clone(&done);
        let path = folder.to_path_buf();
        std::thread::spawn(move || {
            let result = if path.exists() {
                std::fs::remove_dir_all(&path)
                    .map_err(|error| format!("Could not remove {}: {error}", path.display()))
            } else {
                Ok(())
            };
            *signal.lock().unwrap_or_else(PoisonError::into_inner) = Some(result);
        });
        Self { done }
    }

    /// `Some(Ok(()))` when the folder is gone, `Some(Err(..))` on failure.
    pub fn finished(&self) -> Option<Result<(), String>> {
        self.done
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_is_removed() {
        let folder = std::env::temp_dir().join(format!("cc-uninstall-{}", std::process::id()));
        std::fs::create_dir_all(folder.join("app").join("env")).expect("a test folder");
        std::fs::write(folder.join("app").join("env").join("python.exe"), b"x").expect("a file");

        let removal = MaestroUninstall::start(&folder);
        let mut result = None;
        for _ in 0..200 {
            result = removal.finished();
            if result.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            matches!(result, Some(Ok(()))),
            "removal finished: {result:?}"
        );
        assert!(!folder.exists(), "the folder is gone");
    }

    #[test]
    fn a_missing_folder_is_not_an_error() {
        let folder =
            std::env::temp_dir().join(format!("cc-uninstall-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let removal = MaestroUninstall::start(&folder);
        let mut result = None;
        for _ in 0..100 {
            result = removal.finished();
            if result.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(matches!(result, Some(Ok(()))));
    }
}
