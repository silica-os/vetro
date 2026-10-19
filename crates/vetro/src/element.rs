//! Elements: the values a component's `body` returns, built with functions and modifiers.

use std::any::{Any, TypeId};
use std::rc::Rc;

use vetro_kit::{Color, Kind, Prop, Signal, Value, View as KitView};

use crate::Cx;

/// A property value of an element (compared between renders).
#[derive(Debug, Clone, PartialEq)]
pub enum PValue {
    /// Clears the property.
    None,
    /// Boolean.
    Bool(bool),
    /// Integer.
    Int(i64),
    /// Number.
    Float(f64),
    /// Text.
    Str(Rc<str>),
    /// Colour.
    Color(Color),
    /// Insets: top, left, bottom, right.
    Insets([f32; 4]),
    /// Size.
    Size(f32, f32),
    /// Point.
    Point(f32, f32),
    /// List of strings.
    Strings(Rc<[String]>),
}

impl PValue {
    pub(crate) fn apply(&self, view: &KitView, prop: Prop) {
        let r = match self {
            PValue::None => view.unset(prop),
            PValue::Bool(b) => view.set_bool(prop, *b),
            PValue::Int(i) => view.set_int(prop, *i),
            PValue::Float(f) => view.set_float(prop, *f),
            PValue::Str(s) => view.set_str(prop, s),
            PValue::Color(c) => view.set_color(prop, *c),
            PValue::Insets([t, l, b, r]) => view.set_insets(prop, *t, *l, *b, *r),
            PValue::Size(w, h) => view.set_size(prop, *w, *h),
            PValue::Point(x, y) => view.set_point(prop, *x, *y),
            PValue::Strings(l) => view.set_strings(prop, l),
        };
        // A property the view does not have is a programming error in the element: report it
        // once per render in debug builds, never crash.
        if let Err(e) = r
            && cfg!(debug_assertions)
        {
            eprintln!("vetro: setting {prop:?}: {e}");
        }
    }
}

/// An event handler of an element.
pub type Handler = Rc<dyn Fn(&Value)>;

/// A key distinguishing siblings across renders (`ForEach` ids).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// A number.
    Int(i64),
    /// A string.
    Str(Rc<str>),
}

impl From<i64> for Key {
    fn from(v: i64) -> Self {
        Key::Int(v)
    }
}
impl From<usize> for Key {
    fn from(v: usize) -> Self {
        Key::Int(i64::try_from(v).unwrap_or(i64::MAX))
    }
}
impl From<u32> for Key {
    fn from(v: u32) -> Self {
        Key::Int(i64::from(v))
    }
}
impl From<&str> for Key {
    fn from(v: &str) -> Self {
        Key::Str(Rc::from(v))
    }
}
impl From<String> for Key {
    fn from(v: String) -> Self {
        Key::Str(Rc::from(v.as_str()))
    }
}

/// A component: a reusable piece of UI with its own state.
pub trait Component: 'static {
    /// The element this component shows, computed from its fields and state.
    fn body(&self, cx: &mut Cx) -> Element;
}

/// A component behind a type-erased handle.
pub(crate) trait AnyComponent {
    fn body(&self, cx: &mut Cx) -> Element;
    fn type_id_of(&self) -> TypeId;
}

impl<C: Component> AnyComponent for C {
    fn body(&self, cx: &mut Cx) -> Element {
        Component::body(self, cx)
    }
    fn type_id_of(&self) -> TypeId {
        TypeId::of::<C>()
    }
}

/// What an element makes.
#[derive(Clone)]
pub(crate) enum What {
    /// A view of a kind.
    Kind(Kind),
    /// A component.
    Component(Rc<dyn AnyComponent>),
    /// An imperative view hosted as is.
    Host(KitView),
    /// A custom-drawn view, made once per position by the factory.
    Custom(TypeId, Rc<dyn Fn() -> Option<KitView>>),
}

/// A row of a context menu.
#[derive(Clone)]
pub struct MenuEntry {
    /// Label.
    pub title: String,
    /// Key equivalent shown (`cmd+c` …).
    pub shortcut: Option<String>,
    /// What choosing it does (none: disabled).
    pub action: Option<Rc<dyn Fn()>>,
    /// A separator row.
    pub separator: bool,
}

