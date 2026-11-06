//! Identify Displays and "Keep these display settings?" (rust.surfaces:
//! displayoverlay), compare shell/modules/osd/DisplayOverlay.qml and its
//! Material twins MaterialDisplayIdentify.qml and MaterialDisplayConfirm.qml.
//! Settings → Displays on every screen: "Identify" shows each screen's number
//! and name in its top left corner (for 2.5 s); after a change that could
//! leave a screen dark every screen asks "Keep these display settings?" with
//! the countdown, so it can be answered from whichever screen still shows a
//! picture. Virtual screens (HEADLESS-*, TESSERA-*) get nothing.
//!
//! The revert is not this surface's: the countdown runs in the host
//! (host/system displays.rs: 15 s, then the previous settings are put back
//! unless `displays.keep` came first), whether or not a card is on any
//! screen. This only shows it and answers: "Go back" says `displays.revert`,
//! "Keep settings" `displays.keep`. The host says (every second while
//! confirming) what to show:
//!   {"to":"displayoverlay","identifying":false,"confirming":true,"left":12,
//!    "monitors":[{"name":"DP-1","label":"Dell …","number":1,"lw":2560,"lh":1440}]}
//!
//! Glass: the cards on the plugin's glass (a tint without it); Material: a
//! surfaceContainerHigh card that springs in, the confirm one with a wavy
//! bar running down. Material draws its cookie from a polar outline (not the
//! QML's shader shape).

use super::*;
use serde_json::{json, Value};
use slint::ComponentHandle;
use smithay_client_toolkit::shell::WaylandSurface;
use tessera_core::shell::{self, Part};
use tessera_core::surfaces::{kit, shapes};
use tessera_core::theme;

const NAME: &str = "displayoverlay";
const ID_NS: &str = "tessera:display-identify";
const CF_NS: &str = "tessera:display-confirm";

/// Identify's card window (the card sits 24 px inside it, for the glass's shadow)
const ID_W: u32 = 640;

enum IdUi {
    Glass(ui::IdentifyGlass),
    Material(ui::IdentifyMaterial),
}

enum CfUi {
    Glass(ui::ConfirmGlass),
    Material(ui::ConfirmMaterial),
}

struct IdWin {
    output: String,
    surface: Rc<Surface>,
    ui: IdUi,
    glass_sent: String,
}

struct CfWin {
    output: String,
    surface: Rc<Surface>,
    ui: CfUi,
    /// the countdown's length, as it was when the dialog came up
    total: i32,
    glass_sent: String,
    region: (i32, i32, i32, i32),
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Mon {
    name: String,
    label: String,
    number: i64,
    lw: i64,
    lh: i64,
}

#[derive(Default)]
pub struct DisplayOverlay {
    identifying: bool,
    confirming: bool,
    left: i32,
    mons: Vec<Mon>,
    ids: Vec<IdWin>,
    cfs: Vec<CfWin>,
}

/// The countdown in words
pub fn countdown_text(left: i32) -> String {
    format!("Going back to the previous settings in {left} {}.", if left == 1 { "second" } else { "seconds" })
}

/// A screen that gets a card (not a virtual one)
pub fn real_screen(name: &str) -> bool {
    !(name.starts_with("HEADLESS") || name.starts_with("TESSERA-"))
}

impl DisplayOverlay {
    fn mon(&self, st: &State, name: &str) -> Mon {
        if let Some(m) = self.mons.iter().find(|m| m.name == name) {
            return m.clone();
        }
        let size = st.output_named(name).map(|o| kit::screen_size(st, &o)).unwrap_or((0.0, 0.0));
        Mon { name: name.to_string(), label: name.to_string(), number: 0, lw: size.0 as i64, lh: size.1 as i64 }
    }

    fn outputs(&self, st: &State) -> Vec<(WlOutput, String)> {
        st.outputs_now().into_iter().filter(|(_, n)| real_screen(n)).collect()
    }

