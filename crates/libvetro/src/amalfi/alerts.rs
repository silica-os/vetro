//! Alerts (kit `Alerts`): the alert panel (icon, title, message, accessories, buttons) in its
//! solid and glass looks, with the warning badge of the critical alerts.

use crate::amalfi::button::{PushButton, PushVariant};
use crate::amalfi::text::{Text, TextRole};
use crate::amalfi::tokens::typography;
use crate::core::a11y::{A11y, Role};
use crate::core::event::{Event, Handled, Key};
use crate::core::layout::{Alignment, Proposal, Stack};
use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Insets, Rect, Size};
use crate::shape::Shape;
use crate::style::{Border, Style};

/// Fixed measures of the kit.
pub mod metrics {
    /// The alert's width.
    pub const WIDTH: f32 = 280.0;
    /// The corner radius.
    pub const RADIUS: f32 = 20.0;
    /// Space above the icon.
    pub const TOP: f32 = 24.0;
    /// Space below the buttons.
    pub const BOTTOM: f32 = 20.0;
    /// Space at the sides.
    pub const SIDE: f32 = 20.0;
    /// Gap between the parts.
    pub const GAP: f32 = 16.0;
    /// The width of text and fields.
    pub const CONTENT_WIDTH: f32 = 240.0;
    /// The app icon's size.
    pub const ICON: f32 = 56.0;
    /// The warning badge's disc.
    pub const BADGE: f32 = 24.0;
    /// Gap between the buttons.
    pub const BUTTON_GAP: f32 = 8.0;
    /// The narrowest button.
    pub const BUTTON_MIN_WIDTH: f32 = 72.0;
}

/// The look of the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Material {
    /// `Surface/Raised` with a border and a level 3 shadow.
    #[default]
    Solid,
    /// `Glass/Thick`.
    Glass,
}

/// The alert panel. Its children, top to bottom, are centred: the icon first, then whatever
/// the builder put.
#[derive(Debug)]
pub struct Alert {
    /// The panel's look.
    pub material: Material,
    /// A warning badge on the icon's corner (critical alerts).
    pub badge: bool,
    /// The button that Return activates (its index among the buttons).
    pub default_button: Option<usize>,
}

impl Alert {
    /// A solid alert.
    pub fn new() -> Alert {
        Alert {
            material: Material::Solid,
            badge: false,
            default_button: None,
        }
    }
}

impl Default for Alert {
    fn default() -> Self {
        Alert::new()
    }
}

impl Widget for Alert {
    fn kind(&self) -> &'static str {
        "Alert"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let w = match p.width {
            Some(w) if w.is_finite() => w,
            _ => metrics::WIDTH,
        };
        let inner = (w - 2.0 * metrics::SIDE).max(0.0);
        let kids = cx.children();
        let mut h = metrics::TOP + metrics::BOTTOM;
        for (i, c) in kids.iter().enumerate() {
            if i > 0 {
                h += metrics::GAP;
            }
            h += cx.measure(*c, Proposal::new(Some(inner), None)).height;
        }
        Size::new(w, h)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let inner = (size.width - 2.0 * metrics::SIDE).max(0.0);
        let mut y = metrics::TOP;
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(Some(inner), None));
            let w = s.width.min(inner);
            cx.place(c, Rect::new((size.width - w) / 2.0, y, w, s.height));
            y += s.height + metrics::GAP;
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let shape = Shape::rounded(metrics::RADIUS);
        match self.material {
            Material::Solid => {
                let mut style =
                    Style::fill(c.surface_raised).with_border(Border::inside(c.line_border, 1.0));
                if let Some(s) = theme.shadow(3) {
                    style.shadows.push(s);
                }
                cx.rec.layer("Surface", r, shape, style);
            }
            Material::Glass => {
                cx.rec
                    .layer("Surface", r, shape, theme.material_style("Glass/Thick"));
            }
        }
    }

    fn paint_over(&mut self, cx: &mut PaintCx<'_>) {
        if !self.badge {
            return;
        }
        let theme = *cx.theme();
        let c = theme.colors;
        let Some(icon) = cx.ui.tree.children(cx.id).first().copied() else {
            return;
        };
        let origin = cx.bounds().origin();
        let at = cx.ui.tree.window_rect(icon);
        let me = cx.ui.tree.window_rect(cx.id);
        let icon = Rect::new(
            origin.x + at.x - me.x,
            origin.y + at.y - me.y,
            at.width,
            at.height,
        );
        // The disc sits on the icon's bottom right corner: 40 right and 36 down of its origin.
        let disc = Rect::new(icon.x + 40.0, icon.y + 36.0, metrics::BADGE, metrics::BADGE);
        cx.rec.layer(
            "Badge disc",
            disc,
            Shape::rounded(metrics::BADGE / 2.0),
            Style::fill(c.surface_raised).with_border(Border::inside(c.line_border, 1.0)),
        );
        cx.rec.glyph(
            "Glyph Warning",
            Rect::new(disc.x + 4.0, disc.y + 4.0, 16.0, 16.0),
            "warning",
            400,
            c.palette_terracotta,
        );
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Key(k) if k.pressed && k.key == Key::Escape => {
                cx.emit(Signal::CloseRequested, Value::None);
                Handled::Yes
            }
            Event::Key(k) if k.pressed && k.key == Key::Enter => {
                if let Some(i) = self.default_button {
                    cx.emit(Signal::Activated, Value::Int(i as i64));
                    Handled::Yes
                } else {
                    Handled::No
                }
            }
            _ => Handled::No,
        }
    }

    fn focus(&self) -> FocusKind {
        // The alert takes the keys (Return, Escape) when nothing inside has the focus.
        FocusKind::Always
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value.as_bool()) {
            (Prop::Material, Some(glass)) => {
                self.material = if glass {
                    Material::Glass
                } else {
                    Material::Solid
                };
                PropChange::Paint
            }
            _ => PropChange::Unknown,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Dialog;
    }
}

