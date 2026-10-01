use super::*;
use std::path::PathBuf;

#[test]
fn test_semantic_model_basic() {
    let mut model = SemanticModel::new();

    let scope_id = model.add_scope(None, Range::default());
    let symbol_id = model.add_symbol(SymbolData {
        name: "foo".to_string(),
        kind: SemanticSymbolKind::Function,
        def_range: Range {
            start: crate::diagnostic::Position {
                line: 0,
                character: 4,
            },
            end: crate::diagnostic::Position {
                line: 0,
                character: 7,
            },
        },
        file_path: PathBuf::from("test.py"),
        type_info: TypeInfo::Callable {
            params: vec![],
            return_type: Box::new(TypeInfo::Int),
        },
        documentation: Some("A test function".to_string()),
        scope_id,
        parent_id: None,
    });

    assert!(model.symbols.contains_key(&symbol_id));
    assert_eq!(model.symbols_by_name("foo").len(), 1);
}

#[test]
fn test_type_info_display() {
    assert_eq!(TypeInfo::Int.display(), "int");
    assert_eq!(TypeInfo::Str.display(), "str");
    assert_eq!(
        TypeInfo::List(Box::new(TypeInfo::Int)).display(),
        "list[int]"
    );
    assert_eq!(
        TypeInfo::Dict(Box::new(TypeInfo::Str), Box::new(TypeInfo::Int)).display(),
        "dict[str, int]"
    );
    assert_eq!(
        TypeInfo::Optional(Box::new(TypeInfo::Str)).display(),
        "str | None"
    );
    assert_eq!(
        TypeInfo::Union(vec![TypeInfo::Int, TypeInfo::Str]).display(),
        "int | str"
    );
}

#[test]
fn test_type_at_lookup() {
    let mut model = SemanticModel::new();

    model.add_typed_range(
        Range {
            start: crate::diagnostic::Position {
                line: 0,
                character: 0,
            },
            end: crate::diagnostic::Position {
                line: 0,
                character: 5,
            },
        },
        TypeInfo::Int,
        None,
    );

    model.add_typed_range(
        Range {
            start: crate::diagnostic::Position {
                line: 1,
                character: 0,
            },
            end: crate::diagnostic::Position {
                line: 1,
                character: 10,
            },
        },
        TypeInfo::Str,
        None,
    );

    model.finalize();

    assert_eq!(model.type_at(0, 2), Some(&TypeInfo::Int));
    assert_eq!(model.type_at(1, 5), Some(&TypeInfo::Str));
    assert_eq!(model.type_at(2, 0), None);
}

#[test]
fn test_symbol_reference_lookup() {
    let mut model = SemanticModel::new();

    let scope_id = model.add_scope(None, Range::default());
    let symbol_id = model.add_symbol(SymbolData {
        name: "x".to_string(),
        kind: SemanticSymbolKind::Variable,
        def_range: Range {
            start: crate::diagnostic::Position {
                line: 0,
                character: 0,
            },
            end: crate::diagnostic::Position {
                line: 0,
                character: 1,
            },
        },
        file_path: PathBuf::from("test.py"),
        type_info: TypeInfo::Int,
        documentation: None,
        scope_id,
        parent_id: None,
    });

    // Add a reference at line 2
    model.add_reference(
        symbol_id,
        Range {
            start: crate::diagnostic::Position {
                line: 2,
                character: 4,
            },
            end: crate::diagnostic::Position {
                line: 2,
                character: 5,
            },
        },
    );

    // Definition lookup should work at definition site
    let def = model.definition_at(0, 0);
    assert!(def.is_some());
    assert_eq!(def.unwrap().name, "x");

    // Definition lookup should work at reference site
    let def = model.definition_at(2, 4);
    assert!(def.is_some());
    assert_eq!(def.unwrap().name, "x");

    // References should be found
    let refs = model.references_at(0, 0, true);
    assert_eq!(refs.len(), 2); // Definition + 1 reference

    let refs = model.references_at(0, 0, false);
    assert_eq!(refs.len(), 1); // Only the reference, not the definition
}
