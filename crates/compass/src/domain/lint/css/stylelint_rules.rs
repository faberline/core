use crate::domain::syntax::parsed_file::NodeRange;
use std::collections::HashMap;

use super::CssChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};
use crate::syntax::ParsedFile;

impl CssChecker {
    /// CSS006: no-id-selectors — detect #id selectors
    pub(super) fn check_no_id_selectors(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "id_selector" {
                diagnostics.push(Diagnostic::new(
                    node.to_range(),
                    DiagnosticSeverity::Information,
                    "CSS006",
                    DiagnosticCategory::Style,
                    "Avoid ID selectors — they have high specificity and are hard to override",
                ));
            }
            true
        });
        diagnostics
    }

    /// CSS007: shorthand-property-overrides — longhand after shorthand in same rule
    pub(super) fn check_shorthand_overrides(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let shorthands = [
            (
                "margin",
                &["margin-top", "margin-right", "margin-bottom", "margin-left"][..],
            ),
            (
                "padding",
                &[
                    "padding-top",
                    "padding-right",
                    "padding-bottom",
                    "padding-left",
                ],
            ),
            (
                "border",
                &[
                    "border-top",
                    "border-right",
                    "border-bottom",
                    "border-left",
                    "border-width",
                    "border-style",
                    "border-color",
                ],
            ),
            (
                "background",
                &[
                    "background-color",
                    "background-image",
                    "background-position",
                ],
            ),
        ];
        file.walk(|node, _depth| {
            if node.kind() == "rule_set" || node.kind() == "block" {
                let mut seen_short: Vec<&str> = Vec::new();
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "declaration" {
                        if let Some(prop) = child.child_by_field_name("name") {
                            let name = file.node_text(&prop).trim().to_string();
                            // Check if this is a shorthand
                            for &(short, _) in &shorthands {
                                if name == short {
                                    seen_short.push(short);
                                }
                            }
                            // Check if this longhand follows its shorthand
                            for &(short, longs) in &shorthands {
                                if seen_short.contains(&short) && longs.contains(&name.as_str()) {
                                    diagnostics.push(Diagnostic::warning(
                                        child.to_range(),
                                        "CSS007",
                                        DiagnosticCategory::Logic,
                                        format!(
                                            "'{}' overrides part of shorthand '{}'",
                                            name, short
                                        ),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// CSS008: z-index-max — z-index > 9999
    pub(super) fn check_z_index_max(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "declaration" {
                if let Some(prop) = node.child_by_field_name("name") {
                    if file.node_text(&prop).trim() == "z-index" {
                        if let Some(val) = node.child_by_field_name("value") {
                            let val_text = file.node_text(&val).trim().to_string();
                            if let Ok(n) = val_text.parse::<i64>() {
                                if n > 9999 {
                                    diagnostics.push(Diagnostic::warning(
                                        node.to_range(),
                                        "CSS008",
                                        DiagnosticCategory::Style,
                                        format!(
                                            "z-index value {} exceeds 9999 — use a lower value",
                                            n
                                        ),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// CSS009: color-no-invalid-hex — invalid hex color length
    pub(super) fn check_invalid_hex_color(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "color_value" {
                let text = file.node_text(node).trim().to_string();
                if text.starts_with('#') {
                    let hex_part = &text[1..];
                    let len = hex_part.len();
                    if len != 3 && len != 4 && len != 6 && len != 8 {
                        diagnostics.push(Diagnostic::error(
                            node.to_range(),
                            "CSS009",
                            DiagnosticCategory::Syntax,
                            format!(
                                "Invalid hex color '{}' — must be 3, 4, 6, or 8 hex digits",
                                text
                            ),
                        ));
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// CSS010: font-family-no-missing-generic — font-family without generic fallback
    pub(super) fn check_font_family_generic(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let generics = [
            "serif",
            "sans-serif",
            "monospace",
            "cursive",
            "fantasy",
            "system-ui",
            "ui-serif",
            "ui-sans-serif",
            "ui-monospace",
            "ui-rounded",
        ];
        file.walk(|node, _depth| {
            if node.kind() == "declaration" {
                if let Some(prop) = node.child_by_field_name("name") {
                    if file.node_text(&prop).trim() == "font-family" {
                        if let Some(val) = node.child_by_field_name("value") {
                            let val_text = file.node_text(&val).to_lowercase();
                            let has_generic = generics.iter().any(|g| val_text.contains(g));
                            if !has_generic {
                                diagnostics.push(Diagnostic::warning(
                                    node.to_range(), "CSS010", DiagnosticCategory::Style,
                                    "font-family missing a generic family keyword (e.g., sans-serif)",
                                ));
                            }
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// CSS011: no-duplicate-properties — duplicate property names in same rule
    pub(super) fn check_duplicate_properties(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "rule_set" || node.kind() == "block" {
                let mut seen: HashMap<String, usize> = HashMap::new();
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "declaration" {
                        if let Some(prop) = child.child_by_field_name("name") {
                            let name = file.node_text(&prop).trim().to_string();
                            if let Some(&prev_line) = seen.get(&name) {
                                diagnostics.push(Diagnostic::warning(
                                    child.to_range(),
                                    "CSS011",
                                    DiagnosticCategory::Style,
                                    format!(
                                        "Duplicate property '{}' (first at line {})",
                                        name, prev_line
                                    ),
                                ));
                            } else {
                                seen.insert(name, prop.start_position().row + 1);
                            }
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// CSS012: declaration-block-no-shorthand-override — shorthand after longhand
    pub(super) fn check_shorthand_after_longhand(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let shorthands = [
            (
                "margin",
                &["margin-top", "margin-right", "margin-bottom", "margin-left"][..],
            ),
            (
                "padding",
                &[
                    "padding-top",
                    "padding-right",
                    "padding-bottom",
                    "padding-left",
                ],
            ),
            (
                "border",
                &["border-top", "border-right", "border-bottom", "border-left"],
            ),
            (
                "background",
                &[
                    "background-color",
                    "background-image",
                    "background-position",
                ],
            ),
        ];
        file.walk(|node, _depth| {
            if node.kind() == "rule_set" || node.kind() == "block" {
                let mut seen_long: Vec<String> = Vec::new();
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "declaration" {
                        if let Some(prop) = child.child_by_field_name("name") {
                            let name = file.node_text(&prop).trim().to_string();
                            for &(short, longs) in &shorthands {
                                if longs.contains(&name.as_str()) {
                                    seen_long.push(name.clone());
                                }
                                if name == short && seen_long.iter().any(|l| longs.contains(&l.as_str())) {
                                    diagnostics.push(Diagnostic::warning(
                                        child.to_range(), "CSS012", DiagnosticCategory::Logic,
                                        format!("Shorthand '{}' overrides preceding longhand properties", short),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// CSS013: no-descending-specificity — #id rule after .class rule
    pub(super) fn check_descending_specificity(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut seen_class = false;
        file.walk(|node, _depth| {
            if node.kind() == "class_selector" {
                seen_class = true;
            }
            if node.kind() == "id_selector" && seen_class {
                diagnostics.push(Diagnostic::new(
                    node.to_range(),
                    DiagnosticSeverity::Information,
                    "CSS013",
                    DiagnosticCategory::Style,
                    "ID selector appears after class selector — potential specificity issue",
                ));
            }
            true
        });
        diagnostics
    }

    /// CSS014: unit-no-unknown — detect non-standard CSS units
    pub(super) fn check_unknown_units(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let known = [
            "px", "em", "rem", "%", "vh", "vw", "vmin", "vmax", "ch", "ex", "cm", "mm", "in", "pt",
            "pc", "s", "ms", "deg", "rad", "grad", "turn", "fr", "dpi", "dpcm", "dppx", "lh",
            "rlh", "dvh", "dvw", "svh", "svw", "lvh", "lvw", "cqw", "cqh",
        ];
        file.walk(|node, _depth| {
            if node.kind() == "declaration" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "plain_value" {
                        let text = file.node_text(&child).trim().to_string();
                        // Check if it looks like number+unit
                        let unit_start = text.find(|c: char| c.is_alphabetic());
                        if let Some(idx) = unit_start {
                            if text[..idx].parse::<f64>().is_ok() {
                                let unit = &text[idx..];
                                if !known.contains(&unit) {
                                    diagnostics.push(Diagnostic::warning(
                                        child.to_range(),
                                        "CSS014",
                                        DiagnosticCategory::Syntax,
                                        format!("Unknown CSS unit '{}'", unit),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }

    /// CSS015: block-no-empty — detect empty blocks in at-rules (e.g. @media)
    pub(super) fn check_empty_at_rule_blocks(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "at_rule"
                || node.kind() == "media_statement"
                || node.kind() == "supports_statement"
                || node.kind() == "keyframes_statement"
            {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "block" {
                        let mut inner = child.walk();
                        let has_content = child.children(&mut inner).any(|c| {
                            c.kind() != "{" && c.kind() != "}" && !c.kind().contains("comment")
                        });
                        if !has_content {
                            diagnostics.push(Diagnostic::warning(
                                node.to_range(),
                                "CSS015",
                                DiagnosticCategory::Style,
                                "Empty block — remove the at-rule or add declarations",
                            ));
                        }
                    }
                }
            }
            true
        });
        diagnostics
    }
}
