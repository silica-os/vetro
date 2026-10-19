use std::cell::Cell;
use std::rc::Rc;

use vetro_kit::{Headless, Prop};

use crate::prelude::*;
use crate::{mount, preview};

struct Counter {
    renders: Rc<Cell<u32>>,
}

impl Component for Counter {
    fn body(&self, cx: &mut Cx) -> Element {
        self.renders.set(self.renders.get() + 1);
        let count = cx.state(|| 0i64);
        let c2 = count.clone();
        column((
            label(format!("Count: {}", count.get())).id("label"),
            button("Increment")
                .id("inc")
                .on_activate(move || c2.update(|c| *c += 1)),
        ))
        .spacing(12.0)
        .padding(20.0)
    }
}

fn host() -> vetro_kit::View {
    let w = vetro_kit::Window::new(300.0, 200.0).unwrap();
    let h = vetro_kit::View::new(vetro_kit::Kind::VIEW).unwrap();
    w.set_content(&h).unwrap();
    w.show().unwrap();
    h
}

#[test]
fn state_changes_re_render_and_update_in_place() {
    let rt = Headless::new().unwrap();
    let renders = Rc::new(Cell::new(0));
    let r2 = renders.clone();
    let root = mount(host(), move || Counter {
        renders: r2.clone(),
    });
    let label = root.host().find("label").unwrap();
    let inc = root.host().find("inc").unwrap();
    assert_eq!(label.get_str(Prop::TEXT).unwrap(), "Count: 0");
    inc.click().unwrap();
    inc.click().unwrap();
    // The two changes are batched into one render on the next turn.
    rt.advance(0.01);
    assert_eq!(renders.get(), 2);
    assert_eq!(label.get_str(Prop::TEXT).unwrap(), "Count: 2");
    // The same views were updated, not replaced.
    assert_eq!(root.host().find("label").unwrap(), label);
    assert_eq!(root.host().find("inc").unwrap(), inc);
}

struct List {
    items: crate::Model<Vec<&'static str>>,
}

impl Component for List {
    fn body(&self, cx: &mut Cx) -> Element {
        cx.watch(&self.items);
        let items = self.items.get();
        column(for_each(items, |s| *s, |s| label(*s).id(s)))
    }
}

#[test]
fn keyed_children_keep_their_views() {
    let rt = Headless::new().unwrap();
    let model = crate::Model::new(vec!["a", "b", "c"]);
    let m2 = model.clone();
    let root = mount(host(), move || List { items: m2.clone() });
    let col = root.host().children().remove(0);
    let b = root.host().find("b").unwrap();
    model.update(|v| v.reverse());
    rt.advance(0.01);
    let order: Vec<String> = col
        .children()
        .iter()
        .map(|v| v.get_str(Prop::TEXT).unwrap())
        .collect();
    assert_eq!(order, ["c", "b", "a"]);
    assert_eq!(root.host().find("b").unwrap(), b);
    model.update(|v| v.retain(|s| *s != "b"));
    rt.advance(0.01);
    assert!(root.host().find("b").is_none());
    assert_eq!(col.children().len(), 2);
}

struct Toggle;

impl Component for Toggle {
    fn body(&self, cx: &mut Cx) -> Element {
        let on = cx.state(|| false);
        let o2 = on.clone();
        let content = if on.get() {
            label("On").id("x")
        } else {
            button("Off").id("x")
        };
        column((
            content,
            button("Flip")
                .id("flip")
                .on_activate(move || o2.update(|v| *v = !*v)),
        ))
    }
}

#[test]
fn kind_changes_replace_the_view() {
    let rt = Headless::new().unwrap();
    let root = mount(host(), || Toggle);
    let before = root.host().find("x").unwrap();
    root.host().find("flip").unwrap().click().unwrap();
    rt.advance(0.01);
    let after = root.host().find("x").unwrap();
    assert_ne!(before, after);
    assert_eq!(after.get_str(Prop::TEXT).unwrap(), "On");
}

#[test]
fn preview_renders_a_png() {
    let _rt = Headless::new().unwrap();
    let renders = Rc::new(Cell::new(0));
    let png = preview((240.0, 120.0), 2.0, false, move || Counter {
        renders: renders.clone(),
    })
    .unwrap();
    assert_eq!(png.get(1..4), Some(&b"PNG"[..]));
}

