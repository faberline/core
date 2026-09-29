use crate::domain::modules::import::ModuleInfo;
use crate::type_inference::Type;

/// Create pathlib module stub
pub fn create_pathlib_stub() -> ModuleInfo {
    let mut info = ModuleInfo::new("pathlib");

    let path_type = Type::ClassType {
        name: "Path".to_string(),
        module: Some("pathlib".to_string()),
    };

    info.exports.insert("Path".to_string(), path_type.clone());
    info.exports.insert(
        "PurePath".to_string(),
        Type::ClassType {
            name: "PurePath".to_string(),
            module: Some("pathlib".to_string()),
        },
    );
    info.exports.insert(
        "PurePosixPath".to_string(),
        Type::ClassType {
            name: "PurePosixPath".to_string(),
            module: Some("pathlib".to_string()),
        },
    );
    info.exports.insert(
        "PureWindowsPath".to_string(),
        Type::ClassType {
            name: "PureWindowsPath".to_string(),
            module: Some("pathlib".to_string()),
        },
    );
    info.exports.insert(
        "PosixPath".to_string(),
        Type::ClassType {
            name: "PosixPath".to_string(),
            module: Some("pathlib".to_string()),
        },
    );
    info.exports.insert(
        "WindowsPath".to_string(),
        Type::ClassType {
            name: "WindowsPath".to_string(),
            module: Some("pathlib".to_string()),
        },
    );

    info
}

/// Create functools module stub
pub fn create_functools_stub() -> ModuleInfo {
    let mut info = ModuleInfo::new("functools");

    info.exports.insert(
        "reduce".to_string(),
        Type::callable(vec![Type::Any, Type::Any], Type::Any),
    );
    info.exports.insert(
        "partial".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "wraps".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports
        .insert("lru_cache".to_string(), Type::callable(vec![], Type::Any));
    info.exports.insert(
        "cache".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "cached_property".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "total_ordering".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "cmp_to_key".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );

    info
}

/// Create itertools module stub
pub fn create_itertools_stub() -> ModuleInfo {
    let mut info = ModuleInfo::new("itertools");

    // Infinite iterators
    info.exports.insert(
        "count".to_string(),
        Type::callable(vec![Type::Int], Type::Any),
    );
    info.exports.insert(
        "cycle".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "repeat".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );

    // Combinatoric iterators
    info.exports.insert(
        "product".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "permutations".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "combinations".to_string(),
        Type::callable(vec![Type::Any, Type::Int], Type::Any),
    );
    info.exports.insert(
        "combinations_with_replacement".to_string(),
        Type::callable(vec![Type::Any, Type::Int], Type::Any),
    );

    // Terminating iterators
    info.exports.insert(
        "chain".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "compress".to_string(),
        Type::callable(vec![Type::Any, Type::Any], Type::Any),
    );
    info.exports.insert(
        "dropwhile".to_string(),
        Type::callable(vec![Type::Any, Type::Any], Type::Any),
    );
    info.exports.insert(
        "takewhile".to_string(),
        Type::callable(vec![Type::Any, Type::Any], Type::Any),
    );
    info.exports.insert(
        "groupby".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "islice".to_string(),
        Type::callable(vec![Type::Any, Type::Int], Type::Any),
    );
    info.exports.insert(
        "starmap".to_string(),
        Type::callable(vec![Type::Any, Type::Any], Type::Any),
    );
    info.exports.insert(
        "tee".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "zip_longest".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );
    info.exports.insert(
        "filterfalse".to_string(),
        Type::callable(vec![Type::Any, Type::Any], Type::Any),
    );
    info.exports.insert(
        "accumulate".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );

    info
}

/// Create datetime module stub
pub fn create_datetime_stub() -> ModuleInfo {
    let mut info = ModuleInfo::new("datetime");

    info.exports.insert(
        "date".to_string(),
        Type::ClassType {
            name: "date".to_string(),
            module: Some("datetime".to_string()),
        },
    );
    info.exports.insert(
        "time".to_string(),
        Type::ClassType {
            name: "time".to_string(),
            module: Some("datetime".to_string()),
        },
    );
    info.exports.insert(
        "datetime".to_string(),
        Type::ClassType {
            name: "datetime".to_string(),
            module: Some("datetime".to_string()),
        },
    );
    info.exports.insert(
        "timedelta".to_string(),
        Type::ClassType {
            name: "timedelta".to_string(),
            module: Some("datetime".to_string()),
        },
    );
    info.exports.insert(
        "timezone".to_string(),
        Type::ClassType {
            name: "timezone".to_string(),
            module: Some("datetime".to_string()),
        },
    );
    info.exports.insert(
        "tzinfo".to_string(),
        Type::ClassType {
            name: "tzinfo".to_string(),
            module: Some("datetime".to_string()),
        },
    );

    // Constants
    info.exports.insert("MINYEAR".to_string(), Type::Int);
    info.exports.insert("MAXYEAR".to_string(), Type::Int);

    info
}

#[cfg(test)]
mod tests;
