//! Titlebars (kit `Titlebars`): the window buttons, title texts, the glass search and view
//! switcher pieces, the horizontal strip that lays pieces out, bars and sections, the document
//! title bar and the tab bar.

use crate::amalfi::foundation::{Interaction as Transitions, Label};
use crate::amalfi::theme::Theme;
use crate::amalfi::tokens::{typography, window_controls};
use crate::color::Color;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Event, Handled, Key};
use crate::core::layout::Proposal;
use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size};
use crate::shape::Shape;
use crate::style::{Border, Paint, Shadow, Style};
use crate::text::Align;

/// Fixed measures of the kit.
pub mod metrics {
    /// A window button's diameter.
    pub const DOT: f32 = 16.0;
    /// Gap between window buttons.
    pub const DOT_GAP: f32 = 8.0;
    /// The three buttons' width.
    pub const CONTROLS_WIDTH: f32 = 64.0;
    /// A toolbar piece's height.
    pub const PIECE_HEIGHT: f32 = 52.0;
    /// The glass pieces' height.
    pub const GLASS_HEIGHT: f32 = 36.0;
    /// The glass pieces' corner radius.
    pub const GLASS_RADIUS: f32 = 18.0;
    /// A document title bar's height.
    pub const TITLE_BAR_HEIGHT: f32 = 48.0;
    /// A tab's height.
    pub const TAB_HEIGHT: f32 = 28.0;
    /// A tab's width alone.
    pub const TAB_WIDTH: f32 = 160.0;
    /// The tab bar's height.
    pub const TAB_BAR_HEIGHT: f32 = 36.0;
}

// ---------------------------------------------------------------------------------------
// Window buttons.
// ---------------------------------------------------------------------------------------

/// Which window button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DotKind {
    /// Close.
    Close,
    /// Minimise.
    Minimise,
    /// Zoom.
    Zoom,
}

/// One look of a window button: fill gradient top and bottom, rim, glyph colour.
struct DotLook {
    fill: [Color; 2],
    rim: Color,
    glyph: Color,
    /// The dark buttons' inner highlights: below (1 pt down) and above (1 pt up).
    inner: Option<[Color; 2]>,
}

impl DotKind {
    fn symbol(self) -> &'static str {
        match self {
            DotKind::Close => "xmark",
            DotKind::Minimise => "minus",
            DotKind::Zoom => "plus",
        }
    }

    fn name(self) -> &'static str {
        match self {
            DotKind::Close => "Close",
            DotKind::Minimise => "Minimise",
            DotKind::Zoom => "Zoom",
        }
    }

    /// The look (measured values of `docs/spec/windows.md` for Light at rest; the others are
    /// the kit's).
    fn look(self, dark: bool, inactive: bool, hover: bool) -> DotLook {
        let c = |v: u32| Color::from_u32(v);
        if inactive {
            return if dark {
                DotLook {
                    fill: [c(0x4d4d4dff), c(0x555555ff)],
                    rim: c(0x999999b3),
                    glyph: Color::TRANSPARENT,
                    inner: Some([c(0x999999ff), c(0x909091cc)]),
                }
            } else {
                DotLook {
                    fill: [c(0xf4f4f7ff), c(0xeaebecff)],
                    rim: c(0xc3c3c7ff),
                    glyph: Color::TRANSPARENT,
                    inner: None,
                }
            };
        }
        let rest = match self {
            DotKind::Close => window_controls::CLOSE,
            DotKind::Minimise => window_controls::MINIMISE,
            DotKind::Zoom => window_controls::ZOOM,
        };
        if dark {
            let (fill, rim, glyph, inner) = match self {
                DotKind::Close => (
                    [0xf8635aff, 0xee6660ff],
                    0xee5549ff,
                    0x8c0000ff,
                    [0xfab1a7ff, 0xfbada5cc],
                ),
                DotKind::Minimise => (
                    [0xfab500ff, 0xfdcd29ff],
                    0xf8b904ff,
                    0xa53a00ff,
                    [0xfbee5aff, 0xfbf869cc],
                ),
                DotKind::Zoom => (
                    [0x01bc00ff, 0x4bc133ff],
                    0x09b901ff,
                    0x2d7201ff,
                    [0x96fb78ff, 0x8bfb76cc],
                ),
            };
            return DotLook {
                fill: [c(fill[0]), c(fill[1])],
                rim: c(rim),
                glyph: c(glyph),
                inner: Some([c(inner[0]), c(inner[1])]),
            };
        }
        let (flat, glyph) = match self {
            DotKind::Close => (0xf97c72ff, 0xa30000ff),
            DotKind::Minimise => (0xffcf41ff, 0xc14400ff),
            DotKind::Zoom => (0x8dd969ff, 0x368800ff),
        };
        DotLook {
            fill: if hover { [c(flat), c(flat)] } else { rest.0 },
            rim: rest.1,
            glyph: c(glyph),
            inner: None,
        }
    }
}

