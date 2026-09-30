use std::path::Path;

/// Supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Python,
    TypeScript,
    Rust,
    JavaScript,
    Go,
    Html,
    Css,
    Dockerfile,
    Hcl,
    Yaml,
    Markdown,
    Mdx,
    Mermaid,
    Toml,
    Sql,
    Proto,
    GraphQL,
}

impl Language {
    /// Detect the language of a file from its name and extension.
    ///
    /// `Dockerfile`, `Dockerfile.*` and `*.dockerfile` are Dockerfiles; any
    /// other file is matched by extension. Returns `None` for an unknown
    /// extension or a path with none.
    pub fn from_path(path: &Path) -> Option<Language> {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name == "Dockerfile"
                || name.starts_with("Dockerfile.")
                || name.ends_with(".dockerfile")
            {
                return Some(Language::Dockerfile);
            }
        }

        let ext = path.extension()?.to_str()?;
        match ext {
            "py" | "pyi" => Some(Language::Python),
            "ts" | "tsx" => Some(Language::TypeScript),
            "rs" => Some(Language::Rust),
            "js" | "jsx" => Some(Language::JavaScript),
            "go" => Some(Language::Go),
            "html" | "htm" => Some(Language::Html),
            "css" => Some(Language::Css),
            "tf" | "tfvars" => Some(Language::Hcl),
            "yaml" | "yml" => Some(Language::Yaml),
            "md" | "markdown" => Some(Language::Markdown),
            "mdx" => Some(Language::Mdx),
            "mmd" | "mermaid" => Some(Language::Mermaid),
            "toml" => Some(Language::Toml),
            "sql" => Some(Language::Sql),
            "proto" => Some(Language::Proto),
            "graphql" | "gql" => Some(Language::GraphQL),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Python => "python",
            Language::TypeScript => "typescript",
            Language::Rust => "rust",
            Language::JavaScript => "javascript",
            Language::Go => "go",
            Language::Html => "html",
            Language::Css => "css",
            Language::Dockerfile => "dockerfile",
            Language::Hcl => "hcl",
            Language::Yaml => "yaml",
            Language::Markdown => "markdown",
            Language::Mdx => "mdx",
            Language::Mermaid => "mermaid",
            Language::Toml => "toml",
            Language::Sql => "sql",
            Language::Proto => "proto",
            Language::GraphQL => "graphql",
        }
    }

    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Language::Python => &["py", "pyi"],
            Language::TypeScript => &["ts", "tsx"],
            Language::Rust => &["rs"],
            Language::JavaScript => &["js", "jsx"],
            Language::Go => &["go"],
            Language::Html => &["html", "htm"],
            Language::Css => &["css"],
            Language::Dockerfile => &[],
            Language::Hcl => &["tf", "tfvars"],
            Language::Yaml => &["yaml", "yml"],
            Language::Markdown => &["md", "markdown"],
            Language::Mdx => &["mdx"],
            Language::Mermaid => &["mmd", "mermaid"],
            Language::Toml => &["toml"],
            Language::Sql => &["sql"],
            Language::Proto => &["proto"],
            Language::GraphQL => &["graphql", "gql"],
        }
    }
}
