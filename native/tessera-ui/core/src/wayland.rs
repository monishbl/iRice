//! The Wayland side: Slint has no layer-shell backend, so this is a small
//! Slint platform of our own. smithay-client-toolkit (SCTK) speaks
//! wlr-layer-shell, the seat (pointer, keyboard with xkb and repeat), cursor
//! shapes, fractional scaling (wp_fractional_scale + wp_viewporter) and
//! text-input-v3; each layer surface is a Slint `WindowAdapter` drawn by
//! Slint's FemtoVG renderer through EGL (egl.rs). Frames are paced by
//! wl_surface.frame callbacks: a surface draws only when Slint asked for a
//! redraw and the compositor has shown the previous frame.

use crate::egl::{Egl, GlSurface};
use crate::Msg;
use slint::platform::femtovg_renderer::FemtoVGRenderer;
use slint::platform::{Key, PointerEventButton, WindowAdapter, WindowEvent};
use slint::{LogicalPosition, LogicalSize, PhysicalSize};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{channel::Sender, LoopHandle},
        client::{
            globals::GlobalList,
            protocol::{wl_keyboard::WlKeyboard, wl_output::WlOutput, wl_pointer::WlPointer, wl_seat::WlSeat, wl_surface::WlSurface},
            Connection, Dispatch, Proxy as WlProxy, QueueHandle,
        },
        protocols::wp::{
            cursor_shape::v1::client::wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1},
            fractional_scale::v1::client::{
                wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
                wp_fractional_scale_v1::{self, WpFractionalScaleV1},
            },
            text_input::zv3::client::{
                zwp_text_input_manager_v3::ZwpTextInputManagerV3,
                zwp_text_input_v3::{self, ContentHint, ContentPurpose, ZwpTextInputV3},
            },
            viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter},
            presentation_time::client::{
                wp_presentation::{self, WpPresentation},
                wp_presentation_feedback::{self, WpPresentationFeedback},
            },
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{cursor_shape::CursorShapeManager, PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
        WaylandSurface,
    },
};
use smithay_client_toolkit::shell::xdg::{
    window::{Window as XdgWindow, WindowConfigure, WindowDecorations, WindowHandler},
    XdgShell,
};
use smithay_client_toolkit::{delegate_xdg_shell, delegate_xdg_window};
// (the lock's: ext-session-lock-v1, and an idle inhibitor while Bedside Mode shows)
use smithay_client_toolkit::delegate_session_lock;
use smithay_client_toolkit::reexports::protocols::wp::idle_inhibit::zv1::client::{zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1, zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1};
// (idle notifications, core/src/idle.rs: ext-idle-notify-v1)
use smithay_client_toolkit::reexports::protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::{self, ExtIdleNotificationV1},
    ext_idle_notifier_v1::ExtIdleNotifierV1,
};
use smithay_client_toolkit::session_lock::{SessionLock, SessionLockHandler, SessionLockState, SessionLockSurface, SessionLockSurfaceConfigure};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Instant;

/// How long a frame callback may take before it counts as lost (a frame at
/// 60 Hz is 17 ms; a busy compositor takes a few). Hyprland withholds the
/// callback while nobody can see the surface: its output off, the session
/// locked (every layer surface but the lock's), under a full-screen window.
/// The real logs showed what drawing on every 250 ms meant there: the
/// laptop's bar lost 12,155 callbacks in a session, 1-2 a second for two
/// hours with the screen off, each one a frame drawn for nothing; server1's
/// lock screen did the same while locked with its screen off. So a surface
/// that keeps losing them waits twice as long each time, up to 16 times this
/// (4 s): when it's seen again the compositor sends the callback it kept,
/// and it draws at once.
const LOST_FRAME: std::time::Duration = std::time::Duration::from_millis(250);

// ── One layer surface = one Slint window ────────────────────────────────────

pub struct Surface {
    window: slint::Window,
    /// FemtoVG, and what changed in each frame (src/damage.rs)
    renderer: crate::damage::Renderer,
    /// where render() leaves that for the EGL swap
    damage: Rc<RefCell<Option<(Vec<[i32; 4]>, i32)>>>,
    /// a frame begins: egl.rs clips its drawing to what changed (GlSurface::frame)
    gl_frame: Rc<Cell<bool>>,
    egl: Rc<Egl>,
    /// frames drawn, and of those the ones the compositor has shown (one
    /// is in flight at most: frame n is on screen once `presented` >= n)
    pub rendered: Cell<u32>,
    /// How many times the pointer has entered it (a part that never redraws,
    /// as a hot corner, tells a new visit from this, even when it left and
    /// came back between two turns of the loop)
    pub enters: Cell<u32>,
    pub presented: Cell<u32>,
    /// frames drawn since `show` (0 after `hide`), and whether one was drawn
    /// in this turn of the loop: the glass plugin's shapes for this surface
    /// wait for them (State::flush_glass), so glass never shows over a frame
    /// that isn't on screen yet, or over an empty first frame
    shown_frames: Cell<u32>,
    drew: Cell<bool>,
    pub wl: WlSurface,
    /// what it is to the compositor (`Role`): `layer` derefs to the layer
    /// surface, as it always did, for every part's layer surfaces
    pub layer: Role,
    viewport: Option<WpViewport>,
    _fractional: Option<WpFractionalScaleV1>,
    pub namespace: String,
    pub output: String, // the output's name (Hyprland's monitor name)
    logical: Cell<(u32, u32)>,
    scale: Cell<f32>,
    configured: Cell<bool>,
    needs_redraw: Cell<bool>,
    frame_pending: Cell<bool>,
    /// when the pending frame's callback was asked for: one that never comes
    /// (the compositor can withhold it) would freeze the surface for good,
    /// and with it the glass shapes waiting for that frame (`recover_lost_frames`)
    frame_asked: Cell<Option<Instant>>,
    /// frame callbacks lost in a row (LOST_FRAME's back-off)
    lost_streak: Cell<u32>,
    /// battery saver held its last frame back (at most 60 a second): the
    /// one drawn when it may is still an animation's next frame
    held: Cell<bool>,
    /// the drawing turn (RENDER_TURN) before its last frame callback came: a
    /// frame drawn in the turn right after one is an animation's next frame
    /// (pacing.rs)
    last_done: Cell<Option<u64>>,
    /// when its last animation frame reached the screen (wp_presentation, ns
    /// on the compositor's clock; 0: none yet)
    last_present_ns: Cell<u64>,
    /// the last frame that took the processor long (its `rendered` number)
    cpu_slow: Cell<Option<u32>>,
    pub mapped: Cell<bool>,
    /// hidden by `hide` until `show`: a configure meanwhile (Hyprland answers
    /// the null buffer with one) must not draw and map it again
    hidden: Cell<bool>,
    /// its role destroyed by drop_surface: never shown again
    retired: Cell<bool>,
    ime: RefCell<Option<slint_ime::Props>>, // what the focused text field wants from the input method
    pub stats: RefCell<crate::stats::Frames>,
    /// the pointer's shape Slint asked for over this surface (a TouchArea's `mouse-cursor`)
    pub cursor: Cell<Shape>,
    /// the part that made it: its input runs as that part's code, and it
    /// goes with that part if it panics (contain.rs)
    pub owner: &'static str,
}

/// What a surface is to the compositor: a layer surface (every part's), a
/// session-lock surface (the lock's, ext-session-lock-v1: `prepare_lock_surface`)
/// or a regular window (an xdg toplevel: the App Store's, `prepare_window`).
/// `layer` derefs to the layer surface as it always did; only the lock part
/// makes lock surfaces and only `prepare_window` windows, and neither calls
/// layer-surface methods on them (that panics).
pub enum Role {
    Layer(LayerSurface),
    Lock(SessionLockSurface),
    Window(XdgWindow),
    /// a popup (xdg_popup) on a window or a layer surface: menus that may
    /// leave their window, as the QML's popups do (prepare_popup)
    Popup(smithay_client_toolkit::shell::xdg::popup::Popup),
}

impl std::ops::Deref for Role {
    type Target = LayerSurface;
    fn deref(&self) -> &LayerSurface {
        match self {
            Role::Layer(l) => l,
            Role::Lock(_) => panic!("tessera-ui: a session-lock surface has no layer surface"),
            Role::Window(_) => panic!("tessera-ui: a window has no layer surface"),
            Role::Popup(_) => panic!("tessera-ui: a popup has no layer surface"),
        }
    }
}

impl PartialEq<LayerSurface> for Role {
    fn eq(&self, other: &LayerSurface) -> bool {
        matches!(self, Role::Layer(l) if l == other)
    }
}

impl Role {
    /// The regular window, for a surface `prepare_window` made
    pub fn window(&self) -> Option<&XdgWindow> {
        match self {
            Role::Window(w) => Some(w),
            _ => None,
        }
    }
    /// The popup, for a surface `prepare_popup` made
    pub fn popup(&self) -> Option<&smithay_client_toolkit::shell::xdg::popup::Popup> {
        match self {
            Role::Popup(p) => Some(p),
            _ => None,
        }
    }
}

/// The session lock's news, for the lock part (its `message`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LockEvent {
    /// the compositor locked the session: every output shows only lock surfaces
    Locked,
    /// refused, or taken away (another lock client holds it)
    Finished,
}

/// The input-method request a TextInput made (kept for text-input-v3)
pub mod slint_ime {
    #[derive(Clone)]
    pub struct Props {
        pub x: i32,
        pub y: i32,
        pub w: i32,
        pub h: i32,
        pub password: bool,
    }
}

