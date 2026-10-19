//! Wells (kit `Wells`): the colour well and its panel of swatches, the image well, and the
//! swatch chip and row.

use crate::amalfi::foundation::{Label, focus_halo_named};
use crate::amalfi::tokens::{self, typography};
use crate::color::Color;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Event, Handled, Key};
use crate::core::layout::{Proposal, Stack};
use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::core::window::{WindowId, WindowKind};
use crate::geom::{Insets, Point, Rect, Size};
use crate::motion::{Animated, Timing};
use crate::shape::Shape;
use crate::style::{Border, BorderPosition, Style};
use crate::text::Align;

/// Fixed measures.
pub mod metrics {
    /// A swatch chip's side.
    pub const CHIP: f32 = 24.0;
    /// The selection ring's width.
    pub const RING: f32 = 2.0;
    /// Space between chips.
    pub const GAP: f32 = 8.0;
    /// Space around a row of chips.
    pub const ROW_PADDING: f32 = 4.0;
    /// The colour panel's corner radius.
    pub const PANEL_RADIUS: f32 = 16.0;
    /// The colour panel's padding: left, top, right, bottom.
    pub const PANEL_PADDING: (f32, f32, f32, f32) = (12.0, 14.0, 12.0, 16.0);
    /// The colour panel's title height.
    pub const PANEL_TITLE: f32 = 18.0;
    /// A swatch row's height.
    pub const ROW_HEIGHT: f32 = 32.0;
    /// Space between a colour well and its panel.
    pub const PANEL_GAP: f32 = 4.0;
}

// ---------------------------------------------------------------------------------------
// Swatches.
// ---------------------------------------------------------------------------------------

/// A colour swatch: a 24 pt disc with a 1 pt border, a 2 pt accent ring outside when selected.
#[derive(Debug)]
pub struct SwatchChip {
    /// The colour.
    pub color: Color,
    /// Selected.
    pub selected: bool,
}

impl SwatchChip {
    /// A chip.
    pub fn new(color: Color) -> SwatchChip {
        SwatchChip {
            color,
            selected: false,
        }
    }

    /// Selected.
    pub fn selected(mut self, selected: bool) -> SwatchChip {
        self.selected = selected;
        self
    }
}

impl Widget for SwatchChip {
    fn kind(&self) -> &'static str {
        "SwatchChip"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::CHIP, metrics::CHIP)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let shape = Shape::rounded(metrics::CHIP / 2.0);
        if self.selected {
            cx.rec.layer(
                "Selection Ring",
                r,
                shape.clone(),
                Style::default().with_border(Border::at(
                    c.accent_amalfi,
                    metrics::RING,
                    BorderPosition::Outside,
                )),
            );
        }
        cx.rec.layer(
            "Colour",
            r,
            shape.clone(),
            Style::fill(self.color).with_border(Border::inside(c.line_border, 1.0)),
        );
        if cx.focus_visible() && !cx.is_disabled() {
            focus_halo_named(cx.rec, "Focus Halo", r, shape, &theme);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Activate => {
                cx.emit(Signal::Activated, Value::Color(self.color));
                Handled::Yes
            }
            Event::PointerEnter => {
                cx.set_cursor(crate::core::ui::Cursor::Pointer);
                Handled::No
            }
            _ => Handled::No,
        }
    }

    fn interaction(&self) -> Interaction {
        Interaction::Button
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::ColorValue => value.as_color().map_or(PropChange::Unknown, |c| {
                self.color = c;
                PropChange::Paint
            }),
            Prop::State => value.as_bool().map_or(PropChange::Unknown, |b| {
                self.selected = b;
                PropChange::Paint
            }),
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::ColorValue => Some(Value::Color(self.color)),
            Prop::State => Some(Value::Bool(self.selected)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Button;
        a.selected = Some(self.selected);
        a.actions = vec![Action::Click, Action::Focus];
    }
}

