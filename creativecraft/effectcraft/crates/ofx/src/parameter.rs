//! Parameter Set implementation and `OfxParameterSuiteV1`.

#![allow(unsafe_code, unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_double, c_int, c_uint, c_void};
use std::sync::{Arc, Mutex};

use crate::ffi::*;
use crate::property::PropertySet;

#[derive(Clone, Debug)]
pub enum ParamValue {
    Double(f64),
    Int(i32),
    Bool(bool),
    RGBA(f64, f64, f64, f64),
    Point2D(f64, f64),
    Int2D(i32, i32),
    String(String),
}

pub struct Param {
    pub name: String,
    pub param_type: String,
    pub props: PropertySet,
    pub current_value: Mutex<ParamValue>,
}

impl Param {
    pub fn new(name: &str, param_type: &str) -> Self {
        let props = PropertySet::new();
        props.set_string(OFX_PROP_TYPE, OFX_TYPE_PARAMETER);
        props.set_string(OFX_PARAM_PROP_TYPE, param_type);
        props.set_string(OFX_PROP_NAME, name);
        props.set_string(OFX_PROP_LABEL, name);

        let default_val = default_value(param_type, &props);

        Self { name: name.to_string(), param_type: param_type.to_string(), props, current_value: Mutex::new(default_val) }
    }

    /// A fresh parameter with the same definition (properties and default) as
    /// another one — used to give an instance the parameters its descriptor
    /// declared.
    pub fn from_template(source: &Param) -> Self {
        let props = PropertySet::new();
        props.copy_from(&source.props);
        let current = default_value(&source.param_type, &props);
        Self { name: source.name.clone(), param_type: source.param_type.clone(), props, current_value: Mutex::new(current) }
    }

    pub fn set_from_flat_f64(&self, vals: &[f64]) {
        let mut cur = self.current_value.lock().unwrap_or_else(|e| e.into_inner());
        match self.param_type.as_str() {
            OFX_PARAM_TYPE_DOUBLE => {
                if let Some(&v) = vals.first() {
                    *cur = ParamValue::Double(v);
                }
            }
            OFX_PARAM_TYPE_INTEGER | OFX_PARAM_TYPE_CHOICE => {
                if let Some(&v) = vals.first() {
                    *cur = ParamValue::Int(v.round() as i32);
                }
            }
            OFX_PARAM_TYPE_BOOLEAN => {
                if let Some(&v) = vals.first() {
                    *cur = ParamValue::Bool(v > 0.5);
                }
            }
            OFX_PARAM_TYPE_RGBA => {
                if vals.len() >= 4 {
                    *cur = ParamValue::RGBA(vals[0], vals[1], vals[2], vals[3]);
                }
            }
            OFX_PARAM_TYPE_RGB => {
                if vals.len() >= 3 {
                    *cur = ParamValue::RGBA(vals[0], vals[1], vals[2], 1.0);
                }
            }
            OFX_PARAM_TYPE_DOUBLE_2D => {
                if vals.len() >= 2 {
                    *cur = ParamValue::Point2D(vals[0], vals[1]);
                }
            }
            OFX_PARAM_TYPE_INTEGER_2D => {
                if vals.len() >= 2 {
                    *cur = ParamValue::Int2D(vals[0].round() as i32, vals[1].round() as i32);
                }
            }
            _ => {}
        }
    }
}