/// A window button; the glyph shows while the pointer is over the group of buttons.
#[derive(Debug)]
pub struct Dot {
    /// Which button.
    pub kind: DotKind,
    /// Whether the window is active (`None`: ask the window).
    pub active: Option<bool>,
    /// Shows the glyph (`None`: while the pointer is over the button or its group).
    pub revealed: Option<bool>,
}

impl Dot {
    /// A button.
    pub fn new(kind: DotKind) -> Dot {
        Dot {
            kind,
            active: None,
            revealed: None,
        }
    }
}

impl Widget for Dot {
    fn kind(&self) -> &'static str {
        "WindowButton"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::DOT, metrics::DOT)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let group_hover = cx
            .ui
            .tree
            .parent(cx.id)
            .is_some_and(|p| cx.ui.tree.flags(p).contains(NodeFlags::HOVERED));
        let hover = self.revealed.unwrap_or(cx.is_hovered() || group_hover);
        let active = self.active.unwrap_or_else(|| cx.window_active());
        // An inactive window shows its buttons grey until the pointer comes.
        let inactive = !active && !hover;
        let look = self.kind.look(theme.dark(), inactive, hover);
        cx.rec.layer(
            "Dot",
            r,
            Shape::rounded(metrics::DOT / 2.0),
            Style {
                fills: vec![Paint::Linear {
                    from: Point::new(0.5, 0.0),
                    to: Point::new(0.5, 1.0),
                    stops: vec![(0.0, look.fill[0]), (1.0, look.fill[1])],
                }],
                borders: vec![Border::inside(look.rim, 0.5)],
                inner_shadows: look
                    .inner
                    .map(|[down, up]| {
                        vec![
                            Shadow {
                                color: down,
                                x: 0.0,
                                y: 1.0,
                                blur: 0.0,
                                spread: 0.0,
                            },
                            Shadow {
                                color: up,
                                x: 0.0,
                                y: -1.0,
                                blur: 0.0,
                                spread: 0.0,
                            },
                        ]
                    })
                    .unwrap_or_default(),
                ..Style::default()
            },
        );
        if hover {
            cx.rec.glyph(
                "Glyph",
                Rect::new(r.x + 2.0, r.y + 2.0, 12.0, 12.0),
                self.kind.symbol(),
                400,
                look.glyph,
            );
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Activate => {
                cx.emit(Signal::Activated, Value::None);
                Handled::Yes
            }
            Event::PointerEnter | Event::PointerLeave => {
                cx.request_paint();
                Handled::No
            }
            _ => Handled::No,
        }
    }

    fn interaction(&self) -> Interaction {
        Interaction::Button
    }

    fn focus(&self) -> FocusKind {
        FocusKind::None
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Button;
        a.label = Some(self.kind.name().to_string());
        a.actions = vec![Action::Click];
    }
}

/// The three window buttons (children `Close`, `Minimise` and `Zoom`, left to right).
#[derive(Debug, Default)]
pub struct WindowControls;

impl Widget for WindowControls {
    fn kind(&self) -> &'static str {
        "WindowControls"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        for c in cx.children() {
            let _ = cx.measure(c, Proposal::new(None, None));
        }
        Size::new(metrics::CONTROLS_WIDTH, metrics::DOT)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, _size: Size) {
        for (i, c) in cx.children().into_iter().enumerate() {
            cx.place(
                c,
                Rect::new(
                    i as f32 * (metrics::DOT + metrics::DOT_GAP),
                    0.0,
                    metrics::DOT,
                    metrics::DOT,
                ),
            );
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if matches!(event, Event::PointerEnter | Event::PointerLeave) {
            // The glyphs show on all three while the pointer is over any of them.
            let id = cx.id();
            let kids = cx.ui().tree.children(id).to_vec();
            for k in kids {
                cx.ui().request_paint(k);
            }
        }
        Handled::No
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
        a.label = Some("Window controls".to_string());
    }
}

/// Makes the three buttons.
pub fn window_controls_view(ui: &mut Ui) -> ViewId {
    let g = ui.create(WindowControls);
    for kind in [DotKind::Close, DotKind::Minimise, DotKind::Zoom] {
        let d = ui.create(Dot::new(kind));
        let _ = ui.set(d, Prop::Identifier, Value::from(kind.name()));
        ui.add_child(g, d);
    }
    g
}

// ---------------------------------------------------------------------------------------
// Title texts.
// ---------------------------------------------------------------------------------------

/// What a title block shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TitleStyle {
    /// The title alone.
    #[default]
    Plain,
    /// A subtitle below the title.
    Subtitle,
    /// A document proxy icon before the title and a chevron after it.
    Document,
}

