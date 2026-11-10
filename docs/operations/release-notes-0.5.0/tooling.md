# Release notes 0.5.0: codegen, tooling and UI crates

Scope: the changes from `v0.4.14` to `origin/main` in the crates listed below.
Source: `git diff v0.4.14..origin/main -- crates/<crate>`, the crate root
(`lib.rs`) re-exports, and the commit subjects in
`git log v0.4.14..origin/main -- crates/<crate>`. Many crates moved their
sources into `api/`, `application/`, `domain/`, `infrastructure/` and
`interfaces/` folders. Those moves are not breaking by themselves. A root
re-export that still resolves is not listed as breaking.

No crate in this list added or removed a Cargo feature. `openapi-codegen` added
the `sha2` dependency, `cli-std` added `thiserror`, and `compass` removed
`uuid`, `indexmap` and fifteen other unused dependencies. None of these
changes a public signature.

## Crates checked

- `crates/openapi-codegen`
- `crates/compass`
- `crates/cli-std`
- `crates/surface`
- `crates/ui-runtime`
- `crates/build-stamp`

## Breaking changes

### openapi-codegen: root modules and the `target` compat facade

- `pub mod emit`, `pub mod ir` and `pub mod llm` are now `api::{emit, ir, llm}`,
  re-exported at the root. The paths `openapi_codegen::emit`, `::ir` and `::llm` still resolve.
- `pub mod target` and its compat facade were deleted (commit `cb011090`).
  `target` items are re-exported from the crate root through `application` and
  `domain`. A path such as `openapi_codegen::target::X` no longer resolves.

Migration: import from the crate root, for example
`openapi_codegen::{GenOptions, TargetProfile}`, not from `openapi_codegen::target`.

### openapi-codegen: `GenOptions` fields are private and built with builders

`GenOptions` had public fields (`lang`, `target`, `spec_path`, `out_dir`,
`client_name`, `http_client`, `emit_types`, and others). They are private now.
Build it through `GenOptions::new(lang, spec_path, out_dir, client_name)` and
the builders `with_target`, `with_service_compatibility` (or
`try_with_service_compatibility`, which returns a `Result`), `with_http_client`,
`with_emit_types`, `with_emit_client` and `with_emit_hooks` (commit `c5de0770`).

Migration: replace each struct literal with `GenOptions::new(...)` and the
builder chain. Read each field through its getter: `lang()`, `target()`,
`service_compatibility()`, `spec_path()`, `out_dir()`, `client_name()`,
`http_client()`, `emit_types()`, `emit_client()` and `emit_hooks()`.

### openapi-codegen: `TargetProfile::default_for` removed

The method was deleted as unused (commit `f45de6e3`).

Migration: construct the target profile from the explicit `TargetProfile`
variant or parser that your code uses.

### openapi-codegen: emitters and parsers return typed errors

The emitter entry points (`generate`, `generate_with_file_bearer_auth`,
`generate_for_target` and `generate_for_target_with_file_bearer_auth`) return
`Result<GeneratedOutput, SpecParseError>` in each language module (commit
`eee4ca9d`). `SpecParseError` is now re-exported from the root. `FileBearerAuth`
and the target-profile parsers return `FileBearerAuthError` (new, root
re-export) instead of `anyhow::Error`.

Migration: `?` still works in a function that returns `anyhow::Result`. Code
that names `anyhow::Error` or matches its text must match the new error types.

### openapi-codegen: generated Rust clients return typed errors for declared failures

Generated Rust clients decode declared non-2xx responses into typed API errors
(commits `cfe44372` and `1a669a7e`). The return type of each operation changes
in the regenerated code.

Migration: regenerate the client, then handle the typed error variant at each
call site that matched a raw status or a generic error before.

### compass: root modules `checker`, `format`, `outline`, `refactoring`, `search` and `watch` removed

The compat facades for `checker`, `outline` and `watch` were deleted (commit
`42014497`). The modules `format`, `refactoring` and `search` were deleted with
their contents (commits `017eb95b`, `2bc40662` and `83c44480`). The other
root modules (`check_pipeline`, `core`, `diagnostic`, `gen`, `graph`,
`lens_error`, `lint`, `lsp`, `output`, `schemas`, `semantic`, `server`, `spec`,
`storage`, `syntax`, `type_inference`) remain, re-exported through `api`.

