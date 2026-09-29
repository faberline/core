use crate::domain::modules::import::ModuleInfo;
use crate::type_inference::Type;

/// Create re module stub
pub fn create_re_stub() -> ModuleInfo {
    let mut info = ModuleInfo::new("re");

    // Pattern and Match types
    info.exports.insert(
        "Pattern".to_string(),
        Type::ClassType {
            name: "Pattern".to_string(),
            module: Some("re".to_string()),
        },
    );
    info.exports.insert(
        "Match".to_string(),
        Type::ClassType {
            name: "Match".to_string(),
            module: Some("re".to_string()),
        },
    );

    // Functions
    info.exports.insert(
        "compile".to_string(),
        Type::callable(
            vec![Type::Str],
            Type::Instance {
                name: "Pattern".to_string(),
                module: Some("re".to_string()),
                type_args: vec![Type::Str],
            },
        ),
    );
    info.exports.insert(
        "match".to_string(),
        Type::callable(
            vec![Type::Str, Type::Str],
            Type::optional(Type::Instance {
                name: "Match".to_string(),
                module: Some("re".to_string()),
                type_args: vec![Type::Str],
            }),
        ),
    );
    info.exports.insert(
        "search".to_string(),
        Type::callable(
            vec![Type::Str, Type::Str],
            Type::optional(Type::Instance {
                name: "Match".to_string(),
                module: Some("re".to_string()),
                type_args: vec![Type::Str],
            }),
        ),
    );
    info.exports.insert(
        "findall".to_string(),
        Type::callable(vec![Type::Str, Type::Str], Type::list(Type::Str)),
    );
    info.exports.insert(
        "finditer".to_string(),
        Type::callable(
            vec![Type::Str, Type::Str],
            Type::Any, // Iterator[Match]
        ),
    );
    info.exports.insert(
        "sub".to_string(),
        Type::callable(vec![Type::Str, Type::Str, Type::Str], Type::Str),
    );
    info.exports.insert(
        "subn".to_string(),
        Type::callable(
            vec![Type::Str, Type::Str, Type::Str],
            Type::Tuple(vec![Type::Str, Type::Int]),
        ),
    );
    info.exports.insert(
        "split".to_string(),
        Type::callable(vec![Type::Str, Type::Str], Type::list(Type::Str)),
    );
    info.exports.insert(
        "escape".to_string(),
        Type::callable(vec![Type::Str], Type::Str),
    );

    // Flags
    info.exports.insert("IGNORECASE".to_string(), Type::Int);
    info.exports.insert("I".to_string(), Type::Int);
    info.exports.insert("MULTILINE".to_string(), Type::Int);
    info.exports.insert("M".to_string(), Type::Int);
    info.exports.insert("DOTALL".to_string(), Type::Int);
    info.exports.insert("S".to_string(), Type::Int);
    info.exports.insert("VERBOSE".to_string(), Type::Int);
    info.exports.insert("X".to_string(), Type::Int);
    info.exports.insert("ASCII".to_string(), Type::Int);
    info.exports.insert("A".to_string(), Type::Int);

    info
}

/// Create json module stub
pub fn create_json_stub() -> ModuleInfo {
    let mut info = ModuleInfo::new("json");

    info.exports.insert(
        "dumps".to_string(),
        Type::callable(vec![Type::Any], Type::Str),
    );
    info.exports.insert(
        "loads".to_string(),
        Type::callable(vec![Type::Str], Type::Any),
    );
    info.exports.insert(
        "dump".to_string(),
        Type::callable(vec![Type::Any, Type::Any], Type::None),
    );
    info.exports.insert(
        "load".to_string(),
        Type::callable(vec![Type::Any], Type::Any),
    );

    info.exports.insert(
        "JSONEncoder".to_string(),
        Type::ClassType {
            name: "JSONEncoder".to_string(),
            module: Some("json".to_string()),
        },
    );
    info.exports.insert(
        "JSONDecoder".to_string(),
        Type::ClassType {
            name: "JSONDecoder".to_string(),
            module: Some("json".to_string()),
        },
    );
    info.exports.insert(
        "JSONDecodeError".to_string(),
        Type::ClassType {
            name: "JSONDecodeError".to_string(),
            module: Some("json".to_string()),
        },
    );

    info
}

#[cfg(test)]
mod tests;
