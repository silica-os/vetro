//! Vetro's safe Rust API (the imperative layer, AppKit-like): views, windows, menus, timers
//! and custom drawing, as Rust objects over the stable C ABI of `libvetro.so`. All real work
//! happens in the shared library; this crate only translates.
//!
//! Handles are reference counted: cloning a [`View`] retains it, dropping releases it.
//! Callbacks are Rust closures, run on the UI thread, and may use the API.

// The ABI calls are `unsafe` (FFI); each is wrapped with the invariants stated.
#![allow(unsafe_code)]

mod ids;

#[cfg(feature = "static")]
use vetro_lib as _;

use std::cell::RefCell;
use std::ffi::{CString, c_char, c_void};
use std::ptr::NonNull;
use std::rc::Rc;

pub use ids::{Kind, Prop, Signal};
use vetro_sys as sys;
use vetro_sys::functions as f;

/// An error of the API (the ABI's status).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The view or window no longer exists.
    NoSuchObject,
    /// The view does not have this property.
    UnknownProperty,
    /// The value has the wrong type.
    BadValue,
    /// A cycle, or a child with another parent.
    BadHierarchy,
    /// Another thread than the UI thread.
    WrongThread,
    /// An invalid argument (a string with a NUL byte …).
    InvalidArgument,
    /// The platform failed.
    Platform,
    /// No runtime: inside [`run`] or after [`Headless::new`] only.
    NoRuntime,
    /// Another status.
    Other(i32),
}

impl Error {
    fn from_status(s: i32) -> Error {
        match s {
            sys::VETRO_ERROR_NO_SUCH_OBJECT => Error::NoSuchObject,
            sys::VETRO_ERROR_UNKNOWN_PROPERTY => Error::UnknownProperty,
            sys::VETRO_ERROR_BAD_VALUE => Error::BadValue,
            sys::VETRO_ERROR_BAD_HIERARCHY => Error::BadHierarchy,
            sys::VETRO_ERROR_WRONG_THREAD => Error::WrongThread,
            sys::VETRO_ERROR_INVALID_ARGUMENT => Error::InvalidArgument,
            sys::VETRO_ERROR_PLATFORM => Error::Platform,
            sys::VETRO_ERROR_NO_RUNTIME => Error::NoRuntime,
            other => Error::Other(other),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, fm: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fm, "{self:?}")
    }
}

impl std::error::Error for Error {}

/// A result of the API.
pub type Result<T> = std::result::Result<T, Error>;

fn check(s: i32) -> Result<()> {
    if s == sys::VETRO_OK {
        Ok(())
    } else {
        Err(Error::from_status(s))
    }
}

fn last_error() -> Error {
    // SAFETY: no arguments; reads the thread's last status.
    Error::from_status(unsafe { f::vetro_last_status() })
}

fn cstring(s: &str) -> Result<CString> {
    CString::new(s).map_err(|_| Error::InvalidArgument)
}

/// A colour (straight alpha).
pub type Color = sys::VetroColor;

/// A colour from `0xRRGGBBAA`.
pub fn rgba(v: u32) -> Color {
    Color {
        r: (v >> 24) as u8,
        g: (v >> 16) as u8,
        b: (v >> 8) as u8,
        a: v as u8,
    }
}

/// A rectangle.
pub type Rect = sys::VetroRect;

/// A value received with a signal.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    /// None.
    #[default]
    None,
    /// Boolean.
    Bool(bool),
    /// Integer.
    Int(i64),
    /// Number.
    Float(f64),
    /// Text.
    Str(String),
    /// Colour.
    Color(Color),
    /// Point.
    Point(f32, f32),
    /// Size.
    Size(f32, f32),
    /// Rectangle.
    Rect(Rect),
    /// Byte range.
    Range(usize, usize),
    /// List of strings.
    List(Vec<String>),
}

impl Value {
    /// The value as an integer, if it is a number.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Float(f) => Some(*f as i64),
            Value::Bool(b) => Some(i64::from(*b)),
            _ => None,
        }
    }

    /// The value as a number.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// The text.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The value as a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            Value::Int(i) => Some(*i != 0),
            _ => None,
        }
    }

    fn from_c(v: &sys::VetroValue) -> Value {
        let text = || -> String {
            if v.text.is_null() || v.text_len == 0 {
                return String::new();
            }
            // SAFETY: libvetro hands `text_len` valid bytes for the callback's duration.
            let bytes = unsafe { std::slice::from_raw_parts(v.text.cast::<u8>(), v.text_len) };
            String::from_utf8_lossy(bytes).into_owned()
        };
        match v.kind {
            sys::VETRO_VALUE_BOOL => Value::Bool(v.b != 0),
            sys::VETRO_VALUE_INT => Value::Int(v.i),
            sys::VETRO_VALUE_FLOAT => Value::Float(v.f),
            sys::VETRO_VALUE_STRING => Value::Str(text()),
            sys::VETRO_VALUE_COLOR => Value::Color(v.color),
            sys::VETRO_VALUE_POINT => Value::Point(v.rect.x, v.rect.y),
            sys::VETRO_VALUE_SIZE => Value::Size(v.rect.width, v.rect.height),
            sys::VETRO_VALUE_RECT => Value::Rect(v.rect),
            sys::VETRO_VALUE_RANGE => {
                Value::Range(usize::try_from(v.i).unwrap_or(0), v.f.max(0.0) as usize)
            }
            sys::VETRO_VALUE_STRING_LIST => {
                let t = text();
                if t.is_empty() {
                    Value::List(Vec::new())
                } else {
                    Value::List(t.split('\0').map(str::to_string).collect())
                }
            }
            _ => Value::None,
        }
    }
}

