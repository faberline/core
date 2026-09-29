use super::*;

#[test]
fn test_parallel_state_with_regions() {
    // Parallel states should have -- separators between regions
    let machine = parse_machine(json!({
        "id": "upload",
        "initial": "processing",
        "states": {
            "processing": {
                "type": "parallel",
                "states": {
                    "upload": {
                        "initial": "pending",
                        "type": "compound",
                        "states": {
                            "pending": { "on": { "COMPLETE": "done" } },
                            "done": { "type": "final" }
                        }
                    },
                    "thumbnail": {
                        "initial": "generating",
                        "type": "compound",
                        "states": {
                            "generating": { "on": { "COMPLETE": "done" } },
                            "done": { "type": "final" }
                        }
                    }
                }
            }
        }
    }));

    let validation = StateMachineValidator::new().validate(&machine);
    let output = MermaidPlusGenerator::new()
        .generate(&machine, validation)
        .unwrap();

    // Should have parallel state wrapper
    assert!(
        output.diagram.contains("state processing {"),
        "Should have parallel state wrapper. Diagram:\n{}",
        output.diagram
    );
    // Should have region separator
    assert!(
        output.diagram.contains("--"),
        "Should have region separator --. Diagram:\n{}",
        output.diagram
    );
    // Both regions should be rendered
    assert!(
        output.diagram.contains("state thumbnail {"),
        "Should have thumbnail region. Diagram:\n{}",
        output.diagram
    );
    assert!(
        output.diagram.contains("state upload {"),
        "Should have upload region. Diagram:\n{}",
        output.diagram
    );
}

#[test]
fn test_nested_parallel_states() {
    // Nested parallel states should also have region separators
    let machine = parse_machine(json!({
        "id": "nested_parallel",
        "initial": "outer",
        "states": {
            "outer": {
                "type": "parallel",
                "states": {
                    "region1": {
                        "type": "parallel",
                        "states": {
                            "sub1": {},
                            "sub2": {}
                        }
                    },
                    "region2": {
                        "type": "compound",
                        "initial": "a",
                        "states": {
                            "a": { "on": { "GO": "b" } },
                            "b": { "type": "final" }
                        }
                    }
                }
            }
        }
    }));

    let validation = StateMachineValidator::new().validate(&machine);
    let output = MermaidPlusGenerator::new()
        .generate(&machine, validation)
        .unwrap();

    // Outer parallel should have separators
    assert!(
        output.diagram.contains("state outer {"),
        "Should have outer parallel. Diagram:\n{}",
        output.diagram
    );
    // Nested parallel (region1) should also be rendered with separators
    assert!(
        output.diagram.contains("state region1 {"),
        "Should have nested parallel region1. Diagram:\n{}",
        output.diagram
    );
    // Should have multiple -- separators (outer and nested)
    let separator_count = output.diagram.matches("--").count();
    assert!(
        separator_count >= 2,
        "Should have at least 2 separators (outer + nested). Got {}. Diagram:\n{}",
        separator_count,
        output.diagram
    );
}

#[test]
fn test_atomic_states_visible_in_parallel() {
    // Atomic states without descriptions should still be visible in parallel regions
    let machine = parse_machine(json!({
        "id": "parallel_atomic",
        "initial": "parallel",
        "states": {
            "parallel": {
                "type": "parallel",
                "states": {
                    "atomic1": {},
                    "atomic2": {},
                    "atomic3": { "description": "Third region" }
                }
            }
        }
    }));

    let validation = StateMachineValidator::new().validate(&machine);
    let output = MermaidPlusGenerator::new()
        .generate(&machine, validation)
        .unwrap();

    // All atomic states should be visible (have state declarations)
    assert!(
        output.diagram.contains("state atomic1"),
        "atomic1 should be declared. Diagram:\n{}",
        output.diagram
    );
    assert!(
        output.diagram.contains("state atomic2"),
        "atomic2 should be declared. Diagram:\n{}",
        output.diagram
    );
    assert!(
        output.diagram.contains("state \"Third region\" as atomic3"),
        "atomic3 should have description. Diagram:\n{}",
        output.diagram
    );
}
