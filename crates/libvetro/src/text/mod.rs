//! Text: fonts, shaping and layout (SkParagraph), attributed strings, the editing engine.

pub mod fonts;
pub mod layout;

pub use layout::{Align, AttributedString, ParagraphOptions, TextLayout, TextStyle};
