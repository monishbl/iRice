//! The touchscreen swipe that reveals the dock (DockLogic.qml's DragHandler
//! for TouchScreen: a swipe up of more than 24 px while it auto-hides calls
//! touchReveal, and touchHold keeps it up 4 s).
//!
//! Taps, drags and long presses on every surface (the dock's icons, its
//! menu, the Downloads stack) are core's: core/src/touch.rs turns the first
//! finger into left press / move / release (a 500 ms hold into a right
//! click) on the touched window, as Qt's mouse made from touches. So this
//! file only watches the finger on the dock window for the swipe; it sends
//! no pointer events itself (one touch reaches the dock once, through
//! core). Core can't do the swipe: what it means (reveal while hidden, the
//! 4 s hold) is the dock's.
//!
//! It binds its own wl_touch (a second wl_registry and wl_seat on the same
//! connection, whose user data types are this crate's so the Dispatch impls
//! live here): the compositor sends a touch to every wl_touch of the seat,
//! core's and this one, so watching here takes nothing from core.

use smithay_client_toolkit::reexports::client::protocol::wl_registry::{self, WlRegistry};
use smithay_client_toolkit::reexports::client::protocol::wl_seat::{self, Capability, WlSeat};
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::protocol::wl_touch::{self, WlTouch};
use smithay_client_toolkit::reexports::client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};
use std::cell::RefCell;
use tessera_core::wayland::State;

/// What the dock window hears (its part's message)
pub enum Touch {
    Down(String, f32, f32),
    Move(String, f32, f32),
    Up(String),
    Cancel(String),
}

pub struct TouchRegistry;
pub struct TouchSeat;
pub struct TouchData;

#[derive(Default)]
struct Track {
    asked: bool,
    touch: Option<WlTouch>,
    /// the first finger: its id and the dock window's output it went down on
    id: Option<i32>,
    output: Option<String>,
}

thread_local! {
    static T: RefCell<Track> = RefCell::new(Track::default());
}

/// Bind the seat's touch (once), on the connection `any` lives on
pub fn bind(st: &State, any: &WlSurface) {
    if T.with(|t| std::mem::replace(&mut t.borrow_mut().asked, true)) {
        return;
    }
    let Some(backend) = any.backend().upgrade() else { return };
    let conn = Connection::from_backend(backend);
    let _registry = conn.display().get_registry(&st.qh, TouchRegistry);
}

impl Dispatch<WlRegistry, TouchRegistry> for State {
    fn event(_: &mut Self, registry: &WlRegistry, event: wl_registry::Event, _: &TouchRegistry, _: &Connection, qh: &QueueHandle<Self>) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            if interface == WlSeat::interface().name {
                let _seat: WlSeat = registry.bind(name, version.min(7), qh, TouchSeat);
            }
        }
    }
}

impl Dispatch<WlSeat, TouchSeat> for State {
    fn event(_: &mut Self, seat: &WlSeat, event: wl_seat::Event, _: &TouchSeat, _: &Connection, qh: &QueueHandle<Self>) {
        let wl_seat::Event::Capabilities { capabilities: WEnum::Value(caps) } = event else { return };
        T.with(|t| {
            let mut t = t.borrow_mut();
            if caps.contains(Capability::Touch) && t.touch.is_none() {
                t.touch = Some(seat.get_touch(qh, TouchData));
            } else if !caps.contains(Capability::Touch) {
                if let Some(old) = t.touch.take() {
                    if old.version() >= 3 {
                        old.release();
                    }
                }
            }
        });
    }
}

/// The dock window a wl_surface is (only the dock window: the menu and the
/// stack take touches through core alone): its output
fn dock_output(st: &State, s: &WlSurface) -> Option<String> {
    st.surfaces.iter().find(|x| x.wl == *s && x.namespace == super::NS).map(|x| x.output.clone())
}

impl Dispatch<WlTouch, TouchData> for State {
    fn event(st: &mut Self, _: &WlTouch, event: wl_touch::Event, _: &TouchData, _: &Connection, _: &QueueHandle<Self>) {
        let to = st.shell.sender(super::NAME);
        match event {
            wl_touch::Event::Down { surface, id, x, y, .. } => {
                if T.with(|t| t.borrow().id.is_some()) {
                    return; // (one finger)
                }
                let Some(o) = dock_output(st, &surface) else { return };
                T.with(|t| {
                    let mut t = t.borrow_mut();
                    t.id = Some(id);
                    t.output = Some(o.clone());
                });
                to(Box::new(Touch::Down(o, x as f32, y as f32)));
            }
            wl_touch::Event::Motion { id, x, y, .. } => {
                let Some(o) = T.with(|t| t.borrow().output.clone().filter(|_| t.borrow().id == Some(id))) else { return };
                to(Box::new(Touch::Move(o, x as f32, y as f32)));
            }
            wl_touch::Event::Up { id, .. } => {
                let Some(o) = T.with(|t| {
                    let mut t = t.borrow_mut();
                    if t.id != Some(id) {
                        return None;
                    }
                    t.id = None;
                    t.output.take()
                }) else {
                    return;
                };
                to(Box::new(Touch::Up(o)));
            }
            wl_touch::Event::Cancel => {
                let Some(o) = T.with(|t| {
                    let mut t = t.borrow_mut();
                    t.id = None;
                    t.output.take()
                }) else {
                    return;
                };
                to(Box::new(Touch::Cancel(o)));
            }
            _ => {}
        }
    }
}

/// DragHandler: a swipe up of more than this reveals the dock
pub const SWIPE_UP: f32 = 24.0;
/// touchHold: how long a swipe keeps it up
pub const HOLD_MS: u64 = 4000;

/// From where a finger went down and where it is: a swipe up as soon as it's
/// more than 24 px above (DragHandler's translation.y < -24)
pub fn swiped_up(from: (f32, f32), to: (f32, f32)) -> bool {
    to.1 - from.1 < -SWIPE_UP
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_swipe_up_past_24_px_reveals() {
        assert!(!swiped_up((900.0, 120.0), (905.0, 100.0)));
        assert!(swiped_up((900.0, 120.0), (905.0, 95.0)));
        assert!(!swiped_up((900.0, 120.0), (900.0, 160.0))); // (down is no swipe)
    }
}
