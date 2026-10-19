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

/// An element: a view to make, its properties, handlers and children.
#[derive(Clone)]
pub struct Element {
    pub(crate) what: What,
    pub(crate) props: Vec<(Prop, PValue)>,
    pub(crate) handlers: Vec<(Signal, Handler)>,
    pub(crate) children: Vec<Element>,
    pub(crate) key: Option<Key>,
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

    /// Adds children.
    pub fn children(mut self, children: impl IntoChildren) -> Self {
        self.children.extend(children.into_children());
        self
    }
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
    }
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
