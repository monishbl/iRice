//! The cheatsheet (rust.surfaces: cheatsheet), compare
//! shell/modules/cheatsheet/Cheatsheet.qml in its glass Dialog
//! (shell/glass/Dialog.qml): keyboard shortcuts read live from Hyprland
//! (binds.rs) and a periodic table (elements.rs), on two tabs (Tab
//! switches). A panel: Panels opens and closes it (its shortcut, IPC) and
//! RustUi.qml forwards those here; Esc or a click outside closes it.
//! Where it differs: the dialog's elastic pull while dragged (ElasticGrab)
//! isn't here; it opens on a spring and closes on the exit curve as the QML.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod binds;
pub mod elements;

use tessera_core::shell::{self, Command, Part};
use tessera_core::surfaces::kit::hex;
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use slint::{ComponentHandle, ModelRc, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::Any;
use std::rc::Rc;

const NAME: &str = "cheatsheet";
const NS: &str = "tessera:cheatsheet";

enum Msg {
    Binds(Vec<binds::Group>),
    Dismissed,
    Closed,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::Cheatsheet,
    a11y: Option<crate::a11y::A11y>,
    shown: bool,
    glass_sent: String,
    settled: bool,
}

#[derive(Default)]
pub struct Cheatsheet {
    win: Option<Win>,
    groups: Vec<binds::Group>,
}

/// The dialog's content size on a screen (Cheatsheet.qml)
pub fn content_size(w: f32, h: f32) -> (f32, f32) {
    ((w - 160.0).min(1100.0), (h - 200.0).min(660.0))
}

/// The groups, three to a row (the QML's Flow at a third of the width each)
pub fn in_rows<T: Clone>(items: &[T], per: usize) -> Vec<Vec<T>> {
    items.chunks(per.max(1)).map(|c| c.to_vec()).collect()
}

impl Cheatsheet {
    fn refresh_binds(&self, st: &State) {
        let tx = st.tx.clone();
        std::thread::spawn(move || {
            if let Some(g) = crate::services::hypr::request("j/binds").ok().and_then(|j| binds::groups(&j)) {
                let _ = tx.send(tessera_core::Msg::Part(NAME, Box::new(Msg::Binds(g))));
            }
        });
    }

    fn set_groups(&self, st: &State) {
        let Some(w) = &self.win else { return };
        let style = binds::style(&st.shell.cfg);
        let groups: Vec<ui::CsGroup> = self
            .groups
            .iter()
            .map(|g| ui::CsGroup {
                name: g.name.clone().into(),
                binds: ModelRc::new(VecModel::from(
                    g.binds
                        .iter()
                        .map(|b| ui::CsBind {
                            description: b.description.clone().into(),
                            keys: ModelRc::new(VecModel::from(binds::keys(style, &b.keys).into_iter().map(Into::into).collect::<Vec<slint::SharedString>>())),
                        })
                        .collect::<Vec<_>>(),
                )),
            })
            .collect();
        let rows: Vec<ui::CsRow> = in_rows(&groups, 3).into_iter().map(|r| ui::CsRow { groups: ModelRc::new(VecModel::from(r)) }).collect();
        w.ui.set_rows(ModelRc::new(VecModel::from(rows)));
    }

    fn set_elements(&self, st: &State) {
        let Some(w) = &self.win else { return };
        let tints = if st.shell.look.dark { elements::TINTS_DARK } else { elements::TINTS_LIGHT };
        let cats = elements::categories();
        let els: Vec<ui::CsElement> = elements::elements()
            .iter()
            .map(|e| {
                let (x, y) = elements::cell_xy(e);
                let (cat, tint) = cats.iter().find(|c| c.0 == e.category).map_or((String::new(), 0), |c| (c.1.clone(), c.2));
                ui::CsElement {
                    number: e.number as i32,
                    symbol: e.symbol.clone().into(),
                    name: e.name.clone().into(),
                    detail: format!("Atomic number {} · {} u · {cat}", e.number, e.mass).into(),
                    cx: x,
                    cy: y,
                    tint: hex(tints[tint.min(9)]),
                }
            })
            .collect();
        w.ui.set_elements(ModelRc::new(VecModel::from(els)));
    }

    fn open(&mut self, st: &mut State, screen: Option<String>) {
        let output = screen.or_else(|| self.win.as_ref().map(|w| w.output.clone())).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(output) = output else { return };
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            st.shell.glass.set(&w.output, NS, &[]);
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
            let ui = ui::Cheatsheet::new().expect("Cheatsheet");
            st.shell.apply_theme(&ui.global::<ui::Theme>());
            let to = st.shell.sender(NAME);
            let (t1, t2) = (to.clone(), to);
            ui.on_dismissed(move || t1(Box::new(Msg::Dismissed)));
            ui.on_closed(move || t2(Box::new(Msg::Closed)));
            ui.show().expect("show cheatsheet");
            let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
            self.win = Some(Win { output: output.clone(), surface, ui, a11y, shown: false, glass_sent: String::new(), settled: false });
            self.set_groups(st);
            self.set_elements(st);
        }
        let w = self.win.as_mut().unwrap();
        w.ui.set_on_glass(st.shell.plugin && !st.shell.look.material);
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        w.surface.show(0, 0);
        st.set_input_region(&w.surface, None);
        if !w.shown {
            w.shown = true;
            w.ui.set_shown(true);
            w.ui.invoke_focus_keys();
        }
        self.refresh_binds(st);
    }

    fn close(&mut self, st: &mut State, tell_shell: bool) {
        let Some(w) = self.win.as_mut() else { return };
        if w.shown {
            w.shown = false;
            w.ui.set_shown(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            // (while closing, clicks pass through: the layer stays for the fade)
            st.set_input_region(&w.surface, Some(&[]));
            w.surface.wl.commit();
        }
        if tell_shell {
            shell::closed(NAME);
        }
    }

    fn closed(&mut self, st: &mut State) {
        if self.win.as_ref().is_some_and(|w| !w.shown) {
            let w = self.win.take().unwrap();
            st.shell.glass.set(&w.output, NS, &[]);
            w.surface.hide(); // (unmapped now: dropping the window alone leaves it on screen)
            st.drop_surface(&w.surface);
        }
    }
}

impl Part for Cheatsheet {
    fn name(&self) -> &'static str {
        NAME
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.open(st, cmd.screen.clone()),
            "close" => self.close(st, false),
            _ => {}
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Binds(g) => {
                if g != self.groups {
                    self.groups = g;
                    self.set_groups(st);
                }
            }
            Msg::Dismissed => self.close(st, true),
            Msg::Closed => self.closed(st),
        }
    }

    fn keyboard_left(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| w.shown && Rc::ptr_eq(&w.surface, s)) {
            self.close(st, true);
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
            self.set_elements(st);
        }
        self.set_groups(st);
    }

    fn plugin_changed(&mut self, st: &mut State) {
        self.config_changed(st, true);
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
        if !st.shell.plugin || st.shell.look.material || !w.surface.mapped.get() {
            return;
        }
        let ui = &w.ui;
        let (p, s) = (ui.get_fade(), ui.get_scale());
        let (bx, by, bw, bh) = (ui.get_body_x(), ui.get_body_y(), ui.get_body_w(), ui.get_body_h());
        let (sw, sh) = (bw * s, bh * s);
        let shape = theme::glass_shape(&st.shell.look, bx + (bw - sw) / 2.0, by + (bh - sh) / 2.0, sw, sh, 28.0 * s, p, 1.0);
        let key = format!("{shape:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, if p > 0.002 { std::slice::from_ref(&shape) } else { &[] });
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
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

    #[test]
    fn sized_and_flowed_as_the_qml() {
        assert_eq!(content_size(1920.0, 1080.0), (1100.0, 660.0));
        assert_eq!(content_size(1280.0, 800.0), (1100.0, 600.0));
        assert_eq!(content_size(1000.0, 700.0), (840.0, 500.0));
        assert_eq!(in_rows(&[1, 2, 3, 4, 5], 3), vec![vec![1, 2, 3], vec![4, 5]]);
        assert!(in_rows::<i32>(&[], 3).is_empty());
    }
}
