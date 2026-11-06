//! The region selector (RegionSelector.qml): dims the screen, drag to select
//! (its size beside it), or click a highlighted window to take all of it;
//! Circle to Search draws its stroke from the first pixel. The pointer's
//! moves are handled in the window's callbacks, synchronously, so the box and
//! the stroke are in the frame that follows the hand. Once chosen it hides
//! itself, waits 160 ms for the compositor to drop it, then asks the shell
//! to capture (`screen.captured`).

use super::geometry::{self, box_between, clip, stroke_box, stroke_kind, Rect, Stroke};
use super::M;
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use slint::platform::Key;
use slint::{ComponentHandle, SharedString};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const NS: &str = "tessera:region";

#[derive(Default)]
struct Drag {
    dragging: bool,
    circling: bool,
    start: (f32, f32),
    end: (f32, f32),
    stroke: Vec<(f32, f32)>,
    path: String,
    hovered: Option<Rect>,
    windows: Vec<Rect>,
    w: f32,
    h: f32,
}

impl Drag {
    fn clamp(&self, x: f32, y: f32) -> (f32, f32) {
        (x.clamp(0.0, self.w), y.clamp(0.0, self.h))
    }
    fn kind(&self) -> Stroke {
        if self.circling && self.dragging {
            stroke_kind(&self.stroke)
        } else {
            Stroke::Box
        }
    }
    fn selection(&self) -> Rect {
        let r = if self.dragging {
            if self.circling {
                stroke_box(&self.stroke, 12.0, self.w, self.h)
            } else {
                box_between(self.start, self.end)
            }
        } else {
            self.hovered.unwrap_or_default()
        };
        clip(r, self.w, self.h)
    }
    fn apply(&self, ui: &ui::RegionSelector) {
        let s = self.selection();
        ui.set_sel_x(s.x);
        ui.set_sel_y(s.y);
        ui.set_sel_w(s.w);
        ui.set_sel_h(s.h);
        ui.set_dragging(self.dragging);
        ui.set_circling(self.circling);
        ui.set_show_box(s.w > 0.0 && self.kind() == Stroke::Box);
        ui.set_show_size(s.w > 0.0 && !self.circling);
        if self.circling && self.dragging {
            ui.set_stroke(SharedString::from(self.path.as_str()));
        }
    }
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::RegionSelector,
    a11y: Option<crate::a11y::A11y>,
}

#[derive(Default)]
pub struct Region {
    win: Option<Win>,
    mode: String,
    screen: String,
    drag: Rc<RefCell<Drag>>,
    delay: slint::Timer,
}

