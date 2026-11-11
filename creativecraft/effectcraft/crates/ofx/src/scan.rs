//! Scans standard and custom paths for OpenFX plugin bundles.

#![allow(unsafe_code, unsafe_op_in_unsafe_fn)]

use std::ffi::c_int;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::ffi::OfxPlugin;
use crate::host::get_ofx_host;
use crate::plugin::OfxEffectPlugin;

type GetNumberOfPluginsFn = unsafe extern "C" fn() -> c_int;
type GetPluginFn = unsafe extern "C" fn(nth: c_int) -> *const OfxPlugin;

#[cfg(windows)]
unsafe extern "system" {
    fn SetDllDirectoryW(lpPathName: *const u16) -> i32;
    fn GetDllDirectoryW(nBufferLength: u32, lpBuffer: *mut u16) -> u32;
}

/// Discover standard OFX plugin directories on this platform.
pub fn default_plugin_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    #[cfg(windows)]
    {
        if let Ok(common) = std::env::var("CommonProgramFiles") {
            let p = PathBuf::from(common).join("OFX").join("Plugins");
            if p.exists() {
                dirs.push(p);
            }
        }
        let fallback = PathBuf::from(r"C:\Program Files\Common Files\OFX\Plugins");
        if fallback.exists() && !dirs.contains(&fallback) {
            dirs.push(fallback);
        }
        // Vendors that install outside the shared OFX folder. The scan walks
        // each tree, so a vendor's own layout (Plugins\OFX, Universe\OFX, …)
        // needs no separate entry.
        for vendor in [
            r"C:\Program Files\Red Giant",
            r"C:\Program Files\Boris FX",
            r"C:\Program Files\BorisFX",
            r"C:\Program Files\GenArts",
            r"C:\Program Files\Sapphire",
        ] {
            let path = PathBuf::from(vendor);
            if path.is_dir() && !dirs.contains(&path) {
                dirs.push(path);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/Library/OFX/Plugins"));
    }

    #[cfg(target_os = "linux")]
    {
        dirs.push(PathBuf::from("/usr/OFX/Plugins"));
    }

    if let Ok(custom) = std::env::var("OFX_PLUGIN_PATH") {
        for part in std::env::split_paths(&custom) {
            if part.exists() && !dirs.contains(&part) {
                dirs.push(part);
            }
        }
    }

    dirs
}

/// Recursively find all `.ofx` dynamic library files in a directory.
pub fn find_ofx_binaries(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if !dir.is_dir() {
        return results;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                results.extend(find_ofx_binaries(&path));
            } else if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext.eq_ignore_ascii_case("ofx") {
                        results.push(path);
                    }
                }
            }
        }
    }
    results
}

/// Load an individual `.ofx` dynamic library and extract all its plugins.
pub fn load_ofx_library(path: &Path) -> Result<Vec<Arc<OfxEffectPlugin>>, String> {
    // Plug-ins link against sibling DLLs in their bundle; Windows resolves them
    // relative to the DLL directory, so point it at the containing folder.
    #[cfg(windows)]
    let _dll_dir = DllDirectoryGuard::new(path.parent());

    let res = unsafe {
        let lib = libloading::Library::new(path).map_err(|e| format!("failed to load {}: {e}", path.display()))?;
        let lib_arc = Arc::new(lib);

        let get_num_plugins: libloading::Symbol<GetNumberOfPluginsFn> =
            lib_arc.get(b"OfxGetNumberOfPlugins\0").map_err(|e| format!("missing OfxGetNumberOfPlugins in {}: {e}", path.display()))?;

        let get_plugin: libloading::Symbol<GetPluginFn> =
            lib_arc.get(b"OfxGetPlugin\0").map_err(|e| format!("missing OfxGetPlugin in {}: {e}", path.display()))?;

        // OFX 1.4 hosts may call the optional OfxSetHost export before asking
        // for the plug-in count; a plug-in can use it to decline the host.
        if let Ok(set_host) = lib_arc.get::<SetHostFn>(b"OfxSetHost\0") {
            let status = set_host(get_ofx_host() as *const crate::ffi::OfxHost);
            if status == crate::ffi::OFX_STAT_FAILED {
                return Err("the bundle declined this host in OfxSetHost".into());
            }
        }

        let count = get_num_plugins();
        if count <= 0 {
            return Err("the bundle exports no plug-ins".into());
        }
        let mut plugins = Vec::new();

        for i in 0..count {
            let raw_plugin = get_plugin(i);
            if !raw_plugin.is_null() {
                match OfxEffectPlugin::from_raw(raw_plugin, lib_arc.clone()) {
                    Ok(effect) => {
                        log::info!("[OFX Host] Loaded effect '{}' ({}) from {}", effect.manifest.name, effect.manifest.id, path.display());
                        plugins.push(Arc::new(effect));
                    }
                    Err(e) => {
                        log::warn!("[OFX Host] Skipped plugin {} in {}: {}", i, path.display(), e);
                    }
                }
            }
        }

        Ok(plugins)
    };

    res
}

type SetHostFn = unsafe extern "C" fn(host: *const crate::ffi::OfxHost) -> c_int;

#[cfg(windows)]
struct DllDirectoryGuard {
    previous: Option<Vec<u16>>,
}

#[cfg(windows)]
impl DllDirectoryGuard {
    fn new(dir: Option<&Path>) -> Self {
        use std::os::windows::ffi::OsStrExt;
        let previous = unsafe {
            let len = GetDllDirectoryW(0, std::ptr::null_mut());
            if len > 0 {
                let mut buf = vec![0u16; len as usize];
                let copied = GetDllDirectoryW(len, buf.as_mut_ptr());
                buf.truncate(copied.max(0) as usize);
                Some(buf)
            } else {
                None
            }
        };
        if let Some(parent) = dir {
            let wide: Vec<u16> = parent.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
            unsafe {
                SetDllDirectoryW(wide.as_ptr());
            }
        }
        Self { previous }
    }
}

#[cfg(windows)]
impl Drop for DllDirectoryGuard {
    fn drop(&mut self) {
        let previous = self.previous.take();
        unsafe {
            match previous {
                Some(buf) if !buf.is_empty() => {
                    let mut wide = buf;
                    wide.push(0);
                    SetDllDirectoryW(wide.as_ptr());
                }
                _ => {
                    SetDllDirectoryW(std::ptr::null());
                }
            }
        }
    }
}
