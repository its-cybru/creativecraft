//! Property Set implementation and `OfxPropertySuiteV1`.

#![allow(unsafe_code, unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_double, c_int, c_void};
use std::sync::Mutex;

use crate::ffi::*;

#[derive(Clone, Debug)]
pub enum PropVal {
    Pointer(*mut c_void),
    String(CString),
    Double(c_double),
    Int(c_int),
}

unsafe impl Send for PropVal {}
unsafe impl Sync for PropVal {}

#[derive(Default, Debug)]
pub struct PropertySet {
    pub props: Mutex<HashMap<String, Vec<PropVal>>>,
    /// Host descriptors answer unknown properties with a zero/empty value:
    /// plug-ins probe for optional host capabilities and treat a missing one as
    /// fatal (`kOfxStatErrBadIndex`), which would reject the whole host.
    pub lenient: bool,
}

impl PropertySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lenient() -> Self {
        Self { props: Mutex::new(HashMap::new()), lenient: true }
    }

    pub fn set_string(&self, key: &str, val: &str) {
        if let Ok(cs) = CString::new(val) {
            let mut map = self.props.lock().unwrap_or_else(|e| e.into_inner());
            map.insert(key.to_string(), vec![PropVal::String(cs)]);
        }
    }

    pub fn set_int(&self, key: &str, val: i32) {
        let mut map = self.props.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(key.to_string(), vec![PropVal::Int(val)]);
    }

    pub fn set_double(&self, key: &str, val: f64) {
        let mut map = self.props.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(key.to_string(), vec![PropVal::Double(val)]);
    }

    pub fn set_pointer(&self, key: &str, val: *mut c_void) {
        let mut map = self.props.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(key.to_string(), vec![PropVal::Pointer(val)]);
    }

    pub fn get_string(&self, key: &str, index: usize) -> Option<String> {
        let map = self.props.lock().unwrap_or_else(|e| e.into_inner());
        map.get(key).and_then(|v| v.get(index)).and_then(|p| match p {
            PropVal::String(s) => s.to_str().ok().map(|s| s.to_string()),
            _ => None,
        })
    }

    pub fn get_double(&self, key: &str, index: usize) -> Option<f64> {
        let map = self.props.lock().unwrap_or_else(|e| e.into_inner());
        map.get(key).and_then(|v| v.get(index)).and_then(|p| match p {
            PropVal::Double(d) => Some(*d),
            _ => None,
        })
    }

    pub fn get_int(&self, key: &str, index: usize) -> Option<i32> {
        let map = self.props.lock().unwrap_or_else(|e| e.into_inner());
        map.get(key).and_then(|v| v.get(index)).and_then(|p| match p {
            PropVal::Int(i) => Some(*i),
            _ => None,
        })
    }

    pub fn get_pointer(&self, key: &str, index: usize) -> Option<*mut c_void> {
        let map = self.props.lock().unwrap_or_else(|e| e.into_inner());
        map.get(key).and_then(|v| v.get(index)).and_then(|p| match p {
            PropVal::Pointer(ptr) => Some(*ptr),
            _ => None,
        })
    }

    pub fn get_all_strings(&self, key: &str) -> Vec<String> {
        let map = self.props.lock().unwrap_or_else(|e| e.into_inner());
        map.get(key)
            .map(|vals| {
                vals.iter()
                    .filter_map(|p| match p {
                        PropVal::String(s) => s.to_str().ok().map(|s| s.to_string()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Log every property a plug-in has set (development aid for diagnosing a
    /// bundle that refuses the host).
    pub fn log_dump(&self, context: &str) {
        let map = self.props.lock().unwrap_or_else(|e| e.into_inner());
        for (key, values) in map.iter() {
            let shown: Vec<String> = values
                .iter()
                .map(|v| match v {
                    PropVal::Pointer(p) => format!("ptr({p:p})"),
                    PropVal::String(s) => format!("{:?}", s.to_string_lossy()),
                    PropVal::Double(d) => format!("{d}"),
                    PropVal::Int(i) => format!("{i}"),
                })
                .collect();
            log::trace!("[ofx] {context}: {key} = [{}]", shown.join(", "));
        }
    }

    /// Replace every property with a copy of another set's (used when a plug-in
    /// descriptor's parameters become an instance's parameters).
    pub fn copy_from(&self, other: &PropertySet) {
        if std::ptr::eq(self, other) {
            return;
        }
        let source = other.props.lock().unwrap_or_else(|e| e.into_inner());
        let mut dest = self.props.lock().unwrap_or_else(|e| e.into_inner());
        *dest = source.clone();
    }
}

// ---------------------------------------------------------------- C Suite implementation

pub static PROPERTY_SUITE: OfxPropertySuiteV1 = OfxPropertySuiteV1 {
    prop_set_pointer,
    prop_set_string,
    prop_set_double,
    prop_set_int,
    prop_set_pointer_n,
    prop_set_string_n,
    prop_set_double_n,
    prop_set_int_n,
    prop_get_pointer,
    prop_get_string,
    prop_get_double,
    prop_get_int,
    prop_get_pointer_n,
    prop_get_string_n,
    prop_get_double_n,
    prop_get_int_n,
    prop_reset,
    prop_get_dimension,
};

unsafe fn to_prop_set<'a>(handle: OfxPropertySetHandle) -> Option<&'a PropertySet> {
    if handle.is_null() { None } else { Some(&*(handle as *const PropertySet)) }
}

unsafe fn to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() { None } else { CStr::from_ptr(ptr).to_str().ok() }
}

unsafe extern "C" fn prop_set_pointer(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: *mut c_void) -> OfxStatus {
    let Some(props) = to_prop_set(properties) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let Some(key) = to_str(property) else {
        return OFX_STAT_ERR_VALUE;
    };
    if index < 0 {
        return OFX_STAT_ERR_BAD_INDEX;
    }

    let mut map = props.props.lock().unwrap_or_else(|e| e.into_inner());
    let entry = map.entry(key.to_string()).or_default();
    let idx = index as usize;
    if entry.len() <= idx {
        entry.resize(idx + 1, PropVal::Pointer(std::ptr::null_mut()));
    }
    entry[idx] = PropVal::Pointer(value);
    OFX_STAT_OK
}

unsafe extern "C" fn prop_set_string(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: *const c_char) -> OfxStatus {
    let Some(props) = to_prop_set(properties) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let Some(key) = to_str(property) else {
        return OFX_STAT_ERR_VALUE;
    };
    if index < 0 {
        return OFX_STAT_ERR_BAD_INDEX;
    }
    let cstr = if value.is_null() { CString::default() } else { CStr::from_ptr(value).to_owned() };

    let mut map = props.props.lock().unwrap_or_else(|e| e.into_inner());
    let entry = map.entry(key.to_string()).or_default();
    let idx = index as usize;
    if entry.len() <= idx {
        entry.resize(idx + 1, PropVal::String(CString::default()));
    }
    entry[idx] = PropVal::String(cstr);
    OFX_STAT_OK
}

unsafe extern "C" fn prop_set_double(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: c_double) -> OfxStatus {
    let Some(props) = to_prop_set(properties) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let Some(key) = to_str(property) else {
        return OFX_STAT_ERR_VALUE;
    };
    if index < 0 {
        return OFX_STAT_ERR_BAD_INDEX;
    }

    let mut map = props.props.lock().unwrap_or_else(|e| e.into_inner());
    let entry = map.entry(key.to_string()).or_default();
    let idx = index as usize;
    if entry.len() <= idx {
        entry.resize(idx + 1, PropVal::Double(0.0));
    }
    entry[idx] = PropVal::Double(value);
    OFX_STAT_OK
}

unsafe extern "C" fn prop_set_int(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: c_int) -> OfxStatus {
    let Some(props) = to_prop_set(properties) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let Some(key) = to_str(property) else {
        return OFX_STAT_ERR_VALUE;
    };
    if index < 0 {
        return OFX_STAT_ERR_BAD_INDEX;
    }

    let mut map = props.props.lock().unwrap_or_else(|e| e.into_inner());
    let entry = map.entry(key.to_string()).or_default();
    let idx = index as usize;
    if entry.len() <= idx {
        entry.resize(idx + 1, PropVal::Int(0));
    }
    entry[idx] = PropVal::Int(value);
    OFX_STAT_OK
}

unsafe extern "C" fn prop_set_pointer_n(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *const *mut c_void) -> OfxStatus {
    for i in 0..count {
        let val = *value.add(i as usize);
        let status = prop_set_pointer(properties, property, i, val);
        if status != OFX_STAT_OK {
            return status;
        }
    }
    OFX_STAT_OK
}

unsafe extern "C" fn prop_set_string_n(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *const *const c_char) -> OfxStatus {
    for i in 0..count {
        let val = *value.add(i as usize);
        let status = prop_set_string(properties, property, i, val);
        if status != OFX_STAT_OK {
            return status;
        }
    }
    OFX_STAT_OK
}

unsafe extern "C" fn prop_set_double_n(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *const c_double) -> OfxStatus {
    for i in 0..count {
        let val = *value.add(i as usize);
        let status = prop_set_double(properties, property, i, val);
        if status != OFX_STAT_OK {
            return status;
        }
    }
    OFX_STAT_OK
}

unsafe extern "C" fn prop_set_int_n(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *const c_int) -> OfxStatus {
    for i in 0..count {
        let val = *value.add(i as usize);
        let status = prop_set_int(properties, property, i, val);
        if status != OFX_STAT_OK {
            return status;
        }
    }
    OFX_STAT_OK
}

unsafe extern "C" fn prop_get_pointer(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: *mut *mut c_void) -> OfxStatus {
    if value.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    let Some(props) = to_prop_set(properties) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let Some(key) = to_str(property) else {
        return OFX_STAT_ERR_VALUE;
    };
    if index < 0 {
        return OFX_STAT_ERR_BAD_INDEX;
    }

    let map = props.props.lock().unwrap_or_else(|e| e.into_inner());
    match map.get(key).and_then(|v| v.get(index as usize)) {
        Some(PropVal::Pointer(ptr)) => {
            *value = *ptr;
            OFX_STAT_OK
        }
        _ if props.lenient => {
            log::trace!("[ofx] propGetPointer default for missing '{key}'[{index}]");
            *value = std::ptr::null_mut();
            OFX_STAT_OK
        }
        _ => {
            log::trace!("[ofx] propGetPointer missing '{key}'[{index}]");
            OFX_STAT_ERR_BAD_INDEX
        }
    }
}

unsafe extern "C" fn prop_get_string(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: *mut *mut c_char) -> OfxStatus {
    if value.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    let Some(props) = to_prop_set(properties) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let Some(key) = to_str(property) else {
        return OFX_STAT_ERR_VALUE;
    };
    if index < 0 {
        return OFX_STAT_ERR_BAD_INDEX;
    }

    let map = props.props.lock().unwrap_or_else(|e| e.into_inner());
    match map.get(key).and_then(|v| v.get(index as usize)) {
        Some(PropVal::String(cs)) => {
            *value = cs.as_ptr() as *mut c_char;
            OFX_STAT_OK
        }
        _ if props.lenient => {
            log::trace!("[ofx] propGetString default for missing '{key}'[{index}]");
            static EMPTY: &[u8] = b"\0";
            *value = EMPTY.as_ptr() as *mut c_char;
            OFX_STAT_OK
        }
        _ => {
            log::trace!("[ofx] propGetString missing '{key}'[{index}]");
            OFX_STAT_ERR_BAD_INDEX
        }
    }
}

unsafe extern "C" fn prop_get_double(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: *mut c_double) -> OfxStatus {
    if value.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    let Some(props) = to_prop_set(properties) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let Some(key) = to_str(property) else {
        return OFX_STAT_ERR_VALUE;
    };
    if index < 0 {
        return OFX_STAT_ERR_BAD_INDEX;
    }

    let map = props.props.lock().unwrap_or_else(|e| e.into_inner());
    match map.get(key).and_then(|v| v.get(index as usize)) {
        Some(PropVal::Double(d)) => {
            *value = *d;
            OFX_STAT_OK
        }
        _ if props.lenient => {
            log::trace!("[ofx] propGetDouble default for missing '{key}'[{index}]");
            *value = 0.0;
            OFX_STAT_OK
        }
        _ => {
            log::trace!("[ofx] propGetDouble missing '{key}'[{index}]");
            OFX_STAT_ERR_BAD_INDEX
        }
    }
}

unsafe extern "C" fn prop_get_int(properties: OfxPropertySetHandle, property: *const c_char, index: c_int, value: *mut c_int) -> OfxStatus {
    if value.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    let Some(props) = to_prop_set(properties) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let Some(key) = to_str(property) else {
        return OFX_STAT_ERR_VALUE;
    };
    if index < 0 {
        return OFX_STAT_ERR_BAD_INDEX;
    }

    let map = props.props.lock().unwrap_or_else(|e| e.into_inner());
    match map.get(key).and_then(|v| v.get(index as usize)) {
        Some(PropVal::Int(i)) => {
            *value = *i;
            OFX_STAT_OK
        }
        _ if props.lenient => {
            log::trace!("[ofx] propGetInt default for missing '{key}'[{index}]");
            *value = 0;
            OFX_STAT_OK
        }
        _ => {
            log::trace!("[ofx] propGetInt missing '{key}'[{index}]");
            OFX_STAT_ERR_BAD_INDEX
        }
    }
}

unsafe extern "C" fn prop_get_pointer_n(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *mut *mut c_void) -> OfxStatus {
    for i in 0..count {
        let status = prop_get_pointer(properties, property, i, value.add(i as usize));
        if status != OFX_STAT_OK {
            return status;
        }
    }
    OFX_STAT_OK
}

unsafe extern "C" fn prop_get_string_n(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *mut *mut c_char) -> OfxStatus {
    for i in 0..count {
        let status = prop_get_string(properties, property, i, value.add(i as usize));
        if status != OFX_STAT_OK {
            return status;
        }
    }
    OFX_STAT_OK
}

unsafe extern "C" fn prop_get_double_n(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *mut c_double) -> OfxStatus {
    for i in 0..count {
        let status = prop_get_double(properties, property, i, value.add(i as usize));
        if status != OFX_STAT_OK {
            return status;
        }
    }
    OFX_STAT_OK
}

unsafe extern "C" fn prop_get_int_n(properties: OfxPropertySetHandle, property: *const c_char, count: c_int, value: *mut c_int) -> OfxStatus {
    for i in 0..count {
        let status = prop_get_int(properties, property, i, value.add(i as usize));
        if status != OFX_STAT_OK {
            return status;
        }
    }
    OFX_STAT_OK
}

unsafe extern "C" fn prop_reset(properties: OfxPropertySetHandle, property: *const c_char) -> OfxStatus {
    let Some(props) = to_prop_set(properties) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let Some(key) = to_str(property) else {
        return OFX_STAT_ERR_VALUE;
    };
    let mut map = props.props.lock().unwrap_or_else(|e| e.into_inner());
    map.remove(key);
    OFX_STAT_OK
}

unsafe extern "C" fn prop_get_dimension(properties: OfxPropertySetHandle, property: *const c_char, dimension: *mut c_int) -> OfxStatus {
    if dimension.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    let Some(props) = to_prop_set(properties) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let Some(key) = to_str(property) else {
        return OFX_STAT_ERR_VALUE;
    };

    let map = props.props.lock().unwrap_or_else(|e| e.into_inner());
    let len = map.get(key).map(|v| v.len()).unwrap_or(0);
    *dimension = len as c_int;
    OFX_STAT_OK
}
