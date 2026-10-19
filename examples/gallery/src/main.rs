//! The gallery: every Vetro component in one window, for the dev VM and screenshots.

use vetro_lib::amalfi::button::{PushButton, PushVariant};
use vetro_lib::amalfi::foundation::ControlSize;
use vetro_lib::core::menu::{Menu, MenuItem, PopupKind};
use vetro_lib::core::{Alignment, Prop, Signal, Stack, Ui, Value, WindowKind};
use vetro_lib::geom::Size;

fn build(ui: &mut Ui) {
    let win = ui.create_window(WindowKind::Normal, Size::new(720.0, 480.0));
    ui.set_title(win, "Vetro Gallery");
    let column = ui.create(Stack::column(16.0).aligned(Alignment::LEADING));
    let _ = ui.set(column, Prop::Padding, Value::from(24.0));
    let _ = ui.set(column, Prop::MaxWidth, Value::from(f32::INFINITY));
    let _ = ui.set(column, Prop::MaxHeight, Value::from(f32::INFINITY));
    let _ = ui.set(column, Prop::FrameAlignment, Value::Int(0));
    for size in [
        ControlSize::Mini,
        ControlSize::Small,
        ControlSize::Regular,
        ControlSize::Large,
    ] {
        let row = ui.create(Stack::row(12.0));
        for variant in [
            PushVariant::Primary,
            PushVariant::Secondary,
            PushVariant::Destructive,
            PushVariant::Borderless,
            PushVariant::Glass,
        ] {
            let b = ui.create(PushButton::new("Button").variant(variant).size(size));
            ui.connect(b, Signal::Activated, move |_, _, _| {
                println!("activated {variant:?} {size:?}");
            });
            ui.add_child(row, b);
        }
        ui.add_child(column, row);
    }
    let quit = ui.create(PushButton::new("Quit").variant(PushVariant::Primary));
    ui.connect(quit, Signal::Activated, |ui, _, _| {
        ui.request(vetro_lib::core::Request::Quit)
    });
    ui.add_child(column, quit);
    ui.set_content(win, column);
    ui.connect(column, Signal::ContextMenu, |ui, v, at| {
        let p = match at {
            Value::Point(p) => *p,
            _ => vetro_lib::geom::Point::ZERO,
        };
        let origin = ui.tree.window_origin(v);
        let _ = ui.popup_menu(
            v,
            vetro_lib::geom::Rect::new(p.x - origin.x, p.y - origin.y, 0.0, 0.0),
            PopupKind::Context,
            0,
            vec![
                MenuItem::new("Copy").shortcut("cmd+c"),
                MenuItem::new("Paste").shortcut("cmd+v"),
                MenuItem::separator(),
                MenuItem::new("Toggle Appearance").action(toggle_dark),
            ],
            None,
        );
    });
    ui.set_menu_bar(vec![
        Menu {
            title: "Vetro Gallery".into(),
            role: Some("app".into()),
            items: vec![
                MenuItem::new("About Vetro Gallery").role("about"),
                MenuItem::separator(),
                MenuItem::new("Quit Vetro Gallery")
                    .shortcut("cmd+q")
                    .role("quit")
                    .action(|ui| ui.request(vetro_lib::core::Request::Quit)),
            ],
        },
        Menu {
            title: "View".into(),
            role: None,
            items: vec![
                MenuItem::new("Toggle Appearance")
                    .shortcut("cmd+shift+d")
                    .action(toggle_dark),
            ],
        },
        Menu {
            title: "Window".into(),
            role: Some("window".into()),
            items: vec![MenuItem::new("Minimize").shortcut("cmd+m")],
        },
    ]);
    ui.show_window(win);
}

fn toggle_dark(ui: &mut Ui) {
    let mut a = ui.appearance;
    a.dark = !a.dark;
    ui.set_appearance(a);
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    if let Err(e) = vetro_lib::platform::run("dev.silica.VetroGallery", build) {
        eprintln!("vetro-gallery: {e}");
        std::process::exit(1);
    }
}
