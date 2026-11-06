//! The rounded bottom screen corners (rust.surfaces: screencorners), compare
//! shell/modules/background/ScreenCorners.qml: when
//! `appearance.screenCorners` is on (the default), a strip `screenCornerRadius`
//! (12) high along the bottom of every screen, on the overlay layer, with a
//! black concave corner at each end. The top corners are the menu bar's. The
//! strip takes no input.

use super::*;
use serde_json::Value;
use slint::ComponentHandle;
use tessera_core::shell::Part;

const NAME: &str = "screencorners";
const NS: &str = "tessera:corners";

struct Win {
    output: String,
    surface: Rc<Surface>,
    #[allow(dead_code)] // (kept: dropping the component would drop the window)
    ui: ui::Corners,
}

#[derive(Default)]
pub struct ScreenCorners {
    wins: Vec<Win>,
    made_for: (bool, i32),
}

/// (on, radius) from the settings
fn setting(cfg: &Value) -> (bool, i32) {
    let a = &cfg["appearance"];
    (a["screenCorners"].as_bool().unwrap_or(true), a["screenCornerRadius"].as_i64().unwrap_or(12).clamp(1, 96) as i32)
}

impl ScreenCorners {
    fn make(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        let (on, r) = setting(&st.shell.cfg);
        if !on || !super::on() {
            return;
        }
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((output, name)),
            anchor: Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, r as u32),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::Corners::new().expect("Corners");
        ui.set_r(r as f32);
        ui.show().expect("show corners");
        st.set_input_region(&surface, Some(&[]));
        self.wins.push(Win { output: name.to_string(), surface, ui });
    }

    fn drop_all(&mut self, st: &mut State) {
        for w in std::mem::take(&mut self.wins) {
            retire(st, &w.surface);
        }
    }

    fn rebuild(&mut self, st: &mut State) {
        self.drop_all(st);
        for (o, n) in st.outputs_now() {
            self.make(st, &o, &n);
        }
    }
}

impl Part for ScreenCorners {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        self.made_for = setting(&st.shell.cfg);
    }

    fn output_added(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        if !self.wins.iter().any(|w| w.output == name) {
            self.make(st, output, name);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.wins.iter().position(|w| w.output == name) {
            let w = self.wins.remove(i);
            retire(st, &w.surface);
        }
    }

    fn config_changed(&mut self, st: &mut State, _: bool) {
        let now = setting(&st.shell.cfg);
        if now != self.made_for {
            self.made_for = now;
            self.rebuild(st);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.wins.iter().any(|w| Rc::ptr_eq(&w.surface, s)) {
            let name = s.output.clone();
            self.output_removed(st, &name);
        }
    }

    fn quit(&mut self, st: &mut State) {
        self.drop_all(st);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn on_by_default_at_twelve() {
        assert_eq!(setting(&json!({})), (true, 12));
        assert_eq!(setting(&json!({"appearance": {"screenCorners": false, "screenCornerRadius": 20}})), (false, 20));
    }
}
