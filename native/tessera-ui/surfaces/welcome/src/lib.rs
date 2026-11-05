//! The welcome screen (rust.surfaces: welcome), compare
//! shell/modules/welcome/Welcome.qml: once, on a new setup (or from
//! Settings → About, `ipc call welcome open`), over your wallpaper, blurred,
//! a card that asks three things, one a step: the look (Glass or Material,
//! where its files are), light, dark or auto, and a wallpaper; then "Start
//! using Tessera". Enter continues, Esc goes back, Tab moves between the
//! choices (Space or Enter picks one).
//!
//! When it opens is still Welcome.qml's (its timer on a new setup, its IPC,
//! Panels.welcomeRequested); it tells this part `{"to":"welcome","open":
//! true,"screen":…,"look":…,"materialAvailable":…,"tahoeDir":…,
//! "materialDir":…}` and `{"open":false}`. This part never changes the
//! system itself: light or dark, a wallpaper, the file picker, "later" and
//! the look switch at the end are asked of the shell (flow.rs's `Ask`, sent
//! as `welcome.*` calls that Welcome.qml answers with its own functions:
//! Config, Wallpaper, FilePicker, switch-look.sh), and its tests check what
//! would be asked with a recorder in the shell's place.
//!
//! Where it differs: Slint has no blur, so the wallpaper behind the card is
//! decoded very small and drawn stretched and smoothed (MultiEffect's blur
//! in the QML); the look previews' Material shapes are the polar outlines of
//! src/surfaces/shapes.rs; the Glass card's caption drops "like macOS" (no
//! company names in what the UI says).

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod flow;

use tessera_core::shell::{self, Part};
use tessera_core::{picture, wallpaper};
use tessera_core::surfaces::kit::{hex, hm_short};
tessera_core::gl_fn!(ui);
use tessera_core::wayland::{Spec, State, Surface};
use flow::{Flow, Wall};
use serde_json::Value;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::Any;
use std::rc::Rc;

const NAME: &str = "welcome";
const NS: &str = "tessera-welcome";

/// The shell's side of flow::Ask: each call by its name (RustUi.qml's list)
struct ShellAsk;
impl flow::Ask for ShellAsk {
    fn ask(&mut self, call: &'static str, args: Value) {
        match call {
            "welcome.mode" => shell::ask("welcome.mode", args),
            "welcome.wall" => shell::ask("welcome.wall", args),
            "welcome.pick" => shell::ask("welcome.pick", args),
            "welcome.later" => shell::ask("welcome.later", args),
            "welcome.finish" => shell::ask("welcome.finish", args),
            _ => {}
        }
    }
}

enum Msg {
    Next,
    Back,
    Later,
    Look(String),
    Mode(String),
    Wall(i32),
    Own,
    Closed,
    Clock,
    /// a decoded picture: (generation, which, pixels): which is "backdrop",
    /// "mini", "own" or a wallpaper's index
    Picture(u64, String, Option<slint::SharedPixelBuffer<slint::Rgb8Pixel>>),
    Seed(u64, String),
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::Welcome,
    a11y: Option<crate::a11y::A11y>,
    shown: bool,
    settled: bool,
}

#[derive(Default)]
pub struct Welcome {
    win: Option<Win>,
    flow: Option<Flow>,
    walls: Vec<Wall>,
    gen: u64,
    clock: Option<slint::Timer>,
    showing: String,
}

/// The Material preview's colours for a seed (LookPreview's `m`)
fn mini_colors(seed: &str, dark: bool, style: &str) -> ui::MiniColors {
    let roles = crate::theme::material_roles(seed, dark, style);
    let r = |k: &str| hex(roles.iter().find(|(n, _)| n == k).map_or("#808080", |(_, v)| v.as_str()));
    ui::MiniColors {
        primary: r("primary"),
        primary_container: r("primaryContainer"),
        on_primary_container: r("onPrimaryContainer"),
        secondary_container: r("secondaryContainer"),
        tertiary: r("tertiary"),
        tertiary_container: r("tertiaryContainer"),
        surface_container: r("surfaceContainer"),
        surface_container_high: r("surfaceContainerHigh"),
        surface_container_highest: r("surfaceContainerHighest"),
        on_surface: r("onSurface"),
        on_surface_variant: r("onSurfaceVariant"),
    }
}

