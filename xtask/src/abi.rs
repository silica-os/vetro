//! The C header, the shared library with versioned symbols, and the ABI check.
//!
//! - `cargo xtask header` writes `include/vetro.h` from vetro-sys's tables (the function
//!   list, the identifier groups) and the struct definitions below; cbindgen cannot see the
//!   macro-generated single list, so the header is generated from it directly.
//! - `cargo xtask package [--debug]` links `target/vetro/libvetro.so.1`: libvetro's static
//!   library with the version script `abi/vetro.map` (every function at `VETRO_1.0`) and the
//!   soname `libvetro.so.1`, plus `libvetro.so` and `vetro.pc`.
//! - `cargo xtask abi-check` verifies that the header is current, that the packaged library
//!   exports exactly the listed functions at `VETRO_1.0` (no more, no less), that the C
//!   struct layouts equal Rust's (a C program compiled against the header), and that a C
//!   program runs headless against the library. `abi/vetro-1.symbols` is the frozen list:
//!   new functions are appended; removing or changing one is an ABI break.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use vetro_sys::{SIGNATURES, Signature};

use crate::{Result, root};

/// Rust type tokens (as `stringify!` prints them) to C.
fn c_type(rust: &str) -> String {
    let t = rust
        .replace("$crate ::", "")
        .replace(":: std :: ffi ::", "")
        .replace("::std::ffi::", "")
        .trim()
        .to_string();
    let words: Vec<&str> = t.split_whitespace().collect();
    fn base(w: &str) -> String {
        match w {
            "u8" => "uint8_t".into(),
            "u32" => "uint32_t".into(),
            "u64" => "uint64_t".into(),
            "i32" => "int32_t".into(),
            "i64" => "int64_t".into(),
            "f32" => "float".into(),
            "f64" => "double".into(),
            "usize" => "size_t".into(),
            "c_char" => "char".into(),
            "c_void" => "void".into(),
            other => other.to_string(),
        }
    }
    // Pointers: "* mut X", "* const X", "* const * mut X", "* const * const X".
    let mut quals: Vec<&str> = Vec::new();
    let mut i = 0;
    while words.get(i) == Some(&"*") {
        quals.push(words.get(i + 1).copied().unwrap_or("mut"));
        i += 2;
    }
    let ty = base(
        words
            .get(i..)
            .map(|w| w.join(" "))
            .unwrap_or_default()
            .as_str(),
    );
    if quals.is_empty() {
        return ty;
    }
    // The innermost pointer's constness qualifies the pointee.
    let mut out = String::new();
    let inner_const = quals.last() == Some(&"const");
    if inner_const {
        out.push_str("const ");
    }
    out.push_str(&ty);
    out.push_str(" *");
    for q in quals.iter().rev().skip(1) {
        if *q == "const" {
            out.push_str("const *");
        } else {
            out.push('*');
        }
    }
    out
}

fn prototype(s: &Signature) -> String {
    let ret = if s.ret.is_empty() {
        "void".to_string()
    } else {
        c_type(s.ret)
    };
    let args: Vec<String> = s
        .args
        .iter()
        .map(|(n, t)| {
            let ct = c_type(t);
            if ct.ends_with('*') {
                format!("{ct}{n}")
            } else {
                format!("{ct} {n}")
            }
        })
        .collect();
    let args = if args.is_empty() {
        "void".to_string()
    } else {
        args.join(", ")
    };
    if ret.ends_with('*') {
        format!("{ret}{}({args});", s.name)
    } else {
        format!("{ret} {}({args});", s.name)
    }
}

const STRUCTS: &str = r#"typedef struct VetroView VetroView;
typedef struct VetroWindow VetroWindow;
typedef struct VetroCanvas VetroCanvas;
typedef struct VetroMenu VetroMenu;

typedef int32_t VetroStatus;
typedef uint32_t VetroValueKind;

typedef struct VetroColor {
    uint8_t r, g, b, a;
} VetroColor;

