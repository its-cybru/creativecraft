//! OpenGL rendering for OpenFX plug-ins.
//!
//! Some effects (Red Giant among them) only render when the host gives them a
//! real OpenGL context and the `OfxImageEffectOpenGLRenderSuite`. This module
//! keeps a hidden WGL context, uploads the source frame into a texture, lets the
//! plug-in draw into a framebuffer, and reads the result back into the CPU frame.
//!
//! The context lives on the calling thread: both the helper process and the
//! tests render on one thread, and a WGL context may only be current on one.

#![allow(unsafe_code, unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;

use crate::ffi::*;
use crate::image_effect::{Clip, active_render};
use crate::property::{PropVal, PropertySet};

/// `GL_TEXTURE_2D`, the only target this host hands out.
const GL_TEXTURE_2D: i32 = 0x0DE1;

pub static OPENGL_RENDER_SUITE: OfxImageEffectOpenGLRenderSuiteV1 = OfxImageEffectOpenGLRenderSuiteV1 { clip_load_texture, clip_free_texture, flush_resources };

/// Is a usable OpenGL context available on this machine and thread?
pub fn available() -> bool {
    imp::available()
}

/// Make this thread's context current (the plug-in expects it during render).
pub fn make_current() -> bool {
    imp::make_current()
}

/// Prepare the output framebuffer for one frame and bind it.
pub fn begin_frame(width: u32, height: u32) -> bool {
    imp::begin_frame(width, height)
}

/// The texture the plug-in draws into, for the render action's properties.
pub fn output_texture() -> u32 {
    imp::output_texture()
}

/// Copy the framebuffer back into `out`.
pub fn read_output(out: &mut [[f32; 4]], width: u32, height: u32) -> bool {
    imp::read_output(out, width, height)
}

fn upload(pixels: &[[f32; 4]], width: u32, height: u32) -> Option<u32> {
    imp::upload(pixels, width, height)
}

fn delete_texture(texture: u32) {
    imp::delete_texture(texture);
}

/// One texture handed to a plug-in by `clipLoadTexture`.
///
/// `props` must stay the first field: the handle is the address of the property
/// set, and `clipFreeTexture` turns it back into the whole box.
#[repr(C)]
struct TextureBuffer {
    props: PropertySet,
    texture: u32,
    /// Source textures belong to the plug-in once handed over; the output
    /// texture belongs to the host and must survive the free.
    owned: bool,
}

unsafe extern "C" fn clip_load_texture(
    clip: OfxImageClipHandle,
    time: OfxTime,
    _format: *const std::ffi::c_char,
    _region: *const OfxRectD,
    texture_handle: *mut OfxPropertySetHandle,
) -> OfxStatus {
    if clip.is_null() || texture_handle.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    let Some(active) = active_render() else {
        return OFX_STAT_FAILED;
    };
    let clip = &*(clip as *const Clip);
    let is_output = clip.name == OFX_IMAGE_EFFECT_OUTPUT_CLIP_NAME;

    let (texture, owned) = if is_output {
        (output_texture(), false)
    } else {
        if active.source.is_null() {
            return OFX_STAT_FAILED;
        }
        let count = (active.width as usize).saturating_mul(active.height as usize);
        let pixels = std::slice::from_raw_parts(active.source as *const [f32; 4], count);
        match upload(pixels, active.width, active.height) {
            Some(texture) => (texture, true),
            None => return OFX_STAT_FAILED,
        }
    };
    if texture == 0 {
        return OFX_STAT_FAILED;
    }

    let buffer = Box::new(TextureBuffer { props: PropertySet::lenient(), texture, owned });
    let props = &buffer.props;
    props.set_string(OFX_PROP_TYPE, OFX_TYPE_IMAGE);
    props.set_string(OFX_IMAGE_EFFECT_PROP_PIXEL_DEPTH, OFX_BIT_DEPTH_FLOAT);
    props.set_string(OFX_IMAGE_EFFECT_PROP_COMPONENTS, OFX_IMAGE_COMPONENT_RGBA);
    props.set_string(OFX_IMAGE_EFFECT_PROP_PRE_MULTIPLICATION, OFX_IMAGE_PRE_MULTIPLIED);
    props.set_int(OFX_IMAGE_EFFECT_PROP_OPENGL_TEXTURE_INDEX, texture as i32);
    props.set_int(OFX_IMAGE_EFFECT_PROP_OPENGL_TEXTURE_TARGET, GL_TEXTURE_2D);
    props.set_double(OFX_PROP_TIME, time);
    props.set_double(OFX_IMAGE_PROP_PIXEL_ASPECT_RATIO, 1.0);
    props.set_string(OFX_IMAGE_PROP_FIELD, OFX_IMAGE_FIELD_NONE);
    props.set_string(OFX_IMAGE_PROP_UNIQUE_IDENTIFIER, &format!("ec-gl-texture-{time}"));
    {
        let (w, h) = (active.width as i32, active.height as i32);
        let mut map = props.props.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(OFX_IMAGE_PROP_BOUNDS.to_string(), vec![PropVal::Int(0), PropVal::Int(0), PropVal::Int(w), PropVal::Int(h)]);
        map.insert(OFX_IMAGE_PROP_REGION_OF_DEFINITION.to_string(), vec![PropVal::Int(0), PropVal::Int(0), PropVal::Int(w), PropVal::Int(h)]);
        map.insert(OFX_IMAGE_PROP_ROW_BYTES.to_string(), vec![PropVal::Int(w * 16)]);
        map.insert(OFX_IMAGE_EFFECT_PROP_RENDER_SCALE.to_string(), vec![PropVal::Double(active.scale), PropVal::Double(active.scale)]);
    }

    *texture_handle = Box::into_raw(buffer) as *mut c_void;
    OFX_STAT_OK
}

