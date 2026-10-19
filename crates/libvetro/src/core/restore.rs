//! Window state restoration: a window with an autosave name keeps its size, and the
//! state of its identified views (column widths, sort, expanded nodes, split positions,
//! selected tabs, collapsed panes, scroll offsets) in a small text file under
//! `$XDG_STATE_HOME/<app id>/windows/<name>.state`, written when the window closes or
//! the app quits and read back when a window with the same name opens.
//!
//! The format is one `key=value` line per entry: `window.size`, then
//! `<view identifier>.<property name>`; values are typed (`i:` integer, `f:` number,
//! `b:` boolean, `l:` integers, `p:` point, `s:` size, `t:` text, `n:` none). Unknown
//! lines are skipped, so the file survives version changes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::core::value::{Prop, Value};
use crate::core::window::WindowId;
use crate::geom::{Point, Size};

/// The view properties a window restores (when the view has an identifier and reports
/// the property).
pub const RESTORED: &[Prop] = &[
    Prop::ColumnWidths,
    Prop::SortDescriptor,
    Prop::ExpandedItems,
    Prop::SplitPosition,
    Prop::SelectedIndex,
    Prop::Collapsed,
    Prop::ScrollOffset,
];

/// Saved state: the window size and each view's properties.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SavedState {
    /// The window's size.
    pub size: Option<Size>,
    /// Identifier → properties.
    pub views: HashMap<String, Vec<(Prop, Value)>>,
}

fn encode(v: &Value) -> Option<String> {
    Some(match v {
        Value::None => "n:".to_string(),
        Value::Bool(b) => format!("b:{}", u8::from(*b)),
        Value::Int(i) => format!("i:{i}"),
        Value::Float(f) if f.is_finite() => format!("f:{f}"),
        Value::IntList(l) => format!(
            "l:{}",
            l.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
        ),
        Value::Point(p) => format!("p:{},{}", p.x, p.y),
        Value::Size(s) => format!("s:{},{}", s.width, s.height),
        Value::Str(s) => format!("t:{}", s.replace('\\', "\\\\").replace('\n', "\\n")),
        _ => return None,
    })
}

fn pair(s: &str) -> Option<(f32, f32)> {
    let (a, b) = s.split_once(',')?;
    let (a, b) = (a.trim().parse::<f32>().ok()?, b.trim().parse::<f32>().ok()?);
    (a.is_finite() && b.is_finite()).then_some((a, b))
}

fn decode(s: &str) -> Option<Value> {
    let (tag, body) = s.split_once(':')?;
    Some(match tag {
        "n" => Value::None,
        "b" => Value::Bool(body == "1"),
        "i" => Value::Int(body.parse().ok()?),
        "f" => Value::Float(body.parse().ok().filter(|f: &f64| f.is_finite())?),
        "l" if body.is_empty() => Value::IntList(Vec::new()),
        "l" => Value::IntList(
            body.split(',')
                .map(|x| x.trim().parse().ok())
                .collect::<Option<Vec<i64>>>()?,
        ),
        "p" => {
            let (x, y) = pair(body)?;
            Value::Point(Point::new(x, y))
        }
        "s" => {
            let (w, h) = pair(body)?;
            Value::Size(Size::new(w, h))
        }
        "t" => {
            let mut out = String::new();
            let mut it = body.chars();
            while let Some(c) = it.next() {
                if c == '\\' {
                    match it.next() {
                        Some('n') => out.push('\n'),
                        Some(o) => out.push(o),
                        None => {}
                    }
                } else {
                    out.push(c);
                }
            }
            Value::Str(out)
        }
        _ => return None,
    })
}

impl SavedState {
    /// The file's text.
    pub fn to_text(&self) -> String {
        let mut lines = Vec::new();
        if let Some(s) = self.size {
            lines.push(format!("window.size=s:{},{}", s.width, s.height));
        }
        let mut ids: Vec<&String> = self.views.keys().collect();
        ids.sort();
        for id in ids {
            for (p, v) in self.views.get(id).into_iter().flatten() {
                if let Some(e) = encode(v) {
                    lines.push(format!("{id}.{}={e}", p.name()));
                }
            }
        }
        lines.push(String::new());
        lines.join("\n")
    }

