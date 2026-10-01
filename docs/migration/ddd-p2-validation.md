# DDD P2 validation

Verified on 2026-10-01 before the documentation commit.
P1 baseline: `f15b896fe1dbba00fc4b4ecc0d4debb8eefacd67`. P2 source and test layout: `daf8bf61270e2547d5626e519ada6c7376283a73`.
The documentation commit changes no Rust source, schema fixture or checker policy.

## Results

| Check | Result |
|---|---|
| Full workspace tests, all features | 2,092 passed, 0 failed, 24 ignored; exit 0 |
| Workspace Clippy, all targets and features | exit 0; existing warnings remain |
| Strict architecture check against P1 | 0 errors, 0 warnings; exit 0 |
| Architecture policy | unchanged; enforcement remains enabled |
| Exception ratchet | 10 long-term entries; no new exception and no large-file exception |
| Public API inventories | all 30 crates captured; 28 have changes listed in the guide |
| Fixed downstream snapshots | P1 passes 17/17; P2 passes meter; 16 need migration |
| Downstream diagnostics | all 506 observed diagnostics mapped to migration recipes |

The 24 ignored tests were not run. This receipt does not replace a downstream
feature build, product acceptance gate or release approval.
The pinned `adversarial_recovery`, `stateful_instance_render` and
`stateful_adapter_equivalence` module names and paths remain in place.
The Raft implementor-gate help text is unchanged.

## Commands and cache control

The checks used Cargo and rustc 1.96.1. The commands ran from the core root:

```sh
cargo test --offline --locked --workspace --all-features --no-fail-fast
cargo clippy --offline --locked --workspace --all-targets --all-features
uv run --script /Users/chrischeng/faberlines/workspace/scripts/meta/rust_arch_contract.py check --repo /Users/chrischeng/faberlines/core --base refactor/ddd-p1 --strict
```

The `UV_CACHE_DIR` and `npm_config_cache` paths were temporary task directories.
This lets generated-client tests write caches within the permitted files.
The first restricted-cache attempt failed in five client profile tests.
The final full test run used the permitted caches and passed.

P1 was exported with `git archive`. The final comparison cleared only the 30
workspace packages from the Cargo target directory between P1 and P2.
It then compiled both test inventories and both Clippy runs from source.
The final P2 tests were executed after that clear.
Future comparisons should use separate target directories for the two source trees.
Sharing workspace artifacts can reuse a test binary from the other tree.

Fresh warning output contained 255 warning headers for P1 and 231 for P2.
These counts exclude Cargo summary lines and include compiler warnings.
The P2 log introduced no warning category absent from P1.
The two `clone_on_copy` messages name `Index` instead of the former `u64`.

## Test inventory

Both inventories use `cargo test --offline --locked --workspace --all-features -- --list`.
P1 lists 2,033 test and documentation-test entries. P2 lists 2,116.
The comparison matches function names within each crate and test kind.
A module-only move is paired. Unpaired entries include renamed helpers or examples;
they are listed below so the count can be audited.
There are 237 added or renamed entries and 154 removed or renamed entries: net +83.

| Crate | P1 | P2 | Added or renamed | Removed or renamed |
|---|---|---|---|---|
| build-stamp | 11 | 11 | 0 | 0 |
| claim-token | 3 | 7 | 4 | 0 |
| cli-std | 72 | 84 | 16 | 4 |
| compass | 809 | 706 | 32 | 135 |
| index-text | 6 | 12 | 6 | 0 |
| metrics-prometheus | 12 | 13 | 1 | 0 |
| metrics-remote-write | 2 | 2 | 0 | 0 |
| openapi-codegen | 112 | 122 | 10 | 0 |
| peer-tls | 41 | 41 | 0 | 0 |
| raft-core | 62 | 65 | 8 | 5 |
| raft-runtime | 220 | 242 | 24 | 2 |
| server-http | 12 | 14 | 2 | 0 |
| server-lifecycle | 21 | 23 | 2 | 0 |
| server-tcp | 7 | 12 | 5 | 0 |
| service-auth | 157 | 167 | 12 | 2 |
| service-backup | 34 | 58 | 26 | 2 |
| service-collector | 4 | 7 | 3 | 0 |
| service-executor | 7 | 8 | 1 | 0 |
| service-http | 78 | 79 | 3 | 2 |
| service-k8s | 224 | 247 | 23 | 0 |
| service-mcp | 1 | 1 | 0 | 0 |
| service-observability | 16 | 16 | 0 | 0 |
| service-projection | 4 | 20 | 16 | 0 |
| storage-durable | 79 | 100 | 23 | 2 |
| storage-object | 3 | 5 | 2 | 0 |
| storage-segment | 9 | 16 | 7 | 0 |
| surface | 3 | 8 | 5 | 0 |
| transport-h2c | 21 | 24 | 3 | 0 |
| transport-otlp | 2 | 2 | 0 | 0 |
| ui-runtime | 1 | 4 | 3 | 0 |

The removals have these causes:

