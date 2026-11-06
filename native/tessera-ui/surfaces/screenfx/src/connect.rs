//! The display connect effect (rust.surfaces: displayconnect), compare
//! shell/modules/osd/DisplayConnect.qml and displayfx.frag: a display
//! connects or leaves; the screen that arrived fills with the wallpaper from
//! the edge it shares with its neighbour (extend, liquid or warp), and the
//! neighbours light up at that edge (a display that left: the light pulls
//! back in). Each screen gets an overlay only while its effect plays (about
//! a second); Settings → Displays → When a display connects.
//!
//! What and where come from the host (host/system displays.rs, DisplayFx):
//!   {"to":"displayconnect","runs":[{id,screen,role,edge,focus,reverse,style,pending,lead}],"motion":1.0}
//! `role` 0 is the arrived screen, 1 a neighbour; `edge` the shared edge (0
//! left, 1 top, 2 right, 3 bottom); `focus` where along it the other display
//! is (0..1); `pending` until the arrangement is known; `lead` ms the arrived
//! screen waits for its neighbour's part; `motion` Cool-down's scale of the
//! timings. When an effect has played this says `displays.finish [id]`.
//!
//! The timeline is the QML's: wait `lead`, then progress 0 → 1 over 1100 /
//! 1050 / 950 / 1000 / 800 ms (liquid arrived / extend arrived / other
//! arrived / liquid beside / the rest; × motion) linear for liquid and
//! extend-arrived, in-out cubic for the other arrived, in-out sine beside,
//! then the overlay fades out in 220 ms (ease-in). The arrived screen starts
//! once its wallpaper is decoded (off the loop), or after 250 ms without it;
//! a pending screen that is never placed is let go after 3 s. Reduced motion:
//! no travel; the arrived screen fades in from dark, neighbours show nothing.
//!
//! Not like the QML: Slint has no custom shaders, so the shader's shapes are
//! drawn with rectangles, gradients and a clipped picture (the extend wipe
//! has no motion blur and the picture doesn't slide with the front; the
//! liquid drop is a circle without a feathered edge, and the neighbour's pill
//! and drop have no refraction; the warp has no lens). The hand-over to the
//! new screen's spaces happens when the effect is finished (the QML did it
//! as the fade began).

use super::*;
use serde_json::{json, Value};
use slint::ComponentHandle;
use std::time::{Duration, Instant};
use tessera_core::shell::{self, Part};
use tessera_core::surfaces::kit;

const NAME: &str = "displayconnect";
const NS: &str = "tessera:display-connect";