/// Boxed closure data handed to the ABI with a destroy trampoline.
fn boxed<T>(value: T) -> *mut c_void {
    Box::into_raw(Box::new(value)).cast()
}

unsafe extern "C" fn destroy<T>(user: *mut c_void) {
    if !user.is_null() {
        // SAFETY: `user` came from `boxed::<T>` and libvetro calls destroy once.
        drop(unsafe { Box::from_raw(user.cast::<T>()) });
    }
}

// ----- Runtime ------------------------------------------------------------------------------

/// Runs the app: connects to the display, calls `setup`, and runs the event loop until the
/// app quits.
pub fn run(app_id: &str, setup: impl FnOnce() + 'static) -> Result<()> {
    type Setup = Option<Box<dyn FnOnce()>>;
    unsafe extern "C" fn call(user: *mut c_void) {
        // SAFETY: `user` is the `Setup` below, alive during vetro_app_run.
        let s = unsafe { &mut *(user.cast::<Setup>()) };
        if let Some(f) = s.take() {
            f();
        }
    }
    let id = cstring(app_id)?;
    let mut s: Setup = Some(Box::new(setup));
    // SAFETY: `s` outlives the call; the callback is called at most once.
    check(unsafe { f::vetro_app_run(id.as_ptr(), Some(call), (&mut s as *mut Setup).cast()) })
}

/// Leaves the event loop.
pub fn quit() {
    // SAFETY: no arguments.
    unsafe { f::vetro_quit() };
}

/// A headless runtime for tests and offscreen rendering; shut down when dropped.
pub struct Headless(());

impl Headless {
    /// Starts it (one per thread).
    pub fn new() -> Result<Headless> {
        // SAFETY: no arguments.
        check(unsafe { f::vetro_headless_init() })?;
        Ok(Headless(()))
    }

    /// Advances time (animations, timers).
    pub fn advance(&self, seconds: f64) {
        // SAFETY: plain value.
        unsafe { f::vetro_advance(seconds) };
    }
}

impl Drop for Headless {
    fn drop(&mut self) {
        // SAFETY: no arguments.
        unsafe { f::vetro_headless_shutdown() };
    }
}

/// A timer; cancelled with [`Timer::cancel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timer(u64);

impl Timer {
    /// Cancels it.
    pub fn cancel(self) {
        // SAFETY: a plain id; unknown ids are ignored.
        unsafe { f::vetro_cancel(self.0) };
    }
}

/// Runs `cb` after `seconds`.
pub fn after(seconds: f64, cb: impl FnOnce() + 'static) -> Result<Timer> {
    type Cb = Option<Box<dyn FnOnce()>>;
    unsafe extern "C" fn call(user: *mut c_void) {
        // SAFETY: `user` is a boxed `Cb`, alive until destroy.
        let c = unsafe { &mut *(user.cast::<Cb>()) };
        if let Some(f) = c.take() {
            f();
        }
    }
    let data: Cb = Some(Box::new(cb));
    // SAFETY: the data is freed by `destroy::<Cb>` exactly once.
    let id = unsafe { f::vetro_after(seconds, Some(call), boxed(data), Some(destroy::<Cb>)) };
    if id == 0 {
        Err(last_error())
    } else {
        Ok(Timer(id))
    }
}

/// Sets the dark appearance (the platform follows the system otherwise).
pub fn set_dark(dark: bool) -> Result<()> {
    // SAFETY: plain value.
    check(unsafe { f::vetro_set_dark(u8::from(dark)) })
}

/// Whether the appearance is dark.
pub fn is_dark() -> bool {
    // SAFETY: no arguments.
    unsafe { f::vetro_is_dark() != 0 }
}

// ----- Views --------------------------------------------------------------------------------

/// A view (one reference).
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct View {
    ptr: NonNull<sys::VetroView>,
}

impl Clone for View {
    fn clone(&self) -> Self {
        // SAFETY: a handle of a view; a stale one is reported and ignored.
        unsafe { f::vetro_view_retain(self.ptr.as_ptr()) };
        View { ptr: self.ptr }
    }
}

impl Drop for View {
    fn drop(&mut self) {
        // SAFETY: drops this handle's reference.
        unsafe { f::vetro_view_release(self.ptr.as_ptr()) };
    }
}

/// A connected signal handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Connection(u64);

impl Connection {
    /// Disconnects the handler.
    pub fn disconnect(self) {
        // SAFETY: a plain id; unknown ids are ignored.
        unsafe { f::vetro_disconnect(self.0) };
    }
}

