//! The quick note (rust.surfaces: notes): Super+Alt+N, a hot corner or
//! Spotlight's "note …". Type, then Ctrl+Enter (or Save) saves it as a
//! Markdown file in the Notes folder; Esc or a click outside closes it and
//! keeps the draft for next time; an empty note isn't saved. Material adds
//! the FAB menu: a checklist item, what's copied, today's date, the Notes
//! folder. Compare shell/modules/notes/QuickNote.qml and shell/glass/Dialog.qml.
//!
//! It opens as the QML one does, through Panels ("quickNote", RustUi.qml's
//! panelNames maps it to "notes"): open and close come as commands, and it
//! says `closed` when it closes itself. Saving is the shell's (services/
//! Notes.qml: the file name, the heading, save-note.py and the island's
//! "Note saved"): `shell::ask("notes", ["save", text])`, and the feed
//! (shell/modules/notes/NotesFeed.qml) answers `{"to":"notes","saving":path}`,
//! then `{"saved":path}` or `{"failed":path}`; it also sends `{"folder":…}`
//! (with ~). "Open the Notes folder" is `["openFolder"]`.
//!
//! Differences from the QML: Material draws its own tonal surface (no glass),
//! as the clipboard does; the dialog doesn't stretch when dragged
//! (ElasticGrab); the folder's path is cut at its end, not in the middle;
//! Slint doesn't say where the cursor is, so the menu's text goes in at the
//! cursor but a checklist item starts a new line unless the note is empty or
//! ends with one (QML: unless the character before the cursor is a newline);
//! the FAB's press swell is a plain spring scale, and the FAB and its pills
//! aren't reached with Tab (QML: Tab to it, Space or Enter opens it, Esc
//! closes the menu).

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

use tessera_core::services::hypr::Glass;
use tessera_core::shell::{Command, Part};
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use slint::{ComponentHandle, ModelRc, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::{Any, TypeId};
use std::rc::Rc;

const NAME: &str = "notes";
pub const NS: &str = "tessera:quickNote";

enum M {
    Edited(String),
    Save,
    Dismissed,
    Closed,
    Pick(i32),
    Paste(String),
}

/// The FAB menu (QuickNote.qml's items): (text, Phosphor name)
pub const MENU: [(&str, &str); 4] = [
    ("Checklist item", "list-checks"),
    ("Paste what you copied", "clipboard-text"),
    ("Today's date", "calendar-blank"),
    ("Open the Notes folder", "folder-open"),
];

/// A checklist item, on a line of its own
pub fn checklist(text: &str) -> &'static str {
    if text.is_empty() || text.ends_with('\n') {
        "- [ ] "
    } else {
        "\n- [ ] "
    }
}

/// Nothing to save (only blanks)
pub fn blank(text: &str) -> bool {
    text.trim().is_empty()
}

/// Today as the QML writes it (Qt's "dddd d MMMM yyyy")
pub fn today() -> String {
    crate::util::now("%A %-d %B %Y")
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::QuickNote,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
}

#[derive(Default)]
pub struct Notes {
    win: Option<Win>,
    open: bool,
    text: String,
    /// a save on its way: Some("") until the shell says which file
    saving: Option<String>,
    folder: String,
    /// the loop, for what a thread reads (the clipboard)
    to: Option<smithay_client_toolkit::reexports::calloop::channel::Sender<crate::Msg>>,
}

