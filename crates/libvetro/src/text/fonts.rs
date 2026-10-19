//! Fonts: Inter, JetBrains Mono and Silica Symbols are bundled in the library (identical
//! rendering everywhere, no packaging dependency); fontconfig serves every other family and
//! the fallback (Noto Color Emoji, CJK).

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};

use skia_safe::ConditionallySend;
use skia_safe::textlayout::{FontCollection, TypefaceFontProvider};
use skia_safe::{Data, FontMgr, FontStyle, Typeface, font_style};

/// The bundled faces: (family, CSS weight, bytes).
static BUNDLED: &[(&str, u16, &[u8])] = &[
    (
        "Inter",
        400,
        include_bytes!("../../fonts/Inter-Regular.ttf"),
    ),
    ("Inter", 500, include_bytes!("../../fonts/Inter-Medium.ttf")),
    (
        "Inter",
        600,
        include_bytes!("../../fonts/Inter-SemiBold.ttf"),
    ),
    ("Inter", 700, include_bytes!("../../fonts/Inter-Bold.ttf")),
    (
        "JetBrains Mono",
        400,
        include_bytes!("../../fonts/JetBrainsMono-Regular.ttf"),
    ),
    (
        "Silica Symbols",
        300,
        include_bytes!("../../fonts/SilicaSymbols-Light.otf"),
    ),
    (
        "Silica Symbols",
        400,
        include_bytes!("../../fonts/SilicaSymbols-Regular.otf"),
    ),
    (
        "Silica Symbols",
        600,
        include_bytes!("../../fonts/SilicaSymbols-Semibold.otf"),
    ),
];

/// The UI font family.
pub const UI_FAMILY: &str = "Inter";
/// The symbol font family.
pub const SYMBOLS_FAMILY: &str = "Silica Symbols";
/// The monospaced family.
pub const MONO_FAMILY: &str = "JetBrains Mono";

/// The font system of the thread (all UI runs on the main thread).
pub struct Fonts {
    collection: FontCollection,
    bundled: Vec<(String, u16, Typeface)>,
    /// The system font manager (fontconfig): made on a thread of its own, since scanning
    /// the fonts can take a second (a cold cache) and the first frame needs only the
    /// bundled faces.
    system: System,
}

enum System {
    /// Not wanted (deterministic rendering).
    Off,
    /// Being made.
    Loading(Receiver<skia_safe::Sendable<FontMgr>>),
    /// Ready (and installed as the collection's fallback).
    Ready(FontMgr),
    /// The loader failed: no system fonts.
    Failed,
}

/// Bumped when the fonts change (the system fonts are loaded): text is laid out again.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The fonts' generation (layouts made with an older one are stale).
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

thread_local! {
    static FONTS: RefCell<Option<Fonts>> = const { RefCell::new(None) };
}

impl Fonts {
    fn new(system_fallback: bool) -> Fonts {
        // The bundled faces load without scanning the system's fonts.
        let loader = FontMgr::custom_empty().unwrap_or_else(FontMgr::empty);
        let mut provider = TypefaceFontProvider::new();
        let mut bundled = Vec::new();
        for (family, weight, bytes) in BUNDLED {
            match loader.new_from_data(Data::new_copy(bytes), None) {
                Some(tf) => {
                    provider.register_typeface(tf.clone(), Some(*family));
                    bundled.push(((*family).to_string(), *weight, tf));
                }
                None => tracing::error!("bundled font {family} {weight} failed to load"),
            }
        }
        let mut collection = FontCollection::new();
        let provider: FontMgr = provider.into();
        collection.set_asset_font_manager(Some(provider));
        collection.set_dynamic_font_manager(None);
        collection.enable_font_fallback();
        let system = if system_fallback {
            let (tx, rx) = std::sync::mpsc::channel();
            let spawned = std::thread::Builder::new()
                .name("vetro-fonts".into())
                .spawn(move || {
                    let mgr = FontMgr::new();
                    // Touch the configuration so the scan happens here.
                    let _ = mgr.count_families();
                    if let Ok(m) = mgr.wrap_send() {
                        let _ = tx.send(m);
                    }
                });
            match spawned {
                Ok(_) => System::Loading(rx),
                Err(_) => System::Ready(FontMgr::new()),
            }
        } else {
            System::Off
        };
        let mut f = Fonts {
            collection,
            bundled,
            system,
        };
        f.install_system();
        f
    }

