//! Typing for the on-screen keyboard: a zwp_virtual_keyboard_v1 on a
//! Wayland connection of its own (the compositor tessera-ui runs in, so in
//! the sandbox only the arena's windows get the keys), on its own thread.
//! The QML ran `wtype` per key; this is the same protocol without a process
//! per key: the keymap once (keys::keymap()), then per stroke the modifiers,
//! the key down and up, the modifiers back.

use super::keys::{keymap, Stroke};
use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use std::sync::mpsc;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_registry::WlRegistry, wl_seat::WlSeat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1, zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

struct Q;

impl Dispatch<WlRegistry, GlobalListContents> for Q {
    fn event(_: &mut Self, _: &WlRegistry, _: <WlRegistry as Proxy>::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlSeat, ()> for Q {
    fn event(_: &mut Self, _: &WlSeat, _: <WlSeat as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for Q {
    fn event(_: &mut Self, _: &ZwpVirtualKeyboardManagerV1, _: <ZwpVirtualKeyboardManagerV1 as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ZwpVirtualKeyboardV1, ()> for Q {
    fn event(_: &mut Self, _: &ZwpVirtualKeyboardV1, _: <ZwpVirtualKeyboardV1 as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

/// The keymap in a sealed memory file, as the protocol wants it
fn keymap_fd() -> Option<(OwnedFd, u32)> {
    let mut bytes = keymap().into_bytes();
    bytes.push(0);
    unsafe {
        let fd = libc::memfd_create(c"tessera-osk-keymap".as_ptr(), libc::MFD_CLOEXEC);
        if fd < 0 {
            return None;
        }
        let owned = OwnedFd::from_raw_fd(fd);
        let mut off = 0;
        while off < bytes.len() {
            let n = libc::write(fd, bytes[off..].as_ptr() as *const libc::c_void, bytes.len() - off);
            if n <= 0 {
                return None;
            }
            off += n as usize;
        }
        Some((owned, bytes.len() as u32))
    }
}

fn now_ms(start: std::time::Instant) -> u32 {
    start.elapsed().as_millis() as u32
}

/// The typist: strokes sent here are typed in order. Made on first use
pub struct Typist {
    tx: mpsc::Sender<Stroke>,
}

impl Typist {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<Stroke>();
        std::thread::Builder::new()
            .name("tessera-osk".into())
            .spawn(move || {
                let Ok(conn) = Connection::connect_to_env() else {
                    eprintln!("tessera-ui: osk: no Wayland display to type into");
                    return;
                };
                let Ok((globals, mut queue)) = registry_queue_init::<Q>(&conn) else { return };
                let qh = queue.handle();
                let (Ok(seat), Ok(mgr)) = (globals.bind::<WlSeat, _, _>(&qh, 1..=7, ()), globals.bind::<ZwpVirtualKeyboardManagerV1, _, _>(&qh, 1..=1, ())) else {
                    eprintln!("tessera-ui: osk: the compositor has no virtual keyboard (zwp_virtual_keyboard_manager_v1)");
                    return;
                };
                let kb = mgr.create_virtual_keyboard(&seat, &qh, ());
                let Some((fd, size)) = keymap_fd() else { return };
                kb.keymap(1, fd.as_fd(), size); // (1: xkb_v1)
                let _ = queue.roundtrip(&mut Q);
                let start = std::time::Instant::now();
                while let Ok(s) = rx.recv() {
                    kb.modifiers(s.depressed, 0, 0, 0);
                    kb.key(now_ms(start), s.code, 1);
                    kb.key(now_ms(start), s.code, 0);
                    kb.modifiers(0, 0, 0, 0);
                    if conn.flush().is_err() {
                        return;
                    }
                    let _ = queue.dispatch_pending(&mut Q);
                }
                kb.destroy();
                let _ = conn.flush();
            })
            .expect("osk thread");
        Self { tx }
    }

    pub fn type_stroke(&self, s: Stroke) {
        let _ = self.tx.send(s);
    }
}
