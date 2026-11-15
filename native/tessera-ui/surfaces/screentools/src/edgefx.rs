//! Circle to Search's screen-edge light, drawn here a frame at a time (the
//! looks Slint's shapes can't make: fields of light, a dot grid). Setting
//! `circle.glow`: "dots" (the default), "siri", "gemini", "ripple",
//! "vignette", or "bands" (the gradient bands in circle.slint, not drawn here).
//!
//! Two pictures, stretched over the screen: `soft`, a quarter of the
//! screen's size (the light is soft, so that's plenty), and for the dot grid
//! `dots`, half its size, the dots splatted one by one. Each pixel is the
//! light premultiplied with its brightest channel as alpha, which over the
//! frozen screen comes out close to a "screen" blend.
//!
//! The formulas are the previews' (tessera-verify/c2s-styles/render.py), in
//! its units: the screen's height is 540 of them, whatever its size.

use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::f32::consts::TAU;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Style {
    Bands,
    #[default]
    Dots,
    Siri,
    Gemini,
    Ripple,
    Vignette,
}

impl Style {
    pub fn from_cfg(s: &str) -> Style {
        match s {
            "bands" => Style::Bands,
            "siri" => Style::Siri,
            "gemini" => Style::Gemini,
            "ripple" => Style::Ripple,
            "vignette" => Style::Vignette,
            _ => Style::Dots,
        }
    }
    /// The index circle.slint's `edge-style` takes (0: its own bands)
    pub fn index(self) -> i32 {
        match self {
            Style::Bands => 0,
            Style::Dots => 1,
            Style::Siri => 2,
            Style::Gemini => 3,
            Style::Ripple => 4,
            Style::Vignette => 5,
        }
    }
}

/// The settings' list: (config value, label)
pub const CHOICES: [(&str, &str); 6] = [
    ("dots", "Dot ripple"),
    ("siri", "Glow"),
    ("gemini", "Pool"),
    ("ripple", "Ripple to rim"),
    ("vignette", "Colour vignette"),
    ("bands", "Bands"),
];

/// The previews' loop: wave terms make whole cycles in it
const T: f32 = 4.0;
const H_UNITS: f32 = 540.0;
/// soft texels per logical px (a quarter), dots texels (a half)
const SOFT_DIV: f32 = 4.0;
const DOTS_DIV: f32 = 2.0;
/// the dot grid's spacing, in units
const G9: f32 = 14.0;

type Rgb = [f32; 3];
pub type Buf = SharedPixelBuffer<Rgba8Pixel>;

/// A field's value at each soft texel (fixed for a size and an origin)
#[derive(Default)]
struct Fields {
    w: usize,
    h: usize,
    /// units per texel
    step: f32,
    x: Vec<f32>,
    y: Vec<f32>,
    ds: Vec<f32>,
    s: Vec<f32>,
    rp: Vec<f32>,
    ang: Vec<f32>,
    /// the vignette's: distance to each corner, and each corner's weight
    cd: Vec<[f32; 4]>,
    cw: Vec<[f32; 4]>,
}

