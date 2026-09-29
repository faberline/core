use std::collections::{HashMap, HashSet};

use super::{field_name, field_number, import_path, is_snake_case, lr, to_pascal, ProtoChecker};
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};

impl ProtoChecker {
    /// PB001: Brace/semicolon validation.
    pub(super) fn check_syntax(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut depth: i32 = 0;
        let mut open_line: Option<u32> = None;
        for (i, line) in source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("//") || t.is_empty() {
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
                            "PB001",
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
                "PB001",
                DiagnosticCategory::Syntax,
                format!("Unmatched opening brace ({} unclosed)", depth),
            ));
        }
        diags
    }

    /// PB002: Duplicate field numbers within a message.
    pub(super) fn check_duplicate_field_numbers(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut in_msg = false;
        let mut nums: HashMap<String, u32> = HashMap::new();
        let mut msg_name = String::new();
        let mut depth: i32 = 0;
        for (i, line) in source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("//") || t.is_empty() {
                continue;
            }
            if t.starts_with("message ") {
                in_msg = true;
                msg_name = t
                    .strip_prefix("message ")
                    .unwrap_or("")
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string();
                nums.clear();
            }
            for ch in t.chars() {
                if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth == 0 {
                        in_msg = false;
                    }
                }
            }
            if in_msg && depth == 1 && t.contains('=') && t.ends_with(';') {
                if let Some(n) = field_number(t) {
                    if let Some(&prev) = nums.get(&n) {
                        diags.push(Diagnostic::new(
                            lr(i as u32),
                            DiagnosticSeverity::Error,
                            "PB002",
                            DiagnosticCategory::Logic,
                            format!(
                                "Duplicate field number {} in '{}' (first at line {})",
                                n,
                                msg_name,
                                prev + 1
                            ),
                        ));
                    } else {
                        nums.insert(n, i as u32);
                    }
                }
            }
        }
        diags
    }

    /// PB003: Reserved field number used.
    pub(super) fn check_reserved_fields(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut reserved: HashSet<u32> = HashSet::new();
        let mut in_msg = false;
        let mut depth: i32 = 0;
        for (i, line) in source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("//") || t.is_empty() {
                continue;
            }
            if t.starts_with("message ") {
                in_msg = true;
                reserved.clear();
            }
            for ch in t.chars() {
                if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth == 0 {
                        in_msg = false;
                    }
                }
            }
            if in_msg && t.starts_with("reserved ") {
                let nums_part = t
                    .strip_prefix("reserved ")
                    .unwrap_or("")
                    .trim_end_matches(';');
                for part in nums_part.split(',') {
                    let p = part.trim().trim_matches('"');
                    if let Ok(n) = p.parse::<u32>() {
                        reserved.insert(n);
                    } else if p.contains(" to ") {
                        let ps: Vec<&str> = p.split(" to ").collect();
                        if ps.len() == 2 {
                            if let (Ok(a), Ok(b)) =
                                (ps[0].trim().parse::<u32>(), ps[1].trim().parse::<u32>())
                            {
                                for n in a..=b {
                                    reserved.insert(n);
                                }
                            }
                        }
                    }
                }
                continue;
            }
            if in_msg && depth == 1 && t.contains('=') && t.ends_with(';') {
                if let Some(ns) = field_number(t) {
                    if let Ok(n) = ns.parse::<u32>() {
                        if reserved.contains(&n) {
                            diags.push(Diagnostic::new(
                                lr(i as u32),
                                DiagnosticSeverity::Error,
                                "PB003",
                                DiagnosticCategory::Logic,
                                format!("Field number {} is reserved", n),
                            ));
                        }
                    }
                }
            }
        }
        diags
    }

    /// PB004: Missing package declaration.
    pub(super) fn check_missing_package(&self, source: &str) -> Vec<Diagnostic> {
        if source.lines().any(|l| l.trim().starts_with("package ")) {
            Vec::new()
        } else {
            vec![Diagnostic::new(
                lr(0),
                DiagnosticSeverity::Warning,
                "PB004",
                DiagnosticCategory::Style,
                "Missing 'package' declaration",
            )]
        }
    }

    /// PB005: Service without rpc methods.
    pub(super) fn check_empty_service(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut svc: Option<(String, u32)> = None;
        let mut has_rpc = false;
        let mut depth: i32 = 0;
        for (i, line) in source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("//") || t.is_empty() {
                continue;
            }
            if t.starts_with("service ") && svc.is_none() {
                let name = t
                    .strip_prefix("service ")
                    .unwrap_or("")
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string();
                svc = Some((name, i as u32));
                has_rpc = false;
            }
            for ch in t.chars() {
                if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth == 0 {
                        if let Some((ref name, sl)) = svc {
                            if !has_rpc {
                                diags.push(Diagnostic::new(
                                    lr(sl),
                                    DiagnosticSeverity::Warning,
                                    "PB005",
                                    DiagnosticCategory::Logic,
                                    format!("Service '{}' has no rpc methods", name),
                                ));
                            }
                        }
                        svc = None;
                    }
                }
            }
            if svc.is_some() && t.starts_with("rpc ") {
                has_rpc = true;
            }
        }
        diags
    }

    /// PB006: Import not used.
    pub(super) fn check_unused_imports(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut imports: Vec<(String, u32)> = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("import ") {
                if let Some(path) = import_path(t) {
                    imports.push((path, i as u32));
                }
            }
        }
        let rest: String = source
            .lines()
            .filter(|l| !l.trim().starts_with("import "))
            .collect::<Vec<_>>()
            .join("\n");
        for (path, line) in &imports {
            let base = path
                .rsplit('/')
                .next()
                .unwrap_or(path)
                .strip_suffix(".proto")
                .unwrap_or(path);
            let pascal = to_pascal(base);
            if !rest.contains(&pascal) && !rest.contains(base) {
                diags.push(Diagnostic::new(
                    lr(*line),
                    DiagnosticSeverity::Warning,
                    "PB006",
                    DiagnosticCategory::Logic,
                    format!("Import '{}' appears unused", path),
                ));
            }
        }
        diags
    }

    /// PB007: Field naming convention (should be snake_case).
    pub(super) fn check_field_naming(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut in_block = false;
        let mut depth: i32 = 0;
        for (i, line) in source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("//") || t.is_empty() {
                continue;
            }
            if t.starts_with("message ") || t.starts_with("enum ") {
                in_block = true;
            }
            for ch in t.chars() {
                if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth == 0 {
                        in_block = false;
                    }
                }
            }
            if in_block && depth >= 1 && t.contains('=') && t.ends_with(';') {
                if let Some(name) = field_name(t) {
                    if !is_snake_case(name) {
                        diags.push(Diagnostic::new(
                            lr(i as u32),
                            DiagnosticSeverity::Warning,
                            "PB007",
                            DiagnosticCategory::Style,
                            format!("Field '{}' should be snake_case", name),
                        ));
                    }
                }
            }
        }
        diags
    }
}