typedef struct VetroRect {
    float x, y, width, height;
} VetroRect;

typedef struct VetroInsets {
    float top, left, bottom, right;
} VetroInsets;

typedef struct VetroValue {
    VetroValueKind kind;
    uint8_t b;
    int64_t i;
    double f;
    const char *text;
    size_t text_len;
    VetroRect rect;
    VetroColor color;
} VetroValue;

typedef struct VetroEvent {
    uint32_t kind;
    float x, y, dx, dy;
    uint32_t button, clicks, mods, key;
    uint8_t pressed;
    const char *text;
    size_t text_len;
} VetroEvent;

typedef void (*VetroCallback)(void *user);
typedef void (*VetroDestroy)(void *user);
typedef void (*VetroSignalFn)(VetroView *view, uint32_t signal, const VetroValue *value, void *user);
typedef void (*VetroDrawFn)(VetroCanvas *canvas, float width, float height, void *user);
typedef void (*VetroMeasureFn)(float proposed_width, float proposed_height, float *out_width,
                               float *out_height, void *user);
typedef uint8_t (*VetroEventFn)(VetroView *view, const VetroEvent *event, void *user);
typedef void (*VetroBytesFn)(const uint8_t *data, size_t len, void *user);
typedef void (*VetroMenuFn)(uint32_t id, void *user);
"#;

/// The header's text.
pub fn header_text() -> String {
    let mut h = String::new();
    let _ = writeln!(
        h,
        "/* vetro.h: the C API of Vetro, Silica's native system toolkit (libvetro.so.1,\n * symbol version VETRO_1.0). Generated by `cargo xtask header` from vetro-sys; do\n * not edit. Licensed under MIT OR Apache-2.0. */\n\n#ifndef VETRO_H\n#define VETRO_H\n\n#include <stddef.h>\n#include <stdint.h>\n\n#ifdef __cplusplus\nextern \"C\" {{\n#endif\n"
    );
    let _ = writeln!(
        h,
        "#define VETRO_ABI_VERSION 0x{:06X}u\n",
        vetro_sys::VETRO_ABI_VERSION
    );
    h.push_str(STRUCTS);
    h.push('\n');
    for (name, v) in vetro_sys::STATUSES {
        let _ = writeln!(h, "#define {name} ({v})");
    }
    h.push('\n');
    let groups: [(&str, &[(&str, u32)]); 10] = [
        ("Accessibility actions", vetro_sys::ACTIONS),
        ("Value kinds", vetro_sys::VALUE_KINDS),
        ("Events, modifiers and keys", vetro_sys::EVENT_IDS),
        ("Properties", vetro_sys::PROPERTIES),
        ("Signals", vetro_sys::SIGNALS),
        ("View kinds", vetro_sys::KINDS),
        ("Window kinds", vetro_sys::WINDOW_KINDS),
        ("Shell layers, edges and keyboard modes", vetro_sys::LAYERS),
        ("Menu flags and pop-up kinds", vetro_sys::MENU_FLAGS),
        ("(end)", &[]),
    ];
    for (title, list) in groups {
        if list.is_empty() {
            continue;
        }
        let _ = writeln!(h, "/* {title}. */");
        for (name, v) in list {
            let _ = writeln!(h, "#define {name} {v}u");
        }
        h.push('\n');
    }
    for s in SIGNATURES {
        let _ = writeln!(h, "{}", prototype(s));
    }
    h.push_str("\n#ifdef __cplusplus\n}\n#endif\n\n#endif /* VETRO_H */\n");
    h
}

