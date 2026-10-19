//! For the desktop shell (silica-desktop's shell process): the outputs, the session lock and
//! the `silica_shell_manager_v1` channel to Ossido (commands, states shared between shell
//! components, window events, app menus). The core keeps the state and queues requests; the
//! platform talks to the compositor. Apps never need this.

use crate::core::app::AppEvent;
use crate::core::ui::{Request, Ui};
use crate::geom::{Point, Size};

/// A screen.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputInfo {
    /// The connector name (`Virtual-1`, `eDP-1` …).
    pub name: String,
    /// Position in the desktop (logical points).
    pub position: Point,
    /// Size in logical points.
    pub size: Size,
    /// Device pixels per point.
    pub scale: f32,
}

/// An event of Ossido's shell channel.
#[derive(Debug, Clone, PartialEq)]
pub enum ShellEvent {
    /// The window with the keyboard (its foreign-toplevel identifier; empty: none).
    Focused(String),
    /// A catalog command for the shell (`faro.toggle` …).
    Command(String),
    /// A window was minimized (true) or restored.
    Minimized {
        /// Its identifier.
        identifier: String,
        /// Minimized.
        minimized: bool,
    },
    /// A state announced by a shell component, or by Ossido (`session.locked` …).
    Broadcast {
        /// Topic.
        topic: String,
        /// Value.
        value: String,
    },
    /// The window menu was asked for (a right click on a title bar) at a point.
    WindowMenu {
        /// The window.
        identifier: String,
        /// Where (desktop coordinates).
        at: Point,
    },
    /// An app's menu bar model (silica-menu-v1 JSON).
    AppMenu {
        /// The app.
        app_id: String,
        /// Serial to answer with ([`ShellRequest::ActivateMenuItem`]).
        serial: u32,
        /// The model.
        json: String,
    },
}

/// A request on Ossido's shell channel.
#[derive(Debug, Clone, PartialEq)]
pub enum ShellRequest {
    /// Brings a window to the front.
    Activate(String),
    /// Brings an app to the front.
    ActivateApp(String),
    /// Runs a catalog command.
    Execute(String),
    /// Runs a catalog command with JSON parameters.
    ExecuteWith(String, String),
    /// Starts an app (a command line).
    Launch(String),
    /// Announces a state to the shell components.
    Broadcast(String, String),
    /// Chooses an item of an app's menu.
    ActivateMenuItem(u32, u32),
}

impl Ui {
    /// The screens.
    pub fn outputs(&self) -> &[OutputInfo] {
        &self.outputs
    }

    /// The platform's screens changed.
    pub fn set_outputs(&mut self, outputs: Vec<OutputInfo>) {
        if self.outputs == outputs {
            return;
        }
        self.outputs = outputs;
        self.app_event(AppEvent::OutputsChanged);
    }

    /// Joins Ossido's shell channel: its events come as [`AppEvent::Shell`].
    pub fn connect_shell(&mut self) {
        self.request(Request::ConnectShell);
    }

    /// Sends a request on the shell channel.
    pub fn shell(&mut self, request: ShellRequest) {
        self.request(Request::Shell(request));
    }

    /// Ends the session lock (after the password was checked): the lock surfaces go.
    pub fn unlock_session(&mut self) {
        let locks: Vec<_> = self
            .windows()
            .filter(|w| matches!(w.kind, crate::core::window::WindowKind::Lock { .. }))
            .map(|w| w.id)
            .collect();
        self.request(Request::Unlock);
        for w in locks {
            self.close_window(w);
        }
    }

    /// An event of the shell channel arrived.
    pub fn shell_event(&mut self, e: ShellEvent) {
        self.app_event(AppEvent::Shell(e));
    }
}