- compass: 135 entries belonged to dead or duplicate search, formatting,
  autofix, embedded-Markdown, Go/Rust/TypeScript type-system and code-generation modules removed under D7.
- cli-std: three entries exercised the removed `assert_command_chainable` helper.
  The secret-data decoder test was renamed with the surviving private decoder.
- raft-core: five entries exercised the removed learner-gap convenience method.
- raft-runtime: static-membership and cache-footprint convenience items each lost one test.
- service-auth: two tests belonged to the removed duplicate metadata token adapter.
- service-backup: two lenient-fetch tests were replaced by strict transport tests.
- service-http: two documentation examples moved from facades to their defining items.
- storage-durable: two entries exercised removed cursor reread helpers.
  The replacement pin/read boundary is covered by Raft storage and command-lease tests.

The additions cover wire and schema golden fixtures, typed errors, constructors,
port wiring, downcasts, protocol equivalence and the new identity types.

### claim-token

Added or renamed:

```text
crates/claim-token/src/domain/scope.rs - domain::scope::Scope::new (line 21)
domain::scope::tests::expiry_boundaries_keep_the_wire_number_and_inclusive_deadline
domain::scope::tests::scope_json_is_pinned
domain::scope::tests::signed_token_bytes_are_pinned
```


### cli-std

Added or renamed:

```text
application::issue::tests::golden_diagnostics_and_followup_bytes
application::llm::tests::golden_sectioned_topic_output_bytes
application::llm::tests::golden_topic_output_bytes
application::port_tests::comment_declined_stops_before_the_client_is_built
application::port_tests::comment_dry_run_builds_no_client_and_asks_nothing
application::port_tests::comment_reopens_then_comments_with_the_github_token
application::port_tests::create_goes_through_courier_when_it_is_configured
application::port_tests::create_without_a_credential_submits_nothing_and_asks_nothing
application::port_tests::public_futures_are_send
application::port_tests::upgrade_check_lists_releases_and_installs_nothing
crates/cli-std/src/application/issue/comment.rs - application::issue::comment::CommentOptions (line 25)
crates/cli-std/src/application/issue/create.rs - application::issue::create::CreateOptions (line 30)
crates/cli-std/src/application/issue/search.rs - application::issue::search::SearchOptions (line 18)
crates/cli-std/src/application/llm.rs - application::llm::Topic (line 5)
crates/cli-std/src/application/upgrade.rs - application::upgrade::Options (line 15)
infrastructure::connect::tests::decode_secret_data_decodes_base64_field
```

Removed or renamed:

```text
application::chainable::tests::assert_command_chainable_surfaces_violation
application::chainable::tests::assert_command_chainable_wraps_process_output
crates/cli-std/src/application/chainable.rs - application::chainable::assert_command_chainable (line 92)
infrastructure::connect::tests::secret_data_bytes_decodes_base64_field
```


### compass

Added or renamed:

```text
application::editor::tests::analyse_answers_symbol_queries
application::editor::tests::close_forgets_the_document
application::editor::tests::completions_cover_modules_builtins_and_keywords
application::editor::tests::open_rejects_unknown_languages
application::editor::tests::refactorings_offer_extract_for_a_selection
domain::check::lint_config::tests::builders_replace_the_defaults
domain::diagnostic::rule_code::tests::displays_and_compares_as_its_string
domain::diagnostic::rule_code::tests::serializes_as_a_bare_string
domain::import_graph::extract::tests::test_extract_imports_whitespace_is_ascii
domain::lint::custom::pattern::tests::a_not_word_boundary_inside_a_character_does_not_hide_a_match
domain::lint::custom::pattern::tests::bracket_classes_are_ascii
domain::lint::custom::pattern::tests::case_insensitivity_is_ascii
domain::lint::custom::pattern::tests::everyday_rules_match_as_before
domain::lint::custom::pattern::tests::flags_and_anchors
domain::lint::custom::pattern::tests::nesting_limit_is_regex_lite_s
domain::lint::custom::pattern::tests::perl_classes_are_ascii
domain::lint::custom::pattern::tests::rejections_keep_regex_lite_messages
domain::lint::custom::pattern::tests::translation_spells_out_ascii_classes
domain::lint::custom::pattern::tests::word_boundaries_are_ascii
domain::lint::custom::tests::test_valid_rules_are_not_rejected
golden_daemon_protocol::method_payload_json_bytes_are_pinned
golden_daemon_protocol::request_json_bytes_are_pinned
golden_daemon_protocol::request_without_params_field_decodes
golden_daemon_protocol::response_json_bytes_are_pinned
golden_daemon_protocol::rpc_error_codes_are_pinned
golden_diagnostic::diagnostic_json_bytes_are_pinned
golden_diagnostic::every_severity_and_category_json_is_pinned
golden_diagnostic::quick_fix_json_bytes_are_pinned
golden_diagnostic::reporter_output_bytes_are_pinned
golden_semantic_model_cache::diagnostic_without_fixes_bincode_bytes_are_pinned
golden_semantic_model_cache::disk_cache_entry_bytes_are_pinned
golden_semantic_model_cache::semantic_model_json_bytes_are_pinned
```