impl Surface {
    pub fn window(&self) -> &slint::Window {
        &self.window
    }
    /// A frame is on its way to the screen: its frame callback will wake the loop
    pub fn frame_pending(&self) -> bool {
        self.frame_pending.get()
    }
    pub fn logical_size(&self) -> (u32, u32) {
        self.logical.get()
    }
    /// An event for its window: Slint runs the part's callbacks inside, so
    /// as that part's code (contain.rs: a panic there is the part's)
    pub fn dispatch(&self, e: WindowEvent) {
        crate::contain::guard(self.owner, || self.window.dispatch_event(e));
    }
    fn physical(&self) -> PhysicalSize {
        let (w, h) = self.logical.get();
        // (rounded, as wp_fractional_scale asks: Slint's to_physical cuts, and
        // a buffer a pixel short of the viewport is stretched into it, blurring
        // the whole surface at 1.25 or 1.75)
        let s = self.scale.get();
        PhysicalSize::new((w as f32 * s).round() as u32, (h as f32 * s).round() as u32)
    }
    /// The compositor's size and scale reached us: tell Slint
    fn apply_geometry(&self) {
        let (w, h) = self.logical.get();
        if let Some(vp) = &self.viewport {
            vp.set_destination(w as i32, h as i32);
        } else {
            self.wl.set_buffer_scale(self.scale.get().round().max(1.0) as i32);
        }
        self.dispatch(WindowEvent::ScaleFactorChanged { scale_factor: self.scale.get() });
        self.dispatch(WindowEvent::Resized { size: LogicalSize::new(w as f32, h as f32) });
        self.needs_redraw.set(true);
        self.renderer.damage.all();
    }
    /// Battery saver: when this surface may draw again, if not yet (at most
    /// 60 frames a second, saver.rs; a little early is fine: the frame waits
    /// for the display's next refresh anyway)
    fn held_until(&self) -> Option<Instant> {
        let gap = crate::saver::min_frame()?;
        let at = self.frame_asked.get()? + gap - gap / 8;
        (Instant::now() < at).then_some(at)
    }
    fn render(&self, qh: &QueueHandle<State>, presentation: Option<&WpPresentation>) {
        if !self.configured.get() || self.frame_pending.get() || !self.needs_redraw.get() {
            return;
        }
        if self.held_until().is_some() {
            self.held.set(true);
            return;
        }
        self.needs_redraw.set(false);
        self.wl.frame(qh, self.wl.clone());
        let t = Instant::now();
        // (drawn as soon as its frame callback let it: the next frame of an
        // animation, timed on screen by wp_presentation; pacing.rs)
        let refresh = crate::pacing::refresh_of(&self.output);
        let chained = self.last_done.get().is_some_and(|d| d + 1 == RENDER_TURN.get()) | self.held.replace(false);
        let cpu = thread_cpu();
        let feedback = presentation.filter(|_| chained && (wakes_on() || crate::gpu::watching())).map(|p| p.feedback(&self.wl, qh, Fb { surface: WlProxy::id(&self.wl), frame: self.rendered.get() }));
        // (the first two frames after `show` are drawn whole: the first can
        // come out empty, and the second, drawn to fix it, found "nothing
        // changed" and fixed nothing: a frosted panel left empty until
        // something in it moved)
        if self.shown_frames.get() < 2 {
            self.renderer.damage.all();
        }
        // What changed since the last frame: the compositor repaints only that
        if self.egl.has_damage() {
            let (w, h) = self.logical.get();
            let px = self.physical();
            let tp = Instant::now();
            let region = crate::perf::time(|| format!("damage probe {}", self.namespace), || self.renderer.damage.region(&self.window, (w as f32, h as f32)));
            PROBE_MS.with(|c| c.set(tp.elapsed().as_secs_f32() * 1000.0));
            if crate::env::is_set("DAMAGE_LOG") {
                eprintln!("damage {} {:?}", self.namespace, region.as_ref().map(|r| r.iter().map(|x| [x.origin.x as i32, x.origin.y as i32, x.size.width as i32, x.size.height as i32]).collect::<Vec<_>>()));
            }
            *self.damage.borrow_mut() = region.map(|r| (crate::damage::to_buffer(&r, self.scale.get(), (px.width as i32, px.height as i32)), px.height as i32));
            // (and the frame draws only that, with the back buffer's age: egl.rs)
            self.gl_frame.set(true);
        }
        if let Err(e) = crate::perf::time(|| format!("femtovg draw {}", self.namespace), || self.renderer.draw()) {
            eprintln!("tessera-ui: render failed: {e}");
        }
        // (a frame that skipped items left the window tracking only the drawn
        // ones: every item is read again, so a change anywhere asks for a frame;
        // damage.rs track_all)
        #[cfg(not(feature = "skia"))]
        if self.egl.has_damage() && i_slint_renderer_femtovg::last_frame().culled > 0 {
            let (w, h) = self.logical.get();
            crate::perf::time(|| format!("damage track {}", self.namespace), || self.renderer.damage.track_all(&self.window, (w as f32, h as f32)));
        }
        let took = t.elapsed();
        // (TESSERA_DRAW_STATS=1: where each frame's time went, one line a frame)
        if draw_stats() {
            let f = i_slint_renderer_femtovg::last_frame();
            eprintln!(
                "draw-stats {}{}{} {:.2} ms: probe {:.2} current {:.2} age-wait {:.2} items {:.2} flush {:.2} present {:.2}; drawn {} culled {} layers {}+{}p new {} {} age {}",
                self.namespace,
                if self.output.is_empty() { "" } else { "@" },
                self.output,
                took.as_secs_f64() * 1000.0,
                PROBE_MS.with(|c| c.replace(0.0)),
                crate::egl::last_begin_ms()[0],
                crate::egl::last_begin_ms()[1],
                f.items_ms,
                f.flush_ms,
                f.present_ms,
                f.drawn,
                f.culled,
                f.layers_full,
                f.layers_partial,
                f.layers_new,
                if f.partial { "partial" } else { "whole" },
                crate::egl::last_age()
            );
        }
        self.stats.borrow_mut().rendered(took);
        framelog_on("frame", &self.namespace, &self.output);
        if wakes_on() {
            drew_after_press(&self.namespace, took.as_secs_f64() * 1000.0);
        }
        if chained {
            crate::pacing::animating();
        }
        if feedback.is_none() {
            // (a lone frame: the next one's interval doesn't start from an old one)
            self.last_present_ns.set(0);
        } else if thread_cpu().saturating_sub(cpu) >= refresh / 2 {
            // (a frame that kept the processor more than half a frame: if it's
            // late, that isn't the GPU's doing, so the slow-GPU watch skips it)
            self.cpu_slow.set(Some(self.rendered.get()));
        }
        self.rendered.set(self.rendered.get().wrapping_add(1));
        self.shown_frames.set(self.shown_frames.get().saturating_add(1));
        self.drew.set(true);
        self.frame_pending.set(true);
        self.frame_asked.set(Some(Instant::now()));
        self.mapped.set(true);
        // (a window's first frame can come out unpainted, garbage or nothing;
        // one that never animates would keep it: so the first after each
        // `show` is drawn twice)
        if self.shown_frames.get() == 1 {
            // (mapped: a layer taking the keyboard moves Hyprland's focused monitor)
            note_layer_change();
            self.needs_redraw.set(true);
            if wakes_on() {
                // (open timing: the first frame after a show, by the wall clock)
                eprintln!("first-frame {} at {} drew {:.1} ms", self.namespace, crate::util::now_ms(), t.elapsed().as_secs_f64() * 1000.0);
            }
        }
        // (no "redraw while animations run" here: Slint's animation clock is
        // global, so every window would redraw while any one animates; an
        // animated property marks only its own window dirty)
    }
    /// The next frame draws everything (its damage can't be trusted: the
    /// look or the glass changed under it; egl.rs then doesn't clip it)
    pub fn redraw_all(&self) {
        self.renderer.damage.all();
        self.needs_redraw.set(true);
    }
    /// When its pending frame callback counts as lost (LOST_FRAME, doubled
    /// for each one lost in a row before it)
    fn lost_frame_at(&self) -> Option<Instant> {
        self.frame_asked.get().map(|t| t + LOST_FRAME * (1u32 << self.lost_streak.get().min(4)))
    }
    /// Unmap (a null buffer); `show` asks for a new configure
    pub fn hide(&self) {
        // (a session-lock surface must never get a null buffer: it's a protocol
        // error, and the compositor drops the whole connection, every surface
        // with it. It stays as drawn until the lock lets it go; only the
        // bookkeeping below changes)
        if !matches!(self.layer, Role::Lock(_)) {
            self.wl.attach(None, 0, 0);
            self.wl.commit();
        } else if crate::env::is_set("DEBUG") {
            eprintln!("debug: hide on a lock surface: kept its buffer");
        }
        framelog_on("hide", &self.namespace, &self.output);
        if self.mapped.get() {
            note_layer_change();
        }
        self.configured.set(false);
        self.frame_pending.set(false);
        self.mapped.set(false);
        self.shown_frames.set(0);
        self.hidden.set(true);
        self.presented.set(self.rendered.get()); // (a frame in flight won't be shown now)
    }
    /// Destroy the compositor's role object (layer surface or window) ahead
    /// of the Rust value: unmapped, its exclusive zone and layer entry gone
    fn retire_role(&self) {
        self.retired.set(true);
        match &self.layer {
            Role::Layer(l) => {
                #[allow(irrefutable_let_patterns)]
                if let smithay_client_toolkit::shell::wlr_layer::SurfaceKind::Wlr(wlr) = l.kind() {
                    wlr.destroy();
                }
            }
            Role::Window(win) => {
                use smithay_client_toolkit::shell::xdg::XdgSurface as _;
                win.xdg_toplevel().destroy();
                win.xdg_surface().destroy();
            }
            Role::Popup(p) => {
                p.xdg_popup().destroy();
                p.xdg_surface().destroy();
            }
            // (the lock part ends its lock itself)
            Role::Lock(_) => {}
        }
        self.wl.commit();
    }
    pub fn show(&self, w: u32, h: u32) {
        if self.retired.get() {
            return;
        }
        self.hidden.set(false);
        // (a window opening — a shortcut or the shell asked, not input to us —
        // is activity too: its timers keep their time, off the idle grid)
        crate::pacing::input();
        if wakes_on() {
            eprintln!("show {} at {}", self.namespace, crate::util::now_ms());
        }
        match &self.layer {
            Role::Layer(l) => {
                l.set_size(w, h);
                l.commit();
            }
            Role::Window(win) => win.commit(),
            // (a lock surface is sized by the compositor: only shown again)
            Role::Lock(_) => self.wl.commit(),
            // (a popup is placed by the compositor: a dismissed one is made anew)
            Role::Popup(_) => self.wl.commit(),
        }
    }
}

impl WindowAdapter for Surface {
    fn window(&self) -> &slint::Window {
        &self.window
    }
    fn size(&self) -> PhysicalSize {
        self.physical()
    }
    fn renderer(&self) -> &dyn slint::platform::Renderer {
        &self.renderer
    }
    fn request_redraw(&self) {
        // (the wake log: a window that isn't on screen asking to draw, its
        // animations or timers ticking for nothing)
        if !self.needs_redraw.replace(true) && (self.hidden.get() || !self.configured.get()) && wakes_on() {
            HIDDEN_ASKS.with(|h| *h.borrow_mut().entry(self.namespace.clone()).or_default() += 1);
        }
    }
    fn internal(&self, _: i_slint_core::InternalToken) -> Option<&dyn i_slint_core::window::WindowAdapterInternal> {
        Some(self)
    }
}