impl View {
    /// A new view of `kind`.
    pub fn new(kind: Kind) -> Result<View> {
        // SAFETY: plain value.
        let p = unsafe { f::vetro_view_new(kind.0) };
        NonNull::new(p)
            .map(|ptr| View { ptr })
            .ok_or_else(last_error)
    }

    /// A handle for a borrowed pointer (retained).
    fn borrowed(p: *mut sys::VetroView) -> Option<View> {
        let ptr = NonNull::new(p)?;
        // SAFETY: a live handle returned by the ABI; retaining makes this one own a reference.
        unsafe { f::vetro_view_retain(ptr.as_ptr()) };
        Some(View { ptr })
    }

    /// The raw handle (for the C API).
    pub fn as_ptr(&self) -> *mut sys::VetroView {
        self.ptr.as_ptr()
    }

    /// Appends a child.
    pub fn add(&self, child: &View) -> Result<()> {
        // SAFETY: two handles.
        check(unsafe { f::vetro_view_add_child(self.as_ptr(), child.as_ptr()) })
    }

    /// Inserts a child at `index`.
    pub fn insert(&self, index: usize, child: &View) -> Result<()> {
        // SAFETY: two handles.
        check(unsafe { f::vetro_view_insert_child(self.as_ptr(), index, child.as_ptr()) })
    }

    /// Removes this view from its parent.
    pub fn remove_from_parent(&self) -> Result<()> {
        // SAFETY: a handle.
        check(unsafe { f::vetro_view_remove(self.as_ptr()) })
    }

    /// Removes every child.
    pub fn clear(&self) -> Result<()> {
        // SAFETY: a handle.
        check(unsafe { f::vetro_view_clear_children(self.as_ptr()) })
    }

    /// The children.
    pub fn children(&self) -> Vec<View> {
        // SAFETY: a handle.
        let n = unsafe { f::vetro_view_child_count(self.as_ptr()) };
        (0..usize::try_from(n).unwrap_or(0))
            // SAFETY: a handle and an index; null is skipped.
            .filter_map(|i| View::borrowed(unsafe { f::vetro_view_child_at(self.as_ptr(), i) }))
            .collect()
    }

    /// The parent.
    pub fn parent(&self) -> Option<View> {
        // SAFETY: a handle.
        View::borrowed(unsafe { f::vetro_view_parent(self.as_ptr()) })
    }

    /// Clears a property (back to its default).
    pub fn unset(&self, prop: Prop) -> Result<()> {
        // SAFETY: a handle and an id.
        check(unsafe { f::vetro_view_set_none(self.as_ptr(), prop.0) })
    }

    /// Sets a boolean property.
    pub fn set_bool(&self, prop: Prop, v: bool) -> Result<()> {
        // SAFETY: a handle and plain values.
        check(unsafe { f::vetro_view_set_bool(self.as_ptr(), prop.0, u8::from(v)) })
    }

    /// Sets an integer property.
    pub fn set_int(&self, prop: Prop, v: i64) -> Result<()> {
        // SAFETY: a handle and plain values.
        check(unsafe { f::vetro_view_set_int(self.as_ptr(), prop.0, v) })
    }

    /// Sets a number property.
    pub fn set_float(&self, prop: Prop, v: f64) -> Result<()> {
        // SAFETY: a handle and plain values.
        check(unsafe { f::vetro_view_set_float(self.as_ptr(), prop.0, v) })
    }

    /// Sets a text property.
    pub fn set_str(&self, prop: Prop, v: &str) -> Result<()> {
        let s = cstring(v)?;
        // SAFETY: a handle and a NUL-terminated string alive for the call.
        check(unsafe { f::vetro_view_set_string(self.as_ptr(), prop.0, s.as_ptr()) })
    }

    /// Sets a colour property.
    pub fn set_color(&self, prop: Prop, v: Color) -> Result<()> {
        // SAFETY: a handle and plain values.
        check(unsafe { f::vetro_view_set_color(self.as_ptr(), prop.0, v) })
    }

    /// Sets insets (top, left, bottom, right).
    pub fn set_insets(
        &self,
        prop: Prop,
        top: f32,
        left: f32,
        bottom: f32,
        right: f32,
    ) -> Result<()> {
        let v = sys::VetroInsets {
            top,
            left,
            bottom,
            right,
        };
        // SAFETY: a handle and plain values.
        check(unsafe { f::vetro_view_set_insets(self.as_ptr(), prop.0, v) })
    }

    /// Sets a size property.
    pub fn set_size(&self, prop: Prop, w: f32, h: f32) -> Result<()> {
        // SAFETY: a handle and plain values.
        check(unsafe { f::vetro_view_set_size(self.as_ptr(), prop.0, w, h) })
    }

    /// Sets a point property.
    pub fn set_point(&self, prop: Prop, x: f32, y: f32) -> Result<()> {
        // SAFETY: a handle and plain values.
        check(unsafe { f::vetro_view_set_point(self.as_ptr(), prop.0, x, y) })
    }