Removed or renamed:

```text
application::search::engine::tests::test_build_and_search
application::search::engine::tests::test_incremental_update
application::search::engine::tests::test_save_and_load
domain::lint::autofix::tests::test_apply_all_fixes_non_overlapping
domain::lint::autofix::tests::test_apply_all_fixes_skips_overlapping
domain::lint::autofix::tests::test_apply_fix_line_deletion
domain::lint::autofix::tests::test_apply_fix_simple_replacement
domain::lint::autofix::tests::test_position_to_offset_edges
domain::lint::embedded_markdown::tests::test_extract_block_description
domain::lint::embedded_markdown::tests::test_extract_inline_description
domain::lint::embedded_markdown::tests::test_extract_summary_field
domain::lint::embedded_markdown::tests::test_lint_clean_content_no_diagnostics
domain::lint::embedded_markdown::tests::test_lint_heading_skip
domain::lint::embedded_markdown::tests::test_lint_line_length
domain::lint::markdown::tests::test_symbol_extractor_code_fence
domain::lint::markdown::tests::test_symbol_extractor_frontmatter
domain::lint::markdown::tests::test_symbol_extractor_headings
domain::lint::markdown::tests::test_symbol_extractor_links
domain::lint::markdown::tests::test_symbol_extractor_mdx_component
domain::refactoring::rename::tests::test_line_col_to_byte
domain::rust_type_system::advanced::tests::test_advanced_inferencer_array_size
domain::rust_type_system::advanced::tests::test_advanced_inferencer_elision
domain::rust_type_system::advanced::tests::test_binop_add
domain::rust_type_system::advanced::tests::test_binop_div_by_zero
domain::rust_type_system::advanced::tests::test_binop_mul
domain::rust_type_system::advanced::tests::test_const_param_array_size
domain::rust_type_system::advanced::tests::test_elision_no_elided_lifetimes
domain::rust_type_system::advanced::tests::test_elision_rule1_no_output
domain::rust_type_system::advanced::tests::test_elision_rule2_single_input
domain::rust_type_system::advanced::tests::test_elision_rule3_self_ref
domain::rust_type_system::advanced::tests::test_into_array_type
domain::rust_type_system::advanced::tests::test_literal_array_size
domain::rust_type_system::advanced::tests::test_missing_const_param
domain::rust_type_system::infer::tests::test_fresh_type_var
domain::rust_type_system::infer::tests::test_type_binding
domain::rust_type_system::infer::tests::test_unify_same_types
domain::rust_type_system::infer::tests::test_unify_type_mismatch
domain::rust_type_system::infer::tests::test_unify_with_infer
domain::rust_type_system::lifetimes::tests::test_borrow_state_basic
domain::rust_type_system::lifetimes::tests::test_borrow_state_immutable_then_mutable
domain::rust_type_system::lifetimes::tests::test_borrow_state_move
domain::rust_type_system::lifetimes::tests::test_borrow_state_move_while_borrowed
domain::rust_type_system::lifetimes::tests::test_borrow_state_mutable_conflict
domain::rust_type_system::lifetimes::tests::test_lifetime_analyzer_basic
domain::rust_type_system::lifetimes::tests::test_lifetime_analyzer_scope
domain::rust_type_system::lifetimes::tests::test_lifetime_analyzer_use_after_move
domain::rust_type_system::symbols::tests::test_discriminant_parsing_logic
domain::rust_type_system::symbols::tests::test_rust_symbols_default
domain::rust_type_system::symbols::tests::test_symbol_collector_creation
domain::rust_type_system::symbols::tests::test_trait_ref_creation
domain::rust_type_system::symbols::tests::test_where_predicate_types
domain::rust_type_system::traits::tests::test_find_impls_for_type
domain::rust_type_system::traits::tests::test_register_impl
domain::rust_type_system::traits::tests::test_trait_resolver_creation
domain::rust_type_system::types::tests::test_implements_trait_exact_match
domain::rust_type_system::types::tests::test_implements_trait_generic_impl
domain::rust_type_system::types::tests::test_implements_trait_no_impls
domain::rust_type_system::types::tests::test_implements_trait_wrong_trait
domain::rust_type_system::types::tests::test_lifetime_outlives
domain::rust_type_system::types::tests::test_rust_type_to_generic
domain::rust_type_system::types::tests::test_struct_fields
domain::search::index::tests::test_doc_search
domain::search::index::tests::test_insert_and_query_by_name
domain::search::index::tests::test_remove_file
domain::search::index::tests::test_serialization_roundtrip
domain::search::query::tests::test_doc_search
domain::search::query::tests::test_similarity
domain::search::query::tests::test_usages
domain::semantic::types::go_advanced::tests::test_composite_literal_tracking
domain::semantic::types::go_advanced::tests::test_empty_interface_always_satisfied
domain::semantic::types::go_advanced::tests::test_interface_satisfaction_fails
domain::semantic::types::go_advanced::tests::test_interface_satisfaction_passes
domain::semantic::types::go_advanced::tests::test_interface_satisfaction_with_tree_sitter
domain::semantic::types::go_advanced::tests::test_method_set_value_and_pointer_receivers
domain::semantic::types::go_advanced::tests::test_no_structs_no_results
domain::semantic::types::go_advanced::tests::test_partial_interface_satisfaction
domain::semantic::types::go_advanced::tests::test_validate_type_assertions_builtin
domain::semantic::types::go_advanced::tests::test_validate_type_assertions_named
domain::semantic::types::go_tests::test_channel_type_inference
domain::semantic::types::go_tests::test_empty_method_set
domain::semantic::types::go_tests::test_generic_param_extraction
domain::semantic::types::go_tests::test_interface_type_inference
domain::semantic::types::go_tests::test_map_type_inference
domain::semantic::types::go_tests::test_method_set_collection
domain::semantic::types::go_tests::test_multiple_types_in_file
domain::semantic::types::go_tests::test_pointer_type_in_struct
domain::semantic::types::go_tests::test_struct_type_inference
domain::semantic::types::go_tests::test_type_assertion_tracking
domain::semantic_search::rust_provider::tests::test_extract_rust_docstrings
domain::semantic_search::rust_provider::tests::test_find_rust_usages
domain::semantic_search::rust_provider::tests::test_find_trait_implementations
domain::semantic_search::rust_provider::tests::test_rust_call_graph
domain::ts_type_system::advanced::tests::test_check_extends_satisfied
domain::ts_type_system::advanced::tests::test_check_extends_unknown_for_typevar
domain::ts_type_system::advanced::tests::test_check_extends_violated
domain::ts_type_system::advanced::tests::test_conditional_false_branch
domain::ts_type_system::advanced::tests::test_conditional_true_branch
domain::ts_type_system::advanced::tests::test_infer_arity_mismatch
domain::ts_type_system::advanced::tests::test_infer_constraint_violation
domain::ts_type_system::advanced::tests::test_infer_identity_function
domain::ts_type_system::advanced::tests::test_keyof_unknown_interface
domain::ts_type_system::advanced::tests::test_mapped_type_keyof_interface
domain::ts_type_system::advanced::tests::test_template_match_number_interpolation
domain::ts_type_system::advanced::tests::test_template_match_string_interpolation
domain::ts_type_system::advanced::tests::test_template_no_interpolation
domain::ts_type_system::infer::tests::test_bind_and_lookup
domain::ts_type_system::infer::tests::test_fresh_type_var
domain::ts_type_system::infer::tests::test_partial_type
domain::ts_type_system::infer::tests::test_structural_compatibility
domain::ts_type_system::infer::tests::test_type_context_interface
domain::ts_type_system::infer::tests::test_union_type_handling
domain::ts_type_system::types::tests::test_assignability_any
domain::ts_type_system::types::tests::test_assignability_intersection
domain::ts_type_system::types::tests::test_assignability_never
domain::ts_type_system::types::tests::test_assignability_primitives
domain::ts_type_system::types::tests::test_assignability_union
domain::ts_type_system::types::tests::test_conditional_type
domain::ts_type_system::types::tests::test_interface_creation
domain::ts_type_system::types::tests::test_literal_to_base
domain::ts_type_system::types::tests::test_template_literal_evaluation
domain::type_codegen::generator::tests::test_docstring_generation
domain::type_codegen::generator::tests::test_module_stub_generation
domain::type_codegen::generator::tests::test_pytest_generation
domain::type_codegen::generator::tests::test_type_stub_generation
domain::type_refactoring::multilang::tests::test_extract_function_rust
domain::type_refactoring::multilang::tests::test_extract_function_typescript
domain::type_refactoring::multilang::tests::test_keyword_detection
domain::type_refactoring::multilang::tests::test_keyword_validation_rust
domain::type_refactoring::multilang::tests::test_language_detection
infrastructure::format::detect::tests::test_find_binary_known
infrastructure::format::detect::tests::test_find_binary_nonexistent
infrastructure::format::registry::tests::test_format_returns_none_for_unavailable_formatter
infrastructure::format::registry::tests::test_is_available_unknown_language
infrastructure::format::registry::tests::test_language_for_extension
infrastructure::format::registry::tests::test_status_returns_all_registered_formatters
```


