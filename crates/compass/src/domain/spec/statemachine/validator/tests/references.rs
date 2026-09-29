use super::*;

#[test]
fn test_undefined_guard_warning() {
    let machine = parse_machine(json!({
        "id": "test",
        "initial": "a",
        "states": {
            "a": {
                "on": {
                    "GO": { "target": "b", "guard": "undefinedGuard" }
                }
            },
            "b": {}
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    assert!(result.valid);
    let guard_warning = result.warnings.iter().find(|w| w.code == "UNDEFINED_GUARD");
    assert!(
        guard_warning.is_some(),
        "Should have UNDEFINED_GUARD warning"
    );
    // Path should include the specific location
    assert!(
        guard_warning.unwrap().path.contains("states.a.on.GO"),
        "Guard error path should include transition location, got: {}",
        guard_warning.unwrap().path
    );
}

#[test]
fn test_undefined_action_warning_with_location() {
    let machine = parse_machine(json!({
        "id": "test",
        "initial": "a",
        "states": {
            "a": {
                "on": {
                    "CLICK": { "target": "b", "actions": "undefinedAction" }
                }
            },
            "b": {}
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    assert!(result.valid);
    let action_warning = result
        .warnings
        .iter()
        .find(|w| w.code == "UNDEFINED_ACTION");
    assert!(
        action_warning.is_some(),
        "Should have UNDEFINED_ACTION warning"
    );
    // Path should include the specific location
    assert!(
        action_warning.unwrap().path.contains("states.a.on.CLICK"),
        "Action error path should include transition location, got: {}",
        action_warning.unwrap().path
    );
}

#[test]
fn test_undefined_entry_exit_action_with_location() {
    // Test that undefined entry/exit actions report correct locations
    let machine = parse_machine(json!({
        "id": "test",
        "initial": "a",
        "states": {
            "a": {
                "entry": "onEnterA",
                "exit": "onExitA",
                "on": { "GO": "b" }
            },
            "b": {
                "entry": ["onEnterB", "logEntry"],
                "exit": "onExitB"
            }
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    assert!(result.valid, "Should be valid with warnings");

    // Should have warnings for all undefined actions
    let action_warnings: Vec<_> = result
        .warnings
        .iter()
        .filter(|w| w.code == "UNDEFINED_ACTION")
        .collect();

    // 5 undefined actions: onEnterA, onExitA, onEnterB, logEntry, onExitB
    assert_eq!(
        action_warnings.len(),
        5,
        "Should have 5 UNDEFINED_ACTION warnings. Got: {:?}",
        action_warnings.iter().map(|w| &w.path).collect::<Vec<_>>()
    );

    // Verify entry/exit paths are correct
    assert!(
        action_warnings.iter().any(|w| w.path == "states.a.entry"),
        "Should have warning for states.a.entry"
    );
    assert!(
        action_warnings.iter().any(|w| w.path == "states.a.exit"),
        "Should have warning for states.a.exit"
    );
    assert!(
        action_warnings.iter().any(|w| w.path == "states.b.entry"),
        "Should have warning for states.b.entry (onEnterB or logEntry)"
    );
    assert!(
        action_warnings.iter().any(|w| w.path == "states.b.exit"),
        "Should have warning for states.b.exit"
    );
}

#[test]
fn test_undefined_guard_in_conditional() {
    let machine = parse_machine(json!({
        "id": "test",
        "initial": "a",
        "states": {
            "a": {
                "on": {
                    "GO": [
                        { "target": "b", "guard": "condition1" },
                        { "target": "c", "guard": "condition2" }
                    ]
                }
            },
            "b": {},
            "c": {}
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    // Both guards are undefined
    let warnings: Vec<_> = result
        .warnings
        .iter()
        .filter(|w| w.code == "UNDEFINED_GUARD")
        .collect();
    assert_eq!(warnings.len(), 2, "Should have 2 UNDEFINED_GUARD warnings");
    // Paths should include array indices
    assert!(
        warnings
            .iter()
            .any(|w| w.path.contains("[0]") || w.path.contains("[1]")),
        "Conditional guard paths should include array index"
    );
}

#[test]
fn test_conditional_transition_with_guards_and_actions() {
    // Test conditional transitions with both guards and actions
    let machine = parse_machine(json!({
        "id": "conditional",
        "initial": "idle",
        "states": {
            "idle": {
                "on": {
                    "SUBMIT": [
                        { "target": "success", "guard": "isValid", "actions": "logSuccess" },
                        { "target": "error", "guard": "hasErrors", "actions": ["logError", "showAlert"] },
                        { "target": "pending" }  // Default fallback
                    ]
                }
            },
            "success": { "type": "final" },
            "error": { "on": { "RETRY": "idle" } },
            "pending": { "on": { "COMPLETE": "success" } }
        },
        "guards": {
            "isValid": { "condition": "data.isValid" },
            "hasErrors": { "condition": "data.errors.length > 0" }
        },
        "actions": {
            "logSuccess": { "effect": "console.log('success')" },
            "logError": { "effect": "console.error(data.errors)" },
            "showAlert": { "effect": "alert('Error!')" }
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    assert!(
        result.valid,
        "Conditional transitions should be valid. Errors: {:?}",
        result.errors
    );
    assert!(
        result.warnings.is_empty(),
        "Should have no warnings. Warnings: {:?}",
        result.warnings
    );
}
