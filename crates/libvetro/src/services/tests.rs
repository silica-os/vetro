//! Integration tests over a private `dbus-daemon` and the mock servers of `mock.rs`.

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use futures_lite::StreamExt;
use futures_lite::future::{block_on, or};
use zbus::Connection;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{Fd, Value};

use super::mock::*;
use super::*;

const DAEMON: &str = "dev.frascella.Silica";
const PORTAL: &str = "org.freedesktop.portal.Desktop";

/// Runs `future`, failing the test after 15 seconds instead of hanging.
fn run<T>(future: impl Future<Output = T>) -> T {
    block_on(or(future, async {
        async_io::Timer::after(Duration::from_secs(15)).await;
        panic!("test timed out");
    }))
}

/// Runs a watcher beside a script; the script's end ends the test.
async fn drive<T>(watcher: impl Future<Output = ()>, script: impl Future<Output = T>) -> T {
    or(
        async {
            watcher.await;
            panic!("the watcher ended");
        },
        script,
    )
    .await
}

/// Waits until the observable's value satisfies `ready`.
async fn until<T: Clone + PartialEq + 'static>(
    observable: &Observable<T>,
    ready: impl Fn(&T) -> bool,
) -> T {
    let mut changes = observable.changes();
    loop {
        let value = observable.get();
        if ready(&value) {
            return value;
        }
        changes.next().await;
    }
}

