//! The C ABI of Vetro (`libvetro.so`, header `vetro.h`, symbol version `VETRO_1.0`).
//!
//! One list, [`for_each_function!`], declares every function: this crate expands it into
//! `extern "C"` declarations for Rust users (vetro-kit), and libvetro expands it into a
//! compile-time check that its definitions have exactly these signatures. Types are
//! `#[repr(C)]` and shared by both. Identifiers (properties, signals, view kinds) are stable
//! numbers: new ones are only appended.
//!
//! Rules of the ABI:
//! - every handle is opaque; views are reference counted (`vetro_view_retain` / `release`),
//!   a view added to a parent is also held by the parent;
//! - strings are UTF-8, NUL-terminated on input; output strings are written into a caller
//!   buffer, or borrowed for the duration of a callback;
//! - functions return a status (`VETRO_OK` or a negative error), or a null handle on error
//!   with the status in `vetro_last_status()`;
//! - callbacks run on the UI thread, take a `void *user`, and may call back into the ABI;
//!   a `destroy` callback, when given, is called once when the callback is dropped;
//! - nothing panics across the boundary.

#![allow(non_camel_case_types)]
// The declarations of the ABI are an `extern` block: the only unsafe of this crate.
#![allow(unsafe_code)]

use std::ffi::{c_char, c_void};

/// Defines `pub const` statuses and a name table.
macro_rules! ids_i32 {
    ($table:ident, $($name:ident = $value:expr,)*) => {
        $(
            #[allow(missing_docs)]
            pub const $name: VetroStatus = $value;
        )*
        /// Every identifier of the group: (name, value).
        pub static $table: &[(&str, i32)] = &[$((stringify!($name), $value),)*];
    };
}

/// Defines `pub const` identifiers and a name table.
macro_rules! ids {
    ($table:ident, $($name:ident = $value:expr,)*) => {
        $(
            #[allow(missing_docs)]
            pub const $name: u32 = $value;
        )*
        /// Every identifier of the group: (name, value).
        pub static $table: &[(&str, u32)] = &[$((stringify!($name), $value),)*];
    };
}

/// A view (opaque).
#[repr(C)]
pub struct VetroView {
    _private: [u8; 0],
}

/// A window (opaque).
#[repr(C)]
pub struct VetroWindow {
    _private: [u8; 0],
}

/// A drawing surface handed to a custom view's draw callback (opaque).
#[repr(C)]
pub struct VetroCanvas {
    _private: [u8; 0],
}

/// A menu being built (opaque).
#[repr(C)]
pub struct VetroMenu {
    _private: [u8; 0],
}

/// A status: `VETRO_OK` or a negative error.
pub type VetroStatus = i32;

// Statuses.
ids_i32! { STATUSES,
    VETRO_OK = 0, VETRO_ERROR_NO_SUCH_OBJECT = -1, VETRO_ERROR_UNKNOWN_PROPERTY = -2,
    VETRO_ERROR_BAD_VALUE = -3, VETRO_ERROR_BAD_HIERARCHY = -4, VETRO_ERROR_WRONG_THREAD = -5,
    VETRO_ERROR_INVALID_ARGUMENT = -6, VETRO_ERROR_PLATFORM = -7, VETRO_ERROR_NO_RUNTIME = -8,
    VETRO_ERROR_BUFFER_TOO_SMALL = -9,
}

/// The ABI's version: major << 16 | minor << 8 | patch.
pub const VETRO_ABI_VERSION: u32 = 1 << 16;

/// A colour, straight alpha.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VetroColor {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha.
    pub a: u8,
}

/// A rectangle in points.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VetroRect {
    /// Left.
    pub x: f32,
    /// Top.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// Insets in points.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VetroInsets {
    /// Top.
    pub top: f32,
    /// Left.
    pub left: f32,
    /// Bottom.
    pub bottom: f32,
    /// Right.
    pub right: f32,
}

/// The kind of a [`VetroValue`].
pub type VetroValueKind = u32;
ids! { VALUE_KINDS,
    VETRO_VALUE_NONE = 0, VETRO_VALUE_BOOL = 1, VETRO_VALUE_INT = 2, VETRO_VALUE_FLOAT = 3,
    VETRO_VALUE_STRING = 4, VETRO_VALUE_COLOR = 5, VETRO_VALUE_POINT = 6, VETRO_VALUE_SIZE = 7,
    VETRO_VALUE_RECT = 8, VETRO_VALUE_RANGE = 9, VETRO_VALUE_STRING_LIST = 10,
}

