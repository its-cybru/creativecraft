//! Load one OpenFX binary and describe its effects.
//!
//! Used while developing the host: it runs in its own process, so a plugin
//! that crashes the host only takes the probe down with it.
//!
//! `cargo run -p effectcraft-ofx --example probe -- <path-to-.ofx>`

use std::io::Write;

struct StderrLogger;

impl log::Log for StderrLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        let mut err = std::io::stderr().lock();
        let _ = writeln!(err, "[{}] {}: {}", record.level(), record.target(), record.args());
    }
    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }
}

static LOGGER: StderrLogger = StderrLogger;

fn main() {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Trace);

    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: probe <path-to-.ofx> | --all");
        std::process::exit(2);
    };

    if path == "--all" {
        let skip: Vec<String> = std::env::args().skip(2).filter_map(|a| a.strip_prefix("--skip=").map(str::to_string)).collect();
        for dir in effectcraft_ofx::default_plugin_directories() {
            for bin in effectcraft_ofx::find_ofx_binaries(&dir) {
                let shown = bin.to_string_lossy().to_string();
                if skip.iter().any(|s| shown.contains(s)) {
                    eprintln!("--- skipped {shown}");
                    continue;
                }
                eprintln!("--- {shown}");
                match effectcraft_ofx::load_ofx_library(&bin) {
                    Ok(e) => println!("ok {} from {}", e.len(), shown),
                    Err(e) => println!("error from {shown}: {e}"),
                }
            }
        }
        return;
    }

    let path = std::path::Path::new(&path);
    eprintln!("loading {}", path.display());
    match effectcraft_ofx::load_ofx_library(path) {
        Ok(effects) => {
            println!("ok {} effect(s)", effects.len());
            for e in &effects {
                println!("  {} | {} | {} | {} params", e.manifest.id, e.manifest.name, e.manifest.category, e.manifest.params.len());
                for p in &e.manifest.params {
                    println!("      - {} ({})", p.id, p.name);
                }
            }
        }
        Err(e) => {
            println!("error {e}");
            std::process::exit(1);
        }
    }
}