/// A row of swatches, 8 apart with 4 around: each item is an identifier (the colour's token
/// name) and a colour. Returns the row and its chips.
pub fn swatch_row(
    ui: &mut Ui,
    items: &[(String, Color)],
    selected: Option<usize>,
) -> (ViewId, Vec<ViewId>) {
    let row = ui.create(Stack::row(metrics::GAP));
    let _ = ui.set(
        row,
        Prop::Padding,
        Value::Insets(Insets::all(metrics::ROW_PADDING)),
    );
    let mut chips = Vec::new();
    for (i, (name, color)) in items.iter().enumerate() {
        let chip = ui.create(SwatchChip::new(*color).selected(selected == Some(i)));
        let _ = ui.set(chip, Prop::Identifier, Value::from(name.as_str()));
        ui.add_child(row, chip);
        chips.push(chip);
    }
    (row, chips)
}

/// The colour panel: `Glass/Thick`, radius 16, a Headline title and rows of swatches (its
/// children, 32 tall each).
#[derive(Debug)]
pub struct ColorPanel {
    /// The title.
    pub title: String,
    label: Label,
}

impl ColorPanel {
    /// A panel titled `title`.
    pub fn new(title: &str) -> ColorPanel {
        ColorPanel {
            title: title.to_string(),
            label: Label::default(),
        }
    }
}

impl Widget for ColorPanel {
    fn kind(&self) -> &'static str {
        "ColorPanel"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let (l, t, r, b) = metrics::PANEL_PADDING;
        let mut w = 0.0f32;
        let mut h = t + metrics::PANEL_TITLE + b;
        for c in cx.children() {
            let s = cx.measure(c, Proposal::UNSPECIFIED);
            w = w.max(s.width);
            h += s.height;
        }
        Size::new(w + l + r, h)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, _size: Size) {
        let (l, t, _, _) = metrics::PANEL_PADDING;
        let mut y = t + metrics::PANEL_TITLE;
        for c in cx.children() {
            let s = cx.measure(c, Proposal::UNSPECIFIED);
            cx.place(c, Rect::new(l, y, s.width, s.height));
            y += s.height;
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let b = cx.bounds();
        cx.rec.layer(
            "Surface",
            b,
            Shape::rounded(metrics::PANEL_RADIUS),
            theme.material_style("Glass/Thick"),
        );
        let (l, t, _, _) = metrics::PANEL_PADDING;
        let style = theme.text(&typography::HEADLINE, theme.colors.text_primary);
        let w = crate::amalfi::foundation::whole(self.label.width(&self.title, &style), 4.0);
        let layout = self
            .label
            .layout(&self.title, &style, Align::Start, w, Some(1));
        cx.rec.text(
            "Title",
            Rect::new(b.x + l, b.y + t, w, metrics::PANEL_TITLE),
            layout,
            Some(typography::HEADLINE.name),
            style.color,
        );
    }

    fn hit(&self, p: Point, size: Size) -> bool {
        Rect::from_size(size).contains(p)
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
        a.label = Some(self.title.clone());
    }
}

/// A colour panel over `rows` of (token name, colour); returns it and every chip.
pub fn color_panel(
    ui: &mut Ui,
    title: &str,
    rows: &[Vec<(String, Color)>],
    selected: Option<Color>,
) -> (ViewId, Vec<ViewId>) {
    let panel = ui.create(ColorPanel::new(title));
    let mut marked = false;
    let mut chips = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let pick = selected
            .filter(|_| !marked)
            .and_then(|s| row.iter().position(|(_, c)| *c == s));
        marked |= pick.is_some();
        let (r, mut c) = swatch_row(ui, row, pick);
        let _ = ui.set(r, Prop::Identifier, Value::from(format!("Row {}", i + 1)));
        ui.add_child(panel, r);
        chips.append(&mut c);
    }
    (panel, chips)
}

// ---------------------------------------------------------------------------------------
// The colour well.
// ---------------------------------------------------------------------------------------

/// The three looks of a colour well.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WellStyle {
    /// 44 x 24, just the colour.
    Minimal,
    /// 72 x 28, with the picker chevron.
    #[default]
    Standard,
    /// 188 x 28, with the name and the eyedropper.
    Expanded,
}

