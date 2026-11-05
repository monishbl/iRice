//! Drag and drop (wl_data_device), for the surfaces that take drops or let
//! things be dragged out of them (the shelf: shell/modules/shelf). The QML
//! shell gets this from Qt (DropArea, Drag.Automatic); here it's the seat's
//! data device:
//!   - a surface takes drops once its part says so (`State::accept_drops`):
//!     a drag over it reaches the part as `Part::drag` (Enter, Motion,
//!     Leave), and a drop as `Drag::Drop` with the files' URIs and the text,
//!     read for it (text/uri-list and text/plain, off the offer's pipes, on
//!     the event loop). During `drag` the part can `refuse_drop()` (not
//!     here: the drag shows it can't drop). It's a copy, as the QML accepts.
//!   - `State::start_drag` drags data out of a surface from its latest
//!     button press (the button still held): (mime type, bytes) pairs, sent
//!     to whoever takes it. Its own drag coming back over the surface says
//!     `ours`, as Qt's `drag.source`.
//! Slint hears nothing of a drag over its window (the pointer is the
//! compositor's meanwhile): the part shows it from these events.

use crate::wayland::{State, Surface};
use smithay_client_toolkit::{
    data_device_manager::{
        data_device::{DataDevice, DataDeviceHandler},
        data_offer::{DataOfferHandler, DragOffer},
        data_source::{DataSourceHandler, DragSource},
        DataDeviceManagerState, WritePipe,
    },
    delegate_data_device,
    reexports::client::{
        globals::GlobalList,
        protocol::{wl_data_device::WlDataDevice, wl_data_device_manager::DndAction, wl_data_source::WlDataSource, wl_seat::WlSeat},
        Connection, QueueHandle,
    },
};
use std::cell::RefCell;
use std::io::{Read, Write};
use std::rc::{Rc, Weak};

/// What a part hears of a drag over one of its surfaces (logical px)
#[derive(Clone, Debug, PartialEq)]
pub enum Drag {
    /// a drag came over the surface; `ours`: one this process started
    Enter { x: f32, y: f32, mimes: Vec<String>, ours: bool },
    Motion { x: f32, y: f32 },
    Leave,
    /// let go over it: the file URIs (text/uri-list, "file:///…") and the text (text/plain)
    Drop { x: f32, y: f32, uris: Vec<String>, text: String },
}

/// The kinds of data taken, best first: files, then text
const URIS: &str = "text/uri-list";
const TEXTS: &[&str] = &["text/plain;charset=utf-8", "text/plain", "UTF8_STRING", "TEXT", "STRING"];

/// The mime type to accept from what a drag offers (None: nothing taken here)
pub fn pick(mimes: &[String]) -> Option<String> {
    if mimes.iter().any(|m| m == URIS) {
        return Some(URIS.into());
    }
    TEXTS.iter().find(|t| mimes.iter().any(|m| m == *t)).map(|t| t.to_string())
}

/// The text type to read beside the URIs, if any
pub fn text_type(mimes: &[String]) -> Option<String> {
    TEXTS.iter().find(|t| mimes.iter().any(|m| m == *t)).map(|t| t.to_string())
}

