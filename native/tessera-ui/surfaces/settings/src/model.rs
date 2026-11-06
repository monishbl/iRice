//! A Settings page as Rust builds it: blocks (Group.qml, a PagePart's
//! heading, MoreOptions, a paragraph, a page group's own view) of rows
//! (Row.qml, SubpageLink.qml) with their controls, and what each control
//! does (a closure, run when Slint says `act(id, …)`). A page is built again
//! after every change, from the settings and the system's data, as the QML
//! pages' bindings re-evaluate.
//!
//! A page (pages/*.rs):
//!
//! ```ignore
//! pub fn dock(cx: &mut Ctx, p: &mut Page) {
//!     let on = cx.b("dock.enabled");
//!     let g = p.group("");
//!     g.row(Row::new("Show the dock").switch(on, cx.toggle("dock.enabled")));
//!     let g = p.group("").dim(!on);
//!     g.row(Row::new("Size").detail(format!("{} px", cx.i("dock.iconSize")))
//!         .slider(cx.f("dock.iconSize"), 32.0, 72.0, 2.0, cx.num("dock.iconSize", 1.0)).width(220.0));
//!     if p.more(cx) { /* the folded-away groups */ }
//!     p.part(cx, "Quick Settings", super::look::controlcenter);
//! }
//! ```

use super::config::{self, Cfg};
use crate::shell::Shell;
use crate::ui;
use serde_json::Value;
use slint::{Color, Image, ModelRc, SharedString, VecModel};
use std::collections::HashMap;
use std::rc::Rc;

// ── What a control says ─────────────────────────────────────────────────

/// What Slint's `act(id, i, f, s)` carried: a segment's or button's index,
/// a slider's value, a field's text (`i`: 0 while typing, 1 committed)
#[derive(Clone, Debug, Default)]
pub struct Arg {
    pub i: i32,
    pub f: f32,
    pub s: String,
    /// the row's id (a path row's file chooser answers to it)
    pub id: String,
}

pub type Action = Rc<dyn Fn(&mut Ctx, &Arg)>;

/// A thing a page asked for that only the window can do
#[derive(Default)]
pub struct Out {
    pub actions: Vec<Action>,
    /// services to start (st.shell.want)
    pub want: Vec<&'static str>,
    /// a toast: its text, and its Undo
    pub toast: Option<(String, Option<Action>)>,
    /// go to a page (and show a row on it)
    pub go: Option<(String, String)>,
    /// build the page again (after an action, it is anyway)
    pub rebuild: bool,
    /// close the window
    pub close: bool,
    /// menus for buttons that open one: id -> items
    pub menus: HashMap<String, Vec<MenuItem>>,
    /// a file chooser for a path row: (row id, folder, title, start, filter)
    pub pick: Option<(String, bool, String, String, String)>,
    /// pictures to decode: (key, path, width, height)
    pub pictures: Vec<(String, String, u32, u32)>,
}

#[derive(Clone, Debug, Default)]
pub struct MenuItem {
    pub text: String,
    pub checked: bool,
    pub danger: bool,
    pub enabled: bool,
    pub icon: String,
}

impl MenuItem {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), enabled: true, ..Default::default() }
    }
    pub fn checked(mut self, on: bool) -> Self {
        self.checked = on;
        self
    }
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
    pub fn icon(mut self, name: &str) -> Self {
        self.icon = name.into();
        self
    }
}

/// Page-local state (a search field's text, More options open, a list read
/// from a command): kept while Settings is open, by key
pub type Local = HashMap<String, Value>;

/// What a page builder and an action see
pub struct Ctx<'a> {
    /// the look, the services' data (`shell.get::<T>()`), the plugin
    pub shell: &'a Shell,
    pub cfg: &'a mut Cfg,
    /// what the QML shell says (its RustBridge: accounts, updates, Focus…),
    /// by page group: `cx.feed["look"]["daylight"]`
    pub feed: &'a Value,
    pub local: &'a mut Local,
    /// the page on show ("about/health")
    pub page: &'a str,
    pub material: bool,
    pub dark: bool,
    pub out: &'a mut Out,
    pub tx: smithay_client_toolkit::reexports::calloop::channel::Sender<crate::Msg>,
    /// the screens there are now, by name (Quickshell.screens)
    pub screens: Vec<String>,
    /// pictures decoded so far (`picture`)
    pub pictures: &'a HashMap<String, Option<Image>>,
}

