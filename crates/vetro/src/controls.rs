//! Builders for the Amalfi controls, bound to state: each sets the control's value from the
//! binding on every render and writes the user's changes back.

use std::rc::Rc;

use vetro_kit::{Kind, Prop, Signal, Value};

use crate::Binding;
use crate::element::{Element, PValue};

fn s(text: &str) -> PValue {
    PValue::Str(Rc::from(text))
}

fn list(items: &[&str]) -> PValue {
    PValue::Strings(
        items
            .iter()
            .map(|i| (*i).to_string())
            .collect::<Vec<_>>()
            .into(),
    )
}

/// A checkbox bound to a boolean.
pub fn checkbox(title: &str, value: &Binding<bool>) -> Element {
    let b = value.clone();
    Element::of(Kind::CHECKBOX)
        .prop(Prop::TITLE, s(title))
        .prop(Prop::STATE, PValue::Int(i64::from(value.get())))
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Some(on) = v.as_bool() {
                b.set(on);
            }
        })
}

/// A radio button: `selected`, and `select` when the user chooses it.
pub fn radio(title: &str, selected: bool, select: impl Fn() + 'static) -> Element {
    Element::of(Kind::RADIO)
        .prop(Prop::TITLE, s(title))
        .prop(Prop::STATE, PValue::Int(i64::from(selected)))
        .on(Signal::VALUE_CHANGED, move |v| {
            if v.as_bool() == Some(true) {
                select();
            }
        })
}

/// A switch bound to a boolean.
pub fn switch(value: &Binding<bool>) -> Element {
    let b = value.clone();
    Element::of(Kind::SWITCH)
        .prop(Prop::VALUE, PValue::Bool(value.get()))
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Some(on) = v.as_bool() {
                b.set(on);
            }
        })
}

/// A slider bound to a number in `min..=max`.
pub fn slider(value: &Binding<f64>, min: f64, max: f64) -> Element {
    let b = value.clone();
    Element::of(Kind::SLIDER)
        .prop(Prop::MIN_VALUE, PValue::Float(min))
        .prop(Prop::MAX_VALUE, PValue::Float(max))
        .prop(Prop::VALUE, PValue::Float(value.get()))
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Some(x) = v.as_f64() {
                b.set(x);
            }
        })
}

/// A stepper bound to a number in `min..=max` by `step`.
pub fn stepper(value: &Binding<f64>, min: f64, max: f64, step: f64) -> Element {
    let b = value.clone();
    Element::of(Kind::STEPPER)
        .prop(Prop::MIN_VALUE, PValue::Float(min))
        .prop(Prop::MAX_VALUE, PValue::Float(max))
        .prop(Prop::STEP, PValue::Float(step))
        .prop(Prop::VALUE, PValue::Float(value.get()))
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Some(x) = v.as_f64() {
                b.set(x);
            }
        })
}

/// A progress bar: `Some(0..=1)` determinate, `None` indeterminate.
pub fn progress(value: Option<f64>) -> Element {
    let e = Element::of(Kind::PROGRESS_BAR);
    match value {
        Some(v) => e.prop(Prop::VALUE, PValue::Float(v)),
        None => e.prop(Prop::INDETERMINATE, PValue::Bool(true)),
    }
}

/// A spinner.
pub fn spinner() -> Element {
    Element::of(Kind::SPINNER)
}

/// A segmented control of text segments bound to the selected index.
pub fn segmented(items: &[&str], selected: &Binding<usize>) -> Element {
    let b = selected.clone();
    Element::of(Kind::SEGMENTED)
        .prop(Prop::ITEMS, list(items))
        .prop(Prop::SELECTED_INDEX, PValue::Int(selected.get() as i64))
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Some(i) = v.as_int().and_then(|i| usize::try_from(i).ok()) {
                b.set(i);
            }
        })
}

