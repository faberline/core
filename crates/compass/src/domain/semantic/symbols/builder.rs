use super::SymbolTable;
use crate::domain::syntax::parsed_file::ParsedFile;

/// Build symbol table from parsed file
pub struct SymbolTableBuilder {
    pub(crate) table: SymbolTable,
    pub(crate) current_scope: usize,
    pub(crate) scope_stack: Vec<usize>,
    pub(crate) next_scope: usize,
}

impl SymbolTableBuilder {
    pub fn new() -> Self {
        Self {
            table: SymbolTable::new(),
            current_scope: 0,
            scope_stack: vec![0],
            next_scope: 1,
        }
    }

    /// Build symbol table for a Python file
    pub fn build_python(mut self, file: &ParsedFile) -> SymbolTable {
        self.visit_python_node(&file.root_node(), file);
        self.table
    }

    /// Build symbol table for a Rust file
    pub fn build_rust(mut self, file: &ParsedFile) -> SymbolTable {
        self.visit_rust_node(&file.root_node(), file);
        self.table
    }

    /// Build symbol table for a JavaScript file (delegates to TypeScript)
    pub fn build_javascript(self, file: &ParsedFile) -> SymbolTable {
        self.build_typescript(file)
    }

    /// Build symbol table for a TypeScript file
    pub fn build_typescript(mut self, file: &ParsedFile) -> SymbolTable {
        self.visit_typescript_node(&file.root_node(), file);
        self.table
    }

    /// Build symbol table for a Go file
    pub fn build_go(mut self, file: &ParsedFile) -> SymbolTable {
        self.visit_go_node(&file.root_node(), file);
        self.table
    }

    /// Build symbol table for a Dockerfile (line-based)
    pub fn build_dockerfile(mut self, file: &ParsedFile) -> SymbolTable {
        self.visit_dockerfile_lines(&file.source);
        self.table
    }

    /// Build symbol table for Dockerfile from raw source (test helper)
    #[cfg(test)]
    pub fn build_dockerfile_from_source(mut self, source: &str) -> SymbolTable {
        self.visit_dockerfile_lines(source);
        self.table
    }

    /// Build symbol table for Terraform/HCL files
    pub fn build_terraform(mut self, file: &ParsedFile) -> SymbolTable {
        self.visit_hcl_node(&file.root_node(), file);
        self.table
    }

    /// Build symbol table for Kubernetes YAML manifests
    pub fn build_kubernetes(mut self, file: &ParsedFile) -> SymbolTable {
        self.visit_k8s_node(&file.root_node(), file);
        self.table
    }

    /// Build symbol table for GitLab CI YAML
    pub fn build_gitlab_ci(mut self, file: &ParsedFile) -> SymbolTable {
        self.visit_gitlab_ci_lines(&file.source);
        self.table
    }

    /// Build symbol table for GitLab CI from raw source (test helper)
    #[cfg(test)]
    pub fn build_gitlab_ci_from_source(mut self, source: &str) -> SymbolTable {
        self.visit_gitlab_ci_lines(source);
        self.table
    }

    /// Build symbol table for a Markdown file (line-based)
    pub fn build_markdown(mut self, file: &ParsedFile) -> SymbolTable {
        self.visit_markdown_lines(&file.source);
        self.table
    }

    /// Build symbol table for Markdown from raw source (test helper)
    #[cfg(test)]
    pub fn build_markdown_from_source(mut self, source: &str) -> SymbolTable {
        self.visit_markdown_lines(source);
        self.table
    }

    /// Build symbol table for a Mermaid diagram file (line-based)
    pub fn build_mermaid(mut self, file: &ParsedFile) -> SymbolTable {
        self.visit_mermaid_lines(&file.source);
        self.table
    }

    /// Build symbol table for Mermaid from raw source (test helper)
    #[cfg(test)]
    pub fn build_mermaid_from_source(mut self, source: &str) -> SymbolTable {
        self.visit_mermaid_lines(source);
        self.table
    }

    pub(crate) fn push_scope(&mut self) {
        self.scope_stack.push(self.current_scope);
        self.current_scope = self.next_scope;
        self.next_scope += 1;
    }

    pub(crate) fn pop_scope(&mut self) {
        if let Some(parent) = self.scope_stack.pop() {
            self.current_scope = parent;
        }
    }
}

impl Default for SymbolTableBuilder {
    fn default() -> Self {
        Self::new()
    }
}
