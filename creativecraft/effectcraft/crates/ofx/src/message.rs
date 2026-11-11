//! Message Suite implementation (`OfxMessageSuiteV1`).

#![allow(unsafe_code, unsafe_op_in_unsafe_fn)]

use crate::ffi::*;
use std::ffi::{CStr, c_char, c_void};

pub static MESSAGE_SUITE: OfxMessageSuiteV1 = OfxMessageSuiteV1 { message };

unsafe extern "C" fn message(_handle: *mut c_void, _message_type: *const c_char, _message_id: *const c_char, format: *const c_char, _args: ...) -> OfxStatus {
    if !format.is_null() {
        if let Ok(s) = CStr::from_ptr(format).to_str() {
            log::info!("[OFX Plugin] {}", s);
        }
    }
    OFX_STAT_OK
}
