//! `--features skia`: Slint's Skia renderer instead of FemtoVG, on the same
//! EGL as everything else (egl.rs). Skia rasterises text as Qt and browsers
//! do (crisper and lighter than FemtoVG's glyph atlas), the QML shell's look.
//!
//! Slint's own OpenGL surface for Skia goes through glutin; this one wraps
//! our GlSurface instead (the Wayland surface, wl_surface.frame pacing and
//! the damage the compositor hears stay as they are). Each frame is drawn
//! whole (no FemtoVG screen clip here); the compositor is still told only
//! what changed (swap with damage), so its repaint stays small.

use crate::egl::GlSurface;
use i_slint_core::api::{PhysicalSize as PhysicalWindowSize, Window};
use i_slint_core::graphics::RequestedGraphicsAPI;
use i_slint_core::partial_renderer::DirtyRegion;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;
use i_slint_renderer_skia::skia_safe;
use slint::platform::femtovg_renderer::OpenGLInterface;
use std::cell::RefCell;
use std::num::NonZeroU32;
use std::sync::Arc;

pub use i_slint_renderer_skia::{SkiaRenderer, SkiaSharedContext};

pub struct SkiaGl {
    gl: GlSurface,
    gr: RefCell<skia_safe::gpu::DirectContext>,
    target: RefCell<Option<skia_safe::Surface>>,
}

impl SkiaGl {
    pub fn new(gl: GlSurface) -> Result<Self, PlatformError> {
        gl.ensure_current().map_err(|e| format!("Skia: EGL make current: {e}"))?;
        let interface = skia_safe::gpu::gl::Interface::new_load_with_cstr(|name| gl.get_proc_address(name))
            .ok_or("Skia: no OpenGL interface")?;
        let gr = skia_safe::gpu::direct_contexts::make_gl(interface, None).ok_or("Skia: no direct context")?;
        Ok(Self { gl, gr: RefCell::new(gr), target: RefCell::new(None) })
    }

    /// The window's framebuffer (0) as a Skia surface of this size
    fn target_for(&self, w: i32, h: i32) -> Result<skia_safe::Surface, PlatformError> {
        let fb = skia_safe::gpu::gl::FramebufferInfo { fboid: 0, format: skia_safe::gpu::gl::Format::RGBA8.into(), ..Default::default() };
        let rt = skia_safe::gpu::backend_render_targets::make_gl((w, h), Some(0), 8, fb);
        skia_safe::gpu::surfaces::wrap_backend_render_target(
            &mut self.gr.borrow_mut(),
            &rt,
            skia_safe::gpu::SurfaceOrigin::BottomLeft,
            skia_safe::ColorType::RGBA8888,
            skia_safe::ColorSpace::new_srgb(),
            None,
        )
        .ok_or_else(|| "Skia: couldn't wrap the window's framebuffer".into())
    }
}

impl i_slint_renderer_skia::Surface for SkiaGl {
    fn new(
        _: &SkiaSharedContext,
        _: Arc<dyn raw_window_handle::HasWindowHandle + Sync + Send>,
        _: Arc<dyn raw_window_handle::HasDisplayHandle + Sync + Send>,
        _: PhysicalWindowSize,
        _: Option<RequestedGraphicsAPI>,
    ) -> Result<Self, PlatformError> {
        Err("Skia: tessera's surfaces are made by core (SkiaGl::new)".into())
    }

    fn name(&self) -> &'static str {
        "tessera-egl"
    }

    fn with_active_surface(&self, callback: &mut dyn FnMut()) -> Result<(), PlatformError> {
        self.gl.ensure_current().map_err(|e| format!("Skia: EGL make current: {e}"))?;
        callback();
        Ok(())
    }

    fn render(
        &self,
        _window: &Window,
        size: PhysicalWindowSize,
        callback: &dyn Fn(&skia_safe::Canvas, Option<&mut skia_safe::gpu::DirectContext>, u8) -> Option<DirtyRegion>,
        pre_present_callback: &RefCell<Option<Box<dyn FnMut()>>>,
    ) -> Result<DrawOutcome, PlatformError> {
        self.gl.ensure_current().map_err(|e| format!("Skia: EGL make current: {e}"))?;
        // (every window's Skia context shares the one EGL context: another
        // window's frame changed the GL state this one cached, so it's
        // forgotten before drawing, or glyphs and blending come out wrong)
        self.gr.borrow_mut().reset(None);
        let (w, h) = (size.width.max(1) as i32, size.height.max(1) as i32);
        let stale = self.target.borrow().as_ref().map_or(true, |s| s.width() != w || s.height() != h);
        if stale {
            *self.target.borrow_mut() = Some(self.target_for(w, h)?);
        }
        {
            let mut target = self.target.borrow_mut();
            let surface = target.as_mut().unwrap();
            let canvas = surface.canvas();
            canvas.save();
            // (age 0: the whole frame is drawn)
            callback(canvas, Some(&mut self.gr.borrow_mut()), 0);
            canvas.restore();
        }
        self.gr.borrow_mut().flush_and_submit();
        if let Some(cb) = pre_present_callback.borrow_mut().as_mut() {
            cb();
        }
        self.gl.swap_buffers().map_err(|e| format!("Skia: swap: {e}"))?;
        Ok(DrawOutcome::Success)
    }

    fn resize_event(&self, size: PhysicalWindowSize) -> Result<(), PlatformError> {
        if let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
            self.gl.resize(w, h).map_err(|e| format!("Skia: resize: {e}"))?;
        }
        *self.target.borrow_mut() = None;
        Ok(())
    }

    fn bits_per_pixel(&self) -> Result<u8, PlatformError> {
        Ok(32)
    }
}
