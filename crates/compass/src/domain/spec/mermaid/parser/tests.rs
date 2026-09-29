use super::*;
use crate::domain::spec::ir::FlowNodeType;

#[test]
fn test_parse_class_diagram() {
    let mermaid = r#"
classDiagram
    class User {
        +String name
        +String email
        -String password
        +login(username: String, password: String) bool
        +logout() void
    }
    class Order {
        +int id
        +float total
        +createOrder(items: List) Order
    }
    User --* Order : places
"#;

    let parser = MermaidParser::new();
    let result = parser.parse(mermaid).unwrap();

    match result {
        MermaidSpec::DataModel(spec) => {
            assert_eq!(spec.models.len(), 2);

            let user = spec.models.iter().find(|m| m.name == "User").unwrap();
            assert_eq!(user.fields.len(), 3);
            assert_eq!(user.methods.len(), 2);

            let order = spec.models.iter().find(|m| m.name == "Order").unwrap();
            assert_eq!(order.fields.len(), 2);

            assert_eq!(spec.relationships.len(), 1);
        }
        _ => panic!("Expected DataModel"),
    }
}

#[test]
fn test_parse_state_diagram() {
    let mermaid = r#"
stateDiagram-v2
    [*] --> Idle
    Idle --> Processing : start
    Processing --> Complete : finish
    Processing --> Error : fail
    Complete --> [*]
    Error --> Idle : retry
"#;

    let parser = MermaidParser::new();
    let result = parser.parse(mermaid).unwrap();

    match result {
        MermaidSpec::StateMachine(spec) => {
            assert_eq!(spec.initial_state, Some("Idle".to_string()));
            assert!(spec.final_states.contains(&"Complete".to_string()));
            assert!(spec.transitions.len() >= 5);
        }
        _ => panic!("Expected StateMachine"),
    }
}

#[test]
fn test_parse_flowchart() {
    let mermaid = r#"
flowchart TD
    A[Start] --> B{Is valid?}
    B -->|Yes| C[Process]
    B -->|No| D[Error]
    C --> E[End]
    D --> E
"#;

    let parser = MermaidParser::new();
    let result = parser.parse(mermaid).unwrap();

    match result {
        MermaidSpec::ControlFlow(spec) => {
            assert!(spec.nodes.len() >= 4);
            assert!(spec.edges.len() >= 4);

            // Check node types
            let decision = spec.nodes.iter().find(|n| n.id == "B").unwrap();
            assert!(matches!(decision.node_type, FlowNodeType::Decision));
        }
        _ => panic!("Expected ControlFlow"),
    }
}

#[test]
fn test_parse_er_diagram() {
    let mermaid = r#"
erDiagram
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE_ITEM : contains
    CUSTOMER {
        string name
        string email PK
        int age
    }
    ORDER {
        int id PK
        float total
        string status
    }
"#;

    let parser = MermaidParser::new();
    let result = parser.parse(mermaid).unwrap();

    match result {
        MermaidSpec::DataModel(spec) => {
            assert_eq!(spec.models.len(), 2);
            assert_eq!(spec.relationships.len(), 2);

            let customer = spec.models.iter().find(|m| m.name == "CUSTOMER").unwrap();
            assert_eq!(customer.fields.len(), 3);

            let email_field = customer.fields.iter().find(|f| f.name == "email").unwrap();
            assert!(email_field.primary_key);
        }
        _ => panic!("Expected DataModel"),
    }
}

#[test]
fn test_detect_diagram_type() {
    let parser = MermaidParser::new();

    assert_eq!(
        parser.detect_diagram_type("classDiagram\n...").unwrap(),
        DiagramType::ClassDiagram
    );
    assert_eq!(
        parser.detect_diagram_type("stateDiagram-v2\n...").unwrap(),
        DiagramType::StateDiagram
    );
    assert_eq!(
        parser.detect_diagram_type("flowchart TD\n...").unwrap(),
        DiagramType::Flowchart
    );
    assert_eq!(
        parser.detect_diagram_type("erDiagram\n...").unwrap(),
        DiagramType::ErDiagram
    );
}
