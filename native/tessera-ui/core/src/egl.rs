//! EGL on Wayland for Slint's FemtoVG renderer: one display and context for
//! the process, one window surface per layer surface. libEGL is loaded at run
//! time (khronos-egl "dynamic"), as Mesa ships it.
//!
//! A frame redraws only what changed (when the driver says how old the back
//! buffer is, EGL_EXT_buffer_age): FemtoVG draws everything every frame, so
//! the vendored femtovg (vendor/femtovg) scissors its screen drawing to the
//! box that changed in this frame and the frames since the back buffer was
//! last drawn (src/damage.rs finds what changed). A big window whose small
//! part animates fills only that part. TESSERA_UI_FULL_REDRAW=1 turns it off.

use khronos_egl as egl;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
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
    /// eglSwapBuffersWithDamageKHR (or EXT), when the driver has it: the
    /// compositor hears which part of a frame changed (src/damage.rs)
    swap_with_damage: Option<SwapWithDamage>,
    /// EGL_EXT_buffer_age (with the damage above): frames redraw only what changed
    buffer_age: bool,
}

type SwapWithDamage = unsafe extern "system" fn(egl::EGLDisplay, egl::EGLSurface, *const egl::Int, egl::Int) -> egl::Boolean;

/// EGL_BUFFER_AGE_EXT
const BUFFER_AGE: egl::Int = 0x313D;

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
        let extensions = api.query_string(Some(display), egl::EXTENSIONS).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let has = |e: &str| extensions.split_ascii_whitespace().any(|x| x == e);
        let swap_with_damage = [("EGL_KHR_swap_buffers_with_damage", "eglSwapBuffersWithDamageKHR"), ("EGL_EXT_swap_buffers_with_damage", "eglSwapBuffersWithDamageEXT")]
            .iter()
            .filter(|(ext, _)| has(ext) && !crate::env::is_set("FULL_DAMAGE"))
            .find_map(|(_, f)| api.get_proc_address(f))
            .map(|f| unsafe { std::mem::transmute::<extern "system" fn(), SwapWithDamage>(f) });
        let buffer_age = has("EGL_EXT_buffer_age") && swap_with_damage.is_some() && !crate::env::is_set("UI_FULL_REDRAW");
        if crate::env::is_set("DEBUG") {
            eprintln!("debug: egl: damage {} buffer age {}", swap_with_damage.is_some(), buffer_age);
        }
        Ok(Rc::new(Self { api, display, config, context, swap_with_damage, buffer_age }))
    }

    /// Frames can say which part of them changed
    pub fn has_damage(&self) -> bool {
        self.swap_with_damage.is_some()
    }
}

/// One surface's GL target, handed to Slint's FemtoVG renderer.
pub struct GlSurface {
    egl: Rc<Egl>,
    window: wayland_egl::WlEglSurface,
    surface: egl::Surface,
    interval_set: Cell<bool>,
    /// what changed in the frame being drawn: rectangles in buffer pixels,
    /// top-left origin, and the buffer's height (None: all of it)
    pub damage: Rc<RefCell<Option<(Vec<[i32; 4]>, i32)>>>,
    /// a frame starts (Surface::render): the next ensure_current sets its clip
    pub frame: Rc<Cell<bool>>,
    /// the last frames' changed boxes, newest first, in GL's window
    /// coordinates (x, y from the bottom; None: all of it)
    history: RefCell<VecDeque<Option<[i32; 4]>>>,
}

impl GlSurface {
    pub fn new(egl: Rc<Egl>, wl_surface_id: wayland_client::backend::ObjectId) -> Result<Self, Box<dyn Error>> {
        let window = wayland_egl::WlEglSurface::new(wl_surface_id, 1, 1)?;
        let surface = unsafe {
            egl.api.create_window_surface(egl.display, egl.config, window.ptr() as egl::NativeWindowType, None)?
        };
        Ok(Self { egl, window, surface, interval_set: Cell::new(false), damage: Default::default(), frame: Default::default(), history: Default::default() })
    }

    /// The frame about to be drawn redraws only what changed in it and in the
    /// frames since this back buffer was drawn (its age), as one box; all of
    /// it when that isn't known (the first frames, a new size, no age)
    fn set_clip(&self) {
        let now = self.damage.borrow().as_ref().map(|(rects, h)| bbox(&egl_rects(rects, *h)));
        let mut hist = self.history.borrow_mut();
        let clip = if self.egl.buffer_age {
            let age = self.egl.api.query_surface(self.egl.display, self.surface, BUFFER_AGE).unwrap_or(0).max(0) as usize;
            clip_for(now, &hist, age)
        } else {
            None
        };
        hist.push_front(now);
        hist.truncate(4);
        femtovg::renderer::set_screen_clip(clip);
    }
}

