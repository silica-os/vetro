//! Values, property and signal identifiers: the vocabulary shared by the Rust API and the C
//! ABI. Identifiers are stable numbers: new ones are only ever appended, so the ABI stays
//! compatible across minor versions.

use crate::color::Color;
use crate::geom::{Insets, Point, Rect, Size};

/// A dynamically typed value: a property's value or a signal's payload.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    /// No value.
    #[default]
    None,
    /// A boolean.
    Bool(bool),
    /// An integer.
    Int(i64),
    /// A number.
    Float(f64),
    /// UTF-8 text.
    Str(String),
    /// A colour.
    Color(Color),
    /// A point.
    Point(Point),
    /// A size.
    Size(Size),
    /// A rectangle.
    Rect(Rect),
    /// Insets.
    Insets(Insets),
    /// A list of strings (segment titles, menu items, tokens …).
    StrList(Vec<String>),
    /// A list of integers (selected rows …).
    IntList(Vec<i64>),
    /// A byte range in a text.
    Range(usize, usize),
}

impl Value {
    /// The boolean, accepting integers.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            Value::Int(i) => Some(*i != 0),
            _ => None,
        }
    }

    /// The integer, accepting whole floats and booleans.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Bool(b) => Some(i64::from(*b)),
            Value::Float(f) if f.is_finite() && f.fract() == 0.0 => Some(*f as i64),
            _ => None,
        }
    }

    /// The number, accepting integers.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(f) if !f.is_nan() => Some(*f),
            Value::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// The number as `f32` (infinities allowed: a maximum length).
    pub fn as_f32(&self) -> Option<f32> {
        self.as_f64().map(|f| f as f32)
    }

    /// The text.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The colour.
    pub fn as_color(&self) -> Option<Color> {
        match self {
            Value::Color(c) => Some(*c),
            Value::Int(i) => u32::try_from(*i).ok().map(Color::from_u32),
            _ => None,
        }
    }

    /// The string list.
    pub fn as_str_list(&self) -> Option<&[String]> {
        match self {
            Value::StrList(l) => Some(l),
            _ => None,
        }
    }

    /// The size.
    pub fn as_size(&self) -> Option<Size> {
        match self {
            Value::Size(s) => Some(*s),
            _ => None,
        }
    }

    /// The insets, accepting one number for every edge.
    pub fn as_insets(&self) -> Option<Insets> {
        match self {
            Value::Insets(i) => Some(*i),
            other => other.as_f32().map(Insets::all),
        }
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}
impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}
impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Int(i64::from(v))
    }
}
impl From<usize> for Value {
    fn from(v: usize) -> Self {
        Value::Int(i64::try_from(v).unwrap_or(i64::MAX))
    }
}
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}
impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Value::Float(f64::from(v))
    }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Str(v.to_string())
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Str(v)
    }
}
impl From<Color> for Value {
    fn from(v: Color) -> Self {
        Value::Color(v)
    }
}
impl From<Size> for Value {
    fn from(v: Size) -> Self {
        Value::Size(v)
    }
}
impl From<Insets> for Value {
    fn from(v: Insets) -> Self {
        Value::Insets(v)
    }
}
impl From<Vec<String>> for Value {
    fn from(v: Vec<String>) -> Self {
        Value::StrList(v)
    }
}