/// The backdrop's blur: a Gaussian over the picture decoded at a sixth of
/// the screen (MultiEffect's blur 48 px at full size, about 8 here)
fn soften(px: slint::SharedPixelBuffer<slint::Rgb8Pixel>) -> slint::SharedPixelBuffer<slint::Rgb8Pixel> {
    let (w, h) = (px.width(), px.height());
    let Some(img) = image::RgbImage::from_raw(w, h, px.as_bytes().to_vec()) else { return px };
    let out = image::imageops::blur(&img, 8.0);
    slint::SharedPixelBuffer::<slint::Rgb8Pixel>::clone_from_slice(out.as_raw(), w, h)
}

impl Welcome {
    fn sync(&self, st: &State) {
        let (Some(w), Some(f)) = (&self.win, &self.flow) else { return };
        let ui = &w.ui;
        ui.set_step(f.step as i32);
        ui.set_title_text(f.title().into());
        ui.set_help(f.help().into());
        ui.set_look(f.look.clone().into());
        ui.set_material_available(f.material_available);
        ui.set_mode(flow::mode(&st.shell.cfg).into());
        let own = flow::own_wall(&st.shell.cfg, &self.walls);
        ui.set_own_selected(own);
        let model = ui.get_walls();
        for (i, wall) in self.walls.iter().enumerate() {
            if let Some(mut row) = model.row_data(i) {
                let sel = flow::is_wall(&st.shell.cfg, wall);
                if row.selected != sel {
                    row.selected = sel;
                    model.set_row_data(i, row);
                }
            }
        }
    }

    /// Decode what it shows (off the UI thread): the backdrop at a sixth of
    /// the screen, blurred (MultiEffect blur 48 in the QML; soften()) and
    /// drawn stretched; the previews at their size
    fn pictures(&mut self, st: &State, size: (f32, f32), scale: f32) {
        self.gen += 1;
        let gen = self.gen;
        let showing = wallpaper::showing_now(&st.shell.cfg, st.shell.look.material);
        self.showing = showing.clone();
        let own = flow::own_wall(&st.shell.cfg, &self.walls);
        let mut jobs: Vec<(String, String, u32, u32)> = vec![
            ("backdrop".into(), showing.clone(), ((size.0 / 6.0).max(8.0)) as u32, ((size.1 / 6.0).max(8.0)) as u32),
            ("mini".into(), showing.clone(), (360.0 * scale) as u32, (225.0 * scale) as u32),
        ];
        if own {
            jobs.push(("own".into(), showing.clone(), 400, 224));
        }
        for (i, w) in self.walls.iter().enumerate() {
            if !w.preview.is_empty() {
                jobs.push((i.to_string(), w.preview.clone(), 400, 224));
            }
        }
        let tx = st.tx.clone();
        std::thread::spawn(move || {
            for (which, path, bw, bh) in jobs {
                let px = picture::decode(&path, bw.max(1), bh.max(1));
                let px = if which == "backdrop" { px.map(soften) } else { px };
                if tx.send(tessera_core::Msg::Part(NAME, Box::new(Msg::Picture(gen, which, px)))).is_err() {
                    return;
                }
            }
        });
        // The Material preview's colours: the look's own seed, the setting,
        // else the wallpaper's (LookPreview.seed)
        let set = st.shell.cfg["appearance"]["materialSeed"].as_str().unwrap_or("wallpaper");
        let hexseed = set.len() == 7 && set.starts_with('#');
        if let Some(w) = &self.win {
            let look = &st.shell.look;
            let style = st.shell.cfg["appearance"]["materialStyle"].as_str().unwrap_or("tonalSpot");
            let seed = if look.material || hexseed { look.seed.clone() } else { "#6750A4".into() };
            w.ui.set_mini(mini_colors(&seed, look.dark, style));
            if !look.material && !hexseed {
                let tx = st.tx.clone();
                std::thread::spawn(move || {
                    if let Some(s) = crate::seed::from_picture(std::path::Path::new(&showing)) {
                        let _ = tx.send(tessera_core::Msg::Part(NAME, Box::new(Msg::Seed(gen, s))));
                    }
                });
            }
        }
    }

