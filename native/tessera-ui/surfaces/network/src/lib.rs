//! The menu bar's Wi-Fi menu (rust.surfaces: network): Wi-Fi's switch,
//! refresh and networks, then the speed test, in a popover under the bar.
//! Compare shell/modules/menubar/StatusMenus.qml ("wifiMenu": WifiPage, a
//! line, `SpeedTestCard { compact: true }` in a glass Popover with lens 0.35
//! and autoClose 5000), shell/glass/Popover.qml, shell/modules/controlcenter/
//! {DetailPage,WifiPage,WifiList,ListRow,ServiceNotice}.qml and
//! shell/modules/network/SpeedTestCard.qml (the card is ui/network.slint's
//! SpeedTestCard, exported for other Rust surfaces).
//!
//! It opens as the QML one did, through Panels ("wifiMenu", RustUi.qml maps
//! it to this surface): the menu bar's Wi-Fi item, IPC. What it shows is the
//! shell's: shell/modules/network/NetworkFeed.qml sends `{"to":"network", …}`
//! lines while it draws this (`wifi`: adapter, enabled, unmanaged and its
//! notice, the networks as WifiList's snapshot with their status strings;
//! `speed`: SpeedTest's state with the strings it formats; `anchorX`:
//! Panels.anchorX) and does what it asks (`shell::ask("network", [what, …])`):
//!   hold <hovered or its menu open> <a password is being typed>
//!                       (the list holds still meanwhile, as WifiList's)
//!   speedStart, speedCancel, wifi <on>, rescan, connect <name>,
//!   disconnect <name>, join <name> <password>, forget <name>, copy <name>,
//!   settings (Settings → Network), fix (the unmanaged notice's button)
//! The password goes only in `join`'s line, never in a log.
//!
//! Differences from the QML:
//! - Enterprise and hidden networks, "Other options…", "Join other
//!   network…" and "Share network…" (the QR card) open Settings → Network,
//!   as the Rust Control Center does, instead of WifiJoinForm and
//!   WifiShareCard in the menu.
//! - The right-click menu is drawn inside the popover's layer (solid, no
//!   glass of its own); no press-and-hold to open it, no Menu key.
//! - The keyboard: Tab, the arrows, Return and Space move through the
//!   networks, Join other network…, the card's button and its link, with a
//!   ring; the QML's focus chain also reaches the switch and refresh.
//! - Slint draws no underline (the link and Other options… only change
//!   colour on hover), no tabular figures, and Material's wave doesn't ease
//!   flat (it switches); Glass's panel doesn't give when dragged
//!   (ElasticGrab); Material buttons swell on press without the bounce.
//! - Wrapped notes (the failure under a row, the notice, the card's error)
//!   get an estimated height (Slint measures wrapped text in a layout as
//!   one line).

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod model;

use tessera_core::services::hypr::Glass;
use tessera_core::shell::{self, Command, Part};
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use model::{Act, Click, Entry, Speed, Wifi};
use serde_json::{json, Value};
use slint::platform::Key;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::rc::Rc;
use std::time::Duration;

const NAME: &str = "network";
pub const NS: &str = "tessera:wifiMenu";
/// The card's width (Popover's contentWidth: 300)
const CARD_W: f32 = 300.0;
/// Popover.qml's autoClose for the status menus
const AUTO_CLOSE_MS: u64 = 5000;

#[allow(dead_code)] // (some rows' indexes are carried but every row does the same there)
enum M {
    Key(String, bool),
    Switch,
    Refresh,
    RowClick(i32),
    RowMenu(i32, f32, f32),
    RowAction(i32),
    Commit(i32, String),
    Cancel(i32),
    Other(i32),
    JoinOther,
    Notice,
    SpeedAct,
    Details,
    MenuPick(i32),
    MenuClose,
    Hovered(bool),
    AutoClose,
    Dismissed,
    Closed,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::WifiMenu,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
    region: (u32, u32),
}

