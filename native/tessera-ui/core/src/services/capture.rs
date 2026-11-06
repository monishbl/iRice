//! Pictures of windows, for the Overview, Mission Control, the switcher and
//! Stage Manager's strip: Hyprland's hyprland-toplevel-export-v1, the same
//! source the QML's ScreencopyView uses for a window on Hyprland (Quickshell
//! asks it with the window's wlr toplevel handle; here with its address, the
//! protocol's first request, so no foreign-toplevel list is needed).
//!
//! A thread of its own with its own Wayland connection: `ask(address, max)`
//! queues a capture, and the picture comes back through the callback at most
//! `max` (device pixels; downscaled here, off the UI thread, by averaging),
//! None when the window can't be captured. One capture at a time; a second
//! ask for a window already queued is dropped (live previews ask again once
//! the last picture came).

use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_buffer::WlBuffer, wl_registry::WlRegistry, wl_shm, wl_shm::WlShm, wl_shm_pool::WlShmPool};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum};

#[allow(dead_code, non_camel_case_types, non_upper_case_globals, clippy::all)]
pub mod proto {
    use wayland_client;
    use wayland_client::protocol::*;
    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/hyprland-toplevel-export-v1.xml");
    }
    use self::__interfaces::*;
    wayland_scanner::generate_client_code!("protocols/hyprland-toplevel-export-v1.xml");
}

use proto::hyprland_toplevel_export_frame_v1::{self as frame, HyprlandToplevelExportFrameV1};
use proto::hyprland_toplevel_export_manager_v1::HyprlandToplevelExportManagerV1;

/// One window's picture (None: it couldn't be taken)
pub struct Shot {
    pub address: String,
    /// the asker's own number (to drop pictures asked before a reset)
    pub tag: u64,
    pub px: Option<SharedPixelBuffer<Rgba8Pixel>>,
}

struct Req {
    address: String,
    max: (u32, u32),
    tag: u64,
}

#[derive(Clone)]
pub struct Capturer {
    tx: mpsc::Sender<Req>,
}

impl Capturer {
    /// The thread; `reply` gets every picture (on that thread)
    pub fn spawn(reply: impl Fn(Shot) + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel::<Req>();
        std::thread::spawn(move || {
            let mut cap = Session::connect();
            if cap.is_none() {
                eprintln!("tessera-ui: no window pictures (hyprland-toplevel-export-v1 isn't offered)");
            }
            while let Ok(first) = rx.recv() {
                // (what came meanwhile, each window once, in order)
                let mut queue = vec![first];
                while let Ok(r) = rx.try_recv() {
                    if let Some(old) = queue.iter_mut().find(|q| q.address == r.address) {
                        *old = r;
                    } else {
                        queue.push(r);
                    }
                }
                for r in queue {
                    let px = cap.as_mut().and_then(|c| c.take(&r.address, r.max));
                    if cap.as_ref().is_some_and(|c| c.broken) {
                        cap = Session::connect();
                    }
                    reply(Shot { address: r.address, tag: r.tag, px });
                }
            }
        });
        Self { tx }
    }

    pub fn ask(&self, address: &str, max: (u32, u32), tag: u64) {
        let _ = self.tx.send(Req { address: address.to_string(), max, tag });
    }
}

/// Hyprland's handle for a window: the low 32 bits of its address
/// ("0x55d1d161e7b0" → 0xd161e7b0), as `hyprctl clients` shows it
pub fn handle(address: &str) -> Option<u32> {
    let a = address.trim().trim_start_matches("0x");
    u64::from_str_radix(a, 16).ok().map(|v| (v & 0xFFFF_FFFF) as u32)
}

/// The size a picture of `w`×`h` is drawn at to fit `max` (never larger)
pub fn fit(w: u32, h: u32, max: (u32, u32)) -> (u32, u32) {
    if w == 0 || h == 0 {
        return (0, 0);
    }
    let k = (max.0 as f64 / w as f64).min(max.1 as f64 / h as f64).min(1.0);
    (((w as f64 * k).round() as u32).max(1), ((h as f64 * k).round() as u32).max(1))
}

/// wl_shm's 32-bit formats: where red, green, blue and alpha sit in a
/// pixel's four bytes (little-endian), and whether alpha counts
pub fn layout(format: u32) -> Option<([usize; 3], Option<usize>)> {
    match format {
        0 => Some(([2, 1, 0], Some(3))),          // ARGB8888
        1 => Some(([2, 1, 0], None)),             // XRGB8888
        0x3432_4241 => Some(([0, 1, 2], Some(3))), // ABGR8888
        0x3432_4258 => Some(([0, 1, 2], None)),   // XBGR8888
        _ => None,
    }
}

