//! App icons drawn as the shell draws them (shell/common/AppIcon.qml,
//! IconAnalyzer.qml and icon.frag): the icon file decoded (SVG or PNG),
//! measured once at 32×32 (is it already a tile, the plate or tile its logo
//! sits on, the logo's tones and box), then styled pixel by pixel with the
//! shader's maths: on the squircle tile (Settings → Appearance → Icon
//! shape), and in the look's style (Clear in Glass, Themed in Material, or
//! default, dark, tinted). Slint has no custom shaders, so this runs on the
//! CPU, once per icon and size, off the UI thread; the result is a plain
//! image for Slint.

use serde_json::Value;
use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::path::Path;

/// How icons are drawn: from Settings (appearance.iconStyle, iconShape,
/// iconTile, iconContrast) and the look's colours
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Style {
    /// 0 default, 1 dark, 2 clear, 3 tinted, 4 themed (icon.frag's modes)
    pub mode: u8,
    /// on the squircle tile (themed: a circle)
    pub tiled: bool,
    pub dark: bool,
    /// iconContrast, in hundredths
    pub contrast: u8,
    /// the logo's colour in tinted (the accent) and themed (onPrimaryContainer)
    pub tint: [u8; 3],
    /// themed: the tile (primaryContainer); else the tile is chosen per icon
    pub tile: Option<[u8; 3]>,
    /// iconTile: auto (from the logo) | neutral | accent
    pub tile_from: String,
    pub accent: [u8; 3],
}

impl Style {
    /// AppIcon.qml's mode and tiling for these settings. `material`: the look;
    /// `accent`: Theme.accent; `themed`: Material's (primaryContainer,
    /// onPrimaryContainer)
    pub fn from_settings(cfg: &Value, material: bool, dark: bool, accent: [u8; 3], themed: ([u8; 3], [u8; 3])) -> Self {
        let a = &cfg["appearance"];
        let s = a["iconStyle"].as_str().unwrap_or("look");
        let style = match s {
            "look" | "default" => if material { "themed" } else { "clear" },
            "auto" => if dark { "dark" } else { "default" },
            s => s,
        };
        // ("default" was renamed "look": Config.qml moves it on load)
        let mode = match style {
            "dark" => 1,
            "clear" => 2,
            "tinted" => 3,
            "themed" if material => 4,
            _ => 0,
        };
        let tiled = a["iconShape"].as_str().unwrap_or("squircle") == "squircle" || mode == 4;
        let contrast = (a["iconContrast"].as_f64().unwrap_or(0.6).clamp(0.0, 1.0) * 100.0).round() as u8;
        Style {
            mode,
            tiled,
            dark,
            contrast,
            tint: if mode == 4 { themed.1 } else { accent },
            tile: (mode == 4).then_some(themed.0),
            tile_from: a["iconTile"].as_str().unwrap_or("auto").to_string(),
            accent,
        }
    }

    /// Nothing to do: the icon as it is
    pub fn plain(&self) -> bool {
        self.mode == 0 && !self.tiled
    }
}

// ── Decoding ────────────────────────────────────────────────────────────────

/// An image as premultiplied RGBA floats (0..1), row by row
#[derive(Clone)]
pub struct Img {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[f32; 4]>,
}

impl Img {
    /// Bilinear, transparent outside (uv in 0..1)
    pub fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let x = u * self.w as f32 - 0.5;
        let y = v * self.h as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let at = |xi: f32, yi: f32| -> [f32; 4] {
            if xi < 0.0 || yi < 0.0 || xi >= self.w as f32 || yi >= self.h as f32 {
                [0.0; 4]
            } else {
                self.px[yi as usize * self.w + xi as usize]
            }
        };
        let (a, b, c, d) = (at(x0, y0), at(x0 + 1.0, y0), at(x0, y0 + 1.0), at(x0 + 1.0, y0 + 1.0));
        let mut out = [0.0; 4];
        for i in 0..4 {
            out[i] = (a[i] * (1.0 - fx) + b[i] * fx) * (1.0 - fy) + (c[i] * (1.0 - fx) + d[i] * fx) * fy;
        }
        out
    }
}

