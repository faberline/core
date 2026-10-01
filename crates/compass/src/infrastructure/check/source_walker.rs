use crate::domain::check::file_result::FileResult;
use crate::domain::check::lint_config::LintConfig;
use crate::domain::syntax::language::Language;
use crate::domain::syntax::parsed_file::ParsedFile;
use crate::infrastructure::syntax::multi_parser::MultiParser;
use crate::lint::CheckerRegistry;
use std::path::Path;

/// Check a single file
pub(crate) fn check_file(
    parser: &mut MultiParser,
    registry: &CheckerRegistry,
    path: &Path,
    config: &LintConfig,
) -> Option<FileResult> {
    let language = MultiParser::detect_language(path)?;

    if !config.is_language_enabled(language) {
        return None;
    }

    let source = std::fs::read_to_string(path).ok()?;

    // Some languages (Dockerfile, Markdown, Mermaid) use line-based analysis without tree-sitter.
    // SQL, Proto, GraphQL, TOML now have real AST grammars (R3) so parser.parse() handles them;
    // we fall back to line_based only for the remaining line-only languages.
    let parsed = if let Some(p) = parser.parse(&source, language) {
        p
    } else if matches!(
        language,
        Language::Dockerfile | Language::Markdown | Language::Mdx | Language::Mermaid
    ) {
        // Create a minimal ParsedFile for line-based checkers
        ParsedFile::line_based(source, language)
    } else {
        return None;
    };

    let checker = registry.get(language)?;
    let diagnostics = checker.check(&parsed, config);

    Some(FileResult {
        path: path.to_path_buf(),
        language,
        diagnostics,
    })
}

/// Check all files in a directory
pub(crate) fn check_directory(
    parser: &mut MultiParser,
    registry: &CheckerRegistry,
    dir: &Path,
    config: &LintConfig,
) -> Vec<FileResult> {
    use jwalk::WalkDir;

    let mut results = Vec::new();

    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();

        // Skip excluded patterns
        if config.is_excluded(&path) {
            continue;
        }

        if let Some(result) = check_file(parser, registry, &path, config) {
            results.push(result);
        }
    }

    results
}