impl MenuEntry {
    /// An item.
    pub fn item(title: &str, action: impl Fn() + 'static) -> Self {
        MenuEntry {
            title: title.to_string(),
            shortcut: None,
            action: Some(Rc::new(action)),
            separator: false,
        }
    }

    /// A separator.
    pub fn separator() -> Self {
        MenuEntry {
            title: String::new(),
            shortcut: None,
            action: None,
            separator: true,
        }
    }

    /// With a key equivalent shown.
    pub fn shortcut(mut self, s: &str) -> Self {
        self.shortcut = Some(s.to_string());
        self
    }
}

/// A window presented from a view (sheet or popover).
#[derive(Clone)]
pub(crate) struct Presentation {
    pub(crate) presented: bool,
    pub(crate) size: (f32, f32),
    pub(crate) content: Rc<dyn Fn() -> Element>,
    pub(crate) dismissed: Option<Rc<dyn Fn()>>,
}

/// The children and expandability of an outline's nodes.
#[derive(Clone)]
pub(crate) struct TreeFns {
    pub(crate) children: Rc<dyn Fn(Option<u64>) -> Vec<u64>>,
    pub(crate) expandable: Rc<dyn Fn(u64) -> bool>,
}

/// How a collection lays its items out.
#[derive(Clone)]
pub(crate) enum RowsLayout {
    /// A list of rows.
    List,
    /// A table with columns.
    Table(Rc<[vetro_kit::Column]>),
    /// An outline with columns over a tree.
    Outline(Rc<[vetro_kit::Column]>, TreeFns),
    /// A grid of tiles.
    Grid(f32, f32),
}

impl RowsLayout {
    /// Whether two layouts make the same kind of rows (so live rows can be rendered again in
    /// place).
    pub(crate) fn same(&self, o: &RowsLayout) -> bool {
        match (self, o) {
            (RowsLayout::List, RowsLayout::List) => true,
            (RowsLayout::Table(a), RowsLayout::Table(b)) => a == b,
            (RowsLayout::Outline(a, _), RowsLayout::Outline(b, _)) => a == b,
            (RowsLayout::Grid(w, h), RowsLayout::Grid(x, y)) => w == x && h == y,
            _ => false,
        }
    }
}

/// An alert presented from a view.
#[derive(Clone)]
pub struct AlertSpec {
    /// Title.
    pub title: String,
    /// Message under it.
    pub message: Option<String>,
    /// Buttons in visual order (the last is the default).
    pub buttons: Vec<String>,
    /// The default button is destructive.
    pub destructive: bool,
    /// A symbol as the alert's icon.
    pub icon: Option<String>,
    /// Gets the index of the button pressed, or -1 (Escape).
    pub response: Rc<dyn Fn(i64)>,
}

impl AlertSpec {
    /// An alert with a title and buttons; `response` gets the button's index (-1: Escape).
    pub fn new(title: &str, buttons: &[&str], response: impl Fn(i64) + 'static) -> AlertSpec {
        AlertSpec {
            title: title.to_string(),
            message: None,
            buttons: buttons.iter().map(|b| b.to_string()).collect(),
            destructive: false,
            icon: None,
            response: Rc::new(response),
        }
    }

    /// With a message.
    pub fn message(mut self, m: &str) -> Self {
        self.message = Some(m.to_string());
        self
    }

    /// The default button is destructive.
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    /// A symbol as the icon.
    pub fn icon(mut self, symbol: &str) -> Self {
        self.icon = Some(symbol.to_string());
        self
    }
}

/// An element's alert: presented, what it shows, what dismissing it does.
pub(crate) type AlertEffect = (bool, AlertSpec, Rc<dyn Fn()>);

/// The items of a virtualised collection: `cell(key, column)` builds an item's element.
#[derive(Clone)]
pub(crate) struct Rows {
    pub(crate) count: usize,
    pub(crate) height: f32,
    pub(crate) layout: RowsLayout,
    pub(crate) cell: Rc<dyn Fn(u64, u32) -> Element>,
}

/// Side effects of an element beyond its view.
#[derive(Clone, Default)]
pub(crate) struct Effects {
    pub(crate) menu: Option<Rc<Vec<MenuEntry>>>,
    pub(crate) rows: Option<Rows>,
    pub(crate) sheet: Option<Presentation>,
    pub(crate) popover: Option<Presentation>,
    pub(crate) alert: Option<AlertEffect>,
}

