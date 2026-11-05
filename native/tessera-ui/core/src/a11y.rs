//! Accessibility: an AT-SPI tree for each layer surface through AccessKit's
//! Unix adapter. Slint does this only in its winit backend (accesskit_winit);
//! a platform of our own has to walk Slint's item tree itself. This is a
//! small version of i-slint-backend-winit's accesskit.rs: Slint's internal
//! accessibility API (i-slint-core, pinned to one version), the whole tree
//! rebuilt when a property it read changes, only while a screen reader
//! listens (AccessKit asks for the tree on activation).

use crate::wayland::Surface;
use crate::Msg;
use accesskit::{Action, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Node, NodeId, Rect, Role, TreeId, TreeInfo, TreeUpdate};
use i_slint_core::accessibility::{accessible_descendents, AccessibilityAction, AccessibleStringProperty, SupportedAccessibilityAction};
use i_slint_core::input::FocusReason;
use i_slint_core::item_tree::{ItemTreeRc, ItemTreeRef, ItemTreeWeak};
use i_slint_core::items::{AccessibleRole, ItemRc, WindowItem};
use i_slint_core::properties::PropertyTracker;
use i_slint_core::window::WindowInner;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::cell::Cell;
use std::collections::HashMap;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};

thread_local! {
    // Slint moved the keyboard focus between items (WindowAdapterInternal)
    pub static FOCUS_DIRTY: Cell<bool> = const { Cell::new(false) };
}
static NEXT: AtomicU64 = AtomicU64::new(1);

struct Activation(u64, Sender<Msg>);
impl ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        // (called on AccessKit's thread: the tree is built on the UI thread)
        let _ = self.1.send(Msg::A11yActivate(self.0));
        None
    }
}
struct Actions(u64, Sender<Msg>);
impl ActionHandler for Actions {
    fn do_action(&mut self, r: ActionRequest) {
        let _ = self.1.send(Msg::A11yAction(self.0, r));
    }
}
struct Deactivation;
impl DeactivationHandler for Deactivation {
    fn deactivate_accessibility(&mut self) {}
}

pub struct A11y {
    pub id: u64,
    adapter: accesskit_unix::Adapter,
    surface: Weak<Surface>,
    tracker: Pin<Box<PropertyTracker>>,
    active: bool,
    focused: bool,
    components: HashMap<usize, u32>,
    by_id: HashMap<u32, ItemTreeWeak>,
}