/// text/uri-list: one URI a line, `#` comments, CRLF line ends
pub fn uri_list(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

/// The data device's own state, kept in `State`
#[derive(Default)]
pub struct Dnd {
    manager: Option<DataDeviceManagerState>,
    device: Option<DataDevice>,
    /// surfaces that take drops
    targets: Vec<Weak<Surface>>,
    /// the surface a drag is over now (one of the targets), and what it offers
    over: Option<(Rc<Surface>, Vec<String>)>,
    /// a part said "not here" during its last `drag` call
    refused: bool,
    /// the drag this process started, and what it carries
    source: Option<(DragSource, Vec<(String, Vec<u8>)>)>,
    /// the latest button press: its serial and surface (a drag starts from it)
    press: Option<(u32, Weak<Surface>)>,
}

impl Dnd {
    pub fn new(globals: &GlobalList, qh: &QueueHandle<State>) -> Self {
        Self { manager: DataDeviceManagerState::bind(globals, qh).ok(), ..Default::default() }
    }

    /// The seat's pointer came: its data device (once)
    pub fn seat(&mut self, seat: &WlSeat, qh: &QueueHandle<State>) {
        if self.device.is_none() {
            if let Some(m) = &self.manager {
                self.device = Some(m.get_data_device(qh, seat));
            }
        }
    }

    /// A button went down on a surface (wayland.rs: the pointer)
    pub fn pressed(&mut self, serial: u32, surface: &Rc<Surface>) {
        self.press = Some((serial, Rc::downgrade(surface)));
    }

    fn target(&self, s: &Rc<Surface>) -> bool {
        self.targets.iter().any(|t| t.upgrade().is_some_and(|t| Rc::ptr_eq(&t, s)))
    }
}

impl State {
    /// This surface takes drops (a copy of files or text), or no longer
    pub fn accept_drops(&mut self, surface: &Rc<Surface>, on: bool) {
        self.dnd.targets.retain(|t| t.upgrade().is_some_and(|t| !Rc::ptr_eq(&t, surface)));
        if on {
            self.dnd.targets.push(Rc::downgrade(surface));
        }
    }

    /// During `Part::drag`: not here (the drag can't drop now, or the drop isn't taken)
    pub fn refuse_drop(&mut self) {
        self.dnd.refused = true;
    }

    /// Drag data out of `surface`, from its latest button press (still held):
    /// (mime type, bytes) pairs. False if there's no press on it to start from.
    pub fn start_drag(&mut self, surface: &Rc<Surface>, offers: Vec<(String, Vec<u8>)>) -> bool {
        let (Some(m), Some(dev)) = (&self.dnd.manager, &self.dnd.device) else { return false };
        let Some((serial, on)) = &self.dnd.press else { return false };
        if !on.upgrade().is_some_and(|s| Rc::ptr_eq(&s, surface)) || offers.is_empty() {
            return false;
        }
        let src = m.create_drag_and_drop_source(&self.qh, offers.iter().map(|o| o.0.clone()), DndAction::Copy | DndAction::Move | DndAction::Ask);
        src.start_drag(dev, &surface.wl, None, *serial);
        self.dnd.source = Some((src, offers));
        // (the compositor has the pointer now: Slint's press would never end)
        let w = surface.window();
        let p = slint::LogicalPosition::new(-1.0, -1.0);
        w.dispatch_event(slint::platform::WindowEvent::PointerReleased { position: p, button: slint::platform::PointerEventButton::Left });
        w.dispatch_event(slint::platform::WindowEvent::PointerExited);
        true
    }

    /// Tell the parts; then say to the source whether it can drop here
    fn tell_drag(&mut self, s: &Rc<Surface>, ev: Drag, offer: Option<&DragOffer>) {
        self.dnd.refused = false;
        self.each_part(|p, st| p.drag(st, s, &ev));
        let Some(offer) = offer else { return };
        let mime = self.dnd.over.as_ref().and_then(|(_, m)| pick(m));
        match mime.filter(|_| !self.dnd.refused) {
            Some(m) => {
                offer.accept_mime_type(offer.serial, Some(m));
                offer.set_actions(DndAction::Copy, DndAction::Copy);
            }
            None => {
                offer.accept_mime_type(offer.serial, None);
                offer.set_actions(DndAction::empty(), DndAction::empty());
            }
        }
    }

    fn drag_offer(&self, device: &WlDataDevice) -> Option<DragOffer> {
        use smithay_client_toolkit::reexports::client::Proxy;
        device.data::<smithay_client_toolkit::data_device_manager::data_device::DataDeviceData>()?.drag_offer()
    }

    /// Read the dropped data (URIs and text) off the offer's pipes on the
    /// event loop, then tell the parts
    fn read_drop(&mut self, s: Rc<Surface>, offer: DragOffer, mimes: Vec<String>) {
        struct Pending {
            uris: Option<Vec<u8>>,
            text: Option<Vec<u8>>,
            want: usize,
            done: usize,
        }
        let mut kinds: Vec<(bool, String)> = Vec::new();
        if mimes.iter().any(|m| m == URIS) {
            kinds.push((true, URIS.into()));
        }
        if let Some(t) = text_type(&mimes) {
            kinds.push((false, t));
        }
        let (x, y) = (offer.x as f32, offer.y as f32);
        let pending = Rc::new(RefCell::new(Pending { uris: None, text: None, want: kinds.len(), done: 0 }));
        let offer = Rc::new(offer);
        let finish = {
            let (pending, offer, s) = (pending.clone(), offer.clone(), s.clone());
            Rc::new(move |st: &mut State| {
                let p = pending.borrow();
                if p.done < p.want {
                    return;
                }
                let ev = Drag::Drop { x, y, uris: p.uris.as_deref().map(uri_list).unwrap_or_default(), text: p.text.as_deref().map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default() };
                drop(p);
                offer.finish();
                offer.destroy();
                st.dnd.refused = false;
                st.each_part(|part, st| part.drag(st, &s, &ev));
            })
        };
        if kinds.is_empty() {
            finish(self);
            return;
        }
        for (uris, mime) in kinds {
            let Ok(pipe) = offer.receive(mime) else {
                let mut p = pending.borrow_mut();
                p.done += 1;
                continue;
            };
            let (got, done) = (pending.clone(), finish.clone());
            let mut buf: Vec<u8> = Vec::new();
            let r = self.loop_handle.insert_source(pipe, move |_, file, st: &mut State| {
                let mut chunk = [0u8; 16384];
                let mut f: &std::fs::File = file.as_ref();
                match f.read(&mut chunk) {
                    Ok(n) if n > 0 && buf.len() < (4 << 20) => {
                        buf.extend_from_slice(&chunk[..n]);
                        return smithay_client_toolkit::reexports::calloop::PostAction::Continue;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted || e.kind() == std::io::ErrorKind::WouldBlock => {
                        return smithay_client_toolkit::reexports::calloop::PostAction::Continue;
                    }
                    _ => {}
                }
                {
                    let mut p = got.borrow_mut();
                    let data = std::mem::take(&mut buf);
                    if uris {
                        p.uris = Some(data);
                    } else {
                        p.text = Some(data);
                    }
                    p.done += 1;
                }
                done(st);
                smithay_client_toolkit::reexports::calloop::PostAction::Remove
            });
            if r.is_err() {
                pending.borrow_mut().done += 1;
            }
        }
        // (flushed now: the source sends once it hears the receive requests)
        finish(self);
    }
}

impl DataDeviceHandler for State {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice, x: f64, y: f64, wl: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface) {
        let Some(offer) = self.drag_offer(device) else { return };
        let Some(s) = self.surfaces.iter().find(|s| &s.wl == wl).cloned().filter(|s| self.dnd.target(s)) else {
            offer.accept_mime_type(offer.serial, None);
            offer.set_actions(DndAction::empty(), DndAction::empty());
            self.dnd.over = None;
            return;
        };
        let mimes = offer.with_mime_types(|m| m.to_vec());
        self.dnd.over = Some((s.clone(), mimes.clone()));
        let ours = self.dnd.source.is_some();
        self.tell_drag(&s, Drag::Enter { x: x as f32, y: y as f32, mimes, ours }, Some(&offer));
    }

    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {
        if let Some((s, _)) = self.dnd.over.take() {
            self.tell_drag(&s, Drag::Leave, None);
        }
    }

    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice, x: f64, y: f64) {
        let Some((s, _)) = self.dnd.over.clone() else { return };
        let offer = self.drag_offer(device);
        self.tell_drag(&s, Drag::Motion { x: x as f32, y: y as f32 }, offer.as_ref());
    }

    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}

    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice) {
        let Some((s, mimes)) = self.dnd.over.take() else { return };
        let Some(offer) = self.drag_offer(device) else { return };
        if self.dnd.refused || pick(&mimes).is_none() {
            self.tell_drag(&s, Drag::Leave, None);
            offer.destroy();
            return;
        }
        self.read_drop(s, offer, mimes);
    }
}

