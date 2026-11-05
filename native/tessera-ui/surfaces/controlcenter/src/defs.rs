//! Every Control Center module and the saved layout, as plain functions
//! (shell/modules/controlcenter/ModuleDefs.qml): what each is (its kind,
//! sizes, page), the layout from config.json (`controlCenter.layout`, "id:WxH"
//! in order, the default when empty), packing it into four columns, and the
//! editor's changes (size, add, remove, move). What a module says right now
//! (its label, icon, on, status, whether this computer can use it) comes
//! from the shell (bridge.rs); the sliders' levels from the sound server and
//! the backlight.

use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Toggle,
    Action,
    Slider,
    Media,
    Info,
}

impl Kind {
    /// The number the Slint side knows it by
    pub fn code(self) -> i32 {
        match self {
            Kind::Toggle => 0,
            Kind::Action => 1,
            Kind::Slider => 2,
            Kind::Media => 3,
            Kind::Info => 4,
        }
    }
}

pub struct Def {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    /// (columns, rows), the first is the size it's added at
    pub sizes: &'static [(u8, u8)],
    /// its detail page ("" none)
    pub page: &'static str,
    /// its icon until the shell says (Phosphor's name)
    pub icon: &'static str,
    /// it has choices (a menu) besides its switch
    pub menu: bool,
}

const fn d(id: &'static str, label: &'static str, kind: Kind, sizes: &'static [(u8, u8)], page: &'static str, icon: &'static str) -> Def {
    Def { id, label, kind, sizes, page, icon, menu: false }
}

use Kind::*;
const S11_21: &[(u8, u8)] = &[(1, 1), (2, 1)];
const S21_11: &[(u8, u8)] = &[(2, 1), (1, 1)];
const SLIDER: &[(u8, u8)] = &[(4, 1), (1, 2)];

/// ModuleDefs.all, in its order (the gallery lists them so)
pub const DEFS: &[Def] = &[
    d("wifi", "Wi-Fi", Toggle, S21_11, "wifi", "wifi-high"),
    d("bluetooth", "Bluetooth", Toggle, S11_21, "bluetooth", "bluetooth"),
    d("focus", "Focus", Toggle, S21_11, "focus", "moon"),
    d("nightLight", "Night Light", Toggle, S11_21, "", "moon-stars"),
    d("darkMode", "Dark Mode", Toggle, S11_21, "", "circle-half"),
    Def { id: "sharing", label: "Sharing", kind: Toggle, sizes: S11_21, page: "sharing", icon: "share-network", menu: true },
    d("mirroring", "Screen Mirroring", Toggle, S21_11, "mirroring", "screencast"),
    d("captions", "Live Captions", Toggle, S11_21, "", "closed-captioning"),
    d("keepAwake", "Caffeine", Toggle, S11_21, "caffeine", "coffee"),
    d("stageManager", "Stages", Toggle, S11_21, "", "squares-four"),
    d("mic", "Microphone", Toggle, S11_21, "", "microphone"),
    d("lowPower", "Low Power", Toggle, S11_21, "", "leaf"),
    d("coolDown", "Cool-down", Toggle, S11_21, "", "snowflake"),
    d("gameMode", "Game Mode", Toggle, S11_21, "", "game-controller"),
    d("antiFlashbang", "Anti-Flash", Toggle, S11_21, "", "sun-dim"),
    d("record", "Record", Toggle, S11_21, "", "record"),
    d("easyEffects", "EasyEffects", Toggle, S11_21, "", "faders"),
    d("warp", "WARP", Toggle, S11_21, "", "cloud"),
    d("keyboard", "Keyboard", Action, S11_21, "", "keyboard"),
    d("screenshot", "Screenshot", Action, S11_21, "", "camera"),
    d("colorPicker", "Colour Picker", Action, S11_21, "", "eyedropper"),
    d("musicRecognition", "Identify Song", Action, S11_21, "", "music-notes"),
    d("clipboard", "Clipboard", Action, S11_21, "", "clipboard-text"),
    d("calculator", "Calculator", Action, S11_21, "", "calculator"),
    d("timer", "Timer", Action, S11_21, "", "timer"),
    d("tileWindows", "Tile Left & Right", Action, S21_11, "", "square-split-horizontal"),
    d("appLibrary", "App Drawer", Action, S11_21, "", "squares-four"),
    d("display", "Display", Slider, SLIDER, "", "sun"),
    d("sound", "Sound", Slider, SLIDER, "sound", "speaker-high"),
    d("micLevel", "Microphone level", Slider, SLIDER, "mic", "microphone"),
    d("nowPlaying", "Now Playing", Media, &[(2, 2), (4, 1)], "", "music-notes"),
    d("battery", "Battery", Info, S21_11, "", "battery-high"),
    d("resources", "CPU & Memory", Info, &[(2, 1), (2, 2)], "resources", "cpu"),
    d("weather", "Weather", Info, &[(2, 1), (2, 2)], "", "cloud-sun"),
];

