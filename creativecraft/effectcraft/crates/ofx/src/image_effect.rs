//! Image Effect Suite implementation (`OfxImageEffectSuiteV1`).

#![allow(unsafe_code, unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::{Arc, Mutex};

use crate::ffi::*;
use crate::parameter::ParamSet;
use crate::property::{PropVal, PropertySet};

pub struct Clip {
    pub name: String,
    pub props: PropertySet,
}

impl Clip {
    pub fn new(name: &str) -> Self {
        let props = PropertySet::lenient();
        props.set_string(OFX_PROP_TYPE, OFX_TYPE_CLIP);
        props.set_string(OFX_PROP_NAME, name);
        props.set_string(OFX_PROP_LABEL, name);
        props.set_int(OFX_IMAGE_CLIP_PROP_CONNECTED, 1);
        Self { name: name.to_string(), props }
    }
}

/// One image handed to a plug-in by `clipGetImage`.
///
/// `props` must stay the first field: the handle a plug-in receives is the
/// address of the property set, and `clipReleaseImage` turns it back into the
/// whole box to free it.
#[repr(C)]
pub struct ImageBuffer {
    pub props: PropertySet,
    pub data: *mut c_void,
}

/// Everything a plug-in may ask for while one render is running.
///
/// The source and the output are separate buffers: a plug-in is allowed to read
/// the source after it has written the output, so sharing one buffer would feed
/// processed pixels back into the effect (and corrupt convolutions).
pub struct ActiveRender {
    pub source: *mut c_void,
    pub output: *mut c_void,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub time: f64,
    pub frame_rate: f64,
}

pub struct ImageEffectInstance {
    pub props: PropertySet,
    pub param_set: Arc<ParamSet>,
    pub clips: Mutex<HashMap<String, Arc<Clip>>>,
    /// True once the plug-in accepted `kOfxActionOpenGLContextAttached`.
    pub gl_ready: std::sync::atomic::AtomicBool,
}

impl Default for ImageEffectInstance {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageEffectInstance {
    pub fn new() -> Self {
        let props = PropertySet::lenient();
        props.set_string(OFX_PROP_TYPE, OFX_TYPE_IMAGE_EFFECT_INSTANCE);
        props.set_string(OFX_IMAGE_EFFECT_PROP_CONTEXT, OFX_IMAGE_EFFECT_CONTEXT_FILTER);
        props.set_string(OFX_IMAGE_EFFECT_PROP_PIXEL_DEPTH, OFX_BIT_DEPTH_FLOAT);
        props.set_string(OFX_IMAGE_EFFECT_PROP_COMPONENTS, OFX_IMAGE_COMPONENT_RGBA);
        {
            // 2D properties must have both components; plug-ins read index 1 too.
            let mut map = props.props.lock().unwrap_or_else(|e| e.into_inner());
            map.insert(OFX_IMAGE_EFFECT_PROP_RENDER_SCALE.to_string(), vec![PropVal::Double(1.0), PropVal::Double(1.0)]);
        }

        let param_set = Arc::new(ParamSet::new());
        let inst = Self { props, param_set, clips: Mutex::new(HashMap::new()), gl_ready: std::sync::atomic::AtomicBool::new(false) };

        // Standard Source and Output clips; a plug-in may redefine them with
        // clipDefine during DescribeInContext.
        inst.add_clip(Arc::new(Clip::new(OFX_IMAGE_EFFECT_SIMPLE_SOURCE_CLIP_NAME)));
        inst.add_clip(Arc::new(Clip::new(OFX_IMAGE_EFFECT_OUTPUT_CLIP_NAME)));
        inst
    }

    pub fn add_clip(&self, clip: Arc<Clip>) {
        let mut map = self.clips.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(clip.name.clone(), clip);
    }

    pub fn get_clip(&self, name: &str) -> Option<Arc<Clip>> {
        let map = self.clips.lock().unwrap_or_else(|e| e.into_inner());
        map.get(name).cloned()
    }