Migration: use the crate-root items that replace them. `check_paths`,
`check_paths_with_propagation`, `outline`, `outline_parsed`, `FileWatcher`,
`WatchConfig` and `WatchEvent` are re-exported at the root. The `format`,
`refactoring` and `search` modules have no replacement in this crate.

### compass: unused items deleted

Each deletion below was removed as unused or as a duplicate:

- Rust, TypeScript and Go semantic type systems and the Go semantic types module
  (`e7cdac4e`, `ff5cedce`).
- `format`, `autofix` and `embedded_markdown` modules (`017eb95b`).
- `type_inference::CodeGenerator` struct (`e096082d`).
- `RefactoringRegistry` engine (`2bc40662`).
- The search subsystem (`83c44480`).
- `GoImportGraph` (`524dfa31`).
- `MarkdownSymbolExtractor` duplicate (`b192fc03`).
- `ArgusError::TreeSitterLanguage` variant (`48d3c9a5`).

Migration: an app that used a deleted item must reimplement it or stay on 0.4.x.
Check each name in your code before you upgrade.

### compass: `LintConfig` fields are private

`LintConfig` (in `domain::check::lint_config`, re-exported at the root) has
private fields (commit `842b1dc8`). Use the builders `with_languages`,
`with_exclude_patterns` and `with_min_severity`.

Migration: replace each struct literal with the builder chain. Read the
fields through their getters.

### compass: `Diagnostic::code` is `RuleCode`, not `String`

`Diagnostic.code` changed from `String` to the `RuleCode` newtype (commit
`272857ff`). `RuleCode` serializes as the bare string (`#[serde(transparent)]`),
so the JSON output does not change. `RuleCode` is re-exported at the root.

Migration: `RuleCode::new(impl Into<String>)` builds one. Use `as_str()` where
your code needed a `&str`, and `into_string()` where it needed a `String`. It
implements `Display` (`to_string()`), `AsRef<str>`, `From<&str>`, `From<String>`,
`PartialEq<&str>` and `PartialEq<&&str>`.

### compass: `SymbolId` and `ScopeId` fields are private; `RuleCode` replaces `String` in two methods

- `compass::type_inference::SymbolId(pub u64)` and `ScopeId(pub u64)` are now
  `SymbolId(u64)` and `ScopeId(u64)` (commit `272857ff`). Both constructors
  `new` are now `const fn`. The other `SymbolId` (`compass::semantic`, `SymbolId(pub usize)`)
  is not changed.
- `CustomRuleConfig::code(&self)` returns `RuleCode`, not `String`.
- `CustomLintEngine::rule_codes(&self)` returns `Vec<RuleCode>`, not `Vec<String>`.

Migration: replace each `.0` read of a `SymbolId` or `ScopeId` with `.get()`,
which returns `u64`. Where your code needed the `String` from `code()` or
`rule_codes()`, call `as_str()` or `into_string()` on the `RuleCode`.

### compass: `Range::from_node` replaced by `NodeRange`

`Range::from_node` was replaced by the `NodeRange` trait (commit `7449d894`),
re-exported at the root with `ParsedFile`.

Migration: `use compass::NodeRange;` and call the trait method on the node.

### compass: rejected custom rules are returned as data

A custom rule that fails to load is now returned as a `RejectedRule` value
instead of being logged (commit `b93ad27d`). The signatures of `check_paths` and
`check_paths_with_propagation` are unchanged.

Migration: read `RejectedRule` values where your code relied on the log line.

### cli-std: `llm::Topic`, `SectionedTopic` and `ToolInfo` fields are private

- `llm::Topic` fields are private behind a `const` constructor (commit `474a58ae`).
- `SectionedTopic` fields are private behind a `const` constructor (commit `18e4af23`).
- `ToolInfo` fields are private behind a `const` constructor (commit `6e9ab4da`).
  The type is re-exported at the root.

Migration: replace each struct literal with the constructor, and each field read
with the accessor that the constructor's module provides.

### cli-std: option structs are built with builders

`upgrade::Options` (commit `0333b77c`), `issue::SearchOptions` (`4707681a`),
`issue::CommentOptions` (`cd19b0c3`, now `try_new` plus `with_*`) and
`issue::CreateOptions` (`d0e589bc`, now `new` plus `with_*`) have private fields.

