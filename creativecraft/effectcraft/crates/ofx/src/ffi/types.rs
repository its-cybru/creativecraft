//! OpenFX C ABI types, structs, constants, and suites.
//!
//! Based on the OpenFX (OFX) Image Effect API v1.4 (Academy Software Foundation).

use std::ffi::{c_char, c_double, c_int, c_uint, c_void};

// ---------------------------------------------------------------- Status codes

pub type OfxStatus = c_int;

// Values from ofxCore.h — the numbering matters: plug-ins return these and the
// host has to read them the same way (e.g. 4 is "missing host feature", not
// "bad handle").
pub const OFX_STAT_OK: OfxStatus = 0;
pub const OFX_STAT_FAILED: OfxStatus = 1;
pub const OFX_STAT_ERR_FATAL: OfxStatus = 2;
pub const OFX_STAT_ERR_UNKNOWN: OfxStatus = 3;
pub const OFX_STAT_ERR_MISSING_HOST_FEATURE: OfxStatus = 4;
pub const OFX_STAT_ERR_UNSUPPORTED: OfxStatus = 5;
pub const OFX_STAT_ERR_EXISTS: OfxStatus = 6;
pub const OFX_STAT_ERR_FORMAT: OfxStatus = 7;
pub const OFX_STAT_ERR_MEMORY: OfxStatus = 8;
pub const OFX_STAT_ERR_BAD_HANDLE: OfxStatus = 9;
pub const OFX_STAT_ERR_BAD_INDEX: OfxStatus = 10;
pub const OFX_STAT_ERR_VALUE: OfxStatus = 11;
pub const OFX_STAT_REPLY_YES: OfxStatus = 12;
pub const OFX_STAT_REPLY_NO: OfxStatus = 13;
pub const OFX_STAT_REPLY_DEFAULT: OfxStatus = 14;
pub const OFX_STAT_UNLICENSED: OfxStatus = 15;

pub type OfxTime = c_double;

// ---------------------------------------------------------------- Handles

pub type OfxPropertySetHandle = *mut c_void;
pub type OfxImageEffectHandle = *mut c_void;
pub type OfxImageClipHandle = *mut c_void;
pub type OfxParamSetHandle = *mut c_void;
pub type OfxParamHandle = *mut c_void;
pub type OfxInteractHandle = *mut c_void;
pub type OfxImageMemoryHandle = *mut c_void;
pub type OfxMutexHandle = *mut c_void;