impl i_slint_core::window::WindowAdapterInternal for Surface {
    fn input_method_request(&self, req: i_slint_core::window::InputMethodRequest) {
        use i_slint_core::window::InputMethodRequest as R;
        let props = match req {
            R::Enable(p) | R::Update(p) => p,
            R::Disable => {
                self.ime.replace(None);
                IME_DIRTY.set(true);
                return;
            }
            _ => return,
        };
        let o = props.cursor_rect_origin;
        let s = props.cursor_rect_size;
        self.ime.replace(Some(slint_ime::Props {
            x: o.x as i32,
            y: o.y as i32,
            w: s.width.max(1.0) as i32,
            h: s.height.max(1.0) as i32,
            password: props.input_type == i_slint_core::items::InputType::Password,
        }));
        IME_DIRTY.set(true);
    }
    fn handle_focus_change(&self, _old: Option<i_slint_core::items::ItemRc>, _new: Option<i_slint_core::items::ItemRc>) {
        crate::a11y::FOCUS_DIRTY.set(true);
    }
    // (a TouchArea's mouse-cursor: applied after the pointer event that changed it)
    fn set_mouse_cursor(&self, cursor: i_slint_core::cursor::MouseCursorInner) {
        let shape = match cursor {
            i_slint_core::cursor::MouseCursorInner::BuiltIn(c) => cursor_shape(c),
            _ => Shape::Default,
        };
        if self.cursor.replace(shape) != shape {
            CURSOR_DIRTY.set(true);
        }
    }
}

/// Slint's cursor as cursor-shape-v1's
fn cursor_shape(c: i_slint_core::items::BuiltInMouseCursor) -> Shape {
    use i_slint_core::items::BuiltInMouseCursor as C;
    match c {
        C::Pointer => Shape::Pointer,
        C::Crosshair => Shape::Crosshair,
        C::Text => Shape::Text,
        C::Move => Shape::Move,
        C::Grab => Shape::Grab,
        C::Grabbing => Shape::Grabbing,
        C::NwseResize => Shape::NwseResize,
        C::NeswResize => Shape::NeswResize,
        C::EwResize => Shape::EwResize,
        C::NsResize => Shape::NsResize,
        C::NotAllowed => Shape::NotAllowed,
        C::Wait => Shape::Wait,
        C::Progress => Shape::Progress,
        C::Help => Shape::Help,
        _ => Shape::Default,
    }
}

thread_local! {
    // Component::new() asks the platform for a window adapter: this one
    static NEXT: RefCell<Option<Rc<Surface>>> = const { RefCell::new(None) };
    static IME_DIRTY: Cell<bool> = const { Cell::new(false) };
    // a surface asked for another pointer shape; the serial of the pointer's last enter
    static CURSOR_DIRTY: Cell<bool> = const { Cell::new(false) };
    static POINTER_SERIAL: Cell<u32> = const { Cell::new(0) };
    // the last button press or key press (a popup's grab names the input that opened it)
    static INPUT_SERIAL: Cell<u32> = const { Cell::new(0) };
    // render_all's turns, counted (a frame drawn in the turn after its
    // surface's frame callback came is an animation's next frame)
    static RENDER_TURN: Cell<u64> = const { Cell::new(0) };
    // the wake log's: hidden windows that asked to be drawn this second
    static HIDDEN_ASKS: RefCell<std::collections::BTreeMap<String, u32>> = const { RefCell::new(std::collections::BTreeMap::new()) };
}

/// This thread's processor time (not the time it waited, in the GPU driver
/// or elsewhere)
fn thread_cpu() -> std::time::Duration {
    let mut ts: libc::timespec = unsafe { std::mem::zeroed() };
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    std::time::Duration::new(ts.tv_sec.max(0) as u64, ts.tv_nsec.clamp(0, 999_999_999) as u32)
}

/// A presentation feedback's surface
pub struct Fb {
    surface: smithay_client_toolkit::reexports::client::backend::ObjectId,
    /// the surface's frame number (`rendered` as it was drawn)
    frame: u32,
}

// ── The Slint platform ──────────────────────────────────────────────────────

pub struct Platform {
    pub tx: Sender<Msg>,
}

struct LoopProxy(std::sync::Mutex<Sender<Msg>>);

impl slint::platform::EventLoopProxy for LoopProxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        self.0.lock().unwrap().send(Msg::Quit).map_err(|_| slint::EventLoopError::EventLoopTerminated)
    }
    fn invoke_from_event_loop(&self, f: Box<dyn FnOnce() + Send>) -> Result<(), slint::EventLoopError> {
        self.0.lock().unwrap().send(Msg::Run(f)).map_err(|_| slint::EventLoopError::EventLoopTerminated)
    }
}

impl slint::platform::Platform for Platform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        NEXT.with(|n| n.borrow_mut().take())
            .map(|s| s as Rc<dyn WindowAdapter>)
            .ok_or_else(|| "tessera-ui: a component was made without a layer surface".into())
    }
    // (Slint's clock, for its timers and animations: from pacing's epoch, so
    // the idle loop can lay Slint's timers on a grid of its own; pacing.rs)
    fn duration_since_start(&self) -> std::time::Duration {
        crate::pacing::epoch().elapsed()
    }
    fn new_event_loop_proxy(&self) -> Option<Box<dyn slint::platform::EventLoopProxy>> {
        Some(Box::new(LoopProxy(std::sync::Mutex::new(self.tx.clone()))))
    }
    // The clipboard, through wl-clipboard (as the QML shell copies): Ctrl+C,
    // Ctrl+X and Ctrl+V in text fields
    fn set_clipboard_text(&self, text: &str, clipboard: slint::platform::Clipboard) {
        if clipboard != slint::platform::Clipboard::DefaultClipboard {
            return;
        }
        let text = text.to_string();
        std::thread::spawn(move || {
            let _ = std::process::Command::new("wl-copy").args(["--", &text]).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        });
    }
    fn clipboard_text(&self, clipboard: slint::platform::Clipboard) -> Option<String> {
        if clipboard != slint::platform::Clipboard::DefaultClipboard {
            return None;
        }
        let out = std::process::Command::new("timeout").args(["2", "wl-paste", "--no-newline", "--type", "text"]).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

// ── State: everything the Wayland handlers touch ────────────────────────────

/// Where a popup goes (prepare_popup)
pub struct PopupSpec<'a> {
    pub namespace: &'a str,
    /// the rectangle it opens next to, in the parent's logical px: x, y, w, h
    pub anchor: (i32, i32, i32, i32),
    /// its size, logical px
    pub size: (u32, u32),
    /// moved this much from where it would go (logical px)
    pub offset: (i32, i32),
    /// above the anchor rather than below it
    pub above: bool,
    /// takes the keyboard; a click elsewhere dismisses it (menus)
    pub grab: bool,
}

pub struct Spec<'a> {
    pub layer: Layer,
    pub namespace: &'a str,
    pub output: Option<(&'a WlOutput, &'a str)>,
    pub anchor: Anchor,
    pub size: (u32, u32),
    pub exclusive: i32,
    pub keyboard: KeyboardInteractivity,
    pub margin_left: i32,
}

/// A regular window (an xdg toplevel): the App Store's
pub struct WindowSpec<'a> {
    pub title: &'a str,
    pub app_id: &'a str,
    pub namespace: &'a str,
    pub size: (u32, u32),
    pub min_size: (u32, u32),
}

pub struct State {
    registry: RegistryState,
    pub outputs: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    xdg_shell: Option<XdgShell>,
    seat: SeatState,
    cursor_shapes: Option<CursorShapeManager>,
    viewporter: Option<WpViewporter>,
    fractional: Option<WpFractionalScaleManagerV1>,
    text_inputs: Option<ZwpTextInputManagerV3>,
    text_input: Option<ZwpTextInputV3>,
    ti_on: Option<WlSurface>, // where text-input entered
    ti_enabled: bool,
    ti_pending: (Option<String>, Option<(String, i32, i32)>),
    pointer: Option<WlPointer>,
    cursor: Option<WpCursorShapeDeviceV1>,
    keyboard: Option<WlKeyboard>,
    pointer_on: Option<Rc<Surface>>,
    keyboard_on: Option<Rc<Surface>>,
    /// shift, control, alt, logo as Slint last heard them (key events or
    /// the seat's modifiers: a virtual keyboard sends only the latter)
    mods_sent: [bool; 4],
    pub surfaces: Vec<Rc<Surface>>,
    pub egl: Rc<Egl>,
    pub qh: QueueHandle<State>,
    pub loop_handle: LoopHandle<'static, State>,
    pub tx: Sender<Msg>,
    /// what every surface shares (look, settings, services' data, glass)
    pub shell: crate::shell::Shell,
    /// the surfaces drawn here (src/surfaces)
    pub parts: Vec<Box<dyn crate::shell::Part>>,
    pub started: Instant,
    pub first_frame_logged: bool,
    pub exit: bool,
    /// drag and drop (src/dnd.rs)
    pub dnd: crate::dnd::Dnd,
    /// ext-session-lock-v1 (the lock part's; unbound when the compositor has none)
    pub session_lock: SessionLockState,
    idle_inhibit: Option<ZwpIdleInhibitManagerV1>,
    /// wp_presentation: when frames really reached the screen (pacing.rs)
    presentation: Option<WpPresentation>,
    /// ext-idle-notify-v1 (None: the compositor has none), the watches asked
    /// for by id (idle.rs) and what each last said
    idle_notifier: Option<ExtIdleNotifierV1>,
    idle_watches: std::collections::HashMap<u32, (ExtIdleNotificationV1, u32, bool)>,
    idle_state: std::collections::HashMap<u32, bool>,
}

impl State {
    pub fn new(
        globals: &GlobalList,
        qh: QueueHandle<State>,
        egl: Rc<Egl>,
        loop_handle: LoopHandle<'static, State>,
        tx: Sender<Msg>,
        shell: crate::shell::Shell,
        parts: Vec<Box<dyn crate::shell::Part>>,
        started: Instant,
    ) -> Self {
        // (the lock's globals, bound before `qh` moves into the state)
        let session_lock = SessionLockState::new(globals, &qh);
        let idle_inhibit = globals.bind(&qh, 1..=1, ()).ok();
        let idle_notifier = globals.bind(&qh, 1..=2, ()).ok();
        let presentation = globals.bind(&qh, 1..=2, ()).ok();
        Self {
            registry: RegistryState::new(globals),
            outputs: OutputState::new(globals, &qh),
            compositor: CompositorState::bind(globals, &qh).expect("wl_compositor"),
            layer_shell: LayerShell::bind(globals, &qh).expect("this compositor has no wlr-layer-shell"),
            xdg_shell: XdgShell::bind(globals, &qh).ok(),
            dnd: crate::dnd::Dnd::new(globals, &qh),
            seat: SeatState::new(globals, &qh),
            cursor_shapes: CursorShapeManager::bind(globals, &qh).ok(),
            viewporter: globals.bind(&qh, 1..=1, ()).ok(),
            fractional: globals.bind(&qh, 1..=1, ()).ok(),
            text_inputs: globals.bind(&qh, 1..=1, ()).ok(),
            text_input: None,
            ti_on: None,
            ti_enabled: false,
            ti_pending: (None, None),
            pointer: None,
            cursor: None,
            keyboard: None,
            pointer_on: None,
            keyboard_on: None,
            mods_sent: [false; 4],
            surfaces: Vec::new(),
            egl,
            qh,
            loop_handle,
            tx,
            shell,
            parts,
            started,
            first_frame_logged: false,
            exit: false,
            session_lock,
            idle_inhibit,
            presentation,
            idle_notifier,
            idle_watches: Default::default(),
            idle_state: Default::default(),
        }
    }