/// Averages the source down to `tw`×`th`, as RGBA (each target pixel the
/// mean of the source pixels under it); `flip`: the rows come bottom first
#[allow(clippy::too_many_arguments)]
pub fn downscale(src: &[u8], stride: usize, w: u32, h: u32, format: u32, flip: bool, tw: u32, th: u32) -> Option<Vec<u8>> {
    let (rgb, alpha) = layout(format)?;
    let (w, h, tw, th) = (w as usize, h as usize, tw.max(1) as usize, th.max(1) as usize);
    if src.len() < stride * h || stride < w * 4 {
        return None;
    }
    let mut out = vec![0u8; tw * th * 4];
    for ty in 0..th {
        let (y0, y1) = (ty * h / th, ((ty + 1) * h / th).max(ty * h / th + 1).min(h));
        for tx in 0..tw {
            let (x0, x1) = (tx * w / tw, ((tx + 1) * w / tw).max(tx * w / tw + 1).min(w));
            let mut sum = [0u32; 4];
            for y in y0..y1 {
                let row = if flip { h - 1 - y } else { y };
                let base = row * stride;
                for x in x0..x1 {
                    let p = &src[base + x * 4..base + x * 4 + 4];
                    sum[0] += p[rgb[0]] as u32;
                    sum[1] += p[rgb[1]] as u32;
                    sum[2] += p[rgb[2]] as u32;
                    sum[3] += alpha.map(|a| p[a] as u32).unwrap_or(255);
                }
            }
            let n = ((y1 - y0) * (x1 - x0)).max(1) as u32;
            let o = (ty * tw + tx) * 4;
            for i in 0..4 {
                out[o + i] = (sum[i] / n) as u8;
            }
        }
    }
    Some(out)
}

#[derive(Default)]
struct Frame {
    shm: Option<(u32, u32, u32, u32)>, // format, w, h, stride
    listed: bool,
    flip: bool,
    ready: bool,
    failed: bool,
}

struct Session {
    conn: Connection,
    queue: EventQueue<Frame>,
    qh: QueueHandle<Frame>,
    shm: WlShm,
    manager: HyprlandToplevelExportManagerV1,
    state: Frame,
    broken: bool,
}

impl Session {
    fn connect() -> Option<Self> {
        let conn = Connection::connect_to_env().ok()?;
        let (globals, queue) = registry_queue_init::<Frame>(&conn).ok()?;
        let qh = queue.handle();
        let shm: WlShm = globals.bind(&qh, 1..=1, ()).ok()?;
        let manager: HyprlandToplevelExportManagerV1 = globals.bind(&qh, 1..=1, ()).ok()?;
        Some(Self { conn, queue, qh, shm, manager, state: Frame::default(), broken: false })
    }

    /// Wait (up to `until`) for the frame to say something new
    fn pump(&mut self, until: Instant, done: impl Fn(&Frame) -> bool) -> bool {
        loop {
            if self.queue.dispatch_pending(&mut self.state).is_err() {
                self.broken = true;
                return false;
            }
            if done(&self.state) {
                return true;
            }
            if self.state.failed {
                return false;
            }
            if self.queue.flush().is_err() {
                self.broken = true;
                return false;
            }
            let Some(guard) = self.queue.prepare_read() else { continue };
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            let mut pfd = libc::pollfd { fd: std::os::fd::AsRawFd::as_raw_fd(&guard.connection_fd()), events: libc::POLLIN, revents: 0 };
            let n = unsafe { libc::poll(&mut pfd, 1, left.as_millis().min(i32::MAX as u128) as i32) };
            if n <= 0 {
                drop(guard);
                if n < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return false;
            }
            if guard.read().is_err() {
                self.broken = true;
                return false;
            }
        }
    }