/// The starting value of a parameter, read from its `kOfxParamPropDefault`.
fn default_value(param_type: &str, props: &PropertySet) -> ParamValue {
    let dbl = |index| props.get_double(OFX_PARAM_PROP_DEFAULT, index).unwrap_or(0.0);
    let int = |index| props.get_int(OFX_PARAM_PROP_DEFAULT, index).unwrap_or(0);
    match param_type {
        OFX_PARAM_TYPE_DOUBLE => ParamValue::Double(dbl(0)),
        OFX_PARAM_TYPE_INTEGER | OFX_PARAM_TYPE_CHOICE => ParamValue::Int(int(0)),
        OFX_PARAM_TYPE_BOOLEAN => ParamValue::Bool(int(0) != 0),
        OFX_PARAM_TYPE_RGBA => ParamValue::RGBA(
            props.get_double(OFX_PARAM_PROP_DEFAULT, 0).unwrap_or(1.0),
            props.get_double(OFX_PARAM_PROP_DEFAULT, 1).unwrap_or(1.0),
            props.get_double(OFX_PARAM_PROP_DEFAULT, 2).unwrap_or(1.0),
            props.get_double(OFX_PARAM_PROP_DEFAULT, 3).unwrap_or(1.0),
        ),
        OFX_PARAM_TYPE_RGB => ParamValue::RGBA(
            props.get_double(OFX_PARAM_PROP_DEFAULT, 0).unwrap_or(1.0),
            props.get_double(OFX_PARAM_PROP_DEFAULT, 1).unwrap_or(1.0),
            props.get_double(OFX_PARAM_PROP_DEFAULT, 2).unwrap_or(1.0),
            1.0,
        ),
        OFX_PARAM_TYPE_DOUBLE_2D => {
            ParamValue::Point2D(props.get_double(OFX_PARAM_PROP_DEFAULT, 0).unwrap_or(0.5), props.get_double(OFX_PARAM_PROP_DEFAULT, 1).unwrap_or(0.5))
        }
        OFX_PARAM_TYPE_INTEGER_2D => ParamValue::Int2D(int(0), int(1)),
        _ => ParamValue::Double(0.0),
    }
}

#[derive(Default)]
pub struct ParamSet {
    pub props: PropertySet,
    pub params: Mutex<HashMap<String, Arc<Param>>>,
    pub param_order: Mutex<Vec<String>>,
}

impl ParamSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_param(&self, param: Arc<Param>) {
        let name = param.name.clone();
        let mut map = self.params.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(name.clone(), param);
        let mut order = self.param_order.lock().unwrap_or_else(|e| e.into_inner());
        if !order.contains(&name) {
            order.push(name);
        }
    }

    pub fn get_param(&self, name: &str) -> Option<Arc<Param>> {
        let map = self.params.lock().unwrap_or_else(|e| e.into_inner());
        map.get(name).cloned()
    }
}

// ---------------------------------------------------------------- C Suite implementation

pub static PARAMETER_SUITE: OfxParameterSuiteV1 = OfxParameterSuiteV1 {
    param_define,
    param_get_handle,
    param_set_get_property_set,
    param_get_property_set,
    param_get_value,
    param_get_value_at_time,
    param_get_derivative,
    param_get_integral,
    param_set_value,
    param_set_value_at_time,
    param_get_num_keys,
    param_get_key_time,
    param_get_key_index,
    param_delete_key,
    param_delete_all_keys,
    param_copy,
    param_edit_begin,
    param_edit_end,
};

unsafe fn to_param_set<'a>(handle: OfxParamSetHandle) -> Option<&'a ParamSet> {
    if handle.is_null() { None } else { Some(&*(handle as *const ParamSet)) }
}

unsafe fn to_param<'a>(handle: OfxParamHandle) -> Option<&'a Param> {
    if handle.is_null() { None } else { Some(&*(handle as *const Param)) }
}

unsafe fn to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() { None } else { CStr::from_ptr(ptr).to_str().ok() }
}

unsafe extern "C" fn param_define(
    param_set: OfxParamSetHandle,
    param_type: *const c_char,
    name: *const c_char,
    property_set: *mut OfxPropertySetHandle,
) -> OfxStatus {
    let Some(ps) = to_param_set(param_set) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let Some(type_str) = to_str(param_type) else {
        return OFX_STAT_ERR_VALUE;
    };
    let Some(name_str) = to_str(name) else {
        return OFX_STAT_ERR_VALUE;
    };

    let param = Arc::new(Param::new(name_str, type_str));
    if !property_set.is_null() {
        *property_set = &param.props as *const PropertySet as *mut c_void;
    }
    ps.add_param(param);
    OFX_STAT_OK
}

