//! Adapter bridging an OpenFX plug-in to EffectCraft's `EffectPlugin` trait.

#![allow(unsafe_code, unsafe_op_in_unsafe_fn)]

use std::ffi::{CStr, CString, c_void};
use std::sync::{Arc, Mutex};

use effectcraft_effects::plugin::{EffectPlugin, PLUGIN_API_VERSION, PluginFrame, PluginManifest, PluginParam, PluginParamKind, PluginParams};

use crate::ffi::*;
use crate::host::get_ofx_host;
use crate::image_effect::{ActiveRender, ImageEffectInstance, begin_render, end_render};
use crate::parameter::Param;
use crate::property::{PropVal, PropertySet};
use crate::timeline::set_current_time;

/// Contexts EffectCraft can render in, in the order it prefers them.
const HOST_CONTEXTS: [&str; 3] = [OFX_IMAGE_EFFECT_CONTEXT_FILTER, OFX_IMAGE_EFFECT_CONTEXT_GENERAL, OFX_IMAGE_EFFECT_CONTEXT_GENERATOR];

pub struct OfxEffectPlugin {
    pub manifest: PluginManifest,
    pub plugin_ptr: usize, // raw *const OfxPlugin as usize
    pub descriptor: Arc<ImageEffectInstance>,
    /// The real OFX parameter name for each entry of `manifest.params`
    /// (manifest ids are sanitised for EffectCraft, OFX names are kept as-is).
    param_names: Vec<String>,
    /// The render context the plug-in was described in (Filter, General, …).
    context: String,
    /// The plug-in's `OfxImageEffectPropOpenGLRenderSupported` flag:
    /// `Some(true)` when it says "needed", `Some(false)` for "true", `None` for "false".
    gl_needed: Option<bool>,
    live_instance: Mutex<Option<Arc<ImageEffectInstance>>>,
    pub _lib: Arc<libloading::Library>,
}

unsafe impl Send for OfxEffectPlugin {}
unsafe impl Sync for OfxEffectPlugin {}

