//! Accessibility on the desktop: AccessKit's AT-SPI adapter (Orca) per window, fed with the
//! core's accessibility tree, and the assistive technologies' actions delivered back to the
//! core on the UI thread.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use accesskit::{
    Action as AkAction, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Node,
    NodeId, Rect as AkRect, Role as AkRole, Toggled as AkToggled, TreeId, TreeInfo, TreeUpdate,
};
use accesskit_unix::Adapter;
use smithay_client_toolkit::reexports::calloop::ping::Ping;

use crate::core::a11y::{A11yNode, Action, Role, Toggled};
use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::core::window::WindowId;

/// Wakes the loop and records what the adapter threads want.
#[derive(Clone)]
struct Shared {
    window: WindowId,
    active: Arc<AtomicBool>,
    actions: Arc<Mutex<Vec<(WindowId, ActionRequest)>>>,
    ping: Ping,
}

impl ActivationHandler for Shared {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        // The tree lives on the UI thread: it is sent from there, by the next loop turn.
        self.active.store(true, Ordering::SeqCst);
        self.ping.ping();
        None
    }
}

impl ActionHandler for Shared {
    fn do_action(&mut self, request: ActionRequest) {
        if let Ok(mut q) = self.actions.lock() {
            q.push((self.window, request));
        }
        self.ping.ping();
    }
}

impl DeactivationHandler for Shared {
    fn deactivate_accessibility(&mut self) {
        self.active.store(false, Ordering::SeqCst);
    }
}

/// The adapters of the app's windows.
pub struct Accessibility {
    adapters: HashMap<WindowId, (Adapter, Arc<AtomicBool>)>,
    actions: Arc<Mutex<Vec<(WindowId, ActionRequest)>>>,
    ping: Ping,
}

fn role(r: Role) -> AkRole {
    match r {
        Role::Group => AkRole::Group,
        Role::Window => AkRole::Window,
        Role::Button => AkRole::Button,
        Role::CheckBox => AkRole::CheckBox,
        Role::RadioButton => AkRole::RadioButton,
        Role::Switch => AkRole::Switch,
        Role::Label => AkRole::Label,
        Role::TextField | Role::SearchField => AkRole::TextInput,
        Role::PasswordField => AkRole::PasswordInput,
        Role::TextArea => AkRole::MultilineTextInput,
        Role::Slider => AkRole::Slider,
        Role::Stepper => AkRole::SpinButton,
        Role::ProgressIndicator => AkRole::ProgressIndicator,
        Role::PopUpButton => AkRole::ComboBox,
        Role::ComboBox => AkRole::EditableComboBox,
        Role::SegmentedControl => AkRole::TabList,
        Role::Tab => AkRole::Tab,
        Role::List => AkRole::List,
        Role::ListItem => AkRole::ListItem,
        Role::Table => AkRole::Table,
        Role::Row => AkRole::Row,
        Role::Cell => AkRole::Cell,
        Role::Menu => AkRole::Menu,
        Role::MenuItem => AkRole::MenuItem,
        Role::Image => AkRole::Image,
        Role::Link => AkRole::Link,
        Role::ScrollArea => AkRole::ScrollView,
        Role::ScrollBar => AkRole::ScrollBar,
        Role::Toolbar => AkRole::Toolbar,
        Role::Alert => AkRole::AlertDialog,
        Role::Dialog => AkRole::Dialog,
        Role::Disclosure => AkRole::DisclosureTriangle,
        Role::GroupBox => AkRole::Group,
        Role::Splitter => AkRole::Splitter,
        Role::Tooltip => AkRole::Tooltip,
        Role::ColorWell => AkRole::ColorWell,
        Role::DatePicker => AkRole::DateInput,
        Role::Heading => AkRole::Heading,
        Role::Tree => AkRole::Tree,
        Role::TreeItem => AkRole::TreeItem,
        Role::StatusBar => AkRole::Status,
        Role::Meter => AkRole::Meter,
        Role::Popover => AkRole::Dialog,
    }
}

fn node_id(v: ViewId) -> NodeId {
    NodeId(v.to_raw())
}

/// The AccessKit node of a core node.
fn ak_node(n: &A11yNode) -> Node {
    let i = &n.info;
    let mut node = Node::new(role(i.role));
    if let Some(l) = &i.label {
        node.set_label(l.as_str());
    }
    if let Some(v) = &i.value {
        node.set_value(v.as_str());
    }
    if let Some(d) = &i.description {
        node.set_description(d.as_str());
    }
    if let Some((v, min, max, step)) = i.numeric {
        node.set_numeric_value(v);
        node.set_min_numeric_value(min);
        node.set_max_numeric_value(max);
        if step > 0.0 {
            node.set_numeric_value_step(step);
        }
    }
    if let Some(t) = i.toggled {
        node.set_toggled(match t {
            Toggled::False => AkToggled::False,
            Toggled::True => AkToggled::True,
            Toggled::Mixed => AkToggled::Mixed,
        });
    }
    if let Some(s) = i.selected {
        node.set_selected(s);
    }
    if let Some(e) = i.expanded {
        node.set_expanded(e);
    }
    if n.disabled {
        node.set_disabled();
    }
    for a in &i.actions {
        node.add_action(match a {
            Action::Click => AkAction::Click,
            Action::Focus => AkAction::Focus,
            Action::Increment => AkAction::Increment,
            Action::Decrement => AkAction::Decrement,
            Action::Expand => AkAction::Expand,
            Action::Collapse => AkAction::Collapse,
            Action::ShowMenu => AkAction::ShowContextMenu,
            Action::ScrollIntoView => AkAction::ScrollIntoView,
            Action::SetValue => AkAction::SetValue,
        });
    }
    let b = n.bounds;
    node.set_bounds(AkRect {
        x0: f64::from(b.x),
        y0: f64::from(b.y),
        x1: f64::from(b.right()),
        y1: f64::from(b.bottom()),
    });
    node.set_children(n.children.iter().map(|c| node_id(*c)).collect::<Vec<_>>());
    node
}

