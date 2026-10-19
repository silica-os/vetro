//! The object model (the imperative layer, AppKit-like): views in a tree, proposal-based
//! layout, events with a responder chain, focus and the key view loop, windows,
//! accessibility, and the frame.

pub mod a11y;
pub mod display;
pub mod env;
pub mod event;
pub mod layout;
pub mod tree;
pub mod ui;
pub mod value;
pub mod widget;
pub mod window;

pub use display::{DisplayItem, DisplayList, InspectedContent, InspectedLayer, Recorder};
pub use env::Appearance;
pub use event::{Button, Event, Handled, Key, KeyEvent, Mods, Phase};
pub use layout::{Alignment, Axis, Grid, HAlign, Overlay, Proposal, Spacer, Stack, VAlign};
pub use tree::{NodeFlags, ViewId};
pub use ui::{Cursor, Error, Request, Ui};
pub use value::{Prop, Signal, Value};
pub use widget::{Empty, EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
pub use window::{Window, WindowId, WindowKind};
