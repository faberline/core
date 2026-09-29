use super::symbol::{MarkdownSymbol, MarkdownSymbolExtractor};
use super::*;
use crate::checker::LintConfig;

fn make_file(source: &str) -> ParsedFile {
    ParsedFile::line_based(source.to_string(), Language::Markdown)
}

fn codes(diagnostics: &[Diagnostic]) -> Vec<&str> {
    diagnostics.iter().map(|d| d.code.as_str()).collect()
}

#[test]
fn test_heading_level_skip() {
    let source = "# Title\n### Skip\n";
    let file = make_file(source);
    let checker = MarkdownChecker::new();
    let diags = checker.check(&file, &LintConfig::default());
    assert!(
        codes(&diags).contains(&"MD001"),
        "expected MD001 for heading skip, got {:?}",
        codes(&diags)
    );
}

#[test]
fn test_duplicate_heading() {
    let source = "# Hello\n## Section\n# Hello\n";
    let file = make_file(source);
    let checker = MarkdownChecker::new();
    let diags = checker.check(&file, &LintConfig::default());
    assert!(
        codes(&diags).contains(&"MD002"),
        "expected MD002 for duplicate heading, got {:?}",
        codes(&diags)
    );
}

#[test]
fn test_missing_code_lang() {
    let source = "Some text\n```\ncode here\n```\n";
    let file = make_file(source);
    let checker = MarkdownChecker::new();
    let diags = checker.check(&file, &LintConfig::default());
    assert!(
        codes(&diags).contains(&"MD003"),
        "expected MD003 for missing code lang, got {:?}",
        codes(&diags)
    );
}

#[test]
fn test_trailing_whitespace() {
    let source = "# Title\nSome line   \n";
    let file = make_file(source);
    let checker = MarkdownChecker::new();
    let diags = checker.check(&file, &LintConfig::default());
    assert!(
        codes(&diags).contains(&"MD009"),
        "expected MD009 for trailing whitespace, got {:?}",
        codes(&diags)
    );
}

#[test]
fn test_consecutive_blank_lines() {
    let source = "# Title\n\n\n\nContent\n";
    let file = make_file(source);
    let checker = MarkdownChecker::new();
    let diags = checker.check(&file, &LintConfig::default());
    assert!(
        codes(&diags).contains(&"MD010"),
        "expected MD010 for consecutive blanks, got {:?}",
        codes(&diags)
    );
}

#[test]
fn test_unclosed_frontmatter() {
    let source = "---\ntitle: Test\n# Body\n";
    let file = make_file(source);
    let checker = MarkdownChecker::new();
    let diags = checker.check(&file, &LintConfig::default());
    assert!(
        codes(&diags).contains(&"MD007"),
        "expected MD007 for unclosed frontmatter, got {:?}",
        codes(&diags)
    );
}