enum Msg {
    Tick,
    Picture(String, Option<slint::SharedPixelBuffer<slint::Rgb8Pixel>>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub id: i64,
    pub screen: String,
    pub role: i64,
    pub edge: i64,
    pub focus: f32,
    pub reverse: bool,
    pub style: String,
    pub pending: bool,
    pub lead: f32,
}

impl Run {
    pub fn from(v: &Value) -> Option<Run> {
        let screen = v["screen"].as_str().filter(|s| !s.is_empty())?.to_string();
        Some(Run {
            id: v["id"].as_i64().unwrap_or(0),
            screen,
            role: v["role"].as_i64().unwrap_or(1),
            edge: v["edge"].as_i64().unwrap_or(0).clamp(0, 3),
            focus: v["focus"].as_f64().unwrap_or(0.5) as f32,
            reverse: v["reverse"].as_f64().unwrap_or(0.0) > 0.5 || v["reverse"].as_bool() == Some(true),
            style: v["style"].as_str().unwrap_or("extend").to_string(),
            pending: v["pending"].as_bool().unwrap_or(false),
            lead: v["lead"].as_f64().unwrap_or(0.0) as f32,
        })
    }
    fn liquid(&self) -> bool {
        self.style == "liquid"
    }
    /// ms the progress runs for (before Cool-down's scale)
    pub fn duration(&self) -> f32 {
        if self.liquid() {
            if self.role == 0 { 1100.0 } else { 1000.0 }
        } else if self.style == "extend" && self.role == 0 {
            1050.0
        } else if self.role == 0 {
            950.0
        } else {
            800.0
        }
    }
    /// The easing of progress
    pub fn ease(&self, t: f32) -> f32 {
        if self.liquid() || (self.style == "extend" && self.role == 0) {
            clamp01(t)
        } else if self.role == 0 {
            in_out_cubic(t)
        } else {
            in_out_sine(t)
        }
    }
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::Connect,
    size: (f32, f32),
    run: Run,
    /// the run's id (and whether it was placed) when the effect last started
    key: String,
    started: Option<Instant>,
    /// the picture is decoded (or couldn't be)
    pictured: bool,
    asked_picture: bool,
    waiting_since: Option<Instant>,
    seen: Instant,
    finished: bool,
}

#[derive(Default)]
pub struct DisplayConnect {
    wins: Vec<Win>,
    motion: f32,
    tick: slint::Timer,
    ticking: bool,
}

fn spring(x: f32) -> f32 {
    let x = clamp01(x);
    1.0 - (-5.0 * x).exp() * (7.5 * x).cos()
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = clamp01((x - a) / (b - a));
    t * t * (3.0 - 2.0 * t)
}

/// The length of one unit of d and t, in px (the shader's scale())
fn unit(edge: i64, w: f32, h: f32) -> (f32, f32) {
    if edge == 0 || edge == 2 { (w, h) } else { (h, w) }
}

/// A point in the shader's frame (d px in from the shared edge, t px along it)
/// as a point on the screen
fn to_screen(edge: i64, w: f32, h: f32, d: f32, t: f32) -> (f32, f32) {
    match edge {
        0 => (d, t),
        2 => (w - d, t),
        1 => (t, d),
        _ => (t, h - d),
    }
}

/// The neighbour's glass pill and the drop it buds (the shader's
/// pillAndDrop): q runs 0 → 1 the way the motion goes. In the frame of
/// (d px from the shared edge, t px along it from `along`): the capsule's
/// span in d and its radius and centre in t, the drop's centre in d and
/// radius (0: none yet), and the alpha.
#[derive(Debug, PartialEq)]
pub struct Pill {
    pub d_lo: f32,
    pub d_hi: f32,
    pub t_mid: f32,
    pub rad: f32,
    pub drop_d: f32,
    pub drop_r: f32,
    pub alpha: f32,
}

pub fn pill_and_drop(q: f32, rr: f32, sx: f32) -> Pill {
    let pop = spring(q / 0.16);
    let r = rr * pop;
    let yoff = (1.0 - pop.min(1.0)) * rr * 2.5;
    let far_x = 0.24 * sx + rr;
    let lmax = (11.0 * rr).min(0.2 * sx);
    let l = 2.0 * rr + spring((q - 0.12) / 0.30) * (lmax - 2.0 * rr);
    let near_x = far_x - l;
    // the split: the near end becomes the drop; the pill pulls back a little
    let c = clamp01((q - 0.38) / 0.34);
    let retreat = smoothstep(0.0, 0.45, c) * 2.3 * rr;
    let tr = (c - 0.25).max(0.0) / 0.75;
    let travel = tr * tr * (near_x + 2.0 * rr + 8.0);
    // and shrinks away at the end
    let e = clamp01((q - 0.72) / 0.28);
    let shrink = 1.0 - e * e;
    let mid = (near_x + retreat + far_x) * 0.5;
    let x0 = mid + (near_x + retreat + r - mid) * shrink;
    let x1 = mid + (far_x - r - mid) * shrink;
    let rad = r * (0.3 + 0.7 * shrink);
    let alpha = smoothstep(0.0, 0.04, q) * (1.0 - smoothstep(0.85, 1.0, e));
    Pill {
        d_lo: x0.min(x1) - rad,
        d_hi: x0.max(x1) + rad,
        t_mid: yoff,
        rad,
        drop_d: near_x + rr - travel,
        drop_r: if c > 0.0 { r } else { 0.0 },
        alpha,
    }
}

impl Win {
    /// Draw the effect at this progress (0..1, eased) and fade (1: whole)
    fn draw(&self, p: f32, shown: f32, still: bool) {
        let (w, h) = self.size;
        let run = &self.run;
        let ui = &self.ui;
        ui.set_edge(run.edge as i32);
        ui.set_shown(shown);
        ui.set_mode(0);
        let along = run.focus;
        let (sx, sy) = unit(run.edge, w, h);
        let rr = w.min(h) * 0.034;
        if run.role == 0 {
            if still {
                ui.set_mode(6);
                ui.set_cover((1.0 - p) * shown);
                ui.set_shown(1.0);
                return;
            }
            match run.style.as_str() {
                "liquid" => {
                    // the drop arrives through the edge (springy), then swells to fill
                    let a = clamp01(p / 0.4);
                    let g = clamp01((p - 0.16) / 0.84);
                    let ge = g * g * (3.0 - 2.0 * g);
                    let x1 = 0.12 * sx;
                    let c0 = (-rr + (x1 + rr) * spring(a), 0.0f32);
                    let mid = (0.5 * sx, (0.5 - along) * sy);
                    let cd = c0.0 + (mid.0 - c0.0) * ge;
                    let ct = c0.1 + (mid.1 - c0.1) * ge;
                    let feather = 1.5 + ge * ge * sx.min(sy) * 0.45;
                    let dist = ((mid.0 - c0.0).powi(2) + (mid.1 - c0.1).powi(2)).sqrt();
                    let rmax = (sx * sx + sy * sy).sqrt() * 0.5 + dist * 0.6 + feather + 24.0;
                    let r = rr + (rmax - rr) * ge.powf(1.3);
                    let (cx, cy) = to_screen(run.edge, w, h, cd, along * sy + ct);
                    ui.set_mode(2);
                    ui.set_ccx(cx);
                    ui.set_ccy(cy);
                    ui.set_cr(r.min(20000.0));
                    ui.set_rim(1.0 - smoothstep(0.0, 0.35, ge));
                }
                "warp" => {
                    // the picture grows out of the edge; a ring of light at its front
                    let front = p * 1.15;
                    ui.set_mode(1);
                    ui.set_wa(front - 0.05);
                    ui.set_wb(front);
                    ui.set_rim(1.0);
                }
                _ => {
                    // a wide, soft wipe from the shared edge
                    let e = out_cubic(p);
                    let f = 0.6f32;
                    let front = 0.3 + (1.0 + f - 0.3) * e;
                    ui.set_mode(1);
                    ui.set_wa(front - f);
                    ui.set_wb(front);
                    ui.set_rim(0.0);
                }
            }
        } else {
            if still {
                return;
            }
            let fade = (p * std::f32::consts::PI).sin();
            match run.style.as_str() {
                "liquid" => {
                    let q = if run.reverse { 1.0 - p } else { p };
                    let pill = pill_and_drop(q, rr, sx);
                    // the capsule, in the frame, then on the screen
                    let t_c = along * sy + pill.t_mid;
                    let (cap_x, cap_y, cap_w, cap_h) = match run.edge {
                        0 | 2 => {
                            let x = if run.edge == 0 { pill.d_lo } else { w - pill.d_hi };
                            (x, t_c - pill.rad, pill.d_hi - pill.d_lo, 2.0 * pill.rad)
                        }
                        _ => {
                            let y = if run.edge == 1 { pill.d_lo } else { h - pill.d_hi };
                            (t_c - pill.rad, y, 2.0 * pill.rad, pill.d_hi - pill.d_lo)
                        }
                    };
                    let (dx, dy) = to_screen(run.edge, w, h, pill.drop_d, t_c);
                    ui.set_mode(4);
                    ui.set_cap_x(cap_x);
                    ui.set_cap_y(cap_y);
                    ui.set_cap_w(cap_w.max(0.0));
                    ui.set_cap_h(cap_h.max(0.0));
                    ui.set_drop_x(dx - pill.drop_r);
                    ui.set_drop_y(dy - pill.drop_r);
                    ui.set_drop_d(2.0 * pill.drop_r);
                    ui.set_shape_alpha(pill.alpha);
                }
                "warp" => {
                    // a lens ring running to (or from) the edge
                    let q = if run.reverse { p } else { 1.0 - p };
                    ui.set_mode(5);
                    ui.set_amp(fade * 0.75);
                    ui.set_ring_at(q);
                }
                _ => {
                    // a faint, wide glow at the shared edge
                    ui.set_mode(3);
                    ui.set_amp(0.16 * fade * 0.8);
                }
            }
        }
    }
}

impl DisplayConnect {
    fn motion(&self) -> f32 {
        if self.motion > 0.0 { self.motion } else { 1.0 }
    }

