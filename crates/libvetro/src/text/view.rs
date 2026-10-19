//! A multi-line text view for documents of any size (NSTextView-like): the editing engine
//! over the whole text, layout per paragraph done lazily and cached, painting of the visible
//! paragraphs only, scrolling, selection, caret, input method, clipboard.
//!
//! Large documents (Text Editor's 10 MB files) stay fast because work is proportional to
//! what is on screen: a paragraph is laid out when it is shown (or when the caret moves
//! through it), and paragraphs never shown count one line each until they are.

use std::ops::Range;
use std::rc::Rc;

use crate::color::Color;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled, Key, Mods};
use crate::core::layout::Proposal;
use crate::core::tree::NodeFlags;
use crate::core::ui::{Cursor, Request};
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size};
use crate::shape::Shape;
use crate::style::Style;
use crate::text::AttributedString;
use crate::text::edit::{EditAction, Editor, Movement, Selection};
use crate::text::layout::{ParagraphOptions, TextLayout, TextStyle};

/// The caret blinks with this half period (seconds).
pub const CARET_BLINK: f64 = 0.53;

#[derive(Debug, Clone)]
struct Para {
    /// Length in bytes, without the newline.
    len: usize,
    /// Cached layout for a width.
    layout: Option<(u32, Rc<TextLayout>)>,
}

/// A Fenwick tree: prefix sums with point updates and searches in O(log n).
#[derive(Debug, Clone, Default)]
struct Fenwick<T> {
    tree: Vec<T>,
}

impl<T: Copy + Default + std::ops::AddAssign + std::ops::Sub<Output = T> + PartialOrd> Fenwick<T> {
    fn build(values: impl Iterator<Item = T>) -> Self {
        let mut tree: Vec<T> = values.collect();
        let n = tree.len();
        for i in 0..n {
            let j = i | (i + 1);
            if j < n {
                let v = tree.get(i).copied().unwrap_or_default();
                if let Some(t) = tree.get_mut(j) {
                    *t += v;
                }
            }
        }
        Fenwick { tree }
    }

    /// Adds `d` at index `i`.
    fn add(&mut self, mut i: usize, d: T) {
        while i < self.tree.len() {
            if let Some(t) = self.tree.get_mut(i) {
                *t += d;
            }
            i |= i + 1;
        }
    }

    /// The sum of the first `i` values.
    fn prefix(&self, i: usize) -> T {
        let mut s = T::default();
        let mut i = i.min(self.tree.len());
        while i > 0 {
            if let Some(t) = self.tree.get(i - 1) {
                s += *t;
            }
            i &= i - 1;
        }
        s
    }

    /// The largest `i` with `prefix(i) <= target` (the index whose span contains `target`).
    fn find(&self, target: T) -> usize {
        let n = self.tree.len();
        let mut pos = 0usize;
        let mut rem = target;
        let mut step = n.next_power_of_two();
        while step > 0 {
            let next = pos + step;
            if next <= n
                && let Some(t) = self.tree.get(next - 1).copied()
                && t <= rem
            {
                pos = next;
                rem = rem - t;
            }
            step >>= 1;
        }
        pos.min(n.saturating_sub(1))
    }
}

/// The text view.
pub struct TextView {
    editor: Editor,
    paras: Vec<Para>,
    /// Bytes of each paragraph with its newline.
    bytes: Fenwick<i64>,
    /// Height of each paragraph (a line until laid out).
    heights: Fenwick<f32>,
    style: TextStyle,
    /// Insets of the text inside the view.
    pub inset: f32,
    scroll: f32,
    width: f32,
    height: f32,
    editable: bool,
    caret_on: bool,
    blink_at: f64,
    dragging: bool,
    /// Wraps lines (false: no wrapping, horizontal overflow clipped).
    pub wraps: bool,
}

impl std::fmt::Debug for TextView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextView")
            .field("len", &self.editor.text().len())
            .field("paragraphs", &self.paras.len())
            .finish()
    }
}

fn split_paragraphs(text: &str) -> Vec<Para> {
    text.split('\n')
        .map(|p| Para {
            len: p.len(),
            layout: None,
        })
        .collect()
}

