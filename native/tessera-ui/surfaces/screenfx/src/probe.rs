//! The shelf's probes (rust.surfaces: shelfprobes), compare
//! shell/modules/shelf/ShelfProbes.qml. What tells a real drag of files from
//! a held button: only something being dragged can enter a drop area, never a
//! selection, a scrollbar or a window edge. While the button is held (the
//! host's `shelf.probing` in the shell state), each screen gets a thin
//! invisible strip at its left, right and top edges (3 px; when
//! `shelf.edge` is on), and a 360 px square under the pointer just after a
//! shake (`shelf.probe`: x, y, screen); a drag entering one is what opens the
//! shelf. They take no clicks beyond the drag (a held button keeps the pointer
//! with whatever it was pressed on) and go when it comes up. Something dropped
//! on one goes onto the shelf.
//!
//! What it tells the host (shell::ask("shelf", [what, …]), host/sharing
//! shelf.rs): `dragArrived [where, screen, y]` and `dragLeft [where]` with
//! `where` left | right | top | shake; on a drop `offer [screen]` then
//! `keep [uris, text]`.
//!
//! Differences from the QML: the platform tells the types a drag offers when
//! it comes, not its URIs (those are read on the drop), so a drag with a
//! file list opens the shelf (a browser's link drag also offers one) and the
//! drop keeps only `file:` URIs, refusing anything else.

use super::*;
use serde_json::json;
use slint::ComponentHandle;
use smithay_client_toolkit::shell::WaylandSurface;
use tessera_core::services::bridge::ShellState;
use tessera_core::dnd::Drag;
use tessera_core::shell::{self, Part};

const NAME: &str = "shelfprobes";
const NS: &str = "tessera:shelf-probe";

struct Probe {
    output: String,
    wher: &'static str,
    surface: Rc<Surface>,
    #[allow(dead_code)] // (kept: dropping the component would drop the window)
    ui: ui::Probe,
}

#[derive(Default)]
pub struct ShelfProbes {
    probes: Vec<Probe>,
    key: String,
}

/// What the probes should be, as a key: "DP-1:left,DP-1:right,DP-1:top,DP-1:shake@x,y"
/// (nothing while the button isn't held)
pub fn wanted(probing: bool, edge: bool, probe: Option<(f64, f64, String)>, outputs: &[String]) -> Vec<(String, String)> {
    let mut v = Vec::new();
    if !probing {
        return v;
    }
    for o in outputs {
        if edge {
            for w in ["left", "right", "top"] {
                v.push((o.clone(), w.to_string()));
            }
        }
        if let Some((x, y, s)) = &probe {
            if s == o {
                v.push((o.clone(), format!("shake@{},{}", x.round(), y.round())));
            }
        }
    }
    v
}

/// The host's `shelf.probing` and `shelf.probe` ({x, y, screen}) from the shell state
fn shelf_state(st: &State) -> (bool, Option<(f64, f64, String)>) {
    let Some(s) = st.shell.get::<ShellState>() else { return (false, None) };
    let probing = s.get("shelf.probing").as_bool().unwrap_or(false);
    let p = s.get("shelf.probe");
    let probe = match (p["x"].as_f64(), p["y"].as_f64()) {
        (Some(x), Some(y)) => Some((x, y, p["screen"].as_str().unwrap_or("").to_string())),
        _ => None,
    };
    (probing, probe)
}

impl ShelfProbes {
    fn current(&self, st: &State) -> (bool, bool, Option<(f64, f64, String)>) {
        let (probing, probe) = shelf_state(st);
        let edge = st.shell.cfg["shelf"]["edge"].as_bool().unwrap_or(true);
        (probing, edge, probe)
    }

