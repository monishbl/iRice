//! Search (rust.surfaces: spotlight): apps, actions, files, sums and answers,
//! conversions, definitions, emoji, commands, the clipboard, and the "Just
//! ask" hand-off to the AI, as you type. Glass: a floating glass field, the
//! results in a second glass panel below; Material: the docked search view,
//! segmented lists under a hero card. Compare shell/modules/spotlight/
//! (SpotlightGlass.qml, SearchView.qml, SearchModel.qml and their rows) and
//! shell/services/Search.qml.
//!
//! It opens as the QML one did, through Panels ("spotlight": its shortcut,
//! the menu bar's search button, `qs ipc call`): RustUi.qml forwards the
//! open and close, and it says when it closed itself.
//!
//! The ranking is Rust's (rank.rs: Search.query's order, tested as pure
//! functions); the rows only the QML shell knows (plain-language requests,
//! the calendar, contacts, notes, screenshots, the App Store, the clipboard,
//! Chromecasts) come from it on stdin (`{"to":"spotlight","extras":…}`,
//! asked with `search.query`) and run there by key (`search.run`). The AI's
//! quick answer runs in the shell too (`ai.quickAsk`; its progress arrives
//! as `{"quick":…}` lines).
//!
//! Not like the QML yet, where Slint can't do the same:
//! - Material's icons are Material Symbols in the font's default fill (Slint
//!   can't set the FILL axis); shapes are polar outlines near the QML's
//!   shader shapes (cookie, sunny), not the same curves;
//! - the AI answer is CommonMark through Slint's StyledText (no headings,
//!   tables or code boxes, which the QML's rich text draws); links open only
//!   when they're https (the shell's launch.open);
//! - springs are Slint's spring() easing at the QML spring's feel, and the
//!   rows' entrance is an eased stagger, not a spring each;
//! - no tooltips on the field's buttons; Glass's Tab to the buttons is a
//!   focus ring the panel draws (the field keeps the keyboard);
//! - Material's loading row shows the shared Busy arc, not the QML's
//!   morphing loading indicator.
//! And one not Slint's: apps with exactly the same score keep the order of
//! the desktop-file scan; Quickshell's list is in hash order (not the same
//! from one run to the next), so such ties can come out in another order.

pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;

pub mod layout;
pub mod providers;
pub mod rank;

use tessera_core::appicon::{self, Style};
use tessera_core::services::apps::Apps;
use tessera_core::services::hypr::Glass;
use tessera_core::services::icons::Icons;
use tessera_core::shell::{Command, Part};
use tessera_core::surfaces::shapes;
use tessera_core::theme;
use tessera_core::wayland::{Spec, State, Surface};
use layout::Entry;
use rank::{AiMeta, Answer, Definition, Extras, Known, Row, Run};
use slint::platform::Key;
use slint::{ComponentHandle, Model, ModelRc, SharedPixelBuffer, SharedString, VecModel};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;
use std::any::Any;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

const NAME: &str = "spotlight";
pub const NS: &str = "tessera:spotlight";

type Px = SharedPixelBuffer<slint::Rgba8Pixel>;

