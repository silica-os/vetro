//! Single instance through Silica's app registry (`$XDG_RUNTIME_DIR/silica-apps`, the
//! protocol of silica-desktop's `silica_control::apps`, as the Flutter embedder spoke it): a
//! second launch forwards to the running process (reopen, open files) and exits; the running
//! process answers `status`, `reopen`, `quit` and `open [paths]` on its socket.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::Duration;

use silica_control::apps;

use crate::core::app::AppEvent;

/// A request on the socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryRequest {
    /// Is a window shown (`shown`) or none (`hidden`)?
    Status,
    /// Show a window.
    Reopen,
    /// Quit.
    Quit,
    /// Open these paths.
    Open(Vec<String>),
}

/// Parses one request line.
pub fn parse(line: &str) -> Option<RegistryRequest> {
    let line = line.trim();
    if let Some(list) = line.strip_prefix("open ") {
        return parse_string_list(list).map(RegistryRequest::Open);
    }
    match line {
        "status" => Some(RegistryRequest::Status),
        "reopen" => Some(RegistryRequest::Reopen),
        "quit" => Some(RegistryRequest::Quit),
        _ => None,
    }
}

/// A JSON array of strings (the registry's wire format; parsed by hand, nothing else is
/// accepted).
fn parse_string_list(text: &str) -> Option<Vec<String>> {
    let v: Vec<String> = serde_json::from_str(text.trim()).ok()?;
    Some(v)
}

/// The running process's registration: the socket is removed when dropped.
pub struct Registration {
    path: PathBuf,
    parked: PathBuf,
}

impl Drop for Registration {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(&self.parked);
    }
}

/// Forwards to a running instance; true when one took over (this process exits).
pub fn forward(app_id: &str, files: &[String]) -> bool {
    if !apps::valid_app_id(app_id) {
        return false;
    }
    if files.is_empty() {
        apps::reopen(app_id, true)
    } else {
        apps::open(app_id, files)
    }
}

/// Registers this process.
pub fn register(app_id: &str) -> std::io::Result<(Registration, UnixListener)> {
    if !apps::valid_app_id(app_id) {
        return Err(std::io::Error::other(format!(
            "app id not usable: {app_id}"
        )));
    }
    let dir = apps::dir().ok_or_else(|| std::io::Error::other("XDG_RUNTIME_DIR is not set"))?;
    std::fs::create_dir_all(&dir)?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    let parked = apps::parked_path(&dir, app_id);
    let _ = std::fs::remove_file(&parked);
    let path = apps::socket_path(&dir, app_id, std::process::id());
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    listener.set_nonblocking(true)?;
    Ok((Registration { path, parked }, listener))
}

/// Reads one request from a client (short timeouts: a client never blocks the UI).
pub fn read(stream: &UnixStream) -> Option<RegistryRequest> {
    stream.set_nonblocking(false).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(200)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_millis(200)))
        .ok()?;
    let mut line = String::new();
    BufReader::new(std::io::Read::take(stream, apps::MAX_OPEN_LINE as u64))
        .read_line(&mut line)
        .ok()?;
    parse(&line)
}

/// Answers a client.
pub fn answer(mut stream: &UnixStream, text: &str) {
    let _ = stream
        .write_all(text.as_bytes())
        .and_then(|()| stream.write_all(b"\n"));
}

/// The lifecycle event of a request (none for `status`).
pub fn event_of(r: &RegistryRequest) -> Option<AppEvent> {
    match r {
        RegistryRequest::Status => None,
        RegistryRequest::Reopen => Some(AppEvent::Reopen),
        RegistryRequest::Quit => Some(AppEvent::Quit),
        RegistryRequest::Open(l) => Some(AppEvent::Open(l.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests() {
        assert_eq!(parse("status\n"), Some(RegistryRequest::Status));
        assert_eq!(parse(" reopen "), Some(RegistryRequest::Reopen));
        assert_eq!(parse("quit"), Some(RegistryRequest::Quit));
        assert_eq!(parse("rm -rf"), None);
        assert_eq!(
            parse("open [\"/home/a/b c.png\"]"),
            Some(RegistryRequest::Open(vec!["/home/a/b c.png".into()]))
        );
        assert_eq!(parse("open not-json"), None);
    }
}
