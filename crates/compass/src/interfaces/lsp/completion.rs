use tower_lsp::lsp_types::*;

use super::argus_server::ArgusServer;

impl ArgusServer {
    /// Complete attributes after a dot
    pub(super) async fn complete_attribute(&self, prefix: &str) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        // Find the object name before the dot
        let prefix = prefix.trim_end_matches('.');
        let obj_name = prefix.split_whitespace().last().unwrap_or("");

        // Get completions from stubs for known modules
        let stubs = self.stubs.read().await;

        // Check if this might be a module access
        if let Some(module_info) = stubs.get_stub(obj_name) {
            for (name, ty) in &module_info.exports {
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(self.type_to_completion_kind(ty)),
                    detail: Some(ty.to_string()),
                    ..Default::default()
                });
            }
        }

        // Add common completions for known types
        if obj_name.ends_with("str") || obj_name.ends_with('"') || obj_name.ends_with('\'') {
            items.extend(self.string_completions());
        } else if obj_name.ends_with(']') {
            items.extend(self.list_completions());
        } else if obj_name.ends_with('}') {
            items.extend(self.dict_completions());
        }

        items
    }

    /// Complete identifiers (builtins, imports, local variables)
    pub(super) async fn complete_identifiers(&self, prefix: &str) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        // Extract the partial identifier being typed
        let partial = prefix.split_whitespace().last().unwrap_or("");
        let partial_lower = partial.to_lowercase();

        // Get completions from builtins
        let stubs = self.stubs.read().await;

        if let Some(builtins) = stubs.get_stub("builtins") {
            for (name, ty) in &builtins.exports {
                if name.to_lowercase().starts_with(&partial_lower) {
                    items.push(CompletionItem {
                        label: name.clone(),
                        kind: Some(self.type_to_completion_kind(ty)),
                        detail: Some(ty.to_string()),
                        ..Default::default()
                    });
                }
            }
        }

        // Add keywords
        let keywords = [
            "def", "class", "if", "elif", "else", "for", "while", "try", "except", "finally",
            "with", "return", "yield", "import", "from", "as", "pass", "break", "continue",
            "raise", "assert", "global", "nonlocal", "lambda", "True", "False", "None", "and",
            "or", "not", "in", "is", "async", "await", "match", "case",
        ];

        for kw in keywords {
            if kw.to_lowercase().starts_with(&partial_lower) {
                items.push(CompletionItem {
                    label: kw.to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    ..Default::default()
                });
            }
        }

        items
    }

    /// Common string method completions
    fn string_completions(&self) -> Vec<CompletionItem> {
        let methods = [
            ("upper", "() -> str", "Convert to uppercase"),
            ("lower", "() -> str", "Convert to lowercase"),
            ("strip", "() -> str", "Remove leading/trailing whitespace"),
            ("split", "(sep=None) -> list[str]", "Split string"),
            ("join", "(iterable) -> str", "Join strings"),
            ("replace", "(old, new) -> str", "Replace occurrences"),
            ("startswith", "(prefix) -> bool", "Check if starts with"),
            ("endswith", "(suffix) -> bool", "Check if ends with"),
            ("find", "(sub) -> int", "Find substring index"),
            ("format", "(*args, **kwargs) -> str", "Format string"),
        ];

        methods
            .into_iter()
            .map(|(name, sig, doc)| CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::METHOD),
                detail: Some(sig.to_string()),
                documentation: Some(Documentation::String(doc.to_string())),
                ..Default::default()
            })
            .collect()
    }

    /// Common list method completions
    fn list_completions(&self) -> Vec<CompletionItem> {
        let methods = [
            ("append", "(x)", "Add item to end"),
            ("extend", "(iterable)", "Extend with iterable"),
            ("insert", "(i, x)", "Insert at index"),
            ("remove", "(x)", "Remove first occurrence"),
            ("pop", "(i=-1)", "Remove and return item"),
            ("clear", "()", "Remove all items"),
            ("index", "(x)", "Return index of x"),
            ("count", "(x)", "Count occurrences"),
            ("sort", "()", "Sort in place"),
            ("reverse", "()", "Reverse in place"),
            ("copy", "()", "Return shallow copy"),
        ];

        methods
            .into_iter()
            .map(|(name, sig, doc)| CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::METHOD),
                detail: Some(sig.to_string()),
                documentation: Some(Documentation::String(doc.to_string())),
                ..Default::default()
            })
            .collect()
    }

    /// Common dict method completions
    fn dict_completions(&self) -> Vec<CompletionItem> {
        let methods = [
            ("get", "(key, default=None)", "Get value with default"),
            ("keys", "()", "Return keys view"),
            ("values", "()", "Return values view"),
            ("items", "()", "Return items view"),
            ("pop", "(key, default=None)", "Remove and return value"),
            ("update", "(other)", "Update from dict/iterable"),
            ("setdefault", "(key, default=None)", "Get or set default"),
            ("clear", "()", "Remove all items"),
            ("copy", "()", "Return shallow copy"),
        ];

        methods
            .into_iter()
            .map(|(name, sig, doc)| CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::METHOD),
                detail: Some(sig.to_string()),
                documentation: Some(Documentation::String(doc.to_string())),
                ..Default::default()
            })
            .collect()
    }

    /// Convert Argus type to LSP completion item kind
    fn type_to_completion_kind(&self, ty: &crate::type_inference::Type) -> CompletionItemKind {
        use crate::type_inference::Type;

        match ty {
            Type::Callable { .. } => CompletionItemKind::FUNCTION,
            Type::ClassType { .. } => CompletionItemKind::CLASS,
            Type::Instance { .. } => CompletionItemKind::VARIABLE,
            _ => CompletionItemKind::VALUE,
        }
    }
}