### index-text

Added or renamed:

```text
crates/index-text/src/domain/document.rs - domain::document::TextDocument::new (line 19)
domain::document::tests::text_document_json_is_pinned
domain::document::tests::text_document_without_fields_is_pinned
domain::snapshot::tests::text_index_snapshot_bytes_are_pinned
domain::snapshot::tests::text_index_snapshot_without_tombstones_omits_the_key
maximum_version_tombstone_blocks_replay_after_restore
```


### metrics-prometheus

Added or renamed:

```text
histogram::tests::bucket_getters_are_usable_in_const_context
```


### openapi-codegen

Added or renamed:

```text
crates/openapi-codegen/src/domain/generation/options.rs - domain::generation::options::GenOptions (line 43)
run_entry::a_good_spec_writes_the_generated_files_and_exits_0
run_entry::a_spec_that_cannot_be_read_exits_2
run_entry::a_spec_that_does_not_parse_exits_1
run_entry::an_output_directory_that_cannot_be_written_exits_1
tests::error_display::emitter_spec_parse_error_is_pinned
tests::error_display::file_bearer_auth_errors_are_pinned
tests::error_display::file_bearer_auth_non_utf8_path_error_is_pinned
tests::error_display::target_policy_errors_are_pinned
tests::error_display::unknown_target_profile_error_is_pinned
```