pub const DEFAULT_LAYOUT: &[&str] = &["wifi:2x1", "nowPlaying:2x2", "bluetooth:1x1", "nightLight:1x1", "focus:2x1", "stageManager:1x1", "darkMode:1x1", "display:4x1", "sound:4x1", "screenshot:1x1", "record:1x1", "keepAwake:1x1", "clipboard:1x1", "mirroring:2x1"];

pub fn def(id: &str) -> Option<&'static Def> {
    DEFS.iter().find(|d| d.id == id)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub w: u8,
    pub h: u8,
}

impl Entry {
    pub fn saved(&self) -> String {
        format!("{}:{}x{}", self.id, self.w, self.h)
    }
}

/// "id:WxH" -> Entry; a size the module no longer offers becomes the
/// largest one it offers that fits inside, else its first (ModuleDefs.parse)
pub fn parse(entry: &str) -> Entry {
    let (id, size) = entry.split_once(':').unwrap_or((entry, "1x1"));
    let mut it = size.split('x').map(|n| n.trim().parse::<u8>().unwrap_or(0));
    let w = it.next().filter(|v| *v > 0).unwrap_or(1);
    let h = it.next().filter(|v| *v > 0).unwrap_or(1);
    let (mut w, mut h) = (w, h);
    if let Some(d) = def(id) {
        if !d.sizes.contains(&(w, h)) {
            let mut fits: Vec<(u8, u8)> = d.sizes.iter().copied().filter(|z| z.0 <= w && z.1 <= h).collect();
            fits.sort_by_key(|z| std::cmp::Reverse(z.0 as u32 * z.1 as u32));
            (w, h) = fits.first().copied().unwrap_or(d.sizes[0]);
        }
    }
    Entry { id: id.to_string(), w, h }
}

/// The saved list as config.json has it (the default when empty)
pub fn saved(cfg: &Value) -> Vec<String> {
    let list: Vec<String> = cfg["controlCenter"]["layout"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
    if list.is_empty() {
        DEFAULT_LAYOUT.iter().map(|s| s.to_string()).collect()
    } else {
        list
    }
}

/// The layout shown: the saved one, modules this computer can't use left out
pub fn layout(saved: &[String], usable: &dyn Fn(&str) -> bool) -> Vec<Entry> {
    saved.iter().map(|e| parse(e)).filter(|e| def(&e.id).is_some() && usable(&e.id)).collect()
}

/// What to save for these entries: them, then the saved ones that are only
/// unusable right now (no brightness while undocked, a tool not installed
/// yet) kept at the end (ModuleDefs.saveLayout)
pub fn save(entries: &[Entry], saved: &[String], usable: &dyn Fn(&str) -> bool) -> Vec<String> {
    let resting = saved.iter().filter(|e| {
        let id = parse(e).id;
        !(def(&id).is_some() && usable(&id))
    });
    entries.iter().map(Entry::saved).chain(resting.cloned()).collect()
}

pub fn remove(layout: &[Entry], id: &str) -> Vec<Entry> {
    layout.iter().filter(|e| e.id != id).cloned().collect()
}

/// Its next size (ModuleDefs.cycleSize)
pub fn cycle(layout: &[Entry], id: &str) -> Vec<Entry> {
    let Some(d) = def(id) else { return layout.to_vec() };
    layout
        .iter()
        .map(|e| {
            if e.id != id {
                return e.clone();
            }
            let i = d.sizes.iter().position(|z| *z == (e.w, e.h)).map_or(0, |i| (i + 1) % d.sizes.len());
            Entry { id: e.id.clone(), w: d.sizes[i].0, h: d.sizes[i].1 }
        })
        .collect()
}

pub fn add(layout: &[Entry], id: &str) -> Vec<Entry> {
    let Some(d) = def(id) else { return layout.to_vec() };
    let mut out = layout.to_vec();
    out.push(Entry { id: id.into(), w: d.sizes[0].0, h: d.sizes[0].1 });
    out
}

/// Move it to `to` in the order (ModuleDefs.moveModule)
pub fn move_to(layout: &[Entry], id: &str, to: usize) -> Vec<Entry> {
    let mut list = layout.to_vec();
    let Some(from) = list.iter().position(|e| e.id == id) else { return list };
    if from == to {
        return list;
    }
    let e = list.remove(from);
    list.insert(to.min(list.len()), e);
    list
}

/// The editor's gallery: what isn't in the layout yet, the usable first
pub fn gallery(layout: &[Entry], usable: &dyn Fn(&str) -> bool) -> Vec<&'static str> {
    let ids: Vec<&'static str> = DEFS.iter().map(|d| d.id).filter(|id| !layout.iter().any(|e| e.id == *id)).collect();
    ids.iter().copied().filter(|id| usable(id)).chain(ids.iter().copied().filter(|id| !usable(id))).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placed {
    pub id: String,
    pub w: u8,
    pub h: u8,
    pub col: u8,
    pub row: u8,
}

/// First-fit into `cols` columns, row by row (ModuleDefs.pack)
pub fn pack(entries: &[Entry], cols: u8) -> Vec<Placed> {
    let mut taken: Vec<Vec<bool>> = Vec::new();
    let mut out = Vec::new();
    for e in entries {
        let w = e.w.min(cols);
        'search: for r in 0..64u8 {
            for c in 0..cols {
                if c + w > cols {
                    continue;
                }
                let free = (r..r + e.h).all(|y| (c..c + w).all(|x| !taken.get(y as usize).is_some_and(|row| row.get(x as usize).copied().unwrap_or(false))));
                if free {
                    for y in r..r + e.h {
                        while taken.len() <= y as usize {
                            taken.push(vec![false; cols as usize]);
                        }
                        for x in c..c + w {
                            taken[y as usize][x as usize] = true;
                        }
                    }
                    out.push(Placed { id: e.id.clone(), w, h: e.h, col: c, row: r });
                    break 'search;
                }
            }
        }
    }
    out
}

