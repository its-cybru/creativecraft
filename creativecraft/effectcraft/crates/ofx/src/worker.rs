//! Out-of-process OpenFX host.
//!
//! OpenFX plug-ins are native libraries written by third parties. Some of them
//! corrupt the heap, spin forever, or show dialogs when loaded with a host they
//! do not recognise — and no amount of care in this crate can make foreign
//! `unsafe` code trustworthy.
//!
//! So the plug-ins are loaded in a small helper process instead of inside
//! EffectCraft:
//!
//! * the host application re-executes itself with [`WORKER_ARG`] and calls
//!   [`serve`], which loads plug-ins and answers requests;
//! * [`scan_and_register_all`] asks that process to describe each bundle, with a
//!   timeout;
//! * rendering goes through the same process ([`RemotePlugin`]).
//!
//! A plug-in that crashes or hangs therefore costs one helper process: it is
//! killed, remembered in a skip list, and EffectCraft keeps running.

#![allow(unsafe_code)]

use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use effectcraft_effects::plugin::{EffectPlugin, PluginFrame, PluginManifest, PluginParams, register_plugin};

use crate::plugin::OfxEffectPlugin;

/// The argument that turns the host application into an OpenFX worker.
pub const WORKER_ARG: &str = "--ofx-worker";

/// How long the worker gets to describe one bundle before it is considered hung.
/// A big GPU plug-in (a raytracer, a film-look suite) can take tens of seconds
/// to load on a busy machine.
const DESCRIBE_TIMEOUT: Duration = Duration::from_secs(30);

/// How long a single 32x32 test render may take. Deliberately shorter than a
/// real render: a plug-in that cannot even do a thumbnail in this time is not
/// going to be usable.
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// How long one rendered frame may take.
const RENDER_TIMEOUT: Duration = Duration::from_secs(120);

/// How long one start may spend searching for bundles. Rejection answers are
/// cached, so later starts finish the job without repeating the slow work.
const SCAN_BUDGET: Duration = Duration::from_secs(90);

/// How long a bundle that hung or crashed stays skipped. A busy machine can
/// make a large plug-in miss its load timeout once, which should not remove it
/// for good, so after this long it gets one more try. Delete
/// `ofx-skip.txt` (or call [`clear_skips`]) to retry immediately.
const SKIP_RETRY: Duration = Duration::from_secs(24 * 60 * 60);

/// The command that starts a worker, set by the host application.
static WORKER_COMMAND: OnceLock<(PathBuf, Vec<String>)> = OnceLock::new();

/// Paths that already made the worker hang or crash, with when that happened.
static SKIPS: OnceLock<Mutex<Vec<(u64, String)>>> = OnceLock::new();

/// The one live worker, if any.
static WORKER: Mutex<Option<Worker>> = Mutex::new(None);

fn now_seconds() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn skips() -> &'static Mutex<Vec<(u64, String)>> {
    SKIPS.get_or_init(|| Mutex::new(read_skip_file()))
}

fn skip_file() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("CreativeCraft").join("ofx-skip.txt")
}

/// `when<tab>path` per line. A line without a time is an entry written before
/// the time was recorded: it counts as old and is retried at once.
fn read_skip_file() -> Vec<(u64, String)> {
    let Ok(text) = std::fs::read_to_string(skip_file()) else {
        return Vec::new();
    };
    let mut list = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match line.split_once('\t') {
            Some((when, path)) => {
                if let Ok(at) = when.trim().parse::<u64>() {
                    list.push((at, path.trim().to_string()));
                }
            }
            None => list.push((0, line.to_string())),
        }
    }
    list
}

fn write_skip_file(list: &[(u64, String)]) {
    if let Some(dir) = skip_file().parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut text = String::new();
    for (at, path) in list {
        text.push_str(&format!("{at}\t{path}\n"));
    }
    let _ = std::fs::write(skip_file(), text);
}

fn remember_skip(path: &Path) {
    let key = normalize(path);
    let now = now_seconds();
    let mut list = skips().lock().unwrap_or_else(|e| e.into_inner());
    match list.iter_mut().find(|(_, p)| *p == key) {
        Some(entry) => entry.0 = now,
        None => list.push((now, key)),
    }
    write_skip_file(&list);
}