impl TextView {
    /// A view on `text` in `style`.
    pub fn new(text: &str, style: TextStyle) -> Self {
        let mut editor = Editor::new(text, false);
        editor.take_change();
        let paras = split_paragraphs(editor.text());
        let mut v = TextView {
            editor,
            paras,
            bytes: Fenwick::default(),
            heights: Fenwick::default(),
            style,
            inset: 8.0,
            scroll: 0.0,
            width: 0.0,
            height: 0.0,
            editable: true,
            caret_on: true,
            blink_at: 0.0,
            dragging: false,
            wraps: true,
        };
        v.editor.set_selection(Selection::caret(0));
        v.rebuild_sums();
        v
    }

    fn rebuild_sums(&mut self) {
        let lh = self.line_height();
        let key = self.text_width().to_bits();
        self.bytes = Fenwick::build(self.paras.iter().map(|p| p.len as i64 + 1));
        self.heights = Fenwick::build(self.paras.iter().map(|p| match &p.layout {
            Some((k, l)) if *k == key => l.size().height.max(lh),
            _ => lh,
        }));
    }

    /// The byte range of paragraph `i` (without its newline).
    fn range(&self, i: usize) -> Range<usize> {
        let start = usize::try_from(self.bytes.prefix(i)).unwrap_or(0);
        let len = self.paras.get(i).map(|p| p.len).unwrap_or(0);
        start..start + len
    }

    /// The text.
    pub fn text(&self) -> &str {
        self.editor.text()
    }

    /// The editor (selection, undo …).
    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    /// Number of paragraphs.
    pub fn paragraph_count(&self) -> usize {
        self.paras.len()
    }

    fn line_height(&self) -> f32 {
        self.style.line_height.unwrap_or(self.style.size * 1.3)
    }

    fn text_width(&self) -> f32 {
        if self.wraps {
            (self.width - 2.0 * self.inset).max(1.0)
        } else {
            f32::INFINITY
        }
    }

    /// The paragraph containing byte `at`.
    fn para_at(&self, at: usize) -> usize {
        self.bytes.find(i64::try_from(at).unwrap_or(i64::MAX))
    }

    fn layout_of(&mut self, i: usize) -> Option<Rc<TextLayout>> {
        let width = self.text_width();
        let key = width.to_bits();
        let range = self.range(i);
        if let Some((k, l)) = &self.paras.get(i)?.layout
            && *k == key
        {
            return Some(l.clone());
        }
        let text = self.editor.text().get(range).unwrap_or_default();
        let l = Rc::new(TextLayout::new(
            &AttributedString::plain(text),
            &self.style,
            &ParagraphOptions::default(),
            width,
        ));
        let old_h = self.para_height(i);
        if let Some(p) = self.paras.get_mut(i) {
            p.layout = Some((key, l.clone()));
        }
        let new_h = self.para_height(i);
        if (new_h - old_h).abs() > f32::EPSILON {
            self.heights.add(i, new_h - old_h);
        }
        Some(l)
    }

    fn para_height(&self, i: usize) -> f32 {
        let key = self.text_width().to_bits();
        match self.paras.get(i).and_then(|p| p.layout.as_ref()) {
            Some((k, l)) if *k == key => l.size().height.max(self.line_height()),
            _ => self.line_height(),
        }
    }

    /// The top of paragraph `i` in document coordinates.
    fn para_top(&self, i: usize) -> f32 {
        self.heights.prefix(i) + self.inset
    }

    fn content_height(&self) -> f32 {
        self.heights.prefix(self.paras.len()) + 2.0 * self.inset
    }

    /// The paragraph at document y, and its top.
    fn para_at_y(&self, y: f32) -> (usize, f32) {
        let i = self.heights.find((y - self.inset).max(0.0));
        (i, self.para_top(i))
    }

