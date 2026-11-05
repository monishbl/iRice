//! The drop shelf (rust.surfaces: shelf): the shelf's card at the edge of a
//! screen and the island's drop zone, both looks. Compare
//! shell/modules/shelf/ShelfWindow.qml, ShelfRow.qml, ShelfDropZone.qml and
//! shell/services/Shelf.qml, ShelfLogic.js (logic.rs mirrors its drop and
//! mimeFor). The edge and shake probes (ShelfProbes.qml) stay QML: they're
//! invisible drop strips that only tell the shell a drag of files arrived.
//!
//! The shelf itself (its items, opening, the drop zone's timers, the pointer
//! following) stays the shell's (services/Shelf.qml). Its feed,
//! shell/modules/shelf/ShelfFeed.qml, says it on stdin:
//!   {"to":"shelf","state":{"opened","items":[{uri,path,name,kind,text}],
//!     "screen","side","atY","zone","zoneScreen"}}
//! and does what this asks (`shell::ask("shelf", [what, …])`):
//!   drop [uris, text]     dropped on the card (Shelf.drop)
//!   removeAt [i]          a row's ×
//!   clear | hide | share  the title row's buttons (Clear, Close, Share)
//!   hovered [bool]        the pointer or a drag is over the card (Shelf.hovered)
//!   zoneHover [bool]      a drag is over the drop zone (Shelf.zoneHover)
//!   cancelZone            a click on the zone or its ×
//!   keep [uris, text]     dropped on "Keep on the shelf" (Shelf.keep)
//!   sendOn [uris]         dropped on "Send to a phone" (Shelf.sendOn)
//!
//! Drag and drop is the platform's (src/dnd.rs): both windows take drops of
//! files and text (`State::accept_drops`), and a row pressed and moved past
//! Qt's start distance is dragged out (`State::start_drag`) carrying what
//! ShelfLogic.mimeFor gives (text/uri-list and text/plain). It stays on the
//! shelf, as in the QML: whether the drop was taken isn't known on Wayland
//! either way.
//!
//! Differences from the QML: no drag image (Qt shows none either, but the
//! compositor's cursor may differ); file names are elided in the middle by
//! counting characters (Slint only elides the end), so a name can still be
//! cut at the end when its letters run wide; Material's empty-state shape
//! switches between cookie9 and clover8 instead of morphing, and its dashed
//! outline is drawn as short segments (Slint's Path has no dash pattern);
//! Material draws its own tonal card (no glass), as the other Rust panels;
//! a picture that can't be decoded here (SVG, AVIF, HEIC, TIFF) shows the
//! file glyph instead of an empty tile; the list scrolls with the wheel only
//! (a press and move drags the row out, as dragging is what rows are for);
//! Cool-down's slower motion (CoolDown.motionScale) isn't followed; the card
//! window spans the screen's height (the card moves inside it, with the
//! input region and glass on the card only), where the QML window is the
//! card's size.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod logic;