impl WellStyle {
    fn size(self) -> Size {
        match self {
            WellStyle::Minimal => Size::new(44.0, 24.0),
            WellStyle::Standard => Size::new(72.0, 28.0),
            WellStyle::Expanded => Size::new(188.0, 28.0),
        }
    }

    fn radius(self) -> f32 {
        if self == WellStyle::Minimal { 8.0 } else { 9.0 }
    }

    fn colour(self) -> (f32, f32) {
        match self {
            WellStyle::Minimal => (36.0, 5.0),
            WellStyle::Standard => (44.0, 6.0),
            WellStyle::Expanded => (28.0, 6.0),
        }
    }

    fn padding(self) -> (f32, f32, f32, f32) {
        match self {
            WellStyle::Expanded => (4.0, 4.0, 8.0, 4.0),
            _ => (4.0, 4.0, 4.0, 4.0),
        }
    }
}

/// A colour well: the colour in a raised frame; pressed, it opens the panel of swatches under
/// it (a popup) and the chosen swatch is `Signal::ValueChanged` (`Color`).
#[derive(Debug)]
pub struct ColorWell {
    /// The colour.
    pub color: Color,
    /// The look.
    pub style: WellStyle,
    /// The colour's name (expanded).
    pub name: String,
    /// The eyedropper (expanded).
    pub eyedropper: bool,
    /// The panel's rows (`None`: the Amalfi palette).
    pub swatches: Option<Vec<Vec<(String, Color)>>>,
    hover: Animated<f32>,
    label: Label,
    panel: Option<WindowId>,
}

impl ColorWell {
    /// A standard well of `color`.
    pub fn new(color: Color) -> ColorWell {
        ColorWell {
            color,
            style: WellStyle::Standard,
            name: String::new(),
            eyedropper: false,
            swatches: None,
            hover: Animated::new(0.0),
            label: Label::default(),
            panel: None,
        }
    }

    /// With a look.
    pub fn style(mut self, style: WellStyle) -> ColorWell {
        self.style = style;
        self
    }

    /// With a name and an eyedropper (the expanded look).
    pub fn named(mut self, name: &str) -> ColorWell {
        self.name = name.to_string();
        self.style = WellStyle::Expanded;
        self.eyedropper = true;
        self
    }

    fn close_panel(&mut self, ui: &mut Ui) {
        if let Some(w) = self.panel.take() {
            ui.close_window(w);
        }
    }
}