    /// After an edit: the paragraphs touched by the change are rebuilt (the others keep
    /// their layouts); the sums are updated in place when the paragraph count is unchanged.
    fn reindex(&mut self) -> bool {
        let Some((start, old_end, new_end)) = self.editor.take_change() else {
            return false;
        };
        let first = self.para_at(start);
        let last = self.para_at(old_end).max(first);
        let first_start = self.range(first).start;
        let last_end_old = self.range(last).end;
        let delta = new_end as isize - old_end as isize;
        let last_end_new = (last_end_old as isize + delta).max(first_start as isize) as usize;
        let text = self.editor.text();
        let mid = text
            .get(first_start..last_end_new.min(text.len()))
            .unwrap_or_default();
        let replaced = split_paragraphs(mid);
        let same_count = replaced.len() == last - first + 1;
        let tail = self.paras.split_off((last + 1).min(self.paras.len()));
        self.paras.truncate(first);
        let n_new = replaced.len();
        self.paras.extend(replaced);
        self.paras.extend(tail);
        if same_count {
            let lh = self.line_height();
            for k in first..first + n_new {
                let old_bytes = self.bytes.prefix(k + 1) - self.bytes.prefix(k);
                let new_bytes = self.paras.get(k).map(|p| p.len as i64 + 1).unwrap_or(1);
                self.bytes.add(k, new_bytes - old_bytes);
                let old_h = self.heights.prefix(k + 1) - self.heights.prefix(k);
                self.heights.add(k, lh - old_h);
            }
        } else {
            self.rebuild_sums();
        }
        true
    }

    fn caret_rect_doc(&mut self) -> Rect {
        let at = self.editor.selection().focus;
        let i = self.para_at(at);
        let top = self.para_top(i);
        let start = self.range(i).start;
        match self.layout_of(i) {
            Some(l) => {
                let r = l.caret_rect(at - start, false);
                Rect::new(
                    r.x + self.inset,
                    r.y + top,
                    1.0,
                    r.height.max(self.line_height()),
                )
            }
            None => Rect::new(self.inset, top, 1.0, self.line_height()),
        }
    }

    fn scroll_to_caret(&mut self) {
        let c = self.caret_rect_doc();
        let view_h = self.height;
        if c.y < self.scroll {
            self.scroll = c.y - self.inset;
        } else if c.bottom() > self.scroll + view_h {
            self.scroll = c.bottom() - view_h + self.inset;
        }
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        let max = (self.content_height() - self.height).max(0.0);
        self.scroll = self.scroll.clamp(0.0, max);
    }

    /// The byte offset at a point of the view.
    fn offset_at(&mut self, p: Point) -> usize {
        let y = p.y + self.scroll;
        let (i, top) = self.para_at_y(y);
        let start = self.range(i).start;
        match self.layout_of(i) {
            Some(l) => {
                let (off, _) = l.offset_at(Point::new(p.x - self.inset, y - top));
                start + off
            }
            None => start,
        }
    }

    fn vertical(&mut self, up: bool, extend: bool) {
        let at = self.editor.selection().focus;
        let i = self.para_at(at);
        let start = self.range(i).start;
        let Some(l) = self.layout_of(i) else { return };
        let local = at - start;
        let caret = l.caret_rect(local, false);
        let lines = l.lines();
        let li = l.line_index(local, false);
        let target = if up && li > 0 {
            lines
                .get(li - 1)
                .map(|t| start + l.offset_at(Point::new(caret.x, t.top + t.height / 2.0)).0)
        } else if !up && li + 1 < lines.len() {
            lines
                .get(li + 1)
                .map(|t| start + l.offset_at(Point::new(caret.x, t.top + t.height / 2.0)).0)
        } else if up && i > 0 {
            let j = i - 1;
            let s = self.range(j).start;
            self.layout_of(j).map(|pl| {
                let last = pl.lines().last().copied();
                let y = last.map(|t| t.top + t.height / 2.0).unwrap_or(0.0);
                s + pl.offset_at(Point::new(caret.x, y)).0
            })
        } else if !up && i + 1 < self.paras.len() {
            let j = i + 1;
            let s = self.range(j).start;
            self.layout_of(j)
                .map(|pl| s + pl.offset_at(Point::new(caret.x, 1.0)).0)
        } else if up {
            Some(0)
        } else {
            Some(self.editor.text().len())
        };
        if let Some(t) = target {
            let sel = self.editor.selection();
            self.editor.set_selection(if extend {
                Selection {
                    anchor: sel.anchor,
                    focus: t,
                }
            } else {
                Selection::caret(t)
            });
        }
    }