impl OfxEffectPlugin {
    pub unsafe fn from_raw(plugin: *const OfxPlugin, lib: Arc<libloading::Library>) -> Result<Self, String> {
        if plugin.is_null() {
            return Err("null OfxPlugin pointer".into());
        }

        let p = &*plugin;
        let id_str =
            if p.plugin_identifier.is_null() { "org.openfx.unknown".to_string() } else { CStr::from_ptr(p.plugin_identifier).to_string_lossy().to_string() };

        // Handshake: setHost
        let host_ptr = get_ofx_host() as *const OfxHost as *mut OfxHost;
        (p.set_host)(host_ptr);

        log::trace!(
            "[ofx] plugin '{id_str}' api='{}' api_version={} v{}.{}",
            if p.plugin_api.is_null() { "<null>".to_string() } else { CStr::from_ptr(p.plugin_api).to_string_lossy().to_string() },
            p.api_version,
            p.plugin_version_major,
            p.plugin_version_minor
        );

        // Action: Load
        let load_act = CString::new(OFX_ACTION_LOAD).map_err(|e| e.to_string())?;
        let load_status = (p.main_entry)(load_act.as_ptr(), std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut());
        log::debug!("[ofx] '{id_str}' kOfxActionLoad -> {load_status}");
        if load_status == OFX_STAT_FAILED {
            return Err("the plug-in declined to load on this host".into());
        }
        if load_status != OFX_STAT_OK && load_status != OFX_STAT_REPLY_DEFAULT {
            return Err(format!("kOfxActionLoad failed with status {load_status}"));
        }

        // Action: Describe — the handle is the plug-in's descriptor.
        let desc_act = CString::new(OFX_ACTION_DESCRIBE).map_err(|e| e.to_string())?;
        let descriptor = Arc::new(ImageEffectInstance::new());
        let desc_handle = Arc::as_ptr(&descriptor) as *const c_void;
        let desc_status = (p.main_entry)(desc_act.as_ptr(), desc_handle, std::ptr::null_mut(), std::ptr::null_mut());
        log::debug!("[ofx] '{id_str}' kOfxActionDescribe -> {desc_status}");
        if desc_status != OFX_STAT_OK && desc_status != OFX_STAT_REPLY_DEFAULT {
            return Err(format!("kOfxActionDescribe failed with status {desc_status}"));
        }

        // Pick a context both sides support.
        let supported = descriptor.props.get_all_strings(OFX_IMAGE_EFFECT_PROP_SUPPORTED_CONTEXTS);
        let context = HOST_CONTEXTS
            .iter()
            .find(|c| supported.iter().any(|s| s == *c))
            .copied()
            .or_else(|| if supported.is_empty() { Some(OFX_IMAGE_EFFECT_CONTEXT_FILTER) } else { None })
            .ok_or_else(|| format!("no shared context (plug-in offers {:?}, host supports {:?})", supported, HOST_CONTEXTS))?;
        descriptor.props.set_string(OFX_IMAGE_EFFECT_PROP_CONTEXT, context);

        // Action: DescribeInContext
        let in_context_act = CString::new(OFX_IMAGE_EFFECT_ACTION_DESCRIBE_IN_CONTEXT).map_err(|e| e.to_string())?;
        let in_args = PropertySet::lenient();
        in_args.set_string(OFX_IMAGE_EFFECT_PROP_CONTEXT, context);
        let in_args_handle = &in_args as *const PropertySet as *mut c_void;

        let ctx_status = (p.main_entry)(in_context_act.as_ptr(), desc_handle, in_args_handle, std::ptr::null_mut());
        let param_count = descriptor.param_set.param_order.lock().unwrap_or_else(|e| e.into_inner()).len();
        log::debug!("[ofx] '{id_str}' kOfxImageEffectActionDescribeInContext({context}) -> {ctx_status} ({param_count} param(s))");
        descriptor.props.log_dump("descriptor");
        if ctx_status != OFX_STAT_OK && ctx_status != OFX_STAT_REPLY_DEFAULT {
            return Err(format!("kOfxImageEffectActionDescribeInContext failed with status {ctx_status}"));
        }

        // Extract metadata and parameters from descriptor
        let name = descriptor
            .props
            .get_string(OFX_PROP_LABEL, 0)
            .or_else(|| descriptor.props.get_string(OFX_PROP_SHORT_LABEL, 0))
            .or_else(|| descriptor.props.get_string(OFX_PROP_NAME, 0))
            .unwrap_or_else(|| id_str.split('.').last().unwrap_or("Effect").to_string());

        let category = descriptor.props.get_string(OFX_IMAGE_EFFECT_PLUGIN_PROP_GROUPING, 0).unwrap_or_else(|| "OpenFX".to_string());

        let description = descriptor.props.get_string(OFX_PROP_PLUGIN_DESCRIPTION, 0).unwrap_or_default();

        let version = format!("{}.{}", p.plugin_version_major, p.plugin_version_minor);

        // Convert parameters to EffectCraft PluginParam
        let mut plugin_params = Vec::new();
        let mut param_names = Vec::new();
        let mut used_ids = std::collections::HashSet::new();
        let param_order = descriptor.param_set.param_order.lock().unwrap_or_else(|e| e.into_inner()).clone();
        for param_name in param_order {
            if let Some(param) = descriptor.param_set.get_param(&param_name) {
                // Don't expose secret params or page/group containers
                if param.props.get_int(OFX_PARAM_PROP_SECRET, 0).unwrap_or(0) != 0 {
                    continue;
                }
                if param.param_type == OFX_PARAM_TYPE_GROUP || param.param_type == OFX_PARAM_TYPE_PAGE {
                    continue;
                }

                let label = param.props.get_string(OFX_PROP_LABEL, 0).unwrap_or_else(|| param.name.clone());

                // Sanitize ID for EffectCraft (alphanumeric, underscore)
                let mut safe_id = param.name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect::<String>();
                if safe_id.is_empty() {
                    safe_id = format!("param_{}", plugin_params.len());
                }
                // Keep the id unique after sanitising (e.g. "a b" and "a-b").
                while !used_ids.insert(safe_id.clone()) {
                    safe_id.push('_');
                }

                let kind = match param.param_type.as_str() {
                    OFX_PARAM_TYPE_DOUBLE => {
                        let def = param.props.get_double(OFX_PARAM_PROP_DEFAULT, 0).unwrap_or(0.0);
                        let min = param.props.get_double(OFX_PARAM_PROP_MIN, 0).unwrap_or(0.0);
                        let max = param.props.get_double(OFX_PARAM_PROP_MAX, 0).unwrap_or(100.0);
                        let s_min = param.props.get_double(OFX_PARAM_PROP_DISPLAY_MIN, 0);
                        let s_max = param.props.get_double(OFX_PARAM_PROP_DISPLAY_MAX, 0);
                        let digits = param.props.get_int(OFX_PARAM_PROP_DIGITS, 0).unwrap_or(2).clamp(0, 4) as u8;

                        PluginParamKind::Slider {
                            default: clamp_default(def, min, max),
                            min,
                            max: if max > min { max } else { min + 100.0 },
                            slider_min: s_min,
                            slider_max: s_max,
                            decimals: digits,
                        }
                    }
                    OFX_PARAM_TYPE_INTEGER => {
                        let def = param.props.get_double(OFX_PARAM_PROP_DEFAULT, 0).unwrap_or(0.0);
                        let min = param.props.get_double(OFX_PARAM_PROP_MIN, 0).unwrap_or(0.0);
                        let max = param.props.get_double(OFX_PARAM_PROP_MAX, 0).unwrap_or(100.0);

                        PluginParamKind::Slider {
                            default: clamp_default(def, min, max),
                            min,
                            max: if max > min { max } else { min + 100.0 },
                            slider_min: None,
                            slider_max: None,
                            decimals: 0,
                        }
                    }
                    OFX_PARAM_TYPE_BOOLEAN => {
                        let def = param.props.get_int(OFX_PARAM_PROP_DEFAULT, 0).unwrap_or(0) != 0;
                        PluginParamKind::Checkbox { default: def }
                    }
                    OFX_PARAM_TYPE_CHOICE => {
                        let options = param.props.get_all_strings(OFX_PARAM_PROP_CHOICE_OPTION);
                        let def = param.props.get_int(OFX_PARAM_PROP_DEFAULT, 0).unwrap_or(0).max(0) as u32;
                        PluginParamKind::Popup { options: if options.is_empty() { vec!["Default".into()] } else { options }, default: def }
                    }
                    OFX_PARAM_TYPE_RGBA => {
                        let r = param.props.get_double(OFX_PARAM_PROP_DEFAULT, 0).unwrap_or(1.0);
                        let g = param.props.get_double(OFX_PARAM_PROP_DEFAULT, 1).unwrap_or(1.0);
                        let b = param.props.get_double(OFX_PARAM_PROP_DEFAULT, 2).unwrap_or(1.0);
                        let a = param.props.get_double(OFX_PARAM_PROP_DEFAULT, 3).unwrap_or(1.0);
                        PluginParamKind::Color { default: [r, g, b, a] }
                    }
                    OFX_PARAM_TYPE_RGB => {
                        let r = param.props.get_double(OFX_PARAM_PROP_DEFAULT, 0).unwrap_or(1.0);
                        let g = param.props.get_double(OFX_PARAM_PROP_DEFAULT, 1).unwrap_or(1.0);
                        let b = param.props.get_double(OFX_PARAM_PROP_DEFAULT, 2).unwrap_or(1.0);
                        PluginParamKind::Color { default: [r, g, b, 1.0] }
                    }
                    OFX_PARAM_TYPE_DOUBLE_2D | OFX_PARAM_TYPE_INTEGER_2D => {
                        let x = param.props.get_double(OFX_PARAM_PROP_DEFAULT, 0).unwrap_or(0.5);
                        let y = param.props.get_double(OFX_PARAM_PROP_DEFAULT, 1).unwrap_or(0.5);
                        PluginParamKind::Point { default: [x, y] }
                    }
                    _ => continue,
                };

                plugin_params.push(PluginParam { id: safe_id, name: label, kind });
                param_names.push(param.name.clone());
            }
        }

        // Clean id for EffectCraft (cannot start with ec.)
        let clean_id = if id_str.starts_with("ec.") { format!("ofx.{}", id_str) } else { id_str };

        let manifest =
            PluginManifest { api: PLUGIN_API_VERSION, id: clean_id, name, category, version, author: "OpenFX".to_string(), description, params: plugin_params };
        // Fail early with a readable reason rather than at registration time.
        manifest.validate()?;

        // "needed" means the effect cannot render on the CPU at all.
        let gl_needed = match descriptor.props.get_string(OFX_IMAGE_EFFECT_PROP_OPENGL_RENDER_SUPPORTED, 0).as_deref() {
            Some("needed") => Some(true),
            Some("true") => Some(false),
            _ => None,
        };

        Ok(Self {
            manifest,
            plugin_ptr: plugin as usize,
            descriptor,
            param_names,
            context: context.to_string(),
            gl_needed,
            live_instance: Mutex::new(None),
            _lib: lib,
        })
    }

