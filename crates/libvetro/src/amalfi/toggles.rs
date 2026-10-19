//! Checkboxes and radio buttons (kit `Toggles/Checkbox`, `Toggles/Radio`, their labelled
//! rows and the groups). The switch is in [`crate::amalfi::switch`].

use crate::amalfi::foundation::{
    ControlSize, Interaction as Transitions, Label, control_shadow, focus_halo_named, mix, whole,
};
use crate::amalfi::text::Text;
use crate::amalfi::theme::Theme;
use crate::amalfi::tokens::typography;
use crate::core::a11y::{A11y, Action, Role, Toggled};
use crate::core::event::{Event, Handled, Key};
use crate::core::layout::{Alignment, Proposal, Stack};
use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Insets, Point, Rect, Size};
use crate::shape::Shape;
use crate::style::{Border, Style};
use crate::text::Align;

/// The state of a checkbox (a radio button is `Off` or `On`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToggleState {
    /// Unchecked.
    #[default]
    Off,
    /// Checked.
    On,
    /// Neither (a group with some checked).
    Mixed,
}

impl ToggleState {
    /// The ABI number (`Prop::State`).
    pub fn to_int(self) -> i64 {
        match self {
            ToggleState::Off => 0,
            ToggleState::On => 1,
            ToggleState::Mixed => 2,
        }
    }

    /// The state of an ABI number.
    pub fn from_int(n: i64) -> Option<ToggleState> {
        Some(match n {
            0 => ToggleState::Off,
            1 => ToggleState::On,
            2 => ToggleState::Mixed,
            _ => return None,
        })
    }
}

/// Which toggle a [`Toggle`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleKind {
    /// A square box with a check or a dash.
    Checkbox,
    /// A circle with a dot.
    Radio,
}

/// The measures of a box (silica_ui's `AmalfiToggleBoxMetrics`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxMetrics {
    /// Side.
    pub side: f32,
    /// Corner radius.
    pub radius: f32,
    /// The mark's size (the check, the dash or the dot).
    pub mark: f32,
}

impl BoxMetrics {
    /// The inset of the mark.
    pub fn inset(&self) -> f32 {
        (self.side - self.mark) / 2.0
    }

    /// The metrics of `kind` at `size` (Mini is Small).
    pub fn of(kind: ToggleKind, size: ControlSize) -> BoxMetrics {
        let large = size == ControlSize::Large;
        match kind {
            ToggleKind::Checkbox => BoxMetrics {
                side: if large { 20.0 } else { 16.0 },
                radius: match size {
                    ControlSize::Mini | ControlSize::Small => 4.0,
                    ControlSize::Regular => 5.0,
                    ControlSize::Large => 6.0,
                },
                mark: if large { 16.0 } else { 12.0 },
            },
            ToggleKind::Radio => BoxMetrics {
                side: if large { 20.0 } else { 16.0 },
                radius: if large { 10.0 } else { 8.0 },
                mark: 8.0,
            },
        }
    }
}

/// The labelled row's measures (`AmalfiToggleRowMetrics`).
pub mod row {
    /// Horizontal padding.
    pub const PADDING: f32 = 4.0;
    /// Gap between box and label.
    pub const GAP: f32 = 8.0;
    /// Row height.
    pub const HEIGHT: f32 = 20.0;
    /// Space after the label text inside its box.
    pub const LABEL_TRAILING: f32 = 8.0;
    /// The label box's width is a multiple of this (the 4 pt grid).
    pub const STEP: f32 = 4.0;
}

