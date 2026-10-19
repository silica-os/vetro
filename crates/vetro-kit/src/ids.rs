//! Identifiers: properties, signals and view kinds (generated from vetro-sys by a script; the
//! values are vetro-sys's constants).

#![allow(missing_docs)]

use vetro_sys as sys;

/// A property id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Prop(pub u32);

impl Prop {
    pub const PADDING: Prop = Prop(sys::VETRO_PROP_PADDING);
    pub const WIDTH: Prop = Prop(sys::VETRO_PROP_WIDTH);
    pub const HEIGHT: Prop = Prop(sys::VETRO_PROP_HEIGHT);
    pub const MIN_WIDTH: Prop = Prop(sys::VETRO_PROP_MIN_WIDTH);
    pub const MAX_WIDTH: Prop = Prop(sys::VETRO_PROP_MAX_WIDTH);
    pub const MIN_HEIGHT: Prop = Prop(sys::VETRO_PROP_MIN_HEIGHT);
    pub const MAX_HEIGHT: Prop = Prop(sys::VETRO_PROP_MAX_HEIGHT);
    pub const FRAME_ALIGNMENT: Prop = Prop(sys::VETRO_PROP_FRAME_ALIGNMENT);
    pub const LAYOUT_PRIORITY: Prop = Prop(sys::VETRO_PROP_LAYOUT_PRIORITY);
    pub const HIDDEN: Prop = Prop(sys::VETRO_PROP_HIDDEN);
    pub const DISABLED: Prop = Prop(sys::VETRO_PROP_DISABLED);
    pub const OPACITY: Prop = Prop(sys::VETRO_PROP_OPACITY);
    pub const IDENTIFIER: Prop = Prop(sys::VETRO_PROP_IDENTIFIER);
    pub const TOOLTIP: Prop = Prop(sys::VETRO_PROP_TOOLTIP);
    pub const ACCESSIBILITY_LABEL: Prop = Prop(sys::VETRO_PROP_ACCESSIBILITY_LABEL);
    pub const ACCESSIBILITY_HELP: Prop = Prop(sys::VETRO_PROP_ACCESSIBILITY_HELP);
    pub const CLIPS_CHILDREN: Prop = Prop(sys::VETRO_PROP_CLIPS_CHILDREN);
    pub const APPEARANCE_OVERRIDE: Prop = Prop(sys::VETRO_PROP_APPEARANCE_OVERRIDE);
    pub const BACKGROUND: Prop = Prop(sys::VETRO_PROP_BACKGROUND);
    pub const CORNER_RADIUS: Prop = Prop(sys::VETRO_PROP_CORNER_RADIUS);
    pub const ASPECT_RATIO: Prop = Prop(sys::VETRO_PROP_ASPECT_RATIO);
    pub const OFFSET: Prop = Prop(sys::VETRO_PROP_OFFSET);
    pub const SCALE: Prop = Prop(sys::VETRO_PROP_SCALE);
    pub const TITLE: Prop = Prop(sys::VETRO_PROP_TITLE);
    pub const SUBTITLE: Prop = Prop(sys::VETRO_PROP_SUBTITLE);
    pub const SYMBOL: Prop = Prop(sys::VETRO_PROP_SYMBOL);
    pub const IMAGE: Prop = Prop(sys::VETRO_PROP_IMAGE);
    pub const PLACEHOLDER: Prop = Prop(sys::VETRO_PROP_PLACEHOLDER);
    pub const TEXT: Prop = Prop(sys::VETRO_PROP_TEXT);
    pub const VALUE: Prop = Prop(sys::VETRO_PROP_VALUE);
    pub const MIN_VALUE: Prop = Prop(sys::VETRO_PROP_MIN_VALUE);
    pub const MAX_VALUE: Prop = Prop(sys::VETRO_PROP_MAX_VALUE);
    pub const STEP: Prop = Prop(sys::VETRO_PROP_STEP);
    pub const STATE: Prop = Prop(sys::VETRO_PROP_STATE);
    pub const SELECTED_INDEX: Prop = Prop(sys::VETRO_PROP_SELECTED_INDEX);
    pub const ITEMS: Prop = Prop(sys::VETRO_PROP_ITEMS);
    pub const ITEM_SYMBOLS: Prop = Prop(sys::VETRO_PROP_ITEM_SYMBOLS);
    pub const CONTROL_SIZE: Prop = Prop(sys::VETRO_PROP_CONTROL_SIZE);
    pub const STYLE: Prop = Prop(sys::VETRO_PROP_STYLE);
    pub const IS_DEFAULT: Prop = Prop(sys::VETRO_PROP_IS_DEFAULT);
    pub const IS_CANCEL: Prop = Prop(sys::VETRO_PROP_IS_CANCEL);
    pub const SHORTCUT: Prop = Prop(sys::VETRO_PROP_SHORTCUT);
    pub const EDITABLE: Prop = Prop(sys::VETRO_PROP_EDITABLE);
    pub const SELECTABLE: Prop = Prop(sys::VETRO_PROP_SELECTABLE);
    pub const SECURE: Prop = Prop(sys::VETRO_PROP_SECURE);
    pub const MAX_LINES: Prop = Prop(sys::VETRO_PROP_MAX_LINES);
    pub const TEXT_ALIGNMENT: Prop = Prop(sys::VETRO_PROP_TEXT_ALIGNMENT);
    pub const TEXT_STYLE: Prop = Prop(sys::VETRO_PROP_TEXT_STYLE);
    pub const TEXT_COLOR: Prop = Prop(sys::VETRO_PROP_TEXT_COLOR);
    pub const SELECTED_RANGE: Prop = Prop(sys::VETRO_PROP_SELECTED_RANGE);
    pub const TICK_MARKS: Prop = Prop(sys::VETRO_PROP_TICK_MARKS);
    pub const VERTICAL: Prop = Prop(sys::VETRO_PROP_VERTICAL);
    pub const INDETERMINATE: Prop = Prop(sys::VETRO_PROP_INDETERMINATE);
    pub const EXPANDED: Prop = Prop(sys::VETRO_PROP_EXPANDED);
    pub const SPACING: Prop = Prop(sys::VETRO_PROP_SPACING);
    pub const ALIGNMENT: Prop = Prop(sys::VETRO_PROP_ALIGNMENT);
    pub const COLUMNS: Prop = Prop(sys::VETRO_PROP_COLUMNS);
    pub const MATERIAL: Prop = Prop(sys::VETRO_PROP_MATERIAL);
    pub const BADGE: Prop = Prop(sys::VETRO_PROP_BADGE);
    pub const COLOR_VALUE: Prop = Prop(sys::VETRO_PROP_COLOR_VALUE);
    pub const DATE_VALUE: Prop = Prop(sys::VETRO_PROP_DATE_VALUE);
    pub const SELECTED_ROWS: Prop = Prop(sys::VETRO_PROP_SELECTED_ROWS);
    pub const ROW_COUNT: Prop = Prop(sys::VETRO_PROP_ROW_COUNT);
    pub const COLUMN_TITLES: Prop = Prop(sys::VETRO_PROP_COLUMN_TITLES);
    pub const TOKENS: Prop = Prop(sys::VETRO_PROP_TOKENS);
    pub const SCROLL_OFFSET: Prop = Prop(sys::VETRO_PROP_SCROLL_OFFSET);
    pub const CONTENT_SIZE: Prop = Prop(sys::VETRO_PROP_CONTENT_SIZE);
    pub const SPLIT_POSITION: Prop = Prop(sys::VETRO_PROP_SPLIT_POSITION);
    pub const COLLAPSED: Prop = Prop(sys::VETRO_PROP_COLLAPSED);
    pub const SHOWS_CLEAR: Prop = Prop(sys::VETRO_PROP_SHOWS_CLEAR);
    pub const ICON_POSITION: Prop = Prop(sys::VETRO_PROP_ICON_POSITION);
    pub const HELP_ANCHOR: Prop = Prop(sys::VETRO_PROP_HELP_ANCHOR);
    pub const PULLS_DOWN: Prop = Prop(sys::VETRO_PROP_PULLS_DOWN);
    pub const ALERT_KIND: Prop = Prop(sys::VETRO_PROP_ALERT_KIND);
    pub const BUTTONS: Prop = Prop(sys::VETRO_PROP_BUTTONS);
    pub const EDGE: Prop = Prop(sys::VETRO_PROP_EDGE);
    pub const BORDERED: Prop = Prop(sys::VETRO_PROP_BORDERED);
    pub const FORMAT: Prop = Prop(sys::VETRO_PROP_FORMAT);
    pub const TINT: Prop = Prop(sys::VETRO_PROP_TINT);
    pub const WRAPS: Prop = Prop(sys::VETRO_PROP_WRAPS);
    pub const FONT_FAMILY: Prop = Prop(sys::VETRO_PROP_FONT_FAMILY);
    pub const FONT_SIZE: Prop = Prop(sys::VETRO_PROP_FONT_SIZE);
    pub const FONT_WEIGHT: Prop = Prop(sys::VETRO_PROP_FONT_WEIGHT);
}

