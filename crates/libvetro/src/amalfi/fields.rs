//! Fields (kit `Fields`): the text field (plain, secure, with an error, with a trailing
//! button), the text area, the search field, the token field, the combo box, and the small
//! parts they share (token, clear button, combo button). Editing is [`Editor`]'s: the field
//! lays the text out, draws the caret and the selection, and turns pointer, keyboard and
//! input method events into edits.

use std::rc::Rc;

use crate::amalfi::foundation::{ControlSize, Label, focus_halo_named, whole};
use crate::amalfi::theme::Theme;
use crate::amalfi::tokens::{self, TypeStyle, typography};
use crate::color::Color;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled, Key};
use crate::core::layout::Proposal;
use crate::core::tree::ViewId;
use crate::core::ui::{Cursor, Request, Ui};
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::core::window::WindowId;
use crate::geom::{Point, Rect, Size};
use crate::shape::Shape;
use crate::style::{Border, BorderPosition, Style};
use crate::text::edit::{EditAction, Editor, Selection};
use crate::text::{Align, AttributedString, ParagraphOptions, TextLayout};

/// Fixed measures (silica_ui's `AmalfiFieldMetrics`, `AmalfiTextAreaMetrics`,
/// `AmalfiTokenMetrics`).
pub mod metrics {
    use crate::amalfi::foundation::ControlSize;

    /// Default width of a field.
    pub const WIDTH: f32 = 200.0;
    /// Gap between a field's parts.
    pub const GAP: f32 = 4.0;
    /// The border.
    pub const BORDER: f32 = 1.0;
    /// The border of a focused field or one with an error.
    pub const STRONG_BORDER: f32 = 1.5;
    /// The caret's width.
    pub const CARET_WIDTH: f32 = 1.5;
    /// The caret's height.
    pub const CARET_HEIGHT: f32 = 16.0;
    /// The error halo is the error colour at this opacity.
    pub const ERROR_HALO_OPACITY: f32 = 0.3;
    /// The clear button's side.
    pub const CLEAR_BUTTON: f32 = 20.0;
    /// The clear button's glyph.
    pub const CLEAR_GLYPH: f32 = 16.0;
    /// A value field's width.
    pub const VALUE_FIELD_WIDTH: f32 = 56.0;
    /// A text area's size.
    pub const AREA_SIZE: (f32, f32) = (240.0, 88.0);
    /// A text area's padding: left, top, right, bottom.
    pub const AREA_PADDING: (f32, f32, f32, f32) = (8.0, 8.0, 12.0, 8.0);
    /// A token field's width.
    pub const TOKEN_FIELD_WIDTH: f32 = 260.0;
    /// A token field's padding.
    pub const TOKEN_FIELD_LEFT: f32 = 4.0;
    /// A token field's padding at its end.
    pub const TOKEN_FIELD_RIGHT: f32 = 8.0;
    /// A token's height.
    pub const TOKEN_HEIGHT: f32 = 20.0;
    /// A token's padding at its start.
    pub const TOKEN_PAD_LEFT: f32 = 8.0;
    /// A token's padding at its end.
    pub const TOKEN_PAD_RIGHT: f32 = 4.0;
    /// A token's remove glyph.
    pub const TOKEN_REMOVE: f32 = 16.0;
    /// Text boxes are a multiple of this wide (the 4 pt grid).
    pub const STEP: f32 = 4.0;

    /// A field's height.
    pub fn height(size: ControlSize) -> f32 {
        match size {
            ControlSize::Mini | ControlSize::Small => 24.0,
            ControlSize::Regular => 28.0,
            ControlSize::Large => 32.0,
        }
    }

    /// A field's corner radius.
    pub fn radius(size: ControlSize) -> f32 {
        match size {
            ControlSize::Mini | ControlSize::Small => 7.0,
            ControlSize::Regular => 9.0,
            ControlSize::Large => 10.0,
        }
    }

    /// A field's horizontal padding.
    pub fn padding(size: ControlSize) -> f32 {
        match size {
            ControlSize::Mini | ControlSize::Small | ControlSize::Regular => 8.0,
            ControlSize::Large => 12.0,
        }
    }

    /// A field's glyph size.
    pub fn glyph(size: ControlSize) -> f32 {
        match size {
            ControlSize::Mini | ControlSize::Small => 16.0,
            ControlSize::Regular | ControlSize::Large => 20.0,
        }
    }

    /// The combo button's side.
    pub fn combo_button(size: ControlSize) -> f32 {
        match size {
            ControlSize::Mini | ControlSize::Small | ControlSize::Regular => 20.0,
            ControlSize::Large => 24.0,
        }
    }

    /// The combo button's chevron.
    pub const COMBO_GLYPH: f32 = 16.0;
}

fn text_token(size: ControlSize) -> &'static TypeStyle {
    match size {
        ControlSize::Mini | ControlSize::Small => &typography::CALLOUT,
        _ => &typography::BODY,
    }
}

/// What a field is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FieldKind {
    /// One line of text.
    #[default]
    Text,
    /// One line of bullets, with a lock.
    Secure,
    /// Several lines.
    Area,
    /// A glass capsule with a magnifier, tokens and a clear button.
    Search,
    /// Tokens, then the text.
    Tokens,
    /// A text field with a round button at its end that opens a list.
    Combo,
}

/// A text field of any [`FieldKind`].
pub struct TextField {
    /// What it is.
    pub kind: FieldKind,
    /// Size (Mini is Small).
    pub size: ControlSize,
    /// The placeholder.
    pub placeholder: String,
    /// The error message (the field shows the error look).
    pub error: Option<String>,
    /// The tokens (search and token fields).
    pub tokens: Vec<String>,
    /// The options of a combo box.
    pub options: Vec<String>,
    /// A value field (beside a stepper): 56 wide, Up and Down ask for a step.
    pub value_field: bool,
    editor: Editor,
    label: Label,
    layout: Option<Rc<TextLayout>>,
    scroll: Point,
    blink: f32,
    focused: bool,
    dragging: bool,
    selected_token: Option<usize>,
    clear_pressed: bool,
    combo_pressed: bool,
    open: bool,
    /// The popup that shows the list of a combo box while it is open.
    popup: Option<WindowId>,
    token_labels: Vec<Label>,
    last_text_rect: Rect,
}

impl std::fmt::Debug for TextField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextField")
            .field("kind", &self.kind)
            .field("text", &self.editor.text())
            .finish()
    }
}

impl TextField {
    /// A field of `kind`.
    pub fn new(kind: FieldKind) -> TextField {
        TextField {
            kind,
            size: ControlSize::Regular,
            placeholder: String::new(),
            error: None,
            tokens: Vec::new(),
            options: Vec::new(),
            value_field: false,
            editor: Editor::new("", kind != FieldKind::Area),
            label: Label::default(),
            layout: None,
            scroll: Point::ZERO,
            blink: 0.0,
            focused: false,
            dragging: false,
            selected_token: None,
            clear_pressed: false,
            combo_pressed: false,
            open: false,
            popup: None,
            token_labels: Vec::new(),
            last_text_rect: Rect::ZERO,
        }
    }