/// Drop entries that are old enough to try again.
fn forget_stale_skips(list: &mut Vec<(u64, String)>) -> bool {
    let now = now_seconds();
    let before = list.len();
    list.retain(|(at, _)| now.saturating_sub(*at) < SKIP_RETRY.as_secs());
    list.len() != before
}

fn is_skipped(path: &Path) -> bool {
    let key = normalize(path);
    let mut list = skips().lock().unwrap_or_else(|e| e.into_inner());
    if forget_stale_skips(&mut list) {
        write_skip_file(&list);
    }
    list.iter().any(|(_, p)| *p == key)
}

fn normalize(path: &Path) -> String {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()).to_string_lossy().to_string()
}

/// Bundles that refused this host (unsupported API, missing feature, not an
/// image effect). Red Giant alone ships over a hundred bundles that each take
/// seconds to reject, so the answer is remembered with the file's modification
/// time and only retried when the file itself changes.
static UNUSABLE: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();

fn unusable_file() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("CreativeCraft").join("ofx-unusable.txt")
}

fn unusable() -> &'static Mutex<HashMap<String, u64>> {
    UNUSABLE.get_or_init(|| Mutex::new(read_unusable_file()))
}

fn read_unusable_file() -> HashMap<String, u64> {
    read_stamp_file(&unusable_file())
}

fn write_unusable_file(map: &HashMap<String, u64>) {
    write_stamp_file(&unusable_file(), map);
}

fn modified_seconds(path: &Path) -> u64 {
    std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

fn is_known_unusable(path: &Path) -> bool {
    is_known_unusable_key(&normalize(path), modified_seconds(path))
}

fn is_known_unusable_key(key: &str, stamp: u64) -> bool {
    unusable().lock().unwrap_or_else(|e| e.into_inner()).get(key) == Some(&stamp)
}

fn remember_unusable(path: &Path) {
    remember_unusable_key(&normalize(path), modified_seconds(path));
}

fn remember_unusable_key(key: &str, stamp: u64) {
    let mut map = unusable().lock().unwrap_or_else(|e| e.into_inner());
    map.insert(key.to_string(), stamp);
    write_unusable_file(&map);
}

fn forget_unusable(path: &Path) {
    forget_unusable_key(&normalize(path));
}

fn forget_unusable_key(key: &str) {
    let mut map = unusable().lock().unwrap_or_else(|e| e.into_inner());
    if map.remove(key).is_some() {
        write_unusable_file(&map);
    }
}

/// The cache key for one effect inside a bundle.
fn effect_key(path: &Path, id: &str) -> String {
    format!("{}|{id}", normalize(path))
}

/// Effects that already proved they can render (`path|id` -> file stamp).
///
/// Testing is cheap for most plug-ins but not for all: a heavy one can take a
/// minute to produce its first frame, and paying that on every start would make
/// the background scan feel like a hang.
static USABLE: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();

fn usable_file() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("CreativeCraft").join("ofx-usable.txt")
}

fn usable() -> &'static Mutex<HashMap<String, u64>> {
    USABLE.get_or_init(|| Mutex::new(read_stamp_file(&usable_file())))
}

fn read_stamp_file(path: &Path) -> HashMap<String, u64> {
    let mut map = HashMap::new();
    if let Ok(text) = std::fs::read_to_string(path) {
        for line in text.lines() {
            let mut parts = line.splitn(2, '\t');
            if let (Some(stamp), Some(key)) = (parts.next(), parts.next()) {
                if let Ok(stamp) = stamp.parse::<u64>() {
                    map.insert(key.to_string(), stamp);
                }
            }
        }
    }
    map
}

fn write_stamp_file(path: &Path, map: &HashMap<String, u64>) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut text = String::new();
    for (key, stamp) in map {
        text.push_str(&format!("{stamp}\t{key}\n"));
    }
    let _ = std::fs::write(path, text);
}

fn is_known_usable(key: &str, stamp: u64) -> bool {
    usable().lock().unwrap_or_else(|e| e.into_inner()).get(key) == Some(&stamp)
}

fn remember_usable(key: &str, stamp: u64) {
    let mut map = usable().lock().unwrap_or_else(|e| e.into_inner());
    map.insert(key.to_string(), stamp);
    write_stamp_file(&usable_file(), &map);
}

