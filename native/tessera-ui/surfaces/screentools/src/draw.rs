//! Markup as drawn (Annotation.qml): the shapes' geometry, shared by the
//! editor's display (Slint paths, in display pixels) and the flattened
//! picture it saves (tiny-skia at the image's native pixels, text through
//! resvg). QML flattened with grabToImage; here the same marks are drawn
//! again onto the screenshot's own pixels.

use super::geometry::Rect;
use super::session::Ann;
use image::RgbaImage;
use resvg::tiny_skia::{self, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform};

/// "#RRGGBB" → (r, g, b)
pub fn rgb(hex: &str) -> (u8, u8, u8) {
    let h = hex.trim_start_matches('#');
    let v = u32::from_str_radix(h.get(..6).unwrap_or("000000"), 16).unwrap_or(0);
    ((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// An arrow's head (Annotation.qml): its length grows with the stroke, never
/// more than 60% of the arrow. (shaft end, tip, left, right)
pub fn arrow_parts(x1: f32, y1: f32, x2: f32, y2: f32, width: f32) -> ((f32, f32), [(f32, f32); 3]) {
    let (dx, dy) = (x2 - x1, y2 - y1);
    let len = dx.hypot(dy).max(0.001);
    let (ux, uy) = (dx / len, dy / len);
    let head = (len * 0.6).min(width * 3.2 + 10.0);
    let half = head * 0.55;
    let (bx, by) = (x2 - ux * head, y2 - uy * head);
    // (the shaft stops inside the head so it doesn't poke through the tip)
    ((bx + ux * head * 0.3, by + uy * head * 0.3), [(x2, y2), (bx + uy * half, by - ux * half), (bx - uy * half, by + ux * half)])
}

/// An ellipse's radii: its stroke inside the box
pub fn ellipse_radii(r: Rect, width: f32) -> (f32, f32) {
    (((r.w - width) / 2.0).max(0.5), ((r.h - width) / 2.0).max(0.5))
}

/// A rectangle's corner (Annotation.qml: as round as its stroke is wide)
pub fn rect_radius(r: Rect, width: f32) -> f32 {
    width.min(r.w.min(r.h) / 2.0)
}

/// A redaction's blocks: the screenshot under it averaged in `block` px
/// cells (cols × rows, RGBA)
pub fn pixelate(base: &RgbaImage, r: Rect, block: f32) -> (u32, u32, Vec<u8>) {
    let b = block.max(1.0);
    let cols = ((r.w / b).ceil() as u32).max(1);
    let rows = ((r.h / b).ceil() as u32).max(1);
    let (iw, ih) = base.dimensions();
    let mut out = Vec::with_capacity((cols * rows * 4) as usize);
    for j in 0..rows {
        for i in 0..cols {
            let x0 = (r.x + i as f32 * b).floor().max(0.0) as u32;
            let y0 = (r.y + j as f32 * b).floor().max(0.0) as u32;
            let x1 = ((r.x + (i + 1) as f32 * b).min(r.x + r.w).ceil() as u32).min(iw);
            let y1 = ((r.y + (j + 1) as f32 * b).min(r.y + r.h).ceil() as u32).min(ih);
            let (mut s, mut n) = ([0u64; 4], 0u64);
            for y in y0..y1.max(y0) {
                for x in x0..x1.max(x0) {
                    let p = base.get_pixel(x, y).0;
                    for c in 0..4 {
                        s[c] += p[c] as u64;
                    }
                    n += 1;
                }
            }
            let n = n.max(1);
            out.extend((0..4).map(|c| (s[c] / n) as u8));
        }
    }
    (cols, rows, out)
}

/// A polyline's SVG path, mapped (`f`: native → display)
pub fn polyline(points: &[(f32, f32)], f: impl Fn(f32, f32) -> (f32, f32)) -> String {
    let mut d = String::with_capacity(points.len() * 16);
    for (i, &(x, y)) in points.iter().enumerate() {
        let (a, b) = f(x, y);
        d += &format!("{}{a:.1} {b:.1} ", if i == 0 { "M " } else { "L " });
    }
    if points.len() == 1 {
        let (a, b) = f(points[0].0, points[0].1);
        d += &format!("L {:.1} {b:.1}", a + 0.01);
    }
    d
}

/// An ellipse's SVG path round its centre (display px)
pub fn ellipse_path(cx: f32, cy: f32, rx: f32, ry: f32) -> String {
    format!("M {:.1} {cy:.1} A {rx:.1} {ry:.1} 0 1 1 {:.1} {cy:.1} A {rx:.1} {ry:.1} 0 1 1 {:.1} {cy:.1} Z", cx + rx, cx - rx, cx + rx)
}

// ── Flattening ─────────────────────────────────────────────────────────

/// The font labels are drawn in, for text marks: its data and family, and
/// how far below the top its baseline sits (in em)
pub struct LabelFont {
    pub data: Vec<u8>,
    pub ascent: f32,
}

/// The look's font file (the shell's family at weight 600): Material's from
/// the repo, Glass's through fontconfig
pub fn label_font(material: bool) -> Option<LabelFont> {
    if material {
        let p = crate::util::asset("material/assets/fonts/GoogleSansFlex.ttf")?;
        return std::fs::read(p).ok().map(|data| LabelFont { data, ascent: 1.0 });
    }
    let out = std::process::Command::new("fc-match").args(["-f", "%{file}", "Inter:weight=180"]).output().ok()?;
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    std::fs::read(path).ok().map(|data| LabelFont { data, ascent: 0.97 })
}

fn push_rounded(pb: &mut PathBuilder, r: Rect, rad: f32) {
    let k = 0.552_284_8 * rad;
    let (x0, y0, x1, y1) = (r.x, r.y, r.x + r.w, r.y + r.h);
    pb.move_to(x0 + rad, y0);
    pb.line_to(x1 - rad, y0);
    pb.cubic_to(x1 - rad + k, y0, x1, y0 + rad - k, x1, y0 + rad);
    pb.line_to(x1, y1 - rad);
    pb.cubic_to(x1, y1 - rad + k, x1 - rad + k, y1, x1 - rad, y1);
    pb.line_to(x0 + rad, y1);
    pb.cubic_to(x0 + rad - k, y1, x0, y1 - rad + k, x0, y1 - rad);
    pb.line_to(x0, y0 + rad);
    pb.cubic_to(x0, y0 + rad - k, x0 + rad - k, y0, x0 + rad, y0);
    pb.close();
}

fn paint(hex: &str) -> Paint<'static> {
    let (r, g, b) = rgb(hex);
    let mut p = Paint::default();
    p.set_color_rgba8(r, g, b, 255);
    p.anti_alias = true;
    p
}

fn stroke(width: f32, cap: LineCap) -> Stroke {
    Stroke { width, line_cap: cap, line_join: LineJoin::Round, ..Default::default() }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The image as shown (markup over it, then the crop), at native pixels
pub fn flatten(base: &RgbaImage, items: &[Ann], crop: Rect, font: Option<&LabelFont>) -> Option<RgbaImage> {
    let (w, h) = base.dimensions();
    // (tiny-skia works premultiplied)
    let mut data = base.as_raw().clone();
    for p in data.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a < 255 {
            for c in 0..3 {
                p[c] = (p[c] as u32 * a / 255) as u8;
            }
        }
    }
    let mut pm = Pixmap::from_vec(data, tiny_skia::IntSize::from_wh(w, h)?)?;
    let mut fontdb = None;
    for a in items {
        match a {
            Ann::Stroke { points, color, width, highlighter } => {
                let mut pb = PathBuilder::new();
                for (i, &(x, y)) in points.iter().enumerate() {
                    if i == 0 {
                        pb.move_to(x, y);
                    } else {
                        pb.line_to(x, y);
                    }
                }
                if points.len() == 1 {
                    pb.line_to(points[0].0 + 0.01, points[0].1);
                }
                let Some(path) = pb.finish() else { continue };
                if !highlighter {
                    pm.stroke_path(&path, &paint(color), &stroke(*width, LineCap::Round), Transform::identity(), None);
                } else {
                    // opaque in its own layer, then faded as a whole (it
                    // doesn't darken where it crosses itself)
                    let Some(mut layer) = Pixmap::new(w, h) else { continue };
                    layer.stroke_path(&path, &paint(color), &stroke(*width, LineCap::Butt), Transform::identity(), None);
                    pm.draw_pixmap(0, 0, layer.as_ref(), &PixmapPaint { opacity: 0.4, ..Default::default() }, Transform::identity(), None);
                }
            }
            Ann::Arrow { x1, y1, x2, y2, color, width } => {
                let (end, head) = arrow_parts(*x1, *y1, *x2, *y2, *width);
                let mut pb = PathBuilder::new();
                pb.move_to(*x1, *y1);
                pb.line_to(end.0, end.1);
                if let Some(p) = pb.finish() {
                    pm.stroke_path(&p, &paint(color), &stroke(*width, LineCap::Round), Transform::identity(), None);
                }
                let mut pb = PathBuilder::new();
                pb.move_to(head[0].0, head[0].1);
                pb.line_to(head[1].0, head[1].1);
                pb.line_to(head[2].0, head[2].1);
                pb.close();
                if let Some(p) = pb.finish() {
                    pm.fill_path(&p, &paint(color), FillRule::Winding, Transform::identity(), None);
                    pm.stroke_path(&p, &paint(color), &stroke((width * 0.5).max(1.0), LineCap::Butt), Transform::identity(), None);
                }
            }
            Ann::Rect { r, color, width } => {
                // the border inside the box, as a QML Rectangle's
                let rad = rect_radius(*r, *width);
                let inner = Rect::new(r.x + width, r.y + width, (r.w - 2.0 * width).max(0.0), (r.h - 2.0 * width).max(0.0));
                let mut pb = PathBuilder::new();
                push_rounded(&mut pb, *r, rad);
                if inner.w > 0.0 && inner.h > 0.0 {
                    push_rounded(&mut pb, inner, (rad - width).max(0.0));
                }
                if let Some(p) = pb.finish() {
                    pm.fill_path(&p, &paint(color), FillRule::EvenOdd, Transform::identity(), None);
                }
            }
            Ann::Ellipse { r, color, width } => {
                let (rx, ry) = ellipse_radii(*r, *width);
                let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
                if let Some(p) = tiny_skia::Rect::from_xywh(cx - rx, cy - ry, 2.0 * rx, 2.0 * ry).and_then(PathBuilder::from_oval) {
                    pm.stroke_path(&p, &paint(color), &stroke(*width, LineCap::Butt), Transform::identity(), None);
                }
            }
            Ann::Text { x, y, text, color, size } => {
                let Some(f) = font else { continue };
                let db = fontdb.get_or_insert_with(|| {
                    let mut db = resvg::usvg::fontdb::Database::new();
                    db.load_font_data(f.data.clone());
                    db
                });
                let family = db.faces().next().and_then(|face| face.families.first().map(|n| n.0.clone())).unwrap_or_else(|| "sans-serif".into());
                let svg = format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\"><text x=\"{x}\" y=\"{}\" font-family=\"{}\" font-weight=\"600\" font-size=\"{size}\" fill=\"{color}\" xml:space=\"preserve\">{}</text></svg>",
                    y + size * f.ascent,
                    xml_escape(&family),
                    xml_escape(text)
                );
                let mut opt = resvg::usvg::Options::default();
                opt.fontdb = std::sync::Arc::new(db.clone());
                opt.font_family = family;
                if let Ok(tree) = resvg::usvg::Tree::from_str(&svg, &opt) {
                    resvg::render(&tree, Transform::identity(), &mut pm.as_mut());
                }
            }
            Ann::Pixelate { r, block } => {
                let (cols, rows, px) = pixelate(base, *r, *block);
                let b = block.max(1.0);
                for j in 0..rows {
                    for i in 0..cols {
                        let k = ((j * cols + i) * 4) as usize;
                        let mut p = Paint::default();
                        p.set_color_rgba8(px[k], px[k + 1], px[k + 2], px[k + 3]);
                        p.anti_alias = false;
                        let x0 = r.x + i as f32 * b;
                        let y0 = r.y + j as f32 * b;
                        let cw = b.min(r.x + r.w - x0);
                        let ch = b.min(r.y + r.h - y0);
                        if let Some(rect) = tiny_skia::Rect::from_xywh(x0, y0, cw, ch) {
                            pm.fill_rect(rect, &p, Transform::identity(), None);
                        }
                    }
                }
            }
        }
    }
    let out = RgbaImage::from_raw(w, h, pm.take_demultiplied())?;
    if crop.w >= 1.0 && crop.h >= 1.0 {
        let c = super::geometry::integer_clip(crop, w as f32, h as f32);
        return Some(image::imageops::crop_imm(&out, c.x as u32, c.y as u32, c.w.max(1.0) as u32, c.h.max(1.0) as u32).to_image());
    }
    Some(out)
}

/// Written as PNG (a temporary file renamed over the target)
pub fn write_png(img: &RgbaImage, path: &str) -> bool {
    let tmp = format!("{path}.tessera-tmp.png");
    if img.save_with_format(&tmp, image::ImageFormat::Png).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return false;
    }
    std::fs::rename(&tmp, path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_heads_and_shapes() {
        let (end, head) = arrow_parts(0.0, 0.0, 100.0, 0.0, 2.0);
        // head 2 × 3.2 + 10 = 16.4 long, 55% as wide each side
        assert!((head[1].0 - 83.6).abs() < 0.01 && (head[1].1 + 9.02).abs() < 0.01);
        assert!((end.0 - (83.6 + 16.4 * 0.3)).abs() < 0.01);
        // a short arrow's head: at most 60% of it
        let (_, h) = arrow_parts(0.0, 0.0, 10.0, 0.0, 4.0);
        assert!((h[1].0 - 4.0).abs() < 0.01);
        assert_eq!(ellipse_radii(Rect::new(0.0, 0.0, 10.0, 20.0), 2.0), (4.0, 9.0));
        assert_eq!(rect_radius(Rect::new(0.0, 0.0, 10.0, 3.0), 2.0), 1.5);
        assert_eq!(rgb("#D05F43"), (0xD0, 0x5F, 0x43));
    }

    #[test]
    fn redaction_averages_blocks() {
        let mut img = RgbaImage::new(4, 2);
        for (x, _, p) in img.enumerate_pixels_mut() {
            *p = image::Rgba([if x < 2 { 0 } else { 200 }, 0, 0, 255]);
        }
        let (c, r, px) = pixelate(&img, Rect::new(0.0, 0.0, 4.0, 2.0), 2.0);
        assert_eq!((c, r), (2, 1));
        assert_eq!(&px[..], &[0, 0, 0, 255, 200, 0, 0, 255]);
        // ragged edge: a smaller last cell
        let (c, _, _) = pixelate(&img, Rect::new(0.0, 0.0, 3.0, 2.0), 2.0);
        assert_eq!(c, 2);
    }

    #[test]
    fn flattening_draws_marks_and_crops() {
        let base = RgbaImage::from_pixel(40, 30, image::Rgba([255, 255, 255, 255]));
        let items = vec![
            Ann::Rect { r: Rect::new(5.0, 5.0, 20.0, 10.0), color: "#000000".into(), width: 2.0 },
            Ann::Stroke { points: vec![(30.0, 20.0), (35.0, 25.0)], color: "#FF0000".into(), width: 3.0, highlighter: false },
        ];
        let out = flatten(&base, &items, Rect::default(), None).unwrap();
        assert_eq!(out.dimensions(), (40, 30));
        assert_eq!(out.get_pixel(15, 5).0[0], 0); // the border's top edge
        assert_eq!(out.get_pixel(15, 10).0[0], 255); // inside it: untouched
        assert!(out.get_pixel(32, 22).0[1] < 100); // the red stroke
        let cut = flatten(&base, &items, Rect::new(5.0, 5.0, 10.0, 8.0), None).unwrap();
        assert_eq!(cut.dimensions(), (10, 8));
        assert_eq!(cut.get_pixel(5, 0).0[0], 0);
        assert!(polyline(&[(1.0, 2.0)], |x, y| (x * 2.0, y * 2.0)).starts_with("M 2.0 4.0 L 2.0"));
        assert!(ellipse_path(10.0, 10.0, 5.0, 4.0).contains("A 5.0 4.0"));
    }
}