    fn ensure_instance(&self) -> Result<Arc<ImageEffectInstance>, String> {
        let mut guard = self.live_instance.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(inst) = guard.as_ref() {
            return Ok(inst.clone());
        }

        let inst = Arc::new(ImageEffectInstance::new());
        let p = unsafe { &*(self.plugin_ptr as *const OfxPlugin) };
        let inst_handle = Arc::as_ptr(&inst) as *const c_void;

        // A real host's instance inherits everything the plug-in set on its
        // descriptor (clips, context, render support flags). Plug-ins read those
        // back in createInstance and abort with kOfxStatErrBadIndex otherwise.
        inst.props.copy_from(&self.descriptor.props);
        inst.props.set_string(OFX_PROP_TYPE, OFX_TYPE_IMAGE_EFFECT_INSTANCE);
        inst.props.set_string(OFX_IMAGE_EFFECT_PROP_CONTEXT, &self.context);

        // The instance starts with the same clips and parameters the descriptor
        // defined, including their properties (defaults, ranges, options): a
        // plug-in reads those back during createInstance.
        {
            let desc_clips = self.descriptor.clips.lock().unwrap_or_else(|e| e.into_inner());
            for clip in desc_clips.values() {
                let copy = Arc::new(crate::image_effect::Clip::new(&clip.name));
                copy.props.copy_from(&clip.props);
                inst.add_clip(copy);
            }
        }
        let param_order = self.descriptor.param_set.param_order.lock().unwrap_or_else(|e| e.into_inner()).clone();
        for name in param_order {
            if let Some(source) = self.descriptor.param_set.get_param(&name) {
                let param = Arc::new(Param::from_template(&source));
                inst.param_set.add_param(param);
            }
        }

        // Action: CreateInstance
        unsafe {
            let create_act = CString::new(OFX_ACTION_CREATE_INSTANCE).map_err(|e| e.to_string())?;
            let status = (p.main_entry)(create_act.as_ptr(), inst_handle, std::ptr::null_mut(), std::ptr::null_mut());
            log::debug!("[ofx] '{}' kOfxActionCreateInstance -> {status}", self.manifest.id);
            if status != OFX_STAT_OK && status != OFX_STAT_REPLY_DEFAULT {
                return Err(format!("kOfxActionCreateInstance failed with status {status}"));
            }
        }

        // Ask the plug-in to set up its GPU resources while our context is current.
        // Only effects that cannot render on the CPU at all ("needed") get this:
        // an effect that merely supports OpenGL keeps its working CPU path.
        if self.gl_needed == Some(true) && crate::opengl::make_current() {
            unsafe {
                let attach_act = CString::new(OFX_ACTION_OPENGL_CONTEXT_ATTACHED).map_err(|e| e.to_string())?;
                let status = (p.main_entry)(attach_act.as_ptr(), inst_handle, std::ptr::null_mut(), std::ptr::null_mut());
                log::info!("[ofx] '{}' kOfxActionOpenGLContextAttached -> {status}", self.manifest.id);
                if status == OFX_STAT_OK || status == OFX_STAT_REPLY_DEFAULT {
                    inst.gl_ready.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }

        *guard = Some(inst.clone());
        Ok(inst)
    }

    fn render_with_instance(&self, frame: &mut PluginFrame, params: &PluginParams, time: f64, instance: &Arc<ImageEffectInstance>) -> Result<(), String> {
        let p = unsafe { &*(self.plugin_ptr as *const OfxPlugin) };

        // Push EffectCraft's values into the instance's parameters.
        let mut offset = 0;
        for (spec, ofx_name) in self.manifest.params.iter().zip(&self.param_names) {
            let w = spec.kind.width();
            let Some(slice) = params.flat.get(offset..offset + w) else {
                return Err(format!("parameter list too short at '{}'", spec.id));
            };
            if let Some(param) = instance.param_set.get_param(ofx_name) {
                param.set_from_flat_f64(slice);
            }
            offset += w;
        }

        let width = frame.width;
        let height = frame.height;
        let pixels = width as usize * height as usize;
        if frame.pixels.len() < pixels {
            return Err("frame buffer smaller than its declared size".into());
        }

        // A separate source copy and output buffer, as a real host provides.
        let mut source: Vec<[f32; 4]> = frame.pixels[..pixels].to_vec();
        let mut output: Vec<[f32; 4]> = source.clone();

        // GPU effects draw into our framebuffer. `begin_frame` binds it and
        // leaves the context current; the clip suite uploads the source.
        let use_gl = instance.gl_ready.load(std::sync::atomic::Ordering::Relaxed) && crate::opengl::begin_frame(width, height);
        if self.gl_needed == Some(true) && !use_gl {
            return Err("this effect needs an OpenGL render context, which could not be created".into());
        }
        let gl_texture = if use_gl { crate::opengl::output_texture() } else { 0 };
        let set_gpu_flags = |args: &PropertySet| {
            args.set_int(OFX_IMAGE_EFFECT_PROP_OPENGL_ENABLED, i32::from(use_gl));
            if use_gl {
                args.set_int(OFX_IMAGE_EFFECT_PROP_OPENGL_TEXTURE_INDEX, gl_texture as i32);
                args.set_int(OFX_IMAGE_EFFECT_PROP_OPENGL_TEXTURE_TARGET, 0x0DE1);
            }
            args.set_int(OFX_IMAGE_EFFECT_PROP_OPENCL_ENABLED, 0);
            args.set_int(OFX_IMAGE_EFFECT_PROP_CUDA_ENABLED, 0);
            args.set_int(OFX_IMAGE_EFFECT_PROP_METAL_ENABLED, 0);
        };

        let in_args = PropertySet::lenient();
        in_args.set_double(OFX_PROP_TIME, time);
        in_args.set_string(OFX_IMAGE_EFFECT_PROP_PIXEL_DEPTH, OFX_BIT_DEPTH_FLOAT);
        in_args.set_string(OFX_IMAGE_EFFECT_PROP_COMPONENTS, OFX_IMAGE_COMPONENT_RGBA);
        // Everything the render action is required to receive. A plug-in that
        // reads one of these with a strict look-up aborts with kOfxStatErrBadIndex,
        // so each is set even when the answer is "no".
        in_args.set_string(OFX_IMAGE_EFFECT_PROP_FIELD_TO_RENDER, OFX_IMAGE_FIELD_NONE);
        in_args.set_int(OFX_IMAGE_EFFECT_PROP_SEQUENTIAL_RENDER_STATUS, 0);
        in_args.set_int(OFX_IMAGE_EFFECT_PROP_INTERACTIVE_RENDER_STATUS, 0);
        in_args.set_int(OFX_IMAGE_EFFECT_PROP_RENDER_QUALITY_DRAFT, 0);
        set_gpu_flags(&in_args);
        in_args.set_double(OFX_IMAGE_EFFECT_PROP_FRAME_RATE, 24.0);
        in_args.set_double("OfxImagePropPixelAspectRatio", 1.0);
        {
            let mut map = in_args.props.lock().unwrap_or_else(|e| e.into_inner());
            map.insert(
                OFX_IMAGE_EFFECT_PROP_RENDER_WINDOW.to_string(),
                vec![PropVal::Int(0), PropVal::Int(0), PropVal::Int(width as i32), PropVal::Int(height as i32)],
            );
            map.insert(OFX_IMAGE_EFFECT_PROP_RENDER_SCALE.to_string(), vec![PropVal::Double(frame.scale), PropVal::Double(frame.scale)]);
            map.insert(OFX_IMAGE_EFFECT_PROP_FRAME_RANGE.to_string(), vec![PropVal::Double(time), PropVal::Double(time)]);
        }

        let render_act = CString::new(OFX_IMAGE_EFFECT_ACTION_RENDER).map_err(|e| e.to_string())?;
        let inst_handle = Arc::as_ptr(instance) as *const c_void;
        let in_args_handle = &in_args as *const PropertySet as *mut c_void;

        set_current_time(time);
        instance.prepare_clips(time, 24.0);
        begin_render(ActiveRender {
            source: source.as_mut_ptr() as *mut c_void,
            output: output.as_mut_ptr() as *mut c_void,
            width,
            height,
            scale: frame.scale,
            time,
            frame_rate: 24.0,
        });

        // A sequence of one frame, as the OFX spec requires around every render.
        // Plug-ins allocate their render state (and check they may run at all)
        // here, so skipping it makes them fail later with an unhelpful status.
        let sequence_args = PropertySet::lenient();
        sequence_args.set_double(OFX_IMAGE_EFFECT_PROP_FRAME_STEP, 1.0);
        sequence_args.set_int(OFX_PROP_IS_INTERACTIVE, 1);
        sequence_args.set_int(OFX_IMAGE_EFFECT_PROP_SEQUENTIAL_RENDER_STATUS, 0);
        sequence_args.set_int(OFX_IMAGE_EFFECT_PROP_INTERACTIVE_RENDER_STATUS, 0);
        set_gpu_flags(&sequence_args);
        sequence_args.set_double(OFX_IMAGE_EFFECT_PROP_FRAME_RATE, 24.0);
        {
            let mut map = sequence_args.props.lock().unwrap_or_else(|e| e.into_inner());
            // Render scale is a two-dimensional property: both components must
            // exist, or a plug-in that reads index 1 divides by zero.
            map.insert(OFX_IMAGE_EFFECT_PROP_RENDER_SCALE.to_string(), vec![PropVal::Double(frame.scale), PropVal::Double(frame.scale)]);
            map.insert(OFX_IMAGE_EFFECT_PROP_FRAME_RANGE.to_string(), vec![PropVal::Double(time), PropVal::Double(time)]);
        }
        let out_args = PropertySet::lenient();
        let sequence_args_handle = &sequence_args as *const PropertySet as *mut c_void;
        let out_args_handle = &out_args as *const PropertySet as *mut c_void;

        let begin_act = CString::new(OFX_IMAGE_EFFECT_ACTION_BEGIN_SEQUENCE_RENDER).map_err(|e| e.to_string())?;
        let begin_status = unsafe { (p.main_entry)(begin_act.as_ptr(), inst_handle, sequence_args_handle, out_args_handle) };
        if begin_status != OFX_STAT_OK && begin_status != OFX_STAT_REPLY_DEFAULT {
            log::warn!("[ofx] '{}' kOfxImageEffectActionBeginSequenceRender -> {begin_status}", self.manifest.id);
        }

        let status = unsafe { (p.main_entry)(render_act.as_ptr(), inst_handle, in_args_handle, out_args_handle) };

        let end_act = CString::new(OFX_IMAGE_EFFECT_ACTION_END_SEQUENCE_RENDER).map_err(|e| e.to_string())?;
        let end_status = unsafe { (p.main_entry)(end_act.as_ptr(), inst_handle, sequence_args_handle, out_args_handle) };
        if end_status != OFX_STAT_OK && end_status != OFX_STAT_REPLY_DEFAULT {
            log::warn!("[ofx] '{}' kOfxImageEffectActionEndSequenceRender -> {end_status}", self.manifest.id);
        }

        end_render();

        if status != OFX_STAT_OK {
            return Err(format!("OFX render failed with status {status}"));
        }

        if use_gl && !crate::opengl::read_output(&mut output, width, height) {
            return Err("the OpenGL effect did not leave a readable frame".into());
        }

        frame.pixels[..pixels].copy_from_slice(&output);
        Ok(())
    }
}

/// Keep a plug-in's default inside the range it declared (some plug-ins publish
/// defaults outside their own min/max, and EffectCraft validates ranges).
fn clamp_default(default: f64, min: f64, max: f64) -> f64 {
    if !default.is_finite() {
        return min;
    }
    let hi = if max > min { max } else { min };
    default.clamp(min.min(hi), hi.max(min))
}

impl EffectPlugin for OfxEffectPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn render(&self, frame: &mut PluginFrame, params: &PluginParams, time: f64) -> Result<(), String> {
        let instance = self.ensure_instance()?;
        self.render_with_instance(frame, params, time, &instance)
    }
}
