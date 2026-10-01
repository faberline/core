use crate::domain::modules::import::ModuleInfo;
use crate::type_inference::Type;

/// Create module info for common Python builtins
#[allow(dead_code)]
pub fn create_builtins_module() -> ModuleInfo {
    let mut info = ModuleInfo::new("builtins");

    // Add common builtin types
    info.exports.insert(
        "int".to_string(),
        Type::ClassType {
            name: "int".to_string(),
            module: Some("builtins".to_string()),
        },
    );
    info.exports.insert(
        "str".to_string(),
        Type::ClassType {
            name: "str".to_string(),
            module: Some("builtins".to_string()),
        },
    );
    info.exports.insert(
        "float".to_string(),
        Type::ClassType {
            name: "float".to_string(),
            module: Some("builtins".to_string()),
        },
    );
    info.exports.insert(
        "bool".to_string(),
        Type::ClassType {
            name: "bool".to_string(),
            module: Some("builtins".to_string()),
        },
    );
    info.exports.insert(
        "list".to_string(),
        Type::ClassType {
            name: "list".to_string(),
            module: Some("builtins".to_string()),
        },
    );
    info.exports.insert(
        "dict".to_string(),
        Type::ClassType {
            name: "dict".to_string(),
            module: Some("builtins".to_string()),
        },
    );
    info.exports.insert(
        "set".to_string(),
        Type::ClassType {
            name: "set".to_string(),
            module: Some("builtins".to_string()),
        },
    );
    info.exports.insert(
        "tuple".to_string(),
        Type::ClassType {
            name: "tuple".to_string(),
            module: Some("builtins".to_string()),
        },
    );
    info.exports.insert(
        "bytes".to_string(),
        Type::ClassType {
            name: "bytes".to_string(),
            module: Some("builtins".to_string()),
        },
    );
    info.exports.insert(
        "type".to_string(),
        Type::ClassType {
            name: "type".to_string(),
            module: Some("builtins".to_string()),
        },
    );
    info.exports.insert(
        "object".to_string(),
        Type::ClassType {
            name: "object".to_string(),
            module: Some("builtins".to_string()),
        },
    );

    // Common functions
    info.exports.insert(
        "len".to_string(),
        Type::callable(vec![Type::Any], Type::Int),
    );
    info.exports.insert(
        "print".to_string(),
        Type::callable(vec![Type::Any], Type::None),
    );
    info.exports.insert(
        "range".to_string(),
        Type::callable(vec![Type::Int], Type::list(Type::Int)),
    );
    info.exports.insert(
        "enumerate".to_string(),
        Type::callable(
            vec![Type::list(Type::Unknown)],
            Type::list(Type::Tuple(vec![Type::Int, Type::Unknown])),
        ),
    );
    info.exports.insert(
        "zip".to_string(),
        Type::callable(
            vec![Type::list(Type::Unknown), Type::list(Type::Unknown)],
            Type::list(Type::Tuple(vec![Type::Unknown, Type::Unknown])),
        ),
    );

    info
}

/// Create module info for typing module
#[allow(dead_code)]
pub fn create_typing_module() -> ModuleInfo {
    let mut info = ModuleInfo::new("typing");

    // Generic type constructors
    info.exports.insert(
        "List".to_string(),
        Type::ClassType {
            name: "list".to_string(),
            module: Some("typing".to_string()),
        },
    );
    info.exports.insert(
        "Dict".to_string(),
        Type::ClassType {
            name: "dict".to_string(),
            module: Some("typing".to_string()),
        },
    );
    info.exports.insert(
        "Set".to_string(),
        Type::ClassType {
            name: "set".to_string(),
            module: Some("typing".to_string()),
        },
    );
    info.exports.insert(
        "Tuple".to_string(),
        Type::ClassType {
            name: "tuple".to_string(),
            module: Some("typing".to_string()),
        },
    );
    info.exports.insert(
        "Optional".to_string(),
        Type::ClassType {
            name: "Optional".to_string(),
            module: Some("typing".to_string()),
        },
    );
    info.exports.insert(
        "Union".to_string(),
        Type::ClassType {
            name: "Union".to_string(),
            module: Some("typing".to_string()),
        },
    );
    info.exports.insert(
        "Callable".to_string(),
        Type::ClassType {
            name: "Callable".to_string(),
            module: Some("typing".to_string()),
        },
    );
    info.exports.insert("Any".to_string(), Type::Any);
    info.exports.insert(
        "TypeVar".to_string(),
        Type::ClassType {
            name: "TypeVar".to_string(),
            module: Some("typing".to_string()),
        },
    );
    info.exports.insert(
        "Generic".to_string(),
        Type::ClassType {
            name: "Generic".to_string(),
            module: Some("typing".to_string()),
        },
    );
    info.exports.insert(
        "Protocol".to_string(),
        Type::ClassType {
            name: "Protocol".to_string(),
            module: Some("typing".to_string()),
        },
    );

    info
}