/// The title of a window: the title, a subtitle under it or a document icon and a chevron.
#[derive(Debug)]
pub struct TitleBlock {
    /// The title.
    pub title: String,
    /// The subtitle.
    pub subtitle: String,
    /// What it shows.
    pub style: TitleStyle,
    /// The name of the document icon layer (`Document proxy` in the title bar, `Document` in
    /// the toolbar piece).
    pub document_layer: &'static str,
    /// Space at both sides (the toolbar piece's 8 pt).
    pub padding: f32,
    /// Whether the window is active (`None`: ask the window).
    pub active: Option<bool>,
    /// Centres the title in the width it is given (it fills it).
    pub fill: bool,
    /// The title is left-aligned in the width it is given.
    pub align: Align,
    /// The height to centre the text in (a toolbar piece is 52 pt high).
    pub height: Option<f32>,
    title_text: Label,
    subtitle_text: Label,
}

impl TitleBlock {
    /// A title.
    pub fn new(title: &str) -> TitleBlock {
        TitleBlock {
            title: title.to_string(),
            subtitle: String::new(),
            style: TitleStyle::Plain,
            document_layer: "Document proxy",
            padding: 0.0,
            active: None,
            fill: false,
            align: Align::Start,
            height: None,
            title_text: Label::default(),
            subtitle_text: Label::default(),
        }
    }

    fn title_width(&mut self, theme: &Theme) -> f32 {
        let s = theme.text(&typography::HEADLINE, theme.colors.text_primary);
        self.title_text.width(&self.title, &s).ceil()
    }

    fn subtitle_width(&mut self, theme: &Theme) -> f32 {
        let s = theme.text(&typography::CAPTION, theme.colors.text_tertiary);
        self.subtitle_text.width(&self.subtitle, &s).ceil()
    }

    fn content_size(&mut self, theme: &Theme) -> Size {
        let t = self.title_width(theme);
        match self.style {
            TitleStyle::Plain => Size::new(t, 18.0),
            TitleStyle::Subtitle => {
                let s = self.subtitle_width(theme);
                Size::new(t.max(s), 32.0)
            }
            TitleStyle::Document => Size::new(16.0 + 4.0 + t + 4.0 + 12.0, 18.0),
        }
    }
}

impl Widget for TitleBlock {
    fn kind(&self) -> &'static str {
        "TitleBlock"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let theme = cx.theme();
        let s = self.content_size(&theme);
        let w = match p.width {
            Some(w) if self.fill && w.is_finite() => w,
            _ => s.width + 2.0 * self.padding,
        };
        Size::new(w, self.height.unwrap_or(s.height).max(s.height))
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let mut r = cx.bounds();
        let active = self.active.unwrap_or_else(|| cx.window_active());
        let content_height = self.content_size(&theme).height;
        if r.height > content_height {
            r.y += ((r.height - content_height) / 2.0).floor();
        }
        let (title_color, secondary, tertiary) = (
            if active {
                c.text_primary
            } else {
                c.text_tertiary
            },
            if active {
                c.text_secondary
            } else {
                c.text_tertiary
            },
            c.text_tertiary,
        );
        let content = self.content_size(&theme);
        let inner = (r.width - 2.0 * self.padding).max(0.0);
        let title_style = theme.text(&typography::HEADLINE, title_color);
        let tw = self.title_width(&theme);
        match self.style {
            TitleStyle::Plain => {
                let layout = self.title_text.layout(
                    &self.title,
                    &title_style,
                    self.align,
                    inner.max(tw),
                    Some(1),
                );
                cx.rec.text(
                    "Title",
                    Rect::new(r.x + self.padding, r.y, inner.max(tw), 18.0),
                    layout,
                    Some(typography::HEADLINE.name),
                    title_color,
                );
            }
            TitleStyle::Subtitle => {
                let x = r.x + self.padding + (inner - content.width).max(0.0) / 2.0;
                let w = self.subtitle_width(&theme);
                let layout = self.subtitle_text.layout(
                    &self.subtitle,
                    &theme.text(&typography::CAPTION, tertiary),
                    Align::Start,
                    w,
                    Some(1),
                );
                cx.rec.text(
                    "Subtitle",
                    Rect::new(x, r.y + 18.0, w, 14.0),
                    layout,
                    Some(typography::CAPTION.name),
                    tertiary,
                );
                let tx = x + (content.width - tw) / 2.0;
                let layout =
                    self.title_text
                        .layout(&self.title, &title_style, Align::Start, tw, Some(1));
                cx.rec.text(
                    "Title",
                    Rect::new(tx, r.y, tw, 18.0),
                    layout,
                    Some(typography::HEADLINE.name),
                    title_color,
                );
            }
            TitleStyle::Document => {
                let x = r.x + self.padding + (inner - content.width).max(0.0) / 2.0;
                cx.rec.glyph(
                    self.document_layer,
                    Rect::new(x, r.y + 1.0, 16.0, 16.0),
                    "doc",
                    400,
                    secondary,
                );
                let layout =
                    self.title_text
                        .layout(&self.title, &title_style, Align::Start, tw, Some(1));
                cx.rec.text(
                    "Title",
                    Rect::new(x + 20.0, r.y, tw, 18.0),
                    layout,
                    Some(typography::HEADLINE.name),
                    title_color,
                );
                cx.rec.glyph(
                    "Chevron",
                    Rect::new(x + 20.0 + tw + 4.0, r.y + 3.0, 12.0, 12.0),
                    "chevron-down",
                    400,
                    tertiary,
                );
            }
        }
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value.as_str()) {
            (Prop::Title | Prop::Text, Some(s)) => {
                self.title = s.to_string();
                PropChange::Layout
            }
            (Prop::Subtitle, Some(s)) => {
                self.subtitle = s.to_string();
                PropChange::Layout
            }
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title | Prop::Text => Some(Value::Str(self.title.clone())),
            Prop::Subtitle => Some(Value::Str(self.subtitle.clone())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Label;
        a.label = Some(self.title.clone());
    }
}

