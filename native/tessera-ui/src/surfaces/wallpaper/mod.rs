//! The wallpaper picker (rust.surfaces: wallpaper): a strip of thumbnails
//! above the dock, on the screen it was opened on. Compare
//! shell/modules/wallpaper/WallpaperPicker.qml. A panel: the QML shell still
//! owns which panel is open (Panels.qml: its shortcut, IPC, Spotlight), and
//! RustUi.qml forwards its open and close here (`Part::command`); when it
//! closes itself (Esc, Open folder) it says so (`shell::closed`). Choosing a
//! picture and opening the folder are the shell's (`wallpaper.set`, its
//! Wallpaper.set: picture chosen, dynamic and live off; `wallpaper.openFolder`,
//! Files.openFolder); the background follows config.json from there.
//!
//! Thumbnails are decoded off the UI thread, cropped to the thumbnail's
//! shape at its pixels, and only for the ones in view (and a few either
//! side); ones scrolled far away are let go, and all of them when it closes.
//! The strip scrolls with the wheel, a touchpad (gliding on after the fingers
//! lift, as Momentum.qml) and the arrow keys, and drags and flicks with the
//! pointer, as the QML's ListView. Where it differs: Slint gets no scroll
//! phases or wheel clicks from this platform, so a mouse wheel spun fast
//! glides a little too (Momentum.qml leaves the wheel alone); a flick stops at
//! the ends instead of overshooting; the motion doesn't follow Glass's Snappy
//! pace or cool-down's slower motion (Theme.motion.pace).

pub mod files;

use crate::shell::{self, Command, Part};
use crate::surfaces::background::picture::{self, Pool};
use crate::surfaces::background::wallpaper::{self, Bundled, Context, Day};
use serde_json::json;
use crate::theme;
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::Any;
use std::rc::Rc;

const NAME: &str = "wallpaper";
/// (not the QML's tessera:wallpaper, the desktop's namespace: the glass
/// plugin keeps shapes by screen and namespace)
pub const NS: &str = "tessera:wallpicker";
const M: u32 = 24; // room for the shadow round the panel
const HEIGHT: u32 = 124 + 78 + 2 * M; // a thumbnail (220 × 9/16), the title, the shadow
/// Thumbnails kept either side of the ones in view (decoded ahead), and the
/// distance past which they're let go
const AHEAD: usize = 3;
const KEEP: usize = 12;

enum Msg {
    Listed(u64, Vec<String>),
    Thumb(u64, usize, String, Option<slint::SharedPixelBuffer<slint::Rgb8Pixel>>),
    Apply(i32),
    OpenFolder,
    Dismissed,
    Closed,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::WallpaperPicker,
    a11y: Option<crate::a11y::A11y>,
    shown: bool,
    glass_sent: String,
    settled: bool,
}

#[derive(Default)]
pub struct WallpaperPicker {
    win: Option<Win>,
    /// the pictures listed, and the generation of that listing
    files: Vec<String>,
    listing: u64,
    model: Option<Rc<VecModel<ui::Thumb>>>,
    /// which thumbnails are decoded
    loaded: Vec<bool>,
    /// the range in view when the thumbnails were last asked for
    range: (usize, usize),
    pool: Option<Pool>,
    bundled: Option<Bundled>,
}

/// The output's size in logical pixels (the panel's width follows it)
fn logical_width(st: &State, output: &WlOutput) -> u32 {
    let Some(info) = st.outputs.info(output) else { return 1920 };
    if let Some((w, _)) = info.logical_size {
        return w.max(1) as u32;
    }
    let w = info.modes.iter().find(|m| m.current).map_or(1920, |m| m.dimensions.0);
    (w / info.scale_factor.max(1)).max(1) as u32
}

/// The panel's width: the screen less 40 either side, at most 1200, and the shadow's room
pub fn panel_width(screen: u32) -> u32 {
    screen.saturating_sub(80).min(1200) + 2 * M
}

/// Its bottom margin: above the dock when there is one (as the QML: the
/// dock's icons, its padding and gap, then 12), less the shadow's room
pub fn bottom_margin(cfg: &serde_json::Value) -> i32 {
    let dock = cfg["dock"]["enabled"].as_bool().unwrap_or(true);
    let icon = cfg["dock"]["iconSize"].as_i64().unwrap_or(48) as i32;
    (if dock { icon + 14 + 6 + 12 } else { 12 }) - M as i32
}

