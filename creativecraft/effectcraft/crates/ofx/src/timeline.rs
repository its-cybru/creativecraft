//! TimeLine Suite implementation (`OfxTimeLineSuiteV1`).
//!
//! Some plug-ins ask the host which time the effect is being evaluated at, and
//! which part of the timeline exists. EffectCraft knows both for the render it
//! is running, so they live in a thread-local set just before each plug-in
//! action (see [`set_current_time`]).

#![allow(unsafe_code, unsafe_op_in_unsafe_fn)]

use std::cell::Cell;
use std::ffi::{c_double, c_void};

use crate::ffi::*;

pub static TIMELINE_SUITE: OfxTimeLineSuiteV1 = OfxTimeLineSuiteV1 { get_time, goto_time, get_time_bounds };

thread_local! {
    static CURRENT_TIME: Cell<f64> = const { Cell::new(0.0) };
}

/// Record the time of the render about to happen.
pub fn set_current_time(time: f64) {
    CURRENT_TIME.with(|t| t.set(time));
}

/// The time recorded by [`set_current_time`].
pub fn current_time() -> f64 {
    CURRENT_TIME.with(|t| t.get())
}

unsafe extern "C" fn get_time(_instance: *mut c_void, time: *mut c_double) -> OfxStatus {
    if time.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    *time = current_time();
    OFX_STAT_OK
}

unsafe extern "C" fn goto_time(_instance: *mut c_void, time: c_double) -> OfxStatus {
    // The effect cannot move EffectCraft's playhead from inside a render; report
    // the request as unsupported rather than pretending it happened.
    let _ = time;
    OFX_STAT_ERR_UNSUPPORTED
}

unsafe extern "C" fn get_time_bounds(_instance: *mut c_void, first: *mut c_double, last: *mut c_double) -> OfxStatus {
    if first.is_null() || last.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    // A single frame around the current time: truthful for a still render.
    *first = current_time();
    *last = current_time();
    OFX_STAT_OK
}