    /// Reads a file's text (unknown lines are skipped).
    pub fn from_text(text: &str) -> SavedState {
        let mut st = SavedState::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let Some(v) = decode(value) else { continue };
            if key == "window.size" {
                if let Value::Size(s) = v
                    && s.width > 0.0
                    && s.height > 0.0
                {
                    st.size = Some(s);
                }
                continue;
            }
            let Some((id, prop)) = key.rsplit_once('.') else {
                continue;
            };
            let Some(p) = RESTORED.iter().copied().find(|p| p.name() == prop) else {
                continue;
            };
            st.views.entry(id.to_string()).or_default().push((p, v));
        }
        st
    }
}

/// The directory of an app's window states.
pub fn state_dir(app_id: &str) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    let safe: String = app_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect();
    if safe.is_empty() {
        return None;
    }
    Some(base.join(safe).join("windows"))
}

fn file_name(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{safe}.state")
}

/// Reads a window's saved state.
pub fn load(dir: &Path, name: &str) -> Option<SavedState> {
    let text = std::fs::read_to_string(dir.join(file_name(name))).ok()?;
    Some(SavedState::from_text(&text))
}

/// Writes a window's state (atomically: a temporary file renamed over the old one).
pub fn store(dir: &Path, name: &str, state: &SavedState) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(file_name(name));
    let tmp = dir.join(format!(".{}.tmp", file_name(name)));
    std::fs::write(&tmp, state.to_text())?;
    std::fs::rename(&tmp, &path)
}

impl Ui {
    /// Gives `window` an autosave name: its size and its identified views' state are
    /// restored now (views made later are restored when they appear) and saved when it
    /// closes or the app quits. Without a state directory (no app id, headless without
    /// [`Ui::set_state_dir`]) nothing is kept.
    pub fn set_autosave(&mut self, window: WindowId, name: &str) {
        let dir = self.state_dir.clone();
        let saved = dir.as_deref().and_then(|d| load(d, name));
        let size = saved.as_ref().and_then(|s| s.size);
        if let Some(w) = self.window_mut(window) {
            w.autosave = Some(name.to_string());
            w.restore = saved.map(|s| s.views).unwrap_or_default();
        }
        if let Some(s) = size {
            let scale = self.window(window).map_or(1.0, |w| w.scale);
            self.resize_window(window, s, scale);
            let now = self.window(window).map(|w| w.size).unwrap_or(s);
            self.request(crate::core::ui::Request::Resize(window, now));
        }
    }

    /// Where window states are kept (the platform sets it from the app id).
    pub fn set_state_dir(&mut self, dir: Option<PathBuf>) {
        self.state_dir = dir;
    }

    /// Applies the pending restored state to the views of `window` that have appeared.
    pub(crate) fn apply_restore(&mut self, window: WindowId) {
        let Some((root, pending)) = self
            .window(window)
            .filter(|w| !w.restore.is_empty())
            .map(|w| (w.root, w.restore.keys().cloned().collect::<Vec<_>>()))
        else {
            return;
        };
        let mut found: Vec<(String, ViewId)> = Vec::new();
        for v in self.tree.descendants(root) {
            if let Some(id) = self.tree.get(v).and_then(|n| n.identifier.clone())
                && pending.contains(&id)
            {
                found.push((id, v));
            }
        }
        for (id, v) in found {
            let props = self
                .window_mut(window)
                .and_then(|w| w.restore.remove(&id))
                .unwrap_or_default();
            for (p, value) in props {
                let _ = self.set(v, p, value);
            }
        }
    }