/// An element: a view to make, its properties, handlers and children.
#[derive(Clone)]
pub struct Element {
    pub(crate) what: What,
    pub(crate) props: Vec<(Prop, PValue)>,
    pub(crate) handlers: Vec<(Signal, Handler)>,
    pub(crate) children: Vec<Element>,
    pub(crate) key: Option<Key>,
    pub(crate) effects: Effects,
}

impl std::fmt::Debug for Element {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let what = match &self.what {
            What::Kind(k) => format!("{k:?}"),
            What::Component(_) => "Component".to_string(),
            What::Host(_) => "Host".to_string(),
            What::Custom(..) => "Custom".to_string(),
        };
        f.debug_struct("Element")
            .field("what", &what)
            .field("props", &self.props)
            .field("children", &self.children)
            .field("key", &self.key)
            .finish()
    }
}

impl Element {
    pub(crate) fn of(kind: Kind) -> Element {
        Element {
            what: What::Kind(kind),
            props: Vec::new(),
            handlers: Vec::new(),
            children: Vec::new(),
            key: None,
            effects: Effects::default(),
        }
    }

    /// Sets a property (the last value of a property wins).
    pub fn prop(mut self, prop: Prop, value: PValue) -> Self {
        self.props.retain(|(p, _)| *p != prop);
        self.props.push((prop, value));
        self
    }

    /// Handles a signal.
    pub fn on(mut self, signal: Signal, f: impl Fn(&Value) + 'static) -> Self {
        self.handlers.push((signal, Rc::new(f)));
        self
    }

    /// Runs `f` when the control is activated (buttons, menu rows).
    pub fn on_activate(self, f: impl Fn() + 'static) -> Self {
        self.on(Signal::ACTIVATED, move |_| f())
    }

    /// Runs `f` with the selected rows' keys (rows of a list or a table, nodes of an
    /// outline, tiles of a collection).
    pub fn on_select(self, f: impl Fn(Vec<u64>) + 'static) -> Self {
        self.on(Signal::SELECTION_CHANGED, move |v| {
            if let Value::List(l) = v {
                f(l.iter().filter_map(|s| s.parse().ok()).collect());
            }
        })
    }

    /// Selects rows by key.
    pub fn selection(self, keys: &[u64]) -> Self {
        let l: Vec<String> = keys.iter().map(u64::to_string).collect();
        self.prop(Prop::SELECTED_ROWS, PValue::Strings(l.into()))
    }

    /// Runs `f` with a row's key on a double click or Return.
    pub fn on_open(self, f: impl Fn(u64) + 'static) -> Self {
        self.on(Signal::DOUBLE_CLICKED, move |v| {
            if let Value::Int(i) = v {
                f(*i as u64);
            }
        })
    }

    /// Runs `f` with the sorted column and whether ascending (`None`: unsorted) when the
    /// user clicks a column title.
    pub fn on_sort(self, f: impl Fn(Option<(usize, bool)>) + 'static) -> Self {
        self.on(Signal::SORT_CHANGED, move |v| match v {
            Value::Int(i) if *i >= 0 => f(Some((*i as usize, true))),
            Value::Int(i) => f(Some(((-(i + 1)) as usize, false))),
            _ => f(None),
        })
    }

    /// Shows the sort indicator on `column` (ascending or not).
    pub fn sorted(self, sort: Option<(usize, bool)>) -> Self {
        let v = match sort {
            Some((c, true)) => PValue::Int(c as i64),
            Some((c, false)) => PValue::Int(-(c as i64) - 1),
            None => PValue::None,
        };
        self.prop(Prop::SORT_DESCRIPTOR, v)
    }

    /// Runs `f(row key, column, text)` when a cell of an editable column is edited.
    pub fn on_edit(self, f: impl Fn(u64, usize, String) + 'static) -> Self {
        self.on(Signal::CELL_EDITED, move |v| {
            if let Value::Cell { row, column, text } = v {
                f(*row, *column as usize, text.clone());
            }
        })
    }

