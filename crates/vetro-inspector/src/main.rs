//! `vetro-inspector [APP|PID] COMMAND [VIEW]`: talks to a Vetro process started with
//! `VETRO_INSPECT=1` (its socket in `$XDG_RUNTIME_DIR/vetro-inspect`). Commands: `windows`,
//! `tree`, `layers`, `a11y`; VIEW is a view id or identifier. Without arguments, lists the
//! processes.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

fn sockets() -> Vec<PathBuf> {
    let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR") else {
        return Vec::new();
    };
    let dir = PathBuf::from(rt).join("vetro-inspect");
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "sock"))
        .collect();
    v.sort();
    v
}

fn ask(path: &PathBuf, line: &str) -> std::io::Result<String> {
    let mut s = UnixStream::connect(path)?;
    s.set_read_timeout(Some(Duration::from_secs(3)))?;
    s.write_all(line.as_bytes())?;
    s.write_all(b"\n")?;
    s.shutdown(std::net::Shutdown::Write)?;
    let mut out = String::new();
    s.read_to_string(&mut out)?;
    Ok(out)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let all = sockets();
    let Some(target) = args.first() else {
        for p in &all {
            println!(
                "{}",
                p.file_stem()
                    .map(|s| s.to_string_lossy())
                    .unwrap_or_default()
            );
        }
        return;
    };
    let Some(path) = all.iter().find(|p| {
        p.file_name()
            .is_some_and(|n| n.to_string_lossy().contains(target.as_str()))
    }) else {
        eprintln!("vetro-inspector: no process matches {target}");
        std::process::exit(1);
    };
    let line = args.get(1..).unwrap_or_default().join(" ");
    match ask(path, &line) {
        Ok(out) => print!("{out}"),
        Err(e) => {
            eprintln!("vetro-inspector: {e}");
            std::process::exit(1);
        }
    }
}