    /// Sets a list of strings.
    pub fn set_strings<S: AsRef<str>>(&self, prop: Prop, items: &[S]) -> Result<()> {
        let owned: Vec<CString> = items
            .iter()
            .map(|s| cstring(s.as_ref()))
            .collect::<Result<_>>()?;
        let ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
        // SAFETY: `ptrs` holds `len` strings alive for the call.
        check(unsafe {
            f::vetro_view_set_strings(self.as_ptr(), prop.0, ptrs.as_ptr(), ptrs.len())
        })
    }

    /// Reads a boolean property.
    pub fn get_bool(&self, prop: Prop) -> Result<bool> {
        let mut v = 0u8;
        // SAFETY: a handle and a valid out pointer.
        check(unsafe { f::vetro_view_get_bool(self.as_ptr(), prop.0, &mut v) })?;
        Ok(v != 0)
    }

    /// Reads an integer property.
    pub fn get_int(&self, prop: Prop) -> Result<i64> {
        let mut v = 0i64;
        // SAFETY: a handle and a valid out pointer.
        check(unsafe { f::vetro_view_get_int(self.as_ptr(), prop.0, &mut v) })?;
        Ok(v)
    }

    /// Reads a number property.
    pub fn get_float(&self, prop: Prop) -> Result<f64> {
        let mut v = 0f64;
        // SAFETY: a handle and a valid out pointer.
        check(unsafe { f::vetro_view_get_float(self.as_ptr(), prop.0, &mut v) })?;
        Ok(v)
    }

    /// Reads a text property.
    pub fn get_str(&self, prop: Prop) -> Result<String> {
        let mut len = 0usize;
        // SAFETY: a null buffer asks for the length.
        let s = unsafe {
            f::vetro_view_get_string(self.as_ptr(), prop.0, std::ptr::null_mut(), 0, &mut len)
        };
        if s != sys::VETRO_ERROR_BUFFER_TOO_SMALL && s != sys::VETRO_OK {
            return Err(Error::from_status(s));
        }
        let mut buf = vec![0u8; len + 1];
        // SAFETY: the buffer holds `len + 1` bytes.
        check(unsafe {
            f::vetro_view_get_string(
                self.as_ptr(),
                prop.0,
                buf.as_mut_ptr().cast(),
                buf.len(),
                &mut len,
            )
        })?;
        buf.truncate(len);
        String::from_utf8(buf).map_err(|_| Error::BadValue)
    }

    /// Calls `cb` whenever the view emits `signal`.
    pub fn on(
        &self,
        signal: Signal,
        cb: impl FnMut(&View, &Value) + 'static,
    ) -> Result<Connection> {
        type Cb = Box<dyn FnMut(&View, &Value)>;
        unsafe extern "C" fn call(
            view: *mut sys::VetroView,
            _signal: u32,
            value: *const sys::VetroValue,
            user: *mut c_void,
        ) {
            // SAFETY: `user` is a boxed `Cb`, alive until destroy; `value` is valid for the call.
            let cb = unsafe { &mut *(user.cast::<Cb>()) };
            let v = if value.is_null() {
                Value::None
            } else {
                // SAFETY: as above.
                Value::from_c(unsafe { &*value })
            };
            if let Some(view) = View::borrowed(view) {
                cb(&view, &v);
            }
        }
        let data: Cb = Box::new(cb);
        // SAFETY: the data is freed by `destroy::<Cb>` exactly once.
        let id = unsafe {
            f::vetro_view_connect(
                self.as_ptr(),
                signal.0,
                Some(call),
                boxed(data),
                Some(destroy::<Cb>),
            )
        };
        if id == 0 {
            Err(last_error())
        } else {
            Ok(Connection(id))
        }
    }

    /// Records the drawing again.
    pub fn request_paint(&self) {
        // SAFETY: a handle.
        unsafe { f::vetro_view_request_paint(self.as_ptr()) };
    }

    /// Lays the view out again.
    pub fn request_layout(&self) {
        // SAFETY: a handle.
        unsafe { f::vetro_view_request_layout(self.as_ptr()) };
    }

    /// Gives it the keyboard focus.
    pub fn focus(&self) -> Result<()> {
        // SAFETY: a handle.
        check(unsafe { f::vetro_view_focus(self.as_ptr()) })
    }

    /// Its rectangle in window coordinates.
    pub fn frame(&self) -> Result<Rect> {
        let mut r = Rect::default();
        // SAFETY: a handle and a valid out pointer.
        check(unsafe { f::vetro_view_frame(self.as_ptr(), &mut r) })?;
        Ok(r)
    }

    /// The descendant with `identifier`.
    pub fn find(&self, identifier: &str) -> Option<View> {
        let s = cstring(identifier).ok()?;
        // SAFETY: a handle and a string alive for the call.
        View::borrowed(unsafe { f::vetro_view_find(self.as_ptr(), s.as_ptr()) })
    }

