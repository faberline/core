use crate::domain::modules::import::ModuleInfo;
use crate::type_inference::Type;

/// Create typing module stub
pub(crate) fn create_typing_stub() -> ModuleInfo {
    let mut info = ModuleInfo::new("typing");

    // Type constructors
    let type_constructors = [
        "List",
        "Dict",
        "Set",
        "FrozenSet",
        "Tuple",
        "Optional",
        "Union",
        "Callable",
        "Type",
        "Sequence",
        "Mapping",
        "MutableMapping",
        "Iterable",
        "Iterator",
        "Generator",
        "Coroutine",
        "AsyncGenerator",
        "AsyncIterator",
        "AsyncIterable",
        "Awaitable",
        "ContextManager",
        "AsyncContextManager",
    ];

    for tc in type_constructors {
        info.exports.insert(
            tc.to_string(),
            Type::ClassType {
                name: tc.to_string(),
                module: Some("typing".to_string()),
            },
        );
    }

    // Special forms
    info.exports.insert("Any".to_string(), Type::Any);
    info.exports.insert("NoReturn".to_string(), Type::Never);
    info.exports.insert("Never".to_string(), Type::Never);

    // TypeVar and related
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

    // Literal and Final
    info.exports.insert(
        "Literal".to_string(),
        Type::ClassType {
            name: "Literal".to_string(),
            module: Some("typing".to_string()),
        },
    );
    info.exports.insert(
        "Final".to_string(),
        Type::ClassType {
            name: "Final".to_string(),
            module: Some("typing".to_string()),
        },
    );

    // TypedDict
    info.exports.insert(
        "TypedDict".to_string(),
        Type::ClassType {
            name: "TypedDict".to_string(),
            module: Some("typing".to_string()),
        },
    );

    // Other typing utilities
    info.exports.insert(
        "ClassVar".to_string(),
        Type::ClassType {
            name: "ClassVar".to_string(),
            module: Some("typing".to_string()),
        },
    );
    info.exports
        .insert("Self".to_string(), Type::SelfType { class_name: None });
    info.exports.insert(
        "TypeAlias".to_string(),
        Type::ClassType {
            name: "TypeAlias".to_string(),
            module: Some("typing".to_string()),
        },
    );

    // PEP 593: Annotated
    info.exports.insert(
        "Annotated".to_string(),
        Type::ClassType {
            name: "Annotated".to_string(),
            module: Some("typing".to_string()),
        },
    );

    // PEP 612: ParamSpec and Concatenate
    info.exports.insert(
        "ParamSpec".to_string(),
        Type::ClassType {
            name: "ParamSpec".to_string(),
            module: Some("typing".to_string()),
        },
    );
    info.exports.insert(
        "Concatenate".to_string(),
        Type::ClassType {
            name: "Concatenate".to_string(),
            module: Some("typing".to_string()),
        },
    );

    // PEP 646: TypeVarTuple and Unpack
    info.exports.insert(
        "TypeVarTuple".to_string(),
        Type::ClassType {
            name: "TypeVarTuple".to_string(),
            module: Some("typing".to_string()),
        },
    );
    info.exports.insert(
        "Unpack".to_string(),
        Type::ClassType {
            name: "Unpack".to_string(),
            module: Some("typing".to_string()),
        },
    );

    // PEP 675: LiteralString
    info.exports
        .insert("LiteralString".to_string(), Type::LiteralString);

    // PEP 647: TypeGuard
    info.exports.insert(
        "TypeGuard".to_string(),
        Type::ClassType {
            name: "TypeGuard".to_string(),
            module: Some("typing".to_string()),
        },
    );

    // PEP 742: TypeIs
    info.exports.insert(
        "TypeIs".to_string(),
        Type::ClassType {
            name: "TypeIs".to_string(),
            module: Some("typing".to_string()),
        },
    );

    // Functions
    info.exports.insert(
        "cast".to_string(),
        Type::callable(vec![Type::Any, Type::Any], Type::Any),
    );
    info.exports.insert(
        "overload".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "no_type_check".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "get_type_hints".to_string(),
        Type::callable(vec![Type::Any], Type::dict(Type::Str, Type::Any)),
    );

    info
}
