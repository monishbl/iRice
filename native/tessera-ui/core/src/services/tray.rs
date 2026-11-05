//! The system tray: apps' StatusNotifierItems on the session bus
//! (services::tray::Tray), as Quickshell's SystemTray gives the QML bar.
//!
//! The watcher (org.kde.StatusNotifierWatcher) is whoever owns that name:
//! the QML shell's Quickshell when it runs, otherwise this process serves
//! one itself. Either way this is a host: it registers with the watcher,
//! reads RegisteredStatusNotifierItems, follows its Registered/Unregistered
//! signals and every item's New* signals, and sends a fresh list after
//! each burst. Icons: IconName looked up in the icon theme (and the item's
//! IconThemePath), else IconPixmap (ARGB32, network order) as RGBA.
//! Menus are com.canonical.dbusmenu: `menu(key)` reads one (on a thread,
//! the answer comes back as a TrayMenu message), `menu_event` clicks an entry.

use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::Duration;
use zbus::blocking::{proxy::Builder, Connection, MessageIterator, Proxy};
use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedValue, Value};
use zbus::MatchRule;

const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const ITEM: &str = "org.kde.StatusNotifierItem";
const MENU: &str = "com.canonical.dbusmenu";

#[derive(Clone, Debug, PartialEq)]
pub enum Icon {
    None,
    File(PathBuf),
    /// width, height, RGBA (premultiplied as given: SNI pixmaps aren't)
    Pixels(u32, u32, Vec<u8>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrayItem {
    /// "service/path": what the watcher lists it as, and how it's acted on
    pub key: String,
    pub id: String,
    pub title: String,
    pub tooltip: String,
    /// Active | Passive | NeedsAttention
    pub status: String,
    pub icon: Icon,
    /// its dbusmenu's object path ("" none)
    pub menu: String,
    /// only a menu: a click opens it rather than Activate
    pub item_is_menu: bool,
}

impl TrayItem {
    /// What its tooltip says (MenuBar.qml: tooltipTitle || title || id)
    pub fn label(&self) -> &str {
        [self.tooltip.as_str(), self.title.as_str(), self.id.as_str()].into_iter().find(|s| !s.is_empty()).unwrap_or("")
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tray {
    pub items: Vec<TrayItem>,
}

/// An entry of a tray menu (dbusmenu's layout, children nested)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuNode {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub visible: bool,
    pub separator: bool,
    /// None: not checkable; Some(on)
    pub checked: Option<bool>,
    pub children: Vec<MenuNode>,
}

/// The answer to `menu(key)`: the root's entries (sent to the part that asked)
pub struct TrayMenu {
    pub key: String,
    pub root: MenuNode,
}

/// "service/path" or a bare service ("org.kde.StatusNotifierItem-12-1",
/// path /StatusNotifierItem); `sender` stands in when it's only a path
pub fn parse_address(addr: &str, sender: &str) -> (String, String) {
    if addr.starts_with('/') {
        return (sender.to_string(), addr.to_string());
    }
    match addr.find('/') {
        Some(i) => (addr[..i].to_string(), addr[i..].to_string()),
        None => (addr.to_string(), "/StatusNotifierItem".to_string()),
    }
}

/// The pixmap closest above `want` px (else the largest), ARGB32 in network
/// order turned into RGBA
pub fn best_pixmap(list: &[(i32, i32, Vec<u8>)], want: i32) -> Option<(u32, u32, Vec<u8>)> {
    let ok: Vec<&(i32, i32, Vec<u8>)> = list.iter().filter(|(w, h, d)| *w > 0 && *h > 0 && d.len() >= (*w * *h * 4) as usize).collect();
    let pick = ok.iter().filter(|(w, _, _)| *w >= want).min_by_key(|(w, _, _)| *w).or_else(|| ok.iter().max_by_key(|(w, _, _)| *w))?;
    let (w, h, d) = (pick.0 as u32, pick.1 as u32, &pick.2);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for px in d.chunks_exact(4).take((w * h) as usize) {
        rgba.extend_from_slice(&[px[1], px[2], px[3], px[0]]);
    }
    Some((w, h, rgba))
}

/// A label without its mnemonic marks ("_File" → "File", "__" → "_")
pub fn strip_mnemonic(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '_' {
            if it.peek() == Some(&'_') {
                out.push('_');
                it.next();
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn unwrap<'a, 'b>(v: &'a Value<'b>) -> &'a Value<'b> {
    match v {
        Value::Value(b) => unwrap(b),
        v => v,
    }
}

/// One node of dbusmenu's layout, (ia{sv}av)
pub fn parse_node(v: &Value) -> Option<MenuNode> {
    let Value::Structure(s) = unwrap(v) else { return None };
    let f = s.fields();
    let id = match f.first().map(unwrap) {
        Some(Value::I32(i)) => *i,
        _ => return None,
    };
    let mut n = MenuNode { id, enabled: true, visible: true, ..Default::default() };
    if let Some(Value::Dict(d)) = f.get(1).map(unwrap) {
        let props: HashMap<String, &Value> = d
            .iter()
            .filter_map(|(k, v)| match unwrap(k) {
                Value::Str(k) => Some((k.to_string(), unwrap(v))),
                _ => None,
            })
            .collect();
        let s = |k: &str| match props.get(k).copied() {
            Some(Value::Str(s)) => s.to_string(),
            _ => String::new(),
        };
        let b = |k: &str, d: bool| match props.get(k).copied() {
            Some(&Value::Bool(b)) => b,
            _ => d,
        };
        n.label = strip_mnemonic(&s("label"));
        n.enabled = b("enabled", true);
        n.visible = b("visible", true);
        n.separator = s("type") == "separator";
        if !s("toggle-type").is_empty() {
            n.checked = Some(matches!(props.get("toggle-state").copied(), Some(Value::I32(1))));
        }
    }
    if let Some(Value::Array(a)) = f.get(2).map(unwrap) {
        n.children = a.iter().filter_map(parse_node).collect();
    }
    Some(n)
}

// ── The watcher (when nobody else serves one) ───────────────────────────────

#[derive(Default, Clone)]
struct WatcherState {
    items: Arc<Mutex<Vec<String>>>,
}

struct Watcher(WatcherState);

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    async fn register_status_notifier_item(
        &self,
        service: &str,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) {
        let sender = hdr.sender().map(|s| s.to_string()).unwrap_or_default();
        let (svc, path) = parse_address(service, &sender);
        let key = format!("{svc}{path}");
        let added = {
            let mut items = self.0.items.lock().unwrap();
            if items.contains(&key) {
                false
            } else {
                items.push(key.clone());
                true
            }
        };
        if added {
            let _ = Self::status_notifier_item_registered(&emitter, &key).await;
        }
    }
    async fn register_status_notifier_host(&self, _service: &str, #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>) {
        let _ = Self::status_notifier_host_registered(&emitter).await;
    }
    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.0.items.lock().unwrap().clone()
    }
    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
    #[zbus(signal)]
    async fn status_notifier_item_registered(emitter: &zbus::object_server::SignalEmitter<'_>, service: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn status_notifier_item_unregistered(emitter: &zbus::object_server::SignalEmitter<'_>, service: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn status_notifier_host_registered(emitter: &zbus::object_server::SignalEmitter<'_>) -> zbus::Result<()>;
}

/// Serve the watcher if the name is free; its items drop as their owners leave
fn serve_watcher(conn: &Connection) -> zbus::Result<bool> {
    let state = WatcherState::default();
    conn.object_server().at(WATCHER_PATH, Watcher(state.clone()))?;
    let flags = zbus::fdo::RequestNameFlags::DoNotQueue.into();
    match conn.request_name_with_flags(WATCHER, flags) {
        Ok(zbus::fdo::RequestNameReply::PrimaryOwner) | Ok(zbus::fdo::RequestNameReply::AlreadyOwner) => {}
        _ => {
            let _ = conn.object_server().remove::<Watcher, _>(WATCHER_PATH);
            return Ok(false);
        }
    }
    let rule = MatchRule::builder().msg_type(zbus::message::Type::Signal).sender("org.freedesktop.DBus")?.interface("org.freedesktop.DBus")?.member("NameOwnerChanged")?.build();
    let mut it = MessageIterator::for_match_rule(rule, conn, Some(64))?;
    let conn2 = conn.clone();
    std::thread::spawn(move || {
        while let Some(Ok(m)) = it.next() {
            let Ok((name, _old, new)) = m.body().deserialize::<(String, String, String)>() else { continue };
            if !new.is_empty() {
                continue;
            }
            let gone: Vec<String> = {
                let mut items = state.items.lock().unwrap();
                let gone = items.iter().filter(|k| k.starts_with(&format!("{name}/"))).cloned().collect();
                items.retain(|k| !k.starts_with(&format!("{name}/")));
                gone
            };
            for k in gone {
                let _ = conn2.emit_signal(None::<&str>, WATCHER_PATH, WATCHER, "StatusNotifierItemUnregistered", &(k.as_str(),));
            }
        }
    });
    Ok(true)
}

// ── The host ────────────────────────────────────────────────────────────────

fn conn() -> Option<&'static Connection> {
    static C: OnceLock<Option<Connection>> = OnceLock::new();
    C.get_or_init(|| Connection::session().ok()).as_ref()
}

fn proxy<'a>(c: &Connection, dest: &'a str, path: &'a str, iface: &'a str) -> zbus::Result<Proxy<'a>> {
    Builder::new(c).destination(dest)?.path(path)?.interface(iface)?.cache_properties(CacheProperties::No).build()
}

fn read_item(c: &Connection, key: &str) -> Option<TrayItem> {
    let (svc, path) = parse_address(key, "");
    let p = proxy(c, &svc, &path, ITEM).ok()?;
    let s = |k: &str| p.get_property::<String>(k).unwrap_or_default();
    let id = s("Id");
    let status = s("Status");
    let attention = status == "NeedsAttention";
    let name = if attention && !s("AttentionIconName").is_empty() { s("AttentionIconName") } else { s("IconName") };
    let theme = s("IconThemePath");
    let extra = (!theme.is_empty()).then(|| PathBuf::from(&theme));
    let mut icon = crate::icon_theme::find(&name, extra.as_deref()).map(Icon::File).unwrap_or(Icon::None);
    if icon == Icon::None {
        let pix: Vec<(i32, i32, Vec<u8>)> = p.get_property(if attention { "AttentionIconPixmap" } else { "IconPixmap" }).or_else(|_| p.get_property("IconPixmap")).unwrap_or_default();
        if let Some((w, h, rgba)) = best_pixmap(&pix, 32) {
            icon = Icon::Pixels(w, h, rgba);
        }
    }
    if icon == Icon::None {
        icon = crate::icon_theme::find("application-x-executable", None).map(Icon::File).unwrap_or(Icon::None);
    }
    // ToolTip: (icon name, icon pixmaps, title, text)
    let tooltip = p
        .get_property::<OwnedValue>("ToolTip")
        .ok()
        .and_then(|v| match unwrap(&v) {
            Value::Structure(st) => st.fields().get(2).map(|t| match unwrap(t) {
                Value::Str(s) => s.to_string(),
                _ => String::new(),
            }),
            _ => None,
        })
        .unwrap_or_default();
    let menu = p.get_property::<zbus::zvariant::OwnedObjectPath>("Menu").map(|o| o.to_string()).unwrap_or_default();
    Some(TrayItem {
        key: key.to_string(),
        id,
        title: s("Title"),
        tooltip,
        status,
        icon,
        menu: if menu == "/" { String::new() } else { menu },
        item_is_menu: p.get_property("ItemIsMenu").unwrap_or(false),
    })
}

fn watch_rule(c: &Connection, rule: MatchRule<'static>, dirty: mpsc::Sender<()>) -> zbus::Result<()> {
    let mut it = MessageIterator::for_match_rule(rule, c, Some(64))?;
    std::thread::spawn(move || {
        while it.next().is_some() {
            if dirty.send(()).is_err() {
                return;
            }
        }
    });
    Ok(())
}

pub fn spawn(tx: Sender<Msg>) {
    std::thread::spawn(move || loop {
        let run = || -> zbus::Result<()> {
            let c = conn().ok_or(zbus::Error::Failure("no session bus".into()))?.clone();
            let ours = serve_watcher(&c).unwrap_or(false);
            let host = format!("org.kde.StatusNotifierHost-{}-tessera", std::process::id());
            let _ = c.request_name(host.as_str());
            let w = proxy(&c, WATCHER, WATCHER_PATH, WATCHER)?;
            let _: zbus::Result<()> = w.call("RegisterStatusNotifierHost", &(host.as_str(),));
            let (dirty, wake) = mpsc::channel::<()>();
            let sig = |member: &'static str| MatchRule::builder().msg_type(zbus::message::Type::Signal).interface(WATCHER).and_then(|b| b.member(member)).map(|b| b.build());
            watch_rule(&c, sig("StatusNotifierItemRegistered")?, dirty.clone())?;
            watch_rule(&c, sig("StatusNotifierItemUnregistered")?, dirty.clone())?;
            watch_rule(&c, MatchRule::builder().msg_type(zbus::message::Type::Signal).interface(ITEM)?.build(), dirty.clone())?;
            // (the QML shell's watcher can go and come back: then serve or re-register)
            watch_rule(&c, MatchRule::builder().msg_type(zbus::message::Type::Signal).sender("org.freedesktop.DBus")?.member("NameOwnerChanged")?.arg(0, WATCHER)?.build(), dirty)?;
            let mut last: Option<Tray> = None;
            let mut owner_was = ours;
            loop {
                let w = proxy(&c, WATCHER, WATCHER_PATH, WATCHER)?;
                let keys: Vec<String> = w.get_property("RegisteredStatusNotifierItems").unwrap_or_default();
                if keys.is_empty() && !owner_was {
                    // the watcher left (or isn't there): serve it now
                    owner_was = serve_watcher(&c).unwrap_or(false);
                    let _: zbus::Result<()> = w.call("RegisterStatusNotifierHost", &(host.as_str(),));
                }
                let tray = Tray { items: keys.iter().filter_map(|k| read_item(&c, k)).collect() };
                if last.as_ref() != Some(&tray) {
                    tx.send(Msg::Data(Box::new(tray.clone()))).map_err(|_| zbus::Error::Failure("ui gone".into()))?;
                    last = Some(tray);
                }
                if wake.recv().is_err() {
                    return Ok(());
                }
                while wake.recv_timeout(Duration::from_millis(60)).is_ok() {}
            }
        };
        if let Err(zbus::Error::Failure(_)) = run() {
            return;
        }
        std::thread::sleep(Duration::from_secs(3));
    });
}

/// Left click: Activate (an item that's only a menu answers with its menu instead)
pub fn activate(key: &str, x: i32, y: i32) {
    let key = key.to_string();
    std::thread::spawn(move || {
        let Some(c) = conn() else { return };
        let (svc, path) = parse_address(&key, "");
        if let Ok(p) = proxy(c, &svc, &path, ITEM) {
            let _: zbus::Result<()> = p.call("Activate", &(x, y));
        };
    });
}

/// Read an item's menu; the answer comes back to `reply` (Msg::Part) as a TrayMenu
pub fn menu(key: &str, menu_path: &str, reply: impl Fn(Box<dyn std::any::Any + Send>) + Send + 'static) {
    let (key, menu_path) = (key.to_string(), menu_path.to_string());
    std::thread::spawn(move || {
        let Some(c) = conn() else { return };
        let (svc, _) = parse_address(&key, "");
        let Ok(p) = proxy(c, &svc, &menu_path, MENU) else { return };
        let _: zbus::Result<bool> = p.call("AboutToShow", &(0i32,));
        let Ok(m) = p.call_method("GetLayout", &(0i32, -1i32, Vec::<String>::new())) else { return };
        let Ok((_rev, layout)) = m.body().deserialize::<(u32, OwnedValue)>() else { return };
        if let Some(root) = parse_node(&layout) {
            reply(Box::new(TrayMenu { key, root }));
        }
    });
}

/// A submenu is about to show (some apps fill it only then); its fresh layout comes back like `menu`'s
pub fn submenu(key: &str, menu_path: &str, id: i32, reply: impl Fn(Box<dyn std::any::Any + Send>) + Send + 'static) {
    let (key, menu_path) = (key.to_string(), menu_path.to_string());
    std::thread::spawn(move || {
        let Some(c) = conn() else { return };
        let (svc, _) = parse_address(&key, "");
        let Ok(p) = proxy(c, &svc, &menu_path, MENU) else { return };
        let _: zbus::Result<bool> = p.call("AboutToShow", &(id,));
        let Ok(m) = p.call_method("GetLayout", &(id, -1i32, Vec::<String>::new())) else { return };
        let Ok((_rev, layout)) = m.body().deserialize::<(u32, OwnedValue)>() else { return };
        if let Some(root) = parse_node(&layout) {
            reply(Box::new(TrayMenu { key, root }));
        }
    });
}

/// A menu entry was clicked
pub fn menu_event(key: &str, menu_path: &str, id: i32) {
    let (key, menu_path) = (key.to_string(), menu_path.to_string());
    std::thread::spawn(move || {
        let Some(c) = conn() else { return };
        let (svc, _) = parse_address(&key, "");
        if let Ok(p) = proxy(c, &svc, &menu_path, MENU) {
            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as u32).unwrap_or(0);
            let _: zbus::Result<()> = p.call("Event", &(id, "clicked", Value::from(0i32), t));
        };
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::{Dict, Signature, StructureBuilder};

    #[test]
    fn addresses_in_both_forms() {
        assert_eq!(parse_address("org.kde.StatusNotifierItem-12-1", ":1.5"), ("org.kde.StatusNotifierItem-12-1".into(), "/StatusNotifierItem".into()));
        assert_eq!(parse_address(":1.45/org/ayatana/NotificationItem/nm", ""), (":1.45".into(), "/org/ayatana/NotificationItem/nm".into()));
        // (only a path: the one who registered it)
        assert_eq!(parse_address("/StatusNotifierItem", ":1.9"), (":1.9".into(), "/StatusNotifierItem".into()));
    }

    #[test]
    fn pixmaps_become_rgba_at_the_nearest_size() {
        let px = |w: i32, argb: [u8; 4]| (w, w, argb.repeat((w * w) as usize));
        let list = vec![px(16, [255, 1, 2, 3]), px(48, [128, 10, 20, 30]), px(22, [255, 9, 9, 9])];
        let (w, h, d) = best_pixmap(&list, 20).unwrap();
        assert_eq!((w, h), (22, 22));
        assert_eq!(&d[..4], &[9, 9, 9, 255]);
        let (w, _, d) = best_pixmap(&list, 64).unwrap(); // none that big: the largest
        assert_eq!(w, 48);
        assert_eq!(&d[..4], &[10, 20, 30, 128]);
        // a truncated one is skipped
        assert_eq!(best_pixmap(&[(8, 8, vec![0; 10])], 8), None);
    }

    #[test]
    fn mnemonics_go() {
        assert_eq!(strip_mnemonic("_Quit"), "Quit");
        assert_eq!(strip_mnemonic("Save__As"), "Save_As");
        assert_eq!(strip_mnemonic("Plain"), "Plain");
    }

    fn node(id: i32, props: &[(&str, Value<'static>)], children: Vec<Value<'static>>) -> Value<'static> {
        let mut d = Dict::new(&Signature::Str, &Signature::Variant);
        for (k, v) in props {
            d.append(Value::from(k.to_string()), Value::Value(Box::new(v.clone()))).unwrap();
        }
        let kids: Vec<Value<'static>> = children.into_iter().map(|c| Value::Value(Box::new(c))).collect();
        let arr = zbus::zvariant::Array::from(kids);
        Value::Structure(StructureBuilder::new().add_field(id).append_field(Value::Dict(d)).append_field(Value::Array(arr)).build().unwrap())
    }

    #[test]
    fn a_dbusmenu_layout() {
        let sub = node(5, &[("label", Value::from("_Recent"))], vec![node(6, &[("label", Value::from("a.txt"))], vec![])]);
        let root = node(
            0,
            &[],
            vec![
                node(1, &[("label", Value::from("_Show")), ("enabled", Value::from(false))], vec![]),
                node(2, &[("type", Value::from("separator"))], vec![]),
                node(3, &[("label", Value::from("Mute")), ("toggle-type", Value::from("checkmark")), ("toggle-state", Value::from(1i32))], vec![]),
                node(4, &[("label", Value::from("Hidden")), ("visible", Value::from(false))], vec![]),
                sub,
            ],
        );
        let r = parse_node(&root).unwrap();
        assert_eq!(r.children.len(), 5);
        assert_eq!((r.children[0].label.as_str(), r.children[0].enabled), ("Show", false));
        assert!(r.children[1].separator);
        assert_eq!(r.children[2].checked, Some(true));
        assert!(!r.children[3].visible);
        assert_eq!(r.children[4].children[0].label, "a.txt");
        assert_eq!(r.children[0].checked, None);
    }

    #[test]
    fn the_tooltip_falls_back_to_the_title_then_the_id() {
        let mut i = TrayItem { key: "k".into(), id: "nm-applet".into(), title: "".into(), tooltip: "".into(), status: "Active".into(), icon: Icon::None, menu: "".into(), item_is_menu: false };
        assert_eq!(i.label(), "nm-applet");
        i.title = "Network".into();
        assert_eq!(i.label(), "Network");
        i.tooltip = "Wired connected".into();
        assert_eq!(i.label(), "Wired connected");
    }
}
