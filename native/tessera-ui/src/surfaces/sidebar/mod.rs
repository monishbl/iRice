//! The sidebar (rust.surfaces: sidebar): the AI chat, its past chats, the
//! model menu, circle to search's results, Translate and the Anime tab
//! (booru.rs, with sidebar.booru), on the left of the
//! screen it's asked on. Compare shell/modules/sidebar/ (Sidebar.qml and
//! everything it holds) and shell/services/Ai.qml; the chat itself is the AI
//! service (src/services/ai/), the Markdown markdown.rs.
//!
//! How it opens: the QML shell keeps which panel is open (Panels.qml) and,
//! while this process draws the sidebar, tells it on stdin
//! (services/RustUi.qml; src/shell.rs): `open` (on a screen), `close`, `tab`
//! (Panels.sidebarTab, sent before it opens), `attach` (a screenshot or a
//! circled picture for the chat, from ScreenTools) and `continue`
//! (Spotlight's "Continue in chat"). When it closes by itself (Escape, a
//! click outside) it says `closed`, so the shell's Panels agrees; it asks the
//! shell (`shell::ask`) for `sidebar.tab` when its tab changes,
//! `ai.reloadHistory` when it wrote the chat history, `screen.ask` for its
//! screenshot menu, `panels.openSettings` and `launch.openLink`.
//!
//! One full-screen layer surface (Sidebar.qml's window) on the screen it
//! opens on: clicks above the menu bar's 30 px pass through (its input
//! region), anywhere else outside the panel closes it. Glass: the panel on
//! the plugin's glass (radius 26), Theme.panel the tint over it; Material: a
//! solid surface container with a shadow.
//!
//! Not as the QML yet: messages' prose isn't selectable (Slint's StyledText
//! can't select; user bubbles and code can), tables size their columns
//! from their text's length (no text measuring), headings are bold (700,
//! not 600), Material
//! Symbols draw at their default weight and fill (no variable axes), pictures
//! can't be dropped on the chat (no drag and drop yet).

pub mod booru;
pub mod markdown;
pub mod settings;

use crate::services::ai::chats::{self, ListRow};
use crate::services::ai::keys::Keys;
use crate::services::ai::{self, Chat, Event, Picture, Submitted};
use crate::services::hypr::Glass;
use crate::shell::{self, Command as ShellCommand, Part};
use crate::theme;
use crate::ui;
use crate::wayland::{Spec, State, Surface};
use crate::Msg;
use serde_json::{json, Value};
use slint::{ComponentHandle, Model, ModelRc, SharedString, StyledText, VecModel};
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Duration;

pub const NS: &str = "tessera:sidebar";
const NAME: &str = "sidebar";

/// Its own messages (Msg::Part("sidebar", …)): from its callbacks and threads
enum M {
    Ai(Event),
    Close,
    Closed,
    Tab(i32),
    Send(String),
    Stop,
    NewChat,
    ShowHistory(bool),
    PickModel(String),
    ManageModels,
    SaveKey(String),
    AddPictures,
    /// the screenshot menu: "screen", "ask" (an area) or "circle"
    Run(&'static str),
    RemovePicture(i32),
    Copy(String),
    Link(String),
    SearchWeb(String),
    LensDone(String, Value),
    SwitchVision,
    PasteRequested,
    /// pictures made send-ready (ai-image.sh): them, the names it refused,
    /// and whether a paste found no picture (paste the text then)
    Pictures(Vec<Picture>, Vec<String>, bool),
    OpenChat(String),
    Query(String),
    SearchNow(u64),
    Searched(String, Vec<String>),
    Delete(String),
    Rename(String, String),
    Pin(String, bool),
    Move(i32),
    OpenFirst,
    Undo,
    UndoOver,
    NoticeOver,
    Forget,
    TrEdited(String),
    TrSource(String),
    TrTarget(String),
    TrSwap,
    TrCopy,
    TrNow(u64),
    Translated(u64, Result<String, String>),
    BooruPick(i32),
    BooruSearch(String),
    BooruExplicit,
    BooruMore,
    BooruOpen(String),
    BooruPage(u64, Result<Vec<booru::Post>, String>),
    BooruPreview(u64, usize, slint::SharedPixelBuffer<slint::Rgba8Pixel>),
}

/// The Anime tab's search (Booru.qml's state)
#[derive(Default)]
struct Anime {
    /// sidebar.booru, .booruProvider, .booruNsfw
    enabled: bool,
    provider: String,
    nsfw: bool,
    /// on screen since it was last chosen (Booru.qml's Loader: a fresh
    /// search each time the tab is entered)
    active: bool,
    tags: String,
    page: u32,
    loading: bool,
    error: String,
    exhausted: bool,
    gen: u64,
    cells: Rc<VecModel<ui::BooruCell>>,
}

/// "Search the web" for a circled picture: how it's going
#[derive(Clone, Default)]
struct Lens {
    busy: bool,
    ok: bool,
    note: String,
}

#[derive(Default)]
struct Tr {
    source: String,
    /// sidebar.translateTarget, as config.json has it
    target: String,
    input: String,
    result: String,
    error: String,
    gen: u64,
}

struct Win {
    output: String,
    surface: Rc<Surface>,
    ui: ui::Sidebar,
    a11y: Option<crate::a11y::A11y>,
    rows: Rc<VecModel<ui::ChatRow>>,
    glass_sent: String,
}

pub struct Sidebar {
    win: Option<Win>,
    chat: Chat,
    shown: bool,
    tab: i32,
    history_open: bool,
    query: String,
    found: Vec<String>,
    searched_for: String,
    search_gen: u64,
    searching: bool,
    lens: HashMap<String, Lens>,
    images: HashMap<String, Option<slint::Image>>,
    tr: Tr,
    anime: Anime,
    undo_timer: slint::Timer,
    notice_timer: slint::Timer,
    forget_timer: slint::Timer,
    search_timer: slint::Timer,
    tr_timer: slint::Timer,
    picking: bool,
}

impl Default for Sidebar {
    fn default() -> Self {
        Sidebar {
            win: None,
            chat: Chat::new(Some(chats::Store::new(chats::Store::default_dir()))),
            shown: false,
            tab: 0,
            history_open: false,
            query: String::new(),
            found: Vec::new(),
            searched_for: String::new(),
            search_gen: 0,
            searching: false,
            lens: HashMap::new(),
            images: HashMap::new(),
            tr: Tr { source: "auto".into(), ..Default::default() },
            anime: Anime::default(),
            undo_timer: Default::default(),
            notice_timer: Default::default(),
            forget_timer: Default::default(),
            search_timer: Default::default(),
            tr_timer: Default::default(),
            picking: false,
        }
    }
}

/// The checkout the shell runs from (its scripts)
fn shell_root() -> PathBuf {
    let r = crate::env::var("ROOT").map(PathBuf::from).filter(|p| p.join("shell").is_dir());
    r.unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")))
}
fn script(name: &str) -> String {
    shell_root().join("shell/assets/scripts").join(name).to_string_lossy().into_owned()
}

/// Run something off the UI thread, its output ignored
fn spawn_quiet(cmd: Vec<String>) {
    std::thread::spawn(move || {
        let _ = Command::new(&cmd[0]).args(&cmd[1..]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    });
}

/// Settings → AI (the shell's Settings window)
fn open_ai_settings() {
    shell::ask("panels.openSettings", json!(["ai"]));
}

/// A web or mail link in the person's browser (Launch.openLink: nothing else opens)
fn open_link(url: &str) {
    if markdown::safe_url(url).is_some() {
        shell::ask("launch.openLink", json!([url.trim()]));
    }
}

fn copy_text(text: &str) {
    spawn_quiet(vec!["wl-copy".into(), "--".into(), text.into()]);
}

/// base64 (standard, padded): pictures for the request
pub fn base64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { A[n as usize & 63] as char } else { '=' });
    }
    out
}

