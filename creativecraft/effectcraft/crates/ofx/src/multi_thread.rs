//! MultiThread Suite implementation (`OfxMultiThreadSuiteV1`).

#![allow(unsafe_code, unsafe_op_in_unsafe_fn)]

use std::ffi::{c_int, c_uint, c_void};
use std::sync::Mutex;

use crate::ffi::*;

pub static MULTI_THREAD_SUITE: OfxMultiThreadSuiteV1 = OfxMultiThreadSuiteV1 {
    multi_thread,
    multi_thread_num_cpus,
    multi_thread_index,
    multi_thread_is_spawned_thread,
    mutex_create,
    mutex_destroy,
    mutex_lock,
    mutex_unlock,
    mutex_try_lock,
};

thread_local! {
    static CURRENT_THREAD_INDEX: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static IS_SPAWNED_THREAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

unsafe extern "C" fn multi_thread(func: OfxThreadFunctionV1, n_threads: c_uint, custom_arg: *mut c_void) -> OfxStatus {
    if n_threads <= 1 {
        CURRENT_THREAD_INDEX.with(|i| i.set(0));
        IS_SPAWNED_THREAD.with(|s| s.set(false));
        func(0, 1, custom_arg);
        return OFX_STAT_OK;
    }

    // Spawn scoped threads to run tasks in parallel
    std::thread::scope(|s| {
        let mut handles = Vec::with_capacity(n_threads as usize);
        for thread_idx in 0..n_threads {
            let arg = custom_arg as usize; // Pass pointer as usize across threads
            let handle = s.spawn(move || {
                CURRENT_THREAD_INDEX.with(|i| i.set(thread_idx));
                IS_SPAWNED_THREAD.with(|s| s.set(true));
                func(thread_idx, n_threads, arg as *mut c_void);
            });
            handles.push(handle);
        }
        for h in handles {
            let _ = h.join();
        }
    });

    OFX_STAT_OK
}

unsafe extern "C" fn multi_thread_num_cpus(n_cpus: *mut c_uint) -> OfxStatus {
    if n_cpus.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    let count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    *n_cpus = count as c_uint;
    OFX_STAT_OK
}

unsafe extern "C" fn multi_thread_index(thread_index: *mut c_uint) -> OfxStatus {
    if thread_index.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    *thread_index = CURRENT_THREAD_INDEX.with(|i| i.get());
    OFX_STAT_OK
}

unsafe extern "C" fn multi_thread_is_spawned_thread() -> c_int {
    if IS_SPAWNED_THREAD.with(|s| s.get()) { 1 } else { 0 }
}

struct OfxMutexWrapper {
    _inner: Mutex<()>,
}

unsafe extern "C" fn mutex_create(mutex: *mut OfxMutexHandle, _count: c_int) -> OfxStatus {
    if mutex.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    let m = Box::new(OfxMutexWrapper { _inner: Mutex::new(()) });
    *mutex = Box::into_raw(m) as *mut c_void;
    OFX_STAT_OK
}

unsafe extern "C" fn mutex_destroy(mutex: OfxMutexHandle) -> OfxStatus {
    if mutex.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    let _ = Box::from_raw(mutex as *mut OfxMutexWrapper);
    OFX_STAT_OK
}

unsafe extern "C" fn mutex_lock(mutex: OfxMutexHandle) -> OfxStatus {
    if mutex.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    OFX_STAT_OK
}

unsafe extern "C" fn mutex_unlock(mutex: OfxMutexHandle) -> OfxStatus {
    if mutex.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    OFX_STAT_OK
}

unsafe extern "C" fn mutex_try_lock(mutex: OfxMutexHandle) -> OfxStatus {
    if mutex.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    OFX_STAT_OK
}