/// Records a toggle box at `origin`: `Box`, `State`, `Focus Halo` and the mark, their names
/// under `prefix` (`Checkbox box/`).
#[allow(clippy::too_many_arguments)]
fn paint_box(
    cx: &mut PaintCx<'_>,
    prefix: &str,
    origin: Point,
    kind: ToggleKind,
    m: BoxMetrics,
    state: ToggleState,
    t: &Transitions,
) {
    let theme: Theme = *cx.theme();
    let c = theme.colors;
    let rect = Rect::new(origin.x, origin.y, m.side, m.side);
    let shape = Shape::rounded(m.radius);
    let on = match kind {
        ToggleKind::Checkbox => state != ToggleState::Off,
        ToggleKind::Radio => state == ToggleState::On,
    };
    let (hover, press) = (t.hover(), t.press());
    let mut style = Style::default();
    if on {
        if let Some(f) = mix(Some(c.accent_amalfi), Some(c.accent_amalfi_pressed), press) {
            style.fills.push(f.into());
        }
    } else {
        if let Some(f) = mix(Some(c.surface_raised), Some(c.accent_amalfi_soft), press) {
            style.fills.push(f.into());
        }
        let k = hover.max(press).clamp(0.0, 1.0);
        style
            .borders
            .push(Border::inside(c.line_border.lerp(c.accent_amalfi, k), 1.0));
        style.shadows.push(control_shadow(&theme));
    }
    cx.rec
        .layer(&format!("{prefix}Box"), rect, shape.clone(), style);
    if on && hover > 0.0 {
        cx.rec.layer(
            &format!("{prefix}State"),
            rect,
            shape.clone(),
            Style::fill(c.state_hover_overlay).with_opacity(hover),
        );
    }
    if cx.focus_visible() && !cx.is_disabled() {
        focus_halo_named(cx.rec, &format!("{prefix}Focus Halo"), rect, shape, &theme);
    }
    if on {
        let i = m.inset();
        let mark = Rect::new(origin.x + i, origin.y + i, m.mark, m.mark);
        match kind {
            ToggleKind::Checkbox => {
                let (name, symbol) = if state == ToggleState::Mixed {
                    ("Glyph Dash", "minus")
                } else {
                    ("Glyph Check", "checkmark")
                };
                cx.rec.glyph(
                    &format!("{prefix}{name}"),
                    mark,
                    symbol,
                    400,
                    c.text_on_accent,
                );
            }
            ToggleKind::Radio => cx.rec.layer(
                &format!("{prefix}Dot"),
                mark,
                Shape::rounded(m.mark / 2.0),
                Style::fill(c.text_on_accent),
            ),
        }
    }
}

/// A checkbox or a radio button, alone or with its label.
#[derive(Debug)]
pub struct Toggle {
    /// Which toggle.
    pub kind: ToggleKind,
    /// The label (a labelled row).
    pub label: Option<String>,
    /// Size (Mini is Small).
    pub size: ControlSize,
    /// State.
    pub state: ToggleState,
    text: Label,
    transitions: Transitions,
}

impl Toggle {
    fn new(kind: ToggleKind) -> Toggle {
        Toggle {
            kind,
            label: None,
            size: ControlSize::Regular,
            state: ToggleState::Off,
            text: Label::default(),
            transitions: Transitions::default(),
        }
    }

    /// An unchecked checkbox.
    pub fn checkbox() -> Toggle {
        Toggle::new(ToggleKind::Checkbox)
    }

    /// An unselected radio button.
    pub fn radio() -> Toggle {
        Toggle::new(ToggleKind::Radio)
    }

    /// With a label.
    pub fn label(mut self, label: &str) -> Toggle {
        self.label = Some(label.to_string());
        self
    }

    /// With a size.
    pub fn size(mut self, size: ControlSize) -> Toggle {
        self.size = size;
        self
    }

    /// With a state.
    pub fn state(mut self, state: ToggleState) -> Toggle {
        self.state = state;
        self
    }

    fn metrics(&self) -> BoxMetrics {
        BoxMetrics::of(self.kind, self.size)
    }

    fn label_x(&self) -> f32 {
        row::PADDING + self.metrics().side + row::GAP
    }

    fn label_box(&mut self, theme: &Theme) -> f32 {
        let Some(l) = self.label.clone() else {
            return 0.0;
        };
        let style = theme.text(&typography::BODY, theme.colors.text_primary);
        whole(self.text.width(&l, &style) + row::LABEL_TRAILING, row::STEP)
    }