#[derive(Default)]
pub struct Network {
    win: Option<Win>,
    open: bool,
    wifi: Wifi,
    speed: Speed,
    anchor_x: f32,
    /// the network waiting for a password, and why Join didn't go ahead
    asking: Option<String>,
    hint: String,
    hovered: bool,
    /// the row whose menu is open, and that menu
    menu: Option<(usize, Vec<Entry>)>,
    /// where the keyboard is (-1: nowhere yet)
    kbd: i32,
    /// what the shell was last told about holding the list still
    hold_sent: Option<(bool, bool)>,
    rows: Rc<VecModel<ui::NetRow>>,
    wave: String,
    timer: slint::Timer,
    /// to itself, from the timer
    sender: Option<Rc<dyn Fn(Box<dyn Any + Send>)>>,
}

impl Network {
    fn window(&mut self, st: &mut State, output: &str) -> bool {
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
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
        let ui = ui::WifiMenu::new().expect("WifiMenu");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.global::<ui::Symbols>().on_of(|g| crate::icons::symbol(&g).into());
        let to = st.shell.sender(NAME);
        {
            let to = to.clone();
            ui.on_key(move |text, shift| {
                let ok = model::handles(&text);
                if ok {
                    to(Box::new(M::Key(text.to_string(), shift)));
                }
                ok
            });
        }
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                ui.$set(move |$($a),*| to(Box::new($m)));
            }};
        }
        on!(on_switch_toggled, | | M::Switch);
        on!(on_refresh, | | M::Refresh);
        on!(on_row_click, |i| M::RowClick(i));
        on!(on_row_menu, |i, x, y| M::RowMenu(i, x, y));
        on!(on_row_action, |i| M::RowAction(i));
        on!(on_commit, |i, t| M::Commit(i, t.to_string()));
        on!(on_cancel_ask, |i| M::Cancel(i));
        on!(on_other_options, |i| M::Other(i));
        on!(on_join_other, | | M::JoinOther);
        on!(on_notice_act, | | M::Notice);
        on!(on_speed_act, | | M::SpeedAct);
        on!(on_details, | | M::Details);
        on!(on_menu_pick, |j| M::MenuPick(j));
        on!(on_menu_close, | | M::MenuClose);
        on!(on_hovered, |h| M::Hovered(h));
        on!(on_dismissed, | | M::Dismissed);
        on!(on_closed, | | M::Closed);
        ui.set_g(glyphs());
        ui.set_rows(ModelRc::from(self.rows.clone()));
        ui.show().expect("show wifi menu");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: output.into(), surface, ui, a11y, glass_sent: String::new(), region: (0, 0) });
        true
    }

    fn open(&mut self, st: &mut State, screen: Option<String>) {
        let output = screen.or_else(|| self.win.as_ref().map(|w| w.output.clone())).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(output) = output else { return };
        let again = self.open && self.win.as_ref().is_some_and(|w| w.output == output);
        if !self.window(st, &output) {
            return;
        }
        if again {
            return;
        }
        self.open = true;
        self.asking = None;
        self.hint.clear();
        self.menu = None;
        self.kbd = -1;
        self.hovered = false;
        if self.wave.is_empty() {
            self.wave = model::wave(CARD_W);
        }
        self.push_wifi();
        self.push_speed(st);
        let w = self.win.as_mut().unwrap();
        w.region = (0, 0);
        w.ui.set_anchor_x(self.anchor_x);
        w.ui.set_menu_open(false);
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        w.surface.show(0, 0);
        w.ui.invoke_focus_keys();
        w.ui.set_shown(true);
        self.hold();
    }

    fn close(&mut self, st: &mut State, tell: bool) {
        if !self.open {
            return;
        }
        self.open = false;
        self.asking = None;
        self.menu = None;
        self.timer.stop();
        if let Some(w) = &self.win {
            w.ui.set_shown(false);
            w.ui.set_menu_open(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            w.surface.layer.commit();
            // (input passes through while it fades, as the QML's mask)
            st.set_input_region(&w.surface, Some(&[]));
        }
        self.hold();
        if tell {
            shell::closed(NAME);
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
            w.region = (0, 0);
        }
    }

    /// The list holds still while the pointer is over the menu, its menu is
    /// open or a password is typed (the shell's snapshot); the menu closes
    /// itself 5 s after the pointer left, unless one of those holds it
    fn hold(&mut self) {
        let now = (self.open && (self.hovered || self.menu.is_some()), self.open && self.asking.is_some());
        if self.hold_sent != Some(now) {
            self.hold_sent = Some(now);
            shell::ask("network", json!(["hold", now.0, now.1]));
        }
        if self.open && !now.0 && !now.1 {
            if let (false, Some(tx)) = (self.timer.running(), self.sender.clone()) {
                self.timer.start(slint::TimerMode::SingleShot, Duration::from_millis(AUTO_CLOSE_MS), move || tx(Box::new(M::AutoClose)));
            }
        } else {
            self.timer.stop();
        }
    }

    /// How many things the keyboard goes through: the rows, Join other
    /// network… (when shown), the card's button and its link
    fn focusables(&self) -> i32 {
        let rows = if self.wifi.unmanaged || !self.wifi.enabled { 0 } else { self.wifi.networks.len() as i32 };
        rows + self.show_join() as i32 + 2
    }

    fn show_join(&self) -> bool {
        self.wifi.enabled && !self.wifi.unmanaged
    }

    fn push_wifi(&mut self) {
        let Some(w) = &self.win else { return };
        let ui = &w.ui;
        let wifi = &self.wifi;
        ui.set_wifi_on(wifi.enabled);
        ui.set_unmanaged(wifi.unmanaged);
        ui.set_notice_title(wifi.notice.title.as_str().into());
        ui.set_notice_text(wifi.notice.text.as_str().into());
        ui.set_notice_h(model::wrap_height(&wifi.notice.text, CARD_W, 12.0));
        ui.set_notice_action(wifi.notice.action.as_str().into());
        ui.set_notice_busy(wifi.notice.busy);
        ui.set_empty_text(model::empty_text(wifi).into());
        let nets: &[model::Net] = if wifi.enabled && !wifi.unmanaged { &wifi.networks } else { &[] };
        let groups = model::groups(nets, self.asking.as_deref());
        let rows: Vec<ui::NetRow> = nets
            .iter()
            .zip(groups)
            .enumerate()
            .map(|(i, (n, (first, last)))| {
                let asking = self.asking.as_deref() == Some(n.name.as_str());
                ui::NetRow {
                    name: n.name.as_str().into(),
                    status: n.status.as_str().into(),
                    icon: crate::icons::glyph(model::strength_icon(n.signal)).into(),
                    hl: n.connected,
                    lock: n.secured,
                    busy: n.busy,
                    share: n.connected && !n.enterprise,
                    asking,
                    hint: if asking { self.hint.as_str().into() } else { SharedString::new() },
                    failed: n.failed.as_str().into(),
                    failed_h: model::wrap_height(&n.failed, CARD_W - 52.0, 11.0),
                    first,
                    last,
                    focused: self.kbd == i as i32,
                }
            })
            .collect();
        if rows.len() == self.rows.row_count() {
            for (i, r) in rows.into_iter().enumerate() {
                if self.rows.row_data(i).as_ref() != Some(&r) {
                    self.rows.set_row_data(i, r);
                }
            }
        } else {
            self.rows.set_vec(rows);
        }
        let n = nets.len() as i32;
        ui.set_show_join(self.show_join());
        ui.set_join_row(ui::NetRow {
            name: "Join other network…".into(),
            icon: crate::icons::glyph("plus").into(),
            first: true,
            last: true,
            focused: self.show_join() && self.kbd == n,
            ..Default::default()
        });
        let after = n + self.show_join() as i32;
        ui.set_card_focused(self.kbd == after);
        ui.set_link_focused(self.kbd == after + 1);
    }

    fn push_speed(&self, st: &State) {
        let Some(w) = &self.win else { return };
        let sp = &self.speed;
        let stats: Vec<ui::SpeedStat> = sp
            .stats
            .iter()
            .map(|s| ui::SpeedStat { label: s.label.as_str().into(), icon: crate::icons::glyph(&s.icon).into(), value: s.value.as_str().into(), active: s.active, none: s.value == "—" })
            .collect();
        w.ui.set_speed(ui::SpeedCard {
            running: sp.running,
            stage: sp.stage.as_str().into(),
            sub: model::subtitle(sp, crate::util::now_ms()).into(),
            live: sp.live.as_str().into(),
            progress: sp.progress,
            show_stats: sp.show_stats,
            stats: ModelRc::new(VecModel::from(stats)),
            resp: sp.resp.as_str().into(),
            resp_h: model::wrap_height(&sp.resp, CARD_W, 12.0),
            verdict: sp.verdict.as_str().into(),
            verdict_h: model::wrap_height(&sp.verdict, CARD_W, 12.0),
            server: sp.server.as_str().into(),
            error: sp.error.as_str().into(),
            error_h: model::wrap_height(&sp.error, CARD_W, 12.0),
            action: sp.action.as_str().into(),
            wave: if st.shell.look.material { self.wave.as_str().into() } else { SharedString::new() },
            flat: model::flat(sp.progress),
        });
    }

    fn name_at(&self, i: i32) -> Option<model::Net> {
        if i < 0 || !self.wifi.enabled || self.wifi.unmanaged {
            return None;
        }
        self.wifi.networks.get(i as usize).cloned()
    }

    fn row_click(&mut self, i: i32) {
        let Some(n) = self.name_at(i) else { return };
        match model::click(&n) {
            Click::Disconnect => shell::ask("network", json!(["disconnect", n.name])),
            Click::Connect => shell::ask("network", json!(["connect", n.name])),
            Click::Settings => shell::ask("network", json!(["settings"])),
            Click::Ask => {
                self.asking = if self.asking.as_deref() == Some(n.name.as_str()) { None } else { Some(n.name.clone()) };
                self.hint.clear();
                if self.asking.is_none() {
                    self.focus_keys();
                }
                self.push_wifi();
                self.hold();
            }
        }
    }

    fn focus_keys(&self) {
        if let Some(w) = &self.win {
            w.ui.invoke_focus_keys();
        }
    }

    fn open_menu(&mut self, i: i32, x: f32, y: f32) {
        let Some(n) = self.name_at(i) else { return };
        let entries = model::menu(&n);
        let Some(w) = &self.win else { return };
        let items: Vec<ui::NetMenuItem> = entries
            .iter()
            .map(|e| match e {
                Some((t, _, danger)) => ui::NetMenuItem { text: (*t).into(), sep: false, danger: *danger },
                None => ui::NetMenuItem { text: SharedString::new(), sep: true, danger: false },
            })
            .collect();
        w.ui.set_menu_items(ModelRc::new(VecModel::from(items)));
        let (sw, sh) = w.surface.logical_size();
        let (mw, mh) = (w.ui.get_menu_w(), w.ui.get_menu_h());
        let mx = x.min(sw as f32 - mw - 8.0).max(8.0);
        let my = y.min(sh as f32 - mh - 8.0).max(34.0);
        w.ui.set_menu_x(mx);
        w.ui.set_menu_y(my);
        w.ui.set_menu_open(true);
        self.menu = Some((i as usize, entries));
        self.hold();
    }

    fn close_menu(&mut self) {
        if self.menu.take().is_some() {
            if let Some(w) = &self.win {
                w.ui.set_menu_open(false);
            }
            self.hold();
        }
    }

    fn menu_pick(&mut self, j: i32) {
        let Some((i, entries)) = self.menu.clone() else { return };
        let Some(Some((_, act, _))) = entries.get(j.max(0) as usize).cloned() else { return };
        self.close_menu();
        let Some(n) = self.name_at(i as i32) else { return };
        match act {
            Act::Disconnect => shell::ask("network", json!(["disconnect", n.name])),
            Act::Connect => shell::ask("network", json!(["connect", n.name])),
            Act::Password => {
                self.asking = Some(n.name.clone());
                self.hint.clear();
                self.push_wifi();
                self.hold();
            }
            Act::SignIn | Act::Share | Act::Details => shell::ask("network", json!(["settings"])),
            Act::Copy => shell::ask("network", json!(["copy", n.name])),
            Act::Forget => shell::ask("network", json!(["forget", n.name])),
        }
    }

    fn commit(&mut self, i: i32, text: String) {
        let Some(n) = self.name_at(i) else { return };
        if let Some(h) = model::password_hint(&text) {
            self.hint = h.into();
            self.push_wifi();
            return;
        }
        shell::ask("network", json!(["join", n.name, text]));
        self.asking = None;
        self.hint.clear();
        self.push_wifi();
        self.focus_keys();
        self.hold();
    }

    fn speed_act(&self) {
        shell::ask("network", json!([if self.speed.running { "speedCancel" } else { "speedStart" }]));
    }

    fn key(&mut self, st: &mut State, text: &str, shift: bool) {
        let is = |k: Key| text == SharedString::from(k).as_str();
        if is(Key::Escape) {
            if self.menu.is_some() {
                self.close_menu();
            } else if self.asking.is_some() {
                self.asking = None;
                self.push_wifi();
                self.hold();
            } else {
                self.close(st, true);
            }
            return;
        }
        let n = self.focusables();
        if is(Key::Tab) || is(Key::DownArrow) {
            self.kbd = model::step(self.kbd, n, if is(Key::Tab) && shift { -1 } else { 1 });
        } else if is(Key::Backtab) || is(Key::UpArrow) {
            self.kbd = model::step(self.kbd, n, -1);
        } else if is(Key::Return) || text == " " {
            let rows = if self.wifi.enabled && !self.wifi.unmanaged { self.wifi.networks.len() as i32 } else { 0 };
            let join = self.show_join() as i32;
            let k = self.kbd;
            if k < 0 {
                return;
            } else if k < rows {
                self.row_click(k);
            } else if k < rows + join {
                shell::ask("network", json!(["settings"]));
            } else if k == rows + join {
                self.speed_act();
            } else {
                shell::ask("network", json!(["settings"]));
            }
            return;
        } else {
            return;
        }
        self.push_wifi();
    }
}

