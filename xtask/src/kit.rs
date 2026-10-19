//! `sync-kit`: copies masters of the Amalfi kit (silica-design) into
//! `crates/vetro-test/kit`, with the masters they nest. Tests never read silica-design.
//!
//! Prefixes are recorded in `kit/_prefixes` (one per line); without prefixes, every recorded
//! prefix is synced again (after a kit update). `kit/SOURCE.md` records the commit.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::{Result, flag, root, sibling};

pub fn sync(args: &[String]) -> Result<()> {
    let design = PathBuf::from(flag(
        args,
        "--design",
        &sibling("silica-design").to_string_lossy(),
    ));
    let kit = root().join("crates/vetro-test/kit");
    std::fs::create_dir_all(&kit).map_err(|e| e.to_string())?;
    let prefixes_file = kit.join("_prefixes");
    let mut prefixes: BTreeSet<String> = std::fs::read_to_string(&prefixes_file)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    let mut skip = false;
    for (i, a) in args.iter().enumerate() {
        if skip {
            skip = false;
            continue;
        }
        if a == "--design" {
            skip = true;
            continue;
        }
        let _ = i;
        prefixes.insert(a.trim_end_matches('/').to_string());
    }
    let index_path = design.join("kit/index.json");
    let index: Value = serde_json::from_str(
        &std::fs::read_to_string(&index_path)
            .map_err(|e| format!("{}: {e}", index_path.display()))?,
    )
    .map_err(|e| e.to_string())?;
    let symbols = index["symbols"].as_array().ok_or("index: no symbols")?;
    let names: Vec<&str> = symbols.iter().filter_map(|s| s["name"].as_str()).collect();
    let mut wanted: BTreeSet<String> = names
        .iter()
        .filter(|n| {
            prefixes
                .iter()
                .any(|p| **n == p.as_str() || n.starts_with(&format!("{p}/")))
        })
        .map(|n| (*n).to_string())
        .collect();
    // Nested masters.
    let mut queue: Vec<String> = wanted.iter().cloned().collect();
    while let Some(name) = queue.pop() {
        let path = design.join("kit/symbols").join(format!("{name}.json"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let mut nested = Vec::new();
        collect_masters(&v, &mut nested);
        for n in nested {
            if names.contains(&n.as_str()) && wanted.insert(n.clone()) {
                queue.push(n);
            }
        }
    }
    let mut copied = 0;
    for name in &wanted {
        for ext in [".json", "@2x.png"] {
            let src = design.join("kit/symbols").join(format!("{name}{ext}"));
            let dst = kit.join(format!("{name}{ext}"));
            if let Some(dir) = dst.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            if src.exists() {
                copy_if_changed(&src, &dst)?;
                copied += 1;
            }
        }
    }
    std::fs::write(
        &prefixes_file,
        prefixes.iter().cloned().collect::<Vec<_>>().join("\n") + "\n",
    )
    .map_err(|e| e.to_string())?;
    let commit = std::process::Command::new("git")
        .arg("-C")
        .arg(&design)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    std::fs::write(
        kit.join("SOURCE.md"),
        format!(
            "# Kit fixtures\n\nCopies of symbol masters of the Silica 1.0 \"Amalfi\" UI kit, from\n\
             [silica-design](https://github.com/silica-os/silica-design) (CC BY-SA 4.0, the Silica\n\
             project), made by `cargo xtask sync-kit`. Do not edit: run the tool again.\n\n\
             silica-design commit: `{}`\n\nThe selected name prefixes are in `_prefixes`; the masters \
             nested in them as symbol instances are copied too.\n",
            commit.trim()
        ),
    )
    .map_err(|e| e.to_string())?;
    println!("{} masters, {copied} files", wanted.len());
    Ok(())
}

fn collect_masters(v: &Value, out: &mut Vec<String>) {
    if let Some(m) = v["master"].as_str() {
        out.push(m.to_string());
    }
    if let Some(children) = v["children"].as_array() {
        for c in children {
            collect_masters(c, out);
        }
    }
}

fn copy_if_changed(src: &Path, dst: &Path) -> Result<()> {
    let a = std::fs::read(src).map_err(|e| format!("{}: {e}", src.display()))?;
    if std::fs::read(dst).ok().as_deref() == Some(a.as_slice()) {
        return Ok(());
    }
    std::fs::write(dst, a).map_err(|e| format!("{}: {e}", dst.display()))
}
