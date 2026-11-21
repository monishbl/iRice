//! Paste without a helper process: the clipboard's text read through the
//! data-control protocol (ext-data-control-v1, else wlr-data-control), on a
//! Wayland connection of its own made for the read, as wl-paste does but
//! without a fork and two execs per paste (audit #46). The text comes over
//! a pipe from the selection's owner. `None` from `read_text` means neither
//! protocol is offered (then wl-paste is the fallback, wayland.rs).
//!
//! Called off the UI thread (Platform::clipboard_text waits PASTE_WAIT for
//! it): the read itself stops at `deadline`, so an owner that never answers
//! doesn't keep a thread forever.

use smithay_client_toolkit::reexports::client::{
    backend::ObjectId,
    event_created_child,
    globals::{registry_queue_init, GlobalListContents},
    protocol::{wl_registry::WlRegistry, wl_seat::WlSeat},
    Connection, Dispatch, Proxy, QueueHandle,
};
use smithay_client_toolkit::reexports::protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self, ExtDataControlDeviceV1},
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::{self, ExtDataControlOfferV1},
};
use smithay_client_toolkit::reexports::protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1::{self, ZwlrDataControlDeviceV1},
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    zwlr_data_control_offer_v1::{self, ZwlrDataControlOfferV1},
};
use std::collections::HashMap;
use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use std::time::Instant;

/// What a read found
#[derive(Debug, PartialEq)]
pub enum Got {
    /// the selection's text
    Text(String),
    /// nothing to paste: no selection, or none offered as text, or the
    /// owner didn't answer in time
    Nothing,
}

#[derive(Clone)]
enum Offer {
    Ext(ExtDataControlOfferV1),
    Wlr(ZwlrDataControlOfferV1),
}

impl Offer {
    fn id(&self) -> ObjectId {
        match self {
            Offer::Ext(o) => o.id(),
            Offer::Wlr(o) => o.id(),
        }
    }
    fn receive(&self, mime: String, fd: std::os::fd::BorrowedFd<'_>) {
        match self {
            Offer::Ext(o) => o.receive(mime, fd),
            Offer::Wlr(o) => o.receive(mime, fd),
        }
    }
    fn destroy(&self) {
        match self {
            Offer::Ext(o) => o.destroy(),
            Offer::Wlr(o) => o.destroy(),
        }
    }
}

#[derive(Default)]
struct St {
    /// each offer's types, as announced
    types: HashMap<ObjectId, Vec<String>>,
    offers: Vec<Offer>,
    /// the clipboard's offer now (None: nothing selected)
    selection: Option<Offer>,
    /// the selection event came (the device's first answer)
    heard: bool,
}

/// The type to ask for, as wl-paste --type text chooses: UTF-8 plain text
/// first, then the other text names apps offer
pub fn text_type(types: &[String]) -> Option<&str> {
    const WANT: &[&str] = &["text/plain;charset=utf-8", "text/plain;charset=UTF-8", "UTF8_STRING", "text/plain", "TEXT", "STRING", "COMPOUND_TEXT"];
    WANT.iter().find_map(|w| types.iter().find(|t| t.as_str() == *w)).or_else(|| types.iter().find(|t| t.starts_with("text/"))).map(String::as_str)
}

/// The clipboard's text now. None: the compositor offers neither
/// data-control protocol (or there's no Wayland to ask)
pub fn read_text(deadline: Instant) -> Option<Got> {
    let conn = Connection::connect_to_env().ok()?;
    let (globals, mut queue) = registry_queue_init::<St>(&conn).ok()?;
    let qh = queue.handle();
    let seat: WlSeat = globals.bind(&qh, 1..=1, ()).ok()?;
    let mut st = St::default();
    // (ext first, the protocol's standard form; wlr's where only it is)
    enum Mgr {
        Ext(ExtDataControlManagerV1, ExtDataControlDeviceV1),
        Wlr(ZwlrDataControlManagerV1, ZwlrDataControlDeviceV1),
    }
    let mgr = if let Ok(m) = globals.bind::<ExtDataControlManagerV1, _, _>(&qh, 1..=1, ()) {
        let d = m.get_data_device(&seat, &qh, ());
        Mgr::Ext(m, d)
    } else if let Ok(m) = globals.bind::<ZwlrDataControlManagerV1, _, _>(&qh, 1..=1, ()) {
        let d = m.get_data_device(&seat, &qh, ());
        Mgr::Wlr(m, d)
    } else {
        return None;
    };
    // (the device answers with the selection at once: its offer, the
    // offer's types, then the selection event)
    let got = (|| {
        if queue.roundtrip(&mut st).is_err() || !st.heard {
            return Got::Nothing;
        }
        let Some(offer) = st.selection.clone() else { return Got::Nothing };
        let Some(mime) = st.types.get(&offer.id()).and_then(|t| text_type(t)).map(str::to_string) else { return Got::Nothing };
        let mut fds = [0; 2];
        if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
            return Got::Nothing;
        }
        // SAFETY: both ends were just made by pipe2 and are owned here only
        let (read_end, write_end) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        offer.receive(mime, write_end.as_fd());
        if conn.flush().is_err() {
            return Got::Nothing;
        }
        // (our end closed once the request is sent: the owner's copy is
        // the only writer left, so its close is the end of the text)
        drop(write_end);
        match read_until(&read_end, deadline) {
            Some(bytes) => Got::Text(String::from_utf8_lossy(&bytes).into_owned()),
            None => Got::Nothing,
        }
    })();
    for o in &st.offers {
        o.destroy();
    }
    match mgr {
        Mgr::Ext(m, d) => {
            d.destroy();
            m.destroy();
        }
        Mgr::Wlr(m, d) => {
            d.destroy();
            m.destroy();
        }
    }
    let _ = conn.flush();
    Some(got)
}

