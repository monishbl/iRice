//! EGL on Wayland for Slint's FemtoVG renderer: one display and context for
//! the process, one window surface per layer surface. libEGL is loaded at run
//! time (khronos-egl "dynamic"), as Mesa ships it.

use khronos_egl as egl;
use std::cell::Cell;
use std::error::Error;
use std::ffi::{c_void, CStr};
use std::num::NonZeroU32;
use std::rc::Rc;

type Res<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub struct Egl {
    api: egl::DynamicInstance<egl::EGL1_4>,
    display: egl::Display,
    config: egl::Config,
    context: egl::Context,
}

impl Egl {
    pub fn new(wl_display: *mut c_void) -> Result<Rc<Self>, Box<dyn Error>> {
        let api = unsafe { egl::DynamicInstance::<egl::EGL1_4>::load_required()? };
        let display = unsafe { api.get_display(wl_display as egl::NativeDisplayType) }.ok_or("no EGL display")?;
        api.initialize(display)?;
        api.bind_api(egl::OPENGL_ES_API)?;
        #[rustfmt::skip]
        let attrs = [
            egl::SURFACE_TYPE, egl::WINDOW_BIT,
            egl::RENDERABLE_TYPE, egl::OPENGL_ES2_BIT,
            egl::RED_SIZE, 8, egl::GREEN_SIZE, 8, egl::BLUE_SIZE, 8, egl::ALPHA_SIZE, 8,
            egl::STENCIL_SIZE, 8, // FemtoVG fills paths through the stencil
            egl::NONE,
        ];
        let config = api.choose_first_config(display, &attrs)?.ok_or("no RGBA8 EGL config")?;
        let context = api
            .create_context(display, config, None, &[egl::CONTEXT_CLIENT_VERSION, 3, egl::NONE])
            .or_else(|_| api.create_context(display, config, None, &[egl::CONTEXT_CLIENT_VERSION, 2, egl::NONE]))?;
        Ok(Rc::new(Self { api, display, config, context }))
    }
}

/// One surface's GL target, handed to Slint's FemtoVG renderer.
pub struct GlSurface {
    egl: Rc<Egl>,
    window: wayland_egl::WlEglSurface,
    surface: egl::Surface,
    interval_set: Cell<bool>,
}

impl GlSurface {
    pub fn new(egl: Rc<Egl>, wl_surface_id: wayland_client::backend::ObjectId) -> Result<Self, Box<dyn Error>> {
        let window = wayland_egl::WlEglSurface::new(wl_surface_id, 1, 1)?;
        let surface = unsafe {
            egl.api.create_window_surface(egl.display, egl.config, window.ptr() as egl::NativeWindowType, None)?
        };
        Ok(Self { egl, window, surface, interval_set: Cell::new(false) })
    }
}

unsafe impl slint::platform::femtovg_renderer::OpenGLInterface for GlSurface {
    fn ensure_current(&self) -> Res<()> {
        let e = &self.egl;
        e.api.make_current(e.display, Some(self.surface), Some(self.surface), Some(e.context))?;
        if !self.interval_set.replace(true) {
            // Never block in eglSwapBuffers: frames are paced by wl_surface.frame
            e.api.swap_interval(e.display, 0)?;
        }
        Ok(())
    }
    fn swap_buffers(&self) -> Res<()> {
        Ok(self.egl.api.swap_buffers(self.egl.display, self.surface)?)
    }
    fn resize(&self, width: NonZeroU32, height: NonZeroU32) -> Res<()> {
        self.window.resize(width.get() as i32, height.get() as i32, 0, 0);
        Ok(())
    }
    fn get_proc_address(&self, name: &CStr) -> *const c_void {
        name.to_str()
            .ok()
            .and_then(|n| self.egl.api.get_proc_address(n))
            .map_or(std::ptr::null(), |f| f as *const c_void)
    }
}

impl Drop for GlSurface {
    fn drop(&mut self) {
        let _ = self.egl.api.destroy_surface(self.egl.display, self.surface);
    }
}