fn field(kind: Kind, placeholder: &str, text: &Binding<String>) -> Element {
    let b = text.clone();
    Element::of(kind)
        .prop(Prop::PLACEHOLDER, s(placeholder))
        .prop(Prop::TEXT, s(&text.get()))
        .on(Signal::TEXT_CHANGED, move |v| {
            if let Value::Str(t) = v
                && *t != b.get()
            {
                b.set(t.clone());
            }
        })
}

/// A text field bound to a string.
pub fn text_field(placeholder: &str, text: &Binding<String>) -> Element {
    field(Kind::TEXT_FIELD, placeholder, text)
}

/// A secure (password) field bound to a string.
pub fn secure_field(placeholder: &str, text: &Binding<String>) -> Element {
    field(Kind::SECURE_FIELD, placeholder, text)
}

/// A search field bound to a string.
pub fn search_field(placeholder: &str, text: &Binding<String>) -> Element {
    field(Kind::SEARCH_FIELD, placeholder, text)
}

/// A multi-line text area bound to a string (short texts; documents use `text_view`).
pub fn text_area(placeholder: &str, text: &Binding<String>) -> Element {
    field(Kind::TEXT_AREA, placeholder, text)
}

/// A pop-up button bound to the selected index.
pub fn pop_up(items: &[&str], selected: &Binding<usize>) -> Element {
    let b = selected.clone();
    Element::of(Kind::POP_UP_BUTTON)
        .prop(Prop::ITEMS, list(items))
        .prop(Prop::SELECTED_INDEX, PValue::Int(selected.get() as i64))
        .on(Signal::SELECTION_CHANGED, move |v| {
            if let Some(i) = v.as_int().and_then(|i| usize::try_from(i).ok()) {
                b.set(i);
            }
        })
}

/// A pull-down button: `chosen` gets the index of the item picked.
pub fn pull_down(title: &str, items: &[&str], chosen: impl Fn(usize) + 'static) -> Element {
    Element::of(Kind::PULL_DOWN_BUTTON)
        .prop(Prop::TITLE, s(title))
        .prop(Prop::ITEMS, list(items))
        .on(Signal::SELECTION_CHANGED, move |v| {
            if let Some(i) = v.as_int().and_then(|i| usize::try_from(i).ok()) {
                chosen(i);
            }
        })
}

/// A round icon button.
pub fn icon_button(symbol: &str) -> Element {
    Element::of(Kind::ICON_BUTTON).symbol(symbol)
}

/// A toolbar button.
pub fn toolbar_button(symbol: &str) -> Element {
    Element::of(Kind::TOOLBAR_BUTTON).symbol(symbol)
}

/// A toolbar's glass capsule of icon segments (back and forward, an item group): `items`
/// are (symbol, accessible label); `action` gets the index of the segment pressed.
pub fn toolbar_group(items: &[(&str, &str)], action: impl Fn(usize) + 'static) -> Element {
    let symbols: Vec<&str> = items.iter().map(|i| i.0).collect();
    let labels: Vec<&str> = items.iter().map(|i| i.1).collect();
    Element::of(Kind::TOOLBAR_GROUP)
        .prop(Prop::ITEM_SYMBOLS, list(&symbols))
        .prop(Prop::ITEMS, list(&labels))
        .on(Signal::ACTIVATED, move |v| {
            if let Value::Int(i) = v
                && let Ok(i) = usize::try_from(*i)
            {
                action(i);
            }
        })
}

/// A toolbar's title (kit `Piece/Title`: Headline, 8 in).
pub fn toolbar_title(title: &str) -> Element {
    crate::label(title)
        .text_style("Headline")
        .padding_xy(4.0, 0.0)
}

/// A toolbar's search capsule (kit `Piece/Search`: 200 wide).
pub fn toolbar_search(placeholder: &str, text: &Binding<String>) -> Element {
    search_field(placeholder, text).width(200.0)
}

