//! Build steps of libvetro:
//!
//! - the Amalfi tokens and the Silica Symbols codepoints are generated from their JSON
//!   sources (`data/`), so that nothing parses JSON at run time;
//! - Skia links `-lfreetype -lfontconfig`: on a machine with only the runtime packages (no
//!   unversioned `.so` symlinks) the symlinks are made in `OUT_DIR`;
//! - the shared library gets its soname (`libvetro.so.1`) and versioned symbols
//!   (`VETRO_1.0`, `vetro.map`).

use std::path::{Path, PathBuf};

#[path = "build/tokens.rs"]
mod tokens;

fn main() {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default());
    link_shims(&out);
    if let Err(e) = tokens::generate(&out) {
        println!("cargo:warning=tokens: {e}");
        std::process::exit(1);
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build/tokens.rs");
    println!("cargo:rerun-if-changed=data");
}

/// Symlinks `lib<name>.so` → the versioned runtime library when the unversioned one is
/// missing from the usual directories.
fn link_shims(out: &Path) {
    let dirs = [
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib64",
        "/usr/lib",
        "/usr/lib/aarch64-linux-gnu",
    ];
    let shim_dir = out.join("shims");
    let mut needed = false;
    for (name, versioned) in [("freetype", "libfreetype.so.6"), ("fontconfig", "libfontconfig.so.1")] {
        let plain = format!("lib{name}.so");
        if dirs.iter().any(|d| Path::new(d).join(&plain).exists()) {
            continue;
        }
        let Some(found) = dirs
            .iter()
            .map(|d| Path::new(d).join(versioned))
            .find(|p| p.exists())
        else {
            continue;
        };
        let _ = std::fs::create_dir_all(&shim_dir);
        let link = shim_dir.join(&plain);
        let _ = std::fs::remove_file(&link);
        #[cfg(unix)]
        if std::os::unix::fs::symlink(&found, &link).is_ok() {
            needed = true;
        }
    }
    if needed {
        println!("cargo:rustc-link-search=native={}", shim_dir.display());
    }
}
