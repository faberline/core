use super::*;

#[test]
fn test_generate_nested_states() {
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

    let validation = StateMachineValidator::new().validate(&machine);
    let output = MermaidPlusGenerator::new()
        .generate(&machine, validation)
        .unwrap();

    assert!(output.diagram.contains("state review {"));
    assert!(output.diagram.contains("[*] --> pending"));
}

#[test]
fn test_deep_nested_state_rendering() {
    // Deep nesting should render correctly with proper indentation
    let machine = parse_machine(json!({
        "id": "deep",
        "initial": "l1",
        "states": {
            "l1": {
                "type": "compound",
                "initial": "l2",
                "states": {
                    "l2": {
                        "type": "compound",
                        "initial": "l3",
                        "states": {
                            "l3": { "on": { "GO": "done" } },
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

    // Should have nested state blocks
    assert!(
        output.diagram.contains("state l1 {"),
        "Should have l1 state block"
    );
    assert!(
        output.diagram.contains("state l2 {"),
        "Should have l2 state block"
    );
    // Initial transitions at each level
    assert!(
        output.diagram.contains("[*] --> l2"),
        "Should have l1 initial"
    );
    assert!(
        output.diagram.contains("[*] --> l3"),
        "Should have l2 initial"
    );
}