/// A window's toolbar of `items` (toolbar buttons, groups, a title, a search, `spacer()`s
/// sharing the free width), laid out as the kit's pieces after the window controls. See
/// [`Element::toolbar`].
pub fn toolbar(items: impl crate::IntoChildren) -> Element {
    Element::of(Kind::TOOLBAR).children(items).fill_width()
}

/// A form (kit `Forms/Layout`): a window-coloured panel with a title and its parts (sections,
/// then the footer) 20 apart.
pub fn form(title: Option<&str>, parts: impl crate::IntoChildren) -> Element {
    let e = Element::of(Kind::FORM);
    match title {
        Some(t) => e.prop(Prop::TITLE, s(t)),
        None => e,
    }
    .children(parts)
}

/// A form section (kit `Forms/Section`): a titled group box of rows 12 apart.
pub fn form_section(title: Option<&str>, rows: impl crate::IntoChildren) -> Element {
    let surface = Element::of(Kind::BOX_SURFACE)
        .prop(Prop::SPACING, PValue::Float(12.0))
        .id("Group")
        .children(rows);
    group_box(title, surface)
}

/// A form row (kit `Forms/Row`): `label` right-aligned in the label column, then `control`.
pub fn form_row(label: &str, control: Element) -> Element {
    form_row_with(label, control, None, false)
}

/// A form row with a help (or error) message under the control; `expand` makes the control
/// fill the value column (text fields, labelled switches).
pub fn form_row_with(
    label: &str,
    control: Element,
    message: Option<(&str, bool)>,
    expand: bool,
) -> Element {
    let mut value = Element::of(Kind::FORM_VALUE)
        .prop(Prop::EXPANDED, PValue::Bool(expand))
        .children(control);
    if let Some((text, error)) = message {
        let color = if error {
            "Text/Secondary"
        } else {
            "Text/Tertiary"
        };
        value = value.children(
            crate::label(text)
                .text_style("Caption")
                .color(color)
                .prop(Prop::WRAPS, PValue::Bool(true)),
        );
    }
    Element::of(Kind::FORM_ROW)
        .prop(Prop::TITLE, s(label))
        .children(value)
}

/// A form's footer (kit `Forms/Footer`): its buttons at the end, 8 apart.
pub fn form_footer(buttons: impl crate::IntoChildren) -> Element {
    Element::of(Kind::FORM_FOOTER).children(buttons)
}

/// A help button.
pub fn help_button() -> Element {
    Element::of(Kind::HELP_BUTTON)
}

/// A link.
pub fn link(title: &str) -> Element {
    Element::of(Kind::LINK_BUTTON).prop(Prop::TITLE, s(title))
}

/// A disclosure group: a titled row that expands to show `content`.
pub fn disclosure_group(title: &str, expanded: &Binding<bool>, content: Element) -> Element {
    let b = expanded.clone();
    Element::of(Kind::DISCLOSURE_GROUP)
        .prop(Prop::TITLE, s(title))
        .prop(Prop::EXPANDED, PValue::Bool(expanded.get()))
        .on(Signal::EXPANDED_CHANGED, move |v| {
            if let Some(e) = v.as_bool() {
                b.set(e);
            }
        })
        .children(content)
}

/// A group box around `content`.
pub fn group_box(title: Option<&str>, content: Element) -> Element {
    let e = Element::of(Kind::GROUP_BOX);
    match title {
        Some(t) => e.prop(Prop::TITLE, s(t)),
        None => e,
    }
    .children(content)
}

/// A colour well bound to a colour.
pub fn color_well(color: &Binding<vetro_kit::Color>) -> Element {
    let b = color.clone();
    Element::of(Kind::COLOR_WELL)
        .prop(Prop::COLOR_VALUE, PValue::Color(color.get()))
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Value::Color(c) = v {
                b.set(*c);
            }
        })
}

/// A scroll view around `content`.
pub fn scroll_view(content: Element) -> Element {
    Element::of(Kind::SCROLL_VIEW).children(content)
}