/// `cargo xtask header`.
pub fn header() -> Result<()> {
    let path = root().join("include/vetro.h");
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, header_text()).map_err(|e| e.to_string())?;
    let map = root().join("abi/vetro.map");
    if let Some(d) = map.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let mut m = String::from("/* Generated by `cargo xtask header`. */\nVETRO_1.0 {\n  global:\n");
    for s in SIGNATURES {
        let _ = writeln!(m, "    {};", s.name);
    }
    m.push_str("  local:\n    *;\n};\n");
    std::fs::write(&map, m).map_err(|e| e.to_string())?;
    let symbols = root().join("abi/vetro-1.symbols");
    if !symbols.exists() {
        let list: Vec<&str> = SIGNATURES.iter().map(|s| s.name).collect();
        std::fs::write(&symbols, list.join("\n") + "\n").map_err(|e| e.to_string())?;
    }
    println!("wrote {} and {}", path.display(), map.display());
    Ok(())
}

fn run(cmd: &mut Command) -> Result<String> {
    let out = cmd.output().map_err(|e| format!("{cmd:?}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{cmd:?} failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `cargo xtask package`: the shared library with versioned symbols.
pub fn package(args: &[String]) -> Result<PathBuf> {
    let release = !args.iter().any(|a| a == "--debug");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut build = Command::new(&cargo);
    build.current_dir(root()).args([
        "rustc",
        "-p",
        "libvetro",
        "--lib",
        "--crate-type",
        "staticlib",
    ]);
    if release {
        build.arg("--release");
    }
    build.args(["--", "--print", "native-static-libs"]);
    let out = build.output().map_err(|e| format!("cargo rustc: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo rustc failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    // The libraries the static library needs, as rustc reports them (when it rebuilt it), or
    // the last known list.
    let stderr = String::from_utf8_lossy(&out.stderr);
    let cache = root().join("target/vetro/native-libs");
    let native: Vec<String> = match stderr
        .lines()
        .find_map(|l| l.split("native-static-libs:").nth(1))
    {
        Some(list) => {
            let _ = std::fs::create_dir_all(root().join("target/vetro"));
            let _ = std::fs::write(&cache, list.trim());
            list.split_whitespace().map(str::to_string).collect()
        }
        None => std::fs::read_to_string(&cache)
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
            .collect(),
    };
    let profile = if release { "release" } else { "debug" };
    let staticlib = root().join("target").join(profile).join("libvetro_lib.a");
    let out_dir = root().join("target/vetro");
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let so = out_dir.join("libvetro.so.1");
    let map = root().join("abi/vetro.map");
    let shim_dirs = shim_dirs();
    let mut cc = Command::new("cc");
    cc.args(["-shared", "-o"])
        .arg(&so)
        // Every ABI function pulls its object in (the archive bundles Skia's libraries, which
        // a whole-archive link would include twice).
        .args(SIGNATURES.iter().map(|s| format!("-Wl,-u,{}", s.name)))
        .arg(&staticlib)
        .arg(format!("-Wl,--version-script={}", map.display()))
        .arg("-Wl,-soname,libvetro.so.1")
        .arg("-Wl,--gc-sections")
        .arg("-Wl,-z,defs");
    for d in shim_dirs {
        cc.arg(format!("-L{}", d.display()));
    }
    if native.is_empty() {
        return Err("unknown native libraries: touch crates/libvetro/src/lib.rs and retry".into());
    }
    cc.args(&native);
    run(&mut cc)?;
    let link = out_dir.join("libvetro.so");
    let _ = std::fs::remove_file(&link);
    #[cfg(unix)]
    std::os::unix::fs::symlink("libvetro.so.1", &link).map_err(|e| e.to_string())?;
    let pc = format!(
        "prefix=/usr\nlibdir=${{prefix}}/lib64\nincludedir=${{prefix}}/include\n\nName: vetro\nDescription: Silica's native system toolkit\nVersion: {}\nLibs: -L${{libdir}} -lvetro\nCflags: -I${{includedir}}\n",
        env!("CARGO_PKG_VERSION")
    );
    std::fs::write(out_dir.join("vetro.pc"), pc).map_err(|e| e.to_string())?;
    println!("{}", so.display());
    Ok(so)
}

/// The directories with the libfreetype/libfontconfig link shims libvetro's build made.
fn shim_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for profile in ["release", "debug"] {
        let build = root().join("target").join(profile).join("build");
        if let Ok(rd) = std::fs::read_dir(&build) {
            for e in rd.flatten() {
                let p = e.path().join("out/shims");
                if e.file_name().to_string_lossy().starts_with("libvetro-") && p.is_dir() {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// The dynamic symbols of `so` with their versions: `name@VERSION`.
fn exported(so: &Path) -> Result<Vec<String>> {
    let text = run(Command::new("nm")
        .args(["-D", "--defined-only", "--with-symbol-versions"])
        .arg(so))?;
    Ok(text
        .lines()
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let _addr = parts.next()?;
            let kind = parts.next()?;
            let name = parts.next()?;
            (kind == "T" || kind == "t").then(|| name.to_string())
        })
        .collect())
}

const LAYOUT_PROBE: &str = r#"#include <stdio.h>
#include <stddef.h>
#include "vetro.h"
int main(void) {
    printf("VetroColor %zu\n", sizeof(VetroColor));
    printf("VetroRect %zu\n", sizeof(VetroRect));
    printf("VetroInsets %zu\n", sizeof(VetroInsets));
    printf("VetroValue %zu %zu %zu %zu %zu %zu %zu\n", sizeof(VetroValue), offsetof(VetroValue, i),
           offsetof(VetroValue, f), offsetof(VetroValue, text), offsetof(VetroValue, text_len),
           offsetof(VetroValue, rect), offsetof(VetroValue, color));
    printf("VetroEvent %zu %zu %zu\n", sizeof(VetroEvent), offsetof(VetroEvent, pressed),
           offsetof(VetroEvent, text));
    return 0;
}
"#;

const SMOKE: &str = r#"#include <stdio.h>
#include <string.h>
#include "vetro.h"
static int hits;
static void on_activate(VetroView *v, uint32_t s, const VetroValue *val, void *user) {
    (void)v; (void)s; (void)val; (void)user; hits++;
}
static void on_png(const uint8_t *data, size_t len, void *user) {
    (void)user;
    if (len > 8 && memcmp(data + 1, "PNG", 3) == 0) hits += 10;
}
int main(void) {
    if (vetro_abi_version() >> 16 != 1) return 1;
    if (vetro_headless_init() != VETRO_OK) return 2;
    VetroWindow *w = vetro_window_new(VETRO_WINDOW_NORMAL, 200, 100);
    VetroView *col = vetro_view_new(VETRO_KIND_COLUMN);
    VetroView *b = vetro_view_new(VETRO_KIND_PUSH_BUTTON);
    if (!w || !col || !b) return 3;
    if (vetro_view_set_string(b, VETRO_PROP_TITLE, "Hello") != VETRO_OK) return 4;
    vetro_view_add_child(col, b);
    vetro_window_set_content(w, col);
    vetro_view_connect(b, VETRO_SIGNAL_ACTIVATED, on_activate, NULL, NULL);
    vetro_window_show(w);
    if (vetro_window_snapshot_png(w, 1.0f, on_png, NULL) != VETRO_OK) return 5;
    char buf[32]; size_t len = 0;
    if (vetro_view_get_string(b, VETRO_PROP_TITLE, buf, sizeof buf, &len) != VETRO_OK || len != 5) return 6;
    vetro_view_release(b);
    vetro_view_release(col);
    vetro_window_close(w);
    vetro_headless_shutdown();
    printf("hits %d\n", hits);
    return hits == 10 ? 0 : 7;
}
"#;

/// `cargo xtask abi-check`.
pub fn check(args: &[String]) -> Result<()> {
    // 1. The header is current.
    let current = std::fs::read_to_string(root().join("include/vetro.h")).unwrap_or_default();
    if current != header_text() {
        return Err("include/vetro.h is stale: run `cargo xtask header`".into());
    }
    // 2. The frozen list is a prefix of the current one (append-only).
    let frozen =
        std::fs::read_to_string(root().join("abi/vetro-1.symbols")).map_err(|e| e.to_string())?;
    let frozen: Vec<&str> = frozen.lines().filter(|l| !l.trim().is_empty()).collect();
    let now: Vec<&str> = SIGNATURES.iter().map(|s| s.name).collect();
    for name in &frozen {
        if !now.contains(name) {
            return Err(format!("ABI break: {name} was removed"));
        }
    }
    // 3. The library exports exactly the functions, all at VETRO_1.0.
    let so = package(args)?;
    let syms = exported(&so)?;
    let mut problems = Vec::new();
    for name in &now {
        let want = format!("{name}@@VETRO_1.0");
        if !syms.iter().any(|s| *s == want) {
            problems.push(format!("missing {want}"));
        }
    }
    for s in &syms {
        let base = s.split('@').next().unwrap_or_default();
        if !s.starts_with("VETRO_") && !now.contains(&base) {
            problems.push(format!("unexpected export {s}"));
        }
    }
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    // 4. Struct layouts and a headless smoke test in C.
    let dir = root().join("target/vetro/abi");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("layout.c"), LAYOUT_PROBE).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("smoke.c"), SMOKE).map_err(|e| e.to_string())?;
    let inc = root().join("include");
    run(Command::new("cc")
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Werror")
        .arg(format!("-I{}", inc.display()))
        .arg(dir.join("layout.c"))
        .arg("-o")
        .arg(dir.join("layout")))?;
    let layout = run(&mut Command::new(dir.join("layout")))?;
    let expected = format!(
        "VetroColor {}\nVetroRect {}\nVetroInsets {}\nVetroValue {} {} {} {} {} {} {}\nVetroEvent {} {} {}\n",
        std::mem::size_of::<vetro_sys::VetroColor>(),
        std::mem::size_of::<vetro_sys::VetroRect>(),
        std::mem::size_of::<vetro_sys::VetroInsets>(),
        std::mem::size_of::<vetro_sys::VetroValue>(),
        std::mem::offset_of!(vetro_sys::VetroValue, i),
        std::mem::offset_of!(vetro_sys::VetroValue, f),
        std::mem::offset_of!(vetro_sys::VetroValue, text),
        std::mem::offset_of!(vetro_sys::VetroValue, text_len),
        std::mem::offset_of!(vetro_sys::VetroValue, rect),
        std::mem::offset_of!(vetro_sys::VetroValue, color),
        std::mem::size_of::<vetro_sys::VetroEvent>(),
        std::mem::offset_of!(vetro_sys::VetroEvent, pressed),
        std::mem::offset_of!(vetro_sys::VetroEvent, text),
    );
    if layout != expected {
        return Err(format!(
            "struct layouts differ:\nC:\n{layout}Rust:\n{expected}"
        ));
    }
    let lib_dir = so.parent().map(Path::to_path_buf).unwrap_or_default();
    run(Command::new("cc")
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Werror")
        .arg(format!("-I{}", inc.display()))
        .arg(dir.join("smoke.c"))
        .arg(format!("-L{}", lib_dir.display()))
        .arg("-lvetro")
        .arg(format!("-Wl,-rpath,{}", lib_dir.display()))
        .arg("-o")
        .arg(dir.join("smoke")))?;
    let out = run(Command::new(dir.join("smoke")).env("VETRO_DETERMINISTIC", "1"))?;
    println!(
        "ABI check passed: {} functions at VETRO_1.0, layouts equal, C smoke test {}",
        now.len(),
        out.trim()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_types_to_c() {
        assert_eq!(c_type("* mut $crate :: VetroView"), "VetroView *");
        assert_eq!(c_type("* const :: std :: ffi :: c_char"), "const char *");
        assert_eq!(
            c_type("* const * mut $crate :: VetroMenu"),
            "VetroMenu *const *"
        );
        assert_eq!(
            c_type("* const * const :: std :: ffi :: c_char"),
            "const char *const *"
        );
        assert_eq!(c_type("u32"), "uint32_t");
        assert_eq!(c_type("$crate :: VetroCallback"), "VetroCallback");
    }
}
