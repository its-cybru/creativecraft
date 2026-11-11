//! Development aid: run the out-of-process plug-in scan exactly as the
//! application does, and print what ends up in Effects & Presets.
//!
//! ```text
//! cargo run --example scan -- "D:\...\studio-shell.exe"
//! ```
//!
//! The argument is the executable that knows how to become an OFX worker
//! (`<host> --ofx-worker`). With no argument the debug build of CreativeCraft's
//! shell next to this workspace is used.

use std::path::Path;

use effectcraft_ofx::worker;

struct StderrLogger;

impl log::Log for StderrLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        use std::io::Write;
        let mut err = std::io::stderr().lock();
        let _ = writeln!(err, "[{}] {}: {}", record.level(), record.target(), record.args());
    }
    fn flush(&self) {}
}

static LOGGER: StderrLogger = StderrLogger;

fn main() {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Info);

    let exe = std::env::args()
        .nth(1)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../studio-shell/target/debug/studio-shell.exe").to_string_lossy().to_string());
    if !Path::new(&exe).is_file() {
        eprintln!("no host executable at {exe} - build studio-shell first");
        std::process::exit(2);
    }
    worker::configure_worker(exe);

    let count = worker::scan_and_register_all();
    println!("\nregistered {count} OpenFX effect(s):");
    for effect in effectcraft_effects::plugin::plugins() {
        println!("  {:28} {:40} {}", effect.name, effect.id, effect.category);
    }
    println!("\nfolders shown by Effects & Presets:");
    for cat in effectcraft_effects::categories() {
        let n = effectcraft_effects::all().iter().filter(|e| e.category == cat).count();
        println!("  {cat} ({n})");
    }
    if let Some(skipped) = Some(worker::skipped_bundles()).filter(|s| !s.is_empty()) {
        println!("\nskipped after a hang or crash:");
        for path in skipped {
            println!("  {path}");
        }
    }
    worker::shutdown();
}
