//! # effectcraft-ofx
//!
//! OpenFX (OFX) standard host for EffectCraft.
//!
//! Plug-ins are loaded in a helper process (see [`worker`]) so a third-party
//! binary cannot take EffectCraft down: a bundle that crashes or hangs is
//! killed, remembered, and skipped. The host application opts in with
//! [`worker::configure_worker`] and handles [`worker::WORKER_ARG`] in its `main`.

#![allow(unsafe_code)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::missing_safety_doc)]

pub mod ffi;
pub mod host;
pub mod image_effect;
pub mod interact;
pub mod memory;
pub mod message;
pub mod multi_thread;
pub mod opengl;
pub mod parameter;
pub mod plugin;
pub mod progress;
pub mod property;
pub mod scan;
pub mod timeline;
pub mod worker;

use std::path::Path;

pub use plugin::OfxEffectPlugin;
pub use scan::{default_plugin_directories, find_ofx_binaries, load_ofx_library};
pub use worker::{RemotePlugin, clear_skips, configure_worker, describe_and_register, scan_and_register_all, shutdown, skipped_bundles};

#[cfg(test)]
mod tests;

/// Load an individual `.ofx` file and register its effects **in this process**.
///
/// This is only safe for plug-ins known to behave (the probe and tests use it).
/// Applications must use [`describe_and_register`] so loading happens in the
/// helper process instead.
pub fn load_and_register(path: &Path) -> Result<usize, String> {
    let effects = load_ofx_library(path)?;
    let mut count = 0;
    for effect in effects {
        effectcraft_effects::plugin::register_plugin(effect)?;
        count += 1;
    }
    Ok(count)
}