impl Widget for ColorWell {
    fn kind(&self) -> &'static str {
        "ColorWell"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        self.style.size()
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let b = cx.bounds();
        let size = self.style.size();
        let rect = Rect::new(b.x, b.y, size.width, size.height);
        let shape = Shape::rounded(self.style.radius());
        let enabled = !cx.is_disabled();
        cx.rec.layer(
            "Background",
            rect,
            shape.clone(),
            Style::fill(c.surface_raised).with_border(Border::inside(c.line_border, 1.0)),
        );
        let hover = self.hover.get();
        if hover > 0.0 {
            cx.rec.layer(
                "State",
                rect,
                shape.clone(),
                Style::fill(c.accent_amalfi_soft).with_opacity(hover),
            );
        }
        if cx.focus_visible() && enabled {
            focus_halo_named(cx.rec, "Focus Halo", rect, shape, &theme);
        }
        let (l, t, r, bt) = self.style.padding();
        let (cw, cr) = self.style.colour();
        let ch = size.height - t - bt;
        cx.rec.layer(
            "Colour",
            Rect::new(b.x + l, b.y + t, cw, ch),
            Shape::rounded(cr),
            Style::fill(self.color).with_border(Border::inside(c.line_border, 1.0)),
        );
        match self.style {
            WellStyle::Standard => cx.rec.glyph(
                "Picker chevron",
                Rect::new(
                    b.x + l + cw + 4.0,
                    b.y + (size.height - 16.0) / 2.0,
                    16.0,
                    16.0,
                ),
                "chevron-down",
                400,
                c.text_secondary,
            ),
            WellStyle::Expanded => {
                let x = l + cw + 8.0;
                let eye = if self.eyedropper { 20.0 + 8.0 } else { 0.0 };
                let w = (size.width - r - x - eye).max(0.0);
                let style = theme.text(&typography::BODY, c.text_primary);
                let lh = style.line_height.unwrap_or(18.0);
                let layout = self
                    .label
                    .layout(&self.name, &style, Align::Start, w, Some(1));
                cx.rec.text(
                    "Name",
                    Rect::new(b.x + x, b.y + (size.height - lh) / 2.0, w, lh),
                    layout,
                    Some(typography::BODY.name),
                    style.color,
                );
                if self.eyedropper {
                    cx.rec.glyph(
                        "Eyedropper",
                        Rect::new(
                            b.x + size.width - r - 20.0,
                            b.y + (size.height - 20.0) / 2.0,
                            20.0,
                            20.0,
                        ),
                        "eyedropper",
                        400,
                        c.text_secondary,
                    );
                }
            }
            WellStyle::Minimal => {}
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Activate => {
                let id = cx.id();
                if self.panel.is_some() {
                    self.close_panel(cx.ui());
                } else {
                    let theme = cx.theme();
                    let rows = self
                        .swatches
                        .clone()
                        .unwrap_or_else(|| amalfi_swatches(&theme));
                    let rect = cx.window_rect();
                    let Some(parent) = cx.window() else {
                        return Handled::Yes;
                    };
                    let selected = self.color;
                    let w = open_panel(cx.ui(), id, parent, rect, "Swatches", &rows, selected);
                    self.panel = w;
                    cx.emit(Signal::ExpandedChanged, Value::Bool(w.is_some()));
                }
                cx.focus_self();
                cx.request_paint();
                Handled::Yes
            }
            Event::FocusOut => {
                let ui = cx.ui();
                self.close_panel(ui);
                Handled::No
            }
            Event::Key(k) if k.pressed && k.key == Key::Escape && self.panel.is_some() => {
                self.close_panel(cx.ui());
                cx.request_paint();
                Handled::Yes
            }
            Event::Paste(_) | Event::Drop { .. } => Handled::No,
            _ => Handled::No,
        }
    }

    fn state_changed(&mut self, cx: &mut EventCx<'_>, _old: NodeFlags, new: NodeFlags) {
        let on = new.contains(NodeFlags::HOVERED) && !new.contains(NodeFlags::DISABLED);
        let reduce = cx.theme().appearance.reduce_motion;
        let t = if reduce {
            Timing::None
        } else {
            Timing::curve(tokens::motion::FADE_QUICK_CURVE, tokens::motion::FADE_QUICK)
        };
        self.hover.animate_to(if on { 1.0 } else { 0.0 }, t);
        if self.hover.is_animating() {
            cx.animate();
        }
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        self.hover.tick(dt)
    }

    fn interaction(&self) -> Interaction {
        Interaction::Button
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::ColorValue | Prop::Value => value.as_color().map_or(PropChange::Unknown, |c| {
                self.color = c;
                PropChange::Paint
            }),
            Prop::Title => value.as_str().map_or(PropChange::Unknown, |s| {
                self.name = s.to_string();
                PropChange::Paint
            }),
            Prop::Style => match value.as_int() {
                Some(0) => {
                    self.style = WellStyle::Minimal;
                    PropChange::Layout
                }
                Some(1) => {
                    self.style = WellStyle::Standard;
                    PropChange::Layout
                }
                Some(2) => {
                    self.style = WellStyle::Expanded;
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::ColorValue | Prop::Value => Some(Value::Color(self.color)),
            Prop::Title => Some(Value::Str(self.name.clone())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::ColorWell;
        a.label = Some(self.name.clone()).filter(|s| !s.is_empty());
        a.value = Some(self.color.to_hex());
        a.expanded = Some(self.panel.is_some());
        a.actions = vec![Action::Click, Action::Focus];
    }
}

/// The Amalfi palette as the default swatches of the panel (four rows of six).
pub fn amalfi_swatches(theme: &crate::amalfi::theme::Theme) -> Vec<Vec<(String, Color)>> {
    let names: [[&str; 6]; 4] = [
        [
            "Accent/Amalfi",
            "Palette/Limone",
            "Palette/Terracotta",
            "Palette/Bouganville",
            "Palette/Ulivo",
            "Palette/Mare",
        ],
        [
            "Palette/Lavanda",
            "Accent/Amalfi Pressed",
            "Text/Primary",
            "Text/Secondary",
            "Text/Tertiary",
            "Surface/Sunken",
        ],
        [
            "Surface/Sidebar",
            "Surface/Raised",
            "Surface/Window",
            "Surface/Control Pressed",
            "Line/Border",
            "Glass/Tint",
        ],
        [
            "Accent/Amalfi Soft",
            "Line/Separator",
            "Glass/Rim Dark",
            "State/Press Overlay",
            "Shadow/Soft",
            "Text/On Accent",
        ],
    ];
    names
        .iter()
        .map(|row| {
            row.iter()
                .map(|n| {
                    (
                        (*n).to_string(),
                        theme.color(n).unwrap_or(theme.colors.text_tertiary),
                    )
                })
                .collect()
        })
        .collect()
}

/// Shows the panel under `anchor` (window coordinates) as a popup of `parent`; a chip sets
/// the well's colour and closes it.
fn open_panel(
    ui: &mut Ui,
    well: ViewId,
    parent: WindowId,
    anchor: Rect,
    title: &str,
    rows: &[Vec<(String, Color)>],
    selected: Color,
) -> Option<WindowId> {
    let (panel, chips) = color_panel(ui, title, rows, Some(selected));
    let size = ui.measure(panel, Proposal::UNSPECIFIED);
    let at = Point::new(anchor.x, anchor.bottom() + metrics::PANEL_GAP);
    let window = ui.create_window(
        WindowKind::Popup {
            parent,
            anchor: Rect::from_origin_size(at, size),
            grab: true,
        },
        size,
    );
    ui.set_content(window, panel);
    ui.release(panel);
    for chip in chips {
        ui.connect(chip, Signal::Activated, move |ui, _, v| {
            let Some(color) = v.as_color() else { return };
            let mut closing = None;
            ui.with_widget::<ColorWell, _>(well, |w| {
                w.color = color;
                closing = w.panel.take();
            });
            if let Some(w) = closing {
                ui.close_window(w);
            }
            ui.emit(well, Signal::ValueChanged, Value::Color(color));
        });
    }
    ui.show_window(window);
    Some(window)
}

// ---------------------------------------------------------------------------------------
// The image well.
// ---------------------------------------------------------------------------------------

/// The three sizes of an image well.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageWellSize {
    /// 72 pt, no hint.
    Small,
    /// 120 pt.
    #[default]
    Regular,
    /// 168 pt.
    Large,
}

impl ImageWellSize {
    fn side(self) -> f32 {
        match self {
            ImageWellSize::Small => 72.0,
            ImageWellSize::Regular => 120.0,
            ImageWellSize::Large => 168.0,
        }
    }

    fn radius(self) -> f32 {
        match self {
            ImageWellSize::Small => 12.0,
            ImageWellSize::Regular => 16.0,
            ImageWellSize::Large => 20.0,
        }
    }

    fn glyph(self) -> f32 {
        match self {
            ImageWellSize::Small => 20.0,
            ImageWellSize::Regular => 28.0,
            ImageWellSize::Large => 36.0,
        }
    }

    fn shows_hint(self) -> bool {
        self != ImageWellSize::Small
    }
}

/// An image well: a square that takes a dropped image (`Signal::Dropped`, the URIs). Empty,
/// it is a dashed drop zone with the photo glyph and a hint; the image it holds is its child
/// view (it fills the well, under a rim). The platform tells it an image is being dragged
/// over it with `Prop::State` 1.
#[derive(Debug)]
pub struct ImageWell {
    /// Size.
    pub size: ImageWellSize,
    /// The hint of an empty well.
    pub hint: String,
    /// The hint while an image is dragged over it.
    pub drop_hint: String,
    /// An image is being dragged over it.
    pub drag_over: bool,
    label: Label,
}

impl ImageWell {
    /// An empty regular well.
    pub fn new() -> ImageWell {
        ImageWell {
            size: ImageWellSize::Regular,
            hint: "Drop image".to_string(),
            drop_hint: "Release to add".to_string(),
            drag_over: false,
            label: Label::default(),
        }
    }

    /// With a size.
    pub fn size(mut self, size: ImageWellSize) -> ImageWell {
        self.size = size;
        self
    }
}

impl Default for ImageWell {
    fn default() -> Self {
        ImageWell::new()
    }
}

impl Widget for ImageWell {
    fn kind(&self) -> &'static str {
        "ImageWell"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let s = self.size.side();
        for c in cx.children() {
            let _ = cx.measure(c, Proposal::fixed(Size::new(s, s)));
        }
        Size::new(s, s)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        for c in cx.children() {
            cx.place(c, Rect::from_size(size));
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        if !cx.ui.tree.children(cx.id).is_empty() {
            return;
        }
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let enabled = !cx.is_disabled();
        let over = enabled && self.drag_over;
        let radius = self.size.radius();
        let shape = Shape::rounded(radius);
        let ink = if over {
            c.accent_amalfi
        } else {
            c.text_secondary
        };
        let mut border = Border::inside(if over { c.accent_amalfi } else { c.line_border }, 1.5);
        if !over {
            border.dash = vec![6.0, 4.0];
        }
        cx.rec.layer(
            "Background",
            r,
            shape.clone(),
            Style::fill(if over {
                c.accent_amalfi_soft
            } else {
                c.surface_sunken
            })
            .with_border(border),
        );
        if over {
            focus_halo_named(cx.rec, "Focus Halo", r, shape, &theme);
        }
        let g = self.size.glyph();
        let hint = if over { &self.drop_hint } else { &self.hint };
        let style = theme.text(&typography::CAPTION, ink);
        let lh = style.line_height.unwrap_or(14.0);
        let show = self.size.shows_hint();
        let hint_w = if show {
            self.label.width(hint, &style)
        } else {
            0.0
        };
        let total = if show { g + 6.0 + lh } else { g };
        let top = r.y + (r.height - total) / 2.0;
        cx.rec.glyph(
            "Photo",
            Rect::new(r.x + (r.width - g) / 2.0, top, g, g),
            "photo",
            400,
            ink,
        );
        if show {
            let layout = self
                .label
                .layout(hint, &style, Align::Start, hint_w, Some(1));
            cx.rec.text(
                "Hint",
                Rect::new(r.x + (r.width - hint_w) / 2.0, top + g + 6.0, hint_w, lh),
                layout,
                Some(typography::CAPTION.name),
                ink,
            );
        }
    }

    fn paint_over(&mut self, cx: &mut PaintCx<'_>) {
        if cx.ui.tree.children(cx.id).is_empty() {
            return;
        }
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let shape = Shape::rounded(self.size.radius());
        let focused = cx.focus_visible() && !cx.is_disabled();
        let (color, width) = if focused {
            (c.accent_amalfi, 1.5)
        } else {
            (c.line_border, 1.0)
        };
        cx.rec.layer(
            "Rim",
            r,
            shape.clone(),
            Style::default().with_border(Border::inside(color, width)),
        );
        if focused {
            focus_halo_named(cx.rec, "Focus Halo", r, shape, &theme);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Drop { items, .. } => {
                self.drag_over = false;
                cx.emit(Signal::Dropped, Value::StrList(items.clone()));
                cx.request_paint();
                Handled::Yes
            }
            Event::Activate => {
                cx.emit(Signal::Activated, Value::None);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn interaction(&self) -> Interaction {
        Interaction::Button
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::State => value.as_bool().map_or(PropChange::Unknown, |b| {
                self.drag_over = b;
                PropChange::Paint
            }),
            Prop::Placeholder => value.as_str().map_or(PropChange::Unknown, |s| {
                self.hint = s.to_string();
                PropChange::Paint
            }),
            Prop::ControlSize => match value.as_int() {
                Some(0 | 1) => {
                    self.size = ImageWellSize::Small;
                    PropChange::Layout
                }
                Some(2) => {
                    self.size = ImageWellSize::Regular;
                    PropChange::Layout
                }
                Some(3) => {
                    self.size = ImageWellSize::Large;
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::State => Some(Value::Bool(self.drag_over)),
            Prop::Placeholder => Some(Value::Str(self.hint.clone())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Image;
        a.actions = vec![Action::Click, Action::Focus];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Button, Mods};
    use crate::core::window::WindowKind;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn a_swatch_row_is_192_wide() {
        let mut ui = Ui::new_headless();
        let items: Vec<(String, Color)> = (0..6)
            .map(|i| (format!("c{i}"), Color::rgb(i * 40, 0, 0)))
            .collect();
        let (row, chips) = swatch_row(&mut ui, &items, Some(0));
        assert_eq!(
            ui.measure(row, Proposal::UNSPECIFIED),
            Size::new(192.0, 32.0)
        );
        assert_eq!(chips.len(), 6);
    }

    #[test]
    fn the_panel_opens_under_the_well_and_a_chip_sets_the_colour() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(400.0, 300.0));
        let well = ui.create(ColorWell::new(Color::rgb(14, 119, 230)));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let s = seen.clone();
        ui.connect(well, Signal::ValueChanged, move |_, _, v| {
            s.borrow_mut().push(v.as_color());
        });
        ui.set_content(win, well);
        ui.show_window(win);
        ui.prepare_frame(win);
        let c = ui.tree.window_rect(well).center();
        ui.pointer_move(win, c, Mods::empty());
        ui.pointer_down(win, c, Button::Left, Mods::empty());
        ui.pointer_up(win, c, Button::Left, Mods::empty());
        let popups: Vec<_> = ui
            .windows()
            .filter(|w| matches!(w.kind, WindowKind::Popup { .. }))
            .map(|w| (w.id, w.root))
            .collect();
        assert_eq!(popups.len(), 1);
        let Some((popup, root)) = popups.first().copied() else {
            return;
        };
        ui.prepare_frame(popup);
        let chips: Vec<_> = ui
            .tree
            .descendants(root)
            .into_iter()
            .filter(|v| ui.widget::<SwatchChip>(*v).is_some())
            .collect();
        assert_eq!(chips.len(), 24);
        let r = ui.tree.window_rect(chips[1]);
        ui.pointer_move(popup, r.center(), Mods::empty());
        ui.pointer_down(popup, r.center(), Button::Left, Mods::empty());
        ui.pointer_up(popup, r.center(), Button::Left, Mods::empty());
        assert_eq!(seen.borrow().len(), 1);
        assert!(
            ui.windows()
                .all(|w| !matches!(w.kind, WindowKind::Popup { .. }))
        );
    }

    #[test]
    fn dropping_files_on_an_image_well() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(300.0, 300.0));
        let w = ui.create(ImageWell::new());
        let got = Rc::new(RefCell::new(Vec::new()));
        let g = got.clone();
        ui.connect(w, Signal::Dropped, move |_, _, v| {
            g.borrow_mut().push(v.clone());
        });
        ui.set_content(win, w);
        ui.show_window(win);
        ui.prepare_frame(win);
        let c = ui.tree.window_rect(w).center();
        ui.drop_items(win, c, vec!["file:///a.png".to_string()]);
        assert_eq!(
            *got.borrow(),
            vec![Value::StrList(vec!["file:///a.png".to_string()])]
        );
    }
}
