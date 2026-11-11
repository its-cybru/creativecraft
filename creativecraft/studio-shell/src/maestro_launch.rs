//! Starting Maestro without Pinokio.
//!
//! Pinokio is Maestro's installer: it creates the Python environment and runs
//! `app/launch.py` with `SERVER_PORT` set. Once that environment exists, the
//! backend can be started directly — which is what this module does.
//!
//! Maestro can re-launch itself in a child Python, so its "Maestro UI:
//! http://127.0.0.1:PORT/" banner may be written by a process that is not our
//! direct child. Both of its streams therefore go to a **log file** (the same
//! view Pinokio's terminal gives), and the address is read from that file — with
//! the port probe as a second signal.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, PoisonError};

/// One running Maestro backend.
pub struct MaestroLaunch {
    child: Child,
    /// The port we asked for; the page answers there once it is up.
    port: u16,
    /// Everything Maestro printed, and the place to read it back from.
    log: PathBuf,
    /// Set once we have taken the address, so the file is not re-read forever.
    seen: Mutex<Option<String>>,
}

impl MaestroLaunch {
    /// Start `app/launch.py` from Maestro's install folder, on a free port.
    pub fn start(folder: &Path) -> Result<Self, String> {
        let app = folder.join("app");
        if !app.is_dir() {
            return Err(format!(
                "No Maestro app folder at {}. Install Maestro once (Pinokio's Install, or the \
                 steps in its README), then point this tab at that folder.",
                app.display()
            ));
        }
        let python = python_for(&app).ok_or_else(|| {
            format!(
                "No Python environment in {}. Maestro's install creates app/env with Python \
                 3.11; press Install / repair above.",
                app.display()
            )
        })?;
        let port = free_port()?;
        let log = log_path();
        if let Some(parent) = log.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let file = std::fs::File::create(&log)
            .map_err(|error| format!("Could not write {}: {error}", log.display()))?;
        let err = file
            .try_clone()
            .map_err(|error| format!("Could not write {}: {error}", log.display()))?;

        let child = Command::new(&python)
            .arg("launch.py")
            .current_dir(&app)
            .env("SERVER_PORT", port.to_string())
            .env("PYTHONUNBUFFERED", "1")
            .stdout(Stdio::from(file))
            .stderr(Stdio::from(err))
            .spawn()
            .map_err(|error| format!("Could not start {}: {error}", python.display()))?;
        Ok(Self {
            child,
            port,
            log,
            seen: Mutex::new(None),
        })
    }

    /// The address Maestro printed when it was ready, once it has.
    pub fn address(&self) -> Option<String> {
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        if seen.is_none() {
            *seen = find_address(&self.log);
        }
        seen.clone()
    }

    /// The port the page should answer on, whether or not Maestro printed it.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Is anything listening on that port?
    pub fn listening(&self) -> bool {
        std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], self.port)),
            std::time::Duration::from_millis(120),
        )
        .is_ok()
    }

    /// The last line Maestro wrote, for the tab's status line.
    pub fn tail(&self) -> String {
        last_line(&self.log).unwrap_or_default()
    }

    /// Is the backend still running? (A child that re-launched itself may have
    /// exited while the page still answers, so this is not the whole story.)
    pub fn running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Stop the backend, and everything it started.
    pub fn stop(&mut self) {
        self.kill_tree();
    }

    fn kill_tree(&mut self) {
        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .args(["/T", "/F", "/PID", &self.child.id().to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for MaestroLaunch {
    fn drop(&mut self) {
        self.kill_tree();
    }
}

/// Where Maestro is installed and where the run log lives.
fn log_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("CreativeCraft")
        .join("maestro")
        .join("run.log")
}

/// The last `http://…` address Maestro printed.
fn find_address(log: &Path) -> Option<String> {
    let text = read_tail(log, 64 * 1024)?;
    text.lines()
        .filter_map(|line| {
            let start = line.find("http://")?;
            let rest = &line[start..];
            let end = rest
                .find(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'' | ')' | ']'))
                .unwrap_or(rest.len());
            Some(rest[..end].to_string())
        })
        .last()
}

/// The last non-empty line in the log.
fn last_line(log: &Path) -> Option<String> {
    let text = read_tail(log, 8 * 1024)?;
    text.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

/// Read up to `limit` bytes from the end of a file (enough for a URL and a
/// status line, without loading a long run).
fn read_tail(path: &Path, limit: u64) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    if len > limit {
        use std::io::Seek;
        file.seek(std::io::SeekFrom::Start(len - limit)).ok()?;
    }
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).ok()?;
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

/// Maestro's own environment, newest runtime first.
fn python_for(app: &Path) -> Option<PathBuf> {
    let candidates = [
        "env-rtx50/Scripts/python.exe",
        "env-sol/Scripts/python.exe",
        "env/Scripts/python.exe",
        "env/bin/python",
    ];
    candidates
        .iter()
        .map(|rel| app.join(rel))
        .find(|path| path.is_file())
}

/// A port nothing is listening on right now.
fn free_port() -> Result<u16, String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("Could not pick a local port: {error}"))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|error| format!("Could not read the local port: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_is_read_from_the_log() {
        let dir = std::env::temp_dir().join(format!("cc-maestro-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a test folder");
        let log = dir.join("run.log");
        std::fs::write(
            &log,
            "[Maestro] Importing WanGP engine...\n\
             ==================================================\n\
             \x20 Maestro UI:    http://127.0.0.1:59471/\n\
             \x20 API docs:      http://127.0.0.1:59471/docs\n",
        )
        .expect("the log is written");

        assert_eq!(
            find_address(&log).as_deref(),
            Some("http://127.0.0.1:59471/docs"),
            "the last address in the log is the one Maestro ended on"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_log_without_an_address_says_nothing() {
        let dir = std::env::temp_dir().join(format!("cc-maestro-log2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a test folder");
        let log = dir.join("run.log");
        std::fs::write(&log, "Importing WanGP engine...\n").expect("the log is written");
        assert_eq!(find_address(&log), None);
        assert_eq!(
            last_line(&log).as_deref(),
            Some("Importing WanGP engine...")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_free_port_is_offered() {
        assert!(free_port().expect("a port is offered") > 0);
    }

    #[test]
    fn a_missing_folder_says_what_is_missing() {
        let error = MaestroLaunch::start(std::path::Path::new("no-such-maestro-folder"))
            .err()
            .expect("a missing folder is an error");
        assert!(error.contains("No Maestro app folder"), "{error}");
    }
}
