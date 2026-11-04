//! The clipboard history (rust.surfaces: clipboard), Super+V: search, pins,
//! picture previews. Enter pastes into the window you were in, Shift+Enter
//! only copies, Alt+Enter pastes as plain text, Ctrl+P pins, Delete removes,
//! Tab switches All / Text / Images. Entries are grouped by when they were
//! copied; Images is a two-column grid. Compare
//! shell/modules/clipboard/ClipboardPanel.qml, shell/services/Clipboard.qml
//! and shell/glass/Dialog.qml.
//!
//! It opens as the QML one did, through Panels ("clipboard"): RustUi.qml
//! forwards the open and close (src/shell.rs). The history comes from the
//! clipboard service (src/services/clipboard.rs); pins are the shell's
//! (state.json), changed through it (`shell::ask("clipboard.pin", …)`).
//!
//! Differences from the QML: Material draws its own tonal surface (no glass),
//! as the menu bar does; the dialog doesn't stretch when dragged (Dialog's
//! ElasticGrab); Material's empty-state shape is an outline
//! (src/surfaces/shapes.rs).

use crate::services::clipboard::{self as clip, Filter, History, Pin, Previews, Row};
use crate::services::hypr::{self, Glass};
use crate::shell::{Command, Part};
use crate::theme;
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::{Any, TypeId};
use std::rc::Rc;
use std::time::Duration;

const NAME: &str = "clipboard";
pub const NS: &str = "tessera:clipboard";
const WIDTH: f32 = 560.0;
const KINDS: [Filter; 3] = [Filter::All, Filter::Text, Filter::Images];

/// Apps that paste with Ctrl+Shift+V
const TERMINALS: &[&str] = &["foot", "kitty", "alacritty", "org.wezfurlong.wezterm", "com.mitchellh.ghostty", "konsole", "org.gnome.ptyxis", "xterm"];

enum M {
    Key(String, bool, bool, bool),
    Edited(String),
    Hovered(i32),
    Activate(i32, bool, bool),
    Pin(i32),
    Remove(i32),
    SetKind(i32),
    ClearClicked,
    ClearLeft,
    Scrolled(f32),
    Dismissed,
    Closed,
}

// ── Layout ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Heading,
    Row,
    Tile,
}

/// One thing in the scrolled area, placed
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub kind: ItemKind,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// its row (for a heading, the first row under it)
    pub row: usize,
    pub first: bool,
    pub last: bool,
}

/// The sections: (title, first row, how many)
pub fn groups(rows: &[Row]) -> Vec<(String, usize, usize)> {
    let mut out: Vec<(String, usize, usize)> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        match out.last_mut() {
            Some(g) if g.0 == r.section => g.2 += 1,
            _ => out.push((r.section.clone(), i, 1)),
        }
    }
    out
}

/// Grid tiles: two a row
pub fn cell(width: f32) -> (f32, f32) {
    let w = ((width - 8.0) / 2.0).floor();
    (w, (w * 0.72).round())
}

/// Where everything goes: the list (a heading where the section changes,
/// rows 44 px, pictures 168, 2 apart), or the grid (a heading, then two
/// tiles a row, 8 apart). Headings: Material 16 above / 8 below a 16 px
/// line, Glass 12 / 4 round 14 (none above the first)
pub fn layout(rows: &[Row], grid: bool, width: f32, material: bool) -> (Vec<Placed>, f32) {
    let mut out = Vec::new();
    let mut y = 0.0;
    let heading = |first: bool| (if first { 0.0 } else if material { 16.0 } else { 12.0 }) + if material { 16.0 + 8.0 } else { 14.0 + 4.0 };
    let gs = groups(rows);
    if grid {
        let (cw, ch) = cell(width);
        for (gi, (_, start, n)) in gs.iter().enumerate() {
            let h = heading(gi == 0);
            out.push(Placed { kind: ItemKind::Heading, x: 0.0, y, w: width, h, row: *start, first: true, last: false });
            y += h;
            for k in 0..*n {
                let (c, r) = (k % 2, k / 2);
                out.push(Placed { kind: ItemKind::Tile, x: c as f32 * (cw + 8.0), y: y + r as f32 * (ch + 8.0), w: cw, h: ch, row: start + k, first: k < 2, last: k + 2 >= *n });
            }
            let lines = n.div_ceil(2);
            y += lines as f32 * ch + (lines.max(1) - 1) as f32 * 8.0;
        }
        return (out, y);
    }
    for (i, r) in rows.iter().enumerate() {
        let first = i == 0 || rows[i - 1].section != r.section;
        let last = i + 1 == rows.len() || rows[i + 1].section != r.section;
        if first {
            let h = heading(i == 0);
            out.push(Placed { kind: ItemKind::Heading, x: 0.0, y, w: width, h, row: i, first, last: false });
            y += h;
        }
        let h = if r.image.is_some() { 168.0 } else { 44.0 };
        out.push(Placed { kind: ItemKind::Row, x: 0.0, y, w: width, h, row: i, first, last });
        y += h + 2.0;
    }
    (out, (y - 2.0).max(0.0))
}

