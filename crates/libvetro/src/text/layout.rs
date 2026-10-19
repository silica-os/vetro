//! Text styles, attributed strings and laid-out paragraphs (SkParagraph: HarfBuzz shaping,
//! ICU bidi and line breaking).
//!
//! Every offset in this API is a UTF-8 byte offset into the paragraph's text; SkParagraph's
//! UTF-16 positions are converted here and never leak out.

use std::ops::Range;

use skia_safe::textlayout::{
    Affinity, ParagraphBuilder, ParagraphStyle, PlaceholderAlignment, PlaceholderStyle,
    RectHeightStyle, RectWidthStyle, TextAlign, TextBaseline, TextDecoration, TextDirection,
    TextStyle as SkTextStyle,
};
use skia_safe::{Canvas, FontStyle, font_style};

use crate::amalfi::tokens::TypeStyle;
use crate::color::Color;
use crate::geom::{Point, Rect, Size};
use crate::text::fonts::{Fonts, UI_FAMILY};

/// Horizontal alignment of lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// Leading edge (left in left-to-right text).
    #[default]
    Start,
    /// Centred.
    Center,
    /// Trailing edge.
    End,
    /// Justified.
    Justify,
}

/// The style of a run of text.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// Font family.
    pub family: String,
    /// Size in points.
    pub size: f32,
    /// CSS weight.
    pub weight: u16,
    /// Italic.
    pub italic: bool,
    /// Line height in points (`None`: natural); the extra space is split evenly above and
    /// below the glyphs, as in the kit.
    pub line_height: Option<f32>,
    /// Tracking in points.
    pub tracking: f32,
    /// Colour.
    pub color: Color,
    /// OpenType features (`tnum` …).
    pub features: Vec<String>,
    /// Underline.
    pub underline: bool,
    /// Strikethrough.
    pub strikethrough: bool,
    /// Background highlight.
    pub background: Option<Color>,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            family: UI_FAMILY.to_string(),
            size: 13.0,
            weight: 400,
            italic: false,
            line_height: Some(18.0),
            tracking: 0.0,
            color: Color::BLACK,
            features: Vec::new(),
            underline: false,
            strikethrough: false,
            background: None,
        }
    }
}

impl TextStyle {
    /// The style of a type token in `color`.
    pub fn from_token(t: &TypeStyle, color: Color) -> Self {
        TextStyle {
            family: t.family.to_string(),
            size: t.size,
            weight: t.weight,
            italic: false,
            line_height: t.line_height,
            tracking: t.tracking,
            color,
            features: t.features.iter().map(|s| (*s).to_string()).collect(),
            underline: false,
            strikethrough: false,
            background: None,
        }
    }

    /// This style with another colour.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// This style with another weight.
    pub fn with_weight(mut self, weight: u16) -> Self {
        self.weight = weight;
        self
    }

    fn to_sk(&self) -> SkTextStyle {
        let mut s = SkTextStyle::new();
        s.set_font_families(&[self.family.as_str(), UI_FAMILY]);
        s.set_font_size(self.size.max(0.0));
        let slant = if self.italic {
            font_style::Slant::Italic
        } else {
            font_style::Slant::Upright
        };
        s.set_font_style(FontStyle::new(
            font_style::Weight::from(i32::from(self.weight)),
            font_style::Width::NORMAL,
            slant,
        ));
        s.set_color(self.color.to_sk());
        if let Some(lh) = self.line_height
            && self.size > 0.0
            && lh > 0.0
        {
            s.set_height_override(true);
            s.set_height(lh / self.size);
            s.set_half_leading(true);
        }
        if self.tracking != 0.0 {
            s.set_letter_spacing(self.tracking);
        }
        for f in &self.features {
            s.add_font_feature(f.as_str(), 1);
        }
        let mut deco = TextDecoration::NO_DECORATION;
        if self.underline {
            deco |= TextDecoration::UNDERLINE;
        }
        if self.strikethrough {
            deco |= TextDecoration::LINE_THROUGH;
        }
        s.set_decoration_type(deco);
        if self.underline || self.strikethrough {
            s.set_decoration_color(self.color.to_sk());
        }
        if let Some(bg) = self.background {
            let mut p = skia_safe::Paint::default();
            p.set_color(bg.to_sk());
            s.set_background_paint(&p);
        }
        s
    }
}

