//! Per-language emitters. Each reads the shared [`crate::ir`] and renders a typed
//! client in its target language.
//!
//! - [`ts`]: TypeScript — types + fetch/axios client + TanStack Query hooks.
//! - [`py`]: Python — pydantic models + generated sync/async HTTP/2 runtime.
//! - [`rust`]: Rust — serde models + reqwest client.

pub mod py {
    pub use crate::domain::emit::py::{
        generate, generate_for_target, generate_for_target_with_file_bearer_auth,
        generate_with_file_bearer_auth,
    };

    pub mod client_emit {
        pub use crate::domain::emit::py::client_emit::{emit, HEADER};
    }

    pub mod models_emit {
        pub use crate::domain::emit::py::models_emit::{emit, is_py_ident, HEADER};
    }

    pub mod pymap {
        pub use crate::domain::emit::py::pymap::{object_expr, optional, type_expr, union_expr};
    }

    pub mod runtime_emit {
        pub use crate::domain::emit::py::runtime_emit::{emit, HEADER};
    }
}

pub mod rust {
    pub use crate::domain::emit::rust::{
        generate, generate_for_target, generate_for_target_with_file_bearer_auth,
        generate_with_file_bearer_auth,
    };

    pub mod client_emit {
        pub use crate::domain::emit::rust::client_emit::{emit, HEADER};
    }

    pub mod models_emit {
        pub use crate::domain::emit::rust::models_emit::{emit, field_name, HEADER};
    }

    pub mod rsmap {
        pub use crate::domain::emit::rust::rsmap::{object_expr, optional, type_expr};
    }
}

pub mod ts {
    pub use crate::domain::emit::ts::{
        generate, generate_for_target, generate_for_target_with_file_bearer_auth,
        generate_with_file_bearer_auth,
    };

    pub mod client_emit {
        pub use crate::domain::emit::ts::client_emit::{emit_client, emit_runtime, type_import};
    }

    pub mod hooks_emit {
        pub use crate::domain::emit::ts::hooks_emit::emit;
    }

    pub mod plan {
        pub use crate::domain::emit::ts::plan::{build, BodyField, OperationPlan, ParamField};
    }

    pub mod tsmap {
        pub use crate::domain::emit::ts::tsmap::{object_expr, type_expr, TypeMap};
    }

    pub mod types_emit {
        pub use crate::domain::emit::ts::types_emit::{emit, HEADER};
    }
}
