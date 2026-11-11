//! The OpenFX Host instance and suite dispatcher.

#![allow(unsafe_code, unsafe_op_in_unsafe_fn)]

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::sync::OnceLock;

use crate::ffi::*;
use crate::image_effect::IMAGE_EFFECT_SUITE;
use crate::interact::INTERACT_SUITE;
use crate::memory::MEMORY_SUITE;
use crate::message::MESSAGE_SUITE;
use crate::multi_thread::MULTI_THREAD_SUITE;
use crate::parameter::PARAMETER_SUITE;
use crate::progress::PROGRESS_SUITE;
use crate::property::{PROPERTY_SUITE, PropVal, PropertySet};
use crate::timeline::TIMELINE_SUITE;

static HOST_PROPERTIES: OnceLock<PropertySet> = OnceLock::new();
static OFX_HOST: OnceLock<OfxHost> = OnceLock::new();

pub fn get_host_properties() -> &'static PropertySet {
    HOST_PROPERTIES.get_or_init(|| {
        let p = PropertySet::lenient();
        p.set_string(OFX_PROP_TYPE, OFX_TYPE_IMAGE_EFFECT_HOST);
        p.set_string(OFX_PROP_NAME, "EffectCraft");
        p.set_string(OFX_PROP_LABEL, "CreativeCraft EffectCraft");
        p.set_string(OFX_PROP_VERSION_LABEL, "0.18.0");
        // Version 1.5 is what the host understands; plug-ins compare against it.
        // Both are multi-dimensional ints, and plug-ins read several elements,
        // so keep four (missing indices make them abort with kOfxStatErrBadIndex).
        {
            let mut map = p.props.lock().unwrap_or_else(|e| e.into_inner());
            map.insert(OFX_PROP_VERSION.to_string(), vec![PropVal::Int(0), PropVal::Int(18), PropVal::Int(0), PropVal::Int(0)]);
            map.insert(OFX_PROP_APIVERSION.to_string(), vec![PropVal::Int(1), PropVal::Int(5), PropVal::Int(0), PropVal::Int(0)]);
        }
        p.set_int(OFX_PROP_IS_INTERACTIVE, 1);
        p.set_int(OFX_IMAGE_EFFECT_HOST_PROP_SUPPORTS_OVERLAYS, 0);
        p.set_int(OFX_IMAGE_EFFECT_PROP_SUPPORTS_OVERLAYS, 0);
        p.set_int(OFX_IMAGE_EFFECT_PROP_SUPPORTS_MULTI_RESOLUTION, 0);
        p.set_int(OFX_IMAGE_EFFECT_PROP_SUPPORTS_TILES, 0);
        p.set_int(OFX_IMAGE_EFFECT_PROP_TEMPORAL_CLIP_ACCESS, 0);
        p.set_string(OFX_IMAGE_EFFECT_PROP_RENDER_THREAD_SAFETY, "OfxImageEffectRenderFullySafe");
        p.set_int(OFX_IMAGE_EFFECT_HOST_PROP_IS_BACKGROUND, 0);
        p.set_int(OFX_IMAGE_EFFECT_HOST_PROP_SUPPORTS_MULTIPLE_CLIP_DEPTHS, 0);
        p.set_int(OFX_IMAGE_EFFECT_HOST_PROP_SUPPORTS_MULTIPLE_CLIP_PARTS, 0);
        p.set_int(OFX_IMAGE_EFFECT_HOST_PROP_SUPPORTS_MULTIPLE_CLIP_REGIONS, 0);
        p.set_int(OFX_IMAGE_EFFECT_HOST_PROP_SUPPORTS_CUSTOM_COMPONENTS, 0);
        p.set_int(OFX_IMAGE_EFFECT_HOST_PROP_SUPPORTS_OPENCL, 0);
        p.set_int(OFX_IMAGE_EFFECT_HOST_PROP_SUPPORTS_CUDA, 0);
        // An OpenGL render path exists in `opengl.rs`, but it is not advertised:
        // ToneLab declares OpenGL support and switches to that path (and aborts)
        // when the host claims it, while no installed plug-in actually needs it.
        // Advertise it again only after a real plug-in renders through it.
        p.set_string(OFX_IMAGE_EFFECT_PROP_OPENGL_RENDER_SUPPORTED, "false");
        {
            let mut map = p.props.lock().unwrap_or_else(|e| e.into_inner());
            map.insert(OFX_IMAGE_EFFECT_PROP_RENDER_SCALE.to_string(), vec![PropVal::Double(1.0), PropVal::Double(1.0)]);
            map.insert(OFX_IMAGE_EFFECT_PROP_SUPPORTED_PIXEL_DEPTHS.to_string(), vec![PropVal::String(CString::new(OFX_BIT_DEPTH_FLOAT).unwrap())]);
            map.insert(OFX_IMAGE_EFFECT_PROP_SUPPORTED_COMPONENTS.to_string(), vec![PropVal::String(CString::new(OFX_IMAGE_COMPONENT_RGBA).unwrap())]);
            map.insert(
                OFX_IMAGE_EFFECT_PROP_SUPPORTED_CONTEXTS.to_string(),
                vec![
                    PropVal::String(CString::new(OFX_IMAGE_EFFECT_CONTEXT_FILTER).unwrap()),
                    PropVal::String(CString::new(OFX_IMAGE_EFFECT_CONTEXT_GENERAL).unwrap()),
                    PropVal::String(CString::new(OFX_IMAGE_EFFECT_CONTEXT_GENERATOR).unwrap()),
                ],
            );
        }
        p
    })
}

