use super::*;

#[test]
fn test_fastapi_method_signatures() {
    let provider = FastAPITypeProvider::new();

    // Test Depends()
    let depends_sig = provider.get_method_signature(&Type::Any, "Depends");
    assert!(depends_sig.is_some());
    let sig = depends_sig.unwrap();
    assert!(matches!(sig.return_type, Type::Any));

    // Test Path()
    let path_sig = provider.get_method_signature(&Type::Any, "Path");
    assert!(path_sig.is_some());
    let sig = path_sig.unwrap();
    assert!(matches!(sig.return_type, Type::Any));

    // Test Query()
    let query_sig = provider.get_method_signature(&Type::Any, "Query");
    assert!(query_sig.is_some());

    // Test JSONResponse
    let json_response_sig = provider.get_method_signature(&Type::Any, "JSONResponse");
    assert!(json_response_sig.is_some());
    let sig = json_response_sig.unwrap();
    assert!(matches!(sig.return_type, Type::Instance { name, .. } if name == "JSONResponse"));
}
