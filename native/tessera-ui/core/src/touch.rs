//! Touch (wl_touch) for every surface, as pointer input: what Qt does for the
//! QML's items that take no touch themselves (mouse events synthesized from
//! the first finger), so every Slint TouchArea, Flickable and drag works by
//! touch without each part binding its own.
//!
//! One finger (the first down; others are ignored until it lifts):
//!   - a tap (lifted within the slop, before the long press): a left click
//!     where it went down;
//!   - moved past the slop (10 px): the left button goes down where the finger
//!     went down and follows it (drags, Flickable scrolls and flings), up
//!     where it lifts;
//!   - held still for 500 ms: a right click there (the QML's pressAndHold
//!     menus; the context menus Rust parts open on the right button), then
//!     nothing more until it lifts;
//!   - cancelled by the compositor (a gesture took it): the button, if down,
//!     comes up outside, so nothing is clicked.
//! The pointer seen by Slint moves to the finger first, so hovers show.

use crate::wayland::State;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::LogicalPosition;
use smithay_client_toolkit::reexports::client::protocol::{wl_surface::WlSurface, wl_touch::WlTouch};
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::seat::touch::TouchHandler;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

/// How far a finger moves before it's a drag (logical px)
pub const SLOP: f64 = 10.0;
/// How long a finger rests before it's a long press
pub const HOLD: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    /// down, not moved past the slop, not held long enough
    Waiting,
    /// the left button is down, following the finger
    Dragging,
    /// the long press fired: the rest of this touch does nothing
    Held,
}

struct Finger {
    id: i32,
    surface: Rc<crate::wayland::Surface>,
    start: (f64, f64),
    at: (f64, f64),
    phase: Phase,
    /// which touch this is (a long-press timer from an earlier touch is ignored)
    seq: u64,
}

thread_local! {
    static FINGER: RefCell<Option<Finger>> = const { RefCell::new(None) };
    static SEQ: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static TOUCH: RefCell<Option<WlTouch>> = const { RefCell::new(None) };
}

/// The seat has touch: listen (wayland.rs's SeatHandler)
pub fn bind(st: &mut State, qh: &QueueHandle<State>, seat: &smithay_client_toolkit::reexports::client::protocol::wl_seat::WlSeat) {
    if TOUCH.with(|t| t.borrow().is_some()) {
        return;
    }
    if let Ok(t) = smithay_client_toolkit::seat::SeatHandler::seat_state(st).get_touch(qh, seat) {
        TOUCH.with(|x| *x.borrow_mut() = Some(t));
    }
}

/// The seat lost touch
pub fn unbind() {
    FINGER.with(|f| f.borrow_mut().take());
    if let Some(t) = TOUCH.with(|t| t.borrow_mut().take()) {
        t.release();
    }
}

fn pos(p: (f64, f64)) -> LogicalPosition {
    LogicalPosition::new(p.0 as f32, p.1 as f32)
}

fn send(s: &crate::wayland::Surface, e: WindowEvent) {
    s.window().dispatch_event(e);
}

/// What a touch sequence does, from its events (pure: tested below)
#[derive(Debug, PartialEq)]
pub enum Act {
    Nothing,
    /// the left button down at the start, the pointer then at the finger
    StartDrag,
    Move,
    Tap,
    EndDrag,
}

/// The step a finger's event makes: (new phase, what to send)
fn step(phase: Phase, start: (f64, f64), at: (f64, f64), up: bool) -> (Phase, Act) {
    match (phase, up) {
        (Phase::Waiting, false) if (at.0 - start.0).hypot(at.1 - start.1) > SLOP => (Phase::Dragging, Act::StartDrag),
        (Phase::Waiting, false) => (Phase::Waiting, Act::Nothing),
        (Phase::Waiting, true) => (Phase::Waiting, Act::Tap),
        (Phase::Dragging, false) => (Phase::Dragging, Act::Move),
        (Phase::Dragging, true) => (Phase::Dragging, Act::EndDrag),
        (Phase::Held, _) => (Phase::Held, Act::Nothing),
    }
}

fn perform(f: &Finger, act: Act) {
    let s = &f.surface;
    match act {
        Act::Nothing => {}
        Act::StartDrag => {
            send(s, WindowEvent::PointerMoved { position: pos(f.start) });
            send(s, WindowEvent::PointerPressed { position: pos(f.start), button: PointerEventButton::Left });
            send(s, WindowEvent::PointerMoved { position: pos(f.at) });
        }
        Act::Move => send(s, WindowEvent::PointerMoved { position: pos(f.at) }),
        Act::Tap => {
            send(s, WindowEvent::PointerPressed { position: pos(f.start), button: PointerEventButton::Left });
            send(s, WindowEvent::PointerReleased { position: pos(f.start), button: PointerEventButton::Left });
            send(s, WindowEvent::PointerExited);
        }
        Act::EndDrag => {
            send(s, WindowEvent::PointerReleased { position: pos(f.at), button: PointerEventButton::Left });
            send(s, WindowEvent::PointerExited);
        }
    }
}