    /// Make and drop probes until they are what is wanted now
    fn sync(&mut self, st: &mut State) {
        if !super::on() {
            return;
        }
        let (probing, edge, probe) = self.current(st);
        let outputs: Vec<String> = st.outputs_now().into_iter().map(|(_, n)| n).collect();
        let want = wanted(probing, edge, probe.clone(), &outputs);
        let key = want.iter().map(|(o, w)| format!("{o}:{w}")).collect::<Vec<_>>().join(",");
        if key == self.key {
            return;
        }
        self.key = key;
        for p in std::mem::take(&mut self.probes) {
            st.accept_drops(&p.surface, false);
            retire(st, &p.surface);
        }
        for (output, w) in want {
            let Some(wl) = st.output_named(&output) else { continue };
            let (wher, anchor, size, margin) = match w.as_str() {
                "left" => ("left", Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM, (3, 0), (0, 0)),
                "right" => ("right", Anchor::RIGHT | Anchor::TOP | Anchor::BOTTOM, (3, 0), (0, 0)),
                "top" => ("top", Anchor::LEFT | Anchor::RIGHT | Anchor::TOP, (0, 3), (0, 0)),
                _ => {
                    // (under the pointer, where the drag is)
                    let (px, py, _) = probe.clone().unwrap_or((0.0, 0.0, String::new()));
                    let origin = st.outputs.info(&wl).and_then(|i| i.logical_position).unwrap_or((0, 0));
                    let left = (px - origin.0 as f64 - 180.0).max(0.0) as i32;
                    let top = (py - origin.1 as f64 - 180.0).max(0.0) as i32;
                    ("shake", Anchor::LEFT | Anchor::TOP, (360, 360), (top, left))
                }
            };
            let surface = st.prepare_surface(Spec {
                layer: Layer::Overlay,
                namespace: NS,
                output: Some((&wl, &output)),
                anchor,
                size,
                exclusive: -1,
                keyboard: KeyboardInteractivity::None,
                margin_left: margin.1,
            });
            if margin.0 != 0 {
                surface.layer.set_margin(margin.0, 0, 0, margin.1);
                surface.layer.commit();
            }
            let ui = ui::Probe::new().expect("Probe");
            ui.show().expect("show shelf probe");
            st.accept_drops(&surface, true);
            self.probes.push(Probe { output, wher, surface, ui });
        }
    }

    fn find(&self, s: &Rc<Surface>) -> Option<&Probe> {
        self.probes.iter().find(|p| Rc::ptr_eq(&p.surface, s))
    }
}

impl Part for ShelfProbes {
    fn name(&self) -> &'static str {
        NAME
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<ShellState>() {
            self.sync(st);
        }
    }

    fn config_changed(&mut self, st: &mut State, _: bool) {
        self.sync(st);
    }

    fn output_added(&mut self, st: &mut State, _: &WlOutput, _: &str) {
        self.sync(st);
    }

    fn output_removed(&mut self, st: &mut State, _: &str) {
        self.sync(st);
    }

    fn drag(&mut self, st: &mut State, surface: &Rc<Surface>, ev: &Drag) {
        let Some(p) = self.find(surface) else { return };
        let (wher, output) = (p.wher, p.output.clone());
        // (the shake probe's name carries no position)
        match ev {
            Drag::Enter { y, mimes, .. } => {
                if !mimes.iter().any(|m| m == "text/uri-list") {
                    // (files only: a browser's image or selected text is dragged around all the time without meaning the shelf)
                    st.refuse_drop();
                    return;
                }
                shell::ask("shelf", json!(["dragArrived", wher, output, y]));
            }
            Drag::Motion { .. } => {}
            Drag::Leave => shell::ask("shelf", json!(["dragLeft", wher])),
            // (let go right on one: onto the shelf)
            Drag::Drop { uris, text, .. } => {
                if !uris.iter().any(|u| u.starts_with("file:")) {
                    st.refuse_drop();
                    return;
                }
                shell::ask("shelf", json!(["offer", output]));
                shell::ask("shelf", json!(["keep", uris, text]));
            }
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.find(s).is_some() {
            self.key.clear();
            self.sync(st);
        }
    }

    fn quit(&mut self, st: &mut State) {
        for p in std::mem::take(&mut self.probes) {
            retire(st, &p.surface);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_while_the_button_is_held() {
        let outs = vec!["DP-1".to_string(), "HDMI-A-1".to_string()];
        assert!(wanted(false, true, None, &outs).is_empty());
        assert_eq!(wanted(true, true, None, &outs).len(), 6);
        assert!(wanted(true, false, None, &outs).is_empty());
    }

    #[test]
    fn the_square_goes_on_the_shaken_screen_only() {
        let outs = vec!["DP-1".to_string(), "HDMI-A-1".to_string()];
        let w = wanted(true, false, Some((500.4, 300.0, "HDMI-A-1".into())), &outs);
        assert_eq!(w, vec![("HDMI-A-1".to_string(), "shake@500,300".to_string())]);
    }
}