### raft-core

Added or renamed:

```text
driver_ports::a_driver_elects_a_leader_through_the_delivery_port
driver_ports::a_node_restarts_from_what_it_saved_through_the_storage_port
wire_golden::conf_state_binary_encoding_is_pinned
wire_golden::debug_output_is_pinned
wire_golden::membership_and_conf_state_json_are_pinned
wire_golden::raft_entry_and_persisted_state_json_are_pinned
wire_golden::raft_messages_json_are_pinned
wire_golden::refusals_and_node_keyed_maps_json_are_pinned
```

Removed or renamed:

```text
learner_freshness::a_learner_ahead_of_the_commit_index_is_reported_as_owed_nothing
learner_freshness::a_learner_present_from_construction_reports_the_entries_it_is_missing
learner_freshness::only_a_leader_reports_a_gap_and_only_about_a_learner
learner_freshness::the_admission_predicate_is_unchanged_for_a_learner_admitted_at_runtime
learner_freshness::the_gap_is_measured_against_the_commit_index_not_the_last_appended_index
```


### raft-runtime

Added or renamed:

```text
application::fenced_assignment::tests::epoch_exhaustion_is_rejected
application::fenced_assignment::tests::epoch_prints_and_serializes_as_the_bare_number
port_error_downcast::anyhow_backpressure_downcasts_from_propose
port_error_downcast::anyhow_snapshot_error_comes_back_whole
port_error_downcast::backpressure_under_anyhow_context_downcasts_from_propose
port_error_downcast::backpressure_under_boxed_anyhow_context_downcasts_from_propose
port_error_downcast::membership_error_from_anyhow_comes_back_whole
port_error_downcast::other_admission_error_keeps_its_text
port_error_downcast::prefix_unavailable_keeps_its_text
port_error_downcast::typed_backpressure_downcasts_from_propose
port_error_downcast::typed_snapshot_error_is_reachable_through_other
tests::peer_wire_golden::publish_envelope_and_not_leader_reply_are_pinned
tests::peer_wire_golden::snapshot_envelopes_and_capable_reply_are_pinned
tests::peer_wire_golden::vote_append_and_timeout_envelopes_are_pinned
tests::peer_wire_split::capable_reply_without_accepted_decodes_as_refused_in_both_copies
tests::peer_wire_split::publish_envelope_matches
tests::peer_wire_split::snapshot_envelopes_and_capable_reply_match
tests::peer_wire_split::vote_append_and_timeout_envelopes_match
wire_golden::assignment_and_refusal_json_are_pinned
wire_golden::cluster_view_json_is_pinned
wire_golden::conformance_envelope_meta_and_node_view_are_pinned
wire_golden::durable_state_log_and_snapshot_bytes_are_pinned
wire_golden::named_group_state_file_name_is_pinned
wire_golden::raft_status_json_is_pinned
```

Removed or renamed:

```text
application::topology::tests::static_membership_rejects_replica_delta
durable_layout::measurement_4_cache_footprint_bounded_and_invariant
```


### server-http

Added or renamed:

```text
options::http_options_keep_defaults_and_builder_settings
options::tls_options_carry_http_options_and_metrics
```


### server-lifecycle

Added or renamed:

```text
bind_config::bind_config_round_trips_a_socket_addr
shutdown_deadline::new_keeps_a_fixed_expiry_and_checks_the_reserve
```


### server-tcp

Added or renamed:

```text
config_getters_report_defaults_and_builder_settings
connection_result_builders_set_each_counter
handler::error::tests::a_message_logs_as_itself
handler::error::tests::an_anyhow_error_logs_the_text_it_logged_before
handler::error::tests::other_keeps_the_wrapped_message_and_type
```


### service-auth

Added or renamed:

```text
async_trait_review_backend::an_async_trait_backend_authenticates_and_authorizes
async_trait_review_backend::an_async_trait_backend_error_is_unavailable
async_trait_review_backend::an_async_trait_backend_is_a_dyn_review_backend
wire_golden::google_introspection_answer_decode_is_pinned
wire_golden::registry_documents_decode_is_pinned
wire_golden::registry_merge_errors_are_pinned
wire_golden::registry_parse_errors_are_pinned
wire_golden::review_errors_are_pinned
wire_golden::token_claims_decode_is_pinned
wire_golden::token_request_body_is_pinned
wire_golden::token_request_errors_are_pinned
wire_golden::token_request_target_errors_are_pinned
```