    fn sync(&mut self, st: &mut State, runs: Vec<Run>) {
        if !super::on() {
            return;
        }
        // screens whose effect is over go
        let gone: Vec<usize> = (0..self.wins.len()).rev().filter(|i| !runs.iter().any(|r| r.screen == self.wins[*i].output)).collect();
        for i in gone {
            let w = self.wins.remove(i);
            retire(st, &w.surface);
        }
        for run in runs {
            let key = format!("{}{}", run.id, if run.pending { "p" } else { "" });
            if let Some(w) = self.wins.iter_mut().find(|w| w.output == run.screen) {
                w.run = run;
                // (only a different run, or one just placed, plays: the list is rebuilt
                // whenever any screen's run starts or ends, which must not replay this one)
                if w.key != key {
                    w.key = key;
                    w.started = None;
                    w.finished = false;
                    w.waiting_since = None;
                    w.seen = Instant::now();
                    w.draw(0.0, 1.0, st.shell.cfg["appearance"]["reduceMotion"] == true);
                }
                continue;
            }
            let Some(wl) = st.output_named(&run.screen) else { continue };
            let size = kit::screen_size(st, &wl);
            let surface = overlay(st, &wl, &run.screen, NS, KeyboardInteractivity::None);
            let ui = ui::Connect::new().expect("Connect");
            st.shell.apply_theme(&ui.global::<ui::Theme>());
            ui.show().expect("show display connect");
            st.set_input_region(&surface, Some(&[]));
            let mut w = Win { output: run.screen.clone(), surface, ui, size, run, key, started: None, pictured: false, asked_picture: false, waiting_since: None, seen: Instant::now(), finished: false };
            if w.run.role != 0 {
                w.pictured = true; // (neighbours only draw light)
            } else {
                self.picture(st, &mut w);
            }
            w.draw(0.0, 1.0, st.shell.cfg["appearance"]["reduceMotion"] == true);
            self.wins.push(w);
        }
        self.keep_ticking(st);
    }