#[test]
fn test_no_false_positives_for_clean_file() {
    let source = "---\ntitle: Clean\n---\n\n# Title\n\n## Section\n\nSome content here.\n";
    let file = make_file(source);
    let checker = MarkdownChecker::new();
    let diags = checker.check(&file, &LintConfig::default());
    // MD008 is always emitted when frontmatter exists (deferred schema check)
    let unexpected: Vec<_> = diags.iter().filter(|d| d.code != "MD008").collect();
    assert!(
        unexpected.is_empty(),
        "unexpected diagnostics on clean file: {:?}",
        unexpected.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
}

// -----------------------------------------------------------------------
// MD011: Broken relative link
// -----------------------------------------------------------------------

#[test]
fn test_md011_broken_link_fires_on_missing_file() {
    use tempfile::tempdir;

    let dir = tempdir().expect("tempdir");
    // Workspace root has no `missing.md` file
    let checker = MarkdownChecker::with_workspace(dir.path().to_path_buf());

    let source = "See [missing](./missing.md) for details.\n";
    let file = make_file(source);
    let diags = checker.check(&file, &LintConfig::default());
    assert!(
        codes(&diags).contains(&"MD011"),
        "expected MD011 for missing file, got {:?}",
        codes(&diags)
    );
}

#[test]
fn test_md011_no_fire_when_file_exists() {
    use std::fs;
    use tempfile::tempdir;

    let dir = tempdir().expect("tempdir");
    let target = dir.path().join("exists.md");
    fs::write(&target, "# Exists\n").expect("create file");

    let checker = MarkdownChecker::with_workspace(dir.path().to_path_buf());
    let source = "See [exists](./exists.md) for details.\n";
    let file = make_file(source);
    let diags = checker.check(&file, &LintConfig::default());
    let md011_count = diags.iter().filter(|d| d.code == "MD011").count();
    assert_eq!(md011_count, 0, "MD011 should not fire for existing file");
}

#[test]
fn test_md011_disabled_without_workspace() {
    // Without workspace_root, MD011 is disabled (no file-system checks).
    let checker = MarkdownChecker::new();
    let source = "See [missing](./missing.md) for details.\n";
    let file = make_file(source);
    let diags = checker.check(&file, &LintConfig::default());
    assert!(
        !codes(&diags).contains(&"MD011"),
        "MD011 should not fire without workspace root"
    );
}

// -----------------------------------------------------------------------
// MarkdownSymbolExtractor
// -----------------------------------------------------------------------

#[test]
fn test_symbol_extractor_headings() {
    let source = "# Title\n## Section\n### Sub\n";
    let symbols = MarkdownSymbolExtractor::extract(source);
    let headings: Vec<_> = symbols
        .iter()
        .filter_map(|s| {
            if let MarkdownSymbol::Heading { level, text, .. } = s {
                Some((*level, text.as_str()))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(headings, vec![(1, "Title"), (2, "Section"), (3, "Sub")]);
}

#[test]
fn test_symbol_extractor_links() {
    let source = "See [Rust](https://rust-lang.org) for more.\n";
    let symbols = MarkdownSymbolExtractor::extract(source);
    let links: Vec<_> = symbols
        .iter()
        .filter_map(|s| {
            if let MarkdownSymbol::Link { text, url, .. } = s {
                Some((text.as_str(), url.as_str()))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(links, vec![("Rust", "https://rust-lang.org")]);
}

#[test]
fn test_symbol_extractor_code_fence() {
    let source = "```rust\nfn main() {}\n```\n";
    let symbols = MarkdownSymbolExtractor::extract(source);
    let fences: Vec<_> = symbols
        .iter()
        .filter_map(|s| {
            if let MarkdownSymbol::CodeFence { language, .. } = s {
                Some(language.as_deref())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(fences, vec![Some("rust")]);
}

#[test]
fn test_symbol_extractor_mdx_component() {
    let source = "Here is a <MyComponent /> component.\n";
    let symbols = MarkdownSymbolExtractor::extract(source);
    let components: Vec<_> = symbols
        .iter()
        .filter_map(|s| {
            if let MarkdownSymbol::MdxComponent { name, .. } = s {
                Some(name.as_str())
            } else {
                None
            }
        })
        .collect();
    assert!(
        components.contains(&"MyComponent"),
        "expected MyComponent, got {:?}",
        components
    );
}

#[test]
fn test_symbol_extractor_frontmatter() {
    let source = "---\ntitle: Hello\nauthor: Bob\n---\n# Body\n";
    let symbols = MarkdownSymbolExtractor::extract(source);
    let fields: Vec<_> = symbols
        .iter()
        .filter_map(|s| {
            if let MarkdownSymbol::FrontMatterField { key, value, .. } = s {
                Some((key.as_str(), value.as_str()))
            } else {
                None
            }
        })
        .collect();
    assert!(fields.contains(&("title", "Hello")));
    assert!(fields.contains(&("author", "Bob")));
}
