//! Render one frame through one OpenFX effect, to check the whole path.
//!
//! `cargo run -p effectcraft-ofx --example render -- <path-to-.ofx> [width] [height]`

use effectcraft_effects::plugin::{PluginFrame, PluginParamKind, PluginParams};

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

fn default_flat(kind: &PluginParamKind) -> Vec<f64> {
    match kind {
        PluginParamKind::Slider { default, .. } => vec![*default],
        PluginParamKind::Angle { default } => vec![*default],
        PluginParamKind::Checkbox { default } => vec![if *default { 1.0 } else { 0.0 }],
        PluginParamKind::Popup { default, .. } => vec![*default as f64],
        PluginParamKind::Point { default } => vec![default[0], default[1]],
        PluginParamKind::Color { default } => default.to_vec(),
    }
}

fn main() {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Trace);

    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: render <path-to-.ofx> [width] [height] [param=value ...]");
        std::process::exit(2);
    };
    let width: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(64);
    let height: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(64);
    let overrides: Vec<(String, f64)> = args.filter_map(|a| a.split_once('=').and_then(|(k, v)| Some((k.to_string(), v.parse().ok()?)))).collect();

    let effects = match effectcraft_ofx::load_ofx_library(std::path::Path::new(&path)) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("load failed: {e}");
            std::process::exit(1);
        }
    };
    let Some(effect) = effects.first() else {
        eprintln!("no effects");
        std::process::exit(1);
    };

    let mut flat: Vec<f64> = effect.manifest.params.iter().flat_map(|p| default_flat(&p.kind)).collect();
    // Apply `id=value` overrides to the flattened parameter list.
    let mut offset = 0;
    for p in &effect.manifest.params {
        let w = p.kind.width();
        if let Some((_, v)) = overrides.iter().find(|(k, _)| *k == p.id)
            && offset < flat.len()
        {
            flat[offset] = *v;
        }
        offset += w;
    }
    let mut pixels = vec![[0.0f32; 4]; width as usize * height as usize];
    for (i, px) in pixels.iter_mut().enumerate() {
        let x = (i % width as usize) as f32 / width as f32;
        let y = (i / width as usize) as f32 / height as f32;
        *px = [x, y, 0.25, 1.0];
    }
    let before: Vec<[f32; 4]> = pixels.clone();

    let params = PluginParams { manifest: &effect.manifest, flat: &flat };
    let mut frame = PluginFrame { width, height, pixels: &mut pixels, scale: 1.0 };

    use effectcraft_effects::plugin::EffectPlugin;
    match effect.render(&mut frame, &params, 0.0) {
        Ok(()) => {
            let changed = pixels.iter().zip(&before).filter(|(a, b)| a != b).count();
            println!("ok '{}' {}x{} — {} of {} pixels changed; first = {:?}", effect.manifest.name, width, height, changed, pixels.len(), pixels[0]);
        }
        Err(e) => {
            eprintln!("render failed: {e}");
            std::process::exit(1);
        }
    }
}
