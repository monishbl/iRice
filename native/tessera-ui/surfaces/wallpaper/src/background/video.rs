//! A live wallpaper's video, as Background.qml's MediaPlayer + VideoOutput:
//! muted, looping, cropped to cover the screen, paused while nobody would see
//! it (`paused`: the QML's own conditions). Standalone only: with the QML
//! shell running it plays the video over this window's poster.
//!
//! ffmpeg decodes (`-stream_loop -1`, scaled and cropped to the screen's
//! pixels, at most 1920 wide: the window scales it up, as VideoOutput does
//! the video's own size) into raw RGBA on a pipe; a thread reads one frame
//! at a time at the video's own rate (at most 60 fps) and hands it over only
//! when the window took the one before (a slow frame is dropped, never
//! queued). Paused, the thread stops reading: ffmpeg blocks on the full pipe
//! and costs nothing, and the last frame stays up (as a paused VideoOutput).
//! Frame buffers go back and forth between the thread and the window, so
//! playing allocates nothing per frame.

use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

pub type Frame = SharedPixelBuffer<Rgba8Pixel>;

/// The size to decode at: the screen's pixels, at most 1920 wide (an even size)
pub fn decode_size(px: (u32, u32)) -> (u32, u32) {
    let (w, h) = (px.0.max(2), px.1.max(2));
    let (w, h) = if w > 1920 { (1920, ((h as u64 * 1920) / w as u64) as u32) } else { (w, h) };
    (w.max(2) & !1, h.max(2) & !1)
}

/// The video's frame rate from ffprobe's "30000/1001", within 1..=60 (30 when unknown)
pub fn fps_of(rate: &str) -> f64 {
    let r = rate.trim();
    let v = match r.split_once('/') {
        Some((a, b)) => a.parse::<f64>().unwrap_or(0.0) / b.parse::<f64>().unwrap_or(0.0),
        None => r.parse().unwrap_or(0.0),
    };
    if v.is_finite() && v >= 1.0 { v.min(60.0) } else { 30.0 }
}

/// Background.qml's `paused`, from what it reads
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Why {
    /// Desktop.mostlyCovered(screen)
    pub covered: bool,
    /// System.locked
    pub locked: bool,
    /// two minutes without input (IdleMonitor, inhibitors respected)
    pub away: bool,
    /// livePauseOnBattery && UPower.onBattery
    pub battery: bool,
    /// CoolDown.pauseLive
    pub cooling: bool,
}

impl Why {
    pub fn paused(&self) -> bool {
        self.covered || self.locked || self.away || self.battery || self.cooling
    }
}

/// Desktop.mostlyCovered: a full-screen window on the screen's shown space,
/// or windows there covering 70% of its area (sizes added, overlaps and all)
pub fn mostly_covered(w: &tessera_core::services::clients::Windows, output: &str) -> bool {
    let Some(m) = w.monitor(output) else { return false };
    let (sw, sh) = m.logical();
    let mut area = 0.0f64;
    for c in w.clients.iter().filter(|c| c.workspace.id == m.active.id && c.mapped && !c.hidden) {
        if c.fullscreen > 0 {
            return true;
        }
        area += c.size.0 as f64 * c.size.1 as f64;
    }
    area >= 0.7 * sw as f64 * sh as f64
}

struct Ctl {
    paused: bool,
    stop: bool,
}

pub struct Player {
    pub video: String,
    pub size: (u32, u32),
    child: Arc<Mutex<Option<Child>>>,
    ctl: Arc<(Mutex<Ctl>, Condvar)>,
    /// a frame is on its way to the window (the next one waits or is dropped)
    in_flight: Arc<AtomicBool>,
    /// frames the window is done with, back to the thread
    spare: Sender<Frame>,
    /// the frame up now (sent back when the next replaces it)
    pub shown: Option<Frame>,
}

impl Player {
    /// Start decoding `video` at `size`; frames go to `deliver` (any thread)
    pub fn start(video: &str, size: (u32, u32), paused: bool, deliver: impl Fn(Frame) + Send + 'static) -> Self {
        let ctl = Arc::new((Mutex::new(Ctl { paused, stop: false }), Condvar::new()));
        let in_flight = Arc::new(AtomicBool::new(false));
        let child: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
        let (spare, spares) = channel::<Frame>();
        let (v, c, f, ch) = (video.to_string(), ctl.clone(), in_flight.clone(), child.clone());
        std::thread::Builder::new()
            .name("tessera-live".into())
            .spawn(move || run(&v, size, c, f, ch, spares, deliver))
            .ok();
        Self { video: video.to_string(), size, child, ctl, in_flight, spare, shown: None }
    }

    pub fn set_paused(&self, paused: bool) {
        let (m, cv) = &*self.ctl;
        if let Ok(mut g) = m.lock() {
            if g.paused != paused {
                g.paused = paused;
                cv.notify_all();
            }
        }
    }

    /// The window put `frame` up: the one before goes back for reuse, and the next may come
    pub fn shown(&mut self, frame: Frame) {
        if let Some(old) = self.shown.replace(frame) {
            let _ = self.spare.send(old);
        }
        self.in_flight.store(false, Ordering::Release);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let (m, cv) = &*self.ctl;
        if let Ok(mut g) = m.lock() {
            g.stop = true;
            cv.notify_all();
        }
        if let Some(mut c) = self.child.lock().ok().and_then(|mut g| g.take()) {
            let _ = c.kill();
            std::thread::spawn(move || {
                let _ = c.wait();
            });
        }
    }
}

