//! Finder-style window tabs (rust.surfaces: tabs; Settings → Appearance →
//! Window tabs: Finder), both looks: shell/modules/tabs/TabStrip.qml.
//! Hyprland groups the windows and keeps an empty bar's room above them
//! (hypr/tessera/tabs.lua); this draws the tabs there, one small layer per
//! group on the space each screen shows: the app icon and title, the front
//! tab raised, close on hover (or a middle click), + for a new window of the
//! app. Which tab is in front comes from the glass plugin (`groups`). While
//! windows move or spaces switch, the strips step out of the way and come
//! back once things settle (220 ms after the last move), so they never trail.
//!
//! Its own thread follows Hyprland's event socket (the raw events the QML
//! listens to, the plugin's drags included) and reads the groups and the
//! windows' places afresh 120 ms after each burst.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

use tessera_core::services::apps::{self, Apps};
use tessera_core::services::clients::{Client, Monitor};
use tessera_core::services::hypr::request;
use tessera_core::services::icons::Icons;
use tessera_core::shell::Part;
use tessera_core::wm::{self, AppIcons, IconsDone};
use tessera_core::wayland::{Spec, State, Surface};
use serde::Deserialize;
use slint::{ComponentHandle, ModelRc, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::rc::Rc;
use std::time::Duration;

const NAME: &str = "tabs";
pub const NS: &str = "tessera:tabs";
const BAR_H: i32 = 30;
/// group:groupbar:gaps_out
const GAP: i32 = 4;

#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Group {
    #[serde(default)]
    pub current: String,
    #[serde(default)]
    pub windows: Vec<String>,
    #[serde(default)]
    pub locked: bool,
}

/// One strip: its windows (in tab order), the front one, where (monitor-local)
#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    pub output: String,
    pub current: String,
    pub windows: Vec<Client>,
    pub x: i32,
    pub y: i32,
    pub w: i32,
}

fn norm(a: &str) -> String {
    format!("0x{}", a.trim_start_matches("0x").to_lowercase())
}

/// TabStrip.rebuild: a strip for each group of two or more windows on the
/// space a monitor shows, not fullscreen, over the group's bar
pub fn strips(groups: &[Group], clients: &[Client], monitors: &[Monitor]) -> Vec<Strip> {
    let mut out = Vec::new();
    for m in monitors {
        for g in groups {
            let wins: Vec<Client> = g.windows.iter().filter_map(|a| clients.iter().find(|c| norm(&c.address) == norm(a)).cloned()).collect();
            let cur = wins.iter().find(|c| norm(&c.address) == norm(&g.current)).or(wins.first());
            let Some(cur) = cur else { continue };
            if wins.len() < 2 || cur.workspace.id != m.active.id || cur.fullscreen > 0 || (cur.size.0 == 0 && cur.at.0 == 0 && cur.at.1 == 0) {
                continue;
            }
            out.push(Strip { output: m.name.clone(), current: norm(&g.current), windows: wins.clone(), x: cur.at.0 - m.x, y: cur.at.1 - m.y - GAP - BAR_H, w: cur.size.0 });
        }
    }
    out
}

/// The raw events that move windows (the strips step aside) or regroup them
pub fn kind(line: &str) -> Option<bool> {
    let (name, data) = line.split_once(">>").unwrap_or((line, ""));
    const MOVING: &[&str] = &["workspace", "workspacev2", "movewindow", "movewindowv2", "changefloatingmode", "fullscreen", "openwindow", "closewindow", "tessera-drag", "irice-drag", "moveworkspace", "moveworkspacev2", "configreloaded", "monitoradded", "monitorremoved"];
    const GROUPING: &[&str] = &["togglegroup", "moveintogroup", "moveoutofgroup", "activewindow", "activewindowv2", "lockgroups", "ignoregrouplock"];
    if MOVING.contains(&name) {
        // (a drag that ends doesn't hide them)
        Some(!((name == "tessera-drag" || name == "irice-drag") && data.starts_with("drop")))
    } else if GROUPING.contains(&name) {
        Some(false)
    } else {
        None
    }
}

