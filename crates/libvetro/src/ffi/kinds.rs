//! The view kinds `vetro_view_new` makes (`VETRO_KIND_*`).

use vetro_sys::*;

use crate::amalfi::button::PushButton;
use crate::amalfi::label::{Label, SymbolImage};
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
        _ => return None,
    })
}
