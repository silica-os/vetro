//! The gallery: Vetro's components in one window, built with the declarative layer (so it
//! runs the whole stack: vetro → vetro-kit → the C ABI → libvetro), for the dev VM and
//! screenshots.

use vetro::prelude::*;
use vetro::{Animation, MenuEntry, Transition};

/// The window's content; `start` is the page shown first.
struct Gallery {
    start: usize,
}

/// The controls page.
struct Controls;

/// The collections page: a list box, a table, an outline and an icon grid.
struct Collections;

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
        let start = self.start;
        let page = cx.state(move || start);
        let sheet = cx.state(|| false);
        let s2 = sheet.clone();
        let content = if page.get() == 0 {
            component(Controls)
        } else {
            component(Collections)
        };
        column((
            row((
                button("Quit")
                    .style(0)
                    .id("quit")
                    .on_activate(vetro::kit::quit),
                label("Vetro Gallery").text_style("Title 2"),
                spacer(),
                segmented(&["Controls", "Collections"], &page.binding()).id("pages"),
                spacer(),
                button("Open Sheet").on_activate(move || s2.set(true)),
            ))
            .spacing(16.0)
            .fill_width(),
            content.fill(),
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

impl Component for Controls {
    fn body(&self, cx: &mut Cx) -> Element {
        let on = cx.state(|| true);
        let level = cx.state(|| 0.4f64);
        let count = cx.state(|| 3.0f64);
        let name = cx.state(String::new);
        let seg = cx.state(|| 0usize);
        let pick = cx.state(|| 1usize);
        let expanded = cx.state(|| false);
        let items = cx.state(|| vec![1u32, 2, 3]);
        let (i2, i3) = (items.clone(), items.clone());

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
            section("Buttons", buttons),
            section("Toggles", toggles),
            section("Sliders and steppers", values),
            section("Progress", progress_row),
            section("Choices", choices),
            section("Fields", fields),
            section("Animated list", list),
            section("Disclosure", disclosure),
            text_view("A text view for documents of any size.\nType here.")
                .height(80.0)
                .fill_width(),
        ))
        .spacing(18.0)
        .alignment(0)
        .fill()
        .align(0)
    }
}

/// A file of the table demo.
#[derive(Clone)]
struct File {
    name: String,
    size: u64,
    kind: &'static str,
}

fn files() -> Vec<File> {
    let kinds = ["Document", "Image", "Folder", "Archive", "Text"];
    (0..2000u64)
        .map(|i| File {
            name: format!("File {:04}", i + 1),
            size: (i * 7919) % 100_000,
            kind: kinds.get((i % 5) as usize).copied().unwrap_or("Document"),
        })
        .collect()
}

fn cell_label(text: String, secondary: bool) -> Element {
    let l = label(text).align(3);
    if secondary {
        l.text_style("Callout").color("Text/Secondary")
    } else {
        l
    }
}

