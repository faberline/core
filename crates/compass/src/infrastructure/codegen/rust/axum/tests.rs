use super::*;
use crate::domain::spec::ir::{ParamDef, QueryParam, ResponseDef};
use crate::type_inference::Type;

#[test]
fn test_generate_router() {
    let spec = RestApiSpec {
        title: "Pet Store".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        servers: vec![],
        endpoints: vec![
            EndpointDef {
                path: "/pets".to_string(),
                method: HttpMethod::Get,
                operation_id: Some("listPets".to_string()),
                summary: Some("List all pets".to_string()),
                description: None,
                tags: vec![],
                path_params: vec![],
                query_params: vec![QueryParam {
                    name: "limit".to_string(),
                    ty: Type::Int,
                    required: false,
                    description: None,
                    default: None,
                }],
                request_body: None,
                responses: vec![ResponseDef {
                    status_code: 200,
                    description: "Success".to_string(),
                    schema: Some(Type::List(Box::new(Type::Instance {
                        name: "Pet".to_string(),
                        module: None,
                        type_args: vec![],
                    }))),
                    content_type: Some("application/json".to_string()),
                }],
                security: vec![],
                deprecated: false,
            },
            EndpointDef {
                path: "/pets/{pet_id}".to_string(),
                method: HttpMethod::Get,
                operation_id: Some("getPet".to_string()),
                summary: Some("Get a pet".to_string()),
                description: None,
                tags: vec![],
                path_params: vec![ParamDef {
                    name: "pet_id".to_string(),
                    ty: Type::Str,
                    default: None,
                }],
                query_params: vec![],
                request_body: None,
                responses: vec![ResponseDef {
                    status_code: 200,
                    description: "Success".to_string(),
                    schema: Some(Type::Instance {
                        name: "Pet".to_string(),
                        module: None,
                        type_args: vec![],
                    }),
                    content_type: Some("application/json".to_string()),
                }],
                security: vec![],
                deprecated: false,
            },
        ],
        schemas: DataModelSpec::default(),
        security_schemes: vec![],
    };

    let gen = AxumGenerator::new();
    let ctx = GenContext::default();
    let result = gen.generate_rest_api(&spec, &ctx).unwrap();

    assert_eq!(result.len(), 1);
    let code = &result[0].content;

    assert!(code.contains("pub fn create_router() -> Router"));
    assert!(code.contains(".route(\"/pets\", get(list_pets))"));
    assert!(code.contains(".route(\"/pets/:pet_id\", get(get_pet))"));
    assert!(code.contains("pub async fn list_pets"));
    assert!(code.contains("pub async fn get_pet"));
}