    fn take(&mut self, address: &str, max: (u32, u32)) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
        let handle = handle(address)?;
        self.state = Frame::default();
        let f = self.manager.capture_toplevel(0, handle, &self.qh, ());
        let until = Instant::now() + Duration::from_millis(1500);
        let out = self.copy(&f, until, max);
        f.destroy();
        let _ = self.conn.flush();
        out
    }

    fn copy(&mut self, f: &HyprlandToplevelExportFrameV1, until: Instant, max: (u32, u32)) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
        if !self.pump(until, |s| s.listed) {
            return None;
        }
        let (format, w, h, stride) = self.state.shm?;
        layout(format)?;
        let size = (stride as usize).checked_mul(h as usize)?;
        if w == 0 || h == 0 || size == 0 || size > 256 << 20 {
            return None;
        }
        let fd = unsafe { libc::memfd_create(c"tessera-capture".as_ptr(), libc::MFD_CLOEXEC) };
        if fd < 0 {
            return None;
        }
        let file = std::fs::File::from(unsafe { OwnedFd::from_raw_fd(fd) });
        file.set_len(size as u64).ok()?;
        let map = unsafe { memmap2::MmapMut::map_mut(&file) }.ok()?;
        let pool: WlShmPool = self.shm.create_pool(file.as_fd(), size as i32, &self.qh, ());
        let buffer: WlBuffer = pool.create_buffer(0, w as i32, h as i32, stride as i32, wl_shm::Format::try_from(format).ok()?, &self.qh, ());
        f.copy(&buffer, 1);
        let ok = self.pump(until, |s| s.ready);
        let px = if ok {
            let (tw, th) = fit(w, h, max);
            downscale(&map, stride as usize, w, h, format, self.state.flip, tw, th).map(|rgba| {
                let mut b = SharedPixelBuffer::<Rgba8Pixel>::new(tw, th);
                b.make_mut_bytes().copy_from_slice(&rgba);
                b
            })
        } else {
            None
        };
        buffer.destroy();
        pool.destroy();
        px
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for Frame {
    fn event(_: &mut Self, _: &WlRegistry, _: wayland_client::protocol::wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlShm, ()> for Frame {
    fn event(_: &mut Self, _: &WlShm, _: wl_shm::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlShmPool, ()> for Frame {
    fn event(_: &mut Self, _: &WlShmPool, _: wayland_client::protocol::wl_shm_pool::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlBuffer, ()> for Frame {
    fn event(_: &mut Self, _: &WlBuffer, _: wayland_client::protocol::wl_buffer::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<HyprlandToplevelExportManagerV1, ()> for Frame {
    fn event(_: &mut Self, _: &HyprlandToplevelExportManagerV1, _: proto::hyprland_toplevel_export_manager_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<HyprlandToplevelExportFrameV1, ()> for Frame {
    fn event(s: &mut Self, _: &HyprlandToplevelExportFrameV1, e: frame::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match e {
            frame::Event::Buffer { format, width, height, stride } => {
                let f: u32 = match format {
                    WEnum::Value(v) => v.into(),
                    WEnum::Unknown(u) => u,
                };
                // (the first format we can read wins)
                if s.shm.is_none() && layout(f).is_some() {
                    s.shm = Some((f, width, height, stride));
                }
            }
            frame::Event::BufferDone => s.listed = true,
            frame::Event::Flags { flags } => {
                s.flip = matches!(flags, WEnum::Value(frame::Flags::YInvert));
            }
            frame::Event::Ready { .. } => s.ready = true,
            frame::Event::Failed => s.failed = true,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_are_the_addresses_low_half() {
        assert_eq!(handle("0x55d1d161e7b0"), Some(0xd161e7b0));
        assert_eq!(handle("d161e7b0"), Some(3512854448));
        assert_eq!(handle("nonsense"), None);
    }

    #[test]
    fn pictures_fit_never_grow() {
        assert_eq!(fit(1920, 1080, (300, 300)), (300, 169));
        assert_eq!(fit(800, 1200, (300, 300)), (200, 300));
        assert_eq!(fit(100, 50, (300, 300)), (100, 50));
        assert_eq!(fit(0, 50, (300, 300)), (0, 0));
    }

    #[test]
    fn downscaling_averages_and_swaps_to_rgba() {
        // 2×2 XRGB (B, G, R, X bytes): a red row over a blue one
        let px = |r: u8, g: u8, b: u8| [b, g, r, 0];
        let mut src = Vec::new();
        for p in [px(255, 0, 0), px(255, 0, 0), px(0, 0, 255), px(0, 0, 255)] {
            src.extend_from_slice(&p);
        }
        let out = downscale(&src, 8, 2, 2, 1, false, 1, 2).unwrap();
        assert_eq!(out, vec![255, 0, 0, 255, 0, 0, 255, 255]);
        // y-inverted: the bottom row first
        let out = downscale(&src, 8, 2, 2, 1, true, 1, 2).unwrap();
        assert_eq!(out, vec![0, 0, 255, 255, 255, 0, 0, 255]);
        // a 2×1 average
        let out = downscale(&src, 8, 2, 2, 1, false, 1, 1).unwrap();
        assert_eq!(&out[..3], &[127, 0, 127]);
        // ARGB keeps its alpha; unknown formats and short buffers are refused
        let a = [0u8, 0, 0, 128];
        assert_eq!(downscale(&a, 4, 1, 1, 0, false, 1, 1).unwrap()[3], 128);
        assert!(downscale(&a, 4, 1, 1, 7, false, 1, 1).is_none());
        assert!(downscale(&a, 4, 2, 2, 1, false, 1, 1).is_none());
    }
}