    /// A layer surface ready to become the window of the next Slint component made
    pub fn prepare_surface(&mut self, spec: Spec) -> Rc<Surface> {
        let wl = self.compositor.create_surface(&self.qh);
        let layer = self.layer_shell.create_layer_surface(
            &self.qh,
            wl.clone(),
            spec.layer,
            Some(spec.namespace.to_string()),
            spec.output.map(|o| o.0),
        );
        layer.set_anchor(spec.anchor);
        layer.set_size(spec.size.0, spec.size.1);
        layer.set_exclusive_zone(spec.exclusive);
        layer.set_keyboard_interactivity(spec.keyboard);
        layer.set_margin(0, 0, 0, spec.margin_left);
        let viewport = self.viewporter.as_ref().map(|v| v.get_viewport(&wl, &self.qh, ()));
        let fractional = self.fractional.as_ref().map(|f| f.get_fractional_scale(&wl, &self.qh, wl.clone()));
        layer.commit();
        self.adopt(wl, Role::Layer(layer), viewport, fractional, spec.namespace, spec.output.map(|o| o.1.to_string()).unwrap_or_default(), spec.size)
    }

    /// A regular window (an xdg toplevel) ready to become the window of the
    /// next Slint component made; it maps on its first frame. The compositor
    /// asking it to close reaches `Part::surface_closed`; it's destroyed with
    /// its last reference (drop_surface, then the component)
    pub fn prepare_window(&mut self, spec: WindowSpec) -> Rc<Surface> {
        let wl = self.compositor.create_surface(&self.qh);
        let xdg = self.xdg_shell.as_ref().expect("this compositor has no xdg-shell");
        let win = xdg.create_window(wl.clone(), WindowDecorations::ServerDefault, &self.qh);
        win.set_title(spec.title);
        win.set_app_id(spec.app_id);
        if spec.min_size.0 > 0 && spec.min_size.1 > 0 {
            win.set_min_size(Some(spec.min_size));
        }
        let viewport = self.viewporter.as_ref().map(|v| v.get_viewport(&wl, &self.qh, ()));
        let fractional = self.fractional.as_ref().map(|f| f.get_fractional_scale(&wl, &self.qh, wl.clone()));
        win.commit();
        self.adopt(wl, Role::Window(win), viewport, fractional, spec.namespace, String::new(), spec.size)
    }

    /// A popup ready to become the window of the next Slint component made:
    /// placed by the compositor next to `spec.anchor` (a rectangle in the
    /// parent's logical px), below it, flipped above or slid sideways to stay
    /// on screen; it may leave its parent's bounds (Settings' menus, as the
    /// QML's popups). With `grab`, it takes the keyboard and the compositor
    /// dismisses it on a click elsewhere: `Part::surface_closed` then, as for a
    /// window the compositor closed. None if the parent can't have popups.
    pub fn prepare_popup(&mut self, parent: &Rc<Surface>, spec: PopupSpec) -> Option<Rc<Surface>> {
        use smithay_client_toolkit::reexports::protocols::xdg::shell::client::xdg_positioner::{Anchor as PAnchor, ConstraintAdjustment, Gravity};
        use smithay_client_toolkit::shell::xdg::{popup::Popup, XdgPositioner, XdgSurface as _};
        let xdg = self.xdg_shell.as_ref()?;
        let positioner = XdgPositioner::new(xdg).ok()?;
        let (w, h) = (spec.size.0.max(1) as i32, spec.size.1.max(1) as i32);
        positioner.set_size(w, h);
        let (ax, ay, aw, ah) = spec.anchor;
        positioner.set_anchor_rect(ax, ay, aw.max(1), ah.max(1));
        positioner.set_anchor(if spec.above { PAnchor::TopLeft } else { PAnchor::BottomLeft });
        positioner.set_gravity(if spec.above { Gravity::TopRight } else { Gravity::BottomRight });
        positioner.set_offset(spec.offset.0, spec.offset.1);
        positioner.set_constraint_adjustment(ConstraintAdjustment::FlipY | ConstraintAdjustment::SlideX | ConstraintAdjustment::SlideY);
        let wl = self.compositor.create_surface(&self.qh);
        let popup = match &parent.layer {
            Role::Window(win) => Popup::from_surface(Some(win.xdg_surface()), &positioner, &self.qh, wl.clone(), xdg).ok()?,
            Role::Popup(pp) => Popup::from_surface(Some(pp.xdg_surface()), &positioner, &self.qh, wl.clone(), xdg).ok()?,
            Role::Layer(l) => {
                let p = Popup::from_surface(None, &positioner, &self.qh, wl.clone(), xdg).ok()?;
                l.get_popup(p.xdg_popup());
                p
            }
            Role::Lock(_) => return None,
        };
        if spec.grab {
            if let Some(seat) = self.seat.seats().next() {
                popup.xdg_popup().grab(&seat, INPUT_SERIAL.get());
            }
        }
        let viewport = self.viewporter.as_ref().map(|v| v.get_viewport(&wl, &self.qh, ()));
        let fractional = self.fractional.as_ref().map(|f| f.get_fractional_scale(&wl, &self.qh, wl.clone()));
        wl.commit();
        let s = self.adopt(wl, Role::Popup(popup), viewport, fractional, spec.namespace, parent.output.clone(), spec.size);
        s.scale.set(parent.scale.get());
        Some(s)
    }

    /// A surface with its role: its EGL window and renderer, made the next component's window
    fn adopt(&mut self, wl: WlSurface, layer: Role, viewport: Option<WpViewport>, fractional: Option<WpFractionalScaleV1>, namespace: &str, output: String, size: (u32, u32)) -> Rc<Surface> {
        let gl = GlSurface::new(self.egl.clone(), WlProxy::id(&wl)).expect("EGL window surface");
        let damage = gl.damage.clone();
        let gl_frame = gl.frame.clone();
        // (FemtoVG loads GL as it's made: the context must be current)
        slint::platform::femtovg_renderer::OpenGLInterface::ensure_current(&gl).expect("EGL make current");
        let surface = Rc::new_cyclic(|weak: &Weak<Surface>| Surface {
            window: slint::Window::new(weak.clone() as Weak<dyn WindowAdapter>),
            renderer: crate::damage::Renderer { femto: make_renderer(gl), damage: Default::default() },
            damage,
            gl_frame,
            egl: self.egl.clone(),
            rendered: Cell::new(0),
            enters: Cell::new(0),
            presented: Cell::new(0),
            shown_frames: Cell::new(0),
            drew: Cell::new(false),
            wl,
            layer,
            viewport,
            _fractional: fractional,
            namespace: namespace.to_string(),
            output,
            logical: Cell::new(size),
            scale: Cell::new(1.0),
            configured: Cell::new(false),
            needs_redraw: Cell::new(true),
            frame_pending: Cell::new(false),
            frame_asked: Cell::new(None),
            lost_streak: Cell::new(0),
            held: Cell::new(false),
            last_done: Cell::new(None),
            last_present_ns: Cell::new(0),
            cpu_slow: Cell::new(None),
            mapped: Cell::new(false),
            hidden: Cell::new(false),
            retired: Cell::new(false),
            ime: RefCell::new(None),
            stats: RefCell::new(Default::default()),
            cursor: Cell::new(Shape::Default),
            owner: crate::contain::current(),
        });
        NEXT.with(|n| *n.borrow_mut() = Some(surface.clone()));
        self.surfaces.push(surface.clone());
        surface
    }

    /// The surface is done with: off the screen and out of the compositor's
    /// lists now, freed once the part lets go of it and its component.
    /// (Slint keeps a shown window's component alive, and the component
    /// holds its window adapter, this Surface: a cycle. Hiding the Slint
    /// window breaks it. The layer role is destroyed here too, so a
    /// reference still held elsewhere can't keep it mapped, keep its
    /// exclusive zone or stay in Hyprland's layer list; the role's own drop
    /// later sends a second destroy, which wayland-client ignores.)
    pub fn drop_surface(&mut self, s: &Rc<Surface>) {
        let _ = s.window.hide();
        s.hide();
        s.retire_role();
        self.surfaces.retain(|x| !Rc::ptr_eq(x, s));
        // (made but no component yet — a part that panicked in between,
        // contain.rs: the next component made mustn't take it)
        NEXT.with(|n| {
            let mut n = n.borrow_mut();
            if n.as_ref().is_some_and(|x| Rc::ptr_eq(x, s)) {
                *n = None;
            }
        });
        if self.pointer_on.as_ref().is_some_and(|p| Rc::ptr_eq(p, s)) {
            self.pointer_on = None;
        }
        if self.keyboard_on.as_ref().is_some_and(|p| Rc::ptr_eq(p, s)) {
            self.keyboard_on = None;
        }
    }

    /// Where a surface takes the pointer (QML's `mask`): these rectangles, in
    /// surface-local logical px; `Some(&[])` nowhere (clicks go through to
    /// what's below); `None` everywhere
    pub fn set_input_region(&self, s: &Surface, rects: Option<&[(i32, i32, i32, i32)]>) {
        match rects {
            None => s.wl.set_input_region(None),
            Some(rects) => {
                let Ok(r) = smithay_client_toolkit::compositor::Region::new(&self.compositor) else { return };
                for &(x, y, w, h) in rects {
                    if w > 0 && h > 0 {
                        r.add(x, y, w, h);
                    }
                }
                s.wl.set_input_region(Some(r.wl_region()));
            }
        }
        // (double-buffered: the next frame's commit carries it; a surface
        // already on screen and idle gets one now)
        if s.mapped.get() {
            s.wl.commit();
        }
    }

    pub fn keyboard_surface(&self) -> Option<Rc<Surface>> {
        self.keyboard_on.clone()
    }

    /// The surface the pointer is on, if one of ours
    pub fn pointer_surface(&self) -> Option<Rc<Surface>> {
        self.pointer_on.clone()
    }

    /// One of ours, by its wl_surface (touch.rs)
    pub(crate) fn surface_for(&self, wl: &WlSurface) -> Option<Rc<Surface>> {
        self.find(wl)
    }

    fn find(&self, wl: &WlSurface) -> Option<Rc<Surface>> {
        self.surfaces.iter().find(|s| &s.wl == wl).cloned()
    }

    /// Whether any surface drew in this turn (after render_all)
    pub fn drew_this_turn(&self) -> bool {
        self.surfaces.iter().any(|s| s.drew.get())
    }

