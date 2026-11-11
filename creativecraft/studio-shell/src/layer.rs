//! One rendered layer, ready to become an EffectCraft composition layer.

use std::path::PathBuf;

/// A PNG of one design element.
pub struct LayerPng {
    pub name: String,
    pub path: PathBuf,
    /// Centre of the element in document pixels. `None` when the PNG already
    /// covers the whole frame, so the composition's centre is correct.
    pub position: Option<(f64, f64)>,
}

/// A filesystem-safe, readable file name for layer `index`.
///
/// EffectCraft names the composition layer after the file, so this keeps the
/// design's own names ("Pro plan", "Choose plan") visible in its Timeline. It
/// ends in a letter so the importer never mistakes the numbered files for an
/// image sequence.
pub fn file_stem(index: usize, name: &str) -> String {
    let mut clean = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == ' ' || ch == '-' || ch == '_' {
            clean.push(ch);
        } else if !clean.ends_with('-') {
            clean.push('-');
        }
    }
    let clean = clean.trim().trim_matches('-').replace(' ', "-");
    let clean = if clean.is_empty() {
        "layer".to_string()
    } else {
        clean
    };
    let clean: String = clean.chars().take(60).collect();
    format!("{index:02}-{clean}-image")
}
