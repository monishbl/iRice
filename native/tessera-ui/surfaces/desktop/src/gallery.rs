//! Edit Widgets' gallery, in this crate since its previews are the desktop's
//! own widgets (rust.surfaces: desktop; compare
//! shell/modules/desktop/WidgetGallery.qml): a glass sheet along the bottom of
//! the screen, 96 px above the dock. Pick a widget on the left (click, or Up
//! and Down), then click one of its sizes to add it to the desktop (it lands
//! in the first free cell; no room says so on the island). Done or Escape
//! ends editing.
//!
//! It is up while Edit Widgets is on (the host's desktop feed says
//! `editing`, and on which screen), standalone only: with the QML shell
//! running, its own gallery draws. The sheet springs in (a spring, 40 px of
//! travel, opacity) and fades out in 192 ms. Its sizes are the real widgets
//! as the desktop draws them, scaled to 56%, with the data the desktop has
//! (the QML's previews show sample data).
//!
//! Differences from the QML: the sizes' content isn't clipped to the radius
//! the QML gives a widget in a preview (22 px Glass, the tile Material); a
//! Material card's press feel is the scale only (no elastic grab); the
//! kinds list scrolls with the wheel (no momentum).

use super::*;

pub const NS: &str = "tessera:widgetgallery";

pub struct Gal {
    pub output: String,
    surface: Rc<Surface>,
    ui: ui::Gallery,
    sizes: Rc<VecModel<ui::GalSize>>,
    shown: bool,
    glass_sent: String,
}

/// What each kind does (WidgetDefs.qml's desc)
pub fn desc(id: &str) -> &'static str {
    match id {
        "clock" => "An analog face, or the time and date.",
        "worldClock" => "The time in other cities. Right-click it to choose them (Widget Options).",
        "weather" => "Conditions where you are, and the next hours.",
        "today" => "Your day at a glance: a greeting, what's next, to-dos and the weather.",
        "calendar" => "Today, the month, and what's coming up.",
        "reminders" => "Your to-dos. Tick them off right on the desktop.",
        "batteries" => "This computer and connected Bluetooth devices.",
        "nowPlaying" => "What's playing, with controls.",
        "system" => "The last minute of CPU and memory; bigger sizes add temperature, network and more.",
        "photos" => "A picture from your Pictures folder (or one you choose: right-click, Widget Options), changing every 15 minutes.",
        "notes" => "A note you can type into on the desktop.",
        "stack" => "Several widgets in one place: scroll to flip; it brings forward what matters now.",
        "shortcuts" => "One-click buttons: screenshot, dark mode, lock... Right-click to choose.",
        "screenTime" => "How long you've used the computer today, and on which apps.",
        "batteryHistory" => "Your battery over the last 24 hours.",
        "stocks" => "Today's prices for your symbols (right-click it: Widget Options).",
        _ => "",
    }
}

/// The sheet's width on a screen this wide (the window adds the glass's room)
pub fn sheet_width(screen_w: f32) -> f32 {
    880.0f32.min(screen_w - 48.0)
}

/// The previews' row: the tallest one scaled to 56%, 8 px, and its name
pub fn row_height(tallest: f32, material: bool) -> f32 {
    tallest * 0.56 + 8.0 + if material { 32.0 } else { 20.0 }
}

impl Desktop {
    /// The gallery follows Edit Widgets (called from update)
    pub(crate) fn gallery_sync(&mut self, st: &mut State) {
        let want = tessera_core::host::standalone() && self.feed.editing;
        if !want {
            if let Some(g) = self.gal.as_mut().filter(|g| g.shown) {
                g.shown = false;
                g.ui.set_shown(false);
                st.set_input_region(&g.surface, Some(&[]));
            }
            return;
        }
        let out = Some(self.feed.edit_screen.clone()).filter(|s| !s.is_empty() && st.output_named(s).is_some()).or_else(|| Self::primary(st).map(|p| p.1));
        let Some(out) = out else { return };
        if self.gal.as_ref().is_some_and(|g| g.output != out) {
            self.gallery_drop(st);
        }
        if self.gal.is_none() {
            self.gallery_make(st, &out);
        }
        if let Some(g) = self.gal.as_mut().filter(|g| !g.shown) {
            // (opened again while it was fading out)
            g.shown = true;
            g.ui.set_shown(true);
            g.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
            g.surface.layer.commit();
            st.set_input_region(&g.surface, None);
            g.ui.invoke_focus_keys();
        }
        self.gallery_fill(st);
    }

