//! Stage Manager's strip (rust.surfaces: stagemanager), both looks:
//! shell/modules/stagemanager/StageStrip.qml. On every screen while Stage
//! Manager is on (stageManager.enabled and .strip): the waiting stages of the
//! space it shows (up to six), each a stack of its last three windows'
//! pictures with the front one's app icon, on the left edge; it reserves its
//! width (176), so tiled windows never cover it. Click one to bring it
//! forward; Shift+click to add its windows to the stage in front.
//!
//! The stages are the QML StageManager service's (it parks and brings back
//! the windows); RustWindows.qml tells this its waiting stages
//! (`{"to":"stagemanager","stage":…}`) and the clicks go back there (`wm`
//! stage.show, stage.merge).
//!
//! Pictures: services::capture, once; with Settings → Overview → Window
//! previews: Live, again five times a second, but not while a window covers
//! the strip (full screen, floating) or the screen is locked.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

use tessera_core::services::apps::Apps;
use tessera_core::services::capture::Shot;
use tessera_core::services::clients::{Client, Windows};
use tessera_core::services::icons::Icons;
use tessera_core::shell::Part;
use tessera_core::wm::{self, AppIcons, IconsDone, LiveTick, OverviewCfg, Thumbs};
use tessera_core::wayland::{Spec, State, Surface};
use slint::{ComponentHandle, ModelRc, VecModel};
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

const NAME: &str = "stagemanager";
pub const NS: &str = "tessera:stagestrip";
const WIDTH: u32 = 176;

/// One waiting stage: its id and its windows' addresses (without 0x)
#[derive(Clone, Debug, PartialEq)]
pub struct Stage {
    pub id: i64,
    pub wins: Vec<String>,
}

/// The service's waiting stages per space ({"<ws>": [{id, wins}]})
pub fn parse_waiting(v: &serde_json::Value) -> BTreeMap<i32, Vec<Stage>> {
    let mut out = BTreeMap::new();
    if let Some(o) = v.as_object() {
        for (k, list) in o {
            let Ok(ws) = k.parse::<i32>() else { continue };
            let stages = list
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|s| Stage { id: s["id"].as_i64().unwrap_or(0), wins: s["wins"].as_array().map(|w| w.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default() })
                        .collect()
                })
                .unwrap_or_default();
            out.insert(ws, stages);
        }
    }
    out
}

fn key(a: &str) -> &str {
    a.trim_start_matches("0x")
}

/// A stage's last three windows that still exist, back to front
pub fn tops<'a>(s: &Stage, clients: &'a [Client]) -> Vec<&'a Client> {
    let start = s.wins.len().saturating_sub(3);
    s.wins[start..].iter().filter_map(|a| clients.iter().find(|c| key(&c.address) == key(a))).collect()
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::StageStrip,
    a11y: Option<crate::a11y::A11y>,
}

#[derive(Default)]
pub struct StageManager {
    wins: Vec<Win>,
    waiting: BTreeMap<i32, Vec<Stage>>,
    locked: bool,
    icons: AppIcons,
    thumbs: Thumbs,
}

fn send(st: &State) -> impl Fn(Box<dyn Any + Send>) + Clone + 'static {
    st.shell.sender(NAME)
}

impl StageManager {
    fn on(st: &State) -> bool {
        let s = &st.shell.cfg["stageManager"];
        s["enabled"].as_bool().unwrap_or(false) && s["strip"].as_bool().unwrap_or(true)
    }