    fn make_identify(&mut self, st: &mut State, wl: &WlOutput, name: &str) {
        let m = self.mon(st, name);
        let material = st.shell.look.material;
        let h: u32 = (if material { 136 } else { 132 }) + 48;
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: ID_NS,
            output: Some((wl, name)),
            anchor: Anchor::TOP | Anchor::LEFT,
            size: (ID_W, h),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 32 - 24,
        });
        surface.layer.set_margin(48 - 24, 0, 0, 32 - 24);
        surface.layer.commit();
        let number: slint::SharedString = (if m.number > 0 { m.number.to_string() } else { "?".to_string() }).into();
        let label: slint::SharedString = (if m.label.is_empty() { name.to_string() } else { m.label.clone() }).into();
        let detail: slint::SharedString = format!("{name} · {} × {}", m.lw, m.lh).into();
        let ui = if material {
            let u = ui::IdentifyMaterial::new().expect("IdentifyMaterial");
            st.shell.apply_theme(&u.global::<ui::Theme>());
            u.set_number(number);
            u.set_label(label);
            u.set_detail(detail);
            u.set_cookie(shapes::outline("cookie9").into());
            u.show().expect("show identify");
            u.set_shown(true);
            IdUi::Material(u)
        } else {
            let u = ui::IdentifyGlass::new().expect("IdentifyGlass");
            st.shell.apply_theme(&u.global::<ui::Theme>());
            u.set_number(number);
            u.set_label(label);
            u.set_detail(detail);
            u.set_on_glass(st.shell.plugin);
            u.show().expect("show identify");
            IdUi::Glass(u)
        };
        st.set_input_region(&surface, Some(&[]));
        self.ids.push(IdWin { output: name.to_string(), surface, ui, glass_sent: String::new() });
    }

    fn make_confirm(&mut self, st: &mut State, wl: &WlOutput, name: &str) {
        let material = st.shell.look.material;
        let (w, h) = if material { (400 + 64, 280 + 64) } else { (360 + 48, 150 + 48) };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: CF_NS,
            output: Some((wl, name)),
            anchor: Anchor::empty(),
            size: (w, h),
            exclusive: -1,
            keyboard: KeyboardInteractivity::OnDemand,
            margin_left: 0,
        });
        let to = st.shell.sender(NAME);
        let total = self.left.max(1);
        let text = countdown_text(self.left);
        let (ui, region) = if material {
            let u = ui::ConfirmMaterial::new().expect("ConfirmMaterial");
            st.shell.apply_theme(&u.global::<ui::Theme>());
            u.set_countdown(text.into());
            u.set_value(self.left as f32 / total as f32);
            let (a, b) = (to.clone(), to);
            u.on_revert(move || a(Box::new(Msg::Revert)));
            u.on_keep(move || b(Box::new(Msg::Keep)));
            u.show().expect("show confirm");
            u.set_shown(true);
            (CfUi::Material(u), (32, 32, 400, 200))
        } else {
            let u = ui::ConfirmGlass::new().expect("ConfirmGlass");
            st.shell.apply_theme(&u.global::<ui::Theme>());
            u.set_countdown(text.into());
            u.set_on_glass(st.shell.plugin);
            let (a, b) = (to.clone(), to);
            u.on_revert(move || a(Box::new(Msg::Revert)));
            u.on_keep(move || b(Box::new(Msg::Keep)));
            u.show().expect("show confirm");
            (CfUi::Glass(u), (24, 24, 360, 150))
        };
        st.set_input_region(&surface, Some(&[region]));
        self.cfs.push(CfWin { output: name.to_string(), surface, ui, total, glass_sent: String::new(), region });
    }

    fn drop_identify(&mut self, st: &mut State) {
        for w in std::mem::take(&mut self.ids) {
            st.shell.glass.set(&w.output, ID_NS, &[]);
            retire(st, &w.surface);
        }
    }

    fn drop_confirm(&mut self, st: &mut State) {
        for w in std::mem::take(&mut self.cfs) {
            st.shell.glass.set(&w.output, CF_NS, &[]);
            retire(st, &w.surface);
        }
    }

    /// Make and drop cards until they are what is wanted now, and update their words
    fn sync(&mut self, st: &mut State) {
        if !super::on() {
            return;
        }
        let outs = self.outputs(st);
        if !self.identifying {
            self.drop_identify(st);
        } else {
            let stale: Vec<usize> = (0..self.ids.len()).rev().filter(|i| !outs.iter().any(|(_, n)| *n == self.ids[*i].output)).collect();
            for i in stale {
                let w = self.ids.remove(i);
                retire(st, &w.surface);
            }
            for (wl, n) in &outs {
                if !self.ids.iter().any(|w| w.output == *n) {
                    self.make_identify(st, wl, n);
                }
            }
        }
        if !self.confirming {
            self.drop_confirm(st);
        } else {
            let stale: Vec<usize> = (0..self.cfs.len()).rev().filter(|i| !outs.iter().any(|(_, n)| *n == self.cfs[*i].output)).collect();
            for i in stale {
                let w = self.cfs.remove(i);
                retire(st, &w.surface);
            }
            for (wl, n) in &outs {
                if !self.cfs.iter().any(|w| w.output == *n) {
                    self.make_confirm(st, wl, n);
                }
            }
            let text = countdown_text(self.left);
            for w in &self.cfs {
                match &w.ui {
                    CfUi::Glass(u) => u.set_countdown(text.clone().into()),
                    CfUi::Material(u) => {
                        u.set_countdown(text.clone().into());
                        u.set_value(self.left as f32 / w.total as f32);
                    }
                }
            }
        }
    }
}

/// Its own messages (from the cards' buttons)
enum Msg {
    Revert,
    Keep,
}