    /// A text field.
    pub fn text() -> TextField {
        TextField::new(FieldKind::Text)
    }

    /// A value field (the number beside a stepper).
    pub fn value() -> TextField {
        let mut f = TextField::new(FieldKind::Text);
        f.value_field = true;
        f
    }

    /// A secure field.
    pub fn secure() -> TextField {
        TextField::new(FieldKind::Secure)
    }

    /// A text area.
    pub fn area() -> TextField {
        TextField::new(FieldKind::Area)
    }

    /// A search field.
    pub fn search() -> TextField {
        let mut f = TextField::new(FieldKind::Search);
        f.placeholder = "Search".to_string();
        f
    }

    /// A token field.
    pub fn token_field() -> TextField {
        TextField::new(FieldKind::Tokens)
    }

    /// A combo box.
    pub fn combo(options: &[&str]) -> TextField {
        let mut f = TextField::new(FieldKind::Combo);
        f.options = options.iter().map(|o| (*o).to_string()).collect();
        f
    }

    /// With text.
    pub fn with_text(mut self, text: &str) -> TextField {
        self.editor.set_text(text);
        self
    }

    /// With a placeholder.
    pub fn with_placeholder(mut self, p: &str) -> TextField {
        self.placeholder = p.to_string();
        self
    }

    /// With a size.
    pub fn with_size(mut self, size: ControlSize) -> TextField {
        self.size = size;
        self
    }

    /// With an error.
    pub fn with_error(mut self, message: &str) -> TextField {
        self.error = Some(message.to_string());
        self
    }

    /// With tokens.
    pub fn with_tokens(mut self, tokens: &[&str]) -> TextField {
        self.tokens = tokens.iter().map(|t| (*t).to_string()).collect();
        self
    }

    /// The text.
    pub fn content(&self) -> &str {
        self.editor.text()
    }

    // ----- Geometry ---------------------------------------------------------------------

    fn height(&self) -> f32 {
        if self.kind == FieldKind::Area {
            metrics::AREA_SIZE.1
        } else {
            metrics::height(self.size)
        }
    }

    fn natural_width(&self) -> f32 {
        match self.kind {
            FieldKind::Area => metrics::AREA_SIZE.0,
            FieldKind::Tokens => metrics::TOKEN_FIELD_WIDTH,
            _ if self.value_field => metrics::VALUE_FIELD_WIDTH,
            _ => metrics::WIDTH,
        }
    }

    fn radius(&self) -> f32 {
        match self.kind {
            FieldKind::Search => self.height() / 2.0,
            FieldKind::Area => metrics::radius(ControlSize::Regular),
            _ => metrics::radius(self.size),
        }
    }

    fn pad(&self) -> f32 {
        match self.kind {
            FieldKind::Tokens => metrics::TOKEN_FIELD_LEFT,
            _ => metrics::padding(self.size),
        }
    }

    fn token_style(&self, theme: &Theme) -> crate::text::TextStyle {
        theme.text(text_token(self.size), theme.colors.text_primary)
    }

    fn token_width(&mut self, theme: &Theme, i: usize) -> f32 {
        let Some(t) = self.tokens.get(i).cloned() else {
            return 0.0;
        };
        if self.token_labels.len() <= i {
            self.token_labels.resize_with(i + 1, Label::default);
        }
        let style = theme.text(&typography::LABEL, theme.colors.text_primary);
        let w = match self.token_labels.get_mut(i) {
            Some(l) => whole(l.width(&t, &style), metrics::STEP),
            None => 0.0,
        };
        metrics::TOKEN_PAD_LEFT + w + metrics::TOKEN_REMOVE + metrics::TOKEN_PAD_RIGHT
    }

    /// The row's parts for a field of width `w`, with a trailing child `child_w` wide.
    fn row(&mut self, theme: &Theme, w: f32, child_w: f32) -> Row {
        let h = self.height();
        let glyph = metrics::glyph(self.size);
        let pad = self.pad();
        let mut row = Row::default();
        if self.kind == FieldKind::Area {
            let (l, t, r, b) = metrics::AREA_PADDING;
            row.text = Rect::new(l, t, (w - l - r).max(0.0), (h - t - b).max(0.0));
            return row;
        }
        let mut x = pad;
        match self.kind {
            FieldKind::Secure | FieldKind::Search => {
                row.leading = Some(Rect::new(x, (h - glyph) / 2.0, glyph, glyph));
                x += glyph + metrics::GAP;
            }
            _ => {}
        }
        if matches!(self.kind, FieldKind::Search | FieldKind::Tokens) {
            for i in 0..self.tokens.len() {
                let tw = self.token_width(theme, i);
                row.tokens.push(Rect::new(
                    x,
                    (h - metrics::TOKEN_HEIGHT) / 2.0,
                    tw,
                    metrics::TOKEN_HEIGHT,
                ));
                x += tw + metrics::GAP;
            }
        }
        let end_pad = if self.kind == FieldKind::Tokens {
            metrics::TOKEN_FIELD_RIGHT
        } else {
            pad
        };
        let mut end = w - end_pad;
        if child_w > 0.0 {
            row.child = Some(Rect::new(end - child_w, 0.0, child_w, h));
            end -= child_w + metrics::GAP;
        }
        if self.kind == FieldKind::Combo {
            let side = metrics::combo_button(self.size);
            row.combo = Some(Rect::new(end - side, (h - side) / 2.0, side, side));
            end -= side + metrics::GAP;
        }
        if self.error.is_some() && self.kind != FieldKind::Search {
            row.error = Some(Rect::new(end - glyph, (h - glyph) / 2.0, glyph, glyph));
            end -= glyph + metrics::GAP;
        }
        if self.kind == FieldKind::Search && !self.editor.text().is_empty() {
            let side = metrics::CLEAR_BUTTON;
            row.clear = Some(Rect::new(end - side, (h - side) / 2.0, side, side));
            end -= side + metrics::GAP;
        }
        let style = self.token_style(theme);
        let lh = style.line_height.unwrap_or(18.0);
        row.text = Rect::new(x, (h - lh) / 2.0, (end - x).max(0.0), lh);
        row
    }

    // ----- Text -------------------------------------------------------------------------

    fn is_secure(&self) -> bool {
        self.kind == FieldKind::Secure
    }

    /// The displayed string and the map of editor offsets to displayed ones.
    fn display(&self) -> (String, Option<std::ops::Range<usize>>) {
        let (text, pre) = self.editor.display_text();
        if !self.is_secure() {
            return (text, pre);
        }
        let n = unicode_segmentation::UnicodeSegmentation::graphemes(text.as_str(), true).count();
        ("\u{2022}".repeat(n), None)
    }

    fn to_display(&self, at: usize) -> usize {
        if !self.is_secure() {
            return at;
        }
        let text = self.editor.text();
        let upto = text.get(..at.min(text.len())).unwrap_or_default();
        unicode_segmentation::UnicodeSegmentation::graphemes(upto, true).count()
            * '\u{2022}'.len_utf8()
    }