    /// A pop-up menu anchored at `anchor` (view coordinates); `done` gets the chosen id
    /// (0: dismissed).
    pub fn popup_menu(
        &self,
        menu: Menu,
        anchor: Rect,
        popup: bool,
        selected: u32,
        done: impl FnOnce(u32) + 'static,
    ) -> Result<()> {
        type Cb = Option<Box<dyn FnOnce(u32)>>;
        unsafe extern "C" fn call(id: u32, user: *mut c_void) {
            // SAFETY: a boxed `Cb` alive until destroy.
            let c = unsafe { &mut *(user.cast::<Cb>()) };
            if let Some(f) = c.take() {
                f(id);
            }
        }
        let data: Cb = Some(Box::new(done));
        let kind = if popup {
            sys::VETRO_POPUP_POP_UP
        } else {
            sys::VETRO_POPUP_CONTEXT
        };
        let raw = menu.into_raw();
        // SAFETY: the menu is consumed; the data freed by destroy once.
        check(unsafe {
            f::vetro_view_popup_menu(
                self.as_ptr(),
                raw,
                anchor,
                kind,
                selected,
                Some(call),
                boxed(data),
                Some(destroy::<Cb>),
            )
        })
    }
}

/// A font for custom drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct Font {
    /// Family (`None`: Inter).
    pub family: Option<String>,
    /// Size in points.
    pub size: f32,
    /// CSS weight.
    pub weight: u32,
}

impl Font {
    /// Inter at `size`, regular.
    pub fn ui(size: f32) -> Font {
        Font {
            family: None,
            size,
            weight: 400,
        }
    }
}

/// A canvas inside a custom view's draw callback.
pub struct Canvas {
    ptr: *mut sys::VetroCanvas,
}

impl Canvas {
    /// Fills a rectangle.
    pub fn fill_rect(&self, r: Rect, c: Color) {
        // SAFETY: the canvas is valid during the draw callback.
        unsafe { f::vetro_canvas_fill_rect(self.ptr, r, c) };
    }

    /// Fills a smooth rounded rectangle.
    pub fn fill_rounded_rect(&self, r: Rect, radius: f32, c: Color) {
        // SAFETY: as above.
        unsafe { f::vetro_canvas_fill_rounded_rect(self.ptr, r, radius, c) };
    }

    /// Strokes a rectangle.
    pub fn stroke_rect(&self, r: Rect, width: f32, c: Color) {
        // SAFETY: as above.
        unsafe { f::vetro_canvas_stroke_rect(self.ptr, r, width, c) };
    }

    /// Fills an ellipse.
    pub fn fill_ellipse(&self, r: Rect, c: Color) {
        // SAFETY: as above.
        unsafe { f::vetro_canvas_fill_ellipse(self.ptr, r, c) };
    }

    /// Draws a line.
    pub fn line(&self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, c: Color) {
        // SAFETY: as above.
        unsafe { f::vetro_canvas_line(self.ptr, x0, y0, x1, y1, width, c) };
    }

    /// Draws text with its top left at `origin`.
    pub fn text(&self, origin: (f32, f32), text: &str, font: &Font, c: Color) {
        let Ok(t) = cstring(text) else { return };
        let fam = match font.family.as_deref().map(cstring) {
            Some(Ok(s)) => Some(s),
            Some(Err(_)) => return,
            None => None,
        };
        let fp = fam.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
        // SAFETY: as above; the strings live for the call.
        unsafe {
            f::vetro_canvas_text(
                self.ptr,
                origin.0,
                origin.1,
                t.as_ptr(),
                fp,
                font.size,
                font.weight,
                c,
            )
        };
    }

    /// Draws a Silica Symbols glyph in a box (font size = box height).
    pub fn glyph(&self, r: Rect, symbol: &str, weight: u32, c: Color) {
        let Ok(s) = cstring(symbol) else { return };
        // SAFETY: as above.
        unsafe { f::vetro_canvas_glyph(self.ptr, r, s.as_ptr(), weight, c) };
    }

    /// Saves the state.
    pub fn save(&self) {
        // SAFETY: as above.
        unsafe { f::vetro_canvas_save(self.ptr) };
    }

    /// Restores the state.
    pub fn restore(&self) {
        // SAFETY: as above.
        unsafe { f::vetro_canvas_restore(self.ptr) };
    }

    /// Moves the origin.
    pub fn translate(&self, x: f32, y: f32) {
        // SAFETY: as above.
        unsafe { f::vetro_canvas_translate(self.ptr, x, y) };
    }

    /// Clips to a rectangle.
    pub fn clip(&self, r: Rect) {
        // SAFETY: as above.
        unsafe { f::vetro_canvas_clip_rect(self.ptr, r) };
    }
}

/// The width and height of `text` in a font.
pub fn measure_text(
    text: &str,
    family: Option<&str>,
    size: f32,
    weight: u32,
) -> Option<(f32, f32)> {
    let t = cstring(text).ok()?;
    let fam = match family {
        Some(f) => Some(cstring(f).ok()?),
        None => None,
    };
    let (mut w, mut h) = (0.0, 0.0);
    // SAFETY: strings alive for the call; valid out pointers.
    let s = unsafe {
        f::vetro_measure_text(
            t.as_ptr(),
            fam.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
            size,
            weight,
            &mut w,
            &mut h,
        )
    };
    (s == sys::VETRO_OK).then_some((w, h))
}

