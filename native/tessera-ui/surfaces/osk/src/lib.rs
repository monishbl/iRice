//! The on-screen keyboard (rust.surfaces: osk), compare
//! shell/modules/osk/Keyboard.qml. Panels calls it "keyboard" (its
//! shortcut, Control Center's Keyboard button); RustUi.qml maps that panel
//! to this surface and forwards its open and close. It never takes the
//! keyboard focus: a key goes to the focused window through the
//! virtual-keyboard protocol on tessera-ui's own display (vkbd.rs: in the
//! sandbox the arena's windows, nothing else). Modifiers are sticky (tap
//! Shift, then a key); windows make room (an exclusive zone).
//!
//! It types as `wtype` (which the QML ran per key) does: a keymap with a key
//! of its own for every keysym (keys.rs), the latched modifiers as the
//! protocol's `modifiers` request. Where it differs: a shifted key types the
//! character its cap shows ("!" for Shift + 1), where wtype -M shift -k 1
//! typed "1". Material's press swell (ElasticControl) is a small scale on
//! Material's fast spring here.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod keys;
pub mod vkbd;

use tessera_core::shell::{Command, Part};
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use keys::{Mods, ROWS};
use slint::{ComponentHandle, ModelRc, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::rc::Rc;

const NAME: &str = "osk";
pub const NS: &str = "tessera:keyboard";
const M: f32 = 20.0; // room for the shadow round the keyboard

enum Msg {
    Press(i32, i32),
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::Keyboard,
    a11y: Option<crate::a11y::A11y>,
    size: (u32, u32),
    screen: (f32, f32),
    glass_sent: String,
    settled: bool,
}

#[derive(Default)]
pub struct Osk {
    win: Option<Win>,
    mods: Mods,
    caps: bool,
    typist: Option<vkbd::Typist>,
}

impl Osk {
    fn rows(&self) -> ModelRc<ui::OskRow> {
        let rows: Vec<ui::OskRow> = ROWS
            .iter()
            .map(|r| ui::OskRow {
                keys: ModelRc::new(VecModel::from(
                    r.iter()
                        .map(|k| ui::OskKey {
                            text: k.text(self.mods, self.caps).into(),
                            units: k.units,
                            modifier: k.is_mod(),
                            wide: k.wide(),
                            action: k.action(),
                            latched: k.latched(self.mods, self.caps),
                            long: k.label.chars().count() > 1,
                            name: if k.sym == "space" { "Space".into() } else { k.text(self.mods, self.caps).into() },
                        })
                        .collect::<Vec<_>>(),
                )),
            })
            .collect();
        ModelRc::new(VecModel::from(rows))
    }

    fn refresh(&self) {
        if let Some(w) = &self.win {
            w.ui.set_rows(self.rows());
        }
    }

    fn open(&mut self, st: &mut State, screen: Option<String>) {
        let output = screen.or_else(|| self.win.as_ref().map(|w| w.output.clone())).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(output) = output else { return };
        if self.win.as_ref().is_some_and(|w| w.output == output) {
            return;
        }
        self.close(st);
        let Some(wl_output) = st.output_named(&output) else { return };
        let screen = crate::surfaces::kit::screen_size(st, &wl_output);
        let size = keys::window_size(screen.0, M);
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((&wl_output, &output)),
            anchor: Anchor::BOTTOM,
            size,
            exclusive: size.1 as i32 - M as i32,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        surface.layer.set_margin(0, 0, 8 - M as i32, 0);
        surface.layer.commit();
        let ui = ui::Keyboard::new().expect("Keyboard");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.set_on_glass(st.shell.plugin && !st.shell.look.material);
        ui.set_unit(keys::unit(screen.0));
        let to = st.shell.sender(NAME);
        ui.on_pressed(move |r, c| to(Box::new(Msg::Press(r, c))));
        ui.show().expect("show keyboard");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output, surface, ui, a11y, size, screen, glass_sent: String::new(), settled: false });
        self.refresh();
        if self.typist.is_none() {
            self.typist = Some(vkbd::Typist::spawn());
        }
    }

    fn close(&mut self, st: &mut State) {
        if let Some(w) = self.win.take() {
            st.shell.glass.set(&w.output, NS, &[]);
            // (no room kept for it: the zone goes with the same commit that unmaps it)
            w.surface.layer.set_exclusive_zone(0);
            w.surface.hide(); // (unmapped now: dropping the window alone leaves it on screen)
            st.drop_surface(&w.surface);
        }
    }

    fn press(&mut self, r: i32, c: i32) {
        let Some(key) = usize::try_from(r).ok().and_then(|r| ROWS.get(r)).and_then(|row| usize::try_from(c).ok().and_then(|c| row.get(c))) else { return };
        let (stroke, mods, caps) = keys::press(key, self.mods, self.caps);
        if let (Some(s), Some(t)) = (stroke, &self.typist) {
            t.type_stroke(s);
        }
        if (mods, caps) != (self.mods, self.caps) {
            self.mods = mods;
            self.caps = caps;
            self.refresh();
        }
    }
}

impl Part for Osk {
    fn name(&self) -> &'static str {
        NAME
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.open(st, cmd.screen.clone()),
            "close" => self.close(st),
            _ => {}
        }
    }

    fn message(&mut self, _: &mut State, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Press(r, c) => self.press(r, c),
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if let (true, Some(w)) = (look, self.win.as_mut()) {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.ui.set_on_glass(st.shell.plugin && !st.shell.look.material);
            w.glass_sent.clear();
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        self.config_changed(st, true);
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            self.close(st);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.close(st);
        }
    }

    fn after_frame(&mut self, st: &mut State) {
        let Some(w) = self.win.as_mut() else { return };
        crate::picture::second_frame(&w.surface, &mut w.settled);
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, false);
        }
        if !st.shell.plugin || st.shell.look.material || !w.surface.mapped.get() {
            return;
        }
        // (the window sits at the bottom centre, 8 px up, its shadow's room round it)
        let (sw, sh) = w.screen;
        let (ww, wh) = (w.size.0 as f32, w.size.1 as f32);
        let (x, y) = ((sw - ww) / 2.0 + M, sh - wh - (8.0 - M) + M);
        let shape = theme::glass_shape(&st.shell.look, x.round(), y.round(), ww - 2.0 * M, wh - 2.0 * M, 26.0, 1.0, 1.0);
        let key = format!("{shape:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, std::slice::from_ref(&shape));
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    fn stats(&self) {
        if let Some(w) = &self.win {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report("osk"));
        }
    }

    fn quit(&mut self, _: &mut State) {
        if let Some(w) = &self.win {
            crate::services::hypr::Glass::clear_now(&w.output, NS);
        }
    }
}