// ---------------------------------------------------------------- Geometric structs

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct OfxPointI {
    pub x: c_int,
    pub y: c_int,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct OfxPointD {
    pub x: c_double,
    pub y: c_double,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct OfxRangeI {
    pub min: c_int,
    pub max: c_int,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct OfxRangeD {
    pub min: c_double,
    pub max: c_double,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct OfxRectI {
    pub x1: c_int,
    pub y1: c_int,
    pub x2: c_int,
    pub y2: c_int,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct OfxRectD {
    pub x1: c_double,
    pub y1: c_double,
    pub x2: c_double,
    pub y2: c_double,
}

// ---------------------------------------------------------------- Core Plugin & Host structs

#[repr(C)]
pub struct OfxHost {
    pub host: OfxPropertySetHandle,
    pub fetch_suite: unsafe extern "C" fn(host: OfxPropertySetHandle, suite_name: *const c_char, suite_version: c_int) -> *const c_void,
}

unsafe impl Send for OfxHost {}
unsafe impl Sync for OfxHost {}

#[repr(C)]
pub struct OfxPlugin {
    pub plugin_api: *const c_char,
    pub api_version: c_int,
    pub plugin_identifier: *const c_char,
    pub plugin_version_major: c_uint,
    pub plugin_version_minor: c_uint,
    pub set_host: unsafe extern "C" fn(host: *mut OfxHost),
    pub main_entry:
        unsafe extern "C" fn(action: *const c_char, handle: *const c_void, in_args: OfxPropertySetHandle, out_args: OfxPropertySetHandle) -> OfxStatus,
}

// ---------------------------------------------------------------- Property Suite V1

#[repr(C)]
pub struct OfxPropertySuiteV1 {
    pub prop_set_pointer: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: *mut c_void) -> OfxStatus,
    pub prop_set_string: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: *const c_char) -> OfxStatus,
    pub prop_set_double: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: c_double) -> OfxStatus,
    pub prop_set_int: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: c_int) -> OfxStatus,
    pub prop_set_pointer_n:
        unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *const *mut c_void) -> OfxStatus,
    pub prop_set_string_n:
        unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *const *const c_char) -> OfxStatus,
    pub prop_set_double_n: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *const c_double) -> OfxStatus,
    pub prop_set_int_n: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *const c_int) -> OfxStatus,

    pub prop_get_pointer: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: *mut *mut c_void) -> OfxStatus,
    pub prop_get_string: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: *mut *mut c_char) -> OfxStatus,
    pub prop_get_double: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: *mut c_double) -> OfxStatus,
    pub prop_get_int: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: *mut c_int) -> OfxStatus,
    pub prop_get_pointer_n: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *mut *mut c_void) -> OfxStatus,
    pub prop_get_string_n: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *mut *mut c_char) -> OfxStatus,
    pub prop_get_double_n: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *mut c_double) -> OfxStatus,
    pub prop_get_int_n: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *mut c_int) -> OfxStatus,

    pub prop_reset: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char) -> OfxStatus,
    pub prop_get_dimension: unsafe extern "C" fn(properties: OfxPropertySetHandle, property: *const c_char, dimension: *mut c_int) -> OfxStatus,
}

// ---------------------------------------------------------------- Parameter Suite V1

#[repr(C)]
pub struct OfxParameterSuiteV1 {
    pub param_define: unsafe extern "C" fn(
        param_set: OfxParamSetHandle,
        param_type: *const c_char,
        name: *const c_char,
        property_set: *mut OfxPropertySetHandle,
    ) -> OfxStatus,
    pub param_get_handle: unsafe extern "C" fn(
        param_set: OfxParamSetHandle,
        name: *const c_char,
        param: *mut OfxParamHandle,
        property_set: *mut OfxPropertySetHandle,
    ) -> OfxStatus,
    pub param_set_get_property_set: unsafe extern "C" fn(param_set: OfxParamSetHandle, prop_handle: *mut OfxPropertySetHandle) -> OfxStatus,
    pub param_get_property_set: unsafe extern "C" fn(param: OfxParamHandle, prop_handle: *mut OfxPropertySetHandle) -> OfxStatus,

    pub param_get_value: unsafe extern "C" fn(param_handle: OfxParamHandle, ...) -> OfxStatus,
    pub param_get_value_at_time: unsafe extern "C" fn(param_handle: OfxParamHandle, time: OfxTime, ...) -> OfxStatus,
    pub param_get_derivative: unsafe extern "C" fn(param_handle: OfxParamHandle, time: OfxTime, ...) -> OfxStatus,
    pub param_get_integral: unsafe extern "C" fn(param_handle: OfxParamHandle, time1: OfxTime, time2: OfxTime, ...) -> OfxStatus,

    pub param_set_value: unsafe extern "C" fn(param_handle: OfxParamHandle, ...) -> OfxStatus,
    pub param_set_value_at_time: unsafe extern "C" fn(param_handle: OfxParamHandle, time: OfxTime, ...) -> OfxStatus,

    pub param_get_num_keys: unsafe extern "C" fn(param_handle: OfxParamHandle, num_keys: *mut c_uint) -> OfxStatus,
    pub param_get_key_time: unsafe extern "C" fn(param_handle: OfxParamHandle, nth_key: c_uint, time: *mut OfxTime) -> OfxStatus,
    pub param_get_key_index: unsafe extern "C" fn(param_handle: OfxParamHandle, time: OfxTime, direction: c_int, index: *mut c_int) -> OfxStatus,
    pub param_delete_key: unsafe extern "C" fn(param_handle: OfxParamHandle, time: OfxTime) -> OfxStatus,
    pub param_delete_all_keys: unsafe extern "C" fn(param_handle: OfxParamHandle) -> OfxStatus,