- `upgrade::Options`: `Default`, then `with_check`, `with_tag`, `with_force`, `with_yes`.
  Read with `check()`, `force()`, `yes()`.
- `issue::SearchOptions`: `Default`, then `with_query`, `with_state`, `with_limit`.
  Read with `limit()`.
- `issue::CommentOptions`: `try_new(number)`, then `with_message`, `with_repo`,
  `with_dry_run`, `with_yes`.
- `issue::CreateOptions`: `new(title)`, then `with_message`, `with_url`,
  `with_repo`, `with_label`, `with_dry_run`, `with_yes`.

Migration: replace each struct literal with its constructor and builder chain.
`CommentOptions::try_new` returns `Result`, so handle the error.

### cli-std: three functions removed

`kubectl_get_json`, `secret_data_bytes` and `assert_command_chainable` were
removed as unused (commit `21c78c39`). Nothing in the crate replaces them.

Migration: reimplement the function in your application code, or drop the call.

### surface: `Props` and `Component` fields are private

The public fields of `Component` (`name`, `render`, `props`) and of `Props`
(`class_name`, `style`, `on_click`, `on_change`, `on_checked_change`, `id`,
`value`, `input_type`, `placeholder`, `checked`, `aria_label`, `html_for`,
`disabled`, and the rest of the struct) are private (commit `eaa4b1b5`). The
type names and the root re-exports are unchanged (commit `894aab04` splits the
modules, and the root names stay).

- `Component::new(name, render, props)` (`const fn`), then `name()` and `render()`.
  No getter for `props` is provided.
- `Props` derives `Default`. Set values with `with_class_name`, `with_style`,
  `with_on_click`, `with_on_change`, `with_on_checked_change`, `with_id`,
  `with_value`, `with_input_type`, `with_placeholder`, `with_checked`,
  `with_aria_label`, `with_html_for` and `with_disabled`. Read them with the
  getter of the same name; it returns `Option` (for example `class_name()`
  returns `Option<&str>`, `on_click()` returns `Option<&Callback<()>>`).

Migration: build a value with `Props::default().with_...(...)` and pass it to
`Component::new`. Replace each direct field access with the getter above.

### ui-runtime: `FiberId` field is private and debug API takes `FiberId`

- `FiberId(pub u64)` is `FiberId(u64)`, so the tuple field is private (commit
  `5943e0b6`).
- `debug_snapshot_hooks(fiber_id: u64)` is now `debug_snapshot_hooks(fiber_id: FiberId)`.

Migration: pass a `FiberId` value, not a raw `u64`. Take the id from the
runtime's own reports, because the field is no longer readable from outside.

### build-stamp: None.

`stamp(prefix: &str)` keeps its signature and is re-exported with `pub use stamp::stamp`.

## Fixes apps must take

- `compass`: `d514f26a` (`fix(compass): slice test-extractor arguments on byte offsets`).
  The Python test extractor's `translate_assert_eq` walked the argument text with
  `chars().enumerate()` and used that character index as a byte offset. On text
  with multi-byte characters it sliced in the wrong place. It now uses
  `char_indices()`, which gives byte offsets. The commit adds the test
  `test_translate_assert_eq_non_ascii`: `assert_eq!("é", "ü")` becomes
  `assert "é" == "ü"`. Re-run any extraction that depends on such argument text.
- `openapi-codegen`: `b41c6a3f` (`optional untyped Rust field gets Option::is_none on serde_json::Value`).
  The generated Rust code for an optional field with an untyped schema now uses
  `Option::is_none` on the `serde_json::Value`. Regenerate the client to pick
  up the change.

## New

- `openapi-codegen`: `SpecParseError`, `FileBearerAuthError` and `ServiceCompatibilityError` at the root; QUERY fallback, operation retry classification (`b329b335`) and complete manifests (`e3b970db`) in the generator.
- `compass`: `RuleCode`, `NodeRange`, `RejectedRule` and `SourceParser` at the root; `ScopeId::get` and `SymbolId::get` (`const fn`).
- `cli-std`: `IssueNumberError` at the root; `ToolInfo` is now re-exported from `application`.
- `ui-runtime`: `debug_snapshot_fibers` and `debug_snapshot_hooks` are re-exported from the root with their debug types.