use crate::dnd::Drag;
use tessera_core::services::hypr::Glass;
use tessera_core::shell::{self, Part};
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use logic::{Feed, Item, Target};
use serde_json::{json, Value};
use slint::{ComponentHandle, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

const NAME: &str = "shelf";
const NS: &str = "tessera:shelf";
const ZONE_NS: &str = "tessera:shelf-zone";
/// The card's window: the card and its shadow's room either side
const WIN_W: u32 = (logic::CARD_W + 2.0 * logic::M) as u32;

enum M {
    Share,
    Clear,
    Close,
    Remove(i32),
    DragOut(i32),
    Hover(bool),
    Cancel,
    Thumb(String, Option<SharedPixelBuffer<Rgba8Pixel>>),
    CardGone,
    ZoneGone,
}

struct CardWin {
    output: String,
    surface: Rc<Surface>,
    ui: ui::ShelfCard,
    a11y: Option<crate::a11y::A11y>,
    left: Option<bool>,
    region: Vec<(i32, i32, i32, i32)>,
    glass_sent: String,
    /// drawn again once mapped (a first frame can be the 1×1 one, stretched)
    settled: bool,
}

struct ZoneWin {
    output: String,
    surface: Rc<Surface>,
    ui: ui::ShelfZone,
    a11y: Option<crate::a11y::A11y>,
    settled: bool,
}

#[derive(Default)]
pub struct Shelf {
    feed: Feed,
    card: Option<CardWin>,
    zone: Option<ZoneWin>,
    /// the card is showing (not just fading out)
    card_on: bool,
    zone_on: bool,
    card_leave: slint::Timer,
    zone_leave: slint::Timer,
    /// pictures' thumbnails, by path (None: couldn't be decoded)
    thumbs: HashMap<String, Option<SharedPixelBuffer<Rgba8Pixel>>>,
    decoding: HashSet<String>,
    pointer_over: bool,
    drag_over: bool,
    /// the drag over the card is one of ours (a row taken out)
    drag_ours: bool,
    hovered_sent: bool,
    over_zone: bool,
    over: Option<Target>,
}

fn glyph(name: &str) -> SharedString {
    crate::icons::glyph(name).into()
}

/// The output by name, else the first
fn output_for(st: &State, name: &str) -> Option<(smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput, String)> {
    if !name.is_empty() {
        if let Some(o) = st.output_named(name) {
            return Some((o, name.to_string()));
        }
    }
    st.outputs_now().into_iter().next()
}

impl Shelf {
    // ── The card ─────────────────────────────────────────────────────────
    fn card_window(&mut self, st: &mut State, output: &str) -> bool {
        if self.card.as_ref().is_some_and(|w| w.output != output) {
            let w = self.card.take().unwrap();
            Glass::clear_now(&w.output, NS);
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
        }
        if self.card.is_some() {
            return true;
        }
        let Some(wl_output) = st.output_named(output) else { return false };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((&wl_output, output)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::RIGHT,
            size: (WIN_W, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::ShelfCard::new().expect("ShelfCard");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.global::<ui::Symbols>().on_of(|g| crate::icons::symbol(&g).into());
        let to = st.shell.sender(NAME);
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                ui.$set(move |$($a),*| to(Box::new($m)));
            }};
        }
        on!(on_share, | | M::Share);
        on!(on_clear, | | M::Clear);
        on!(on_close, | | M::Close);
        on!(on_remove, |i| M::Remove(i));
        on!(on_drag_out, |i| M::DragOut(i));
        on!(on_hover_changed, |h| M::Hover(h));
        ui.set_g_share(glyph("share-network"));
        ui.set_g_trash(glyph("trash"));
        ui.set_g_x(glyph("x"));
        ui.set_g_tray(glyph("tray"));
        ui.set_outline_rest(crate::surfaces::shapes::outline("cookie9").into());
        ui.set_outline_drop(logic::clover8().into());
        let empty_w = logic::CARD_W - 24.0;
        ui.set_dashes_rest(logic::dashes(empty_w, 160.0, 16.0, 1.5).into());
        ui.set_dashes_drop(logic::dashes(empty_w, 160.0, 16.0, 2.0).into());
        ui.show().expect("show shelf");
        st.accept_drops(&surface, true);
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.card = Some(CardWin { output: output.into(), surface, ui, a11y, left: None, region: Vec::new(), glass_sent: String::new(), settled: false });
        true
    }

    /// The card's rows and numbers, into its window
    fn push_card(&mut self, st: &mut State) {
        let material = st.shell.look.material;
        let Some(w) = &mut self.card else { return };
        let home = std::env::var("HOME").unwrap_or_default();
        let (label_w, size) = if material { (154.0, 14.0) } else { (162.0, 13.0) };
        let n = self.feed.items.len();
        let rows: Vec<ui::ShelfItem> = self
            .feed
            .items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let picture = it.kind == "image";
                let thumb = if picture { self.thumbs.get(&it.path).cloned().flatten() } else { None };
                let label = if it.kind == "text" { it.name.clone() } else { logic::elide_middle(&it.name, logic::fits(label_w, size)) };
                ui::ShelfItem {
                    label: label.into(),
                    detail: logic::elide_middle(&logic::detail(it, &home), logic::fits(label_w, 11.0)).into(),
                    glyph: glyph(logic::glyph_name(it)),
                    picture,
                    has_image: thumb.is_some(),
                    image: thumb.map(slint::Image::from_rgba8).unwrap_or_default(),
                    first: i == 0,
                    last: i + 1 == n,
                    index: i as i32,
                }
            })
            .collect();
        w.ui.set_rows(ModelRc::new(VecModel::from(rows)));
        w.ui.set_all_row(ui::ShelfItem {
            label: format!("All {n} items").into(),
            detail: "Drag to take them together".into(),
            glyph: glyph("stack"),
            first: true,
            last: true,
            index: -1,
            ..Default::default()
        });
        w.ui.set_count(n as i32);
        w.ui.set_has_paths(!self.feed.paths().is_empty());
        w.ui.set_drag_over(self.drag_over);
        // Its place: at the side, and down the screen as the QML's margins
        if w.left != Some(self.feed.left) {
            w.left = Some(self.feed.left);
            let side = if self.feed.left { Anchor::LEFT } else { Anchor::RIGHT };
            w.surface.layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | side);
            // (8 from the edge: the shadow's room hangs off the screen)
            let m = 8 - logic::M as i32;
            if self.feed.left {
                w.surface.layer.set_margin(0, 0, 0, m);
            } else {
                w.surface.layer.set_margin(0, m, 0, 0);
            }
            w.surface.layer.commit();
            w.glass_sent.clear();
        }
        w.ui.set_left(self.feed.left);
        let screen_h = st.output_named(&w.output).and_then(|o| st.outputs.info(&o)).and_then(|i| i.logical_size).map_or(1080.0, |s| s.1.max(1) as f32);
        let h = logic::card_height(n, material);
        let y = logic::card_top(self.feed.at_y, h, screen_h);
        w.ui.set_card_h(h);
        w.ui.set_card_y(y);
        let region = vec![(logic::M as i32, y.round() as i32, logic::CARD_W as i32, h.round() as i32)];
        if region != w.region {
            st.set_input_region(&w.surface, Some(&region));
            w.region = region;
        }
        // Pictures' thumbnails, decoded off this thread
        for it in self.feed.items.iter().filter(|i| i.kind == "image" && !i.path.is_empty()) {
            if self.thumbs.contains_key(&it.path) || !self.decoding.insert(it.path.clone()) {
                continue;
            }
            let (path, tx) = (it.path.clone(), st.tx.clone());
            std::thread::spawn(move || {
                let thumb = image::open(&path).ok().map(|img| {
                    let t = img.resize_to_fill(64, 64, image::imageops::FilterType::Triangle).to_rgba8();
                    SharedPixelBuffer::clone_from_slice(t.as_raw(), t.width(), t.height())
                });
                let _ = tx.send(crate::Msg::Part(NAME, Box::new(M::Thumb(path, thumb))));
            });
        }
    }

    fn show_card(&mut self, st: &mut State) {
        let screen = self.feed.screen.clone();
        let Some((_, output)) = output_for(st, &screen) else { return };
        let was_mapped = self.card.as_ref().is_some_and(|w| w.output == output && w.surface.mapped.get());
        if !self.card_window(st, &output) {
            return;
        }
        self.push_card(st);
        if !self.card_on {
            self.card_on = true;
            self.card_leave.stop();
            let w = self.card.as_mut().unwrap();
            // (opened from nothing: there at once, as the QML's Behavior
            // doesn't run when its loader makes the card; while it's still
            // fading out, it comes back with the animation)
            w.ui.set_instant(!was_mapped);
            w.ui.set_shown(true);
            if !was_mapped {
                w.settled = false;
                w.surface.show(WIN_W, 0);
            }
        }
    }

    fn hide_card(&mut self, st: &State) {
        if !self.card_on {
            return;
        }
        self.card_on = false;
        if let Some(w) = &self.card {
            w.ui.set_instant(false);
            w.ui.set_shown(false);
        }
        self.pointer_over = false;
        self.drag_over = false;
        self.say_hovered();
        // (the QML's loader stays 140 ms for the fade)
        let to = st.shell.sender(NAME);
        self.card_leave.start(slint::TimerMode::SingleShot, Duration::from_millis(140), move || to(Box::new(M::CardGone)));
    }

    fn card_gone(&mut self, st: &mut State) {
        if self.card_on {
            return;
        }
        if let Some(w) = &mut self.card {
            w.surface.hide();
            st.shell.glass.set(&w.output, NS, &[]);
            w.glass_sent.clear();
        }
    }

    /// Shelf.hovered: the pointer or a drag over the card
    fn say_hovered(&mut self) {
        let h = self.pointer_over || self.drag_over;
        if h != self.hovered_sent {
            self.hovered_sent = h;
            shell::ask("shelf", json!(["hovered", h]));
        }
    }

    // ── The drop zone ────────────────────────────────────────────────────
    fn zone_window(&mut self, st: &mut State, output: &str) -> bool {
        if self.zone.as_ref().is_some_and(|w| w.output != output) {
            let w = self.zone.take().unwrap();
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
        }
        if self.zone.is_some() {
            return true;
        }
        let Some(wl_output) = st.output_named(output) else { return false };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: ZONE_NS,
            output: Some((&wl_output, output)),
            anchor: Anchor::TOP,
            size: (logic::ZONE_W, logic::ZONE_H as u32 + 24),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        surface.layer.set_margin(4, 0, 0, 0);
        let ui = ui::ShelfZone::new().expect("ShelfZone");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.global::<ui::Symbols>().on_of(|g| crate::icons::symbol(&g).into());
        let to = st.shell.sender(NAME);
        ui.on_cancel(move || to(Box::new(M::Cancel)));
        ui.set_g_head(glyph("tray-arrow-down"));
        ui.set_g_x(glyph("x"));
        ui.set_g_tray(glyph("tray"));
        ui.set_g_send(glyph("share-network"));
        ui.show().expect("show shelf zone");
        st.accept_drops(&surface, true);
        // (the zone takes input, the window's margin round it doesn't)
        let x0 = ((logic::ZONE_W as f32 - 520.0) / 2.0) as i32;
        st.set_input_region(&surface, Some(&[(x0, 0, 520, logic::ZONE_H as i32)]));
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.zone = Some(ZoneWin { output: output.into(), surface, ui, a11y, settled: false });
        true
    }

    fn show_zone(&mut self, st: &mut State) {
        let screen = self.feed.zone_screen.clone();
        let Some((_, output)) = output_for(st, &screen) else { return };
        if !self.zone_window(st, &output) {
            return;
        }
        let w = self.zone.as_mut().unwrap();
        w.ui.set_count(self.feed.items.len() as i32);
        if !self.zone_on {
            self.zone_on = true;
            self.zone_leave.stop();
            w.ui.set_open(true);
            if !w.surface.mapped.get() {
                w.settled = false;
                w.surface.show(logic::ZONE_W, logic::ZONE_H as u32 + 24);
            }
        }
    }

    fn hide_zone(&mut self, st: &State) {
        if !self.zone_on {
            return;
        }
        self.zone_on = false;
        if let Some(w) = &self.zone {
            w.ui.set_open(false);
        }
        self.set_over(false, None);
        let to = st.shell.sender(NAME);
        self.zone_leave.start(slint::TimerMode::SingleShot, Duration::from_millis(200), move || to(Box::new(M::ZoneGone)));
    }

    /// Where the drag is over the zone: anywhere on it (Shelf.zoneHover), and which target
    fn set_over(&mut self, any: bool, target: Option<Target>) {
        let any = any || target.is_some();
        if any != self.over_zone && self.zone_on {
            shell::ask("shelf", json!(["zoneHover", any]));
        }
        self.over_zone = any;
        self.over = target;
        if let Some(w) = &self.zone {
            w.ui.set_over_keep(target == Some(Target::Keep));
            w.ui.set_over_send(target == Some(Target::Send));
        }
    }

    /// The shell said: follow it
    fn follow(&mut self, st: &mut State) {
        if self.feed.opened {
            self.show_card(st);
        } else {
            self.hide_card(st);
        }
        if self.feed.zone {
            self.show_zone(st);
        } else {
            self.hide_zone(st);
        }
    }

    fn drag_card(&mut self, st: &mut State, ev: &Drag) {
        match ev {
            Drag::Enter { ours, .. } => {
                self.drag_ours = *ours;
                if *ours {
                    st.refuse_drop();
                    return;
                }
                self.drag_over = true;
            }
            Drag::Motion { .. } => {
                if self.drag_ours {
                    st.refuse_drop();
                    return;
                }
            }
            Drag::Leave => {
                self.drag_over = false;
                self.drag_ours = false;
            }
            Drag::Drop { uris, text, .. } => {
                self.drag_over = false;
                if self.drag_ours || !logic::would_add(&self.feed.items, uris, text) {
                    st.refuse_drop();
                } else {
                    shell::ask("shelf", json!(["drop", uris, text]));
                }
                self.drag_ours = false;
            }
        }
        if let Some(w) = &self.card {
            w.ui.set_drag_over(self.drag_over);
        }
        self.say_hovered();
    }

    fn drag_zone(&mut self, st: &mut State, ev: &Drag) {
        match ev {
            Drag::Enter { x, y, .. } | Drag::Motion { x, y } => {
                let t = logic::target_at(*x, *y);
                self.set_over(logic::in_zone(*x, *y), t);
            }
            Drag::Leave => self.set_over(false, None),
            Drag::Drop { x, y, uris, text } => {
                match logic::target_at(*x, *y) {
                    Some(Target::Keep) => shell::ask("shelf", json!(["keep", uris, text])),
                    Some(Target::Send) => shell::ask("shelf", json!(["sendOn", uris])),
                    // (between the targets: nothing)
                    None => st.refuse_drop(),
                }
                self.set_over(false, None);
            }
        }
    }

    /// A row (or "All", -1) pressed and moved: drag it out
    fn drag_out(&mut self, st: &mut State, i: i32) {
        let Some(w) = &self.card else { return };
        let list: Vec<Item> = if i < 0 { self.feed.items.clone() } else { self.feed.items.get(i as usize).cloned().into_iter().collect() };
        let offers = logic::mime_for(&list);
        if offers.is_empty() {
            return;
        }
        let surface = w.surface.clone();
        st.start_drag(&surface, offers);
    }
}