    /// Fill in the clip properties a host publishes at render time.
    ///
    /// A plug-in reads these from `clipGetPropertySet` to decide what it can
    /// process; leaving them empty makes strict plug-ins fail the render.
    pub fn prepare_clips(&self, time: f64, frame_rate: f64) {
        let clips = self.clips.lock().unwrap_or_else(|e| e.into_inner());
        for clip in clips.values() {
            let props = &clip.props;
            props.set_int(OFX_IMAGE_CLIP_PROP_CONNECTED, 1);
            props.set_string(OFX_IMAGE_EFFECT_PROP_PIXEL_DEPTH, OFX_BIT_DEPTH_FLOAT);
            props.set_string(OFX_IMAGE_EFFECT_PROP_COMPONENTS, OFX_IMAGE_COMPONENT_RGBA);
            props.set_string(OFX_IMAGE_EFFECT_PROP_PRE_MULTIPLICATION, OFX_IMAGE_PRE_MULTIPLIED);
            props.set_double(OFX_IMAGE_CLIP_PROP_FRAME_RATE, frame_rate);
            props.set_double(OFX_IMAGE_PROP_PIXEL_ASPECT_RATIO, 1.0);
            props.set_string(OFX_IMAGE_CLIP_PROP_FIELD_ORDER, OFX_IMAGE_FIELD_NONE);
            {
                let mut map = props.props.lock().unwrap_or_else(|e| e.into_inner());
                map.insert(OFX_IMAGE_EFFECT_PROP_RENDER_SCALE.to_string(), vec![PropVal::Double(1.0), PropVal::Double(1.0)]);
                map.insert(OFX_IMAGE_EFFECT_PROP_FRAME_RANGE.to_string(), vec![PropVal::Double(time), PropVal::Double(time)]);
                map.insert(OFX_IMAGE_CLIP_PROP_FRAME_RANGE.to_string(), vec![PropVal::Double(time), PropVal::Double(time)]);
            }
        }
    }
}

// ---------------------------------------------------------------- C Suite implementation

pub static IMAGE_EFFECT_SUITE: OfxImageEffectSuiteV1 = OfxImageEffectSuiteV1 {
    get_property_set,
    get_param_set,
    clip_define,
    clip_get_handle,
    clip_get_property_set,
    clip_get_image,
    clip_release_image,
    clip_get_region_of_definition,
    abort,
    image_memory_alloc,
    image_memory_free,
    image_memory_lock,
    image_memory_unlock,
};

unsafe fn to_inst<'a>(handle: OfxImageEffectHandle) -> Option<&'a ImageEffectInstance> {
    if handle.is_null() { None } else { Some(&*(handle as *const ImageEffectInstance)) }
}

unsafe fn to_clip<'a>(handle: OfxImageClipHandle) -> Option<&'a Clip> {
    if handle.is_null() { None } else { Some(&*(handle as *const Clip)) }
}

unsafe fn to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() { None } else { CStr::from_ptr(ptr).to_str().ok() }
}

unsafe extern "C" fn get_property_set(image_effect: OfxImageEffectHandle, prop_handle: *mut OfxPropertySetHandle) -> OfxStatus {
    let Some(inst) = to_inst(image_effect) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    if prop_handle.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    *prop_handle = &inst.props as *const PropertySet as *mut c_void;
    OFX_STAT_OK
}

unsafe extern "C" fn get_param_set(image_effect: OfxImageEffectHandle, param_set: *mut OfxParamSetHandle) -> OfxStatus {
    let Some(inst) = to_inst(image_effect) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    if param_set.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    *param_set = Arc::as_ptr(&inst.param_set) as *mut c_void;
    OFX_STAT_OK
}

unsafe extern "C" fn clip_define(image_effect: OfxImageEffectHandle, name: *const c_char, property_set: *mut OfxPropertySetHandle) -> OfxStatus {
    let Some(inst) = to_inst(image_effect) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let Some(name_str) = to_str(name) else {
        return OFX_STAT_ERR_VALUE;
    };

    let clip = Arc::new(Clip::new(name_str));
    if !property_set.is_null() {
        *property_set = &clip.props as *const PropertySet as *mut c_void;
    }
    inst.add_clip(clip);
    OFX_STAT_OK
}