/// A value handed to a signal callback (borrowed for the callback's duration).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VetroValue {
    /// Which fields are meaningful.
    pub kind: VetroValueKind,
    /// Boolean.
    pub b: u8,
    /// Integer.
    pub i: i64,
    /// Number.
    pub f: f64,
    /// UTF-8 text (not NUL-terminated; `text_len` bytes).
    pub text: *const c_char,
    /// Length of `text`.
    pub text_len: usize,
    /// Point, size or rectangle.
    pub rect: VetroRect,
    /// Colour.
    pub color: VetroColor,
}

/// An input event handed to a custom view.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VetroEvent {
    /// `VETRO_EVENT_*`.
    pub kind: u32,
    /// Position in the view (pointer events).
    pub x: f32,
    /// Position in the view (pointer events).
    pub y: f32,
    /// Scroll delta.
    pub dx: f32,
    /// Scroll delta.
    pub dy: f32,
    /// Button (1 left, 2 right, 3 middle).
    pub button: u32,
    /// Click count.
    pub clicks: u32,
    /// Modifiers (`VETRO_MOD_*`).
    pub mods: u32,
    /// Key: a character's code point, or `VETRO_KEY_*`.
    pub key: u32,
    /// Pressed (keys).
    pub pressed: u8,
    /// Text typed or committed (UTF-8, `text_len` bytes).
    pub text: *const c_char,
    /// Length of `text`.
    pub text_len: usize,
}

// Events, modifiers and keys (keys above the Unicode range; characters are code points).
ids! { EVENT_IDS,
    VETRO_EVENT_POINTER_DOWN = 1, VETRO_EVENT_POINTER_UP = 2, VETRO_EVENT_POINTER_MOVE = 3,
    VETRO_EVENT_POINTER_ENTER = 4, VETRO_EVENT_POINTER_LEAVE = 5, VETRO_EVENT_SCROLL = 6,
    VETRO_EVENT_KEY = 7, VETRO_EVENT_TEXT = 8, VETRO_EVENT_FOCUS_IN = 9, VETRO_EVENT_FOCUS_OUT = 10,
    VETRO_EVENT_ACTIVATE = 11,
    VETRO_MOD_SHIFT = 1, VETRO_MOD_CTRL = 2, VETRO_MOD_ALT = 4, VETRO_MOD_LOGO = 8,
    VETRO_KEY_SPACE = 0x20, VETRO_KEY_ENTER = 0x0011_0001, VETRO_KEY_TAB = 0x0011_0002,
    VETRO_KEY_ESCAPE = 0x0011_0003, VETRO_KEY_BACKSPACE = 0x0011_0004, VETRO_KEY_DELETE = 0x0011_0005,
    VETRO_KEY_LEFT = 0x0011_0006, VETRO_KEY_RIGHT = 0x0011_0007, VETRO_KEY_UP = 0x0011_0008,
    VETRO_KEY_DOWN = 0x0011_0009, VETRO_KEY_HOME = 0x0011_000A, VETRO_KEY_END = 0x0011_000B,
    VETRO_KEY_PAGE_UP = 0x0011_000C, VETRO_KEY_PAGE_DOWN = 0x0011_000D,
}

/// A plain callback.
pub type VetroCallback = Option<unsafe extern "C" fn(user: *mut c_void)>;
/// Called once when a callback's `user` data is dropped.
pub type VetroDestroy = Option<unsafe extern "C" fn(user: *mut c_void)>;
/// A signal callback.
pub type VetroSignalFn = Option<
    unsafe extern "C" fn(
        view: *mut VetroView,
        signal: u32,
        value: *const VetroValue,
        user: *mut c_void,
    ),
>;
/// A custom view's drawing, in its own coordinates.
pub type VetroDrawFn = Option<
    unsafe extern "C" fn(canvas: *mut VetroCanvas, width: f32, height: f32, user: *mut c_void),
>;
/// A custom view's size for a proposal (NaN: unspecified, infinity: maximum).
pub type VetroMeasureFn = Option<
    unsafe extern "C" fn(
        proposed_width: f32,
        proposed_height: f32,
        out_width: *mut f32,
        out_height: *mut f32,
        user: *mut c_void,
    ),