    fn edited(&mut self, cx: &mut EventCx<'_>) {
        if self.reindex() {
            cx.emit(Signal::TextChanged, Value::None);
        }
        self.after_move(cx);
    }

    fn after_move(&mut self, cx: &mut EventCx<'_>) {
        self.scroll_to_caret();
        self.caret_on = true;
        self.blink_at = cx.now() + CARET_BLINK;
        self.send_text_input(cx);
        cx.emit(Signal::SelectionChanged, {
            let r = self.editor.selection().range();
            Value::Range(r.start, r.end)
        });
        cx.request_paint();
    }

    fn send_text_input(&mut self, cx: &mut EventCx<'_>) {
        let Some(w) = cx.window() else { return };
        let caret = self.caret_rect_doc();
        let origin = cx.window_rect().origin();
        let r = Rect::new(
            caret.x + origin.x,
            caret.y - self.scroll + origin.y,
            caret.width,
            caret.height,
        );
        cx.request(Request::TextInput(w, Some(r)));
        // The surrounding text: the caret's paragraph.
        let sel = self.editor.selection();
        let r = self.range(self.para_at(sel.focus));
        let text = self
            .editor
            .text()
            .get(r.clone())
            .unwrap_or_default()
            .to_string();
        let c = sel.focus.saturating_sub(r.start).min(text.len());
        let a = sel.anchor.saturating_sub(r.start).min(text.len());
        cx.request(Request::TextInputSurrounding(w, text, c, a));
    }
}