/// Arrow keys: up and down the list; in the grid also left and right, and
/// up and down a row (into the section above or below), as the QML's move()
pub fn step(rows: &[Row], sel: usize, dx: i32, dy: i32, grid: bool) -> usize {
    if rows.is_empty() {
        return 0;
    }
    let last = rows.len() as i64 - 1;
    let sel = sel.min(rows.len() - 1);
    if !grid {
        return (sel as i64 + dy as i64).clamp(0, last) as usize;
    }
    if dx != 0 {
        return (sel as i64 + dx as i64).clamp(0, last) as usize;
    }
    let gs = groups(rows);
    let Some(gi) = gs.iter().position(|g| sel >= g.1 && sel < g.1 + g.2) else { return sel };
    let (_, start, n) = gs[gi];
    let col = (sel - start) % 2;
    if dy > 0 {
        if sel + 2 < start + n {
            sel + 2
        } else if (sel - start) / 2 < (n - 1) / 2 {
            start + n - 1 // the shorter last row
        } else if let Some(next) = gs.get(gi + 1) {
            next.1 + col.min(next.2 - 1)
        } else {
            sel
        }
    } else if sel >= start + 2 {
        sel - 2
    } else if gi > 0 {
        let p = &gs[gi - 1];
        let last_row = p.1 + (p.2 - 1) / 2 * 2;
        (last_row + col).min(p.1 + p.2 - 1)
    } else {
        sel
    }
}

/// The scroll that shows the chosen row: just enough (the list), or with
/// 24 px above it (the grid); `bottom`: room the toolbar takes
pub fn reveal(items: &[Placed], sel: usize, scroll: f32, height: f32, bottom: f32, grid: bool, total: f32) -> f32 {
    let Some(it) = items.iter().find(|p| p.kind != ItemKind::Heading && p.row == sel) else { return scroll };
    // (a section's first row brings its title along)
    let top = items.iter().find(|p| p.kind == ItemKind::Heading && p.row == sel && !grid).map_or(it.y, |h| h.y);
    let s = if top < scroll {
        if grid { (it.y - 24.0).max(0.0) } else { top }
    } else if it.y + it.h > scroll + height - bottom {
        it.y + it.h - height + bottom + if grid { 8.0 } else { 0.0 }
    } else {
        scroll
    };
    s.clamp(0.0, (total + bottom - height).max(0.0))
}

/// The keys it takes from the field (the rest edit the search): Left and
/// Right only in the grid (in the list they move the cursor)
pub fn handles(text: &str, ctrl: bool, grid: bool) -> bool {
    let k = |key: Key| text == SharedString::from(key).as_str();
    k(Key::Escape) || k(Key::Return) || k(Key::UpArrow) || k(Key::DownArrow) || k(Key::Tab) || k(Key::Delete) || (grid && (k(Key::LeftArrow) || k(Key::RightArrow))) || (ctrl && text.eq_ignore_ascii_case("p"))
}

// ── The part ─────────────────────────────────────────────────────────────

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::ClipboardPanel,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
}

#[derive(Default)]
pub struct Clipboard {
    win: Option<Win>,
    open: bool,
    /// the history as last read, with what was changed here since
    history: History,
    previews: Previews,
    kind: usize,
    query: String,
    sel: usize,
    confirm: bool,
    rows: Vec<Row>,
    items: Vec<Placed>,
    total: f32,
    scroll: f32,
    later: slint::Timer,
}

impl Clipboard {
    fn grid(&self) -> bool {
        KINDS[self.kind] == Filter::Images
    }