unsafe extern "C" fn clip_get_handle(
    image_effect: OfxImageEffectHandle,
    name: *const c_char,
    clip: *mut OfxImageClipHandle,
    property_set: *mut OfxPropertySetHandle,
) -> OfxStatus {
    let Some(inst) = to_inst(image_effect) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    let Some(name_str) = to_str(name) else {
        return OFX_STAT_ERR_VALUE;
    };

    if let Some(c) = inst.get_clip(name_str) {
        if !clip.is_null() {
            *clip = Arc::as_ptr(&c) as *mut c_void;
        }
        if !property_set.is_null() {
            *property_set = &c.props as *const PropertySet as *mut c_void;
        }
        OFX_STAT_OK
    } else {
        log::trace!("[ofx] clipGetHandle: no clip '{name_str}'");
        OFX_STAT_ERR_BAD_INDEX
    }
}

unsafe extern "C" fn clip_get_property_set(clip: OfxImageClipHandle, prop_handle: *mut OfxPropertySetHandle) -> OfxStatus {
    let Some(c) = to_clip(clip) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    if prop_handle.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    *prop_handle = &c.props as *const PropertySet as *mut c_void;
    OFX_STAT_OK
}

unsafe extern "C" fn clip_get_image(clip: OfxImageClipHandle, time: OfxTime, _region: *const OfxRectD, image_handle: *mut OfxPropertySetHandle) -> OfxStatus {
    let Some(c) = to_clip(clip) else {
        return OFX_STAT_ERR_BAD_HANDLE;
    };
    if image_handle.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    let Some(active) = active_render() else {
        return OFX_STAT_FAILED;
    };

    let is_output = c.name == OFX_IMAGE_EFFECT_OUTPUT_CLIP_NAME;
    let data = if is_output { active.output } else { active.source };
    log::trace!("[ofx] clipGetImage '{}' time={time} output={is_output} {}x{}", c.name, active.width, active.height);
    if data.is_null() {
        return OFX_STAT_FAILED;
    }

    let (w, h) = (active.width as c_int, active.height as c_int);
    let img = Box::new(ImageBuffer { props: PropertySet::lenient(), data });

    img.props.set_string(OFX_PROP_TYPE, OFX_TYPE_IMAGE);
    img.props.set_pointer(OFX_IMAGE_PROP_DATA, data);
    img.props.set_string(OFX_IMAGE_EFFECT_PROP_PIXEL_DEPTH, OFX_BIT_DEPTH_FLOAT);
    img.props.set_string(OFX_IMAGE_EFFECT_PROP_COMPONENTS, OFX_IMAGE_COMPONENT_RGBA);
    img.props.set_string(OFX_IMAGE_EFFECT_PROP_PRE_MULTIPLICATION, OFX_IMAGE_PRE_MULTIPLIED);
    img.props.set_int(OFX_IMAGE_PROP_ROW_BYTES, w * 4 * 4);
    img.props.set_double(OFX_PROP_TIME, time);
    img.props.set_double(OFX_IMAGE_PROP_PIXEL_ASPECT_RATIO, 1.0);
    img.props.set_string(OFX_IMAGE_PROP_FIELD, OFX_IMAGE_FIELD_NONE);
    img.props.set_string(OFX_IMAGE_PROP_UNIQUE_IDENTIFIER, &format!("ec-image-{time}"));

    let mut map = img.props.props.lock().unwrap_or_else(|e| e.into_inner());
    // All four are integer dimensions in the OFX spec (not doubles).
    map.insert(OFX_IMAGE_PROP_BOUNDS.to_string(), vec![PropVal::Int(0), PropVal::Int(0), PropVal::Int(w), PropVal::Int(h)]);
    map.insert(OFX_IMAGE_PROP_REGION_OF_DEFINITION.to_string(), vec![PropVal::Int(0), PropVal::Int(0), PropVal::Int(w), PropVal::Int(h)]);
    map.insert(OFX_IMAGE_EFFECT_PROP_RENDER_SCALE.to_string(), vec![PropVal::Double(active.scale), PropVal::Double(active.scale)]);
    drop(map);

    let raw = Box::into_raw(img);
    *image_handle = raw as *mut c_void;
    OFX_STAT_OK
}

unsafe extern "C" fn clip_release_image(image_handle: OfxPropertySetHandle) -> OfxStatus {
    if image_handle.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    // The handle is the ImageBuffer itself, because `props` is its first field
    // under `#[repr(C)]`.
    drop(Box::from_raw(image_handle as *mut ImageBuffer));
    OFX_STAT_OK
}

