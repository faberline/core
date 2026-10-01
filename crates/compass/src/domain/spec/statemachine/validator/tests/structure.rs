use super::*;

#[test]
fn test_valid_simple_machine() {
    let machine = parse_machine(json!({
        "id": "toggle",
        "initial": "off",
        "states": {
            "off": { "on": { "TOGGLE": "on" } },
            "on": { "on": { "TOGGLE": "off" } }
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    assert!(result.valid);
    assert!(result.errors.is_empty());
}

#[test]
fn test_missing_initial_state() {
    let machine = parse_machine(json!({
        "id": "test",
        "initial": "nonexistent",
        "states": {
            "a": {}
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    assert!(!result.valid);
    assert!(result
        .errors
        .iter()
        .any(|e| e.code == "MISSING_INITIAL_STATE"));
}

#[test]
fn test_invalid_transition_target() {
    let machine = parse_machine(json!({
        "id": "test",
        "initial": "a",
        "states": {
            "a": { "on": { "GO": "nonexistent" } }
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    assert!(!result.valid);
    assert!(result
        .errors
        .iter()
        .any(|e| e.code == "INVALID_TRANSITION_TARGET"));
}

#[test]
fn test_unreachable_state_warning() {
    let machine = parse_machine(json!({
        "id": "test",
        "initial": "a",
        "states": {
            "a": { "on": { "GO": "b" } },
            "b": {},
            "unreachable": {}
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    assert!(result.valid); // warnings don't make it invalid
    assert!(result
        .warnings
        .iter()
        .any(|w| w.code == "UNREACHABLE_STATE"));
}
