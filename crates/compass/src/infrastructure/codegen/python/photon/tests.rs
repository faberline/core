use super::*;
use crate::domain::spec::ir::{ParamDef, QueryParam, ResponseDef, ServerDef};
use crate::type_inference::Type;

#[test]
fn test_generate_client() {
    let spec = RestApiSpec {
        title: "Pet Store".to_string(),
        version: "1.0.0".to_string(),
        description: Some("A sample pet store API".to_string()),
        servers: vec![ServerDef {
            url: "https://api.example.com".to_string(),
            description: None,
        }],
        endpoints: vec![
            EndpointDef {
                path: "/pets".to_string(),
                method: HttpMethod::Get,
                operation_id: Some("listPets".to_string()),
                summary: Some("List all pets".to_string()),
                description: None,
                tags: vec!["pets".to_string()],
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
                path: "/pets/{petId}".to_string(),
                method: HttpMethod::Get,
                operation_id: Some("getPet".to_string()),
                summary: Some("Get a pet by ID".to_string()),
                description: None,
                tags: vec!["pets".to_string()],
                path_params: vec![ParamDef {
                    name: "petId".to_string(),
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

    let gen = PhotonGenerator::new();
    let ctx = GenContext::default();
    let result = gen.generate_rest_api(&spec, &ctx).unwrap();

    assert_eq!(result.len(), 1);
    let code = &result[0].content;

    assert!(code.contains("class PetStoreClient:"));
    assert!(code.contains("async def list_pets"));
    assert!(code.contains("async def get_pet"));
    assert!(code.contains("petId: str"));
}