unsafe extern "C" fn clip_free_texture(texture_handle: OfxPropertySetHandle) -> OfxStatus {
    if texture_handle.is_null() {
        return OFX_STAT_ERR_BAD_HANDLE;
    }
    let buffer = Box::from_raw(texture_handle as *mut TextureBuffer);
    if buffer.owned {
        delete_texture(buffer.texture);
    }
    OFX_STAT_OK
}

unsafe extern "C" fn flush_resources() -> OfxStatus {
    OFX_STAT_REPLY_DEFAULT
}

// ---------------------------------------------------------------- Windows / WGL

#[cfg(windows)]
mod imp {
    use std::ffi::{CString, c_void};
    use std::sync::OnceLock;

    use glow::HasContext;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::GetDC;
    use windows_sys::Win32::Graphics::OpenGL::{
        ChoosePixelFormat, HGLRC, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE, PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR, SetPixelFormat,
        wglCreateContext, wglDeleteContext, wglGetProcAddress, wglMakeCurrent,
    };
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{CS_OWNDC, CreateWindowExW, DefWindowProcW, RegisterClassW, WNDCLASSW, WS_POPUP};

    struct Gl {
        _hwnd: HWND,
        _hdc: *mut c_void,
        _hrc: HGLRC,
        glow: glow::Context,
        fbo: glow::Framebuffer,
        out_tex: Option<glow::Texture>,
        size: (u32, u32),
    }

    thread_local! {
        static GL: std::cell::RefCell<Option<Gl>> = const { std::cell::RefCell::new(None) };
    }