    /// Runs `f(node, expanded)` when an outline node is expanded or collapsed.
    pub fn on_expand(self, f: impl Fn(u64, bool) + 'static) -> Self {
        self.on(Signal::EXPANDED_CHANGED, move |v| {
            if let Value::List(l) = v
                && let [n, e] = l.as_slice()
                && let (Ok(n), Ok(e)) = (n.parse::<u64>(), e.parse::<u8>())
            {
                f(n, e != 0);
            }
        })
    }

    /// Runs `f` with a tab's index when its × is clicked (tabs).
    pub fn on_close_tab(self, f: impl Fn(usize) + 'static) -> Self {
        self.on(Signal::CLOSE_REQUESTED, move |v| {
            if let Value::Int(i) = v
                && let Ok(i) = usize::try_from(*i)
            {
                f(i);
            }
        })
    }

    /// Runs `f` when the new-tab button is clicked (tabs).
    pub fn on_new_tab(self, f: impl Fn() + 'static) -> Self {
        self.on(Signal::ACTIVATED, move |_| f())
    }

    /// Alternating row backgrounds (tables).
    pub fn alternating(self, on: bool) -> Self {
        self.prop(Prop::ALTERNATING_ROWS, PValue::Bool(on))
    }

    /// Runs `f` with the new value when the user changes it.
    pub fn on_change(self, f: impl Fn(&Value) + 'static) -> Self {
        self.on(Signal::VALUE_CHANGED, f)
    }