impl DataOfferHandler for State {
    fn source_actions(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
    fn selected_action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
}

impl DataSourceHandler for State {
    fn accept_mime(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: Option<String>) {}

    fn send_request(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource, mime: String, fd: WritePipe) {
        let Some((src, offers)) = &self.dnd.source else { return };
        if src.inner() != source {
            return;
        }
        let Some(bytes) = offers.iter().find(|o| o.0 == mime).map(|o| o.1.clone()) else { return };
        // (written from a thread: whoever reads it may be slow)
        let fd: std::os::fd::OwnedFd = fd.into();
        std::thread::spawn(move || {
            let mut f = std::fs::File::from(fd);
            let _ = f.write_all(&bytes);
        });
    }

    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
        if self.dnd.source.as_ref().is_some_and(|(s, _)| s.inner() == source) {
            self.dnd.source = None;
        }
    }

    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}

    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
        if self.dnd.source.as_ref().is_some_and(|(s, _)| s.inner() == source) {
            self.dnd.source = None;
        }
    }

    fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: DndAction) {}
}

delegate_data_device!(State);

#[cfg(test)]
mod tests {
    use super::*;

    fn v(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn files_first_then_text() {
        assert_eq!(pick(&v(&["text/plain", "text/uri-list"])).as_deref(), Some("text/uri-list"));
        assert_eq!(pick(&v(&["UTF8_STRING", "text/plain;charset=utf-8"])).as_deref(), Some("text/plain;charset=utf-8"));
        assert_eq!(pick(&v(&["image/png", "application/x-thing"])), None);
        assert_eq!(text_type(&v(&["text/uri-list", "STRING"])).as_deref(), Some("STRING"));
        assert_eq!(text_type(&v(&["text/uri-list"])), None);
    }

    #[test]
    fn uri_lists_as_file_managers_write_them() {
        let l = uri_list(b"# a comment\r\nfile:///home/a/One.pdf\r\nfile:///home/a/Two%20words.png\r\n\r\n");
        assert_eq!(l, vec!["file:///home/a/One.pdf".to_string(), "file:///home/a/Two%20words.png".to_string()]);
        assert!(uri_list(b"").is_empty());
    }
}