/// A custom view's behaviour.
pub trait Custom: 'static {
    /// Draws in a box of `width × height`.
    fn draw(&mut self, canvas: &Canvas, width: f32, height: f32);
    /// The size for a proposal (`None`: unspecified).
    fn measure(&mut self, proposed: (Option<f32>, Option<f32>)) -> (f32, f32) {
        (proposed.0.unwrap_or(0.0), proposed.1.unwrap_or(0.0))
    }
    /// Handles an event; true when handled.
    fn event(&mut self, _view: &View, _event: &sys::VetroEvent) -> bool {
        false
    }
}

impl View {
    /// A view drawn by `custom`.
    pub fn custom(custom: impl Custom) -> Result<View> {
        type Data = RefCell<Box<dyn Custom>>;
        unsafe extern "C" fn draw(c: *mut sys::VetroCanvas, w: f32, h: f32, user: *mut c_void) {
            // SAFETY: `user` is the boxed `Data`, alive until destroy.
            let d = unsafe { &*(user.cast::<Data>()) };
            if let Ok(mut b) = d.try_borrow_mut() {
                b.draw(&Canvas { ptr: c }, w, h);
            }
        }
        unsafe extern "C" fn measure(
            pw: f32,
            ph: f32,
            ow: *mut f32,
            oh: *mut f32,
            user: *mut c_void,
        ) {
            // SAFETY: as above; out pointers valid.
            let d = unsafe { &*(user.cast::<Data>()) };
            let opt = |v: f32| (!v.is_nan()).then_some(v);
            let (w, h) = match d.try_borrow_mut() {
                Ok(mut b) => b.measure((opt(pw), opt(ph))),
                Err(_) => (0.0, 0.0),
            };
            // SAFETY: valid out pointers from libvetro.
            unsafe {
                *ow = w;
                *oh = h;
            }
        }
        unsafe extern "C" fn event(
            v: *mut sys::VetroView,
            e: *const sys::VetroEvent,
            user: *mut c_void,
        ) -> u8 {
            // SAFETY: as above; the event is valid for the call.
            let d = unsafe { &*(user.cast::<Data>()) };
            let (Some(view), false) = (View::borrowed(v), e.is_null()) else {
                return 0;
            };
            match d.try_borrow_mut() {
                // SAFETY: checked non-null.
                Ok(mut b) => u8::from(b.event(&view, unsafe { &*e })),
                Err(_) => 0,
            }
        }
        let data: Data = RefCell::new(Box::new(custom));
        // SAFETY: the data is freed by destroy once.
        let p = unsafe {
            f::vetro_custom_view_new(
                Some(draw),
                Some(measure),
                Some(event),
                boxed(data),
                Some(destroy::<Data>),
            )
        };
        NonNull::new(p)
            .map(|ptr| View { ptr })
            .ok_or_else(last_error)
    }
}

// ----- Windows ------------------------------------------------------------------------------

/// A window handle (windows live until closed; the handle is a plain id).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Window {
    ptr: NonNull<sys::VetroWindow>,
}

/// A shell surface's layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Background.
    Background,
    /// Bottom.
    Bottom,
    /// Top.
    Top,
    /// Overlay.
    Overlay,
}

impl Window {
    fn from(p: *mut sys::VetroWindow) -> Result<Window> {
        NonNull::new(p)
            .map(|ptr| Window { ptr })
            .ok_or_else(last_error)
    }

    /// An app window.
    pub fn new(width: f32, height: f32) -> Result<Window> {
        // SAFETY: plain values.
        Window::from(unsafe { f::vetro_window_new(sys::VETRO_WINDOW_NORMAL, width, height) })
    }

    /// A utility panel.
    pub fn panel(width: f32, height: f32) -> Result<Window> {
        // SAFETY: plain values.
        Window::from(unsafe { f::vetro_window_new(sys::VETRO_WINDOW_PANEL, width, height) })
    }

    /// A popup anchored to a rectangle of `parent`.
    pub fn popup(parent: Window, anchor: Rect, width: f32, height: f32) -> Result<Window> {
        // SAFETY: a handle and plain values.
        Window::from(unsafe {
            f::vetro_window_new_popup(parent.ptr.as_ptr(), anchor, width, height)
        })
    }

    /// A shell surface.
    pub fn shell(
        layer: Layer,
        edges: u32,
        exclusive: i32,
        keyboard: u32,
        namespace: &str,
        width: f32,
        height: f32,
    ) -> Result<Window> {
        let ns = cstring(namespace)?;
        let l = match layer {
            Layer::Background => sys::VETRO_LAYER_BACKGROUND,
            Layer::Bottom => sys::VETRO_LAYER_BOTTOM,
            Layer::Top => sys::VETRO_LAYER_TOP,
            Layer::Overlay => sys::VETRO_LAYER_OVERLAY,
        };
        // SAFETY: plain values and a string alive for the call.
        Window::from(unsafe {
            f::vetro_window_new_shell(l, edges, exclusive, keyboard, ns.as_ptr(), width, height)
        })
    }

