//! Building for and running in Silica's dev VM (started with `cargo xtask os start` in
//! silica-desktop). Every VM interaction holds `/dev/shm/silica-vm.lock`. The VM is never
//! stopped or restarted from here, and the Silica session is not redeployed.
//!
//! ```text
//! cargo xtask vm run [EXAMPLE] [-- ARGS]   build EXAMPLE (default vetro-gallery) in release,
//!                                          copy it to the guest and start it in the session
//! cargo xtask vm stop [EXAMPLE]            stop what `run` started (its own PID only)
//! cargo xtask vm shot PATH                 screenshot of the guest's screen
//! cargo xtask vm log [EXAMPLE]             the program's output in the guest
//! cargo xtask vm cmd ARGS                  a silica-cli command on Ossido's control channel
//! ```
//!
//! The host's glibc must not be newer than the guest's (Fedora 44: 2.43).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::{Result, root, sibling};

/// The guest's SSH port on the host (silica-desktop xtask `os`).
const SSH_PORT: &str = "7998";
/// Ossido's control channel on the host (SSH tunnel).
const CONTROL: &str = "127.0.0.1:7993";
/// Where programs go in the guest.
const GUEST_DIR: &str = "/var/lib/vetro-dev";
/// The session's user.
const GUEST_USER: &str = "silica";

fn work_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SILICA_OS_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("build/silica-os-dev")
}

fn ssh_opts() -> Vec<String> {
    let key = work_dir().join("id_ed25519");
    [
        "-o",
        "BatchMode=yes",
        "-o",
        "StrictHostKeyChecking=no",
        "-o",
        "UserKnownHostsFile=/dev/null",
        "-o",
        "LogLevel=ERROR",
        "-o",
        "ConnectTimeout=5",
        "-i",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .chain([key.to_string_lossy().into_owned()])
    .collect()
}

/// A command run under the VM lock.
fn locked(program: &str) -> Command {
    let mut c = Command::new("flock");
    c.arg("/dev/shm/silica-vm.lock").arg(program);
    c
}

fn ssh(script: &str) -> Result<String> {
    let out = locked("ssh")
        .args(ssh_opts())
        .args(["-p", SSH_PORT, "root@127.0.0.1", script])
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("ssh: {e}"))?;
    if !out.status.success() {
        return Err(format!("ssh: `{script}` failed ({})", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn scp(local: &Path, remote: &str) -> Result<()> {
    let mut opts = ssh_opts();
    opts.extend(["-P".to_string(), SSH_PORT.to_string()]);
    let status = locked("scp")
        .args(opts)
        .arg(local)
        .arg(format!("root@127.0.0.1:{remote}"))
        .status()
        .map_err(|e| format!("scp: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("scp {} failed", local.display()))
    }
}

fn build(example: &str) -> Result<PathBuf> {
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .current_dir(root())
        .args(["build", "--release", "-p", example])
        .status()
        .map_err(|e| format!("cargo: {e}"))?;
    if !status.success() {
        return Err("build failed".into());
    }
    Ok(root().join("target/release").join(example))
}

pub fn vm(args: &[String]) -> Result<()> {
    let rest = args.get(1..).unwrap_or_default();
    let example = rest
        .first()
        .filter(|a| !a.starts_with('-'))
        .cloned()
        .unwrap_or_else(|| "vetro-gallery".to_string());
    match args.first().map(String::as_str) {
        Some("run") => {
            let extra: Vec<String> = rest
                .iter()
                .skip_while(|a| *a != "--")
                .skip(1)
                .cloned()
                .collect();
            let bin = build(&example)?;
            stop(&example)?;
            ssh(&format!("mkdir -p {GUEST_DIR} && chmod 755 {GUEST_DIR}"))?;
            scp(&bin, &format!("{GUEST_DIR}/{example}"))?;
            let script = format!(
                "chmod 755 {GUEST_DIR}/{example}; uid=$(id -u {GUEST_USER}); \
                 sock=$(ls /run/user/$uid | grep -E '^wayland-[0-9]+$' | head -1); \
                 runuser -u {GUEST_USER} -- env XDG_RUNTIME_DIR=/run/user/$uid WAYLAND_DISPLAY=$sock \
                 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$uid/bus RUST_LOG=info {} \
                 setsid {GUEST_DIR}/{example} {} > {GUEST_DIR}/{example}.log 2>&1 < /dev/null & \
                 echo $! > {GUEST_DIR}/{example}.pid; sleep 1; cat {GUEST_DIR}/{example}.pid",
                std::env::var("VETRO_VM_ENV").unwrap_or_default(),
                extra.join(" ")
            );
            let pid = ssh(&script)?;
            println!("{example} started in the VM (pid {})", pid.trim());
            Ok(())
        }
        Some("stop") => stop(&example),
        Some("log") => {
            print!(
                "{}",
                ssh(&format!("cat {GUEST_DIR}/{example}.log 2>/dev/null"))?
            );
            Ok(())
        }
        Some("shot") => {
            let path = rest.first().ok_or("usage: cargo xtask vm shot PATH")?;
            // silica-desktop's `xtask os shot` fetches the PNG inline over the control channel
            // (a path would be the guest's).
            let status = locked("cargo")
                .current_dir(sibling("silica"))
                .args(["xtask", "os", "shot", path])
                .status()
                .map_err(|e| format!("cargo xtask os shot: {e}"))?;
            if status.success() {
                Ok(())
            } else {
                Err("screenshot failed".into())
            }
        }
        Some("cmd") => {
            let a: Vec<&str> = rest.iter().map(String::as_str).collect();
            cli(&a)
        }
        _ => Err(
            "usage: cargo xtask vm <run [EXAMPLE] [-- ARGS] | stop | log | shot PATH | cmd …>"
                .into(),
        ),
    }
}

/// Stops the program `run` started: the PID it recorded, and only if that process is
/// still the program.
fn stop(example: &str) -> Result<()> {
    ssh(&format!(
        "f={GUEST_DIR}/{example}.pid; if [ -f $f ]; then p=$(cat $f); \
         if [ -n \"$p\" ] && grep -q {example} /proc/$p/cmdline 2>/dev/null; then kill $p; fi; rm -f $f; fi; true"
    ))
    .map(|_| ())
}

fn cli(args: &[&str]) -> Result<()> {
    let bin = sibling("silica").join("target/release/silica-cli");
    let status = locked(&bin.to_string_lossy())
        .args(["--control", CONTROL])
        .args(args)
        .status()
        .map_err(|e| format!("silica-cli: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("silica-cli failed".into())
    }
}
