//! Live captions on screen (rust.surfaces: captions), on the display you're
//! using, white on dark whatever the theme. Compare
//! shell/modules/captions/CaptionsWindow.qml and CaptionLines.qml:
//!   bar    a floating bar above the Dock; drag it anywhere (the place is kept)
//!   strip  a strip across the bottom that windows make room for
//!   video  over the bottom of the playing (else focused) window, up to three
//!          lines; clicks go through to the window
//! (In the island, the island shows them: nothing here.) It fades after a
//! while with nothing said.
//!
//! The words come from the shell's Captions service (captions.py, whisper.cpp:
//! never the microphone), sent by shell/modules/captions/CaptionsFeed.qml:
//! `{"to":"captions","state":{on, placement, screen, latest, previous, hint,
//! dim, bar:{x,y}, video:{x,y,w,h}, lines:{hint, previous, latest}}}` (the
//! bar's and strip's words and the video's CaptionLines', as the QML has
//! them). It asks `["off"]` (the × button: Captions.cfg.enabled = false) and
//! `["place", x, y]` (where the bar was let go: Captions.cfg.barX/barY).
//!
//! Differences from the QML: Slint elides only at the end, so the sentence
//! before is cut at its start and what's being said kept to two lines by a
//! character count (the QML: ElideLeft); the video's lines (StyledText, for
//! the dimmer sentence before) are in the font's regular weight at Slint's
//! own line height (QML: medium, 1.35).

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

use tessera_core::shell::Part;
use tessera_core::wayland::{Spec, State, Surface};
use serde_json::Value;
use slint::ComponentHandle;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::{Any, TypeId};
use std::rc::Rc;

const NAME: &str = "captions";
pub const NS: &str = "tessera:captions";

/// What the feed says
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Feed {
    pub on: bool,
    pub placement: String,
    pub screen: String,
    pub latest: String,
    pub previous: String,
    pub hint: bool,
    pub dim: bool,
    /// the bar's kept place (-1: not moved yet)
    pub bar: (f32, f32),
    /// the video's window, in the display's coordinates
    pub video: (f32, f32, f32, f32),
    /// CaptionLines': why there are no words yet, the sentence before, the words
    pub lines: (String, String, String),
}

impl Feed {
    pub fn from(v: &Value) -> Self {
        let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
        let f = |v: &Value, d: f64| v.as_f64().unwrap_or(d) as f32;
        Self {
            on: v["on"].as_bool().unwrap_or(false),
            placement: s(&v["placement"]),
            screen: s(&v["screen"]),
            latest: s(&v["latest"]),
            previous: s(&v["previous"]),
            hint: v["hint"].as_bool().unwrap_or(false),
            dim: v["dim"].as_bool().unwrap_or(false),
            bar: (f(&v["bar"]["x"], -1.0), f(&v["bar"]["y"], -1.0)),
            video: (f(&v["video"]["x"], 0.0), f(&v["video"]["y"], 0.0), f(&v["video"]["w"], 1280.0), f(&v["video"]["h"], 800.0)),
            lines: (s(&v["lines"]["hint"]), s(&v["lines"]["previous"]), s(&v["lines"]["latest"])),
        }
    }
    /// Where they show here ("" for none: off, or in the island)
    pub fn place(&self) -> &str {
        match (self.on, self.placement.as_str()) {
            (false, _) | (_, "island") => "",
            (_, "strip") => "strip",
            (_, "video") => "video",
            _ => "bar",
        }
    }
}

/// About how many characters fit in a width at a size (Inter's average)
pub fn fit(width: f32, size: f32) -> usize {
    ((width / (size * 0.52)).floor() as usize).max(8)
}

/// The end of a text, at most `n` characters, cut at a word ("…" before):
/// the QML's ElideLeft, by count
pub fn tail(text: &str, n: usize) -> String {
    let count = text.chars().count();
    if count <= n {
        return text.to_string();
    }
    let rest: String = text.chars().skip(count - n + 1).collect();
    let rest = match rest.find(' ') {
        Some(i) if i < rest.len() / 3 => rest[i + 1..].to_string(),
        _ => rest,
    };
    format!("…{rest}")
}

/// The bar's size and place: (width, its left, its distance from the bottom)
pub fn bar_place(sw: f32, sh: f32, h: f32, kept: (f32, f32)) -> (f32, f32, f32) {
    let w = (760.0f32).min(sw - 64.0);
    let cx = (if kept.0 >= 0.0 { kept.0 } else { (sw - w) / 2.0 }).min(sw - w).max(0.0);
    let cy = (if kept.1 >= 0.0 { kept.1 } else { 112.0 }).min(sh - h).max(0.0);
    (w, cx, cy)
}