>;
/// A custom view's event handler; returns 1 when it handled the event.
pub type VetroEventFn = Option<
    unsafe extern "C" fn(view: *mut VetroView, event: *const VetroEvent, user: *mut c_void) -> u8,
>;
/// Receives bytes (a PNG …), borrowed for the call.
pub type VetroBytesFn =
    Option<unsafe extern "C" fn(data: *const u8, len: usize, user: *mut c_void)>;
/// A menu item was chosen (id), or a pop-up menu ended (0: dismissed).
pub type VetroMenuFn = Option<unsafe extern "C" fn(id: u32, user: *mut c_void)>;
/// Makes the view of row `index` of a list (a new reference the list takes over), or null.
pub type VetroRowFn = Option<unsafe extern "C" fn(index: u64, user: *mut c_void) -> *mut VetroView>;

// Properties: the numbers of libvetro's `core::value::Prop` (checked by its tests).
ids! { PROPERTIES,
    VETRO_PROP_PADDING = 1, VETRO_PROP_WIDTH = 2, VETRO_PROP_HEIGHT = 3, VETRO_PROP_MIN_WIDTH = 4,
    VETRO_PROP_MAX_WIDTH = 5, VETRO_PROP_MIN_HEIGHT = 6, VETRO_PROP_MAX_HEIGHT = 7,
    VETRO_PROP_FRAME_ALIGNMENT = 8, VETRO_PROP_LAYOUT_PRIORITY = 9, VETRO_PROP_HIDDEN = 10,
    VETRO_PROP_DISABLED = 11, VETRO_PROP_OPACITY = 12, VETRO_PROP_IDENTIFIER = 13,
    VETRO_PROP_TOOLTIP = 14, VETRO_PROP_ACCESSIBILITY_LABEL = 15, VETRO_PROP_ACCESSIBILITY_HELP = 16,
    VETRO_PROP_CLIPS_CHILDREN = 17, VETRO_PROP_APPEARANCE_OVERRIDE = 18, VETRO_PROP_BACKGROUND = 19,
    VETRO_PROP_CORNER_RADIUS = 20, VETRO_PROP_ASPECT_RATIO = 21, VETRO_PROP_OFFSET = 22,
    VETRO_PROP_SCALE = 23,
    VETRO_PROP_TITLE = 100, VETRO_PROP_SUBTITLE = 101, VETRO_PROP_SYMBOL = 102, VETRO_PROP_IMAGE = 103,
    VETRO_PROP_PLACEHOLDER = 104, VETRO_PROP_TEXT = 105, VETRO_PROP_VALUE = 106,
    VETRO_PROP_MIN_VALUE = 107, VETRO_PROP_MAX_VALUE = 108, VETRO_PROP_STEP = 109,
    VETRO_PROP_STATE = 110, VETRO_PROP_SELECTED_INDEX = 111, VETRO_PROP_ITEMS = 112,
    VETRO_PROP_ITEM_SYMBOLS = 113, VETRO_PROP_CONTROL_SIZE = 114, VETRO_PROP_STYLE = 115,
    VETRO_PROP_IS_DEFAULT = 116, VETRO_PROP_IS_CANCEL = 117, VETRO_PROP_SHORTCUT = 118,
    VETRO_PROP_EDITABLE = 119, VETRO_PROP_SELECTABLE = 120, VETRO_PROP_SECURE = 121,
    VETRO_PROP_MAX_LINES = 122, VETRO_PROP_TEXT_ALIGNMENT = 123, VETRO_PROP_TEXT_STYLE = 124,
    VETRO_PROP_TEXT_COLOR = 125, VETRO_PROP_SELECTED_RANGE = 126, VETRO_PROP_TICK_MARKS = 127,
    VETRO_PROP_VERTICAL = 128, VETRO_PROP_INDETERMINATE = 129, VETRO_PROP_EXPANDED = 130,
    VETRO_PROP_SPACING = 131, VETRO_PROP_ALIGNMENT = 132, VETRO_PROP_COLUMNS = 133,
    VETRO_PROP_MATERIAL = 134, VETRO_PROP_BADGE = 135, VETRO_PROP_COLOR_VALUE = 136,
    VETRO_PROP_DATE_VALUE = 137, VETRO_PROP_SELECTED_ROWS = 138, VETRO_PROP_ROW_COUNT = 139,
    VETRO_PROP_COLUMN_TITLES = 140, VETRO_PROP_TOKENS = 141, VETRO_PROP_SCROLL_OFFSET = 142,
    VETRO_PROP_CONTENT_SIZE = 143, VETRO_PROP_SPLIT_POSITION = 144, VETRO_PROP_COLLAPSED = 145,
    VETRO_PROP_SHOWS_CLEAR = 146, VETRO_PROP_ICON_POSITION = 147, VETRO_PROP_HELP_ANCHOR = 148,
    VETRO_PROP_PULLS_DOWN = 149, VETRO_PROP_ALERT_KIND = 150, VETRO_PROP_BUTTONS = 151,
    VETRO_PROP_EDGE = 152, VETRO_PROP_BORDERED = 153, VETRO_PROP_FORMAT = 154, VETRO_PROP_TINT = 155,
    VETRO_PROP_WRAPS = 156, VETRO_PROP_FONT_FAMILY = 157, VETRO_PROP_FONT_SIZE = 158,
    VETRO_PROP_FONT_WEIGHT = 159, VETRO_PROP_ANIMATION = 160, VETRO_PROP_TRANSITION = 161,
}