    /// The raw handle.
    pub fn as_ptr(&self) -> *mut sys::VetroWindow {
        self.ptr.as_ptr()
    }

    /// Sets the title.
    pub fn set_title(&self, title: &str) -> Result<()> {
        let t = cstring(title)?;
        // SAFETY: a handle and a string alive for the call.
        check(unsafe { f::vetro_window_set_title(self.as_ptr(), t.as_ptr()) })
    }

    /// Sets the content view.
    pub fn set_content(&self, view: &View) -> Result<()> {
        // SAFETY: two handles.
        check(unsafe { f::vetro_window_set_content(self.as_ptr(), view.as_ptr()) })
    }

    /// The root view.
    pub fn root(&self) -> Option<View> {
        // SAFETY: a handle.
        View::borrowed(unsafe { f::vetro_window_root(self.as_ptr()) })
    }

    /// Shows it.
    pub fn show(&self) -> Result<()> {
        // SAFETY: a handle.
        check(unsafe { f::vetro_window_show(self.as_ptr()) })
    }

    /// Closes it.
    pub fn close(&self) -> Result<()> {
        // SAFETY: a handle.
        check(unsafe { f::vetro_window_close(self.as_ptr()) })
    }

    /// Resizes it.
    pub fn set_size(&self, w: f32, h: f32) -> Result<()> {
        // SAFETY: a handle and plain values.
        check(unsafe { f::vetro_window_set_size(self.as_ptr(), w, h) })
    }

    /// Minimum and maximum size (0: none).
    pub fn set_size_limits(&self, min: (f32, f32), max: (f32, f32)) -> Result<()> {
        // SAFETY: a handle and plain values.
        check(unsafe { f::vetro_window_set_size_limits(self.as_ptr(), min.0, min.1, max.0, max.1) })
    }

    /// Appearance: `None` system, `Some(true)` always dark, `Some(false)` always light.
    pub fn set_dark(&self, dark: Option<bool>) -> Result<()> {
        let v = match dark {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        };
        // SAFETY: a handle and plain values.
        check(unsafe { f::vetro_window_set_appearance(self.as_ptr(), v) })
    }

    /// Full-size content with a toolbar of `height` (0: Ossido's title bar).
    pub fn set_toolbar_height(&self, height: f32) -> Result<()> {
        // SAFETY: a handle and plain values.
        check(unsafe { f::vetro_window_set_toolbar_height(self.as_ptr(), height) })
    }

    /// Keeps the window open on a close request (the app handles `CLOSE_REQUESTED`).
    pub fn set_keep_on_close_request(&self, keep: bool) -> Result<()> {
        // SAFETY: a handle and plain values.
        check(unsafe { f::vetro_window_set_keep_on_close_request(self.as_ptr(), u8::from(keep)) })
    }

    /// A PNG of the window at `scale`.
    pub fn snapshot_png(&self, scale: f32) -> Result<Vec<u8>> {
        unsafe extern "C" fn keep(data: *const u8, len: usize, user: *mut c_void) {
            // SAFETY: `user` is the Vec below; `data` holds `len` bytes for the call.
            unsafe {
                let out = &mut *(user.cast::<Vec<u8>>());
                out.extend_from_slice(std::slice::from_raw_parts(data, len));
            }
        }
        let mut out: Vec<u8> = Vec::new();
        // SAFETY: `out` outlives the call.
        check(unsafe {
            f::vetro_window_snapshot_png(
                self.as_ptr(),
                scale,
                Some(keep),
                (&mut out as *mut Vec<u8>).cast(),
            )
        })?;
        Ok(out)
    }
}

// ----- Menus --------------------------------------------------------------------------------

/// A menu being built.
#[derive(Debug)]
pub struct Menu {
    ptr: *mut sys::VetroMenu,
}

/// Menu item flags.
pub mod menu_flags {
    use vetro_sys as sys;
    /// Disabled.
    pub const DISABLED: u32 = sys::VETRO_MENU_DISABLED;
    /// Checked.
    pub const CHECKED: u32 = sys::VETRO_MENU_CHECKED;
    /// Mixed.
    pub const MIXED: u32 = sys::VETRO_MENU_MIXED;
    /// Destructive.
    pub const DESTRUCTIVE: u32 = sys::VETRO_MENU_DESTRUCTIVE;
    /// A section heading.
    pub const HEADING: u32 = sys::VETRO_MENU_HEADING;
}

impl Menu {
    /// A menu titled `title` with an optional role (`app`, `window`, `help`).
    pub fn new(title: &str, role: Option<&str>) -> Result<Menu> {
        let t = cstring(title)?;
        let r = role.map(cstring).transpose()?;
        // SAFETY: strings alive for the call.
        let p = unsafe {
            f::vetro_menu_new(
                t.as_ptr(),
                r.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
            )
        };
        if p.is_null() {
            Err(last_error())
        } else {
            Ok(Menu { ptr: p })
        }
    }