impl Notes {
    fn window(&mut self, st: &mut State, output: &str) -> bool {
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            self.text = w.ui.get_text().to_string();
            Glass::clear_now(&w.output, NS);
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
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
        let ui = ui::QuickNote::new().expect("QuickNote");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.global::<ui::Symbols>().on_of(|g| crate::icons::symbol(&g).into());
        let to = st.shell.sender(NAME);
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                ui.$set(move |$($a),*| to(Box::new($m)));
            }};
        }
        on!(on_edited, |t| M::Edited(t.to_string()));
        on!(on_save, | | M::Save);
        on!(on_dismissed, | | M::Dismissed);
        on!(on_closed, | | M::Closed);
        on!(on_pick, |i| M::Pick(i));
        ui.set_cookie(crate::surfaces::shapes::outline("cookie9").into());
        ui.set_note_glyph(crate::icons::glyph("note-pencil").into());
        ui.set_plus_glyph(crate::icons::glyph("plus").into());
        ui.set_x_glyph(crate::icons::glyph("x").into());
        let items: Vec<ui::NoteMenuItem> = MENU.iter().map(|(t, i)| ui::NoteMenuItem { text: (*t).into(), icon: crate::icons::glyph(i).into() }).collect();
        ui.set_menu(ModelRc::new(VecModel::from(items)));
        ui.set_text(self.text.clone().into());
        ui.set_text_trimmed_empty(blank(&self.text));
        if !self.folder.is_empty() {
            ui.set_folder(self.folder.clone().into());
        }
        ui.show().expect("show quick note");
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
        self.open = true;
        let w = self.win.as_ref().unwrap();
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        w.surface.show(0, 0);
        w.ui.set_saving(self.saving.is_some());
        w.ui.set_shown(true);
        w.ui.invoke_open();
    }

    fn close(&mut self, tell: bool) {
        if !self.open {
            return;
        }
        self.open = false;
        if let Some(w) = &self.win {
            // (the draft stays: the text field keeps it)
            self.text = w.ui.get_text().to_string();
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

    fn save(&mut self) {
        let Some(w) = &self.win else { return };
        let text = w.ui.get_text().to_string();
        if self.saving.is_some() || blank(&text) {
            return;
        }
        self.saving = Some(String::new());
        w.ui.set_saving(true);
        crate::shell::ask(NAME, serde_json::json!(["save", text]));
    }

    /// Text into the note at the cursor (typed into the field: Slint keeps
    /// the cursor to itself)
    fn put(&self, t: &str) {
        let Some(w) = &self.win else { return };
        if t.is_empty() || self.saving.is_some() {
            return;
        }
        w.ui.invoke_focus_editor();
        let win = w.surface.window();
        win.dispatch_event(slint::platform::WindowEvent::KeyPressed { text: t.into() });
        win.dispatch_event(slint::platform::WindowEvent::KeyReleased { text: t.into() });
    }

    fn pick(&mut self, i: i32) {
        let text = self.win.as_ref().map(|w| w.ui.get_text().to_string()).unwrap_or_default();
        match i {
            0 => self.put(checklist(&text)),
            1 => {
                // (what's copied, read off the UI thread)
                let to = self.to.clone();
                std::thread::spawn(move || {
                    let out = std::process::Command::new("timeout").args(["2", "wl-paste", "--no-newline", "--type", "text"]).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output();
                    if let (Ok(o), Some(to)) = (out, to) {
                        if o.status.success() {
                            let _ = to.send(crate::Msg::Part(NAME, Box::new(M::Paste(String::from_utf8_lossy(&o.stdout).into_owned()))));
                        }
                    }
                });
            }
            2 => self.put(&today()),
            3 => {
                self.close(true);
                crate::shell::ask(NAME, serde_json::json!(["openFolder"]));
            }
            _ => {}
        }
    }

    fn said(&mut self, v: &serde_json::Value) {
        if let Some(f) = v["folder"].as_str() {
            self.folder = f.to_string();
            if let Some(w) = &self.win {
                w.ui.set_folder(f.into());
            }
        }
        if let Some(p) = v["saving"].as_str() {
            // ("" from the shell: nothing was saved)
            self.saving = (!p.is_empty()).then(|| p.to_string());
            if let Some(w) = &self.win {
                w.ui.set_saving(self.saving.is_some());
            }
        }
        if let Some(p) = v["saved"].as_str() {
            if self.saving.as_deref() == Some(p) {
                self.saving = None;
                self.text.clear();
                if let Some(w) = &self.win {
                    w.ui.set_saving(false);
                    w.ui.set_text("".into());
                    w.ui.set_text_trimmed_empty(true);
                }
                self.close(true);
            }
        }
        if let Some(p) = v["failed"].as_str() {
            if self.saving.as_deref() == Some(p) || self.saving.as_deref() == Some("") {
                self.saving = None;
                if let Some(w) = &self.win {
                    w.ui.set_saving(false);
                }
            }
        }
    }
}

impl Part for Notes {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        self.to = Some(st.tx.clone());
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.open(st, cmd.screen.clone()),
            "close" => self.close(false),
            _ => {}
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            if let Some(w) = &mut self.win {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
                w.glass_sent.clear();
            }
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = &mut self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.glass_sent.clear();
        }
    }

    fn data(&mut self, _st: &mut State, _what: TypeId) {}

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<serde_json::Value>() {
            Ok(v) => return self.said(&v),
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Edited(t) => {
                if let Some(w) = &self.win {
                    w.ui.set_text_trimmed_empty(blank(&t));
                }
                self.text = t;
            }
            M::Save => self.save(),
            M::Dismissed => self.close(true),
            M::Closed => self.closed(st),
            M::Pick(i) => self.pick(i),
            M::Paste(t) => self.put(&t),
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
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
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

    fn quit(&mut self, _: &mut State) {
        if let Some(w) = &self.win {
            Glass::clear_now(&w.output, NS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_checklist_item_starts_its_own_line() {
        assert_eq!(checklist(""), "- [ ] ");
        assert_eq!(checklist("Shopping\n"), "- [ ] ");
        assert_eq!(checklist("Shopping"), "\n- [ ] ");
    }

    #[test]
    fn blanks_are_not_saved() {
        assert!(blank(""));
        assert!(blank("  \n\t "));
        assert!(!blank(" a "));
    }

    #[test]
    fn todays_date_reads_like_the_qml() {
        let d = today();
        // "Thursday 5 November 2026": a weekday, a day without a leading 0, a month, a year
        let parts: Vec<&str> = d.split(' ').collect();
        assert_eq!(parts.len(), 4, "{d}");
        assert!(!parts[1].starts_with('0'));
        assert_eq!(parts[3].len(), 4);
    }

    #[test]
    fn the_menu_has_its_four_items() {
        assert_eq!(MENU.len(), 4);
        for (_, icon) in MENU {
            assert!(!crate::icons::glyph(icon).is_empty(), "{icon}");
        }
    }
}
