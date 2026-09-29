use super::*;
use crate::domain::type_system::ty::Type;
use crate::graph::ImportGraph;
use crate::type_inference::{DeepTypeInferencer, ImportInfo, TypeBinding};

mod imports;
mod invalidation;
mod ordering;

/// Helper: create a TypeBinding.
fn binding(symbol: &str, ty: Type, source: &str, exported: bool) -> TypeBinding {
    TypeBinding {
        ty,
        source_file: PathBuf::from(source),
        symbol: symbol.to_string(),
        line: 1,
        is_exported: exported,
        dependencies: vec![],
        is_propagated: false,
    }
}

/// Helper: set up a two-file scenario (db.py → handler.py) with a
/// `from db import get_user` import.
fn setup_two_file() -> (DeepTypeInferencer, ImportGraph) {
    let mut inf = DeepTypeInferencer::new();
    let db = PathBuf::from("db.py");
    let handler = PathBuf::from("handler.py");

    inf.add_file(db.clone());
    inf.add_file(handler.clone());

    // db.py exports `get_user: Callable[[int], User]`
    inf.add_file_symbol(
        &db,
        "get_user".to_string(),
        binding(
            "get_user",
            Type::Callable {
                params: vec![],
                ret: Box::new(Type::Instance {
                    name: "User".to_string(),
                    module: None,
                    type_args: vec![],
                }),
            },
            "db.py",
            true,
        ),
    );

    // handler.py has import info: from db import get_user
    inf.add_import(
        &handler,
        ImportInfo {
            module: "db".to_string(),
            names: Some(vec!["get_user".to_string()]),
            alias: None,
        },
    );

    // Build import edge in inferencer's internal graph.
    inf.add_import_edge(handler.clone(), db.clone());

    // Build file-level ImportGraph (needed for pipeline).
    let ig = ImportGraph::new();

    (inf, ig)
}