fn forget_usable(key: &str) {
    let mut map = usable().lock().unwrap_or_else(|e| e.into_inner());
    if map.remove(key).is_some() {
        write_stamp_file(&usable_file(), &map);
    }
}

/// Forget every bundle that was skipped after a hang or crash, and every
/// remembered rejection, so the next start tests everything again.
///
/// Returns how many entries were removed.
pub fn clear_skips() -> usize {
    let mut list = skips().lock().unwrap_or_else(|e| e.into_inner());
    let count = list.len();
    list.clear();
    let _ = std::fs::remove_file(skip_file());

    let removed = {
        let mut map = unusable().lock().unwrap_or_else(|e| e.into_inner());
        let removed = map.len();
        map.clear();
        write_unusable_file(&map);
        removed
    };

    let retested = {
        let mut map = usable().lock().unwrap_or_else(|e| e.into_inner());
        let retested = map.len();
        map.clear();
        write_stamp_file(&usable_file(), &map);
        retested
    };
    count + removed + retested
}

/// Bundles currently skipped because loading them hung or crashed.
pub fn skipped_bundles() -> Vec<String> {
    let mut list = skips().lock().unwrap_or_else(|e| e.into_inner());
    if forget_stale_skips(&mut list) {
        write_skip_file(&list);
    }
    list.iter().map(|(_, path)| path.clone()).collect()
}

/// Tell the crate how to start a worker process.
///
/// Must be called once from the application's `main` before
/// [`scan_and_register_all`]. Without it OpenFX support stays off (which keeps
/// tests and headless tools from starting plug-in processes).
pub fn configure_worker(exe: impl Into<PathBuf>) {
    let _ = WORKER_COMMAND.set((exe.into(), vec![WORKER_ARG.to_string()]));
}

/// Whether a worker command has been configured.
pub fn is_configured() -> bool {
    WORKER_COMMAND.get().is_some()
}

// ---------------------------------------------------------------- protocol

#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Ping,
    /// Load one `.ofx` binary and describe its plug-ins.
    Describe {
        path: String,
    },
    /// Render one frame through a plug-in, loading its bundle first if this
    /// helper has not seen it yet (a helper is restarted after a crash).
    Render {
        path: String,
        id: String,
        width: u32,
        height: u32,
        scale: f64,
        time: f64,
        flat: Vec<f64>,
    },
    Quit,
}

#[derive(Serialize, Deserialize, Default)]
struct Response {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    effects: Vec<PluginManifest>,
}

fn write_frame<W: Write>(out: &mut W, json: &[u8], payload: &[u8]) -> std::io::Result<()> {
    out.write_all(&(json.len() as u32).to_le_bytes())?;
    out.write_all(json)?;
    out.write_all(&(payload.len() as u32).to_le_bytes())?;
    out.write_all(payload)?;
    out.flush()
}

fn read_frame<R: Read>(input: &mut R) -> std::io::Result<(Vec<u8>, Vec<u8>)> {
    let mut len = [0u8; 4];
    input.read_exact(&mut len)?;
    let mut json = vec![0u8; u32::from_le_bytes(len) as usize];
    input.read_exact(&mut json)?;
    input.read_exact(&mut len)?;
    let mut payload = vec![0u8; u32::from_le_bytes(len) as usize];
    input.read_exact(&mut payload)?;
    Ok((json, payload))
}

/// Pixels as the wire format: 4 little-endian `f32` per pixel.
fn pixels_to_bytes(pixels: &[[f32; 4]]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixels.len() * 16);
    for px in pixels {
        for channel in px {
            out.extend_from_slice(&channel.to_le_bytes());
        }
    }
    out
}

fn bytes_to_pixels(bytes: &[u8]) -> Vec<[f32; 4]> {
    bytes
        .chunks_exact(16)
        .map(|c| {
            let mut px = [0.0f32; 4];
            for (i, channel) in px.iter_mut().enumerate() {
                let start = i * 4;
                *channel = f32::from_le_bytes([c[start], c[start + 1], c[start + 2], c[start + 3]]);
            }
            px
        })
        .collect()
}

// ---------------------------------------------------------------- worker side

