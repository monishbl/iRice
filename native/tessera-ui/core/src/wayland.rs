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

// ── One layer surface = one Slint window ────────────────────────────────────

pub struct Surface {
    window: slint::Window,
    /// FemtoVG, and what changed in each frame (src/damage.rs)
    renderer: crate::damage::Renderer,
    /// where render() leaves that for the EGL swap
    damage: Rc<RefCell<Option<(Vec<[i32; 4]>, i32)>>>,
    egl: Rc<Egl>,
    /// frames drawn, and of those the ones the compositor has shown (one
    /// is in flight at most: frame n is on screen once `presented` >= n)
    pub rendered: Cell<u32>,
    /// How many times the pointer has entered it (a part that never redraws,
    /// as a hot corner, tells a new visit from this, even when it left and
    /// came back between two turns of the loop)
    pub enters: Cell<u32>,
    pub presented: Cell<u32>,
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
}

impl std::ops::Deref for Role {
    type Target = LayerSurface;
    fn deref(&self) -> &LayerSurface {
        match self {
            Role::Layer(l) => l,
            Role::Lock(_) => panic!("tessera-ui: a session-lock surface has no layer surface"),
            Role::Window(_) => panic!("tessera-ui: a window has no layer surface"),
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
    pub fn logical_size(&self) -> (u32, u32) {
        self.logical.get()
    }
    fn physical(&self) -> PhysicalSize {
        let (w, h) = self.logical.get();
        LogicalSize::new(w as f32, h as f32).to_physical(self.scale.get())
    }
    /// The compositor's size and scale reached us: tell Slint
    fn apply_geometry(&self) {
        let (w, h) = self.logical.get();
        if let Some(vp) = &self.viewport {
            vp.set_destination(w as i32, h as i32);
        } else {
            self.wl.set_buffer_scale(self.scale.get().round().max(1.0) as i32);
        }
        self.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: self.scale.get() });
        self.window.dispatch_event(WindowEvent::Resized { size: LogicalSize::new(w as f32, h as f32) });
        self.needs_redraw.set(true);
        self.renderer.damage.all();
    }
    fn render(&self, qh: &QueueHandle<State>) {
        if !self.configured.get() || self.frame_pending.get() || !self.needs_redraw.get() {
            return;
        }
        self.needs_redraw.set(false);
        self.wl.frame(qh, self.wl.clone());
        let t = Instant::now();
        // What changed since the last frame: the compositor repaints only that
        if self.egl.has_damage() {
            let (w, h) = self.logical.get();
            let px = self.physical();
            let region = self.renderer.damage.region(&self.window, (w as f32, h as f32));
            *self.damage.borrow_mut() = region.map(|r| (crate::damage::to_buffer(&r, self.scale.get(), (px.width as i32, px.height as i32)), px.height as i32));
        }
        if let Err(e) = self.renderer.femto.render() {
            eprintln!("tessera-ui: render failed: {e}");
        }
        self.stats.borrow_mut().rendered(t.elapsed());
        self.rendered.set(self.rendered.get().wrapping_add(1));
        self.frame_pending.set(true);
        self.mapped.set(true);
        // (no "redraw while animations run" here: Slint's animation clock is
        // global, so every window would redraw while any one animates; an
        // animated property marks only its own window dirty)
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
        self.configured.set(false);
        self.frame_pending.set(false);
        self.mapped.set(false);
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
        match &self.layer {
            Role::Layer(l) => {
                l.set_size(w, h);
                l.commit();
            }
            Role::Window(win) => win.commit(),
            // (a lock surface is sized by the compositor: only shown again)
            Role::Lock(_) => self.wl.commit(),
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
        self.needs_redraw.set(true);
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

    /// A surface with its role: its EGL window and renderer, made the next component's window
    fn adopt(&mut self, wl: WlSurface, layer: Role, viewport: Option<WpViewport>, fractional: Option<WpFractionalScaleV1>, namespace: &str, output: String, size: (u32, u32)) -> Rc<Surface> {
        let gl = GlSurface::new(self.egl.clone(), WlProxy::id(&wl)).expect("EGL window surface");
        let damage = gl.damage.clone();
        // (FemtoVG loads GL as it's made: the context must be current)
        slint::platform::femtovg_renderer::OpenGLInterface::ensure_current(&gl).expect("EGL make current");
        let surface = Rc::new_cyclic(|weak: &Weak<Surface>| Surface {
            window: slint::Window::new(weak.clone() as Weak<dyn WindowAdapter>),
            renderer: crate::damage::Renderer { femto: FemtoVGRenderer::new(gl).expect("FemtoVG renderer"), damage: Default::default() },
            damage,
            egl: self.egl.clone(),
            rendered: Cell::new(0),
            enters: Cell::new(0),
            presented: Cell::new(0),
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
            mapped: Cell::new(false),
            hidden: Cell::new(false),
            retired: Cell::new(false),
            ime: RefCell::new(None),
            stats: RefCell::new(Default::default()),
            cursor: Cell::new(Shape::Default),
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

    fn find(&self, wl: &WlSurface) -> Option<Rc<Surface>> {
        self.surfaces.iter().find(|s| &s.wl == wl).cloned()
    }

    /// After each event loop turn: draw what changed
    pub fn render_all(&mut self) {
        for s in self.surfaces.clone() {
            s.render(&self.qh);
        }
        if IME_DIRTY.replace(false) {
            self.sync_text_input();
        }
    }

    pub fn wants_frame_soon(&self) -> bool {
        self.surfaces.iter().any(|s| s.needs_redraw.get() && s.configured.get() && !s.frame_pending.get())
    }

    /// An animation runs but no frame callback will wake the loop: tick anyway
    pub fn animating_unpaced(&self) -> bool {
        !self.surfaces.iter().any(|s| s.frame_pending.get()) && self.surfaces.iter().any(|s| s.window.has_active_animations())
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
        let name = self.outputs.info(&output).and_then(|i| i.name).unwrap_or_default();
        self.each_part(|p, st| p.output_added(st, &output, &name));
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, output: WlOutput) {
        if let Some(name) = self.outputs.info(&output).and_then(|i| i.name) {
            self.each_part(|p, st| p.output_removed(st, &name));
        }
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
fn key_text(e: &KeyEvent, release: bool) -> Option<slint::SharedString> {
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
        _ => {
            let t = if release { None } else { e.utf8.clone() };
            let t = t.or_else(|| e.keysym.key_char().map(String::from))?;
            return (!t.is_empty() && !t.chars().all(char::is_control)).then(|| t.into());
        }
    };
    Some(k.into())
}

impl State {
    fn key(&mut self, e: KeyEvent, kind: KeyKind) {
        let Some(s) = self.keyboard_on.clone() else { return };
        let Some(text) = key_text(&e, matches!(kind, KeyKind::Release)) else { return };
        if let Some(i) = MODS.iter().position(|m| text == slint::SharedString::from(*m)) {
            self.mods_sent[i] = !matches!(kind, KeyKind::Release);
        }
        s.window.dispatch_event(match kind {
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
            s.window.dispatch_event(WindowEvent::WindowActiveChanged(true));
        }
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, wl: &WlSurface, _: u32) {
        if let Some(s) = self.find(wl) {
            s.window.dispatch_event(WindowEvent::WindowActiveChanged(false));
            self.each_part(|p, st| p.keyboard_left(st, &s));
        }
        self.keyboard_on = None;
    }
    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, _: u32, e: KeyEvent) {
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
                s.window.dispatch_event(if want { WindowEvent::KeyPressed { text } } else { WindowEvent::KeyReleased { text } });
            }
        }
    }
}

impl PointerHandler for State {
    fn pointer_frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlPointer, events: &[PointerEvent]) {
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
                    s.window.dispatch_event(WindowEvent::PointerMoved { position });
                }
                PointerEventKind::Leave { .. } => {
                    if crate::env::is_set("DEBUG") {
                        eprintln!("debug: pointer leaves {}", s.namespace);
                    }
                    s.window.dispatch_event(WindowEvent::PointerExited);
                    self.pointer_on = None;
                }
                PointerEventKind::Motion { .. } => s.window.dispatch_event(WindowEvent::PointerMoved { position }),
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
                    }
                    if matches!(e.kind, PointerEventKind::Press { .. }) {
                        s.window.dispatch_event(WindowEvent::PointerPressed { position, button });
                    } else {
                        s.window.dispatch_event(WindowEvent::PointerReleased { position, button });
                    }
                }
                PointerEventKind::Axis { horizontal, vertical, .. } => {
                    let d = |a: &smithay_client_toolkit::seat::pointer::AxisScroll| -(a.absolute as f32);
                    s.window.dispatch_event(WindowEvent::PointerScrolled { position, delta_x: d(horizontal), delta_y: d(vertical) });
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
                    s.window.dispatch_event(WindowEvent::internal(e));
                }
                let (text, b, en) = preedit.unwrap_or_default();
                let e = InternalKeyEvent {
                    event_type: KeyEventType::UpdateComposition,
                    preedit_text: text.into(),
                    // (a cursor, not a selection, when both ends meet)
                    preedit_selection: (b >= 0 && en > b).then_some(b..en),
                    ..Default::default()
                };
                s.window.dispatch_event(WindowEvent::internal(e));
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
delegate_seat!(State);
delegate_keyboard!(State);
delegate_pointer!(State);
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