impl Region {
    fn window(&mut self, st: &mut State, output: &str) -> bool {
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            st.drop_surface(&w.surface);
        }
        if self.win.is_some() {
            return true;
        }
        let Some(wl) = st.output_named(output) else { return false };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((&wl, output)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::RegionSelector::new().expect("RegionSelector");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = st.shell.sender(super::NAME);
        {
            let (d, weak, to) = (self.drag.clone(), ui.as_weak(), to.clone());
            ui.on_down(move |x, y, right| {
                if right {
                    to(Box::new(M::RegionCancel));
                    return;
                }
                let mut d = d.borrow_mut();
                let p = d.clamp(x, y);
                d.start = p;
                d.end = p;
                d.stroke = vec![p];
                d.path = format!("M {:.1} {:.1} L {:.1} {:.1}", p.0, p.1, p.0 + 0.01, p.1);
                d.dragging = true;
                if let Some(ui) = weak.upgrade() {
                    d.apply(&ui);
                }
            });
        }
        {
            let (d, weak) = (self.drag.clone(), ui.as_weak());
            ui.on_moved_to(move |x, y| {
                let mut d = d.borrow_mut();
                if d.dragging {
                    let p = d.clamp(x, y);
                    d.end = p;
                    // (a point every 2 px is smooth enough)
                    let last = *d.stroke.last().unwrap_or(&p);
                    if d.circling && (p.0 - last.0).hypot(p.1 - last.1) >= 2.0 {
                        d.stroke.push(p);
                        d.path += &format!(" L {:.1} {:.1}", p.0, p.1);
                    }
                } else {
                    let hit = d.windows.iter().find(|r| r.contains(x, y)).copied();
                    if hit == d.hovered {
                        return;
                    }
                    d.hovered = hit;
                }
                if let Some(ui) = weak.upgrade() {
                    d.apply(&ui);
                }
            });
        }
        {
            let (d, weak, to) = (self.drag.clone(), ui.as_weak(), to.clone());
            ui.on_up(move |_, _| {
                let mut d = d.borrow_mut();
                if !d.dragging {
                    return;
                }
                // (circling: back where it began still counts as drawn)
                let moved = if d.circling { d.kind() != Stroke::None } else { (d.end.0 - d.start.0).abs() > 4.0 || (d.end.1 - d.start.1).abs() > 4.0 };
                let r = if moved { Some(d.selection()) } else { d.hovered };
                d.dragging = false;
                if let Some(ui) = weak.upgrade() {
                    d.apply(&ui);
                }
                if let Some(r) = r {
                    to(Box::new(M::RegionFinish(r)));
                }
            });
        }
        {
            let (d, to) = (self.drag.clone(), to.clone());
            ui.on_key(move |text| {
                let is = |k: Key| text == SharedString::from(k);
                if is(Key::Escape) {
                    to(Box::new(M::RegionCancel));
                    return true;
                }
                if is(Key::Return) {
                    let d = d.borrow();
                    let s = d.selection();
                    to(Box::new(M::RegionFinish(if s.w > 0.0 { s } else { Rect::new(0.0, 0.0, d.w, d.h) })));
                    return true;
                }
                false
            });
        }
        ui.show().expect("show region selector");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: output.into(), surface, ui, a11y });
        true
    }

    /// The shell's ScreenTools.selecting changed: show it for that mode, or
    /// put it away ("")
    pub fn select(&mut self, st: &mut State, mode: &str, screen: &str) {
        self.delay.stop();
        self.mode = mode.to_string();
        if mode.is_empty() {
            if let Some(w) = &self.win {
                w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
                w.surface.hide();
            }
            return;
        }
        let output = if screen.is_empty() { st.outputs_now().first().map(|o| o.1.clone()).unwrap_or_default() } else { screen.to_string() };
        if !self.window(st, &output) {
            return;
        }
        self.screen = output.clone();
        let (w, h) = super::output_size(st, &output);
        {
            let mut d = self.drag.borrow_mut();
            *d = Drag { circling: mode == "circle", w, h, ..Default::default() };
        }
        let win = self.win.as_ref().unwrap();
        win.ui.set_hint(geometry::hint(mode).into());
        win.ui.set_stroke("".into());
        self.drag.borrow().apply(&win.ui);
        win.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        win.surface.show(0, 0);
        win.ui.invoke_focus_keys();
        // the windows to click: Hyprland's, on this monitor's showing workspace
        let to = st.shell.sender(super::NAME);
        std::thread::spawn(move || {
            let j = |c: &str| crate::services::hypr::request(c).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()).unwrap_or_default();
            let list = geometry::windows_on(&j("j/monitors"), &j("j/clients"), &output);
            to(Box::new(M::RegionWindows(output, list)));
        });
    }

    pub fn windows(&mut self, screen: &str, list: Vec<Rect>) {
        if screen == self.screen {
            self.drag.borrow_mut().windows = list;
        }
    }

    /// Chosen: hide, then (once the compositor dropped the overlay) capture
    pub fn finish(&mut self, st: &mut State, r: Rect) {
        let (w, h) = super::output_size(st, &self.screen);
        let r = clip(r, w, h);
        if r.w < 4.0 || r.h < 4.0 || self.mode.is_empty() {
            return;
        }
        if let Some(win) = &self.win {
            win.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            win.surface.hide();
        }
        let (mode, screen) = (self.mode.clone(), self.screen.clone());
        self.delay.start(slint::TimerMode::SingleShot, Duration::from_millis(160), move || {
            crate::shell::ask("screen.captured", serde_json::json!([mode, screen, r.x, r.y, r.w, r.h]));
        });
    }

    pub fn look(&mut self, st: &mut State) {
        if let Some(w) = &self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
        }
    }

    pub fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let w = self.win.take().unwrap();
            st.drop_surface(&w.surface);
            if !self.mode.is_empty() {
                crate::shell::ask("screen.cancel", serde_json::json!([]));
            }
        }
    }

    pub fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        if let Some(w) = &mut self.win {
            let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, f);
            }
        }
    }

    pub fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }
}