/// Run the helper process: load plug-ins, answer requests, never write to
/// stdout except protocol frames.
///
/// Called by the application when it sees [`WORKER_ARG`].
pub fn serve() -> ! {
    // Plug-ins may print to stdout; keep our protocol stream clean by routing
    // everything else to stderr.
    let mut input = BufReader::new(std::io::stdin());
    let mut output = std::io::stdout();
    let mut effects: HashMap<String, Arc<OfxEffectPlugin>> = HashMap::new();

    loop {
        let (json, payload) = match read_frame(&mut input) {
            Ok(frame) => frame,
            Err(_) => break,
        };
        let request: Request = match serde_json::from_slice(&json) {
            Ok(r) => r,
            Err(e) => {
                let response = Response { ok: false, error: Some(format!("bad request: {e}")), effects: vec![] };
                let text = serde_json::to_vec(&response).unwrap_or_default();
                let _ = write_frame(&mut output, &text, &[]);
                continue;
            }
        };

        match request {
            Request::Ping => {
                let text = serde_json::to_vec(&Response { ok: true, ..Default::default() }).unwrap_or_default();
                let _ = write_frame(&mut output, &text, &[]);
            }
            Request::Quit => {
                let text = serde_json::to_vec(&Response { ok: true, ..Default::default() }).unwrap_or_default();
                let _ = write_frame(&mut output, &text, &[]);
                break;
            }
            Request::Describe { path } => {
                let result = crate::scan::load_ofx_library(Path::new(&path));
                let response = match result {
                    Ok(plugins) => {
                        let mut manifests = Vec::new();
                        for plugin in plugins {
                            effects.insert(plugin.manifest.id.clone(), plugin.clone());
                            manifests.push(plugin.manifest.clone());
                        }
                        Response { ok: true, error: None, effects: manifests }
                    }
                    Err(error) => Response { ok: false, error: Some(error), effects: vec![] },
                };
                let text = serde_json::to_vec(&response).unwrap_or_default();
                let _ = write_frame(&mut output, &text, &[]);
            }
            Request::Render { path, id, width, height, scale, time, flat } => {
                // A helper restarted after another bundle crashed starts empty;
                // reload this bundle on demand instead of failing the render.
                if !effects.contains_key(&id) {
                    if let Ok(plugins) = crate::scan::load_ofx_library(Path::new(&path)) {
                        for plugin in plugins {
                            effects.insert(plugin.manifest.id.clone(), plugin);
                        }
                    }
                }
                let response = match effects.get(&id) {
                    None => Response { ok: false, error: Some(format!("plug-in '{id}' is not loaded")), effects: vec![] },
                    Some(effect) => {
                        let pixels = bytes_to_pixels(&payload);
                        if pixels.len() < width as usize * height as usize {
                            Response { ok: false, error: Some("short frame payload".into()), effects: vec![] }
                        } else {
                            let mut pixels = pixels;
                            let mut frame = PluginFrame { width, height, pixels: &mut pixels, scale };
                            let params = PluginParams { manifest: &effect.manifest, flat: &flat };
                            match effect.render(&mut frame, &params, time) {
                                Ok(()) => {
                                    // The frame lives on; send back the pixels.
                                    let bytes = pixels_to_bytes(&pixels);
                                    let text = serde_json::to_vec(&Response { ok: true, ..Default::default() }).unwrap_or_default();
                                    let _ = write_frame(&mut output, &text, &bytes);
                                    continue;
                                }
                                Err(error) => Response { ok: false, error: Some(error), effects: vec![] },
                            }
                        }
                    }
                };
                let text = serde_json::to_vec(&response).unwrap_or_default();
                let _ = write_frame(&mut output, &text, &[]);
            }
        }
    }
    std::process::exit(0);
}

// ---------------------------------------------------------------- host side

/// A running helper process.
struct Worker {
    child: Child,
    stdin: ChildStdin,
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
    dead: AtomicBool,
}

impl Worker {
    fn spawn() -> Result<Self, String> {
        let (exe, args) = WORKER_COMMAND.get().ok_or_else(|| "OpenFX support is not configured".to_string())?;
        let mut command = Command::new(exe);
        command.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn().map_err(|e| format!("cannot start the OpenFX helper ({}): {e}", exe.display()))?;
        let stdin = child.stdin.take().ok_or("helper has no stdin")?;
        let stdout = child.stdout.take().ok_or("helper has no stdout")?;
        Ok(Self { child, stdin, stdout: Arc::new(Mutex::new(BufReader::new(stdout))), dead: AtomicBool::new(false) })
    }