    fn editor_offset(&self, at: usize) -> usize {
        if !self.is_secure() {
            return at;
        }
        let bullets = at / '\u{2022}'.len_utf8();
        let text = self.editor.text();
        let mut offset = 0;
        for (n, (i, g)) in
            unicode_segmentation::UnicodeSegmentation::grapheme_indices(text, true).enumerate()
        {
            if n == bullets {
                return i;
            }
            offset = i + g.len();
        }
        offset
    }

    fn build_layout(&mut self, theme: &Theme, width: f32) -> Rc<TextLayout> {
        let (shown, pre) = self.display();
        let style = self.token_style(theme);
        let multi = self.kind == FieldKind::Area;
        let mut s = AttributedString::plain(shown);
        if let Some(r) = pre {
            let mut u = style.clone();
            u.underline = true;
            s.set(r, u, None);
        }
        let layout = Rc::new(TextLayout::new(
            &s,
            &style,
            &ParagraphOptions {
                align: Align::Start,
                max_lines: if multi { None } else { Some(1) },
                ellipsis: false,
                ..ParagraphOptions::default()
            },
            if multi { width.max(1.0) } else { f32::INFINITY },
        ));
        self.layout = Some(layout.clone());
        layout
    }

    fn caret_in_layout(&self, layout: &TextLayout) -> Rect {
        let at = self.editor.selection().focus;
        let at = match self.editor.preedit() {
            Some(p) if !p.text.is_empty() => {
                at.min(self.editor.selection().anchor)
                    + p.cursor.map_or(p.text.len(), |(_, e)| e.min(p.text.len()))
            }
            _ => self.to_display(at),
        };
        layout.caret_rect(at, false)
    }

    fn ensure_visible(&mut self, layout: &TextLayout, text: Rect) {
        let caret = self.caret_in_layout(layout);
        if self.kind == FieldKind::Area {
            let top = self.scroll.y;
            if caret.y < top {
                self.scroll.y = caret.y;
            } else if caret.y + caret.height > top + text.height {
                self.scroll.y = caret.y + caret.height - text.height;
            }
            self.scroll.y = self.scroll.y.max(0.0);
            self.scroll.x = 0.0;
            return;
        }
        let total = layout.max_intrinsic_width();
        let x = self.scroll.x;
        if caret.x < x {
            self.scroll.x = caret.x;
        } else if caret.x + 1.0 > x + text.width {
            self.scroll.x = caret.x + 1.0 - text.width;
        }
        self.scroll.x = self
            .scroll
            .x
            .clamp(0.0, (total + 1.0 - text.width).max(0.0));
        self.scroll.y = 0.0;
    }

    fn caret_visible(&self) -> bool {
        self.focused && (self.blink % 1.0) < 0.5
    }

    fn changed(&mut self, cx: &mut EventCx<'_>) {
        self.blink = 0.0;
        cx.emit(
            Signal::TextChanged,
            Value::Str(self.editor.text().to_string()),
        );
        self.report_caret(cx);
        cx.request_layout();
        cx.request_paint();
    }

    fn report_caret(&mut self, cx: &mut EventCx<'_>) {
        if !self.focused {
            return;
        }
        let Some(window) = cx.window() else { return };
        let Some(layout) = self.layout.clone() else {
            return;
        };
        let caret = self.caret_in_layout(&layout);
        let o = cx.window_rect().origin();
        let text = self.last_text_rect;
        let r = Rect::new(
            o.x + text.x + caret.x - self.scroll.x,
            o.y + text.y + caret.y - self.scroll.y,
            1.0,
            caret.height,
        );
        cx.request(Request::TextInput(window, Some(r)));
        // The surrounding text (not for a secure field): the whole text of a line field.
        if self.kind != FieldKind::Secure {
            let sel = self.editor.selection();
            let text = self.editor.text().to_string();
            let c = sel.focus.min(text.len());
            let a = sel.anchor.min(text.len());
            cx.request(Request::TextInputSurrounding(window, text, c, a));
        }
    }

    fn offset_at(&self, local: Point) -> usize {
        let Some(layout) = &self.layout else { return 0 };
        let t = self.last_text_rect;
        let p = Point::new(local.x - t.x + self.scroll.x, local.y - t.y + self.scroll.y);
        let p = if self.kind == FieldKind::Area {
            p
        } else {
            Point::new(p.x, p.y.clamp(0.0, layout.size().height.max(1.0) - 0.5))
        };
        self.editor_offset(layout.offset_at(p).0)
    }

    fn commit_token(&mut self, cx: &mut EventCx<'_>) {
        let text = self.editor.text().trim().to_string();
        if text.is_empty() {
            return;
        }
        self.editor.set_text("");
        self.tokens.push(text);
        cx.emit(Signal::ValueChanged, Value::StrList(self.tokens.clone()));
        self.changed(cx);
    }

    fn remove_token(&mut self, cx: &mut EventCx<'_>, i: usize) {
        if i >= self.tokens.len() {
            return;
        }
        self.tokens.remove(i);
        self.selected_token = None;
        cx.emit(Signal::ValueChanged, Value::StrList(self.tokens.clone()));
        cx.request_layout();
        cx.request_paint();
    }

    fn clear(&mut self, cx: &mut EventCx<'_>) {
        let had_tokens = !self.tokens.is_empty();
        self.editor.set_text("");
        self.tokens.clear();
        self.selected_token = None;
        if had_tokens {
            cx.emit(Signal::ValueChanged, Value::StrList(Vec::new()));
        }
        self.changed(cx);
    }

    fn key(&mut self, cx: &mut EventCx<'_>, k: &crate::core::event::KeyEvent) -> Handled {
        let now = cx.now();
        let tokens_here = matches!(self.kind, FieldKind::Search | FieldKind::Tokens);
        // Tokens before the text: Backspace at the start selects the last one, again removes
        // it; any other key deselects it.
        if tokens_here && k.pressed {
            match (&k.key, self.selected_token) {
                (Key::Backspace | Key::Delete, Some(i)) => {
                    self.remove_token(cx, i);
                    return Handled::Yes;
                }
                (Key::Backspace, None)
                    if self.editor.selection() == Selection::caret(0)
                        && !self.tokens.is_empty() =>
                {
                    self.selected_token = Some(self.tokens.len() - 1);
                    cx.request_paint();
                    return Handled::Yes;
                }
                (Key::Modifier, _) => {}
                (_, Some(_)) => {
                    self.selected_token = None;
                    cx.request_paint();
                }
                _ => {}
            }
        }
        if self.kind == FieldKind::Search
            && k.pressed
            && k.key == Key::Escape
            && (!self.editor.text().is_empty() || !self.tokens.is_empty())
        {
            self.clear(cx);
            return Handled::Yes;
        }
        if self.value_field && k.pressed && matches!(k.key, Key::Up | Key::Down) {
            let by = if k.key == Key::Up { 1 } else { -1 };
            cx.emit(Signal::Activated, Value::Int(by));
            return Handled::Yes;
        }
        if self.kind == FieldKind::Combo && k.pressed {
            match k.key {
                Key::Down | Key::Up => {
                    self.set_open(cx, true);
                    return Handled::Yes;
                }
                Key::Escape if self.open => {
                    self.set_open(cx, false);
                    return Handled::Yes;
                }
                _ => {}
            }
        }
        let layout = self.layout.clone();
        let before = self.editor.text().to_string();
        let sel = self.editor.selection();
        let action = self.editor.handle_key(k, layout.as_deref(), now);
        match action {
            EditAction::Unhandled => Handled::No,
            EditAction::Handled => {
                if self.editor.text() != before {
                    self.changed(cx);
                } else if self.editor.selection() != sel {
                    self.blink = 0.0;
                    self.report_caret(cx);
                    cx.request_paint();
                }
                Handled::Yes
            }
            EditAction::Copy(text) => {
                cx.set_clipboard(text);
                if self.editor.text() != before {
                    self.changed(cx);
                }
                Handled::Yes
            }
            EditAction::Paste => {
                cx.request_paste();
                Handled::Yes
            }
            EditAction::Submit => {
                if self.kind == FieldKind::Tokens {
                    self.commit_token(cx);
                }
                cx.emit(
                    Signal::Submitted,
                    Value::Str(self.editor.text().to_string()),
                );
                Handled::Yes
            }
            EditAction::Cancel => Handled::No,
        }
    }