unsafe extern "C" fn param_get_handle(
    param_set: OfxParamSetHandle,
    name: *const c_char,
    param: *mut OfxParamHandle,
    property_set: *mut OfxPropertySetHandle,
) -> OfxStatus {
    let Some(ps) = to_param_set(param_set) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let Some(name_str) = to_str(name) else {
        return OFX_STAT_ERR_VALUE;
    };

    if let Some(p) = ps.get_param(name_str) {
        let raw = Arc::as_ptr(&p) as *mut c_void;
        if !param.is_null() {
            *param = raw;
        }
        if !property_set.is_null() {
            *property_set = &p.props as *const PropertySet as *mut c_void;
        }
        OFX_STAT_OK
    } else {
        log::trace!("[ofx] paramGetHandle: no parameter '{name_str}'");
        OFX_STAT_ERR_BAD_INDEX
    }
}

unsafe extern "C" fn param_set_get_property_set(param_set: OfxParamSetHandle, prop_handle: *mut OfxPropertySetHandle) -> OfxStatus {
    let Some(ps) = to_param_set(param_set) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    if prop_handle.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    *prop_handle = &ps.props as *const PropertySet as *mut c_void;
    OFX_STAT_OK
}

unsafe extern "C" fn param_get_property_set(param: OfxParamHandle, prop_handle: *mut OfxPropertySetHandle) -> OfxStatus {
    let Some(p) = to_param(param) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    if prop_handle.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    *prop_handle = &p.props as *const PropertySet as *mut c_void;
    OFX_STAT_OK
}

unsafe extern "C" fn param_get_value(param_handle: OfxParamHandle, mut args: ...) -> OfxStatus {
    let Some(p) = to_param(param_handle) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let cur = p.current_value.lock().unwrap_or_else(|e| e.into_inner());

    match &*cur {
        ParamValue::Double(d) => {
            let ptr = args.next_arg::<*mut c_double>();
            if !ptr.is_null() {
                *ptr = *d;
            }
        }
        ParamValue::Int(i) => {
            let ptr = args.next_arg::<*mut c_int>();
            if !ptr.is_null() {
                *ptr = *i;
            }
        }
        ParamValue::Bool(b) => {
            let ptr = args.next_arg::<*mut c_int>();
            if !ptr.is_null() {
                *ptr = if *b { 1 } else { 0 };
            }
        }
        ParamValue::RGBA(r, g, b, a) => {
            let pr = args.next_arg::<*mut c_double>();
            let pg = args.next_arg::<*mut c_double>();
            let pb = args.next_arg::<*mut c_double>();
            let pa = args.next_arg::<*mut c_double>();
            if !pr.is_null() {
                *pr = *r;
            }
            if !pg.is_null() {
                *pg = *g;
            }
            if !pb.is_null() {
                *pb = *b;
            }
            if !pa.is_null() {
                *pa = *a;
            }
        }
        ParamValue::Point2D(x, y) => {
            let px = args.next_arg::<*mut c_double>();
            let py = args.next_arg::<*mut c_double>();
            if !px.is_null() {
                *px = *x;
            }
            if !py.is_null() {
                *py = *y;
            }
        }
        ParamValue::Int2D(x, y) => {
            let px = args.next_arg::<*mut c_int>();
            let py = args.next_arg::<*mut c_int>();
            if !px.is_null() {
                *px = *x;
            }
            if !py.is_null() {
                *py = *y;
            }
        }
        ParamValue::String(_) => {}
    }
    OFX_STAT_OK
}

