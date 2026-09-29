//! Helper for extracting and linting Markdown content embedded in API specs
//! (OpenAPI/AsyncAPI YAML or JSON description fields).

pub use crate::domain::lint::embedded_markdown::{
    extract_description_fields, lint_embedded_markdown, DescriptionField,
};
