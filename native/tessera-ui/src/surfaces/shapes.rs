//! Material 3 Expressive's shapes for empty and error states (the QML's
//! MaterialShape.qml draws them with a shader from MaterialShapes.js): here
//! an SVG outline for Slint's Path, in a 0..100 box, from a polar wave
//! r(θ) = R(1 + a·cos(nθ)), which is what the cookie, clover and burst
//! shapes are, near enough at 64 px.

/// "cookie9" (nine soft scallops), "clover4" (four round leaves),
/// "softBurst" (ten soft points), "sunny" (eight) or "cookie12" (twelve
/// scallops); anything else a circle
pub fn outline(name: &str) -> String {
    let (n, a, r) = match name {
        "cookie9" => (9.0, 0.07, 46.0),
        "clover4" => (4.0, 0.2, 41.0),
        "softBurst" => (10.0, 0.09, 45.0),
        "sunny" => (8.0, 0.09, 45.0),
        "cookie12" => (12.0, 0.05, 47.0),
        _ => (0.0, 0.0, 48.0),
    };
    let steps = 180;
    let mut d = String::new();
    for i in 0..=steps {
        let t = i as f64 / steps as f64 * std::f64::consts::TAU;
        // (clover leaves meet in a pinch: a sharper wave than a cosine)
        let w = if name == "clover4" { (n * t).cos().abs().powf(0.6) * 2.0 - 1.0 } else { (n * t).cos() };
        let rr = r * (1.0 + a * w);
        let (x, y) = (50.0 + rr * (t - std::f64::consts::FRAC_PI_2).cos(), 50.0 + rr * (t - std::f64::consts::FRAC_PI_2).sin());
        d += &format!("{}{x:.2} {y:.2} ", if i == 0 { "M " } else { "L " });
    }
    d + "Z"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outlines_stay_in_their_box() {
        for name in ["cookie9", "clover4", "softBurst", "sunny", "cookie12", "circle"] {
            let d = outline(name);
            assert!(d.starts_with("M ") && d.ends_with('Z'));
            let nums: Vec<f64> = d.split(|c: char| c == ' ' || c.is_ascii_alphabetic()).filter_map(|s| s.parse().ok()).collect();
            assert!(nums.len() > 300 && nums.iter().all(|v| (0.0..=100.0).contains(v)), "{name}");
        }
    }
}