pub fn get_ofx_host() -> &'static OfxHost {
    OFX_HOST.get_or_init(|| {
        let props = get_host_properties();
        OfxHost { host: props as *const PropertySet as *mut c_void, fetch_suite }
    })
}

unsafe extern "C" fn fetch_suite(_host: OfxPropertySetHandle, suite_name: *const c_char, _suite_version: c_int) -> *const c_void {
    if suite_name.is_null() {
        return std::ptr::null();
    }
    let Ok(name) = CStr::from_ptr(suite_name).to_str() else {
        return std::ptr::null();
    };

    let found = match name {
        OFX_PROPERTY_SUITE => &PROPERTY_SUITE as *const OfxPropertySuiteV1 as *const c_void,
        OFX_PARAMETER_SUITE => &PARAMETER_SUITE as *const OfxParameterSuiteV1 as *const c_void,
        OFX_IMAGE_EFFECT_SUITE => &IMAGE_EFFECT_SUITE as *const OfxImageEffectSuiteV1 as *const c_void,
        OFX_MEMORY_SUITE => &MEMORY_SUITE as *const OfxMemorySuiteV1 as *const c_void,
        OFX_MULTI_THREAD_SUITE => &MULTI_THREAD_SUITE as *const OfxMultiThreadSuiteV1 as *const c_void,
        OFX_MESSAGE_SUITE => &MESSAGE_SUITE as *const OfxMessageSuiteV1 as *const c_void,
        OFX_PROGRESS_SUITE => &PROGRESS_SUITE as *const OfxProgressSuiteV1 as *const c_void,
        OFX_TIMELINE_SUITE => &TIMELINE_SUITE as *const OfxTimeLineSuiteV1 as *const c_void,
        OFX_INTERACT_SUITE => &INTERACT_SUITE as *const OfxInteractSuiteV1 as *const c_void,
        OFX_OPENGL_RENDER_SUITE => &crate::opengl::OPENGL_RENDER_SUITE as *const OfxImageEffectOpenGLRenderSuiteV1 as *const c_void,
        _ => std::ptr::null(),
    };
    if found.is_null() {
        log::debug!("[OFX Host] Unknown suite requested: {}", name);
    } else {
        log::trace!("[OFX Host] suite '{}' handed to plug-in", name);
    }
    found
}
