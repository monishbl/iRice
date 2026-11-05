//! Window controls (rust.surfaces: decorations), compare
//! shell/modules/decorations/WindowControls.qml (Settings → Decorations →
//! Window controls): a small pill at the top centre of the focused window.
//! Hover it for close, minimize and zoom (Glass: traffic lights; Material:
//! round icon buttons, minimize, maximize and close); rest on zoom
//! (maximize) for the tile grid. Hidden over fullscreen windows. With the
//! glass plugin the compositor draws the pill itself (it can't lag a moving
//! window); then this shows only the tile grid, when the plugin says the
//! pointer rests on a green button (`tessera-controls>>tile,ADDR,x,y`).
//!
//! The window's place comes from Hyprland's windows (services::clients,
//! refreshed on its events, as the QML's Desktop.revision). Close, minimize
//! and maximize are Hyprland dispatches made here (as Apps.windowActionFor
//! writes them); a tile goes through the shell's Tiling.snap
//! (`tiling.snap`), which keeps the window's size for Restore.
//! The layer surface holds the largest size (the tile grid) and the pill
//! grows inside it; the input region follows the pill, so the rest passes
//! clicks through as the QML's resizing window did.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod layout;

use tessera_core::services::clients::Windows;
use tessera_core::shell::{self, Part};
tessera_core::gl_fn!(ui);
use tessera_core::surfaces::{events, zones};
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use layout::M;
use serde_json::json;
use slint::{ComponentHandle, ModelRc, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::{Any, TypeId};
use std::rc::Rc;

const NAME: &str = "decorations";
const NS: &str = "tessera:windowcontrols";

enum Msg {
    Tile(String, f32, f32),
    Act(String, String),
    Snap(String, String),
    TilingOff,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::WindowControls,
    shown: bool,
    pos: (i32, i32),
    region: (i32, i32, i32, i32),
    glass_sent: String,
}

#[derive(Default)]
pub struct Decorations {
    wins: Vec<Win>,
    /// plugin mode: the window whose green button was rested on, and where
    tile_target: Option<(String, f32, f32)>,
    following: bool,
}

fn enabled(cfg: &serde_json::Value) -> bool {
    cfg["appearance"]["decorations"].as_str() == Some("controls")
}

fn tile_rows() -> ModelRc<ui::TileRow> {
    let rows: Vec<ui::TileRow> = layout::TILE_ROWS
        .iter()
        .map(|r| ui::TileRow {
            zones: ModelRc::new(VecModel::from(
                r.iter()
                    .map(|z| {
                        let f = zones::layout(z).unwrap_or([0.0, 0.0, 1.0, 1.0]);
                        ui::TileZone { name: (*z).into(), fx: f[0] as f32, fy: f[1] as f32, fw: f[2] as f32, fh: f[3] as f32 }
                    })
                    .collect::<Vec<_>>(),
            )),
        })
        .collect();
    ModelRc::new(VecModel::from(rows))
}

impl Decorations {
    fn make(&mut self, st: &mut State, output: &str) -> Option<usize> {
        if let Some(i) = self.wins.iter().position(|w| w.output == output) {
            return Some(i);
        }
        let wl_output = st.output_named(output)?;
        let plugin = st.shell.plugin;
        let size = layout::surface(plugin, st.shell.look.material);
        let surface = st.prepare_surface(Spec {
            layer: Layer::Top,
            namespace: NS,
            output: Some((&wl_output, output)),
            anchor: Anchor::TOP | Anchor::LEFT,
            size,
            exclusive: -1, // (GlassWindow: margins from the screen's edge, not the bars')
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::WindowControls::new().expect("WindowControls");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.set_tile_rows(tile_rows());
        ui.set_close_icon(gl("x"));
        ui.set_min_icon(gl("minus"));
        ui.set_zoom_icon(gl("arrows-out-simple"));
        ui.set_max_icon(gl("square"));
        let to = st.shell.sender(NAME);
        let (t1, t2, t3) = (to.clone(), to.clone(), to);
        let (o1, o2) = (output.to_string(), output.to_string());
        ui.on_act(move |a| t1(Box::new(Msg::Act(o1.clone(), a.to_string()))));
        ui.on_snap(move |z| t2(Box::new(Msg::Snap(o2.clone(), z.to_string()))));
        ui.on_tiling_off(move || t3(Box::new(Msg::TilingOff)));
        ui.show().expect("show window controls");
        surface.hide();
        self.wins.push(Win { output: output.to_string(), surface, ui, shown: false, pos: (i32::MIN, 0), region: (0, 0, 0, 0), glass_sent: String::new() });
        Some(self.wins.len() - 1)
    }

    /// Who the controls are for on this screen, and where: (address, surface top-left)
    fn target(&self, st: &State, output: &str) -> Option<(String, (i32, i32))> {
        let w = st.shell.get::<Windows>()?;
        let m = w.monitor(output)?;
        let plugin = st.shell.plugin;
        let size = layout::surface(plugin, st.shell.look.material);
        if plugin {
            let (addr, x, y) = self.tile_target.as_ref()?;
            let (mw, mh) = m.logical();
            if *x < m.x as f32 || *y < m.y as f32 || *x >= m.x as f32 + mw || *y >= m.y as f32 + mh {
                return None;
            }
            let c = w.clients.iter().find(|c| &c.address == addr)?;
            if c.monitor != m.id {
                return None;
            }
            return Some((addr.clone(), layout::place_at((*x, *y), (m.x, m.y), size.0 as f32)));
        }
        let c = w.active_client()?;
        if c.monitor != m.id || c.fullscreen > 0 || !c.mapped || c.hidden {
            return None;
        }
        Some((c.address.clone(), layout::place(c.at, c.size, (m.x, m.y), size.0 as f32)))
    }

    fn update(&mut self, st: &mut State) {
        if !enabled(&st.shell.cfg) {
            for w in std::mem::take(&mut self.wins) {
                st.shell.glass.set(&w.output, NS, &[]);
                w.surface.hide(); // (unmapped now: dropping the window alone leaves it on screen)
                st.drop_surface(&w.surface);
            }
            return;
        }
        let plugin = st.shell.plugin;
        for (_, name) in st.outputs_now() {
            let target = self.target(st, &name);
            let Some(i) = (if target.is_some() { self.make(st, &name) } else { self.wins.iter().position(|w| w.output == name) }) else { continue };
            let w = &mut self.wins[i];
            w.ui.set_plugin_mode(plugin);
            w.ui.set_on_glass(plugin && !st.shell.look.material);
            let visible = target.is_some() && (!plugin || w.ui.get_tiling());
            match target {
                Some((addr, pos)) if visible => {
                    w.ui.set_address(addr.into());
                    if pos != w.pos {
                        w.pos = pos;
                        w.surface.layer.set_margin(pos.1, 0, 0, pos.0);
                    }
                    if !w.shown {
                        w.shown = true;
                        let (sw, sh) = layout::surface(plugin, st.shell.look.material);
                        w.surface.show(sw, sh);
                    } else {
                        w.surface.layer.commit();
                    }
                }
                _ => {
                    if w.shown {
                        w.shown = false;
                        w.ui.set_tiling(false);
                        st.shell.glass.set(&w.output, NS, &[]);
                        w.glass_sent.clear();
                        w.surface.hide();
                    }
                }
            }
        }
    }

    fn tile(&mut self, st: &mut State, addr: String, x: f32, y: f32) {
        self.tile_target = Some((addr, x, y));
        if !st.shell.plugin {
            return;
        }
        // (the grid opens on the screen the button is on)
        let on: Option<String> = st.shell.get::<Windows>().and_then(|w| {
            w.monitors.iter().find(|m| {
                let (mw, mh) = m.logical();
                x >= m.x as f32 && y >= m.y as f32 && x < m.x as f32 + mw && y < m.y as f32 + mh
            })
        }).map(|m| m.name.clone());
        let Some(on) = on else { return };
        if let Some(i) = self.make(st, &on) {
            self.wins[i].ui.set_tiling(true);
            self.wins[i].ui.invoke_linger();
        }
        self.update(st);
    }
}

impl Part for Decorations {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        st.shell.want("clients");
        if !self.following {
            self.following = true;
            events::follow(NAME, st.tx.clone(), |line| {
                let (a, x, y) = layout::controls_event(line)?;
                Some(Box::new(Msg::Tile(a, x, y)) as Box<dyn Any + Send>)
            });
        }
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<Windows>() {
            self.update(st);
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            // (the surface's size depends on the look: made again)
            for w in std::mem::take(&mut self.wins) {
                st.shell.glass.set(&w.output, NS, &[]);
                w.surface.hide(); // (unmapped now: dropping the window alone leaves it on screen)
                st.drop_surface(&w.surface);
            }
        }
        self.update(st);
    }

    fn plugin_changed(&mut self, st: &mut State) {
        self.config_changed(st, true);
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<Msg>() else { return };
        match *m {
            Msg::Tile(a, x, y) => self.tile(st, a, x, y),
            Msg::Act(o, what) => {
                let Some(w) = self.wins.iter().find(|w| w.output == o) else { return };
                let addr = w.ui.get_address().to_string();
                if let Some(cmd) = layout::action(&what, &addr) {
                    crate::services::hypr::dispatch(cmd);
                }
            }
            Msg::Snap(o, zone) => {
                let Some(w) = self.wins.iter().find(|w| w.output == o) else { return };
                let addr = w.ui.get_address().to_string();
                if zones::layout(&zone).is_some() && !addr.is_empty() {
                    shell::ask("tiling.snap", json!([zone, addr]));
                }
            }
            Msg::TilingOff => {
                if st.shell.plugin {
                    self.update(st);
                }
            }
        }
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

    /// The input region and the glass follow the pill as it grows
    fn after_frame(&mut self, st: &mut State) {
        for w in &mut self.wins {
            if !w.shown || !w.surface.mapped.get() {
                continue;
            }
            let (x, ww, wh) = (w.ui.get_win_x(), w.ui.get_win_w(), w.ui.get_win_h());
            let region = (x.round() as i32, 0, ww.round() as i32, wh.round() as i32);
            if region != w.region {
                w.region = region;
                st.set_input_region(&w.surface, Some(&[region]));
            }
            if !st.shell.plugin || st.shell.look.material {
                continue;
            }
            let (gw, gh) = (ww - 2.0 * M, wh - 2.0 * M);
            let radius = if w.ui.get_tiling() { 18.0 } else { gh / 2.0 };
            let shape = theme::glass_shape(&st.shell.look, w.pos.0 as f32 + x + M, w.pos.1 as f32 + M, gw, gh, radius, 1.0, 1.0);
            let key = format!("{shape:?}");
            if key != w.glass_sent {
                st.shell.glass.set(&w.output, NS, std::slice::from_ref(&shape));
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
