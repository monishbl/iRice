//! The snap preview (rust.surfaces: tiling), compare
//! shell/modules/tiling/SnapPreview.qml: while a window is dragged to a
//! screen edge or corner, an outline of where it will land. Takes no input;
//! the drag goes on underneath. The glass plugin says where the drag is
//! (`tessera-drag>>zone,MON,ZONE,ADDR` on Hyprland's event socket, followed
//! here: events.rs); the target is Tiling.qml's rectangle for that zone on
//! that monitor's usable area with Settings' gaps (zones.rs). Letting go
//! there still tiles the window through the shell's Tiling service, which
//! keeps each window's size before its first snap (Restore).
//! Glass: the plugin's glass at the window radius with an accent tint and a
//! light rim; Material: no glass, a primary container at half alpha
//! outlined in primary, as Android's desktop windowing previews a snap.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub use tessera_core::surfaces::{events, zones}; // (in core: decorations uses them too)

use tessera_core::shell::Part;
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use slint::ComponentHandle;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::Any;
use std::rc::Rc;
use zones::Monitor;

const NAME: &str = "tiling";
const NS: &str = "tessera:snappreview";

enum Msg {
    /// the drag's zone on a monitor ("none": off every edge, or dropped)
    Zone(String, String, Option<Monitor>),
    Faded(String),
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::SnapPreview,
    glass_sent: String,
    shown: bool,
}

#[derive(Default)]
pub struct Tiling {
    wins: Vec<Win>,
}

fn monitors() -> Vec<Monitor> {
    crate::services::hypr::request("j/monitors").ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

impl Tiling {
    fn win(&mut self, st: &mut State, output: &str) -> Option<&mut Win> {
        if let Some(i) = self.wins.iter().position(|w| w.output == output) {
            return self.wins.get_mut(i);
        }
        let wl_output = st.output_named(output)?;
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
        st.set_input_region(&surface, Some(&[]));
        let ui = ui::SnapPreview::new().expect("SnapPreview");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = st.shell.sender(NAME);
        let o = output.to_string();
        ui.on_faded(move || to(Box::new(Msg::Faded(o.clone()))));
        ui.show().expect("show snap preview");
        self.wins.push(Win { output: output.to_string(), surface, ui, glass_sent: String::new(), shown: true });
        self.wins.last_mut()
    }

    fn zone(&mut self, st: &mut State, mon: String, zone: String, m: Option<Monitor>) {
        let target = m.as_ref().and_then(|m| zones::target(&zone, m, &st.shell.cfg));
        for w in self.wins.iter().filter(|w| w.output != mon || target.is_none()) {
            w.ui.set_active(false);
        }
        let Some(t) = target else { return };
        let radius = st.shell.cfg["appearance"]["windowRadius"].as_f64().unwrap_or(20.0) as f32;
        let on_glass = st.shell.plugin && !st.shell.look.material;
        let Some(w) = self.win(st, &mon) else { return };
        if !w.shown {
            w.shown = true;
            w.surface.show(0, 0);
        }
        w.ui.set_radius(radius);
        w.ui.set_on_glass(on_glass);
        w.ui.set_tx(t.x as f32);
        w.ui.set_ty(t.y as f32);
        w.ui.set_tw(t.w as f32);
        w.ui.set_th(t.h as f32);
        w.ui.set_active(true);
    }

    fn faded(&mut self, st: &mut State, output: &str) {
        if let Some(w) = self.wins.iter_mut().find(|w| w.output == output) {
            if !w.ui.get_active() && w.shown {
                w.shown = false;
                st.shell.glass.set(&w.output, NS, &[]);
                w.glass_sent.clear();
                w.surface.hide();
            }
        }
    }
}

impl Part for Tiling {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        events::follow(NAME, st.tx.clone(), |line| {
            let (kind, mon, zone) = zones::drag_event(line)?;
            if kind == "zone" && zone != "none" {
                let m = monitors().into_iter().find(|m| m.name == mon);
                Some(Box::new(Msg::Zone(mon, zone, m)) as Box<dyn Any + Send>)
            } else {
                Some(Box::new(Msg::Zone(mon, "none".into(), None)) as Box<dyn Any + Send>)
            }
        });
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Zone(mon, zone, m) => self.zone(st, mon, zone, m),
            Msg::Faded(o) => self.faded(st, &o),
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            for w in &mut self.wins {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
                w.glass_sent.clear();
            }
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        self.config_changed(st, true);
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.wins.iter().position(|w| w.output == name) {
            let w = self.wins.remove(i);
            w.surface.hide(); // (unmapped now: dropping the window alone leaves it on screen)
            st.drop_surface(&w.surface);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if let Some(w) = self.wins.iter().find(|w| Rc::ptr_eq(&w.surface, s)) {
            let name = w.output.clone();
            self.output_removed(st, &name);
        }
    }

    /// The glass follows the outline as it moves, grows and fades
    fn after_frame(&mut self, st: &mut State) {
        if !st.shell.plugin || st.shell.look.material {
            return;
        }
        for w in &mut self.wins {
            if !w.shown || !w.surface.mapped.get() {
                continue;
            }
            let ui = &w.ui;
            let (o, s) = (ui.get_shown_opacity(), ui.get_shown_scale());
            let (x, y, ww, hh) = (ui.get_ax(), ui.get_ay(), ui.get_aw(), ui.get_ah());
            let (sw, sh) = (ww * s, hh * s);
            let shape = theme::glass_shape(&st.shell.look, x + (ww - sw) / 2.0, y + (hh - sh) / 2.0, sw, sh, ui.get_radius() * s, o, 1.0);
            let key = format!("{shape:?}");
            if key != w.glass_sent {
                st.shell.glass.set(&w.output, NS, if o > 0.01 && sw >= 1.0 { std::slice::from_ref(&shape) } else { &[] });
                w.glass_sent = key;
            }
        }
    }

    fn quit(&mut self, _: &mut State) {
        for w in &self.wins {
            crate::services::hypr::Glass::clear_now(&w.output, NS);
        }
    }
}