// ---------------------------------------------------------------------------------------
// Glass pieces: search, view switcher, send.
// ---------------------------------------------------------------------------------------

/// The small shadow under a raised piece (a selected segment, the send button, a tab).
fn key_shadow(dark: bool) -> Shadow {
    Shadow {
        color: Color::from_u32(if dark { 0x00000033 } else { 0x0000001a }),
        x: 0.0,
        y: 1.0,
        blur: 2.0,
        spread: 0.0,
    }
}

fn glass_track(cx: &mut PaintCx<'_>, name: &str, r: Rect) {
    let theme = *cx.theme();
    cx.rec.layer(
        name,
        r,
        Shape::rounded(metrics::GLASS_RADIUS),
        theme.material_style("Glass/Regular"),
    );
}

/// The search piece: a glass pill with a magnifier and the placeholder (or a token child
/// named `Token`).
#[derive(Debug)]
pub struct SearchPiece {
    /// The placeholder.
    pub value: String,
    /// Width.
    pub width: f32,
    text: Label,
}

impl SearchPiece {
    /// A search piece.
    pub fn new(value: &str) -> SearchPiece {
        SearchPiece {
            value: value.to_string(),
            width: 200.0,
            text: Label::default(),
        }
    }

    /// With a width.
    pub fn width(mut self, width: f32) -> SearchPiece {
        self.width = width;
        self
    }
}

impl Widget for SearchPiece {
    fn kind(&self) -> &'static str {
        "SearchPiece"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        for c in cx.children() {
            let _ = cx.measure(c, Proposal::new(None, None));
        }
        Size::new(self.width, metrics::GLASS_HEIGHT)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, _size: Size) {
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(None, None));
            cx.place(c, Rect::new(36.0, 8.0, s.width, s.height));
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        glass_track(cx, "Background", r);
        cx.rec.glyph(
            "Search glyph",
            Rect::new(r.x + 12.0, r.y + 8.0, 20.0, 20.0),
            "search",
            400,
            c.text_secondary,
        );
        if cx.ui.tree.children(cx.id).is_empty() {
            let style = theme.text(&typography::BODY, c.text_tertiary);
            let w = (r.width - 52.0).max(0.0);
            let layout = self
                .text
                .layout(&self.value, &style, Align::Start, w, Some(1));
            cx.rec.text(
                "Value",
                Rect::new(r.x + 40.0, r.y + 9.0, w, 18.0),
                layout,
                Some(typography::BODY.name),
                c.text_tertiary,
            );
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::SearchField;
        a.label = Some(self.value.clone());
    }
}

/// What a segment of the view switcher shows.
#[derive(Debug, Clone)]
pub enum SwitchItem {
    /// An icon.
    Icon(String),
    /// A label.
    Text(String),
}

/// The view switcher piece: a glass pill with a selected segment.
#[derive(Debug)]
pub struct SwitcherPiece {
    /// The segments.
    pub items: Vec<SwitchItem>,
    /// The selected segment.
    pub selected: usize,
    /// Whether the window is active (`None`: ask the window).
    pub active: Option<bool>,
    texts: Vec<Label>,
}

impl SwitcherPiece {
    /// A switcher.
    pub fn new(items: Vec<SwitchItem>) -> SwitcherPiece {
        let n = items.len();
        SwitcherPiece {
            items,
            selected: 0,
            active: None,
            texts: (0..n).map(|_| Label::default()).collect(),
        }
    }

    /// The selected segment.
    pub fn selected(mut self, i: usize) -> SwitcherPiece {
        self.selected = i;
        self
    }

    fn segment_width(&self) -> f32 {
        match self.items.first() {
            Some(SwitchItem::Text(_)) => 88.0,
            _ => 36.0,
        }
    }

    fn segment_at(&self, p: Point) -> Option<usize> {
        let w = self.segment_width();
        let i = ((p.x - 4.0) / w).floor();
        (i >= 0.0 && (i as usize) < self.items.len()).then_some(i as usize)
    }
}