/// What a button of an alert does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// The default, accent button.
    Primary,
    /// A plain button.
    Secondary,
    /// A destructive default.
    Destructive,
}

/// A button of an alert.
#[derive(Debug, Clone)]
pub struct AlertButton {
    /// The title.
    pub title: String,
    /// What it does.
    pub kind: ButtonKind,
}

impl AlertButton {
    /// A button.
    pub fn new(title: &str, kind: ButtonKind) -> AlertButton {
        AlertButton {
            title: title.to_string(),
            kind,
        }
    }
}

/// What an alert shows.
#[derive(Debug, Default)]
pub struct AlertSpec {
    /// The icon view (56 pt), put first.
    pub icon: Option<ViewId>,
    /// The title.
    pub title: String,
    /// The message under it.
    pub message: Option<String>,
    /// The title and message go in a group (8 pt apart, 4 pt of space below) as in the
    /// Gatekeeper alerts; otherwise they are siblings 16 pt apart.
    pub grouped_text: bool,
    /// Views between the text and the buttons (a suppression checkbox, a field).
    pub accessories: Vec<ViewId>,
    /// The buttons in visual order (left to right, or top to bottom when stacked).
    pub buttons: Vec<AlertButton>,
    /// The buttons stack in a column.
    pub stacked: bool,
    /// Stacked buttons as wide as the text.
    pub wide_buttons: bool,
    /// The panel's look.
    pub glass: bool,
    /// A warning badge on the icon.
    pub badge: bool,
    /// The message's colour role (secondary unless set).
    pub message_role: Option<TextRole>,
    /// The width of text and fields (240 unless set).
    pub text_width: Option<f32>,
}