impl<'a> Ctx<'a> {
    // Settings (config.json with Config.qml's defaults)
    pub fn get(&self, path: &str) -> Value {
        self.cfg.get(path)
    }
    pub fn b(&self, path: &str) -> bool {
        self.cfg.b(path)
    }
    pub fn s(&self, path: &str) -> String {
        self.cfg.s(path)
    }
    pub fn f(&self, path: &str) -> f32 {
        self.cfg.f(path) as f32
    }
    pub fn i(&self, path: &str) -> i64 {
        self.cfg.i(path)
    }
    pub fn list(&self, path: &str) -> Vec<String> {
        self.cfg.list(path)
    }
    /// Change a setting (written to config.json a moment later)
    pub fn set(&mut self, path: &str, value: impl Into<Value>) {
        self.cfg.set(path, value.into());
    }
    /// A number, an integer where Config.qml's property is an int
    pub fn set_num(&mut self, path: &str, n: f64) {
        self.cfg.set(path, config::number_for(path, n));
    }

    /// What a control does: an id for its row
    pub fn on(&mut self, f: impl Fn(&mut Ctx, &Arg) + 'static) -> String {
        self.out.actions.push(Rc::new(f));
        format!("a{}", self.out.actions.len() - 1)
    }
    /// A switch for a boolean setting
    pub fn toggle(&mut self, path: &str) -> String {
        let path = path.to_string();
        self.on(move |cx, _| {
            let v = cx.b(&path);
            cx.set(&path, !v);
        })
    }
    /// A segmented control or choice for a setting: the n-th value
    pub fn pick(&mut self, path: &str, values: &[&str]) -> String {
        let path = path.to_string();
        let values: Vec<String> = values.iter().map(|s| s.to_string()).collect();
        self.on(move |cx, a| {
            if let Some(v) = values.get(a.i.max(0) as usize) {
                cx.set(&path, v.clone());
            }
        })
    }
    /// The same with any JSON values
    pub fn pick_values(&mut self, path: &str, values: Vec<Value>) -> String {
        let path = path.to_string();
        self.on(move |cx, a| {
            if let Some(v) = values.get(a.i.max(0) as usize) {
                cx.set(&path, v.clone());
            }
        })
    }
    /// A slider for a number setting, rounded to `round` (0: as it is)
    pub fn num(&mut self, path: &str, round: f64) -> String {
        let path = path.to_string();
        self.on(move |cx, a| {
            let v = if round > 0.0 { (a.f as f64 / round).round() * round } else { a.f as f64 };
            cx.set_num(&path, v);
        })
    }
    /// A text field for a string setting (committed: Enter or leaving it)
    pub fn text(&mut self, path: &str) -> String {
        let path = path.to_string();
        self.on(move |cx, a| {
            if a.i == 1 && cx.s(&path) != a.s {
                cx.set(&path, a.s.clone());
            }
        })
    }
    /// A path row for a string setting (PathField.qml): the typed path
    /// (~ for home), or Choose… opens a file chooser (`folder`, `filter`
    /// e.g. "Images|*.jpg *.png")
    pub fn path_setting(&mut self, path: &str, folder: bool, filter: &str) -> String {
        let (path, filter) = (path.to_string(), filter.to_string());
        self.on(move |cx, a| {
            if a.i == 2 {
                let cur = cx.s(&path);
                let start = if cur.is_empty() { "~".to_string() } else if folder { cur } else { cur.rsplit_once('/').map_or(cur.clone(), |x| x.0.to_string()) };
                let title = if folder { "Choose a folder" } else { "Choose a file" };
                cx.choose_file(&a.id, folder, title, &start, &filter);
            } else if a.i == 1 {
                let home = std::env::var("HOME").unwrap_or_default();
                let t = a.s.trim();
                let p = if let Some(rest) = t.strip_prefix('~') { format!("{home}{rest}") } else { t.to_string() };
                if p != cx.s(&path) {
                    cx.set(&path, p);
                }
            }
        })
    }
    /// A link's action: go to a page, and show a row on it
    pub fn link(&mut self, page: &str, target: &str) -> String {
        let (page, target) = (page.to_string(), target.to_string());
        self.on(move |cx, _| cx.go(&page, &target))
    }
    pub fn go(&mut self, page: &str, target: &str) {
        self.out.go = Some((page.to_string(), target.to_string()));
    }
    pub fn toast(&mut self, text: impl Into<String>, undo: Option<Action>) {
        self.out.toast = Some((text.into(), undo));
    }
    pub fn want(&mut self, service: &'static str) {
        if !self.out.want.contains(&service) {
            self.out.want.push(service);
        }
    }
    pub fn close(&mut self) {
        self.out.close = true;
    }
    /// A button that opens a menu: its items (picking one acts with its index)
    pub fn menu(&mut self, id: &str, items: Vec<MenuItem>) {
        self.out.menus.insert(id.to_string(), items);
    }

