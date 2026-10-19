//! Text dumps of the runtime for the inspector (`vetro-inspector`) and tests: windows, the
//! view tree with frames and state, a view's layers, the accessibility tree.

use std::fmt::Write as _;

use crate::core::display::InspectedContent;
use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;

impl Ui {
    /// One line per window.
    pub fn dump_windows(&self) -> String {
        let mut s = String::new();
        for w in self.windows() {
            let _ = writeln!(
                s,
                "window {} {:?} \"{}\" {}x{} @{} visible={} active={} focus={:?}",
                w.id.to_raw(),
                w.kind,
                w.title,
                w.size.width,
                w.size.height,
                w.scale,
                w.visible,
                w.active,
                w.focus.map(ViewId::to_raw)
            );
        }
        s
    }

    /// The view tree under `root`, indented, with kinds, identifiers, frames and state.
    pub fn dump_tree(&self, root: ViewId) -> String {
        let mut s = String::new();
        self.dump_node(root, 0, &mut s);
        s
    }

    fn dump_node(&self, id: ViewId, depth: usize, s: &mut String) {
        let Some(n) = self.tree.get(id) else { return };
        let f = n.frame;
        let mut flags = Vec::new();
        for (flag, name) in [
            (NodeFlags::HIDDEN, "hidden"),
            (NodeFlags::DISABLED, "disabled"),
            (NodeFlags::HOVERED, "hovered"),
            (NodeFlags::PRESSED, "pressed"),
            (NodeFlags::FOCUSED, "focused"),
            (NodeFlags::LEAVING, "leaving"),
        ] {
            if n.flags.contains(flag) {
                flags.push(name);
            }
        }
        let _ = writeln!(
            s,
            "{:indent$}{} #{}{} [{} {} {}x{}]{}",
            "",
            n.kind(),
            id.to_raw(),
            n.identifier
                .as_deref()
                .map(|i| format!(" '{i}'"))
                .unwrap_or_default(),
            f.x,
            f.y,
            f.width,
            f.height,
            if flags.is_empty() {
                String::new()
            } else {
                format!(" ({})", flags.join(", "))
            },
            indent = depth * 2
        );
        for c in &n.children {
            self.dump_node(*c, depth + 1, s);
        }
    }

    /// The named layers painted by the subtree at `root`.
    pub fn dump_layers(&self, root: ViewId) -> String {
        let mut s = String::new();
        for l in self.inspect_layers(root) {
            let f = l.frame;
            let what = match &l.content {
                InspectedContent::Shape { style, radii, .. } => format!(
                    "shape fills={} borders={} shadows={} blurs={} radii={:?} opacity={}",
                    style.fills.len(),
                    style.borders.len(),
                    style.shadows.len(),
                    style.blurs.len(),
                    radii,
                    style.opacity
                ),
                InspectedContent::Text {
                    text, token, color, ..
                } => {
                    format!("text {text:?} {token:?} {}", color.to_hex())
                }
                InspectedContent::Glyph {
                    symbol,
                    size,
                    color,
                } => {
                    format!("glyph {symbol} {size} {}", color.to_hex())
                }
                InspectedContent::Other => "other".to_string(),
            };
            let _ = writeln!(
                s,
                "{} [{} {} {}x{}] {what}",
                l.path, f.x, f.y, f.width, f.height
            );
        }
        s
    }

    /// The accessibility tree under `root`.
    pub fn dump_accessibility(&self, root: ViewId) -> String {
        let mut s = String::new();
        for n in self.accessibility_tree(root) {
            let _ = writeln!(
                s,
                "#{} {:?} label={:?} value={:?} actions={:?}{}{}",
                n.id.to_raw(),
                n.info.role,
                n.info.label,
                n.info.value,
                n.info.actions,
                if n.focused { " focused" } else { "" },
                if n.disabled { " disabled" } else { "" }
            );
        }
        s
    }

    /// Answers an inspector command: `windows`, `tree [view]`, `layers [view]`, `a11y
    /// [view]` (a view id or identifier; default: every window's root).
    pub fn inspect_command(&self, line: &str) -> String {
        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or_default();
        let arg = parts.next();
        let roots: Vec<ViewId> = match arg {
            Some(a) => {
                let by_id = a.parse::<u64>().ok().and_then(ViewId::from_raw);
                match by_id.filter(|v| self.tree.contains(*v)) {
                    Some(v) => vec![v],
                    None => self
                        .windows()
                        .filter_map(|w| self.find(w.root, a))
                        .collect(),
                }
            }
            None => self.windows().map(|w| w.root).collect(),
        };
        match cmd {
            "windows" => self.dump_windows(),
            "tree" => roots.iter().map(|r| self.dump_tree(*r)).collect(),
            "layers" => roots.iter().map(|r| self.dump_layers(*r)).collect(),
            "a11y" => roots.iter().map(|r| self.dump_accessibility(*r)).collect(),
            _ => "commands: windows, tree [view], layers [view], a11y [view]\n".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::amalfi::button::PushButton;
    use crate::core::Ui;
    use crate::core::value::{Prop, Value};
    use crate::core::window::WindowKind;
    use crate::geom::Size;

    #[test]
    fn dumps() {
        let mut ui = Ui::new_headless();
        let w = ui.create_window(WindowKind::Normal, Size::new(200.0, 100.0));
        let b = ui.create(PushButton::new("OK"));
        ui.set(b, Prop::Identifier, Value::from("ok")).unwrap();
        ui.set_content(w, b);
        ui.prepare_frame(w);
        assert!(ui.inspect_command("windows").contains("window 1"));
        assert!(ui.inspect_command("tree").contains("PushButton"));
        assert!(ui.inspect_command("layers ok").contains("Label"));
        assert!(ui.inspect_command("a11y").contains("Button"));
        assert!(ui.inspect_command("help").contains("commands"));
    }
}