fn dict(items: Vec<(&str, Value<'_>)>) -> Dict {
    std::sync::Arc::new(std::sync::Mutex::new(
        items
            .into_iter()
            .map(|(k, v)| (k.to_owned(), owned(v)))
            .collect(),
    ))
}

/// The daemon: all Silica interfaces, under both of its names.
async fn serve_daemon(bus: &TestBus, system: Dict, settings: Dict) -> Connection {
    zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .serve_at(SETTINGS_PATH, MockSettings { values: settings })
        .unwrap()
        .serve_at(SYSTEM_PATH, MockSystem { state: system })
        .unwrap()
        .serve_at("/dev/frascella/Silica/Apps1", MockApps)
        .unwrap()
        .serve_at("/dev/frascella/Silica/Trust1", MockTrust)
        .unwrap()
        .serve_at("/dev/frascella/Silica/Volumes1", MockVolumes)
        .unwrap()
        .serve_at("/dev/frascella/Silica/Updates1", MockUpdates)
        .unwrap()
        .serve_at("/dev/frascella/Silica/Thumbnails1", MockThumbnails)
        .unwrap()
        .serve_at("/dev/frascella/Silica/Share1", MockShare)
        .unwrap()
        .serve_at("/dev/frascella/Silica/Clipboard1", MockClipboard)
        .unwrap()
        .serve_at("/dev/frascella/Silica/Actions1", MockActions)
        .unwrap()
        .serve_at("/dev/frascella/Silica/Sound1", MockSound)
        .unwrap()
        .serve_at("/dev/frascella/Silica/Authentication1", MockAuth)
        .unwrap()
        .serve_at(
            "/dev/frascella/Silica/Notifications1",
            MockShellNotifications,
        )
        .unwrap()
        .serve_at("/org/freedesktop/Notifications", MockFdoNotifications)
        .unwrap()
        .name(DAEMON)
        .unwrap()
        .name("org.freedesktop.Notifications")
        .unwrap()
        .build()
        .await
        .unwrap()
}

macro_rules! bus {
    () => {
        match TestBus::start() {
            Some(bus) => bus,
            None => {
                eprintln!("dbus-daemon not available: test skipped");
                return;
            }
        }
    };
}

#[test]
fn system_and_settings_proxies_match_the_server_signatures() {
    let bus = bus!();
    run(async {
        let _daemon = serve_daemon(
            &bus,
            dict(vec![]),
            dict(vec![("dock.autohide", Value::Bool(true))]),
        )
        .await;
        let client = bus.connect().await;

        let settings = Settings1Proxy::new(&client).await.unwrap();
        assert!(
            settings
                .get_all()
                .await
                .unwrap()
                .contains_key("dock.autohide")
        );
        assert_eq!(
            settings
                .get("dock.autohide")
                .await
                .unwrap()
                .downcast_ref::<bool>()
                .ok(),
            Some(true)
        );
        let mut changed = settings.receive_changed().await.unwrap();
        settings
            .set("dock.tileSize", &Value::I64(64))
            .await
            .unwrap();
        let signal = changed.next().await.unwrap();
        let args = signal.args().unwrap();
        assert_eq!(*args.key(), "dock.tileSize");
        assert_eq!(crate::services::wire::as_i64(args.value()), Some(64));
        let many = Value::from("x");
        settings
            .set_many(HashMap::from([("a.b", &many)]))
            .await
            .unwrap();
        settings.reset("a.b").await.unwrap();

        let system = System1Proxy::new(&client).await.unwrap();
        assert!(system.state().await.unwrap().is_empty());
        system.set_wifi_enabled(true).await.unwrap();
        system.set_bluetooth_powered(true).await.unwrap();
        system.set_volume(0.5).await.unwrap();
        system.set_muted(false).await.unwrap();
        system.set_brightness(0.5).await.unwrap();
        system.set_keyboard_brightness(0.5).await.unwrap();
        assert_eq!(
            system.wifi_networks().await.unwrap(),
            vec![types::WifiNetwork {
                name: "Home".into(),
                signal: 80,
                protected: true,
                saved: true,
                in_use: true
            }]
        );
        system.connect_wifi("Home").await.unwrap();
        assert_eq!(system.bluetooth_devices().await.unwrap()[0].name, "Buds");
        system.connect_bluetooth("AA:BB", true).await.unwrap();
        assert_eq!(system.audio_outputs().await.unwrap()[0].id, 51);
        system.set_audio_output(51).await.unwrap();
        system.set_do_not_disturb(true).await.unwrap();
        system.set_appearance_mode("auto").await.unwrap();
        system
            .set_text_replacements(&[("omw", "on my way")])
            .await
            .unwrap();
        assert_eq!(system.outputs().await.unwrap(), "[]");
        assert_eq!(system.configure_output("{}").await.unwrap(), "{}");
        let night = Value::Bool(true);
        system
            .set_night_light(HashMap::from([("enabled", &night)]))
            .await
            .unwrap();
        system.set_focus("").await.unwrap();
        system.suspend().await.unwrap();
        assert!(system.unlock("right").await.unwrap());
        assert!(!system.unlock("wrong").await.unwrap());
        system.report_compositor("{}").await.unwrap();
        let mut changes = system.receive_changed().await.unwrap();
        let accent = Value::from("red");
        system
            .set_look(HashMap::from([("accent", &accent)]))
            .await
            .unwrap();
        let state = changes.next().await.unwrap().args().unwrap().state;
        assert_eq!(
            crate::services::wire::dict_str(&state, "look.accent"),
            Some("red")
        );
    });
}

#[test]
fn apps_trust_volumes_updates_proxies_match() {
    let bus = bus!();
    run(async {
        let _daemon = serve_daemon(&bus, dict(vec![]), dict(vec![])).await;
        let client = bus.connect().await;

        let apps = Apps1Proxy::new(&client).await.unwrap();
        let list = apps.list().await.unwrap();
        assert_eq!(
            (list[0].id.as_str(), list[0].kind.as_str()),
            ("dev.frascella.Clock", "app")
        );
        assert!(list[0].properties.contains_key("icon"));
        assert!(apps.get("x").await.unwrap().contains_key("id"));
        assert_eq!(
            apps.apps_for("text/plain").await.unwrap(),
            vec!["opens-text/plain"]
        );
        assert_eq!(
            apps.default_for("text/plain").await.unwrap(),
            "dev.frascella.Preview"
        );
        let mut defaults = apps.receive_default_changed().await.unwrap();
        apps.set_default("text/plain", "dev.frascella.TextEditor")
            .await
            .unwrap();
        let args = defaults.next().await.unwrap();
        assert_eq!(*args.args().unwrap().id(), "dev.frascella.TextEditor");
        let option = Value::Bool(true);
        assert_eq!(
            apps.open("ab", &["file:///a"], HashMap::from([("k", &option)]))
                .await
                .unwrap(),
            4
        );
        assert_eq!(apps.open_uris(&["a", "b"]).await.unwrap(), 2);
        let mut changed = apps.receive_changed().await.unwrap();
        apps.note_launched("dev.frascella.Clock").await.unwrap();
        assert_eq!(
            *changed.next().await.unwrap().args().unwrap().id(),
            "dev.frascella.Clock"
        );
        assert!(apps.uninstall("x").await.unwrap());
        let _ = apps.receive_added().await.unwrap();
        let _ = apps.receive_removed().await.unwrap();

        let trust = Trust1Proxy::new(&client).await.unwrap();
        assert_eq!(trust.assess("/a").await.unwrap().0, "confirm");
        assert_eq!(trust.prompt("/a").await.unwrap(), "open");
        assert_eq!(trust.suggest_move("/a").await.unwrap(), "cancel");
        assert!(trust.set_sandbox("/a", true).await.unwrap());
        trust.register_agent().await.unwrap();
        assert_eq!(trust.pending().await.unwrap()[0].verdict, "block");
        assert_eq!(trust.blocked().await.unwrap()[0].2, 99);
        assert!(trust.open_anyway("/a").await.unwrap());
        let mut closed = trust.receive_prompt_closed().await.unwrap();
        trust.answer(3, "trash").await.unwrap();
        let signal = closed.next().await.unwrap();
        assert_eq!(
            (signal.args().unwrap().id, *signal.args().unwrap().choice()),
            (3, "trash")
        );
        let _ = trust.receive_prompt_requested().await.unwrap();

        let volumes = Volumes1Proxy::new(&client).await.unwrap();
        assert!(
            volumes
                .inspect("a.sxv")
                .await
                .unwrap()
                .contains_key("signature")
        );
        let mut attached = volumes.receive_attached().await.unwrap();
        let mount = volumes.attach("a", HashMap::new()).await.unwrap();
        assert_eq!(mount, "/Volumes/a");
        assert_eq!(
            *attached.next().await.unwrap().args().unwrap().mountpoint(),
            "/Volumes/a"
        );
        let mut detached = volumes.receive_detached().await.unwrap();
        volumes.detach(&mount).await.unwrap();
        assert!(detached.next().await.is_some());
        assert_eq!(volumes.list().await.unwrap()[0].image, "/home/x.sxv");

        let updates = Updates1Proxy::new(&client).await.unwrap();
        let found = updates.check().await.unwrap();
        assert_eq!(
            (found[0].id.as_str(), found[0].available.as_str()),
            ("os", "1.1")
        );
        let mut progress = updates.receive_progress().await.unwrap();
        let mut finished = updates.receive_finished().await.unwrap();
        assert_eq!(updates.update("os").await.unwrap(), "staged");
        assert_eq!(progress.next().await.unwrap().args().unwrap().fraction, 0.5);
        assert_eq!(
            *finished.next().await.unwrap().args().unwrap().result(),
            "staged"
        );
        assert_eq!(
            updates.update_all().await.unwrap(),
            vec![("os".to_owned(), "staged".to_owned())]
        );
    });
}

#[test]
fn thumbnails_share_clipboard_sound_auth_notification_proxies_match() {
    let bus = bus!();
    run(async {
        let _daemon = serve_daemon(&bus, dict(vec![]), dict(vec![])).await;
        let client = bus.connect().await;

        let thumbs = Thumbnails1Proxy::new(&client).await.unwrap();
        let mut ready = thumbs.receive_ready().await.unwrap();
        assert_eq!(thumbs.thumbnail("file:///a.png", 128).await.unwrap(), "");
        assert_eq!(
            *ready.next().await.unwrap().args().unwrap().path(),
            "/cache/t.png"
        );
        assert_eq!(
            thumbs.icon("file:///a", 64, 2.0, 1).await.unwrap(),
            ("/icon-64-2-1.png".to_owned(), false)
        );
        assert_eq!(
            thumbs
                .app_icon("dev.frascella.Clock", 128, 1.0, 6, 0xFF8800)
                .await
                .unwrap(),
            "/app-dev.frascella.Clock-128-6-16746496.png"
        );
        thumbs.invalidate(&["file:///a"]).await.unwrap();

        let share = Share1Proxy::new(&client).await.unwrap();
        assert_eq!(share.targets(&["file:///a"]).await.unwrap()[0].id, "copy");
        assert!(share.share("copy", &["file:///a"]).await.unwrap());

        let clipboard = Clipboard1Proxy::new(&client).await.unwrap();
        assert_eq!(clipboard.history().await.unwrap(), "[]");
        assert!(clipboard.copy(1).await.unwrap());
        assert!(clipboard.copy_image("/a.png").await.unwrap());
        assert!(clipboard.remove(1).await.unwrap());
        let mut changed = clipboard.receive_changed().await.unwrap();
        clipboard.clear().await.unwrap();
        assert!(changed.next().await.is_some());

        let actions = Actions1Proxy::new(&client).await.unwrap();
        assert!(actions.list().await.unwrap().contains("actions"));

        let sound = Sound1Proxy::new(&client).await.unwrap();
        sound.play_sound("trash-move").await.unwrap();
        assert!(sound.play_sound("").await.is_err());
        sound.play_alert("Glass").await.unwrap();
        assert_eq!(sound.alert_sounds().await.unwrap(), vec!["Glass"]);
        assert!(sound.play_volume_feedback().await.unwrap());
        assert!(sound.ui_sounds_enabled().await.unwrap());
        let mut toggled = sound.receive_ui_sounds_changed().await.unwrap();
        sound.set_ui_sounds_enabled(false).await.unwrap();
        assert!(!toggled.next().await.unwrap().args().unwrap().enabled);

        let auth = Authentication1Proxy::new(&client).await.unwrap();
        assert!(auth.registered().await.unwrap());
        let pending = auth.pending().await.unwrap();
        assert_eq!(pending[0].users, vec![("ada".to_owned(), "Ada".to_owned())]);
        assert_eq!(pending[0].default_user, "ada");
        let mut closed = auth.receive_closed().await.unwrap();
        assert!(!auth.respond(1, "ada", "wrong").await.unwrap());
        assert!(auth.respond(1, "ada", "right").await.unwrap());
        assert_eq!(
            *closed.next().await.unwrap().args().unwrap().outcome(),
            "success"
        );
        auth.cancel(1).await.unwrap();
        let _ = auth.receive_requested().await.unwrap();

        let shell = Notifications1Proxy::new(&client).await.unwrap();
        assert_eq!(shell.list().await.unwrap(), vec![sample_notification()]);
        let mut removed = shell.receive_removed().await.unwrap();
        shell.dismiss(4).await.unwrap();
        assert_eq!(removed.next().await.unwrap().args().unwrap().reason, 2);
        let mut added = shell.receive_added().await.unwrap();
        shell.reply(9, "typed").await.unwrap();
        let notification = added.next().await.unwrap().args().unwrap().notification;
        assert_eq!(
            (notification.id, notification.summary.as_str()),
            (9, "typed")
        );
        shell.clear().await.unwrap();
        shell.invoke_action(4, "default").await.unwrap();

        let fdo = FdoNotificationsProxy::new(&client).await.unwrap();
        let urgency = Value::U8(2);
        let id = fdo
            .notify(
                "Mail",
                1,
                "mail",
                "Hi",
                "Body",
                &["default", "Open"],
                HashMap::from([("urgency", &urgency)]),
                5,
            )
            .await
            .unwrap();
        assert_eq!(id, 1 + 2 + 1 + 5);
        assert_eq!(
            fdo.get_capabilities().await.unwrap(),
            vec!["body", "actions"]
        );
        assert_eq!(fdo.get_server_information().await.unwrap().0, "Silica");
        let mut closed = fdo.receive_notification_closed().await.unwrap();
        let mut invoked = fdo.receive_action_invoked().await.unwrap();
        let mut replied = fdo.receive_notification_replied().await.unwrap();
        fdo.close_notification(8).await.unwrap();
        assert_eq!(closed.next().await.unwrap().args().unwrap().reason, 3);
        assert_eq!(
            *invoked.next().await.unwrap().args().unwrap().action_key(),
            "default"
        );
        assert_eq!(*replied.next().await.unwrap().args().unwrap().text(), "ok");
    });
}

#[test]
fn portal_proxies_match() {
    let bus = bus!();
    run(async {
        let server = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at("/org/freedesktop/portal/desktop", MockFileChooser)
            .unwrap()
            .serve_at("/org/freedesktop/portal/desktop", MockOpenUri)
            .unwrap()
            .serve_at("/org/freedesktop/portal/desktop", MockPortalSettings)
            .unwrap()
            .serve_at("/org/freedesktop/portal/desktop/request/0_4_1", MockRequest)
            .unwrap()
            .name(PORTAL)
            .unwrap()
            .build()
            .await
            .unwrap();
        let client = bus.connect().await;

        let chooser = PortalFileChooserProxy::new(&client).await.unwrap();
        assert_eq!(chooser.version().await.unwrap(), 4);
        let multiple = Value::Bool(true);
        let options = || HashMap::from([("multiple", &multiple)]);
        let handle = chooser.open_file("", "Open", options()).await.unwrap();
        assert_eq!(
            handle.as_str(),
            "/org/freedesktop/portal/desktop/request/0_4_1"
        );
        assert!(
            chooser
                .save_file("x11:1", "Save", options())
                .await
                .unwrap()
                .as_str()
                .ends_with("5_4_1")
        );
        assert!(
            chooser
                .save_files("", "Save", options())
                .await
                .unwrap()
                .as_str()
                .ends_with("0_4_1")
        );

        let request = PortalRequestProxy::builder(&client)
            .destination(PORTAL)
            .unwrap()
            .path(handle.as_ref())
            .unwrap()
            .build()
            .await
            .unwrap();
        let mut responses = request.receive_response().await.unwrap();
        let emitter = SignalEmitter::new(&server, handle.as_ref()).unwrap();
        let uris = Value::from(vec!["file:///a"]);
        MockRequest::response(&emitter, 0, HashMap::from([("uris".to_owned(), uris)]))
            .await
            .unwrap();
        let response = responses.next().await.unwrap();
        let args = response.args().unwrap();
        assert_eq!(args.response, 0);
        assert!(args.results().contains_key("uris"));
        request.close().await.unwrap();

        let open = PortalOpenUriProxy::new(&client).await.unwrap();
        assert_eq!(open.version().await.unwrap(), 3);
        assert!(
            open.open_uri("", "https://x", HashMap::new())
                .await
                .unwrap()
                .as_str()
                .ends_with("0_9_0")
        );
        let file = std::fs::File::open("/dev/null").unwrap();
        assert!(
            open.open_file("", Fd::from(&file), HashMap::new())
                .await
                .unwrap()
                .as_str()
                .ends_with("0_4_0")
        );
        assert!(
            open.open_directory("", Fd::from(&file), HashMap::new())
                .await
                .unwrap()
                .as_str()
                .ends_with("0_3_0")
        );

        let portal = PortalSettingsProxy::new(&client).await.unwrap();
        assert_eq!(portal.version().await.unwrap(), 2);
        let all = portal
            .read_all(&["org.freedesktop.appearance"])
            .await
            .unwrap();
        assert!(all["org.freedesktop.appearance"].contains_key("color-scheme"));
        let scheme = portal
            .read("org.freedesktop.appearance", "color-scheme")
            .await
            .unwrap();
        assert_eq!(crate::services::wire::as_i64(&scheme), Some(1));
        let mut changes = portal.receive_setting_changed().await.unwrap();
        let emitter = SignalEmitter::new(&server, "/org/freedesktop/portal/desktop").unwrap();
        MockPortalSettings::setting_changed(
            &emitter,
            "org.freedesktop.appearance",
            "color-scheme",
            Value::U32(2),
        )
        .await
        .unwrap();
        let args = changes.next().await.unwrap();
        assert_eq!(*args.args().unwrap().key(), "color-scheme");
    });
}

#[test]
fn appearance_follows_the_daemon_live() {
    let bus = bus!();
    run(async {
        let client = bus.connect().await;
        let watcher = AppearanceWatcher::new();
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let log = std::rc::Rc::clone(&seen);
        watcher
            .appearance()
            .subscribe(move |a: &Appearance| log.borrow_mut().push(*a))
            .keep();
        drive(watcher.run(&client), async {
            // No daemon yet: the defaults, no failure.
            assert_eq!(watcher.appearance().get(), Appearance::default());

            let state = dict(vec![
                ("appearance.dark", Value::Bool(true)),
                ("look.accent", Value::from("red")),
                ("look.textScale", Value::F64(1.25)),
            ]);
            let daemon = serve_daemon(&bus, state, dict(vec![])).await;
            let a = until(watcher.appearance(), |a| a.dark).await;
            assert_eq!(a.accent, Some(crate::color::Color::rgb(0xFF, 0x52, 0x57)));
            assert_eq!(a.text_scale, 1.25);

            // A change through the daemon's own calls arrives as `Changed`.
            let system = System1Proxy::new(&client).await.unwrap();
            let on = Value::Bool(true);
            let scale = Value::F64(1.5);
            let green = Value::from("green");
            system
                .set_look(HashMap::from([
                    ("reduceMotion", &on),
                    ("increaseContrast", &on),
                    ("reduceTransparency", &on),
                    ("keyboardNavigation", &on),
                    ("textScale", &scale),
                    ("highlight", &green),
                ]))
                .await
                .unwrap();
            let a = until(watcher.appearance(), |a| a.reduce_motion).await;
            assert!(a.increase_contrast && a.reduce_transparency && a.full_keyboard_access);
            assert_eq!(a.text_scale, 1.5);
            assert_eq!(
                a.highlight,
                Some(crate::color::Color::rgb(0x62, 0xBA, 0x46))
            );
            assert!(a.dark);
            system.set_dark(false).await.unwrap();
            let a = until(watcher.appearance(), |a| !a.dark).await;
            // Dark off: the accent has its light tone.
            assert_eq!(a.accent, Some(crate::color::Color::rgb(0xE0, 0x39, 0x3E)));
            assert_eq!(watcher.look().get().double_click_ms, 300);

            // The daemon goes away: back to the defaults.
            daemon.graceful_shutdown().await;
            until(watcher.appearance(), |a| *a == Appearance::default()).await;
        })
        .await;
        assert!(seen.borrow().len() >= 4);
    });
}

#[test]
fn settings_watcher_gives_key_repeat_and_renderer() {
    let bus = bus!();
    run(async {
        let client = bus.connect().await;
        let watcher = SettingsWatcher::new();
        drive(watcher.run(&client), async {
            assert_eq!(watcher.key_repeat().get(), KeyRepeat::default());
            let values = dict(vec![
                ("keyboard.repeatDelay", Value::I64(500)),
                ("keyboard.repeatRate", Value::I64(30)),
                (
                    "keyboard.layouts",
                    Value::from(vec!["it", "de(nodeadkeys)"]),
                ),
                ("dock.autohide", Value::Bool(true)),
            ]);
            let daemon = serve_daemon(&bus, dict(vec![]), values).await;
            until(watcher.key_repeat(), |k| k.delay_ms == 500).await;
            assert_eq!(
                watcher.key_repeat().get(),
                KeyRepeat {
                    delay_ms: 500,
                    rate_hz: 30
                }
            );
            let all = watcher.settings().get();
            assert_eq!(all.get("dock.autohide"), Some(&SettingValue::Bool(true)));
            assert_eq!(
                all.get("keyboard.layouts"),
                Some(&SettingValue::Texts(vec![
                    "it".into(),
                    "de(nodeadkeys)".into()
                ]))
            );
            assert_eq!(watcher.renderer().get(), None);

            let proxy = Settings1Proxy::new(&client).await.unwrap();
            proxy
                .set("keyboard.repeatRate", &Value::I64(0))
                .await
                .unwrap();
            until(watcher.key_repeat(), |k| k.rate_hz == 0).await;
            proxy
                .set(RENDERER_KEY, &Value::from("software"))
                .await
                .unwrap();
            until(watcher.renderer(), |r| {
                *r == Some(RendererBackend::Software)
            })
            .await;

            daemon.graceful_shutdown().await;
            until(watcher.key_repeat(), |k| *k == KeyRepeat::default()).await;
            assert!(watcher.settings().get().is_empty());
        })
        .await;
    });
}

#[test]
fn facade_connects_lazily_and_reports_typed_errors() {
    let bus = bus!();
    run(async {
        // No bus at the address: a typed error, not a panic.
        let broken = Services::with_address("unix:path=/dev/shm/vetro-svc-missing/bus");
        assert!(matches!(broken.run().await, Err(Error::NoBus(_))));
        assert!(matches!(broken.system().await, Err(Error::NoBus(_))));
        assert_eq!(broken.appearance().get(), Appearance::default());

        // A bus without the daemon: calls fail with "service absent", watchers keep defaults.
        let services = Services::with_address(bus.address.clone());
        let error = services
            .system()
            .await
            .unwrap()
            .state()
            .await
            .map_err(Error::from)
            .unwrap_err();
        assert!(error.is_service_absent(), "{error:?}");

        drive(
            async {
                let _ = services.run().await;
            },
            async {
                let _daemon = serve_daemon(
                    &bus,
                    dict(vec![("look.reduceMotion", Value::Bool(true))]),
                    dict(vec![("keyboard.repeatDelay", Value::I64(800))]),
                )
                .await;
                until(services.appearance(), |a| a.reduce_motion).await;
                until(services.key_repeat(), |k| k.delay_ms == 800).await;
                let apps = services.apps().await.unwrap();
                assert_eq!(apps.list().await.unwrap().len(), 1);
                let _ = services.portal_settings().await.unwrap();
                let mut changes = services.key_repeat().changes();
                services
                    .settings_proxy()
                    .await
                    .unwrap()
                    .set("keyboard.repeatDelay", &Value::I64(900))
                    .await
                    .unwrap();
                assert_eq!(changes.next().await.delay_ms, 900);
            },
        )
        .await;
    });
}
