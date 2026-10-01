# DDD P2 source paths

This table compares Rust source files at the end of P1 with P2.
See [the API migration guide](ddd-p2.md) for public names and signatures.
See [the P1 table](ddd-p1-paths.md) for paths before P1.

The table uses Git renames and unique Rust declarations parsed from the two trees.
Methods are matched together with their owning type.
Free functions, constants and test helpers require the same declaration bytes.
A common method name such as `new` is not used by itself to infer a move.

- A row can list several files when declarations moved to several places.
- A file that still exists is included when some of its declarations moved.
- Files only edited in place are omitted.
- A removed file can contain items that were renamed or rewritten elsewhere.
  Its removal does not by itself mean that every public item was deleted.
  The API guide is the authority for that decision.
- P1 facade files map directly to the public API module that replaces them.

## cli-std

| P1 file | P2 file or status |
|---|---|
| `crates/cli-std/src/application/issue/client.rs` | removed as a source file; see [cli-std API changes](ddd-p2.md#cli-std) |
| `crates/cli-std/src/compat.rs` | `crates/cli-std/src/api.rs` (public module declarations) |
| `crates/cli-std/src/compat/artifact.rs` | `crates/cli-std/src/api/artifact.rs` |
| `crates/cli-std/src/compat/chainable.rs` | `crates/cli-std/src/api/chainable.rs` |
| `crates/cli-std/src/compat/connect.rs` | `crates/cli-std/src/api/connect.rs` |
| `crates/cli-std/src/compat/issue.rs` | `crates/cli-std/src/api/issue.rs` |
| `crates/cli-std/src/compat/llm.rs` | `crates/cli-std/src/api/llm.rs` |
| `crates/cli-std/src/compat/llm/v2.rs` | `crates/cli-std/src/api/llm/v2.rs` |
| `crates/cli-std/src/compat/registry.rs` | `crates/cli-std/src/api/registry.rs` |
| `crates/cli-std/src/compat/report_issue.rs` | `crates/cli-std/src/api/report_issue.rs` |
| `crates/cli-std/src/compat/upgrade.rs` | `crates/cli-std/src/api/upgrade.rs` |

## compass

| P1 file | P2 file or status |
|---|---|
| `crates/compass/src/application/analysis/request_handler.rs` | `crates/compass/src/app/analysis.rs`, `crates/compass/src/application/analysis/request_handler.rs` |
| `crates/compass/src/application/daemon/config.rs` | `crates/compass/src/domain/daemon/config.rs`, `crates/compass/src/infrastructure/daemon/socket_path.rs` |
| `crates/compass/src/application/daemon/config/tests.rs` | `crates/compass/src/infrastructure/daemon/socket_path/tests.rs` |
| `crates/compass/src/application/daemon/protocol.rs` | `crates/compass/src/domain/daemon/protocol.rs` |
| `crates/compass/src/application/search.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/application/search/engine.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat.rs` | `crates/compass/src/api.rs` (public module declarations) |
| `crates/compass/src/compat/check_pipeline.rs` | `crates/compass/src/api/check_pipeline.rs` |
| `crates/compass/src/compat/checker.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat/core.rs` | `crates/compass/src/api/core.rs` |
| `crates/compass/src/compat/core/config.rs` | `crates/compass/src/api/core/config.rs` |
| `crates/compass/src/compat/core/index_config.rs` | `crates/compass/src/api/core/index_config.rs` |
| `crates/compass/src/compat/diagnostic.rs` | `crates/compass/src/api/diagnostic.rs` |
| `crates/compass/src/compat/format.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat/format/detect.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat/gen.rs` | `crates/compass/src/api/gen.rs` |
| `crates/compass/src/compat/gen/python.rs` | `crates/compass/src/api/gen/python.rs` |
| `crates/compass/src/compat/gen/python/meteor.rs` | `crates/compass/src/api/gen/python/meteor.rs` |
| `crates/compass/src/compat/gen/python/nebula.rs` | `crates/compass/src/api/gen/python/nebula.rs` |
| `crates/compass/src/compat/gen/python/photon.rs` | `crates/compass/src/api/gen/python/photon.rs` |
| `crates/compass/src/compat/gen/python/quasar.rs` | `crates/compass/src/api/gen/python/quasar.rs` |
| `crates/compass/src/compat/gen/python/rust_scanner.rs` | `crates/compass/src/api/gen/python/rust_scanner.rs` |
| `crates/compass/src/compat/gen/python/shield.rs` | `crates/compass/src/api/gen/python/shield.rs` |
| `crates/compass/src/compat/gen/python/test_extractor.rs` | `crates/compass/src/api/gen/python/test_extractor.rs` |
| `crates/compass/src/compat/gen/python/titan.rs` | `crates/compass/src/api/gen/python/titan.rs` |
| `crates/compass/src/compat/gen/registry.rs` | `crates/compass/src/api/gen/registry.rs` |
| `crates/compass/src/compat/gen/rust.rs` | `crates/compass/src/api/gen/rust.rs` |
| `crates/compass/src/compat/gen/rust/axum.rs` | `crates/compass/src/api/gen/rust/axum.rs` |
| `crates/compass/src/compat/gen/rust/reqwest.rs` | `crates/compass/src/api/gen/rust/reqwest.rs` |
| `crates/compass/src/compat/gen/rust/serde.rs` | `crates/compass/src/api/gen/rust/serde.rs` |
| `crates/compass/src/compat/gen/rust/sqlx.rs` | `crates/compass/src/api/gen/rust/sqlx.rs` |
| `crates/compass/src/compat/gen/traits.rs` | `crates/compass/src/api/gen/traits.rs` |
| `crates/compass/src/compat/graph.rs` | `crates/compass/src/api/graph.rs` |
| `crates/compass/src/compat/graph/resolve.rs` | `crates/compass/src/api/graph/resolve.rs` |
| `crates/compass/src/compat/lens_error.rs` | `crates/compass/src/api/lens_error.rs` |
| `crates/compass/src/compat/lint.rs` | `crates/compass/src/api/lint.rs` |
| `crates/compass/src/compat/lint/asyncapi.rs` | `crates/compass/src/api/lint/asyncapi.rs` |
| `crates/compass/src/compat/lint/autofix.rs` | `crates/compass/src/api/lint.rs` |
| `crates/compass/src/compat/lint/css.rs` | `crates/compass/src/api/lint/css.rs` |
| `crates/compass/src/compat/lint/custom.rs` | `crates/compass/src/api/lint/custom.rs` |
| `crates/compass/src/compat/lint/dockerfile.rs` | `crates/compass/src/api/lint/dockerfile.rs` |
| `crates/compass/src/compat/lint/embedded_markdown.rs` | `crates/compass/src/api/lint.rs` |
| `crates/compass/src/compat/lint/gitlab_ci.rs` | `crates/compass/src/api/lint/gitlab_ci.rs` |
| `crates/compass/src/compat/lint/gitlab_ci_rules.rs` | `crates/compass/src/api/lint/gitlab_ci_rules.rs` |
| `crates/compass/src/compat/lint/go.rs` | `crates/compass/src/api/lint/go.rs` |
| `crates/compass/src/compat/lint/graphql.rs` | `crates/compass/src/api/lint/graphql.rs` |
| `crates/compass/src/compat/lint/html.rs` | `crates/compass/src/api/lint/html.rs` |
| `crates/compass/src/compat/lint/javascript.rs` | `crates/compass/src/api/lint/javascript.rs` |
| `crates/compass/src/compat/lint/kubernetes.rs` | `crates/compass/src/api/lint/kubernetes.rs` |
| `crates/compass/src/compat/lint/kubernetes_rules.rs` | `crates/compass/src/api/lint/kubernetes_rules.rs` |
| `crates/compass/src/compat/lint/markdown.rs` | `crates/compass/src/api/lint/markdown.rs` |
| `crates/compass/src/compat/lint/mdx.rs` | `crates/compass/src/api/lint/mdx.rs` |
| `crates/compass/src/compat/lint/mermaid.rs` | `crates/compass/src/api/lint/mermaid.rs` |
| `crates/compass/src/compat/lint/openapi.rs` | `crates/compass/src/api/lint/openapi.rs` |
| `crates/compass/src/compat/lint/openrpc.rs` | `crates/compass/src/api/lint/openrpc.rs` |
| `crates/compass/src/compat/lint/proto.rs` | `crates/compass/src/api/lint/proto.rs` |
| `crates/compass/src/compat/lint/python.rs` | `crates/compass/src/api/lint/python.rs` |
| `crates/compass/src/compat/lint/python_security.rs` | `crates/compass/src/api/lint/python_security.rs` |
| `crates/compass/src/compat/lint/rust_checker.rs` | `crates/compass/src/api/lint/rust_checker.rs` |
| `crates/compass/src/compat/lint/sql.rs` | `crates/compass/src/api/lint/sql.rs` |
| `crates/compass/src/compat/lint/terraform.rs` | `crates/compass/src/api/lint/terraform.rs` |
| `crates/compass/src/compat/lint/terraform_rules.rs` | `crates/compass/src/api/lint/terraform_rules.rs` |
| `crates/compass/src/compat/lint/toml_checker.rs` | `crates/compass/src/api/lint/toml_checker.rs` |
| `crates/compass/src/compat/lint/typescript.rs` | `crates/compass/src/api/lint/typescript.rs` |
| `crates/compass/src/compat/lint/yaml_dispatch.rs` | `crates/compass/src/api/lint/yaml_dispatch.rs` |
| `crates/compass/src/compat/lsp.rs` | `crates/compass/src/api/lsp.rs` |
| `crates/compass/src/compat/lsp/server.rs` | `crates/compass/src/api/lsp/server.rs` |
| `crates/compass/src/compat/outline.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat/output.rs` | `crates/compass/src/api/output.rs` |
| `crates/compass/src/compat/output/agent.rs` | `crates/compass/src/api/output/agent.rs` |
| `crates/compass/src/compat/output/agent_types.rs` | `crates/compass/src/api/output/agent_types.rs` |
| `crates/compass/src/compat/output/reporter.rs` | `crates/compass/src/api/output/reporter.rs` |
| `crates/compass/src/compat/refactoring.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat/refactoring/extract.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat/refactoring/extract_helpers.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat/refactoring/inline.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat/refactoring/move_def.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat/refactoring/rename.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat/refactoring/signature.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat/refactoring/signature_helpers.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat/schemas.rs` | `crates/compass/src/api/schemas.rs` |
| `crates/compass/src/compat/schemas/frontmatter.rs` | `crates/compass/src/api/schemas/frontmatter.rs` |
| `crates/compass/src/compat/schemas/gitlab.rs` | `crates/compass/src/api/schemas/gitlab.rs` |
| `crates/compass/src/compat/schemas/k8s.rs` | `crates/compass/src/api/schemas/k8s.rs` |
| `crates/compass/src/compat/search.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat/search/index.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat/search/query.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/compat/semantic.rs` | `crates/compass/src/api/semantic.rs` |
| `crates/compass/src/compat/semantic/pdg.rs` | `crates/compass/src/api/semantic/pdg.rs` |
| `crates/compass/src/compat/semantic/pdg/cfg.rs` | `crates/compass/src/api/semantic/pdg/cfg.rs` |
| `crates/compass/src/compat/semantic/pdg/data_flow.rs` | `crates/compass/src/api/semantic/pdg/data_flow.rs` |
| `crates/compass/src/compat/semantic/pdg/dominator.rs` | `crates/compass/src/api/semantic/pdg/dominator.rs` |
| `crates/compass/src/compat/semantic/scope.rs` | `crates/compass/src/api/semantic/scope.rs` |
| `crates/compass/src/compat/semantic/symbols.rs` | `crates/compass/src/api/semantic/symbols.rs` |
| `crates/compass/src/compat/semantic/symbols/css.rs` | `crates/compass/src/api/semantic/symbols/css.rs` |
| `crates/compass/src/compat/semantic/symbols/dockerfile.rs` | `crates/compass/src/api/semantic/symbols/dockerfile.rs` |
| `crates/compass/src/compat/semantic/symbols/gitlab_ci.rs` | `crates/compass/src/api/semantic/symbols/gitlab_ci.rs` |
| `crates/compass/src/compat/semantic/symbols/go.rs` | `crates/compass/src/api/semantic/symbols/go.rs` |
| `crates/compass/src/compat/semantic/symbols/graphql_sym.rs` | `crates/compass/src/api/semantic/symbols/graphql_sym.rs` |
| `crates/compass/src/compat/semantic/symbols/html.rs` | `crates/compass/src/api/semantic/symbols/html.rs` |
| `crates/compass/src/compat/semantic/symbols/javascript.rs` | `crates/compass/src/api/semantic/symbols/javascript.rs` |
| `crates/compass/src/compat/semantic/symbols/kubernetes.rs` | `crates/compass/src/api/semantic/symbols/kubernetes.rs` |
| `crates/compass/src/compat/semantic/symbols/markdown.rs` | `crates/compass/src/api/semantic/symbols/markdown.rs` |
| `crates/compass/src/compat/semantic/symbols/mermaid.rs` | `crates/compass/src/api/semantic/symbols/mermaid.rs` |
| `crates/compass/src/compat/semantic/symbols/proto_sym.rs` | `crates/compass/src/api/semantic/symbols/proto_sym.rs` |
| `crates/compass/src/compat/semantic/symbols/python.rs` | `crates/compass/src/api/semantic/symbols/python.rs` |
| `crates/compass/src/compat/semantic/symbols/rust.rs` | `crates/compass/src/api/semantic/symbols/rust.rs` |
| `crates/compass/src/compat/semantic/symbols/sql_sym.rs` | `crates/compass/src/api/semantic/symbols/sql_sym.rs` |
| `crates/compass/src/compat/semantic/symbols/terraform.rs` | `crates/compass/src/api/semantic/symbols/terraform.rs` |
| `crates/compass/src/compat/semantic/symbols/toml_sym.rs` | `crates/compass/src/api/semantic/symbols/toml_sym.rs` |
| `crates/compass/src/compat/semantic/symbols/typescript.rs` | `crates/compass/src/api/semantic/symbols/typescript.rs` |
| `crates/compass/src/compat/semantic/types.rs` | `crates/compass/src/api/semantic.rs` |
| `crates/compass/src/compat/semantic/types/go.rs` | `crates/compass/src/api/semantic.rs` |
| `crates/compass/src/compat/semantic/types/go_advanced.rs` | `crates/compass/src/api/semantic.rs` |
| `crates/compass/src/compat/server.rs` | `crates/compass/src/api/server.rs` |
| `crates/compass/src/compat/server/auto_discover.rs` | `crates/compass/src/api/server/auto_discover.rs` |
| `crates/compass/src/compat/server/daemon.rs` | `crates/compass/src/api/server/daemon.rs` |
| `crates/compass/src/compat/server/disk_cache.rs` | `crates/compass/src/api/server/disk_cache.rs` |
| `crates/compass/src/compat/server/handler.rs` | `crates/compass/src/api/server/handler.rs` |
| `crates/compass/src/compat/server/incremental.rs` | `crates/compass/src/api/server/incremental.rs` |
| `crates/compass/src/compat/server/protocol.rs` | `crates/compass/src/api/server/protocol.rs` |
| `crates/compass/src/compat/server/watch_bridge.rs` | `crates/compass/src/api/server/watch_bridge.rs` |
| `crates/compass/src/compat/spec.rs` | `crates/compass/src/api/spec.rs` |
| `crates/compass/src/compat/spec/asyncapi.rs` | `crates/compass/src/api/spec/asyncapi.rs` |
| `crates/compass/src/compat/spec/asyncapi/parser.rs` | `crates/compass/src/api/spec/asyncapi/parser.rs` |
| `crates/compass/src/compat/spec/ir.rs` | `crates/compass/src/api/spec/ir.rs` |
| `crates/compass/src/compat/spec/json_schema.rs` | `crates/compass/src/api/spec/json_schema.rs` |
| `crates/compass/src/compat/spec/json_schema/parser.rs` | `crates/compass/src/api/spec/json_schema/parser.rs` |
| `crates/compass/src/compat/spec/mermaid.rs` | `crates/compass/src/api/spec/mermaid.rs` |
| `crates/compass/src/compat/spec/mermaid/generator.rs` | `crates/compass/src/api/spec/mermaid/generator.rs` |
| `crates/compass/src/compat/spec/mermaid/parser.rs` | `crates/compass/src/api/spec/mermaid/parser.rs` |
| `crates/compass/src/compat/spec/openapi.rs` | `crates/compass/src/api/spec/openapi.rs` |
| `crates/compass/src/compat/spec/openapi/parser.rs` | `crates/compass/src/api/spec/openapi/parser.rs` |
| `crates/compass/src/compat/spec/statemachine.rs` | `crates/compass/src/api/spec/statemachine.rs` |
| `crates/compass/src/compat/spec/statemachine/mermaid_plus.rs` | `crates/compass/src/api/spec/statemachine/mermaid_plus.rs` |
| `crates/compass/src/compat/spec/statemachine/schema.rs` | `crates/compass/src/api/spec/statemachine/schema.rs` |
| `crates/compass/src/compat/spec/statemachine/validator.rs` | `crates/compass/src/api/spec/statemachine/validator.rs` |
| `crates/compass/src/compat/storage.rs` | `crates/compass/src/api/storage.rs` |
| `crates/compass/src/compat/syntax.rs` | `crates/compass/src/api/syntax.rs` |
| `crates/compass/src/compat/syntax/parser.rs` | `crates/compass/src/api/syntax/parser.rs` |
| `crates/compass/src/compat/type_inference.rs` | `crates/compass/src/api/type_inference.rs` |
| `crates/compass/src/compat/type_inference/annotation.rs` | `crates/compass/src/api/type_inference/annotation.rs` |
| `crates/compass/src/compat/type_inference/builtins.rs` | `crates/compass/src/api/type_inference/builtins.rs` |
| `crates/compass/src/compat/type_inference/cache.rs` | `crates/compass/src/api/type_inference/cache.rs` |
| `crates/compass/src/compat/type_inference/cfg_narrow.rs` | `crates/compass/src/api/type_inference/cfg_narrow.rs` |
| `crates/compass/src/compat/type_inference/check.rs` | `crates/compass/src/api/type_inference/check.rs` |
| `crates/compass/src/compat/type_inference/class_info.rs` | `crates/compass/src/api/type_inference/class_info.rs` |
| `crates/compass/src/compat/type_inference/codegen.rs` | `crates/compass/src/api/type_inference.rs` |
| `crates/compass/src/compat/type_inference/config.rs` | `crates/compass/src/api/type_inference/config.rs` |
| `crates/compass/src/compat/type_inference/deep_inference.rs` | `crates/compass/src/api/type_inference/deep_inference.rs` |
| `crates/compass/src/compat/type_inference/env.rs` | `crates/compass/src/api/type_inference/env.rs` |
| `crates/compass/src/compat/type_inference/frameworks.rs` | `crates/compass/src/api/type_inference/frameworks.rs` |
| `crates/compass/src/compat/type_inference/imports.rs` | `crates/compass/src/api/type_inference/imports.rs` |
| `crates/compass/src/compat/type_inference/incremental.rs` | `crates/compass/src/api/type_inference/incremental.rs` |
| `crates/compass/src/compat/type_inference/infer.rs` | `crates/compass/src/api/type_inference/infer.rs` |
| `crates/compass/src/compat/type_inference/model.rs` | `crates/compass/src/api/type_inference/model.rs` |
| `crates/compass/src/compat/type_inference/modules.rs` | `crates/compass/src/api/type_inference/modules.rs` |
| `crates/compass/src/compat/type_inference/mutable_ast.rs` | `crates/compass/src/api/type_inference/mutable_ast.rs` |
| `crates/compass/src/compat/type_inference/narrow.rs` | `crates/compass/src/api/type_inference/narrow.rs` |
| `crates/compass/src/compat/type_inference/package_managers.rs` | `crates/compass/src/api/type_inference/package_managers.rs` |
| `crates/compass/src/compat/type_inference/project.rs` | `crates/compass/src/api/type_inference/project.rs` |
| `crates/compass/src/compat/type_inference/propagation.rs` | `crates/compass/src/api/type_inference/propagation.rs` |
| `crates/compass/src/compat/type_inference/refactoring.rs` | `crates/compass/src/api/type_inference/refactoring.rs` |
| `crates/compass/src/compat/type_inference/refactoring_multilang.rs` | `crates/compass/src/api/type_inference.rs` |
| `crates/compass/src/compat/type_inference/rust_advanced.rs` | `crates/compass/src/api/type_inference.rs` |
| `crates/compass/src/compat/type_inference/rust_infer.rs` | `crates/compass/src/api/type_inference.rs` |
| `crates/compass/src/compat/type_inference/rust_lifetimes.rs` | `crates/compass/src/api/type_inference.rs` |
| `crates/compass/src/compat/type_inference/rust_symbols.rs` | `crates/compass/src/api/type_inference.rs` |
| `crates/compass/src/compat/type_inference/rust_traits.rs` | `crates/compass/src/api/type_inference.rs` |
| `crates/compass/src/compat/type_inference/rust_types.rs` | `crates/compass/src/api/type_inference.rs` |
| `crates/compass/src/compat/type_inference/semantic_search.rs` | `crates/compass/src/api/type_inference/semantic_search.rs` |
| `crates/compass/src/compat/type_inference/semantic_search_rust.rs` | `crates/compass/src/api/type_inference.rs` |
| `crates/compass/src/compat/type_inference/stubs.rs` | `crates/compass/src/api/type_inference/stubs.rs` |
| `crates/compass/src/compat/type_inference/ts_advanced.rs` | `crates/compass/src/api/type_inference.rs` |
| `crates/compass/src/compat/type_inference/ts_infer.rs` | `crates/compass/src/api/type_inference.rs` |
| `crates/compass/src/compat/type_inference/ts_types.rs` | `crates/compass/src/api/type_inference.rs` |
| `crates/compass/src/compat/type_inference/ty.rs` | `crates/compass/src/api/type_inference/ty.rs` |
| `crates/compass/src/compat/type_inference/type_env.rs` | `crates/compass/src/api/type_inference/type_env.rs` |
| `crates/compass/src/compat/type_inference/typeshed.rs` | `crates/compass/src/api/type_inference/typeshed.rs` |
| `crates/compass/src/compat/watch.rs` | removed facade; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/lint/autofix.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/lint/css_rules.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/lint/embedded_markdown.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/lint/go/import_graph.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/lint/html_rules.rs` | `crates/compass/src/domain/semantic/symbols/html.rs` |
| `crates/compass/src/domain/lint/markdown/symbol.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/refactoring.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/refactoring/engine.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/refactoring/extract.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/refactoring/extract_helpers.rs` | `crates/compass/src/domain/type_refactoring/engine.rs` |
| `crates/compass/src/domain/refactoring/inline.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/refactoring/move_def.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/refactoring/rename.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/refactoring/signature.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/refactoring/signature_helpers.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/advanced.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/advanced/elision.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/advanced/projection.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/advanced/tests.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/advanced/trait_bounds.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/infer.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/infer/bound_syntax.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/infer/compound_expr.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/infer/context.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/infer/elision.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/infer/expr.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/infer/tests.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/infer/type_syntax.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/infer/unify.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/lifetimes.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/lifetimes/borrow.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/lifetimes/tests.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/symbols.rs` | `crates/compass/src/domain/rust_source_scan/exports.rs` |
| `crates/compass/src/domain/rust_type_system/symbols/bounds.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/symbols/fields.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/symbols/helpers.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/symbols/signature.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/symbols/tests.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/traits.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/types.rs` | `crates/compass/src/domain/rust_source_scan/exports.rs` |
| `crates/compass/src/domain/rust_type_system/types/adt_def.rs` | `crates/compass/src/domain/spec/ir/data_model.rs` |
| `crates/compass/src/domain/rust_type_system/types/impl_block.rs` | `crates/compass/src/domain/spec/ir/data_model.rs` |
| `crates/compass/src/domain/rust_type_system/types/lifetime.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/types/tests.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/rust_type_system/types/trait_def.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/search.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/search/index.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/search/query.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/search/query/call_hierarchy.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/search/query/documentation.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/search/query/implementations.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/search/query/similar_code.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/search/query/tests.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/search/query/type_signature.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/search/query/usages.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/semantic/types.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/semantic/types/go.rs` | `crates/compass/src/domain/type_system/class_info.rs` |
| `crates/compass/src/domain/semantic/types/go/type_nodes.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/semantic/types/go_advanced.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/semantic/types/go_tests.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/semantic_search/engine/call_graph.rs` | `crates/compass/src/app/semantic_search.rs`, `crates/compass/src/domain/semantic_search/engine/call_graph.rs` |
| `crates/compass/src/domain/semantic_search/engine/docstrings.rs` | `crates/compass/src/app/semantic_search.rs`, `crates/compass/src/domain/semantic_search/engine/docstrings.rs` |
| `crates/compass/src/domain/semantic_search/engine/indexing.rs` | `crates/compass/src/app/semantic_search.rs`, `crates/compass/src/domain/semantic_search/engine/indexing.rs` |
| `crates/compass/src/domain/semantic_search/rust_provider.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/semantic_search/rust_provider/tests.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/semantic_search/rust_provider/usages.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/ts_type_system.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/ts_type_system/advanced.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/ts_type_system/advanced/tests.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/ts_type_system/infer.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/ts_type_system/infer/compound_expr.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/ts_type_system/infer/expr.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/ts_type_system/infer/narrowing.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/ts_type_system/infer/tests.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/ts_type_system/infer/type_annotation.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/ts_type_system/infer/type_operators.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/ts_type_system/types.rs` | `crates/compass/src/domain/spec/ir/data_model.rs` |
| `crates/compass/src/domain/ts_type_system/types/assignability.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/ts_type_system/types/context.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/ts_type_system/types/tests.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/ts_type_system/types/ts_enum.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/ts_type_system/types/type_operators.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/type_codegen.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/type_codegen/generator.rs` | `crates/compass/src/domain/codegen/traits.rs` |
| `crates/compass/src/domain/type_codegen/generator/docstring.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/type_codegen/generator/implementation.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/type_codegen/generator/test_stub.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/type_codegen/generator/tests.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/type_codegen/generator/type_stub.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/type_codegen/request.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/type_codegen/result.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/type_refactoring/engine.rs` | `crates/compass/src/app/refactoring.rs`, `crates/compass/src/domain/type_refactoring/engine.rs` |
| `crates/compass/src/domain/type_refactoring/multilang.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/type_refactoring/multilang/extract_python.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/type_refactoring/multilang/extract_rust.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/type_refactoring/multilang/extract_typescript.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/domain/type_refactoring/multilang/tests.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/infrastructure/codegen/traits.rs` | `crates/compass/src/domain/codegen/traits.rs` |
| `crates/compass/src/infrastructure/codegen/traits/tests.rs` | `crates/compass/src/domain/codegen/traits/tests.rs` |
| `crates/compass/src/infrastructure/format.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/infrastructure/format/detect.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/infrastructure/format/registry.rs` | removed as a source file; see [compass API changes](ddd-p2.md#compass) |
| `crates/compass/src/infrastructure/watch_bridge/bridge.rs` | `crates/compass/src/domain/daemon/watch_event.rs`, `crates/compass/src/infrastructure/watch_bridge/bridge.rs` |
| `crates/compass/src/interfaces/daemon/argus_daemon.rs` | `crates/compass/src/app/daemon.rs`, `crates/compass/src/interfaces/daemon/argus_daemon.rs` |
| `crates/compass/src/interfaces/daemon/request_dispatch.rs` | `crates/compass/src/application/analysis/request_dispatch.rs` |
| `crates/compass/src/interfaces/lsp/argus_server.rs` | `crates/compass/src/app/lsp.rs`, `crates/compass/src/application/editor/session.rs`, `crates/compass/src/interfaces/lsp/argus_server.rs` |
| `crates/compass/src/interfaces/output/agent.rs` | `crates/compass/src/app/output.rs`, `crates/compass/src/interfaces/output/agent.rs` |
| `crates/compass/src/interfaces/output/reporter.rs` | `crates/compass/src/app/output.rs`, `crates/compass/src/interfaces/output/reporter.rs` |

## openapi-codegen

| P1 file | P2 file or status |
|---|---|
| `crates/openapi-codegen/src/compat.rs` | `crates/openapi-codegen/src/api.rs` (public module declarations) |
| `crates/openapi-codegen/src/compat/emit.rs` | `crates/openapi-codegen/src/api/emit.rs` |
| `crates/openapi-codegen/src/compat/ir.rs` | `crates/openapi-codegen/src/api/ir.rs` |
| `crates/openapi-codegen/src/compat/llm.rs` | `crates/openapi-codegen/src/api/llm.rs` |
| `crates/openapi-codegen/src/compat/target.rs` | removed facade; see [openapi-codegen API changes](ddd-p2.md#openapi-codegen) |
| `crates/openapi-codegen/src/interfaces/cli.rs` | removed as a source file; see [openapi-codegen API changes](ddd-p2.md#openapi-codegen) |

## raft-core

| P1 file | P2 file or status |
|---|---|
| `crates/raft-core/src/domain/transport.rs` | removed as a source file; see [raft-core API changes](ddd-p2.md#raft-core) |

## raft-runtime

| P1 file | P2 file or status |
|---|---|
| `crates/raft-runtime/src/application/group.rs` | `crates/raft-runtime/src/domain/group.rs` |
| `crates/raft-runtime/src/application/host/membership_ops.rs` | `crates/raft-runtime/src/app/host.rs`, `crates/raft-runtime/src/application/host/membership_ops.rs` |
| `crates/raft-runtime/src/application/host/spawn.rs` | `crates/raft-runtime/src/app/host.rs`, `crates/raft-runtime/src/application/host/spawn.rs` |
| `crates/raft-runtime/src/application/replica_host.rs` | `crates/raft-runtime/src/app/replica_host.rs`, `crates/raft-runtime/src/application/replica_host.rs` |
| `crates/raft-runtime/src/compat.rs` | `crates/raft-runtime/src/api.rs` (public module declarations) |
| `crates/raft-runtime/src/compat/cluster.rs` | removed facade; see [raft-runtime API changes](ddd-p2.md#raft-runtime) |
| `crates/raft-runtime/src/compat/conformance.rs` | `crates/raft-runtime/src/api/conformance.rs` |
| `crates/raft-runtime/src/compat/group.rs` | removed facade; see [raft-runtime API changes](ddd-p2.md#raft-runtime) |
| `crates/raft-runtime/src/compat/llm.rs` | `crates/raft-runtime/src/api/llm.rs` |
| `crates/raft-runtime/src/infrastructure/peer_rpc.rs` | `crates/raft-runtime/src/application/host/peer_calls.rs`, `crates/raft-runtime/src/infrastructure/peer_rpc.rs` |
| `crates/raft-runtime/src/infrastructure/peer_wire.rs` | `crates/raft-runtime/src/infrastructure/peer_wire.rs`, `crates/raft-runtime/src/interfaces/peer_http/wire.rs` |
| `crates/raft-runtime/src/infrastructure/topology_env.rs` | `crates/raft-runtime/src/app/topology_env.rs`, `crates/raft-runtime/src/infrastructure/topology_env.rs` |
| `crates/raft-runtime/src/interfaces/conformance/deterministic_host.rs` | `crates/raft-runtime/src/app/conformance.rs`, `crates/raft-runtime/src/interfaces/conformance/deterministic_host.rs` |
| `crates/raft-runtime/src/interfaces/peer_http/registry.rs` | `crates/raft-runtime/src/application/registry.rs`, `crates/raft-runtime/src/interfaces/peer_http/registry.rs` |

## service-auth

| P1 file | P2 file or status |
|---|---|
| `crates/service-auth/src/application/google/introspection.rs` | `crates/service-auth/src/domain/google/introspection.rs` |
| `crates/service-auth/src/application/k8s/authenticator.rs` | `crates/service-auth/src/app/delegated_authenticator.rs`, `crates/service-auth/src/application/k8s/authenticator.rs` |
| `crates/service-auth/src/application/k8s/review.rs` | `crates/service-auth/src/domain/k8s/review.rs` |
| `crates/service-auth/src/application/k8s/system_clock.rs` | `crates/service-auth/src/infrastructure/k8s/system_clock.rs` |
| `crates/service-auth/src/application/k8s/token_request/source.rs` | `crates/service-auth/src/app/token_source.rs`, `crates/service-auth/src/application/k8s/token_request/source.rs` |
| `crates/service-auth/src/compat.rs` | `crates/service-auth/src/api.rs` (public module declarations) |
| `crates/service-auth/src/compat/async_verifier.rs` | removed facade; see [service-auth API changes](ddd-p2.md#service-auth) |
| `crates/service-auth/src/compat/gcp.rs` | `crates/service-auth/src/api/gcp.rs` |
| `crates/service-auth/src/compat/k8s.rs` | `crates/service-auth/src/api/k8s.rs` |
| `crates/service-auth/src/compat/k8s/cache.rs` | `crates/service-auth/src/api/k8s/cache.rs` |
| `crates/service-auth/src/compat/k8s/delegated.rs` | `crates/service-auth/src/api/k8s/delegated.rs` |
| `crates/service-auth/src/compat/k8s/kube_backend.rs` | `crates/service-auth/src/api/k8s/kube_backend.rs` |
| `crates/service-auth/src/compat/k8s/loopback_proxy.rs` | `crates/service-auth/src/api/k8s/loopback_proxy.rs` |
| `crates/service-auth/src/compat/k8s/principal.rs` | `crates/service-auth/src/api/k8s/principal.rs` |
| `crates/service-auth/src/compat/k8s/projected.rs` | `crates/service-auth/src/api/k8s/projected.rs` |
| `crates/service-auth/src/compat/k8s/review.rs` | `crates/service-auth/src/api/k8s/review.rs` |
| `crates/service-auth/src/compat/k8s/token_request.rs` | `crates/service-auth/src/api/k8s/token_request.rs` |
| `crates/service-auth/src/compat/llm.rs` | `crates/service-auth/src/api/llm.rs` |
| `crates/service-auth/src/compat/reload.rs` | `crates/service-auth/src/api/reload.rs` |
| `crates/service-auth/src/compat/role_map.rs` | removed facade; see [service-auth API changes](ddd-p2.md#service-auth) |
| `crates/service-auth/src/compat/scoped.rs` | removed facade; see [service-auth API changes](ddd-p2.md#service-auth) |
| `crates/service-auth/src/infrastructure/google/http.rs` | `crates/service-auth/src/app/google_verifier.rs`, `crates/service-auth/src/infrastructure/google/http.rs` |
| `crates/service-auth/src/infrastructure/google/metadata.rs` | removed as a source file; see [service-auth API changes](ddd-p2.md#service-auth) |
| `crates/service-auth/src/infrastructure/google/metadata/tests.rs` | removed as a source file; see [service-auth API changes](ddd-p2.md#service-auth) |

## service-backup

| P1 file | P2 file or status |
|---|---|
| `crates/service-backup/src/compat.rs` | `crates/service-backup/src/api.rs` (public module declarations) |
| `crates/service-backup/src/compat/llm.rs` | `crates/service-backup/src/api/llm.rs` |
| `crates/service-backup/src/infrastructure/admin_snapshot/lenient_fetch.rs` | removed as a source file; see [service-backup API changes](ddd-p2.md#service-backup) |
| `crates/service-backup/src/infrastructure/admin_snapshot/transport.rs` | `crates/service-backup/src/app/projected_bearer.rs`, `crates/service-backup/src/infrastructure/admin_snapshot/transport.rs`, `crates/service-backup/src/infrastructure/admin_snapshot/transport_config.rs` |
| `crates/service-backup/src/infrastructure/sink.rs` | `crates/service-backup/src/app/sink_from_destination.rs`, `crates/service-backup/src/domain/backup_sink.rs`, `crates/service-backup/src/infrastructure/sink.rs` |

## service-http

| P1 file | P2 file or status |
|---|---|
| `crates/service-http/src/compat.rs` | `crates/service-http/src/api.rs` (public module declarations) |
| `crates/service-http/src/compat/admission.rs` | removed facade; see [service-http API changes](ddd-p2.md#service-http) |
| `crates/service-http/src/compat/body_limit.rs` | removed facade; see [service-http API changes](ddd-p2.md#service-http) |
| `crates/service-http/src/compat/config.rs` | removed facade; see [service-http API changes](ddd-p2.md#service-http) |
| `crates/service-http/src/compat/content_decode.rs` | removed facade; see [service-http API changes](ddd-p2.md#service-http) |
| `crates/service-http/src/compat/error.rs` | removed facade; see [service-http API changes](ddd-p2.md#service-http) |
| `crates/service-http/src/compat/logging.rs` | removed facade; see [service-http API changes](ddd-p2.md#service-http) |
| `crates/service-http/src/compat/metrics.rs` | removed facade; see [service-http API changes](ddd-p2.md#service-http) |
| `crates/service-http/src/compat/probes.rs` | removed facade; see [service-http API changes](ddd-p2.md#service-http) |
| `crates/service-http/src/compat/readiness.rs` | removed facade; see [service-http API changes](ddd-p2.md#service-http) |
| `crates/service-http/src/compat/reverse_proxy.rs` | removed facade; see [service-http API changes](ddd-p2.md#service-http) |
| `crates/service-http/src/compat/server_timing.rs` | removed facade; see [service-http API changes](ddd-p2.md#service-http) |
| `crates/service-http/src/compat/signal.rs` | removed facade; see [service-http API changes](ddd-p2.md#service-http) |
| `crates/service-http/src/compat/transport.rs` | `crates/service-http/src/api/transport.rs` |
| `crates/service-http/src/compat/weighted_admission.rs` | removed facade; see [service-http API changes](ddd-p2.md#service-http) |

## service-k8s

| P1 file | P2 file or status |
|---|---|
| `crates/service-k8s/src/application/certificate/reconcile.rs` | `crates/service-k8s/src/app/certificate.rs`, `crates/service-k8s/src/application/certificate/reconcile.rs` |
| `crates/service-k8s/src/compat.rs` | `crates/service-k8s/src/api.rs` (public module declarations) |
| `crates/service-k8s/src/compat/certificate.rs` | `crates/service-k8s/src/api/certificate.rs` |
| `crates/service-k8s/src/compat/controller.rs` | `crates/service-k8s/src/api/controller.rs` |
| `crates/service-k8s/src/compat/crd.rs` | `crates/service-k8s/src/api/crd.rs` |
| `crates/service-k8s/src/compat/lease.rs` | `crates/service-k8s/src/api/lease.rs` |
| `crates/service-k8s/src/compat/lifecycle.rs` | `crates/service-k8s/src/api/lifecycle.rs` |
| `crates/service-k8s/src/compat/llm.rs` | `crates/service-k8s/src/api/llm.rs` |
| `crates/service-k8s/src/compat/metrics.rs` | `crates/service-k8s/src/api/metrics.rs` |
| `crates/service-k8s/src/compat/render.rs` | `crates/service-k8s/src/api/render.rs` |
| `crates/service-k8s/src/compat/resize.rs` | `crates/service-k8s/src/api/resize.rs` |
| `crates/service-k8s/src/compat/service.rs` | `crates/service-k8s/src/api/service.rs` |
| `crates/service-k8s/src/compat/stateful.rs` | `crates/service-k8s/src/api/stateful.rs` |
| `crates/service-k8s/src/domain/condition.rs` | `crates/service-k8s/src/application/condition.rs`, `crates/service-k8s/src/domain/condition.rs` |
| `crates/service-k8s/src/domain/condition/tests.rs` | `crates/service-k8s/src/application/condition/tests.rs`, `crates/service-k8s/src/domain/condition/tests.rs` |
| `crates/service-k8s/src/infrastructure/certificate/secret_layout.rs` | `crates/service-k8s/src/domain/certificate/secret_layout.rs`, `crates/service-k8s/src/infrastructure/certificate/leaf_parser.rs` |
| `crates/service-k8s/src/infrastructure/certificate/secret_layout/tests.rs` | `crates/service-k8s/src/domain/certificate/secret_layout/tests.rs` |
| `crates/service-k8s/src/infrastructure/lease.rs` | `crates/service-k8s/src/domain/leadership.rs`, `crates/service-k8s/src/infrastructure/lease.rs` |
| `crates/service-k8s/src/infrastructure/manifest.rs` | `crates/service-k8s/src/infrastructure/manifest.rs`, `crates/service-k8s/src/infrastructure/manifest/render_ctx.rs` |
| `crates/service-k8s/src/interfaces/operator.rs` | `crates/service-k8s/src/app/operator.rs`, `crates/service-k8s/src/interfaces/operator.rs` |
| `crates/service-k8s/src/interfaces/operator/managed_service.rs` | `crates/service-k8s/src/application/operator/managed_service.rs` |
| `crates/service-k8s/src/interfaces/operator/managed_service/tests.rs` | `crates/service-k8s/src/application/operator/managed_service/tests.rs` |

## service-projection

| P1 file | P2 file or status |
|---|---|
| `crates/service-projection/src/application/registry.rs` | `crates/service-projection/src/app/registry.rs`, `crates/service-projection/src/application/registry.rs` |

## storage-segment

| P1 file | P2 file or status |
|---|---|
| `crates/storage-segment/src/application/archive.rs` | `crates/storage-segment/src/app/archive.rs`, `crates/storage-segment/src/application/archive.rs` |
| `crates/storage-segment/src/application/paged_catalog.rs` | `crates/storage-segment/src/app/paged_catalog.rs`, `crates/storage-segment/src/application/paged_catalog.rs` |
| `crates/storage-segment/src/infrastructure/content_hash.rs` | `crates/storage-segment/src/domain/catalog/content_hash.rs` |
| `crates/storage-segment/src/infrastructure/page_codec.rs` | `crates/storage-segment/src/domain/catalog/page_codec.rs` |

## surface

| P1 file | P2 file or status |
|---|---|
| `crates/surface/src/lib.rs` | `crates/surface/src/callback.rs`, `crates/surface/src/component.rs`, `crates/surface/src/element.rs`, `crates/surface/src/lib.rs`, `crates/surface/src/props.rs`, `crates/surface/src/snapshot.rs`, `crates/surface/src/surface_node.rs` |

## ui-runtime

| P1 file | P2 file or status |
|---|---|
| `crates/ui-runtime/src/lib.rs` | `crates/ui-runtime/src/domain/debug.rs`, `crates/ui-runtime/src/domain/debug/fibers.rs`, `crates/ui-runtime/src/domain/debug/hooks.rs`, `crates/ui-runtime/src/domain/fiber.rs`, `crates/ui-runtime/src/domain/fiber_id.rs`, `crates/ui-runtime/src/domain/hook_slot.rs`, `crates/ui-runtime/src/domain/mount_handle.rs`, `crates/ui-runtime/src/domain/runtime.rs`, `crates/ui-runtime/src/domain/update_scheduler.rs`, `crates/ui-runtime/src/domain/use_callback.rs`, `crates/ui-runtime/src/domain/use_effect_once.rs`, `crates/ui-runtime/src/domain/use_memo.rs`, `crates/ui-runtime/src/domain/use_reducer.rs`, `crates/ui-runtime/src/domain/use_ref.rs`, `crates/ui-runtime/src/domain/use_state.rs`, `crates/ui-runtime/src/lib.rs` |