Removed or renamed:

```text
infrastructure::google::metadata::tests::metadata_token_source_sends_header_and_query_and_returns_token
infrastructure::google::metadata::tests::metadata_token_source_surfaces_typed_error_naming_base_url
```


### service-backup

Added or renamed:

```text
admin_request_golden::missing_projected_token_error_is_pinned
admin_request_golden::wrong_audience_projected_token_error_is_pinned_and_redacted
admin_snapshot_backup::a_non_200_status_is_redacted_and_opens_no_sink
admin_snapshot_backup::a_redirect_is_not_followed
admin_snapshot_backup::the_backup_future_is_send
admin_snapshot_backup::writes_the_exact_200_bytes_to_the_destination
application::destination_schemes::tests::every_scheme_is_listed_in_parse_order_with_its_support
application::destination_schemes::tests::s3_support_follows_the_feature
crates/service-backup/src/infrastructure/admin_snapshot/transport_config.rs - infrastructure::admin_snapshot::transport_config::AdminSnapshotTransportConfig (line 16)
infrastructure::admin_snapshot::tests::strict_backup_redacts_a_non_success_status_body
infrastructure::admin_snapshot::tests::strict_backup_sends_the_bearer_and_returns_exact_bytes
schema_golden::backup_destination_schema_is_pinned
schema_golden::backup_policy_schema_is_pinned
schema_golden::retention_policy_schema_is_pinned
schema_golden::scheduled_backup_policy_schema_is_pinned
sink_error::a_put_error_keeps_its_text_and_chain
sink_error::a_sink_error_is_transparent
wire_golden::backup_destination_gcs_json_is_pinned
wire_golden::backup_destination_local_json_is_pinned
wire_golden::backup_destination_s3_json_is_pinned
wire_golden::backup_policy_and_retention_json_are_pinned
wire_golden::destination_uri_errors_are_pinned
wire_golden::rendered_sectioned_llm_topic_is_pinned
wire_golden::scheduled_backup_policy_json_is_pinned
wire_golden::scheduled_policy_conversion_errors_are_pinned
wire_golden::static_llm_topic_is_pinned
```

Removed or renamed:

```text
infrastructure::admin_snapshot::tests::fetches_exact_snapshot_bytes_with_bearer_auth
infrastructure::admin_snapshot::tests::keeps_non_success_status_and_body_in_the_diagnostic
```


### service-collector

Added or renamed:

```text
application::config::tests::try_new_keeps_every_field
application::config::tests::try_new_rejects_zero_limits
domain::source_offset::tests::source_offsets_keep_bare_numbers_in_checkpoints
```


### service-executor

Added or renamed:

```text
group_commit::config_getters_report_the_limits_and_queue_capacity
```


### service-http

Added or renamed:

```text
crates/service-http/src/interfaces/body_limit.rs - interfaces::body_limit::body_limit_layer (line 159)
crates/service-http/src/interfaces/server_timing.rs - interfaces::server_timing::ServerTimingDisclosure (line 112)
ingest_runtime::ingest_configs_report_the_limits_they_were_built_with
```

Removed or renamed:

```text
crates/service-http/src/compat/body_limit.rs - compat::body_limit (line 41)
crates/service-http/src/compat/server_timing.rs - compat::server_timing (line 58)
```


### service-k8s

Added or renamed:

```text
application::operator::leadership::tests::a_campaign_starts_the_lease_on_its_own_election_and_waits_for_it
application::operator::leadership::tests::a_held_follower_is_never_promoted
application::operator::leadership::tests::a_held_leader_may_act
application::operator::managed_service::tests::constructors_keep_what_they_were_given
domain::certificate::issuer::tests::a_generator_failure_is_the_build_failure
domain::certificate::issuer::tests::build_puts_the_csr_in_the_request_and_hands_the_key_back_apart
infrastructure::manifest::tests::children::render_ctx_getters_return_the_constructor_arguments_in_order
infrastructure::manifest::tests::children::render_ctx_with_owner_takes_an_optional_owner
status_patch_golden::the_status_patch_body_is_pinned_byte_for_byte
wire_golden::a_cluster_spec_keeps_its_wire_bytes
wire_golden::a_condition_keeps_its_wire_bytes
wire_golden::a_condition_without_a_generation_omits_the_field
wire_golden::a_minimal_cluster_spec_fills_its_defaults
wire_golden::certificate_secrets::a_material_secret_keeps_its_bytes
wire_golden::certificate_secrets::a_trust_bundle_secret_keeps_its_bytes
wire_golden::the_capacity_policies_keep_their_wire_bytes
wire_golden::the_condition_schema_is_pinned
wire_golden::the_lifecycle_policies_keep_their_wire_bytes
wire_golden::the_lifecycle_policy_schema_is_pinned
wire_golden::the_probe_timing_schema_is_pinned
wire_golden::the_replica_layer_policy_schema_is_pinned
wire_golden::the_shard_split_policy_schema_is_pinned
workload_plan::typed_plan_renders_role_role_binding_and_cron_job_from_constructors
```