/// An inline attachment: a box of `size` in the line (an image, a token, a symbol view).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Attachment {
    /// Size of the box.
    pub size: Size,
    /// Distance from the box's top to the baseline.
    pub baseline: f32,
}

/// A run of attributes over a byte range.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    /// Byte range in the text.
    pub range: Range<usize>,
    /// The style.
    pub style: TextStyle,
    /// A link target.
    pub link: Option<String>,
}

/// Text with per-range styles, links and attachments.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AttributedString {
    /// The text.
    pub text: String,
    /// Runs over the text; gaps use the base style, later runs win on overlap.
    pub runs: Vec<Run>,
    /// Attachments at byte offsets (each replaces the object replacement character U+FFFC
    /// at that offset).
    pub attachments: Vec<(usize, Attachment)>,
}

impl AttributedString {
    /// Plain text.
    pub fn plain(text: impl Into<String>) -> Self {
        AttributedString {
            text: text.into(),
            runs: Vec::new(),
            attachments: Vec::new(),
        }
    }

    /// Sets `style` (and `link`) over `range`.
    pub fn set(&mut self, range: Range<usize>, style: TextStyle, link: Option<String>) {
        let end = range.end.min(self.text.len());
        let start = range.start.min(end);
        self.runs.push(Run {
            range: start..end,
            style,
            link,
        });
    }

    /// The link at byte offset `at`.
    pub fn link_at(&self, at: usize) -> Option<&str> {
        self.runs
            .iter()
            .rev()
            .find(|r| r.range.contains(&at))
            .and_then(|r| r.link.as_deref())
    }
}

/// How a paragraph is laid out.
#[derive(Debug, Clone, PartialEq)]
pub struct ParagraphOptions {
    /// Alignment.
    pub align: Align,
    /// Right-to-left base direction.
    pub rtl: bool,
    /// Maximum lines (`None`: unlimited).
    pub max_lines: Option<usize>,
    /// Ellipsis when the text is cut.
    pub ellipsis: bool,
}

impl Default for ParagraphOptions {
    fn default() -> Self {
        ParagraphOptions {
            align: Align::Start,
            rtl: false,
            max_lines: None,
            ellipsis: true,
        }
    }
}

/// One line of a laid-out paragraph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineInfo {
    /// Byte range of the line, without its trailing newline.
    pub start: usize,
    /// End byte (exclusive).
    pub end: usize,
    /// Top of the line box.
    pub top: f32,
    /// Height of the line box.
    pub height: f32,
    /// Baseline, from the paragraph's top.
    pub baseline: f32,
    /// Left of the first glyph.
    pub left: f32,
    /// Width of the glyphs.
    pub width: f32,
}

/// A laid-out paragraph.
pub struct TextLayout {
    paragraph: skia_safe::textlayout::Paragraph,
    text: String,
    /// UTF-16 index of each UTF-8 byte boundary we care about: `utf16_of[i]` for byte i at a
    /// char boundary.
    utf16_starts: Vec<(usize, usize)>,
    width: f32,
}

impl std::fmt::Debug for TextLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextLayout")
            .field("text", &self.text)
            .field("width", &self.width)
            .finish()
    }
}

