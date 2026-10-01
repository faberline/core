//! Python completions: stub module exports after a dot, builtins and
//! keywords otherwise, and the common str, list and dict methods.

use crate::application::editor::session::EditorSession;
use crate::application::editor::views::{CompletionCandidate, CompletionKind};
use crate::domain::syntax::language::Language;
use crate::domain::type_system::ty::Type;

const KEYWORDS: [&str; 36] = [
    "def", "class", "if", "elif", "else", "for", "while", "try", "except", "finally", "with",
    "return", "yield", "import", "from", "as", "pass", "break", "continue", "raise", "assert",
    "global", "nonlocal", "lambda", "True", "False", "None", "and", "or", "not", "in", "is",
    "async", "await", "match", "case",
];

const STRING_METHODS: [(&str, &str, &str); 10] = [
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

const LIST_METHODS: [(&str, &str, &str); 11] = [
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

const DICT_METHODS: [(&str, &str, &str); 9] = [
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

impl EditorSession {
    /// Completion candidates at a position of an open Python document.
    ///
    /// `dot_trigger` is true when the client triggered completion with a
    /// dot. Returns `None` when the document is not open, is not Python, or
    /// the line is past its end.
    pub(crate) async fn completions(
        &self,
        uri: &str,
        line: u32,
        character: u32,
        dot_trigger: bool,
    ) -> Option<Vec<CompletionCandidate>> {
        let (content, language) = {
            let documents = self.documents.read().await;
            documents
                .get(uri)
                .map(|d| (d.content.clone(), d.language))?
        };

        // Only provide Python completions for now
        if language != Language::Python {
            return None;
        }

        let lines: Vec<&str> = content.lines().collect();
        let line = *lines.get(line as usize)?;
        let col = character as usize;
        let prefix = &line[..col.min(line.len())];

        if prefix.ends_with('.') || dot_trigger {
            Some(self.complete_attribute(prefix))
        } else {
            Some(self.complete_identifiers(prefix))
        }
    }

    /// Complete attributes after a dot
    fn complete_attribute(&self, prefix: &str) -> Vec<CompletionCandidate> {
        let mut items = Vec::new();

        // Find the object name before the dot
        let prefix = prefix.trim_end_matches('.');
        let obj_name = prefix.split_whitespace().last().unwrap_or("");

        // Check if this might be a module access
        if let Some(module_info) = self.modules.get(obj_name) {
            for (name, ty) in &module_info.exports {
                items.push(export_candidate(name, ty));
            }
        }

        // Add common completions for known types
        if obj_name.ends_with("str") || obj_name.ends_with('"') || obj_name.ends_with('\'') {
            items.extend(method_candidates(&STRING_METHODS));
        } else if obj_name.ends_with(']') {
            items.extend(method_candidates(&LIST_METHODS));
        } else if obj_name.ends_with('}') {
            items.extend(method_candidates(&DICT_METHODS));
        }

        items
    }

    /// Complete identifiers (builtins and keywords)
    fn complete_identifiers(&self, prefix: &str) -> Vec<CompletionCandidate> {
        let mut items = Vec::new();

        // Extract the partial identifier being typed
        let partial = prefix.split_whitespace().last().unwrap_or("");
        let partial_lower = partial.to_lowercase();

        if let Some(builtins) = self.modules.get("builtins") {
            for (name, ty) in &builtins.exports {
                if name.to_lowercase().starts_with(&partial_lower) {
                    items.push(export_candidate(name, ty));
                }
            }
        }

        for kw in KEYWORDS {
            if kw.to_lowercase().starts_with(&partial_lower) {
                items.push(CompletionCandidate {
                    label: kw.to_string(),
                    kind: CompletionKind::Keyword,
                    detail: None,
                    documentation: None,
                });
            }
        }

        items
    }
}

/// A module export, with its type as the detail.
fn export_candidate(name: &str, ty: &Type) -> CompletionCandidate {
    CompletionCandidate {
        label: name.to_string(),
        kind: type_kind(ty),
        detail: Some(ty.to_string()),
        documentation: None,
    }
}

/// Method candidates from (name, signature, doc) triples.
fn method_candidates(methods: &[(&str, &str, &str)]) -> Vec<CompletionCandidate> {
    methods
        .iter()
        .map(|(name, sig, doc)| CompletionCandidate {
            label: name.to_string(),
            kind: CompletionKind::Method,
            detail: Some(sig.to_string()),
            documentation: Some(doc.to_string()),
        })
        .collect()
}

/// The completion kind of a type.
fn type_kind(ty: &Type) -> CompletionKind {
    match ty {
        Type::Callable { .. } => CompletionKind::Function,
        Type::ClassType { .. } => CompletionKind::Class,
        Type::Instance { .. } => CompletionKind::Variable,
        _ => CompletionKind::Value,
    }
}
