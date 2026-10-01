use super::*;
use crate::domain::spec::ir::{ParamDef, ResponseDef};
use crate::type_inference::Type;

#[test]
fn test_generate_client() {
    let spec = RestApiSpec {
        title: "Pet Store".to_string(),
        version: "1.0.0".to_string(),
        description: Some("A sample pet store API".to_string()),
        servers: vec![],
        endpoints: vec![EndpointDef {
            path: "/pets/{pet_id}".to_string(),
            method: HttpMethod::Get,
            operation_id: Some("getPet".to_string()),
            summary: Some("Get a pet by ID".to_string()),
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
        }],
        schemas: DataModelSpec::default(),
        security_schemes: vec![],
    };

    let gen = ReqwestGenerator::new();
    let ctx = GenContext::default();
    let result = gen.generate_rest_api(&spec, &ctx).unwrap();

    assert_eq!(result.len(), 1);
    let code = &result[0].content;

    assert!(code.contains("pub struct PetStoreClient"));
    assert!(code.contains("pub async fn get_pet"));
    assert!(code.contains("pet_id: impl AsRef<str>"));
}
