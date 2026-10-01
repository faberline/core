use super::*;

#[test]
fn test_compound_state_validation() {
    let machine = parse_machine(json!({
        "id": "workflow",
        "initial": "draft",
        "states": {
            "draft": { "on": { "SUBMIT": "review" } },
            "review": {
                "type": "compound",
                "initial": "pending",
                "states": {
                    "pending": { "on": { "APPROVE": "approved" } },
                    "approved": { "type": "final" }
                }
            }
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    assert!(result.valid);
}

#[test]
fn test_invalid_compound_initial() {
    let machine = parse_machine(json!({
        "id": "workflow",
        "initial": "review",
        "states": {
            "review": {
                "type": "compound",
                "initial": "nonexistent",
                "states": {
                    "pending": {}
                }
            }
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    assert!(!result.valid);
    assert!(result
        .errors
        .iter()
        .any(|e| e.code == "INVALID_COMPOUND_INITIAL"));
}

#[test]
fn test_strict_mode_compound_initial() {
    // In non-strict mode, missing compound initial is a warning
    // In strict mode, it becomes an error
    let machine = parse_machine(json!({
        "id": "test",
        "initial": "parent",
        "states": {
            "parent": {
                "type": "compound",
                // Missing initial - should be warning in normal mode, error in strict
                "states": {
                    "child1": {},
                    "child2": {}
                }
            }
        }
    }));

    // Non-strict mode: warning, still valid
    let result = StateMachineValidator::new().validate(&machine);
    assert!(
        result.valid,
        "Non-strict mode should be valid with warnings"
    );
    assert!(result
        .warnings
        .iter()
        .any(|w| w.code == "MISSING_COMPOUND_INITIAL"));

    // Strict mode: error, invalid
    let result = StateMachineValidator::strict().validate(&machine);
    assert!(
        !result.valid,
        "Strict mode should fail for missing compound initial"
    );
    assert!(result
        .errors
        .iter()
        .any(|e| e.code == "MISSING_COMPOUND_INITIAL"));
}

#[test]
fn test_strict_mode_with_valid_compound() {
    // Valid compound state should pass in both modes
    let machine = parse_machine(json!({
        "id": "test",
        "initial": "parent",
        "states": {
            "parent": {
                "type": "compound",
                "initial": "child1",
                "states": {
                    "child1": { "on": { "NEXT": "child2" } },
                    "child2": { "type": "final" }
                }
            }
        }
    }));

    let result = StateMachineValidator::strict().validate(&machine);
    assert!(
        result.valid,
        "Valid compound should pass strict mode. Errors: {:?}",
        result.errors
    );
}