    /// The system font manager when it is ready (`wait`: block until it is).
    fn system(&mut self, wait: bool) -> Option<FontMgr> {
        if let System::Loading(rx) = &self.system {
            let got = if wait {
                rx.recv().map_err(|_| TryRecvError::Disconnected)
            } else {
                rx.try_recv()
            };
            match got {
                Ok(m) => self.system = System::Ready(m.into_inner()),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => self.system = System::Failed,
            }
            if let System::Ready(m) = &self.system {
                self.collection.set_dynamic_font_manager(Some(m.clone()));
                self.collection.clear_caches();
                GENERATION.fetch_add(1, Ordering::Relaxed);
            }
        }
        match &self.system {
            System::Ready(m) => Some(m.clone()),
            _ => None,
        }
    }

    /// Installs the system fonts if they finished loading; returns whether they just did.
    pub fn install_system(&mut self) -> bool {
        let before = generation();
        let _ = self.system(false);
        generation() != before
    }

    /// Runs `f` with the thread's font system, creating it on first use.
    pub fn with<R>(f: impl FnOnce(&mut Fonts) -> R) -> R {
        FONTS.with(|cell| {
            let mut slot = cell.borrow_mut();
            let fonts = slot.get_or_insert_with(|| Fonts::new(!deterministic()));
            // The system fonts join the fallback once they are loaded.
            fonts.install_system();
            f(fonts)
        })
    }

    /// The paragraph font collection.
    pub fn collection(&self) -> &FontCollection {
        &self.collection
    }

    /// A bundled face of `family` closest to `weight`.
    pub fn bundled(&self, family: &str, weight: u16) -> Option<Typeface> {
        self.bundled
            .iter()
            .filter(|(f, _, _)| f == family)
            .min_by_key(|(_, w, _)| (i32::from(*w) - i32::from(weight)).abs())
            .map(|(_, _, tf)| tf.clone())
    }

    /// The typeface for `family` at `weight`: bundled, else from fontconfig, else Inter.
    pub fn typeface(&mut self, family: &str, weight: u16, italic: bool) -> Option<Typeface> {
        if let Some(tf) = self.bundled(family, weight) {
            return Some(tf);
        }
        let slant = if italic {
            font_style::Slant::Italic
        } else {
            font_style::Slant::Upright
        };
        let style = FontStyle::new(
            font_style::Weight::from(i32::from(weight)),
            font_style::Width::NORMAL,
            slant,
        );
        if let Some(tf) = self
            .system(true)
            .and_then(|m| m.match_family_style(family, style))
        {
            return Some(tf);
        }
        self.bundled(UI_FAMILY, weight)
    }
}

/// Whether text must render identically on every machine (tests, `VETRO_DETERMINISTIC`):
/// only the bundled fonts, no system fallback.
pub fn deterministic() -> bool {
    cfg!(test) || std::env::var_os("VETRO_DETERMINISTIC").is_some()
}

/// Silica Symbols glyphs by name (silica-icons `fonts/codepoints.json`).
pub mod symbols {
    include!(concat!(env!("OUT_DIR"), "/silica_symbols.rs"));

    /// The codepoint of the symbol `name` (`chevron-down`, `trash` …).
    pub fn codepoint(name: &str) -> Option<char> {
        SYMBOLS
            .binary_search_by(|(k, _)| (*k).cmp(name))
            .ok()
            .and_then(|i| SYMBOLS.get(i))
            .and_then(|(_, cp)| char::from_u32(*cp))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_faces_load() {
        Fonts::with(|f| {
            assert!(f.bundled("Inter", 400).is_some());
            assert!(f.bundled("Inter", 650).is_some());
            assert!(f.bundled("Silica Symbols", 400).is_some());
            assert!(f.typeface("No Such Family", 400, false).is_some());
        });
        assert!(symbols::codepoint("chevron-down").is_some());
        assert!(symbols::codepoint("no-such-symbol").is_none());
    }
}