    // Page-local state
    pub fn local_b(&self, key: &str) -> bool {
        self.local.get(key).and_then(Value::as_bool).unwrap_or(false)
    }
    pub fn local_s(&self, key: &str) -> String {
        self.local.get(key).and_then(Value::as_str).unwrap_or("").to_string()
    }
    pub fn local_set(&mut self, key: &str, v: impl Into<Value>) {
        self.local.insert(key.to_string(), v.into());
    }

    /// Something only the QML shell does (its RustBridge.qml's
    /// `settings.act`, by page group): `cx.ask("look", "repairShell", json!([]))`
    pub fn ask(&self, group: &str, what: &str, args: Value) {
        let mut a = vec![Value::from("act"), Value::from(group), Value::from(what)];
        if let Value::Array(x) = args {
            a.extend(x);
        } else if !args.is_null() {
            a.push(args);
        }
        crate::shell::ask("settings", Value::Array(a));
    }

    /// Work off the UI thread (a command's output, a file): its result lands
    /// in `local[key]` and the page is built again. Once per key while it
    /// runs (`local[key + ".busy"]`)
    pub fn spawn(&mut self, key: &str, f: impl FnOnce() -> Value + Send + 'static) {
        let busy = format!("{key}.busy");
        if self.local_b(&busy) {
            return;
        }
        self.local_set(&busy, true);
        let tx = self.tx.clone();
        let key = key.to_string();
        std::thread::spawn(move || {
            let v = f();
            let _ = tx.send(crate::Msg::Part(super::NAME, Box::new(super::M::Local(key, v))));
        });
    }
    /// Read once while Settings is open: `local[key]`, spawning `f` the first time
    pub fn once(&mut self, key: &str, f: impl FnOnce() -> Value + Send + 'static) -> Value {
        if let Some(v) = self.local.get(key) {
            return v.clone();
        }
        self.spawn(key, f);
        Value::Null
    }

    /// A picture from a file, decoded off the UI thread to cover a `w`×`h`
    /// box (pixels): None until it's ready (the page is built again then),
    /// and None for good when it can't be read
    pub fn picture(&mut self, path: &str, w: u32, h: u32) -> Option<Image> {
        let key = format!("{path}@{w}x{h}");
        match self.pictures.get(&key) {
            Some(i) => i.clone(),
            None => {
                self.out.pictures.push((key, path.to_string(), w, h));
                None
            }
        }
    }
    /// Choose a file for a path row (PathField's Choose…): the row's action
    /// gets the path (`i` 1) when one is chosen
    pub fn choose_file(&mut self, id: &str, folder: bool, title: &str, start: &str, filter: &str) {
        self.out.pick = Some((id.to_string(), folder, title.to_string(), start.to_string(), filter.to_string()));
    }

