use crate::domain::modules::import::ModuleInfo;
use crate::type_inference::Type;

/// Create os module stub
pub fn create_os_stub() -> ModuleInfo {
    let mut info = ModuleInfo::new("os");

    // Path operations
    info.exports
        .insert("getcwd".to_string(), Type::callable(vec![], Type::Str));
    info.exports.insert(
        "chdir".to_string(),
        Type::callable(vec![Type::Str], Type::None),
    );
    info.exports.insert(
        "listdir".to_string(),
        Type::callable(vec![Type::Str], Type::list(Type::Str)),
    );
    info.exports.insert(
        "mkdir".to_string(),
        Type::callable(vec![Type::Str], Type::None),
    );
    info.exports.insert(
        "makedirs".to_string(),
        Type::callable(vec![Type::Str], Type::None),
    );
    info.exports.insert(
        "remove".to_string(),
        Type::callable(vec![Type::Str], Type::None),
    );
    info.exports.insert(
        "rmdir".to_string(),
        Type::callable(vec![Type::Str], Type::None),
    );
    info.exports.insert(
        "rename".to_string(),
        Type::callable(vec![Type::Str, Type::Str], Type::None),
    );
    info.exports.insert(
        "stat".to_string(),
        Type::callable(vec![Type::Str], Type::Any),
    );
    info.exports.insert(
        "walk".to_string(),
        Type::callable(vec![Type::Str], Type::Any),
    );

    // Environment
    info.exports
        .insert("environ".to_string(), Type::dict(Type::Str, Type::Str));
    info.exports.insert(
        "getenv".to_string(),
        Type::callable(vec![Type::Str], Type::optional(Type::Str)),
    );
    info.exports.insert(
        "putenv".to_string(),
        Type::callable(vec![Type::Str, Type::Str], Type::None),
    );

    // Process
    info.exports
        .insert("getpid".to_string(), Type::callable(vec![], Type::Int));
    info.exports
        .insert("getppid".to_string(), Type::callable(vec![], Type::Int));
    info.exports.insert(
        "system".to_string(),
        Type::callable(vec![Type::Str], Type::Int),
    );
    info.exports.insert(
        "popen".to_string(),
        Type::callable(vec![Type::Str], Type::Any),
    );

    // Path separator
    info.exports.insert("sep".to_string(), Type::Str);
    info.exports.insert("linesep".to_string(), Type::Str);
    info.exports.insert("pathsep".to_string(), Type::Str);
    info.exports.insert("name".to_string(), Type::Str);

    info
}

/// Create os.path module stub
pub fn create_os_path_stub() -> ModuleInfo {
    let mut info = ModuleInfo::new("os.path");

    info.exports.insert(
        "join".to_string(),
        Type::callable(vec![Type::Str, Type::Str], Type::Str),
    );
    info.exports.insert(
        "exists".to_string(),
        Type::callable(vec![Type::Str], Type::Bool),
    );
    info.exports.insert(
        "isfile".to_string(),
        Type::callable(vec![Type::Str], Type::Bool),
    );
    info.exports.insert(
        "isdir".to_string(),
        Type::callable(vec![Type::Str], Type::Bool),
    );
    info.exports.insert(
        "isabs".to_string(),
        Type::callable(vec![Type::Str], Type::Bool),
    );
    info.exports.insert(
        "islink".to_string(),
        Type::callable(vec![Type::Str], Type::Bool),
    );
    info.exports.insert(
        "basename".to_string(),
        Type::callable(vec![Type::Str], Type::Str),
    );
    info.exports.insert(
        "dirname".to_string(),
        Type::callable(vec![Type::Str], Type::Str),
    );
    info.exports.insert(
        "split".to_string(),
        Type::callable(vec![Type::Str], Type::Tuple(vec![Type::Str, Type::Str])),
    );
    info.exports.insert(
        "splitext".to_string(),
        Type::callable(vec![Type::Str], Type::Tuple(vec![Type::Str, Type::Str])),
    );
    info.exports.insert(
        "abspath".to_string(),
        Type::callable(vec![Type::Str], Type::Str),
    );
    info.exports.insert(
        "realpath".to_string(),
        Type::callable(vec![Type::Str], Type::Str),
    );
    info.exports.insert(
        "normpath".to_string(),
        Type::callable(vec![Type::Str], Type::Str),
    );
    info.exports.insert(
        "expanduser".to_string(),
        Type::callable(vec![Type::Str], Type::Str),
    );
    info.exports.insert(
        "expandvars".to_string(),
        Type::callable(vec![Type::Str], Type::Str),
    );
    info.exports.insert(
        "getsize".to_string(),
        Type::callable(vec![Type::Str], Type::Int),
    );
    info.exports.insert(
        "getmtime".to_string(),
        Type::callable(vec![Type::Str], Type::Float),
    );
    info.exports.insert(
        "getctime".to_string(),
        Type::callable(vec![Type::Str], Type::Float),
    );
    info.exports.insert(
        "getatime".to_string(),
        Type::callable(vec![Type::Str], Type::Float),
    );

    info
}