/// Everything until the writer closes, or None past the deadline
fn read_until(fd: &OwnedFd, deadline: Instant) -> Option<Vec<u8>> {
    use std::os::fd::AsRawFd;
    let mut out = Vec::new();
    let mut buf = [0u8; 16384];
    loop {
        let left = deadline.checked_duration_since(Instant::now())?;
        let mut p = libc::pollfd { fd: fd.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        let r = unsafe { libc::poll(&mut p, 1, left.as_millis().min(i32::MAX as u128) as i32) };
        if r < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return None;
        }
        if r == 0 {
            return None;
        }
        let n = unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
        if n < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return None;
        }
        if n == 0 {
            return Some(out);
        }
        out.extend_from_slice(&buf[..n as usize]);
        // (a paste into a text field: 64 MiB is more than enough)
        if out.len() > 64 << 20 {
            return None;
        }
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for St {
    fn event(_: &mut Self, _: &WlRegistry, _: <WlRegistry as Proxy>::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<WlSeat, ()> for St {
    fn event(_: &mut Self, _: &WlSeat, _: <WlSeat as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ExtDataControlManagerV1, ()> for St {
    fn event(_: &mut Self, _: &ExtDataControlManagerV1, _: <ExtDataControlManagerV1 as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ZwlrDataControlManagerV1, ()> for St {
    fn event(_: &mut Self, _: &ZwlrDataControlManagerV1, _: <ZwlrDataControlManagerV1 as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ExtDataControlDeviceV1, ()> for St {
    fn event(st: &mut Self, _: &ExtDataControlDeviceV1, e: ext_data_control_device_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match e {
            ext_data_control_device_v1::Event::DataOffer { id } => st.offers.push(Offer::Ext(id)),
            ext_data_control_device_v1::Event::Selection { id } => {
                st.selection = id.map(Offer::Ext);
                st.heard = true;
            }
            ext_data_control_device_v1::Event::Finished => st.heard = true,
            _ => {}
        }
    }
    event_created_child!(St, ExtDataControlDeviceV1, [
        ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ZwlrDataControlDeviceV1, ()> for St {
    fn event(st: &mut Self, _: &ZwlrDataControlDeviceV1, e: zwlr_data_control_device_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match e {
            zwlr_data_control_device_v1::Event::DataOffer { id } => st.offers.push(Offer::Wlr(id)),
            zwlr_data_control_device_v1::Event::Selection { id } => {
                st.selection = id.map(Offer::Wlr);
                st.heard = true;
            }
            zwlr_data_control_device_v1::Event::Finished => st.heard = true,
            _ => {}
        }
    }
    event_created_child!(St, ZwlrDataControlDeviceV1, [
        zwlr_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ZwlrDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ExtDataControlOfferV1, ()> for St {
    fn event(st: &mut Self, o: &ExtDataControlOfferV1, e: ext_data_control_offer_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let ext_data_control_offer_v1::Event::Offer { mime_type } = e {
            st.types.entry(o.id()).or_default().push(mime_type);
        }
    }
}

impl Dispatch<ZwlrDataControlOfferV1, ()> for St {
    fn event(st: &mut Self, o: &ZwlrDataControlOfferV1, e: zwlr_data_control_offer_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let zwlr_data_control_offer_v1::Event::Offer { mime_type } = e {
            st.types.entry(o.id()).or_default().push(mime_type);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_asked_for_as_utf8_first() {
        let t = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(text_type(&t(&["image/png", "STRING", "text/plain;charset=utf-8"])), Some("text/plain;charset=utf-8"));
        assert_eq!(text_type(&t(&["TEXT", "UTF8_STRING"])), Some("UTF8_STRING"));
        assert_eq!(text_type(&t(&["text/html", "image/png"])), Some("text/html"));
        assert_eq!(text_type(&t(&["image/png"])), None);
    }

    #[test]
    fn a_pipe_is_read_to_its_end_or_given_up_on() {
        let mut fds = [0; 2];
        assert_eq!(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) }, 0);
        let (r, w) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        let mut f = std::fs::File::from(w);
        std::io::Write::write_all(&mut f, b"hello").unwrap();
        drop(f);
        assert_eq!(read_until(&r, Instant::now() + std::time::Duration::from_secs(1)), Some(b"hello".to_vec()));
        // (a writer that never closes: given up on at the deadline)
        assert_eq!(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) }, 0);
        let (r, _w) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        assert_eq!(read_until(&r, Instant::now() + std::time::Duration::from_millis(30)), None);
    }
}