/// An icon file at `size`×`size` (kept square, centred, as Qt's icon provider)
pub fn load(path: &Path, size: u32) -> Option<Img> {
    let size = size.clamp(1, 1024);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "svg" || ext == "svgz" {
        use resvg::{tiny_skia, usvg};
        let data = std::fs::read(path).ok()?;
        let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).ok()?;
        let (w, h) = (tree.size().width(), tree.size().height());
        let k = size as f32 / w.max(h).max(1.0);
        let mut pix = tiny_skia::Pixmap::new(size, size)?;
        let t = tiny_skia::Transform::from_scale(k, k).post_translate((size as f32 - w * k) / 2.0, (size as f32 - h * k) / 2.0);
        resvg::render(&tree, t, &mut pix.as_mut());
        let px = pix.pixels().iter().map(|p| [p.red() as f32 / 255.0, p.green() as f32 / 255.0, p.blue() as f32 / 255.0, p.alpha() as f32 / 255.0]).collect();
        return Some(Img { w: size as usize, h: size as usize, px });
    }
    let src = image::open(path).ok()?.to_rgba8();
    let (w, h) = src.dimensions();
    let k = size as f32 / w.max(h).max(1) as f32;
    let (nw, nh) = (((w as f32 * k).round() as u32).max(1), ((h as f32 * k).round() as u32).max(1));
    // (premultiplied before scaling: no dark fringes from transparent pixels)
    let mut pre = src.clone();
    for p in pre.pixels_mut() {
        let a = p[3] as u32;
        for c in 0..3 {
            p[c] = (p[c] as u32 * a / 255) as u8;
        }
    }
    let scaled = image::imageops::resize(&pre, nw, nh, image::imageops::FilterType::Triangle);
    let mut img = Img { w: size as usize, h: size as usize, px: vec![[0.0; 4]; (size * size) as usize] };
    let (ox, oy) = ((size - nw) / 2, (size - nh) / 2);
    for (x, y, p) in scaled.enumerate_pixels() {
        img.px[((y + oy) * size + x + ox) as usize] = [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, p[3] as f32 / 255.0];
    }
    Some(img)
}

// ── Measuring (IconAnalyzer.qml) ────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct Info {
    pub square: bool,
    pub plate: bool,
    /// the background behind the logo, top and bottom (0..255)
    pub bg_top: [f32; 3],
    pub bg_bottom: [f32; 3],
    pub floor: f32,
    pub edge: f32,
    pub dominant: [f32; 3],
    pub lo: f32,
    pub hi: f32,
    pub med: f32,
    pub pol: f32,
    /// the opaque part's box and the logo's own (0..1: left, top, right, bottom)
    pub bbox: [f32; 4],
    pub gbox: Option<[f32; 4]>,
}

fn pct(list: &[f32], p: f32) -> f32 {
    if list.is_empty() {
        return 0.0;
    }
    let mut v = list.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    v[((p * (v.len() - 1) as f32).round() as usize).min(v.len() - 1)]
}

