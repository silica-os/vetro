/// `dev.frascella.Silica.Thumbnails1`: paths of cached PNGs.
#[zbus::proxy(
    interface = "dev.frascella.Silica.Thumbnails1",
    default_service = "dev.frascella.Silica",
    default_path = "/dev/frascella/Silica/Thumbnails1"
)]
pub trait Thumbnails1 {
    /// The cached PNG for `uri` (empty: none); `Ready` follows a render.
    fn thumbnail(&self, uri: &str, size_px: u32) -> zbus::Result<String>;
    /// (path, is_preview). `flags`: bit 0 `NO_PREVIEW`, bits 1-3 icon style, bits 8-31 accent.
    fn icon(&self, uri: &str, size_px: u32, scale: f64, flags: u32)
    -> zbus::Result<(String, bool)>;
    /// `appearance`: 0 light, 1 clear, 2 tinted, +4 for dark.
    fn app_icon(
        &self,
        app: &str,
        size_px: u32,
        scale: f64,
        appearance: u32,
        accent_rgb: u32,
    ) -> zbus::Result<String>;
    fn invalidate(&self, uris: &[&str]) -> zbus::Result<()>;

    #[zbus(signal)]
    fn ready(&self, uri: &str, size_px: u32, path: &str) -> zbus::Result<()>;
}
