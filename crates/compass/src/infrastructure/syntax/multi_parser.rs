use crate::domain::error::argus_error::{ArgusError, Result};
use crate::domain::syntax::language::Language;
use crate::domain::syntax::parsed_file::ParsedFile;
use std::path::Path;
use tree_sitter::Parser;

/// Multi-language parser
pub struct MultiParser {
    python_parser: Parser,
    typescript_parser: Parser,
    rust_parser: Parser,
    javascript_parser: Parser,
    go_parser: Parser,
    html_parser: Parser,
    css_parser: Parser,
    hcl_parser: Parser,
    yaml_parser: Parser,
    // R3: AST grammars for SQL, Protobuf, GraphQL, TOML
    sql_parser: Parser,
    proto_parser: Parser,
    graphql_parser: Parser,
    toml_parser: Parser,
}

impl MultiParser {
    pub fn new() -> Result<Self> {
        let mut python_parser = Parser::new();
        python_parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .map_err(|e| ArgusError::parser(format!("Failed to load Python grammar: {}", e)))?;

        let mut typescript_parser = Parser::new();
        typescript_parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            .map_err(|e| ArgusError::parser(format!("Failed to load TypeScript grammar: {}", e)))?;

        let mut rust_parser = Parser::new();
        rust_parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .map_err(|e| ArgusError::parser(format!("Failed to load Rust grammar: {}", e)))?;

        let mut javascript_parser = Parser::new();
        javascript_parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .map_err(|e| ArgusError::parser(format!("Failed to load JavaScript grammar: {}", e)))?;

        let mut go_parser = Parser::new();
        go_parser
            .set_language(&tree_sitter_go::LANGUAGE.into())
            .map_err(|e| ArgusError::parser(format!("Failed to load Go grammar: {}", e)))?;

        let mut html_parser = Parser::new();
        html_parser
            .set_language(&tree_sitter_html::LANGUAGE.into())
            .map_err(|e| ArgusError::parser(format!("Failed to load HTML grammar: {}", e)))?;

        let mut css_parser = Parser::new();
        css_parser
            .set_language(&tree_sitter_css::LANGUAGE.into())
            .map_err(|e| ArgusError::parser(format!("Failed to load CSS grammar: {}", e)))?;

        let mut hcl_parser = Parser::new();
        hcl_parser
            .set_language(&tree_sitter_hcl::LANGUAGE.into())
            .map_err(|e| ArgusError::parser(format!("Failed to load HCL grammar: {}", e)))?;

        let mut yaml_parser = Parser::new();
        yaml_parser
            .set_language(&tree_sitter_yaml::LANGUAGE.into())
            .map_err(|e| ArgusError::parser(format!("Failed to load YAML grammar: {}", e)))?;

        // R3: AST grammars for SQL, Protobuf, GraphQL, TOML
        // Crate names: tree-sitter-sequel, tree-sitter-proto,
        //              tree-sitter-graphql, tree-sitter-toml-ng
        let mut sql_parser = Parser::new();
        sql_parser
            .set_language(&tree_sitter_sequel::LANGUAGE.into())
            .map_err(|e| ArgusError::parser(format!("Failed to load SQL grammar: {}", e)))?;

        let mut proto_parser = Parser::new();
        proto_parser
            .set_language(&tree_sitter_proto::LANGUAGE.into())
            .map_err(|e| ArgusError::parser(format!("Failed to load Protobuf grammar: {}", e)))?;

        let mut graphql_parser = Parser::new();
        graphql_parser
            .set_language(&tree_sitter_graphql::LANGUAGE.into())
            .map_err(|e| ArgusError::parser(format!("Failed to load GraphQL grammar: {}", e)))?;

        let mut toml_parser = Parser::new();
        toml_parser
            .set_language(&tree_sitter_toml_ng::LANGUAGE.into())
            .map_err(|e| ArgusError::parser(format!("Failed to load TOML grammar: {}", e)))?;

        Ok(Self {
            python_parser,
            typescript_parser,
            rust_parser,
            javascript_parser,
            go_parser,
            html_parser,
            css_parser,
            hcl_parser,
            yaml_parser,
            sql_parser,
            proto_parser,
            graphql_parser,
            toml_parser,
        })
    }

    /// Detect language from file path (extension + filename)
    pub fn detect_language(path: &Path) -> Option<Language> {
        // Check filename first (for Dockerfile)
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

    /// Parse source code
    pub fn parse(&mut self, source: &str, language: Language) -> Option<ParsedFile> {
        let parser = match language {
            Language::Python => &mut self.python_parser,
            Language::TypeScript => &mut self.typescript_parser,
            Language::Rust => &mut self.rust_parser,
            Language::JavaScript => &mut self.javascript_parser,
            Language::Go => &mut self.go_parser,
            Language::Html => &mut self.html_parser,
            Language::Css => &mut self.css_parser,
            Language::Dockerfile => return None, // line-based parsing, no tree-sitter
            Language::Hcl => &mut self.hcl_parser,
            Language::Yaml => &mut self.yaml_parser,
            Language::Markdown | Language::Mdx | Language::Mermaid => return None, // line-based
            // R3: AST grammars for SQL, Protobuf, GraphQL, TOML
            Language::Sql => &mut self.sql_parser,
            Language::Proto => &mut self.proto_parser,
            Language::GraphQL => &mut self.graphql_parser,
            Language::Toml => &mut self.toml_parser,
        };

        let tree = parser.parse(source, None)?;
        let has_errors = tree.root_node().has_error();

        Some(ParsedFile {
            source: source.to_string(),
            tree,
            language,
            has_errors,
            is_line_based: false,
        })
    }
}