/// A signal id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Signal(pub u32);

impl Signal {
    pub const ACTIVATED: Signal = Signal(sys::VETRO_SIGNAL_ACTIVATED);
    pub const VALUE_CHANGED: Signal = Signal(sys::VETRO_SIGNAL_VALUE_CHANGED);
    pub const TEXT_CHANGED: Signal = Signal(sys::VETRO_SIGNAL_TEXT_CHANGED);
    pub const SUBMITTED: Signal = Signal(sys::VETRO_SIGNAL_SUBMITTED);
    pub const SELECTION_CHANGED: Signal = Signal(sys::VETRO_SIGNAL_SELECTION_CHANGED);
    pub const EDITING_BEGAN: Signal = Signal(sys::VETRO_SIGNAL_EDITING_BEGAN);
    pub const EDITING_ENDED: Signal = Signal(sys::VETRO_SIGNAL_EDITING_ENDED);
    pub const CLOSE_REQUESTED: Signal = Signal(sys::VETRO_SIGNAL_CLOSE_REQUESTED);
    pub const CLOSED: Signal = Signal(sys::VETRO_SIGNAL_CLOSED);
    pub const POINTER_ENTERED: Signal = Signal(sys::VETRO_SIGNAL_POINTER_ENTERED);
    pub const POINTER_EXITED: Signal = Signal(sys::VETRO_SIGNAL_POINTER_EXITED);
    pub const EXPANDED_CHANGED: Signal = Signal(sys::VETRO_SIGNAL_EXPANDED_CHANGED);
    pub const DOUBLE_CLICKED: Signal = Signal(sys::VETRO_SIGNAL_DOUBLE_CLICKED);
    pub const CONTEXT_MENU: Signal = Signal(sys::VETRO_SIGNAL_CONTEXT_MENU);
    pub const DROPPED: Signal = Signal(sys::VETRO_SIGNAL_DROPPED);
    pub const LINK_ACTIVATED: Signal = Signal(sys::VETRO_SIGNAL_LINK_ACTIVATED);
    pub const SCROLLED: Signal = Signal(sys::VETRO_SIGNAL_SCROLLED);
    pub const RESIZED: Signal = Signal(sys::VETRO_SIGNAL_RESIZED);
    pub const FOCUS_IN: Signal = Signal(sys::VETRO_SIGNAL_FOCUS_IN);
    pub const FOCUS_OUT: Signal = Signal(sys::VETRO_SIGNAL_FOCUS_OUT);
    pub const KEY_UNHANDLED: Signal = Signal(sys::VETRO_SIGNAL_KEY_UNHANDLED);
    pub const DRAG_BEGAN: Signal = Signal(sys::VETRO_SIGNAL_DRAG_BEGAN);
    pub const DRAW: Signal = Signal(sys::VETRO_SIGNAL_DRAW);
}

/// A view kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Kind(pub u32);

impl Kind {
    pub const VIEW: Kind = Kind(sys::VETRO_KIND_VIEW);
    pub const ROW: Kind = Kind(sys::VETRO_KIND_ROW);
    pub const COLUMN: Kind = Kind(sys::VETRO_KIND_COLUMN);
    pub const OVERLAY: Kind = Kind(sys::VETRO_KIND_OVERLAY);
    pub const GRID: Kind = Kind(sys::VETRO_KIND_GRID);
    pub const SPACER: Kind = Kind(sys::VETRO_KIND_SPACER);
    pub const LABEL: Kind = Kind(sys::VETRO_KIND_LABEL);
    pub const SYMBOL: Kind = Kind(sys::VETRO_KIND_SYMBOL);
    pub const PUSH_BUTTON: Kind = Kind(sys::VETRO_KIND_PUSH_BUTTON);
    pub const TEXT_VIEW: Kind = Kind(sys::VETRO_KIND_TEXT_VIEW);
}