    /// A program's output (stdout, trimmed); "" when it failed
    pub fn run(cmd: &[&str]) -> String {
        std::process::Command::new(cmd[0])
            .args(&cmd[1..])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    }
    /// Run something and forget it (off the UI thread)
    pub fn detach(cmd: Vec<String>) {
        std::thread::spawn(move || {
            let _ = std::process::Command::new(&cmd[0]).args(&cmd[1..]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        });
    }
}

// ── Icons ───────────────────────────────────────────────────────────────

/// An icon by Phosphor name: its glyph and Material's symbol
pub fn ic(name: &str) -> ui::SIc {
    if name.is_empty() {
        return ui::SIc::default();
    }
    let p = crate::icons::glyph(name);
    ui::SIc { p: p.into(), m: crate::icons::symbol(p).into() }
}

/// "#RRGGBB" (or "#AARRGGBB") as a colour
pub fn color(hex: &str) -> Color {
    let h = hex.trim_start_matches('#');
    let n = u32::from_str_radix(h, 16).unwrap_or(0x808080);
    match h.len() {
        8 => Color::from_argb_u8((n >> 24) as u8, (n >> 16) as u8, (n >> 8) as u8, n as u8),
        _ => Color::from_rgb_u8((n >> 16) as u8, (n >> 8) as u8, n as u8),
    }
}

// ── Items ───────────────────────────────────────────────────────────────

/// A small item: a segment, a chip, a button, a swatch, a custom view's datum
#[derive(Clone, Debug, Default)]
pub struct Opt {
    pub text: String,
    pub detail: String,
    pub icon: String,
    pub on: bool,
    pub off: bool,
    pub style: i32,
    pub color: Option<Color>,
    pub image: Option<Image>,
    pub num: f32,
    pub id: String,
}

impl Opt {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Default::default() }
    }
    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = d.into();
        self
    }
    pub fn icon(mut self, name: &str) -> Self {
        self.icon = name.into();
        self
    }
    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }
    pub fn off(mut self, off: bool) -> Self {
        self.off = off;
        self
    }
    pub fn style(mut self, s: i32) -> Self {
        self.style = s;
        self
    }
    pub fn primary(self) -> Self {
        self.style(1)
    }
    pub fn danger(self) -> Self {
        self.style(2)
    }
    pub fn minor(self) -> Self {
        self.style(3)
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    pub fn image(mut self, i: Image) -> Self {
        self.image = Some(i);
        self
    }
    pub fn num(mut self, n: f32) -> Self {
        self.num = n;
        self
    }
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }
    fn to_ui(&self) -> ui::SOpt {
        ui::SOpt {
            text: self.text.as_str().into(),
            detail: self.detail.as_str().into(),
            icon: ic(&self.icon),
            on: self.on,
            off: self.off,
            style: self.style,
            color: self.color.unwrap_or_default(),
            image: self.image.clone().unwrap_or_default(),
            num: self.num,
            id: self.id.as_str().into(),
        }
    }
}

/// A button among a row's buttons
pub fn btn(text: impl Into<String>) -> Opt {
    Opt::new(text)
}

fn opts(v: &[Opt]) -> ModelRc<ui::SOpt> {
    ModelRc::new(VecModel::from(v.iter().map(Opt::to_ui).collect::<Vec<_>>()))
}

// ── Rows ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Kind {
    #[default]
    Row,
    Link,
    Text,
    Custom,
}

/// One row: a label (and detail) with a control on the right, a link to a
/// page, a line of text, or a page group's own view
#[derive(Clone, Default)]
pub struct Row {
    pub id: String,
    pub kind: Kind,
    pub label: String,
    pub detail: String,
    pub icon: String,
    pub tone: i32,
    pub value: String,
    pub control: &'static str,
    pub on: bool,
    pub enabled: bool,
    pub num: f32,
    pub from: f32,
    pub to: f32,
    pub step: f32,
    pub low: String,
    pub high: String,
    pub index: i32,
    pub opts: Vec<Opt>,
    pub text: String,
    pub placeholder: String,
    pub secret: bool,
    pub width: f32,
    pub style: i32,
    pub busy: bool,
    pub visible: bool,
    pub flash: bool,
    pub group: String,
    pub sub: String,
    pub data: Vec<Opt>,
    pub image: Option<Image>,
    /// a link: where it goes
    pub page: String,
    pub target: String,
}