/// Create sys module stub
pub fn create_sys_stub() -> ModuleInfo {
    let mut info = ModuleInfo::new("sys");

    // Streams
    info.exports.insert("stdin".to_string(), Type::Any);
    info.exports.insert("stdout".to_string(), Type::Any);
    info.exports.insert("stderr".to_string(), Type::Any);

    // Arguments
    info.exports
        .insert("argv".to_string(), Type::list(Type::Str));

    // Paths
    info.exports
        .insert("path".to_string(), Type::list(Type::Str));
    info.exports
        .insert("modules".to_string(), Type::dict(Type::Str, Type::Any));

    // Version info
    info.exports.insert("version".to_string(), Type::Str);
    info.exports.insert(
        "version_info".to_string(),
        Type::Tuple(vec![Type::Int, Type::Int, Type::Int, Type::Str, Type::Int]),
    );
    info.exports.insert("platform".to_string(), Type::Str);
    info.exports.insert("executable".to_string(), Type::Str);
    info.exports.insert("prefix".to_string(), Type::Str);

    // Functions
    info.exports.insert(
        "exit".to_string(),
        Type::callable(vec![Type::Int], Type::Never),
    );
    info.exports.insert(
        "getrecursionlimit".to_string(),
        Type::callable(vec![], Type::Int),
    );
    info.exports.insert(
        "setrecursionlimit".to_string(),
        Type::callable(vec![Type::Int], Type::None),
    );
    info.exports.insert(
        "getsizeof".to_string(),
        Type::callable(vec![Type::Any], Type::Int),
    );

    // Numeric limits
    info.exports.insert("maxsize".to_string(), Type::Int);
    info.exports.insert("float_info".to_string(), Type::Any);
    info.exports.insert("int_info".to_string(), Type::Any);

    info
}

/// Create io module stub
pub fn create_io_stub() -> ModuleInfo {
    let mut info = ModuleInfo::new("io");

    // Base classes
    info.exports.insert(
        "IOBase".to_string(),
        Type::ClassType {
            name: "IOBase".to_string(),
            module: Some("io".to_string()),
        },
    );
    info.exports.insert(
        "RawIOBase".to_string(),
        Type::ClassType {
            name: "RawIOBase".to_string(),
            module: Some("io".to_string()),
        },
    );
    info.exports.insert(
        "BufferedIOBase".to_string(),
        Type::ClassType {
            name: "BufferedIOBase".to_string(),
            module: Some("io".to_string()),
        },
    );
    info.exports.insert(
        "TextIOBase".to_string(),
        Type::ClassType {
            name: "TextIOBase".to_string(),
            module: Some("io".to_string()),
        },
    );

    // Concrete classes
    info.exports.insert(
        "FileIO".to_string(),
        Type::ClassType {
            name: "FileIO".to_string(),
            module: Some("io".to_string()),
        },
    );
    info.exports.insert(
        "BytesIO".to_string(),
        Type::ClassType {
            name: "BytesIO".to_string(),
            module: Some("io".to_string()),
        },
    );
    info.exports.insert(
        "StringIO".to_string(),
        Type::ClassType {
            name: "StringIO".to_string(),
            module: Some("io".to_string()),
        },
    );
    info.exports.insert(
        "BufferedReader".to_string(),
        Type::ClassType {
            name: "BufferedReader".to_string(),
            module: Some("io".to_string()),
        },
    );
    info.exports.insert(
        "BufferedWriter".to_string(),
        Type::ClassType {
            name: "BufferedWriter".to_string(),
            module: Some("io".to_string()),
        },
    );
    info.exports.insert(
        "TextIOWrapper".to_string(),
        Type::ClassType {
            name: "TextIOWrapper".to_string(),
            module: Some("io".to_string()),
        },
    );

    // Functions
    info.exports.insert(
        "open".to_string(),
        Type::callable(vec![Type::Str, Type::Str], Type::Any),
    );

    // Constants
    info.exports
        .insert("DEFAULT_BUFFER_SIZE".to_string(), Type::Int);
    info.exports.insert("SEEK_SET".to_string(), Type::Int);
    info.exports.insert("SEEK_CUR".to_string(), Type::Int);
    info.exports.insert("SEEK_END".to_string(), Type::Int);

    info
}

#[cfg(test)]
mod tests;