    fn set_open(&mut self, cx: &mut EventCx<'_>, open: bool) {
        if open == self.open || self.options.is_empty() && open {
            return;
        }
        self.open = open;
        if open {
            self.open_list(cx);
        } else if let Some(w) = self.popup.take() {
            cx.ui().close_window(w);
        }
        cx.emit(Signal::ExpandedChanged, Value::Bool(open));
        cx.request_paint();
    }

    /// Shows the options as a menu under the field, as wide as it, the current one checked.
    fn open_list(&mut self, cx: &mut EventCx<'_>) {
        let id = cx.id();
        let Some(parent) = cx.window() else {
            return;
        };
        let anchor = cx.window_rect();
        let current = self.selected_option();
        let rows: Vec<crate::amalfi::menus::Row> = self
            .options
            .iter()
            .enumerate()
            .map(|(i, o)| crate::amalfi::menus::Row {
                checked: current == Some(i),
                ..crate::amalfi::menus::Row::item(o)
            })
            .collect();
        let ui = cx.ui();
        let (m, _) = crate::amalfi::menus::menu(ui, &rows);
        ui.connect(m, Signal::Activated, move |ui, _, v| {
            if let Some(i) = v.as_int().and_then(|i| usize::try_from(i).ok()) {
                TextField::choose_in(ui, id, i);
            }
        });
        ui.connect(m, Signal::CloseRequested, move |ui, _, _| {
            TextField::dismiss(ui, id);
        });
        let w = crate::amalfi::menus::present(
            ui,
            parent,
            anchor,
            m,
            crate::amalfi::menus::Placement::List,
        );
        self.popup = Some(w);
        ui.set_focus(m, true);
    }

    /// The list of the combo box `id` chose option `i`: its text becomes the option and the
    /// list closes.
    pub fn choose_in(ui: &mut Ui, id: ViewId, i: usize) {
        let mut closing = None;
        let chosen = ui
            .with_widget::<TextField, _>(id, |f| {
                let option = f.options.get(i).cloned()?;
                f.open = false;
                closing = f.popup.take();
                Some(option)
            })
            .flatten();
        if let Some(w) = closing {
            ui.close_window(w);
        }
        if let Some(option) = chosen {
            let _ = ui.set(id, Prop::Text, Value::Str(option.clone()));
            ui.emit(id, Signal::SelectionChanged, Value::Str(option.clone()));
            ui.emit(id, Signal::TextChanged, Value::Str(option));
            ui.emit(id, Signal::ExpandedChanged, Value::Bool(false));
        }
    }

    /// Closes the list of the combo box `id` without a choice.
    pub fn dismiss(ui: &mut Ui, id: ViewId) {
        let mut closing = None;
        ui.with_widget::<TextField, _>(id, |f| {
            f.open = false;
            closing = f.popup.take();
        });
        if let Some(w) = closing {
            ui.close_window(w);
            ui.emit(id, Signal::ExpandedChanged, Value::Bool(false));
        }
    }

    /// Whether the editor's text is one of the options (the one checked in the list).
    pub fn selected_option(&self) -> Option<usize> {
        self.options.iter().position(|o| o == self.editor.text())
    }

    fn colors_for(&self, theme: &Theme, enabled: bool) -> (Color, f32, bool, bool) {
        let c = &theme.colors;
        let error = enabled && self.error.is_some();
        let strong = enabled && (error || self.focused);
        let border = if !strong {
            c.line_border
        } else if error {
            c.palette_terracotta
        } else {
            c.accent_amalfi
        };
        let width = if strong && !self.value_field {
            metrics::STRONG_BORDER
        } else {
            metrics::BORDER
        };
        (border, width, strong, error)
    }
}

/// The parts of a field's row, in the field's coordinates.
#[derive(Debug, Default, Clone)]
struct Row {
    leading: Option<Rect>,
    tokens: Vec<Rect>,
    text: Rect,
    error: Option<Rect>,
    clear: Option<Rect>,
    combo: Option<Rect>,
    child: Option<Rect>,
}

/// Paints a token at `rect`: `Background`, `Label`, `Remove` under `prefix`.
fn paint_token(
    cx: &mut PaintCx<'_>,
    cache: &mut Label,
    prefix: &str,
    rect: Rect,
    label: &str,
    selected: bool,
) {
    let theme = *cx.theme();
    let c = theme.colors;
    let ink = if selected {
        c.text_on_accent
    } else {
        c.accent_amalfi
    };
    cx.rec.layer(
        &format!("{prefix}Background"),
        rect,
        Shape::rounded(metrics::TOKEN_HEIGHT / 2.0),
        Style::fill(if selected {
            c.accent_amalfi
        } else {
            c.accent_amalfi_soft
        }),
    );
    let style = theme.text(&typography::LABEL, ink);
    let lh = style.line_height.unwrap_or(14.0);
    let w = whole(cache.width(label, &style), metrics::STEP);
    let layout = cache.layout(label, &style, Align::Start, w, Some(1));
    cx.rec.text(
        &format!("{prefix}Label"),
        Rect::new(
            rect.x + metrics::TOKEN_PAD_LEFT,
            rect.y + (rect.height - lh) / 2.0,
            w,
            lh,
        ),
        layout,
        Some(typography::LABEL.name),
        ink,
    );
    cx.rec.glyph(
        &format!("{prefix}Remove"),
        Rect::new(
            rect.x + metrics::TOKEN_PAD_LEFT + w,
            rect.y + (rect.height - metrics::TOKEN_REMOVE) / 2.0,
            metrics::TOKEN_REMOVE,
            metrics::TOKEN_REMOVE,
        ),
        "xmark",
        400,
        ink,
    );
}