/// Over the video: the text size, the window's width, and its left and top
/// for a box `h` high (QML: bigger on a big window, like a film's subtitles)
pub fn video_place(r: (f32, f32, f32, f32), h: f32) -> (f32, f32, i32, i32) {
    let (x, y, w, rh) = r;
    let size = (w / 42.0).min(28.0).max(18.0).round();
    let width = (w - 48.0).min(64.0 * size).max(240.0);
    let left = (x + (w - width) / 2.0).round().max(0.0) as i32;
    let top = (y + rh - h - (rh * 0.08).max(24.0)).round().max(0.0) as i32;
    (size, width, left, top)
}

/// Markdown's escapes, so captions are shown as said
pub fn escape(t: &str) -> String {
    let mut o = String::with_capacity(t.len());
    for c in t.chars() {
        if c.is_ascii_punctuation() {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

/// CaptionLines' words: the hint dimmed, or the sentence before dimmed and the words
pub fn lines_markup(hint: &str, previous: &str, latest: &str) -> (String, String) {
    const DIM: &str = "#ffffff99";
    if !hint.is_empty() {
        return (format!("<font color=\"{DIM}\">{}</font>", escape(hint)), hint.to_string());
    }
    let mut md = String::new();
    let mut plain = String::new();
    if !previous.is_empty() {
        md += &format!("<font color=\"{DIM}\">{}</font> ", escape(previous));
        plain += previous;
        plain += " ";
    }
    md += &escape(latest);
    plain += latest;
    (md, plain)
}

enum Ui {
    Bar(ui::CaptionsBar),
    Strip(ui::CaptionsStrip),
    Video(ui::CaptionsVideo),
}

impl Ui {
    fn hide(&self) -> Result<(), slint::PlatformError> {
        match self {
            Ui::Bar(u) => u.hide(),
            Ui::Strip(u) => u.hide(),
            Ui::Video(u) => u.hide(),
        }
    }
}

enum M {
    Dragging(bool),
    Released(f32, f32),
    Off,
}

struct Win {
    place: String,
    output: String,
    surface: Rc<Surface>,
    ui: Ui,
    a11y: Option<crate::a11y::A11y>,
    /// what the layer was last given: (full, w, h, left, bottom/top)
    sent: (bool, i32, i32, i32, i32),
    /// drawn again once mapped (a still strip's first frame is stretched)
    settled: bool,
}

#[derive(Default)]
pub struct Captions {
    feed: Feed,
    win: Option<Win>,
    dragging: bool,
}

impl Captions {
    fn screen(&self, st: &State) -> Option<(String, (f32, f32))> {
        let outs = st.outputs_now();
        let (o, n) = outs.iter().find(|(_, n)| *n == self.feed.screen).or(outs.first())?;
        let size = st.outputs.info(o).and_then(|i| i.logical_size).unwrap_or((1280, 800));
        Some((n.clone(), (size.0 as f32, size.1 as f32)))
    }

    /// The window for where they show now (made, moved or dropped)
    fn sync(&mut self, st: &mut State) {
        let place = self.feed.place().to_string();
        let screen = self.screen(st);
        let stale = self.win.as_ref().is_some_and(|w| place.is_empty() || w.place != place || screen.as_ref().is_some_and(|s| s.0 != w.output));
        if stale && !self.dragging {
            let w = self.win.take().unwrap();
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
        }
        let Some((output, _)) = screen else { return };
        if place.is_empty() {
            return;
        }
        if self.win.is_none() {
            self.make(st, &place, &output);
        }
        self.push(st);
    }

    fn make(&mut self, st: &mut State, place: &str, output: &str) {
        let Some(wl_output) = st.output_named(output) else { return };
        let (layer, anchor, size, exclusive) = match place {
            "strip" => (Layer::Top, Anchor::LEFT | Anchor::RIGHT | Anchor::BOTTOM, (0, 76), 76),
            "video" => (Layer::Overlay, Anchor::LEFT | Anchor::TOP, (240, 60), -1),
            _ => (Layer::Overlay, Anchor::LEFT | Anchor::BOTTOM, (760, 80), -1),
        };
        let surface = st.prepare_surface(Spec {
            layer,
            namespace: NS,
            output: Some((&wl_output, output)),
            anchor,
            size,
            exclusive,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let to = st.shell.sender(NAME);
        let x = crate::icons::glyph("x");
        let ui = match place {
            "strip" => {
                let u = ui::CaptionsStrip::new().expect("CaptionsStrip");
                st.shell.apply_theme(&u.global::<ui::Theme>());
                u.set_x_glyph(x.into());
                let t = to.clone();
                u.on_turn_off(move || t(Box::new(M::Off)));
                u.show().expect("show captions");
                Ui::Strip(u)
            }
            "video" => {
                let u = ui::CaptionsVideo::new().expect("CaptionsVideo");
                st.shell.apply_theme(&u.global::<ui::Theme>());
                u.show().expect("show captions");
                // (clicks go through to the window)
                st.set_input_region(&surface, Some(&[]));
                Ui::Video(u)
            }
            _ => {
                let u = ui::CaptionsBar::new().expect("CaptionsBar");
                st.shell.apply_theme(&u.global::<ui::Theme>());
                u.set_x_glyph(x.into());
                let t = to.clone();
                u.on_turn_off(move || t(Box::new(M::Off)));
                let t = to.clone();
                u.on_dragging(move |on| t(Box::new(M::Dragging(on))));
                let t = to.clone();
                u.on_released(move |x, y| t(Box::new(M::Released(x, y))));
                u.show().expect("show captions");
                Ui::Bar(u)
            }
        };
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { place: place.into(), output: output.into(), surface, ui, a11y, sent: (false, 0, 0, -1, -1), settled: false });
    }

    /// The words into the window, and the layer where it goes
    fn push(&mut self, st: &mut State) {
        let screen = self.screen(st).map(|s| s.1).unwrap_or((1280.0, 800.0));
        let dragging = self.dragging;
        let f = &self.feed;
        let Some(w) = &mut self.win else { return };
        match &w.ui {
            Ui::Bar(u) => {
                let (sw, sh) = screen;
                let h = u.get_bar_h().max(1.0);
                let (bw, cx, cy) = bar_place(sw, sh, h, f.bar);
                let n = fit(bw - 40.0, 20.0);
                u.set_previous(tail(&f.previous, n).into());
                u.set_latest(tail(&f.latest, 2 * n).into());
                u.set_hint(f.hint);
                u.set_dim(f.dim);
                u.set_bar_w(bw);
                u.set_sw(sw);
                u.set_sh(sh);
                u.set_cx(cx);
                u.set_cy_top(sh - cy - h);
                let want = if dragging { (true, 0, 0, 0, 0) } else { (false, bw.round() as i32, h.round() as i32, cx.round() as i32, cy.round() as i32) };
                if want != w.sent {
                    w.sent = want;
                    w.settled = false;
                    if want.0 {
                        w.surface.layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
                        w.surface.layer.set_margin(0, 0, 0, 0);
                        w.surface.show(0, 0);
                    } else {
                        w.surface.layer.set_anchor(Anchor::LEFT | Anchor::BOTTOM);
                        w.surface.layer.set_margin(0, 0, want.4, want.3);
                        w.surface.show(want.1 as u32, want.2 as u32);
                    }
                }
            }
            Ui::Strip(u) => {
                let n = fit(screen.0 - 128.0, 20.0);
                u.set_previous(tail(&f.previous, n).into());
                u.set_latest(tail(&f.latest, 2 * n).into());
                u.set_hint(f.hint);
                u.set_dim(f.dim);
            }
            Ui::Video(u) => {
                let (md, plain) = lines_markup(&f.lines.0, &f.lines.1, &f.lines.2);
                let h = u.get_box_h().max(1.0);
                let (size, width, left, top) = video_place(f.video, h);
                u.set_words(slint::StyledText::from_markdown(&md).unwrap_or_else(|_| slint::StyledText::from_plain_text(&plain)));
                u.set_plain(plain.into());
                u.set_dim(f.dim);
                u.set_size(size);
                let want = (false, width.round() as i32, h.round() as i32, left, top);
                if want != w.sent {
                    w.sent = want;
                    w.settled = false;
                    w.surface.layer.set_margin(top, 0, 0, left);
                    w.surface.show(want.1 as u32, want.2 as u32);
                }
            }
        }
    }
}

impl Part for Captions {
    fn name(&self) -> &'static str {
        NAME
    }

    fn data(&mut self, _st: &mut State, _what: TypeId) {}

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Value>() {
            Ok(v) => {
                if v.get("state").is_some() {
                    self.feed = Feed::from(&v["state"]);
                    self.sync(st);
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Off => crate::shell::ask(NAME, serde_json::json!(["off"])),
            M::Dragging(on) => {
                self.dragging = on;
                self.push(st);
            }
            M::Released(x, y) => {
                // (kept here at once: the feed says it again)
                let (sw, sh) = self.screen(st).map(|s| s.1).unwrap_or((1280.0, 800.0));
                let h = match self.win.as_ref().map(|w| &w.ui) {
                    Some(Ui::Bar(u)) => u.get_bar_h(),
                    _ => 80.0,
                };
                let _ = sw;
                let place = (x.round(), (sh - y - h).round());
                self.feed.bar = place;
                crate::shell::ask(NAME, serde_json::json!(["place", place.0, place.1]));
                self.dragging = false;
                self.push(st);
            }
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let w = self.win.take().unwrap();
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
            self.dragging = false;
            self.sync(st);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            let name = s.output.clone();
            self.output_removed(st, &name);
        }
    }

    /// The bar's and the video box's heights follow their words: the layer follows them
    fn after_frame(&mut self, st: &mut State) {
        let Some(w) = &mut self.win else { return };
        crate::picture::second_frame(&w.surface, &mut w.settled);
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, false);
        }
        let h = match &w.ui {
            Ui::Bar(u) => u.get_bar_h().round() as i32,
            Ui::Video(u) => u.get_box_h().round() as i32,
            Ui::Strip(_) => return,
        };
        if !w.sent.0 && h != w.sent.2 {
            self.push(st);
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn where_they_show() {
        let mut f = Feed::from(&json!({"on": true, "placement": "bar"}));
        assert_eq!(f.place(), "bar");
        f.placement = "island".into();
        assert_eq!(f.place(), "");
        f.placement = "video".into();
        assert_eq!(f.place(), "video");
        f.placement = "".into();
        assert_eq!(f.place(), "bar");
        f.on = false;
        assert_eq!(f.place(), "");
    }

    #[test]
    fn the_feed_reads_what_the_qml_sends() {
        let f = Feed::from(&json!({"on": true, "placement": "strip", "screen": "DP-1", "latest": "and then", "previous": "It rained.", "hint": false, "dim": true,
            "bar": {"x": 300, "y": -1}, "video": {"x": 10, "y": 20, "w": 1200, "h": 700}, "lines": {"hint": "", "previous": "It rained.", "latest": "and then"}}));
        assert_eq!((f.screen.as_str(), f.dim, f.bar), ("DP-1", true, (300.0, -1.0)));
        assert_eq!(f.video, (10.0, 20.0, 1200.0, 700.0));
        assert_eq!(f.lines.2, "and then");
    }

    #[test]
    fn long_words_keep_their_end() {
        assert_eq!(tail("short", 10), "short");
        let t = tail("the quick brown fox jumps over the lazy dog", 20);
        assert!(t.starts_with('…') && t.ends_with("lazy dog"), "{t}");
        assert!(t.chars().count() <= 20);
        assert!(fit(720.0, 20.0) > 60);
    }

    #[test]
    fn the_bar_sits_above_the_dock_until_moved() {
        // centred, 112 up
        assert_eq!(bar_place(1920.0, 1080.0, 80.0, (-1.0, -1.0)), (760.0, 580.0, 112.0));
        // kept, but kept on screen
        assert_eq!(bar_place(1920.0, 1080.0, 80.0, (1800.0, 2000.0)), (760.0, 1160.0, 1000.0));
        // a narrow screen: 64 px less than it
        assert_eq!(bar_place(700.0, 800.0, 80.0, (-1.0, -1.0)).0, 636.0);
    }

    #[test]
    fn subtitles_grow_with_the_window() {
        let (size, w, left, top) = video_place((0.0, 0.0, 1680.0, 1000.0), 60.0);
        assert_eq!(size, 28.0);
        assert_eq!(w, (1680.0f32 - 48.0).min(64.0 * 28.0));
        assert_eq!(left, ((1680.0 - w) / 2.0).round() as i32);
        assert_eq!(top, (1000.0f32 - 60.0 - 80.0).round() as i32);
        // a small window: 18 px text, at least 240 wide
        let (size, w, _, _) = video_place((0.0, 0.0, 300.0, 200.0), 40.0);
        assert_eq!((size, w), (18.0, 252.0));
    }

    #[test]
    fn words_go_through_markdown_as_said() {
        assert_eq!(escape("*not* bold <b> #1"), "\\*not\\* bold \\<b\\> \\#1");
        let (md, plain) = lines_markup("", "Before.", "now *this*");
        assert_eq!(plain, "Before. now *this*");
        assert!(md.starts_with("<font color=\"#ffffff99\">Before\\.</font> now \\*this\\*"), "{md}");
        let (md, plain) = lines_markup("Captions appear here", "x", "y");
        assert_eq!(plain, "Captions appear here");
        assert!(md.contains("#ffffff99"));
    }
}