/// Makes an alert from `spec`. Pressing a button emits `Signal::Activated` on the alert with
/// the button's index; Return does the same for the primary or destructive button, Escape
/// emits `Signal::CloseRequested`.
pub fn alert(ui: &mut Ui, spec: AlertSpec) -> ViewId {
    let mut panel = Alert::new();
    panel.badge = spec.badge;
    if spec.glass {
        panel.material = Material::Glass;
    }
    panel.default_button = spec
        .buttons
        .iter()
        .position(|b| matches!(b.kind, ButtonKind::Primary | ButtonKind::Destructive));
    let a = ui.create(panel);
    if let Some(icon) = spec.icon {
        ui.add_child(a, icon);
    }
    let title = ui.create(
        Text::new(&spec.title)
            .token(&typography::TITLE_3)
            .role(TextRole::Primary)
            .layer("Title")
            .wrapping(),
    );
    let message = spec.message.as_deref().map(|m| {
        ui.create(
            Text::new(m)
                .token(&typography::CALLOUT)
                .role(spec.message_role.unwrap_or(TextRole::Secondary))
                .layer("Message")
                .wrapping(),
        )
    });
    let text_width = spec.text_width.unwrap_or(metrics::CONTENT_WIDTH);
    let width = Value::from(text_width);
    if spec.grouped_text {
        let g = ui.create(Stack::column(8.0).aligned(Alignment::LEADING));
        let _ = ui.set(g, Prop::Width, width.clone());
        let _ = ui.set(
            g,
            Prop::Padding,
            Value::Insets(Insets {
                bottom: 4.0,
                ..Insets::ZERO
            }),
        );
        let _ = ui.set(g, Prop::Identifier, Value::from("Text"));
        let _ = ui.set(title, Prop::Width, width.clone());
        ui.add_child(g, title);
        if let Some(m) = message {
            let _ = ui.set(m, Prop::Width, width.clone());
            ui.add_child(g, m);
        }
        ui.add_child(a, g);
    } else {
        let _ = ui.set(title, Prop::Width, width.clone());
        ui.add_child(a, title);
        if let Some(m) = message {
            let _ = ui.set(m, Prop::Width, width.clone());
            ui.add_child(a, m);
        }
    }
    for v in spec.accessories {
        ui.add_child(a, v);
    }
    if !spec.buttons.is_empty() {
        let row = if spec.stacked {
            ui.create(Stack::column(metrics::BUTTON_GAP))
        } else {
            ui.create(Stack::row(metrics::BUTTON_GAP))
        };
        let _ = ui.set(row, Prop::Identifier, Value::from("Buttons"));
        for (i, b) in spec.buttons.iter().enumerate() {
            let variant = match b.kind {
                ButtonKind::Primary => PushVariant::Primary,
                ButtonKind::Secondary => PushVariant::Secondary,
                ButtonKind::Destructive => PushVariant::Destructive,
            };
            let v = ui.create(PushButton::new(&b.title).variant(variant));
            let _ = ui.set(v, Prop::Identifier, Value::from(b.title.as_str()));
            let _ = ui.set(v, Prop::MinWidth, Value::from(metrics::BUTTON_MIN_WIDTH));
            if spec.stacked && spec.wide_buttons {
                let _ = ui.set(v, Prop::Width, Value::from(metrics::CONTENT_WIDTH));
            }
            ui.add_child(row, v);
            ui.connect(v, Signal::Activated, move |ui, _, _| {
                ui.emit(a, Signal::Activated, Value::Int(i as i64));
            });
        }
        ui.add_child(a, row);
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Button as Btn, KeyEvent, Mods};
    use crate::core::window::WindowKind;
    use crate::geom::Point;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn spec() -> AlertSpec {
        AlertSpec {
            title: "Empty the outbox?".into(),
            message: Some("Messages waiting to be sent will be sent now.".into()),
            buttons: vec![
                AlertButton::new("Cancel", ButtonKind::Secondary),
                AlertButton::new("Send", ButtonKind::Primary),
            ],
            ..AlertSpec::default()
        }
    }

    fn shown() -> (
        Ui,
        crate::core::window::WindowId,
        ViewId,
        Rc<RefCell<Vec<i64>>>,
    ) {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(280.0, 400.0));
        let a = alert(&mut ui, spec());
        let log = Rc::new(RefCell::new(Vec::new()));
        let l = log.clone();
        ui.connect(a, Signal::Activated, move |_, _, v| {
            l.borrow_mut().push(v.as_int().unwrap_or(-1));
        });
        let l = log.clone();
        ui.connect(a, Signal::CloseRequested, move |_, _, _| {
            l.borrow_mut().push(-2);
        });
        ui.set_content(win, a);
        ui.show_window(win);
        ui.set_focus(a, true);
        ui.prepare_frame(win);
        (ui, win, a, log)
    }

    #[test]
    fn the_alert_is_280_wide_and_centres_its_parts() {
        let (ui, _, a, _) = shown();
        let r = ui.tree.window_rect(a);
        assert_eq!(r.width, 280.0);
        // 24 + title 22 + 16 + message 32 + 16 + button 28 + 20.
        assert_eq!(r.height, 24.0 + 22.0 + 16.0 + 32.0 + 16.0 + 28.0 + 20.0);
    }

    #[test]
    fn a_button_click_reports_its_index() {
        let (mut ui, win, a, log) = shown();
        let kids = ui.tree.children(a).to_vec();
        let row = *kids.last().unwrap_or(&a);
        let send = ui.tree.children(row)[1];
        let r = ui.tree.window_rect(send);
        let p = Point::new(r.x + 10.0, r.y + 10.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Btn::Left, Mods::empty());
        ui.pointer_up(win, p, Btn::Left, Mods::empty());
        assert_eq!(*log.borrow(), vec![1]);
    }

    #[test]
    fn return_takes_the_default_button_and_escape_closes() {
        let (mut ui, win, _, log) = shown();
        ui.key(win, KeyEvent::press(Key::Enter, Mods::empty()));
        ui.key(win, KeyEvent::press(Key::Escape, Mods::empty()));
        assert_eq!(*log.borrow(), vec![1, -2]);
    }
}
