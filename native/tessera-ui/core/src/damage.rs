//! What changed in a frame, so the compositor repaints only that.
//!
//! Slint's FemtoVG renderer has no partial rendering: it clears and draws
//! the whole surface every frame, and EGL then tells the compositor all of
//! it changed, so Hyprland repaints (and re-blurs, and the glass plugin
//! re-refracts) the whole area under a full-screen surface for a menu that
//! opens in one corner. The frame is still drawn whole here, but before it
//! is, Slint core's own dirty tracking (partial_renderer, the one its
//! software and Skia renderers use) walks the item tree with a renderer that
//! draws nothing and only reads what each item would draw with, so each
//! item's tracker learns its dependencies; the rectangles of the items whose
//! geometry or drawing changed since the last frame are the damage handed to
//! eglSwapBuffersWithDamage (egl.rs). Text inputs are always counted as
//! changed (their caret and preedit live outside their properties).
//! TESSERA_UI_FULL_DAMAGE=1 turns it off (every frame damages everything).

use i_slint_core::graphics::euclid;
use i_slint_core::item_rendering::{
    CachedRenderingData, ItemRenderer, ItemRendererFeatures, RenderBorderRectangle, RenderImage, RenderRectangle, RenderText,
};
use i_slint_core::items::{BoxShadow, ItemRc, Path, TextInput};
use i_slint_core::lengths::{ItemTransform, LogicalBorderRadius, LogicalRect, LogicalSize, LogicalVector, ScaleFactor};
use i_slint_core::partial_renderer::PartialRenderingState;
use i_slint_core::window::WindowInner;
use i_slint_core::Property;
use std::cell::Cell;
use std::pin::Pin;

/// One surface's tracking, kept from frame to frame
pub struct Damage {
    state: PartialRenderingState,
    /// bumped every frame: what reads it is changed every frame
    tick: Pin<Box<Property<u32>>>,
    /// the next frame is all new (the first, a new size or scale)
    all: Cell<bool>,
}

impl Default for Damage {
    fn default() -> Self {
        Self { state: Default::default(), tick: Box::pin(Property::new(0)), all: Cell::new(true) }
    }
}

impl Damage {
    /// The next frame damages everything (its size or scale changed)
    pub fn all(&self) {
        self.all.set(true);
        self.state.clear_cache();
    }

    /// The parts of the window (logical px) that differ from the last frame
    /// drawn; None when all of it does
    pub fn region(&self, window: &slint::Window, (w, h): (f32, f32)) -> Option<Vec<LogicalRect>> {
        let size = LogicalSize::new(w, h);
        let inner = WindowInner::from_pub(window);
        let tick = self.tick.as_ref();
        tick.set(tick.get_untracked().wrapping_add(1));
        let all = self.all.replace(false);
        if all {
            self.state.force_screen_refresh();
        }
        let probe = Probe { window: inner, tick, stack: vec![Frame { transform: ItemTransform::identity(), clip: LogicalRect::from_size(size) }] };
        let mut renderer = self.state.create_partial_renderer(probe);
        let adapter = inner.window_adapter();
        let region = inner.draw_contents(|components, _| {
            let r = self.state.apply_dirty_region(&mut renderer, components, size, None);
            // (each item's drawing is read once, so its tracker knows what it depends on)
            for (c, origin) in components {
                if let Some(c) = i_slint_core::item_tree::ItemTreeWeak::upgrade(c) {
                    i_slint_core::item_rendering::render_component_items(&c, &mut renderer, *origin, &adapter);
                }
            }
            r
        })?;
        if all {
            return None;
        }
        Some(region.iter().map(|b| b.to_rect()).collect())
    }
}

/// The surface's renderer: Slint's FemtoVG one, and the tracking above,
/// which hears when parts of the tree go away (a menu closing, rows
/// removed) so where they were is repainted too. Everything else is
/// FemtoVG's (the methods it has of its own are passed on; the rest of the
/// trait's are its defaults either way).
pub struct Renderer {
    pub femto: slint::platform::femtovg_renderer::FemtoVGRenderer,
    pub damage: Damage,
}

use i_slint_core::renderer::RendererSealed;