    /// Run `f` with an initialised context, creating it on first use.
    fn with_gl<R>(f: impl FnOnce(&mut Gl) -> R) -> Option<R> {
        GL.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                *slot = Gl::new();
            }
            slot.as_mut().map(f)
        })
    }

    pub fn available() -> bool {
        with_gl(|_| ()).is_some()
    }

    pub fn make_current() -> bool {
        with_gl(|gl| unsafe { wglMakeCurrent(gl._hdc, gl._hrc) != 0 }).unwrap_or(false)
    }

    pub fn begin_frame(width: u32, height: u32) -> bool {
        let Some(result) = with_gl(|gl| unsafe {
            if gl.size != (width, height) || gl.out_tex.is_none() {
                if let Some(texture) = gl.out_tex.take() {
                    gl.glow.delete_texture(texture);
                }
                let texture = create_texture(&gl.glow, width, height)?;
                gl.glow.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(texture), 0);
                if gl.glow.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
                    gl.glow.delete_texture(texture);
                    return None;
                }
                gl.out_tex = Some(texture);
                gl.size = (width, height);
            }
            let texture = gl.out_tex?;
            gl.glow.bind_framebuffer(glow::FRAMEBUFFER, Some(gl.fbo));
            gl.glow.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(texture), 0);
            gl.glow.viewport(0, 0, width as i32, height as i32);
            gl.glow.disable(glow::SCISSOR_TEST);
            gl.glow.disable(glow::DEPTH_TEST);
            gl.glow.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.glow.clear(glow::COLOR_BUFFER_BIT);
            Some(())
        }) else {
            return false;
        };
        result.is_some()
    }

    pub fn output_texture() -> u32 {
        with_gl(|gl| gl.out_tex.map(|t| t.0.get())).flatten().unwrap_or(0)
    }

    pub fn upload(pixels: &[[f32; 4]], width: u32, height: u32) -> Option<u32> {
        with_gl(|gl| unsafe {
            let texture = create_texture(&gl.glow, width, height)?;
            let bytes = std::slice::from_raw_parts(pixels.as_ptr() as *const u8, pixels.len().saturating_mul(16));
            gl.glow.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.glow.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.glow.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                width as i32,
                height as i32,
                glow::RGBA,
                glow::FLOAT,
                glow::PixelUnpackData::Slice(Some(bytes)),
            );
            Some(texture.0.get())
        })
        .flatten()
    }

    pub fn delete_texture(texture: u32) {
        with_gl(|gl| unsafe {
            if let Some(texture) = std::num::NonZeroU32::new(texture).map(glow::NativeTexture) {
                gl.glow.delete_texture(texture);
            }
        });
    }

    pub fn read_output(out: &mut [[f32; 4]], width: u32, height: u32) -> bool {
        with_gl(|gl| unsafe {
            if gl.size != (width, height) || out.len() < (width as usize).saturating_mul(height as usize) {
                return false;
            }
            gl.glow.bind_framebuffer(glow::FRAMEBUFFER, Some(gl.fbo));
            gl.glow.finish();
            gl.glow.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
            let bytes = std::slice::from_raw_parts_mut(out.as_mut_ptr() as *mut u8, out.len().saturating_mul(16));
            gl.glow.read_pixels(0, 0, width as i32, height as i32, glow::RGBA, glow::FLOAT, glow::PixelPackData::Slice(Some(bytes)));
            true
        })
        .unwrap_or(false)
    }

    unsafe fn create_texture(gl: &glow::Context, width: u32, height: u32) -> Option<glow::Texture> {
        let texture = gl.create_texture().ok()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA32F as i32, width as i32, height as i32, 0, glow::RGBA, glow::FLOAT, glow::PixelUnpackData::Slice(None));
        Some(texture)
    }

    impl Gl {
        fn new() -> Option<Self> {
            unsafe {
                let class = wide("CreativeCraftOFXGL");
                let instance = GetModuleHandleW(std::ptr::null());
                let mut window_class: WNDCLASSW = std::mem::zeroed();
                window_class.style = CS_OWNDC;
                window_class.lpfnWndProc = Some(DefWindowProcW);
                window_class.hInstance = instance;
                window_class.lpszClassName = class.as_ptr();
                RegisterClassW(&window_class);

                let hwnd = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    std::ptr::null(),
                    WS_POPUP,
                    0,
                    0,
                    1,
                    1,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    instance,
                    std::ptr::null_mut(),
                );
                if hwnd.is_null() {
                    log::warn!("[ofx] could not create the hidden OpenGL window");
                    return None;
                }
                let hdc = GetDC(hwnd);
                if hdc.is_null() {
                    return None;
                }

                let mut pfd: PIXELFORMATDESCRIPTOR = std::mem::zeroed();
                pfd.nSize = std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16;
                pfd.nVersion = 1;
                pfd.dwFlags = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL;
                pfd.iPixelType = PFD_TYPE_RGBA;
                pfd.cColorBits = 32;
                pfd.cDepthBits = 24;
                pfd.cStencilBits = 8;
                pfd.iLayerType = PFD_MAIN_PLANE as u8;
                let format = ChoosePixelFormat(hdc, &pfd);
                if format == 0 || SetPixelFormat(hdc, format, &pfd) == 0 {
                    log::warn!("[ofx] no OpenGL pixel format on this machine");
                    return None;
                }

                let mut hrc = wglCreateContext(hdc);
                if hrc.is_null() || wglMakeCurrent(hdc, hrc) == 0 {
                    log::warn!("[ofx] wglCreateContext failed");
                    return None;
                }

                // Prefer a 3.0 compatibility context: new enough for framebuffer
                // objects, old enough that fixed-function plug-ins still work.
                let modern = load_proc("wglCreateContextAttribsARB");
                if !modern.is_null() {
                    type CreateContextAttribs = unsafe extern "system" fn(*mut c_void, HGLRC, *const i32) -> HGLRC;
                    let create: CreateContextAttribs = std::mem::transmute(modern);
                    let attributes = [
                        0x2091, 3, // WGL_CONTEXT_MAJOR_VERSION_ARB
                        0x2092, 0, // WGL_CONTEXT_MINOR_VERSION_ARB
                        0x9126, 2, // WGL_CONTEXT_PROFILE_MASK_ARB, compatibility
                        0,
                    ];
                    let newer = create(hdc, std::ptr::null_mut(), attributes.as_ptr());
                    if !newer.is_null() && wglMakeCurrent(hdc, newer) != 0 {
                        wglDeleteContext(hrc);
                        hrc = newer;
                    }
                }

                let loader = |name: &str| -> *const c_void {
                    let Ok(name) = CString::new(name) else {
                        return std::ptr::null();
                    };
                    if let Some(proc) = wglGetProcAddress(name.as_ptr() as *const u8) {
                        return proc as *const c_void;
                    }
                    static OPENGL32: OnceLock<isize> = OnceLock::new();
                    let library = *OPENGL32.get_or_init(|| {
                        let name = wide("opengl32.dll");
                        LoadLibraryW(name.as_ptr()) as isize
                    });
                    if library == 0 {
                        return std::ptr::null();
                    }
                    GetProcAddress(library as *mut c_void, name.as_ptr() as *const u8).map(|proc| proc as *const c_void).unwrap_or(std::ptr::null())
                };
                let glow = glow::Context::from_loader_function(loader);
                let fbo = glow.create_framebuffer().ok()?;
                log::info!("[ofx] OpenGL context ready for plug-ins");

                Some(Self { _hwnd: hwnd, _hdc: hdc, _hrc: hrc, glow, fbo, out_tex: None, size: (0, 0) })
            }
        }
    }

    unsafe fn load_proc(name: &str) -> *const c_void {
        let Ok(name) = CString::new(name) else {
            return std::ptr::null();
        };
        if let Some(proc) = wglGetProcAddress(name.as_ptr() as *const u8) {
            return proc as *const c_void;
        }
        let library = LoadLibraryW(wide("opengl32.dll").as_ptr());
        if library.is_null() {
            return std::ptr::null();
        }
        GetProcAddress(library, name.as_ptr() as *const u8).map(|proc| proc as *const c_void).unwrap_or(std::ptr::null())
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    // Keep the imports used only by signatures above alive on all builds.
    #[allow(dead_code)]
    fn _keep(_: WPARAM, _: LPARAM) -> LRESULT {
        0
    }
}

// ---------------------------------------------------------------- Other platforms

#[cfg(not(windows))]
mod imp {
    pub fn available() -> bool {
        false
    }
    pub fn make_current() -> bool {
        false
    }
    pub fn begin_frame(_width: u32, _height: u32) -> bool {
        false
    }
    pub fn output_texture() -> u32 {
        0
    }
    pub fn upload(_pixels: &[[f32; 4]], _width: u32, _height: u32) -> Option<u32> {
        None
    }
    pub fn delete_texture(_texture: u32) {}
    pub fn read_output(_out: &mut [[f32; 4]], _width: u32, _height: u32) -> bool {
        false
    }
}
