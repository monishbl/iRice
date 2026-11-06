//! The recording countdown (RecordCountdown.qml: a glass disc, Material a
//! hero shape, in the middle of the focused screen, clicks going through,
//! each second popping in) and a screen translation's card
//! (TranslationCard.qml: next to the translated area, Copy and close, Esc).

use super::{ic, M};
use crate::services::hypr::Glass;
use crate::theme;
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use serde_json::Value;
use slint::platform::Key;
use slint::{ComponentHandle, SharedString};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::rc::Rc;
use std::time::{Duration, Instant};

const NS_COUNT: &str = "tessera:recordCountdown";
const NS_TRANSLATION: &str = "tessera:translation";

/// Material's shapes, one a second (Android's timers morph between them)
const SHAPES: [&str; 5] = ["cookie9", "sunny", "cookie12", "clover4", "softBurst"];

/// The pop each second: 0.82 → 1 over 220 ms, overshooting (OutBack)
pub fn pop_at(ms: f32) -> f32 {
    let t = (ms / 220.0).clamp(0.0, 1.0);
    let (c1, c3) = (1.70158f32, 2.70158f32);
    let e = 1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2);
    0.82 + 0.18 * e
}

struct CWin {
    output: String,
    surface: Rc<Surface>,
    ui: ui::RecordCountdown,
    glass_sent: String,
}

#[derive(Default)]
pub struct Countdown {
    win: Option<CWin>,
    n: i64,
    tick: slint::Timer,
}

impl Countdown {
    pub fn set(&mut self, st: &mut State, n: i64, output: &str) {
        self.n = n;
        if n <= 0 {
            self.tick.stop();
            if let Some(w) = &mut self.win {
                w.surface.hide();
                st.shell.glass.set(&w.output, NS_COUNT, &[]);
                w.glass_sent.clear();
            }
            return;
        }
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            Glass::clear_now(&w.output, NS_COUNT);
            st.drop_surface(&w.surface);
        }
        if self.win.is_none() {
            let Some(wl) = st.output_named(output) else { return };
            let surface = st.prepare_surface(Spec {
                layer: Layer::Overlay,
                namespace: NS_COUNT,
                output: Some((&wl, output)),
                anchor: Anchor::empty(),
                size: (168, 168),
                exclusive: -1,
                keyboard: KeyboardInteractivity::None,
                margin_left: 0,
            });
            let ui = ui::RecordCountdown::new().expect("RecordCountdown");
            st.shell.apply_theme(&ui.global::<ui::Theme>());
            ui.show().expect("show countdown");
            st.set_input_region(&surface, Some(&[]));
            self.win = Some(CWin { output: output.into(), surface, ui, glass_sent: String::new() });
        }
        let w = self.win.as_ref().unwrap();
        w.ui.set_count(n as i32);
        w.ui.set_shape(crate::surfaces::shapes::outline(SHAPES[(n.max(0) as usize) % SHAPES.len()]).into());
        w.ui.set_pop(0.82);
        w.surface.show(168, 168);
        let weak = w.ui.as_weak();
        let t0 = Instant::now();
        self.tick.start(slint::TimerMode::Repeated, Duration::from_millis(16), move || {
            let ms = t0.elapsed().as_secs_f32() * 1000.0;
            if let Some(ui) = weak.upgrade() {
                ui.set_pop(pop_at(ms));
            }
        });
    }

    pub fn look(&mut self, st: &mut State) {
        if let Some(w) = &mut self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.glass_sent.clear();
        }
    }

    pub fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let w = self.win.take().unwrap();
            st.drop_surface(&w.surface);
        }
    }

    pub fn after_frame(&mut self, st: &mut State) {
        if self.tick.running() && pop_done(&self.win) {
            self.tick.stop();
        }
        let n = self.n;
        let Some(w) = &mut self.win else { return };
        if !w.surface.mapped.get() || !st.shell.plugin || st.shell.look.material {
            return;
        }
        let shapes = if n > 0 { vec![theme::glass_shape(&st.shell.look, 24.0, 24.0, 120.0, 120.0, 60.0, 1.0, 1.0)] } else { Vec::new() };
        let output = w.output.clone();
        super::send_glass(st, &output, NS_COUNT, &shapes, &mut w.glass_sent);
    }

    pub fn quit(&mut self) {
        if let Some(w) = &self.win {
            Glass::clear_now(&w.output, NS_COUNT);
        }
    }
}

fn pop_done(w: &Option<CWin>) -> bool {
    w.as_ref().is_some_and(|w| (w.ui.get_pop() - 1.0).abs() < 0.0001)
}

// ── The translation card ─────────────────────────────────────────────────

struct TWin {
    output: String,
    surface: Rc<Surface>,
    ui: ui::TranslationCard,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
    region_sent: Vec<(i32, i32, i32, i32)>,
}