/// The thumbnails to have: the ones in view and AHEAD either side (in view
/// first, then outwards), given the strip's scroll and width in thumbnail steps
pub fn wanted(offset: f32, width: f32, step: f32, count: usize) -> Vec<usize> {
    if count == 0 || step <= 0.0 {
        return Vec::new();
    }
    let first = (offset / step).floor().max(0.0) as usize;
    let last = (((offset + width) / step).ceil().max(1.0) as usize - 1).min(count - 1).max(first.min(count - 1));
    let first = first.min(count - 1);
    let mut out: Vec<usize> = (first..=last).collect();
    for d in 1..=AHEAD {
        if last + d < count {
            out.push(last + d);
        }
        if first >= d {
            out.push(first - d);
        }
    }
    out
}

impl WallpaperPicker {
    fn day(&self) -> Day {
        let (r, s) = wallpaper::sun();
        Day::now(r, s)
    }

    /// The picture showing now (the chosen ring goes round it)
    fn showing(&mut self, st: &State) -> String {
        let material = st.shell.look.material;
        let cfg = &st.shell.cfg;
        let tf = wallpaper::time_folder(cfg);
        let folder = if tf.is_empty() { Vec::new() } else { wallpaper::folder_pictures(tf) };
        let live = wallpaper::live(cfg);
        let poster = (!live.is_empty()).then(|| wallpaper::poster_path(&wallpaper::cache_dir(), live)).filter(|p| p.exists()).map(|p| p.to_string_lossy().into_owned());
        let day = self.day();
        let bundled = self.bundled.get_or_insert_with(Bundled::find);
        let file = |n: &str| bundled.file(material, n);
        wallpaper::picture(cfg, &Context { material, day, bundled: &file, folder: &folder, poster: poster.as_deref() })
    }

    fn applied(&mut self, st: &State) -> i32 {
        let now = self.showing(st);
        files::index_of(&self.files, &now).map_or(-1, |i| i as i32)
    }