/// The full tree of a window.
pub fn tree_update(ui: &Ui, window: WindowId) -> Option<TreeUpdate> {
    let w = ui.window(window)?;
    let nodes = ui.accessibility_tree(w.root);
    let root = nodes.first()?.id;
    let focus = w
        .focus
        .filter(|f| nodes.iter().any(|n| n.id == *f))
        .unwrap_or(root);
    let mut out: Vec<(NodeId, Node)> = nodes.iter().map(|n| (node_id(n.id), ak_node(n))).collect();
    if let Some((_, r)) = out.first_mut() {
        r.set_role(AkRole::Window);
        r.set_label(w.title.as_str());
    }
    let mut info = TreeInfo::new(node_id(root));
    info.toolkit_name = Some("Vetro".into());
    info.toolkit_version = Some(env!("CARGO_PKG_VERSION").into());
    Some(TreeUpdate {
        nodes: out,
        tree: Some(info),
        tree_id: TreeId::ROOT,
        focus: node_id(focus),
    })
}

impl Accessibility {
    /// The bridge; `ping` wakes the event loop when an adapter wants something.
    pub fn new(ping: Ping) -> Self {
        Accessibility {
            adapters: HashMap::new(),
            actions: Arc::new(Mutex::new(Vec::new())),
            ping,
        }
    }

    /// An adapter for a mapped window.
    pub fn add_window(&mut self, window: WindowId) {
        if self.adapters.contains_key(&window) {
            return;
        }
        let active = Arc::new(AtomicBool::new(false));
        let shared = Shared {
            window,
            active: active.clone(),
            actions: self.actions.clone(),
            ping: self.ping.clone(),
        };
        let adapter = Adapter::new(shared.clone(), shared.clone(), shared);
        self.adapters.insert(window, (adapter, active));
    }

    /// Drops a window's adapter.
    pub fn remove_window(&mut self, window: WindowId) {
        self.adapters.remove(&window);
    }

    /// The window's keyboard focus changed.
    pub fn set_focused(&mut self, window: WindowId, focused: bool) {
        if let Some((a, _)) = self.adapters.get_mut(&window) {
            a.update_window_focus_state(focused);
        }
    }

    /// Sends the trees of active adapters and performs queued actions.
    pub fn sync(&mut self, ui: &mut Ui) {
        for (w, (a, active)) in &mut self.adapters {
            if active.load(Ordering::SeqCst) {
                let window = *w;
                a.update_if_active(|| {
                    tree_update(ui, window).unwrap_or(TreeUpdate {
                        nodes: Vec::new(),
                        tree: None,
                        tree_id: TreeId::ROOT,
                        focus: NodeId(0),
                    })
                });
            }
        }
        let queued = match self.actions.lock() {
            Ok(mut q) => std::mem::take(&mut *q),
            Err(_) => Vec::new(),
        };
        for (_, req) in queued {
            let Some(view) = ViewId::from_raw(req.target_node.0) else {
                continue;
            };
            let action = match req.action {
                AkAction::Click => Action::Click,
                AkAction::Focus => Action::Focus,
                AkAction::Increment => Action::Increment,
                AkAction::Decrement => Action::Decrement,
                AkAction::Expand => Action::Expand,
                AkAction::Collapse => Action::Collapse,
                AkAction::ShowContextMenu => Action::ShowMenu,
                _ => continue,
            };
            ui.perform(view, action);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amalfi::button::PushButton;
    use crate::core::layout::Stack;
    use crate::core::window::WindowKind;
    use crate::geom::Size;

    #[test]
    fn tree_of_a_window() {
        let mut ui = Ui::new_headless();
        let w = ui.create_window(WindowKind::Normal, Size::new(200.0, 100.0));
        ui.set_title(w, "Demo");
        let row = ui.create(Stack::row(4.0));
        let b = ui.create(PushButton::new("OK"));
        ui.add_child(row, b);
        ui.set_content(w, row);
        ui.prepare_frame(w);
        let t = tree_update(&ui, w).unwrap();
        assert_eq!(t.nodes.len(), 2);
        assert_eq!(t.nodes[0].1.role(), AkRole::Window);
        assert_eq!(t.nodes[0].1.label(), Some("Demo"));
        let button = &t.nodes[1].1;
        assert_eq!(button.role(), AkRole::Button);
        assert_eq!(button.label(), Some("OK"));
        assert!(button.supports_action(AkAction::Click));
    }
}