#[derive(Default)]
pub struct Fx {
    pub style: Style,
    /// the screen in units
    wu: f32,
    hu: f32,
    /// the origin (pointer or bottom centre), units
    px: f32,
    py: f32,
    /// logical px
    size: (f32, f32),
    f: Fields,
    pal: [Rgb; 4],
    /// the dot grid's resting sizes
    dots_w: usize,
    dots_h: usize,
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let x = ((x - a) / (b - a)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
fn ease_out(x: f32) -> f32 {
    1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3)
}
fn spring(x: f32) -> f32 {
    let x = x.max(0.0);
    1.0 - (-5.0 * x).exp() * (7.5 * x).cos()
}
/// Travelling sines: (amplitude, cycles along s, cycles per loop, phase)
fn wave(s: f32, t: f32, terms: &[(f32, f32, f32, f32)]) -> f32 {
    terms.iter().map(|(a, k, n, ph)| a * (TAU * (k * s + n * t / T) + ph).sin()).sum()
}

/// `wave` over s in 0..1, sampled (looked up per pixel)
struct Table([f32; 513]);
impl Table {
    fn new(t: f32, terms: &[(f32, f32, f32, f32)]) -> Table {
        let mut a = [0.0; 513];
        for (i, v) in a.iter_mut().enumerate() {
            *v = wave(i as f32 / 512.0, t, terms);
        }
        Table(a)
    }
    fn at(&self, s: f32) -> f32 {
        let f = s.rem_euclid(1.0) * 512.0;
        let i = (f as usize).min(511);
        let fr = f - i as f32;
        self.0[i] * (1.0 - fr) + self.0[i + 1] * fr
    }
}

const SIRI: [(f32, f32, f32, f32); 5] = [(0.32, 2.0, 1.0, 0.0), (0.24, 3.0, -2.0, 1.3), (0.16, 5.0, 3.0, 2.1), (0.10, 7.0, -2.0, 0.4), (0.06, 11.0, 3.0, 3.3)];
const GEM: [(f32, f32, f32, f32); 5] = [(0.28, 1.0, 1.0, 0.3), (0.20, 2.0, -1.0, 1.1), (0.14, 3.0, 2.0, 2.0), (0.09, 5.0, -3.0, 0.7), (0.05, 8.0, 2.0, 1.9)];
const RIM9: [(f32, f32, f32, f32); 2] = [(0.6, 3.0, -1.0, 0.0), (0.4, 5.0, 2.0, 1.0)];
const RIM13: [(f32, f32, f32, f32); 3] = [(0.5, 2.0, 1.0, 0.3), (0.3, 3.0, -2.0, 1.7), (0.2, 7.0, 1.0, 0.9)];
const EDGE14: [(f32, f32, f32, f32); 3] = [(0.5, 2.0, 1.0, 0.0), (0.3, 3.0, -1.0, 2.1), (0.2, 5.0, 2.0, 0.8)];

fn rgb(c: slint::Color) -> Rgb {
    [c.red() as f32 / 255.0, c.green() as f32 / 255.0, c.blue() as f32 / 255.0]
}
fn hex(v: u32) -> Rgb {
    [((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0]
}
fn mix(a: Rgb, b: Rgb, f: f32) -> Rgb {
    [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f]
}
fn add(a: &mut Rgb, c: Rgb, k: f32) {
    a[0] += c[0] * k;
    a[1] += c[1] * k;
    a[2] += c[2] * k;
}

impl Fx {
    /// The glass palette, or the wallpaper's Material tones
    pub fn palette(&mut self, material: Option<[slint::Color; 4]>) {
        self.pal = match material {
            // (the tones are pastel in the dark scheme: as light, they'd read
            // as white; the same hues, fuller)
            Some(c) => c.map(|c| vivid(rgb(c))),
            None => [hex(0x3d82ff), hex(0xa463ff), hex(0xff5aa8), hex(0xff9d45)],
        };
    }
    /// The cyclic palette, ph in turns
    fn pal(&self, ph: f32) -> Rgb {
        let f = ph.rem_euclid(1.0) * 4.0;
        let i = (f as usize).min(3);
        let fr = f - i as f32;
        let fr = fr * fr * (3.0 - 2.0 * fr);
        mix(self.pal[i], self.pal[(i + 1) % 4], fr)
    }

    /// A screen of `w`×`h` logical px, the light starting at (ox, oy)
    pub fn setup(&mut self, style: Style, w: f32, h: f32, ox: f32, oy: f32) {
        self.style = style;
        let u = h / H_UNITS;
        self.size = (w, h);
        self.wu = w / u;
        self.hu = H_UNITS;
        self.px = ox / u;
        self.py = oy / u;
        let sw = (w / SOFT_DIV).ceil().max(1.0) as usize;
        let sh = (h / SOFT_DIV).ceil().max(1.0) as usize;
        let step = SOFT_DIV / u;
        let (wu, hu) = (self.wu, self.hu);
        let per = 2.0 * (wu + hu);
        let mut f = Fields { w: sw, h: sh, step, ..Default::default() };
        let n = sw * sh;
        for v in [&mut f.x, &mut f.y, &mut f.ds, &mut f.s, &mut f.rp, &mut f.ang] {
            v.reserve(n);
        }
        let corners = [(0.0, 0.0), (wu, 0.0), (wu, hu), (0.0, hu)];
        for j in 0..sh {
            for i in 0..sw {
                let x = (i as f32 + 0.5) * step;
                let y = (j as f32 + 0.5) * step;
                f.x.push(x);
                f.y.push(y);
                f.ds.push(soft_dist(x, y, wu, hu));
                f.s.push(perimeter(x, y, wu, hu, per));
                f.rp.push((x - self.px).hypot(y - self.py));
                f.ang.push((y - self.py).atan2(x - self.px));
                if style == Style::Vignette {
                    let cd = corners.map(|(cx, cy)| (x - cx).hypot(y - cy));
                    let w4 = cd.map(|d| (-(d / 600.0).powi(2)).exp());
                    let sum: f32 = w4.iter().sum::<f32>() + 1e-4;
                    f.cd.push(cd);
                    f.cw.push(w4.map(|v| v / sum));
                }
            }
        }
        self.f = f;
        self.dots_w = (w / DOTS_DIV).ceil().max(1.0) as usize;
        self.dots_h = (h / DOTS_DIV).ceil().max(1.0) as usize;
    }

    /// The pictures at `t` s since it opened: (soft, dots), premultiplied
    pub fn frame(&self, t: f32) -> (Buf, Option<Buf>) {
        let mut soft = SharedPixelBuffer::<Rgba8Pixel>::new(self.f.w as u32, self.f.h as u32);
        let mut dots = None;
        match self.style {
            Style::Bands => {}
            Style::Siri => self.siri(t, soft.make_mut_slice()),
            Style::Gemini => self.gemini(t, soft.make_mut_slice()),
            Style::Ripple => self.ripple(t, soft.make_mut_slice()),
            Style::Vignette => self.vignette(t, soft.make_mut_slice()),
            Style::Dots => {
                let mut d = SharedPixelBuffer::<Rgba8Pixel>::new(self.dots_w as u32, self.dots_h as u32);
                self.dot_grid(t, soft.make_mut_slice(), d.make_mut_slice());
                dots = Some(d);
            }
        }
        (soft, dots)
    }

    fn siri(&self, t: f32, out: &mut [Rgba8Pixel]) {
        let g = spring(t / 0.8);
        let op = smoothstep(0.0, 0.25, t);
        let flash = 1.0 + 0.5 * (-((t - 0.35) / 0.25).powi(2)).exp();
        let tab = Table::new(t, &SIRI);
        let f = &self.f;
        for k in 0..out.len() {
            let ds = f.ds[k];
            if ds > 200.0 {
                continue;
            }
            let w = tab.at(f.s[k]);
            let depth = (30.0 * (1.0 + 0.75 * w) * g).max(0.5);
            // (past 3.2 depths the light is under 1/255)
            if ds > 3.2 * depth && ds > 15.0 {
                continue;
            }
            let i = (-(ds / depth).powf(1.7)).exp();
            let crest = 0.65 + 0.45 * w.clamp(-1.0, 1.0);
            let col = self.pal(f.s[k] + t / T + 0.10 * w + ds / 140.0);
            let core = (-ds / 2.5).exp() * 0.35 * g * op;
            let a = i * crest * 0.95 * flash * op;
            out[k] = px([col[0] * a + core, col[1] * a + core, col[2] * a + core]);
        }
    }

    fn gemini(&self, t: f32, out: &mut [Rgba8Pixel]) {
        let rise = ease_out(t / 1.0);
        let op = smoothstep(0.0, 0.2, t);
        let breathe = 0.88 + 0.12 * (TAU * 2.0 * t / T + 0.4).sin();
        let f = &self.f;
        let (wu, hu) = (self.wu, self.hu);
        // per column: the pool's height and colour
        let blobs: [(u32, f32, f32, f32, f32); 5] = [(0x3d82ff, 1.0, 0.0, 0.25, 0.22), (0x6fb6ff, -1.0, 1.7, 0.75, 0.2), (0xa463ff, 2.0, 2.9, 0.5, 0.35), (0xff5aa8, -1.0, 4.1, 0.6, 0.3), (0x3d82ff, 1.0, 5.0, 0.85, 0.15)];
        let material = self.pal[0] != hex(0x3d82ff);
        let cols: Vec<(f32, Rgb)> = (0..f.w)
            .map(|i| {
                let x = f.x[i];
                let hw = wave(x / wu, t, &GEM);
                let h = (80.0 * (1.0 + 0.7 * hw) * breathe * rise).max(0.5);
                let (mut num, mut den) = ([0.0f32; 3], 1e-4f32);
                for (j, (c, n, ph, cx0, amp)) in blobs.iter().enumerate() {
                    let c = if material { self.pal[[0, 1, 2, 3, 0][j]] } else { hex(*c) };
                    let bx = wu * (cx0 + amp * (TAU * n * t / T + ph).sin());
                    let wg = (-((x - bx) / 170.0).powi(2)).exp();
                    add(&mut num, c, wg);
                    den += wg;
                }
                (h, num.map(|v| v / den))
            })
            .collect();
        let side_fall = 240.0 * rise + 1.0;
        let haze_fall = 260.0 * rise + 1.0;
        for k in 0..out.len() {
            let i = k % f.w;
            let (h, col) = cols[i];
            let x = f.x[k];
            let yb = hu - f.y[k];
            let pool = if yb < 6.0 * h { (-(yb / h).powf(1.5)).exp() } else { 0.0 };
            let side = if x < 80.0 || wu - x < 80.0 {
                ((-(x / 16.0).powf(1.3)).exp() + (-((wu - x).max(0.0) / 16.0).powf(1.3)).exp()) * (-yb / side_fall).exp() * 0.55
            } else {
                0.0
            };
            let haze = (-yb / haze_fall).exp() * 0.10;
            let a = (pool * 0.95 + side + haze) * op;
            if a < 0.002 {
                continue;
            }
            out[k] = px(col.map(|c| c * a));
        }
    }

    fn dot_grid(&self, t: f32, soft: &mut [Rgba8Pixel], dots: &mut [Rgba8Pixel]) {
        let r = t * 620.0;
        let fade = (-t / 1.6).exp() * smoothstep(0.0, 0.08, t);
        let base = 0.11 * (-t / 1.8).exp() + 0.035;
        let tab = Table::new(t, &RIM9);
        let ring_at = |rp: f32| {
            let rd = rp - r;
            ((-(rd / 45.0).powi(2)).exp() + 0.45 * (-((rd + 110.0) / 40.0).powi(2)).exp()) * fade
        };
        let rim_at = |ds: f32, s: f32, rp: f32| 1.5 * (-ds / 34.0).exp() * smoothstep(-60.0, 60.0, r - rp) * (0.7 + 0.3 * tab.at(s));
        // the soft glow under the dots
        let f = &self.f;
        let ring_on = fade > 0.002;
        for k in 0..soft.len() {
            let rd = f.rp[k] - r;
            let ring = if ring_on && rd > -260.0 && rd < 140.0 { ring_at(f.rp[k]) } else { 0.0 };
            let rim = if f.ds[k] < 160.0 && r - f.rp[k] > -60.0 { rim_at(f.ds[k], f.s[k], f.rp[k]) } else { 0.0 };
            if ring + rim < 0.01 {
                continue;
            }
            let mut c = [0.0; 3];
            add(&mut c, self.pal(f.ang[k] / TAU + 0.1), ring * 0.10);
            add(&mut c, self.pal(f.s[k] - t / T), rim * 0.10);
            soft[k] = px(c);
        }
        // the dots: each at its grid place, pushed out by the ripple
        let (wu, hu) = (self.wu, self.hu);
        let per = 2.0 * (wu + hu);
        let dstep = DOTS_DIV / (self.size.1 / H_UNITS); // units per dots texel
        let (dw, dh) = (self.dots_w as i32, self.dots_h as i32);
        let nx = (wu / G9).ceil() as i32;
        let ny = (hu / G9).ceil() as i32;
        for gy in 0..ny {
            for gx in 0..nx {
                let x = (gx as f32 + 0.5) * G9;
                let y = (gy as f32 + 0.5) * G9;
                let rp = (x - self.px).hypot(y - self.py);
                let ds = soft_dist(x, y, wu, hu);
                let s = perimeter(x, y, wu, hu, per);
                let rd = rp - r;
                let ring = if ring_on && rd > -260.0 && rd < 140.0 { ring_at(rp) } else { 0.0 };
                let rim = if ds < 160.0 && r - rp > -60.0 { rim_at(ds, s, rp) } else { 0.0 };
                let disp = 9.0 * (-(rd / 45.0).powi(2)).exp() * (-rd / 45.0) * fade;
                let (ux, uy) = ((x - self.px) / (rp + 1e-3), (y - self.py) / (rp + 1e-3));
                // (the preview samples the grid at X - u·disp: the dot shows at + u·disp)
                let (cx, cy) = (x + ux * disp, y + uy * disp);
                let lit = (ring * 1.1 + rim).clamp(0.0, 1.5);
                let sigma = 0.85 + 1.4 * lit;
                let inten = base + lit * 0.95;
                let ringcol = self.pal((y - self.py).atan2(x - self.px) / TAU + 0.1);
                let rimcol = self.pal(s - t / T);
                let wsum = base + ring + rim + 1e-4;
                let mut col = [0.0; 3];
                add(&mut col, [1.0; 3], base * 0.9 / wsum);
                add(&mut col, ringcol, ring / wsum);
                add(&mut col, rimcol, rim / wsum);
                // a gaussian disc, in dots texels
                let (tx, ty) = (cx / dstep, cy / dstep);
                let sg = sigma / dstep;
                let rad = (sg * 2.2).ceil() as i32;
                let (ix, iy) = (tx.floor() as i32, ty.floor() as i32);
                for yy in (iy - rad).max(0)..=(iy + rad).min(dh - 1) {
                    for xx in (ix - rad).max(0)..=(ix + rad).min(dw - 1) {
                        let d = (xx as f32 + 0.5 - tx).hypot(yy as f32 + 0.5 - ty);
                        let a = (-(d / sg).powi(2)).exp() * inten;
                        if a < 0.004 {
                            continue;
                        }
                        let p = &mut dots[(yy * dw + xx) as usize];
                        let c = col.map(|v| v * a);
                        let prev = [p.r as f32 / 255.0, p.g as f32 / 255.0, p.b as f32 / 255.0];
                        *p = px([prev[0].max(c[0]), prev[1].max(c[1]), prev[2].max(c[2])]);
                    }
                }
            }
        }
    }

    fn ripple(&self, t: f32, out: &mut [Rgba8Pixel]) {
        let r = t * 700.0;
        let tf = smoothstep(0.0, 0.08, t) * (1.0 - smoothstep(1.2, 2.0, t));
        let tab = Table::new(t, &RIM13);
        let f = &self.f;
        for k in 0..out.len() {
            let rp = f.rp[k];
            let rd = rp - r;
            let mut rings = 0.0;
            if tf > 0.001 && rd > -200.0 && rd < 40.0 {
                let mut amp = 1.0;
                for j in 0..3 {
                    let q = rd + j as f32 * 70.0;
                    rings += (-(q / 16.0).powi(2)).exp() * amp;
                    amp *= 0.6;
                }
                rings *= (-rp / 700.0).exp() * tf;
            }
            let ds = f.ds[k];
            let mut rim = 0.0;
            if ds < 140.0 {
                let rimon = smoothstep(-40.0, 60.0, r - rp);
                let flash = (-((r - rp) / 90.0).powi(2)).exp();
                let depth = 24.0 * (1.0 + 0.4 * tab.at(f.s[k]));
                rim = (-(ds / depth).powf(1.5)).exp() * (rimon * (0.8 + 0.2 * (TAU * (2.0 * t / T + 2.0 * f.s[k])).sin()) + 0.9 * flash);
            }
            if rings + rim < 0.003 {
                continue;
            }
            let mut c = [0.0; 3];
            add(&mut c, self.pal(f.ang[k] / TAU + 0.2), rings * 0.7);
            add(&mut c, self.pal(f.s[k] - t / T), rim * 0.9);
            out[k] = px(c);
        }
    }

    fn vignette(&self, t: f32, out: &mut [Rgba8Pixel]) {
        let g = ease_out(t / 1.0);
        let on = smoothstep(0.0, 0.2, t);
        let corners: [(f32, f32); 4] = [(1.0, 0.0), (-1.0, 1.5), (2.0, 3.0), (-1.0, 4.4)];
        let cj: [Rgb; 4] = std::array::from_fn(|j| self.pal(j as f32 / 4.0 + t / T));
        let rj: [f32; 4] = std::array::from_fn(|j| 170.0 * (1.0 + 0.18 * (TAU * corners[j].0 * t / T + corners[j].1).sin()) * g + 1.0);
        let tab = Table::new(t, &EDGE14);
        let f = &self.f;
        for k in 0..out.len() {
            let mut c = [0.0; 3];
            let mut ecol = [0.0; 3];
            for j in 0..4 {
                let d = f.cd[k][j];
                if d < rj[j] * 3.0 {
                    add(&mut c, cj[j], (-(d / rj[j]).powi(2)).exp() * 0.28 * g);
                }
                add(&mut ecol, cj[j], f.cw[k][j]);
            }
            let ds = f.ds[k];
            if ds < 300.0 {
                let depth = (60.0 * (1.0 + 0.35 * tab.at(f.s[k])) * g).max(0.5);
                let edge = (-(ds / depth).powf(1.3)).exp() * on;
                add(&mut c, ecol, edge * 0.42);
            }
            if c[0] + c[1] + c[2] < 0.004 {
                continue;
            }
            out[k] = px(c);
        }
    }
}

/// The same hue with at least 0.7 saturation and 0.95 value
fn vivid(c: Rgb) -> Rgb {
    let max = c[0].max(c[1]).max(c[2]);
    let min = c[0].min(c[1]).min(c[2]);
    if max <= 0.0 || max - min < 0.02 {
        return c;
    }
    let (s, v) = (((max - min) / max).max(0.7), max.max(0.95));
    // each channel's place between the min and the max, kept
    c.map(|x| v * (1.0 - s * (1.0 - (x - min) / (max - min))))
}

/// Light as a premultiplied pixel: its brightest channel the alpha
fn px(c: Rgb) -> Rgba8Pixel {
    let r = c[0].clamp(0.0, 1.0);
    let g = c[1].clamp(0.0, 1.0);
    let b = c[2].clamp(0.0, 1.0);
    let a = r.max(g).max(b);
    Rgba8Pixel { r: (r * 255.0) as u8, g: (g * 255.0) as u8, b: (b * 255.0) as u8, a: (a * 255.0) as u8 }
}

/// The distance to the nearest edge, the corners rounded (a soft minimum)
fn soft_dist(x: f32, y: f32, w: f32, h: f32) -> f32 {
    let k = 12.0;
    let m = x.min(w - x).min(y).min(h - y);
    // (relative to the nearest edge: no underflow far inside)
    let e = (-(x - m) / k).exp() + (-((w - x) - m) / k).exp() + (-(y - m) / k).exp() + (-((h - y) - m) / k).exp();
    (m - k * e.ln()).max(0.0)
}

/// 0..1 clockwise round the edge from the top-left corner, along the ray from the centre
fn perimeter(x: f32, y: f32, w: f32, h: f32, per: f32) -> f32 {
    let u = (x - w / 2.0) / (w / 2.0);
    let v = (y - h / 2.0) / (h / 2.0);
    let m = u.abs().max(v.abs()).max(1e-6);
    let ex = w / 2.0 + (w / 2.0) * u / m;
    let ey = h / 2.0 + (h / 2.0) * v / m;
    let hz = v.abs() >= u.abs();
    let s = if hz && v < 0.0 {
        ex
    } else if !hz && u > 0.0 {
        w + ey
    } else if hz {
        w + h + (w - ex)
    } else {
        2.0 * w + h + (h - ey)
    };
    (s / per).rem_euclid(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_from_config() {
        assert_eq!(Style::from_cfg(""), Style::Dots);
        assert_eq!(Style::from_cfg("bands"), Style::Bands);
        assert_eq!(Style::from_cfg("vignette").index(), 5);
        for (v, _) in CHOICES {
            assert!(v == "dots" || Style::from_cfg(v) != Style::Dots);
        }
    }

    #[test]
    fn every_style_draws_light_quickly() {
        for st in [Style::Dots, Style::Siri, Style::Gemini, Style::Ripple, Style::Vignette] {
            let mut fx = Fx::default();
            fx.palette(None);
            fx.setup(st, 1920.0, 1080.0, 700.0, 370.0);
            let _ = fx.frame(0.5);
            let t0 = std::time::Instant::now();
            let (soft, dots) = fx.frame(1.0);
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            let lit = |b: &Buf| b.as_slice().iter().filter(|p| p.a > 8).count();
            let n = lit(&soft) + dots.as_ref().map_or(0, lit);
            assert!(n > 500, "{st:?} drew {n} lit pixels");
            eprintln!("{st:?}: {ms:.1} ms, {n} lit");
        }
    }

    #[test]
    fn pastel_tones_made_vivid() {
        let c = vivid([1.0, 0.85, 0.8]);
        assert!(c[0] > 0.9 && c[2] < 0.4, "{c:?}");
        assert_eq!(vivid([0.5, 0.5, 0.5]), [0.5, 0.5, 0.5]);
    }

    #[test]
    fn edge_fields() {
        assert!(soft_dist(960.0, 270.0, 1920.0, 540.0) > 200.0);
        assert!(soft_dist(1.0, 270.0, 1920.0, 540.0) < 2.0);
        let s = perimeter(960.0, 539.0, 1920.0, 540.0, 2.0 * (1920.0 + 540.0));
        assert!((s - (1920.0 + 540.0 + 960.0) / (2.0 * (1920.0 + 540.0))).abs() < 0.01);
    }
}