impl Widget for SwitcherPiece {
    fn kind(&self) -> &'static str {
        "SwitcherPiece"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(
            8.0 + self.segment_width() * self.items.len() as f32,
            metrics::GLASS_HEIGHT,
        )
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let active = self.active.unwrap_or_else(|| cx.window_active());
        glass_track(cx, "Track", r);
        let w = self.segment_width();
        for (i, item) in self.items.iter().enumerate() {
            let seg = Rect::new(r.x + 4.0 + i as f32 * w, r.y + 4.0, w, 28.0);
            let on = i == self.selected;
            if on {
                cx.rec.layer(
                    &format!("Segment {} Background", i + 1),
                    seg,
                    Shape::rounded(14.0),
                    Style {
                        shadows: vec![key_shadow(theme.dark())],
                        ..Style::fill(c.surface_control)
                            .with_border(Border::inside(c.line_border, 1.0))
                    },
                );
            }
            let color = if active {
                c.text_primary
            } else {
                c.text_tertiary
            };
            match item {
                SwitchItem::Icon(symbol) => {
                    cx.rec.glyph(
                        "Glyph Sidebar",
                        Rect::new(seg.x + 8.0, seg.y + 4.0, 20.0, 20.0),
                        symbol,
                        400,
                        color,
                    );
                }
                SwitchItem::Text(label) => {
                    let color = c.text_primary;
                    let style = theme.text(&typography::BODY, color);
                    if self.texts.len() <= i {
                        self.texts.resize_with(i + 1, Label::default);
                    }
                    let Some(cache) = self.texts.get_mut(i) else {
                        continue;
                    };
                    let layout = cache.layout(label, &style, Align::Center, w, Some(1));
                    cx.rec.text(
                        "Label",
                        Rect::new(seg.x, seg.y + 5.0, w, 18.0),
                        layout,
                        Some(typography::BODY.name),
                        color,
                    );
                }
            }
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if let Event::PointerDown { pos, .. } = event
            && let Some(i) = self.segment_at(*pos)
        {
            if i != self.selected {
                self.selected = i;
                cx.emit(Signal::SelectionChanged, Value::Int(i as i64));
                cx.request_paint();
            }
            return Handled::Yes;
        }
        Handled::No
    }

    fn interaction(&self) -> Interaction {
        Interaction::Button
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value.as_int()) {
            (Prop::SelectedIndex, Some(i)) if i >= 0 => {
                self.selected = i as usize;
                PropChange::Paint
            }
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        (prop == Prop::SelectedIndex).then_some(Value::Int(self.selected as i64))
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
    }
}

/// The send piece: an accent circle with an arrow.
#[derive(Debug, Default)]
pub struct SendPiece;

impl Widget for SendPiece {
    fn kind(&self) -> &'static str {
        "SendPiece"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::GLASS_HEIGHT, metrics::GLASS_HEIGHT)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let c = cx.theme().colors;
        let r = cx.bounds();
        cx.rec.layer(
            "Background",
            r,
            Shape::rounded(metrics::GLASS_RADIUS),
            Style {
                shadows: vec![key_shadow(cx.theme().dark())],
                ..Style::fill(c.accent_amalfi).with_border(Border::inside(c.line_border, 1.0))
            },
        );
        cx.rec.glyph(
            "Glyph Plus",
            Rect::new(r.x + 8.0, r.y + 8.0, 20.0, 20.0),
            "arrow-up",
            400,
            c.text_on_accent,
        );
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if *event == Event::Activate {
            cx.emit(Signal::Activated, Value::None);
            Handled::Yes
        } else {
            Handled::No
        }
    }

    fn interaction(&self) -> Interaction {
        Interaction::Button
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Button;
        a.label = Some("Send".to_string());
        a.actions = vec![Action::Click, Action::Focus];
    }
}

// ---------------------------------------------------------------------------------------
// The strip and the bars.
// ---------------------------------------------------------------------------------------

/// A flexible gap (or the title's fill): it takes what the other children leave.
#[derive(Debug)]
pub struct Spacer {
    /// The least width.
    pub min: f32,
    /// Height of the (invisible) space.
    pub height: f32,
    /// Takes the free width (otherwise it is exactly `min`).
    pub flexible: bool,
}

impl Spacer {
    /// A flexible space of at least `min`.
    pub fn flexible(min: f32) -> Spacer {
        Spacer {
            min,
            height: 8.0,
            flexible: true,
        }
    }

    /// A space of exactly `width`.
    pub fn fixed(width: f32) -> Spacer {
        Spacer {
            min: width,
            height: 8.0,
            flexible: false,
        }
    }
}

impl Widget for Spacer {
    fn kind(&self) -> &'static str {
        "Spacer"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(self.min, self.height)
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

/// What a strip paints behind its children (layer `Surface`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backdrop {
    /// Nothing (pieces).
    #[default]
    None,
    /// `Surface/Window`.
    Window,
    /// `Surface/Sidebar`.
    Sidebar,
    /// The translucent sidebar title bar (`Blur/Sidebar Title Bar`).
    SidebarBlur,
}

/// Children side by side, centred vertically; spacers (and a filling title) share the free
/// width.
#[derive(Debug)]
pub struct Strip {
    /// Padding before the first child.
    pub pad_left: f32,
    /// Padding after the last child.
    pub pad_right: f32,
    /// Gap between children.
    pub gap: f32,
    /// The strip's height.
    pub height: f32,
    /// What it paints behind.
    pub backdrop: Backdrop,
    /// Whether the window edge line is drawn at the bottom (title bars).
    pub edge: bool,
}