impl Row {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), enabled: true, visible: true, index: -1, ..Default::default() }
    }
    /// A row that opens a page (SubpageLink.qml): `target` a row there to show
    pub fn link(label: impl Into<String>, page: &str) -> Self {
        Self { kind: Kind::Link, page: page.into(), ..Self::new(label) }
    }
    /// A line of text in a group (a status, an explanation)
    pub fn text(text: impl Into<String>) -> Self {
        Self { kind: Kind::Text, tone: 1, ..Self::new(text) }
    }
    /// A page group's own view as a row (settings-<group>.slint's <Group>Ctl
    /// with `r.sub`): `id` its action
    pub fn custom(group: &str, sub: &str) -> Self {
        Self { kind: Kind::Custom, group: group.into(), sub: sub.into(), ..Self::new("") }
    }

    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = d.into();
        self
    }
    pub fn icon(mut self, name: &str) -> Self {
        self.icon = name.into();
        self
    }
    /// The label's colour: 0 label, 1 secondary, 2 danger, 3 success
    pub fn tone(mut self, t: i32) -> Self {
        self.tone = t;
        self
    }
    pub fn value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }
    pub fn target(mut self, t: &str) -> Self {
        self.target = t.into();
        self
    }
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
    /// Shown only when (QML's `visible:`): a hidden row is left out
    pub fn visible(mut self, on: bool) -> Self {
        self.visible = on;
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }
    pub fn id(mut self, id: String) -> Self {
        self.id = id;
        self
    }
    pub fn data(mut self, d: Vec<Opt>) -> Self {
        self.data = d;
        self
    }
    pub fn num(mut self, n: f32) -> Self {
        self.num = n;
        self
    }
    pub fn image(mut self, i: Image) -> Self {
        self.image = Some(i);
        self
    }
    pub fn text_value(mut self, t: impl Into<String>) -> Self {
        self.text = t.into();
        self
    }

    // Controls
    pub fn switch(mut self, on: bool, id: String) -> Self {
        self.control = "switch";
        self.on = on;
        self.id = id;
        self
    }
    pub fn slider(mut self, value: f32, from: f32, to: f32, step: f32, id: String) -> Self {
        self.control = "slider";
        (self.num, self.from, self.to, self.step) = (value, from, to, step);
        if self.width == 0.0 {
            self.width = 220.0;
        }
        self.id = id;
        self
    }
    /// A slider with a word under each end (WordSlider.qml)
    pub fn words(mut self, low: &str, high: &str) -> Self {
        self.low = low.into();
        self.high = high.into();
        self
    }
    pub fn segmented(mut self, options: &[&str], index: i32, id: String) -> Self {
        self.control = "segmented";
        self.opts = options.iter().map(|o| Opt::new(*o)).collect();
        self.index = index;
        self.id = id;
        self
    }
    /// Segments that can't be chosen
    pub fn off(mut self, which: &[usize]) -> Self {
        for i in which {
            if let Some(o) = self.opts.get_mut(*i) {
                o.off = true;
            }
        }
        self
    }
    /// A pop-up list (Choice.qml / Dropdown.qml): it opens the window's menu
    pub fn choice(mut self, options: &[String], index: i32, id: String) -> Self {
        self.control = "choice";
        self.opts = options.iter().map(|o| Opt::new(o.clone())).collect();
        self.index = index;
        self.id = id;
        self
    }
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }
    /// One button (SettingsButton.qml); `.primary()`, `.danger()`, `.minor()`
    pub fn button(mut self, text: impl Into<String>, id: String) -> Self {
        self.control = "button";
        self.text = text.into();
        self.id = id;
        self
    }
    pub fn primary(mut self) -> Self {
        self.style = 1;
        self
    }
    pub fn danger(mut self) -> Self {
        self.style = 2;
        self
    }
    pub fn minor(mut self) -> Self {
        self.style = 3;
        self
    }
    pub fn busy(mut self, b: bool) -> Self {
        self.busy = b;
        self
    }
    /// Several buttons: the action gets the index (an Opt's `on`: busy; `off`: disabled)
    pub fn buttons(mut self, b: Vec<Opt>, id: String) -> Self {
        self.control = "buttons";
        self.opts = b;
        self.id = id;
        self
    }
    /// A text field: the action gets the text (`i` 0 typing, 1 committed)
    pub fn field(mut self, text: impl Into<String>, placeholder: impl Into<String>, id: String) -> Self {
        self.control = "field";
        self.text = text.into();
        self.placeholder = placeholder.into();
        if self.width == 0.0 {
            self.width = 220.0;
        }
        self.id = id;
        self
    }
    pub fn secret(mut self) -> Self {
        self.secret = true;
        self
    }
    /// A path with Choose… (PathField.qml): the action gets the path
    /// (`i` 1); Choose… opens a file chooser (`Page::path_pick`)
    pub fn path(mut self, text: impl Into<String>, placeholder: impl Into<String>, id: String) -> Self {
        self.control = "path";
        self.text = text.into();
        self.placeholder = placeholder.into();
        self.id = id;
        self
    }
    pub fn chips(mut self, chips: Vec<Opt>, id: String) -> Self {
        self.control = "chips";
        self.opts = chips;
        self.id = id;
        self
    }
    pub fn swatches(mut self, s: Vec<Opt>, id: String) -> Self {
        self.control = "swatches";
        self.opts = s;
        self.id = id;
        self
    }
    /// A value on the right (read only)
    pub fn shows(mut self, v: impl Into<String>) -> Self {
        self.control = "value";
        self.value = v.into();
        self
    }
    pub fn spinner(mut self) -> Self {
        self.control = "spinner";
        self
    }
    /// A page group's own control on the right (settings-<group>.slint's
    /// <Group>Ctl, `r.sub`)
    pub fn ctl(mut self, group: &str, sub: &str, id: String) -> Self {
        self.control = "custom";
        self.group = group.into();
        self.sub = sub.into();
        self.id = id;
        self
    }

    fn to_ui(&self) -> ui::SetRow {
        ui::SetRow {
            // (a link with no action of its own goes to its page)
            id: if self.kind == Kind::Link && self.id.is_empty() { format!("go:{}\t{}", self.page, self.target).into() } else { self.id.as_str().into() },
            kind: match self.kind {
                Kind::Row => "row",
                Kind::Link => "link",
                Kind::Text => "text",
                Kind::Custom => "custom",
            }
            .into(),
            label: self.label.as_str().into(),
            detail: self.detail.as_str().into(),
            icon: ic(&self.icon),
            tone: self.tone,
            value: self.value.as_str().into(),
            control: self.control.into(),
            on: self.on,
            enabled: self.enabled,
            num: self.num,
            from: self.from,
            to: self.to,
            step: self.step,
            low: self.low.as_str().into(),
            high: self.high.as_str().into(),
            index: self.index,
            opts: opts(&self.opts),
            text: self.text.as_str().into(),
            placeholder: self.placeholder.as_str().into(),
            secret: self.secret,
            width: self.width,
            style: self.style,
            busy: self.busy,
            flash: self.flash,
            group: self.group.as_str().into(),
            sub: self.sub.as_str().into(),
            data: opts(&self.data),
            image: self.image.clone().unwrap_or_default(),
        }
    }
}

