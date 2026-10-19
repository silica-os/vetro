//! Text: fonts, shaping and layout (SkParagraph), attributed strings, the editing engine.

pub mod edit;
pub mod fonts;
pub mod layout;
pub mod view;

pub use layout::{Align, AttributedString, ParagraphOptions, TextLayout, TextStyle};