    /// After each event loop turn: draw what changed
    pub fn render_all(&mut self) {
        RENDER_TURN.set(RENDER_TURN.get() + 1);
        self.recover_lost_frames();
        for s in &self.surfaces {
            s.drew.set(false);
        }
        for s in self.surfaces.clone() {
            // (its bindings call the part's pure callbacks: the part's code, contain.rs)
            crate::perf::time(|| format!("render {}", s.namespace), || crate::contain::guard(s.owner, || s.render(&self.qh, self.presentation.as_ref())));
        }
        if IME_DIRTY.replace(false) {
            self.sync_text_input();
        }
    }

    /// Glass shapes a part set for one of its surfaces go to the plugin only
    /// with the frame they belong to (Glass::set holds them): when that
    /// surface drew in this turn, or has nothing to draw (its content stands
    /// still; the shapes moved on their own). Not while a frame is still on
    /// its way (the shapes would run ahead of the picture: an island pill's
    /// glass wider than the pill), nor before the surface has a good frame on
    /// screen (unmapped, or its very first frame, which can come out empty:
    /// a frosted panel with nothing in it). Clearing is never held. Call
    /// after every part's after_frame.
    pub fn flush_glass(&self) {
        let gate = |monitor: &str, ns: &str| {
            let mut best: Option<crate::services::hypr::GlassGate> = None;
            for s in self.surfaces.iter().filter(|s| s.output == monitor && s.namespace == ns && !s.retired.get()) {
                // (a surface's first frame ever is drawn twice: render())
                let good = s.mapped.get() && s.shown_frames.get() >= 1 && s.rendered.get() >= 2;
                let g = crate::services::hypr::GlassGate {
                    good,
                    drew: s.drew.get().then(|| s.rendered.get()),
                    idle: !s.frame_pending.get() && !(s.needs_redraw.get() && s.configured.get()),
                    presented: s.presented.get(),
                };
                // (several with one namespace: the one that's on screen)
                if best.as_ref().is_none_or(|b| !b.good && good) {
                    best = Some(g);
                }
            }
            best
        };
        self.shell.glass.flush(gate);
    }

    /// A frame callback that hasn't come after LOST_FRAME is taken as lost:
    /// the frame counts as shown and the surface may draw again. (Live, the
    /// compositor can keep a callback back — an occluded or idle output — and
    /// the surface then never drew again: a bar stuck mid-animation, and the
    /// island's glass waiting on that frame left up as an empty pill.)
    fn recover_lost_frames(&self) {
        let now = Instant::now();
        for s in &self.surfaces {
            if s.frame_pending.get() && s.lost_frame_at().is_some_and(|t| now >= t) {
                let streak = s.lost_streak.get();
                if streak == 0 && (crate::env::is_set("DEBUG") || wakes_on()) {
                    eprintln!("debug: frame callback lost on {}: drawing on", s.namespace);
                }
                s.lost_streak.set(streak.saturating_add(1));
                s.frame_pending.set(false);
                s.last_done.set(None);
                s.presented.set(s.rendered.get());
            }
        }
    }

    /// A surface waits on a frame callback with more to draw: the loop wakes
    /// by LOST_FRAME to draw on should the callback never come
    pub fn waits_on_frame(&self) -> bool {
        self.lost_frame_deadline().is_some()
    }

    /// When the first callback still awaited by a surface with more to draw
    /// counts as lost (the loop's deadline for it)
    pub fn lost_frame_deadline(&self) -> Option<Instant> {
        self.surfaces.iter().filter(|s| s.frame_pending.get() && s.needs_redraw.get() && s.configured.get()).filter_map(|s| s.lost_frame_at()).min()
    }

    pub fn wants_frame_soon(&self) -> bool {
        self.surfaces.iter().any(|s| s.needs_redraw.get() && s.configured.get() && !s.frame_pending.get() && s.held_until().is_none())
    }

    /// Battery saver holds a frame back (60 a second at most): when the
    /// first surface held may draw (the loop's deadline for it)
    pub fn held_frame_deadline(&self) -> Option<Instant> {
        self.surfaces.iter().filter(|s| s.needs_redraw.get() && s.configured.get() && !s.frame_pending.get()).filter_map(|s| s.held_until()).min()
    }

    /// An animation runs but no frame callback will wake the loop: tick anyway
    /// (at the display's refresh: pacing::refresh). Slint's flag is one for
    /// every window (`has_active_animations` asks the one animation clock);
    /// with no window on screen, nothing of it can be seen.
    pub fn animating_unpaced(&self) -> bool {
        !self.surfaces.iter().any(|s| s.frame_pending.get())
            && self.surfaces.iter().any(|s| s.mapped.get())
            && self.surfaces.first().is_some_and(|s| s.window.has_active_animations())
    }

    fn set_cursor(&self, serial: u32, shape: Shape) {
        if let Some(c) = &self.cursor {
            c.set_shape(serial, shape);
        }
    }

    // text-input-v3: enable it while the focused surface's TextInput wants an
    // input method, with the cursor rectangle for the candidate popup
    fn sync_text_input(&mut self) {
        if crate::env::is_set("DEBUG") {
            eprintln!("debug: text-input sync (object {}, entered {})", self.text_input.is_some(), self.ti_on.is_some());
        }
        let (Some(ti), Some(on)) = (&self.text_input, &self.ti_on) else { return };
        let Some(s) = self.find(on) else { return };
        let props = s.ime.borrow().clone();
        match props {
            Some(p) => {
                if !self.ti_enabled {
                    ti.enable();
                }
                ti.set_content_type(
                    ContentHint::None,
                    if p.password { ContentPurpose::Password } else { ContentPurpose::Normal },
                );
                ti.set_cursor_rectangle(p.x, p.y, p.w, p.h);
                ti.commit();
                self.ti_enabled = true;
            }
            None if self.ti_enabled => {
                ti.disable();
                ti.commit();
                self.ti_enabled = false;
            }
            None => {}
        }
    }
}

// ── Handlers ────────────────────────────────────────────────────────────────

impl CompositorHandler for State {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, wl: &WlSurface, factor: i32) {
        // Integer scale (wl_surface.preferred_buffer_scale), used only
        // without wp_fractional_scale
        if self.fractional.is_some() {
            return;
        }
        if let Some(s) = self.find(wl) {
            s.scale.set(factor as f32);
            if s.configured.get() {
                s.apply_geometry();
            }
        }
    }
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: smithay_client_toolkit::reexports::client::protocol::wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, wl: &WlSurface, time: u32) {
        if let Some(s) = self.find(wl) {
            s.frame_pending.set(false);
            s.lost_streak.set(0);
            s.last_done.set(Some(RENDER_TURN.get()));
            s.presented.set(s.presented.get().wrapping_add(1).min(s.rendered.get()));
            s.stats.borrow_mut().presented(time);
            if !self.first_frame_logged {
                self.first_frame_logged = true;
                crate::stats::log_first_frame(self.started);
                // (on screen: the shell can stop drawing its own)
                crate::surfaces::announce(&self.shell.surfaces);
            }
        }
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: &WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: &WlOutput) {}
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: WlOutput) {
        self.note_refresh(&output);
        let name = self.outputs.info(&output).and_then(|i| i.name).unwrap_or_default();
        self.each_part(|p, st| p.output_added(st, &output, &name));
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: WlOutput) {
        self.note_refresh(&output);
    }
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, output: WlOutput) {
        if let Some(name) = self.outputs.info(&output).and_then(|i| i.name) {
            crate::pacing::set_output_refresh(&name, 0);
            self.each_part(|p, st| p.output_removed(st, &name));
        }
    }
}

impl State {
    /// An output's refresh rate (its current mode), for animations' pace
    fn note_refresh(&self, output: &WlOutput) {
        let Some(info) = self.outputs.info(output) else { return };
        let mhz = info.modes.iter().find(|m| m.current).map_or(0, |m| m.refresh_rate);
        crate::pacing::set_output_refresh(&info.name.unwrap_or_default(), mhz);
    }
}

impl LayerShellHandler for State {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        if let Some(s) = self.surfaces.iter().find(|s| s.layer == *layer).cloned() {
            self.each_part(|p, st| p.surface_closed(st, &s));
        }
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface, c: LayerSurfaceConfigure, _: u32) {
        let Some(s) = self.surfaces.iter().find(|s| s.layer == *layer).cloned() else { return };
        if crate::env::is_set("DEBUG") {
            eprintln!("debug: configure {} {:?} scale {}", s.namespace, c.new_size, s.scale.get());
        }
        if wakes_on() {
            eprintln!("configure {} at {}", s.namespace, crate::util::now_ms());
        }
        let (w, h) = s.logical.get();
        let size = (if c.new_size.0 > 0 { c.new_size.0 } else { w }, if c.new_size.1 > 0 { c.new_size.1 } else { h });
        s.logical.set(size);
        if s.hidden.get() {
            return; // (unmapped on purpose: the size waits for `show`)
        }
        s.configured.set(true);
        s.frame_pending.set(false);
        s.apply_geometry();
    }
}

// A regular window (the App Store's): the compositor's size, or the one it
// asked for; a close request goes to its part (Part::surface_closed)
impl WindowHandler for State {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &XdgWindow) {
        if let Some(s) = self.surfaces.iter().find(|s| s.layer.window() == Some(window)).cloned() {
            self.each_part(|p, st| p.surface_closed(st, &s));
        }
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &XdgWindow, c: WindowConfigure, _: u32) {
        let Some(s) = self.surfaces.iter().find(|s| s.layer.window() == Some(window)).cloned() else { return };
        let (w, h) = s.logical.get();
        let size = (c.new_size.0.map_or(w, |v| v.get()), c.new_size.1.map_or(h, |v| v.get()));
        s.logical.set(size);
        if s.hidden.get() {
            return;
        }
        s.configured.set(true);
        s.frame_pending.set(false);
        s.apply_geometry();
    }
}

