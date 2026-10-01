use std::collections::{HashMap, HashSet};

use super::{lr, GraphqlChecker};
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};

impl GraphqlChecker {
    // =========================================================================
    // Line-based fallback (used for tests / dummy trees)
    // =========================================================================

    pub(super) fn lb_check_syntax(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut depth: i32 = 0;
        let mut open_line: Option<u32> = None;
        for (i, line) in source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with('#') || t.is_empty() {
                continue;
            }
            for ch in t.chars() {
                if ch == '{' {
                    if depth == 0 {
                        open_line = Some(i as u32);
                    }
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth < 0 {
                        diags.push(Diagnostic::new(
                            lr(i as u32),
                            DiagnosticSeverity::Error,
                            "GQ001",
                            DiagnosticCategory::Syntax,
                            "Unmatched closing brace",
                        ));
                        depth = 0;
                    }
                }
            }
        }
        if depth > 0 {
            diags.push(Diagnostic::new(
                lr(open_line.unwrap_or(0)),
                DiagnosticSeverity::Error,
                "GQ001",
                DiagnosticCategory::Syntax,
                format!("Unmatched opening brace ({} unclosed)", depth),
            ));
        }
        diags
    }

    pub(super) fn lb_check_undefined_types(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut defined: HashSet<String> = ["String", "Int", "Float", "Boolean", "ID"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut refs: Vec<(String, u32)> = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with('#') || t.is_empty() {
                continue;
            }
            for kw in &[
                "type ",
                "input ",
                "interface ",
                "enum ",
                "union ",
                "scalar ",
            ] {
                if t.starts_with(kw) {
                    let name = t[kw.len()..].split_whitespace().next().unwrap_or("");
                    if !name.is_empty() {
                        defined.insert(name.to_string());
                    }
                }
            }
            if t.contains(':') {
                if let Some(cp) = t.find(':') {
                    let tp = t[cp + 1..]
                        .trim()
                        .trim_start_matches('[')
                        .trim_end_matches('!')
                        .trim_end_matches(']')
                        .trim_end_matches('!')
                        .trim();
                    if !tp.is_empty()
                        && tp.chars().next().map_or(false, |c| c.is_uppercase())
                        && !tp.contains('(')
                    {
                        refs.push((tp.to_string(), i as u32));
                    }
                }
            }
        }
        for (tn, line) in &refs {
            if !defined.contains(tn.as_str()) {
                diags.push(Diagnostic::new(
                    lr(*line),
                    DiagnosticSeverity::Warning,
                    "GQ002",
                    DiagnosticCategory::Names,
                    format!("Reference to undefined type '{}'", tn),
                ));
            }
        }
        diags
    }

    pub(super) fn lb_check_deprecated(&self, source: &str) -> Vec<Diagnostic> {
        source
            .lines()
            .enumerate()
            .filter(|(_, l)| l.contains("@deprecated"))
            .map(|(i, _)| {
                Diagnostic::new(
                    lr(i as u32),
                    DiagnosticSeverity::Warning,
                    "GQ003",
                    DiagnosticCategory::Logic,
                    "Field marked @deprecated — consider removing or updating usage",
                )
            })
            .collect()
    }

    pub(super) fn lb_check_deep_nesting(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut depth: i32 = 0;
        let mut warned: HashSet<u32> = HashSet::new();
        for (i, line) in source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with('#') || t.is_empty() {
                continue;
            }
            for ch in t.chars() {
                if ch == '{' {
                    depth += 1;
                    if depth > 5 && !warned.contains(&(i as u32)) {
                        warned.insert(i as u32);
                        diags.push(Diagnostic::new(
                            lr(i as u32),
                            DiagnosticSeverity::Warning,
                            "GQ004",
                            DiagnosticCategory::Style,
                            format!("Nesting depth {} exceeds maximum of 5", depth),
                        ));
                    }
                } else if ch == '}' {
                    depth -= 1;
                    if depth < 0 {
                        depth = 0;
                    }
                }
            }
        }
        diags
    }

    pub(super) fn lb_check_missing_descriptions(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim();
            for kw in &["type ", "input ", "interface "] {
                if t.starts_with(kw) {
                    let name = t[kw.len()..].split_whitespace().next().unwrap_or("");
                    if matches!(name, "Query" | "Mutation" | "Subscription") {
                        continue;
                    }
                    let has_desc = i > 0 && {
                        let prev = lines[i - 1].trim();
                        prev.starts_with('#') || prev.starts_with("\"\"\"") || prev.starts_with('"')
                    };
                    if !has_desc && !name.is_empty() {
                        diags.push(Diagnostic::new(
                            lr(i as u32),
                            DiagnosticSeverity::Hint,
                            "GQ005",
                            DiagnosticCategory::Style,
                            format!("Type '{}' has no description", name),
                        ));
                    }
                }
            }
        }
        diags
    }

    pub(super) fn lb_check_unused_fragments(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut defs: HashMap<String, u32> = HashMap::new();
        let mut spreads: HashSet<String> = HashSet::new();
        for (i, line) in source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with('#') || t.is_empty() {
                continue;
            }
            if t.starts_with("fragment ") {
                let name = t
                    .strip_prefix("fragment ")
                    .unwrap_or("")
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                if !name.is_empty() {
                    defs.insert(name.to_string(), i as u32);
                }
            }
            let mut s = t;
            while let Some(pos) = s.find("...") {
                let after = &s[pos + 3..];
                let sn = after
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("");
                if !sn.is_empty() && sn != "on" {
                    spreads.insert(sn.to_string());
                }
                if pos + 3 >= s.len() {
                    break;
                }
                s = &s[pos + 3..];
            }
        }
        for (name, line) in &defs {
            if !spreads.contains(name) {
                diags.push(Diagnostic::new(
                    lr(*line),
                    DiagnosticSeverity::Warning,
                    "GQ006",
                    DiagnosticCategory::Logic,
                    format!("Fragment '{}' is defined but never used", name),
                ));
            }
        }
        diags
    }

    pub(super) fn lb_check_duplicate_fields(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut depth_fields: Vec<HashMap<String, u32>> = vec![HashMap::new()];
        let mut depth: usize = 0;
        let skip = [
            "...",
            "query",
            "mutation",
            "subscription",
            "fragment",
            "type",
            "input",
            "interface",
            "enum",
        ];
        for (i, line) in source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with('#') || t.is_empty() {
                continue;
            }
            for ch in t.chars() {
                if ch == '{' {
                    depth += 1;
                    if depth >= depth_fields.len() {
                        depth_fields.push(HashMap::new());
                    } else {
                        depth_fields[depth].clear();
                    }
                } else if ch == '}' {
                    if depth > 0 {
                        depth_fields[depth].clear();
                        depth -= 1;
                    }
                }
            }
            if !t.contains('{')
                && !t.contains('}')
                && depth > 0
                && !skip.iter().any(|s| t.starts_with(s))
            {
                let field = t
                    .split(|c: char| c == '(' || c == ':' || c == '@' || c.is_whitespace())
                    .next()
                    .unwrap_or("");
                if !field.is_empty()
                    && field.chars().next().map_or(false, |c| c.is_alphabetic())
                    && depth < depth_fields.len()
                {
                    if let Some(&prev) = depth_fields[depth].get(field) {
                        diags.push(Diagnostic::new(
                            lr(i as u32),
                            DiagnosticSeverity::Warning,
                            "GQ007",
                            DiagnosticCategory::Logic,
                            format!("Duplicate field '{}' (first at line {})", field, prev + 1),
                        ));
                    } else {
                        depth_fields[depth].insert(field.to_string(), i as u32);
                    }
                }
            }
        }
        diags
    }
}