// ── Blocks ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq)]
pub enum BKind {
    #[default]
    Group,
    Heading,
    More,
    Text,
    Custom,
}

#[derive(Clone, Default)]
pub struct Block {
    pub kind: BKind,
    pub id: String,
    pub title: String,
    pub footer: String,
    pub text: String,
    pub dim: bool,
    pub on: bool,
    pub flash: bool,
    pub rows: Vec<Row>,
    pub group: String,
    pub sub: String,
    pub data: Vec<Opt>,
    pub num: f32,
    pub image: Option<Image>,
    /// inside a closed More options (search opens it)
    pub folded: bool,
    /// the space above it, when not the page's own (22)
    pub above: Option<f32>,
}

impl Block {
    pub fn footer(&mut self, f: impl Into<String>) -> &mut Self {
        self.footer = f.into();
        self
    }
    /// Its rows can't be used now (QML: `enabled: false; opacity: 0.45`)
    pub fn dim(&mut self, d: bool) -> &mut Self {
        self.dim = d;
        self
    }
    /// A row (left out when it isn't `visible`)
    pub fn row(&mut self, r: Row) -> &mut Self {
        if r.visible {
            self.rows.push(r);
        }
        self
    }
    pub fn data(&mut self, d: Vec<Opt>) -> &mut Self {
        self.data = d;
        self
    }
    pub fn num(&mut self, n: f32) -> &mut Self {
        self.num = n;
        self
    }
    pub fn id(&mut self, id: String) -> &mut Self {
        self.id = id;
        self
    }
    pub fn text(&mut self, t: impl Into<String>) -> &mut Self {
        self.text = t.into();
        self
    }
    pub fn image(&mut self, i: Image) -> &mut Self {
        self.image = Some(i);
        self
    }
    pub fn title(&mut self, t: impl Into<String>) -> &mut Self {
        self.title = t.into();
        self
    }
    pub fn above(&mut self, a: f32) -> &mut Self {
        self.above = Some(a);
        self
    }
}

/// A page being built
#[derive(Default)]
pub struct Page {
    /// a subpage's title (QML's pageTitle), shown after its section's name
    pub title: String,
    pub blocks: Vec<Block>,
    /// inside a More options that's closed (its blocks are kept, hidden,
    /// so a search can find and open it)
    folding: bool,
    /// the key of the More options being built
    more_key: String,
    /// a part being built (its rows search finds under its heading)
    part: String,
}