// A popup: its size when the compositor places it; dismissed (a click
// elsewhere, its parent gone) reaches its part as a closed surface
impl smithay_client_toolkit::shell::xdg::popup::PopupHandler for State {
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, popup: &smithay_client_toolkit::shell::xdg::popup::Popup, c: smithay_client_toolkit::shell::xdg::popup::PopupConfigure) {
        let Some(s) = self.surfaces.iter().find(|s| s.layer.popup().is_some_and(|p| p.xdg_popup() == popup.xdg_popup())).cloned() else { return };
        if c.width > 0 && c.height > 0 {
            s.logical.set((c.width as u32, c.height as u32));
        }
        if s.hidden.get() {
            return;
        }
        s.configured.set(true);
        s.frame_pending.set(false);
        s.apply_geometry();
    }
    fn done(&mut self, _: &Connection, _: &QueueHandle<Self>, popup: &smithay_client_toolkit::shell::xdg::popup::Popup) {
        if let Some(s) = self.surfaces.iter().find(|s| s.layer.popup().is_some_and(|p| p.xdg_popup() == popup.xdg_popup())).cloned() {
            self.each_part(|p, st| p.surface_closed(st, &s));
        }
    }
}

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: WlSeat, cap: Capability) {
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            // (SCTK reports seats that existed at start only as capabilities)
            if let (None, Some(m)) = (&self.text_input, &self.text_inputs) {
                self.text_input = Some(m.get_text_input(&seat, qh, ()));
            }
            let cb = Box::new(|state: &mut State, _: &WlKeyboard, e: KeyEvent| state.key(e, KeyKind::Repeat));
            self.keyboard = self.seat.get_keyboard_with_repeat(qh, &seat, None, self.loop_handle.clone(), cb).ok();
        }
        // (touch: taps, drags and long presses as the pointer's, touch.rs)
        if cap == Capability::Touch {
            crate::touch::bind(self, qh, &seat);
        }
        if cap == Capability::Pointer && self.pointer.is_none() {
            self.dnd.seat(&seat, qh);
            if let Ok(p) = self.seat.get_pointer(qh, &seat) {
                self.cursor = self.cursor_shapes.as_ref().map(|m| m.get_shape_device(&p, qh));
                self.pointer = Some(p);
            }
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat, cap: Capability) {
        if cap == Capability::Keyboard {
            if let Some(k) = self.keyboard.take() {
                k.release();
            }
        }
        if cap == Capability::Pointer {
            if let Some(p) = self.pointer.take() {
                p.release();
            }
        }
        if cap == Capability::Touch {
            crate::touch::unbind();
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
}

const MODS: [Key; 4] = [Key::Shift, Key::Control, Key::Alt, Key::Meta];

enum KeyKind {
    Press,
    Repeat,
    Release,
}

/// xkb keysym -> Slint's key text (special keys are private-use characters)
fn key_text(e: &KeyEvent, release: bool, mods: [bool; 4]) -> Option<slint::SharedString> {
    let k = match e.keysym {
        Keysym::Up => Key::UpArrow,
        Keysym::Down => Key::DownArrow,
        Keysym::Left => Key::LeftArrow,
        Keysym::Right => Key::RightArrow,
        Keysym::Return | Keysym::KP_Enter => Key::Return,
        Keysym::Escape => Key::Escape,
        Keysym::BackSpace => Key::Backspace,
        Keysym::Tab => Key::Tab,
        Keysym::ISO_Left_Tab => Key::Backtab,
        Keysym::Delete => Key::Delete,
        Keysym::Home => Key::Home,
        Keysym::End => Key::End,
        Keysym::Page_Up => Key::PageUp,
        Keysym::Page_Down => Key::PageDown,
        // (function keys: the text login screen's F1 shut down, F2 restart, F3 sleep)
        Keysym::F1 => Key::F1,
        Keysym::F2 => Key::F2,
        Keysym::F3 => Key::F3,
        Keysym::F4 => Key::F4,
        Keysym::F5 => Key::F5,
        Keysym::F6 => Key::F6,
        Keysym::F7 => Key::F7,
        Keysym::F8 => Key::F8,
        Keysym::F9 => Key::F9,
        Keysym::F10 => Key::F10,
        Keysym::F11 => Key::F11,
        Keysym::F12 => Key::F12,
        Keysym::Shift_L | Keysym::Shift_R => Key::Shift,
        Keysym::Control_L | Keysym::Control_R => Key::Control,
        Keysym::Alt_L | Keysym::Alt_R => Key::Alt,
        Keysym::Super_L | Keysym::Super_R => Key::Meta,
        Keysym::Menu => Key::Menu, // (the app drawer's app menu; Shift+F10 is F10 above)
        // (Print Screen: Slint's SysReq is it; Settings' shortcut recorder names it "Print")
        Keysym::Print | Keysym::Sys_Req => Key::SysReq,
        _ => {
            // (a shortcut: Ctrl or Super held; mods [shift, ctrl, alt, super])
            if mods[1] || mods[3] {
                if let Some(t) = shortcut_text(e.keysym.key_char(), e.raw_code, mods[0]) {
                    return Some(t.into());
                }
            }
            let t = if release { None } else { e.utf8.clone() };
            let t = t.or_else(|| e.keysym.key_char().map(String::from))?;
            return (!t.is_empty() && !t.chars().all(char::is_control)).then(|| t.into());
        }
    };
    Some(k.into())
}

/// A shortcut's key on any layout: the keysym's own character (not the
/// control character xkb makes of Ctrl+F), and on a non-Latin layout
/// (Russian, Greek, Hebrew, Arabic…) the Latin letter on the same key, as
/// GTK and Qt do, so Ctrl+F finds and Ctrl+V pastes on ЙЦУКЕН too. None:
/// no character (the usual text then)
pub fn shortcut_text(sym: Option<char>, evdev: u32, shift: bool) -> Option<String> {
    let c = sym?;
    if c.is_control() {
        return None;
    }
    if c.is_ascii() || !c.is_alphabetic() {
        return Some(c.to_string());
    }
    // KEY_Q.. (16..25), KEY_A.. (30..38), KEY_Z.. (44..50): the keys' places, whatever the layout
    const ROWS: [(u32, &str); 3] = [(16, "qwertyuiop"), (30, "asdfghjkl"), (44, "zxcvbnm")];
    let latin = ROWS.iter().find_map(|(first, row)| evdev.checked_sub(*first).and_then(|i| row.chars().nth(i as usize)));
    Some(match latin {
        Some(l) if shift => l.to_ascii_uppercase().to_string(),
        Some(l) => l.to_string(),
        // (a Latin layout's own letter, é or ß: as it is)
        None => c.to_string(),
    })
}

impl State {
    fn key(&mut self, e: KeyEvent, kind: KeyKind) {
        crate::pacing::input();
        if crate::perf::on() {
            let on = self.keyboard_on.as_ref().map(|s| s.namespace.clone()).unwrap_or_default();
            return crate::perf::time(|| format!("key on {on}"), || self.key_now(e, kind));
        }
        self.key_now(e, kind)
    }
    fn key_now(&mut self, e: KeyEvent, kind: KeyKind) {
        let Some(s) = self.keyboard_on.clone() else { return };
        let Some(text) = key_text(&e, matches!(kind, KeyKind::Release), self.mods_sent) else { return };
        if let Some(i) = MODS.iter().position(|m| text == slint::SharedString::from(*m)) {
            self.mods_sent[i] = !matches!(kind, KeyKind::Release);
        }
        s.dispatch(match kind {
            KeyKind::Press => WindowEvent::KeyPressed { text },
            KeyKind::Repeat => WindowEvent::KeyPressRepeated { text },
            KeyKind::Release => WindowEvent::KeyReleased { text },
        });
    }
}

impl KeyboardHandler for State {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, wl: &WlSurface, _: u32, _: &[u32], _: &[Keysym]) {
        self.keyboard_on = self.find(wl);
        self.mods_sent = [false; 4]; // (the seat's modifiers follow)
        if let Some(s) = &self.keyboard_on {
            s.dispatch(WindowEvent::WindowActiveChanged(true));
        }
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, wl: &WlSurface, _: u32) {
        if let Some(s) = self.find(wl) {
            s.dispatch(WindowEvent::WindowActiveChanged(false));
            self.each_part(|p, st| p.keyboard_left(st, &s));
        }
        self.keyboard_on = None;
    }
    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, serial: u32, e: KeyEvent) {
        INPUT_SERIAL.set(serial);
        self.key(e, KeyKind::Press);
    }
    fn repeat_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, _: u32, e: KeyEvent) {
        self.key(e, KeyKind::Repeat);
    }
    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, _: u32, e: KeyEvent) {
        self.key(e, KeyKind::Release);
    }
    /// The seat's modifiers: Slint follows modifier key presses, so one it
    /// didn't see go down or up (a virtual keyboard's, wtype -M) is pressed
    /// or released for it here
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, _: u32, m: Modifiers, _: RawModifiers, _: u32) {
        if crate::env::is_set("DEBUG") {
            eprintln!("debug: modifiers {m:?} (sent {:?})", self.mods_sent);
        }
        let Some(s) = self.keyboard_on.clone() else { return };
        for (i, want) in [m.shift, m.ctrl, m.alt, m.logo].into_iter().enumerate() {
            if self.mods_sent[i] != want {
                self.mods_sent[i] = want;
                let text = slint::SharedString::from(MODS[i]);
                s.dispatch(if want { WindowEvent::KeyPressed { text } } else { WindowEvent::KeyReleased { text } });
            }
        }
    }
}

impl PointerHandler for State {
    fn pointer_frame(&mut self, c: &Connection, q: &QueueHandle<Self>, p: &WlPointer, events: &[PointerEvent]) {
        crate::pacing::input();
        if crate::perf::on() || wakes_on() {
            let what = events.iter().find_map(|e| self.find(&e.surface).map(|s| format!("pointer {} on {}", match e.kind {
                PointerEventKind::Enter { .. } => "enter",
                PointerEventKind::Leave { .. } => "leave",
                PointerEventKind::Motion { .. } => "motion",
                PointerEventKind::Press { .. } => "press",
                PointerEventKind::Release { .. } => "release",
                PointerEventKind::Axis { .. } => "scroll",
            }, s.namespace))).unwrap_or_else(|| "pointer".into());
            return crate::perf::time(|| what, || self.pointer_frame_now(c, q, p, events));
        }
        self.pointer_frame_now(c, q, p, events)
    }
}

