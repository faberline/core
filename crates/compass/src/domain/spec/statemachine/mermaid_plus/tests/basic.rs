use super::*;

#[test]
fn test_generate_simple_mermaid() {
    let machine = parse_machine(json!({
        "id": "toggle",
        "initial": "off",
        "states": {
            "off": { "on": { "TOGGLE": "on" } },
            "on": { "on": { "TOGGLE": "off" } }
        }
    }));

    let validation = StateMachineValidator::new().validate(&machine);
    let output = MermaidPlusGenerator::new()
        .generate(&machine, validation)
        .unwrap();

    assert!(output.diagram.contains("stateDiagram-v2"));
    assert!(output.diagram.contains("[*] --> off"));
    assert!(output.diagram.contains("off --> on: TOGGLE"));
    assert!(output.diagram.contains("on --> off: TOGGLE"));
}

#[test]
fn test_generate_with_guards() {
    let machine = parse_machine(json!({
        "id": "fetch",
        "initial": "idle",
        "states": {
            "idle": {
                "on": {
                    "FETCH": { "target": "loading", "guard": "canFetch" }
                }
            },
            "loading": { "on": { "SUCCESS": "done" } },
            "done": { "type": "final" }
        },
        "guards": {
            "canFetch": { "condition": "retries < 3" }
        }
    }));

    let validation = StateMachineValidator::new().validate(&machine);
    let output = MermaidPlusGenerator::new()
        .generate(&machine, validation)
        .unwrap();

    assert!(output
        .diagram
        .contains("idle --> loading: FETCH [canFetch]"));
    assert!(output.diagram.contains("done --> [*]"));
}

#[test]
fn test_mermaid_plus_format() {
    let machine = parse_machine(json!({
        "id": "simple",
        "initial": "a",
        "states": {
            "a": { "on": { "GO": "b" } },
            "b": { "type": "final" }
        }
    }));

    let validation = StateMachineValidator::new().validate(&machine);
    let output = MermaidPlusGenerator::new()
        .generate(&machine, validation)
        .unwrap();

    // Check combined format (frontmatter inside code block)
    assert!(output.combined.starts_with("```mermaid\n---\n"));
    assert!(output.combined.contains("id: simple"));
    assert!(output.combined.contains("initial: a"));
    assert!(output.combined.contains("stateDiagram-v2"));
}