pub fn rows_used(packed: &[Placed]) -> u8 {
    packed.iter().map(|p| p.row + p.h).max().unwrap_or(0)
}

/// Where a dragged control lands: the packed cell whose centre is nearest
/// (cx, cy), cells cell_w × cell_h apart by gap (ModuleDefs.nearestSlot)
pub fn nearest_slot(packed: &[Placed], cx: f32, cy: f32, cell_w: f32, cell_h: f32, gap: f32) -> Option<usize> {
    let mut best = None;
    let mut best_d = f32::MAX;
    for (i, e) in packed.iter().enumerate() {
        let ex = e.col as f32 * (cell_w + gap) + (e.w as f32 * cell_w + (e.w as f32 - 1.0) * gap) / 2.0;
        let ey = e.row as f32 * (cell_h + gap) + (e.h as f32 * cell_h + (e.h as f32 - 1.0) * gap) / 2.0;
        let d = (ex - cx).powi(2) + (ey - cy).powi(2);
        if d < best_d {
            best_d = d;
            best = Some(i);
        }
    }
    best
}

/// Arrow keys between controls: the nearest whose centre lies that way, rows
/// first for up and down, columns for left and right (ModuleDefs.nearestToward);
/// rects are (x, y, w, h)
pub fn nearest_toward(rects: &[(f32, f32, f32, f32)], from: usize, dx: i32, dy: i32) -> Option<usize> {
    let f = rects.get(from)?;
    let (cx, cy) = (f.0 + f.2 / 2.0, f.1 + f.3 / 2.0);
    let mut best = None;
    let mut best_d = f32::MAX;
    for (i, m) in rects.iter().enumerate() {
        if i == from {
            continue;
        }
        let (mx, my) = (m.0 + m.2 / 2.0 - cx, m.1 + m.3 / 2.0 - cy);
        let sign = |v: f32| if v > 0.0 { 1 } else if v < 0.0 { -1 } else { 0 };
        if (dx != 0 && sign(mx) != dx) || (dy != 0 && sign(my) != dy) {
            continue;
        }
        let d = if dx != 0 { mx.abs() + 2.0 * my.abs() } else { my.abs() + 2.0 * mx.abs() };
        if d < best_d {
            best_d = d;
            best = Some(i);
        }
    }
    best
}