impl TextLayout {
    /// Lays out `text` in `base` style (and its runs) for `width` (infinite: one line per
    /// paragraph).
    pub fn new(
        text: &AttributedString,
        base: &TextStyle,
        options: &ParagraphOptions,
        width: f32,
    ) -> TextLayout {
        let mut ps = ParagraphStyle::new();
        ps.set_text_style(&base.to_sk());
        ps.set_text_align(match options.align {
            Align::Start => TextAlign::Start,
            Align::Center => TextAlign::Center,
            Align::End => TextAlign::End,
            Align::Justify => TextAlign::Justify,
        });
        ps.set_text_direction(if options.rtl {
            TextDirection::RTL
        } else {
            TextDirection::LTR
        });
        if let Some(n) = options.max_lines {
            ps.set_max_lines(n.max(1));
            if options.ellipsis {
                ps.set_ellipsis("…");
            }
        }
        ps.set_apply_rounding_hack(false);
        let paragraph = Fonts::with(|fonts| {
            let mut b = ParagraphBuilder::new(&ps, fonts.collection().clone());
            let s = &text.text;
            // Boundaries where the style changes.
            let mut cuts: Vec<usize> = vec![0, s.len()];
            for r in &text.runs {
                cuts.push(r.range.start.min(s.len()));
                cuts.push(r.range.end.min(s.len()));
            }
            for (at, _) in &text.attachments {
                cuts.push((*at).min(s.len()));
                cuts.push(next_char(s, *at));
            }
            cuts.retain(|c| s.is_char_boundary(*c));
            cuts.sort_unstable();
            cuts.dedup();
            for w in cuts.windows(2) {
                let (Some(&a), Some(&z)) = (w.first(), w.get(1)) else {
                    continue;
                };
                if a >= z {
                    continue;
                }
                let style = text
                    .runs
                    .iter()
                    .rev()
                    .find(|r| r.range.start <= a && r.range.end >= z)
                    .map(|r| &r.style)
                    .unwrap_or(base);
                if let Some((_, att)) = text.attachments.iter().find(|(at, _)| *at == a) {
                    b.push_style(&style.to_sk());
                    b.add_placeholder(&PlaceholderStyle::new(
                        att.size.width,
                        att.size.height,
                        PlaceholderAlignment::Baseline,
                        TextBaseline::Alphabetic,
                        att.baseline,
                    ));
                    b.pop();
                    if let Some(rest) = s.get(next_char(s, a)..z)
                        && !rest.is_empty()
                    {
                        b.push_style(&style.to_sk());
                        b.add_text(rest);
                        b.pop();
                    }
                    continue;
                }
                if let Some(slice) = s.get(a..z) {
                    b.push_style(&style.to_sk());
                    b.add_text(slice);
                    b.pop();
                }
            }
            b.build()
        });
        let mut utf16_starts = Vec::with_capacity(text.text.len() + 1);
        let mut u16i = 0usize;
        for (i, ch) in text.text.char_indices() {
            utf16_starts.push((i, u16i));
            u16i += ch.len_utf16();
        }
        utf16_starts.push((text.text.len(), u16i));
        let mut layout = TextLayout {
            paragraph,
            text: text.text.clone(),
            utf16_starts,
            width: f32::INFINITY,
        };
        layout.relayout(width);
        layout
    }

    /// Lays out plain `text`.
    pub fn plain(text: &str, style: &TextStyle, width: f32) -> TextLayout {
        TextLayout::new(
            &AttributedString::plain(text),
            style,
            &ParagraphOptions::default(),
            width,
        )
    }

    /// Lays out again for another width.
    pub fn relayout(&mut self, width: f32) {
        let w = if width.is_finite() && width > 0.0 {
            width
        } else if width.is_finite() {
            0.0
        } else {
            1.0e7
        };
        self.paragraph.layout(w);
        self.width = width;
    }

    /// The text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The width laid out for.
    pub fn layout_width(&self) -> f32 {
        self.width
    }

    /// The size of the text: the longest line's width and the total height.
    pub fn size(&self) -> Size {
        Size::new(self.paragraph.longest_line(), self.paragraph.height())
    }