    fn gallery_make(&mut self, st: &mut State, name: &str) {
        let Some(wl) = st.output_named(name) else { return };
        let (sw, _) = crate::surfaces::kit::screen_size(st, &wl);
        let surface = st.prepare_surface(Spec {
            layer: Layer::Top,
            namespace: NS,
            output: Some((&wl, name)),
            anchor: Anchor::BOTTOM,
            size: ((sheet_width(sw) + 48.0) as u32, 316 + 48),
            exclusive: -1,
            keyboard: KeyboardInteractivity::OnDemand,
            margin_left: 0,
        });
        // (above the dock)
        surface.layer.set_margin(0, 0, 96, 0);
        surface.layer.commit();
        let ui = ui::Gallery::new().expect("Gallery");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.global::<ui::Symbols>().on_of(|g| crate::icons::symbol(&g).into());
        ui.set_cookie12(view_shape("cookie12").into());
        ui.set_cookie9(crate::surfaces::shapes::outline("cookie9").into());
        let kinds: Vec<ui::GalKind> = grid::KINDS.iter().map(|k| ui::GalKind { id: k.id.into(), label: k.label.into(), icon: crate::icons::glyph(k.icon).into(), desc: gallery_desc(k.id).into() }).collect();
        ui.set_kinds(ModelRc::new(VecModel::from(kinds)));
        let sizes = Rc::new(VecModel::<ui::GalSize>::default());
        ui.set_sizes(ModelRc::from(sizes.clone()));
        let to = st.shell.sender(NAME);
        let t = to.clone();
        ui.on_pick(move |i| t(Box::new(M::GalPick(i))));
        let t = to.clone();
        ui.on_add(move |i| t(Box::new(M::GalAdd(i))));
        let t = to.clone();
        ui.on_done(move || t(Box::new(M::GalDone)));
        ui.on_closed(move || to(Box::new(M::GalClosed)));
        ui.show().expect("show gallery");
        ui.set_shown(true);
        ui.invoke_focus_keys();
        self.gal = Some(Gal { output: name.to_string(), surface, ui, sizes, shown: true, glass_sent: String::new() });
        self.gallery_theme(st);
    }

    /// The picked kind, its words and its sizes
    fn gallery_fill(&mut self, st: &mut State) {
        let picked = self.gal_picked.min(grid::KINDS.len() - 1);
        let def = &grid::KINDS[picked];
        let material = st.shell.look.material;
        let mut rows = Vec::new();
        let mut tallest = 0.0f32;
        for size in def.sizes {
            let w = W { kind: def.id.to_string(), size: size.to_string(), col: 0, row: 0, id: String::new() };
            let key = format!("gallery:{}:{size}", def.id);
            let mut widget = self.widget_of(st, w, 0, &key);
            widget.x = 0.0;
            widget.y = 0.0;
            let (pw, ph) = grid::pixel_size(size);
            tallest = tallest.max(ph);
            rows.push(ui::GalSize { key: key.as_str().into(), name: grid::size_name(size).into(), w: pw, h: ph, widget });
        }
        let Some(g) = self.gal.as_mut() else { return };
        g.ui.set_picked(picked as i32);
        g.ui.set_kind_label(def.label.into());
        g.ui.set_kind_desc(gallery_desc(def.id).into());
        g.ui.set_row_h(row_height(tallest, material));
        g.ui.set_on_glass(st.shell.plugin && !material);
        while g.sizes.row_count() > rows.len() {
            g.sizes.remove(g.sizes.row_count() - 1);
        }
        sync(&g.sizes, rows);
    }