fn paint_clear(cx: &mut PaintCx<'_>, prefix: &str, rect: Rect, pressed: bool) {
    let c = cx.theme().colors;
    cx.rec.layer(
        &format!("{prefix}Disc"),
        rect,
        Shape::rounded(metrics::CLEAR_BUTTON / 2.0),
        Style::fill(if pressed {
            c.text_secondary
        } else {
            c.text_tertiary
        }),
    );
    cx.rec.glyph(
        &format!("{prefix}Clear glyph"),
        Rect::new(
            rect.x + (rect.width - metrics::CLEAR_GLYPH) / 2.0,
            rect.y + (rect.height - metrics::CLEAR_GLYPH) / 2.0,
            metrics::CLEAR_GLYPH,
            metrics::CLEAR_GLYPH,
        ),
        "xmark",
        400,
        c.surface_window,
    );
}

fn paint_combo(cx: &mut PaintCx<'_>, prefix: &str, rect: Rect, pressed: bool) {
    let c = cx.theme().colors;
    cx.rec.layer(
        &format!("{prefix}Disc"),
        rect,
        Shape::rounded(rect.width / 2.0),
        Style::fill(if pressed {
            c.surface_control_pressed
        } else {
            c.surface_sunken
        }),
    );
    cx.rec.glyph(
        &format!("{prefix}Chevron"),
        Rect::new(
            rect.x + (rect.width - metrics::COMBO_GLYPH) / 2.0,
            rect.y + (rect.height - metrics::COMBO_GLYPH) / 2.0,
            metrics::COMBO_GLYPH,
            metrics::COMBO_GLYPH,
        ),
        "chevron-down",
        400,
        c.text_secondary,
    );
}

