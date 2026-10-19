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