/// What the thread reads after a burst
pub struct Read {
    pub groups: Vec<Group>,
    pub clients: Vec<Client>,
    pub monitors: Vec<Monitor>,
}

enum M {
    Event(bool),
    Query,
    Got(Read),
    Settle,
    Focus(usize, i32),
    Close(usize, i32),
    New(usize),
}

fn read() -> Read {
    let groups = ["tessera-glass groups", "irice-glass groups"].iter().find_map(|c| request(c).ok().and_then(|s| serde_json::from_str::<Vec<Group>>(&s).ok())).unwrap_or_default();
    let clients = request("j/clients").ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let monitors = request("j/monitors").ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    Read { groups, clients, monitors }
}

struct Bar {
    strip: Strip,
    surface: Rc<Surface>,
    ui: ui::TabBar,
}

#[derive(Default)]
pub struct Tabs {
    bars: Vec<Bar>,
    strips: Vec<Strip>,
    on: bool,
    settled: bool,
    listening: bool,
    icons: AppIcons,
    later: slint::Timer,
    settle: slint::Timer,
}

fn send(st: &State) -> impl Fn(Box<dyn Any + Send>) + Clone + 'static {
    st.shell.sender(NAME)
}

impl Tabs {
    fn wanted(st: &State) -> bool {
        st.shell.cfg["appearance"]["windowTabs"].as_str() == Some("finder")
    }

