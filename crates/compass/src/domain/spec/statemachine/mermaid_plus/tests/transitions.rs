use super::*;

#[test]
fn test_internal_transition_self_loop() {
    // Internal transitions (no target) should render as self-transitions
    let machine = parse_machine(json!({
        "id": "counter",
        "initial": "counting",
        "states": {
            "counting": {
                "on": {
                    "INCREMENT": { "actions": "increment" },
                    "RESET": "idle"
                }
            },
            "idle": { "on": { "START": "counting" } }
        },
        "actions": {
            "increment": { "effect": "context.count += 1" }
        }
    }));

    let validation = StateMachineValidator::new().validate(&machine);
    let output = MermaidPlusGenerator::new()
        .generate(&machine, validation)
        .unwrap();

    // Internal transition should render as self-loop
    assert!(
        output
            .diagram
            .contains("counting --> counting: INCREMENT / increment"),
        "Internal transition should render as self-loop. Diagram:\n{}",
        output.diagram
    );
    // Regular transition should still work
    assert!(output.diagram.contains("counting --> idle: RESET"));
}

#[test]
fn test_internal_transition_with_guard() {
    // Internal transition with guard should also render as self-loop
    let machine = parse_machine(json!({
        "id": "validator",
        "initial": "editing",
        "states": {
            "editing": {
                "on": {
                    "VALIDATE": { "guard": "hasInput", "actions": "runValidation" }
                }
            }
        },
        "guards": { "hasInput": { "condition": "input.length > 0" } },
        "actions": { "runValidation": { "effect": "validate(context.input)" } }
    }));

    let validation = StateMachineValidator::new().validate(&machine);
    let output = MermaidPlusGenerator::new()
        .generate(&machine, validation)
        .unwrap();

    assert!(
        output
            .diagram
            .contains("editing --> editing: VALIDATE [hasInput] / runValidation"),
        "Internal transition with guard should render correctly. Diagram:\n{}",
        output.diagram
    );
}

#[test]
fn test_conditional_transitions_render() {
    // Conditional transitions should render multiple arrows with guards
    let machine = parse_machine(json!({
        "id": "conditional",
        "initial": "idle",
        "states": {
            "idle": {
                "on": {
                    "SUBMIT": [
                        { "target": "success", "guard": "isValid" },
                        { "target": "error", "guard": "hasErrors" },
                        { "target": "pending" }
                    ]
                }
            },
            "success": { "type": "final" },
            "error": {},
            "pending": {}
        },
        "guards": {
            "isValid": { "condition": "true" },
            "hasErrors": { "condition": "false" }
        }
    }));

    let validation = StateMachineValidator::new().validate(&machine);
    let output = MermaidPlusGenerator::new()
        .generate(&machine, validation)
        .unwrap();

    // All conditional branches should be rendered
    assert!(
        output
            .diagram
            .contains("idle --> success: SUBMIT [isValid]"),
        "Should render guarded transition to success. Diagram:\n{}",
        output.diagram
    );
    assert!(
        output
            .diagram
            .contains("idle --> error: SUBMIT [hasErrors]"),
        "Should render guarded transition to error. Diagram:\n{}",
        output.diagram
    );
    assert!(
        output.diagram.contains("idle --> pending: SUBMIT"),
        "Should render default transition to pending. Diagram:\n{}",
        output.diagram
    );
}
