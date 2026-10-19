//! End to end in Ossido headless (silica-desktop's compositor): the gallery maps a window,
//! renders, reacts to a click, and quits from its Quit button. Skipped when Ossido is not
//! built (`VETRO_OSSIDO`, default `~/silica/silica/target/release/ossido`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn sibling(rel: &str) -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("silica/silica").join(rel)
}

struct Kill(Child);

impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn gallery_in_ossido_headless() {
    let ossido = std::env::var_os("VETRO_OSSIDO")
        .map(PathBuf::from)
        .unwrap_or_else(|| sibling("target/release/ossido"));
    let cli = sibling("target/release/silica-cli");
    if !ossido.exists() || !cli.exists() {
        eprintln!("skipped: no Ossido at {}", ossido.display());
        return;
    }
    let dir = PathBuf::from(format!("/dev/shm/vetro-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::PermissionsExt::set_mode(
        &mut std::fs::metadata(&dir).unwrap().permissions(),
        0o700,
    );
    let mut child = Command::new(&ossido)
        .args([
            "--backend",
            "headless",
            "--size",
            "1024x700",
            "--animation-scale",
            "0",
        ])
        .args(["--glass", "full", "--control", "0"])
        .env("XDG_RUNTIME_DIR", &dir)
        .env("XDG_CONFIG_HOME", dir.join("config"))
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            format!("unix:path={}", dir.join("no-bus").display()),
        )
        .env("RUST_LOG", "error")
        .env_remove("WAYLAND_DISPLAY")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut wayland = None;
    let mut control = None;
    for line in BufReader::new(child.stdout.take().unwrap()).lines() {
        let line = line.unwrap();
        if let Some(n) = line.strip_prefix("WAYLAND_DISPLAY=") {
            wayland = Some(n.to_string());
        }
        if let Some(a) = line.strip_prefix("OSSIDO_CONTROL=") {
            control = Some(a.to_string());
            break;
        }
    }
    let _ossido = Kill(child);
    let (wayland, control) = (wayland.unwrap(), control.unwrap());
    let gallery = Command::new(env!("CARGO_BIN_EXE_vetro-gallery"))
        .env("XDG_RUNTIME_DIR", &dir)
        .env("WAYLAND_DISPLAY", &wayland)
        .env("VETRO_RENDERER", "software")
        .env("VETRO_DETERMINISTIC", "1")
        .env("VETRO_MULTI_INSTANCE", "1")
        .env("VETRO_STARTUP_TIMING", "1")
        .env("XDG_STATE_HOME", dir.join("state"))
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            format!("unix:path={}", dir.join("no-bus").display()),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut gallery = Kill(gallery);
    // The cold start (#254: 150 ms in release on the reference machine): reported here, and
    // held to a loose bound in this (debug) build.
    let stderr = gallery.0.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Some(ms) = line
                .strip_prefix("vetro: first frame ")
                .and_then(|r| r.split(' ').next())
                .and_then(|v| v.parse::<f64>().ok())
            {
                let _ = tx.send(ms);
            }
        }
    });
    let ctl = |args: &[&str]| -> String {
        let out = Command::new(&cli)
            .arg("--control")
            .arg(&control)
            .args(args)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    // The window appears.
    let start = Instant::now();
    loop {
        if ctl(&["window.list"]).contains("Vetro Gallery") {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "no window");
        std::thread::sleep(Duration::from_millis(100));
    }
    let first = rx.recv_timeout(Duration::from_secs(20)).unwrap_or(f64::NAN);
    eprintln!("cold start: first frame {first:.1} ms");
    assert!(first < 3000.0, "cold start {first} ms");
    std::thread::sleep(Duration::from_millis(500));
    let shot = dir.join("shot.png");
    ctl(&["screenshot", shot.to_str().unwrap()]);
    assert!(std::fs::metadata(&shot).map(|m| m.len()).unwrap_or(0) > 10_000);
    // The window is at Ossido's cascade origin; its Quit button is the last row. Find the
    // window frame from the list and click the button inside it.
    let list = ctl(&["window.list"]);
    let num = |key: &str| -> f32 {
        list.split(&format!("\"{key}\":"))
            .nth(1)
            .and_then(|r| r.split([',', '}']).next())
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0.0)
    };
    let (x, y) = (num("x"), num("y"));
    // `window.list` gives the content's origin; Quit is the first button (padding 24).
    let click = |dx: f32, dy: f32| {
        ctl(&[
            "input.click",
            &format!("x={}", x + dx),
            &format!("y={}", y + dy),
        ]);
    };
    click(52.0, 90.0);
    let start = Instant::now();
    loop {
        if gallery.0.try_wait().unwrap().is_some() {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "Quit did not quit (window at {x},{y})"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