/// An icon measured on a 32×32 canvas (straight RGBA, 0..255), as IconAnalyzer
pub fn analyze(d: &[[u8; 4]]) -> Info {
    assert_eq!(d.len(), 1024);
    let px = |x: i32, y: i32| -> [f32; 4] {
        let p = d[(y.clamp(0, 31) * 32 + x.clamp(0, 31)) as usize];
        [p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32]
    };
    let solid = |x: i32, y: i32| d[(y * 32 + x) as usize][3] >= 200;
    let luma = |p: [f32; 4]| (0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]) / 255.0;
    let dist3 = |p: [f32; 4], q: [f32; 3]| ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt() / 255.0;

    let probes = [(4, 4), (27, 4), (4, 27), (27, 27), (16, 1), (16, 30), (1, 16), (30, 16)];
    let filled = d.iter().filter(|p| p[3] >= 200).count();
    let square = probes.iter().filter(|&&(x, y)| px(x, y)[3] > 200.0).count() >= 7 && filled as f32 >= 0.86 * 1024.0;

    let (mut opaque, mut x0, mut x1, mut y0, mut y1) = (0, 32, -1, 32, -1);
    for y in 0..32 {
        for x in 0..32 {
            if solid(x, y) {
                opaque += 1;
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    let mut rim: Vec<([f32; 4], i32)> = Vec::new();
    for i in 0..32 {
        let (mut r0, mut r1, mut c0, mut c1, mut rn, mut cn) = (-1, -1, -1, -1, 0, 0);
        for j in 0..32 {
            if solid(j, i) {
                if r0 < 0 {
                    r0 = j;
                }
                r1 = j;
                rn += 1;
            }
            if solid(i, j) {
                if c0 < 0 {
                    c0 = j;
                }
                c1 = j;
                cn += 1;
            }
        }
        if rn >= 8 {
            rim.push((px(r0 + 2, i), i));
            rim.push((px(r1 - 2, i), i));
        }
        if cn >= 8 {
            rim.push((px(i, c0 + 2), c0 + 2));
            rim.push((px(i, c1 - 2), c1 - 2));
        }
    }
    let rim_ok: Vec<([f32; 4], i32)> = rim.into_iter().filter(|e| e.0[3] >= 200.0).collect();
    let (bw, bh) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
    let simple = opaque > 0 && bw / bh >= 0.8 && bw / bh <= 1.25 && opaque as f32 / (bw * bh) >= 0.74;
    let (mut plate, mut floor) = (false, 0.0f32);
    let grey = [128.0f32; 3];
    let (mut bg_t, mut bg_b) = (grey, grey);
    if !rim_ok.is_empty() && opaque as f32 >= 0.35 * 1024.0 && (square || simple) {
        let ys: Vec<f32> = rim_ok.iter().map(|e| e.1 as f32).collect();
        let (ya, yb, t33, t67) = (pct(&ys, 0.15), pct(&ys, 0.85), pct(&ys, 0.33), pct(&ys, 0.67));
        let top: Vec<[f32; 4]> = rim_ok.iter().filter(|e| e.1 as f32 <= t33).map(|e| e.0).collect();
        let bot: Vec<[f32; 4]> = rim_ok.iter().filter(|e| e.1 as f32 >= t67).map(|e| e.0).collect();
        let all: Vec<[f32; 4]> = rim_ok.iter().map(|e| e.0).collect();
        let med3 = |l: &[[f32; 4]]| -> [f32; 3] { [0, 1, 2].map(|c| pct(&l.iter().map(|p| p[c]).collect::<Vec<_>>(), 0.5)) };
        let mt = med3(if top.is_empty() { &all } else { &top });
        let mb = med3(if bot.is_empty() { &all } else { &bot });
        let at = |y: f32| -> [f32; 3] {
            let t = if yb == ya { 0.0 } else { ((y - ya) / (yb - ya)).clamp(0.0, 1.0) };
            [0, 1, 2].map(|c| mt[c] + (mb[c] - mt[c]) * t)
        };
        let dev: Vec<f32> = rim_ok.iter().map(|e| dist3(e.0, at(e.1 as f32))).collect();
        if pct(&dev, 0.6) < 0.16 {
            plate = !square;
            bg_t = at(0.0);
            bg_b = at(31.0);
            floor = pct(&dev, 0.85).min(0.3);
        }
    }
    if square && bg_t == grey && floor == 0.0 {
        let cs: Vec<[f32; 4]> = probes[..4].iter().map(|&(x, y)| px(x, y)).collect();
        bg_t = [0, 1, 2].map(|c| cs.iter().map(|p| p[c]).sum::<f32>() / 4.0);
        bg_b = bg_t;
        floor = 0.15;
    }
    let bg_at = |y: f32| -> [f32; 3] { [0, 1, 2].map(|c| bg_t[c] + (bg_b[c] - bg_t[c]) * y / 31.0) };
    let on_tile = square || plate;
    let (mut tones, mut gaps) = (Vec::new(), Vec::new());
    let (mut gx0, mut gx1, mut gy0, mut gy1) = (32, -1, 32, -1);
    let (mut r, mut g, mut b, mut w) = (0.0, 0.0, 0.0, 0.0);
    for y in 0..32 {
        for x in 0..32 {
            let p = px(x, y);
            if p[3] < 128.0 {
                continue;
            }
            let (mx, mn) = (p[0].max(p[1]).max(p[2]), p[0].min(p[1]).min(p[2]));
            let wt = 0.15 + (mx - mn) / 255.0;
            r += p[0] * wt;
            g += p[1] * wt;
            b += p[2] * wt;
            w += wt;
            let logo = if on_tile {
                let dd = dist3(p, bg_at(y as f32));
                if dd > floor + 0.08 && p[3] >= 230.0 {
                    gaps.push(dd);
                    true
                } else {
                    false
                }
            } else {
                true
            };
            if logo {
                tones.push(luma(p));
                gx0 = gx0.min(x);
                gx1 = gx1.max(x);
                gy0 = gy0.min(y);
                gy1 = gy1.max(y);
            }
        }
    }
    let many = tones.len() >= 12;
    let med = if many { pct(&tones, 0.5) } else { 0.5 };
    let mid = bg_at(16.0);
    Info {
        square,
        plate,
        bg_top: bg_t,
        bg_bottom: bg_b,
        floor,
        edge: if gaps.len() >= 12 { pct(&gaps, 0.7).max(0.15) } else { 0.3 },
        dominant: if w > 0.0 { [r / w, g / w, b / w] } else { grey },
        lo: if many { pct(&tones, 0.1) } else { 0.0 },
        hi: if many { pct(&tones, 0.9) } else { 1.0 },
        med,
        pol: if on_tile && med < (0.2126 * mid[0] + 0.7152 * mid[1] + 0.0722 * mid[2]) / 255.0 { -1.0 } else { 1.0 },
        bbox: if opaque > 0 { [x0 as f32 / 32.0, y0 as f32 / 32.0, (x1 + 1) as f32 / 32.0, (y1 + 1) as f32 / 32.0] } else { [0.0, 0.0, 1.0, 1.0] },
        gbox: (gx1 >= gx0 && many).then(|| [gx0 as f32 / 32.0, gy0 as f32 / 32.0, (gx1 + 1) as f32 / 32.0, (gy1 + 1) as f32 / 32.0]),
    }
}

/// Where a logo goes on its tile (AppIcon.qml's fit): scale and centre
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub scale: f32,
    pub x: f32,
    pub y: f32,
    pub glass: bool,
}

pub fn fit(info: &Info, mode: u8) -> Fit {
    let place = |b: [f32; 4], glass: bool| {
        let (w, h) = ((b[2] - b[0]).max(0.05), (b[3] - b[1]).max(0.05));
        Fit { scale: (0.46 / h).min(0.72 / w).min(4.0), x: (b[0] + b[2]) / 2.0, y: (b[1] + b[3]) / 2.0, glass }
    };
    if mode >= 2 && (info.square || info.plate) {
        return place(info.gbox.unwrap_or(info.bbox), true);
    }
    if info.square {
        return Fit { scale: 0.7, x: 0.5, y: 0.5, glass: false };
    }
    if info.plate {
        let b = info.bbox;
        return Fit { scale: 1.06 / (b[2] - b[0]).max(b[3] - b[1]).max(0.1), x: (b[0] + b[2]) / 2.0, y: (b[1] + b[3]) / 2.0, glass: false };
    }
    place(info.bbox, false)
}

// ── Styling (icon.frag) ─────────────────────────────────────────────────────

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}
fn luma3(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}
fn rgb01(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| v as f32 / 255.0)
}