impl Page {
    fn push(&mut self, mut b: Block) -> &mut Block {
        b.folded = self.folding;
        self.blocks.push(b);
        self.blocks.last_mut().unwrap()
    }
    /// A group (Group.qml): its caption ("" none); a group with no rows on
    /// show folds to nothing
    pub fn group(&mut self, title: impl Into<String>) -> &mut Block {
        self.push(Block { kind: BKind::Group, title: title.into(), ..Default::default() })
    }
    /// A paragraph between the groups
    pub fn text(&mut self, t: impl Into<String>) -> &mut Block {
        self.push(Block { kind: BKind::Text, text: t.into(), ..Default::default() })
    }
    /// A page group's own view (settings-<group>.slint's <Group>Block, `b.sub`)
    pub fn custom(&mut self, group: &str, sub: &str) -> &mut Block {
        self.push(Block { kind: BKind::Custom, group: group.into(), sub: sub.into(), ..Default::default() })
    }
    /// MoreOptions.qml: the button, and whether what follows shows. Closed
    /// each time the page opens. Call `end_more` after its blocks.
    ///   `p.more(cx); …groups…; p.end_more();`
    pub fn more(&mut self, cx: &mut Ctx) -> bool {
        let n = self.blocks.iter().filter(|b| b.kind == BKind::More).count();
        let key = format!("more:{}:{}:{n}", cx.page, self.part);
        let open = cx.local_b(&key);
        let k = key.clone();
        let id = cx.on(move |cx, _| {
            let v = cx.local_b(&k);
            cx.local_set(&k, !v);
        });
        self.push(Block { kind: BKind::More, id, on: open, text: key.clone(), ..Default::default() });
        self.folding = !open;
        self.more_key = key;
        open
    }
    pub fn end_more(&mut self) {
        self.folding = false;
    }
    /// Another page's settings as a part of this one (PagePart.qml): its
    /// heading, then what `f` builds
    pub fn part(&mut self, cx: &mut Ctx, title: &str, f: impl FnOnce(&mut Ctx, &mut Page)) {
        if !title.is_empty() {
            self.push(Block { kind: BKind::Heading, title: title.into(), ..Default::default() });
        }
        let was = std::mem::replace(&mut self.part, title.to_string());
        let first = self.blocks.len();
        f(cx, self);
        self.end_more();
        self.part = was;
        // (the part's first block sits closer under its heading)
        if !title.is_empty() {
            if let Some(b) = self.blocks.get_mut(first) {
                if b.above.is_none() {
                    b.above = Some(if cx.material { 12.0 } else { 10.0 });
                }
            }
        }
    }
    /// The key of the More options the target row is in, if it's folded
    pub fn folded_key_of(&self, target: &str) -> Option<String> {
        let mut key: Option<String> = None;
        for b in &self.blocks {
            if b.kind == BKind::More {
                key = if b.on { None } else { Some(b.text.clone()) };
                continue;
            }
            if !b.folded {
                key = None;
            }
            let hit = b.title == target || b.rows.iter().any(|r| r.label == target);
            if hit {
                return if b.folded { key } else { None };
            }
        }
        None
    }
    /// Mark the row or block called `target` (a search landed there);
    /// false when there's none on show
    pub fn mark(&mut self, target: &str) -> bool {
        for b in self.blocks.iter_mut().filter(|b| !b.folded) {
            if let Some(r) = b.rows.iter_mut().find(|r| r.label == target) {
                r.flash = true;
                return true;
            }
            if b.title == target {
                if b.kind == BKind::Group {
                    if let Some(r) = b.rows.first_mut() {
                        r.flash = true;
                        return true;
                    }
                }
                b.flash = true;
                return true;
            }
        }
        false
    }

    /// The blocks on show, with the space above each
    pub fn shown(&self, material: bool) -> Vec<&Block> {
        let mut out: Vec<&Block> = Vec::new();
        for b in &self.blocks {
            if b.folded || (b.kind == BKind::Group && b.rows.is_empty()) {
                continue;
            }
            out.push(b);
        }
        let _ = material;
        out
    }
}

/// The page as Slint's blocks: (block, its rows)
pub fn to_ui(page: &Page, material: bool) -> Vec<(ui::SBlock, Vec<ui::SetRow>)> {
    let shown = page.shown(material);
    let mut out = Vec::new();
    for (i, b) in shown.iter().enumerate() {
        let above = if i == 0 { 0.0 } else { b.above.unwrap_or(22.0) };
        let rows: Vec<ui::SetRow> = b.rows.iter().map(Row::to_ui).collect();
        out.push((
            ui::SBlock {
                kind: match b.kind {
                    BKind::Group => "group",
                    BKind::Heading => "heading",
                    BKind::More => "more",
                    BKind::Text => "text",
                    BKind::Custom => "custom",
                }
                .into(),
                id: b.id.as_str().into(),
                title: b.title.as_str().into(),
                footer: b.footer.as_str().into(),
                text: b.text.as_str().into(),
                dim: b.dim,
                on: b.on,
                above,
                flash: b.flash,
                rows: ModelRc::default(),
                group: b.group.as_str().into(),
                sub: b.sub.as_str().into(),
                data: opts(&b.data),
                num: b.num,
                image: b.image.clone().unwrap_or_default(),
            },
            rows,
        ));
    }
    out
}