/// What a frame must redraw: this frame's box and the `age - 1` before it
/// (age 1: the back buffer is the last frame); None: all of it
fn clip_for(now: Option<[i32; 4]>, hist: &VecDeque<Option<[i32; 4]>>, age: usize) -> Option<[i32; 4]> {
    if age == 0 || age - 1 > hist.len() {
        return None;
    }
    hist.iter().take(age - 1).try_fold(now?, |a, f| f.map(|b| union(a, b)))
}

fn bbox(r: &[i32]) -> [i32; 4] {
    let mut b = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
    for q in r.chunks(4) {
        b = [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0] + q[2]), b[3].max(q[1] + q[3])];
    }
    [b[0], b[1], (b[2] - b[0]).max(0), (b[3] - b[1]).max(0)]
}

fn union(a: [i32; 4], b: [i32; 4]) -> [i32; 4] {
    let (x0, y0) = (a[0].min(b[0]), a[1].min(b[1]));
    let (x1, y1) = ((a[0] + a[2]).max(b[0] + b[2]), (a[1] + a[3]).max(b[1] + b[3]));
    [x0, y0, x1 - x0, y1 - y0]
}

/// Damage rectangles for EGL: x, y from the bottom-left, w, h (EGL's buffer
/// coordinates), from top-left ones; at least one (none would mean all)
pub fn egl_rects(rects: &[[i32; 4]], buffer_h: i32) -> Vec<i32> {
    let mut out: Vec<i32> = rects.iter().filter(|r| r[2] > 0 && r[3] > 0).flat_map(|r| [r[0], buffer_h - (r[1] + r[3]), r[2], r[3]]).collect();
    if out.is_empty() {
        out.extend([0, buffer_h.max(1) - 1, 1, 1]);
    }
    out
}

unsafe impl slint::platform::femtovg_renderer::OpenGLInterface for GlSurface {
    fn ensure_current(&self) -> Res<()> {
        let e = &self.egl;
        e.api.make_current(e.display, Some(self.surface), Some(self.surface), Some(e.context))?;
        if !self.interval_set.replace(true) {
            // Never block in eglSwapBuffers: frames are paced by wl_surface.frame
            e.api.swap_interval(e.display, 0)?;
        }
        // (once per frame, as it begins: other calls make the context current too)
        if self.frame.replace(false) {
            self.set_clip();
        }
        Ok(())
    }
    fn swap_buffers(&self) -> Res<()> {
        femtovg::renderer::set_screen_clip(None);
        // Only what changed is damaged: Hyprland then repaints that part of
        // the screen, not all of it (and the frame drew only that: set_clip)
        if let (Some(f), Some((rects, h))) = (self.egl.swap_with_damage, self.damage.borrow_mut().take()) {
            let r = egl_rects(&rects, h);
            if unsafe { f(self.egl.display.as_ptr(), self.surface.as_ptr(), r.as_ptr(), (r.len() / 4) as egl::Int) } == egl::TRUE {
                return Ok(());
            }
        }
        Ok(self.egl.api.swap_buffers(self.egl.display, self.surface)?)
    }
    fn resize(&self, width: NonZeroU32, height: NonZeroU32) -> Res<()> {
        self.window.resize(width.get() as i32, height.get() as i32, 0, 0);
        // (a new size: no old frame to keep)
        self.history.borrow_mut().clear();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_redraws_what_changed_since_its_buffer() {
        let h: VecDeque<Option<[i32; 4]>> = [Some([0, 0, 10, 10]), Some([50, 50, 10, 10]), None].into_iter().collect();
        // (unknown age, or older than what's remembered: all)
        assert_eq!(clip_for(Some([5, 5, 5, 5]), &h, 0), None);
        assert_eq!(clip_for(Some([5, 5, 5, 5]), &h, 9), None);
        // the last frame is the buffer: only now
        assert_eq!(clip_for(Some([5, 5, 5, 5]), &h, 1), Some([5, 5, 5, 5]));
        // two frames back: now and the last
        assert_eq!(clip_for(Some([5, 5, 5, 5]), &h, 2), Some([0, 0, 10, 10]));
        assert_eq!(clip_for(Some([5, 5, 5, 5]), &h, 3), Some([0, 0, 60, 60]));
        // a frame that was all new in between: all
        assert_eq!(clip_for(Some([5, 5, 5, 5]), &h, 4), None);
        assert_eq!(clip_for(None, &h, 1), None);
    }
}