impl State {
    fn pointer_frame_now(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlPointer, events: &[PointerEvent]) {
        for e in events {
            let Some(s) = self.find(&e.surface) else { continue };
            let position = LogicalPosition::new(e.position.0 as f32, e.position.1 as f32);
            match &e.kind {
                PointerEventKind::Enter { serial } => {
                    POINTER_SERIAL.set(*serial);
                    if crate::env::is_set("DEBUG") {
                        eprintln!("debug: pointer enters {} at {:?}", s.namespace, e.position);
                    }
                    self.set_cursor(*serial, s.cursor.get());
                    s.enters.set(s.enters.get().wrapping_add(1));
                    self.pointer_on = Some(s.clone());
                    s.dispatch(WindowEvent::PointerMoved { position });
                }
                PointerEventKind::Leave { .. } => {
                    if crate::env::is_set("DEBUG") {
                        eprintln!("debug: pointer leaves {}", s.namespace);
                    }
                    s.dispatch(WindowEvent::PointerExited);
                    self.pointer_on = None;
                }
                PointerEventKind::Motion { .. } => s.dispatch(WindowEvent::PointerMoved { position }),
                PointerEventKind::Press { button, .. } | PointerEventKind::Release { button, .. } => {
                    let button = match *button {
                        0x110 => PointerEventButton::Left,
                        0x111 => PointerEventButton::Right,
                        0x112 => PointerEventButton::Middle,
                        _ => PointerEventButton::Other,
                    };
                    if crate::env::is_set("DEBUG") {
                        eprintln!("debug: button {:?} on {} at {:?}", e.kind, s.namespace, e.position);
                    }
                    if let PointerEventKind::Press { serial, .. } = e.kind {
                        self.dnd.pressed(serial, &s);
                        INPUT_SERIAL.set(serial);
                    }
                    if matches!(e.kind, PointerEventKind::Press { .. }) {
                        if wakes_on() {
                            pressed_now(&s.namespace);
                        }
                        // (off the window: on another screen, the panel holding the pointer)
                        let (w, h) = s.logical_size();
                        if off_window(e.position, (w, h)) {
                            if crate::env::is_set("DEBUG") || wakes_on() {
                                eprintln!("press off {} at {:?} ({w}x{h}): elsewhere", s.namespace, e.position);
                            }
                            self.each_part(|p, st| p.pointer_pressed_outside(st, &s));
                            continue;
                        }
                        self.each_part(|p, st| p.pointer_pressed(st, &s));
                        s.dispatch(WindowEvent::PointerPressed { position, button });
                    } else {
                        if wakes_on() {
                            eprintln!("release on {} at {}", s.namespace, crate::util::now_ms());
                        }
                        s.dispatch(WindowEvent::PointerReleased { position, button });
                    }
                }
                PointerEventKind::Axis { horizontal, vertical, .. } => {
                    let d = |a: &smithay_client_toolkit::seat::pointer::AxisScroll| -(a.absolute as f32);
                    s.dispatch(WindowEvent::PointerScrolled { position, delta_x: d(horizontal), delta_y: d(vertical) });
                }
            }
            if CURSOR_DIRTY.replace(false) && self.pointer_on.as_ref().is_some_and(|p| Rc::ptr_eq(p, &s)) {
                self.set_cursor(POINTER_SERIAL.get(), s.cursor.get());
            }
        }
    }
}

// fractional scale: preferred scale in 120ths
impl Dispatch<WpFractionalScaleV1, WlSurface> for State {
    fn event(state: &mut Self, _: &WpFractionalScaleV1, ev: wp_fractional_scale_v1::Event, wl: &WlSurface, _: &Connection, _: &QueueHandle<Self>) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = ev {
            if let Some(s) = state.find(wl) {
                let f = scale as f32 / 120.0;
                if (s.scale.get() - f).abs() > f32::EPSILON {
                    s.scale.set(f);
                    if s.configured.get() {
                        s.apply_geometry();
                    }
                }
            }
        }
    }
}

// text-input-v3: commit and preedit from the input method, applied on `done`
impl Dispatch<ZwpTextInputV3, ()> for State {
    fn event(state: &mut Self, _: &ZwpTextInputV3, ev: zwp_text_input_v3::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        use i_slint_core::input::{InternalKeyEvent, KeyEventType};
        match ev {
            zwp_text_input_v3::Event::Enter { surface } => {
                if crate::env::is_set("DEBUG") {
                    eprintln!("debug: text-input entered a surface");
                }
                state.ti_on = Some(surface);
                state.ti_enabled = false;
                state.sync_text_input();
            }
            zwp_text_input_v3::Event::Leave { .. } => {
                state.ti_on = None;
                state.ti_enabled = false;
            }
            zwp_text_input_v3::Event::PreeditString { text, cursor_begin, cursor_end } => {
                state.ti_pending.1 = Some((text.unwrap_or_default(), cursor_begin, cursor_end));
            }
            zwp_text_input_v3::Event::CommitString { text } => state.ti_pending.0 = text,
            zwp_text_input_v3::Event::Done { .. } => {
                let Some(s) = state.ti_on.as_ref().and_then(|w| state.find(w)) else { return };
                let (commit, preedit) = std::mem::take(&mut state.ti_pending);
                if let Some(text) = commit {
                    let mut e = InternalKeyEvent { event_type: KeyEventType::CommitComposition, ..Default::default() };
                    e.key_event.text = text.into();
                    s.dispatch(WindowEvent::internal(e));
                }
                let (text, b, en) = preedit.unwrap_or_default();
                let e = InternalKeyEvent {
                    event_type: KeyEventType::UpdateComposition,
                    preedit_text: text.into(),
                    // (a cursor, not a selection, when both ends meet)
                    preedit_selection: (b >= 0 && en > b).then_some(b..en),
                    ..Default::default()
                };
                s.dispatch(WindowEvent::internal(e));
            }
            _ => {}
        }
    }
}