macro_rules! numbered_enum {
    ($(#[$meta:meta])* $vis:vis enum $name:ident { $($(#[$vm:meta])* $variant:ident = $num:expr,)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u32)]
        $vis enum $name {
            $($(#[$vm])* $variant = $num,)*
        }

        impl $name {
            /// Every identifier.
            pub const ALL: &'static [$name] = &[$($name::$variant,)*];

            /// The identifier with ABI number `n`.
            pub fn from_u32(n: u32) -> Option<$name> {
                match n {
                    $($num => Some($name::$variant),)*
                    _ => None,
                }
            }

            /// The identifier's name.
            pub fn name(self) -> &'static str {
                match self {
                    $($name::$variant => stringify!($variant),)*
                }
            }
        }
    };
}

numbered_enum! {
    /// A property of a view. Generic properties (layout, appearance, accessibility) apply to
    /// every view; the others to the components that have them.
    pub enum Prop {
        // Generic: layout modifiers (core/layout.rs).
        /// Insets around the content (`Value::Insets` or a number).
        Padding = 1,
        /// Fixed width (`Float`; `None` clears).
        Width = 2,
        /// Fixed height.
        Height = 3,
        /// Minimum width.
        MinWidth = 4,
        /// Maximum width (infinite: fills).
        MaxWidth = 5,
        /// Minimum height.
        MinHeight = 6,
        /// Maximum height (infinite: fills).
        MaxHeight = 7,
        /// Alignment of the content in a larger frame (`Int`, [`crate::core::layout::Alignment`]).
        FrameAlignment = 8,
        /// Layout priority (`Float`): higher gets space first in stacks.
        LayoutPriority = 9,
        /// Hidden (`Bool`): not drawn, not laid out, not hit.
        Hidden = 10,
        /// Disabled (`Bool`): drawn at the disabled opacity, no interaction.
        Disabled = 11,
        /// Opacity, 0–1.
        Opacity = 12,
        /// Identifier for tests and the inspector (`Str`).
        Identifier = 13,
        /// Tooltip text.
        Tooltip = 14,
        /// Accessibility label.
        AccessibilityLabel = 15,
        /// Accessibility help.
        AccessibilityHelp = 16,
        /// Clip children to the bounds (`Bool`).
        ClipsChildren = 17,
        /// Appearance override for the subtree (`Int`: 0 inherit, 1 light, 2 dark).
        AppearanceOverride = 18,
        /// Background fill of the view (`Color`).
        Background = 19,
        /// Corner radius of the background and the clip (`Float`).
        CornerRadius = 20,
        /// Fixed aspect ratio width / height (`Float`).
        AspectRatio = 21,
        /// Position offset after layout (`Point`).
        Offset = 22,
        /// Scale after layout (`Float`).
        Scale = 23,

        // Component content.
        /// Title or label text.
        Title = 100,
        /// Secondary text (subtitle, informative text).
        Subtitle = 101,
        /// Symbol name of an icon (`Str`, Silica Symbols).
        Symbol = 102,
        /// Image path or resource.
        Image = 103,
        /// Placeholder text of a field.
        Placeholder = 104,
        /// Text value of a field or text view.
        Text = 105,
        /// Numeric value (slider, stepper, progress …).
        Value = 106,
        /// Minimum of the value.
        MinValue = 107,
        /// Maximum of the value.
        MaxValue = 108,
        /// Step of the value.
        Step = 109,
        /// State of a toggle (`Int`: 0 off, 1 on, 2 mixed) or a selection.
        State = 110,
        /// Selected index.
        SelectedIndex = 111,
        /// Item titles (segments, pop-up items, tabs).
        Items = 112,
        /// Item symbols.
        ItemSymbols = 113,
        /// Control size (`Int`: 0 mini, 1 small, 2 regular, 3 large).
        ControlSize = 114,
        /// Style variant of a component (`Int`, per component).
        Style = 115,
        /// Default button (Return activates it).
        IsDefault = 116,
        /// Cancel button (Escape activates it).
        IsCancel = 117,
        /// Keyboard shortcut (`Str`, e.g. `cmd+s`).
        Shortcut = 118,
        /// Editable (`Bool`).
        Editable = 119,
        /// Selectable text (`Bool`).
        Selectable = 120,
        /// Secure entry (`Bool`).
        Secure = 121,
        /// Maximum number of lines (`Int`, 0 unlimited).
        MaxLines = 122,
        /// Text alignment (`Int`: 0 start, 1 centre, 2 end, 3 justify).
        TextAlignment = 123,
        /// Type style token name (`Str`, e.g. `Headline`).
        TextStyle = 124,
        /// Text colour role or colour (`Color`, or `Str` token name).
        TextColor = 125,
        /// Selected range in a text (`Range`).
        SelectedRange = 126,
        /// Tick marks of a slider (`Int`).
        TickMarks = 127,
        /// Vertical orientation (`Bool`).
        Vertical = 128,
        /// Indeterminate progress (`Bool`).
        Indeterminate = 129,
        /// Expanded (disclosure) (`Bool`).
        Expanded = 130,
        /// Spacing of a stack or grid (`Float`).
        Spacing = 131,
        /// Alignment of a stack's children (`Int`).
        Alignment = 132,
        /// Number of columns of a grid (`Int`).
        Columns = 133,
        /// Material of a glass view (`Str`: `Glass/Regular` …).
        Material = 134,
        /// Badge text or count.
        Badge = 135,
        /// Colour value of a colour well.
        ColorValue = 136,
        /// Date and time value, seconds since the epoch (`Float`).
        DateValue = 137,
        /// Selected rows (`IntList`).
        SelectedRows = 138,
        /// Row count of a list or table (`Int`).
        RowCount = 139,
        /// Column titles of a table.
        ColumnTitles = 140,
        /// Tokens of a token field.
        Tokens = 141,
        /// Scroll offset (`Point`).
        ScrollOffset = 142,
        /// Content size of a scroll view (`Size`).
        ContentSize = 143,
        /// Sidebar width or split position (`Float`).
        SplitPosition = 144,
        /// Collapsed (`Bool`).
        Collapsed = 145,
        /// Show a clear button in a search field (`Bool`).
        ShowsClear = 146,
        /// Icon position of a button (`Int`: 0 none, 1 leading, 2 trailing, 3 only, 4 above).
        IconPosition = 147,
        /// Help anchor.
        HelpAnchor = 148,
        /// Pull-down (`Bool`) for a pop-up button.
        PullsDown = 149,
        /// Kind of an alert (`Int`: 0 informational, 1 warning, 2 critical).
        AlertKind = 150,
        /// Buttons of an alert or sheet.
        Buttons = 151,
        /// Edge of a popover's arrow or a sheet (`Int`).
        Edge = 152,
        /// Bordered (`Bool`).
        Bordered = 153,
        /// Number formatter pattern (`Str`).
        Format = 154,
        /// Accent role override of a control (`Color`).
        Tint = 155,
        /// Line wrapping (`Bool`).
        Wraps = 156,
        /// Font family override (`Str`).
        FontFamily = 157,
        /// Font size override (`Float`).
        FontSize = 158,
        /// Font weight override (`Int`).
        FontWeight = 159,
        /// Implicit animation of position changes (`Int`: 0 none, 1–5 springs, 6–9 fades).
        Animation = 160,
        /// Insertion and removal transition (`Int`: 0 none, 1 opacity, 2 scale).
        Transition = 161,
    }
}

numbered_enum! {
    /// Something a view tells its observers.
    pub enum Signal {
        /// A button, menu item or row was activated (clicked, Return, Space).
        Activated = 1,
        /// The value changed through the user (`Value` payload: the new value).
        ValueChanged = 2,
        /// The text of a field changed.
        TextChanged = 3,
        /// Return in a field.
        Submitted = 4,
        /// The selection changed.
        SelectionChanged = 5,
        /// Editing began.
        EditingBegan = 6,
        /// Editing ended.
        EditingEnded = 7,
        /// A window or panel was asked to close.
        CloseRequested = 8,
        /// A window or panel closed.
        Closed = 9,
        /// The pointer entered.
        PointerEntered = 10,
        /// The pointer left.
        PointerExited = 11,
        /// Expanded or collapsed.
        ExpandedChanged = 12,
        /// A row was double-clicked.
        DoubleClicked = 13,
        /// A context menu is requested (payload: the point).
        ContextMenu = 14,
        /// Files were dropped (payload: `StrList` of URIs).
        Dropped = 15,
        /// A link in a text was activated (payload: the URL).
        LinkActivated = 16,
        /// Scrolled (payload: the offset).
        Scrolled = 17,
        /// The view was resized (payload: the size).
        Resized = 18,
        /// Focus came in.
        FocusIn = 19,
        /// Focus went out.
        FocusOut = 20,
        /// A key went unhandled through the responder chain (payload: key name).
        KeyUnhandled = 21,
        /// A drag began (payload: point).
        DragBegan = 22,
        /// Custom drawing requested (custom views).
        Draw = 23,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for p in Prop::ALL {
            assert_eq!(Prop::from_u32(*p as u32), Some(*p));
        }
        for s in Signal::ALL {
            assert_eq!(Signal::from_u32(*s as u32), Some(*s));
        }
        assert_eq!(Prop::from_u32(0), None);
        assert_eq!(Prop::Title.name(), "Title");
    }

    #[test]
    fn conversions() {
        assert_eq!(Value::Int(3).as_f32(), Some(3.0));
        assert_eq!(Value::Float(2.0).as_int(), Some(2));
        assert_eq!(Value::Float(2.5).as_int(), None);
        assert_eq!(Value::Float(f64::NAN).as_f64(), None);
        assert_eq!(Value::Float(f64::INFINITY).as_f32(), Some(f32::INFINITY));
        assert_eq!(Value::Float(4.0).as_insets(), Some(Insets::all(4.0)));
    }
}
