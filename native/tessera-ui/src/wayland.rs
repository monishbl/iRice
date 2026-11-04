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
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Instant;

// ── One layer surface = one Slint window ────────────────────────────────────

pub struct Surface {
    window: slint::Window,
    renderer: FemtoVGRenderer,
    pub wl: WlSurface,
    pub layer: LayerSurface,
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
    ime: RefCell<Option<slint_ime::Props>>, // what the focused text field wants from the input method
    pub stats: RefCell<crate::stats::Frames>,
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
    }
    fn render(&self, qh: &QueueHandle<State>) {
        if !self.configured.get() || self.frame_pending.get() || !self.needs_redraw.get() {
            return;
        }
        self.needs_redraw.set(false);
        self.wl.frame(qh, self.wl.clone());
        let t = Instant::now();
        if let Err(e) = self.renderer.render() {
            eprintln!("tessera-ui: render failed: {e}");
        }
        self.stats.borrow_mut().rendered(t.elapsed());
        self.frame_pending.set(true);
        self.mapped.set(true);
        // (no "redraw while animations run" here: Slint's animation clock is
        // global, so every window would redraw while any one animates; an
        // animated property marks only its own window dirty)
    }
    /// Unmap (a null buffer); `show` asks for a new configure
    pub fn hide(&self) {
        self.wl.attach(None, 0, 0);
        self.wl.commit();
        self.configured.set(false);
        self.frame_pending.set(false);
        self.mapped.set(false);
    }
    pub fn show(&self, w: u32, h: u32) {
        self.layer.set_size(w, h);
        self.layer.commit();
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
}

thread_local! {
    // Component::new() asks the platform for a window adapter: this one
    static NEXT: RefCell<Option<Rc<Surface>>> = const { RefCell::new(None) };
    static IME_DIRTY: Cell<bool> = const { Cell::new(false) };
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

pub struct State {
    registry: RegistryState,
    pub outputs: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
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
        Self {
            registry: RegistryState::new(globals),
            outputs: OutputState::new(globals, &qh),
            compositor: CompositorState::bind(globals, &qh).expect("wl_compositor"),
            layer_shell: LayerShell::bind(globals, &qh).expect("this compositor has no wlr-layer-shell"),
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
        let gl = GlSurface::new(self.egl.clone(), WlProxy::id(&wl)).expect("EGL window surface");
        // (FemtoVG loads GL as it's made: the context must be current)
        slint::platform::femtovg_renderer::OpenGLInterface::ensure_current(&gl).expect("EGL make current");
        let surface = Rc::new_cyclic(|weak: &Weak<Surface>| Surface {
            window: slint::Window::new(weak.clone() as Weak<dyn WindowAdapter>),
            renderer: FemtoVGRenderer::new(gl).expect("FemtoVG renderer"),
            wl,
            layer,
            viewport,
            _fractional: fractional,
            namespace: spec.namespace.to_string(),
            output: spec.output.map(|o| o.1.to_string()).unwrap_or_default(),
            logical: Cell::new(spec.size),
            scale: Cell::new(1.0),
            configured: Cell::new(false),
            needs_redraw: Cell::new(true),
            frame_pending: Cell::new(false),
            mapped: Cell::new(false),
            ime: RefCell::new(None),
            stats: RefCell::new(Default::default()),
        });
        NEXT.with(|n| *n.borrow_mut() = Some(surface.clone()));
        self.surfaces.push(surface.clone());
        surface
    }

    pub fn drop_surface(&mut self, s: &Rc<Surface>) {
        self.surfaces.retain(|x| !Rc::ptr_eq(x, s));
        if self.pointer_on.as_ref().is_some_and(|p| Rc::ptr_eq(p, s)) {
            self.pointer_on = None;
        }
        if self.keyboard_on.as_ref().is_some_and(|p| Rc::ptr_eq(p, s)) {
            self.keyboard_on = None;
        }
    }

    /// Where a surface takes the pointer: these rectangles (logical px), or
    /// everywhere (None). A surface taller than what it shows (the menu
    /// bar, with room below it for the island) lets clicks through elsewhere
    pub fn set_input_region(&self, s: &Surface, rects: Option<&[(i32, i32, i32, i32)]>) {
        match rects {
            None => s.wl.set_input_region(None),
            Some(list) => {
                let Ok(region) = smithay_client_toolkit::compositor::Region::new(&self.compositor) else { return };
                for &(x, y, w, h) in list {
                    if w > 0 && h > 0 {
                        region.add(x, y, w, h);
                    }
                }
                s.wl.set_input_region(Some(region.wl_region()));
            }
        }
        s.wl.commit();
    }

    pub fn keyboard_surface(&self) -> Option<Rc<Surface>> {
        self.keyboard_on.clone()
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
        if let Some(s) = self.surfaces.iter().find(|s| &s.layer == layer).cloned() {
            self.each_part(|p, st| p.surface_closed(st, &s));
        }
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface, c: LayerSurfaceConfigure, _: u32) {
        let Some(s) = self.surfaces.iter().find(|s| &s.layer == layer).cloned() else { return };
        if crate::env::is_set("DEBUG") {
            eprintln!("debug: configure {} {:?} scale {}", s.namespace, c.new_size, s.scale.get());
        }
        let (w, h) = s.logical.get();
        let size = (if c.new_size.0 > 0 { c.new_size.0 } else { w }, if c.new_size.1 > 0 { c.new_size.1 } else { h });
        s.logical.set(size);
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
        Keysym::Shift_L | Keysym::Shift_R => Key::Shift,
        Keysym::Control_L | Keysym::Control_R => Key::Control,
        Keysym::Alt_L | Keysym::Alt_R => Key::Alt,
        Keysym::Super_L | Keysym::Super_R => Key::Meta,
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
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, _: u32, _: Modifiers, _: RawModifiers, _: u32) {}
}

impl PointerHandler for State {
    fn pointer_frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlPointer, events: &[PointerEvent]) {
        for e in events {
            let Some(s) = self.find(&e.surface) else { continue };
            let position = LogicalPosition::new(e.position.0 as f32, e.position.1 as f32);
            match &e.kind {
                PointerEventKind::Enter { serial } => {
                    self.set_cursor(*serial, Shape::Default);
                    self.pointer_on = Some(s.clone());
                    s.window.dispatch_event(WindowEvent::PointerMoved { position });
                }
                PointerEventKind::Leave { .. } => {
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

delegate_compositor!(State);
delegate_output!(State);
delegate_layer!(State);
delegate_seat!(State);
delegate_keyboard!(State);
delegate_pointer!(State);
delegate_registry!(State);

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}