/// Its own messages (Msg::Part("spotlight", …))
enum M {
    Key(String, bool, bool),
    Edited(String),
    Activate(i32),
    Look(i32),
    Cleared,
    Ask,
    Back,
    Continue,
    CopyAnswer,
    Link(String),
    Dismissed,
    Closed,
    AskAnswer,
    Answered(u64, Answer),
    Converted(String, String),
    Defined(String, Definition),
    FindFiles,
    Files(String, Vec<(bool, String)>),
    Contents(String, Vec<String>),
    Hold,
    IntroEnd,
    Idle,
    Icons(Vec<(IconKey, Option<Px>)>),
    Picture(String, Option<Px>),
    Fetched(String, Option<PathBuf>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct IconKey {
    path: PathBuf,
    px: u32,
    style: Style,
}

/// What the field's key callback has to know at once (Slint wants the answer
/// synchronously)
#[derive(Clone, Copy, Default)]
struct KeyCtx {
    material: bool,
    has_path: bool,
    focus: i32,
}

/// The keys the panel takes from the field (the field gets the rest)
fn handles(c: KeyCtx, text: &str, shift: bool, ctrl: bool) -> bool {
    let is = |k: Key| text == SharedString::from(k).as_str();
    if is(Key::Escape) || is(Key::Return) || is(Key::UpArrow) || is(Key::DownArrow) || is(Key::Tab) {
        return true;
    }
    if is(Key::Backtab) {
        return !c.material;
    }
    if c.focus != 0 && text == " " {
        return true;
    }
    c.has_path && ((text == " " && shift) || (ctrl && text.eq_ignore_ascii_case("y")))
}

/// Glass's Tab and Shift+Tab: the field (0) → through the results, or to
/// Clear (1) and Ask (2), and back (SpotlightGlass.qml's KeyNavigation)
fn glass_tab(focus: i32, back: bool, rows: usize, has_text: bool) -> (i32, bool) {
    match (focus, back) {
        (_, true) if focus == 0 => (2, false),
        (2, true) => (if has_text { 1 } else { 0 }, false),
        (1, true) => (0, false),
        (0, false) if rows > 1 => (0, true), // (next result)
        (0, false) => (if has_text { 1 } else { 2 }, false),
        (1, false) => (2, false),
        _ => (0, false),
    }
}

/// Glass's answer card colour (AnswerCard.qml's tone)
fn tone(r: &Row, dark: bool, accent: slint::Color) -> slint::Color {
    let a = |light: &str, dk: &str| hexc(if dark { dk } else { light });
    if r.section == "Do" {
        return accent;
    }
    match r.kind.as_str() {
        "math" => return a("#FF9500", "#FF9F0A"),
        "date" => return a("#FF3B30", "#FF453A"),
        "time" => return a("#007AFF", "#0A84FF"),
        "unit" | "currency" => return a("#34C759", "#30D158"),
        _ => {}
    }
    match r.section.as_str() {
        "Calculator" => a("#FF9500", "#FF9F0A"),
        "Conversion" => a("#34C759", "#30D158"),
        "Dictionary" => a("#AF52DE", "#BF5AF2"),
        _ => accent,
    }
}

fn hexc(s: &str) -> slint::Color {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0);
    slint::Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// What Enter does with a card (AnswerCard.qml, SearchHero.qml)
fn card_hint(r: &Row) -> &'static str {
    match r.section.as_str() {
        "Top Hit" => "",
        "Do" => "Enter to do it",
        "Dictionary" => "Enter opens it",
        _ => "Enter copies it",
    }
}

/// The line under (or beside) a row's title, as each look writes it
pub fn sub_line(r: &Row, material: bool, card: bool) -> String {
    let store_row = if card { r.section == "Top Hit" && r.subtitle.starts_with("Get · ") } else { r.section == "Software" && !r.loading };
    let body = if material && store_row { r.subtitle.trim_start_matches("Get · ").to_string() } else { r.subtitle.clone() };
    if !card {
        return body;
    }
    [body.as_str(), card_hint(r)].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · ")
}

/// The shell's rows, from its extras line
fn shell_rows(v: &serde_json::Value) -> Vec<Row> {
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|r| {
                    let s = |k: &str| r[k].as_str().unwrap_or("").to_string();
                    Row {
                        section: s("section"),
                        title: s("title"),
                        subtitle: s("subtitle"),
                        glyph: s("glyph"),
                        icon: s("icon"),
                        icon_fallback: "application-x-executable",
                        picture: s("picture"),
                        path: s("path"),
                        loading: r["loading"].as_bool().unwrap_or(false),
                        run: Run::Shell(s("key")),
                        ..Default::default()
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_extras(v: &serde_json::Value) -> Extras {
    Extras {
        q: v["q"].as_str().unwrap_or("").trim().to_string(),
        intents: shell_rows(&v["intents"]),
        reminder: shell_rows(&v["reminder"]),
        store: shell_rows(&v["store"]),
        calendar: shell_rows(&v["calendar"]),
        contacts: shell_rows(&v["contacts"]),
        notes: shell_rows(&v["notes"]),
        shots: shell_rows(&v["shots"]),
        clipboard: shell_rows(&v["clipboard"]),
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Quick {
    busy: bool,
    answer: String,
    error: String,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::SpotlightPanel,
    a11y: Option<crate::a11y::A11y>,
    glass_sent: String,
}

pub struct Spotlight {
    win: Option<Win>,
    open: bool,
    text: String,
    rows: Vec<Row>,
    entries: Vec<Entry>,
    sel: usize,
    scroll: f32,
    focus: i32,
    key_ctx: Rc<Cell<KeyCtx>>,
    answering: bool,
    quick: Quick,
    ai: AiMeta,
    cast: Vec<rank::Action>,
    actions: Vec<rank::Action>,
    apps: Vec<rank::App>,
    extras: Option<Extras>,
    asked: Option<(String, bool)>,
    hold_until: Option<Instant>,
    had_rows: bool,
    // slow answers
    answerer: providers::Answerer,
    ans_query: String,
    answer: Option<(String, Answer)>,
    conv_query: String,
    conv: Option<(String, String)>,
    def_word: String,
    def: Option<(String, Definition)>,
    file_query: String,
    files: Vec<(bool, String)>,
    contents: Vec<String>,
    qalc: bool,
    plocate: bool,
    rg: bool,
    home: String,
    timers: [slint::Timer; 7],
    // pictures
    icons: HashMap<IconKey, slint::Image>,
    /// each row's icon render
    row_keys: Vec<Option<IconKey>>,
    pending: HashSet<IconKey>,
    pictures: HashMap<String, Option<slint::Image>>,
    /// App Store logos on the web (AppStore.iconFor's URL), fetched once into the cache
    remote: HashMap<String, Option<PathBuf>>,
}

impl Default for Spotlight {
    fn default() -> Self {
        Self {
            win: None,
            open: false,
            text: String::new(),
            rows: Vec::new(),
            entries: Vec::new(),
            sel: 0,
            scroll: 0.0,
            focus: 0,
            key_ctx: Rc::new(Cell::new(KeyCtx::default())),
            answering: false,
            quick: Quick::default(),
            ai: AiMeta::default(),
            cast: Vec::new(),
            actions: rank::actions(),
            apps: Vec::new(),
            extras: None,
            asked: None,
            hold_until: None,
            had_rows: false,
            answerer: providers::Answerer::new(),
            ans_query: String::new(),
            answer: None,
            conv_query: String::new(),
            conv: None,
            def_word: String::new(),
            def: None,
            file_query: String::new(),
            files: Vec::new(),
            contents: Vec::new(),
            qalc: false,
            plocate: false,
            rg: false,
            home: std::env::var("HOME").unwrap_or_default(),
            timers: Default::default(),
            icons: HashMap::new(),
            row_keys: Vec::new(),
            pending: HashSet::new(),
            pictures: HashMap::new(),
            remote: HashMap::new(),
        }
    }
}

// (the timers, by job)
const T_ANSWER: usize = 0;
const T_CONV: usize = 1;
const T_DEF: usize = 2;
const T_FILES: usize = 3;
const T_HOLD: usize = 4;
const T_INTRO: usize = 5;
const T_IDLE: usize = 6;

fn glyph(name: &str, material: bool, fill: bool, bold: bool) -> ui::SGlyph {
    let (text, family, scale) = providers::glyph(name, material, fill, bold);
    ui::SGlyph { text: text.into(), family: family.into(), scale }
}

impl Spotlight {
    fn send(st: &State) -> impl Fn(Box<dyn Any + Send>) + Clone + 'static {
        st.shell.sender(NAME)
    }

    fn after(&self, st: &State, t: usize, ms: u64, m: impl Fn() -> M + 'static) {
        let to = Self::send(st);
        self.timers[t].start(slint::TimerMode::SingleShot, Duration::from_millis(ms), move || to(Box::new(m())));
    }

    fn material(st: &State) -> bool {
        st.shell.look.material
    }

    /// The window on this output (made the first time; kept while closed)
    fn window(&mut self, st: &mut State, output: &str) -> bool {
        if self.win.as_ref().is_some_and(|w| w.output != output) {
            let w = self.win.take().unwrap();
            Glass::clear_now(&w.output, NS);
            st.drop_surface(&w.surface);
        }
        if self.win.is_some() {
            return true;
        }
        let Some(wl_output) = st.output_named(output) else { return false };
        let surface = st.prepare_surface(Spec {
            layer: Layer::Overlay,
            namespace: NS,
            output: Some((&wl_output, output)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::SpotlightPanel::new().expect("SpotlightPanel");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let to = Self::send(st);
        {
            let (to, ctx) = (to.clone(), self.key_ctx.clone());
            ui.on_key(move |text, shift, ctrl| {
                let ok = handles(ctx.get(), &text, shift, ctrl);
                if ok {
                    to(Box::new(M::Key(text.to_string(), shift, ctrl)));
                }
                ok
            });
        }
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let to = to.clone();
                ui.$set(move |$($a),*| to(Box::new($m)));
            }};
        }
        on!(on_edited, |t| M::Edited(t.to_string()));
        on!(on_activate, |i| M::Activate(i));
        on!(on_quick_look, |i| M::Look(i));
        on!(on_cleared, | | M::Cleared);
        on!(on_ask, | | M::Ask);
        on!(on_back, | | M::Back);
        on!(on_continue_chat, | | M::Continue);
        on!(on_copy_answer, | | M::CopyAnswer);
        on!(on_link, |l| M::Link(l.to_string()));
        on!(on_dismissed, | | M::Dismissed);
        on!(on_closed, | | M::Closed);
        ui.show().expect("show spotlight");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: output.into(), surface, ui, a11y, glass_sent: String::new() });
        self.look(st);
        true
    }

    /// Everything that follows the look and settings
    fn look(&mut self, st: &State) {
        let Some(w) = &self.win else { return };
        let ui = &w.ui;
        let m = Self::material(st);
        let look = &st.shell.look;
        ui.set_g_search(glyph("magnifying-glass", m, false, true));
        ui.set_g_clear(glyph(if m { "x" } else { "x-circle" }, m, true, false));
        ui.set_g_ask(glyph("sparkle", m, false, false));
        ui.set_g_ask_on(glyph("sparkle", m, true, false));
        ui.set_g_back(glyph("caret-left", m, false, false));
        ui.set_g_eye(glyph("eye", m, false, false));
        ui.set_g_spark(glyph("sparkle", false, true, false));
        ui.set_cookie9(shapes::outline("cookie9").into());
        ui.set_cookie12(shapes::outline("cookie12").into());
        ui.set_sunny(shapes::outline("sunny").into());
        ui.set_emoji_font(tessera_core::settings::emoji_font(&st.shell.cfg, &[]).into());
        let t = ui.global::<ui::Theme>();
        let accent = t.get_accent();
        if m {
            let roles = theme::material_roles(&look.seed, look.dark, &look.style);
            let r = |k: &str| hexc(roles.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or("#ff00ff"));
            ui.set_surface_bright(r("surfaceBright"));
            ui.set_on_tertiary(r("onTertiary"));
            ui.set_accent_tint(r("secondaryContainer"));
            ui.set_accent_ink(r("primary"));
        } else {
            // (Theme.accentTint: the accent faintly in dark mode, a pale shade of it in light)
            let pale = slint::Color::from_argb_u8(230, mixc(accent.red(), 255, 0.8), mixc(accent.green(), 255, 0.8), mixc(accent.blue(), 255, 0.8));
            ui.set_accent_tint(if look.dark { accent.with_alpha(0.26) } else { pale });
            ui.set_accent_ink(accent);
        }
        ui.set_scrim(t.get_m3_scrim());
        ui.set_glass_fill(glass_fill(st.shell.plugin, look.dark));
        ui.set_placeholder(if m { "Search apps, files and the web" } else { "Search" }.into());
        self.icons.clear();
        self.pending.clear();
    }

    fn open(&mut self, st: &mut State, screen: Option<String>) {
        let output = screen.or_else(|| self.win.as_ref().map(|w| w.output.clone())).or_else(|| st.outputs_now().first().map(|o| o.1.clone()));
        let Some(output) = output else { return };
        if !self.window(st, &output) {
            return;
        }
        st.shell.want("apps");
        st.shell.want("icons");
        self.open = true;
        // SearchModel.reset: no answer, an empty field, no results
        if self.answering || self.quick.busy {
            crate::shell::ask("ai.stopQuick", serde_json::json!([]));
        }
        self.answering = false;
        self.quick = Quick::default();
        self.text.clear();
        self.rows.clear();
        self.entries.clear();
        self.sel = 0;
        self.scroll = 0.0;
        self.focus = 0;
        self.had_rows = false;
        self.asked = None;
        crate::shell::ask("clipboard.refresh", serde_json::json!([]));
        // (the AI's name and the Chromecasts, before anything's typed)
        self.ask_extras(String::new(), false);
        self.push(st);
        let w = self.win.as_ref().unwrap();
        w.ui.invoke_open();
        w.ui.set_list_y(0.0);
        w.ui.set_shown(true);
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        w.surface.show(0, 0);
    }

    /// Close it (`tell`: it closed itself, so the shell's Panels follows)
    fn close(&mut self, tell: bool) {
        if !self.open {
            return;
        }
        self.open = false;
        if let Some(w) = &self.win {
            w.ui.set_shown(false);
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            w.surface.layer.commit();
        }
        if tell {
            crate::shell::closed(NAME);
        }
    }

    fn closed(&mut self, st: &mut State) {
        if self.open {
            return;
        }
        if let Some(w) = &mut self.win {
            w.surface.hide();
            st.shell.glass.set(&w.output, NS, &[]);
            w.glass_sent.clear();
        }
    }

    fn ask_extras(&mut self, q: String, store: bool) {
        let want = (q, store);
        if self.asked.as_ref() != Some(&want) {
            crate::shell::ask("search.query", serde_json::json!([want.0, want.1]));
            self.asked = Some(want);
        }
    }

    /// The rows again, for the field's text and what's known
    fn refresh(&mut self, st: &State, typed: bool) {
        let raw = self.text.trim().to_string();
        let ans = self.answer.as_ref().filter(|a| a.0 == raw).map(|a| &a.1);
        let conv = self.conv.as_ref().filter(|c| c.0 == raw).map(|c| c.1.as_str());
        let word = rank::word_to_define(&raw);
        let def = self.def.as_ref().filter(|d| !word.is_empty() && d.0 == word).map(|d| &d.1);
        let extras = self.extras.as_ref().filter(|e| e.q == raw);
        let mut actions = self.actions.clone();
        actions.extend(self.cast.iter().cloned());
        let k = Known {
            apps: &self.apps,
            actions: &actions,
            answer: ans,
            qalc: self.qalc,
            conversion: conv,
            definition: def,
            extras,
            ai: Some(&self.ai),
            emoji: if raw.starts_with(':') { providers::emoji() } else { &[] },
            files: &self.files,
            contents: &self.contents,
            home: &self.home,
        };
        let (rows, wants) = rank::query(&self.text, &k);
        let have_extras = extras.is_some();
        // What's still to ask, as Search.qml's debounces
        if let Some(q) = wants.answer {
            if q != self.ans_query {
                self.ans_query = q;
                self.answer = None;
                self.after(st, T_ANSWER, 90, || M::AskAnswer);
            }
        }
        if let Some(q) = wants.conversion {
            if q != self.conv_query {
                self.conv_query = q;
                self.conv = None;
                let q2 = self.conv_query.clone();
                let to = Self::send(st);
                self.timers[T_CONV].start(slint::TimerMode::SingleShot, Duration::from_millis(180), move || {
                    let (q, to) = (q2.clone(), to.clone());
                    std::thread::spawn(move || to(Box::new(M::Converted(q.clone(), providers::convert(&q)))));
                });
            }
        }
        if let Some(wd) = wants.define {
            if wd != self.def_word {
                self.def_word = wd;
                self.def = None;
                let w2 = self.def_word.clone();
                let to = Self::send(st);
                self.timers[T_DEF].start(slint::TimerMode::SingleShot, Duration::from_millis(300), move || {
                    let (w, to) = (w2.clone(), to.clone());
                    std::thread::spawn(move || to(Box::new(M::Defined(w.clone(), providers::define(&w)))));
                });
            }
        }
        if let Some(fq) = wants.files {
            if fq != self.file_query {
                self.file_query = fq;
                self.after(st, T_FILES, 200, || M::FindFiles);
            }
        }
        if let Some((q, store)) = wants.extras {
            self.ask_extras(q, store);
        }
        // (while the shell's rows for new text are on their way, what's shown
        // stays a moment: no flicker of a list without them)
        if typed {
            self.hold_until = (!have_extras && self.asked.as_ref().is_some_and(|a| a.0 == raw) && !raw.is_empty()).then(|| Instant::now() + Duration::from_millis(120));
            if self.hold_until.is_some() {
                self.after(st, T_HOLD, 130, || M::Hold);
            }
        }
        if !have_extras && self.hold_until.is_some_and(|t| Instant::now() < t) {
            return;
        }
        self.hold_until = None;
        self.sel = if typed { 0 } else { rank::keep_selection(&self.rows, self.sel, &rows) };
        self.rows = rows;
        if typed {
            self.scroll = 0.0;
            if let Some(w) = &self.win {
                w.ui.set_list_y(0.0);
            }
        }
        // Material: a list that just appeared comes in row by row
        let has = !self.rows.is_empty();
        if has && !self.had_rows && Self::material(st) {
            if let Some(w) = &self.win {
                w.ui.invoke_start_intro();
                w.ui.set_introducing(true);
            }
            self.after(st, T_INTRO, 700, || M::IntroEnd);
        }
        self.had_rows = has;
        self.rebuild(st);
    }

    /// The rows into the window
    fn rebuild(&mut self, st: &State) {
        let m = Self::material(st);
        let (entries, h) = if m { layout::material(&self.rows) } else { layout::glass(&self.rows) };
        self.entries = entries;
        let Some(w) = &self.win else { return };
        let t = w.ui.global::<ui::Theme>();
        let accent = t.get_accent();
        let dark = st.shell.look.dark;
        let mut want_icons = Vec::new();
        let mut want_remote: Vec<String> = Vec::new();
        let mut row_keys = Vec::new();
        let mut want_pics = Vec::new();
        let icons = st.shell.get::<Icons>().cloned();
        let scale = w.ui.window().scale_factor().max(1.0);
        let style_plain = Style::from_settings(&serde_json::json!({"appearance": {"iconStyle": "plain", "iconShape": "none"}}), m, dark, [0, 0, 0], ([0, 0, 0], [0, 0, 0]));
        let style_app = {
            let themed = (rgb(t.get_m3_primary_container()), rgb(t.get_m3_on_primary_container()));
            Style::from_settings(&st.shell.cfg, m, dark, rgb(accent), themed)
        };
        let out: Vec<ui::SRow> = self
            .rows
            .iter()
            .zip(&self.entries)
            .enumerate()
            .map(|(i, (r, e))| {
                // the icon: looked up in the theme, drawn as the look draws it
                let size = if m && e.card { 40.0 } else { 26.0 };
                let styled = !m && (r.section == "Applications" || (r.section == "Top Hit" && matches!(r.run, Run::App(_))));
                let mut icon = None;
                let mut has_icon = false;
                let mut row_key = None;
                let web = r.icon.starts_with("https://") || r.icon.starts_with("http://");
                if web && !r.loading && !self.remote.contains_key(&r.icon) {
                    want_remote.push(r.icon.clone());
                }
                let local = if web { self.remote.get(&r.icon).cloned().flatten() } else { None };
                if !r.icon.is_empty() && !r.loading && (!web || local.is_some()) {
                    let found = match local {
                        Some(p) => Some(p),
                        None => icons.as_ref().and_then(|ic| ic.path(&[r.icon.as_str()], &[r.icon_fallback])),
                    };
                    if let Some(p) = found {
                        let key = IconKey { path: p, px: (size * scale).ceil() as u32, style: if styled { style_app.clone() } else { style_plain.clone() } };
                        has_icon = true;
                        match self.icons.get(&key) {
                            Some(img) => icon = Some(img.clone()),
                            None => want_icons.push(key.clone()),
                        }
                        row_key = Some(key);
                    }
                }
                let mut picture = None;
                if !r.picture.is_empty() {
                    match self.pictures.get(&r.picture) {
                        Some(p) => picture = p.clone(),
                        None => want_pics.push(r.picture.clone()),
                    }
                }
                row_keys.push(row_key);
                let g = if r.glyph.is_empty() { if e.card { "sparkle" } else { "circle" } } else { r.glyph.as_str() };
                let (hero_kind, key, key_label) = match r.section.as_str() {
                    "Top Hit" => (0, "arrow-up-right", "Open"),
                    "Do" => (1, "arrow-up-right", "Open"),
                    "Dictionary" => (2, "arrow-square-out", "Open"),
                    _ => (2, "copy", "Copy"),
                };
                ui::SRow {
                    y: e.y,
                    h: e.h,
                    head: e.head.as_str().into(),
                    head_y: e.head_y,
                    row_y: e.row_y,
                    row_h: e.row_h,
                    card: e.card,
                    first: e.first,
                    last: e.last,
                    index: i as i32,
                    section: r.section.as_str().into(),
                    title: r.title.as_str().into(),
                    sub: sub_line(r, m, e.card).into(),
                    glyph: glyph(g, m, false, false),
                    glyph_fill: glyph(g, m, true, false),
                    icon: icon.unwrap_or_default(),
                    has_icon,
                    emoji: r.emoji.as_str().into(),
                    picture: picture.clone().unwrap_or_default(),
                    has_picture: picture.is_some(),
                    has_path: !r.path.is_empty(),
                    loading: r.loading,
                    store: if e.card { r.section == "Top Hit" && r.subtitle.starts_with("Get · ") } else { m && r.section == "Software" && !r.loading },
                    tone: tone(r, dark, accent),
                    hero_kind,
                    key: glyph(key, m, false, false),
                    key_label: key_label.into(),
                }
            })
            .collect();
        self.row_keys = row_keys;
        w.ui.set_rows(ModelRc::new(VecModel::from(out)));
        w.ui.set_content_h(h);
        self.request_icons(st, want_icons);
        self.request_pictures(st, want_pics);
        for u in want_remote {
            self.remote.insert(u.clone(), None);
            let to = Self::send(st);
            std::thread::spawn(move || {
                let p = providers::fetch_icon(&u);
                to(Box::new(M::Fetched(u, p)));
            });
        }
        self.push(st);
    }

    fn request_icons(&mut self, st: &State, want: Vec<IconKey>) {
        let missing: Vec<IconKey> = want.into_iter().filter(|k| !self.pending.contains(k)).collect::<HashSet<_>>().into_iter().collect();
        if missing.is_empty() {
            return;
        }
        self.pending.extend(missing.iter().cloned());
        let to = Self::send(st);
        std::thread::spawn(move || {
            let out = missing.into_iter().map(|k| {
                let img = appicon::render(&k.path, k.px, &k.style, None);
                (k, img)
            });
            to(Box::new(M::Icons(out.collect())));
        });
    }

    fn request_pictures(&mut self, st: &State, want: Vec<String>) {
        for p in want {
            if self.pictures.contains_key(&p) {
                continue;
            }
            self.pictures.insert(p.clone(), None);
            let to = Self::send(st);
            std::thread::spawn(move || {
                let px = image::open(&p).ok().map(|i| {
                    let t = i.resize_to_fill(96, 96, image::imageops::FilterType::Triangle).to_rgba8();
                    let mut b = Px::new(t.width(), t.height());
                    b.make_mut_bytes().copy_from_slice(t.as_raw());
                    b
                });
                to(Box::new(M::Picture(p, px)));
            });
        }
    }

    /// Selection, the answer and the field's state into the window
    fn push(&mut self, st: &State) {
        let Some(w) = &self.win else { return };
        let ui = &w.ui;
        let m = Self::material(st);
        ui.set_sel(self.sel as i32);
        ui.set_answering(self.answering);
        ui.set_button_focus(if m { 0 } else { self.focus });
        ui.set_quick_busy(self.quick.busy);
        let shown = !self.quick.answer.is_empty() || !self.quick.error.is_empty();
        ui.set_quick_shown(shown);
        ui.set_quick_error(self.quick.error.as_str().into());
        ui.set_quick_answer(slint::StyledText::from_markdown(&self.quick.answer).unwrap_or_else(|_| slint::StyledText::from_plain_text(&self.quick.answer)));
        ui.set_quick_done(!self.quick.busy && !self.quick.answer.is_empty() && self.quick.error.is_empty());
        ui.set_model_name(self.ai.name.as_str().into());
        let short = rank::ai_row("", Some(&self.ai)).title;
        ui.set_ask_label(if self.ai.configured { short.into() } else { "Set up AI".into() });
        let has_path = self.rows.get(self.sel).is_some_and(|r| !r.path.is_empty());
        self.key_ctx.set(KeyCtx { material: m, has_path, focus: if m { 0 } else { self.focus } });
    }

    fn reveal(&mut self, st: &State) {
        let view = if Self::material(st) { layout::MATERIAL_MAX } else { layout::GLASS_MAX };
        let s = layout::reveal(&self.entries, self.sel, self.scroll, view);
        // (the Flickable may have been scrolled by hand)
        if let Some(w) = &self.win {
            let cur = -w.ui.get_list_y();
            let s2 = layout::reveal(&self.entries, self.sel, cur, view);
            if s2 != cur {
                w.ui.set_list_y(-s2);
            }
            self.scroll = s2;
            return;
        }
        self.scroll = s;
    }

    fn select(&mut self, st: &State, i: usize) {
        self.sel = i;
        self.reveal(st);
        self.push(st);
    }

    /// SearchModel.run: an AI row answers in place; the rest close and run
    fn run(&mut self, st: &mut State, i: usize) {
        let Some(r) = self.rows.get(i).cloned() else { return };
        if r.ai {
            if let Run::Ask(q) = &r.run {
                self.ask_ai(q.clone());
            }
            self.push(st);
            return;
        }
        self.close(true);
        self.run_row(st, &r.run);
    }

    fn run_row(&mut self, st: &State, run: &Run) {
        use serde_json::json;
        match run {
            Run::Nothing | Run::Ask(_) => {}
            Run::Copy(t) => crate::util::pipe_to(&["wl-copy"], t.clone().into_bytes()),
            Run::App(id) => {
                if let Some(e) = st.shell.get::<Apps>().and_then(|a| a.by_id(id)) {
                    crate::services::apps::launch(e);
                }
            }
            Run::Url(u) => crate::shell::ask("launch.open", json!([u])),
            Run::Terminal(c) => crate::shell::ask("search.command", json!([c])),
            Run::File(p) => crate::shell::ask("files.open", json!([p])),
            Run::Action(t) => crate::shell::ask("search.action", json!([t])),
            Run::Shell(k) => crate::shell::ask("search.run", json!([k])),
            Run::SetupAi => crate::shell::ask("panels.openSettings", json!(["ai"])),
        }
    }

    fn ask_ai(&mut self, q: String) {
        self.answering = true;
        self.quick = Quick { busy: true, ..Default::default() };
        crate::shell::ask("ai.quickAsk", serde_json::json!([q]));
    }

    /// The sparkle: ask about what's typed (or open the chat)
    fn ask_button(&mut self, st: &mut State) {
        let t = self.text.trim();
        let q = t.strip_prefix('?').unwrap_or(t).trim().to_string();
        if q.is_empty() {
            crate::shell::ask("panels.show", serde_json::json!(["sidebar"]));
            return;
        }
        let r = rank::ai_row(&q, Some(&self.ai));
        if r.ai {
            self.ask_ai(q);
            self.push(st);
        } else {
            self.close(true);
            self.run_row(st, &r.run);
        }
        if let Some(w) = &self.win {
            w.ui.invoke_focus_field();
        }
    }

    fn back(&mut self, st: &State) {
        self.answering = false;
        crate::shell::ask("ai.stopQuick", serde_json::json!([]));
        self.refresh(st, true);
        if let Some(w) = &self.win {
            w.ui.invoke_focus_field();
        }
    }

    fn key(&mut self, st: &mut State, text: &str, shift: bool, ctrl: bool) {
        let is = |k: Key| text == SharedString::from(k).as_str();
        let m = Self::material(st);
        let has_text = !self.text.is_empty();
        if is(Key::Escape) {
            self.close(true);
        } else if is(Key::Return) || (self.focus != 0 && text == " ") {
            match self.focus {
                1 => {
                    self.focus = 0;
                    if let Some(w) = &self.win {
                        w.ui.invoke_set_text("".into());
                    }
                    self.edited(st, String::new());
                }
                2 => self.ask_button(st),
                _ if self.answering => crate::shell::ask("ai.continueQuick", serde_json::json!([])),
                _ => self.run(st, self.sel),
            }
        } else if is(Key::DownArrow) {
            self.focus = 0;
            let i = (self.sel + 1).min(self.rows.len().saturating_sub(1));
            self.select(st, i);
        } else if is(Key::UpArrow) {
            self.focus = 0;
            let i = self.sel.saturating_sub(1);
            self.select(st, i);
        } else if is(Key::Tab) || is(Key::Backtab) {
            let back = is(Key::Backtab) || shift;
            if m {
                let i = (self.sel + 1) % self.rows.len().max(1);
                self.select(st, i);
                return;
            }
            let (f, next) = glass_tab(self.focus, back, self.rows.len(), has_text);
            self.focus = f;
            if next {
                let i = (self.sel + 1) % self.rows.len().max(1);
                self.select(st, i);
            } else {
                self.push(st);
            }
        } else if (text == " " && shift) || (ctrl && text.eq_ignore_ascii_case("y")) {
            if let Some(p) = self.rows.get(self.sel).map(|r| r.path.clone()).filter(|p| !p.is_empty()) {
                crate::shell::ask("panels.quickLook", serde_json::json!([p]));
            }
        }
    }

    fn edited(&mut self, st: &State, t: String) {
        self.focus = 0;
        if self.answering {
            self.answering = false;
            crate::shell::ask("ai.stopQuick", serde_json::json!([]));
        }
        self.text = t;
        self.refresh(st, true);
    }

    fn find_files(&mut self, st: &State) {
        let fq = self.file_query.clone();
        self.contents.clear();
        let (to, plocate) = (Self::send(st), self.plocate);
        let q = fq.clone();
        std::thread::spawn(move || {
            let out = providers::run_lines(&providers::names_command(&q, plocate));
            to(Box::new(M::Files(q, providers::parse_names(&out))));
        });
        if self.rg && rank::js_len(&fq) >= 3 {
            let to = Self::send(st);
            std::thread::spawn(move || {
                let out = providers::run_lines(&providers::contents_command(&fq));
                to(Box::new(M::Contents(fq, out.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect())));
            });
        }
    }

    fn apps_changed(&mut self, st: &State) {
        let Some(a) = st.shell.get::<Apps>() else { return };
        self.apps = a
            .entries
            .iter()
            .filter(|e| !e.no_display)
            .map(|e| rank::App { id: e.id.clone(), name: e.name.clone(), generic: e.generic_name.clone(), comment: e.comment.clone(), keywords: e.keywords.clone(), icon: e.icon.clone() })
            .collect();
    }
}

/// Glass's panels: over the plugin's glass nothing (the plugin tints it, the
/// QML item draws only its shadow); without it, the frosted fallback's tint
/// (Theme.glass.frost) over Hyprland's blur
pub fn glass_fill(plugin: bool, dark: bool) -> slint::Color {
    let c = |r: f32, g: f32, b: f32, a: f32| slint::Color::from_argb_f32(a, r, g, b);
    match (plugin, dark) {
        (true, _) => c(0.0, 0.0, 0.0, 0.0),
        (false, true) => c(0.11, 0.11, 0.13, 0.62),
        (false, false) => c(0.97, 0.97, 0.98, 0.66),
    }
}

fn rgb(c: slint::Color) -> [u8; 3] {
    [c.red(), c.green(), c.blue()]
}

fn mixc(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

impl Part for Spotlight {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        st.shell.want("apps");
        st.shell.want("icons");
        self.qalc = crate::util::on_path("qalc");
        self.plocate = crate::util::on_path("plocate");
        self.rg = crate::util::on_path("rg");
    }

    fn command(&mut self, st: &mut State, cmd: &Command) {
        match cmd.cmd.as_str() {
            "open" => self.open(st, cmd.screen.clone()),
            "close" => self.close(false),
            _ => {}
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        if let Some(w) = &mut self.win {
            if look {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
                w.glass_sent.clear();
            }
        }
        self.look(st);
        if self.open {
            self.rebuild(st);
        }
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = &mut self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.ui.set_glass_fill(glass_fill(st.shell.plugin, st.shell.look.dark));
            w.glass_sent.clear();
        }
    }

    fn data(&mut self, st: &mut State, what: std::any::TypeId) {
        if what == std::any::TypeId::of::<Apps>() {
            self.apps_changed(st);
            if self.open {
                self.refresh(st, false);
            }
        } else if what == std::any::TypeId::of::<Icons>() && self.open {
            self.rebuild(st);
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        // A line from the shell: its rows for a query, the AI's progress
        let msg = match msg.downcast::<serde_json::Value>() {
            Ok(v) => {
                if v["extras"].is_object() {
                    let x = &v["extras"];
                    if x["ai"].is_object() {
                        self.ai = AiMeta { configured: x["ai"]["configured"].as_bool().unwrap_or(false), name: x["ai"]["name"].as_str().unwrap_or("").into() };
                    }
                    if let Some(c) = x["cast"].as_array() {
                        self.cast = c
                            .iter()
                            .map(|a| rank::Action { title: a["title"].as_str().unwrap_or("").into(), glyph: a["glyph"].as_str().unwrap_or("").into(), keywords: a["keywords"].as_str().unwrap_or("").into() })
                            .collect();
                    }
                    self.extras = Some(parse_extras(x));
                    if self.open {
                        self.refresh(st, false);
                    }
                }
                if v["quick"].is_object() {
                    let q = &v["quick"];
                    self.quick = Quick { busy: q["busy"].as_bool().unwrap_or(false), answer: q["answer"].as_str().unwrap_or("").into(), error: q["error"].as_str().unwrap_or("").into() };
                    self.push(st);
                }
                return;
            }
            Err(m) => m,
        };
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Key(t, shift, ctrl) => self.key(st, &t, shift, ctrl),
            M::Edited(t) => self.edited(st, t),
            M::Activate(i) => self.run(st, i.max(0) as usize),
            M::Look(i) => {
                if let Some(p) = self.rows.get(i.max(0) as usize).map(|r| r.path.clone()) {
                    crate::shell::ask("panels.quickLook", serde_json::json!([p]));
                }
            }
            M::Cleared => self.edited(st, String::new()),
            M::Ask => self.ask_button(st),
            M::Back => self.back(st),
            M::Continue => crate::shell::ask("ai.continueQuick", serde_json::json!([])),
            M::CopyAnswer => {
                crate::util::pipe_to(&["wl-copy"], self.quick.answer.clone().into_bytes());
                self.close(true);
            }
            M::Link(l) => crate::shell::ask("launch.open", serde_json::json!([l])),
            M::Dismissed => self.close(true),
            M::Closed => self.closed(st),
            M::AskAnswer => {
                let to = Self::send(st);
                let q = self.ans_query.clone();
                self.answerer.ask(&q, move |id, a| to(Box::new(M::Answered(id, a))));
                self.after(st, T_IDLE, 120_000, || M::Idle);
            }
            M::Answered(id, a) => {
                if id == self.answerer.id {
                    self.answer = Some((self.ans_query.clone(), a));
                    if self.open && !self.answering {
                        self.refresh(st, false);
                    }
                }
            }
            M::Idle => self.answerer.stop(),
            M::Converted(q, r) => {
                if q == self.conv_query {
                    self.conv = Some((q, r));
                    if self.open && !self.answering {
                        self.refresh(st, false);
                    }
                }
            }
            M::Defined(w, d) => {
                if w == self.def_word {
                    self.def = Some((w, d));
                    if self.open && !self.answering {
                        self.refresh(st, false);
                    }
                }
            }
            M::FindFiles => self.find_files(st),
            M::Files(q, f) => {
                if q == self.file_query {
                    self.files = f;
                    if self.open && !self.answering {
                        self.refresh(st, false);
                    }
                }
            }
            M::Contents(q, c) => {
                if q == self.file_query {
                    self.contents = c;
                    if self.open && !self.answering {
                        self.refresh(st, false);
                    }
                }
            }
            M::Hold => {
                if self.hold_until.is_some() && self.open {
                    self.hold_until = Some(Instant::now());
                    self.refresh(st, false);
                }
            }
            M::IntroEnd => {
                if let Some(w) = &self.win {
                    w.ui.set_introducing(false);
                }
            }
            M::Icons(list) => {
                for (k, px) in list {
                    self.pending.remove(&k);
                    if let Some(px) = px {
                        self.icons.insert(k, slint::Image::from_rgba8(px));
                    }
                }
                if self.icons.len() > 400 {
                    self.icons.clear();
                }
                self.update_images();
            }
            M::Fetched(u, p) => {
                let got = p.is_some();
                self.remote.insert(u, p);
                if got && self.open {
                    self.rebuild(st);
                }
            }
            M::Picture(p, px) => {
                self.pictures.insert(p, px.map(slint::Image::from_rgba8));
                if self.pictures.len() > 200 {
                    self.pictures.clear();
                }
                self.update_images();
            }
        }
    }

    fn keyboard_left(&mut self, _st: &mut State, s: &Rc<Surface>) {
        if self.open && self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.close(true);
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            let was = self.open;
            self.close(was);
            let w = self.win.take().unwrap();
            st.drop_surface(&w.surface);
        }
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            let name = s.output.clone();
            self.output_removed(st, &name);
        }
    }

    /// Glass: the field's and the results' glass for the plugin, as they
    /// move; the accessibility tree
    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let Some(w) = &mut self.win else { return };
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
        if !w.surface.mapped.get() || !st.shell.plugin || st.shell.look.material {
            return;
        }
        let ui = &w.ui;
        let o = ui.get_openness().clamp(0.0, 1.0);
        let (x, y) = (ui.get_g_x(), ui.get_g_top());
        let mut shapes = vec![theme::glass_shape(&st.shell.look, x, y, 680.0, 56.0, 28.0, o, 1.0)];
        let bh = ui.get_g_body_h();
        if bh > 0.0 {
            shapes.push(theme::glass_shape(&st.shell.look, x, y + 64.0, 680.0, bh, 24.0, o, 1.0));
        }
        let key = format!("{shapes:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, if o > 0.002 { &shapes } else { &[] });
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    fn stats(&self) {
        if let Some(w) = &self.win {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report("spotlight"));
        }
    }

    fn quit(&mut self, _: &mut State) {
        self.answerer.stop();
        if let Some(w) = &self.win {
            Glass::clear_now(&w.output, NS);
        }
    }
}

impl Spotlight {
    /// Icons and pictures that arrived: into the rows already shown
    fn update_images(&mut self) {
        let Some(w) = &self.win else { return };
        let model = w.ui.get_rows();
        for (i, r) in self.rows.iter().enumerate() {
            let Some(mut row) = model.row_data(i) else { continue };
            let mut changed = false;
            if let Some(img) = self.row_keys.get(i).and_then(|k| k.as_ref()).and_then(|k| self.icons.get(k)) {
                if row.icon.size().width == 0 {
                    row.icon = img.clone();
                    changed = true;
                }
            }
            if !r.picture.is_empty() && !row.has_picture {
                if let Some(Some(p)) = self.pictures.get(&r.picture) {
                    row.picture = p.clone();
                    row.has_picture = true;
                    changed = true;
                }
            }
            if changed {
                model.set_row_data(i, row);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn k(key: Key) -> String {
        SharedString::from(key).to_string()
    }

    #[test]
    fn keys_it_takes_from_the_field() {
        let glass = KeyCtx::default();
        let material = KeyCtx { material: true, ..Default::default() };
        for key in [Key::Escape, Key::Return, Key::UpArrow, Key::DownArrow, Key::Tab] {
            assert!(handles(glass, &k(key), false, false) && handles(material, &k(key), false, false));
        }
        assert!(handles(glass, &k(Key::Backtab), true, false));
        assert!(!handles(material, &k(Key::Backtab), true, false));
        assert!(!handles(glass, "a", false, false) && !handles(glass, &k(Key::Backspace), false, false));
        // Quick Look only for a file
        assert!(!handles(glass, " ", true, false));
        let file = KeyCtx { has_path: true, ..Default::default() };
        assert!(handles(file, " ", true, false) && handles(file, "y", false, true));
        assert!(!handles(file, " ", false, false));
        // a focused button takes Space
        assert!(handles(KeyCtx { focus: 2, ..Default::default() }, " ", false, false));
    }

    #[test]
    fn glass_tab_order() {
        // Tab moves through the results; with one or none, to the buttons
        assert_eq!(glass_tab(0, false, 3, true), (0, true));
        assert_eq!(glass_tab(0, false, 1, true), (1, false));
        assert_eq!(glass_tab(0, false, 0, false), (2, false));
        assert_eq!(glass_tab(1, false, 3, true), (2, false));
        assert_eq!(glass_tab(2, false, 3, true), (0, false));
        // Shift+Tab: Ask, then Clear (while there's text), then the field
        assert_eq!(glass_tab(0, true, 3, true), (2, false));
        assert_eq!(glass_tab(2, true, 3, true), (1, false));
        assert_eq!(glass_tab(2, true, 3, false), (0, false));
        assert_eq!(glass_tab(1, true, 3, true), (0, false));
    }

    #[test]
    fn lines_under_titles() {
        let r = |section: &str, sub: &str| Row { section: section.into(), subtitle: sub.into(), ..Default::default() };
        assert_eq!(sub_line(&r("Calculator", "2+2 ="), false, true), "2+2 = · Enter copies it");
        assert_eq!(sub_line(&r("Do", ""), true, true), "Enter to do it");
        assert_eq!(sub_line(&r("Dictionary", "Luck"), true, true), "Luck · Enter opens it");
        assert_eq!(sub_line(&r("Top Hit", "Get · Flathub"), true, true), "Flathub");
        assert_eq!(sub_line(&r("Top Hit", "Get · Flathub"), false, false), "Get · Flathub");
        assert_eq!(sub_line(&r("Software", "Get · AUR · 12 votes"), true, false), "AUR · 12 votes");
        assert_eq!(sub_line(&r("Software", "Get · AUR"), false, false), "Get · AUR");
    }

    #[test]
    fn answer_card_tones() {
        let a = slint::Color::from_rgb_u8(1, 2, 3);
        let r = |section: &str, kind: &str| Row { section: section.into(), kind: kind.into(), ..Default::default() };
        assert_eq!(tone(&r("Answer", "date"), true, a), hexc("#FF453A"));
        assert_eq!(tone(&r("Calculator", ""), false, a), hexc("#FF9500"));
        assert_eq!(tone(&r("Dictionary", ""), true, a), hexc("#BF5AF2"));
        assert_eq!(tone(&r("Do", "math"), true, a), a);
        assert_eq!(tone(&r("Answer", ""), true, a), a);
    }

    #[test]
    fn the_shells_rows() {
        let x = parse_extras(&json!({"q": " gimp ", "store": [{"key": "r3", "section": "Software", "title": "GIMP", "subtitle": "Get · Flathub", "icon": "/x/gimp.png", "loading": false}], "intents": "nonsense"}));
        assert_eq!(x.q, "gimp");
        assert_eq!((x.store[0].title.as_str(), &x.store[0].run, x.store[0].icon.as_str()), ("GIMP", &Run::Shell("r3".into()), "/x/gimp.png"));
        assert!(x.intents.is_empty() && x.notes.is_empty());
    }

    #[test]
    fn glass_is_clear_over_the_plugin() {
        assert_eq!(glass_fill(true, true).alpha(), 0);
        assert_eq!(glass_fill(false, true).alpha(), (0.62f32 * 255.0).round() as u8);
        assert!(glass_fill(false, false).red() > 240);
    }
}