fn styled(markup: &str) -> StyledText {
    StyledText::from_markdown(markup).unwrap_or_else(|_| StyledText::from_plain_text(markup))
}

/// Markdown blocks as Slint's
fn md_blocks(text: &str) -> ModelRc<ui::MdBlock> {
    let parsed = markdown::blocks(text);
    let gaps = markdown::gaps(&parsed);
    let blocks: Vec<ui::MdBlock> = parsed
        .into_iter()
        .zip(gaps)
        .map(|(b, above)| ui::MdBlock {
            above,
            kind: match b.kind {
                markdown::Kind::Paragraph => 0,
                markdown::Kind::Heading => 1,
                markdown::Kind::List => 2,
                markdown::Kind::Quote => 3,
                markdown::Kind::Code => 4,
                markdown::Kind::Table => 5,
                markdown::Kind::Rule => 6,
            },
            widths: ModelRc::new(VecModel::from(if b.kind == markdown::Kind::Table { markdown::column_widths(&b.rows, 396.0) } else { Vec::new() })),
            level: b.level as i32,
            size: markdown::HEAD[b.level.min(6)],
            text: if b.kind == markdown::Kind::Heading { styled(&format!("**{}**", b.text)) } else { styled(&b.text) },
            code: b.code.into(),
            lang: b.lang.into(),
            open: b.open,
            items: ModelRc::new(VecModel::from(b.items.iter().map(|i| ui::MdItem { depth: i.depth as i32, marker: i.marker.as_str().into(), text: styled(&i.text) }).collect::<Vec<_>>())),
            rows: ModelRc::new(VecModel::from(
                b.rows.iter().map(|(head, cells)| ui::MdRow { head: *head, cells: ModelRc::new(VecModel::from(cells.iter().map(|c| styled(&if *head { format!("**{c}**") } else { c.clone() })).collect::<Vec<_>>())) }).collect::<Vec<_>>(),
            )),
        })
        .collect();
    ModelRc::new(VecModel::from(blocks))
}

