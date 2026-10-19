//! The view kinds `vetro_view_new` makes (`VETRO_KIND_*`).

use vetro_sys::*;

use crate::amalfi::button::PushButton;
use crate::amalfi::buttons::{HelpButton, IconButton, LinkButton, MenuButton, ToolbarButton};
use crate::amalfi::collections::{GridView, ListView, TableHeader, TableView};
use crate::amalfi::disclosure::{Disclosure, DisclosureGroup};
use crate::amalfi::fields::{FieldKind, TextField};
use crate::amalfi::forms::{FormLayout, FormRow};
use crate::amalfi::group_box::GroupBox;
use crate::amalfi::label::{Label, SymbolImage};
use crate::amalfi::progress::{CircularProgress, ProgressBar, Spinner};
use crate::amalfi::scrollbars::ScrollView;
use crate::amalfi::segments::SegmentedControl;
use crate::amalfi::sidebars;
use crate::amalfi::sliders::{Dial, Slider};
use crate::amalfi::statusbar::StatusBar;
use crate::amalfi::steppers::Stepper;
use crate::amalfi::switch::Switch;
use crate::amalfi::titlebars::{Tab, TabBar, TitleBar};
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
        VETRO_KIND_LIST => Box::new(ListView::new(0, 28.0, Box::new(|_, _| None))),
        VETRO_KIND_TABLE_HEADER => Box::new(TableHeader::new(Vec::new())),
        VETRO_KIND_SIDEBAR_ROW => Box::new(sidebars::Row::new("folder", "")),
        VETRO_KIND_SIDEBAR_HEADER => Box::new(sidebars::Header::new("")),
        VETRO_KIND_SIDEBAR_GROUP => Box::new(sidebars::Group { open: true }),
        VETRO_KIND_SIDEBAR_PANEL => Box::new(sidebars::Panel::default()),
        VETRO_KIND_FORM => Box::new(FormLayout::new(None)),
        VETRO_KIND_FORM_ROW => Box::new(FormRow::new("")),
        VETRO_KIND_TITLE_BAR => Box::new(TitleBar),
        VETRO_KIND_TAB_BAR => Box::new(TabBar),
        VETRO_KIND_TAB => Box::new(Tab::new("")),
        VETRO_KIND_STATUS_BAR => Box::new(StatusBar),
        VETRO_KIND_TEXT => Box::new(crate::amalfi::text::Text::new("")),
        VETRO_KIND_TABLE => Box::new(TableView::new(Vec::new())),
        VETRO_KIND_OUTLINE => Box::new(TableView::outline(Vec::new())),
        VETRO_KIND_GRID_VIEW => Box::new(GridView::new(0, Box::new(|_, _| None))),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::a11y::{A11y, Role};

    #[test]
    fn every_control_describes_itself() {
        let containers = [
            VETRO_KIND_VIEW,
            VETRO_KIND_ROW,
            VETRO_KIND_COLUMN,
            VETRO_KIND_OVERLAY,
            VETRO_KIND_GRID,
            VETRO_KIND_SPACER,
            VETRO_KIND_SYMBOL,
            VETRO_KIND_SCROLL_VIEW,
            VETRO_KIND_GROUP_BOX,
            VETRO_KIND_SIDEBAR_GROUP,
            VETRO_KIND_FORM,
            VETRO_KIND_FORM_ROW,
        ];
        let mut missing = Vec::new();
        for (name, kind) in vetro_sys::KINDS {
            let Some(w) = make(*kind) else {
                missing.push(format!("{name}: no widget"));
                continue;
            };
            let mut a = A11y::default();
            w.accessibility(&mut a);
            if a.role == Role::Group && !containers.contains(kind) {
                missing.push(format!("{name}: no role"));
            }
        }
        assert!(missing.is_empty(), "{missing:?}");
    }
}