fn probe_fps(video: &str) -> f64 {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=avg_frame_rate", "-of", "csv=p=0", video])
        .stderr(Stdio::null())
        .output();
    out.map(|o| fps_of(String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or(""))).unwrap_or(30.0)
}

fn run(video: &str, (w, h): (u32, u32), ctl: Arc<(Mutex<Ctl>, Condvar)>, in_flight: Arc<AtomicBool>, child: Arc<Mutex<Option<Child>>>, spares: Receiver<Frame>, deliver: impl Fn(Frame)) {
    let step = Duration::from_secs_f64(1.0 / probe_fps(video));
    let filter = format!("scale={w}:{h}:force_original_aspect_ratio=increase:flags=fast_bilinear,crop={w}:{h}");
    let spawned = Command::new("ffmpeg")
        .args(["-nostdin", "-loglevel", "error", "-threads", "2", "-stream_loop", "-1", "-i", video, "-an", "-sn", "-vf", &filter, "-pix_fmt", "rgba", "-f", "rawvideo", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut c) = spawned else {
        eprintln!("tessera-ui: live wallpaper: ffmpeg didn't start");
        return;
    };
    let Some(mut out) = c.stdout.take() else { return };
    match child.lock() {
        Ok(mut g) => *g = Some(c),
        Err(_) => return,
    }
    // (stopped while it started: Drop found no child to end)
    if ctl.0.lock().map_or(true, |g| g.stop) {
        if let Some(mut c) = child.lock().ok().and_then(|mut g| g.take()) {
            let _ = c.kill();
            let _ = c.wait();
        }
        return;
    }
    let mut spare: Option<Frame> = None;
    let mut next = Instant::now();
    loop {
        // paused: read nothing (ffmpeg waits on the pipe); from now when it goes on
        {
            let (m, cv) = &*ctl;
            let Ok(mut g) = m.lock() else { return };
            if g.paused {
                while g.paused && !g.stop {
                    g = match cv.wait(g) {
                        Ok(g) => g,
                        Err(_) => return,
                    };
                }
                next = Instant::now();
            }
            if g.stop {
                return;
            }
        }
        let mut buf = spare.take().or_else(|| spares.try_recv().ok()).filter(|b| b.width() == w && b.height() == h).unwrap_or_else(|| Frame::new(w, h));
        if out.read_exact(buf.make_mut_bytes()).is_err() {
            return; // (ffmpeg ended: an unreadable video, or it was stopped)
        }
        // at the video's own pace; far behind (a stall): from now
        next += step;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else if now - next > Duration::from_millis(250) {
            next = now;
        }
        if in_flight.swap(true, Ordering::AcqRel) {
            spare = Some(buf); // (the window hasn't taken the last one: this one is dropped)
        } else {
            deliver(buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoded_at_the_screen_at_most_1920_wide() {
        assert_eq!(decode_size((1920, 1080)), (1920, 1080));
        assert_eq!(decode_size((3840, 2160)), (1920, 1080));
        assert_eq!(decode_size((2560, 1600)), (1920, 1200));
        assert_eq!(decode_size((1366, 769)), (1366, 768));
        assert_eq!(decode_size((0, 0)), (2, 2));
    }

    #[test]
    fn frame_rates() {
        assert!((fps_of("30000/1001") - 29.97).abs() < 0.01);
        assert_eq!(fps_of("25/1"), 25.0);
        assert_eq!(fps_of("120/1"), 60.0);
        assert_eq!(fps_of("0/0"), 30.0);
        assert_eq!(fps_of(""), 30.0);
    }

    #[test]
    fn paused_as_the_qml() {
        assert!(!Why::default().paused());
        for w in [
            Why { covered: true, ..Default::default() },
            Why { locked: true, ..Default::default() },
            Why { away: true, ..Default::default() },
            Why { battery: true, ..Default::default() },
            Why { cooling: true, ..Default::default() },
        ] {
            assert!(w.paused());
        }
    }

    #[test]
    fn mostly_covered_as_desktop_qml() {
        use tessera_core::services::clients::{Client, Monitor, Windows, WsRef};
        let m = Monitor { id: 0, name: "A".into(), width: 1920, height: 1080, scale: 1.0, active: WsRef { id: 1, name: "1".into() }, ..Default::default() };
        let win = |ws: i32, w: i32, h: i32, fs: i32| Client { workspace: WsRef { id: ws, name: String::new() }, size: (w, h), mapped: true, fullscreen: fs, ..Default::default() };
        let mut ws = Windows { monitors: vec![m], ..Default::default() };
        assert!(!mostly_covered(&ws, "A"));
        ws.clients = vec![win(1, 960, 1080, 0)];
        assert!(!mostly_covered(&ws, "A")); // half
        ws.clients.push(win(1, 960, 600, 0));
        assert!(mostly_covered(&ws, "A")); // 0.78
        ws.clients = vec![win(2, 1920, 1080, 0), win(1, 400, 300, 1)];
        assert!(mostly_covered(&ws, "A")); // full screen, however small it says it is
        ws.clients = vec![win(2, 1920, 1080, 0)];
        assert!(!mostly_covered(&ws, "A")); // another space
        assert!(!mostly_covered(&ws, "B"));
    }
}