    /// The width with no wrapping.
    pub fn max_intrinsic_width(&self) -> f32 {
        self.paragraph.max_intrinsic_width()
    }

    /// The narrowest width without breaking words.
    pub fn min_intrinsic_width(&self) -> f32 {
        self.paragraph.min_intrinsic_width()
    }

    /// The first line's baseline from the top.
    pub fn first_baseline(&self) -> f32 {
        self.paragraph.alphabetic_baseline()
    }

    /// Whether lines were cut by `max_lines`.
    pub fn did_exceed_max_lines(&self) -> bool {
        self.paragraph.did_exceed_max_lines()
    }

    /// Paints the text with its top-left at `origin`.
    pub fn paint(&self, canvas: &Canvas, origin: Point) {
        self.paragraph.paint(canvas, (origin.x, origin.y));
    }

    fn to_utf16(&self, byte: usize) -> usize {
        match self.utf16_starts.binary_search_by(|(b, _)| b.cmp(&byte)) {
            Ok(i) => self.utf16_starts.get(i).map(|(_, u)| *u).unwrap_or(0),
            Err(i) => self
                .utf16_starts
                .get(i.saturating_sub(1))
                .map(|(_, u)| *u)
                .unwrap_or(0),
        }
    }

    fn to_utf8(&self, u16i: usize) -> usize {
        match self.utf16_starts.binary_search_by(|(_, u)| u.cmp(&u16i)) {
            Ok(i) => self.utf16_starts.get(i).map(|(b, _)| *b).unwrap_or(0),
            Err(i) => self
                .utf16_starts
                .get(i.saturating_sub(1))
                .map(|(b, _)| *b)
                .unwrap_or(0),
        }
    }

    /// The byte offset nearest to `p` (paragraph coordinates), and whether the caret sits
    /// at the end of the previous line (upstream affinity).
    pub fn offset_at(&self, p: Point) -> (usize, bool) {
        let pos = self.paragraph.get_glyph_position_at_coordinate((p.x, p.y));
        let off = usize::try_from(pos.position.max(0)).unwrap_or(0);
        (self.to_utf8(off), pos.affinity == Affinity::Upstream)
    }

    /// The rectangles covering the byte range (selection highlight), tight to the line boxes.
    pub fn selection_rects(&self, range: Range<usize>) -> Vec<Rect> {
        if range.start >= range.end {
            return Vec::new();
        }
        let a = self.to_utf16(range.start);
        let z = self.to_utf16(range.end);
        self.paragraph
            .get_rects_for_range(a..z, RectHeightStyle::Max, RectWidthStyle::Tight)
            .iter()
            .map(|b| Rect::new(b.rect.left, b.rect.top, b.rect.width(), b.rect.height()))
            .collect()
    }

    /// The caret rectangle (1 pt wide) at byte offset `at`; `upstream` puts a caret at a
    /// line break at the end of the first line.
    pub fn caret_rect(&self, at: usize, upstream: bool) -> Rect {
        let lines = self.lines();
        let at = at.min(self.text.len());
        let line = self.line_index(at, upstream);
        let Some(info) = lines.get(line) else {
            let h = self.paragraph.height().max(1.0);
            return Rect::new(0.0, 0.0, 1.0, h);
        };
        // The x of the caret: the leading edge of the character at `at`, or the trailing edge
        // of the previous one at the end of a line.
        let x = if at < info.end {
            let next = next_char(&self.text, at);
            let rects = self.selection_rects(at..next);
            match rects.first() {
                Some(r) => r.x,
                None => info.left,
            }
        } else if at > info.start {
            let prev = prev_char(&self.text, at);
            let rects = self.selection_rects(prev..at);
            match rects.last() {
                Some(r) => r.right(),
                None => info.left + info.width,
            }
        } else {
            info.left
        };
        Rect::new(x, info.top, 1.0, info.height)
    }