    /// The key distinguishing this element among its siblings.
    pub fn key(mut self, key: impl Into<Key>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Insets around the content, on every side.
    pub fn padding(self, v: f32) -> Self {
        self.prop(Prop::PADDING, PValue::Insets([v, v, v, v]))
    }

    /// Horizontal and vertical insets.
    pub fn padding_xy(self, horizontal: f32, vertical: f32) -> Self {
        self.prop(
            Prop::PADDING,
            PValue::Insets([vertical, horizontal, vertical, horizontal]),
        )
    }

    /// Insets: top, left, bottom, right.
    pub fn padding_edges(self, top: f32, left: f32, bottom: f32, right: f32) -> Self {
        self.prop(Prop::PADDING, PValue::Insets([top, left, bottom, right]))
    }

    /// A fixed size.
    pub fn frame(self, width: f32, height: f32) -> Self {
        self.width(width).height(height)
    }

    /// A fixed width.
    pub fn width(self, v: f32) -> Self {
        self.prop(Prop::WIDTH, PValue::Float(f64::from(v)))
    }

    /// A fixed height.
    pub fn height(self, v: f32) -> Self {
        self.prop(Prop::HEIGHT, PValue::Float(f64::from(v)))
    }

    /// A minimum width.
    pub fn min_width(self, v: f32) -> Self {
        self.prop(Prop::MIN_WIDTH, PValue::Float(f64::from(v)))
    }

    /// A maximum width (`f32::INFINITY`: take the width offered).
    pub fn max_width(self, v: f32) -> Self {
        self.prop(Prop::MAX_WIDTH, PValue::Float(f64::from(v)))
    }

    /// A minimum height.
    pub fn min_height(self, v: f32) -> Self {
        self.prop(Prop::MIN_HEIGHT, PValue::Float(f64::from(v)))
    }

    /// A maximum height.
    pub fn max_height(self, v: f32) -> Self {
        self.prop(Prop::MAX_HEIGHT, PValue::Float(f64::from(v)))
    }

    /// Takes all the space offered.
    pub fn fill(self) -> Self {
        self.max_width(f32::INFINITY).max_height(f32::INFINITY)
    }

    /// Takes all the width offered.
    pub fn fill_width(self) -> Self {
        self.max_width(f32::INFINITY)
    }

    /// Alignment of the content in a larger frame (`vetro_kit` alignment numbers: 0
    /// top-leading … 8 bottom-trailing).
    pub fn align(self, alignment: i64) -> Self {
        self.prop(Prop::FRAME_ALIGNMENT, PValue::Int(alignment))
    }

    /// Layout priority in stacks.
    pub fn priority(self, v: f32) -> Self {
        self.prop(Prop::LAYOUT_PRIORITY, PValue::Float(f64::from(v)))
    }

    /// A background fill.
    pub fn background(self, c: Color) -> Self {
        self.prop(Prop::BACKGROUND, PValue::Color(c))
    }

    /// Corner radius of the background and the clip.
    pub fn corner_radius(self, r: f32) -> Self {
        self.prop(Prop::CORNER_RADIUS, PValue::Float(f64::from(r)))
    }

    /// Clips the children to the bounds.
    pub fn clipped(self) -> Self {
        self.prop(Prop::CLIPS_CHILDREN, PValue::Bool(true))
    }

    /// Opacity, 0–1.
    pub fn opacity(self, v: f32) -> Self {
        self.prop(Prop::OPACITY, PValue::Float(f64::from(v)))
    }

    /// Hidden (not laid out, not drawn).
    pub fn hidden(self, h: bool) -> Self {
        self.prop(Prop::HIDDEN, PValue::Bool(h))
    }

    /// Disabled.
    pub fn disabled(self, d: bool) -> Self {
        self.prop(Prop::DISABLED, PValue::Bool(d))
    }

    /// An identifier for tests and the inspector.
    pub fn id(self, ident: &str) -> Self {
        self.prop(Prop::IDENTIFIER, PValue::Str(Rc::from(ident)))
    }

    /// A tooltip.
    pub fn tooltip(self, text: &str) -> Self {
        self.prop(Prop::TOOLTIP, PValue::Str(Rc::from(text)))
    }

    /// The accessibility label.
    pub fn accessibility_label(self, text: &str) -> Self {
        self.prop(Prop::ACCESSIBILITY_LABEL, PValue::Str(Rc::from(text)))
    }

    /// Always dark (`Some(true)`), always light, or the system's (`None`).
    pub fn dark(self, dark: Option<bool>) -> Self {
        let v = match dark {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        };
        self.prop(Prop::APPEARANCE_OVERRIDE, PValue::Int(v))
    }

    /// Space between a stack's children.
    pub fn spacing(self, v: f32) -> Self {
        self.prop(Prop::SPACING, PValue::Float(f64::from(v)))
    }

    /// Cross-axis alignment of a stack's children (alignment numbers as [`Element::align`]).
    pub fn alignment(self, a: i64) -> Self {
        self.prop(Prop::ALIGNMENT, PValue::Int(a))
    }

    /// A component's style variant (push button: 0 primary … 4 glass).
    pub fn style(self, s: i64) -> Self {
        self.prop(Prop::STYLE, PValue::Int(s))
    }

    /// Control size: 0 mini, 1 small, 2 regular, 3 large.
    pub fn control_size(self, s: i64) -> Self {
        self.prop(Prop::CONTROL_SIZE, PValue::Int(s))
    }

    /// The type style token (`Title 1`, `Headline` …).
    pub fn text_style(self, token: &str) -> Self {
        self.prop(Prop::TEXT_STYLE, PValue::Str(Rc::from(token)))
    }

    /// Alignment of a label's lines: 0 leading, 1 centre, 2 trailing, 3 justified.
    pub fn text_align(self, a: i64) -> Self {
        self.prop(Prop::TEXT_ALIGNMENT, PValue::Int(a))
    }

    /// The font size of a label or the size of a symbol image.
    pub fn font_size(self, size: f32) -> Self {
        self.prop(Prop::FONT_SIZE, PValue::Float(f64::from(size)))
    }

    /// The text colour token (`Text/Secondary` …).
    pub fn color(self, token: &str) -> Self {
        self.prop(Prop::TEXT_COLOR, PValue::Str(Rc::from(token)))
    }

    /// A Silica Symbols name (buttons, symbol images).
    pub fn symbol(self, name: &str) -> Self {
        self.prop(Prop::SYMBOL, PValue::Str(Rc::from(name)))
    }

    /// The default button (Return activates it).
    pub fn default_action(self) -> Self {
        self.prop(Prop::IS_DEFAULT, PValue::Bool(true))
    }

    /// The cancel button (Escape activates it).
    pub fn cancel_action(self) -> Self {
        self.prop(Prop::IS_CANCEL, PValue::Bool(true))
    }

    /// Animates position changes of this view with `a`.
    pub fn animation(self, a: Animation) -> Self {
        self.prop(Prop::ANIMATION, PValue::Int(a as i64))
    }

    /// How this view enters and leaves.
    pub fn transition(self, t: Transition) -> Self {
        self.prop(Prop::TRANSITION, PValue::Int(t as i64))
    }

    /// A context menu (secondary click or the accessibility action).
    pub fn context_menu(mut self, entries: Vec<MenuEntry>) -> Self {
        self.effects.menu = Some(Rc::new(entries));
        self
    }

    /// Presents `content` in a sheet attached to this view's window while `presented` is
    /// true; when the user closes it, `presented` is set to false.
    pub fn sheet(
        mut self,
        presented: &crate::Binding<bool>,
        size: (f32, f32),
        content: impl Fn() -> Element + 'static,
    ) -> Self {
        let b = presented.clone();
        self.effects.sheet = Some(Presentation {
            presented: presented.get(),
            size,
            content: Rc::new(content),
            dismissed: Some(Rc::new(move || b.set(false))),
        });
        self
    }

    /// Presents `content` in a popover anchored to this view while `presented` is true;
    /// dismissing it (a click outside) sets `presented` to false.
    pub fn popover(
        mut self,
        presented: &crate::Binding<bool>,
        size: (f32, f32),
        content: impl Fn() -> Element + 'static,
    ) -> Self {
        let b = presented.clone();
        self.effects.popover = Some(Presentation {
            presented: presented.get(),
            size,
            content: Rc::new(content),
            dismissed: Some(Rc::new(move || b.set(false))),
        });
        self
    }

    /// Presents `alert` as a sheet of this view's window while `presented` is true; a
    /// button (or Escape) dismisses it, sets `presented` to false and calls its response.
    pub fn alert(mut self, presented: &crate::Binding<bool>, alert: AlertSpec) -> Self {
        let b = presented.clone();
        self.effects.alert = Some((presented.get(), alert, Rc::new(move || b.set(false))));
        self
    }

    /// Adds children.
    pub fn children(mut self, children: impl IntoChildren) -> Self {
        self.children.extend(children.into_children());
        self
    }
}

/// An implicit animation (the motion tokens; measured curves belong to components).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum Animation {
    /// None.
    None = 0,
    /// `spring.snappy`.
    Snappy = 1,
    /// `spring.panel`.
    Panel = 2,
    /// `spring.window`.
    Window = 3,
    /// `spring.morph`.
    Morph = 4,
    /// `spring.bouncy`.
    Bouncy = 5,
    /// `fade.instant`.
    FadeInstant = 6,
    /// `fade.quick`.
    FadeQuick = 7,
    /// `fade.standard`.
    FadeStandard = 8,
    /// `fade.slow`.
    FadeSlow = 9,
}