impl TouchHandler for State {
    fn down(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlTouch, _: u32, _: u32, surface: WlSurface, id: i32, position: (f64, f64)) {
        if FINGER.with(|f| f.borrow().is_some()) {
            return; // (a second finger: only the first is the pointer)
        }
        let Some(s) = self.surface_for(&surface) else { return };
        if crate::env::is_set("DEBUG") {
            eprintln!("debug: touch down on {} at {position:?}", s.namespace);
        }
        let seq = SEQ.with(|q| {
            q.set(q.get() + 1);
            q.get()
        });
        send(&s, WindowEvent::PointerMoved { position: pos(position) });
        FINGER.with(|f| *f.borrow_mut() = Some(Finger { id, surface: s, start: position, at: position, phase: Phase::Waiting, seq }));
        // the long press: a right click where the finger rests
        let timer = smithay_client_toolkit::reexports::calloop::timer::Timer::from_duration(HOLD);
        let _ = self.loop_handle.insert_source(timer, move |_, _, _state: &mut State| {
            FINGER.with(|f| {
                let mut f = f.borrow_mut();
                if let Some(fg) = f.as_mut().filter(|fg| fg.seq == seq && fg.phase == Phase::Waiting) {
                    fg.phase = Phase::Held;
                    let s = &fg.surface;
                    send(s, WindowEvent::PointerPressed { position: pos(fg.start), button: PointerEventButton::Right });
                    send(s, WindowEvent::PointerReleased { position: pos(fg.start), button: PointerEventButton::Right });
                }
            });
            smithay_client_toolkit::reexports::calloop::timer::TimeoutAction::Drop
        });
    }

    fn up(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlTouch, _: u32, _: u32, id: i32) {
        let Some(f) = FINGER.with(|f| f.borrow_mut().take_if(|f| f.id == id)) else { return };
        let (_, act) = step(f.phase, f.start, f.at, true);
        perform(&f, act);
        if f.phase == Phase::Held {
            send(&f.surface, WindowEvent::PointerExited);
        }
    }

    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlTouch, _: u32, id: i32, position: (f64, f64)) {
        FINGER.with(|f| {
            let mut f = f.borrow_mut();
            let Some(fg) = f.as_mut().filter(|fg| fg.id == id) else { return };
            fg.at = position;
            let (phase, act) = step(fg.phase, fg.start, fg.at, false);
            fg.phase = phase;
            perform(fg, act);
        });
    }

    fn shape(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlTouch, _: i32, _: f64, _: f64) {}

    fn orientation(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlTouch, _: i32, _: f64) {}

    fn cancel(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlTouch) {
        if let Some(f) = FINGER.with(|f| f.borrow_mut().take()) {
            if f.phase == Phase::Dragging {
                // (released far outside: nothing under the finger is clicked)
                send(&f.surface, WindowEvent::PointerReleased { position: LogicalPosition::new(-1e5, -1e5), button: PointerEventButton::Left });
            }
            send(&f.surface, WindowEvent::PointerExited);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taps_drags_and_holds() {
        // lifted where it went down: a tap
        assert_eq!(step(Phase::Waiting, (0.0, 0.0), (3.0, 4.0), true), (Phase::Waiting, Act::Tap));
        // moved a little: still waiting
        assert_eq!(step(Phase::Waiting, (0.0, 0.0), (6.0, 6.0), false), (Phase::Waiting, Act::Nothing));
        // past the slop: the drag starts, then follows, then ends
        assert_eq!(step(Phase::Waiting, (0.0, 0.0), (0.0, 11.0), false), (Phase::Dragging, Act::StartDrag));
        assert_eq!(step(Phase::Dragging, (0.0, 0.0), (0.0, 40.0), false), (Phase::Dragging, Act::Move));
        assert_eq!(step(Phase::Dragging, (0.0, 0.0), (0.0, 40.0), true), (Phase::Dragging, Act::EndDrag));
        // after a long press, nothing more
        assert_eq!(step(Phase::Held, (0.0, 0.0), (50.0, 0.0), false), (Phase::Held, Act::Nothing));
        assert_eq!(step(Phase::Held, (0.0, 0.0), (50.0, 0.0), true), (Phase::Held, Act::Nothing));
    }
}