    fn kill(&mut self) {
        self.dead.store(true, Ordering::SeqCst);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Send a request and wait at most `timeout` for the answer.
    ///
    /// The read happens on its own thread so a hung plug-in cannot block the
    /// caller forever; on timeout the helper is killed.
    fn request(&mut self, request: &Request, payload: &[u8], timeout: Duration) -> Result<(Response, Vec<u8>), String> {
        let json = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        write_frame(&mut self.stdin, &json, payload).map_err(|e| format!("helper write failed: {e}"))?;

        let (tx, rx) = mpsc::channel();
        let reader = Arc::clone(&self.stdout);
        std::thread::spawn(move || {
            let mut guard = reader.lock().unwrap_or_else(|e| e.into_inner());
            let _ = tx.send(read_frame(&mut *guard));
        });

        match rx.recv_timeout(timeout) {
            Ok(Ok((json, payload))) => {
                let response: Response = serde_json::from_slice(&json).map_err(|e| format!("bad helper response: {e}"))?;
                Ok((response, payload))
            }
            Ok(Err(e)) => {
                self.dead.store(true, Ordering::SeqCst);
                Err(format!("helper stopped: {e}"))
            }
            Err(_) => {
                self.kill();
                Err("timed out (the plug-in stopped responding)".into())
            }
        }
    }

    fn describe(&mut self, path: &Path) -> Result<Vec<PluginManifest>, String> {
        let (response, _) = self.request(&Request::Describe { path: path.to_string_lossy().to_string() }, &[], DESCRIBE_TIMEOUT)?;
        if response.ok { Ok(response.effects) } else { Err(response.error.unwrap_or_else(|| "unknown error".into())) }
    }

    /// Render one tiny frame to prove the plug-in can run in this host.
    ///
    /// Loading and describing are only half the story: several commercial
    /// plug-ins accept both and then refuse to render without a GPU context or
    /// a licence. Better to find out once, here, than in the middle of the
    /// user's edit.
    fn probe(&mut self, path: &Path, manifest: &PluginManifest) -> Result<(), String> {
        const SIDE: u32 = 32;
        let pixels = vec![[0.1f32, 0.2, 0.3, 1.0]; (SIDE * SIDE) as usize];
        let payload = pixels_to_bytes(&pixels);
        let width = manifest.params.iter().map(|p| p.kind.width()).sum();
        let request = Request::Render {
            path: path.to_string_lossy().to_string(),
            id: manifest.id.clone(),
            width: SIDE,
            height: SIDE,
            scale: 1.0,
            time: 0.0,
            flat: vec![0.0; width],
        };
        let (response, _) = self.request(&request, &payload, PROBE_TIMEOUT)?;
        if response.ok { Ok(()) } else { Err(response.error.unwrap_or_else(|| "unknown error".into())) }
    }
}

/// The shared worker, started on first use.
fn with_worker<T>(f: impl FnOnce(&mut Worker) -> Result<T, String>) -> Result<T, String> {
    let mut guard = WORKER.lock().unwrap_or_else(|e| e.into_inner());
    if guard.as_ref().is_none_or(|w| w.dead.load(Ordering::SeqCst)) {
        *guard = None;
        *guard = Some(Worker::spawn()?);
    }
    let worker = guard.as_mut().expect("worker just started");
    let result = f(worker);
    if worker.dead.load(Ordering::SeqCst) {
        *guard = None;
    }
    result
}

/// A plug-in that lives in the helper process.
pub struct RemotePlugin {
    manifest: PluginManifest,
    source: String,
}

impl EffectPlugin for RemotePlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn render(&self, frame: &mut PluginFrame, params: &PluginParams, time: f64) -> Result<(), String> {
        let pixels = frame.pixels.len();
        let payload = pixels_to_bytes(&frame.pixels[..pixels]);
        let request = Request::Render {
            path: self.source.clone(),
            id: self.manifest.id.clone(),
            width: frame.width,
            height: frame.height,
            scale: frame.scale,
            time,
            flat: params.flat.to_vec(),
        };
        let (response, bytes) = with_worker(|worker| worker.request(&request, &payload, RENDER_TIMEOUT))?;
        if !response.ok {
            return Err(response.error.unwrap_or_else(|| "unknown error".into()));
        }
        let output = bytes_to_pixels(&bytes);
        if output.len() < pixels {
            return Err("helper returned a short frame".into());
        }
        frame.pixels.copy_from_slice(&output[..pixels]);
        Ok(())
    }