    /// The siblings of `id` that are radio buttons (the radio group).
    fn radio_group(ui: &Ui, id: ViewId) -> Vec<ViewId> {
        let Some(parent) = ui.tree.parent(id) else {
            return vec![id];
        };
        ui.tree
            .children(parent)
            .iter()
            .copied()
            .filter(|c| {
                *c == id
                    || ui
                        .widget::<Toggle>(*c)
                        .is_some_and(|t| t.kind == ToggleKind::Radio)
            })
            .collect()
    }

    /// Selects `target` of a radio group and deselects its siblings.
    fn select_exclusive(ui: &mut Ui, target: ViewId, group: &[ViewId]) {
        for other in group {
            if *other != target {
                ui.with_widget::<Toggle, _>(*other, |t| t.state = ToggleState::Off);
            }
        }
        ui.with_widget::<Toggle, _>(target, |t| t.state = ToggleState::On);
    }
}

impl Widget for Toggle {
    fn kind(&self) -> &'static str {
        match self.kind {
            ToggleKind::Checkbox => "Checkbox",
            ToggleKind::Radio => "Radio",
        }
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let side = self.metrics().side;
        if self.label.is_none() {
            return Size::new(side, side);
        }
        let theme = cx.theme();
        let natural = self.label_x() + self.label_box(&theme) + row::PADDING;
        let min = self.label_x() + row::PADDING;
        let w = match p.width {
            Some(w) if w.is_finite() => w.clamp(min, natural.max(min)),
            _ => natural,
        };
        Size::new(w, row::HEIGHT)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let m = self.metrics();
        let labelled = self.label.is_some();
        let (prefix, origin) = if labelled {
            let name = match self.kind {
                ToggleKind::Checkbox => "Checkbox box/",
                ToggleKind::Radio => "Radio box/",
            };
            (
                name,
                Point::new(r.x + row::PADDING, r.y + (r.height - m.side) / 2.0),
            )
        } else {
            ("", Point::new(r.x, r.y))
        };
        paint_box(
            cx,
            prefix,
            origin,
            self.kind,
            m,
            self.state,
            &self.transitions,
        );
        let Some(label) = self.label.clone() else {
            return;
        };
        let style = theme.text(&typography::BODY, theme.colors.text_primary);
        let lh = style.line_height.unwrap_or(18.0);
        let avail = (r.width - self.label_x() - row::PADDING).max(0.0);
        let box_w = whole(
            self.text.width(&label, &style) + row::LABEL_TRAILING,
            row::STEP,
        )
        .min(avail);
        let layout = self.text.layout(
            &label,
            &style,
            Align::Start,
            (box_w - row::LABEL_TRAILING).max(0.0),
            Some(1),
        );
        cx.rec.text(
            "Label",
            Rect::new(r.x + self.label_x(), r.y + (r.height - lh) / 2.0, box_w, lh),
            layout,
            Some(typography::BODY.name),
            style.color,
        );
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Activate => {
                match self.kind {
                    ToggleKind::Checkbox => {
                        self.state = if self.state == ToggleState::On {
                            ToggleState::Off
                        } else {
                            ToggleState::On
                        };
                        let on = self.state == ToggleState::On;
                        cx.emit(Signal::ValueChanged, Value::Bool(on));
                    }
                    ToggleKind::Radio => {
                        if self.state != ToggleState::On {
                            let id = cx.id();
                            let group = Toggle::radio_group(cx.ui(), id);
                            for other in group.iter().filter(|o| **o != id) {
                                cx.ui().with_widget::<Toggle, _>(*other, |t| {
                                    t.state = ToggleState::Off
                                });
                            }
                            self.state = ToggleState::On;
                            cx.emit(Signal::ValueChanged, Value::Bool(true));
                        }
                    }
                }
                cx.request_paint();
                Handled::Yes
            }
            Event::Key(k) if k.pressed && self.kind == ToggleKind::Radio => {
                let step: isize = match k.key {
                    Key::Down | Key::Right => 1,
                    Key::Up | Key::Left => -1,
                    _ => return Handled::No,
                };
                let id = cx.id();
                let group = Toggle::radio_group(cx.ui(), id);
                let n = group.len();
                let Some(at) = group.iter().position(|g| *g == id) else {
                    return Handled::No;
                };
                for k in 1..n {
                    let i = (at as isize + step * k as isize).rem_euclid(n as isize) as usize;
                    let Some(&cand) = group.get(i) else { continue };
                    if cx.ui().is_disabled(cand) {
                        continue;
                    }
                    self.state = ToggleState::Off;
                    Toggle::select_exclusive(cx.ui(), cand, &group);
                    cx.ui().emit(cand, Signal::ValueChanged, Value::Bool(true));
                    cx.ui().set_focus(cand, true);
                    cx.request_paint();
                    break;
                }
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
        Interaction::Toggle
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::State => match value.as_int().and_then(ToggleState::from_int) {
                Some(s) => {
                    self.state = s;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            Prop::Title => match value {
                Value::Str(s) => {
                    self.label = Some(s.clone()).filter(|s| !s.is_empty());
                    PropChange::Layout
                }
                Value::None => {
                    self.label = None;
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            Prop::ControlSize => match value.as_int().and_then(ControlSize::from_int) {
                Some(s) => {
                    self.size = s;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::State => Some(Value::Int(self.state.to_int())),
            Prop::Title => Some(self.label.clone().map(Value::Str).unwrap_or_default()),
            Prop::ControlSize => Some(Value::Int(self.size.to_int())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = match self.kind {
            ToggleKind::Checkbox => Role::CheckBox,
            ToggleKind::Radio => Role::RadioButton,
        };
        a.label.clone_from(&self.label);
        a.toggled = Some(match self.state {
            ToggleState::Off => Toggled::False,
            ToggleState::On => Toggled::True,
            ToggleState::Mixed => Toggled::Mixed,
        });
        a.actions = vec![Action::Click, Action::Focus];
    }

    fn baseline(&self) -> Option<f32> {
        self.label.as_ref()?;
        Some((row::HEIGHT - 18.0) / 2.0 + 18.0 * 0.75)
    }
}

/// A heading over a column of toggles (kit `Toggles/Checkbox Group/List`, `Radio Group/List`):
/// padding 8 / 10 / 8 / 8, rows 4 apart. The rows are views already made; give them an
/// identifier (their label) to find them in the inspector.
pub fn toggle_group(ui: &mut Ui, heading: Option<&str>, rows: &[ViewId]) -> ViewId {
    let group = ui.create(Stack::column(4.0).aligned(Alignment::LEADING));
    let _ = ui.set(
        group,
        Prop::Padding,
        Value::Insets(Insets::new(10.0, 8.0, 8.0, 8.0)),
    );
    if let Some(h) = heading {
        let t = ui.create(
            Text::new(h)
                .token(&typography::HEADLINE)
                .layer("Heading")
                .whole(row::STEP, 0.0),
        );
        ui.add_child(group, t);
    }
    for r in rows {
        ui.add_child(group, *r);
    }
    group
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Button, KeyEvent, Mods};
    use crate::core::window::{WindowId, WindowKind};
    use std::cell::RefCell;
    use std::rc::Rc;

    type Log = Rc<RefCell<Vec<(usize, bool)>>>;

    fn scene(rows: Vec<Toggle>) -> (Ui, WindowId, Vec<ViewId>, Log) {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(300.0, 200.0));
        let col = ui.create(Stack::column(4.0).aligned(Alignment::LEADING));
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut ids = Vec::new();
        for (i, t) in rows.into_iter().enumerate() {
            let id = ui.create(t);
            ui.add_child(col, id);
            let l = log.clone();
            ui.connect(id, Signal::ValueChanged, move |_, _, v| {
                l.borrow_mut().push((i, v.as_bool().unwrap_or(false)));
            });
            ids.push(id);
        }
        ui.set_content(win, col);
        ui.show_window(win);
        ui.prepare_frame(win);
        (ui, win, ids, log)
    }

    fn click(ui: &mut Ui, win: WindowId, id: ViewId) {
        let c = ui.tree.window_rect(id).center();
        ui.pointer_down(win, c, Button::Left, Mods::empty());
        ui.pointer_up(win, c, Button::Left, Mods::empty());
    }

    fn state(ui: &Ui, id: ViewId) -> ToggleState {
        ui.widget::<Toggle>(id).map(|t| t.state).unwrap_or_default()
    }

    #[test]
    fn checkbox_toggles_on_click_label_and_space() {
        let (mut ui, win, ids, log) = scene(vec![Toggle::checkbox().label("Show hidden files")]);
        let id = ids[0];
        click(&mut ui, win, id);
        assert_eq!(state(&ui, id), ToggleState::On);
        // On the label, far from the box.
        let r = ui.tree.window_rect(id);
        let p = Point::new(r.right() - 10.0, r.center().y);
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        assert_eq!(state(&ui, id), ToggleState::Off);
        ui.set_focus(id, true);
        ui.key(win, KeyEvent::press(Key::Space, Mods::empty()));
        let mut up = KeyEvent::press(Key::Space, Mods::empty());
        up.pressed = false;
        ui.key(win, up);
        assert_eq!(state(&ui, id), ToggleState::On);
        assert_eq!(*log.borrow(), vec![(0, true), (0, false), (0, true)]);
    }

    #[test]
    fn mixed_turns_on_and_disabled_ignores_input() {
        let (mut ui, win, ids, log) = scene(vec![
            Toggle::checkbox().state(ToggleState::Mixed),
            Toggle::checkbox(),
        ]);
        let _ = ui.set(ids[1], Prop::Disabled, Value::Bool(true));
        click(&mut ui, win, ids[0]);
        click(&mut ui, win, ids[1]);
        assert_eq!(state(&ui, ids[0]), ToggleState::On);
        assert_eq!(state(&ui, ids[1]), ToggleState::Off);
        assert_eq!(*log.borrow(), vec![(0, true)]);
    }

    #[test]
    fn a_release_outside_does_not_toggle() {
        let (mut ui, win, ids, log) = scene(vec![Toggle::checkbox()]);
        let c = ui.tree.window_rect(ids[0]).center();
        ui.pointer_down(win, c, Button::Left, Mods::empty());
        ui.pointer_move(win, Point::new(250.0, 150.0), Mods::empty());
        ui.pointer_up(win, Point::new(250.0, 150.0), Button::Left, Mods::empty());
        assert!(log.borrow().is_empty());
    }

    #[test]
    fn radios_select_exclusively_and_arrows_skip_disabled() {
        let (mut ui, win, ids, log) = scene(vec![
            Toggle::radio().label("Name").state(ToggleState::On),
            Toggle::radio().label("Date"),
            Toggle::radio().label("Size"),
        ]);
        let _ = ui.set(ids[1], Prop::Disabled, Value::Bool(true));
        click(&mut ui, win, ids[2]);
        assert_eq!(state(&ui, ids[0]), ToggleState::Off);
        assert_eq!(state(&ui, ids[2]), ToggleState::On);
        // A click on the selected one changes nothing.
        click(&mut ui, win, ids[2]);
        assert_eq!(log.borrow().len(), 1);
        ui.set_focus(ids[2], true);
        ui.key(win, KeyEvent::press(Key::Down, Mods::empty()));
        assert_eq!(state(&ui, ids[0]), ToggleState::On);
        assert_eq!(state(&ui, ids[2]), ToggleState::Off);
        assert_eq!(ui.focus(win), Some(ids[0]));
        ui.key(win, KeyEvent::press(Key::Up, Mods::empty()));
        assert_eq!(state(&ui, ids[2]), ToggleState::On);
    }

    #[test]
    fn properties_and_accessibility() {
        let (mut ui, _win, ids, _log) = scene(vec![Toggle::checkbox()]);
        let id = ids[0];
        assert!(ui.set(id, Prop::State, Value::Int(2)).is_ok());
        assert_eq!(ui.get(id, Prop::State), Some(Value::Int(2)));
        assert!(ui.set(id, Prop::Title, Value::from("Wi-Fi")).is_ok());
        let a = ui.accessibility_of(id).unwrap_or_default();
        assert_eq!(a.role, Role::CheckBox);
        assert_eq!(a.toggled, Some(Toggled::Mixed));
        assert_eq!(a.label.as_deref(), Some("Wi-Fi"));
    }
}
