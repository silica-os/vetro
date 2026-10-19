//! Accessibility: what every view tells assistive technologies (roles, names, values,
//! states, actions), and the tree built from it. The platform layer exposes the tree through
//! AccessKit's AT-SPI adapter (Orca).

use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;
use crate::geom::Rect;

/// A role (a stable ABI number each).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum Role {
    /// A generic container (no semantics of its own).
    #[default]
    Group = 0,
    /// A window.
    Window = 1,
    /// A push, toolbar or help button.
    Button = 2,
    /// A checkbox.
    CheckBox = 3,
    /// A radio button.
    RadioButton = 4,
    /// A switch.
    Switch = 5,
    /// Static text.
    Label = 6,
    /// A one-line text field.
    TextField = 7,
    /// A multi-line text view.
    TextArea = 8,
    /// A slider.
    Slider = 9,
    /// A stepper (spin button).
    Stepper = 10,
    /// A progress bar or spinner.
    ProgressIndicator = 11,
    /// A pop-up or pull-down button.
    PopUpButton = 12,
    /// A combo box.
    ComboBox = 13,
    /// A segmented control (a tab list).
    SegmentedControl = 14,
    /// One segment (a tab).
    Tab = 15,
    /// A list.
    List = 16,
    /// A list row.
    ListItem = 17,
    /// A table.
    Table = 18,
    /// A table row.
    Row = 19,
    /// A table cell.
    Cell = 20,
    /// A menu.
    Menu = 21,
    /// A menu item.
    MenuItem = 22,
    /// An image.
    Image = 23,
    /// A link.
    Link = 24,
    /// A scroll area.
    ScrollArea = 25,
    /// A scroll bar.
    ScrollBar = 26,
    /// A toolbar.
    Toolbar = 27,
    /// An alert or dialog.
    Alert = 28,
    /// A dialog (sheet, panel).
    Dialog = 29,
    /// A disclosure triangle.
    Disclosure = 30,
    /// A group box.
    GroupBox = 31,
    /// A split view's divider.
    Splitter = 32,
    /// A tooltip.
    Tooltip = 33,
    /// A colour well.
    ColorWell = 34,
    /// A date picker.
    DatePicker = 35,
    /// A search field.
    SearchField = 36,
    /// A heading.
    Heading = 37,
    /// A tree (outline view).
    Tree = 38,
    /// A tree item.
    TreeItem = 39,
    /// A status bar.
    StatusBar = 40,
    /// A level indicator / gauge.
    Meter = 41,
    /// A popover.
    Popover = 42,
    /// A password field.
    PasswordField = 43,
}

/// Toggle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggled {
    /// Off.
    False,
    /// On.
    True,
    /// Mixed.
    Mixed,
}

/// An action an assistive technology can perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Click / activate.
    Click,
    /// Focus.
    Focus,
    /// Increment a value.
    Increment,
    /// Decrement a value.
    Decrement,
    /// Expand.
    Expand,
    /// Collapse.
    Collapse,
    /// Show the context menu.
    ShowMenu,
    /// Scroll into view.
    ScrollIntoView,
    /// Set the value (text).
    SetValue,
}

/// A view's accessibility description.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct A11y {
    /// Role.
    pub role: Role,
    /// Name (label).
    pub label: Option<String>,
    /// Text value or value description.
    pub value: Option<String>,
    /// Numeric value, minimum, maximum and step.
    pub numeric: Option<(f64, f64, f64, f64)>,
    /// Description (help).
    pub description: Option<String>,
    /// Toggle state.
    pub toggled: Option<Toggled>,
    /// Selected (rows, segments).
    pub selected: Option<bool>,
    /// Expanded (disclosures, pop-ups with an open menu).
    pub expanded: Option<bool>,
    /// Supported actions.
    pub actions: Vec<Action>,
    /// Text selection (byte offsets) for text roles.
    pub text_selection: Option<(usize, usize)>,
    /// Live region (announced when it changes): polite.
    pub live: bool,
    /// Hidden from assistive technologies (decorations).
    pub hidden: bool,
    /// Keyboard shortcut.
    pub shortcut: Option<String>,
}