/// Quick Settings' press (Material 3 Expressive's button groups): the
/// pressed tile widens by `give` and its neighbours in the row make room.
/// For each tile (in `packed`'s order), how far it moves and widens per unit
/// of the squeeze (QuickSettings.shift)
pub fn squeeze(packed: &[Placed], pressing: &str, give: f32) -> Vec<(f32, f32)> {
    let mut out = vec![(0.0, 0.0); packed.len()];
    let Some(pi) = packed.iter().position(|p| p.id == pressing) else { return out };
    let p = &packed[pi];
    let left = packed.iter().position(|q| q.row == p.row && q.h == p.h && q.col + q.w == p.col);
    let right = packed.iter().position(|q| q.row == p.row && q.h == p.h && p.col + p.w == q.col);
    if left.is_none() && right.is_none() {
        return out;
    }
    let from_left = if left.is_some() { if right.is_some() { give / 2.0 } else { give } } else { 0.0 };
    let from_right = give - from_left;
    out[pi] = (-from_left, give);
    if let Some(l) = left {
        out[l] = (0.0, -from_left);
    }
    if let Some(r) = right {
        out[r] = (from_right, -from_right);
    }
    out
}

/// Glass's Control Center draws each control as its own glass chip unless
/// the panel has a background (`controlCenter.background`) or the glass is
/// solid (Theme.glass.tier, ControlCenterGlass's `bare`)
pub fn bare(cfg: &Value) -> bool {
    let a = &cfg["appearance"];
    let reduce = a["reduceTransparency"].as_bool().unwrap_or(false);
    let glass = a["glass"].as_str().unwrap_or("frosted");
    let solid = reduce || glass == "solid" || (glass == "custom" && a["glassSolid"].as_f64().unwrap_or(0.0) >= 0.97);
    !solid && !cfg["controlCenter"]["background"].as_bool().unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn e(id: &str, w: u8, h: u8) -> Entry {
        Entry { id: id.into(), w, h }
    }

    #[test]
    fn every_module_is_known_once_and_the_default_uses_them() {
        for (i, d) in DEFS.iter().enumerate() {
            assert!(DEFS[i + 1..].iter().all(|o| o.id != d.id), "{} twice", d.id);
            assert!(!d.sizes.is_empty());
        }
        for s in DEFAULT_LAYOUT {
            let p = parse(s);
            assert!(def(&p.id).is_some(), "{s}");
            assert_eq!(p.saved(), *s);
        }
    }

    #[test]
    fn parsing_saved_entries() {
        assert_eq!(parse("wifi:2x1"), e("wifi", 2, 1));
        assert_eq!(parse("wifi"), e("wifi", 1, 1)); // (no size: 1x1, which it offers)
        assert_eq!(parse("bluetooth"), e("bluetooth", 1, 1));
        // a size it doesn't offer: the largest that fits inside, else its first
        assert_eq!(parse("sharing:2x2"), e("sharing", 2, 1));
        assert_eq!(parse("sharing:4x1"), e("sharing", 2, 1));
        assert_eq!(parse("display:2x1"), e("display", 4, 1));
        assert_eq!(parse("nowPlaying:4x2"), e("nowPlaying", 2, 2)); // (a tie: the first offered)
        assert_eq!(parse("mystery:3x0"), e("mystery", 3, 1));
    }

    #[test]
    fn the_layout_leaves_out_what_cant_be_used_and_keeps_it_saved() {
        let saved: Vec<String> = ["wifi:2x1", "display:4x1", "nope:1x1", "sound:4x1"].iter().map(|s| s.to_string()).collect();
        let no_display = |id: &str| id != "display";
        let l = layout(&saved, &no_display);
        assert_eq!(l.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["wifi", "sound"]);
        let out = save(&remove(&l, "wifi"), &saved, &no_display);
        assert_eq!(out, ["sound:4x1", "display:4x1", "nope:1x1"]);
        assert_eq!(super::saved(&json!({})).len(), DEFAULT_LAYOUT.len());
        assert_eq!(super::saved(&json!({"controlCenter": {"layout": ["wifi:1x1"]}})), ["wifi:1x1"]);
    }

    #[test]
    fn editing_sizes_adds_and_moves() {
        let l = vec![e("wifi", 2, 1), e("bluetooth", 1, 1), e("display", 4, 1)];
        assert_eq!(cycle(&l, "wifi")[0], e("wifi", 1, 1));
        assert_eq!(cycle(&cycle(&l, "wifi"), "wifi")[0], e("wifi", 2, 1));
        assert_eq!(cycle(&l, "display")[2], e("display", 1, 2));
        assert_eq!(add(&l, "nowPlaying").last(), Some(&e("nowPlaying", 2, 2)));
        let m = move_to(&l, "display", 0);
        assert_eq!(m.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["display", "wifi", "bluetooth"]);
        assert_eq!(move_to(&l, "wifi", 9).last().unwrap().id, "wifi");
        let g = gallery(&l, &|id| id != "warp");
        assert!(!g.contains(&"wifi") && g.contains(&"focus"));
        assert_eq!(*g.last().unwrap(), "warp");
    }

    #[test]
    fn packing_first_fit_in_four_columns() {
        let l: Vec<Entry> = DEFAULT_LAYOUT.iter().map(|s| parse(s)).collect();
        let p = pack(&l, 4);
        let at = |id: &str| p.iter().find(|x| x.id == id).map(|x| (x.col, x.row)).unwrap();
        assert_eq!(at("wifi"), (0, 0));
        assert_eq!(at("nowPlaying"), (2, 0));
        assert_eq!(at("bluetooth"), (0, 1));
        assert_eq!(at("nightLight"), (1, 1));
        assert_eq!(at("focus"), (0, 2));
        assert_eq!(at("stageManager"), (2, 2));
        assert_eq!(at("darkMode"), (3, 2));
        assert_eq!(at("display"), (0, 3));
        assert_eq!(at("sound"), (0, 4));
        assert_eq!(at("mirroring"), (0, 6));
        assert_eq!(rows_used(&p), 7);
        // a standing slider takes two rows of one column
        let p = pack(&[e("display", 1, 2), e("wifi", 2, 1), e("bluetooth", 1, 1), e("darkMode", 1, 1)], 4);
        assert_eq!((p[3].col, p[3].row), (1, 1));
    }

    #[test]
    fn dropping_lands_on_the_nearest_cell() {
        let p = pack(&[e("wifi", 2, 1), e("bluetooth", 1, 1), e("darkMode", 1, 1)], 4);
        assert_eq!(nearest_slot(&p, 10.0, 10.0, 72.5, 72.5, 10.0), Some(0));
        assert_eq!(nearest_slot(&p, 280.0, 30.0, 72.5, 72.5, 10.0), Some(2));
        assert_eq!(nearest_slot(&[], 0.0, 0.0, 1.0, 1.0, 1.0), None);
    }

    #[test]
    fn arrows_go_to_the_nearest_that_way() {
        // a 2x1 pill, two 1x1 beside it, a slider under all
        let r = [(0.0, 0.0, 155.0, 72.0), (165.0, 0.0, 72.0, 72.0), (247.0, 0.0, 72.0, 72.0), (0.0, 82.0, 320.0, 72.0)];
        assert_eq!(nearest_toward(&r, 0, 1, 0), Some(1));
        assert_eq!(nearest_toward(&r, 1, 1, 0), Some(2));
        assert_eq!(nearest_toward(&r, 2, 1, 0), None);
        assert_eq!(nearest_toward(&r, 2, 0, 1), Some(3));
        assert_eq!(nearest_toward(&r, 3, 0, -1), Some(1));
    }

    #[test]
    fn a_press_widens_the_tile_and_its_neighbours_give() {
        let p = pack(&[e("wifi", 2, 1), e("bluetooth", 1, 1), e("darkMode", 1, 1), e("display", 4, 1)], 4);
        // the middle one: half from each side
        let s = squeeze(&p, "bluetooth", 14.0);
        assert_eq!(s[1], (-7.0, 14.0));
        assert_eq!(s[0], (0.0, -7.0));
        assert_eq!(s[2], (7.0, -7.0));
        assert_eq!(s[3], (0.0, 0.0));
        // at the end: all from the left
        assert_eq!(squeeze(&p, "darkMode", 14.0)[2], (-14.0, 14.0));
        // alone in its row: nothing moves
        assert!(squeeze(&p, "display", 14.0).iter().all(|v| *v == (0.0, 0.0)));
    }

    #[test]
    fn chips_unless_a_background_or_solid_glass() {
        assert!(bare(&json!({})));
        assert!(!bare(&json!({"controlCenter": {"background": true}})));
        assert!(!bare(&json!({"appearance": {"glass": "solid"}})));
        assert!(!bare(&json!({"appearance": {"reduceTransparency": true}})));
        assert!(!bare(&json!({"appearance": {"glass": "custom", "glassSolid": 1.0}})));
        assert!(bare(&json!({"appearance": {"glass": "liquid"}})));
    }
}