    pub param_copy: unsafe extern "C" fn(param_to: OfxParamHandle, param_from: OfxParamHandle, dst_offset: OfxTime, frame_range: *const OfxRangeD) -> OfxStatus,
    pub param_edit_begin: unsafe extern "C" fn(param_set: OfxParamSetHandle, name: *const c_char) -> OfxStatus,
    pub param_edit_end: unsafe extern "C" fn(param_set: OfxParamSetHandle) -> OfxStatus,
}

// ---------------------------------------------------------------- Image Effect Suite V1

#[repr(C)]
pub struct OfxImageEffectSuiteV1 {
    pub get_property_set: unsafe extern "C" fn(image_effect: OfxImageEffectHandle, prop_handle: *mut OfxPropertySetHandle) -> OfxStatus,
    pub get_param_set: unsafe extern "C" fn(image_effect: OfxImageEffectHandle, param_set: *mut OfxParamSetHandle) -> OfxStatus,
    pub clip_define: unsafe extern "C" fn(image_effect: OfxImageEffectHandle, name: *const c_char, property_set: *mut OfxPropertySetHandle) -> OfxStatus,
    pub clip_get_handle: unsafe extern "C" fn(
        image_effect: OfxImageEffectHandle,
        name: *const c_char,
        clip: *mut OfxImageClipHandle,
        property_set: *mut OfxPropertySetHandle,
    ) -> OfxStatus,
    pub clip_get_property_set: unsafe extern "C" fn(clip: OfxImageClipHandle, prop_handle: *mut OfxPropertySetHandle) -> OfxStatus,
    pub clip_get_image:
        unsafe extern "C" fn(clip: OfxImageClipHandle, time: OfxTime, region: *const OfxRectD, image_handle: *mut OfxPropertySetHandle) -> OfxStatus,
    pub clip_release_image: unsafe extern "C" fn(image_handle: OfxPropertySetHandle) -> OfxStatus,
    pub clip_get_region_of_definition: unsafe extern "C" fn(clip: OfxImageClipHandle, time: OfxTime, bounds: *mut OfxRectD) -> OfxStatus,
    pub abort: unsafe extern "C" fn(image_effect: OfxImageEffectHandle) -> c_int,
    pub image_memory_alloc: unsafe extern "C" fn(image_effect: OfxImageEffectHandle, n_bytes: usize, memory_handle: *mut OfxImageMemoryHandle) -> OfxStatus,
    pub image_memory_free: unsafe extern "C" fn(memory_handle: OfxImageMemoryHandle) -> OfxStatus,
    pub image_memory_lock: unsafe extern "C" fn(memory_handle: OfxImageMemoryHandle, returned_ptr: *mut *mut c_void) -> OfxStatus,
    pub image_memory_unlock: unsafe extern "C" fn(memory_handle: OfxImageMemoryHandle) -> OfxStatus,
}

// ---------------------------------------------------------------- Memory Suite V1

#[repr(C)]
pub struct OfxMemorySuiteV1 {
    pub memory_alloc: unsafe extern "C" fn(handle: *mut c_void, n_bytes: usize, allocated_data: *mut *mut c_void) -> OfxStatus,
    pub memory_free: unsafe extern "C" fn(allocated_data: *mut c_void) -> OfxStatus,
}

// ---------------------------------------------------------------- Interact Suite V1