    /// The wallpaper as the desktop shows it (cropped to the screen, centred),
    /// decoded off the loop
    fn picture(&self, st: &State, w: &mut Win) {
        if w.asked_picture {
            return;
        }
        w.asked_picture = true;
        let path = tessera_core::wallpaper::showing_now(&st.shell.cfg, st.shell.look.material);
        let scale = st.output_named(&w.output).and_then(|o| st.outputs.info(&o)).map(|i| i.scale_factor).unwrap_or(1).max(1) as f32;
        let (bw, bh) = ((w.size.0 * scale).ceil() as u32, (w.size.1 * scale).ceil() as u32);
        let to = st.shell.sender(NAME);
        let out = w.output.clone();
        std::thread::spawn(move || {
            let px = tessera_core::picture::decode(&path, bw, bh);
            to(Box::new(Msg::Picture(out, px)));
        });
    }

    fn keep_ticking(&mut self, st: &State) {
        if self.wins.is_empty() {
            self.tick.stop();
            self.ticking = false;
        } else if !self.ticking {
            self.ticking = true;
            let to = st.shell.sender(NAME);
            self.tick.start(slint::TimerMode::Repeated, Duration::from_millis(16), move || to(Box::new(Msg::Tick)));
        }
    }

    fn step(&mut self, st: &mut State) {
        let still = st.shell.cfg["appearance"]["reduceMotion"] == true;
        let motion = self.motion();
        let now = Instant::now();
        for w in &mut self.wins {
            if w.finished {
                continue;
            }
            // a pending screen that never gets placed doesn't stay dark
            if w.run.pending && now.duration_since(w.seen) >= Duration::from_millis(3000) {
                w.finished = true;
                shell::ask("displays.finish", json!([w.run.id]));
                continue;
            }
            if w.started.is_none() && !w.run.pending {
                let waited = *w.waiting_since.get_or_insert(now);
                if w.pictured || now.duration_since(waited) >= Duration::from_millis(250) {
                    w.started = Some(now);
                }
            }
            let Some(t0) = w.started else { continue };
            let ms = now.duration_since(t0).as_secs_f32() * 1000.0;
            let lead = (w.run.lead * motion).round();
            let dur = (w.run.duration() * motion).round();
            let fade = (220.0 * motion).round();
            if ms < lead {
                w.draw(0.0, 1.0, still);
            } else if ms < lead + dur {
                w.draw(w.run.ease((ms - lead) / dur), 1.0, still);
            } else if ms < lead + dur + fade {
                w.draw(1.0, 1.0 - in_cubic((ms - lead - dur) / fade), still);
            } else {
                w.draw(1.0, 0.0, still);
                w.finished = true;
                shell::ask("displays.finish", json!([w.run.id]));
            }
        }
    }
}

impl Part for DisplayConnect {
    fn name(&self) -> &'static str {
        NAME
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match said(msg) {
            Ok(v) => {
                if let Some(m) = v["motion"].as_f64() {
                    self.motion = m as f32;
                }
                if let Some(list) = v["runs"].as_array() {
                    let runs: Vec<Run> = list.iter().filter_map(Run::from).collect();
                    self.sync(st, runs);
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Tick => self.step(st),
            Msg::Picture(output, px) => {
                if let Some(w) = self.wins.iter_mut().find(|w| w.output == output) {
                    w.pictured = true;
                    if let Some(px) = px {
                        w.ui.set_picture(slint::Image::from_rgb8(px));
                        w.ui.set_has_picture(true);
                    }
                }
            }
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            for w in &self.wins {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            }
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.wins.iter().position(|w| w.output == name) {
            let w = self.wins.remove(i);
            retire(st, &w.surface);
            self.keep_ticking(st);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.wins.iter().any(|w| Rc::ptr_eq(&w.surface, s)) {
            let name = s.output.clone();
            self.output_removed(st, &name);
        }
    }

    fn quit(&mut self, st: &mut State) {
        for w in std::mem::take(&mut self.wins) {
            retire(st, &w.surface);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(style: &str, role: i64) -> Run {
        Run::from(&json!({"id": 3, "screen": "DP-2", "role": role, "edge": 2, "focus": 0.4, "style": style, "lead": 620})).unwrap()
    }

    #[test]
    fn the_timings_are_the_qmls() {
        assert_eq!(run("liquid", 0).duration(), 1100.0);
        assert_eq!(run("liquid", 1).duration(), 1000.0);
        assert_eq!(run("extend", 0).duration(), 1050.0);
        assert_eq!(run("warp", 0).duration(), 950.0);
        assert_eq!(run("extend", 1).duration(), 800.0);
        assert_eq!(run("liquid", 0).ease(0.25), 0.25);
        assert!(run("warp", 0).ease(0.25) < 0.25);
        assert!((run("warp", 1).ease(0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_run_needs_a_screen() {
        assert!(Run::from(&json!({"id": 1})).is_none());
        let r = run("extend", 0);
        assert_eq!((r.id, r.edge, r.pending), (3, 2, false));
        assert_eq!(r.lead, 620.0);
    }

    #[test]
    fn frames_and_edges() {
        assert_eq!(unit(0, 1920.0, 1080.0), (1920.0, 1080.0));
        assert_eq!(unit(1, 1920.0, 1080.0), (1080.0, 1920.0));
        assert_eq!(to_screen(2, 1920.0, 1080.0, 100.0, 500.0), (1820.0, 500.0));
        assert_eq!(to_screen(3, 1920.0, 1080.0, 100.0, 500.0), (500.0, 980.0));
    }

    #[test]
    fn the_pill_pops_in_and_goes() {
        let rr = 36.0;
        assert_eq!(pill_and_drop(0.0, rr, 1920.0).alpha, 0.0);
        let mid = pill_and_drop(0.5, rr, 1920.0);
        assert!(mid.alpha > 0.9 && mid.drop_r > 0.0);
        assert!(pill_and_drop(1.0, rr, 1920.0).alpha < 0.01);
        // no drop before the split
        assert_eq!(pill_and_drop(0.2, rr, 1920.0).drop_r, 0.0);
    }
}