// Signals: libvetro's `core::value::Signal`.
ids! { SIGNALS,
    VETRO_SIGNAL_ACTIVATED = 1, VETRO_SIGNAL_VALUE_CHANGED = 2, VETRO_SIGNAL_TEXT_CHANGED = 3,
    VETRO_SIGNAL_SUBMITTED = 4, VETRO_SIGNAL_SELECTION_CHANGED = 5, VETRO_SIGNAL_EDITING_BEGAN = 6,
    VETRO_SIGNAL_EDITING_ENDED = 7, VETRO_SIGNAL_CLOSE_REQUESTED = 8, VETRO_SIGNAL_CLOSED = 9,
    VETRO_SIGNAL_POINTER_ENTERED = 10, VETRO_SIGNAL_POINTER_EXITED = 11,
    VETRO_SIGNAL_EXPANDED_CHANGED = 12, VETRO_SIGNAL_DOUBLE_CLICKED = 13,
    VETRO_SIGNAL_CONTEXT_MENU = 14, VETRO_SIGNAL_DROPPED = 15, VETRO_SIGNAL_LINK_ACTIVATED = 16,
    VETRO_SIGNAL_SCROLLED = 17, VETRO_SIGNAL_RESIZED = 18, VETRO_SIGNAL_FOCUS_IN = 19,
    VETRO_SIGNAL_FOCUS_OUT = 20, VETRO_SIGNAL_KEY_UNHANDLED = 21, VETRO_SIGNAL_DRAG_BEGAN = 22,
    VETRO_SIGNAL_DRAW = 23,
}