    fn open(&mut self, st: &mut State, v: &Value) {
        let screen = v["screen"].as_str().unwrap_or("").to_string();
        let output = Some(screen).filter(|s| st.output_named(s).is_some()).or_else(|| self.win.as_ref().map(|w| w.output.clone())).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(output) = output else { return };
        let current = st.shell.cfg["appearance"]["look"].as_str().unwrap_or("glass").to_string();
        let look = v["look"].as_str().unwrap_or(&current).to_string();
        if self.win.as_ref().is_some_and(|w| w.shown) {
            return;
        }
        let mut f = Flow::new(&current, v["materialAvailable"].as_bool().unwrap_or(st.shell.look.material));
        f.pick_look(&look);
        self.flow = Some(f);
        self.walls = flow::walls(v["tahoeDir"].as_str().unwrap_or(""), v["materialDir"].as_str().unwrap_or(""), st.shell.look.dark);
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            w.surface.hide(); // (unmapped now: dropping the window alone leaves it on screen)
            st.drop_surface(&w.surface);
        }
        if self.win.is_none() {
            let Some(wl_output) = st.output_named(&output) else { return };
            let surface = st.prepare_surface(Spec {
                layer: Layer::Overlay,
                namespace: NS,
                output: Some((&wl_output, &output)),
                anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                size: (0, 0),
                exclusive: -1,
                keyboard: KeyboardInteractivity::Exclusive,
                margin_left: 0,
            });
            let ui = ui::Welcome::new().expect("Welcome");
            st.shell.apply_theme(&ui.global::<ui::Theme>());
            let to = st.shell.sender(NAME);
            macro_rules! on {
                ($set:ident, $m:expr) => {{
                    let t = to.clone();
                    ui.$set(move || t(Box::new($m)));
                }};
            }
            on!(on_next, Msg::Next);
            on!(on_back, Msg::Back);
            on!(on_later, Msg::Later);
            on!(on_pick_own, Msg::Own);
            on!(on_closed, Msg::Closed);
            let t = to.clone();
            ui.on_pick_look(move |l| t(Box::new(Msg::Look(l.to_string()))));
            let t = to.clone();
            ui.on_pick_mode(move |m| t(Box::new(Msg::Mode(m.to_string()))));
            let t = to.clone();
            ui.on_pick_wall(move |i| t(Box::new(Msg::Wall(i))));
            ui.set_image_icon(gl("image"));
            ui.set_cookie(crate::surfaces::shapes::outline("cookie9").into());
            ui.set_sunny(crate::surfaces::shapes::outline("sunny").into());
            ui.show().expect("show welcome");
            let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
            self.win = Some(Win { output: output.clone(), surface, ui, a11y, shown: false, settled: false });
        }
        let wl_output = st.output_named(&output);
        let size = wl_output.as_ref().map_or((1920.0, 1080.0), |o| crate::surfaces::kit::screen_size(st, o));
        let w = self.win.as_mut().unwrap();
        let scale = w.surface.window().scale_factor().max(1.0);
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        w.surface.show(0, 0);
        st.set_input_region(&w.surface, None);
        let walls: Vec<ui::WWall> = self.walls.iter().map(|x| ui::WWall { name: x.name.clone().into(), image: Default::default(), selected: false }).collect();
        w.ui.set_walls(ModelRc::new(VecModel::from(walls)));
        w.ui.set_still(flow::still(&st.shell.cfg));
        w.ui.set_clock(hm_short(&st.shell.cfg).into());
        w.shown = true;
        w.ui.set_shown(true);
        w.ui.invoke_focus_card();
        let tx = st.tx.clone();
        let t = slint::Timer::default();
        t.start(slint::TimerMode::Repeated, std::time::Duration::from_secs(30), move || {
            let _ = tx.send(tessera_core::Msg::Part(NAME, Box::new(Msg::Clock)));
        });
        self.clock = Some(t);
        self.pictures(st, size, scale);
        self.sync(st);
    }

    /// Put away (the fade out runs, then `closed`)
    fn hide(&mut self, st: &mut State) {
        let Some(w) = self.win.as_mut() else { return };
        if w.shown {
            w.shown = false;
            w.ui.set_shown(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            st.set_input_region(&w.surface, Some(&[]));
            w.surface.wl.commit();
        }
        self.clock = None;
    }

    fn closed(&mut self, st: &mut State) {
        if self.win.as_ref().is_some_and(|w| !w.shown) {
            let w = self.win.take().unwrap();
            w.surface.hide(); // (unmapped now: dropping the window alone leaves it on screen)
            st.drop_surface(&w.surface);
            self.gen += 1;
        }
    }

    fn picture(&mut self, gen: u64, which: String, px: Option<slint::SharedPixelBuffer<slint::Rgb8Pixel>>) {
        let (Some(w), Some(px)) = (&self.win, px) else { return };
        if gen != self.gen {
            return;
        }
        let img = slint::Image::from_rgb8(px);
        match which.as_str() {
            "backdrop" => w.ui.set_backdrop(img),
            "mini" => w.ui.set_wallpaper(img),
            "own" => w.ui.set_own_image(img),
            i => {
                let Ok(i) = i.parse::<usize>() else { return };
                let model = w.ui.get_walls();
                if let Some(mut row) = model.row_data(i) {
                    row.image = img;
                    model.set_row_data(i, row);
                }
            }
        }
    }
}