impl Sidebar {
    fn sender(st: &State) -> impl Fn(M) + Clone + 'static {
        let to = st.shell.sender(NAME);
        move |m: M| to(Box::new(m))
    }
    fn thread_sender(st: &State) -> impl Fn(M) + Clone + Send + 'static {
        let tx = st.tx.clone();
        move |m: M| {
            let _ = tx.send(Msg::Part(NAME, Box::new(m)));
        }
    }

    fn image(&mut self, path: &str) -> Option<slint::Image> {
        self.images.entry(path.to_string()).or_insert_with(|| slint::Image::load_from_path(std::path::Path::new(path)).ok()).clone()
    }

    fn chat_row(&mut self, r: &chats::Row) -> ui::ChatRow {
        let pictures: Vec<slint::Image> = if r.role == "user" && !r.image.is_empty() { r.image.split('\n').filter_map(|p| self.image(p)).collect() } else { Vec::new() };
        let lens = self.lens.get(&r.search).cloned().unwrap_or_default();
        let assistant = r.role == "assistant" && r.status != "error";
        ui::ChatRow {
            role: r.role.as_str().into(),
            text: r.text.as_str().into(),
            status: r.status.as_str().into(),
            model: r.model.as_str().into(),
            thinking: styled(&markdown::flatten(&r.thinking)),
            has_thinking: !r.thinking.is_empty(),
            pictures: ModelRc::new(VecModel::from(pictures)),
            blocks: if assistant { md_blocks(&r.text) } else { ModelRc::default() },
            search: r.search.as_str().into(),
            lens_busy: lens.busy,
            lens_note: lens.note.into(),
            lens_ok: lens.ok,
        }
    }

    /// Every row again (a new message, a chat opened)
    fn sync_rows(&mut self) {
        let rows: Vec<chats::Row> = self.chat.rows.clone();
        let new: Vec<ui::ChatRow> = rows.iter().map(|r| self.chat_row(r)).collect();
        let Some(w) = &self.win else { return };
        let grew = new.len() != w.rows.row_count();
        w.rows.set_vec(new);
        if grew {
            w.ui.invoke_scroll_end();
        }
    }

    /// The streaming answer (the last row) only
    fn sync_last(&mut self) {
        let Some(r) = self.chat.rows.last().cloned() else { return };
        let row = self.chat_row(&r);
        if let Some(w) = &self.win {
            let n = w.rows.row_count();
            if n == self.chat.rows.len() && n > 0 {
                w.rows.set_row_data(n - 1, row);
            }
        }
    }

    /// Everything but the rows
    fn sync_state(&mut self) {
        let pictures: Vec<slint::Image> = self.chat.attachments.iter().map(|a| a.path.clone()).collect::<Vec<_>>().iter().filter_map(|p| self.image(p)).collect();
        let Some(w) = &self.win else { return };
        let c = &self.chat;
        let ui = &w.ui;
        let m = &c.model;
        ui.set_tab(self.tab);
        ui.set_tabs(ModelRc::new(VecModel::from(settings::tabs(self.anime.enabled).into_iter().map(SharedString::from).collect::<Vec<_>>())));
        ui.set_model_name(m.name.as_str().into());
        ui.set_configured(c.configured());
        ui.set_needs_key(c.needs_key());
        ui.set_provider_name(c.provider_name().into());
        ui.set_credential_label(c.credential_label().to_lowercase().into());
        ui.set_credential_name(c.credential_label().into());
        ui.set_busy(c.busy());
        ui.set_pictures(ModelRc::new(VecModel::from(pictures)));
        ui.set_can_attach(c.attachments.len() < ai::MAX_IMAGES && !self.picking);
        let n = c.attachments.len();
        ui.set_placeholder(
            if !c.configured() {
                "Add a model in Settings → AI".to_string()
            } else if c.needs_key() {
                "Add a credential to start".into()
            } else if n == 1 {
                "Ask about this picture".into()
            } else if n > 1 {
                "Ask about these pictures".into()
            } else {
                format!("Message {}", m.name)
            }
            .into(),
        );
        // Pictures waiting, but the model can't read them
        let blind = n > 0 && (!c.configured() || !m.vision);
        let can_see = c.can_see().cloned();
        ui.set_blind(blind);
        ui.set_blind_text(
            if !c.configured() {
                "Add an AI model that reads pictures to ask about this.".to_string()
            } else {
                format!("{} can't read pictures. {}", m.name, can_see.as_ref().map_or("Add one that can (Claude, Gemini or another vision model) in Settings → AI.".to_string(), |s| format!("{} can.", s.name)))
            }
            .into(),
        );
        ui.set_switch_label(can_see.as_ref().map_or("Add a model".to_string(), |s| format!("Switch to {}", s.name)).into());
        ui.set_switch_add(can_see.is_none());
        let lens_pic = c.attachments.iter().find(|a| a.lens).map(|a| a.path.clone()).unwrap_or_default();
        let l = self.lens.get(&lens_pic).cloned().unwrap_or_default();
        ui.set_blind_lens(lens_pic.as_str().into());
        ui.set_blind_lens_busy(l.busy);
        ui.set_blind_lens_note(l.note.into());
        ui.set_blind_lens_ok(l.ok);
        let keep = c.keep_history();
        ui.set_can_history(keep && (!c.sessions.is_empty() || c.pending_delete.is_some()));
        ui.set_history_open(self.history_open);
        let items: Vec<ui::ModelItem> =
            c.models.iter().map(|x| ui::ModelItem { key: x.key.as_str().into(), label: format!("{} · {}", x.name, ai::models::provider_name(&x.provider)).into(), chosen: x.key == m.key }).collect();
        ui.set_models(ModelRc::new(VecModel::from(items)));
        ui.set_undo_message(c.pending_delete.as_ref().map_or(c.notice.clone(), |p| format!("Deleted “{}”", p.entry.title)).into());
        ui.set_can_undo(c.pending_delete.is_some());
        // Translate
        let target = self.tr.target.clone();
        ui.set_source_name(settings::lang_name(&self.tr.source).into());
        ui.set_target_name(settings::lang_name(&target).into());
        ui.set_source_auto(self.tr.source == "auto");
        ui.set_tr_result(self.tr.result.as_str().into());
        ui.set_tr_error(self.tr.error.as_str().into());
        // Anime
        let a = &self.anime;
        ui.set_booru_provider(booru::provider_index(&a.provider) as i32);
        ui.set_booru_nsfw(a.nsfw);
        ui.set_booru_loading(a.loading);
        ui.set_booru_error(a.error.as_str().into());
        ui.set_booru_none(a.exhausted && a.cells.row_count() == 0);
    }

    /// The past chats as shown: grouped, filtered by the search
    fn sync_history(&mut self) {
        let Some(w) = &self.win else { return };
        let now = chats::now_ms();
        let q = self.query.trim().to_string();
        let found: &[String] = if self.searched_for == q { &self.found } else { &[] };
        let list = chats::group(&chats::filter(&self.chat.sessions, &q, found), now);
        let names: HashMap<&str, &str> = self.chat.models.iter().map(|m| (m.key.as_str(), m.name.as_str())).collect();
        let entries: Vec<ui::ChatEntry> = list
            .iter()
            .enumerate()
            .map(|(i, r)| match r {
                ListRow::Header(h) => ui::ChatEntry { header: h.as_str().into(), first: i == 0, ..Default::default() },
                ListRow::Chat(c) => {
                    let model = names.get(c.model.as_str()).copied().unwrap_or(c.model_name.as_str());
                    let when = chats::relative_time(c.updated, now);
                    ui::ChatEntry {
                        header: "".into(),
                        id: c.id.as_str().into(),
                        title: c.title.as_str().into(),
                        meta: if model.is_empty() { when } else { format!("{when} · {model}") }.into(),
                        open: c.id == self.chat.current_id,
                        pinned: c.pinned,
                        first: i == 0,
                    }
                }
            })
            .collect();
        w.ui.set_chats(ModelRc::new(VecModel::from(entries)));
        w.ui.set_chats_none(self.chat.sessions.is_empty());
        w.ui.set_searching(self.searching);
    }

    /// Scroll the past chats so the selected one shows (History's rows:
    /// headers and chats, each its look's height)
    fn reveal(&self, st: &State, id: &str) {
        let Some(w) = &self.win else { return };
        let q = self.query.trim();
        let found: &[String] = if self.searched_for == q { &self.found } else { &[] };
        let list = chats::group(&chats::filter(&self.chat.sessions, q, found), chats::now_ms());
        if let Some((top, bottom)) = settings::row_span(&list, id, st.shell.look.material) {
            w.ui.invoke_reveal_chat(top, bottom);
        }
    }

    fn visible_ids(&self) -> Vec<String> {
        let q = self.query.trim();
        let found: &[String] = if self.searched_for == q { &self.found } else { &[] };
        chats::group(&chats::filter(&self.chat.sessions, q, found), chats::now_ms())
            .into_iter()
            .filter_map(|r| if let ListRow::Chat(c) = r { Some(c.id) } else { None })
            .collect()
    }

    fn sync(&mut self) {
        self.sync_rows();
        self.sync_state();
        self.sync_history();
    }

    /// After anything that may have written the chat history: the shell is told
    fn tell_chats(&mut self) {
        if std::mem::take(&mut self.chat.wrote) {
            shell::ask("ai.reloadHistory", json!([]));
        }
    }

    // ── The window ──────────────────────────────────────────────────────

    fn make_window(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        let surface = st.prepare_surface(Spec {
            layer: Layer::Top,
            namespace: NS,
            output: Some((output, name)),
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            margin_left: 0,
        });
        let ui = ui::Sidebar::new().expect("Sidebar");
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        let rows = Rc::new(VecModel::default());
        ui.set_rows(ModelRc::from(rows.clone()));
        ui.set_cookie(crate::surfaces::shapes::outline("cookie9").into());
        ui.set_booru_providers(ModelRc::new(VecModel::from(booru::PROVIDERS.iter().map(|p| SharedString::from(p.1)).collect::<Vec<_>>())));
        ui.set_booru_cells(ModelRc::from(self.anime.cells.clone()));
        self.anime.active = false;
        ui.set_langs(ModelRc::new(VecModel::from(settings::LANGUAGES.iter().map(|l| ui::Lang { code: l.0.into(), name: l.1.into() }).collect::<Vec<_>>())));
        let s = Self::sender(st);
        macro_rules! on {
            ($set:ident, |$($a:ident),*| $m:expr) => {{
                let s = s.clone();
                ui.$set(move |$($a),*| s($m));
            }};
            ($set:ident, $m:expr) => {{
                let s = s.clone();
                ui.$set(move || s($m));
            }};
        }
        on!(on_dismiss, M::Close);
        on!(on_closed, M::Closed);
        on!(on_tab_chosen, |i| M::Tab(i));
        on!(on_send, |t| M::Send(t.to_string()));
        on!(on_stop, M::Stop);
        on!(on_new_chat, M::NewChat);
        on!(on_show_history, |b| M::ShowHistory(b));
        on!(on_pick_model, |k| M::PickModel(k.to_string()));
        on!(on_manage_models, M::ManageModels);
        on!(on_picker_clicked, M::ManageModels);
        on!(on_save_key, |k| M::SaveKey(k.to_string()));
        on!(on_add_pictures, M::AddPictures);
        on!(on_shot_screen, M::Run("screen"));
        on!(on_shot_area, M::Run("ask"));
        on!(on_circle, M::Run("circle"));
        on!(on_remove_picture, |i| M::RemovePicture(i));
        on!(on_copy, |t| M::Copy(t.to_string()));
        on!(on_open_link, |u| M::Link(u.to_string()));
        on!(on_search_web, |p| M::SearchWeb(p.to_string()));
        on!(on_switch_vision, M::SwitchVision);
        on!(on_paste_requested, M::PasteRequested);
        on!(on_open_chat, |id| M::OpenChat(id.to_string()));
        on!(on_query_edited, |q| M::Query(q.to_string()));
        on!(on_delete_chat, |id| M::Delete(id.to_string()));
        on!(on_rename_chat, |id, t| M::Rename(id.to_string(), t.to_string()));
        on!(on_pin_chat, |id, p| M::Pin(id.to_string(), p));
        on!(on_move_selection, |d| M::Move(d));
        on!(on_open_first, M::OpenFirst);
        on!(on_undo, M::Undo);
        on!(on_tr_edited, |t| M::TrEdited(t.to_string()));
        on!(on_tr_source, |c| M::TrSource(c.to_string()));
        on!(on_tr_target, |c| M::TrTarget(c.to_string()));
        on!(on_tr_swap, M::TrSwap);
        on!(on_tr_copy, M::TrCopy);
        on!(on_booru_pick, |i| M::BooruPick(i));
        on!(on_booru_search, |t| M::BooruSearch(t.to_string()));
        on!(on_booru_explicit, M::BooruExplicit);
        on!(on_booru_more, M::BooruMore);
        on!(on_booru_open, |p| M::BooruOpen(p.to_string()));
        ui.show().expect("show sidebar");
        let a11y = crate::a11y::A11y::new(&surface, st.tx.clone());
        self.win = Some(Win { output: name.to_string(), surface, ui, a11y, rows, glass_sent: String::new() });
        self.images.clear();
        self.sync();
    }

    fn drop_window(&mut self, st: &mut State) {
        if let Some(w) = self.win.take() {
            st.shell.glass.set(&w.output, NS, &[]);
            st.drop_surface(&w.surface);
        }
    }

    /// The panel's place for the screen it's on
    fn place(&mut self, st: &State) {
        let Some(w) = &self.win else { return };
        let h = st.output_named(&w.output).and_then(|o| st.outputs.info(&o)).and_then(|i| i.logical_size).map_or(1080, |s| s.1) as f32;
        w.ui.set_panel_x(settings::PANEL_X);
        w.ui.set_panel_y(settings::PANEL_Y);
        w.ui.set_panel_h(settings::panel_height(h, &st.shell.cfg));
        let (sw, sh) = st.output_named(&w.output).and_then(|o| st.outputs.info(&o)).and_then(|i| i.logical_size).unwrap_or((1920, 1080));
        // (above the menu bar, clicks go to it)
        st.set_input_region(&w.surface, Some(&[(0, settings::BAR_H, sw, sh - settings::BAR_H)]));
    }

    fn open(&mut self, st: &mut State, screen: Option<&str>, tab: Option<i32>) {
        let outs = st.outputs_now();
        let want = screen.filter(|s| !s.is_empty()).map(String::from).or_else(|| self.win.as_ref().map(|w| w.output.clone()));
        let Some((output, name)) = want.and_then(|n| outs.iter().find(|o| o.1 == n).cloned()).or_else(|| outs.first().cloned()) else { return };
        if self.win.as_ref().is_some_and(|w| w.output != name) {
            self.drop_window(st);
        }
        if self.win.is_none() {
            self.make_window(st, &output, &name);
        }
        if let Some(t) = tab {
            self.tab = t.clamp(0, settings::tabs(self.anime.enabled).len() as i32 - 1);
        }
        self.forget_timer.stop();
        self.chat.reload_sessions();
        self.shown = true;
        self.place(st);
        self.booru_follow(st);
        let w = self.win.as_ref().unwrap();
        w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        w.surface.show(0, 0);
        w.ui.set_shown(true);
        self.sync();
        let w = self.win.as_ref().unwrap();
        w.ui.invoke_opened();
        if self.history_open {
            w.ui.invoke_enter_history();
        }
    }

    /// `say`: closed here (Escape, a click outside), so the shell is told
    fn close(&mut self, st: &mut State, say: bool) {
        if !self.shown {
            return;
        }
        self.shown = false;
        if let Some(w) = &self.win {
            w.ui.set_shown(false); // (the slide out, then Closed hides it)
            w.surface.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            w.surface.wl.commit();
        }
        self.history_open = false;
        if say {
            shell::closed(NAME);
        }
        // History off: the chat lives only while it's in use (a minute after closing)
        if !self.chat.keep_history() {
            let s = Self::sender(st);
            self.forget_timer.start(slint::TimerMode::SingleShot, Duration::from_secs(60), move || s(M::Forget));
        }
    }

    fn closed(&mut self, st: &mut State) {
        if self.shown {
            return;
        }
        if let Some(w) = &mut self.win {
            w.surface.hide();
            st.shell.glass.set(&w.output, NS, &[]);
            w.glass_sent.clear();
        }
    }

    // ── The chat ────────────────────────────────────────────────────────

    fn run_submitted(&mut self, st: &mut State, s: Submitted) {
        match s {
            Submitted::Nothing => {}
            Submitted::Model(key) => settings::write(&["ai", "model"], json!(key)),
            Submitted::Request(req, proto, gen, cancel) => {
                let to = Self::thread_sender(st);
                ai::stream(req, proto, gen, cancel, move |e| to(M::Ai(e)));
            }
        }
        self.tell_chats();
        self.sync();
    }

    /// Pictures from files or the clipboard, made send-ready by the shell's
    /// ai-image.sh (upright, at most 1568 px, a JPEG in the runtime folder)
    fn prepare(&mut self, st: &State, args: Vec<String>, paste: bool) {
        let to = Self::thread_sender(st);
        self.picking = true;
        std::thread::spawn(move || {
            let out = Command::new(script("ai-image.sh")).args(&args).stdin(Stdio::null()).stderr(Stdio::null()).output();
            let (mut pics, mut bad) = (Vec::new(), Vec::new());
            let mut text = false;
            if let Ok(out) = out {
                text = paste && out.status.code() == Some(3);
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    let Some((file, data)) = line.split_once('\t') else { continue };
                    if file == "!" {
                        bad.push(data.to_string());
                    } else {
                        pics.push(Picture { path: file.into(), data: data.into(), media_type: "image/jpeg".into(), lens: false });
                    }
                }
            }
            to(M::Pictures(pics, bad, text));
        });
    }

    fn add_pictures(&mut self, st: &State) {
        let room = ai::MAX_IMAGES - self.chat.attachments.len().min(ai::MAX_IMAGES);
        if room == 0 {
            self.chat.note_full();
            return self.sync();
        }
        let to = Self::thread_sender(st);
        self.picking = true;
        let home = std::env::var("HOME").unwrap_or_default();
        std::thread::spawn(move || {
            let out = Command::new("python3")
                .args([script("pick-file.py").as_str(), "--multiple", "--title", "Add pictures", "--start", &format!("{home}/Pictures"), "--filter", "Pictures|*.png *.jpg *.jpeg *.webp *.gif *.heic *.avif *.bmp *.tif *.tiff"])
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output();
            let paths: Vec<String> = out.ok().filter(|o| o.status.success()).and_then(|o| serde_json::from_slice(&o.stdout).ok()).unwrap_or_default();
            if paths.is_empty() {
                return to(M::Pictures(Vec::new(), Vec::new(), false));
            }
            // (the first ones that fit: a message carries four)
            let n = paths.len();
            let take: Vec<String> = paths.into_iter().take(room).collect();
            let mut bad = Vec::new();
            if n > take.len() {
                bad.push(format!("\u{0}{room}"));
            }
            let out = Command::new(script("ai-image.sh")).args(&take).stdin(Stdio::null()).stderr(Stdio::null()).output();
            let mut pics = Vec::new();
            if let Ok(out) = out {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    let Some((file, data)) = line.split_once('\t') else { continue };
                    if file == "!" {
                        bad.push(data.to_string());
                    } else {
                        pics.push(Picture { path: file.into(), data: data.into(), media_type: "image/jpeg".into(), lens: false });
                    }
                }
            }
            to(M::Pictures(pics, bad, false));
        });
    }

    /// Circle to search's "Search the web": the picture goes to Lens only on
    /// this click (the shell's lens-upload.py); the results open in the browser
    fn search_web(&mut self, st: &State, path: String) {
        if path.is_empty() || self.lens.values().any(|l| l.busy) {
            return;
        }
        self.lens.insert(path.clone(), Lens { busy: true, ..Default::default() });
        let to = Self::thread_sender(st);
        std::thread::spawn(move || {
            let out = Command::new("python3").args(["-I", &script("lens-upload.py"), &path]).stdin(Stdio::null()).stderr(Stdio::null()).output();
            let r = match out {
                Ok(o) => serde_json::from_slice::<Value>(&o.stdout).unwrap_or(json!({"ok": false})),
                Err(_) => json!({"ok": false, "error": "Couldn't start the search. Make sure python3 is installed, then try again."}),
            };
            to(M::LensDone(path, r));
        });
    }

    fn lens_done(&mut self, path: String, r: Value) {
        let url = r["url"].as_str().unwrap_or("");
        let l = if r["ok"] == true && markdown::safe_url(url).is_some() {
            open_link(url);
            Lens { busy: false, ok: true, note: "Opened in your browser".into() }
        } else if r["fallback"] == true {
            // (Lens changed how it takes pictures: paste it there instead)
            spawn_quiet(vec!["sh".into(), "-c".into(), "wl-copy --type image/jpeg < \"$1\"".into(), "sh".into(), path.clone()]);
            open_link("https://lens.google.com/");
            Lens { busy: false, ok: true, note: "Paste the picture into Lens with Ctrl+V: it's on the clipboard.".into() }
        } else {
            Lens { busy: false, ok: false, note: r["error"].as_str().filter(|e| !e.is_empty()).unwrap_or("Couldn't search the picture. Try again.").into() }
        };
        self.lens.insert(path, l);
    }

    fn show_history(&mut self, on: bool) {
        self.history_open = on;
        if on {
            self.query.clear();
            self.found.clear();
            self.searched_for.clear();
            self.chat.reload_sessions();
            if let Some(w) = &self.win {
                w.ui.set_selected(self.chat.current_id.as_str().into());
                w.ui.invoke_enter_history();
            }
        } else if let Some(w) = &self.win {
            w.ui.invoke_focus_composer();
        }
        self.sync_state();
        self.sync_history();
    }

    fn open_chat(&mut self, id: &str) {
        match self.chat.open_chat(id) {
            Ok(key) => {
                if let Some(k) = key {
                    settings::write(&["ai", "model"], json!(k));
                }
                self.show_history(false);
            }
            Err(()) => {}
        }
        self.sync();
    }

    // ── Translate (translate-shell, a moment after typing stops) ────────

    fn translate_soon(&mut self, st: &State) {
        self.tr.gen += 1;
        let gen = self.tr.gen;
        let s = Self::sender(st);
        self.tr_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(600), move || s(M::TrNow(gen)));
    }

    fn translate(&mut self, st: &State) {
        self.tr.gen += 1;
        let gen = self.tr.gen;
        self.tr.result.clear();
        self.tr.error.clear();
        if self.tr.input.trim().is_empty() {
            return;
        }
        let target = settings::translate_target(&st.shell.cfg);
        let (source, text) = (self.tr.source.clone(), self.tr.input.clone());
        let to = Self::thread_sender(st);
        std::thread::spawn(move || {
            let out = Command::new("trans").args(["-brief", "-no-ansi", "-s", &source, "-t", &target, "--", &text]).stdin(Stdio::null()).stderr(Stdio::null()).output();
            let r = match out {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err("Translating needs translate-shell. Install it, then try again.".to_string()),
                Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).trim().to_string()),
                _ => Err("Couldn't translate. Check your connection and translate-shell installation.".to_string()),
            };
            to(M::Translated(gen, r));
        });
    }
}