// View kinds for `vetro_view_new` (libvetro's `ffi::kinds`).
ids! { KINDS,
    VETRO_KIND_VIEW = 1, VETRO_KIND_ROW = 2, VETRO_KIND_COLUMN = 3, VETRO_KIND_OVERLAY = 4,
    VETRO_KIND_GRID = 5, VETRO_KIND_SPACER = 6, VETRO_KIND_LABEL = 7, VETRO_KIND_SYMBOL = 8,
    VETRO_KIND_PUSH_BUTTON = 9, VETRO_KIND_TEXT_VIEW = 10,
    VETRO_KIND_CHECKBOX = 11, VETRO_KIND_RADIO = 12, VETRO_KIND_SWITCH = 13, VETRO_KIND_SLIDER = 14,
    VETRO_KIND_STEPPER = 15, VETRO_KIND_PROGRESS_BAR = 16, VETRO_KIND_CIRCULAR_PROGRESS = 17,
    VETRO_KIND_SPINNER = 18, VETRO_KIND_SEGMENTED = 19, VETRO_KIND_TEXT_FIELD = 20,
    VETRO_KIND_SECURE_FIELD = 21, VETRO_KIND_SEARCH_FIELD = 22, VETRO_KIND_TOKEN_FIELD = 23,
    VETRO_KIND_TEXT_AREA = 24, VETRO_KIND_COMBO_BOX = 25, VETRO_KIND_POP_UP_BUTTON = 26,
    VETRO_KIND_PULL_DOWN_BUTTON = 27, VETRO_KIND_ICON_BUTTON = 28, VETRO_KIND_HELP_BUTTON = 29,
    VETRO_KIND_LINK_BUTTON = 30, VETRO_KIND_TOOLBAR_BUTTON = 31, VETRO_KIND_DISCLOSURE = 32,
    VETRO_KIND_DISCLOSURE_GROUP = 33, VETRO_KIND_GROUP_BOX = 34, VETRO_KIND_COLOR_WELL = 35,
    VETRO_KIND_IMAGE_WELL = 36, VETRO_KIND_SCROLL_VIEW = 37, VETRO_KIND_DIAL = 38,
    VETRO_KIND_LIST = 39, VETRO_KIND_TABLE_HEADER = 40, VETRO_KIND_SIDEBAR_ROW = 41,
    VETRO_KIND_SIDEBAR_HEADER = 42, VETRO_KIND_SIDEBAR_GROUP = 43, VETRO_KIND_SIDEBAR_PANEL = 44,
    VETRO_KIND_FORM = 45, VETRO_KIND_FORM_ROW = 46, VETRO_KIND_TITLE_BAR = 47,
    VETRO_KIND_TAB_BAR = 48, VETRO_KIND_TAB = 49, VETRO_KIND_STATUS_BAR = 50, VETRO_KIND_TEXT = 51,
}

// Window kinds.
ids! { WINDOW_KINDS,
    VETRO_WINDOW_NORMAL = 1, VETRO_WINDOW_PANEL = 2,
}

// Shell layers and edges.
ids! { LAYERS,
    VETRO_LAYER_BACKGROUND = 0, VETRO_LAYER_BOTTOM = 1, VETRO_LAYER_TOP = 2, VETRO_LAYER_OVERLAY = 3,
    VETRO_EDGE_TOP = 1, VETRO_EDGE_BOTTOM = 2, VETRO_EDGE_LEFT = 4, VETRO_EDGE_RIGHT = 8,
    VETRO_KEYBOARD_NONE = 0, VETRO_KEYBOARD_EXCLUSIVE = 1, VETRO_KEYBOARD_ON_DEMAND = 2,
}

// Accessibility actions.
ids! { ACTIONS,
    VETRO_ACTION_CLICK = 1, VETRO_ACTION_FOCUS = 2, VETRO_ACTION_INCREMENT = 3,
    VETRO_ACTION_DECREMENT = 4, VETRO_ACTION_EXPAND = 5, VETRO_ACTION_COLLAPSE = 6,
    VETRO_ACTION_SHOW_MENU = 7,
}

// Menu item flags and pop-up kinds.
ids! { MENU_FLAGS,
    VETRO_MENU_DISABLED = 1, VETRO_MENU_CHECKED = 2, VETRO_MENU_MIXED = 4,
    VETRO_MENU_DESTRUCTIVE = 8, VETRO_MENU_HEADING = 16,
    VETRO_POPUP_CONTEXT = 0, VETRO_POPUP_POP_UP = 1,
}