impl Widget for TextView {
    fn kind(&self) -> &'static str {
        "TextView"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        // A text view takes the space it is offered (it scrolls).
        p.replacing_unspecified(Size::new(320.0, 200.0))
    }

    fn place(&mut self, _cx: &mut LayoutCx<'_>, size: Size) {
        self.width = size.width;
        self.height = size.height;
        self.clamp_scroll();
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let bounds = cx.bounds();
        self.width = bounds.width;
        self.height = bounds.height;
        if self.style.color != theme.colors.text_primary {
            // The appearance changed: the cached layouts carry the old colour.
            self.style.color = theme.colors.text_primary;
            self.invalidate_layouts();
        }
        cx.rec.push_clip(bounds, Shape::RECT);
        let (first, mut top) = self.para_at_y(self.scroll);
        let sel = self.editor.selection().range();
        let selection_color = theme
            .appearance
            .highlight
            .unwrap_or(theme.colors.accent_amalfi)
            .with_alpha(if cx.is_focused() { 0x4d } else { 0x26 });
        let mut i = first;
        while i < self.paras.len() && top < self.scroll + bounds.height {
            let Some(l) = self.layout_of(i) else { break };
            let Range { start, end } = self.range(i);
            let y = top - self.scroll + bounds.y;
            if sel.start < sel.end && sel.start <= end && sel.end >= start {
                let a = sel.start.max(start) - start;
                let b = sel.end.min(end) - start;
                for r in l.selection_rects(a..b) {
                    cx.rec.layer(
                        "Selection",
                        r.translate(Point::new(bounds.x + self.inset, y)),
                        Shape::RECT,
                        Style::fill(selection_color),
                    );
                }
            }
            let h = l.size().height.max(self.line_height());
            cx.rec.text(
                "Text",
                Rect::new(bounds.x + self.inset, y, l.size().width, h),
                l,
                None,
                self.style.color,
            );
            top += h;
            i += 1;
        }
        if cx.is_focused() && self.caret_on && self.editor.selection().is_collapsed() {
            let c = self.caret_rect_doc();
            cx.rec.layer(
                "Caret",
                Rect::new(c.x + bounds.x, c.y - self.scroll + bounds.y, 1.5, c.height),
                Shape::RECT,
                Style::fill(theme.colors.accent_amalfi),
            );
        }
        if let Some(p) = self.editor.preedit().cloned() {
            let c = self.caret_rect_doc();
            let mut st = self.style.clone();
            st.underline = true;
            st.color = theme.colors.text_primary;
            let l = Rc::new(TextLayout::plain(&p.text, &st, f32::INFINITY));
            let w = l.size().width;
            cx.rec.layer(
                "Preedit Background",
                Rect::new(c.x + bounds.x, c.y - self.scroll + bounds.y, w, c.height),
                Shape::RECT,
                Style::fill(theme.colors.surface_window),
            );
            cx.rec.text(
                "Preedit",
                Rect::new(c.x + bounds.x, c.y - self.scroll + bounds.y, w, c.height),
                l,
                None,
                theme.colors.text_primary,
            );
        }
        cx.rec.pop();
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::PointerDown {
                pos,
                button: Button::Left,
                clicks,
                mods,
            } => {
                let local = *pos - cx.bounds().origin();
                let at = self.offset_at(local);
                match clicks {
                    2 => self.editor.select_word_at(at),
                    n if *n >= 3 => self.editor.select_paragraph_at(at),
                    _ => {
                        let sel = self.editor.selection();
                        self.editor.set_selection(if mods.contains(Mods::SHIFT) {
                            Selection {
                                anchor: sel.anchor,
                                focus: at,
                            }
                        } else {
                            Selection::caret(at)
                        });
                    }
                }
                self.dragging = true;
                cx.focus_self();
                cx.capture_pointer();
                self.after_move(cx);
                Handled::Yes
            }
            Event::PointerMove { pos, dragging, .. } => {
                cx.set_cursor(Cursor::Text);
                if *dragging && self.dragging {
                    let local = *pos - cx.bounds().origin();
                    let at = self.offset_at(local);
                    let sel = self.editor.selection();
                    self.editor.set_selection(Selection {
                        anchor: sel.anchor,
                        focus: at,
                    });
                    self.after_move(cx);
                }
                Handled::Yes
            }
            Event::PointerUp { .. } => {
                self.dragging = false;
                Handled::Yes
            }
            Event::Scroll { dy, phase, .. } => {
                let _ = phase;
                self.scroll += *dy;
                self.clamp_scroll();
                cx.emit(Signal::Scrolled, Value::Point(Point::new(0.0, self.scroll)));
                cx.request_paint();
                Handled::Yes
            }
            Event::Key(k) => {
                if !k.pressed {
                    return Handled::No;
                }
                let extend = k.mods.contains(Mods::SHIFT);
                let plain_vertical = !k.mods.intersects(Mods::LOGO | Mods::ALT | Mods::CTRL);
                match k.key {
                    Key::Up if plain_vertical => {
                        self.vertical(true, extend);
                        self.after_move(cx);
                        return Handled::Yes;
                    }
                    Key::Down if plain_vertical => {
                        self.vertical(false, extend);
                        self.after_move(cx);
                        return Handled::Yes;
                    }
                    Key::PageUp | Key::PageDown => {
                        let d = if k.key == Key::PageUp {
                            -self.height
                        } else {
                            self.height
                        };
                        self.scroll += d;
                        self.clamp_scroll();
                        cx.request_paint();
                        return Handled::Yes;
                    }
                    _ => {}
                }
                if !self.editable && k.text.is_some() {
                    return Handled::No;
                }
                let at = self.editor.selection().focus;
                let i = self.para_at(at);
                let l = self.layout_of(i);
                let start = self.range(i).start;
                // Line movements inside the paragraph use its layout; offsets are local.
                let action = match (&k.key, &l) {
                    (Key::Left | Key::Right | Key::Home | Key::End, Some(_))
                        if k.mods.contains(Mods::LOGO) || matches!(k.key, Key::Home | Key::End) =>
                    {
                        let mv = if matches!(k.key, Key::Left | Key::Home) {
                            Movement::LineStart
                        } else {
                            Movement::LineEnd
                        };
                        if let Some(layout) = &l {
                            let local = at - start;
                            let lines = layout.lines();
                            let li = layout.line_index(local, false);
                            let t = lines
                                .get(li)
                                .map(|ln| {
                                    if mv == Movement::LineStart {
                                        ln.start
                                    } else {
                                        ln.end
                                    }
                                })
                                .unwrap_or(local);
                            let sel = self.editor.selection();
                            self.editor.set_selection(if extend {
                                Selection {
                                    anchor: sel.anchor,
                                    focus: start + t,
                                }
                            } else {
                                Selection::caret(start + t)
                            });
                        }
                        EditAction::Handled
                    }
                    _ if !self.editable
                        && !matches!(
                            k.key,
                            Key::Left | Key::Right | Key::Char('a') | Key::Char('c')
                        ) =>
                    {
                        EditAction::Unhandled
                    }
                    _ => self.editor.handle_key(k, None, cx.now()),
                };
                match action {
                    EditAction::Unhandled => Handled::No,
                    EditAction::Handled | EditAction::Submit => {
                        self.edited(cx);
                        Handled::Yes
                    }
                    EditAction::Copy(t) => {
                        cx.set_clipboard(t);
                        self.edited(cx);
                        Handled::Yes
                    }
                    EditAction::Paste => {
                        if self.editable {
                            cx.request_paste();
                        }
                        Handled::Yes
                    }
                    EditAction::Cancel => Handled::No,
                }
            }
            Event::Text(t) if self.editable => {
                self.editor.insert(t, cx.now());
                self.edited(cx);
                Handled::Yes
            }
            Event::Paste(t) if self.editable => {
                self.editor.insert(t, cx.now());
                self.edited(cx);
                Handled::Yes
            }
            Event::Preedit { text, cursor } if self.editable => {
                self.editor.set_preedit(text, *cursor);
                cx.request_paint();
                Handled::Yes
            }
            Event::DeleteSurrounding {
                before: b,
                after: a,
            } if self.editable => {
                self.editor.delete_surrounding(*b, *a, cx.now());
                self.edited(cx);
                Handled::Yes
            }
            Event::FocusIn => {
                self.caret_on = true;
                self.blink_at = cx.now() + CARET_BLINK;
                cx.animate();
                self.send_text_input(cx);
                cx.request_paint();
                Handled::Yes
            }
            Event::FocusOut => {
                if let Some(w) = cx.window() {
                    cx.request(Request::TextInput(w, None));
                }
                cx.request_paint();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, cx: &mut EventCx<'_>, _dt: f32) -> bool {
        if !cx.is_focused() {
            return false;
        }
        if cx.now() >= self.blink_at {
            self.caret_on = !self.caret_on;
            self.blink_at = cx.now() + CARET_BLINK;
            cx.request_paint();
        }
        true
    }

    fn state_changed(&mut self, _cx: &mut EventCx<'_>, _old: NodeFlags, _new: NodeFlags) {}

    fn focus(&self) -> FocusKind {
        FocusKind::Always
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Text => match value.as_str() {
                Some(t) => {
                    self.editor.set_text(t);
                    self.editor.set_selection(Selection::caret(0));
                    self.editor.take_change();
                    self.paras = split_paragraphs(self.editor.text());
                    self.rebuild_sums();
                    self.scroll = 0.0;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            Prop::Editable => match value.as_bool() {
                Some(b) => {
                    self.editable = b;
                    PropChange::Nothing
                }
                None => PropChange::Unknown,
            },
            Prop::Wraps => match value.as_bool() {
                Some(b) => {
                    self.wraps = b;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
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
            Prop::FontFamily => match value.as_str() {
                Some(f) => {
                    self.style.family = f.to_string();
                    self.invalidate_layouts();
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            Prop::FontSize => match value.as_f32() {
                Some(s) if s > 0.0 => {
                    let k = s / self.style.size.max(1.0);
                    self.style.size = s;
                    self.style.line_height = self.style.line_height.map(|l| (l * k).round());
                    self.invalidate_layouts();
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Text => Some(Value::Str(self.editor.text().to_string())),
            Prop::Editable => Some(Value::Bool(self.editable)),
            Prop::SelectedRange => {
                let s = self.editor.selection();
                Some(Value::Range(s.anchor, s.focus))
            }
            Prop::ScrollOffset => Some(Value::Point(Point::new(0.0, self.scroll))),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::TextArea;
        let s = self.editor.selection();
        a.text_selection = Some((s.anchor, s.focus));
        // Long documents are not copied into the tree: the value is the caret's paragraph.
        let r = self.range(self.para_at(s.focus));
        a.value = self.editor.text().get(r).map(str::to_string);
        a.actions = vec![Action::Focus, Action::SetValue];
    }
}

impl TextView {
    fn invalidate_layouts(&mut self) {
        for p in &mut self.paras {
            p.layout = None;
        }
        self.rebuild_sums();
    }
}

/// The colour of selected text's background in `c`'s appearance (kept for components).
pub fn selection_color(accent: Color, focused: bool) -> Color {
    accent.with_alpha(if focused { 0x4d } else { 0x26 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Ui;
    use crate::core::event::KeyEvent;
    use crate::core::window::WindowKind;

    fn setup(text: &str) -> (Ui, crate::core::WindowId, crate::core::ViewId) {
        let mut ui = Ui::new_headless();
        let w = ui.create_window(WindowKind::Normal, Size::new(400.0, 300.0));
        let v = ui.create(TextView::new(text, TextStyle::default()));
        ui.set(v, Prop::MaxWidth, Value::from(f32::INFINITY))
            .unwrap();
        ui.set(v, Prop::MaxHeight, Value::from(f32::INFINITY))
            .unwrap();
        ui.set_content(w, v);
        ui.show_window(w);
        ui.prepare_frame(w);
        ui.set_focus(v, false);
        (ui, w, v)
    }

    #[test]
    fn typing_moves_and_reindexes() {
        let (mut ui, w, v) = setup("one\ntwo\nthree");
        ui.text_input(w, "X");
        assert_eq!(
            ui.get(v, Prop::Text),
            Some(Value::Str("Xone\ntwo\nthree".into()))
        );
        ui.key(w, KeyEvent::press(Key::Down, Mods::empty()));
        ui.text_input(w, "Y");
        let tv = ui.widget::<TextView>(v).unwrap();
        assert!(tv.text().contains('Y'));
        assert_eq!(tv.paragraph_count(), 3);
        ui.key(w, KeyEvent::press(Key::Enter, Mods::empty()));
        assert_eq!(ui.widget::<TextView>(v).unwrap().paragraph_count(), 4);
        ui.key(w, KeyEvent::press(Key::Backspace, Mods::empty()));
        assert_eq!(ui.widget::<TextView>(v).unwrap().paragraph_count(), 3);
        // The paragraph index always matches the text.
        let tv = ui.widget::<TextView>(v).unwrap();
        let fresh = split_paragraphs(tv.text());
        assert_eq!(tv.paras.len(), fresh.len());
        for (k, (p, q)) in tv.paras.iter().zip(fresh.iter()).enumerate() {
            assert_eq!(p.len, q.len);
            let start: usize = fresh.iter().take(k).map(|x| x.len + 1).sum();
            assert_eq!(tv.range(k), start..start + q.len);
        }
    }

    #[test]
    fn large_documents_stay_fast() {
        // About 10 MB: 120 000 lines of 85 bytes.
        let line =
            "The quick brown fox jumps over the lazy dog, again and again, every single day! ok\n";
        let text: String = line.repeat(120_000);
        let t0 = std::time::Instant::now();
        let (mut ui, w, v) = setup(&text);
        let open = t0.elapsed();
        // Only the visible paragraphs were laid out.
        let laid = ui
            .widget::<TextView>(v)
            .unwrap()
            .paras
            .iter()
            .filter(|p| p.layout.is_some())
            .count();
        assert!(laid < 40, "{laid} paragraphs laid out");
        let t1 = std::time::Instant::now();
        for _ in 0..20 {
            ui.text_input(w, "a");
            ui.prepare_frame(w);
        }
        let per_key = t1.elapsed() / 20;
        eprintln!("10 MB: open {open:?}, per key {per_key:?}");
        // Generous bounds for unoptimised test builds; release numbers are far lower.
        assert!(open.as_secs_f32() < 2.0, "{open:?}");
        assert!(per_key.as_secs_f32() < 0.05, "{per_key:?}");
        assert_eq!(ui.widget::<TextView>(v).unwrap().paragraph_count(), 120_001);
    }

    #[test]
    fn fenwick() {
        let f = Fenwick::build([3i64, 1, 4, 1, 5].into_iter());
        assert_eq!(f.prefix(0), 0);
        assert_eq!(f.prefix(3), 8);
        assert_eq!(f.find(0), 0);
        assert_eq!(f.find(2), 0);
        assert_eq!(f.find(3), 1);
        assert_eq!(f.find(8), 3);
        assert_eq!(f.find(100), 4);
    }
}