impl Sidebar {
    fn booru_settings(&mut self, cfg: &Value) {
        self.anime.enabled = booru::enabled(cfg);
        self.anime.provider = booru::provider(cfg);
        self.anime.nsfw = booru::nsfw(cfg);
        let n = settings::tabs(self.anime.enabled).len() as i32;
        if self.tab >= n {
            self.tab = n - 1;
        }
    }

    /// The Anime tab chosen: a fresh search (Booru.qml loads anew each time
    /// its tab is entered); left: it stops
    fn booru_follow(&mut self, st: &State) {
        let on = self.win.is_some() && self.tab == 2 && self.anime.enabled;
        if on && !self.anime.active {
            self.anime.active = true;
            self.anime.tags.clear();
            if let Some(w) = &self.win {
                w.ui.invoke_booru_clear();
            }
            self.booru_search(st, true);
        } else if !on && self.anime.active {
            self.anime.active = false;
            self.anime.gen += 1;
            self.anime.loading = false;
        }
    }

    /// A page of posts (`reset`: the first, the grid emptied)
    fn booru_search(&mut self, st: &State, reset: bool) {
        let a = &mut self.anime;
        if !reset && (a.loading || a.exhausted) {
            return;
        }
        if reset {
            a.gen += 1;
            a.page = 1;
            a.cells.set_vec(Vec::new());
            a.exhausted = false;
        }
        a.loading = true;
        a.error.clear();
        let (gen, provider, tags, page, nsfw) = (a.gen, a.provider.clone(), a.tags.clone(), a.page, a.nsfw);
        let to = Self::thread_sender(st);
        std::thread::spawn(move || to(M::BooruPage(gen, booru::fetch(&provider, &tags, page, nsfw))));
    }

