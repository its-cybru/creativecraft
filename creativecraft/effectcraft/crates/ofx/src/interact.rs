//! Interact Suite implementation (`OfxInteractSuiteV1`).
//!
//! OFX plug-ins can draw their own on-screen handles ("interacts"). EffectCraft
//! does not render those overlays, so the suite reports that it cannot: honest
//! answers instead of pretending, and enough of the suite that plug-ins which
//! merely *probe* for it (Red Giant Universe) can still describe their effects.

#![allow(unsafe_code, unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;

use crate::ffi::*;

pub static INTERACT_SUITE: OfxInteractSuiteV1 = OfxInteractSuiteV1 { interact_swap_buffers, interact_redraw, interact_get_property_set };

unsafe extern "C" fn interact_swap_buffers(_interact: *mut c_void) -> OfxStatus {
    OFX_STAT_ERR_UNSUPPORTED
}

unsafe extern "C" fn interact_redraw(_interact: *mut c_void) -> OfxStatus {
    OFX_STAT_ERR_UNSUPPORTED
}

unsafe extern "C" fn interact_get_property_set(_interact: *mut c_void, _property: *mut OfxPropertySetHandle) -> OfxStatus {
    OFX_STAT_ERR_BAD_HANDLE
}
