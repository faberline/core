use std::{any::Any, cell::Cell, rc::Rc};
use surface::{Callback, Component, Element, Props};
use ui_runtime::{mount, set_update_scheduler, use_state};

fn counter(_: &Rc<dyn Any>) -> surface_upstream::Element {
    let (count, set_count) = use_state(0u32);
    Element::intrinsic(
        "button",
        Props::default()
            .with_id("counter")
            .with_on_click(Callback::new(move |()| set_count.set(count + 1))),
        vec![Element::text(count.to_string())],
    )
}

#[test]
fn facades_share_types_hook_storage_and_scheduler_with_standalone_packages() {
    let updates = Rc::new(Cell::new(0));
    let calls = updates.clone();
    set_update_scheduler(Some(Rc::new(move || calls.set(calls.get() + 1))));
    let component: surface_upstream::Component = Component::new("Counter", counter, Rc::new(()));
    let first: ui_runtime_upstream::MountHandle = mount(component.clone());
    let second: ui_runtime::MountHandle = ui_runtime_upstream::mount(component);
    first.snapshot().find_on_click("counter").unwrap().call(());
    assert_eq!(updates.get(), 1);
    assert_eq!(first.snapshot().text_content(), "0");
    assert!(first.flush());
    let tree: surface::Element = first.snapshot();
    assert_eq!(tree.text_content(), "1");
    assert!(!first.flush());
    assert!(!second.flush());
    assert_eq!(second.snapshot().text_content(), "0");
    ui_runtime_upstream::set_update_scheduler(None);
}

#[cfg(feature = "debug")]
#[test]
fn debug_feature_forwards_the_same_fiber_identity() {
    let mounted = mount(Component::new("Counter", counter, Rc::new(())));
    let id: ui_runtime_upstream::FiberId = mounted.fiber_id;
    let hooks: Vec<ui_runtime_upstream::DebugHookSummary> = ui_runtime::debug_snapshot_hooks(id);
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0].kind, "State");
    assert_eq!(hooks[0].type_name, Some("u32"));
    assert_eq!(
        hooks[0].value_json.as_ref().and_then(|v| v.as_u64()),
        Some(0)
    );
}