    fn source(&self) -> String {
        self.source.clone()
    }
}

/// Describe every plug-in bundle and register the results.
///
/// Bundles that hang or crash the helper are killed, recorded in the skip list
/// and ignored from then on. Returns the number of registered effects.
pub fn scan_and_register_all() -> usize {
    if !is_configured() {
        log::debug!("[OFX] no worker configured; OpenFX is off for this process");
        return 0;
    }
    let started = std::time::Instant::now();
    let mut count = 0;
    let mut deferred = 0;
    for dir in crate::scan::default_plugin_directories() {
        for binary in crate::scan::find_ofx_binaries(&dir) {
            // One launch must not spend half an hour on other people's files.
            // Whatever is left waits for the next start; accepted and rejected
            // bundles are remembered, so the list shrinks every time.
            if started.elapsed() > SCAN_BUDGET {
                deferred += 1;
                continue;
            }
            count += describe_and_register(&binary);
        }
    }
    if deferred > 0 {
        log::info!("[OFX] {deferred} bundle(s) left for the next start (scan time limit reached)");
    }
    count
}

/// Describe one bundle and register its effects.
pub fn describe_and_register(path: &Path) -> usize {
    if is_skipped(path) {
        log::debug!("[OFX] skipping {} (previously hung or crashed)", path.display());
        return 0;
    }
    if is_known_unusable(path) {
        log::trace!("[OFX] {} refused this host before; not retesting", path.display());
        return 0;
    }

    let mut attempt = 0;
    let manifests = loop {
        attempt += 1;
        match with_worker(|worker| worker.describe(path)) {
            Ok(manifests) => break manifests,
            Err(error) => {
                let crashed = error.contains("timed out") || error.contains("helper stopped");
                if crashed {
                    // The helper is unusable now; the next attempt starts a new
                    // one. A hang is recorded so the next start is fast.
                    if error.contains("timed out") {
                        remember_skip(path);
                        log::warn!("[OFX] {} hung while loading and will be skipped from now on", path.display());
                        return 0;
                    }
                    if attempt >= 2 {
                        remember_skip(path);
                        log::warn!("[OFX] {} crashed the OpenFX helper twice and will be skipped", path.display());
                        return 0;
                    }
                } else {
                    log::debug!("[OFX] {} not loaded: {error}", path.display());
                    remember_unusable(path);
                    return 0;
                }
            }
        }
    };

    let source = path.to_string_lossy().to_string();
    let stamp = modified_seconds(path);
    let mut registered = 0;
    for manifest in manifests {
        let key = effect_key(path, &manifest.id);
        if is_known_unusable_key(&key, stamp) {
            log::debug!("[OFX] '{}' cannot render here; not retesting", manifest.name);
            continue;
        }

        // Prove it can actually produce a frame before offering it. A plug-in
        // that already passed is not tested again until the file changes.
        if !is_known_usable(&key, stamp) {
            match with_worker(|worker| worker.probe(path, &manifest)) {
                Ok(()) => remember_usable(&key, stamp),
                Err(error) => {
                    forget_usable(&key);
                    remember_unusable_key(&key, stamp);
                    log::info!("[OFX] '{}' loads but cannot render in EffectCraft: {error}", manifest.name);
                    continue;
                }
            }
        }

        let name = manifest.name.clone();
        let plugin = Arc::new(RemotePlugin { manifest, source: source.clone() });
        match register_plugin(plugin) {
            Ok(_) => registered += 1,
            Err(error) => log::warn!("[OFX] could not register '{name}': {error}"),
        }
    }
    if registered > 0 {
        log::info!("[OFX] {registered} effect(s) from {}", path.display());
    }
    forget_unusable(path);
    registered
}

/// Stop the helper (called when the application exits).
pub fn shutdown() {
    let mut guard = WORKER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut worker) = guard.take() {
        let _ = worker.request(&Request::Quit, &[], Duration::from_secs(2));
        worker.kill();
    }
}