    fn open(&mut self, st: &mut State, screen: Option<String>) {
        let output = screen.or_else(|| self.win.as_ref().map(|w| w.output.clone())).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(output) = output else { return };
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            st.shell.glass.set(&w.output, NS, &[]);
            st.drop_surface(&w.surface);
        }
        if self.win.is_none() {
            let Some(wl_output) = st.output_named(&output) else { return };
            let width = panel_width(logical_width(st, &wl_output));
            let surface = st.prepare_surface(Spec {
                layer: Layer::Overlay,
                namespace: NS,
                output: Some((&wl_output, &output)),
                anchor: Anchor::BOTTOM,
                size: (width, HEIGHT),
                exclusive: -1,
                keyboard: KeyboardInteractivity::Exclusive,
                margin_left: 0,
            });
            let ui = ui::WallpaperPicker::new().expect("WallpaperPicker");
            st.shell.apply_theme(&ui.global::<ui::Theme>());
            let to = st.shell.sender(NAME);
            let (t1, t2, t3, t4) = (to.clone(), to.clone(), to.clone(), to.clone());
            ui.on_apply(move |i| t1(Box::new(Msg::Apply(i))));
            ui.on_open_folder(move || t2(Box::new(Msg::OpenFolder)));
            ui.on_dismissed(move || t3(Box::new(Msg::Dismissed)));
            ui.on_closed(move || t4(Box::new(Msg::Closed)));
            ui.show().expect("show wallpaper picker");
            let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
            self.win = Some(Win { output: output.clone(), surface, ui, a11y, shown: false, glass_sent: String::new(), settled: false });
        }
        let home = std::env::var("HOME").unwrap_or_default();
        let dir = wallpaper::directory(&st.shell.cfg);
        let material = st.shell.look.material;
        let w = self.win.as_mut().unwrap();
        w.ui.set_on_glass(st.shell.plugin && !material);
        w.surface.layer.set_margin(0, 0, bottom_margin(&st.shell.cfg), 0);
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        let width = st.output_named(&output).map_or(1200 + 2 * M, |o| panel_width(logical_width(st, &o)));
        w.surface.show(width, HEIGHT);
        w.ui.set_folder(if home.is_empty() { dir.clone() } else { dir.replacen(&home, "~", 1) }.into());
        if !w.shown {
            w.shown = true;
            w.ui.set_shown(true);
            // (listed afresh at every open, as the QML's folder models)
            self.listing += 1;
            let (gen, tx, dirs) = (self.listing, st.tx.clone(), self.bundled.get_or_insert_with(Bundled::find).dirs(material));
            std::thread::spawn(move || {
                let list = files::list(&dir, &dirs);
                let _ = tx.send(crate::Msg::Part(NAME, Box::new(Msg::Listed(gen, list))));
            });
        }
    }

    fn listed(&mut self, st: &mut State, gen: u64, list: Vec<String>) {
        if gen != self.listing || self.win.is_none() {
            return;
        }
        self.files = list;
        let names = self.files.iter().map(|p| ui::Thumb { name: p.rsplit('/').next().unwrap_or(p).into(), image: Default::default(), ready: false });
        let model = Rc::new(VecModel::from(names.collect::<Vec<_>>()));
        self.loaded = vec![false; self.files.len()];
        self.range = (usize::MAX, 0);
        let applied = self.applied(st);
        let w = self.win.as_ref().unwrap();
        w.ui.set_thumbs(ModelRc::from(model.clone()));
        w.ui.set_applied(applied);
        w.ui.set_current(applied.max(0));
        w.ui.set_listed(true);
        w.ui.invoke_reveal();
        self.model = Some(model);
        self.pool.get_or_insert_with(|| Pool::new(2, "tessera-thumbs"));
        self.thumbs(st);
    }

    /// Ask for the thumbnails in view, let go of the far ones
    fn thumbs(&mut self, st: &mut State) {
        let (Some(w), Some(model)) = (&self.win, &self.model) else { return };
        let (offset, width, step) = (w.ui.get_offset(), w.ui.get_list_width(), w.ui.get_step());
        let want = wanted(offset, width, step, self.files.len());
        let range = (want.iter().copied().min().unwrap_or(0), want.iter().copied().max().unwrap_or(0));
        if range == self.range {
            return;
        }
        self.range = range;
        for i in 0..self.loaded.len() {
            let far = i + KEEP < range.0 || i > range.1 + KEEP;
            if far && self.loaded[i] {
                self.loaded[i] = false;
                if let Some(mut t) = model.row_data(i) {
                    t.image = Default::default();
                    t.ready = false;
                    model.set_row_data(i, t);
                }
            }
        }
        let scale = w.surface.window().scale_factor();
        let (bw, bh) = ((220.0 * scale).round() as u32, (124.0 * scale).round() as u32);
        let mut jobs: Vec<Box<dyn FnOnce() + Send>> = Vec::new();
        // (every one in view not decoded yet: what was waiting is forgotten
        // below, so it's asked again here)
        for i in want {
            if self.loaded[i] {
                continue;
            }
            let (path, gen, tx) = (self.files[i].clone(), self.listing, st.tx.clone());
            jobs.push(Box::new(move || {
                let px = picture::decode(&path, bw, bh);
                let _ = tx.send(crate::Msg::Part(NAME, Box::new(Msg::Thumb(gen, i, path, px))));
            }));
        }
        if let Some(p) = &self.pool {
            p.replace(jobs);
        }
    }

    fn thumb(&mut self, gen: u64, i: usize, path: String, px: Option<slint::SharedPixelBuffer<slint::Rgb8Pixel>>) {
        if gen != self.listing || self.files.get(i) != Some(&path) || self.loaded[i] {
            return;
        }
        // (an unreadable one stays an empty tile, as the QML's)
        self.loaded[i] = true;
        let Some(model) = &self.model else { return };
        let Some(mut t) = model.row_data(i) else { return };
        if let Some(px) = px {
            t.image = slint::Image::from_rgb8(px);
            t.ready = true;
            model.set_row_data(i, t);
        }
    }

    fn apply(&mut self, i: i32) {
        let Some(path) = usize::try_from(i).ok().and_then(|i| self.files.get(i)).cloned() else { return };
        // (the shell's Wallpaper.set writes config.json)
        shell::ask("wallpaper.set", json!([path]));
        // (at once; config.json's change confirms it)
        if let Some(w) = &self.win {
            w.ui.set_applied(i);
        }
    }

    /// Close (the animation runs, then `closed`); `tell_shell`: the close
    /// started here, so the shell's panel state follows
    fn close(&mut self, tell_shell: bool) {
        let Some(w) = self.win.as_mut() else { return };
        if w.shown {
            w.shown = false;
            w.ui.set_shown(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            w.surface.wl.commit();
        }
        if tell_shell {
            shell::closed(NAME);
        }
    }

    /// The close animation ended: everything goes (window, thumbnails, threads)
    fn closed(&mut self, st: &mut State) {
        if self.win.as_ref().is_some_and(|w| !w.shown) {
            let w = self.win.take().unwrap();
            st.shell.glass.set(&w.output, NS, &[]);
            st.drop_surface(&w.surface);
            self.model = None;
            self.files.clear();
            self.loaded.clear();
            self.pool = None;
            self.listing += 1;
        }
    }

    /// Open folder: the panel closes, the shell opens the folder (its file
    /// manager setting)
    fn open_folder(&mut self) {
        self.close(true);
        shell::ask("wallpaper.openFolder", json!([]));
    }
}