impl Strip {
    /// A strip of `height` with padding at both sides.
    pub fn new(height: f32, pad_left: f32, pad_right: f32) -> Strip {
        Strip {
            pad_left,
            pad_right,
            gap: 0.0,
            height,
            backdrop: Backdrop::None,
            edge: false,
        }
    }

    /// With a gap between children.
    pub fn gap(mut self, gap: f32) -> Strip {
        self.gap = gap;
        self
    }

    /// With a backdrop.
    pub fn backdrop(mut self, backdrop: Backdrop) -> Strip {
        self.backdrop = backdrop;
        self
    }

    fn flexes(ui: &Ui, id: ViewId) -> bool {
        ui.widget::<Spacer>(id).is_some_and(|s| s.flexible)
            || ui.widget::<TitleBlock>(id).is_some_and(|t| t.fill)
    }

    fn naturals(&self, cx: &mut LayoutCx<'_>) -> Vec<(ViewId, Size, bool)> {
        let h = self.height;
        cx.children()
            .into_iter()
            .map(|c| {
                let flex = Strip::flexes(cx.ui, c);
                let s = cx.measure(c, Proposal::new(None, Some(h)));
                (c, s, flex)
            })
            .collect()
    }
}

impl Widget for Strip {
    fn kind(&self) -> &'static str {
        "Strip"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let kids = self.naturals(cx);
        let n = kids.len();
        let natural = self.pad_left
            + self.pad_right
            + kids.iter().map(|k| k.1.width).sum::<f32>()
            + self.gap * n.saturating_sub(1) as f32;
        let w = match p.width {
            Some(w) if w.is_finite() => w,
            _ => natural,
        };
        Size::new(w, self.height)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let kids = self.naturals(cx);
        let n = kids.len();
        let fixed: f32 = kids.iter().filter(|k| !k.2).map(|k| k.1.width).sum();
        let flex = kids.iter().filter(|k| k.2).count();
        let free = size.width
            - self.pad_left
            - self.pad_right
            - self.gap * n.saturating_sub(1) as f32
            - fixed;
        let share = if flex > 0 { free / flex as f32 } else { 0.0 };
        let mut x = self.pad_left;
        for (c, s, f) in kids {
            let w = if f { share.max(s.width) } else { s.width };
            let y = (size.height - s.height) / 2.0;
            let fit = cx.measure(c, Proposal::new(Some(w), Some(s.height)));
            cx.place(c, Rect::new(x, y, w, fit.height));
            x += w + self.gap;
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        match self.backdrop {
            Backdrop::None => {}
            Backdrop::Window => {
                cx.rec
                    .layer("Surface", r, Shape::RECT, Style::fill(c.surface_window));
            }
            Backdrop::Sidebar => {
                cx.rec
                    .layer("Surface", r, Shape::RECT, Style::fill(c.surface_sidebar));
            }
            Backdrop::SidebarBlur => {
                cx.rec.layer(
                    "Surface",
                    r,
                    Shape::RECT,
                    theme.material_style("Blur/Sidebar Title Bar"),
                );
            }
        }
        if self.edge {
            cx.rec.layer(
                "Edge",
                Rect::new(r.x, r.y + r.height - 1.0, r.width, 1.0),
                Shape::RECT,
                Style::fill(c.line_separator),
            );
        }
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Toolbar;
    }
}

/// The document title bar: the window buttons at the left, the title block centred between
/// them and a balancing space at the right, an edge line below.
#[derive(Debug, Default)]
pub struct TitleBar;

impl TitleBar {
    const PAD: f32 = 16.0;
    const GAP: f32 = 12.0;
}

impl Widget for TitleBar {
    fn kind(&self) -> &'static str {
        "TitleBar"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        for c in cx.children() {
            let _ = cx.measure(c, Proposal::new(None, None));
        }
        let w = match p.width {
            Some(w) if w.is_finite() => w,
            _ => 520.0,
        };
        Size::new(w, metrics::TITLE_BAR_HEIGHT)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let kids = cx.children();
        let controls = metrics::CONTROLS_WIDTH;
        if let Some(c) = kids.first().copied() {
            cx.place(
                c,
                Rect::new(
                    TitleBar::PAD,
                    (size.height - metrics::DOT) / 2.0,
                    controls,
                    metrics::DOT,
                ),
            );
        }
        if let Some(b) = kids.get(1).copied() {
            let left = TitleBar::PAD + controls + TitleBar::GAP;
            let right = size.width - TitleBar::PAD - controls - TitleBar::GAP;
            let avail = (right - left).max(0.0);
            let s = cx.measure(b, Proposal::new(None, None));
            let w = s.width.min(avail);
            cx.place(
                b,
                Rect::new(
                    left + (avail - w) / 2.0,
                    (size.height - s.height) / 2.0,
                    w,
                    s.height,
                ),
            );
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let c = cx.theme().colors;
        let r = cx.bounds();
        cx.rec.layer(
            "Edge",
            Rect::new(r.x, r.y + r.height - 1.0, r.width, 1.0),
            Shape::RECT,
            Style::fill(c.line_separator),
        );
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Toolbar;
    }
}