### service-projection

Added or renamed:

```text
crates/service-projection/src/domain/ids.rs - domain::ids::ProjectionCursor::distance_since (line 65)
domain::checkpoint::tests::empty_projection_checkpoint_json_is_pinned
domain::checkpoint::tests::projection_checkpoint_json_is_pinned
domain::checkpoint::timestamp_tests::checkpoint_stamps_the_given_time_with_milliseconds
domain::checkpoint::timestamp_tests::empty_stamps_the_given_time_with_milliseconds
domain::lag::tests::projection_lag_json_is_pinned
domain::projection::tests::projection_descriptor_json_is_pinned
domain::projection::tests::try_new_keeps_the_fields_and_rejects_invalid_names
domain::projection_error::tests::an_invalid_name_keeps_its_message
domain::projection_error::tests::other_keeps_the_wrapped_message
infrastructure::file_state_store::tests::persisted_state_reads_back_and_restores
infrastructure::file_state_store::tests::quarantine_moves_the_state_file_aside
infrastructure::state_store::tests::persisted_state_file_bytes_are_pinned
infrastructure::state_store::tests::projection_state_envelope_json_is_pinned
newtype_wire_golden::cursor_and_generation_keep_extreme_wire_values
newtype_wire_golden::projection_openapi_schemas_keep_the_p2_baseline_bytes
```


### storage-durable

Added or renamed:

```text
data_root_errors::a_policy_error_downcasts_through_other
data_root_errors::an_io_failure_from_open_is_found_in_the_anyhow_chain
data_root_errors::other_keeps_the_wrapped_message_and_type
data_root_errors::the_path_functions_keep_a_direct_io_downcast
data_root_golden::a_blocked_layout_write_keeps_its_error_text
data_root_golden::a_directory_that_is_a_file_keeps_its_error_text
data_root_golden::a_layout_that_is_a_directory_keeps_its_error_text
data_root_golden::a_locked_root_keeps_its_error_text
data_root_golden::a_new_root_writes_the_pinned_layout
data_root_golden::a_rejected_layout_keeps_the_policy_text
data_root_golden::a_root_that_is_a_file_keeps_its_create_error_text
data_root_golden::a_root_under_a_file_keeps_its_inspect_error_text
data_root_golden::an_undecodable_layout_keeps_its_error_text
data_root_golden::an_unreadable_layout_keeps_its_read_error_text
data_root_golden::an_unsafe_directory_keeps_its_error_text
data_root_golden::replace_manifest_errors_keep_their_text
data_root_golden::symlinks_keep_their_error_text
data_root_golden::the_default_legacy_error_keeps_its_text
data_root_golden::the_pinned_layout_decodes_to_its_manifest
framed_log_bytes::the_pinned_bytes_decode_to_two_frames
framed_log_bytes::two_frames_encode_to_the_pinned_bytes
log_frame::a_new_frame_equals_the_frame_read_from_disk
log_frame::a_new_frame_reads_back_its_parts
```

Removed or renamed:

```text
framed_log::tests::cursor::cursor_reread_keeps_initial_length_and_validates_crc
framed_log::tests::cursor::cursor_rereads_pinned_frame_after_compaction_without_advancing
```


### storage-object

Added or renamed:

```text
object::tests::object_meta_json_is_pinned
object::tests::object_meta_without_etag_or_update_time_writes_nulls
```


### storage-segment

Added or renamed:

```text
domain::catalog::entry::tests::catalog_entry_json_is_pinned
domain::catalog::entry::tests::catalog_entry_with_an_empty_value_is_pinned
infrastructure::object_store_error::tests::an_object_store_error_keeps_its_message
persisted_format::a_built_catalog_tree_has_a_pinned_root
persisted_format::a_built_catalog_writes_pinned_leaf_page_bytes
persisted_format::archive_receipts_json_is_pinned
persisted_format::catalog_root_and_page_ref_json_is_pinned
```


### surface

Added or renamed:

```text
builders::component_renders_its_props_through_its_render_fn
builders::default_props_leave_every_field_unset
builders::props_builders_set_what_the_getters_read
snapshot_json::pinned_json_decodes_to_surface_snapshot
snapshot_json::surface_snapshot_encodes_to_pinned_json
```


### transport-h2c

Added or renamed:

```text
config::connection_options_new_and_default_set_the_stream_cap
config::for_concurrency_caps_admission_at_the_target
config::manager_config_builders_set_every_field
```


### ui-runtime

Added or renamed:

```text
domain::debug::tests::debug_snapshots_address_fibers_by_fiber_id
domain::fiber_id::tests::fiber_id_debug_output_is_the_tuple_form
domain::fiber_id::tests::fiber_id_round_trips_its_raw_value
```