    fn window(&mut self, st: &mut State, output: &str) -> bool {
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            Glass::clear_now(&w.output, NS);
            st.drop_surface(&w.surface);
        }
        if self.win.is_some() {
            return true;
        }
        let Some(wl_output) = st.output_named(output) else { return false };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((&wl_output, output)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::ClipboardPanel::new().expect("ClipboardPanel");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = st.shell.sender(NAME);
        {
            let (to, weak) = (to.clone(), ui.as_weak());
            ui.on_key(move |text, shift, alt, ctrl| {
                let grid = weak.upgrade().is_some_and(|u| u.get_grid());
                let ok = handles(&text, ctrl, grid);
                if ok {
                    to(Box::new(M::Key(text.to_string(), shift, alt, ctrl)));
                }
                ok
            });
        }
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                ui.$set(move |$($a),*| to(Box::new($m)));
            }};
        }
        on!(on_edited, |t| M::Edited(t.to_string()));
        on!(on_hovered, |i| M::Hovered(i));
        on!(on_activate, |i, p, c| M::Activate(i, p, c));
        on!(on_pin, |i| M::Pin(i));
        on!(on_remove, |i| M::Remove(i));
        on!(on_set_kind, |i| M::SetKind(i));
        on!(on_clear_clicked, | | M::ClearClicked);
        on!(on_clear_left, | | M::ClearLeft);
        on!(on_scrolled, |y| M::Scrolled(y));
        on!(on_dismissed, | | M::Dismissed);
        on!(on_closed, | | M::Closed);
        ui.set_empty_outline(crate::surfaces::shapes::outline("cookie9").into());
        ui.show().expect("show clipboard");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: output.into(), surface, ui, a11y, glass_sent: String::new() });
        true
    }

    fn open(&mut self, st: &mut State, screen: Option<String>) {
        let output = screen.or_else(|| self.win.as_ref().map(|w| w.output.clone())).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(output) = output else { return };
        if !self.window(st, &output) {
            return;
        }
        st.shell.want("clipboard");
        clip::refresh();
        // (the shell's Clipboard stamps new entries with the time they came)
        crate::shell::ask("clipboard.refresh", serde_json::json!([]));
        if let Some(h) = st.shell.get::<History>() {
            self.history = h.clone();
        }
        if let Some(p) = st.shell.get::<Previews>() {
            self.previews = p.clone();
        }
        self.open = true;
        self.kind = 0;
        self.query.clear();
        self.sel = 0;
        self.confirm = false;
        self.scroll = 0.0;
        let w = self.win.as_ref().unwrap();
        w.ui.invoke_open();
        w.ui.set_list_y(0.0);
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        w.surface.show(0, 0);
        self.rebuild(st);
        self.win.as_ref().unwrap().ui.set_shown(true);
    }

    fn close(&mut self, tell: bool) {
        if !self.open {
            return;
        }
        self.open = false;
        if let Some(w) = &self.win {
            w.ui.set_shown(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            w.surface.layer.commit();
        }
        if tell {
            crate::shell::closed(NAME);
        }
    }

    fn closed(&mut self, st: &mut State) {
        if self.open {
            return;
        }
        if let Some(w) = &mut self.win {
            w.surface.hide();
            st.shell.glass.set(&w.output, NS, &[]);
            w.glass_sent.clear();
        }
    }

    /// The rows for the search and filter, laid out, into the window
    fn rebuild(&mut self, st: &State) {
        self.rows = clip::filtered(&self.history, &self.query, KINDS[self.kind], crate::util::now_ms());
        self.sel = self.sel.min(self.rows.len().saturating_sub(1));
        let grid = self.grid();
        let (items, total) = layout(&self.rows, grid, WIDTH, st.shell.look.material);
        self.items = items;
        self.total = total;
        self.push_items();
        self.push(st);
    }

    fn push_items(&self) {
        let Some(w) = &self.win else { return };
        let items: Vec<ui::CItem> = self
            .items
            .iter()
            .map(|p| {
                let r = &self.rows[p.row];
                let (image, has_image, aspect) = match r.image.as_ref().and_then(|i| self.previews.0.get(i)) {
                    Some(px) => (slint::Image::from_rgba8(px.clone()), true, px.width() as f32 / px.height().max(1) as f32),
                    None => (slint::Image::default(), false, 0.0),
                };
                let text = clip::norm(&r.text);
                let is_color = r.image.is_none() && clip::is_color(&r.text);
                let swatch = if is_color { u32::from_str_radix(&text.trim()[1..], 16).map(|v| slint::Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8)).unwrap_or_default() } else { Default::default() };
                ui::CItem {
                    kind: match p.kind {
                        ItemKind::Heading => 0,
                        ItemKind::Row => 1,
                        ItemKind::Tile => 2,
                    },
                    x: p.x,
                    y: p.y,
                    w: p.w,
                    h: p.h,
                    index: p.row as i32,
                    title: r.section.as_str().into(),
                    icon: if clip::is_link(&r.text) { "\u{e2e2}" } else { "\u{e484}" }.into(),
                    text: text.into(),
                    is_color,
                    swatch,
                    image,
                    has_image,
                    picture: r.image.is_some(),
                    aspect,
                    pinned: r.pinned(),
                    first: p.first,
                    last: p.last,
                }
            })
            .collect();
        w.ui.set_items(ModelRc::new(VecModel::from(items)));
        w.ui.set_items_height(self.total);
    }

    fn push(&self, _st: &State) {
        let Some(w) = &self.win else { return };
        let ui = &w.ui;
        ui.set_sel(self.sel as i32);
        ui.set_count(self.rows.len() as i32);
        ui.set_kind(self.kind as i32);
        ui.set_grid(self.grid());
        ui.set_history_count(self.history.entries.len() as i32);
        ui.set_confirm_clear(self.confirm);
        ui.set_query(self.query.trim().into());
        let r = self.rows.get(self.sel);
        ui.set_sel_image(r.is_some_and(|r| r.image.is_some()));
        ui.set_sel_pinned(r.is_some_and(|r| r.pinned()));
        ui.set_empty_outline(crate::surfaces::shapes::outline(if self.query.trim().is_empty() { "cookie9" } else { "clover4" }).into());
    }

    fn reveal(&mut self) {
        let Some(w) = &self.win else { return };
        let h = w.ui.get_list_height();
        let bottom = if w.ui.get_count() > 0 && material(w) { 64.0 + 12.0 } else { 0.0 };
        let s = reveal(&self.items, self.sel, self.scroll, h, bottom, self.grid(), self.total);
        if s != self.scroll {
            self.scroll = s;
            w.ui.set_list_y(-s);
        }
    }

    fn select(&mut self, st: &State, i: usize) {
        self.sel = i.min(self.rows.len().saturating_sub(1));
        self.push(st);
        self.reveal();
    }

    /// Enter pastes into the app; Shift only copies; Alt pastes plain text
    fn activate(&mut self, i: usize, plain: bool, copy_only: bool) {
        let Some(r) = self.rows.get(i).cloned() else { return };
        self.close(true);
        copy(&r, plain);
        if !copy_only {
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_millis(180)); // (the panel lets go of the keyboard first)
                let class = hypr::request("j/activewindow").ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()).and_then(|v| v["class"].as_str().map(str::to_lowercase)).unwrap_or_default();
                let mods = if TERMINALS.contains(&class.as_str()) { "CTRL SHIFT" } else { "CTRL" };
                let _ = hypr::request(&format!("dispatch hl.dsp.send_shortcut({{ mods = \"{mods}\", key = \"V\" }})"));
            });
        }
    }

    fn toggle_pin(&mut self, st: &State, i: usize) {
        let Some(r) = self.rows.get(i).cloned() else { return };
        match (&r.pin, &r.entry) {
            (Some(p), _) => {
                crate::shell::ask("clipboard.unpin", serde_json::json!([pin_json(p)]));
                self.history.pins.retain(|x| x != p);
            }
            (None, Some(e)) => {
                let image = e.image.as_ref().map(|p| p.to_string_lossy().into_owned());
                crate::shell::ask("clipboard.pin", serde_json::json!([{"id": e.id, "text": e.text, "image": image.clone().unwrap_or_default()}]));
                // (until the shell has it: the shell keeps the whole text and a copy of the picture)
                self.history.pins.insert(0, Pin { text: image.is_none().then(|| e.text.clone()), image, source: Some(e.id.clone()) });
            }
            _ => return,
        }
        self.reread_soon(st);
        self.rebuild(st);
    }

    fn remove(&mut self, st: &State, i: usize) {
        let Some(r) = self.rows.get(i).cloned() else { return };
        if let Some(p) = &r.pin {
            crate::shell::ask("clipboard.unpin", serde_json::json!([pin_json(p)]));
            self.history.pins.retain(|x| x != p);
        } else if let Some(e) = &r.entry {
            crate::util::pipe_to(&["cliphist", "-db-path", &clip::db_path(), "delete"], format!("{}\t{}\n", e.id, e.text).into_bytes());
            if let Some(img) = &e.image {
                let _ = std::fs::remove_file(img);
            }
            self.history.entries.retain(|x| x.id != e.id);
        }
        self.reread_soon(st);
        self.rebuild(st);
    }

    fn clear(&mut self, st: &State) {
        let _ = std::process::Command::new("cliphist").args(["-db-path", &clip::db_path(), "wipe"]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map(|mut c| std::thread::spawn(move || c.wait()));
        if let Ok(rd) = std::fs::read_dir(clip::preview_dir()) {
            for e in rd.flatten() {
                let _ = std::fs::remove_file(e.path());
            }
        }
        self.history.entries.clear();
        self.confirm = false;
        self.rebuild(st);
    }

    /// The shell writes pins a moment later: read the history again then
    fn reread_soon(&mut self, _st: &State) {
        self.later.start(slint::TimerMode::SingleShot, Duration::from_millis(1500), clip::refresh);
    }

    fn key(&mut self, st: &State, text: &str, shift: bool, alt: bool, ctrl: bool) {
        let is = |key: Key| text == SharedString::from(key).as_str();
        if is(Key::Escape) {
            self.close(true);
        } else if is(Key::Tab) {
            self.set_kind(st, (self.kind + 1) % KINDS.len());
        } else if is(Key::UpArrow) || is(Key::DownArrow) {
            let s = step(&self.rows, self.sel, 0, if is(Key::UpArrow) { -1 } else { 1 }, self.grid());
            self.select(st, s);
        } else if is(Key::LeftArrow) || is(Key::RightArrow) {
            let s = step(&self.rows, self.sel, if is(Key::LeftArrow) { -1 } else { 1 }, 0, self.grid());
            self.select(st, s);
        } else if is(Key::Return) {
            self.activate(self.sel, alt, shift);
        } else if is(Key::Delete) {
            self.remove(st, self.sel);
        } else if ctrl && text.eq_ignore_ascii_case("p") {
            self.toggle_pin(st, self.sel);
        }
    }

    /// All / Text / Images (the choice stays where it was, within the new
    /// rows, as the QML's)
    fn set_kind(&mut self, st: &State, k: usize) {
        self.kind = k.min(KINDS.len() - 1);
        self.scroll = 0.0;
        if let Some(w) = &self.win {
            w.ui.set_list_y(0.0);
        }
        self.rebuild(st);
        self.reveal();
    }
}

