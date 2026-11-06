//! The Downloads stack (rust.surfaces: dock; part `downloadsstack`), compare
//! shell/modules/dock/DownloadsStack.qml (Glass), MaterialDownloadsStack.qml
//! (Material) and DownloadsFiles.qml (the files and what a click does). The
//! dock opens it (a click on the Downloads item, or "Show Recent Files" in its
//! menu) with a message naming the screen and the x it points at:
//!   {"open":true,"screen":"DP-1","x":812.0,"barH":64.0,"gap":16.0}
//! A grid of four of the newest files in ~/Downloads (up to 12, newest
//! first, no folders or hidden files) above the dock: a click opens a file
//! (xdg-open), a right-click shares it, the eye is Quick Look, "Open in
//! Files" opens the folder. Escape, a click outside, or opening any panel
//! closes it. Glass is on the plugin's glass, Material a tonal popover with a
//! title; both spring up out of the dock.
//!
//! Differences from the QML: a file's name is shortened in the middle before
//! it is drawn (Slint only elides at the end); the files are listed once, as
//! it opens (the QML's list model stays live); Material draws Phosphor icons.

use super::*;
use tessera_core::services::bridge::ShellState;
use tessera_core::theme;

const STACK_NAME: &str = "downloadsstack";
const STACK_NS: &str = "tessera:stack";

/// Its own messages (from the cards' clicks)
enum S {
    Open(i32, bool),
    Quick(i32),
    Folder,
    Dismissed,
}

enum StackUi {
    Glass(ui::StackGlass),
    Material(ui::StackMaterial),
}

struct StackWin {
    output: String,
    surface: Rc<Surface>,
    ui: StackUi,
    files: Vec<String>,
    /// the panel open when it came up: another one opening closes it
    open_was: String,
    glass_sent: String,
    settled: bool,
}

#[derive(Default)]
pub struct DownloadsStack {
    win: Option<StackWin>,
}

/// The newest files of a folder (no folders, no hidden files), newest first, at most `max`
pub fn newest(dir: &std::path::Path, max: usize) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<(std::time::SystemTime, String)> = rd
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|e| {
            let md = e.metadata().ok()?;
            // (a link to a file counts as a file)
            let is_file = md.is_file() || (md.file_type().is_symlink() && e.path().is_file());
            is_file.then(|| (md.modified().unwrap_or(std::time::UNIX_EPOCH), e.path().to_string_lossy().into_owned()))
        })
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    v.into_iter().take(max).map(|(_, p)| p).collect()
}

