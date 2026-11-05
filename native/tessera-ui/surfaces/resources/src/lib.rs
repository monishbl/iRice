//! The resources popover (rust.surfaces: resources), compare
//! shell/modules/resources/ResourcePopover.qml and ResourceRow.qml: a small
//! glass popover under a hovered figure (the island's CPU, memory and
//! temperature, the menu bar's resources item) with every stat grouped on
//! rows (processor, graphics, memory, network), a small graph of the last
//! minute each; the group pointed at is highlighted. It takes no input, so
//! the pointer stays on what it's hovering.
//!
//! When to show it is still the hovered item's (ResourceHover.qml: half a
//! second's rest, gone 120 ms after leaving): it tells this part
//! `{"to":"resources","open":true,"key":"cpu","x":…,"y":…,"screen":"DP-1"}`
//! (x the centre it hangs under, y its top edge, screen coordinates) and
//! `{"open":false,…}`. It samples once a second while shown (stats.rs).
//! Where it differs: no NVIDIA reading (nvidia-smi) yet: Graphics shows
//! for amdgpu and Intel only.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod stats;

use tessera_core::shell::Part;
use tessera_core::sysstat::{self, Sampler};
use tessera_core::surfaces::kit::hex;
tessera_core::gl_fn!(ui);
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use slint::{ComponentHandle, ModelRc, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::rc::Rc;

const NAME: &str = "resources";
const NS: &str = "tessera:resources-hover";
const M: f32 = 24.0; // shadow margin
const PAD: f32 = 14.0;
const CONTENT_W: f32 = 300.0;
const GW: f32 = 76.0; // a row's graph
const GH: f32 = 26.0;

/// The popover's size for so many rows (48 high, 2 apart)
pub fn size(rows: usize) -> (u32, u32) {
    let list = rows as f32 * 48.0 + rows.saturating_sub(1) as f32 * 2.0;
    ((CONTENT_W + 2.0 * PAD + 2.0 * M) as u32, (list + 2.0 * PAD + 2.0 * M) as u32)
}

/// Its top-left on the screen: centred under the point, kept on screen
pub fn place(x: f32, y: f32, w: f32, screen_w: f32) -> (i32, i32) {
    ((0f32.max((screen_w - w).min(x - w / 2.0))).round() as i32, (0f32.max(y - M)).round() as i32)
}

/// The menu bar shows the figures (Settings → Menu bar → Resources), so
/// their popover may open any time
pub fn bar_figures(cfg: &serde_json::Value) -> bool {
    cfg["menubar"]["resources"].as_bool().unwrap_or(false)
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::ResourcePopover,
    pos: (i32, i32),
    glass_sent: String,
}

struct Tick;

#[derive(Default)]
pub struct Resources {
    win: Option<Win>,
    key: String,
    sampler: Sampler,
    gpu: stats::Gpu,
    timer: Option<slint::Timer>,
    shown: bool,
    started: Option<std::time::Instant>,
}

impl Resources {
    fn now(&mut self) -> f64 {
        self.started.get_or_insert_with(std::time::Instant::now).elapsed().as_secs_f64() + 100.0
    }

    fn sample(&mut self) {
        let now = self.now();
        self.sampler.sample(now);
        self.gpu.sample(now);
    }

    fn rows(&self, st: &State) -> Vec<ui::ResRow> {
        let style = st.shell.cfg["resources"]["graphStyle"].as_str().unwrap_or("area").to_string();
        let dark = st.shell.look.dark;
        let mem = stats::mem_detail(&std::fs::read_to_string("/proc/meminfo").unwrap_or_default());
        let gpu = self.gpu.stat();
        let group = stats::group_of(&self.key);
        stats::rows(&self.sampler.stats(), &mem, gpu.as_ref())
            .into_iter()
            .map(|r| {
                let net = r.group == "network";
                let top = if net { stats::net_top(&r.main, &r.second) } else { r.max };
                let (area, line) = sysstat::graph(&r.main, top, GW, GH, &style, 1.25);
                let (_, line2) = if net { sysstat::graph(&r.second, top, GW, GH, "line", 1.25) } else { (String::new(), String::new()) };
                ui::ResRow {
                    title: r.title.into(),
                    icon: gl(r.icon),
                    value: r.value.into(),
                    value2: r.value2.into(),
                    detail: r.detail.into(),
                    highlighted: r.group == group,
                    network: net,
                    tone: hex(&theme::accent_hex(r.tone, dark)),
                    tone2: hex(&theme::accent_hex(r.tone2, dark)),
                    role: stats::role(r.tone),
                    role2: stats::role(r.tone2),
                    area: area.into(),
                    line: line.into(),
                    line2: line2.into(),
                }
            })
            .collect()
    }

    fn refresh(&mut self, st: &State) {
        let rows = self.rows(st);
        if let Some(w) = &self.win {
            w.ui.set_rows(ModelRc::new(VecModel::from(rows)));
        }
    }

    fn open(&mut self, st: &mut State, key: String, x: f32, y: f32, screen: String) {
        self.key = key;
        let Some(wl_output) = st.output_named(&screen).or_else(|| st.outputs_now().first().map(|o| o.0.clone())) else { return };
        let screen = st.outputs_now().into_iter().find(|(o, _)| *o == wl_output).map(|o| o.1).unwrap_or(screen);
        if self.win.as_ref().is_some_and(|w| w.output != screen) {
            self.close(st);
        }
        self.shown = true;
        self.sampling(st);
        let n = stats::groups(self.gpu.stat().is_some()).len();
        let (w, h) = size(n);
        let (sw, _) = crate::surfaces::kit::screen_size(st, &wl_output);
        let pos = place(x, y, w as f32, sw);
        if self.win.is_none() {
            let surface = st.prepare_surface(Spec {
                layer: Layer::Overlay,
                namespace: NS,
                output: Some((&wl_output, &screen)),
                anchor: Anchor::TOP | Anchor::LEFT,
                size: (w, h),
                exclusive: -1, // (GlassWindow: margins from the screen's edge, not the bars')
                keyboard: KeyboardInteractivity::None,
                margin_left: pos.0,
            });
            surface.layer.set_margin(pos.1, 0, 0, pos.0);
            surface.layer.commit();
            st.set_input_region(&surface, Some(&[]));
            let ui = ui::ResourcePopover::new().expect("ResourcePopover");
            st.shell.apply_theme(&ui.global::<ui::Theme>());
            ui.set_on_glass(st.shell.plugin && !st.shell.look.material);
            ui.show().expect("show resources popover");
            self.win = Some(Win { output: screen, surface, ui, pos, glass_sent: String::new() });
        } else if let Some(win) = self.win.as_mut() {
            if win.pos != pos {
                win.pos = pos;
                win.surface.layer.set_margin(pos.1, 0, 0, pos.0);
            }
            win.surface.layer.set_size(w, h);
            win.surface.layer.commit();
        }
        self.refresh(st);
        if let Some(w) = &self.win {
            w.ui.set_shown(true);
        }
    }

    /// Sampling once a second while shown, or while the menu bar shows its
    /// figures (Resources.qml's menubarActive: the graphs then already hold
    /// the last minute when the popover opens)
    fn sampling(&mut self, st: &State) {
        let want = self.shown || bar_figures(&st.shell.cfg);
        if want && self.timer.is_none() {
            self.sample();
            let tx = st.tx.clone();
            let t = slint::Timer::default();
            t.start(slint::TimerMode::Repeated, std::time::Duration::from_secs(1), move || {
                let _ = tx.send(tessera_core::Msg::Part(NAME, Box::new(Tick)));
            });
            self.timer = Some(t);
        } else if !want {
            self.timer = None;
        }
    }

    fn close(&mut self, st: &mut State) {
        self.shown = false;
        self.sampling(st);
        if let Some(w) = self.win.take() {
            st.shell.glass.set(&w.output, NS, &[]);
            w.surface.hide(); // (unmapped now: dropping the window alone leaves it on screen)
            st.drop_surface(&w.surface);
        }
    }
}

impl Part for Resources {
    fn name(&self) -> &'static str {
        NAME
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<serde_json::Value>() {
            Ok(v) => {
                let key = v["key"].as_str().unwrap_or("cpu").to_string();
                let screen = v["screen"].as_str().unwrap_or("").to_string();
                if v["open"].as_bool() == Some(true) {
                    let (x, y) = (v["x"].as_f64().unwrap_or(0.0) as f32, v["y"].as_f64().unwrap_or(0.0) as f32);
                    self.open(st, key, x, y, screen);
                } else if v["open"].as_bool() == Some(false) {
                    // (another figure may have opened it since: only its own close counts)
                    let same = self.key == key && self.win.as_ref().is_none_or(|w| screen.is_empty() || w.output == screen);
                    if same {
                        self.close(st);
                    }
                }
                return;
            }
            Err(m) => m,
        };
        if msg.downcast::<Tick>().is_ok() {
            self.sample();
            if self.win.is_some() {
                self.refresh(st);
            }
        }
    }

    fn start(&mut self, st: &mut State) {
        self.sampling(st);
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        self.sampling(st);
        if let Some(w) = self.win.as_mut() {
            if look {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
                w.ui.set_on_glass(st.shell.plugin && !st.shell.look.material);
                w.glass_sent.clear();
            }
        }
        if self.win.is_some() {
            self.refresh(st);
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
        if !st.shell.plugin || st.shell.look.material || !w.surface.mapped.get() {
            return;
        }
        let p = w.ui.get_progress();
        let (sw, sh) = w.surface.logical_size();
        let shape = theme::glass_shape(&st.shell.look, w.pos.0 as f32 + M, w.pos.1 as f32 + M + (p - 1.0) * 6.0, sw as f32 - 2.0 * M, sh as f32 - 2.0 * M, 18.0, p, 0.35);
        let key = format!("{shape:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, if p > 0.002 { std::slice::from_ref(&shape) } else { &[] });
            w.glass_sent = key;
        }
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
    fn sized_and_placed_as_the_qml() {
        assert_eq!(size(3), (376, 3 * 48 + 2 * 2 + 28 + 48));
        assert_eq!(size(4).1, 4 * 48 + 3 * 2 + 76);
        assert!(!bar_figures(&serde_json::json!({})));
        assert!(bar_figures(&serde_json::json!({"menubar": {"resources": true}})));
        // centred under the point, kept on the screen
        assert_eq!(place(960.0, 40.0, 376.0, 1920.0), (772, 16));
        assert_eq!(place(10.0, 40.0, 376.0, 1920.0), (0, 16));
        assert_eq!(place(1915.0, 10.0, 376.0, 1920.0), (1544, 0));
    }
}
