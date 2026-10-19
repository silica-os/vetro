//! Test doubles: a private `dbus-daemon` and mock servers whose methods, properties and
//! signals copy the signatures of the daemon's server implementations (silica-desktop,
//! `crates/daemon/src`), so the proxies are checked against the wire types, not against
//! themselves.

#![allow(clippy::type_complexity)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zbus::fdo;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedValue, Value};

use crate::services::types::Notification;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A private session bus, killed (by its own PID) and removed when dropped.
pub struct TestBus {
    child: Child,
    dir: PathBuf,
    pub address: String,
}

impl TestBus {
    /// `None` when `dbus-daemon` cannot be started (the tests then skip).
    pub fn start() -> Option<TestBus> {
        let dir = PathBuf::from(format!(
            "/dev/shm/vetro-svc-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).ok()?;
        let socket = dir.join("bus");
        let config = dir.join("bus.conf");
        let xml = format!(
            "<!DOCTYPE busconfig PUBLIC \"-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN\" \
             \"http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd\">\n\
             <busconfig><type>session</type><listen>unix:path={}</listen><auth>EXTERNAL</auth>\
             <policy context=\"default\"><allow send_destination=\"*\" eavesdrop=\"true\"/>\
             <allow eavesdrop=\"true\"/><allow own=\"*\"/></policy></busconfig>\n",
            socket.display()
        );
        std::fs::write(&config, xml).ok()?;
        let child = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .arg("--nofork")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let bus = TestBus {
            child,
            dir,
            address: format!("unix:path={}", socket.display()),
        };
        let started = Instant::now();
        while !socket.exists() {
            if started.elapsed() > Duration::from_secs(5) {
                return None;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Some(bus)
    }

    pub async fn connect(&self) -> zbus::Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }
}

impl Drop for TestBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub type Dict = Arc<Mutex<HashMap<String, OwnedValue>>>;

pub fn owned(value: Value<'_>) -> OwnedValue {
    OwnedValue::try_from(value).unwrap()
}

pub const SETTINGS_PATH: &str = "/dev/frascella/Silica/Settings1";
pub const SYSTEM_PATH: &str = "/dev/frascella/Silica/System1";

// ---- Settings1 -----------------------------------------------------------------------------

pub struct MockSettings {
    pub values: Dict,
}

#[zbus::interface(name = "dev.frascella.Silica.Settings1")]
impl MockSettings {
    fn get(&self, key: &str) -> fdo::Result<Value<'static>> {
        let values = self.values.lock().unwrap();
        let value = values
            .get(key)
            .ok_or_else(|| fdo::Error::InvalidArgs(format!("no key {key}")))?;
        Ok(Value::from(value.try_clone().unwrap()))
    }

    fn get_all(&self) -> HashMap<String, Value<'static>> {
        let values = self.values.lock().unwrap();
        values
            .iter()
            .map(|(k, v)| (k.clone(), Value::from(v.try_clone().unwrap())))
            .collect()
    }

    async fn set(
        &mut self,
        key: &str,
        value: Value<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.values
            .lock()
            .unwrap()
            .insert(key.to_owned(), OwnedValue::try_from(&value).unwrap());
        Self::changed(&emitter, key, value).await?;
        Ok(())
    }

    async fn set_many(
        &mut self,
        values: HashMap<String, Value<'_>>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        for (key, value) in values {
            self.values
                .lock()
                .unwrap()
                .insert(key.clone(), OwnedValue::try_from(&value).unwrap());
            Self::changed(&emitter, &key, value).await?;
        }
        Ok(())
    }

    async fn reset(
        &mut self,
        key: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.values.lock().unwrap().remove(key);
        Self::changed(&emitter, key, Value::I64(0)).await?;
        Ok(())
    }

    #[zbus(signal)]
    async fn changed(emitter: &SignalEmitter<'_>, key: &str, value: Value<'_>) -> zbus::Result<()>;
}

// ---- System1 -------------------------------------------------------------------------------

pub struct MockSystem {
    pub state: Dict,
}