unsafe extern "C" fn param_get_value_at_time(param_handle: OfxParamHandle, _time: OfxTime, mut args: ...) -> OfxStatus {
    let Some(p) = to_param(param_handle) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let cur = p.current_value.lock().unwrap_or_else(|e| e.into_inner());

    match &*cur {
        ParamValue::Double(d) => {
            let ptr = args.next_arg::<*mut c_double>();
            if !ptr.is_null() {
                *ptr = *d;
            }
        }
        ParamValue::Int(i) => {
            let ptr = args.next_arg::<*mut c_int>();
            if !ptr.is_null() {
                *ptr = *i;
            }
        }
        ParamValue::Bool(b) => {
            let ptr = args.next_arg::<*mut c_int>();
            if !ptr.is_null() {
                *ptr = if *b { 1 } else { 0 };
            }
        }
        ParamValue::RGBA(r, g, b, a) => {
            let pr = args.next_arg::<*mut c_double>();
            let pg = args.next_arg::<*mut c_double>();
            let pb = args.next_arg::<*mut c_double>();
            let pa = args.next_arg::<*mut c_double>();
            if !pr.is_null() {
                *pr = *r;
            }
            if !pg.is_null() {
                *pg = *g;
            }
            if !pb.is_null() {
                *pb = *b;
            }
            if !pa.is_null() {
                *pa = *a;
            }
        }
        ParamValue::Point2D(x, y) => {
            let px = args.next_arg::<*mut c_double>();
            let py = args.next_arg::<*mut c_double>();
            if !px.is_null() {
                *px = *x;
            }
            if !py.is_null() {
                *py = *y;
            }
        }
        ParamValue::Int2D(x, y) => {
            let px = args.next_arg::<*mut c_int>();
            let py = args.next_arg::<*mut c_int>();
            if !px.is_null() {
                *px = *x;
            }
            if !py.is_null() {
                *py = *y;
            }
        }
        ParamValue::String(_) => {}
    }
    OFX_STAT_OK
}

unsafe extern "C" fn param_get_derivative(param_handle: OfxParamHandle, _time: OfxTime, _args: ...) -> OfxStatus {
    if to_param(param_handle).is_none() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    OFX_STAT_OK
}

unsafe extern "C" fn param_get_integral(param_handle: OfxParamHandle, _t1: OfxTime, _t2: OfxTime, _args: ...) -> OfxStatus {
    if to_param(param_handle).is_none() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    OFX_STAT_OK
}

unsafe extern "C" fn param_set_value(param_handle: OfxParamHandle, mut args: ...) -> OfxStatus {
    let Some(p) = to_param(param_handle) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let mut cur = p.current_value.lock().unwrap_or_else(|e| e.into_inner());

    match p.param_type.as_str() {
        OFX_PARAM_TYPE_DOUBLE => {
            let val = args.next_arg::<c_double>();
            *cur = ParamValue::Double(val);
        }
        OFX_PARAM_TYPE_INTEGER | OFX_PARAM_TYPE_CHOICE => {
            let val = args.next_arg::<c_int>();
            *cur = ParamValue::Int(val);
        }
        OFX_PARAM_TYPE_BOOLEAN => {
            let val = args.next_arg::<c_int>();
            *cur = ParamValue::Bool(val != 0);
        }
        OFX_PARAM_TYPE_RGBA => {
            let r = args.next_arg::<c_double>();
            let g = args.next_arg::<c_double>();
            let b = args.next_arg::<c_double>();
            let a = args.next_arg::<c_double>();
            *cur = ParamValue::RGBA(r, g, b, a);
        }
        OFX_PARAM_TYPE_RGB => {
            let r = args.next_arg::<c_double>();
            let g = args.next_arg::<c_double>();
            let b = args.next_arg::<c_double>();
            *cur = ParamValue::RGBA(r, g, b, 1.0);
        }
        OFX_PARAM_TYPE_DOUBLE_2D => {
            let x = args.next_arg::<c_double>();
            let y = args.next_arg::<c_double>();
            *cur = ParamValue::Point2D(x, y);
        }
        OFX_PARAM_TYPE_INTEGER_2D => {
            let x = args.next_arg::<c_int>();
            let y = args.next_arg::<c_int>();
            *cur = ParamValue::Int2D(x, y);
        }
        _ => {}
    }
    OFX_STAT_OK
}