impl Component for Collections {
    fn body(&self, cx: &mut Cx) -> Element {
        let data = cx.state(files);
        let sort = cx.state(|| None::<(usize, bool)>);
        let picked = cx.state(|| vec![2u64]);
        // The rows in the sorted order.
        let mut order: Vec<usize> = (0..data.get().len()).collect();
        if let Some((col, asc)) = sort.get() {
            let d = data.get();
            order.sort_by(|a, b| {
                let (Some(x), Some(y)) = (d.get(*a), d.get(*b)) else {
                    return std::cmp::Ordering::Equal;
                };
                let o = match col {
                    0 => x.name.cmp(&y.name),
                    1 => x.size.cmp(&y.size),
                    _ => x.kind.cmp(y.kind),
                };
                if asc { o } else { o.reverse() }
            });
        }
        let order = std::rc::Rc::new(order);
        let rows = data.get().len();
        let (d2, o2) = (data.clone(), order.clone());
        let (d3, o3) = (data.clone(), order.clone());
        let s2 = sort.clone();
        let p2 = picked.clone();
        let table_view = table(
            vec![
                Column::new("Name", 260.0).editable(),
                Column::new("Size", 100.0).align(2),
                Column::new("Kind", 120.0),
            ],
            rows,
            move |r, c| {
                let d = d2.get();
                let Some(f) = o2.get(r).and_then(|i| d.get(*i)) else {
                    return label("");
                };
                match c {
                    0 => row((
                        symbol(if f.kind == "Folder" { "folder" } else { "doc" })
                            .color("Accent/Amalfi"),
                        label(f.name.clone()),
                    ))
                    .spacing(4.0)
                    .align(3),
                    1 => cell_label(format!("{} KB", f.size), true).align(5),
                    _ => cell_label(f.kind.to_string(), true),
                }
            },
        )
        .alternating(true)
        .sorted(sort.get())
        .on_sort(move |s| s2.set(s))
        .on_edit(move |r, _, text| {
            let i = o3.get(r as usize).copied();
            d3.update(|d| {
                if let Some(f) = i.and_then(|i| d.get_mut(i)) {
                    f.name = text.clone();
                }
            });
        })
        .selection(&picked.get())
        .on_select(move |keys| p2.set(keys))
        .id("table")
        .fill_width()
        .height(220.0);

        let tree = outline(
            vec![Column::new("Folder", 200.0), Column::new("Items", 60.0)],
            |parent| match parent {
                None => vec![1, 2, 3],
                Some(n) if n < 1000 => (1..=4).map(|k| n * 10 + k).collect(),
                Some(_) => Vec::new(),
            },
            |n| n < 1000,
            |n, c| {
                if c == 0 {
                    row((
                        symbol(if n < 1000 { "folder" } else { "doc" }).color("Accent/Amalfi"),
                        label(format!("Item {n}")),
                    ))
                    .spacing(4.0)
                    .align(3)
                } else {
                    cell_label(if n < 1000 { "4".into() } else { "—".into() }, true)
                }
            },
        )
        .style(2)
        .id("outline")
        .width(380.0)
        .height(220.0);

        let tiles = collection(500, (104.0, 104.0), |i| {
            column((
                symbol(if i % 3 == 0 { "folder" } else { "doc" })
                    .color("Accent/Amalfi")
                    .font_size(48.0)
                    .frame(64.0, 64.0),
                label(format!("Item {}", i + 1))
                    .text_style("Callout")
                    .text_align(1)
                    .frame(88.0, 16.0),
            ))
            .spacing(10.0)
            .align(1)
        })
        .id("grid")
        .fill_width()
        .height(220.0);

        column((
            section(
                "A list box of 10 000 rows",
                vetro::list(10_000, 24.0, |i| {
                    label(format!("Row {}", i + 1))
                        .padding_xy(8.0, 0.0)
                        .align(3)
                })
                .style(2)
                .id("list")
                .fill_width()
                .height(120.0),
            ),
            section(
                &format!(
                    "Table of {} files: click a title to sort, Return renames ({} selected)",
                    rows,
                    picked.get().len()
                ),
                table_view,
            ),
            row((
                section("Outline", tree),
                section("Collection of 500 tiles", tiles),
            ))
            .spacing(24.0)
            .alignment(0)
            .fill_width(),
        ))
        .spacing(18.0)
        .alignment(0)
        .fill()
        .align(0)
    }
}

/// `--preview PATH [PAGE] [dark]`: renders the window headless to a PNG at 2× and exits.
fn preview(args: &[String]) -> Option<i32> {
    let i = args.iter().position(|a| a == "--preview")?;
    let path = args.get(i + 1)?.clone();
    let start = args.get(i + 2).and_then(|p| p.parse().ok()).unwrap_or(0);
    let dark = args.iter().any(|a| a == "dark");
    let _rt = match vetro::kit::Headless::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("vetro-gallery: {e}");
            return Some(1);
        }
    };
    match vetro::preview((900.0, 800.0), 2.0, dark, move || Gallery { start }) {
        Ok(png) => match std::fs::write(&path, png) {
            Ok(()) => Some(0),
            Err(e) => {
                eprintln!("vetro-gallery: {path}: {e}");
                Some(1)
            }
        },
        Err(e) => {
            eprintln!("vetro-gallery: {e}");
            Some(1)
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = preview(&args) {
        std::process::exit(code);
    }
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
        if let Ok(root) = vetro::window("Vetro Gallery", (900.0, 800.0), || Gallery { start: 0 }) {
            // The window lives as long as the process.
            std::mem::forget(root);
        }
    });
    if let Err(e) = r {
        eprintln!("vetro-gallery: {e}");
        std::process::exit(1);
    }
}