/// The menus of a page's choices (id -> their items), with the ones pages registered
pub fn menus(page: &Page, extra: &HashMap<String, Vec<MenuItem>>) -> HashMap<String, Vec<MenuItem>> {
    let mut m = extra.clone();
    for b in &page.blocks {
        for r in &b.rows {
            if r.control == "choice" {
                m.insert(r.id.clone(), r.opts.iter().enumerate().map(|(i, o)| MenuItem::new(o.text.clone()).checked(i as i32 == r.index).enabled(!o.off)).collect());
            }
        }
    }
    m
}

/// The window's models, kept between builds: rows changed in place, so a
/// control being dragged or typed in keeps its state
#[derive(Default)]
pub struct Models {
    pub blocks: Rc<VecModel<ui::SBlock>>,
    rows: Vec<Rc<VecModel<ui::SetRow>>>,
    shape: Vec<(SharedString, SharedString, usize)>,
}

impl Models {
    /// The new page; `fresh`: another page (everything made anew)
    pub fn apply(&mut self, new: Vec<(ui::SBlock, Vec<ui::SetRow>)>, fresh: bool) {
        use slint::Model;
        let shape: Vec<(SharedString, SharedString, usize)> = new.iter().map(|(b, r)| (b.kind.clone(), b.sub.clone(), r.len())).collect();
        if fresh || shape.len() != self.shape.len() || shape.iter().zip(&self.shape).any(|(a, b)| a.0 != b.0 || a.1 != b.1) {
            self.rows = new.iter().map(|(_, r)| Rc::new(VecModel::from(r.clone()))).collect();
            let blocks: Vec<ui::SBlock> = new.into_iter().zip(&self.rows).map(|((mut b, _), m)| {
                b.rows = ModelRc::from(m.clone());
                b
            }).collect();
            self.blocks.set_vec(blocks);
            self.shape = shape;
            return;
        }
        for (i, (mut b, rows)) in new.into_iter().enumerate() {
            let m = &self.rows[i];
            if m.row_count() != rows.len() {
                m.set_vec(rows);
            } else {
                for (j, r) in rows.into_iter().enumerate() {
                    if m.row_data(j).as_ref() != Some(&r) {
                        m.set_row_data(j, r);
                    }
                }
            }
            b.rows = ModelRc::from(m.clone());
            if self.blocks.row_data(i).as_ref() != Some(&b) {
                self.blocks.set_row_data(i, b);
            }
        }
        self.shape = shape;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_rows_and_empty_groups_fold_away() {
        let mut p = Page::default();
        p.group("A").row(Row::new("x").visible(false));
        p.group("B").row(Row::new("y"));
        let ui = to_ui(&p, false);
        assert_eq!(ui.len(), 1);
        assert_eq!(ui[0].0.title, "B");
        assert_eq!(ui[0].0.above, 0.0);
    }

    #[test]
    fn spacing_follows_the_page_and_its_parts() {
        let mut p = Page::default();
        p.group("A").row(Row::new("x"));
        p.blocks.push(Block { kind: BKind::Heading, title: "Dock".into(), ..Default::default() });
        p.group("B").row(Row::new("y")).above(10.0);
        let ui = to_ui(&p, false);
        assert_eq!(ui.iter().map(|b| b.0.above).collect::<Vec<_>>(), [0.0, 22.0, 10.0]);
    }

    #[test]
    fn a_search_finds_rows_in_closed_more_options() {
        let mut p = Page::default();
        p.group("A").row(Row::new("x"));
        p.blocks.push(Block { kind: BKind::More, text: "more:k".into(), ..Default::default() });
        p.folding = true;
        p.group("Hidden").row(Row::new("Deep"));
        p.end_more();
        assert_eq!(p.folded_key_of("Deep"), Some("more:k".into()));
        assert_eq!(p.folded_key_of("x"), None);
        assert!(!p.mark("Deep"));
        assert!(p.mark("x"));
        assert_eq!(to_ui(&p, false).len(), 2);
    }
}
