//! Live Text's layer and bar as Slint models (the editor's and Quick
//! Look's), and the found words' glow.

use super::ic;
use super::livetext::{self, Live, Ocr};
use crate::ui;
use slint::{ModelRc, VecModel};

/// The word boxes over a picture drawn at `dw` × `dh` (`region`: the part shown)
pub fn boxes(live: &Live, region: (f32, f32, f32, f32), dw: f32, dh: f32) -> ModelRc<ui::LtBox> {
    let v: Vec<ui::LtBox> = livetext::boxes(live, region, dw, dh).into_iter().map(|(x, y, w, h, sel, jl, jr)| ui::LtBox { x, y, w, h, sel, jl, jr }).collect();
    ModelRc::new(VecModel::from(v))
}

/// The bar's state; `room`: the width it has (chips that don't fit stay out)
pub fn bar(live: &Live, ocr: &Ocr, material: bool, room: f32) -> ui::LtBar {
    let (message, glyph) = livetext::bar_message(live, ocr);
    let actions = if live.status == "ready" { live.actions() } else { Vec::new() };
    let labels: Vec<String> = actions.iter().map(|a| livetext::chip_label(a, ocr)).collect();
    // (the status and the copy buttons first)
    let fixed = message.chars().count() as f32 * 7.0 + 24.0 + if live.has_selection() { 230.0 } else { 140.0 };
    let n = livetext::chips_that_fit(&labels, room - fixed - 16.0, material);
    let chips: Vec<ui::LtChip> = actions
        .iter()
        .zip(labels)
        .take(n)
        .map(|(a, label)| ui::LtChip { icon: ic(livetext::chip_icon(&a.kind), material), label: label.into(), tooltip: livetext::chip_tooltip(a, ocr).into() })
        .collect();
    let st = live.status.as_str();
    ui::LtBar {
        status: live.status.as_str().into(),
        message: message.into(),
        glyph: ic(glyph, material),
        danger: st == "error" || st == "missing",
        spinning: st == "reading" && live.said.is_empty(),
        chips: ModelRc::new(VecModel::from(chips)),
        has_sel: live.has_selection(),
        installing: ocr.installing,
        updates_running: ocr.updates_running,
    }
}

/// The glow once words are found: up in 150 ms, held 400, down over 450
pub fn glow_at(ms: f32) -> f32 {
    if ms < 150.0 {
        let t = ms / 150.0;
        1.0 - (1.0 - t).powi(3)
    } else if ms < 550.0 {
        1.0
    } else if ms < 1000.0 {
        let t = (ms - 550.0) / 450.0;
        let e = if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 };
        1.0 - e
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glow_comes_and_goes() {
        assert_eq!(glow_at(0.0), 0.0);
        assert_eq!(glow_at(300.0), 1.0);
        assert!(glow_at(775.0) > 0.4 && glow_at(775.0) < 0.6);
        assert_eq!(glow_at(1200.0), 0.0);
    }
}
