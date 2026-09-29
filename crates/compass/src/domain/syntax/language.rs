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
