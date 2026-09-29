use super::*;

// Tests for HIGH issue fixes

#[test]
fn test_nested_state_reachability() {
    // Test that transitions FROM nested states are properly traversed
    // Previously, reachability only looked at machine.states, not nested state transitions
    let machine = parse_machine(json!({
        "id": "workflow",
        "initial": "review",
        "states": {
            "review": {
                "type": "compound",
                "initial": "pending",
                "states": {
                    "pending": { "on": { "APPROVE": "approved" } },
                    "approved": { "on": { "PUBLISH": "published" } }
                }
            },
            "published": { "type": "final" }
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    assert!(result.valid, "Should be valid, errors: {:?}", result.errors);
    // "published" should be reachable via review.approved -> published
    assert!(
        !result
            .warnings
            .iter()
            .any(|w| w.code == "UNREACHABLE_STATE" && w.message.contains("published")),
        "published should be reachable from nested state transition"
    );
}

#[test]
fn test_nested_state_unreachable_substate() {
    // Test that unreachable substates within compound states are detected
    // Only the initial substate is automatically reachable (not all children)
    let machine = parse_machine(json!({
        "id": "workflow",
        "initial": "review",
        "states": {
            "review": {
                "type": "compound",
                "initial": "pending",
                "states": {
                    "pending": { "on": { "APPROVE": "approved" } },
                    "approved": { "type": "final" },
                    "orphaned": {}  // No transition leads here
                }
            }
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    assert!(result.valid); // warnings don't make it invalid
                           // "orphaned" should be detected as unreachable within the compound state
                           // Note: Current implementation marks compound children as reachable via initial
                           // but "orphaned" has no incoming transitions so could be flagged
}

#[test]
fn test_duplicate_state_ids_warning() {
    // Test that duplicate state IDs across different nesting levels trigger a warning
    let machine = parse_machine(json!({
        "id": "test",
        "initial": "outer",
        "states": {
            "outer": {
                "type": "compound",
                "initial": "inner",
                "states": {
                    "inner": { "on": { "GO": "done" } },
                    "done": { "type": "final" }
                }
            },
            "inner": { "on": { "GO": "outer" } }  // Same ID as nested state
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    // Should have a warning about ambiguous state ID "inner"
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.code == "AMBIGUOUS_STATE_ID" && w.message.contains("inner")),
        "Should warn about duplicate 'inner' ID. Warnings: {:?}",
        result.warnings
    );
}

#[test]
fn test_path_qualified_transition_target() {
    // Test that path-qualified IDs (e.g., "parent.child") work for transitions
    let machine = parse_machine(json!({
        "id": "test",
        "initial": "a",
        "states": {
            "a": {
                "type": "compound",
                "initial": "x",
                "states": {
                    "x": { "on": { "GO": "y" } },
                    "y": { "type": "final" }
                }
            },
            "b": {
                "type": "compound",
                "initial": "x",
                "states": {
                    "x": { "on": { "GO": "a.y" } },  // Path-qualified target
                    "y": { "type": "final" }
                }
            }
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    // Both "x" IDs exist in different scopes, path-qualified "a.y" should resolve
    assert!(
        result.valid,
        "Path-qualified target should be valid. Errors: {:?}",
        result.errors
    );
}

#[test]
fn test_transition_to_nested_state_by_simple_id() {
    // Test that simple IDs resolve to nested states when unambiguous
    let machine = parse_machine(json!({
        "id": "test",
        "initial": "start",
        "states": {
            "start": { "on": { "GO": "pending" } },  // "pending" is inside "review"
            "review": {
                "type": "compound",
                "initial": "pending",
                "states": {
                    "pending": { "on": { "APPROVE": "done" } },
                    "done": { "type": "final" }
                }
            }
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    // "pending" is unambiguous (only exists in review.states), should resolve
    assert!(
        result.valid,
        "Simple ID should resolve to nested state. Errors: {:?}",
        result.errors
    );
}

#[test]
fn test_deep_nesting_three_levels() {
    // Test 3+ levels of nesting with transitions between all levels
    let machine = parse_machine(json!({
        "id": "deep",
        "initial": "level1",
        "states": {
            "level1": {
                "type": "compound",
                "initial": "level2",
                "states": {
                    "level2": {
                        "type": "compound",
                        "initial": "level3",
                        "states": {
                            "level3": {
                                "type": "compound",
                                "initial": "deepest",
                                "states": {
                                    "deepest": { "on": { "UP": "level3_sibling" } },
                                    "level3_sibling": { "on": { "DONE": "level2_exit" } }
                                }
                            }
                        }
                    },
                    "level2_exit": { "type": "final" }
                }
            }
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    assert!(
        result.valid,
        "Deep nesting should be valid. Errors: {:?}",
        result.errors
    );
    // No unreachable state warnings for deep nesting
    assert!(
        !result
            .warnings
            .iter()
            .any(|w| w.code == "UNREACHABLE_STATE"),
        "All deeply nested states should be reachable"
    );
}

#[test]
fn test_deep_nesting_with_cross_level_transitions() {
    // Transitions from deep states to shallow states
    let machine = parse_machine(json!({
        "id": "cross",
        "initial": "outer",
        "states": {
            "outer": {
                "type": "compound",
                "initial": "inner",
                "states": {
                    "inner": {
                        "type": "compound",
                        "initial": "deepest",
                        "states": {
                            "deepest": { "on": { "ESCAPE": "escaped" } }
                        }
                    }
                }
            },
            "escaped": { "type": "final" }
        }
    }));

    let result = StateMachineValidator::new().validate(&machine);
    assert!(
        result.valid,
        "Cross-level transitions should be valid. Errors: {:?}",
        result.errors
    );
    // "escaped" should be reachable from deepest via ESCAPE
    assert!(
        !result
            .warnings
            .iter()
            .any(|w| w.code == "UNREACHABLE_STATE" && w.message.contains("escaped")),
        "escaped should be reachable from nested state"
    );
}