impl RendererSealed for Renderer {
    fn text_layout_cache(&self) -> Option<&i_slint_core::textlayout::sharedparley::TextLayoutCache> {
        RendererSealed::text_layout_cache(&self.femto)
    }
    fn set_rendering_notifier(&self, callback: Box<dyn i_slint_core::api::RenderingNotifier>) -> Result<(), i_slint_core::api::SetRenderingNotifierError> {
        RendererSealed::set_rendering_notifier(&self.femto, callback)
    }
    fn free_graphics_resources(
        &self,
        component: i_slint_core::item_tree::ItemTreeRef,
        items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'_>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        self.damage.state.free_graphics_resources(component, items);
        RendererSealed::free_graphics_resources(&self.femto, component, &mut std::iter::empty())
    }
    fn mark_dirty_region(&self, region: i_slint_core::partial_renderer::DirtyRegion) {
        self.damage.state.mark_dirty_region(region);
    }
    fn set_window_adapter(&self, adapter: &std::rc::Rc<dyn i_slint_core::window::WindowAdapter>) {
        RendererSealed::set_window_adapter(&self.femto, adapter)
    }
    fn window_adapter(&self) -> Option<std::rc::Rc<dyn i_slint_core::window::WindowAdapter>> {
        RendererSealed::window_adapter(&self.femto)
    }
    fn resize(&self, size: slint::PhysicalSize) -> Result<(), slint::PlatformError> {
        RendererSealed::resize(&self.femto, size)
    }
    fn take_snapshot(&self) -> Result<slint::SharedPixelBuffer<slint::Rgba8Pixel>, slint::PlatformError> {
        RendererSealed::take_snapshot(&self.femto)
    }
    fn supports_transformations(&self) -> bool {
        RendererSealed::supports_transformations(&self.femto)
    }
}

/// Logical rectangles to buffer pixels (top-left origin), grown a little for
/// antialiased edges and clipped to the buffer
pub fn to_buffer(rects: &[LogicalRect], scale: f32, buffer: (i32, i32)) -> Vec<[i32; 4]> {
    rects
        .iter()
        .filter_map(|r| {
            let x0 = ((r.origin.x - 2.0) * scale).floor().max(0.0) as i32;
            let y0 = ((r.origin.y - 2.0) * scale).floor().max(0.0) as i32;
            let x1 = ((r.origin.x + r.size.width + 2.0) * scale).ceil().min(buffer.0 as f32) as i32;
            let y1 = ((r.origin.y + r.size.height + 2.0) * scale).ceil().min(buffer.1 as f32) as i32;
            (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
        })
        .collect()
}

#[derive(Clone, Copy)]
struct Frame {
    /// item coordinates to the window's
    transform: ItemTransform,
    /// the clip, in the window's coordinates (a bounding box)
    clip: LogicalRect,
}

/// A renderer that draws nothing: it reads each item's properties as the
/// real one would (so they're tracked) and keeps the transform and clip
/// (so the partial renderer knows where items are)
struct Probe<'a> {
    window: &'a WindowInner,
    tick: Pin<&'a Property<u32>>,
    stack: Vec<Frame>,
}

impl Probe<'_> {
    fn top(&mut self) -> &mut Frame {
        self.stack.last_mut().unwrap()
    }
}

impl ItemRendererFeatures for Probe<'_> {
    const SUPPORTS_TRANSFORMATIONS: bool = true;
}

