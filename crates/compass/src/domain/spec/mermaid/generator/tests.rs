use super::*;
use crate::domain::spec::ir::{
    EndpointDef, FieldDef, FlowEdge, FlowNode, HttpMethod, MethodDef, Relationship, ResponseDef,
    StateDef, TransitionDef,
};
use crate::type_inference::Type;

#[test]
fn test_generate_class_diagram() {
    let spec = DataModelSpec {
        models: vec![ModelDef {
            name: "User".to_string(),
            fields: vec![
                FieldDef {
                    name: "id".to_string(),
                    ty: Type::Int,
                    ..Default::default()
                },
                FieldDef {
                    name: "name".to_string(),
                    ty: Type::Str,
                    ..Default::default()
                },
            ],
            methods: vec![MethodDef {
                name: "save".to_string(),
                params: vec![],
                return_type: Type::None,
                visibility: Visibility::Public,
                is_static: false,
                is_async: false,
                description: None,
            }],
            ..Default::default()
        }],
        enums: vec![],
        relationships: vec![],
    };

    let gen = MermaidGenerator::new();
    let result = gen.generate_class_diagram(&spec);

    assert!(result.contains("classDiagram"));
    assert!(result.contains("class User"));
    assert!(result.contains("int id"));
    assert!(result.contains("String name"));
    assert!(result.contains("+save()"));
}

#[test]
fn test_generate_er_diagram() {
    let spec = DataModelSpec {
        models: vec![
            ModelDef {
                name: "Customer".to_string(),
                fields: vec![
                    FieldDef {
                        name: "id".to_string(),
                        ty: Type::Int,
                        primary_key: true,
                        ..Default::default()
                    },
                    FieldDef {
                        name: "name".to_string(),
                        ty: Type::Str,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            ModelDef {
                name: "Order".to_string(),
                fields: vec![FieldDef {
                    name: "id".to_string(),
                    ty: Type::Int,
                    primary_key: true,
                    ..Default::default()
                }],
                ..Default::default()
            },
        ],
        enums: vec![],
        relationships: vec![Relationship {
            from_model: "Customer".to_string(),
            from_field: "id".to_string(),
            to_model: "Order".to_string(),
            to_field: "customer_id".to_string(),
            rel_type: RelationType::OneToMany,
        }],
    };

    let gen = MermaidGenerator::new();
    let result = gen.generate_er_diagram(&spec);

    assert!(result.contains("erDiagram"));
    assert!(result.contains("Customer ||--o{ Order"));
    assert!(result.contains("int id PK"));
}

#[test]
fn test_generate_state_diagram() {
    let spec = StateMachineSpec {
        name: "OrderState".to_string(),
        states: vec![
            StateDef {
                name: "Pending".to_string(),
                description: Some("Order is pending".to_string()),
                on_enter: None,
                on_exit: None,
                nested: None,
            },
            StateDef {
                name: "Processing".to_string(),
                description: None,
                on_enter: None,
                on_exit: None,
                nested: None,
            },
        ],
        transitions: vec![TransitionDef {
            from: "Pending".to_string(),
            to: "Processing".to_string(),
            event: Some("confirm".to_string()),
            guard: None,
            action: None,
        }],
        initial_state: Some("Pending".to_string()),
        final_states: vec!["Completed".to_string()],
    };

    let gen = MermaidGenerator::new();
    let result = gen.generate_state_diagram(&spec);

    assert!(result.contains("stateDiagram-v2"));
    assert!(result.contains("[*] --> Pending"));
    assert!(result.contains("Pending --> Processing : confirm"));
    assert!(result.contains("Completed --> [*]"));
}

#[test]
fn test_generate_flowchart() {
    let spec = ControlFlowSpec {
        name: "LoginFlow".to_string(),
        nodes: vec![
            FlowNode {
                id: "A".to_string(),
                label: "Start".to_string(),
                node_type: FlowNodeType::Start,
            },
            FlowNode {
                id: "B".to_string(),
                label: "Valid?".to_string(),
                node_type: FlowNodeType::Decision,
            },
            FlowNode {
                id: "C".to_string(),
                label: "Login".to_string(),
                node_type: FlowNodeType::Process,
            },
        ],
        edges: vec![
            FlowEdge {
                from: "A".to_string(),
                to: "B".to_string(),
                label: None,
                condition: None,
            },
            FlowEdge {
                from: "B".to_string(),
                to: "C".to_string(),
                label: Some("Yes".to_string()),
                condition: None,
            },
        ],
    };

    let gen = MermaidGenerator::new();
    let result = gen.generate_flowchart(&spec, "TD");

    assert!(result.contains("flowchart TD"));
    assert!(result.contains("A([Start])"));
    assert!(result.contains("B{Valid?}"));
    assert!(result.contains("C[Login]"));
    assert!(result.contains("B -->|Yes| C"));
}

#[test]
fn test_generate_sequence_diagram() {
    let spec = RestApiSpec {
        title: "User API".to_string(),
        version: "1.0".to_string(),
        description: None,
        servers: vec![],
        endpoints: vec![EndpointDef {
            path: "/users".to_string(),
            method: HttpMethod::Get,
            operation_id: None,
            summary: Some("List users".to_string()),
            description: None,
            tags: vec![],
            path_params: vec![],
            query_params: vec![],
            request_body: None,
            responses: vec![ResponseDef {
                status_code: 200,
                description: "OK".to_string(),
                schema: None,
                content_type: None,
            }],
            security: vec![],
            deprecated: false,
        }],
        schemas: DataModelSpec::default(),
        security_schemes: vec![],
    };

    let gen = MermaidGenerator::new();
    let result = gen.generate_sequence_diagram(&spec);

    assert!(result.contains("sequenceDiagram"));
    assert!(result.contains("participant Client"));
    assert!(result.contains("Client->>Server: GET /users"));
    assert!(result.contains("Server-->>Client: 200 OK"));
}