/// The tile behind a transparent logo (AppIcon.qml's tileFor)
pub fn tile_for(style: &Style, dominant: [f32; 3]) -> [f32; 3] {
    if let Some(t) = style.tile {
        return rgb01(t);
    }
    if style.tile_from == "neutral" {
        return if style.dark { rgb01([0x2c, 0x2c, 0x31]) } else { [1.0; 3] };
    }
    let c = if style.tile_from == "accent" { rgb01(style.accent) } else { dominant.map(|v| v / 255.0) };
    if style.dark {
        [c[0] * 0.32 + 0.07, c[1] * 0.32 + 0.07, c[2] * 0.32 + 0.08]
    } else {
        [c[0] * 0.16 + 0.84, c[1] * 0.16 + 0.84, c[2] * 0.16 + 0.85]
    }
}

/// The icon in its style, `size`×`size`, straight RGBA (icon.frag, pixel by pixel)
pub fn style(src: &Img, info: &Info, s: &Style, size: usize) -> Vec<[u8; 4]> {
    let dark = s.dark;
    let contrast = s.contrast as f32 / 100.0;
    let tint = rgb01(s.tint);
    let mode = s.mode;
    let fit = fit(info, mode);
    let tile_color = tile_for(s, info.dominant);
    let bg = info.bg_top.map(|v| v / 255.0);
    let bg2 = info.bg_bottom.map(|v| v / 255.0);
    let shade = |lum: f32| (0.75 + 0.5 * ((lum - info.med) / (info.hi - info.lo).max(0.12) * info.pol)).clamp(0.0, 1.0);
    let r = 0.2 + (0.65 - 0.2) * contrast;
    let clear_glyph = |n: f32| if dark { [1.0 - 0.6 * r * (1.0 - n); 3] } else { [0.1 + 0.9 * r * (1.0 - n); 3] };
    let tint_glyph = |n: f32| {
        if dark {
            mix3(tint.map(|v| v * (1.0 - 0.6 * r)), mix3(tint, [1.0; 3], 0.35), n)
        } else {
            mix3(mix3(tint, [1.0; 3], 0.45 * r), tint.map(|v| v * 0.55), n)
        }
    };
    let unpremul = |c: [f32; 4]| if c[3] > 0.0 { [c[0] / c[3], c[1] / c[3], c[2] / c[3], c[3]] } else { [0.0; 4] };
    let n = if mode == 4 { 2.0f32 } else { 5.0 };
    let dist = |x: f32, y: f32| (x.abs().powf(n) + y.abs().powf(n)).powf(1.0 / n);
    let step = 2.0 / size as f32;
    let mut out = vec![[0u8; 4]; size * size];
    for py in 0..size {
        for px in 0..size {
            let (u, v) = ((px as f32 + 0.5) / size as f32, (py as f32 + 0.5) / size as f32);
            // (premultiplied, as the shader writes it)
            let c: [f32; 4] = if !s.tiled {
                let t = src.sample(u, v);
                if t[3] <= 0.0 || mode == 0 {
                    t
                } else {
                    let p = unpremul(t);
                    let rgb = [p[0], p[1], p[2]];
                    let lum = luma3(rgb);
                    let mut a = p[3];
                    let col = match mode {
                        1 => {
                            let sat = rgb[0].max(rgb[1]).max(rgb[2]) - rgb[0].min(rgb[1]).min(rgb[2]);
                            let bgk = smoothstep(0.6, 0.85, lum) * (1.0 - smoothstep(0.12, 0.3, sat));
                            mix3(rgb.map(|x| (x * 1.12 + 0.03).min(1.0)), mix3([0.16, 0.16, 0.18], [0.24, 0.24, 0.27], lum), bgk)
                        }
                        2 => {
                            a *= 0.85 + 0.15 * contrast;
                            clear_glyph(shade(lum))
                        }
                        _ => {
                            a *= 0.85 + 0.15 * contrast;
                            tint_glyph(shade(lum))
                        }
                    };
                    [col[0] * a, col[1] * a, col[2] * a, a]
                }
            } else {
                let (x, y) = (u * 2.0 - 1.0, v * 2.0 - 1.0);
                let d = dist(x, y);
                let aa = ((dist(x + step, y) - d).abs() + (dist(x, y + step) - d).abs()).max(1e-3);
                let shape = 1.0 - smoothstep(1.0 - aa * 1.5, 1.0, d);
                if shape <= 0.0 {
                    [0.0; 4]
                } else {
                    let (base, art, vy) = if info.square && !fit.glass {
                        let (su, sv) = ((u - 0.5) * 0.9 + 0.5, (v - 0.5) * 0.9 + 0.5);
                        (mix3(bg, bg2, 0.5), unpremul(src.sample(su, sv)), sv)
                    } else {
                        let (su, sv) = ((u - 0.5) / fit.scale + fit.x, (v - 0.5) / fit.scale + fit.y);
                        let inside = (0.0..=1.0).contains(&su) && (0.0..=1.0).contains(&sv);
                        (tile_color, if inside { unpremul(src.sample(su, sv)) } else { [0.0; 4] }, sv)
                    };
                    let art_rgb = [art[0], art[1], art[2]];
                    let mut rgb = mix3(base, art_rgb, art[3]);
                    let mut glyph = art[3];
                    if info.square || info.plate {
                        let behind = mix3(bg, bg2, vy.clamp(0.0, 1.0));
                        let dd = ((art_rgb[0] - behind[0]).powi(2) + (art_rgb[1] - behind[1]).powi(2) + (art_rgb[2] - behind[2]).powi(2)).sqrt();
                        glyph *= smoothstep(info.floor + 0.04, info.floor + (info.edge * 0.45).max(0.1), dd);
                        if fit.glass {
                            glyph *= smoothstep(0.85, 0.98, art[3]);
                        }
                    }
                    let lum_n = shade(luma3(art_rgb));
                    let rim_shade = 0.5 + 0.5 * ((-x - y) * 0.7).clamp(-1.0, 1.0);
                    let rim = smoothstep(0.86, 0.995, d) * rim_shade;
                    match mode {
                        1 => {
                            let tile_dark = mix3([0.21, 0.21, 0.23], [0.13, 0.13, 0.15], v);
                            let keep = if info.square { (glyph * 1.4).clamp(0.0, 1.0) } else { art[3] };
                            rgb = mix3(tile_dark, art_rgb.map(|c| (c * 1.1 + 0.04).min(1.0)), keep).map(|c| c + rim * 0.10);
                            let c = rgb.map(|c| c.clamp(0.0, 1.0));
                            [c[0] * shape, c[1] * shape, c[2] * shape, shape]
                        }
                        4 => {
                            let g = glyph * (0.85 + 0.15 * contrast);
                            let ink = tint.map(|t| t * (0.72 + 0.28 * lum_n));
                            let c = mix3(tile_color, ink, g).map(|c| c.clamp(0.0, 1.0));
                            [c[0] * shape, c[1] * shape, c[2] * shape, shape]
                        }
                        2 | 3 => {
                            let mut tile_c = if dark { [0.16, 0.17, 0.19] } else { [0.96, 0.96, 0.98] };
                            let a = if dark { 0.42 } else { 0.5 } + 0.08 * (1.0 - v);
                            let ink = if mode == 2 {
                                clear_glyph(lum_n)
                            } else {
                                tile_c = mix3(tile_c, tint, 0.12);
                                tint_glyph(lum_n)
                            };
                            let g = glyph * (0.85 + 0.15 * contrast);
                            let c = mix3(tile_c, ink, g).map(|c| (c + rim * if dark { 0.3 } else { 0.12 }).clamp(0.0, 1.0));
                            let alpha = (a + (1.0 - a) * g + rim * 0.3).min(1.0) * shape;
                            [c[0] * alpha, c[1] * alpha, c[2] * alpha, alpha]
                        }
                        _ => {
                            let c = rgb.map(|c| (c + rim * 0.06).clamp(0.0, 1.0));
                            [c[0] * shape, c[1] * shape, c[2] * shape, shape]
                        }
                    }
                }
            };
            let a = c[3].clamp(0.0, 1.0);
            out[py * size + px] = if a <= 0.0 {
                [0; 4]
            } else {
                let q = |x: f32| ((x / a).clamp(0.0, 1.0) * 255.0).round() as u8;
                [q(c[0]), q(c[1]), q(c[2]), (a * 255.0).round() as u8]
            };
        }
    }
    out
}

