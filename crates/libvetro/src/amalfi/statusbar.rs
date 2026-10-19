//! Status Bar (kit `Status Bar`): the status items (glyph, clock, battery, input source) and the
//! bar that lines them up, with the plate of the Increase Contrast and Reduce Transparency
//! looks. The bar is the menu bar's right side and the lock screen's.

use crate::amalfi::foundation::{Label, whole};
use crate::amalfi::theme::Theme;
use crate::amalfi::tokens::typography;
use crate::color::Color;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Event, Handled, Key};
use crate::core::layout::Proposal;
use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Rect, Size};
use crate::shape::Shape;
use crate::style::{Blur, Border, Shadow, Style};
use crate::text::Align;

/// Fixed measures of the kit.
pub mod metrics {
    /// An item's height.
    pub const ITEM_HEIGHT: f32 = 28.0;
    /// The bar's height (2 pt above and below the items).
    pub const BAR_HEIGHT: f32 = 32.0;
    /// An item's padding at its sides.
    pub const PADDING: f32 = 8.0;
    /// A status glyph's size.
    pub const GLYPH: f32 = 20.0;
    /// Gap between an item's text and glyph.
    pub const GAP: f32 = 4.0;
    /// The selection's and the plate's corner radius.
    pub const RADIUS: f32 = 14.0;
    /// The label grid.
    pub const STEP: f32 = 4.0;
}

/// What an item shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemContent {
    /// A glyph (Wi-Fi, sound …).
    Glyph(String),
    /// The date and time.
    Clock(String),
    /// A glyph after a percentage.
    Battery {
        /// The glyph.
        symbol: String,
        /// The text before it (`84%`).
        percentage: String,
    },
    /// A glyph after the name of the input source.
    InputSource {
        /// The glyph.
        symbol: String,
        /// The name.
        label: String,
    },
}

/// An item of the status bar.
#[derive(Debug)]
pub struct StatusItem {
    /// What it shows.
    pub content: ItemContent,
    /// The item's menu is open (the selection shows).
    pub active: bool,
    /// Drawn in the dark palette whatever the appearance (on the lock screen's wallpaper).
    pub on_dark: bool,
    /// The accessible label.
    pub title: String,
    text: Label,
    hovered: bool,
}

impl StatusItem {
    /// An item.
    pub fn new(content: ItemContent) -> StatusItem {
        StatusItem {
            content,
            active: false,
            on_dark: false,
            title: String::new(),
            text: Label::default(),
            hovered: false,
        }
    }

    /// A glyph item.
    pub fn glyph(symbol: &str) -> StatusItem {
        StatusItem::new(ItemContent::Glyph(symbol.to_string()))
    }

    /// With an accessible label.
    pub fn titled(mut self, title: &str) -> StatusItem {
        self.title = title.to_string();
        self
    }

    fn ink(&self, theme: &Theme) -> Color {
        if self.on_dark {
            Theme::dark_theme().colors.text_primary
        } else {
            theme.colors.text_primary
        }
    }

    fn text_width(&mut self, theme: &Theme, text: &str) -> f32 {
        let style = theme.text(&typography::BODY_EMPHASIS, theme.colors.text_primary);
        whole(self.text.width(text, &style), metrics::STEP)
    }

    fn natural_width(&mut self, theme: &Theme) -> f32 {
        let pad = 2.0 * metrics::PADDING;
        match self.content.clone() {
            ItemContent::Glyph(_) => pad + metrics::GLYPH,
            ItemContent::Clock(t) => pad + self.text_width(theme, &t),
            ItemContent::Battery { percentage: t, .. }
            | ItemContent::InputSource { label: t, .. } => {
                pad + self.text_width(theme, &t) + metrics::GAP + metrics::GLYPH
            }
        }
    }
}