## Public API inventory

The inventory command was `cargo-public-api public-api -p <crate> --all-features`.
It used the installed nightly documentation toolchain and a task-local target directory.
P1 inventories were captured at the P1 final revision. P2 captures use the final Rust API.
The comparison uses sets of complete inventory lines, so reordered re-exports do not
look like a removed method. A derived trait implementation can add many lines.
These counts are inventory entries, not counts of independent breaking changes.

| Crate | Removed entries | Added entries | Migration |
|---|---|---|---|
| build-stamp | 0 | 0 | unchanged |
| claim-token | 3 | 192 | [claim-token](ddd-p2.md#claim-token) |
| cli-std | 43 | 113 | [cli-std](ddd-p2.md#cli-std) |
| compass | 5983 | 187 | [compass](ddd-p2.md#compass) |
| index-text | 9 | 132 | [index-text](ddd-p2.md#index-text) |
| metrics-prometheus | 16 | 16 | [metrics-prometheus](ddd-p2.md#metrics-prometheus) |
| metrics-remote-write | 1 | 0 | [metrics-remote-write](ddd-p2.md#metrics-remote-write) |
| openapi-codegen | 54 | 221 | [openapi-codegen](ddd-p2.md#openapi-codegen) |
| peer-tls | 4 | 5 | [peer-tls](ddd-p2.md#peer-tls) |
| raft-core | 8 | 196 | [raft-core](ddd-p2.md#raft-core) |
| raft-runtime | 44 | 214 | [raft-runtime](ddd-p2.md#raft-runtime) |
| server-http | 79 | 20 | [server-http](ddd-p2.md#server-http) |
| server-lifecycle | 10 | 9 | [server-lifecycle](ddd-p2.md#server-lifecycle) |
| server-tcp | 18 | 66 | [server-tcp](ddd-p2.md#server-tcp) |
| service-auth | 179 | 72 | [service-auth](ddd-p2.md#service-auth) |
| service-backup | 17 | 199 | [service-backup](ddd-p2.md#service-backup) |
| service-collector | 12 | 79 | [service-collector](ddd-p2.md#service-collector) |
| service-executor | 4 | 4 | [service-executor](ddd-p2.md#service-executor) |
| service-http | 151 | 13 | [service-http](ddd-p2.md#service-http) |
| service-k8s | 95 | 203 | [service-k8s](ddd-p2.md#service-k8s) |
| service-mcp | 0 | 0 | unchanged |
| service-observability | 6 | 3 | [service-observability](ddd-p2.md#service-observability) |
| service-projection | 31 | 325 | [service-projection](ddd-p2.md#service-projection) |
| storage-durable | 11 | 63 | [storage-durable](ddd-p2.md#storage-durable) |
| storage-object | 8 | 15 | [storage-object](ddd-p2.md#storage-object) |
| storage-segment | 12 | 69 | [storage-segment](ddd-p2.md#storage-segment) |
| surface | 16 | 31 | [surface](ddd-p2.md#surface) |
| transport-h2c | 15 | 29 | [transport-h2c](ddd-p2.md#transport-h2c) |
| transport-otlp | 2 | 0 | [transport-otlp](ddd-p2.md#transport-otlp) |
| ui-runtime | 8 | 11 | [ui-runtime](ddd-p2.md#ui-runtime) |

## Evidence digests

The local verification logs have these SHA-256 digests.
The repository records their results above; raw build output is not part of the source.

| Log | SHA-256 |
|---|---|
| `workspace-test-fresh.log` | `d7606e62bf2dc3e9b9bdeeae03a7f677ce75ecbe9cf9cfbbdeafdb50e102754c` |
| `p1-test-list.log` | `b663bd1fdf174d5dbed806987e28bc20b7b4310f44b28a97d547f8e432bd336f` |
| `p2-test-list.log` | `9f5c7911b3329ac4592de240c4a224da02aa6d539140d9ef56e7c583eb8f79f2` |
| `p1-clippy-fresh.log` | `4dda2099319df0565c6961bbd43803291523baaffd0625a2688672b9436b61d4` |
| `p2-clippy-fresh.log` | `1ba6592a6779db9c33980e6acb847620c4db7167bbf569d6825c9106ed78df5d` |
| `architecture-final.json` | `8260d557d94178fbb1d8fd54c9f4b4ed7aa0ca3173e7ee524e2f148784764dd8` |
| `downstream/summary.json` | `889e685492dff511787738fc22d4f6d920f4e8df9106579e05d43fd4feaed134` |
| `downstream-baseline/summary.json` | `ff56ac305d58c11ffb315102c034eabd9c857a1a7fbc40b6f3ed33adaaac25d1` |
| `api/summary.json` | `87160abfdd7ad1f39d0cc29224fd44b7bfaca0508c5b676fe427a3e3ecf12ee2` |

See [the downstream report](ddd-p2-downstream.md) for exact revisions and diagnostics.