/// The glyphs the menu draws (Phosphor; SymIcon draws Material Symbols for them)
fn glyphs() -> ui::NetGlyphs {
    let g = |n: &str| SharedString::from(crate::icons::glyph(n));
    ui::NetGlyphs {
        gauge: g("gauge"),
        refresh: g("arrow-clockwise"),
        spinner: g("circle-notch"),
        lock: g("lock-simple"),
        qr: g("qr-code"),
        plus: g("plus"),
        check: g("check"),
        wifi_slash: g("wifi-slash"),
    }
}

impl Part for Network {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        let to: Rc<dyn Fn(Box<dyn Any + Send>)> = Rc::new(st.shell.sender(NAME));
        self.sender = Some(to);
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.open(st, cmd.screen.clone()),
            "close" => self.close(st, false),
            "toggle" => {
                if self.open {
                    self.close(st, true)
                } else {
                    self.open(st, cmd.screen.clone())
                }
            }
            _ => {}
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            if let Some(w) = &mut self.win {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
                w.glass_sent.clear();
            }
            self.push_speed(st);
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = &mut self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.glass_sent.clear();
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        // The shell's feed (NetworkFeed.qml)
        let msg = match msg.downcast::<Value>() {
            Ok(v) => {
                if v.get("wifi").is_some() {
                    let before = self.wifi.networks.len();
                    self.wifi = model::wifi_from(&v["wifi"]);
                    if crate::env::is_set("DEBUG") && before != self.wifi.networks.len() {
                        // (how many, never their names)
                        eprintln!("debug: network: {} networks listed", self.wifi.networks.len());
                    }
                    let n = self.focusables();
                    if self.kbd >= n {
                        self.kbd = n - 1;
                    }
                    self.push_wifi();
                }
                if v.get("speed").is_some() {
                    self.speed = model::speed_from(&v["speed"]);
                    self.push_speed(st);
                }
                if let Some(x) = v["anchorX"].as_f64() {
                    self.anchor_x = x as f32;
                    if let Some(w) = &self.win {
                        w.ui.set_anchor_x(self.anchor_x);
                    }
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Key(t, shift) => self.key(st, &t, shift),
            M::Switch => shell::ask("network", json!(["wifi", !self.wifi.enabled])),
            M::Refresh => shell::ask("network", json!(["rescan"])),
            M::RowClick(i) => self.row_click(i),
            M::RowMenu(i, x, y) => self.open_menu(i, x, y),
            M::RowAction(_) | M::JoinOther | M::Details => shell::ask("network", json!(["settings"])),
            M::Commit(i, t) => self.commit(i, t),
            M::Cancel(_) => {
                self.asking = None;
                self.hint.clear();
                self.push_wifi();
                self.focus_keys();
                self.hold();
            }
            M::Other(_) => {
                self.asking = None;
                self.push_wifi();
                self.hold();
                shell::ask("network", json!(["settings"]));
            }
            M::Notice => shell::ask("network", json!(["fix"])),
            M::SpeedAct => self.speed_act(),
            M::MenuPick(j) => self.menu_pick(j),
            M::MenuClose => self.close_menu(),
            M::Hovered(h) => {
                self.hovered = h;
                self.hold();
            }
            M::AutoClose => {
                if self.open && !self.hovered && self.menu.is_none() && self.asking.is_none() {
                    self.close(st, true);
                }
            }
            M::Dismissed => self.close(st, true),
            M::Closed => self.closed(st),
        }
    }

    fn keyboard_left(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.open && self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.close(st, true);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let was = self.open;
            self.close(st, was);
            let w = self.win.take().unwrap();
            Glass::clear_now(&w.output, NS);
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

    /// The menu bar strip stays the bar's; the popover's glass (Glass), and
    /// its row menu's; the accessibility tree
    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let open = self.open;
        let menu = self.menu.is_some();
        let Some(w) = &mut self.win else { return };
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
        if !w.surface.mapped.get() {
            return;
        }
        let size = w.surface.logical_size();
        if open && size != w.region && size.0 > 0 {
            st.set_input_region(&w.surface, Some(&[(0, 30, size.0 as i32, size.1 as i32 - 30)]));
            w.region = size;
        }
        if !st.shell.plugin || st.shell.look.material {
            return;
        }
        let ui = &w.ui;
        let o = ui.get_panel_opacity();
        let mut shapes = Vec::new();
        if o > 0.002 {
            shapes.push(theme::glass_shape(&st.shell.look, ui.get_panel_x(), ui.get_panel_y(), ui.get_panel_w(), ui.get_panel_h(), ui.get_panel_radius(), o, 0.35));
            if menu {
                shapes.push(theme::glass_shape(&st.shell.look, ui.get_menu_x(), ui.get_menu_y(), ui.get_menu_w(), ui.get_menu_h(), 14.0, o, 0.35));
            }
        }
        let key = format!("{shapes:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, &shapes);
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    fn stats(&self) {
        if let Some(w) = &self.win {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report(NAME));
        }
    }

    fn quit(&mut self, _: &mut State) {
        if let Some(w) = &self.win {
            Glass::clear_now(&w.output, NS);
        }
    }
}