/// A name shortened in the middle to `max` characters ("annual-rep…2024.pdf")
pub fn middle(name: &str, max: usize) -> String {
    let c: Vec<char> = name.chars().collect();
    if c.len() <= max || max < 4 {
        return name.to_string();
    }
    let keep = max - 1;
    let (head, tail) = (keep.div_ceil(2), keep / 2);
    format!("{}…{}", c[..head].iter().collect::<String>(), c[c.len() - tail..].iter().collect::<String>())
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

impl DownloadsStack {
    fn open(&mut self, st: &mut State, output: &str, x: f32, bar_h: f32, gap: f32) {
        self.close(st, false);
        let Some(wl) = st.output_named(output) else { return };
        let material = st.shell.look.material;
        let surface = overlay_surface(st, &wl, output);
        let files = newest(&downloads_dir(), 12);
        let model: Vec<ui::StackFile> = files
            .iter()
            .map(|p| {
                let name: slint::SharedString = middle(&file_name(p), if material { 14 } else { 13 }).into();
                let icon = tessera_core::icon_theme::find(tessera_core::rank::icon_for_file(p), None)
                    .or_else(|| tessera_core::icon_theme::find("text-x-generic", None))
                    .and_then(|path| slint::Image::load_from_path(&path).ok());
                ui::StackFile { name, has_icon: icon.is_some(), icon: icon.unwrap_or_default() }
            })
            .collect();
        let to = st.shell.sender(STACK_NAME);
        let open_was = st.shell.get::<ShellState>().map(|s| s.str("open").to_string()).unwrap_or_default();
        let ui = if material {
            let u = ui::StackMaterial::new().expect("StackMaterial");
            st.shell.apply_theme(&u.global::<ui::Theme>());
            u.set_files(ModelRc::new(VecModel::from(model)));
            u.set_stack_x(x);
            u.set_bar_height(bar_h);
            u.set_bottom_gap(gap);
            u.set_cookie(tessera_core::surfaces::shapes::outline("cookie9").into());
            let (a, b, c, d) = (to.clone(), to.clone(), to.clone(), to);
            u.on_open(move |i, right| a(Box::new(S::Open(i, right))));
            u.on_quick_look(move |i| b(Box::new(S::Quick(i))));
            u.on_folder(move || c(Box::new(S::Folder)));
            u.on_dismissed(move || d(Box::new(S::Dismissed)));
            u.show().expect("show stack");
            u.set_shown(true);
            u.invoke_focus_keys();
            StackUi::Material(u)
        } else {
            let u = ui::StackGlass::new().expect("StackGlass");
            st.shell.apply_theme(&u.global::<ui::Theme>());
            u.set_files(ModelRc::new(VecModel::from(model)));
            u.set_stack_x(x);
            u.set_bar_height(bar_h);
            u.set_bottom_gap(gap);
            u.set_on_glass(st.shell.plugin);
            let (a, b, c, d) = (to.clone(), to.clone(), to.clone(), to);
            u.on_open(move |i, right| a(Box::new(S::Open(i, right))));
            u.on_quick_look(move |i| b(Box::new(S::Quick(i))));
            u.on_folder(move || c(Box::new(S::Folder)));
            u.on_dismissed(move || d(Box::new(S::Dismissed)));
            u.show().expect("show stack");
            u.set_shown(true);
            u.invoke_focus_keys();
            StackUi::Glass(u)
        };
        self.win = Some(StackWin { output: output.to_string(), surface, ui, files, open_was, glass_sent: String::new(), settled: false });
    }

    /// Close it (instantly, as the QML's Loader), and tell the dock
    fn close(&mut self, st: &mut State, _tell_dock: bool) {
        let Some(w) = self.win.take() else { return };
        st.shell.glass.set(&w.output, STACK_NS, &[]);
        w.surface.hide(); // (unmapped now: dropping the window alone leaves it on screen)
        st.drop_surface(&w.surface);
        // (the dock may hide again)
        st.shell.sender("dock")(Box::new(M::StackClosed));
    }

    fn said(&mut self, st: &mut State, v: &serde_json::Value) {
        match v["open"].as_bool() {
            Some(true) => {
                let screen = v["screen"].as_str().unwrap_or("").to_string();
                let Some(out) = Some(screen).filter(|s| !s.is_empty()).or_else(|| st.outputs_now().into_iter().next().map(|o| o.1)) else { return };
                self.open(st, &out, v["x"].as_f64().unwrap_or(0.0) as f32, v["barH"].as_f64().unwrap_or(64.0) as f32, v["gap"].as_f64().unwrap_or(16.0) as f32);
            }
            Some(false) => self.close(st, true),
            None => {}
        }
    }
}

/// A layer surface over a whole output that takes the keyboard (Escape)
fn overlay_surface(st: &mut State, wl: &WlOutput, name: &str) -> Rc<Surface> {
    st.prepare_surface(Spec {
        layer: Layer::Overlay,
        namespace: STACK_NS,
        output: Some((wl, name)),
        anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
        size: (0, 0),
        exclusive: -1,
        keyboard: KeyboardInteractivity::Exclusive,
        margin_left: 0,
    })
}

impl Part for DownloadsStack {
    fn name(&self) -> &'static str {
        STACK_NAME
    }

    fn command(&mut self, st: &mut State, cmd: &tessera_core::shell::Command) {
        match cmd.cmd.as_str() {
            "open" => {
                let mut v = cmd.args.clone();
                v["open"] = serde_json::json!(true);
                if let Some(s) = &cmd.screen {
                    v["screen"] = serde_json::json!(s);
                }
                self.said(st, &v);
            }
            "close" => self.close(st, true),
            _ => {}
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<serde_json::Value>() {
            Ok(v) => return self.said(st, &v),
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<S>() else { return };
        let path = |w: &Option<StackWin>, i: i32| w.as_ref().and_then(|w| w.files.get(i.max(0) as usize).cloned());
        match *m {
            S::Open(i, right) => {
                let Some(p) = path(&self.win, i) else { return };
                self.close(st, true);
                // right-click: share it with a nearby device
                if right {
                    crate::shell::ask("panels.share", serde_json::json!([[p]]));
                } else {
                    apps::run(vec!["xdg-open".into(), p], "open", None);
                }
            }
            S::Quick(i) => {
                if let Some(p) = path(&self.win, i) {
                    crate::shell::ask("panels.quickLook", serde_json::json!([p]));
                }
            }
            S::Folder => {
                self.close(st, true);
                let cfg = st.shell.cfg.clone();
                open_folder(&cfg, &downloads_dir());
            }
            S::Dismissed => self.close(st, true),
        }
    }

    /// Opening any panel closes the stack (DockLogic.qml)
    fn data(&mut self, st: &mut State, what: TypeId) {
        if what != TypeId::of::<ShellState>() {
            return;
        }
        let now = st.shell.get::<ShellState>().map(|s| s.str("open").to_string()).unwrap_or_default();
        if self.win.as_ref().is_some_and(|w| w.open_was != now) {
            self.close(st, true);
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        // (each look has its own stack: it closes)
        if look && self.win.is_some() {
            self.close(st, true);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            self.close(st, true);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.close(st, true);
        }
    }

    /// Glass: the card's shape on the plugin's glass, scaled about the dock
    fn after_frame(&mut self, st: &mut State) {
        let Some(w) = self.win.as_mut() else { return };
        crate::picture::second_frame(&w.surface, &mut w.settled);
        let StackUi::Glass(u) = &w.ui else { return };
        if !st.shell.plugin || !w.surface.mapped.get() {
            return;
        }
        let (cx, cy, cw, ch) = (u.get_card_x(), u.get_card_y(), u.get_card_w(), u.get_card_h());
        let (p, s) = (u.get_fade(), u.get_scale());
        // (the card scales about the bottom middle of the window it sits in: 24 px under the card)
        let (ox, oy) = (cx + cw / 2.0, cy + ch + 24.0);
        let (x, y) = (ox + (cx - ox) * s, oy + (cy - oy) * s);
        let shape = theme::glass_shape(&st.shell.look, x, y, cw * s, ch * s, 18.0 * s, p, 1.0);
        let key = format!("{shape:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, STACK_NS, if p > 0.002 { std::slice::from_ref(&shape) } else { &[] });
            w.glass_sent = key;
        }
    }

    fn quit(&mut self, _: &mut State) {
        if let Some(w) = &self.win {
            tessera_core::services::hypr::Glass::clear_now(&w.output, STACK_NS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_names_are_cut_in_the_middle() {
        assert_eq!(middle("short.pdf", 14), "short.pdf");
        assert_eq!(middle("annual-report-final-2024.pdf", 14), "annual-…24.pdf");
        assert_eq!(middle("annual-report-final-2024.pdf", 14).chars().count(), 14);
        assert_eq!(middle("ab", 3), "ab");
    }

    #[test]
    fn the_newest_files_without_folders_or_hidden_ones() {
        let dir = std::env::temp_dir().join(format!("tessera-stack-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("a-folder")).unwrap();
        for (i, n) in ["old.txt", "mid.txt", ".hidden", "new.txt"].iter().enumerate() {
            let p = dir.join(n);
            std::fs::write(&p, n).unwrap();
            let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000 + i as u64 * 100);
            std::fs::File::options().write(true).open(&p).unwrap().set_modified(t).unwrap();
        }
        let got: Vec<String> = newest(&dir, 2).iter().map(|p| file_name(p)).collect();
        assert_eq!(got, vec!["new.txt", "mid.txt"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_name_is_the_last_part_of_a_path() {
        assert_eq!(file_name("/home/a/Downloads/x.zip"), "x.zip");
    }
}