    /// The index of the line containing byte `at`.
    pub fn line_index(&self, at: usize, upstream: bool) -> usize {
        let lines = self.lines();
        for (i, l) in lines.iter().enumerate() {
            if at < l.end || (at == l.end && (upstream || i + 1 == lines.len())) {
                return i;
            }
            if let Some(next) = lines.get(i + 1)
                && at < next.start
            {
                return i;
            }
        }
        lines.len().saturating_sub(1)
    }

    /// The lines.
    pub fn lines(&self) -> Vec<LineInfo> {
        self.paragraph
            .get_line_metrics()
            .iter()
            .map(|m| LineInfo {
                start: self.to_utf8(m.start_index),
                end: self.to_utf8(m.end_index.max(m.start_index)),
                top: (m.baseline - m.ascent) as f32,
                height: m.height as f32,
                baseline: m.baseline as f32,
                left: m.left as f32,
                width: m.width as f32,
            })
            .collect()
    }

    /// The byte range of the word at `at` (double-click selection).
    pub fn word_at(&self, at: usize) -> Range<usize> {
        let u = self.to_utf16(at);
        let r = self
            .paragraph
            .get_word_boundary(u32::try_from(u).unwrap_or(0));
        self.to_utf8(r.start)..self.to_utf8(r.end)
    }
}

/// The byte offset after the character at `at`.
pub fn next_char(s: &str, at: usize) -> usize {
    if at >= s.len() {
        return s.len();
    }
    let mut i = at + 1;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// The byte offset of the character before `at`.
pub fn prev_char(s: &str, at: usize) -> usize {
    if at == 0 {
        return 0;
    }
    let mut i = at.min(s.len()) - 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> TextStyle {
        TextStyle::default()
    }

    #[test]
    fn measures_with_line_height() {
        let l = TextLayout::plain("Add to favourites", &body(), f32::INFINITY);
        let s = l.size();
        assert_eq!(s.height, 18.0);
        // Inter 13 pt: about 100 pt wide.
        assert!(s.width > 90.0 && s.width < 115.0, "{}", s.width);
        // Even leading: the baseline sits a bit below the middle of the 18 pt line.
        let b = l.first_baseline();
        assert!(b > 12.0 && b < 15.0, "{b}");
    }

    #[test]
    fn wraps_and_hit_tests() {
        let l = TextLayout::plain("one two three four five six", &body(), 60.0);
        let lines = l.lines();
        assert!(lines.len() >= 3);
        assert_eq!(l.size().height, 18.0 * lines.len() as f32);
        let (off, _) = l.offset_at(Point::new(0.0, 1.0));
        assert_eq!(off, 0);
        let (off, _) = l.offset_at(Point::new(500.0, 1.0));
        assert!(off > 0 && off <= lines[0].end + 1);
    }

    #[test]
    fn utf8_offsets_round_trip() {
        let text = "a€😀b";
        let l = TextLayout::plain(text, &body(), f32::INFINITY);
        let r = l.selection_rects(1..4);
        assert!(!r.is_empty());
        let caret_end = l.caret_rect(text.len(), false);
        let caret_mid = l.caret_rect(4, false);
        assert!(caret_end.x > caret_mid.x);
        assert_eq!(l.word_at(0), 0..1);
    }

    #[test]
    fn attributed_runs_and_attachments() {
        let mut a = AttributedString::plain("Hello \u{FFFC} world");
        a.set(0..5, body().with_weight(700), Some("https://silica".into()));
        a.attachments.push((
            6,
            Attachment {
                size: Size::new(20.0, 10.0),
                baseline: 10.0,
            },
        ));
        let l = TextLayout::new(&a, &body(), &ParagraphOptions::default(), f32::INFINITY);
        assert!(l.size().width > 60.0);
        assert_eq!(a.link_at(2), Some("https://silica"));
        assert_eq!(a.link_at(7), None);
    }
}