    fn booru_page(&mut self, st: &State, gen: u64, r: Result<Vec<booru::Post>, String>) {
        let a = &mut self.anime;
        if gen != a.gen {
            return;
        }
        a.loading = false;
        match r {
            Err(e) => a.error = e,
            Ok(posts) => {
                a.exhausted = posts.is_empty();
                a.page += 1;
                let first = a.cells.row_count();
                for p in &posts {
                    a.cells.push(ui::BooruCell { picture: Default::default(), ready: false, page: p.page.as_str().into() });
                }
                // (the pictures, one after another, each as it comes)
                let to = Self::thread_sender(st);
                std::thread::spawn(move || {
                    for (i, p) in posts.iter().enumerate() {
                        if let Some(px) = booru::preview(&p.preview, 400) {
                            to(M::BooruPreview(gen, first + i, px));
                        }
                    }
                });
            }
        }
    }
}

impl Part for Sidebar {
    fn name(&self) -> &'static str {
        NAME
    }

    fn start(&mut self, st: &mut State) {
        st.shell.want("ai");
        self.chat.configure(&st.shell.cfg);
        self.tr.target = settings::translate_target(&st.shell.cfg);
        self.booru_settings(&st.shell.cfg.clone());
        self.chat.start_history();
        self.tell_chats();
    }

    fn command(&mut self, st: &mut State, c: &ShellCommand) {
        let cmd = &c.args;
        match c.cmd.as_str() {
            "open" => self.open(st, c.screen.as_deref(), cmd["tab"].as_i64().map(|t| t as i32)),
            "close" => self.close(st, false),
            "tab" => {
                self.tab = cmd["tab"].as_i64().unwrap_or(0).clamp(0, settings::tabs(self.anime.enabled).len() as i64 - 1) as i32;
                self.booru_follow(st);
                self.sync_state();
            }
            // A screenshot (lens: circled) for the chat, from the shell's screen tools
            "attach" => {
                let path = cmd["path"].as_str().unwrap_or("").to_string();
                let Ok(bytes) = std::fs::read(&path) else { return };
                let p = Picture { path, data: base64(&bytes), media_type: "image/jpeg".into(), lens: cmd["lens"] == true };
                self.tab = 0;
                self.history_open = false;
                let s = if p.lens {
                    self.chat.search_picture(p)
                } else {
                    self.chat.attach(p);
                    Submitted::Nothing
                };
                self.run_submitted(st, s);
            }
            "continue" => {
                self.chat.continue_quick(cmd["question"].as_str().unwrap_or(""), cmd["answer"].as_str().unwrap_or(""));
                self.tab = 0;
                self.history_open = false;
                self.tell_chats();
                self.sync();
            }
            other => eprintln!("tessera-ui: sidebar: no command {other:?}"),
        }
    }

    fn output_removed(&mut self, st: &mut State, name: &str) {
        if self.win.as_ref().is_some_and(|w| w.output == name) {
            self.close(st, true);
            self.drop_window(st);
        }
    }

    fn config_changed(&mut self, st: &mut State, look: bool) {
        self.chat.configure(&st.shell.cfg);
        self.tr.target = settings::translate_target(&st.shell.cfg);
        let (nsfw, provider) = (self.anime.nsfw, self.anime.provider.clone());
        self.booru_settings(&st.shell.cfg.clone());
        // (changed in Settings while it shows: search again, as the QML's binding does)
        if self.anime.active && (nsfw != self.anime.nsfw || provider != self.anime.provider) {
            self.booru_search(st, true);
        }
        self.booru_follow(st);
        if look {
            if let Some(w) = &mut self.win {
                st.shell.apply_theme(&w.ui.global::<ui::Theme>());
                w.glass_sent.clear();
            }
        }
        self.place(st);
        self.sync();
    }

    fn plugin_changed(&mut self, st: &mut State) {
        if let Some(w) = &mut self.win {
            st.shell.apply_theme(&w.ui.global::<ui::Theme>());
            w.glass_sent.clear();
        }
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<Keys>() {
            if let Some(k) = st.shell.get::<Keys>() {
                self.chat.keys.merge(k.clone());
            }
            self.sync_state();
        }
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        let Ok(m) = msg.downcast::<M>() else { return };
        match *m {
            M::Ai(Event::Update { gen, text, thinking }) => {
                if self.chat.update(gen, text, thinking) {
                    self.sync_last();
                }
                return;
            }
            M::Ai(Event::Done { gen, status, parser, body }) => {
                if self.chat.done(gen, status, &parser, &body) {
                    self.sync_rows();
                }
            }
            M::Close => self.close(st, true),
            M::Closed => self.closed(st),
            M::Tab(i) => {
                self.tab = i;
                shell::ask("sidebar.tab", json!([i]));
                if let Some(w) = &self.win {
                    w.ui.invoke_opened();
                }
            }
            M::Send(t) => {
                let s = self.chat.submit(&t);
                return self.run_submitted(st, s);
            }
            M::Stop => self.chat.stop(),
            M::NewChat => {
                self.chat.new_chat();
                self.history_open = false;
                if let Some(w) = &self.win {
                    w.ui.invoke_focus_composer();
                }
            }
            M::ShowHistory(on) => self.show_history(on),
            M::PickModel(k) => {
                if let Some(key) = self.chat.set_model(&k) {
                    settings::write(&["ai", "model"], json!(key));
                }
            }
            M::ManageModels => open_ai_settings(),
            M::SaveKey(k) => {
                self.chat.save_credential(&k);
            }
            M::AddPictures => self.add_pictures(st),
            M::Run(what) => shell::ask("screen.ask", json!([what])),
            M::RemovePicture(i) => self.chat.detach(i.max(0) as usize),
            M::Copy(t) => copy_text(&t),
            M::Link(u) => open_link(&u),
            M::SearchWeb(p) => self.search_web(st, p),
            M::LensDone(p, r) => self.lens_done(p, r),
            M::SwitchVision => match self.chat.can_see().map(|m| m.key.clone()) {
                Some(k) => {
                    if let Some(key) = self.chat.set_model(&k) {
                        settings::write(&["ai", "model"], json!(key));
                    }
                    if let Some(w) = &self.win {
                        w.ui.invoke_focus_composer();
                    }
                }
                None => open_ai_settings(),
            },
            M::PasteRequested => {
                if self.chat.attachments.len() >= ai::MAX_IMAGES {
                    self.chat.note_full();
                } else {
                    self.prepare(st, vec!["--clipboard".into()], true);
                }
            }
            M::Pictures(pics, bad, text) => {
                self.picking = false;
                for p in pics {
                    self.chat.attach(p);
                }
                let (room, names): (Vec<String>, Vec<String>) = bad.into_iter().partition(|b| b.starts_with('\u{0}'));
                if let Some(r) = room.first() {
                    self.chat.note(&format!("Only the first {} of those were added: a message carries {} pictures.", &r[1..], ai::MAX_IMAGES));
                }
                if !names.is_empty() {
                    self.chat.note(&format!("Couldn't add {}: only pictures up to 64 MB (JPEG, PNG, WebP, GIF, HEIC…) can be sent.", names.join(", ")));
                }
                if text {
                    if let Some(w) = &self.win {
                        w.ui.invoke_paste_text();
                    }
                }
            }
            M::OpenChat(id) => return self.open_chat(&id),
            M::Query(q) => {
                self.query = q;
                self.search_gen += 1;
                let gen = self.search_gen;
                let s = Self::sender(st);
                self.search_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(250), move || s(M::SearchNow(gen)));
                self.sync_history();
                return;
            }
            M::SearchNow(gen) => {
                if gen != self.search_gen {
                    return;
                }
                let q = self.query.trim().to_string();
                if q.is_empty() {
                    self.found.clear();
                    self.searched_for.clear();
                } else {
                    self.searching = true;
                    let dir = chats::Store::default_dir();
                    let to = Self::thread_sender(st);
                    std::thread::spawn(move || {
                        let ids = chats::search_files(&dir, &q);
                        to(M::Searched(q, ids));
                    });
                }
            }
            M::Searched(q, ids) => {
                self.searching = false;
                if q == self.query.trim() {
                    self.searched_for = q;
                    self.found = ids;
                }
            }
            M::Delete(id) => {
                // (the selection moves on to the next chat)
                let ids = self.visible_ids();
                let i = ids.iter().position(|x| *x == id);
                let next = i.and_then(|i| ids.get(i + 1).or(i.checked_sub(1).and_then(|j| ids.get(j)))).cloned().unwrap_or_default();
                self.chat.delete_chat(&id);
                if let Some(w) = &self.win {
                    w.ui.set_selected(next.as_str().into());
                    w.ui.invoke_focus_history_list();
                }
                let s = Self::sender(st);
                self.undo_timer.start(slint::TimerMode::SingleShot, Duration::from_secs(8), move || s(M::UndoOver));
            }
            M::Rename(id, t) => self.chat.rename_chat(&id, &t),
            M::Pin(id, p) => self.chat.set_pinned(&id, p),
            // (0: Down from the search field, landing on the selected chat
            // first; Up from the first: back to the field)
            M::Move(d) => {
                let ids = self.visible_ids();
                if let Some(w) = &self.win {
                    let sel = w.ui.get_selected().to_string();
                    let i = ids.iter().position(|x| *x == sel);
                    if d < 0 && i.unwrap_or(0) == 0 {
                        w.ui.invoke_focus_history_search();
                        return;
                    }
                    let next = match i {
                        None => ids.first(),
                        Some(i) => ids.get((i as i64 + d as i64).clamp(0, ids.len() as i64 - 1) as usize),
                    };
                    if let Some(n) = next.cloned() {
                        w.ui.set_selected(n.as_str().into());
                        w.ui.invoke_focus_history_list();
                        self.reveal(st, &n);
                    }
                }
                return;
            }
            M::OpenFirst => {
                if let Some(id) = self.visible_ids().first().cloned() {
                    return self.open_chat(&id);
                }
            }
            M::Undo => {
                self.undo_timer.stop();
                if let Some(k) = self.chat.undo_delete() {
                    settings::write(&["ai", "model"], json!(k));
                }
            }
            M::UndoOver => self.chat.finish_delete(),
            M::NoticeOver => self.chat.notice.clear(),
            M::Forget => {
                if !self.shown && !self.chat.keep_history() && !self.chat.rows.is_empty() && !self.chat.busy() {
                    self.chat.new_chat();
                }
            }
            M::TrEdited(t) => {
                self.tr.input = t;
                return self.translate_soon(st);
            }
            M::TrNow(gen) => {
                if gen == self.tr.gen {
                    self.translate(st);
                }
            }
            M::TrSource(c) => {
                self.tr.source = c;
                self.translate(st);
            }
            M::TrTarget(c) => {
                settings::write(&["sidebar", "translateTarget"], json!(c));
                settings::set(&mut st.shell.cfg, &["sidebar", "translateTarget"], json!(c));
                self.translate(st);
            }
            M::TrSwap => {
                if self.tr.source != "auto" {
                    let old = std::mem::replace(&mut self.tr.source, settings::translate_target(&st.shell.cfg));
                    settings::write(&["sidebar", "translateTarget"], json!(old));
                    settings::set(&mut st.shell.cfg, &["sidebar", "translateTarget"], json!(old));
                    self.tr.input = self.tr.result.clone();
                    if let Some(w) = &self.win {
                        w.ui.invoke_set_translate_input(self.tr.input.as_str().into());
                    }
                    self.translate(st);
                }
            }
            M::TrCopy => copy_text(&self.tr.result),
            M::BooruPick(i) => {
                let id = booru::PROVIDERS[(i.max(0) as usize).min(booru::PROVIDERS.len() - 1)].0;
                settings::write(&["sidebar", "booruProvider"], json!(id));
                settings::set(&mut st.shell.cfg, &["sidebar", "booruProvider"], json!(id));
                self.anime.provider = id.into();
                self.booru_search(st, true);
            }
            M::BooruSearch(t) => {
                self.anime.tags = t;
                self.booru_search(st, true);
            }
            M::BooruExplicit => {
                let on = !self.anime.nsfw;
                settings::write(&["sidebar", "booruNsfw"], json!(on));
                settings::set(&mut st.shell.cfg, &["sidebar", "booruNsfw"], json!(on));
                self.anime.nsfw = on;
                self.booru_search(st, true);
            }
            M::BooruMore => {
                if self.anime.loading || self.anime.exhausted {
                    return;
                }
                self.booru_search(st, false);
            }
            M::BooruOpen(p) => open_link(&p),
            M::BooruPage(gen, r) => self.booru_page(st, gen, r),
            M::BooruPreview(gen, i, px) => {
                if gen == self.anime.gen {
                    if let Some(mut c) = self.anime.cells.row_data(i) {
                        c.picture = slint::Image::from_rgba8(px);
                        c.ready = true;
                        self.anime.cells.set_row_data(i, c);
                    }
                }
                return;
            }
            M::Translated(gen, r) => {
                if gen == self.tr.gen {
                    match r {
                        Ok(t) => self.tr.result = t,
                        Err(e) => self.tr.error = e,
                    }
                }
            }
        }
        self.tr.target = settings::translate_target(&st.shell.cfg);
        // A note to show for a while
        if !self.chat.notice.is_empty() && !self.notice_timer.running() {
            let s = Self::sender(st);
            self.notice_timer.start(slint::TimerMode::SingleShot, Duration::from_secs(5), move || s(M::NoticeOver));
        }
        self.tell_chats();
        self.booru_follow(st);
        self.sync_state();
        self.sync_history();
        // (new rows, statuses, the lens's notes)
        self.sync_rows();
    }

    fn keyboard_left(&mut self, _: &mut State, _: &Rc<Surface>) {}

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if self.win.as_ref().is_some_and(|w| Rc::ptr_eq(&w.surface, s)) {
            self.close(st, true);
            self.drop_window(st);
        }
    }

    /// The panel's glass for the plugin as it slides (Glass only), then the
    /// accessibility tree
    fn after_frame(&mut self, st: &mut State) {
        let kbd = st.keyboard_surface();
        let Some(w) = &mut self.win else { return };
        let f = kbd.as_ref().is_some_and(|k| Rc::ptr_eq(k, &w.surface));
        if let Some(a) = w.a11y.as_mut() {
            a.update(false, f);
        }
        if !st.shell.plugin || st.shell.look.material || !w.surface.mapped.get() {
            return;
        }
        let ui = &w.ui;
        let p = ui.get_progress();
        let shapes: Vec<[f32; 19]> = if p > 0.002 { vec![theme::glass_shape(&st.shell.look, ui.get_slide_x(), ui.get_panel_y(), settings::PANEL_W, ui.get_panel_h(), 26.0, p, 1.0)] } else { Vec::new() };
        let key = format!("{shapes:?}");
        if key != w.glass_sent {
            st.shell.glass.set(&w.output, NS, &shapes);
            w.glass_sent = key;
        }
    }

    fn a11y(&mut self) -> Vec<&mut crate::a11y::A11y> {
        self.win.as_mut().and_then(|w| w.a11y.as_mut()).into_iter().collect()
    }

    fn stats(&self) {
        if let Some(w) = &self.win {
            eprintln!("tessera-ui: stats {}", w.surface.stats.borrow_mut().report("sidebar"));
        }
    }

    fn quit(&mut self, _: &mut State) {
        self.chat.stop();
        self.chat.finish_delete();
        if let Some(w) = &self.win {
            Glass::clear_now(&w.output, NS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_as_coreutils() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(&[0xff, 0xfe, 0x00, 0x10]), "//4AEA==");
    }

    #[test]
    fn replies_become_blocks_slint_reads() {
        let b = md_blocks("# Plan\n\n- **one**\n- [two](https://x.y)\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```sh\nls\n```");
        let kinds: Vec<i32> = b.iter().map(|x| x.kind).collect();
        assert_eq!(kinds, [1, 2, 5, 4]);
        assert_eq!(b.row_data(0).unwrap().size, 20.0);
        assert_eq!(b.row_data(1).unwrap().items.row_count(), 2);
        assert_eq!(b.row_data(3).unwrap().code, "ls");
    }
}