/// How a view enters and leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum Transition {
    /// At once.
    None = 0,
    /// Fades.
    Opacity = 1,
    /// Fades and scales.
    Scale = 2,
}

/// Things that become a list of children.
pub trait IntoChildren {
    /// The elements.
    fn into_children(self) -> Vec<Element>;
}

impl IntoChildren for Element {
    fn into_children(self) -> Vec<Element> {
        vec![self]
    }
}

impl IntoChildren for Vec<Element> {
    fn into_children(self) -> Vec<Element> {
        self
    }
}

impl<const N: usize> IntoChildren for [Element; N] {
    fn into_children(self) -> Vec<Element> {
        self.into_iter().collect()
    }
}

impl IntoChildren for Option<Element> {
    fn into_children(self) -> Vec<Element> {
        self.into_iter().collect()
    }
}

macro_rules! tuple_children {
    ($($name:ident),+) => {
        impl<$($name: IntoChildren),+> IntoChildren for ($($name,)+) {
            #[allow(non_snake_case)]
            fn into_children(self) -> Vec<Element> {
                let ($($name,)+) = self;
                let mut v = Vec::new();
                $(v.extend($name.into_children());)+
                v
            }
        }
    };
}

tuple_children!(A);
tuple_children!(A, B);
tuple_children!(A, B, C);
tuple_children!(A, B, C, D);
tuple_children!(A, B, C, D, E);
tuple_children!(A, B, C, D, E, F);
tuple_children!(A, B, C, D, E, F, G);
tuple_children!(A, B, C, D, E, F, G, H);
tuple_children!(A, B, C, D, E, F, G, H, I);
tuple_children!(A, B, C, D, E, F, G, H, I, J);
tuple_children!(A, B, C, D, E, F, G, H, I, J, K);
tuple_children!(A, B, C, D, E, F, G, H, I, J, K, L);