unsafe extern "C" fn param_set_value_at_time(param_handle: OfxParamHandle, _time: OfxTime, mut args: ...) -> OfxStatus {
    let Some(p) = to_param(param_handle) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let mut cur = p.current_value.lock().unwrap_or_else(|e| e.into_inner());

    match p.param_type.as_str() {
        OFX_PARAM_TYPE_DOUBLE => {
            *cur = ParamValue::Double(args.next_arg::<c_double>());
        }
        OFX_PARAM_TYPE_INTEGER | OFX_PARAM_TYPE_CHOICE => {
            *cur = ParamValue::Int(args.next_arg::<c_int>());
        }
        OFX_PARAM_TYPE_BOOLEAN => {
            *cur = ParamValue::Bool(args.next_arg::<c_int>() != 0);
        }
        OFX_PARAM_TYPE_RGBA => {
            let r = args.next_arg::<c_double>();
            let g = args.next_arg::<c_double>();
            let b = args.next_arg::<c_double>();
            let a = args.next_arg::<c_double>();
            *cur = ParamValue::RGBA(r, g, b, a);
        }
        OFX_PARAM_TYPE_RGB => {
            let r = args.next_arg::<c_double>();
            let g = args.next_arg::<c_double>();
            let b = args.next_arg::<c_double>();
            *cur = ParamValue::RGBA(r, g, b, 1.0);
        }
        OFX_PARAM_TYPE_DOUBLE_2D => {
            let x = args.next_arg::<c_double>();
            let y = args.next_arg::<c_double>();
            *cur = ParamValue::Point2D(x, y);
        }
        OFX_PARAM_TYPE_INTEGER_2D => {
            let x = args.next_arg::<c_int>();
            let y = args.next_arg::<c_int>();
            *cur = ParamValue::Int2D(x, y);
        }
        _ => {}
    }
    OFX_STAT_OK
}

unsafe extern "C" fn param_get_num_keys(param_handle: OfxParamHandle, num_keys: *mut c_uint) -> OfxStatus {
    if to_param(param_handle).is_none() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    if !num_keys.is_null() {
        *num_keys = 0;
    }
    OFX_STAT_OK
}

unsafe extern "C" fn param_get_key_time(param_handle: OfxParamHandle, _nth: c_uint, _time: *mut OfxTime) -> OfxStatus {
    if to_param(param_handle).is_none() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    OFX_STAT_FAILED
}

unsafe extern "C" fn param_get_key_index(param_handle: OfxParamHandle, _time: OfxTime, _dir: c_int, _index: *mut c_int) -> OfxStatus {
    if to_param(param_handle).is_none() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    OFX_STAT_FAILED
}

unsafe extern "C" fn param_delete_key(param_handle: OfxParamHandle, _time: OfxTime) -> OfxStatus {
    if to_param(param_handle).is_none() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    OFX_STAT_OK
}

unsafe extern "C" fn param_delete_all_keys(param_handle: OfxParamHandle) -> OfxStatus {
    if to_param(param_handle).is_none() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    OFX_STAT_OK
}

unsafe extern "C" fn param_copy(_to: OfxParamHandle, _from: OfxParamHandle, _offset: OfxTime, _range: *const OfxRangeD) -> OfxStatus {
    OFX_STAT_OK
}

unsafe extern "C" fn param_edit_begin(_set: OfxParamSetHandle, _name: *const c_char) -> OfxStatus {
    OFX_STAT_OK
}

unsafe extern "C" fn param_edit_end(_set: OfxParamSetHandle) -> OfxStatus {
    OFX_STAT_OK
}
