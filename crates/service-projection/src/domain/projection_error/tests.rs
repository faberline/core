use super::ProjectionError;

#[test]
fn an_invalid_name_keeps_its_message() {
    assert_eq!(
        ProjectionError::InvalidName.to_string(),
        "projection name is invalid"
    );
}

#[test]
fn other_keeps_the_wrapped_message() {
    let error = ProjectionError::other(std::io::Error::other("disk is full"));
    assert_eq!(error.to_string(), "disk is full");
    let ProjectionError::Other(inner) = error else {
        panic!("other() builds the Other variant");
    };
    assert!(inner.downcast_ref::<std::io::Error>().is_some());
}