impl Part for Shelf {
    fn name(&self) -> &'static str {
        NAME
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if !look {
            return;
        }
        if let Some(w) = &mut self.card {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.glass_sent.clear();
        }
        if let Some(w) = &self.zone {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
        }
        if self.card_on {
            self.push_card(st);
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = &mut self.card {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.glass_sent.clear();
        }
        if let Some(w) = &self.zone {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let msg = match msg.downcast::<Value>() {
            Ok(v) => {
                if let Some(f) = Feed::parse(&v) {
                    // (pictures gone from the shelf: their thumbnails too)
                    self.thumbs.retain(|p, _| f.items.iter().any(|i| &i.path == p));
                    self.feed = f;
                    self.follow(st);
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Share => shell::ask("shelf", json!(["share"])),
            M::Clear => shell::ask("shelf", json!(["clear"])),
            M::Close => shell::ask("shelf", json!(["hide"])),
            M::Remove(i) => {
                if i >= 0 {
                    shell::ask("shelf", json!(["removeAt", i]));
                }
            }
            M::DragOut(i) => self.drag_out(st, i),
            M::Hover(h) => {
                self.pointer_over = h && self.card_on;
                self.say_hovered();
            }
            M::Cancel => shell::ask("shelf", json!(["cancelZone"])),
            M::Thumb(path, thumb) => {
                self.decoding.remove(&path);
                if self.feed.items.iter().any(|i| i.path == path) {
                    self.thumbs.insert(path, thumb);
                    if self.card_on {
                        self.push_card(st);
                    }
                }
            }
            M::CardGone => self.card_gone(st),
            M::ZoneGone => {
                if !self.zone_on {
                    if let Some(w) = &self.zone {
                        w.surface.hide();
                    }
                }
            }
        }
    }

    fn drag(&mut self, st: &mut State, surface: &Rc<Surface>, ev: &Drag) {
        if self.card.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, surface)) {
            self.drag_card(st, ev);
        } else if self.zone.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, surface)) {
            self.drag_zone(st, ev);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.card.as_ref().is_some_and(|w| w.output == name) {
            let w = self.card.take().unwrap();
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
            self.card_on = false;
        }
        if self.zone.as_ref().is_some_and(|w| w.output == name) {
            let w = self.zone.take().unwrap();
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
            self.zone_on = false;
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.card.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            let w = self.card.take().unwrap();
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
            self.card_on = false;
        }
        if self.zone.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            let w = self.zone.take().unwrap();
            // (hidden first: Slint keeps a shown window, and with it the layer, alive)
            let _ = w.ui.hide();
            w.surface.hide();
            st.drop_surface(&w.surface);
            self.zone_on = false;
        }
    }

    /// The card's glass (Glass), following its fade and slide; accessibility
    fn after_frame(&mut self, st: &mut State) {
        if let Some(w) = &mut self.zone {
            crate::picture::second_frame(&w.surface, &mut w.settled);
            if let Some(a) = w.a11y.as_mut() {
                a.update(false, false);
            }
        }
        let Some(w) = &mut self.card else { return };
        crate::picture::second_frame(&w.surface, &mut w.settled);
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, false);
        }
        if !w.surface.mapped.get() || !st.shell.plugin || st.shell.look.material {
            return;
        }
        let ui = &w.ui;
        let o = ui.get_fade();
        let shape = theme::glass_shape(&st.shell.look, logic::M + ui.get_slide(), ui.get_card_y(), logic::CARD_W, ui.get_card_h(), 20.0, o, 1.0);
        let key = if o > 0.002 { format!("{shape:?}") } else { String::new() };
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, if o > 0.002 { std::slice::from_ref(&shape) } else { &[] });
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        let mut v = Vec::new();
        if let Some(a) = self.card.as_mut().and_then(|w| w.a11y.as_mut()) {
            v.push(a);
        }
        if let Some(a) = self.zone.as_mut().and_then(|w| w.a11y.as_mut()) {
            v.push(a);
        }
        v
    }

    fn stats(&self) {
        if let Some(w) = &self.card {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report("shelf"));
        }
    }

    fn quit(&mut self, _: &mut State) {
        if let Some(w) = &self.card {
            Glass::clear_now(&w.output, NS);
        }
        if self.hovered_sent {
            shell::ask("shelf", json!(["hovered", false]));
        }
    }
}