    fn add(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        if self.wins.iter().any(|w| w.output == name) {
            return;
        }
        let surface = st.prepare_surface(Spec {
            layer: Layer::Bottom,
            namespace: NS,
            output: Some((output, name)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT,
            size: (WIDTH, 0),
            exclusive: WIDTH as i32,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::StageStrip::new().expect("StageStrip");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let (to, o) = (send(st), name.to_string());
        ui.on_activate(move |id, shift| to(Box::new((o.clone(), id, shift))));
        ui.show().expect("show stage strip");
        surface.show(WIDTH, 0);
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        if self.wins.is_empty() {
            let t = ui.global::<ui::Theme>();
            self.icons.restyle(st, t.get_accent(), t.get_m3_primary_container(), t.get_m3_on_primary_container());
        }
        self.wins.push(Win { output: name.into(), surface, ui, a11y });
    }

    /// Strips on every screen while it's on, none while it's off
    fn follow(&mut self, st: &mut State) {
        if Self::on(st) {
            for (o, n) in st.outputs_now() {
                self.add(st, &o, &n);
            }
            self.update(st);
        } else {
            for w in self.wins.drain(..) {
                st.drop_surface(&w.surface);
            }
            self.thumbs.clear();
        }
    }

    fn update(&mut self, st: &mut State) {
        if self.wins.is_empty() {
            return;
        }
        let wins = st.shell.get::<Windows>().cloned().unwrap_or_default();
        let live_on = OverviewCfg::read(&st.shell.cfg).previews == "live" && !self.locked;
        let mut live = HashMap::new();
        for i in 0..self.wins.len() {
            let out = self.wins[i].output.clone();
            let ws = wins.monitor(&out).map(|m| m.active.id).unwrap_or(-1);
            let scale = self.wins[i].ui.window().scale_factor().max(1.0);
            let h = wins.monitor(&out).map(|m| m.logical().1).unwrap_or(1080.0);
            let covered = wins.covered(&out, (0.0, 0.0, WIDTH as f32, h));
            let list: Vec<Stage> = self.waiting.get(&ws).cloned().unwrap_or_default().into_iter().take(6).collect();
            let max = ((120.0 * scale) as u32, (80.0 * scale) as u32);
            let stages: Vec<ui::StStage> = list
                .iter()
                .map(|s| {
                    let t = tops(s, &wins.clients);
                    let n = t.len();
                    let cards: Vec<ui::StCard> = t
                        .iter()
                        .enumerate()
                        .map(|(k, c)| {
                            self.thumbs.ask(st, NAME, &c.address, max);
                            if live_on && !covered {
                                live.insert(c.address.clone(), max);
                            }
                            let pic = self.thumbs.images.get(&c.address).cloned();
                            ui::StCard { has_picture: pic.is_some(), picture: pic.unwrap_or_default(), depth: (n - 1 - k) as i32 }
                        })
                        .collect();
                    let front = t.last().map(|c| c.class.clone()).unwrap_or_default();
                    let icon = if front.is_empty() { None } else { self.icons.get(st, &front, 36.0, scale, true) };
                    let label = st.shell.get::<Apps>().and_then(|a| a.entry_for(&front)).map(|e| e.name.clone()).unwrap_or(front);
                    ui::StStage { id: s.id as i32, cards: ModelRc::new(VecModel::from(cards)), has_icon: icon.is_some(), icon: icon.unwrap_or_default(), label: label.into() }
                })
                .collect();
            self.wins[i].ui.set_stages(ModelRc::new(VecModel::from(stages)));
        }
        self.thumbs.interval = 200;
        self.thumbs.set_live(st, NAME, live);
        self.icons.flush(send(st));
    }

    fn activate(&mut self, st: &State, out: &str, id: i32, shift: bool) {
        let wins = st.shell.get::<Windows>().cloned().unwrap_or_default();
        let ws = wins.monitor(out).map(|m| m.active.id).unwrap_or(-1);
        wm::ask(if shift { "stage.merge" } else { "stage.show" }, serde_json::json!([ws, id]));
    }
}

impl Part for StageManager {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        for s in ["clients", "apps", "icons"] {
            st.shell.want(s);
        }
    }

    fn output_added(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        if Self::on(st) {
            self.add(st, output, name);
            self.update(st);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if let Some(i) = self.wins.iter().position(|w| w.output == name) {
            let w = self.wins.remove(i);
            st.drop_surface(&w.surface);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if let Some(n) = self.wins.iter().find(|w| Rc::ptr_eq(&w.surface, s)).map(|w| w.output.clone()) {
            self.output_removed(st, &n);
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            for w in &self.wins {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            }
            if let Some(w) = self.wins.first() {
                let t = w.ui.global::<ui::Theme>();
                self.icons.restyle(st, t.get_accent(), t.get_m3_primary_container(), t.get_m3_on_primary_container());
            }
        }
        self.follow(st);
    }

    fn data(&mut self, st: &mut State, what: std::any::TypeId) {
        if what == std::any::TypeId::of::<Windows>() || what == std::any::TypeId::of::<Icons>() || what == std::any::TypeId::of::<Apps>() {
            self.update(st);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<serde_json::Value>() {
            Ok(v) => {
                if v["stage"].is_object() {
                    self.waiting = parse_waiting(&v["stage"]["waiting"]);
                }
                if let Some(l) = v["locked"].as_bool() {
                    self.locked = l;
                }
                self.update(st);
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<Shot>() {
            Ok(s) => {
                if self.thumbs.got(*s) {
                    self.update(st);
                }
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<IconsDone>() {
            Ok(d) => {
                self.icons.done(*d);
                self.update(st);
                return;
            }
            Err(m) => m,
        };
        let msg = match msg.downcast::<LiveTick>() {
            Ok(_) => {
                self.thumbs.tick(st, NAME);
                return;
            }
            Err(m) => m,
        };
        if let Ok(a) = msg.downcast::<(String, i32, bool)>() {
            let (o, id, shift) = *a;
            self.activate(st, &o, id, shift);
        }
    }

    fn after_frame(&mut self, st: &mut State) {
        let _ = st;
        for w in &mut self.wins {
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, false);
            }
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.wins.iter_mut().filter_map(|w| w.a11y.as_mut()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_services_stages() {
        let w = parse_waiting(&json!({"1": [{"id": 3, "wins": ["5f1", "5f2"]}], "x": [], "2": []}));
        assert_eq!(w.get(&1).unwrap()[0], Stage { id: 3, wins: vec!["5f1".into(), "5f2".into()] });
        assert!(w.get(&2).unwrap().is_empty());
        assert_eq!(w.len(), 2);
    }

    #[test]
    fn the_last_three_windows_that_exist() {
        let c = |a: &str| Client { address: a.into(), ..Default::default() };
        let clients = vec![c("0xa"), c("0xb"), c("0xc"), c("0xd")];
        let s = Stage { id: 1, wins: vec!["a".into(), "b".into(), "gone".into(), "c".into(), "d".into()] };
        assert_eq!(tops(&s, &clients).iter().map(|c| c.address.as_str()).collect::<Vec<_>>(), vec!["0xc", "0xd"]);
    }
}