/// A column (vertical stack).
pub fn column(children: impl IntoChildren) -> Element {
    Element::of(Kind::COLUMN).children(children)
}

/// A row (horizontal stack).
pub fn row(children: impl IntoChildren) -> Element {
    Element::of(Kind::ROW).children(children)
}

/// Children on top of each other.
pub fn overlay(children: impl IntoChildren) -> Element {
    Element::of(Kind::OVERLAY).children(children)
}

/// A grid of `columns` equal columns.
pub fn grid(columns: i64, children: impl IntoChildren) -> Element {
    Element::of(Kind::GRID)
        .prop(Prop::COLUMNS, PValue::Int(columns))
        .children(children)
}

/// A flexible space.
pub fn spacer() -> Element {
    Element::of(Kind::SPACER)
}

/// An empty view (a container: its children are overlaid).
pub fn group(children: impl IntoChildren) -> Element {
    Element::of(Kind::VIEW).children(children)
}

/// Static text.
pub fn label(text: impl AsRef<str>) -> Element {
    Element::of(Kind::LABEL).prop(Prop::TEXT, PValue::Str(Rc::from(text.as_ref())))
}

/// A Silica Symbols glyph.
pub fn symbol(name: &str) -> Element {
    Element::of(Kind::SYMBOL).symbol(name)
}

/// A push button.
pub fn button(title: impl AsRef<str>) -> Element {
    Element::of(Kind::PUSH_BUTTON).prop(Prop::TITLE, PValue::Str(Rc::from(title.as_ref())))
}

/// A multi-line text view (documents of any size); its text is set once and then edited by
/// the user: read it back with [`Element::on`] `TEXT_CHANGED`.
pub fn text_view(initial: impl AsRef<str>) -> Element {
    Element::of(Kind::TEXT_VIEW).prop(Prop::TEXT, PValue::Str(Rc::from(initial.as_ref())))
}

/// A view of any kind (for components without a builder yet).
pub fn view(kind: Kind) -> Element {
    Element::of(kind)
}

/// A component.
pub fn component(c: impl Component) -> Element {
    Element {
        what: What::Component(Rc::new(c)),
        props: Vec::new(),
        handlers: Vec::new(),
        children: Vec::new(),
        key: None,
        effects: Effects::default(),
    }
}

/// An imperative view hosted in the declarative tree (the escape hatch).
pub fn host(view: KitView) -> Element {
    Element {
        what: What::Host(view),
        props: Vec::new(),
        handlers: Vec::new(),
        children: Vec::new(),
        key: None,
        effects: Effects::default(),
    }
}

/// A custom-drawn view: `make` builds the drawing object once per position (its state lives
/// in it); the element's properties and handlers apply as for any view.
pub fn custom<T: vetro_kit::Custom>(make: impl Fn() -> T + 'static) -> Element {
    Element {
        what: What::Custom(
            TypeId::of::<T>(),
            Rc::new(move || KitView::custom(make()).ok()),
        ),
        props: Vec::new(),
        handlers: Vec::new(),
        children: Vec::new(),
        key: None,
        effects: Effects::default(),
    }
}

/// A virtualised list of `count` rows of `row_height`: `row` builds the element of a row when
/// it scrolls into sight, and rows out of sight are dropped (a list of a million rows costs
/// what is shown). Rows are built again on every render of the component holding the list.
/// `Signal::SELECTION_CHANGED` carries the selected rows, `Signal::DOUBLE_CLICKED` a row.
pub fn list(count: usize, row_height: f32, row: impl Fn(usize) -> Element + 'static) -> Element {
    let mut e = Element::of(Kind::LIST);
    e.effects.rows = Some(Rows {
        count,
        height: row_height,
        layout: RowsLayout::List,
        cell: Rc::new(move |k, _| row(usize::try_from(k).unwrap_or(usize::MAX))),
    });
    e
}