// ---------------------------------------------------------------------------------------
// Tabs.
// ---------------------------------------------------------------------------------------

/// A tab: the title, a close button while selected or hovered, an optional icon.
#[derive(Debug)]
pub struct Tab {
    /// The title.
    pub title: String,
    /// The icon (a symbol name).
    pub icon: Option<String>,
    /// Selected.
    pub selected: bool,
    /// Width (`None`: the tab's own width).
    pub width: Option<f32>,
    transitions: Transitions,
    text: Label,
    /// The press began on the close button.
    close_pressed: bool,
}

impl Tab {
    /// A tab.
    pub fn new(title: &str) -> Tab {
        Tab {
            title: title.to_string(),
            icon: None,
            selected: false,
            width: None,
            transitions: Transitions::default(),
            text: Label::default(),
            close_pressed: false,
        }
    }

    /// Selected.
    pub fn selected(mut self, selected: bool) -> Tab {
        self.selected = selected;
        self
    }

    fn close_rect(w: f32) -> Rect {
        Rect::new(w - 20.0, 8.0, 12.0, 12.0)
    }
}

impl Widget for Tab {
    fn kind(&self) -> &'static str {
        "Tab"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let w = match (self.width, p.width) {
            (_, Some(w)) if w.is_finite() => w,
            (Some(w), _) => w,
            _ => metrics::TAB_WIDTH,
        };
        Size::new(w, metrics::TAB_HEIGHT)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let hover = self.transitions.hover.get();
        let shape = Shape::rounded(9.0);
        if self.selected {
            cx.rec.layer(
                "Surface",
                r,
                shape,
                Style {
                    shadows: vec![key_shadow(theme.dark())],
                    ..Style::fill(c.surface_raised).with_border(Border::inside(c.line_border, 1.0))
                },
            );
        } else if hover > 0.0 {
            cx.rec.layer(
                "Surface",
                r,
                shape,
                Style::fill(c.accent_amalfi_soft).with_opacity(hover),
            );
        }
        let (label, icon) = if self.selected {
            (c.text_primary, c.text_secondary)
        } else {
            (c.text_secondary, c.text_secondary)
        };
        if self.selected || hover > 0.0 {
            let cr = Tab::close_rect(r.width);
            cx.rec.glyph(
                "Close",
                Rect::new(r.x + cr.x, r.y + cr.y, cr.width, cr.height),
                "xmark",
                400,
                icon,
            );
        }
        if let Some(symbol) = &self.icon {
            cx.rec.glyph(
                "Icon",
                Rect::new(r.x + 8.0, r.y + 8.0, 12.0, 12.0),
                symbol,
                400,
                icon,
            );
        }
        let style = theme.text(&typography::BODY, label);
        let w = (r.width - 48.0).max(0.0);
        let layout = self
            .text
            .layout(&self.title, &style, Align::Start, w, Some(1));
        cx.rec.text(
            "Label",
            Rect::new(r.x + 24.0, r.y + 5.0, w, 18.0),
            layout,
            Some(typography::BODY.name),
            label,
        );
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::PointerDown { pos, .. } => {
                let w = cx.bounds().width;
                let cr = Tab::close_rect(w);
                self.close_pressed =
                    (self.selected || self.transitions.hover.get() > 0.0) && cr.contains(*pos);
                Handled::No
            }
            Event::Activate => {
                if std::mem::take(&mut self.close_pressed) {
                    cx.emit(Signal::CloseRequested, Value::None);
                } else {
                    cx.emit(Signal::Activated, Value::None);
                }
                Handled::Yes
            }
            Event::Key(k) if k.pressed && matches!(k.key, Key::Enter | Key::Space) => {
                cx.emit(Signal::Activated, Value::None);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn state_changed(&mut self, cx: &mut EventCx<'_>, _old: NodeFlags, new: NodeFlags) {
        let reduce = cx.theme().appearance.reduce_motion;
        if self
            .transitions
            .update(new, new.contains(NodeFlags::DISABLED), reduce)
        {
            cx.animate();
        }
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        self.transitions.tick(dt)
    }

    fn interaction(&self) -> Interaction {
        Interaction::Button
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Title | Prop::Text => value.as_str().map_or(PropChange::Unknown, |s| {
                self.title = s.to_string();
                PropChange::Paint
            }),
            Prop::State => value.as_bool().map_or(PropChange::Unknown, |b| {
                self.selected = b;
                PropChange::Paint
            }),
            Prop::Symbol => value.as_str().map_or(PropChange::Unknown, |s| {
                self.icon = Some(s.to_string()).filter(|s| !s.is_empty());
                PropChange::Paint
            }),
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title | Prop::Text => Some(Value::Str(self.title.clone())),
            Prop::State => Some(Value::Bool(self.selected)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Tab;
        a.label = Some(self.title.clone());
        a.selected = Some(self.selected);
        a.actions = vec![Action::Click, Action::Focus];
    }
}

/// The tab bar: a sunken track with the tabs sharing its width and the new-tab button (the
/// last child) at the end.
#[derive(Debug, Default)]
pub struct TabBar;

impl TabBar {
    const PAD: f32 = 4.0;
    const GAP: f32 = 4.0;
}

impl Widget for TabBar {
    fn kind(&self) -> &'static str {
        "TabBar"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        for c in cx.children() {
            let _ = cx.measure(c, Proposal::new(None, None));
        }
        let n = cx.children().len().saturating_sub(1);
        let w = match p.width {
            Some(w) if w.is_finite() => w,
            _ => 2.0 * TabBar::PAD + n as f32 * (metrics::TAB_WIDTH + TabBar::GAP) + 24.0,
        };
        Size::new(w, metrics::TAB_BAR_HEIGHT)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let kids = cx.children();
        let Some((button, tabs)) = kids.split_last() else {
            return;
        };
        let bs = cx.measure(*button, Proposal::new(None, None));
        let inner = size.width - 2.0 * TabBar::PAD - bs.width - TabBar::GAP;
        let n = tabs.len().max(1) as f32;
        let tab_w = ((inner - TabBar::GAP * (n - 1.0)) / n).max(0.0);
        let mut x = TabBar::PAD;
        for t in tabs {
            let _ = cx.measure(*t, Proposal::new(Some(tab_w), None));
            cx.place(*t, Rect::new(x, TabBar::PAD, tab_w, metrics::TAB_HEIGHT));
            x += tab_w + TabBar::GAP;
        }
        cx.place(
            *button,
            Rect::new(
                size.width - TabBar::PAD - bs.width,
                (size.height - bs.height) / 2.0,
                bs.width,
                bs.height,
            ),
        );
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let c = cx.theme().colors;
        cx.rec.layer(
            "Track",
            cx.bounds(),
            Shape::rounded(12.0),
            Style::fill(c.surface_sunken),
        );
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::List;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Button, Mods};
    use crate::core::window::WindowKind;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn mount(ui: &mut Ui, v: ViewId) -> crate::core::window::WindowId {
        let win = ui.create_window(WindowKind::Normal, Size::new(600.0, 200.0));
        ui.set_content(win, v);
        ui.show_window(win);
        ui.prepare_frame(win);
        win
    }

    fn click(ui: &mut Ui, win: crate::core::window::WindowId, p: Point) {
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
    }

    #[test]
    fn a_tab_closes_from_its_x_and_activates_elsewhere() {
        let mut ui = Ui::new_headless();
        let t = ui.create(Tab::new("Lemon cake").selected(true));
        let log = Rc::new(RefCell::new(Vec::new()));
        let l = log.clone();
        ui.connect(t, Signal::CloseRequested, move |_, _, _| {
            l.borrow_mut().push("close");
        });
        let l = log.clone();
        ui.connect(t, Signal::Activated, move |_, _, _| {
            l.borrow_mut().push("activate");
        });
        let win = mount(&mut ui, t);
        let r = ui.tree.window_rect(t);
        click(&mut ui, win, Point::new(r.x + 100.0, r.y + 14.0));
        click(&mut ui, win, Point::new(r.x + r.width - 14.0, r.y + 14.0));
        assert_eq!(*log.borrow(), vec!["activate", "close"]);
    }

    #[test]
    fn the_switcher_selects_the_segment_that_is_clicked() {
        let mut ui = Ui::new_headless();
        let s = ui.create(SwitcherPiece::new(vec![
            SwitchItem::Icon("list-bullet".into()),
            SwitchItem::Icon("square-grid-2x2".into()),
        ]));
        let win = mount(&mut ui, s);
        let r = ui.tree.window_rect(s);
        click(
            &mut ui,
            win,
            Point::new(r.x + 4.0 + 36.0 + 18.0, r.y + 18.0),
        );
        assert_eq!(ui.get(s, Prop::SelectedIndex), Some(Value::Int(1)));
    }

    #[test]
    fn the_strip_gives_the_free_width_to_its_spacer() {
        let mut ui = Ui::new_headless();
        let s = ui.create(Strip::new(52.0, 12.0, 8.0));
        let a = ui.create(Spacer::fixed(40.0));
        let sp = ui.create(Spacer::flexible(8.0));
        let b = ui.create(Spacer::fixed(60.0));
        for c in [a, sp, b] {
            ui.add_child(s, c);
        }
        let _ = ui.set(s, Prop::Width, Value::from(300.0));
        mount(&mut ui, s);
        let origin = ui.tree.window_rect(s).x;
        let r = ui.tree.window_rect(sp);
        assert_eq!(r.x - origin, 12.0 + 40.0);
        assert_eq!(r.width, 300.0 - 12.0 - 8.0 - 100.0);
    }
}
