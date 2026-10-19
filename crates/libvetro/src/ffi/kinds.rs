//! The view kinds `vetro_view_new` makes (`VETRO_KIND_*`).

use vetro_sys::*;

use crate::amalfi::button::PushButton;
use crate::amalfi::buttons::{HelpButton, IconButton, LinkButton, MenuButton, ToolbarButton};
use crate::amalfi::disclosure::{Disclosure, DisclosureGroup};
use crate::amalfi::fields::{FieldKind, TextField};
use crate::amalfi::group_box::GroupBox;
use crate::amalfi::label::{Label, SymbolImage};
use crate::amalfi::progress::{CircularProgress, ProgressBar, Spinner};
use crate::amalfi::scrollbars::ScrollView;
use crate::amalfi::segments::SegmentedControl;
use crate::amalfi::sliders::{Dial, Slider};
use crate::amalfi::steppers::Stepper;
use crate::amalfi::switch::Switch;
use crate::amalfi::toggles::Toggle;
use crate::amalfi::wells::{ColorWell, ImageWell};
use crate::core::layout::{Alignment, Grid, Overlay, Spacer, Stack};
use crate::core::widget::{Empty, Widget};

/// A new widget of `kind`.
pub fn make(kind: u32) -> Option<Box<dyn Widget>> {
    Some(match kind {
        VETRO_KIND_VIEW => Box::new(Empty),
        VETRO_KIND_ROW => Box::new(Stack::row(8.0)),
        VETRO_KIND_COLUMN => Box::new(Stack::column(8.0)),
        VETRO_KIND_OVERLAY => Box::new(Overlay {
            alignment: Alignment::CENTER,
        }),
        VETRO_KIND_GRID => Box::new(Grid {
            columns: 2,
            column_spacing: 8.0,
            row_spacing: 8.0,
            alignment: Alignment::TOP_LEADING,
        }),
        VETRO_KIND_SPACER => Box::new(Spacer::default()),
        VETRO_KIND_LABEL => Box::new(Label::new("")),
        VETRO_KIND_SYMBOL => Box::new(SymbolImage::new("questionmark-circle")),
        VETRO_KIND_PUSH_BUTTON => Box::new(PushButton::new("")),
        VETRO_KIND_TEXT_VIEW => Box::new(crate::text::view::TextView::new(
            "",
            crate::text::TextStyle::default(),
        )),
        VETRO_KIND_CHECKBOX => Box::new(Toggle::checkbox()),
        VETRO_KIND_RADIO => Box::new(Toggle::radio()),
        VETRO_KIND_SWITCH => Box::new(Switch::new()),
        VETRO_KIND_SLIDER => Box::new(Slider::new()),
        VETRO_KIND_STEPPER => Box::new(Stepper::new()),
        VETRO_KIND_PROGRESS_BAR => Box::new(ProgressBar::new()),
        VETRO_KIND_CIRCULAR_PROGRESS => Box::new(CircularProgress::new(0.0)),
        VETRO_KIND_SPINNER => Box::new(Spinner::new()),
        VETRO_KIND_SEGMENTED => Box::new(SegmentedControl::new(Vec::new())),
        VETRO_KIND_TEXT_FIELD => Box::new(TextField::new(FieldKind::Text)),
        VETRO_KIND_SECURE_FIELD => Box::new(TextField::new(FieldKind::Secure)),
        VETRO_KIND_SEARCH_FIELD => Box::new(TextField::new(FieldKind::Search)),
        VETRO_KIND_TOKEN_FIELD => Box::new(TextField::new(FieldKind::Tokens)),
        VETRO_KIND_TEXT_AREA => Box::new(TextField::new(FieldKind::Area)),
        VETRO_KIND_COMBO_BOX => Box::new(TextField::new(FieldKind::Combo)),
        VETRO_KIND_POP_UP_BUTTON => Box::new(MenuButton::pop_up(&[], 0)),
        VETRO_KIND_PULL_DOWN_BUTTON => Box::new(MenuButton::pull_down("", &[])),
        VETRO_KIND_ICON_BUTTON => Box::new(IconButton::new("questionmark-circle")),
        VETRO_KIND_HELP_BUTTON => Box::new(HelpButton::default()),
        VETRO_KIND_LINK_BUTTON => Box::new(LinkButton::new("")),
        VETRO_KIND_TOOLBAR_BUTTON => Box::new(ToolbarButton::new("questionmark-circle")),
        VETRO_KIND_DISCLOSURE => Box::new(Disclosure::new()),
        VETRO_KIND_DISCLOSURE_GROUP => Box::new(DisclosureGroup::new("")),
        VETRO_KIND_GROUP_BOX => Box::new(GroupBox::new(None)),
        VETRO_KIND_COLOR_WELL => {
            Box::new(ColorWell::new(crate::color::Color::rgb(0x0e, 0x77, 0xe6)))
        }
        VETRO_KIND_IMAGE_WELL => Box::new(ImageWell::new()),
        VETRO_KIND_SCROLL_VIEW => Box::new(ScrollView::new()),
        VETRO_KIND_DIAL => Box::new(Dial::new()),
        _ => return None,
    })
}
