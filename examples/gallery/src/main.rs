//! The gallery: Vetro's components in one window, built with the declarative layer (so it
//! runs the whole stack: vetro → vetro-kit → the C ABI → libvetro), for the dev VM and
//! screenshots.

use vetro::prelude::*;
use vetro::{Animation, MenuEntry, Transition};

struct Gallery;

fn section(title: &str, content: Element) -> Element {
    column((
        label(title).text_style("Headline").color("Text/Secondary"),
        content,
    ))
    .spacing(8.0)
    .alignment(0)
}

impl Component for Gallery {
    fn body(&self, cx: &mut Cx) -> Element {
        let on = cx.state(|| true);
        let level = cx.state(|| 0.4f64);
        let count = cx.state(|| 3.0f64);
        let name = cx.state(String::new);
        let seg = cx.state(|| 0usize);
        let pick = cx.state(|| 1usize);
        let expanded = cx.state(|| false);
        let sheet = cx.state(|| false);
        let items = cx.state(|| vec![1u32, 2, 3]);
        let (i2, i3) = (items.clone(), items.clone());
        let s2 = sheet.clone();

        let buttons = row((
            button("Primary").style(0),
            button("Secondary").style(1),
            button("Destructive").style(2),
            button("Borderless").style(3),
            button("Glass").style(4),
            help_button(),
            icon_button("gear"),
            link("Link"),
        ))
        .spacing(12.0);
        let toggles = row((
            checkbox("Checkbox", &on.binding()),
            radio("Radio", on.get(), {
                let o = on.clone();
                move || o.set(true)
            }),
            switch(&on.binding()),
        ))
        .spacing(16.0);
        let values = row((
            slider(&level.binding(), 0.0, 1.0).width(200.0),
            stepper(&count.binding(), 0.0, 10.0, 1.0),
            label(format!("{:.0} %  ·  {}", level.get() * 100.0, count.get())),
        ))
        .spacing(16.0);
        let progress_row = row((
            progress(Some(level.get())).width(200.0),
            progress(None).width(120.0),
            spinner(),
        ))
        .spacing(16.0);
        let choices = row((
            segmented(&["Day", "Week", "Month"], &seg.binding()),
            pop_up(&["Small", "Medium", "Large"], &pick.binding()),
            pull_down("Actions", &["Rename", "Duplicate", "Move to Trash"], |_| {}),
        ))
        .spacing(16.0);
        let fields = row((
            text_field("Name", &name.binding()).width(180.0),
            secure_field("Password", &cx.state(String::new).binding()).width(160.0),
            search_field("Search", &cx.state(String::new).binding()).width(180.0),
        ))
        .spacing(12.0);
        let list = row((
            button("Add").on_activate(move || {
                i2.update(|v| {
                    let n = v.iter().max().copied().unwrap_or(0) + 1;
                    v.push(n)
                })
            }),
            button("Remove").on_activate(move || {
                i3.update(|v| {
                    v.pop();
                })
            }),
            row(for_each(
                items.get(),
                |n| *n,
                |n| {
                    button(format!("Item {n}"))
                        .transition(Transition::Scale)
                        .animation(Animation::Snappy)
                },
            ))
            .spacing(8.0),
        ))
        .spacing(12.0);
        let disclosure = disclosure_group(
            "More options",
            &expanded.binding(),
            column((
                label("Hidden until expanded"),
                checkbox("Another", &on.binding()),
            ))
            .spacing(8.0),
        );

        column((
            row((
                button("Quit")
                    .style(0)
                    .id("quit")
                    .on_activate(vetro::kit::quit),
                label("Vetro Gallery").text_style("Title 2"),
                spacer(),
                button("Open Sheet").on_activate(move || s2.set(true)),
            ))
            .spacing(16.0)
            .fill_width(),
            section("Buttons", buttons),
            section("Toggles", toggles),
            section("Sliders and steppers", values),
            section("Progress", progress_row),
            section("Choices", choices),
            section("Fields", fields),
            section("Animated list", list),
            section(
                "Disclosure and a list of 10 000 rows",
                row((
                    disclosure,
                    vetro::list(10_000, 24.0, |i| {
                        label(format!("Row {}", i + 1))
                            .padding_xy(8.0, 0.0)
                            .align(0)
                    })
                    .frame(260.0, 72.0),
                ))
                .spacing(24.0)
                .alignment(0),
            ),
            text_view("A text view for documents of any size.\nType here.")
                .height(80.0)
                .fill_width(),
        ))
        .spacing(18.0)
        .alignment(0)
        .padding(24.0)
        .fill()
        .align(0)
        .context_menu(vec![
            MenuEntry::item("Toggle Appearance", || {
                let _ = vetro::kit::set_dark(!vetro::kit::is_dark());
            }),
            MenuEntry::separator(),
            MenuEntry::item("Quit", vetro::kit::quit).shortcut("cmd+q"),
        ])
        .sheet(&sheet.binding(), (360.0, 160.0), {
            let s3 = sheet.clone();
            move || {
                let s4 = s3.clone();
                column((
                    label("A sheet").text_style("Title 3"),
                    label("Attached to the gallery window."),
                    button("Done")
                        .style(0)
                        .default_action()
                        .on_activate(move || s4.set(false)),
                ))
                .spacing(12.0)
                .padding(20.0)
            }
        })
    }
}

fn main() {
    let menus = || -> vetro::kit::Result<()> {
        let app = vetro::kit::Menu::new("Vetro Gallery", Some("app"))?
            .item("About Vetro Gallery", None, 1, 0)?
            .separator()?
            .item("Quit Vetro Gallery", Some("cmd+q"), 2, 0)?;
        let view = vetro::kit::Menu::new("View", None)?.item(
            "Toggle Appearance",
            Some("cmd+shift+d"),
            3,
            0,
        )?;
        let window = vetro::kit::Menu::new("Window", Some("window"))?;
        vetro::kit::set_menu_bar(vec![app, view, window], |id| match id {
            2 => vetro::kit::quit(),
            3 => {
                let _ = vetro::kit::set_dark(!vetro::kit::is_dark());
            }
            _ => {}
        })
    };
    let r = vetro::kit::run("dev.silica.VetroGallery", move || {
        let _ = menus();
        if let Ok(root) = vetro::window("Vetro Gallery", (900.0, 800.0), || Gallery) {
            // The window lives as long as the process.
            std::mem::forget(root);
        }
    });
    if let Err(e) = r {
        eprintln!("vetro-gallery: {e}");
        std::process::exit(1);
    }
}