#[zbus::interface(name = "dev.frascella.Silica.System1")]
impl MockSystem {
    fn state(&self) -> HashMap<String, OwnedValue> {
        let state = self.state.lock().unwrap();
        state
            .iter()
            .map(|(k, v)| (k.clone(), v.try_clone().unwrap()))
            .collect()
    }

    async fn set_wifi_enabled(&self, _enabled: bool) -> fdo::Result<()> {
        Ok(())
    }
    async fn set_bluetooth_powered(&self, _powered: bool) -> fdo::Result<()> {
        Ok(())
    }
    async fn set_volume(&self, _volume: f64) -> fdo::Result<()> {
        Ok(())
    }
    async fn set_muted(&self, _muted: bool) -> fdo::Result<()> {
        Ok(())
    }
    async fn set_brightness(&self, _brightness: f64) -> fdo::Result<()> {
        Ok(())
    }
    async fn set_keyboard_brightness(&self, _brightness: f64) -> fdo::Result<()> {
        Ok(())
    }
    async fn wifi_networks(&self) -> fdo::Result<Vec<(String, u8, bool, bool, bool)>> {
        Ok(vec![("Home".into(), 80, true, true, true)])
    }
    async fn connect_wifi(&self, _ssid: String) -> fdo::Result<()> {
        Ok(())
    }
    async fn bluetooth_devices(&self) -> fdo::Result<Vec<(String, String, String, bool)>> {
        Ok(vec![("AA:BB".into(), "Buds".into(), "audio".into(), true)])
    }
    async fn connect_bluetooth(&self, _address: String, _connect: bool) -> fdo::Result<()> {
        Ok(())
    }
    async fn audio_outputs(&self) -> fdo::Result<Vec<(u32, String, bool)>> {
        Ok(vec![(51, "Speaker".into(), true)])
    }
    async fn set_audio_output(&self, _id: u32) -> fdo::Result<()> {
        Ok(())
    }
    fn set_do_not_disturb(&self, _enabled: bool) {}

    async fn set_dark(
        &self,
        dark: bool,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.state
            .lock()
            .unwrap()
            .insert("appearance.dark".into(), owned(Value::Bool(dark)));
        self.emit(&emitter).await
    }

    async fn set_appearance_mode(&self, _mode: &str) -> fdo::Result<()> {
        Ok(())
    }

    async fn set_look(
        &self,
        settings: HashMap<String, OwnedValue>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        for (key, value) in settings {
            self.state
                .lock()
                .unwrap()
                .insert(format!("look.{key}"), value);
        }
        self.emit(&emitter).await
    }

    fn set_text_replacements(&self, _list: Vec<(String, String)>) -> fdo::Result<()> {
        Ok(())
    }
    async fn outputs(&self) -> fdo::Result<String> {
        Ok("[]".into())
    }
    async fn configure_output(&self, json: &str) -> fdo::Result<String> {
        Ok(json.to_owned())
    }
    fn set_night_light(&self, _settings: HashMap<String, OwnedValue>) -> fdo::Result<()> {
        Ok(())
    }
    fn set_focus(&self, _id: &str) -> fdo::Result<()> {
        Ok(())
    }
    async fn suspend(&self) -> fdo::Result<()> {
        Ok(())
    }
    async fn unlock(&self, password: &str) -> fdo::Result<bool> {
        Ok(password == "right")
    }
    fn report_compositor(&self, _json: &str) -> fdo::Result<()> {
        Ok(())
    }

    #[zbus(signal)]
    pub async fn changed(
        emitter: &SignalEmitter<'_>,
        state: HashMap<String, OwnedValue>,
    ) -> zbus::Result<()>;
}

impl MockSystem {
    async fn emit(&self, emitter: &SignalEmitter<'_>) -> fdo::Result<()> {
        let dict: HashMap<String, OwnedValue> = {
            let state = self.state.lock().unwrap();
            state
                .iter()
                .map(|(k, v)| (k.clone(), v.try_clone().unwrap()))
                .collect()
        };
        Self::changed(emitter, dict).await?;
        Ok(())
    }
}

// ---- The other Silica interfaces -----------------------------------------------------------

pub struct MockApps;