unsafe extern "C" fn clip_get_region_of_definition(_clip: OfxImageClipHandle, _time: OfxTime, bounds: *mut OfxRectD) -> OfxStatus {
    if bounds.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    // The region of definition is asked for outside a render too (before the
    // first frame, to size the output). Answer with the render size if one has
    // happened, otherwise with the last size EffectCraft used, then 1920x1080.
    let (w, h) = match active_render() {
        Some(active) => (active.width, active.height),
        None => last_frame_size(),
    };
    *bounds = OfxRectD { x1: 0.0, y1: 0.0, x2: w as f64, y2: h as f64 };
    OFX_STAT_OK
}

unsafe extern "C" fn abort(_image_effect: OfxImageEffectHandle) -> c_int {
    0
}

/// Buffers for an image-memory block, so `imageMemoryFree` can release them.
#[repr(C)]
struct ImageMemory {
    layout: std::alloc::Layout,
    data: *mut u8,
}

unsafe extern "C" fn image_memory_alloc(_image_effect: OfxImageEffectHandle, n_bytes: usize, memory_handle: *mut OfxImageMemoryHandle) -> OfxStatus {
    if memory_handle.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    log::trace!("[ofx] imageMemoryAlloc {n_bytes} byte(s)");
    let Ok(layout) = std::alloc::Layout::from_size_align(n_bytes.max(1), 16) else {
        return OFX_STAT_ERR_MEMORY;
    };
    let data = std::alloc::alloc(layout);
    if data.is_null() {
        return OFX_STAT_ERR_MEMORY;
    }
    let block = Box::new(ImageMemory { layout, data });
    *memory_handle = Box::into_raw(block) as *mut c_void;
    OFX_STAT_OK
}

unsafe extern "C" fn image_memory_free(memory_handle: OfxImageMemoryHandle) -> OfxStatus {
    if memory_handle.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    let block = Box::from_raw(memory_handle as *mut ImageMemory);
    if !block.data.is_null() {
        std::alloc::dealloc(block.data, block.layout);
    }
    OFX_STAT_OK
}

unsafe extern "C" fn image_memory_lock(memory_handle: OfxImageMemoryHandle, returned_ptr: *mut *mut c_void) -> OfxStatus {
    if memory_handle.is_null() || returned_ptr.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    log::trace!("[ofx] imageMemoryLock");
    *returned_ptr = (*(memory_handle as *const ImageMemory)).data as *mut c_void;
    OFX_STAT_OK
}

unsafe extern "C" fn image_memory_unlock(_memory_handle: OfxImageMemoryHandle) -> OfxStatus {
    OFX_STAT_OK
}

// ---------------------------------------------------------------- Active render thread-local

thread_local! {
    static ACTIVE_RENDER: std::cell::RefCell<Option<Box<ActiveRender>>> =
        const { std::cell::RefCell::new(None) };
    static LAST_FRAME_SIZE: std::cell::Cell<(u32, u32)> = const { std::cell::Cell::new((1920, 1080)) };
}

/// The size of the most recent frame, for questions asked between renders.
fn last_frame_size() -> (u32, u32) {
    LAST_FRAME_SIZE.with(|s| s.get())
}

/// Install the buffers for the render about to run.
pub fn begin_render(render: ActiveRender) {
    LAST_FRAME_SIZE.with(|s| s.set((render.width, render.height)));
    ACTIVE_RENDER.with(|slot| *slot.borrow_mut() = Some(Box::new(render)));
}

/// Remove them again, whatever the plug-in did.
pub fn end_render() {
    ACTIVE_RENDER.with(|slot| *slot.borrow_mut() = None);
}

/// A copy of the current render's buffers.
pub(crate) fn active_render() -> Option<Box<ActiveRender>> {
    ACTIVE_RENDER.with(|slot| {
        slot.borrow().as_ref().map(|r| {
            Box::new(ActiveRender {
                source: r.source,
                output: r.output,
                width: r.width,
                height: r.height,
                scale: r.scale,
                time: r.time,
                frame_rate: r.frame_rate,
            })
        })
    })
}