    /// The current state of `window` (its size and its identified views' properties).
    pub fn window_state(&self, window: WindowId) -> Option<SavedState> {
        let w = self.window(window)?;
        let mut st = SavedState {
            size: Some(w.size),
            views: HashMap::new(),
        };
        for v in self.tree.descendants(w.root) {
            let Some(id) = self.tree.get(v).and_then(|n| n.identifier.clone()) else {
                continue;
            };
            let Some(widget) = self.tree.widget(v) else {
                continue;
            };
            let props: Vec<(Prop, Value)> = RESTORED
                .iter()
                .filter_map(|p| widget.prop(*p).map(|val| (*p, val)))
                .collect();
            if !props.is_empty() {
                st.views.insert(id, props);
            }
        }
        // Views not made this time keep what was saved for them.
        for (id, props) in &w.restore {
            st.views.entry(id.clone()).or_insert_with(|| props.clone());
        }
        Some(st)
    }

    /// Saves `window`'s state when it has an autosave name.
    pub fn save_window_state(&self, window: WindowId) {
        let (Some(dir), Some(name)) = (
            self.state_dir.as_deref(),
            self.window(window).and_then(|w| w.autosave.clone()),
        ) else {
            return;
        };
        if let Some(st) = self.window_state(window)
            && let Err(e) = store(dir, &name, &st)
        {
            tracing::warn!("saving the state of window {name}: {e}");
        }
    }

    /// Saves every autosaved window (the app quits).
    pub fn save_window_states(&self) {
        let ids: Vec<WindowId> = self.windows().map(|w| w.id).collect();
        for w in ids {
            self.save_window_state(w);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amalfi::collections::{TableColumn, TableView};
    use crate::core::window::WindowKind;

    #[test]
    fn values_round_trip() {
        let mut st = SavedState {
            size: Some(Size::new(800.0, 600.0)),
            ..SavedState::default()
        };
        st.views.insert(
            "files".into(),
            vec![
                (Prop::ColumnWidths, Value::IntList(vec![200, 80])),
                (Prop::SortDescriptor, Value::Int(-2)),
                (Prop::ScrollOffset, Value::Point(Point::new(0.0, 120.5))),
            ],
        );
        st.views
            .insert("tabs".into(), vec![(Prop::SelectedIndex, Value::Int(2))]);
        let text = st.to_text();
        assert_eq!(SavedState::from_text(&text), st);
        // Garbage is skipped.
        let more = format!("{text}junk\nx.Title=t:no\nfiles.Columns=i:3\nwindow.size=s:-1,2\n");
        assert_eq!(SavedState::from_text(&more), st);
        assert_eq!(decode("t:a\\nb\\\\c"), Some(Value::Str("a\nb\\c".into())));
    }

    #[test]
    fn a_window_restores_its_size_and_views() {
        let dir = std::env::temp_dir().join(format!("vetro-restore-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let make = |ui: &mut Ui| {
            let w = ui.create_window(WindowKind::Normal, Size::new(500.0, 400.0));
            let mut t = TableView::new(vec![
                TableColumn::new("Name", 200.0),
                TableColumn::new("Size", 80.0),
            ]);
            t.set_cells(10, 28.0, Box::new(|_, _, _| None));
            let v = ui.create(t);
            let _ = ui.set(v, Prop::Identifier, Value::from("files"));
            ui.set_content(w, v);
            ui.show_window(w);
            (w, v)
        };
        let mut ui = Ui::new_headless();
        ui.set_state_dir(Some(dir.clone()));
        let (w, v) = make(&mut ui);
        ui.set_autosave(w, "main");
        ui.prepare_frame(w);
        ui.resize_window(w, Size::new(640.0, 480.0), 1.0);
        ui.set(v, Prop::ColumnWidths, Value::IntList(vec![260, 90]))
            .unwrap();
        ui.set(v, Prop::SortDescriptor, Value::Int(1)).unwrap();
        ui.close_window(w);
        // A new run.
        let mut ui = Ui::new_headless();
        ui.set_state_dir(Some(dir.clone()));
        let (w, v) = make(&mut ui);
        ui.set_autosave(w, "main");
        ui.prepare_frame(w);
        assert_eq!(ui.window(w).map(|w| w.size), Some(Size::new(640.0, 480.0)));
        assert_eq!(
            ui.get(v, Prop::ColumnWidths),
            Some(Value::IntList(vec![260, 90]))
        );
        assert_eq!(ui.get(v, Prop::SortDescriptor), Some(Value::Int(1)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