#[zbus::interface(name = "dev.frascella.Silica.Apps1")]
impl MockApps {
    fn list(
        &self,
    ) -> Vec<(
        String,
        String,
        String,
        String,
        HashMap<String, Value<'static>>,
    )> {
        vec![(
            "dev.frascella.Clock".into(),
            "Clock".into(),
            "app".into(),
            "/apps/Clock".into(),
            HashMap::from([("icon".to_owned(), Value::from("clock"))]),
        )]
    }
    fn get(&self, id: &str) -> fdo::Result<HashMap<String, Value<'static>>> {
        Ok(HashMap::from([(
            "id".to_owned(),
            Value::from(id.to_owned()),
        )]))
    }
    fn apps_for(&self, r#type: &str) -> Vec<String> {
        vec![format!("opens-{}", r#type)]
    }
    fn default_for(&self, _type: &str) -> String {
        "dev.frascella.Preview".into()
    }
    async fn set_default(
        &self,
        r#type: &str,
        id: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        Self::default_changed(&emitter, r#type, id).await?;
        Ok(())
    }
    fn open(
        &self,
        id: &str,
        uris: Vec<String>,
        options: HashMap<String, OwnedValue>,
    ) -> fdo::Result<u32> {
        Ok(id.len() as u32 + uris.len() as u32 + options.len() as u32)
    }
    fn open_uris(&self, uris: Vec<String>) -> fdo::Result<u32> {
        Ok(uris.len() as u32)
    }
    async fn note_launched(
        &self,
        id: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        Self::changed(&emitter, id).await?;
        Ok(())
    }
    async fn uninstall(&self, id: &str) -> fdo::Result<bool> {
        Ok(!id.is_empty())
    }
    #[zbus(signal)]
    async fn added(emitter: &SignalEmitter<'_>, id: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn removed(emitter: &SignalEmitter<'_>, id: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn changed(emitter: &SignalEmitter<'_>, id: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn default_changed(
        emitter: &SignalEmitter<'_>,
        r#type: &str,
        id: &str,
    ) -> zbus::Result<()>;
}

pub struct MockTrust;

#[zbus::interface(name = "dev.frascella.Silica.Trust1")]
impl MockTrust {
    async fn assess(&self, path: &str) -> (String, HashMap<String, Value<'static>>) {
        (
            "confirm".into(),
            HashMap::from([("path".to_owned(), Value::from(path.to_owned()))]),
        )
    }
    async fn prompt(&self, _path: &str) -> fdo::Result<String> {
        Ok("open".into())
    }
    async fn suggest_move(&self, _path: &str) -> fdo::Result<String> {
        Ok("cancel".into())
    }
    async fn set_sandbox(&self, _path: &str, on: bool) -> bool {
        on
    }
    fn register_agent(&self) -> fdo::Result<()> {
        Ok(())
    }
    async fn answer(
        &self,
        id: u32,
        choice: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        Self::prompt_closed(&emitter, id, choice).await?;
        Ok(())
    }
    fn pending(&self) -> Vec<(u32, String, HashMap<String, Value<'static>>)> {
        vec![(3, "block".into(), HashMap::new())]
    }
    fn blocked(&self) -> Vec<(String, String, u64)> {
        vec![("/a".into(), "unsigned".into(), 99)]
    }
    async fn open_anyway(&self, _path: &str) -> fdo::Result<bool> {
        Ok(true)
    }
    #[zbus(signal)]
    async fn prompt_requested(
        emitter: &SignalEmitter<'_>,
        id: u32,
        verdict: &str,
        info: HashMap<String, Value<'_>>,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn prompt_closed(emitter: &SignalEmitter<'_>, id: u32, choice: &str) -> zbus::Result<()>;
}

pub struct MockVolumes;

#[zbus::interface(name = "dev.frascella.Silica.Volumes1")]
impl MockVolumes {
    async fn inspect(&self, image: &str) -> fdo::Result<HashMap<String, Value<'static>>> {
        Ok(HashMap::from([
            ("name".to_owned(), Value::from(image.to_owned())),
            ("signature".to_owned(), Value::from("valid")),
        ]))
    }
    async fn attach(
        &self,
        image: &str,
        _options: HashMap<String, OwnedValue>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<String> {
        let mountpoint = format!("/Volumes/{image}");
        Self::attached(&emitter, &mountpoint).await?;
        Ok(mountpoint)
    }
    async fn detach(
        &self,
        mountpoint: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        Self::detached(&emitter, mountpoint).await?;
        Ok(())
    }
    fn list(&self) -> Vec<(String, String, HashMap<String, Value<'static>>)> {
        vec![("/Volumes/x".into(), "/home/x.sxv".into(), HashMap::new())]
    }
    #[zbus(signal)]
    async fn attached(emitter: &SignalEmitter<'_>, mountpoint: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn detached(emitter: &SignalEmitter<'_>, mountpoint: &str) -> zbus::Result<()>;
}

pub struct MockUpdates;

#[zbus::interface(name = "dev.frascella.Silica.Updates1")]
impl MockUpdates {
    async fn check(
        &self,
    ) -> Vec<(
        String,
        String,
        String,
        String,
        String,
        HashMap<String, Value<'static>>,
    )> {
        vec![(
            "os".into(),
            "system".into(),
            "Silica OS".into(),
            "1.0".into(),
            "1.1".into(),
            HashMap::from([("size".to_owned(), Value::U64(10))]),
        )]
    }
    async fn update(
        &self,
        id: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<String> {
        Self::progress(&emitter, id, 0.5).await?;
        Self::finished(&emitter, id, "staged").await?;
        Ok("staged".into())
    }
    async fn update_all(&self) -> Vec<(String, String)> {
        vec![("os".into(), "staged".into())]
    }
    #[zbus(signal)]
    async fn progress(emitter: &SignalEmitter<'_>, id: &str, fraction: f64) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn finished(emitter: &SignalEmitter<'_>, id: &str, result: &str) -> zbus::Result<()>;
}

pub struct MockThumbnails;

#[zbus::interface(name = "dev.frascella.Silica.Thumbnails1")]
impl MockThumbnails {
    async fn thumbnail(
        &self,
        uri: String,
        size_px: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> String {
        let _ = Self::ready(&emitter, &uri, size_px, "/cache/t.png").await;
        String::new()
    }
    async fn icon(&self, _uri: String, size_px: u32, scale: f64, flags: u32) -> (String, bool) {
        (
            format!("/icon-{size_px}-{scale}-{flags}.png"),
            flags & 1 == 0,
        )
    }
    async fn app_icon(
        &self,
        app: String,
        size_px: u32,
        _scale: f64,
        appearance: u32,
        accent_rgb: u32,
    ) -> String {
        format!("/app-{app}-{size_px}-{appearance}-{accent_rgb}.png")
    }
    fn invalidate(&self, _uris: Vec<String>) {}
    #[zbus(signal)]
    async fn ready(
        emitter: &SignalEmitter<'_>,
        uri: &str,
        size_px: u32,
        path: &str,
    ) -> zbus::Result<()>;
}

pub struct MockShare;

#[zbus::interface(name = "dev.frascella.Silica.Share1")]
impl MockShare {
    fn targets(&self, _uris: Vec<String>) -> Vec<(String, String, String)> {
        vec![("copy".into(), "Copy".into(), "copy-symbol".into())]
    }
    fn share(&self, target: String, _uris: Vec<String>) -> bool {
        target == "copy"
    }
}

pub struct MockClipboard;

#[zbus::interface(name = "dev.frascella.Silica.Clipboard1")]
impl MockClipboard {
    fn history(&self) -> String {
        "[]".into()
    }
    fn copy(&self, id: u64) -> bool {
        id == 1
    }
    fn copy_image(&self, _path: &str) -> bool {
        true
    }
    fn remove(&self, _id: u64) -> bool {
        true
    }
    async fn clear(&self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        let _ = Self::changed(&emitter).await;
    }
    #[zbus(signal)]
    async fn changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

pub struct MockActions;

#[zbus::interface(name = "dev.frascella.Silica.Actions1")]
impl MockActions {
    fn list(&self) -> String {
        "{\"actions\":[]}".into()
    }
}

pub struct MockSound;

#[zbus::interface(name = "dev.frascella.Silica.Sound1")]
impl MockSound {
    fn play_sound(&self, name: &str) -> fdo::Result<()> {
        if name.is_empty() {
            return Err(fdo::Error::InvalidArgs("name".into()));
        }
        Ok(())
    }
    fn play_alert(&self, _name: &str) -> fdo::Result<()> {
        Ok(())
    }
    fn alert_sounds(&self) -> Vec<String> {
        vec!["Glass".into()]
    }
    fn play_volume_feedback(&self) -> bool {
        true
    }
    fn ui_sounds_enabled(&self) -> bool {
        true
    }
    async fn set_ui_sounds_enabled(
        &self,
        enabled: bool,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        Self::ui_sounds_changed(&emitter, enabled).await?;
        Ok(())
    }
    #[zbus(signal)]
    async fn ui_sounds_changed(emitter: &SignalEmitter<'_>, enabled: bool) -> zbus::Result<()>;
}

pub struct MockAuth;

#[zbus::interface(name = "dev.frascella.Silica.Authentication1")]
impl MockAuth {
    fn pending(
        &self,
    ) -> Vec<(
        u32,
        String,
        String,
        String,
        String,
        Vec<(String, String)>,
        String,
    )> {
        vec![(
            1,
            "org.x.install".into(),
            "Install?".into(),
            "icon".into(),
            "app".into(),
            vec![("ada".into(), "Ada".into())],
            "ada".into(),
        )]
    }
    async fn respond(
        &self,
        id: u32,
        _user: String,
        password: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<bool> {
        let right = password == "right";
        if right {
            Self::closed(&emitter, id, "success").await?;
        }
        Ok(right)
    }
    fn cancel(&self, _id: u32) -> fdo::Result<()> {
        Ok(())
    }
    #[zbus(property)]
    fn registered(&self) -> bool {
        true
    }
    #[zbus(signal)]
    async fn requested(
        emitter: &SignalEmitter<'_>,
        request: (
            u32,
            String,
            String,
            String,
            String,
            Vec<(String, String)>,
            String,
        ),
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn closed(emitter: &SignalEmitter<'_>, id: u32, outcome: &str) -> zbus::Result<()>;
}

pub struct MockShellNotifications;

#[zbus::interface(name = "dev.frascella.Silica.Notifications1")]
impl MockShellNotifications {
    fn list(&self) -> Vec<Notification> {
        vec![sample_notification()]
    }
    async fn dismiss(
        &self,
        id: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        Self::removed(&emitter, id, 2).await?;
        Ok(())
    }
    async fn clear(&self) -> fdo::Result<()> {
        Ok(())
    }
    async fn invoke_action(&self, _id: u32, _action_key: String) -> fdo::Result<()> {
        Ok(())
    }
    async fn reply(
        &self,
        id: u32,
        text: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        Self::added(
            &emitter,
            Notification {
                summary: text,
                id,
                ..sample_notification()
            },
        )
        .await?;
        Ok(())
    }
    #[zbus(signal)]
    async fn added(emitter: &SignalEmitter<'_>, notification: Notification) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn removed(emitter: &SignalEmitter<'_>, id: u32, reason: u32) -> zbus::Result<()>;
}

pub fn sample_notification() -> Notification {
    Notification {
        id: 4,
        app_name: "Mail".into(),
        app_icon: "mail".into(),
        summary: "Hi".into(),
        body: "Body".into(),
        actions: vec!["default".into(), "Open".into()],
        urgency: 1,
        category: "email".into(),
        desktop_entry: "dev.frascella.Mail".into(),
        image_path: String::new(),
        transient: false,
        resident: true,
        timestamp_ms: 1_700_000_000_000,
        expire_timeout: -1,
    }
}

pub struct MockFdoNotifications;

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl MockFdoNotifications {
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        _app_name: String,
        replaces_id: u32,
        _app_icon: String,
        _summary: String,
        _body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> fdo::Result<u32> {
        Ok(replaces_id + actions.len() as u32 + hints.len() as u32 + expire_timeout as u32)
    }
    async fn close_notification(
        &self,
        id: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        Self::notification_closed(&emitter, id, 3).await?;
        Self::action_invoked(&emitter, id, "default").await?;
        Self::notification_replied(&emitter, id, "ok").await?;
        Ok(())
    }
    fn get_capabilities(&self) -> Vec<String> {
        vec!["body".into(), "actions".into()]
    }
    fn get_server_information(&self) -> (String, String, String, String) {
        ("Silica".into(), "silica".into(), "1".into(), "1.2".into())
    }
    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn notification_replied(
        emitter: &SignalEmitter<'_>,
        id: u32,
        text: &str,
    ) -> zbus::Result<()>;
}

// ---- Portals -------------------------------------------------------------------------------

pub struct MockFileChooser;

#[zbus::interface(name = "org.freedesktop.portal.FileChooser")]
impl MockFileChooser {
    async fn open_file(
        &self,
        parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> fdo::Result<zbus::zvariant::OwnedObjectPath> {
        request_path(&parent_window, &title, options.len())
    }
    async fn save_file(
        &self,
        parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> fdo::Result<zbus::zvariant::OwnedObjectPath> {
        request_path(&parent_window, &title, options.len())
    }
    async fn save_files(
        &self,
        parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> fdo::Result<zbus::zvariant::OwnedObjectPath> {
        request_path(&parent_window, &title, options.len())
    }
    #[zbus(property)]
    fn version(&self) -> u32 {
        4
    }
}

fn request_path(
    parent: &str,
    title: &str,
    options: usize,
) -> fdo::Result<zbus::zvariant::OwnedObjectPath> {
    zbus::zvariant::OwnedObjectPath::try_from(format!(
        "/org/freedesktop/portal/desktop/request/{}_{}_{options}",
        parent.len(),
        title.len()
    ))
    .map_err(|e| fdo::Error::Failed(e.to_string()))
}

pub struct MockOpenUri;

#[zbus::interface(name = "org.freedesktop.portal.OpenURI")]
impl MockOpenUri {
    #[zbus(name = "OpenURI")]
    async fn open_uri(
        &self,
        parent_window: String,
        uri: String,
        options: HashMap<String, OwnedValue>,
    ) -> fdo::Result<zbus::zvariant::OwnedObjectPath> {
        request_path(&parent_window, &uri, options.len())
    }
    async fn open_file(
        &self,
        parent_window: String,
        fd: zbus::zvariant::OwnedFd,
        options: HashMap<String, OwnedValue>,
    ) -> fdo::Result<zbus::zvariant::OwnedObjectPath> {
        let _ = fd;
        request_path(&parent_window, "file", options.len())
    }
    async fn open_directory(
        &self,
        parent_window: String,
        fd: zbus::zvariant::OwnedFd,
        options: HashMap<String, OwnedValue>,
    ) -> fdo::Result<zbus::zvariant::OwnedObjectPath> {
        let _ = fd;
        request_path(&parent_window, "dir", options.len())
    }
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }
}

pub struct MockPortalSettings;

#[zbus::interface(name = "org.freedesktop.portal.Settings")]
impl MockPortalSettings {
    fn read_all(&self, namespaces: Vec<String>) -> HashMap<String, HashMap<String, OwnedValue>> {
        namespaces
            .into_iter()
            .map(|ns| {
                (
                    ns,
                    HashMap::from([("color-scheme".to_owned(), owned(Value::U32(1)))]),
                )
            })
            .collect()
    }
    fn read(&self, namespace: &str, key: &str) -> fdo::Result<OwnedValue> {
        if namespace == "org.freedesktop.appearance" && key == "color-scheme" {
            Ok(owned(Value::Value(Box::new(Value::U32(1)))))
        } else {
            Err(fdo::Error::Failed("unknown".into()))
        }
    }
    #[zbus(property)]
    fn version(&self) -> u32 {
        2
    }
    #[zbus(signal)]
    pub async fn setting_changed(
        emitter: &SignalEmitter<'_>,
        namespace: &str,
        key: &str,
        value: Value<'_>,
    ) -> zbus::Result<()>;
}

pub struct MockRequest;

#[zbus::interface(name = "org.freedesktop.portal.Request")]
impl MockRequest {
    async fn close(&self) {}
    #[zbus(signal)]
    pub async fn response(
        emitter: &SignalEmitter<'_>,
        response: u32,
        results: HashMap<String, Value<'_>>,
    ) -> zbus::Result<()>;
}
