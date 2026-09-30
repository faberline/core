//! The stub modules bundled with compass: builtins, typing, collections and
//! the typeshed subset for os, sys, io, re, json, pathlib, functools,
//! itertools and datetime.

use std::collections::HashMap;

use crate::domain::modules::import::ModuleInfo;
use crate::domain::stubs::builtins::create_builtins_stub;
use crate::domain::stubs::collections::{create_collections_abc_stub, create_collections_stub};
use crate::domain::stubs::typing::create_typing_stub;
use crate::domain::typeshed::system::{
    create_io_stub, create_os_path_stub, create_os_stub, create_sys_stub,
};
use crate::domain::typeshed::text::{create_json_stub, create_re_stub};
use crate::domain::typeshed::utility::{
    create_datetime_stub, create_functools_stub, create_itertools_stub, create_pathlib_stub,
};

/// The bundled stub modules by module path.
pub(crate) fn bundled_stubs() -> HashMap<String, ModuleInfo> {
    [
        // Core builtin types
        ("builtins", create_builtins_stub()),
        ("typing", create_typing_stub()),
        ("collections", create_collections_stub()),
        ("collections.abc", create_collections_abc_stub()),
        // Bundled typeshed stubs
        ("os", create_os_stub()),
        ("os.path", create_os_path_stub()),
        ("sys", create_sys_stub()),
        ("io", create_io_stub()),
        ("re", create_re_stub()),
        ("json", create_json_stub()),
        ("pathlib", create_pathlib_stub()),
        ("functools", create_functools_stub()),
        ("itertools", create_itertools_stub()),
        ("datetime", create_datetime_stub()),
    ]
    .into_iter()
    .map(|(path, info)| (path.to_string(), info))
    .collect()
}