/// Custom overlay GUIs. EffectCraft does not draw plug-in overlays, but several
/// plug-ins (Red Giant Universe, for one) refuse to describe themselves when the
/// suite is absent, so the three entry points exist and politely decline.
#[repr(C)]
pub struct OfxInteractSuiteV1 {
    pub interact_swap_buffers: unsafe extern "C" fn(interact_instance: OfxInteractHandle) -> OfxStatus,
    pub interact_redraw: unsafe extern "C" fn(interact_instance: OfxInteractHandle) -> OfxStatus,
    pub interact_get_property_set: unsafe extern "C" fn(interact_instance: OfxInteractHandle, property: *mut OfxPropertySetHandle) -> OfxStatus,
}

// ---------------------------------------------------------------- OpenGL Render Suite V1

/// Host-side image-to-texture conversion, from `ofxGPURender.h`.
#[repr(C)]
pub struct OfxImageEffectOpenGLRenderSuiteV1 {
    pub clip_load_texture: unsafe extern "C" fn(
        clip: OfxImageClipHandle,
        time: OfxTime,
        format: *const c_char,
        region: *const OfxRectD,
        texture_handle: *mut OfxPropertySetHandle,
    ) -> OfxStatus,
    pub clip_free_texture: unsafe extern "C" fn(texture_handle: OfxPropertySetHandle) -> OfxStatus,
    pub flush_resources: unsafe extern "C" fn() -> OfxStatus,
}

// ---------------------------------------------------------------- MultiThread Suite V1

pub type OfxThreadFunctionV1 = unsafe extern "C" fn(thread_index: c_uint, thread_count: c_uint, custom_arg: *mut c_void);

#[repr(C)]
pub struct OfxMultiThreadSuiteV1 {
    pub multi_thread: unsafe extern "C" fn(func: OfxThreadFunctionV1, n_threads: c_uint, custom_arg: *mut c_void) -> OfxStatus,
    pub multi_thread_num_cpus: unsafe extern "C" fn(n_cpus: *mut c_uint) -> OfxStatus,
    pub multi_thread_index: unsafe extern "C" fn(thread_index: *mut c_uint) -> OfxStatus,
    /// Returns a flag directly (not an OfxStatus) — see ofxCore.h.
    pub multi_thread_is_spawned_thread: unsafe extern "C" fn() -> c_int,
    pub mutex_create: unsafe extern "C" fn(mutex: *mut OfxMutexHandle, count: c_int) -> OfxStatus,
    pub mutex_destroy: unsafe extern "C" fn(mutex: OfxMutexHandle) -> OfxStatus,
    pub mutex_lock: unsafe extern "C" fn(mutex: OfxMutexHandle) -> OfxStatus,
    pub mutex_unlock: unsafe extern "C" fn(mutex: OfxMutexHandle) -> OfxStatus,
    pub mutex_try_lock: unsafe extern "C" fn(mutex: OfxMutexHandle) -> OfxStatus,
}

// ---------------------------------------------------------------- Progress Suite V1

#[repr(C)]
pub struct OfxProgressSuiteV1 {
    pub progress_start: unsafe extern "C" fn(instance: *mut c_void, message: *const c_char) -> OfxStatus,
    pub progress_update: unsafe extern "C" fn(instance: *mut c_void, progress: c_double) -> OfxStatus,
    pub progress_end: unsafe extern "C" fn(instance: *mut c_void) -> OfxStatus,
}

// ---------------------------------------------------------------- TimeLine Suite V1

#[repr(C)]
pub struct OfxTimeLineSuiteV1 {
    pub get_time: unsafe extern "C" fn(instance: *mut c_void, time: *mut c_double) -> OfxStatus,
    pub goto_time: unsafe extern "C" fn(instance: *mut c_void, time: c_double) -> OfxStatus,
    pub get_time_bounds: unsafe extern "C" fn(instance: *mut c_void, first: *mut c_double, last: *mut c_double) -> OfxStatus,
}

// ---------------------------------------------------------------- Message Suite V1

#[repr(C)]
pub struct OfxMessageSuiteV1 {
    pub message: unsafe extern "C" fn(handle: *mut c_void, message_type: *const c_char, message_id: *const c_char, format: *const c_char, ...) -> OfxStatus,
}
