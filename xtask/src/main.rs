//! `cargo xtask`: Vetro's development tasks.
//!
//! ```text
//! cargo xtask sync-kit [--design DIR] [PREFIX ...]   copy kit masters into the fixtures
//! cargo xtask check-sources [--desktop DIR]          compare data/ with silica-desktop
//! cargo xtask header                                 regenerate include/vetro.h
//! cargo xtask package [--debug]                      libvetro.so.1 with versioned symbols
//! cargo xtask abi-check                              compare the exported ABI with abi/
//! cargo xtask vm <build|run EXAMPLE|shot PATH>       build for and run in the dev VM
//! ```

// serde_json's `value["key"]` returns Null for a missing key: it never panics.
#![allow(clippy::indexing_slicing)]

mod abi;
mod kit;
mod vm;

use std::path::{Path, PathBuf};

/// A task's result.
pub type Result<T> = std::result::Result<T, String>;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or_default();
    let result = match args.first().map(String::as_str) {
        Some("sync-kit") => kit::sync(rest),
        Some("check-sources") => check_sources(rest),
        Some("header") => abi::header(),
        Some("abi-check") => abi::check(rest),
        Some("package") => abi::package(rest).map(|_| ()),
        Some("vm") => vm::vm(rest),
        _ => Err("usage: cargo xtask <sync-kit|check-sources|header|abi-check|vm> …".to_string()),
    };
    if let Err(e) = result {
        eprintln!("xtask: {e}");
        std::process::exit(1);
    }
}

/// The repository's root.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The value of `--name VALUE` in `args`, or `default`.
pub fn flag(args: &[String], name: &str, default: &str) -> String {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

/// `~/silica/<dir>`.
pub fn sibling(dir: &str) -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("silica").join(dir)
}

/// The copies of silica-desktop's and silica-icons' sources in `crates/libvetro/data`.
fn check_sources(args: &[String]) -> Result<()> {
    let desktop = PathBuf::from(flag(
        args,
        "--desktop",
        &sibling("silica").to_string_lossy(),
    ));
    let icons = PathBuf::from(flag(
        args,
        "--icons",
        &sibling("silica-icons").to_string_lossy(),
    ));
    let data = root().join("crates/libvetro/data");
    let pairs = [
        (
            desktop.join("docs/design/amalfi/tokens.json"),
            data.join("amalfi-tokens.json"),
        ),
        (
            desktop.join("docs/design/amalfi/motion.md"),
            data.join("amalfi-motion.md"),
        ),
        (
            icons.join("fonts/codepoints.json"),
            data.join("silica-symbols-codepoints.json"),
        ),
    ];
    let mut stale = Vec::new();
    for (src, copy) in pairs {
        let a = std::fs::read(&src).map_err(|e| format!("{}: {e}", src.display()))?;
        let b = std::fs::read(&copy).map_err(|e| format!("{}: {e}", copy.display()))?;
        if a != b {
            stale.push(format!("{} differs from {}", copy.display(), src.display()));
        }
    }
    if stale.is_empty() {
        println!("sources up to date");
        Ok(())
    } else {
        Err(stale.join("\n"))
    }
}