struct WithSheet {
    shown: Rc<Cell<u32>>,
}

impl Component for WithSheet {
    fn body(&self, cx: &mut Cx) -> Element {
        let open = cx.state(|| false);
        let o2 = open.clone();
        let shown = self.shown.clone();
        column(button("Open").id("open").on_activate(move || o2.set(true)))
            .sheet(&open.binding(), (200.0, 100.0), move || {
                shown.set(shown.get() + 1);
                label("In the sheet")
            })
            .context_menu(vec![
                crate::MenuEntry::item("Copy", || {}),
                crate::MenuEntry::separator(),
            ])
    }
}

#[test]
fn sheets_open_from_state() {
    let rt = Headless::new().unwrap();
    let shown = Rc::new(Cell::new(0));
    let s2 = shown.clone();
    let root = mount(host(), move || WithSheet { shown: s2.clone() });
    assert_eq!(shown.get(), 0);
    root.host().find("open").unwrap().click().unwrap();
    rt.advance(0.01);
    assert_eq!(shown.get(), 1, "the sheet's content rendered once");
}

struct Form;

impl Component for Form {
    fn body(&self, cx: &mut Cx) -> Element {
        let on = cx.state(|| false);
        let level = cx.state(|| 0.5f64);
        let name = cx.state(String::new);
        let seg = cx.state(|| 1usize);
        column((
            checkbox("Enabled", &on.binding()).id("check"),
            switch(&on.binding()).id("switch"),
            slider(&level.binding(), 0.0, 1.0).id("slider"),
            text_field("Name", &name.binding()).id("name"),
            segmented(&["One", "Two", "Three"], &seg.binding()).id("seg"),
            progress(Some(level.get())),
            spinner(),
            label(format!(
                "{} {:.1} {} {}",
                on.get(),
                level.get(),
                name.get(),
                seg.get()
            ))
            .id("summary"),
        ))
        .spacing(8.0)
    }
}

#[test]
fn controls_bind_state() {
    let rt = Headless::new().unwrap();
    let root = mount(host(), || Form);
    let summary = root.host().find("summary").unwrap();
    assert_eq!(summary.get_str(Prop::TEXT).unwrap(), "false 0.5  1");
    // Clicking the checkbox writes the binding; the switch follows on the next render.
    root.host().find("check").unwrap().click().unwrap();
    rt.advance(0.01);
    assert_eq!(summary.get_str(Prop::TEXT).unwrap(), "true 0.5  1");
    assert!(
        root.host()
            .find("switch")
            .unwrap()
            .get_bool(Prop::VALUE)
            .unwrap()
    );
    assert_eq!(
        root.host()
            .find("seg")
            .unwrap()
            .get_int(Prop::SELECTED_INDEX)
            .unwrap(),
        1
    );
}

struct Big {
    prefix: crate::Model<&'static str>,
}

impl Component for Big {
    fn body(&self, cx: &mut Cx) -> Element {
        cx.watch(&self.prefix);
        let p = self.prefix.get();
        crate::list(100_000, 28.0, move |i| {
            label(format!("{p} {i}")).id(&format!("r{i}"))
        })
        .id("list")
        .fill()
    }
}

#[test]
fn lists_make_only_the_rows_in_sight() {
    let rt = Headless::new().unwrap();
    let prefix = crate::Model::new("Row");
    let p2 = prefix.clone();
    let w = vetro_kit::Window::new(300.0, 200.0).unwrap();
    let h = vetro_kit::View::new(vetro_kit::Kind::VIEW).unwrap();
    w.set_content(&h).unwrap();
    w.show().unwrap();
    let root = mount(h, move || Big { prefix: p2.clone() });
    // A frame lays the list out.
    w.snapshot_png(1.0).unwrap();
    let list = root.host().find("list").unwrap();
    let live = list.children().len();
    assert!(live > 4 && live < 20, "{live} rows");
    let r0 = root.host().find("r0").unwrap();
    assert_eq!(r0.get_str(Prop::TEXT).unwrap(), "Row 0");
    // A render updates the live rows in place.
    prefix.update(|p| *p = "Item");
    rt.advance(0.01);
    w.snapshot_png(1.0).unwrap();
    assert_eq!(r0.get_str(Prop::TEXT).unwrap(), "Item 0");
    assert_eq!(root.host().find("r0").unwrap(), r0);
}
