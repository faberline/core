use super::*;
use crate::domain::spec::ir::{HttpMethod, SecuritySchemeType};

const PETSTORE_YAML: &str = r#"
openapi: "3.0.0"
info:
  title: Petstore API
  version: "1.0.0"
  description: A sample API for pets
servers:
  - url: https://api.example.com/v1
    description: Production
paths:
  /pets:
    get:
      operationId: listPets
      summary: List all pets
      tags:
        - pets
      parameters:
        - name: limit
          in: query
          description: Maximum number of pets to return
          required: false
          schema:
            type: integer
            minimum: 1
            maximum: 100
      responses:
        "200":
          description: A list of pets
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/Pet'
    post:
      operationId: createPet
      summary: Create a pet
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/NewPet'
      responses:
        "201":
          description: Pet created
  /pets/{petId}:
    get:
      operationId: getPet
      summary: Get a pet by ID
      parameters:
        - name: petId
          in: path
          required: true
          schema:
            type: string
      responses:
        "200":
          description: A pet
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Pet'
components:
  schemas:
    Pet:
      type: object
      required:
        - id
        - name
      properties:
        id:
          type: integer
        name:
          type: string
        tag:
          type: string
    NewPet:
      type: object
      required:
        - name
      properties:
        name:
          type: string
          minLength: 1
          maxLength: 100
        tag:
          type: string
  securitySchemes:
    bearerAuth:
      type: http
      scheme: bearer
      bearerFormat: JWT
"#;

#[test]
fn test_parse_petstore() {
    let mut parser = OpenApiParser::new();
    let spec = parser.parse_yaml(PETSTORE_YAML).unwrap();

    assert_eq!(spec.title, "Petstore API");
    assert_eq!(spec.version, "1.0.0");
    assert_eq!(spec.servers.len(), 1);
    assert_eq!(spec.endpoints.len(), 3);

    // Check list pets endpoint
    let list_pets = spec
        .endpoints
        .iter()
        .find(|e| e.operation_id == Some("listPets".into()));
    assert!(list_pets.is_some());
    let list_pets = list_pets.unwrap();
    assert_eq!(list_pets.method, HttpMethod::Get);
    assert_eq!(list_pets.path, "/pets");
    assert_eq!(list_pets.query_params.len(), 1);

    // Check create pet endpoint
    let create_pet = spec
        .endpoints
        .iter()
        .find(|e| e.operation_id == Some("createPet".into()));
    assert!(create_pet.is_some());
    let create_pet = create_pet.unwrap();
    assert!(create_pet.request_body.is_some());

    // Check get pet endpoint
    let get_pet = spec
        .endpoints
        .iter()
        .find(|e| e.operation_id == Some("getPet".into()));
    assert!(get_pet.is_some());
    let get_pet = get_pet.unwrap();
    assert_eq!(get_pet.path_params.len(), 1);
    assert_eq!(get_pet.path_params[0].name, "petId");
}

#[test]
fn test_parse_schemas() {
    let mut parser = OpenApiParser::new();
    let spec = parser.parse_yaml(PETSTORE_YAML).unwrap();

    assert_eq!(spec.schemas.models.len(), 2);

    let pet = spec.schemas.get_model("Pet").unwrap();
    assert_eq!(pet.fields.len(), 3);

    let new_pet = spec.schemas.get_model("NewPet").unwrap();
    let name_field = new_pet.fields.iter().find(|f| f.name == "name").unwrap();
    assert!(name_field.required);
    assert_eq!(name_field.constraints.min_length, Some(1));
    assert_eq!(name_field.constraints.max_length, Some(100));
}

#[test]
fn test_parse_security_schemes() {
    let mut parser = OpenApiParser::new();
    let spec = parser.parse_yaml(PETSTORE_YAML).unwrap();

    assert_eq!(spec.security_schemes.len(), 1);
    let auth = &spec.security_schemes[0];
    assert_eq!(auth.name, "bearerAuth");
    assert!(
        matches!(&auth.scheme_type, SecuritySchemeType::Http { scheme, .. } if scheme == "bearer")
    );
}
