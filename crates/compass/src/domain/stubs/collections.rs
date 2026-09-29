use crate::domain::modules::import::ModuleInfo;
use crate::type_inference::Type;

/// Create collections module stub
pub(crate) fn create_collections_stub() -> ModuleInfo {
    let mut info = ModuleInfo::new("collections");

    let types = [
        (
            "deque",
            Type::ClassType {
                name: "deque".to_string(),
                module: Some("collections".to_string()),
            },
        ),
        (
            "defaultdict",
            Type::ClassType {
                name: "defaultdict".to_string(),
                module: Some("collections".to_string()),
            },
        ),
        (
            "OrderedDict",
            Type::ClassType {
                name: "OrderedDict".to_string(),
                module: Some("collections".to_string()),
            },
        ),
        (
            "Counter",
            Type::ClassType {
                name: "Counter".to_string(),
                module: Some("collections".to_string()),
            },
        ),
        (
            "ChainMap",
            Type::ClassType {
                name: "ChainMap".to_string(),
                module: Some("collections".to_string()),
            },
        ),
        (
            "namedtuple",
            Type::ClassType {
                name: "namedtuple".to_string(),
                module: Some("collections".to_string()),
            },
        ),
    ];

    for (name, ty) in types {
        info.exports.insert(name.to_string(), ty);
    }

    info
}

/// Create collections.abc module stub
pub(crate) fn create_collections_abc_stub() -> ModuleInfo {
    let mut info = ModuleInfo::new("collections.abc");

    let abstract_types = [
        "Awaitable",
        "Coroutine",
        "AsyncIterable",
        "AsyncIterator",
        "AsyncGenerator",
        "Hashable",
        "Iterable",
        "Iterator",
        "Generator",
        "Reversible",
        "Container",
        "Collection",
        "Callable",
        "Set",
        "MutableSet",
        "Mapping",
        "MutableMapping",
        "MappingView",
        "KeysView",
        "ItemsView",
        "ValuesView",
        "Sequence",
        "MutableSequence",
        "ByteString",
    ];

    for name in abstract_types {
        info.exports.insert(
            name.to_string(),
            Type::ClassType {
                name: name.to_string(),
                module: Some("collections.abc".to_string()),
            },
        );
    }

    info
}