macro_rules! no_events {
    ($($t:ty),*) => {$(
        impl Dispatch<$t, ()> for State {
            fn event(_: &mut Self, _: &$t, _: <$t as WlProxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
        }
    )*};
}
no_events!(WpViewporter, WpViewport, WpFractionalScaleManagerV1, ZwpTextInputManagerV3);
no_events!(ZwpIdleInhibitManagerV1, ZwpIdleInhibitorV1);
no_events!(ExtIdleNotifierV1);

// wp_presentation: its clock (CLOCK_MONOTONIC on Linux compositors: the
// timestamps are only compared with each other)
impl Dispatch<WpPresentation, ()> for State {
    fn event(_: &mut Self, _: &WpPresentation, _: wp_presentation::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

// An animation frame reached the screen, or never did: the time since the
// surface's last one on screen is a frame interval (pacing.rs)
impl Dispatch<WpPresentationFeedback, Fb> for State {
    fn event(state: &mut Self, _: &WpPresentationFeedback, ev: wp_presentation_feedback::Event, fb: &Fb, _: &Connection, _: &QueueHandle<Self>) {
        let Some(s) = state.surfaces.iter().find(|s| WlProxy::id(&s.wl) == fb.surface) else { return };
        match ev {
            wp_presentation_feedback::Event::Presented { tv_sec_hi, tv_sec_lo, tv_nsec, refresh, .. } => {
                let ns = ((((tv_sec_hi as u64) << 32) | tv_sec_lo as u64) * 1_000_000_000).wrapping_add(tv_nsec as u64);
                let last = s.last_present_ns.replace(ns);
                if last > 0 && ns > last {
                    crate::pacing::presented(ns - last, refresh as u64, s.cpu_slow.get() != Some(fb.frame));
                }
            }
            wp_presentation_feedback::Event::Discarded => {
                s.last_present_ns.set(0);
                crate::pacing::discarded();
            }
            _ => {}
        }
    }
}

// An idle watch said idle or resumed: the newest picture of all watches is the
// shell's `Idle` data, and every part (and the host's services) hear of it
impl Dispatch<ExtIdleNotificationV1, u32> for State {
    fn event(state: &mut Self, _: &ExtIdleNotificationV1, event: ext_idle_notification_v1::Event, id: &u32, _: &Connection, _: &QueueHandle<Self>) {
        let idle = match event {
            ext_idle_notification_v1::Event::Idled => true,
            ext_idle_notification_v1::Event::Resumed => false,
            _ => return,
        };
        if !state.idle_watches.contains_key(id) {
            return; // (a watch dropped a moment ago)
        }
        state.idle_state.insert(*id, idle);
        let what = state.shell.store(Box::new(crate::idle::Idle(state.idle_state.clone())));
        state.each_part(|p, st| p.data(st, what));
    }
}

delegate_compositor!(State);
delegate_output!(State);
delegate_layer!(State);
delegate_xdg_shell!(State);
delegate_xdg_window!(State);
smithay_client_toolkit::delegate_xdg_popup!(State);
delegate_seat!(State);
delegate_keyboard!(State);
delegate_pointer!(State);
smithay_client_toolkit::delegate_touch!(State);
delegate_registry!(State);
delegate_session_lock!(State);

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

// ── The session lock (ext-session-lock-v1), for the lock part ───────────────
// Taken only when the lock part asks (src/surfaces/lock: its top comment says
// when); its news reach that part's `message` as a LockEvent.

impl State {
    /// Ask the compositor to lock the session (None: it has no session lock)
    pub fn lock_session(&mut self) -> Option<SessionLock> {
        self.session_lock.lock(&self.qh).ok()
    }

    /// A lock surface on `output`, ready to become the window of the next
    /// Slint component made (as `prepare_surface`)
    pub fn prepare_lock_surface(&mut self, lock: &SessionLock, output: &WlOutput, output_name: &str) -> Rc<Surface> {
        let wl = self.compositor.create_surface(&self.qh);
        let viewport = self.viewporter.as_ref().map(|v| v.get_viewport(&wl, &self.qh, ()));
        let fractional = self.fractional.as_ref().map(|f| f.get_fractional_scale(&wl, &self.qh, wl.clone()));
        let role = lock.create_lock_surface(wl.clone(), output, &self.qh);
        self.adopt(wl, Role::Lock(role), viewport, fractional, "tessera:lock", output_name.to_string(), (1, 1))
    }

    /// ext-idle-notify-v1: watch for `ms` without input (the compositor's idle
    /// notification, as Quickshell's IdleMonitor; `respect`: a video playing or
    /// another inhibitor keeps it from going idle). `None` stops the watch.
    /// Asking again with the same numbers does nothing, so a service may state
    /// what it wants after every turn. False: the compositor can't say (no
    /// protocol, no seat yet) and nothing is watched. The answers are the shell's
    /// `idle::Idle` data (idle.rs).
    pub fn idle_set(&mut self, id: u32, spec: Option<(u32, bool)>) -> bool {
        let Some(n) = self.idle_notifier.clone() else { return false };
        match spec {
            None => {
                if let Some((w, _, _)) = self.idle_watches.remove(&id) {
                    w.destroy();
                    self.idle_state.remove(&id);
                }
            }
            Some((ms, respect)) => {
                if self.idle_watches.get(&id).is_some_and(|(_, m, r)| *m == ms && *r == respect) {
                    return true;
                }
                let Some(seat) = self.seat.seats().next() else { return false };
                if let Some((w, _, _)) = self.idle_watches.remove(&id) {
                    w.destroy();
                }
                // (version 2 has the input-only watch that ignores inhibitors)
                let w = if respect || WlProxy::version(&n) < 2 { n.get_idle_notification(ms, &seat, &self.qh, id) } else { n.get_input_idle_notification(ms, &seat, &self.qh, id) };
                self.idle_watches.insert(id, (w, ms, respect));
                self.idle_state.insert(id, false);
            }
        }
        true
    }

    /// Keep the screen on while this surface shows (None: the compositor has
    /// no idle-inhibit protocol); `destroy` the inhibitor to let it sleep again
    pub fn inhibit_idle(&self, s: &Surface) -> Option<ZwpIdleInhibitorV1> {
        self.idle_inhibit.as_ref().map(|m| m.create_inhibitor(&s.wl, &self.qh, ()))
    }
}

impl SessionLockHandler for State {
    fn locked(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {
        self.part("lock", |p, st| p.message(st, Box::new(LockEvent::Locked)));
    }
    fn finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {
        self.part("lock", |p, st| p.message(st, Box::new(LockEvent::Finished)));
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: SessionLockSurface, c: SessionLockSurfaceConfigure, _: u32) {
        let Some(s) = self.surfaces.iter().find(|s| s.wl == *surface.wl_surface()).cloned() else { return };
        s.logical.set((c.new_size.0.max(1), c.new_size.1.max(1)));
        s.configured.set(true);
        s.frame_pending.set(false);
        s.apply_geometry();
    }
}

/// The window's renderer on its EGL surface: FemtoVG, or Skia (--features skia)
#[cfg(not(feature = "skia"))]
fn make_renderer(gl: GlSurface) -> crate::damage::Inner {
    FemtoVGRenderer::new(gl).expect("FemtoVG renderer")
}
#[cfg(feature = "skia")]
fn make_renderer(gl: GlSurface) -> crate::damage::Inner {
    thread_local! {
        static SHARED: crate::skia::SkiaSharedContext = Default::default();
    }
    let surface = crate::skia::SkiaGl::new(gl).expect("Skia on EGL");
    SHARED.with(|c| crate::skia::SkiaRenderer::new_with_surface(c, Box::new(surface)))
}

thread_local! {
    /// the last pointer press (wake log): when, on which surface, and the
    /// surfaces that have drawn since (each logged once, for 2 s)
    static PRESS: RefCell<Option<(Instant, String, Vec<String>)>> = const { RefCell::new(None) };
}
fn pressed_now(ns: &str) {
    eprintln!("press on {ns} at {}", crate::util::now_ms());
    PRESS.with(|p| *p.borrow_mut() = Some((Instant::now(), ns.to_string(), Vec::new())));
}
/// A surface drew: the first time since the last press, how long after it
/// (`click <surface> -> <surface> drew <ms> after, render <ms>`)
fn drew_after_press(ns: &str, render_ms: f64) {
    PRESS.with(|p| {
        let mut p = p.borrow_mut();
        let Some((at, on, seen)) = p.as_mut() else { return };
        let ms = at.elapsed().as_secs_f64() * 1000.0;
        if ms > 2000.0 || seen.iter().any(|s| s == ns) {
            return;
        }
        seen.push(ns.to_string());
        eprintln!("click {on} -> {ns} drew {ms:.0} ms after, render {render_ms:.1} ms, at {}", crate::util::now_ms());
    });
}

static LAYER_CHANGE_AT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// (also when a panel is told to open or close, or closes itself: it gives
/// the keyboard back as its close starts, before it unmaps)
pub fn note_layer_change() {
    LAYER_CHANGE_AT.store(crate::util::now_ms(), std::sync::atomic::Ordering::Relaxed);
}

/// When one of the shell's surfaces last mapped or unmapped (ms, util::now_ms;
/// 0: never). Hyprland moves its focused monitor with a layer that takes or
/// gives back the keyboard, and its `focusedmonv2` can come before its own
/// open/closelayer: the display-switch cue asks this, which we know first
pub fn layer_changed_at() -> i64 {
    LAYER_CHANGE_AT.load(std::sync::atomic::Ordering::Relaxed)
}

/// A press at this surface-local position is off the window (`size`
/// logical px; 0: not known yet, never off)
pub fn off_window(at: (f64, f64), size: (u32, u32)) -> bool {
    size.0 > 0 && size.1 > 0 && (at.0 < 0.0 || at.1 < 0.0 || at.0 >= size.0 as f64 || at.1 >= size.1 as f64)
}

/// TESSERA_FRAMELOG=1: every frame a surface draws and every hide, with the
/// wall time (`framelog frame <namespace> <unix s>`): which surface drew the
/// frames a compositor frame log (tools/gate/framelog.py) counts, and when a
/// panel's close animation really ended. Off: one cached bool test.
pub fn framelog(what: &str, ns: &str) {
    framelog_on(what, ns, "");
}

/// The same for a surface on an output: `<namespace>@<output>` (two bars,
/// one per screen, told apart)
pub fn framelog_on(what: &str, ns: &str, output: &str) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ON.get_or_init(|| crate::env::is_set("FRAMELOG")) {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
        let at = if output.is_empty() { "" } else { "@" };
        eprintln!("framelog {what} {ns}{at}{output} {t:.6}");
    }
}

/// TESSERA_WAKES=1: once a second, why the loop turned and which surfaces
/// drew (finding what keeps an idle shell busy):
/// `wakes: {why: turns} sleeps N cpu MS drew {surface: frames} animating
/// TURNS hidden-asks {surface: n} frames {…} msgs {…} ms […]`.
/// `sleeps` are the times the UI thread really slept and woke (its voluntary
/// context switches), `cpu` its processor time; `animating` the turns
/// Slint's one animation clock ran (it is shared by every window, so it says
/// nothing of which); `hidden-asks` windows that aren't on screen asking to
/// be drawn (an animation or timer ticking where nobody sees it); `frames`
/// the animation frames on screen (pacing.rs: intervals, missed).
#[derive(Default)]
pub struct Wakes {
    since: Option<std::time::Instant>,
    turns: std::collections::BTreeMap<&'static str, u32>,
    drew: std::collections::BTreeMap<String, u32>,
    anim: u32,
    /// the UI thread's (voluntary switches, cpu µs) at `since`
    usage: (i64, i64),
}

/// This thread's voluntary context switches and processor time (µs)
fn thread_usage() -> (i64, i64) {
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrusage(libc::RUSAGE_THREAD, &mut ru) } != 0 {
        return (0, 0);
    }
    let us = |t: libc::timeval| t.tv_sec * 1_000_000 + t.tv_usec;
    (ru.ru_nvcsw, us(ru.ru_utime) + us(ru.ru_stime))
}

impl Wakes {
    pub fn note(&mut self, why: &'static str, st: &State) {
        if !wakes_on() {
            return;
        }
        *self.turns.entry(why).or_default() += 1;
        for s in &st.surfaces {
            if s.drew.get() {
                *self.drew.entry(s.namespace.clone()).or_default() += 1;
            }
        }
        if st.surfaces.first().is_some_and(|s| s.window.has_active_animations()) {
            self.anim += 1;
        }
        let now = std::time::Instant::now();
        if self.since.is_none() {
            self.since = Some(now);
            self.usage = thread_usage();
        }
        if now.duration_since(self.since.unwrap()) >= std::time::Duration::from_secs(1) {
            let msgs = WAKE_MSGS.with(|m| std::mem::take(&mut *m.borrow_mut()));
            let hidden = HIDDEN_ASKS.with(|h| std::mem::take(&mut *h.borrow_mut()));
            let u = thread_usage();
            let frames = crate::pacing::take_second().unwrap_or_else(|| "{}".into());
            eprintln!(
                "wakes: {:?} sleeps {} cpu {:.1} drew {:?} animating {} hidden-asks {:?} frames {frames} msgs {:?} ms {:?}",
                self.turns,
                u.0 - self.usage.0,
                (u.1 - self.usage.1) as f64 / 1000.0,
                self.drew,
                self.anim,
                hidden,
                msgs,
                crate::perf::spent_top(14)
            );
            *self = Wakes { since: Some(now), usage: u, ..Default::default() };
        }
    }
}

thread_local! {
    /// TESSERA_WAKES: the loop's messages this second, by part
    pub static WAKE_MSGS: std::cell::RefCell<std::collections::BTreeMap<String, u32>> = Default::default();
}

thread_local! {
    /// the last frame's damage probe, ms (draw stats)
    static PROBE_MS: Cell<f32> = const { Cell::new(0.0) };
}

/// TESSERA_DRAW_STATS=1: every frame logs where its time went (the
/// vendored Slint renderer's FrameStats)
fn draw_stats() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| crate::env::is_set("DRAW_STATS"))
}

/// TESSERA_WAKES set, or the file $XDG_RUNTIME_DIR/tessera-wakes there
/// (looked at every 2 s: on and off without a restart)
pub fn wakes_on() -> bool {
    static ENV: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENV.get_or_init(|| crate::env::is_set("WAKES")) {
        return true;
    }
    thread_local! {
        static FILE: std::cell::Cell<(bool, Option<std::time::Instant>)> = const { std::cell::Cell::new((false, None)) };
    }
    FILE.with(|f| {
        let (on, at) = f.get();
        if at.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(2)) {
            return on;
        }
        let on = std::path::Path::new(&std::env::var("XDG_RUNTIME_DIR").unwrap_or_default()).join("tessera-wakes").exists();
        f.set((on, Some(std::time::Instant::now())));
        on
    })
}

#[cfg(test)]
mod tests {
    use super::{off_window, shortcut_text};

    #[test]
    fn a_press_off_the_window_is_elsewhere() {
        // (a 1920x1080 panel on DP-1 at 1920,0; the press on eDP-1 at 300,500)
        assert!(off_window((-1620.0, 500.0), (1920, 1080)));
        assert!(off_window((2100.0, 10.0), (1920, 1080)));
        assert!(off_window((10.0, 1080.0), (1920, 1080)));
        assert!(!off_window((0.0, 0.0), (1920, 1080)));
        assert!(!off_window((1919.5, 1079.5), (1920, 1080)));
        // (no size yet: never)
        assert!(!off_window((-5.0, -5.0), (0, 0)));
    }

    #[test]
    fn shortcuts_by_key_on_any_layout() {
        // (US: the letter itself, not Ctrl+F's control character)
        assert_eq!(shortcut_text(Some('f'), 33, false).as_deref(), Some("f"));
        // (Russian ЙЦУКЕН: the Latin letter of the same key; Shift capitalises it)
        assert_eq!(shortcut_text(Some('а'), 33, false).as_deref(), Some("f"));
        assert_eq!(shortcut_text(Some('м'), 47, false).as_deref(), Some("v"));
        assert_eq!(shortcut_text(Some('Я'), 44, true).as_deref(), Some("Z"));
        // (Greek, Hebrew)
        assert_eq!(shortcut_text(Some('ψ'), 46, false).as_deref(), Some("c"));
        assert_eq!(shortcut_text(Some('ב'), 46, false).as_deref(), Some("c"));
        // (AZERTY: its own letters; a Latin layout's é off the letter rows stays é)
        assert_eq!(shortcut_text(Some('a'), 16, false).as_deref(), Some("a"));
        assert_eq!(shortcut_text(Some('é'), 3, false).as_deref(), Some("é"));
        assert_eq!(shortcut_text(Some('1'), 2, false).as_deref(), Some("1"));
        // (no character, or a control one: the usual text)
        assert_eq!(shortcut_text(None, 33, false), None);
        assert_eq!(shortcut_text(Some('\u{6}'), 33, false), None);
    }
}