impl Part for Welcome {
    fn name(&self) -> &'static str {
        NAME
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Value>() {
            Ok(v) => {
                match v["open"].as_bool() {
                    Some(true) => self.open(st, &v),
                    Some(false) => self.hide(st),
                    None => {}
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<Msg>() else { return };
        let mut ask = ShellAsk;
        match *m {
            Msg::Next => {
                if let Some(f) = self.flow.as_mut() {
                    f.next(&mut ask);
                    if f.done {
                        self.hide(st);
                    }
                }
            }
            Msg::Back => {
                if let Some(f) = self.flow.as_mut() {
                    f.back();
                }
            }
            Msg::Later => {
                if let Some(f) = self.flow.as_mut() {
                    f.later(&mut ask);
                    self.hide(st);
                }
            }
            Msg::Look(l) => {
                if let Some(f) = self.flow.as_mut() {
                    f.pick_look(&l);
                }
            }
            Msg::Mode(m) => {
                if let Some(f) = &self.flow {
                    f.pick_mode(&mut ask, &m);
                    // (at once; config.json's change confirms it)
                    if let Some(w) = &self.win {
                        w.ui.set_mode(m.into());
                    }
                }
                return;
            }
            Msg::Wall(i) => {
                if let (Some(f), Some(wall)) = (&self.flow, usize::try_from(i).ok().and_then(|i| self.walls.get(i))) {
                    f.pick_wall(&mut ask, &self.walls, &wall.id.clone());
                }
                return;
            }
            Msg::Own => {
                if let Some(f) = &self.flow {
                    f.pick_wall(&mut ask, &self.walls, "own");
                }
                return;
            }
            Msg::Closed => {
                self.closed(st);
                return;
            }
            Msg::Clock => {
                if let Some(w) = &self.win {
                    w.ui.set_clock(hm_short(&st.shell.cfg).into());
                }
                return;
            }
            Msg::Picture(gen, which, px) => {
                self.picture(gen, which, px);
                return;
            }
            Msg::Seed(gen, s) => {
                if gen == self.gen {
                    if let Some(w) = &self.win {
                        let style = st.shell.cfg["appearance"]["materialStyle"].as_str().unwrap_or("tonalSpot");
                        w.ui.set_mini(mini_colors(&s, st.shell.look.dark, style));
                    }
                }
                return;
            }
        }
        self.sync(st);
    }

    /// Light or dark, the wallpaper: chosen through the shell, shown here
    /// once config.json says so
    fn config_changed(&mut self, st: &mut State, look: bool) {
        if !self.win.as_ref().is_some_and(|w| w.shown) {
            return;
        }
        if look {
            let w = self.win.as_ref().unwrap();
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
        }
        if let Some(f) = self.flow.as_mut() {
            f.current_look = st.shell.cfg["appearance"]["look"].as_str().unwrap_or("glass").to_string();
        }
        let showing = wallpaper::showing_now(&st.shell.cfg, st.shell.look.material);
        if look || showing != self.showing {
            let dark = st.shell.look.dark;
            if let Some(m) = self.walls.iter_mut().find(|w| w.id == "material") {
                m.preview = if dark { m.dark.clone() } else { m.light.clone() };
            }
            let size = self.win.as_ref().map(|w| (w.ui.get_screen_w(), w.ui.get_screen_h())).unwrap_or((1920.0, 1080.0));
            let scale = self.win.as_ref().map_or(1.0, |w| w.surface.window().scale_factor().max(1.0));
            self.pictures(st, size, scale);
        }
        self.sync(st);
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let w = self.win.take().unwrap();
            w.surface.hide(); // (unmapped now: dropping the window alone leaves it on screen)
            st.drop_surface(&w.surface);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            let name = s.output.clone();
            self.output_removed(st, &name);
        }
    }

    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let Some(w) = self.win.as_mut() else { return };
        crate::picture::second_frame(&w.surface, &mut w.settled);
        let focused = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, focused);
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }
}