/// A symbolic icon in one colour (as a theme draws them, in its text colour)
fn inked(mut img: Img, ink: Option<[u8; 3]>) -> Img {
    if let Some(c) = ink {
        let c = rgb01(c);
        for p in img.px.iter_mut() {
            *p = [c[0] * p[3], c[1] * p[3], c[2] * p[3], p[3]];
        }
    }
    img
}

/// An icon file, drawn at `size` pixels in a style, ready for Slint
/// (`ink`: a symbolic icon's colour)
pub fn render(path: &Path, size: u32, s: &Style, ink: Option<[u8; 3]>) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
    let size = size.clamp(8, 512);
    let load = |p: &Path, n: u32| load(p, n).map(|i| inked(i, ink));
    let pixels: Vec<[u8; 4]> = if s.plain() {
        let img = load(path, size)?;
        img.px.iter().map(|c| if c[3] <= 0.0 { [0; 4] } else { [0, 1, 2, 3].map(|i| ((if i == 3 { c[3] } else { c[i] / c[3] }).clamp(0.0, 1.0) * 255.0).round() as u8) }).collect()
    } else {
        // measured small, drawn from a sharper copy (logos are scaled up on tiles)
        let small = load(path, 32)?;
        let small: Vec<[u8; 4]> = small.px.iter().map(|c| if c[3] <= 0.0 { [0; 4] } else { [0, 1, 2, 3].map(|i| ((if i == 3 { c[3] } else { c[i] / c[3] }).clamp(0.0, 1.0) * 255.0).round() as u8) }).collect();
        let info = analyze(&small);
        let src = load(path, (size * 2).min(1024))?;
        style(&src, &info, s, size as usize)
    };
    let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(size, size);
    for (o, p) in buf.make_mut_slice().iter_mut().zip(pixels) {
        *o = Rgba8Pixel { r: p[0], g: p[1], b: p[2], a: p[3] };
    }
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ACCENT: [u8; 3] = [0x0a, 0x84, 0xff];
    const THEMED: ([u8; 3], [u8; 3]) = ([0x30, 0x40, 0x60], [0xd0, 0xe0, 0xff]);

    #[test]
    fn each_look_has_its_own_style_by_default() {
        let g = Style::from_settings(&json!({}), false, true, ACCENT, THEMED);
        assert_eq!((g.mode, g.tiled), (2, true)); // Glass: Clear, on the squircle tile
        let m = Style::from_settings(&json!({}), true, true, ACCENT, THEMED);
        assert_eq!((m.mode, m.tiled, m.tint, m.tile), (4, true, THEMED.1, Some(THEMED.0))); // Material: Themed
        let plain = Style::from_settings(&json!({"appearance": {"iconStyle": "default", "iconShape": "original"}}), false, true, ACCENT, THEMED);
        assert_eq!(plain.mode, 2); // ("default" is the old name of "look")
        let plain = Style::from_settings(&json!({"appearance": {"iconStyle": "auto", "iconShape": "original"}}), false, false, ACCENT, THEMED);
        assert!(plain.plain());
        // themed means nothing in Glass (no Material colours there)
        assert_eq!(Style::from_settings(&json!({"appearance": {"iconStyle": "themed"}}), false, true, ACCENT, THEMED).mode, 0);
        assert_eq!(Style::from_settings(&json!({"appearance": {"iconContrast": 0.25}}), false, true, ACCENT, THEMED).contrast, 25);
    }

    fn canvas(f: impl Fn(i32, i32) -> [u8; 4]) -> Vec<[u8; 4]> {
        (0..1024).map(|i| f(i % 32, i / 32)).collect()
    }

    #[test]
    fn a_full_tile_is_square_and_its_background_is_found() {
        // a blue tile with a white bar in the middle (the logo)
        let d = canvas(|x, y| if (12..20).contains(&x) && (8..24).contains(&y) { [255, 255, 255, 255] } else { [20, 60, 200, 255] });
        let i = analyze(&d);
        assert!(i.square && !i.plate);
        assert!((i.bg_top[2] - 200.0).abs() < 1.0);
        assert!(i.med > 0.9); // the logo is the white
        let g = i.gbox.unwrap();
        assert_eq!(g, [12.0 / 32.0, 8.0 / 32.0, 20.0 / 32.0, 24.0 / 32.0]);
    }

    #[test]
    fn a_disc_is_a_plate_and_a_bare_logo_is_neither() {
        let disc = canvas(|x, y| {
            let (dx, dy) = (x as f32 - 15.5, y as f32 - 15.5);
            if dx * dx + dy * dy < 14.0 * 14.0 {
                if dx.abs() < 3.0 { [250, 250, 250, 255] } else { [200, 40, 40, 255] }
            } else {
                [0; 4]
            }
        });
        let i = analyze(&disc);
        assert!(i.plate && !i.square);
        // a thin diagonal stroke: no background of its own
        let stroke = canvas(|x, y| if (x - y).abs() <= 1 { [30, 30, 30, 255] } else { [0; 4] });
        let i = analyze(&stroke);
        assert!(!i.plate && !i.square);
        let f = fit(&i, 0);
        assert!(f.scale < 1.0 && (f.x - 0.5).abs() < 0.02);
    }

    #[test]
    fn styled_tiles_have_their_shape() {
        let src = Img { w: 64, h: 64, px: vec![[0.1, 0.2, 0.8, 1.0]; 64 * 64] };
        let info = analyze(&canvas(|_, _| [25, 51, 204, 255]));
        let clear = Style::from_settings(&json!({}), false, true, ACCENT, THEMED);
        let out = style(&src, &info, &clear, 48);
        // corners outside the squircle are clear; the middle is glass, see-through but there
        assert_eq!(out[0][3], 0);
        let mid = out[24 * 48 + 24];
        assert!(mid[3] > 90 && mid[3] < 255, "{mid:?}");
        // themed: an opaque circle in the tile colour
        let themed = Style::from_settings(&json!({}), true, true, ACCENT, THEMED);
        let out = style(&src, &info, &themed, 48);
        assert_eq!(out[24 * 48 + 24][3], 255);
        assert_eq!(out[2 * 48 + 2][3], 0); // (a circle leaves more of the corner out)
    }
}
