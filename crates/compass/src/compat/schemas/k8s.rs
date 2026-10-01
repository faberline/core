//! Programmatic K8s JSON-Schema definitions for common resources.
//!
//! Each function returns `(kind_name, schema_value)`.  The schemas cover the
//! most commonly misconfigured required-field constraints so that `K8002`
//! catches real issues without bundling the full 15 MB upstream schemas.