/// One node of the accessibility tree.
#[derive(Debug, Clone, PartialEq)]
pub struct A11yNode {
    /// The view.
    pub id: ViewId,
    /// The description, with the generic overrides applied.
    pub info: A11y,
    /// Bounds in window coordinates.
    pub bounds: Rect,
    /// Focused.
    pub focused: bool,
    /// Disabled.
    pub disabled: bool,
    /// Children in the accessibility tree (groups without semantics are flattened).
    pub children: Vec<ViewId>,
}

impl Ui {
    /// The accessibility description of one view.
    pub fn accessibility_of(&self, id: ViewId) -> Option<A11y> {
        let node = self.tree.get(id)?;
        let mut a = A11y::default();
        if let Some(w) = node.widget.as_deref() {
            w.accessibility(&mut a);
        }
        if let Some(l) = &node.a11y.label {
            a.label = Some(l.clone());
        }
        if let Some(h) = &node.a11y.help {
            a.description = Some(h.clone());
        } else if a.description.is_none() {
            a.description.clone_from(&node.tooltip);
        }
        Some(a)
    }

    /// The accessibility tree of the subtree at `root`, in pre-order. Views that are hidden
    /// or have nothing to say (a group without a label) are left out and their children
    /// lifted to the nearest kept ancestor.
    pub fn accessibility_tree(&self, root: ViewId) -> Vec<A11yNode> {
        let mut out: Vec<A11yNode> = Vec::new();
        self.collect_a11y(root, None, &mut out);
        out
    }

    fn collect_a11y(&self, id: ViewId, parent_index: Option<usize>, out: &mut Vec<A11yNode>) {
        let flags = self.tree.flags(id);
        if flags.contains(NodeFlags::HIDDEN) {
            return;
        }
        let Some(info) = self.accessibility_of(id) else {
            return;
        };
        let keep = !info.hidden
            && (info.role != Role::Group || info.label.is_some() || parent_index.is_none());
        let my_index = if keep {
            out.push(A11yNode {
                id,
                info,
                bounds: self.tree.window_rect(id),
                focused: flags.contains(NodeFlags::FOCUSED),
                disabled: self.is_disabled(id),
                children: Vec::new(),
            });
            let idx = out.len() - 1;
            if let Some(p) = parent_index.and_then(|p| out.get_mut(p)) {
                p.children.push(id);
            }
            Some(idx)
        } else {
            parent_index
        };
        for c in self.tree.children(id).to_vec() {
            self.collect_a11y(c, my_index, out);
        }
    }
}

impl Ui {
    /// Performs an accessibility action on a view (Orca, tests): a click activates a control,
    /// increment and decrement press the arrow keys, expand and collapse toggle a disclosure,
    /// show-menu asks for the context menu.
    pub fn perform(&mut self, id: ViewId, action: Action) -> bool {
        use crate::core::event::{Event, Key, KeyEvent, Mods};
        if !self.tree.contains(id) || self.is_disabled(id) {
            return false;
        }
        let handled = match action {
            Action::Click => self.send(id, &Event::Activate),
            Action::Focus => {
                self.set_focus(id, true);
                true
            }
            Action::Increment => {
                self.send(id, &Event::Key(KeyEvent::press(Key::Up, Mods::empty())))
            }
            Action::Decrement => {
                self.send(id, &Event::Key(KeyEvent::press(Key::Down, Mods::empty())))
            }
            Action::Expand | Action::Collapse => self.send(id, &Event::Activate),
            Action::ShowMenu => {
                let at = self.tree.window_rect(id).center();
                self.emit(
                    id,
                    crate::core::value::Signal::ContextMenu,
                    crate::core::value::Value::Point(at),
                );
                true
            }
            Action::ScrollIntoView | Action::SetValue => false,
        };
        self.flush();
        handled
    }
}