/// Window tabs: the children are the tabs' contents, `titles` their titles; the kit's tab
/// bar shows while there are two tabs or more, and the tab picked goes to `selected`.
/// See [`Element::on_close_tab`] and [`Element::on_new_tab`].
pub fn tabs(
    titles: &[&str],
    selected: &crate::Binding<usize>,
    children: impl IntoChildren,
) -> Element {
    let b = selected.clone();
    let titles: Vec<String> = titles.iter().map(|t| t.to_string()).collect();
    Element::of(Kind::TAB_VIEW)
        .prop(Prop::ITEMS, PValue::Strings(titles.into()))
        .prop(Prop::SELECTED_INDEX, PValue::Int(selected.get() as i64))
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Value::Int(i) = v
                && let Ok(i) = usize::try_from(*i)
            {
                b.set(i);
            }
        })
        .children(children)
}

/// A virtualised table of `rows` rows under `columns`: `cell(row, column)` builds a cell's
/// element when its row scrolls into sight (cells fill their column: use
/// [`Element::align`] to place the content). Row height: the kit's 28.
/// `Signal::SORT_CHANGED` carries the sort (see [`Element::on_sort`]),
/// `Signal::CELL_EDITED` an edit of an editable column ([`Element::on_edit`]),
/// `Signal::SELECTION_CHANGED` the selected rows, `Signal::DOUBLE_CLICKED` a row.
pub fn table(
    columns: Vec<vetro_kit::Column>,
    rows: usize,
    cell: impl Fn(usize, usize) -> Element + 'static,
) -> Element {
    let mut e = Element::of(Kind::TABLE);
    e.effects.rows = Some(Rows {
        count: rows,
        height: 28.0,
        layout: RowsLayout::Table(columns.into()),
        cell: Rc::new(move |k, c| cell(usize::try_from(k).unwrap_or(usize::MAX), c as usize)),
    });
    e
}

/// A virtualised outline: a table whose rows are the visible nodes of a tree. `children`
/// lists a node's children (`None`: the roots), `expandable` says whether a node shows a
/// chevron, `cell(node, column)` builds a cell's element. Node ids are the app's; the
/// tree is read again on every render, keeping the expanded nodes and the selection.
/// Signals carry node ids; `Signal::EXPANDED_CHANGED` carries `[node, 1 | 0]`.
pub fn outline(
    columns: Vec<vetro_kit::Column>,
    children: impl Fn(Option<u64>) -> Vec<u64> + 'static,
    expandable: impl Fn(u64) -> bool + 'static,
    cell: impl Fn(u64, usize) -> Element + 'static,
) -> Element {
    let mut e = Element::of(Kind::OUTLINE);
    e.effects.rows = Some(Rows {
        count: 0,
        height: 28.0,
        layout: RowsLayout::Outline(
            columns.into(),
            TreeFns {
                children: Rc::new(children),
                expandable: Rc::new(expandable),
            },
        ),
        cell: Rc::new(move |k, c| cell(k, c as usize)),
    });
    e
}

/// A virtualised grid of `count` tiles (icon views) of `tile` points (the kit's tile is
/// 104 × 104): `item` builds a tile's element when it scrolls into sight; tiles reflow
/// with the width.
pub fn collection(
    count: usize,
    tile: (f32, f32),
    item: impl Fn(usize) -> Element + 'static,
) -> Element {
    let mut e = Element::of(Kind::GRID_VIEW);
    e.effects.rows = Some(Rows {
        count,
        height: tile.1,
        layout: RowsLayout::Grid(tile.0, tile.1),
        cell: Rc::new(move |k, _| item(usize::try_from(k).unwrap_or(usize::MAX))),
    });
    e
}

/// One element per item, keyed by `id` so that items keep their views (and state) when the
/// list changes.
pub fn for_each<T, K: Into<Key>>(
    items: impl IntoIterator<Item = T>,
    id: impl Fn(&T) -> K,
    build: impl Fn(&T) -> Element,
) -> Vec<Element> {
    items
        .into_iter()
        .map(|it| {
            let k = id(&it).into();
            build(&it).key(k)
        })
        .collect()
}

pub(crate) fn same_what(a: &What, b: &What) -> bool {
    match (a, b) {
        (What::Kind(x), What::Kind(y)) => x == y,
        (What::Component(x), What::Component(y)) => x.type_id_of() == y.type_id_of(),
        (What::Host(x), What::Host(y)) => x == y,
        (What::Custom(x, _), What::Custom(y, _)) => x == y,
        _ => false,
    }
}

#[allow(dead_code)]
fn _assert_any(_: &dyn Any) {}
