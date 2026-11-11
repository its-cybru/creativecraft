//! Progress Suite implementation (`OfxProgressSuiteV1`).
//!
//! Plug-ins call this while a long operation runs. EffectCraft renders in the
//! background, so the report is simply logged at debug level.

#![allow(unsafe_code, unsafe_op_in_unsafe_fn)]

use std::ffi::{CStr, c_char, c_double, c_void};

use crate::ffi::*;

pub static PROGRESS_SUITE: OfxProgressSuiteV1 = OfxProgressSuiteV1 { progress_start, progress_update, progress_end };

unsafe extern "C" fn progress_start(_instance: *mut c_void, message: *const c_char) -> OfxStatus {
    if !message.is_null()
        && let Ok(text) = CStr::from_ptr(message).to_str()
    {
        log::debug!("[OFX Plugin] {text}");
    }
    OFX_STAT_OK
}

unsafe extern "C" fn progress_update(_instance: *mut c_void, progress: c_double) -> OfxStatus {
    log::trace!("[OFX Plugin] progress {:.0}%", progress * 100.0);
    OFX_STAT_OK
}

unsafe extern "C" fn progress_end(_instance: *mut c_void) -> OfxStatus {
    OFX_STAT_OK
}