    /// Adds an item with an id (and a key equivalent like `cmd+s`).
    pub fn item(self, title: &str, shortcut: Option<&str>, id: u32, flags: u32) -> Result<Menu> {
        let t = cstring(title)?;
        let s = shortcut.map(cstring).transpose()?;
        // SAFETY: the menu is ours; strings alive for the call.
        check(unsafe {
            f::vetro_menu_add_item(
                self.ptr,
                t.as_ptr(),
                s.as_ref().map_or(std::ptr::null(), |x| x.as_ptr()),
                id,
                flags,
            )
        })?;
        Ok(self)
    }

    /// Adds a separator.
    pub fn separator(self) -> Result<Menu> {
        // SAFETY: the menu is ours.
        check(unsafe { f::vetro_menu_add_separator(self.ptr) })?;
        Ok(self)
    }

    /// Adds a submenu.
    pub fn submenu(self, sub: Menu) -> Result<Menu> {
        let raw = sub.into_raw();
        // SAFETY: the submenu is consumed.
        check(unsafe { f::vetro_menu_add_submenu(self.ptr, raw) })?;
        Ok(self)
    }

    fn into_raw(self) -> *mut sys::VetroMenu {
        let p = self.ptr;
        std::mem::forget(self);
        p
    }
}

impl Drop for Menu {
    fn drop(&mut self) {
        // SAFETY: an unconsumed menu of ours.
        unsafe { f::vetro_menu_free(self.ptr) };
    }
}

/// Sets the app's menu bar; `chosen` gets the id of an activated item.
pub fn set_menu_bar(menus: Vec<Menu>, chosen: impl FnMut(u32) + 'static) -> Result<()> {
    type Cb = Box<dyn FnMut(u32)>;
    unsafe extern "C" fn call(id: u32, user: *mut c_void) {
        // SAFETY: a boxed `Cb` alive until destroy.
        let c = unsafe { &mut *(user.cast::<Cb>()) };
        c(id);
    }
    let raws: Vec<*mut sys::VetroMenu> = menus.into_iter().map(Menu::into_raw).collect();
    let data: Cb = Box::new(chosen);
    // SAFETY: the menus are consumed; the data freed by destroy once (it is shared by every
    // item and lives as long as the menu bar).
    check(unsafe {
        f::vetro_app_set_menu_bar(
            raws.as_ptr(),
            raws.len(),
            Some(call),
            boxed(data),
            Some(destroy::<Cb>),
        )
    })
}

/// A shared, mutable value for closures (a convenience for callbacks).
pub type Shared<T> = Rc<RefCell<T>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_signals_and_snapshot() {
        let h = Headless::new().unwrap();
        let w = Window::new(240.0, 120.0).unwrap();
        let col = View::new(Kind::COLUMN).unwrap();
        let b = View::new(Kind::PUSH_BUTTON).unwrap();
        b.set_str(Prop::TITLE, "Press").unwrap();
        assert_eq!(b.get_str(Prop::TITLE).unwrap(), "Press");
        assert_eq!(b.set_str(Prop(9999), "x"), Err(Error::UnknownProperty));
        col.add(&b).unwrap();
        assert_eq!(b.add(&col), Err(Error::BadHierarchy));
        assert_eq!(col.children().len(), 1);
        assert_eq!(b.parent().unwrap(), col);
        w.set_content(&col).unwrap();
        w.show().unwrap();
        let hits: Shared<u32> = Rc::default();
        let h2 = hits.clone();
        b.on(Signal::RESIZED, move |_, v| {
            assert!(matches!(v, Value::Size(..)));
            *h2.borrow_mut() += 1;
        })
        .unwrap();
        let png = w.snapshot_png(1.0).unwrap();
        assert_eq!(png.get(1..4), Some(&b"PNG"[..]));
        assert!(*hits.borrow() >= 1);
        let fired: Shared<bool> = Rc::default();
        let f2 = fired.clone();
        after(0.5, move || *f2.borrow_mut() = true).unwrap();
        h.advance(0.6);
        assert!(*fired.borrow());
        struct Dot;
        impl Custom for Dot {
            fn draw(&mut self, c: &Canvas, w: f32, h: f32) {
                c.fill_ellipse(
                    Rect {
                        x: 0.0,
                        y: 0.0,
                        width: w,
                        height: h,
                    },
                    rgba(0xff0000ff),
                );
            }
            fn measure(&mut self, _p: (Option<f32>, Option<f32>)) -> (f32, f32) {
                (10.0, 10.0)
            }
        }
        let dot = View::custom(Dot).unwrap();
        col.add(&dot).unwrap();
        let png = w.snapshot_png(1.0).unwrap();
        assert!(png.len() > 100);
        let menu = Menu::new("File", None)
            .unwrap()
            .item("New", Some("cmd+n"), 1, 0)
            .unwrap()
            .separator()
            .unwrap();
        set_menu_bar(vec![menu], |_| {}).unwrap();
        w.close().unwrap();
        assert_eq!(b.set_bool(Prop::HIDDEN, true), Err(Error::NoSuchObject));
    }
}