    pub(crate) fn gallery_theme(&mut self, st: &mut State) {
        if self.gal.is_none() {
            return;
        }
        self.colors(st);
        let look = st.shell.look.clone();
        let green = self.accent(st, "green");
        let red = self.accent(st, "red");
        let yellow = self.accent(st, "yellow");
        let (on_tertiary, secondary) = (self.role("onTertiary"), self.role("secondary"));
        if let Some(g) = self.gal.as_mut() {
            st.shell.apply_theme(&g.ui.global::<ui::Theme>());
            let d = g.ui.global::<ui::Desk>();
            d.set_green(green);
            d.set_red(red);
            d.set_yellow(yellow);
            let accent = g.ui.global::<ui::Theme>().get_accent();
            d.set_accent_ink(if look.material || look.dark { accent } else { tint(accent, Color::from_rgb_u8(0, 0, 0), 0.12) });
            d.set_m3_on_tertiary(on_tertiary);
            d.set_m3_secondary(secondary);
            d.set_tile(if look.material { 28.0 } else { 36.0 });
            g.glass_sent.clear();
        }
        self.gallery_fill(st);
    }

    pub(crate) fn gallery_pick(&mut self, st: &mut State, i: i32) {
        self.gal_picked = (i.max(0) as usize).min(grid::KINDS.len() - 1);
        self.gallery_fill(st);
    }

    /// A size clicked: the widget goes onto the desktop
    pub(crate) fn gallery_add(&mut self, st: &mut State, i: i32) {
        let def = &grid::KINDS[self.gal_picked.min(grid::KINDS.len() - 1)];
        let Some(size) = def.sizes.get(i.max(0) as usize) else { return };
        self.add(st, def.id, size);
        self.update(st);
    }

    /// Done or Escape: Edit Widgets ends
    pub(crate) fn gallery_done(&mut self, st: &mut State) {
        ask("endEdit", vec![]);
        shell::ask("panels.set", json!(["widgetEditing", false]));
        self.feed.editing = false;
        self.gallery_sync(st);
        self.update(st);
    }

    /// The fade out is over: the sheet goes
    pub(crate) fn gallery_closed(&mut self, st: &mut State) {
        if self.gal.as_ref().is_some_and(|g| !g.shown) {
            self.gallery_drop(st);
        }
    }

    pub(crate) fn gallery_output_gone(&mut self, st: &mut State, name: &str) {
        if self.gal.as_ref().is_some_and(|g| g.output == name) {
            self.gallery_drop(st);
        }
    }

    fn gallery_drop(&mut self, st: &mut State) {
        if let Some(g) = self.gal.take() {
            st.shell.glass.set(&g.output, NS, &[]);
            g.surface.hide(); // (unmapped now: dropping the window alone leaves it on screen)
            st.drop_surface(&g.surface);
        }
    }

    /// Glass: the sheet's shape on the plugin's glass, with its spring
    pub(crate) fn gallery_frame(&mut self, st: &mut State) {
        let Some(g) = self.gal.as_mut() else { return };
        if !st.shell.plugin || st.shell.look.material || !g.surface.mapped.get() {
            return;
        }
        let ui = &g.ui;
        let fade = ui.get_fade();
        let shape = theme::glass_shape(&st.shell.look, ui.get_body_x(), ui.get_body_y() + (1.0 - fade) * 40.0, ui.get_body_w(), ui.get_body_h(), 28.0, fade, 1.0);
        let key = format!("{shape:?}");
        if key != g.glass_sent {
            st.shell.glass.set(&g.output, NS, if fade > 0.002 { std::slice::from_ref(&shape) } else { &[] });
            g.glass_sent = key;
        }
    }
}

fn gallery_desc(id: &str) -> &'static str {
    desc(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_words() {
        for k in grid::KINDS {
            assert!(!desc(k.id).is_empty(), "{}", k.id);
        }
    }

    #[test]
    fn the_sheet_and_its_row() {
        assert_eq!(sheet_width(1920.0), 880.0);
        assert_eq!(sheet_width(800.0), 752.0);
        assert!((row_height(344.0, false) - 220.64).abs() < 0.01);
    }
}
