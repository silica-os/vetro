//! The editing engine: a text with a selection, caret movement by grapheme, word, line and
//! document, the macOS key bindings, undo with typing coalesced into groups, and the input
//! method's pre-edit. Offsets are UTF-8 bytes on grapheme boundaries.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use crate::core::event::{Key, KeyEvent, Mods};
use crate::geom::Point;
use crate::text::layout::TextLayout;

/// A selection: the anchor stays, the focus moves. Collapsed: a caret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    /// Where the selection started.
    pub anchor: usize,
    /// Where it ends (the caret).
    pub focus: usize,
}

impl Selection {
    /// A caret at `at`.
    pub fn caret(at: usize) -> Self {
        Selection {
            anchor: at,
            focus: at,
        }
    }

    /// The selected range, ordered.
    pub fn range(self) -> Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }

    /// Whether nothing is selected.
    pub fn is_collapsed(self) -> bool {
        self.anchor == self.focus
    }
}

/// Where a caret moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Movement {
    /// One grapheme left.
    Left,
    /// One grapheme right.
    Right,
    /// To the start of the word on the left.
    WordLeft,
    /// To the end of the word on the right.
    WordRight,
    /// To the start of the visual line.
    LineStart,
    /// To the end of the visual line.
    LineEnd,
    /// One line up.
    Up,
    /// One line down.
    Down,
    /// To the start of the paragraph.
    ParagraphStart,
    /// To the end of the paragraph.
    ParagraphEnd,
    /// To the start of the text.
    DocumentStart,
    /// To the end of the text.
    DocumentEnd,
}

/// What a key did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditAction {
    /// Not an editing key.
    Unhandled,
    /// The text or the selection changed (or nothing changed but the key was consumed).
    Handled,
    /// Copy this text to the clipboard.
    Copy(String),
    /// Paste from the clipboard (the caller asks for it, then calls [`Editor::insert`]).
    Paste,
    /// Return in a one-line field.
    Submit,
    /// Escape.
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Change {
    at: usize,
    removed: String,
    inserted: String,
    before: Selection,
    after: Selection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Typing,
    Deleting,
    Other,
}

/// The input method's composition text, shown at the caret and not yet part of the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preedit {
    /// The text being composed.
    pub text: String,
    /// The caret (or selection) inside it, in bytes.
    pub cursor: Option<(usize, usize)>,
}

/// Seconds of pause after which typing starts a new undo group.
pub const UNDO_COALESCE: f64 = 1.0;

/// An editable text.
#[derive(Debug, Clone)]
pub struct Editor {
    text: String,
    selection: Selection,
    preedit: Option<Preedit>,
    undo: Vec<Vec<Change>>,
    redo: Vec<Vec<Change>>,
    last_kind: Kind,
    last_time: f64,
    /// One line: Return submits, newlines in pasted text become spaces.
    pub single_line: bool,
    /// Maximum length in characters (`None`: unlimited).
    pub max_chars: Option<usize>,
    /// The x the caret keeps while moving up and down.
    goal_x: Option<f32>,
    /// The span changed since [`Editor::take_change`]: start, old end, new end.
    changed: Option<(usize, usize, usize)>,
}

impl Default for Editor {
    fn default() -> Self {
        Editor::new("", false)
    }
}

impl Editor {
    /// An editor on `text` with the caret at its end.
    pub fn new(text: &str, single_line: bool) -> Self {
        let text = if single_line {
            text.replace(['\n', '\r'], " ")
        } else {
            text.to_string()
        };
        let end = text.len();
        Editor {
            text,
            selection: Selection::caret(end),
            preedit: None,
            undo: Vec::new(),
            redo: Vec::new(),
            last_kind: Kind::Other,
            last_time: f64::NEG_INFINITY,
            single_line,
            max_chars: None,
            goal_x: None,
            changed: None,
        }
    }

    /// The text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The selection.
    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// The pre-edit.
    pub fn preedit(&self) -> Option<&Preedit> {
        self.preedit.as_ref()
    }

    /// The text as displayed: with the pre-edit spliced in at the caret, and the pre-edit's
    /// range in it.
    pub fn display_text(&self) -> (String, Option<Range<usize>>) {
        match &self.preedit {
            Some(p) if !p.text.is_empty() => {
                let at = self.selection.range().start;
                let mut s = String::with_capacity(self.text.len() + p.text.len());
                s.push_str(self.text.get(..at).unwrap_or_default());
                s.push_str(&p.text);
                s.push_str(
                    self.text
                        .get(self.selection.range().end..)
                        .unwrap_or_default(),
                );
                (s, Some(at..at + p.text.len()))
            }
            _ => (self.text.clone(), None),
        }
    }

