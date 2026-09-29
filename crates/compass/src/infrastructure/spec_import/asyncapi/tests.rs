use super::*;
use crate::type_inference::Type;

#[test]
fn test_parse_asyncapi_2x() {
    let yaml = r#"
asyncapi: 2.6.0
info:
  title: User Service
  version: 1.0.0
  description: User events service

channels:
  user/created:
    description: User creation events
    subscribe:
      operationId: onUserCreated
      summary: Handle user created event
      message:
        payload:
          type: object
          properties:
            userId:
              type: string
            email:
              type: string
              format: email
            createdAt:
              type: string
              format: date-time

  user/updated:
    publish:
      operationId: publishUserUpdated
      message:
        $ref: '#/components/messages/UserUpdated'

components:
  messages:
    UserUpdated:
      payload:
        type: object
        properties:
          userId:
            type: string
          changes:
            type: object
  schemas:
    User:
      type: object
      properties:
        id:
          type: string
        name:
          type: string
        email:
          type: string
"#;

    let mut parser = AsyncApiParser::new();
    let spec = parser.parse_yaml(yaml).unwrap();

    assert_eq!(spec.title, "User Service");
    assert_eq!(spec.version, "1.0.0");
    assert_eq!(spec.channels.len(), 2);

    // Check user/created channel
    let created_channel = spec
        .channels
        .iter()
        .find(|c| c.name == "user/created")
        .unwrap();
    assert!(created_channel.subscribe.is_some());
    let sub = created_channel.subscribe.as_ref().unwrap();
    assert_eq!(sub.operation_id, Some("onUserCreated".to_string()));

    // Check user/updated channel
    let updated_channel = spec
        .channels
        .iter()
        .find(|c| c.name == "user/updated")
        .unwrap();
    assert!(updated_channel.publish.is_some());

    // Check messages
    assert!(!spec.messages.models.is_empty());
}

#[test]
fn test_parse_message_ref() {
    let yaml = r#"
asyncapi: 2.6.0
info:
  title: Order Service
  version: 1.0.0

channels:
  orders/placed:
    subscribe:
      message:
        $ref: '#/components/messages/OrderPlaced'

components:
  messages:
    OrderPlaced:
      payload:
        type: object
        properties:
          orderId:
            type: string
          total:
            type: number
"#;

    let mut parser = AsyncApiParser::new();
    let spec = parser.parse_yaml(yaml).unwrap();

    assert_eq!(spec.channels.len(), 1);
    let channel = &spec.channels[0];
    assert!(channel.subscribe.is_some());
}

#[test]
fn test_parse_oneof_messages() {
    let yaml = r#"
asyncapi: 2.6.0
info:
  title: Notification Service
  version: 1.0.0

channels:
  notifications:
    subscribe:
      message:
        oneOf:
          - $ref: '#/components/messages/EmailNotification'
          - $ref: '#/components/messages/SmsNotification'

components:
  messages:
    EmailNotification:
      payload:
        type: object
        properties:
          email:
            type: string
          subject:
            type: string
    SmsNotification:
      payload:
        type: object
        properties:
          phone:
            type: string
          text:
            type: string
"#;

    let mut parser = AsyncApiParser::new();
    let spec = parser.parse_yaml(yaml).unwrap();

    let channel = &spec.channels[0];
    let sub = channel.subscribe.as_ref().unwrap();

    // Should be a Union type
    match &sub.message {
        Type::Union(types) => assert_eq!(types.len(), 2),
        _ => panic!("Expected Union type"),
    }
}
