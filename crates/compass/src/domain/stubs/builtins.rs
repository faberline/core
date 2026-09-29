use crate::domain::modules::import::ModuleInfo;
use crate::type_inference::Type;

/// Create builtins stub
pub(crate) fn create_builtins_stub() -> ModuleInfo {
    let mut info = ModuleInfo::new("builtins");

    // Primitive type constructors
    let primitives = [
        (
            "int",
            Type::ClassType {
                name: "int".to_string(),
                module: Some("builtins".to_string()),
            },
        ),
        (
            "float",
            Type::ClassType {
                name: "float".to_string(),
                module: Some("builtins".to_string()),
            },
        ),
        (
            "str",
            Type::ClassType {
                name: "str".to_string(),
                module: Some("builtins".to_string()),
            },
        ),
        (
            "bool",
            Type::ClassType {
                name: "bool".to_string(),
                module: Some("builtins".to_string()),
            },
        ),
        (
            "bytes",
            Type::ClassType {
                name: "bytes".to_string(),
                module: Some("builtins".to_string()),
            },
        ),
        (
            "bytearray",
            Type::ClassType {
                name: "bytearray".to_string(),
                module: Some("builtins".to_string()),
            },
        ),
        (
            "object",
            Type::ClassType {
                name: "object".to_string(),
                module: Some("builtins".to_string()),
            },
        ),
        (
            "type",
            Type::ClassType {
                name: "type".to_string(),
                module: Some("builtins".to_string()),
            },
        ),
    ];

    for (name, ty) in primitives {
        info.exports.insert(name.to_string(), ty);
    }

    // Container types
    let containers = [
        (
            "list",
            Type::ClassType {
                name: "list".to_string(),
                module: Some("builtins".to_string()),
            },
        ),
        (
            "dict",
            Type::ClassType {
                name: "dict".to_string(),
                module: Some("builtins".to_string()),
            },
        ),
        (
            "set",
            Type::ClassType {
                name: "set".to_string(),
                module: Some("builtins".to_string()),
            },
        ),
        (
            "frozenset",
            Type::ClassType {
                name: "frozenset".to_string(),
                module: Some("builtins".to_string()),
            },
        ),
        (
            "tuple",
            Type::ClassType {
                name: "tuple".to_string(),
                module: Some("builtins".to_string()),
            },
        ),
    ];

    for (name, ty) in containers {
        info.exports.insert(name.to_string(), ty);
    }

    // Common functions
    info.exports.insert(
        "len".to_string(),
        Type::callable(vec![Type::Any], Type::Int),
    );
    info.exports.insert(
        "abs".to_string(),
        Type::callable(vec![Type::Any], Type::Int),
    );
    info.exports.insert(
        "min".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "max".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "sum".to_string(),
        Type::callable(vec![Type::Any], Type::Int),
    );
    info.exports.insert(
        "sorted".to_string(),
        Type::callable(vec![Type::Any], Type::list(Type::Unknown)),
    );
    info.exports.insert(
        "reversed".to_string(),
        Type::callable(vec![Type::Any], Type::list(Type::Unknown)),
    );
    info.exports.insert(
        "enumerate".to_string(),
        Type::callable(
            vec![Type::Any],
            Type::list(Type::Tuple(vec![Type::Int, Type::Unknown])),
        ),
    );
    info.exports.insert(
        "zip".to_string(),
        Type::callable(
            vec![Type::Any, Type::Any],
            Type::list(Type::Tuple(vec![Type::Unknown, Type::Unknown])),
        ),
    );
    info.exports.insert(
        "map".to_string(),
        Type::callable(vec![Type::Any, Type::Any], Type::list(Type::Unknown)),
    );
    info.exports.insert(
        "filter".to_string(),
        Type::callable(vec![Type::Any, Type::Any], Type::list(Type::Unknown)),
    );
    info.exports.insert(
        "range".to_string(),
        Type::callable(vec![Type::Int], Type::list(Type::Int)),
    );
    info.exports.insert(
        "print".to_string(),
        Type::callable(vec![Type::Any], Type::None),
    );
    info.exports.insert(
        "input".to_string(),
        Type::callable(vec![Type::Str], Type::Str),
    );
    info.exports.insert(
        "open".to_string(),
        Type::callable(vec![Type::Str], Type::Any),
    ); // Simplified
    info.exports.insert(
        "isinstance".to_string(),
        Type::callable(vec![Type::Any, Type::Any], Type::Bool),
    );
    info.exports.insert(
        "issubclass".to_string(),
        Type::callable(vec![Type::Any, Type::Any], Type::Bool),
    );
    info.exports.insert(
        "hasattr".to_string(),
        Type::callable(vec![Type::Any, Type::Str], Type::Bool),
    );
    info.exports.insert(
        "getattr".to_string(),
        Type::callable(vec![Type::Any, Type::Str], Type::Any),
    );
    info.exports.insert(
        "setattr".to_string(),
        Type::callable(vec![Type::Any, Type::Str, Type::Any], Type::None),
    );
    info.exports.insert(
        "delattr".to_string(),
        Type::callable(vec![Type::Any, Type::Str], Type::None),
    );
    info.exports.insert(
        "callable".to_string(),
        Type::callable(vec![Type::Any], Type::Bool),
    );
    info.exports.insert(
        "repr".to_string(),
        Type::callable(vec![Type::Any], Type::Str),
    );
    info.exports.insert(
        "hash".to_string(),
        Type::callable(vec![Type::Any], Type::Int),
    );
    info.exports
        .insert("id".to_string(), Type::callable(vec![Type::Any], Type::Int));
    info.exports.insert(
        "iter".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "next".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );

    // Exception types
    let exceptions = [
        "Exception",
        "BaseException",
        "ValueError",
        "TypeError",
        "KeyError",
        "IndexError",
        "AttributeError",
        "RuntimeError",
        "StopIteration",
        "ImportError",
        "ModuleNotFoundError",
        "FileNotFoundError",
        "IOError",
        "OSError",
        "AssertionError",
        "ZeroDivisionError",
        "OverflowError",
        "NameError",
        "UnboundLocalError",
        "NotImplementedError",
    ];

    for exc in exceptions {
        info.exports.insert(
            exc.to_string(),
            Type::ClassType {
                name: exc.to_string(),
                module: Some("builtins".to_string()),
            },
        );
    }

    // Constants
    info.exports.insert("None".to_string(), Type::None);
    info.exports.insert("True".to_string(), Type::Bool);
    info.exports.insert("False".to_string(), Type::Bool);
    info.exports.insert("Ellipsis".to_string(), Type::Any);
    info.exports.insert("NotImplemented".to_string(), Type::Any);

    info
}