    /// Hyprland's events, on a thread of its own
    fn listen(&mut self, st: &State) {
        if self.listening {
            return;
        }
        self.listening = true;
        let to = send(st);
        std::thread::spawn(move || loop {
            let Some(path) = crate::services::hypr::dir().map(|d| d.join(".socket2.sock")) else { return };
            let Ok(stream) = UnixStream::connect(path) else {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            };
            let mut r = BufReader::new(stream);
            let mut line = String::new();
            let mut moving: Option<bool> = None;
            loop {
                line.clear();
                match r.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                if let Some(m) = kind(line.trim_end()) {
                    moving = Some(moving.unwrap_or(false) || m);
                }
                if r.buffer().is_empty() {
                    if let Some(m) = moving.take() {
                        to(Box::new(M::Event(m)));
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        });
    }

    fn refresh(&mut self, st: &State, moving: bool) {
        if moving && self.settled {
            self.settled = false;
            for b in &self.bars {
                b.ui.set_settled(false);
            }
        }
        let to = send(st);
        self.later.start(slint::TimerMode::SingleShot, Duration::from_millis(120), move || to(Box::new(M::Query)));
    }

    fn query(&self, st: &State) {
        let to = send(st);
        std::thread::spawn(move || to(Box::new(M::Got(read()))));
    }

    fn follow(&mut self, st: &mut State) {
        self.on = Self::wanted(st);
        if self.on {
            self.listen(st);
            self.settled = true;
            self.refresh(st, false);
        } else {
            for b in self.bars.drain(..) {
                st.drop_surface(&b.surface);
            }
            self.strips.clear();
        }
    }

    /// One layer per strip, placed over its group's bar
    fn place(&mut self, st: &mut State) {
        // (only remade when something about the strips changed: no flashing)
        let mut keep: Vec<Bar> = Vec::new();
        let mut old = std::mem::take(&mut self.bars);
        for s in self.strips.clone() {
            let found = old.iter().position(|b| b.strip.output == s.output && b.strip.windows.iter().map(|c| &c.address).eq(s.windows.iter().map(|c| &c.address)));
            let bar = match found {
                Some(i) => {
                    let b = old.remove(i);
                    if (b.strip.x, b.strip.y, b.strip.w) != (s.x, s.y, s.w) {
                        b.surface.layer.set_margin(s.y.max(0), 0, 0, s.x.max(0));
                        b.surface.layer.set_size(s.w.max(1) as u32, BAR_H as u32);
                        b.surface.layer.commit();
                    }
                    Bar { strip: s, ..b }
                }
                None => {
                    let Some(output) = st.output_named(&s.output) else { continue };
                    let surface = st.prepare_surface(Spec {
                        layer: Layer::Top,
                        namespace: NS,
                        output: Some((&output, &s.output)),
                        anchor: Anchor::TOP | Anchor::LEFT,
                        size: (s.w.max(1) as u32, BAR_H as u32),
                        exclusive: -1,
                        keyboard: KeyboardInteractivity::None,
                        margin_left: s.x.max(0),
                    });
                    surface.layer.set_margin(s.y.max(0), 0, 0, s.x.max(0));
                    let ui = ui::TabBar::new().expect("TabBar");
                    st.shell.apply_theme(&ui.global::<ui::Theme>());
                    let m = st.shell.look.material;
                    ui.set_g_x(wglyph("x", m, true));
                    ui.set_g_plus(wglyph("plus", m, true));
                    let n = keep.len();
                    let to = send(st);
                    {
                        let to = to.clone();
                        ui.on_focus_tab(move |i| to(Box::new(M::Focus(n, i))));
                    }
                    {
                        let to = to.clone();
                        ui.on_close_tab(move |i| to(Box::new(M::Close(n, i))));
                    }
                    ui.on_new_tab(move || to(Box::new(M::New(n))));
                    ui.show().expect("show tabs");
                    surface.show(s.w.max(1) as u32, BAR_H as u32);
                    if self.bars.is_empty() && keep.is_empty() {
                        let t = ui.global::<ui::Theme>();
                        self.icons.restyle(st, t.get_accent(), t.get_m3_primary_container(), t.get_m3_on_primary_container());
                    }
                    Bar { strip: s, surface, ui }
                }
            };
            keep.push(bar);
        }
        for b in old {
            st.drop_surface(&b.surface);
        }
        // (callbacks name a bar by its place: re-made ones keep theirs)
        self.bars = keep;
        for (n, b) in self.bars.iter().enumerate() {
            let to = send(st);
            {
                let to = to.clone();
                b.ui.on_focus_tab(move |i| to(Box::new(M::Focus(n, i))));
            }
            {
                let to = to.clone();
                b.ui.on_close_tab(move |i| to(Box::new(M::Close(n, i))));
            }
            b.ui.on_new_tab(move || to(Box::new(M::New(n))));
        }
        self.push(st);
    }

    fn push(&mut self, st: &State) {
        for i in 0..self.bars.len() {
            let scale = self.bars[i].ui.window().scale_factor().max(1.0);
            let s = self.bars[i].strip.clone();
            let tabs: Vec<ui::TbTab> = s
                .windows
                .iter()
                .enumerate()
                .map(|(k, c)| {
                    let front = norm(&c.address) == s.current;
                    let prev_back = k > 0 && norm(&s.windows[k - 1].address) != s.current;
                    let icon = self.icons.get(st, &c.class, 14.0, scale, true);
                    ui::TbTab { title: if c.title.is_empty() { c.class.as_str().into() } else { c.title.as_str().into() }, has_icon: icon.is_some(), icon: icon.unwrap_or_default(), front, divider: k > 0 && !front && prev_back }
                })
                .collect();
            let b = &self.bars[i];
            b.ui.set_tabs(ModelRc::new(VecModel::from(tabs)));
            b.ui.set_settled(self.settled);
        }
        self.icons.flush(send(st));
    }
}

impl Part for Tabs {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        st.shell.want("apps");
        st.shell.want("icons");
        self.follow(st);
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if look {
            for b in &self.bars {
                st.shell.apply_theme(&b.ui.global::<ui::Theme>());
                let m = st.shell.look.material;
                b.ui.set_g_x(wglyph("x", m, true));
                b.ui.set_g_plus(wglyph("plus", m, true));
            }
            if let Some(b) = self.bars.first() {
                let t = b.ui.global::<ui::Theme>();
                self.icons.restyle(st, t.get_accent(), t.get_m3_primary_container(), t.get_m3_on_primary_container());
            }
        }
        if Self::wanted(st) != self.on {
            self.follow(st);
        } else {
            self.push(st);
        }
    }

    fn data(&mut self, st: &mut State, what: std::any::TypeId) {
        if what == std::any::TypeId::of::<Icons>() || what == std::any::TypeId::of::<Apps>() {
            self.push(st);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        let (gone, keep): (Vec<Bar>, Vec<Bar>) = std::mem::take(&mut self.bars).into_iter().partition(|b| b.strip.output == name);
        for b in gone {
            st.drop_surface(&b.surface);
        }
        self.bars = keep;
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if let Some(i) = self.bars.iter().position(|b| Rc::ptr_eq(&b.surface, s)) {
            let b = self.bars.remove(i);
            st.drop_surface(&b.surface);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<IconsDone>() {
            Ok(d) => {
                self.icons.done(*d);
                self.push(st);
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        if !self.on {
            return;
        }
        match *m {
            M::Event(moving) => self.refresh(st, moving),
            M::Query => self.query(st),
            M::Got(r) => {
                let next = strips(&r.groups, &r.clients, &r.monitors);
                if next != self.strips {
                    self.strips = next;
                    self.place(st);
                }
                let to = send(st);
                self.settle.start(slint::TimerMode::SingleShot, Duration::from_millis(220), move || to(Box::new(M::Settle)));
            }
            M::Settle => {
                self.settled = true;
                for b in &self.bars {
                    b.ui.set_settled(true);
                }
            }
            M::Focus(n, i) => {
                if let Some(c) = self.bars.get(n).and_then(|b| b.strip.windows.get(i.max(0) as usize)) {
                    crate::services::clients::focus(&c.address);
                }
            }
            M::Close(n, i) => {
                if let Some(c) = self.bars.get(n).and_then(|b| b.strip.windows.get(i.max(0) as usize)) {
                    crate::services::clients::close(&c.address);
                }
            }
            M::New(n) => {
                let class = self.bars.get(n).and_then(|b| b.strip.windows.first()).map(|c| c.class.clone()).unwrap_or_default();
                if let Some(e) = st.shell.get::<Apps>().and_then(|a| a.entry_for(&class)) {
                    apps::launch(e);
                }
            }
        }
    }
}

/// A glyph as this crate's Slint takes it (tessera_core::wm::glyph)
fn wglyph(name: &str, material: bool, bold: bool) -> ui::WGlyph {
    let (text, family, scale) = tessera_core::wm::glyph(name, material, bold);
    ui::WGlyph { text, family, scale }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::clients::WsRef;

    fn c(a: &str, ws: i32, at: (i32, i32), size: (i32, i32)) -> Client {
        Client { address: a.into(), workspace: WsRef { id: ws, name: ws.to_string() }, at, size, ..Default::default() }
    }

    #[test]
    fn a_strip_over_each_group_on_the_space_shown() {
        let m = Monitor { id: 0, name: "DP-1".into(), x: 0, y: 0, width: 1920, height: 1080, scale: 1.0, active: WsRef { id: 1, name: "1".into() }, ..Default::default() };
        let clients = vec![c("0xa", 1, (100, 200), (800, 600)), c("0xb", 1, (100, 200), (800, 600)), c("0xc", 2, (0, 0), (10, 10)), c("0xd", 2, (0, 0), (10, 10)), c("0xe", 1, (0, 0), (5, 5))];
        let groups = vec![
            Group { current: "0xb".into(), windows: vec!["0xa".into(), "0xb".into()], locked: false },
            Group { current: "0xc".into(), windows: vec!["0xc".into(), "0xd".into()], locked: false },
            Group { current: "0xe".into(), windows: vec!["0xe".into()], locked: false },
        ];
        let s = strips(&groups, &clients, &[m]);
        assert_eq!(s.len(), 1, "another space's group, and a group of one, have none");
        assert_eq!((s[0].x, s[0].y, s[0].w, s[0].current.as_str()), (100, 200 - 4 - 30, 800, "0xb"));
    }

    #[test]
    fn which_events_move_or_regroup() {
        assert_eq!(kind("movewindowv2>>a,1,1"), Some(true));
        assert_eq!(kind("tessera-drag>>drop,1"), Some(false));
        assert_eq!(kind("tessera-drag>>start"), Some(true));
        assert_eq!(kind("activewindowv2>>a"), Some(false));
        assert_eq!(kind("urgent>>a"), None);
    }
}