fn parse_mons(v: &Value) -> Vec<Mon> {
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|m| Mon {
                    name: m["name"].as_str().unwrap_or("").to_string(),
                    label: m["label"].as_str().unwrap_or("").to_string(),
                    number: m["number"].as_i64().unwrap_or(0),
                    lw: m["lw"].as_i64().or_else(|| m["width"].as_i64()).unwrap_or(0),
                    lh: m["lh"].as_i64().or_else(|| m["height"].as_i64()).unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default()
}

impl Part for DisplayOverlay {
    fn name(&self) -> &'static str {
        NAME
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match said(msg) {
            Ok(v) => {
                if let Some(b) = v["identifying"].as_bool() {
                    self.identifying = b;
                }
                if let Some(b) = v["confirming"].as_bool() {
                    // (a new question: its countdown starts from the full length)
                    if b && !self.confirming {
                        self.drop_confirm(st);
                    }
                    self.confirming = b;
                }
                if let Some(n) = v["left"].as_i64() {
                    self.left = n as i32;
                }
                if v["monitors"].is_array() {
                    self.mons = parse_mons(&v["monitors"]);
                }
                self.sync(st);
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Revert => shell::ask("displays.revert", json!([])),
            Msg::Keep => shell::ask("displays.keep", json!([])),
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look && (!self.ids.is_empty() || !self.cfs.is_empty()) {
            // (each look has its own card: made again)
            self.drop_identify(st);
            self.drop_confirm(st);
            self.sync(st);
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        let on = st.shell.plugin && !st.shell.look.material;
        for w in &mut self.ids {
            if let IdUi::Glass(u) = &w.ui {
                u.set_on_glass(on);
            }
            w.glass_sent.clear();
        }
        for w in &mut self.cfs {
            if let CfUi::Glass(u) = &w.ui {
                u.set_on_glass(on);
            }
            w.glass_sent.clear();
        }
    }

    fn output_added(&mut self, st: &mut State, _: &WlOutput, _: &str) {
        self.sync(st);
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.ids.iter().position(|w| w.output == name) {
            let w = self.ids.remove(i);
            retire(st, &w.surface);
        }
        if let Some(i) = self.cfs.iter().position(|w| w.output == name) {
            let w = self.cfs.remove(i);
            retire(st, &w.surface);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        let name = s.output.clone();
        if self.ids.iter().any(|w| Rc::ptr_eq(&w.surface, s)) || self.cfs.iter().any(|w| Rc::ptr_eq(&w.surface, s)) {
            self.output_removed(st, &name);
        }
    }

    /// Glass: the cards' shapes on the plugin's glass; Material's confirm card
    /// takes input only where it is
    fn after_frame(&mut self, st: &mut State) {
        let glass_on = st.shell.plugin && !st.shell.look.material;
        for w in &mut self.ids {
            let IdUi::Glass(u) = &w.ui else { continue };
            if !glass_on || !w.surface.mapped.get() {
                continue;
            }
            let shape = theme::glass_shape(&st.shell.look, u.get_card_x(), u.get_card_y(), u.get_card_w(), u.get_card_h(), 26.0, 1.0, 1.0);
            let key = format!("{shape:?}");
            if key != w.glass_sent {
                st.shell.glass.set(&w.output, ID_NS, std::slice::from_ref(&shape));
                w.glass_sent = key;
            }
        }
        for w in &mut self.cfs {
            if !w.surface.mapped.get() {
                continue;
            }
            match &w.ui {
                CfUi::Glass(u) => {
                    if !glass_on {
                        continue;
                    }
                    let shape = theme::glass_shape(&st.shell.look, u.get_card_x(), u.get_card_y(), u.get_card_w(), u.get_card_h(), 22.0, 1.0, 1.0);
                    let key = format!("{shape:?}");
                    if key != w.glass_sent {
                        st.shell.glass.set(&w.output, CF_NS, std::slice::from_ref(&shape));
                        w.glass_sent = key;
                    }
                }
                CfUi::Material(u) => {
                    let r = (u.get_card_x() as i32, u.get_card_y() as i32, u.get_card_w() as i32, u.get_card_h() as i32);
                    if r != w.region && r.2 > 0 && r.3 > 0 {
                        w.region = r;
                        st.set_input_region(&w.surface, Some(&[r]));
                    }
                }
            }
        }
    }

    fn quit(&mut self, st: &mut State) {
        for w in &self.ids {
            tessera_core::services::hypr::Glass::clear_now(&w.output, ID_NS);
        }
        for w in &self.cfs {
            tessera_core::services::hypr::Glass::clear_now(&w.output, CF_NS);
        }
        let _ = st;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_countdown_in_words() {
        assert_eq!(countdown_text(15), "Going back to the previous settings in 15 seconds.");
        assert_eq!(countdown_text(1), "Going back to the previous settings in 1 second.");
    }

    #[test]
    fn virtual_screens_get_no_card() {
        assert!(real_screen("DP-1"));
        assert!(!real_screen("HEADLESS-2"));
        assert!(!real_screen("TESSERA-1"));
    }

    #[test]
    fn monitors_as_the_host_says_them() {
        let m = parse_mons(&serde_json::json!([{"name": "DP-1", "label": "Dell", "number": 2, "lw": 2560, "lh": 1440}]));
        assert_eq!(m, vec![Mon { name: "DP-1".into(), label: "Dell".into(), number: 2, lw: 2560, lh: 1440 }]);
    }
}