impl Part for WallpaperPicker {
    fn name(&self) -> &'static str {
        NAME
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let w = self.win.take().unwrap();
            st.drop_surface(&w.surface);
            self.model = None;
            self.pool = None;
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if self.win.is_none() {
            return;
        }
        if look {
            let w = self.win.as_mut().unwrap();
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.ui.set_on_glass(st.shell.plugin && !st.shell.look.material);
            w.glass_sent.clear();
        }
        let applied = self.applied(st);
        if let Some(w) = &self.win {
            w.ui.set_applied(applied);
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = self.win.as_mut() {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.ui.set_on_glass(st.shell.plugin && !st.shell.look.material);
            w.glass_sent.clear();
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<serde_json::Value>() {
            // From the shell (RustUi.tell): the day's sunrise and sunset (the
            // ring round the picture a time-of-day wallpaper shows now)
            Ok(v) => {
                if wallpaper::told_sun(&v) && self.win.is_some() {
                    let applied = self.applied(st);
                    self.win.as_ref().unwrap().ui.set_applied(applied);
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Listed(gen, list) => self.listed(st, gen, list),
            Msg::Thumb(gen, i, path, px) => self.thumb(gen, i, path, px),
            Msg::Apply(i) => self.apply(i),
            Msg::OpenFolder => self.open_folder(),
            Msg::Dismissed => self.close(true),
            Msg::Closed => self.closed(st),
        }
    }

    /// Panels opened or closed it (RustUi.qml forwards them)
    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.open(st, cmd.screen.clone()),
            "close" => self.close(false),
            _ => {}
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            let name = s.output.clone();
            self.output_removed(st, &name);
        }
    }

    /// The thumbnails in view (the strip scrolls), the glass shape as the
    /// panel moves and fades, the accessibility tree
    fn after_frame(&mut self, st: &mut State) {
        if self.win.is_none() {
            return;
        }
        self.thumbs(st);
        let kbd = st.keyboard_surface();
        let w = self.win.as_mut().unwrap();
        crate::surfaces::background::second_frame(&w.surface, &mut w.settled);
        let focused = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, focused);
        }
        if !st.shell.plugin || st.shell.look.material || !w.surface.mapped.get() {
            return;
        }
        let ui = &w.ui;
        let p = ui.get_progress();
        let shape = theme::glass_shape(&st.shell.look, ui.get_panel_x(), ui.get_panel_y(), ui.get_panel_width(), ui.get_panel_height(), 26.0, p, 1.0);
        let key = format!("{shape:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, if p > 0.002 { std::slice::from_ref(&shape) } else { &[] });
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    fn stats(&self) {
        if let Some(w) = &self.win {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report("wallpaper"));
        }
    }

    fn quit(&mut self, _: &mut State) {
        if let Some(w) = &self.win {
            crate::services::hypr::Glass::clear_now(&w.output, NS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sized_as_the_qml() {
        assert_eq!(HEIGHT, 250);
        assert_eq!(panel_width(1920), 1200 + 48);
        assert_eq!(panel_width(1280), 1200 + 48);
        assert_eq!(panel_width(1000), 920 + 48);
        assert_eq!(bottom_margin(&json!({})), 48 + 14 + 6 + 12 - 24);
        assert_eq!(bottom_margin(&json!({"dock": {"iconSize": 64}})), 64 + 14 + 6 + 12 - 24);
        assert_eq!(bottom_margin(&json!({"dock": {"enabled": false}})), -12);
    }

    #[test]
    fn thumbnails_in_view_first_then_either_side() {
        // a 1172-wide strip of 232-wide steps, at the start: 0..=5 in view, then 6, 7, 8
        assert_eq!(wanted(0.0, 1172.0, 232.0, 40), vec![0, 1, 2, 3, 4, 5, 6, 7, 8]);
        // scrolled: the ones before too
        assert_eq!(wanted(2320.0, 1172.0, 232.0, 40), vec![10, 11, 12, 13, 14, 15, 16, 9, 17, 8, 18, 7]);
        // near the end, and fewer than fit
        assert_eq!(wanted(0.0, 1172.0, 232.0, 2), vec![0, 1]);
        assert_eq!(wanted(0.0, 1172.0, 232.0, 0), Vec::<usize>::new());
    }
}