    /// Replaces the whole text (not undoable: a programmatic change); the caret goes to the
    /// end.
    /// Replaces `range` and records the change.
    fn splice(&mut self, range: Range<usize>, text: &str) {
        let (a, b) = (range.start, range.end);
        let len = text.len();
        self.text.replace_range(range, text);
        // Merge with the changes not yet taken, in current coordinates.
        self.changed = Some(match self.changed {
            None => (a, b, a + len),
            Some((s, oe, ne)) => {
                let delta = ne as isize - oe as isize;
                let start = s.min(a);
                let end_cur = ne.max(b);
                let old_end = (end_cur as isize - delta).max(start as isize) as usize;
                let new_end = (end_cur as isize + len as isize - (b - a) as isize)
                    .max(start as isize) as usize;
                (start, old_end.max(oe), new_end)
            }
        });
    }

    /// The span changed since the last call (start, old end, new end), if any.
    pub fn take_change(&mut self) -> Option<(usize, usize, usize)> {
        self.changed.take()
    }

    pub fn set_text(&mut self, text: &str) {
        let text = if self.single_line {
            text.replace(['\n', '\r'], " ")
        } else {
            text.to_string()
        };
        let old_len = self.text.len();
        self.text = text;
        self.changed = Some((0, old_len, self.text.len()));
        self.selection = Selection::caret(self.text.len());
        self.preedit = None;
        self.undo.clear();
        self.redo.clear();
        self.goal_x = None;
    }

    /// Sets the selection (snapped to character boundaries).
    pub fn set_selection(&mut self, sel: Selection) {
        self.selection = Selection {
            anchor: self.snap(sel.anchor),
            focus: self.snap(sel.focus),
        };
        self.goal_x = None;
    }

    /// The selected text.
    pub fn selected_text(&self) -> &str {
        self.text.get(self.selection.range()).unwrap_or_default()
    }

    fn snap(&self, at: usize) -> usize {
        let mut at = at.min(self.text.len());
        while at > 0 && !self.text.is_char_boundary(at) {
            at -= 1;
        }
        at
    }

    /// Selects everything.
    pub fn select_all(&mut self) {
        self.selection = Selection {
            anchor: 0,
            focus: self.text.len(),
        };
    }

    /// Selects the word at `at` (double click).
    pub fn select_word_at(&mut self, at: usize) {
        let at = self.snap(at);
        for (i, w) in self.text.split_word_bound_indices() {
            let end = i + w.len();
            if at >= i && at < end || (at == end && end == self.text.len()) {
                self.selection = Selection {
                    anchor: i,
                    focus: end,
                };
                return;
            }
        }
        self.selection = Selection::caret(at);
    }

    /// Selects the paragraph at `at` (triple click).
    pub fn select_paragraph_at(&mut self, at: usize) {
        let at = self.snap(at);
        let start = self.paragraph_start(at);
        let end = self.paragraph_end(at);
        self.selection = Selection {
            anchor: start,
            focus: end,
        };
    }

    fn paragraph_start(&self, at: usize) -> usize {
        self.text
            .get(..at)
            .and_then(|s| s.rfind('\n'))
            .map(|i| i + 1)
            .unwrap_or(0)
    }

    fn paragraph_end(&self, at: usize) -> usize {
        self.text
            .get(at..)
            .and_then(|s| s.find('\n'))
            .map(|i| at + i)
            .unwrap_or(self.text.len())
    }

