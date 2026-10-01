// ============================================================================
// MarkdownSymbol — tree-sitter-md style symbol extraction
// ============================================================================

/// A symbol extracted from a Markdown / MDX document.
#[derive(Debug, Clone, PartialEq)]
pub enum MarkdownSymbol {
    /// ATX heading (`# Title`)
    Heading {
        level: usize,
        text: String,
        line: u32,
    },
    /// Inline or reference link
    Link {
        text: String,
        url: String,
        line: u32,
    },
    /// Fenced code block
    CodeFence {
        language: Option<String>,
        start_line: u32,
    },
    /// MDX component invocation (`<MyComponent />` or `<MyComponent>...</MyComponent>`)
    MdxComponent { name: String, line: u32 },
    /// Front-matter field
    FrontMatterField {
        key: String,
        value: String,
        line: u32,
    },
}

/// Extracts structural symbols from Markdown / MDX source.
///
/// This provides the same information that `tree-sitter-md` would give through
/// its AST — headings, links, code fences, MDX components, and front-matter
/// fields — without requiring the grammar crate.
pub struct MarkdownSymbolExtractor;

impl MarkdownSymbolExtractor {
    /// Extract all symbols from a raw Markdown/MDX source string.
    pub fn extract(source: &str) -> Vec<MarkdownSymbol> {
        let mut symbols = Vec::new();
        let mut in_code_block = false;
        let mut in_frontmatter = false;
        let mut frontmatter_done = false;

        for (line_idx, line) in source.lines().enumerate() {
            let line_num = line_idx as u32;
            let trimmed = line.trim();

            // --- Front-matter (first block only) ---
            if line_num == 0 && trimmed == "---" {
                in_frontmatter = true;
                continue;
            }
            if in_frontmatter {
                if trimmed == "---" || trimmed == "..." {
                    in_frontmatter = false;
                    frontmatter_done = true;
                } else if let Some(colon_pos) = trimmed.find(':') {
                    let key = trimmed[..colon_pos].trim().to_string();
                    let value = trimmed[colon_pos + 1..].trim().to_string();
                    symbols.push(MarkdownSymbol::FrontMatterField {
                        key,
                        value,
                        line: line_num,
                    });
                }
                continue;
            }
            let _ = frontmatter_done;

            // --- Code fence toggle ---
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                if in_code_block {
                    in_code_block = false;
                } else {
                    in_code_block = true;
                    let fence_chars = if trimmed.starts_with("```") {
                        "```"
                    } else {
                        "~~~"
                    };
                    let lang_part = trimmed[fence_chars.len()..].trim();
                    let language = if lang_part.is_empty() {
                        None
                    } else {
                        Some(lang_part.to_string())
                    };
                    symbols.push(MarkdownSymbol::CodeFence {
                        language,
                        start_line: line_num,
                    });
                }
                continue;
            }
            if in_code_block {
                continue;
            }

            // --- Headings ---
            if trimmed.starts_with('#') {
                let level = trimmed.chars().take_while(|c| *c == '#').count();
                if level <= 6 {
                    let text = trimmed[level..].trim().to_string();
                    symbols.push(MarkdownSymbol::Heading {
                        level,
                        text,
                        line: line_num,
                    });
                    continue;
                }
            }

            // --- MDX components (<ComponentName ...>) ---
            // Simple heuristic: `<Uppercase...>` that looks like a JSX tag.
            let mut remaining = trimmed;
            while let Some(lt) = remaining.find('<') {
                let after = &remaining[lt + 1..];
                if let Some(first_char) = after.chars().next() {
                    if first_char.is_ascii_uppercase() {
                        // Collect the component name
                        let name: String = after
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
                            .collect();
                        if !name.is_empty() {
                            symbols.push(MarkdownSymbol::MdxComponent {
                                name,
                                line: line_num,
                            });
                        }
                    }
                }
                remaining = &after[after.find('>').map(|i| i + 1).unwrap_or(after.len())..];
            }

            // --- Links `[text](url)` ---
            let mut src = line;
            while let Some(open_bracket) = src.find("](") {
                let before = &src[..open_bracket];
                let after = &src[open_bracket + 2..];
                let text_start = before.rfind('[').map(|i| i + 1).unwrap_or(0);
                let link_text = before[text_start..].to_string();
                let url_end = after.find(')').unwrap_or(after.len());
                let url = after[..url_end].to_string();
                symbols.push(MarkdownSymbol::Link {
                    text: link_text,
                    url,
                    line: line_num,
                });
                src = &after[url_end.min(after.len())..];
            }
        }

        symbols
    }
}
