//! Memory Suite implementation (`OfxMemorySuiteV1`).

#![allow(unsafe_code, unsafe_op_in_unsafe_fn)]

use std::alloc::{Layout, alloc, dealloc};
use std::ffi::c_void;

use crate::ffi::*;

pub static MEMORY_SUITE: OfxMemorySuiteV1 = OfxMemorySuiteV1 { memory_alloc, memory_free };

// Store prefix size before allocated block so we can deallocate with proper layout
const HEADER_SIZE: usize = 16;

unsafe extern "C" fn memory_alloc(_handle: *mut c_void, n_bytes: usize, allocated_data: *mut *mut c_void) -> OfxStatus {
    if allocated_data.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }

    let total = n_bytes + HEADER_SIZE;
    let Ok(layout) = Layout::from_size_align(total, 16) else {
        return OFX_STAT_ERR_MEMORY;
    };
    let ptr = alloc(layout);
    if ptr.is_null() {
        return OFX_STAT_ERR_MEMORY;
    }

    // Store total size in header
    *(ptr as *mut usize) = total;

    *allocated_data = ptr.add(HEADER_SIZE) as *mut c_void;
    OFX_STAT_OK
}

unsafe extern "C" fn memory_free(allocated_data: *mut c_void) -> OfxStatus {
    if allocated_data.is_null() {
        return OFX_STAT_OK;
    }

    let ptr = (allocated_data as *mut u8).sub(HEADER_SIZE);
    let total = *(ptr as *const usize);
    if let Ok(layout) = Layout::from_size_align(total, 16) {
        dealloc(ptr, layout);
    }
    OFX_STAT_OK
}
