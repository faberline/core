use std::collections::HashMap;

use super::checker::Checker;
use super::css::CssChecker;
use super::dockerfile::DockerfileChecker;
use super::go::GoChecker;
use super::graphql::GraphqlChecker;
use super::html::HtmlChecker;
use super::javascript::JavaScriptChecker;
use super::markdown::MarkdownChecker;
use super::mdx::MdxChecker;
use super::mermaid::MermaidChecker;
use super::proto::ProtoChecker;
use super::python::PythonChecker;
use super::rust_checker::RustChecker;
use super::sql::SqlChecker;
use super::terraform::TerraformChecker;
use super::toml_checker::TomlChecker;
use super::typescript::TypeScriptChecker;
use super::yaml_dispatch::YamlDispatcher;
use crate::syntax::Language;

/// Registry of all checkers
pub struct CheckerRegistry {
    checkers: HashMap<Language, Box<dyn Checker>>,
}

impl CheckerRegistry {
    pub fn new() -> Self {
        let mut checkers: HashMap<Language, Box<dyn Checker>> = HashMap::new();

        checkers.insert(Language::Python, Box::new(PythonChecker::new()));
        checkers.insert(Language::TypeScript, Box::new(TypeScriptChecker::new()));
        checkers.insert(Language::Rust, Box::new(RustChecker::new()));
        checkers.insert(Language::JavaScript, Box::new(JavaScriptChecker::new()));
        checkers.insert(Language::Go, Box::new(GoChecker::new()));
        checkers.insert(Language::Html, Box::new(HtmlChecker::new()));
        checkers.insert(Language::Css, Box::new(CssChecker::new()));
        checkers.insert(Language::Dockerfile, Box::new(DockerfileChecker));
        checkers.insert(Language::Hcl, Box::new(TerraformChecker));
        checkers.insert(Language::Yaml, Box::new(YamlDispatcher::new()));
        checkers.insert(Language::Markdown, Box::new(MarkdownChecker::new()));
        checkers.insert(Language::Mdx, Box::new(MdxChecker::new()));
        checkers.insert(Language::Mermaid, Box::new(MermaidChecker::new()));
        checkers.insert(Language::Toml, Box::new(TomlChecker::new()));
        checkers.insert(Language::Sql, Box::new(SqlChecker::new()));
        checkers.insert(Language::Proto, Box::new(ProtoChecker::new()));
        checkers.insert(Language::GraphQL, Box::new(GraphqlChecker::new()));

        Self { checkers }
    }

    pub fn get(&self, language: Language) -> Option<&dyn Checker> {
        self.checkers.get(&language).map(|c| c.as_ref())
    }
}

impl Default for CheckerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