impl A11y {
    pub fn new(surface: &Rc<Surface>, tx: Sender<Msg>) -> Option<Self> {
        if crate::env::is_set("NO_A11Y") {
            return None;
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let adapter = accesskit_unix::Adapter::new(Activation(id, tx.clone()), Actions(id, tx), Deactivation);
        Some(Self {
            id,
            adapter,
            surface: Rc::downgrade(surface),
            tracker: Box::pin(PropertyTracker::default()),
            active: false,
            focused: false,
            components: HashMap::new(),
            by_id: HashMap::new(),
        })
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.update(true, self.focused);
    }

    /// After each loop turn: send a new tree if something it shows changed
    pub fn update(&mut self, force: bool, focused: bool) {
        if focused != self.focused {
            self.focused = focused;
            self.adapter.update_window_focus_state(focused);
        }
        if !self.active || !(force || self.tracker.is_dirty() || FOCUS_DIRTY.get()) {
            return;
        }
        let Some(s) = self.surface.upgrade() else { return };
        let mut update = None;
        // (the tracker records what the build reads; it's taken out while it runs)
        let tracker = std::mem::replace(&mut self.tracker, Box::pin(PropertyTracker::default()));
        tracker.as_ref().evaluate(|| update = self.build(&s));
        self.tracker = tracker;
        if let Some(u) = update {
            self.adapter.update_if_active(|| u);
        }
    }

    pub fn action(&mut self, r: ActionRequest) {
        let Some(s) = self.surface.upgrade() else { return };
        let Some(item) = self.item(r.target_node) else { return };
        match r.action {
            Action::Click => {
                item.accessible_action(&AccessibilityAction::Default);
            }
            Action::Focus => {
                WindowInner::from_pub(s.window()).set_focus_item(&item, true, FocusReason::PointerClick);
            }
            _ => {}
        }
    }

    fn encode(&mut self, item: &ItemRc) -> NodeId {
        let tree = item.item_tree();
        let ptr = ItemTreeRef::as_ptr(ItemTreeRc::borrow(tree)).as_ptr() as usize;
        let n = self.components.len() as u32 + 1;
        let c = *self.components.entry(ptr).or_insert(n);
        self.by_id.entry(c).or_insert_with(|| ItemTreeRc::downgrade(tree));
        NodeId(((c as u64) << 32) | item.index() as u64)
    }

    fn item(&self, id: NodeId) -> Option<ItemRc> {
        let tree = self.by_id.get(&((id.0 >> 32) as u32))?.upgrade()?;
        Some(ItemRc::new(tree, (id.0 & 0xffff_ffff) as u32))
    }

    fn build(&mut self, s: &Rc<Surface>) -> Option<TreeUpdate> {
        let window = s.window();
        let inner = WindowInner::from_pub(window);
        let root = ItemRc::new_root(inner.try_component()?);
        let scale = window.scale_factor();
        let mut nodes = Vec::new();
        let root_id = self.node(root, &mut nodes, scale);
        let focus = inner
            .focus_item
            .borrow()
            .upgrade()
            .map(|mut f| {
                while !f.is_accessible() {
                    match f.parent_item(i_slint_core::item_tree::ParentItemTraversalMode::StopAtPopups) {
                        Some(p) => f = p,
                        None => break,
                    }
                }
                self.encode(&f)
            })
            .unwrap_or(root_id);
        FOCUS_DIRTY.set(false);
        let mut info = TreeInfo::new(root_id);
        info.toolkit_name = Some("Slint (tessera-ui)".into());
        info.toolkit_version = Some("1.18.1".into());
        Some(TreeUpdate { nodes, tree: Some(info), tree_id: TreeId::ROOT, focus })
    }

    fn node(&mut self, item: ItemRc, nodes: &mut Vec<(NodeId, Node)>, scale: f32) -> NodeId {
        let id = self.encode(&item);
        let children: Vec<NodeId> = accessible_descendents(&item).map(|c| self.node(c, nodes, scale)).collect();
        let prop = |p| item.accessible_string_property(p);
        let role = if let Some(w) = item.downcast::<WindowItem>() {
            let _ = w;
            Role::Window
        } else {
            match item.accessible_role() {
                AccessibleRole::Button => Role::Button,
                AccessibleRole::Checkbox => Role::CheckBox,
                AccessibleRole::Combobox => Role::ComboBox,
                AccessibleRole::Groupbox => Role::Group,
                AccessibleRole::List => Role::ListBox,
                AccessibleRole::ListItem => Role::ListBoxOption,
                AccessibleRole::Slider => Role::Slider,
                AccessibleRole::Tab => Role::Tab,
                AccessibleRole::TabList => Role::TabList,
                AccessibleRole::TabPanel => Role::TabPanel,
                AccessibleRole::Text => Role::Label,
                AccessibleRole::TextInput => Role::TextInput,
                AccessibleRole::Image => Role::Image,
                AccessibleRole::Switch => Role::Switch,
                AccessibleRole::ProgressIndicator => Role::ProgressIndicator,
                _ => Role::Unknown,
            }
        };
        let mut n = Node::new(role);
        let label = if role == Role::Window {
            item.downcast::<WindowItem>().map(|w| w.as_pin_ref().title().to_string())
        } else {
            prop(AccessibleStringProperty::Label).map(|l| l.to_string())
        };
        if let Some(l) = label.filter(|l| !l.is_empty()) {
            if role == Role::Label {
                n.set_value(l);
            } else {
                n.set_label(l);
            }
        }
        if let Some(v) = prop(AccessibleStringProperty::Value).filter(|v| !v.is_empty()) {
            n.set_value(v.to_string());
        }
        if let Some(p) = prop(AccessibleStringProperty::PlaceholderText) {
            n.set_placeholder(p.to_string());
        }
        if let Some(d) = prop(AccessibleStringProperty::Description) {
            n.set_description(d.to_string());
        }
        if prop(AccessibleStringProperty::Expandable).is_some_and(|x| x == "true") {
            n.set_expanded(prop(AccessibleStringProperty::Expanded).is_some_and(|x| x == "true"));
        }
        if prop(AccessibleStringProperty::ItemSelectable).is_some_and(|x| x == "true") {
            n.set_selected(prop(AccessibleStringProperty::ItemSelected).is_some_and(|x| x == "true"));
        }
        if !item.is_visible() {
            n.set_hidden();
        }
        let g = item.geometry();
        let o = item.map_to_window(g.origin);
        n.set_bounds(Rect {
            x0: (o.x * scale) as f64,
            y0: (o.y * scale) as f64,
            x1: ((o.x + g.size.width) * scale) as f64,
            y1: ((o.y + g.size.height) * scale) as f64,
        });
        if item.supported_accessibility_actions().contains(SupportedAccessibilityAction::Default) {
            n.add_action(Action::Click);
        }
        if matches!(role, Role::Button | Role::TextInput | Role::ListBoxOption | Role::Tab) {
            n.add_action(Action::Focus);
        }
        n.set_children(children);
        nodes.push((id, n));
        id
    }
}