#[derive(Default)]
pub struct Translation {
    win: Option<TWin>,
    text: String,
    shown: bool,
}

impl Translation {
    fn window(&mut self, st: &mut State, output: &str) -> bool {
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            Glass::clear_now(&w.output, NS_TRANSLATION);
            st.drop_surface(&w.surface);
        }
        if self.win.is_some() {
            return true;
        }
        let Some(wl) = st.output_named(output) else { return false };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS_TRANSLATION,
            output: Some((&wl, output)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::OnDemand,
            margin_left: 0,
        });
        let ui = ui::TranslationCard::new().expect("TranslationCard");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = st.shell.sender(super::NAME);
        {
            let to = to.clone();
            ui.on_copy(move || to(Box::new(M::TranslationCopy)));
        }
        {
            let to = to.clone();
            ui.on_dismiss(move || to(Box::new(M::TranslationClose)));
        }
        {
            let to = to.clone();
            ui.on_key(move |t| {
                if t == SharedString::from(Key::Escape) {
                    to(Box::new(M::TranslationClose));
                    return true;
                }
                false
            });
        }
        let m = st.shell.look.material;
        ui.set_ic_copy(ic("copy", m));
        ui.set_ic_close(ic("x", m));
        ui.show().expect("show translation");
        st.set_input_region(&surface, Some(&[]));
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(TWin { output: output.into(), surface, ui, a11y, glass_sent: String::new(), region_sent: Vec::new() });
        true
    }

    /// ScreenTools.translation: { x, y, w, h, screen, source, text, busy, error } or none
    pub fn set(&mut self, st: &mut State, t: Option<&Value>) {
        let Some(t) = t else {
            self.shown = false;
            if let Some(w) = &mut self.win {
                w.surface.hide();
                st.shell.glass.set(&w.output, NS_TRANSLATION, &[]);
                w.glass_sent.clear();
                w.region_sent.clear();
            }
            return;
        };
        let screen = t["screen"].as_str().map(str::to_string).filter(|s| !s.is_empty()).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(screen) = screen else { return };
        if !self.window(st, &screen) {
            return;
        }
        let f = |k: &str| t[k].as_f64().unwrap_or(0.0) as f32;
        let s = |k: &str| t[k].as_str().unwrap_or("").to_string();
        self.text = s("text");
        let w = self.win.as_ref().unwrap();
        w.ui.set_area_x(f("x"));
        w.ui.set_area_y(f("y"));
        w.ui.set_area_w(f("w"));
        w.ui.set_area_h(f("h"));
        w.ui.set_text(self.text.as_str().into());
        w.ui.set_source(s("source").into());
        w.ui.set_busy(t["busy"].as_bool().unwrap_or(false));
        w.ui.set_error(s("error").into());
        if !self.shown {
            w.surface.show(0, 0);
            self.shown = true;
        }
    }

    pub fn copy(&self) {
        if !self.text.is_empty() {
            crate::util::pipe_to(&["wl-copy"], self.text.clone().into_bytes());
        }
    }

    pub fn look(&mut self, st: &mut State) {
        if let Some(w) = &mut self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.glass_sent.clear();
        }
    }

    pub fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let w = self.win.take().unwrap();
            st.drop_surface(&w.surface);
            self.shown = false;
        }
    }

    pub fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let shown = self.shown;
        let Some(w) = &mut self.win else { return };
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
        if !w.surface.mapped.get() {
            return;
        }
        let ui = &w.ui;
        let (x, y, cw, ch) = (ui.get_card_x(), ui.get_card_y(), ui.get_card_w(), ui.get_card_h());
        let region = if shown { vec![(x.floor() as i32, y.floor() as i32, cw.ceil() as i32, ch.ceil() as i32)] } else { Vec::new() };
        if region != w.region_sent {
            st.set_input_region(&w.surface, Some(&region));
            w.region_sent = region;
        }
        if !st.shell.plugin || st.shell.look.material {
            return;
        }
        let shapes = if shown { vec![theme::glass_shape(&st.shell.look, x, y, cw, ch, 20.0, 1.0, 1.0)] } else { Vec::new() };
        let output = w.output.clone();
        super::send_glass(st, &output, NS_TRANSLATION, &shapes, &mut w.glass_sent);
    }

    pub fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    pub fn quit(&mut self) {
        if let Some(w) = &self.win {
            Glass::clear_now(&w.output, NS_TRANSLATION);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_second_pops_in() {
        assert!((pop_at(0.0) - 0.82).abs() < 1e-4);
        assert!((pop_at(220.0) - 1.0).abs() < 1e-4);
        assert!((pop_at(500.0) - 1.0).abs() < 1e-4);
        // (it overshoots on the way)
        assert!((0..220).map(|ms| pop_at(ms as f32)).any(|v| v > 1.0));
    }
}
