pub mod sys;

use openvg::sys::*;
use std::ptr::null_mut;

pub use openvg::sys::VC_RECT_T;

pub struct OpenVG(());

impl OpenVG {
    pub fn init() -> OpenVG {
        unsafe {
            bcm_host_init();
        }
        OpenVG(())
    }

    pub fn get_display_size(&self, index: u16) -> Option<(i32, i32)> {
        let mut width = 0u32;
        let mut height = 0u32;
        let status = unsafe { graphics_get_display_size(index, &mut width, &mut height) };
        if status >= 0 {
            Option::Some((width as i32, height as i32))
        } else {
            Option::None
        }
    }
}

pub struct DispmanDisplay {
    index: i32,
    display: DISPMANX_DISPLAY_HANDLE_T,
}

impl DispmanDisplay {
    pub fn open(index: i32) -> DispmanDisplay {
        let display = unsafe { vc_dispmanx_display_open(index as u32) };
        DispmanDisplay { index, display }
    }

    pub fn start_update(&self) -> DispmanUpdate {
        DispmanUpdate {
            display: self,
            update: unsafe { vc_dispmanx_update_start(self.index) },
        }
    }
}

pub struct DispmanUpdate<'a> {
    display: &'a DispmanDisplay,
    update: u32,
}

impl<'a> DispmanUpdate<'a> {
    pub fn element_add(
        &self,
        layer: i32,
        src: &VC_RECT_T,
        dest: &VC_RECT_T,
    ) -> DispmanElement {
        DispmanElement(unsafe {
            vc_dispmanx_element_add(
                self.update,
                self.display.display,
                layer,
                dest,
                0, // resource
                src,
                DISPMANX_PROTECTION_NONE,
                null_mut(), // alpha
                null_mut(), // clamp
                DISPMANX_TRANSFORM_T_DISPMANX_NO_ROTATE,
            )
        })
    }

    pub fn submit_sync(self) {
        unsafe {
            vc_dispmanx_update_submit_sync(self.update);
        }
    }
}

pub struct DispmanElement(DISPMANX_ELEMENT_HANDLE_T);

impl DispmanElement {
    pub fn to_window(&self, width: i32, height: i32) -> EGL_DISPMANX_WINDOW_T {
        EGL_DISPMANX_WINDOW_T {
            element: self.0,
            width,
            height,
        }
    }
}

pub struct EGLDisplay(self::sys::EGLDisplay);

impl EGLDisplay {
    pub fn initialize(window: &mut EGL_DISPMANX_WINDOW_T) -> EGLDisplay {
        unsafe {
            let display = eglGetDisplay(null_mut());
            eglInitialize(display, null_mut(), null_mut());
            eglBindAPI(EGL_OPENVG_API);
            EGLDisplay(display)
        }
    }

    pub fn choose_config(&self, config: &[u32]) -> EGLConfig {
        let mut egl_config = null_mut();
        let mut num_config = 0i32;
        let mut attrs: Vec<_> = config.iter().cloned().collect();
        attrs.push(self::sys::EGL_NONE as u32);
        let status = unsafe {
            eglChooseConfig(
                self.0,
                attrs.as_ptr() as *const i32,
                &mut egl_config,
                1,
                &mut num_config,
            )
        };
        EGLConfig {
            display: self,
            config: egl_config,
        }
    }

    pub fn make_current(
        &self,
        write_surface: &EGLSurface,
        read_surface: &EGLSurface,
        context: &EGLContext,
    ) {
        unsafe {
            eglMakeCurrent(
                self.0,
                write_surface.surface,
                read_surface.surface,
                context.context,
            );
        }
    }
}

impl Drop for EGLDisplay {
    fn drop(&mut self) {
        unsafe {
            eglTerminate(self.0);
            eglReleaseThread();
        }
    }
}

pub struct EGLConfig<'a> {
    display: &'a EGLDisplay,
    config: self::sys::EGLConfig,
}

impl<'a> EGLConfig<'a> {
    pub fn create_window_surface(&self, window: &mut EGL_DISPMANX_WINDOW_T) -> EGLSurface {
        let surface = unsafe {
            eglCreateWindowSurface(
                self.display.0,
                self.config,
                ::std::mem::transmute(window),
                null_mut(),
            )
        };
        EGLSurface {
            display: self.display,
            config: self,
            surface,
        }
    }