fn material(w: &Win) -> bool {
    w.ui.global::<ui::Theme>().get_material()
}

/// A pin as the shell keeps it (Clipboard.unpin finds it by its text or picture)
fn pin_json(p: &Pin) -> serde_json::Value {
    let mut v = serde_json::Map::new();
    if let Some(t) = &p.text {
        v.insert("text".into(), t.clone().into());
    }
    if let Some(i) = &p.image {
        v.insert("image".into(), i.clone().into());
    }
    if let Some(s) = &p.source {
        v.insert("source".into(), s.clone().into());
    }
    v.into()
}

/// Onto the clipboard: a history entry decoded from cliphist (plain: as text
/// only), a pinned picture's file, or a pinned text (through wl-copy's input,
/// never its command line)
pub fn copy(r: &Row, plain: bool) {
    use std::process::{Command, Stdio};
    if let Some(e) = &r.entry {
        let (id, db, text) = (e.id.clone(), clip::db_path(), e.image.is_none());
        std::thread::spawn(move || {
            let Ok(mut dec) = Command::new("cliphist").args(["-db-path", &db, "decode", &id]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else { return };
            let mut args = vec![];
            if plain && text {
                args = vec!["--type", "text/plain"];
            }
            if let Some(out) = dec.stdout.take() {
                let _ = Command::new("wl-copy").args(&args).stdin(Stdio::from(out)).stdout(Stdio::null()).stderr(Stdio::null()).status();
            }
            let _ = dec.wait();
        });
    } else if let Some(img) = &r.image {
        if let Ok(b) = std::fs::read(img) {
            crate::util::pipe_to(&["wl-copy"], b);
        }
    } else {
        crate::util::pipe_to(&["wl-copy", "--type", "text/plain"], r.text.clone().into_bytes());
    }
}

impl Part for Clipboard {
    fn name(&self) -> &'static str {
        NAME
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.open(st, cmd.screen.clone()),
            "close" => self.close(false),
            "toggle" => {
                if self.open {
                    self.close(true)
                } else {
                    self.open(st, cmd.screen.clone())
                }
            }
            _ => {}
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            if let Some(w) = &mut self.win {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
                w.glass_sent.clear();
            }
            if self.open {
                self.rebuild(st);
            }
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = &mut self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.glass_sent.clear();
        }
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<History>() {
            if let Some(h) = st.shell.get::<History>() {
                self.history = h.clone();
            }
            if self.open {
                self.rebuild(st);
            }
        } else if what == TypeId::of::<Previews>() {
            if let Some(p) = st.shell.get::<Previews>() {
                self.previews = p.clone();
            }
            if self.open {
                self.push_items();
            }
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Key(t, shift, alt, ctrl) => self.key(st, &t, shift, alt, ctrl),
            M::Edited(t) => {
                self.query = t;
                self.sel = 0;
                self.scroll = 0.0;
                if let Some(w) = &self.win {
                    w.ui.set_list_y(0.0);
                }
                self.rebuild(st);
            }
            M::Hovered(i) => {
                if i >= 0 && i as usize != self.sel {
                    self.sel = i as usize;
                    self.push(st);
                }
            }
            M::Activate(i, p, c) => self.activate(i.max(0) as usize, p, c),
            M::Pin(i) => self.toggle_pin(st, i.max(0) as usize),
            M::Remove(i) => self.remove(st, i.max(0) as usize),
            M::SetKind(k) => self.set_kind(st, k.max(0) as usize),
            M::ClearClicked => {
                if self.confirm {
                    self.clear(st);
                } else {
                    self.confirm = true;
                    self.push(st);
                }
            }
            M::ClearLeft => {
                if self.confirm {
                    self.confirm = false;
                    self.push(st);
                }
            }
            M::Scrolled(y) => self.scroll = -y,
            M::Dismissed => self.close(true),
            M::Closed => self.closed(st),
        }
    }

    fn keyboard_left(&mut self, _st: &mut State, s: &Rc<Surface>) {
        if self.open && self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.close(true);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let was = self.open;
            self.close(was);
            let w = self.win.take().unwrap();
            st.drop_surface(&w.surface);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            let name = s.output.clone();
            self.output_removed(st, &name);
        }
    }

    /// The dialog's glass (Glass), following its spring; the accessibility tree
    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let Some(w) = &mut self.win else { return };
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
        if !w.surface.mapped.get() || !st.shell.plugin || st.shell.look.material {
            return;
        }
        let ui = &w.ui;
        let s = ui.get_body_scale();
        let (bw, bh) = (ui.get_body_width(), ui.get_body_height());
        let x = ui.get_body_x() + bw * (1.0 - s) / 2.0;
        let y = ui.get_body_y() + bh * (1.0 - s) / 2.0;
        let o = ui.get_body_opacity();
        let shape = theme::glass_shape(&st.shell.look, x, y, bw * s, bh * s, 28.0 * s, o, 1.0);
        let key = format!("{shape:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, if o > 0.002 { std::slice::from_ref(&shape) } else { &[] });
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    fn stats(&self) {
        if let Some(w) = &self.win {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report("clipboard"));
        }
    }

    fn quit(&mut self, _: &mut State) {
        if let Some(w) = &self.win {
            Glass::clear_now(&w.output, NS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn row(section: &str, image: bool) -> Row {
        Row { pin: None, entry: None, text: "x".into(), image: image.then(|| PathBuf::from("/p.png")), section: section.into() }
    }

    #[test]
    fn the_list_puts_a_heading_where_the_section_changes() {
        let rows = vec![row("Pinned", false), row("Just now", false), row("Just now", true), row("Earlier", false)];
        let (items, total) = layout(&rows, false, 560.0, false);
        let kinds: Vec<ItemKind> = items.iter().map(|p| p.kind).collect();
        use ItemKind::*;
        assert_eq!(kinds, vec![Heading, Row, Heading, Row, Row, Heading, Row]);
        // first heading: no room above; rows 44 (pictures 168), 2 apart
        assert_eq!(items[0].h, 18.0);
        assert_eq!(items[1].y, 18.0);
        assert_eq!(items[2].y, 18.0 + 46.0);
        assert_eq!(items[2].h, 30.0);
        assert_eq!(items[4].h, 168.0);
        assert!(items[3].first && !items[3].last && items[4].last);
        assert_eq!(total, items[6].y + 44.0);
        // Material's headings are taller
        assert_eq!(layout(&rows, false, 560.0, true).0[2].h, 40.0);
    }

    #[test]
    fn the_grid_goes_two_a_row() {
        let rows = vec![row("Pinned", true), row("Today", true), row("Today", true), row("Today", true)];
        let (cw, ch) = cell(560.0);
        assert_eq!((cw, ch), (276.0, 199.0));
        let (items, total) = layout(&rows, true, 560.0, false);
        let tiles: Vec<&Placed> = items.iter().filter(|p| p.kind == ItemKind::Tile).collect();
        assert_eq!(tiles.len(), 4);
        assert_eq!((tiles[1].x, tiles[2].x), (0.0, 284.0));
        assert_eq!(tiles[3].y, tiles[1].y + ch + 8.0);
        assert_eq!(total, tiles[3].y + ch);
    }

    #[test]
    fn arrows_in_the_list_and_the_grid() {
        let list = vec![row("A", false), row("A", false), row("B", false)];
        assert_eq!(step(&list, 0, 0, 1, false), 1);
        assert_eq!(step(&list, 2, 0, 1, false), 2);
        assert_eq!(step(&list, 0, 0, -1, false), 0);
        assert_eq!(step(&[], 3, 0, 1, false), 0);
        // grid: A has 3 (two rows), B has 2
        let g = vec![row("A", true), row("A", true), row("A", true), row("B", true), row("B", true)];
        assert_eq!(step(&g, 0, 1, 0, true), 1);
        assert_eq!(step(&g, 0, 0, 1, true), 2);
        assert_eq!(step(&g, 1, 0, 1, true), 2); // the shorter last row
        assert_eq!(step(&g, 2, 0, 1, true), 3); // into the next section, same column
        assert_eq!(step(&g, 4, 0, -1, true), 2); // up into A's last row (one tile: it)
        assert_eq!(step(&g, 3, 0, -1, true), 2);
        assert_eq!(step(&g, 2, 0, -1, true), 0);
        assert_eq!(step(&g, 4, 0, 1, true), 4);
    }

    #[test]
    fn reveal_keeps_the_choice_in_view() {
        let rows: Vec<Row> = (0..20).map(|_| row("A", false)).collect();
        let (items, total) = layout(&rows, false, 560.0, false);
        assert_eq!(reveal(&items, 0, 0.0, 396.0, 0.0, false, total), 0.0);
        let s = reveal(&items, 19, 0.0, 396.0, 0.0, false, total);
        assert_eq!(s, total - 396.0);
        // back up to the first: its section's title comes along
        assert_eq!(reveal(&items, 0, s, 396.0, 0.0, false, total), 0.0);
        // the toolbar's room at the foot
        assert_eq!(reveal(&items, 19, 0.0, 396.0, 76.0, false, total), total - 396.0 + 76.0);
    }

    #[test]
    fn keys_it_takes_from_the_field() {
        let k = |key: Key| SharedString::from(key).to_string();
        assert!(handles(&k(Key::Return), false, false) && handles(&k(Key::Delete), false, false) && handles(&k(Key::Tab), false, false));
        assert!(handles("p", true, false) && !handles("p", false, false));
        // left and right move the text cursor, except in the grid
        assert!(!handles(&k(Key::LeftArrow), false, false) && handles(&k(Key::LeftArrow), false, true));
        assert!(!handles("a", false, true));
    }

    #[test]
    fn pins_go_back_as_the_shell_keeps_them() {
        let p = Pin { text: None, image: Some("/s/1.png".into()), source: Some("11".into()) };
        assert_eq!(pin_json(&p), serde_json::json!({"image": "/s/1.png", "source": "11"}));
        let p = Pin { text: Some("hi".into()), ..Default::default() };
        assert_eq!(pin_json(&p), serde_json::json!({"text": "hi"}));
    }
}