/// Calls `$m! { … }` with every function of the ABI, as
/// `fn name(arg: Type, …) -> Ret;` items. Order and signatures are the ABI.
#[macro_export]
macro_rules! for_each_function {
    ($m:ident) => {
        $m! {
            // Runtime.
            fn vetro_abi_version() -> u32;
            fn vetro_last_status() -> $crate::VetroStatus;
            fn vetro_status_message(status: $crate::VetroStatus) -> *const ::std::ffi::c_char;
            fn vetro_headless_init() -> $crate::VetroStatus;
            fn vetro_headless_shutdown();
            fn vetro_app_run(app_id: *const ::std::ffi::c_char, setup: $crate::VetroCallback, user: *mut ::std::ffi::c_void) -> $crate::VetroStatus;
            fn vetro_quit();
            fn vetro_advance(seconds: f64);
            fn vetro_after(seconds: f64, callback: $crate::VetroCallback, user: *mut ::std::ffi::c_void, destroy: $crate::VetroDestroy) -> u64;
            fn vetro_cancel(timer: u64);
            fn vetro_set_dark(dark: u8) -> $crate::VetroStatus;
            fn vetro_is_dark() -> u8;
            // Views.
            fn vetro_view_new(kind: u32) -> *mut $crate::VetroView;
            fn vetro_custom_view_new(draw: $crate::VetroDrawFn, measure: $crate::VetroMeasureFn, event: $crate::VetroEventFn, user: *mut ::std::ffi::c_void, destroy: $crate::VetroDestroy) -> *mut $crate::VetroView;
            fn vetro_view_retain(view: *mut $crate::VetroView) -> $crate::VetroStatus;
            fn vetro_view_release(view: *mut $crate::VetroView);
            fn vetro_view_add_child(parent: *mut $crate::VetroView, child: *mut $crate::VetroView) -> $crate::VetroStatus;
            fn vetro_view_insert_child(parent: *mut $crate::VetroView, index: usize, child: *mut $crate::VetroView) -> $crate::VetroStatus;
            fn vetro_view_remove(child: *mut $crate::VetroView) -> $crate::VetroStatus;
            fn vetro_view_clear_children(parent: *mut $crate::VetroView) -> $crate::VetroStatus;
            fn vetro_view_child_count(view: *mut $crate::VetroView) -> i64;
            fn vetro_view_child_at(view: *mut $crate::VetroView, index: usize) -> *mut $crate::VetroView;
            fn vetro_view_parent(view: *mut $crate::VetroView) -> *mut $crate::VetroView;
            fn vetro_view_set_none(view: *mut $crate::VetroView, prop: u32) -> $crate::VetroStatus;
            fn vetro_view_set_bool(view: *mut $crate::VetroView, prop: u32, value: u8) -> $crate::VetroStatus;
            fn vetro_view_set_int(view: *mut $crate::VetroView, prop: u32, value: i64) -> $crate::VetroStatus;
            fn vetro_view_set_float(view: *mut $crate::VetroView, prop: u32, value: f64) -> $crate::VetroStatus;
            fn vetro_view_set_string(view: *mut $crate::VetroView, prop: u32, value: *const ::std::ffi::c_char) -> $crate::VetroStatus;
            fn vetro_view_set_color(view: *mut $crate::VetroView, prop: u32, value: $crate::VetroColor) -> $crate::VetroStatus;
            fn vetro_view_set_insets(view: *mut $crate::VetroView, prop: u32, value: $crate::VetroInsets) -> $crate::VetroStatus;
            fn vetro_view_set_size(view: *mut $crate::VetroView, prop: u32, width: f32, height: f32) -> $crate::VetroStatus;
            fn vetro_view_set_point(view: *mut $crate::VetroView, prop: u32, x: f32, y: f32) -> $crate::VetroStatus;
            fn vetro_view_set_strings(view: *mut $crate::VetroView, prop: u32, items: *const *const ::std::ffi::c_char, count: usize) -> $crate::VetroStatus;
            fn vetro_view_get_bool(view: *mut $crate::VetroView, prop: u32, out: *mut u8) -> $crate::VetroStatus;
            fn vetro_view_get_int(view: *mut $crate::VetroView, prop: u32, out: *mut i64) -> $crate::VetroStatus;
            fn vetro_view_get_float(view: *mut $crate::VetroView, prop: u32, out: *mut f64) -> $crate::VetroStatus;
            fn vetro_view_get_string(view: *mut $crate::VetroView, prop: u32, buffer: *mut ::std::ffi::c_char, capacity: usize, out_len: *mut usize) -> $crate::VetroStatus;
            fn vetro_view_connect(view: *mut $crate::VetroView, signal: u32, callback: $crate::VetroSignalFn, user: *mut ::std::ffi::c_void, destroy: $crate::VetroDestroy) -> u64;
            fn vetro_disconnect(handler: u64);
            fn vetro_view_request_paint(view: *mut $crate::VetroView) -> $crate::VetroStatus;
            fn vetro_view_request_layout(view: *mut $crate::VetroView) -> $crate::VetroStatus;
            fn vetro_view_focus(view: *mut $crate::VetroView) -> $crate::VetroStatus;
            fn vetro_view_frame(view: *mut $crate::VetroView, out: *mut $crate::VetroRect) -> $crate::VetroStatus;
            fn vetro_view_find(root: *mut $crate::VetroView, identifier: *const ::std::ffi::c_char) -> *mut $crate::VetroView;
            // Windows.
            fn vetro_window_new(kind: u32, width: f32, height: f32) -> *mut $crate::VetroWindow;
            fn vetro_window_new_popup(parent: *mut $crate::VetroWindow, anchor: $crate::VetroRect, width: f32, height: f32) -> *mut $crate::VetroWindow;
            fn vetro_window_new_shell(layer: u32, edges: u32, exclusive: i32, keyboard: u32, namespace: *const ::std::ffi::c_char, width: f32, height: f32) -> *mut $crate::VetroWindow;
            fn vetro_window_new_lock() -> *mut $crate::VetroWindow;
            fn vetro_window_set_title(window: *mut $crate::VetroWindow, title: *const ::std::ffi::c_char) -> $crate::VetroStatus;
            fn vetro_window_set_content(window: *mut $crate::VetroWindow, view: *mut $crate::VetroView) -> $crate::VetroStatus;
            fn vetro_window_root(window: *mut $crate::VetroWindow) -> *mut $crate::VetroView;
            fn vetro_window_show(window: *mut $crate::VetroWindow) -> $crate::VetroStatus;
            fn vetro_window_close(window: *mut $crate::VetroWindow) -> $crate::VetroStatus;
            fn vetro_window_set_size(window: *mut $crate::VetroWindow, width: f32, height: f32) -> $crate::VetroStatus;
            fn vetro_window_set_size_limits(window: *mut $crate::VetroWindow, min_width: f32, min_height: f32, max_width: f32, max_height: f32) -> $crate::VetroStatus;
            fn vetro_window_set_appearance(window: *mut $crate::VetroWindow, appearance: u32) -> $crate::VetroStatus;
            fn vetro_window_set_toolbar_height(window: *mut $crate::VetroWindow, height: f32) -> $crate::VetroStatus;
            fn vetro_window_set_keep_on_close_request(window: *mut $crate::VetroWindow, keep: u8) -> $crate::VetroStatus;
            fn vetro_window_snapshot_png(window: *mut $crate::VetroWindow, scale: f32, callback: $crate::VetroBytesFn, user: *mut ::std::ffi::c_void) -> $crate::VetroStatus;
            // Menus.
            fn vetro_menu_new(title: *const ::std::ffi::c_char, role: *const ::std::ffi::c_char) -> *mut $crate::VetroMenu;
            fn vetro_menu_free(menu: *mut $crate::VetroMenu);
            fn vetro_menu_add_item(menu: *mut $crate::VetroMenu, title: *const ::std::ffi::c_char, shortcut: *const ::std::ffi::c_char, id: u32, flags: u32) -> $crate::VetroStatus;
            fn vetro_menu_add_separator(menu: *mut $crate::VetroMenu) -> $crate::VetroStatus;
            fn vetro_menu_add_submenu(menu: *mut $crate::VetroMenu, submenu: *mut $crate::VetroMenu) -> $crate::VetroStatus;
            fn vetro_app_set_menu_bar(menus: *const *mut $crate::VetroMenu, count: usize, callback: $crate::VetroMenuFn, user: *mut ::std::ffi::c_void, destroy: $crate::VetroDestroy) -> $crate::VetroStatus;
            fn vetro_view_popup_menu(view: *mut $crate::VetroView, menu: *mut $crate::VetroMenu, anchor: $crate::VetroRect, kind: u32, selected: u32, callback: $crate::VetroMenuFn, user: *mut ::std::ffi::c_void, destroy: $crate::VetroDestroy) -> $crate::VetroStatus;
            // Canvas (inside a draw callback).
            fn vetro_canvas_save(canvas: *mut $crate::VetroCanvas);
            fn vetro_canvas_restore(canvas: *mut $crate::VetroCanvas);
            fn vetro_canvas_translate(canvas: *mut $crate::VetroCanvas, x: f32, y: f32);
            fn vetro_canvas_clip_rect(canvas: *mut $crate::VetroCanvas, rect: $crate::VetroRect);
            fn vetro_canvas_fill_rect(canvas: *mut $crate::VetroCanvas, rect: $crate::VetroRect, color: $crate::VetroColor);
            fn vetro_canvas_fill_rounded_rect(canvas: *mut $crate::VetroCanvas, rect: $crate::VetroRect, radius: f32, color: $crate::VetroColor);
            fn vetro_canvas_stroke_rect(canvas: *mut $crate::VetroCanvas, rect: $crate::VetroRect, width: f32, color: $crate::VetroColor);
            fn vetro_canvas_fill_ellipse(canvas: *mut $crate::VetroCanvas, rect: $crate::VetroRect, color: $crate::VetroColor);
            fn vetro_canvas_line(canvas: *mut $crate::VetroCanvas, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: $crate::VetroColor);
            fn vetro_canvas_text(canvas: *mut $crate::VetroCanvas, x: f32, y: f32, text: *const ::std::ffi::c_char, family: *const ::std::ffi::c_char, size: f32, weight: u32, color: $crate::VetroColor);
            fn vetro_canvas_glyph(canvas: *mut $crate::VetroCanvas, rect: $crate::VetroRect, symbol: *const ::std::ffi::c_char, weight: u32, color: $crate::VetroColor);
            fn vetro_measure_text(text: *const ::std::ffi::c_char, family: *const ::std::ffi::c_char, size: f32, weight: u32, out_width: *mut f32, out_height: *mut f32) -> $crate::VetroStatus;
            // Accessibility actions (assistive technologies, tests).
            fn vetro_view_perform(view: *mut $crate::VetroView, action: u32) -> $crate::VetroStatus;
            // Windows of views; sheets.
            fn vetro_view_window(view: *mut $crate::VetroView) -> *mut $crate::VetroWindow;
            fn vetro_window_new_sheet(parent: *mut $crate::VetroWindow, width: f32, height: f32) -> *mut $crate::VetroWindow;
            // Virtualised lists.
            fn vetro_list_set_rows(view: *mut $crate::VetroView, rows: u64, row_height: f32, provider: $crate::VetroRowFn, user: *mut ::std::ffi::c_void, destroy: $crate::VetroDestroy) -> $crate::VetroStatus;
        }
    };
}