    pub fn create_image_surface(&self, image: &VGImage) -> EGLSurface {
        let surface = unsafe {
            eglCreatePbufferFromClientBuffer(
                self.display.0,
                self::sys::EGL_OPENVG_IMAGE,
                ::std::mem::transmute(image.0),
                self.config,
                null_mut(),
            )
        };
        EGLSurface {
            display: self.display,
            config: self,
            surface,
        }
    }

    pub fn create_context(&self) -> EGLContext {
        let context =
            unsafe { eglCreateContext(self.display.0, self.config, null_mut(), null_mut()) };
        EGLContext { context }
    }
}

pub struct EGLSurface<'a> {
    display: &'a EGLDisplay,
    config: &'a EGLConfig<'a>,
    surface: self::sys::EGLSurface,
}

impl<'a> EGLSurface<'a> {
    pub fn swap_buffers(&self) {
        unsafe {
            eglSwapBuffers(self.display.0, self.surface);
        }
    }
}

pub struct EGLContext {
    context: self::sys::EGLContext,
}

pub use self::sys::VGImageFormat;
pub use self::sys::VGImageParamType;
pub use self::sys::VGImageQuality;

pub struct VGImage(self::sys::VGImage);
impl VGImage {
    pub fn new(
        format: VGImageFormat,
        width: i32,
        height: i32,
        quality: VGImageQuality,
    ) -> VGImage {
        unsafe { VGImage(vgCreateImage(format, width, height, quality as u32)) }
    }

    pub fn sub_data(
        &self,
        bytes: &[u8],
        stride: i32,
        format: VGImageFormat,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) {
        unsafe {
            vgImageSubData(
                self.0,
                bytes.as_ptr() as *mut ::std::os::raw::c_void,
                stride,
                format,
                x,
                y,
                width,
                height,
            )
        }
    }

    pub fn copy_image(&self, dest: &mut VGImage, dx: i32, dy: i32, sx: i32, sy: i32, width: i32, height: i32, dither: bool) {
        unsafe {
            vgCopyImage(
                dest.0,
                dx,
                dy,
                self.0,
                sx,
                sy,
                width,
                height,
                dither as u32,
            )
        }
    }

    pub fn height(&self) -> i32 {
        unsafe {
            sys::vgGetParameteri(self.0, VGImageParamType_VG_IMAGE_HEIGHT as i32)
        }
    }

    pub fn width(&self) -> i32 {
        unsafe {
            sys::vgGetParameteri(self.0, VGImageParamType_VG_IMAGE_WIDTH as i32)
        }
    }

    pub fn format(&self) -> VGImageFormat {
        unsafe {
            ::std::mem::transmute(sys::vgGetParameteri(self.0, VGImageParamType_VG_IMAGE_FORMAT as i32))
        }
    }
}
impl Drop for VGImage {
    fn drop(&mut self) {
        unsafe {
            vgDestroyImage(self.0);
        }
    }
}

pub use self::sys::VGBlendMode;
pub use self::sys::VGMatrixMode;
pub use self::sys::VGParamType;

pub fn vg_setfv(param: VGParamType, value: &[f32]) {
    unsafe {
        self::sys::vgSetfv(param, value.len() as i32, value.as_ptr());
    }
}

pub fn vg_clear(x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        self::sys::vgClear(x, y, w, h);
    }
}

pub fn vg_seti(param: VGParamType, value: i32) {
    unsafe {
        self::sys::vgSeti(param, value);
    }
}

pub fn vg_load_identity() {
    unsafe {
        self::sys::vgLoadIdentity();
    }
}

pub fn vg_translate(x: f32, y: f32) {
    unsafe {
        self::sys::vgTranslate(x, y);
    }
}

pub fn vg_scale(sx: f32, sy: f32) {
    unsafe {
        self::sys::vgScale(sx, sy);
    }
}

pub fn vg_rotate(theta: f32) {
    unsafe {
        self::sys::vgRotate(theta);
    }
}

pub fn vg_draw_image(image: &VGImage) {
    unsafe {
        self::sys::vgDrawImage(image.0);
    }
}

pub fn vg_get_matrix(m: &mut [f32]) {
    unsafe {
        sys::vgGetMatrix(m.as_mut_ptr())
    }
}

pub fn vg_load_matrix(m: &[f32]) {
    unsafe {
        sys::vgLoadMatrix(m.as_ptr())
    }
}

pub fn vg_color_matrix(dst: &mut VGImage, image: &VGImage, matrix: &[f32]) {
    unsafe {
        sys::vgColorMatrix(dst.0, image.0, matrix.as_ptr());
    }
}