    fn prev_grapheme(&self, at: usize) -> usize {
        let head = self.text.get(..at).unwrap_or_default();
        head.grapheme_indices(true)
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    fn next_grapheme(&self, at: usize) -> usize {
        let tail = self.text.get(at..).unwrap_or_default();
        tail.graphemes(true)
            .next()
            .map(|g| at + g.len())
            .unwrap_or(self.text.len())
    }

    fn word_left(&self, at: usize) -> usize {
        // Skip non-word characters, then the word (macOS ⌥←).
        let mut pos = 0;
        let mut found = false;
        for (i, w) in self.text.split_word_bound_indices() {
            if i >= at {
                break;
            }
            if w.chars().any(char::is_alphanumeric) {
                pos = i;
                found = true;
            }
        }
        if found { pos } else { 0 }
    }

    fn word_right(&self, at: usize) -> usize {
        for (i, w) in self.text.split_word_bound_indices() {
            let end = i + w.len();
            if end <= at {
                continue;
            }
            if w.chars().any(char::is_alphanumeric) {
                return end;
            }
        }
        self.text.len()
    }

    /// Where `movement` takes the caret from `at`. Line movements use `layout` (the laid-out
    /// text; without it, paragraphs stand for lines).
    pub fn target(&mut self, at: usize, movement: Movement, layout: Option<&TextLayout>) -> usize {
        let vertical = matches!(movement, Movement::Up | Movement::Down);
        if !vertical {
            self.goal_x = None;
        }
        match movement {
            Movement::Left => self.prev_grapheme(at),
            Movement::Right => self.next_grapheme(at),
            Movement::WordLeft => self.word_left(at),
            Movement::WordRight => self.word_right(at),
            Movement::LineStart => match layout {
                Some(l) => {
                    let lines = l.lines();
                    lines
                        .get(l.line_index(at, false))
                        .map(|li| li.start)
                        .unwrap_or(0)
                }
                None => self.paragraph_start(at),
            },
            Movement::LineEnd => match layout {
                Some(l) => {
                    let lines = l.lines();
                    lines
                        .get(l.line_index(at, false))
                        .map(|li| {
                            // Before a trailing space or newline of a wrapped line.
                            let mut e = li.end.min(self.text.len());
                            while e > li.start
                                && self.text.as_bytes().get(e - 1).is_some_and(|b| *b == b'\n')
                            {
                                e -= 1;
                            }
                            e
                        })
                        .unwrap_or(self.text.len())
                }
                None => self.paragraph_end(at),
            },
            Movement::ParagraphStart => self.paragraph_start(at),
            Movement::ParagraphEnd => self.paragraph_end(at),
            Movement::DocumentStart => 0,
            Movement::DocumentEnd => self.text.len(),
            Movement::Up | Movement::Down => {
                let Some(l) = layout else {
                    return if movement == Movement::Up {
                        0
                    } else {
                        self.text.len()
                    };
                };
                let caret = l.caret_rect(at, false);
                let x = *self.goal_x.get_or_insert(caret.x);
                let lines = l.lines();
                let li = l.line_index(at, false);
                let target_line = if movement == Movement::Up {
                    if li == 0 {
                        return 0;
                    }
                    li - 1
                } else {
                    if li + 1 >= lines.len() {
                        return self.text.len();
                    }
                    li + 1
                };
                let Some(t) = lines.get(target_line) else {
                    return at;
                };
                let (off, _) = l.offset_at(Point::new(x, t.top + t.height / 2.0));
                self.snap(off)
            }
        }
    }

    /// Moves the caret (`extend`: the selection grows instead).
    pub fn move_caret(&mut self, movement: Movement, extend: bool, layout: Option<&TextLayout>) {
        let sel = self.selection;
        let from = if !extend && !sel.is_collapsed() {
            // A collapsed move from a selection goes to the selection's edge.
            match movement {
                Movement::Left | Movement::Up => {
                    if movement == Movement::Left {
                        self.selection = Selection::caret(sel.range().start);
                        self.goal_x = None;
                        return;
                    }
                    sel.range().start
                }
                Movement::Right | Movement::Down => {
                    if movement == Movement::Right {
                        self.selection = Selection::caret(sel.range().end);
                        self.goal_x = None;
                        return;
                    }
                    sel.range().end
                }
                _ => sel.focus,
            }
        } else {
            sel.focus
        };
        let to = self.target(from, movement, layout);
        self.selection = if extend {
            Selection {
                anchor: sel.anchor,
                focus: to,
            }
        } else {
            Selection::caret(to)
        };
    }

    fn apply(&mut self, range: Range<usize>, inserted: &str, kind: Kind, now: f64) {
        let range = self.snap(range.start)..self.snap(range.end.max(range.start));
        let mut inserted = if self.single_line {
            inserted.replace(['\n', '\r'], " ")
        } else {
            inserted.to_string()
        };
        if let Some(max) = self.max_chars {
            let current = self.text.chars().count()
                - self
                    .text
                    .get(range.clone())
                    .map(|s| s.chars().count())
                    .unwrap_or(0);
            let room = max.saturating_sub(current);
            if inserted.chars().count() > room {
                inserted = inserted.chars().take(room).collect();
            }
        }
        let removed = self.text.get(range.clone()).unwrap_or_default().to_string();
        if removed.is_empty() && inserted.is_empty() {
            return;
        }
        let before = self.selection;
        self.splice(range.clone(), &inserted);
        let caret = range.start + inserted.len();
        self.selection = Selection::caret(caret);
        let change = Change {
            at: range.start,
            removed,
            inserted,
            before,
            after: self.selection,
        };
        let coalesce = kind != Kind::Other
            && kind == self.last_kind
            && now - self.last_time < UNDO_COALESCE
            && self
                .undo
                .last()
                .and_then(|g| g.last())
                .is_some_and(|c| c.after == before);
        match self.undo.last_mut() {
            Some(group) if coalesce => group.push(change),
            _ => self.undo.push(vec![change]),
        }
        if self.undo.len() > 1000 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.last_kind = kind;
        self.last_time = now;
        self.goal_x = None;
    }

    /// Types `text` over the selection (also a paste or a committed input method string).
    pub fn insert(&mut self, text: &str, now: f64) {
        self.preedit = None;
        let kind = if text.chars().count() == 1 && !text.contains('\n') {
            Kind::Typing
        } else {
            Kind::Other
        };
        self.apply(self.selection.range(), text, kind, now);
    }

    /// Replaces a range (programmatic, undoable as one step).
    pub fn replace(&mut self, range: Range<usize>, text: &str, now: f64) {
        self.apply(range, text, Kind::Other, now);
    }

    /// Deletes the selection, or from the caret to where `movement` goes.
    pub fn delete(&mut self, movement: Movement, now: f64) {
        let sel = self.selection;
        let range = if sel.is_collapsed() {
            let to = self.target(sel.focus, movement, None);
            sel.focus.min(to)..sel.focus.max(to)
        } else {
            sel.range()
        };
        self.apply(range, "", Kind::Deleting, now);
    }

    /// Undoes the last group; returns whether there was one.
    pub fn undo(&mut self) -> bool {
        let Some(group) = self.undo.pop() else {
            return false;
        };
        for c in group.iter().rev() {
            let end = c.at + c.inserted.len();
            if self.text.is_char_boundary(c.at)
                && self.text.is_char_boundary(end.min(self.text.len()))
            {
                let r = c.at..end.min(self.text.len());
                let removed = c.removed.clone();
                self.splice(r, &removed);
            }
            self.selection = c.before;
        }
        self.redo.push(group);
        self.last_kind = Kind::Other;
        true
    }

    /// Redoes the last undone group.
    pub fn redo(&mut self) -> bool {
        let Some(group) = self.redo.pop() else {
            return false;
        };
        for c in &group {
            let end = (c.at + c.removed.len()).min(self.text.len());
            if self.text.is_char_boundary(c.at) && self.text.is_char_boundary(end) {
                let ins = c.inserted.clone();
                self.splice(c.at..end, &ins);
            }
            self.selection = c.after;
        }
        self.undo.push(group);
        self.last_kind = Kind::Other;
        true
    }

    /// Whether there is something to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is something to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Sets the input method's pre-edit (empty text ends the composition).
    pub fn set_preedit(&mut self, text: &str, cursor: Option<(usize, usize)>) {
        self.preedit = if text.is_empty() {
            None
        } else {
            Some(Preedit {
                text: text.to_string(),
                cursor,
            })
        };
    }

    /// The input method deletes `before` bytes before and `after` bytes after the caret.
    pub fn delete_surrounding(&mut self, before: usize, after: usize, now: f64) {
        let r = self.selection.range();
        let start = self.snap(r.start.saturating_sub(before));
        let end = self.snap((r.end + after).min(self.text.len()));
        self.apply(start..end, "", Kind::Other, now);
    }

    /// Handles a key with the macOS bindings (⌘ = Logo). `layout` serves line movement.
    pub fn handle_key(
        &mut self,
        key: &KeyEvent,
        layout: Option<&TextLayout>,
        now: f64,
    ) -> EditAction {
        if !key.pressed {
            return EditAction::Unhandled;
        }
        let m = key.mods - Mods::CAPS;
        let shift = m.contains(Mods::SHIFT);
        let cmd = m.contains(Mods::LOGO);
        let alt = m.contains(Mods::ALT);
        let ctrl = m.contains(Mods::CTRL);
        let mv = |e: &mut Editor, mv: Movement| {
            e.move_caret(mv, shift, layout);
            EditAction::Handled
        };
        match &key.key {
            Key::Left if cmd => mv(self, Movement::LineStart),
            Key::Right if cmd => mv(self, Movement::LineEnd),
            Key::Left if alt => mv(self, Movement::WordLeft),
            Key::Right if alt => mv(self, Movement::WordRight),
            Key::Left => mv(self, Movement::Left),
            Key::Right => mv(self, Movement::Right),
            Key::Up if cmd => mv(self, Movement::DocumentStart),
            Key::Down if cmd => mv(self, Movement::DocumentEnd),
            Key::Up if alt => mv(self, Movement::ParagraphStart),
            Key::Down if alt => mv(self, Movement::ParagraphEnd),
            Key::Up if self.single_line => mv(self, Movement::DocumentStart),
            Key::Down if self.single_line => mv(self, Movement::DocumentEnd),
            Key::Up => mv(self, Movement::Up),
            Key::Down => mv(self, Movement::Down),
            Key::Home => mv(self, Movement::LineStart),
            Key::End => mv(self, Movement::LineEnd),
            Key::Char('a') if ctrl && !cmd => mv(self, Movement::LineStart),
            Key::Char('e') if ctrl && !cmd => mv(self, Movement::LineEnd),
            Key::Backspace => {
                let movement = if cmd {
                    Movement::LineStart
                } else if alt || ctrl {
                    Movement::WordLeft
                } else {
                    Movement::Left
                };
                self.delete(movement, now);
                EditAction::Handled
            }
            Key::Delete => {
                let movement = if cmd {
                    Movement::LineEnd
                } else if alt || ctrl {
                    Movement::WordRight
                } else {
                    Movement::Right
                };
                self.delete(movement, now);
                EditAction::Handled
            }
            Key::Char('k') if ctrl && !cmd => {
                self.delete(Movement::ParagraphEnd, now);
                EditAction::Handled
            }
            Key::Char('a') if cmd => {
                self.select_all();
                EditAction::Handled
            }
            Key::Char('c') if cmd => {
                if self.selection.is_collapsed() {
                    EditAction::Handled
                } else {
                    EditAction::Copy(self.selected_text().to_string())
                }
            }
            Key::Char('x') if cmd => {
                if self.selection.is_collapsed() {
                    EditAction::Handled
                } else {
                    let t = self.selected_text().to_string();
                    self.apply(self.selection.range(), "", Kind::Other, now);
                    EditAction::Copy(t)
                }
            }
            Key::Char('v') if cmd => EditAction::Paste,
            Key::Char('z') if cmd && shift => {
                self.redo();
                EditAction::Handled
            }
            Key::Char('z') if cmd => {
                self.undo();
                EditAction::Handled
            }
            Key::Enter if self.single_line => EditAction::Submit,
            Key::Enter if m.is_empty() || m == Mods::SHIFT => {
                self.insert("\n", now);
                EditAction::Handled
            }
            Key::Tab if !self.single_line && m.is_empty() => {
                self.insert("\t", now);
                EditAction::Handled
            }
            Key::Escape => EditAction::Cancel,
            _ if cmd || ctrl => EditAction::Unhandled,
            _ => match &key.text {
                Some(t) if !t.is_empty() && !t.chars().any(char::is_control) => {
                    self.insert(t, now);
                    EditAction::Handled
                }
                _ => EditAction::Unhandled,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: Key, mods: Mods) -> KeyEvent {
        KeyEvent::press(key, mods)
    }

    #[test]
    fn typing_and_grouped_undo() {
        let mut e = Editor::new("", false);
        for (i, c) in "hello".chars().enumerate() {
            e.handle_key(&press(Key::Char(c), Mods::empty()), None, i as f64 * 0.1);
        }
        assert_eq!(e.text(), "hello");
        e.handle_key(&press(Key::Space, Mods::empty()), None, 5.0);
        e.handle_key(&press(Key::Char('w'), Mods::empty()), None, 5.1);
        assert_eq!(e.text(), "hello w");
        // The pause made two groups.
        assert!(e.undo());
        assert_eq!(e.text(), "hello");
        assert!(e.undo());
        assert_eq!(e.text(), "");
        assert!(!e.undo());
        assert!(e.redo());
        assert_eq!(e.text(), "hello");
    }

    #[test]
    fn graphemes_and_words() {
        let mut e = Editor::new("a👍🏽 b cd", false);
        e.set_selection(Selection::caret(0));
        e.move_caret(Movement::Right, false, None);
        e.move_caret(Movement::Right, false, None);
        // The thumbs up with its skin tone is one grapheme.
        assert_eq!(e.selection().focus, "a👍🏽".len());
        e.handle_key(&press(Key::Right, Mods::ALT), None, 0.0);
        assert_eq!(e.selection().focus, "a👍🏽 b".len());
        e.handle_key(&press(Key::Left, Mods::ALT | Mods::SHIFT), None, 0.0);
        assert_eq!(e.selected_text(), "b");
        e.handle_key(&press(Key::Backspace, Mods::empty()), None, 0.0);
        assert_eq!(e.text(), "a👍🏽  cd");
        e.handle_key(&press(Key::Backspace, Mods::LOGO), None, 0.0);
        assert_eq!(e.text(), " cd");
    }

    #[test]
    fn clipboard_actions_and_single_line() {
        let mut e = Editor::new("one\ntwo", true);
        assert_eq!(e.text(), "one two");
        e.handle_key(&press(Key::Char('a'), Mods::LOGO), None, 0.0);
        assert_eq!(
            e.handle_key(&press(Key::Char('x'), Mods::LOGO), None, 0.0),
            EditAction::Copy("one two".into())
        );
        assert_eq!(e.text(), "");
        assert_eq!(
            e.handle_key(&press(Key::Char('v'), Mods::LOGO), None, 0.0),
            EditAction::Paste
        );
        e.insert("pasted\ntext", 0.0);
        assert_eq!(e.text(), "pasted text");
        assert_eq!(
            e.handle_key(&press(Key::Enter, Mods::empty()), None, 0.0),
            EditAction::Submit
        );
        e.max_chars = Some(12);
        e.insert("abc", 0.0);
        assert_eq!(e.text(), "pasted texta");
    }

    #[test]
    fn preedit_and_surrounding() {
        let mut e = Editor::new("ab", false);
        e.set_selection(Selection::caret(1));
        e.set_preedit("か", Some((3, 3)));
        assert_eq!(e.display_text(), ("aかb".to_string(), Some(1..4)));
        e.insert("漢", 0.0);
        assert_eq!(e.text(), "a漢b");
        assert!(e.preedit().is_none());
        e.delete_surrounding(3, 0, 0.0);
        assert_eq!(e.text(), "ab");
    }

    #[test]
    fn vertical_movement_keeps_the_column() {
        let style = crate::text::TextStyle::default();
        let mut e = Editor::new("first line\nsecond\nthird line", false);
        let layout = TextLayout::plain(e.text(), &style, f32::INFINITY);
        e.set_selection(Selection::caret(10));
        e.move_caret(Movement::Down, false, Some(&layout));
        // "second" is shorter than "first line": the caret goes to its end.
        assert_eq!(e.selection().focus, "first line\nsecond".len());
        e.move_caret(Movement::Down, false, Some(&layout));
        let third = "first line\nsecond\n".len();
        assert!(e.selection().focus > third + 5 && e.selection().focus <= third + 10);
        e.move_caret(Movement::Up, false, Some(&layout));
        e.move_caret(Movement::Up, false, Some(&layout));
        // Back on the first line at the column it started from.
        assert_eq!(e.selection().focus, 10);
    }

    #[test]
    fn changes_are_tracked_and_merged() {
        let mut e = Editor::new("hello world", false);
        e.take_change();
        e.set_selection(Selection::caret(5));
        e.insert("!", 0.0);
        assert_eq!(e.take_change(), Some((5, 5, 6)));
        e.insert("a", 0.0);
        e.set_selection(Selection::caret(0));
        e.insert("b", 0.0);
        // "b" at 0 and "a" at 7 (old 6): the span covers both.
        assert_eq!(e.take_change(), Some((0, 6, 8)));
        assert_eq!(e.text(), "bhello!a world");
        e.undo();
        assert!(e.take_change().is_some());
        assert_eq!(e.take_change(), None);
    }

    #[test]
    fn select_word_and_paragraph() {
        let mut e = Editor::new("alpha beta\ngamma", false);
        e.select_word_at(7);
        assert_eq!(e.selected_text(), "beta");
        e.select_paragraph_at(13);
        assert_eq!(e.selected_text(), "gamma");
    }
}