impl Widget for StatusItem {
    fn kind(&self) -> &'static str {
        "StatusItem"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let theme = cx.theme();
        let w = self.natural_width(&theme);
        Size::new(
            p.width.filter(|w| w.is_finite()).unwrap_or(w).max(w),
            metrics::ITEM_HEIGHT,
        )
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let ink = self.ink(&theme);
        if self.active || self.hovered {
            let color = theme
                .color("Menu Bar/Selection")
                .unwrap_or(theme.colors.state_hover_overlay);
            cx.rec.layer(
                "Active fill",
                r,
                Shape::rounded(metrics::RADIUS),
                Style::fill(color),
            );
        }
        let pad = metrics::PADDING;
        let glyph_at = |x: f32| Rect::new(r.x + x, r.y + 4.0, metrics::GLYPH, metrics::GLYPH);
        let content = self.content.clone();
        match content {
            ItemContent::Glyph(symbol) => {
                cx.rec
                    .glyph("Glyph Status", glyph_at(pad), &symbol, 400, ink);
            }
            ItemContent::Clock(text) => {
                let style = theme.text(&typography::BODY_EMPHASIS, ink);
                let w = (r.width - 2.0 * pad).max(0.0);
                let layout = self.text.layout(&text, &style, Align::Center, w, Some(1));
                cx.rec.text(
                    "Label",
                    Rect::new(r.x + pad, r.y + 5.0, w, 18.0),
                    layout,
                    Some(typography::BODY_EMPHASIS.name),
                    ink,
                );
            }
            ItemContent::Battery { symbol, percentage } => {
                let style = theme.text(&typography::BODY_EMPHASIS, ink);
                let tw = self.text_width(&theme, &percentage);
                let layout = self
                    .text
                    .layout(&percentage, &style, Align::End, tw, Some(1));
                cx.rec.text(
                    "Percentage",
                    Rect::new(r.x + pad, r.y + 5.0, tw, 18.0),
                    layout,
                    Some(typography::BODY_EMPHASIS.name),
                    ink,
                );
                cx.rec.glyph(
                    "Glyph Status",
                    glyph_at(pad + tw + metrics::GAP),
                    &symbol,
                    400,
                    ink,
                );
            }
            ItemContent::InputSource { symbol, label } => {
                let style = theme.text(&typography::BODY_EMPHASIS, ink);
                let tw = self.text_width(&theme, &label);
                let layout = self.text.layout(&label, &style, Align::Start, tw, Some(1));
                cx.rec.text(
                    "Label",
                    Rect::new(r.x + pad, r.y + 5.0, tw, 18.0),
                    layout,
                    Some(typography::BODY_EMPHASIS.name),
                    ink,
                );
                cx.rec.glyph(
                    "Glyph Status",
                    glyph_at(pad + tw + metrics::GAP),
                    &symbol,
                    400,
                    ink,
                );
            }
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Activate => {
                cx.emit(Signal::Activated, Value::None);
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
        let hovered = new.contains(NodeFlags::HOVERED);
        if hovered != self.hovered {
            self.hovered = hovered;
            cx.request_paint();
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
            Prop::Expanded => value.as_bool().map_or(PropChange::Unknown, |b| {
                self.active = b;
                PropChange::Paint
            }),
            Prop::Title | Prop::AccessibilityLabel => {
                value.as_str().map_or(PropChange::Unknown, |s| {
                    self.title = s.to_string();
                    PropChange::Nothing
                })
            }
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Expanded => Some(Value::Bool(self.active)),
            Prop::Title => Some(Value::Str(self.title.clone())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Button;
        a.label = Some(self.title.clone()).filter(|t| !t.is_empty());
        a.expanded = Some(self.active).filter(|b| *b);
        a.actions = vec![Action::Click, Action::Focus];
    }
}

/// The status bar: items side by side, 2 pt below the top, and — with Increase Contrast or
/// Reduce Transparency — the plate behind them.
#[derive(Debug, Default)]
pub struct StatusBar;

impl StatusBar {
    fn plate(theme: &Theme) -> Option<Style> {
        let a = theme.appearance;
        let dark = theme.dark();
        let c = |v: u32| Color::from_u32(v);
        if a.reduce_transparency {
            // The materials fallback: the fill, the rims kept, no blur.
            let (fill, rim, light) = if dark {
                (c(0x5b5c5eff), c(0xffffff3d), c(0xffffff38))
            } else {
                (c(0xffffffff), c(0x0000001f), c(0xffffffb3))
            };
            let mut style = Style::fill(fill).with_border(Border::inside(rim, 1.0));
            style.inner_shadows.push(rim_light(light));
            Some(style)
        } else if a.increase_contrast {
            let (fill, rim) = if dark {
                (c(0x2a2b30f2), c(0xffffff99))
            } else {
                (c(0xfffffff2), c(0x1d1e2199))
            };
            let mut style = Style::fill(fill).with_border(Border::inside(rim, 1.0));
            style.blurs.push(Blur::Background {
                radius: 20.0,
                saturation: 1.0,
                brightness: 1.0,
            });
            Some(style)
        } else {
            None
        }
    }
}

/// The rim light of the lock materials: a 1 pt inner shadow below the top edge.
fn rim_light(color: Color) -> Shadow {
    Shadow {
        color,
        x: 0.0,
        y: 1.0,
        blur: 1.0,
        spread: 0.0,
    }
}

impl Widget for StatusBar {
    fn kind(&self) -> &'static str {
        "StatusBar"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let mut w = 0.0;
        for c in cx.children() {
            w += cx.measure(c, Proposal::new(None, None)).width;
        }
        Size::new(w, metrics::BAR_HEIGHT)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, _size: Size) {
        let mut x = 0.0;
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(None, None));
            cx.place(c, Rect::new(x, 2.0, s.width, s.height));
            x += s.width;
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        if let Some(style) = StatusBar::plate(&theme) {
            cx.rec.layer(
                "Plate",
                Rect::new(r.x, r.y + 2.0, r.width, metrics::ITEM_HEIGHT),
                Shape::rounded(metrics::RADIUS),
                style,
            );
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Toolbar;
        a.label = Some("Status".to_string());
    }
}

/// Makes a status bar over `items`.
pub fn status_bar(ui: &mut Ui, items: &[ViewId]) -> ViewId {
    let b = ui.create(StatusBar);
    for i in items {
        ui.add_child(b, *i);
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::window::WindowKind;

    #[test]
    fn items_are_28_high_and_a_glyph_item_36_wide() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(300.0, 100.0));
        let a = ui.create(StatusItem::glyph("wifi"));
        let b = ui.create(StatusItem::new(ItemContent::Battery {
            symbol: "battery".into(),
            percentage: "84%".into(),
        }));
        let bar = status_bar(&mut ui, &[a, b]);
        ui.set_content(win, bar);
        ui.show_window(win);
        ui.prepare_frame(win);
        let ra = ui.tree.window_rect(a);
        let rb = ui.tree.window_rect(b);
        assert_eq!((ra.width, ra.height), (36.0, 28.0));
        assert_eq!(rb.x, ra.right());
        assert_eq!(ui.tree.window_rect(bar).height, 32.0);
    }

    #[test]
    fn expanded_shows_the_selection() {
        let mut ui = Ui::new_headless();
        let a = ui.create(StatusItem::glyph("wifi"));
        let _ = ui.set(a, Prop::Expanded, Value::Bool(true));
        assert_eq!(ui.get(a, Prop::Expanded), Some(Value::Bool(true)));
    }
}