impl ItemRenderer for Probe<'_> {
    fn draw_rectangle(&mut self, rect: Pin<&dyn RenderRectangle>, _: &ItemRc, _: LogicalSize, _: &CachedRenderingData) {
        let _ = rect.background();
    }
    fn draw_border_rectangle(&mut self, rect: Pin<&dyn RenderBorderRectangle>, _: &ItemRc, _: LogicalSize, _: &CachedRenderingData) {
        let _ = (rect.background(), rect.border_width(), rect.border_radius(), rect.border_color());
    }
    fn draw_window_background(&mut self, rect: Pin<&dyn RenderRectangle>, _: &ItemRc, _: LogicalSize, _: &CachedRenderingData) {
        let _ = rect.background();
    }
    fn draw_image(&mut self, image: Pin<&dyn RenderImage>, _: &ItemRc, _: LogicalSize, _: &CachedRenderingData) {
        let _ = (image.target_size(), image.source(), image.source_clip(), image.image_fit(), image.rendering(), image.colorize(), image.alignment(), image.tiling());
    }
    fn draw_text(&mut self, text: Pin<&dyn RenderText>, item: &ItemRc, _: LogicalSize, _: &CachedRenderingData) {
        let _ = (text.text(), text.font_request(item), text.max_lines(), text.stroke(), text.link_color());
        let _ = (text.target_size(), text.color(), text.alignment(), text.wrap(), text.overflow(), text.is_markdown());
    }
    fn draw_text_input(&mut self, input: Pin<&TextInput>, _: &ItemRc, _: LogicalSize) {
        // (the caret blinks and the preedit shows from state outside the item)
        let _ = (self.tick.get(), input.text(), input.color());
    }
    fn draw_path(&mut self, path: Pin<&Path>, item: &ItemRc, _: LogicalSize) {
        let _ = (path.fitted_path_events(item).is_some(), path.fill(), path.stroke(), path.stroke_width());
    }
    fn draw_box_shadow(&mut self, shadow: Pin<&BoxShadow>, _: &ItemRc, _: LogicalSize) {
        let _ = (shadow.color(), shadow.blur(), shadow.offset_x(), shadow.offset_y(), shadow.spread(), shadow.inset(), shadow.logical_border_radius());
    }
    fn combine_clip(&mut self, rect: LogicalRect, _: LogicalBorderRadius) -> bool {
        let f = self.top();
        let on_screen = f.transform.outer_transformed_rect(&rect.cast::<f32>()).cast();
        f.clip = f.clip.intersection(&on_screen).unwrap_or_default();
        !f.clip.is_empty()
    }
    fn get_current_clip(&self) -> LogicalRect {
        let f = self.stack.last().unwrap();
        match f.transform.inverse() {
            Some(back) => back.outer_transformed_rect(&f.clip.cast::<f32>()).cast(),
            None => LogicalRect::default(),
        }
    }
    fn translate(&mut self, d: LogicalVector) {
        let f = self.top();
        f.transform = ItemTransform::translation(d.x as f32, d.y as f32).then(&f.transform);
    }
    fn current_transform(&self) -> ItemTransform {
        self.stack.last().unwrap().transform
    }
    fn rotate(&mut self, degrees: f32) {
        let f = self.top();
        f.transform = ItemTransform::rotation(euclid::Angle::degrees(degrees)).then(&f.transform);
    }
    fn scale(&mut self, x: f32, y: f32) {
        let f = self.top();
        f.transform = ItemTransform::scale(x, y).then(&f.transform);
    }
    fn apply_opacity(&mut self, _: f32) {}
    fn save_state(&mut self) {
        let f = *self.stack.last().unwrap();
        self.stack.push(f);
    }
    fn restore_state(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
        }
    }
    fn scale_factor(&self) -> ScaleFactor {
        ScaleFactor::new(self.window.scale_factor())
    }
    fn draw_cached_pixmap(&mut self, _: &ItemRc, _: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8]))) {}
    fn draw_string(&mut self, _: &str, _: slint::Color) {}
    fn draw_image_direct(&mut self, _: i_slint_core::graphics::Image) {}
    fn window(&self) -> &WindowInner {
        self.window
    }
    fn as_any(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_core::lengths::LogicalPoint;

    #[test]
    fn rectangles_reach_the_buffer_whole_and_inside() {
        let r = |x: f32, y: f32, w: f32, h: f32| LogicalRect::new(LogicalPoint::new(x, y), LogicalSize::new(w, h));
        // grown 2 px each side, at scale 1.5, clipped to the buffer
        assert_eq!(to_buffer(&[r(10.0, 20.0, 100.0, 40.0)], 1.5, (2880, 1620)), vec![[12, 27, 156, 66]]);
        assert_eq!(to_buffer(&[r(0.0, 0.0, 1920.0, 1080.0)], 1.0, (1920, 1080)), vec![[0, 0, 1920, 1080]]);
        // (nothing left after clipping: no rectangle)
        assert!(to_buffer(&[r(3000.0, 0.0, 10.0, 10.0)], 1.0, (1920, 1080)).is_empty());
        // EGL counts from the bottom; an empty damage is one pixel, never "all"
        assert_eq!(crate::egl::egl_rects(&[[12, 27, 156, 66]], 1620), vec![12, 1620 - 93, 156, 66]);
        assert_eq!(crate::egl::egl_rects(&[], 1080), vec![0, 1079, 1, 1]);
    }
}