impl Widget for TextField {
    fn kind(&self) -> &'static str {
        match self.kind {
            FieldKind::Text => "TextField",
            FieldKind::Secure => "SecureField",
            FieldKind::Area => "TextArea",
            FieldKind::Search => "SearchField",
            FieldKind::Tokens => "TokenField",
            FieldKind::Combo => "ComboBox",
        }
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let h = self.height();
        let w = self.natural_width();
        for c in cx.children() {
            let _ = cx.measure(c, Proposal::new(None, Some(h)));
        }
        Size::new(w, h)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let theme = cx.theme();
        if let Some(child) = cx.children().first().copied() {
            let cs = cx.measure(child, Proposal::new(None, Some(size.height)));
            let row = self.row(&theme, size.width, cs.width);
            let x = row.child.map_or(0.0, |r| r.x);
            cx.place(
                child,
                Rect::new(x, (size.height - cs.height) / 2.0, cs.width, cs.height),
            );
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let b = cx.bounds();
        let o = Point::new(b.x, b.y);
        let enabled = !cx.is_disabled();
        let child_w = cx
            .ui
            .tree
            .children(cx.id)
            .first()
            .and_then(|ch| cx.ui.tree.get(*ch))
            .map_or(0.0, |n| n.frame.width);
        let row = self.row(&theme, b.width, child_w);
        let h = self.height();
        let field = Rect::new(b.x, b.y, b.width, h);
        let radius = self.radius();
        let shape = Shape::rounded(radius);
        let (border, border_w, strong, error) = self.colors_for(&theme, enabled);

        // Background and halo.
        if self.kind == FieldKind::Search {
            cx.rec.layer(
                "Background",
                field,
                shape.clone(),
                theme.material_style("Glass/Regular"),
            );
            if self.focused && enabled {
                focus_halo_named(cx.rec, "Focus Halo", field, shape, &theme);
            }
        } else {
            let mut style = Style::fill(c.surface_raised);
            style.borders.push(Border::inside(border, border_w));
            cx.rec.layer("Background", field, shape.clone(), style);
            if strong {
                let halo_color = if error {
                    c.palette_terracotta
                } else {
                    c.line_focus
                };
                let mut halo = Style::default().with_border(Border::at(
                    halo_color,
                    tokens::focus::HALO,
                    BorderPosition::Outside,
                ));
                if error {
                    halo.opacity = metrics::ERROR_HALO_OPACITY;
                }
                cx.rec.layer("Focus Halo", field, shape, halo);
            }
        }

        // The text.
        let text_rect = Rect::new(
            b.x + row.text.x,
            b.y + row.text.y,
            row.text.width,
            row.text.height,
        );
        self.last_text_rect = Rect::new(row.text.x, row.text.y, row.text.width, row.text.height);
        let style = self.token_style(&theme);
        let lh = style.line_height.unwrap_or(18.0);
        let empty = self.editor.text().is_empty() && self.editor.preedit().is_none();
        if empty {
            let ph = theme.text(text_token(self.size), c.text_tertiary);
            let hide = !self.tokens.is_empty() && self.kind == FieldKind::Search;
            if !self.placeholder.is_empty() && !hide {
                let layout = self.label.layout(
                    &self.placeholder,
                    &ph,
                    Align::Start,
                    text_rect.width,
                    if self.kind == FieldKind::Area {
                        None
                    } else {
                        Some(1)
                    },
                );
                let height = if self.kind == FieldKind::Area {
                    layout.size().height.max(lh)
                } else {
                    lh
                };
                cx.rec.text(
                    "Value",
                    Rect::new(text_rect.x, text_rect.y, text_rect.width, height),
                    layout,
                    Some(text_token(self.size).name),
                    ph.color,
                );
            }
            self.layout = None;
            if self.focused {
                // The caret at the start of the empty text.
                self.layout = Some(self.build_layout(&theme, text_rect.width));
            }
        } else {
            let layout = self.build_layout(&theme, text_rect.width);
            self.ensure_visible(&layout, row.text);
            let height = if self.kind == FieldKind::Area {
                layout.size().height.max(lh)
            } else {
                lh
            };
            cx.rec.push_clip(text_rect, Shape::RECT);
            // The selection under the text.
            let sel = self.editor.selection();
            if !sel.is_collapsed() && self.editor.preedit().is_none() {
                let r = self.to_display(sel.range().start)..self.to_display(sel.range().end);
                for rect in layout.selection_rects(r) {
                    cx.rec.layer(
                        "Selection",
                        Rect::new(
                            text_rect.x + rect.x - self.scroll.x,
                            text_rect.y + rect.y - self.scroll.y,
                            rect.width,
                            rect.height,
                        ),
                        Shape::RECT,
                        Style::fill(c.accent_amalfi_soft),
                    );
                }
            }
            cx.rec.text(
                "Value",
                Rect::new(
                    text_rect.x - self.scroll.x,
                    text_rect.y - self.scroll.y,
                    if self.kind == FieldKind::Area {
                        text_rect.width
                    } else {
                        text_rect.width.max(layout.max_intrinsic_width())
                    },
                    height,
                ),
                layout,
                Some(text_token(self.size).name),
                style.color,
            );
            cx.rec.pop();
        }

        // The caret.
        if self.caret_visible()
            && enabled
            && let Some(layout) = self.layout.clone()
        {
            let caret = self.caret_in_layout(&layout);
            let (cy, ch) = if self.kind == FieldKind::Area {
                (
                    caret.y + (caret.height - metrics::CARET_HEIGHT) / 2.0 - self.scroll.y,
                    metrics::CARET_HEIGHT,
                )
            } else {
                (
                    (row.text.height - metrics::CARET_HEIGHT) / 2.0,
                    metrics::CARET_HEIGHT,
                )
            };
            cx.rec.layer(
                "Caret",
                Rect::new(
                    text_rect.x + caret.x - self.scroll.x,
                    text_rect.y + cy,
                    metrics::CARET_WIDTH,
                    ch,
                ),
                Shape::RECT,
                Style::fill(c.accent_amalfi),
            );
        }

        // The glyphs and the parts.
        let glyph = metrics::glyph(self.size);
        if let Some(r) = row.leading {
            let (name, symbol) = if self.kind == FieldKind::Search {
                ("Search glyph", "search")
            } else {
                ("Leading icon", "lock")
            };
            cx.rec.glyph_sized(
                name,
                r.translate(o).inflate(0.0, 0.0),
                glyph,
                symbol,
                400,
                c.text_secondary,
            );
        }
        if let Some(r) = row.error {
            cx.rec.glyph_sized(
                "Error icon",
                r.translate(o),
                glyph,
                "warning",
                400,
                c.palette_terracotta,
            );
        }
        let single = self.tokens.len() == 1 && self.kind == FieldKind::Search;
        for (i, r) in row.tokens.iter().enumerate() {
            let prefix = if single {
                "Token/".to_string()
            } else {
                format!("Token {}/", i + 1)
            };
            let label = self.tokens.get(i).cloned().unwrap_or_default();
            if self.token_labels.len() <= i {
                self.token_labels.resize_with(i + 1, Label::default);
            }
            if let Some(cache) = self.token_labels.get_mut(i) {
                paint_token(
                    cx,
                    cache,
                    &prefix,
                    r.translate(o),
                    &label,
                    self.selected_token == Some(i),
                );
            }
        }
        if let Some(r) = row.clear {
            paint_clear(cx, "Clear button/", r.translate(o), self.clear_pressed);
        }
        if let Some(r) = row.combo {
            paint_combo(
                cx,
                "Inline button/",
                r.translate(o),
                self.combo_pressed || self.open,
            );
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let theme = cx.theme();
        let origin = cx.bounds().origin();
        let id = cx.id();
        let enabled = !cx.ui().is_disabled(id);
        if !enabled {
            return Handled::No;
        }
        let local = |p: &Point| Point::new(p.x - origin.x, p.y - origin.y);
        let child_w = {
            let kids = cx.ui().tree.children(id).to_vec();
            kids.first()
                .and_then(|c| cx.ui().tree.get(*c))
                .map_or(0.0, |n| n.frame.width)
        };
        let w = cx.size().width;
        let row = self.row(&theme, w, child_w);
        match event {
            Event::FocusIn => {
                self.focused = true;
                self.blink = 0.0;
                cx.emit(Signal::EditingBegan, Value::None);
                cx.animate();
                cx.request_paint();
                Handled::Yes
            }
            Event::FocusOut => {
                self.focused = false;
                self.dragging = false;
                self.selected_token = None;
                self.editor.set_preedit("", None);
                if let Some(window) = cx.window() {
                    cx.request(Request::TextInput(window, None));
                }
                if self.open {
                    self.open = false;
                    cx.emit(Signal::ExpandedChanged, Value::Bool(false));
                }
                cx.emit(
                    Signal::EditingEnded,
                    Value::Str(self.editor.text().to_string()),
                );
                cx.request_paint();
                Handled::Yes
            }
            Event::PointerEnter
            | Event::PointerMove {
                dragging: false, ..
            } => {
                cx.set_cursor(Cursor::Text);
                Handled::No
            }
            Event::PointerDown {
                pos,
                button: Button::Left,
                clicks,
                mods,
            } => {
                let p = local(pos);
                if let Some(r) = row.clear.filter(|r| r.contains(p)) {
                    let _ = r;
                    self.clear_pressed = true;
                    cx.request_paint();
                    return Handled::Yes;
                }
                if row.combo.is_some_and(|r| r.contains(p)) {
                    self.combo_pressed = true;
                    cx.request_paint();
                    return Handled::Yes;
                }
                for (i, r) in row.tokens.iter().enumerate() {
                    if r.contains(p) {
                        let remove = Rect::new(
                            r.right() - metrics::TOKEN_PAD_RIGHT - metrics::TOKEN_REMOVE,
                            r.y,
                            metrics::TOKEN_REMOVE + metrics::TOKEN_PAD_RIGHT,
                            r.height,
                        );
                        if remove.contains(p) {
                            self.remove_token(cx, i);
                        } else {
                            self.selected_token = Some(i);
                            cx.request_paint();
                        }
                        return Handled::Yes;
                    }
                }
                self.selected_token = None;
                let at = if row.text.contains(p) || self.kind == FieldKind::Area {
                    self.offset_at(p)
                } else if p.x < row.text.x {
                    0
                } else {
                    self.editor.text().len()
                };
                let shift = mods.contains(crate::core::event::Mods::SHIFT);
                match clicks {
                    2 => self.editor.select_word_at(at),
                    n if *n >= 3 => self.editor.select_paragraph_at(at),
                    _ => {
                        let anchor = if shift {
                            self.editor.selection().anchor
                        } else {
                            at
                        };
                        self.editor.set_selection(Selection { anchor, focus: at });
                    }
                }
                self.dragging = true;
                self.blink = 0.0;
                self.report_caret(cx);
                cx.request_paint();
                Handled::Yes
            }
            Event::PointerMove {
                pos,
                dragging: true,
                ..
            } if self.dragging => {
                let at = self.offset_at(local(pos));
                let anchor = self.editor.selection().anchor;
                self.editor.set_selection(Selection { anchor, focus: at });
                self.report_caret(cx);
                cx.request_paint();
                Handled::Yes
            }
            Event::PointerUp {
                pos,
                button: Button::Left,
                ..
            } => {
                self.dragging = false;
                let p = local(pos);
                if std::mem::take(&mut self.clear_pressed) {
                    if row.clear.is_some_and(|r| r.contains(p)) {
                        self.clear(cx);
                    }
                    cx.request_paint();
                }
                if std::mem::take(&mut self.combo_pressed) {
                    if row.combo.is_some_and(|r| r.contains(p)) {
                        let open = !self.open;
                        self.set_open(cx, open);
                        if !self.focused {
                            cx.focus_self();
                        }
                    }
                    cx.request_paint();
                }
                Handled::Yes
            }
            Event::Key(k) => self.key(cx, k),
            Event::Text(t) => {
                if self.kind == FieldKind::Tokens && t.contains([',', ';']) {
                    let before: String = t.chars().take_while(|c| *c != ',' && *c != ';').collect();
                    let now = cx.now();
                    self.editor.insert(&before, now);
                    self.commit_token(cx);
                    return Handled::Yes;
                }
                let now = cx.now();
                self.editor.insert(t, now);
                self.changed(cx);
                Handled::Yes
            }
            Event::Paste(t) => {
                let now = cx.now();
                self.editor.insert(t, now);
                self.changed(cx);
                Handled::Yes
            }
            Event::Preedit { text, cursor } => {
                self.editor.set_preedit(text, *cursor);
                self.blink = 0.0;
                cx.request_layout();
                cx.request_paint();
                Handled::Yes
            }
            Event::DeleteSurrounding { before, after } => {
                let now = cx.now();
                self.editor.delete_surrounding(*before, *after, now);
                self.changed(cx);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, cx: &mut EventCx<'_>, dt: f32) -> bool {
        if !self.focused {
            return false;
        }
        let before = self.caret_visible();
        self.blink += dt;
        if self.caret_visible() != before {
            cx.request_paint();
        }
        true
    }

    fn interaction(&self) -> Interaction {
        Interaction::Hover
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Always
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Text | Prop::Title => value.as_str().map_or(PropChange::Unknown, |t| {
                self.editor.set_text(t);
                PropChange::Layout
            }),
            Prop::Placeholder => value.as_str().map_or(PropChange::Unknown, |t| {
                self.placeholder = t.to_string();
                PropChange::Paint
            }),
            Prop::Tokens => value.as_str_list().map_or(PropChange::Unknown, |l| {
                self.tokens = l.to_vec();
                PropChange::Layout
            }),
            Prop::Items => value.as_str_list().map_or(PropChange::Unknown, |l| {
                self.options = l.to_vec();
                PropChange::Paint
            }),
            Prop::Subtitle => {
                self.error = value.as_str().filter(|s| !s.is_empty()).map(str::to_string);
                PropChange::Layout
            }
            Prop::SelectedRange => match value {
                Value::Range(a, b) => {
                    self.editor.set_selection(Selection {
                        anchor: *a,
                        focus: *b,
                    });
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            Prop::ControlSize => {
                value
                    .as_int()
                    .and_then(ControlSize::from_int)
                    .map_or(PropChange::Unknown, |s| {
                        self.size = s;
                        PropChange::Layout
                    })
            }
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Text | Prop::Title => Some(Value::Str(self.editor.text().to_string())),
            Prop::Placeholder => Some(Value::Str(self.placeholder.clone())),
            Prop::Tokens => Some(Value::StrList(self.tokens.clone())),
            Prop::Items => Some(Value::StrList(self.options.clone())),
            Prop::Subtitle => Some(self.error.clone().map(Value::Str).unwrap_or_default()),
            Prop::SelectedRange => {
                let r = self.editor.selection();
                Some(Value::Range(r.anchor, r.focus))
            }
            Prop::ControlSize => Some(Value::Int(self.size.to_int())),
            Prop::Expanded if self.kind == FieldKind::Combo => Some(Value::Bool(self.open)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = match self.kind {
            FieldKind::Text | FieldKind::Tokens => Role::TextField,
            FieldKind::Area => Role::TextArea,
            FieldKind::Secure => Role::PasswordField,
            FieldKind::Search => Role::SearchField,
            FieldKind::Combo => Role::ComboBox,
        };
        a.label = Some(self.placeholder.clone()).filter(|s| !s.is_empty());
        a.description.clone_from(&self.error);
        if self.kind != FieldKind::Secure {
            a.value = Some(self.editor.text().to_string());
        }
        let s = self.editor.selection();
        a.text_selection = Some((s.anchor, s.focus));
        if self.kind == FieldKind::Combo {
            a.expanded = Some(self.open);
        }
        a.actions = vec![Action::Focus, Action::SetValue];
    }
}

// ---------------------------------------------------------------------------------------
// Parts as views of their own (the kit has masters for them).
// ---------------------------------------------------------------------------------------

/// A token on its own (`Fields/Token`).
#[derive(Debug)]
pub struct Token {
    /// The label.
    pub label: String,
    /// Selected.
    pub selected: bool,
    cache: Label,
}

impl Token {
    /// A token.
    pub fn new(label: &str) -> Token {
        Token {
            label: label.to_string(),
            selected: false,
            cache: Label::default(),
        }
    }

    /// Selected.
    pub fn selected(mut self, selected: bool) -> Token {
        self.selected = selected;
        self
    }
}

impl Widget for Token {
    fn kind(&self) -> &'static str {
        "Token"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let theme = cx.theme();
        let style = theme.text(&typography::LABEL, theme.colors.text_primary);
        let w = whole(self.cache.width(&self.label, &style), metrics::STEP);
        Size::new(
            metrics::TOKEN_PAD_LEFT + w + metrics::TOKEN_REMOVE + metrics::TOKEN_PAD_RIGHT,
            metrics::TOKEN_HEIGHT,
        )
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let b = cx.bounds();
        let label = self.label.clone();
        let mut cache = std::mem::take(&mut self.cache);
        paint_token(cx, &mut cache, "", b, &label, self.selected);
        self.cache = cache;
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

/// A clear button on its own (`Fields/Clear Button`).
#[derive(Debug, Default)]
pub struct ClearButton;

impl Widget for ClearButton {
    fn kind(&self) -> &'static str {
        "ClearButton"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::CLEAR_BUTTON, metrics::CLEAR_BUTTON)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let b = cx.bounds();
        let pressed = cx.is_pressed();
        paint_clear(cx, "", b, pressed);
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if *event == Event::Activate {
            cx.emit(Signal::Activated, Value::None);
            return Handled::Yes;
        }
        Handled::No
    }

    fn interaction(&self) -> Interaction {
        Interaction::Button
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Button;
        a.label = Some("Clear".to_string());
        a.actions = vec![Action::Click];
    }
}

/// A combo button on its own (`Fields/Combo Button`).
#[derive(Debug, Default)]
pub struct ComboButton {
    /// Size.
    pub size: ControlSize,
}

impl Widget for ComboButton {
    fn kind(&self) -> &'static str {
        "ComboButton"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let s = metrics::combo_button(self.size);
        Size::new(s, s)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let b = cx.bounds();
        let pressed = cx.is_pressed() && !cx.is_disabled();
        paint_combo(cx, "", b, pressed);
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if *event == Event::Activate {
            cx.emit(Signal::Activated, Value::None);
            return Handled::Yes;
        }
        Handled::No
    }

    fn interaction(&self) -> Interaction {
        Interaction::Button
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Button;
        a.label = Some("Show options".to_string());
        a.actions = vec![Action::Click];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{KeyEvent, Mods};
    use crate::core::ui::Ui;
    use crate::core::window::{WindowId, WindowKind};
    use std::cell::RefCell;

    type Log = Rc<RefCell<Vec<String>>>;

    fn scene(f: TextField) -> (Ui, WindowId, crate::core::ViewId, Log) {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(400.0, 200.0));
        let id = ui.create(f);
        let log: Log = Rc::new(RefCell::new(Vec::new()));
        for (sig, tag) in [
            (Signal::TextChanged, "text"),
            (Signal::Submitted, "submit"),
            (Signal::ValueChanged, "tokens"),
        ] {
            let l = log.clone();
            ui.connect(id, sig, move |_, _, v| {
                let s = match v {
                    Value::Str(s) => s.clone(),
                    Value::StrList(l) => l.join("|"),
                    _ => String::new(),
                };
                l.borrow_mut().push(format!("{tag}:{s}"));
            });
        }
        let _ = ui.set(id, Prop::MaxWidth, Value::from(200.0));
        ui.set_content(win, id);
        ui.show_window(win);
        ui.prepare_frame(win);
        (ui, win, id, log)
    }

    fn key(ui: &mut Ui, win: WindowId, k: Key, mods: Mods) {
        ui.key(win, KeyEvent::press(k, mods));
    }

    #[test]
    fn typing_editing_and_submitting() {
        let (mut ui, win, id, log) = scene(TextField::text());
        ui.set_focus(id, true);
        ui.text_input(win, "Hello");
        key(&mut ui, win, Key::Backspace, Mods::empty());
        key(&mut ui, win, Key::Enter, Mods::empty());
        assert_eq!(ui.get(id, Prop::Text), Some(Value::from("Hell")));
        assert_eq!(
            *log.borrow(),
            vec!["text:Hello", "text:Hell", "submit:Hell"]
        );
        // Undo restores what Backspace removed.
        key(&mut ui, win, Key::Char('z'), Mods::LOGO);
        assert_eq!(ui.get(id, Prop::Text), Some(Value::from("Hello")));
    }

    #[test]
    fn clicks_place_the_caret_and_select_words() {
        let (mut ui, win, id, _log) = scene(TextField::text().with_text("Amalfi coast"));
        ui.prepare_frame(win);
        let r = ui.tree.window_rect(id);
        // Double click on "coast".
        let p = Point::new(r.x + 70.0, r.center().y);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        assert_eq!(ui.focus(win), Some(id));
        assert_eq!(ui.get(id, Prop::SelectedRange), Some(Value::Range(7, 12)));
    }

    #[test]
    fn secure_fields_draw_bullets_and_keep_the_text() {
        let (mut ui, win, id, _log) = scene(TextField::secure().with_text("hunter2"));
        ui.prepare_frame(win);
        let layers = ui.inspect_layers(id);
        let value = layers.iter().find(|l| l.path == "Value");
        assert!(matches!(
            value.map(|l| &l.content),
            Some(crate::core::InspectedContent::Text { text, .. }) if text.chars().all(|c| c == '\u{2022}') && text.chars().count() == 7
        ));
        assert_eq!(ui.get(id, Prop::Text), Some(Value::from("hunter2")));
        assert!(layers.iter().any(|l| l.path == "Leading icon"));
    }

    #[test]
    fn search_clears_with_escape_and_the_clear_button() {
        let (mut ui, win, id, log) = scene(TextField::search().with_text("Limoni"));
        ui.prepare_frame(win);
        let r = ui.tree.window_rect(id);
        let p = Point::new(r.x + 182.0, r.center().y);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        assert_eq!(ui.get(id, Prop::Text), Some(Value::from("")));
        assert!(log.borrow().contains(&"text:".to_string()));
        ui.set_focus(id, true);
        ui.text_input(win, "a");
        key(&mut ui, win, Key::Escape, Mods::empty());
        assert_eq!(ui.get(id, Prop::Text), Some(Value::from("")));
    }

    #[test]
    fn tokens_commit_select_and_remove() {
        let (mut ui, win, id, log) = scene(TextField::token_field());
        let _ = ui.set(id, Prop::MaxWidth, Value::from(260.0));
        ui.set_focus(id, true);
        ui.text_input(win, "Travel,");
        ui.text_input(win, "Coast");
        key(&mut ui, win, Key::Enter, Mods::empty());
        assert_eq!(
            ui.get(id, Prop::Tokens),
            Some(Value::StrList(vec!["Travel".into(), "Coast".into()]))
        );
        // Backspace at the start selects the last token, again removes it.
        key(&mut ui, win, Key::Backspace, Mods::empty());
        key(&mut ui, win, Key::Backspace, Mods::empty());
        assert_eq!(
            ui.get(id, Prop::Tokens),
            Some(Value::StrList(vec!["Travel".into()]))
        );
        assert!(log.borrow().iter().any(|l| l == "tokens:Travel|Coast"));
    }

    #[test]
    fn long_text_scrolls_to_keep_the_caret_visible() {
        let (mut ui, win, id, _log) = scene(TextField::text());
        ui.set_focus(id, true);
        ui.text_input(win, &"abcdefghij".repeat(8));
        ui.prepare_frame(win);
        assert!(ui.widget::<TextField>(id).is_some_and(|f| f.scroll.x > 0.0));
        key(&mut ui, win, Key::Home, Mods::empty());
        ui.prepare_frame(win);
        assert_eq!(ui.widget::<TextField>(id).map(|f| f.scroll.x), Some(0.0));
    }

    #[test]
    fn preedit_shows_without_changing_the_text() {
        let (mut ui, win, id, _log) = scene(TextField::text().with_text("ab"));
        ui.set_focus(id, true);
        ui.preedit(win, "かな", Some((0, 6)));
        ui.prepare_frame(win);
        assert_eq!(ui.get(id, Prop::Text), Some(Value::from("ab")));
        let layers = ui.inspect_layers(id);
        assert!(layers.iter().any(|l| matches!(
            &l.content,
            crate::core::InspectedContent::Text { text, .. } if text == "abかな"
        )));
        ui.text_input(win, "か");
        assert_eq!(ui.get(id, Prop::Text), Some(Value::from("abか")));
    }
}

#[cfg(test)]
mod combo_list_tests {
    use super::*;
    use crate::core::event::{KeyEvent, Mods};
    use crate::core::window::WindowKind;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn the_down_key_opens_the_list_and_a_choice_sets_the_text() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(400.0, 300.0));
        let f = ui.create(TextField::combo(&["Lemon", "Orange", "Pear"]).with_text("Pear"));
        let _ = ui.set(f, Prop::Width, Value::from(200.0));
        let log = Rc::new(RefCell::new(Vec::new()));
        let l = log.clone();
        ui.connect(f, Signal::SelectionChanged, move |_, _, v| {
            l.borrow_mut()
                .push(v.as_str().unwrap_or_default().to_string());
        });
        ui.set_content(win, f);
        ui.show_window(win);
        ui.set_focus(f, true);
        ui.prepare_frame(win);
        ui.key(win, KeyEvent::press(Key::Down, Mods::empty()));
        assert_eq!(ui.get(f, Prop::Expanded), Some(Value::Bool(true)));
        TextField::choose_in(&mut ui, f, 1);
        ui.flush();
        assert_eq!(ui.get(f, Prop::Text), Some(Value::Str("Orange".into())));
        assert_eq!(ui.get(f, Prop::Expanded), Some(Value::Bool(false)));
        assert_eq!(*log.borrow(), vec!["Orange".to_string()]);
    }
}
