//! Kubernetes structural-schema normalization shared by service CRDs.

use serde_json::{json, Value};

/// Rewrite schemars' unsigned integer formats into Kubernetes-compatible
/// integer schemas while preserving their non-negative contract.
///
/// Kubernetes structural OpenAPI does not recognize `uint32` or `uint64`.
/// Schemars emits them for Rust unsigned types, so every service CRD must
/// remove the format and retain unsigned semantics with `minimum: 0`.
pub fn normalize_unsigned_integer_formats(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if matches!(
                map.get("format").and_then(|value| value.as_str()),
                Some("uint32" | "uint64")
            ) {
                map.remove("format");
                map.entry("minimum").or_insert_with(|| json!(0));
            }
            for child in map.values_mut() {
                normalize_unsigned_integer_formats(child);
            }
        }
        Value::Array(items) => {
            for child in items {
                normalize_unsigned_integer_formats(child);
            }
        }
        _ => {}
    }
}

/// Attach a CEL validation rule to every version's `spec` schema.
///
/// Cross-field invariants ("exactly one of these two fields") cannot be
/// expressed in structural OpenAPI, and enforcing them only in the reconcile
/// loop means `kubectl apply` reports success on a resource the operator will
/// refuse — the operator's complaint then lives in its log, which is not where
/// the person who ran `apply` is looking. `x-kubernetes-validations` (CEL,
/// GA since Kubernetes 1.29) moves the rejection to admission, where the
/// author sees it.
///
/// `rule` is evaluated with `self` bound to `spec`. Test presence with
/// `has(self.x)` and nothing else — including for `nullable: true` fields.
/// Kubernetes prunes an explicitly-null field before CEL runs, so `has()`
/// already reports it absent, and a defensive `self.x != null` does not merely
/// duplicate that: it FAILS TO COMPILE. Kubernetes types a `nullable: true`
/// string as plain `string`, so the rule is rejected with "found no matching
/// overload for '_!=_' applied to '(string, null)'" — and rejected by the API
/// server at `kubectl apply`, not by any test here, because the unit tests
/// assert on YAML structure and never compile the expression. That is a CRD
/// which passes every local gate and cannot be installed on any cluster
/// (verified against a live API server: with a `has()`-only rule,
/// `{a: "x", b: null}` is accepted and `{a: "x", b: "y"}` is rejected).
///
/// Returns the number of versions the rule was attached to; a caller that
/// generated the CRD from a derive macro should assert this is non-zero,
/// since zero means the CRD's shape changed under it.
pub fn add_spec_validation_rule(crd: &mut Value, rule: &str, message: &str) -> usize {
    let Some(versions) = crd
        .get_mut("spec")
        .and_then(|spec| spec.get_mut("versions"))
        .and_then(Value::as_array_mut)
    else {
        return 0;
    };
    let mut attached = 0;
    for version in versions {
        let Some(spec_schema) = version
            .get_mut("schema")
            .and_then(|schema| schema.get_mut("openAPIV3Schema"))
            .and_then(|schema| schema.get_mut("properties"))
            .and_then(|properties| properties.get_mut("spec"))
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        let rules = spec_schema
            .entry("x-kubernetes-validations")
            .or_insert_with(|| json!([]));
        let Some(rules) = rules.as_array_mut() else {
            continue;
        };
        rules.push(json!({ "rule": rule, "message": message }));
        attached += 1;
    }
    attached
}

/// The CRD `subresources.scale` block for a service that scales read capacity with learners.
/// `None` when `learners` is false: such a service has a fixed replica layout and must not expose `scale`.
pub fn scale_subresource(learners: bool) -> Option<Value> {
    learners.then(|| {
        json!({
            "specReplicasPath": ".spec.replicasPerShard",
            "statusReplicasPath": ".status.replicasPerShard",
            "labelSelectorPath": ".status.selector",
        })
    })
}

/// Add `subresources.scale` to every version of `crd` when `learners` is true.
/// Existing keys under `subresources` (for example `status`) stay.
/// Returns the number of versions changed; 0 when `learners` is false or `crd` has no `spec.versions` array.
pub fn add_scale_subresource(crd: &mut Value, learners: bool) -> usize {
    let Some(scale) = scale_subresource(learners) else {
        return 0;
    };
    let Some(versions) = crd
        .get_mut("spec")
        .and_then(|spec| spec.get_mut("versions"))
        .and_then(Value::as_array_mut)
    else {
        return 0;
    };
    let mut changed = 0;
    for version in versions {
        let Some(version) = version.as_object_mut() else {
            continue;
        };
        let subresources = version.entry("subresources").or_insert_with(|| json!({}));
        let Some(subresources) = subresources.as_object_mut() else {
            continue;
        };
        subresources.insert("scale".to_string(), scale.clone());
        changed += 1;
    }
    changed
}

