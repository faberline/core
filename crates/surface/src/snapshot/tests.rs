use crate::{Callback, Element, Props, SurfaceSnapshot};

#[test]
fn snapshot_captures_semantic_surface_tree() {
    let surface = Element::intrinsic(
        "main",
        Props {
            id: Some("app".to_string()),
            ..Default::default()
        },
        vec![Element::intrinsic(
            "button",
            Props {
                id: Some("save".to_string()),
                on_click: Some(Callback::new(|_| {})),
                ..Default::default()
            },
            vec![Element::text("Save")],
        )],
    )
    .surface_snapshot();

    assert_eq!(surface.schema_version, SurfaceSnapshot::SCHEMA_VERSION);
    assert_eq!(surface.text_content(), "Save");

    let button = surface.find_by_semantic_id("save").unwrap();
    assert_eq!(button.role.as_deref(), Some("button"));
    assert_eq!(button.name.as_deref(), Some("Save"));
    assert!(button.props.has_on_click);
    assert_eq!(surface.find_by_role("button"), vec![button]);
}