/// Expands the function list into `extern "C"` declarations.
macro_rules! declare {
    ($(fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)?;)*) => {
        unsafe extern "C" {
            $(
                #[allow(missing_docs)]
                pub fn $name($($arg: $ty),*) $(-> $ret)?;
            )*
        }
        /// The names of every function, in ABI order.
        pub static FUNCTIONS: &[&str] = &[$(stringify!($name),)*];
    };
}

/// A function's signature as Rust tokens, for the header generator and the ABI check.
#[derive(Debug, Clone, Copy)]
pub struct Signature {
    /// Name.
    pub name: &'static str,
    /// (argument, type) pairs.
    pub args: &'static [(&'static str, &'static str)],
    /// Return type (empty: none).
    pub ret: &'static str,
}

macro_rules! signatures {
    ($(fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)?;)*) => {
        /// Every function's signature, in ABI order.
        pub static SIGNATURES: &[Signature] = &[$(
            Signature {
                name: stringify!($name),
                args: &[$((stringify!($arg), stringify!($ty))),*],
                ret: concat!("" $(, stringify!($ret))?),
            },
        )*];
    };
}

for_each_function!(signatures);

/// The functions of the ABI (`use vetro_sys::functions::*`), apart from the types and
/// constants so that a library defining them can glob-import the latter.
pub mod functions {
    for_each_function!(declare);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_are_stable() {
        assert_eq!(std::mem::size_of::<VetroColor>(), 4);
        assert_eq!(std::mem::size_of::<VetroRect>(), 16);
        assert_eq!(std::mem::size_of::<VetroInsets>(), 16);
        assert_eq!(std::mem::size_of::<VetroValue>(), 64);
        assert_eq!(std::mem::size_of::<VetroEvent>(), 56);
        assert!(functions::FUNCTIONS.len() > 70);
        let mut names: Vec<&str> = functions::FUNCTIONS.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), functions::FUNCTIONS.len());
    }
}

#[cfg(test)]
mod signature_tests {
    #[test]
    fn signatures_are_listed() {
        let s = super::SIGNATURES
            .iter()
            .find(|s| s.name == "vetro_view_set_string")
            .unwrap();
        assert_eq!(s.args.len(), 3);
        assert!(s.ret.contains("VetroStatus"));
        println!("{:?}", s.args);
    }
}