/// Quote plain scalars that Kubernetes' YAML 1.1 decoder would coerce to a
/// boolean even though the CRD JSON schema declares them as strings.
///
/// `serde_yaml` follows YAML 1.2 and therefore emits strings such as `off`
/// without quotes. Kubernetes still accepts YAML 1.1 input, where `off`,
/// `on`, `yes`, and `no` (including their single-letter forms) are booleans.
/// The API server then rejects a string schema default such as `default: off`
/// because it arrives as `false`. JSON booleans serialize as `true`/`false`,
/// so quoting these legacy spellings cannot change a real boolean value.
pub fn quote_yaml_1_1_boolean_like_strings(yaml: &str) -> String {
    let trailing_newline = yaml.ends_with('\n');
    let mut normalized = yaml
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            let scalar = if let Some((_, value)) = trimmed.split_once(": ") {
                Some(value)
            } else {
                trimmed.strip_prefix("- ")
            };

            match scalar {
                Some(value)
                    if matches!(
                        value.to_ascii_lowercase().as_str(),
                        "y" | "yes" | "n" | "no" | "on" | "off"
                    ) =>
                {
                    let prefix_len = line.len() - value.len();
                    format!("{}\"{value}\"", &line[..prefix_len])
                }
                _ => line.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if trailing_newline {
        normalized.push('\n');
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_unsigned_formats_recursively_and_keeps_nonnegative_semantics() {
        let mut schema = json!({
            "properties": {
                "replicas": { "type": "integer", "format": "uint32" },
                "retention": { "type": "integer", "format": "uint64", "minimum": 1 },
            },
            "items": [{ "type": "integer", "format": "uint32" }],
        });

        normalize_unsigned_integer_formats(&mut schema);

        assert_eq!(schema["properties"]["replicas"]["type"], "integer");
        assert!(schema["properties"]["replicas"].get("format").is_none());
        assert_eq!(schema["properties"]["replicas"]["minimum"], 0);
        assert_eq!(schema["properties"]["retention"]["minimum"], 1);
        assert!(schema["items"][0].get("format").is_none());
        assert_eq!(schema["items"][0]["minimum"], 0);
    }

    fn crd_with_versions(count: usize) -> Value {
        json!({
            "spec": {
                "versions": (0..count)
                    .map(|_| json!({
                        "schema": { "openAPIV3Schema": { "properties": { "spec": {
                            "type": "object",
                            "properties": { "a": { "type": "string" } },
                        } } } }
                    }))
                    .collect::<Vec<_>>(),
            }
        })
    }

    #[test]
    fn spec_validation_rules_attach_to_every_version_and_accumulate() {
        let mut crd = crd_with_versions(2);

        assert_eq!(add_spec_validation_rule(&mut crd, "self.a != ''", "a"), 2);
        assert_eq!(add_spec_validation_rule(&mut crd, "self.a != 'b'", "b"), 2);

        for version in crd["spec"]["versions"].as_array().unwrap() {
            let rules = version["schema"]["openAPIV3Schema"]["properties"]["spec"]
                ["x-kubernetes-validations"]
                .as_array()
                .unwrap();
            assert_eq!(rules.len(), 2, "a second rule must not replace the first");
            assert_eq!(rules[0]["rule"], "self.a != ''");
            assert_eq!(rules[0]["message"], "a");
        }
    }

    #[test]
    fn a_crd_without_a_spec_schema_attaches_nothing_rather_than_panicking() {
        let mut crd = json!({ "spec": { "versions": [{ "name": "v1" }] } });
        assert_eq!(add_spec_validation_rule(&mut crd, "true", "m"), 0);
        assert_eq!(add_spec_validation_rule(&mut json!({}), "true", "m"), 0);
    }

    #[test]
    fn quotes_yaml_1_1_boolean_like_strings_without_touching_real_booleans() {
        let yaml = "properties:\n  auth:\n    default: off\n    enum:\n    - off\n    - required\n    description: mode defaults to: off\n  enabled:\n    default: false\n";

        let normalized = quote_yaml_1_1_boolean_like_strings(yaml);

        assert!(normalized.contains("default: \"off\""));
        assert!(normalized.contains("- \"off\""));
        assert!(normalized.contains("description: mode defaults to: off"));
        assert!(normalized.contains("default: false"));
        assert!(normalized.ends_with('\n'));
    }

    #[test]
    fn scale_subresource_has_the_three_paths_for_learners() {
        let scale = scale_subresource(true).expect("learners services expose scale");

        assert_eq!(scale["specReplicasPath"], ".spec.replicasPerShard");
        assert_eq!(scale["statusReplicasPath"], ".status.replicasPerShard");
        assert_eq!(scale["labelSelectorPath"], ".status.selector");
    }

    #[test]
    fn scale_subresource_is_none_without_learners() {
        assert!(scale_subresource(false).is_none());
    }

    #[test]
    fn scale_is_added_to_every_version_and_keeps_status() {
        let mut crd = crd_with_versions(2);
        crd["spec"]["versions"][0]["subresources"] = json!({ "status": {} });

        assert_eq!(add_scale_subresource(&mut crd, true), 2);

        for version in crd["spec"]["versions"].as_array().unwrap() {
            assert_eq!(
                version["subresources"]["scale"]["specReplicasPath"],
                ".spec.replicasPerShard"
            );
        }
        assert!(crd["spec"]["versions"][0]["subresources"]["status"].is_object());
    }

    #[test]
    fn no_scale_key_without_learners() {
        let mut crd = crd_with_versions(2);

        assert_eq!(add_scale_subresource(&mut crd, false), 0);

        for version in crd["spec"]["versions"].as_array().unwrap() {
            assert!(version["subresources"]["scale"].is_null());
            assert!(version.get("subresources").is_none());
        }
    }

    #[test]
    fn a_crd_without_versions_gets_no_scale_and_does_not_panic() {
        assert_eq!(add_scale_subresource(&mut json!({}), true), 0);
    }
}
