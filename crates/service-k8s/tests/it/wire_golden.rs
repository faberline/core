//! W0 golden bytes: the serde wire shape of the types P2 moves or re-layers,
//! pinned as literal text and checked in both directions.
//!
//! Each case serializes a fixed value and compares the text byte for byte, then
//! parses the same text back and compares the value. The schema cases pin the
//! `schemars` JSON that downstream CRDs embed (lumen's and tape's
//! `Vec<service_k8s::Condition>`, the capacity and lifecycle policies), so a
//! move of these types between layers cannot change a generated CRD.

use std::fmt::Debug;

use schemars::schema::RootSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;

use service_k8s::lifecycle::{LifecyclePolicy, ProbeTiming};
use service_k8s::service::{ClusterSpec, Condition, ResourceSpec};
use service_k8s::{ReplicaLayerPolicy, ShardSplitPolicy};

/// Encode gives `text`, and decode of `text` gives `value`.
fn both_ways<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T, text: &str) {
    assert_eq!(serde_json::to_string(value).unwrap(), text, "encode");
    assert_eq!(&serde_json::from_str::<T>(text).unwrap(), value, "decode");
}

/// The pretty-printed schema is `text`, and `text` decodes to the same schema.
fn schema_both_ways(schema: RootSchema, text: &str) {
    assert_eq!(
        serde_json::to_string_pretty(&schema).unwrap(),
        text,
        "encode"
    );
    assert_eq!(
        serde_json::from_str::<RootSchema>(text).unwrap(),
        schema,
        "decode"
    );
}

fn ready() -> Condition {
    Condition {
        type_: "Ready".to_string(),
        status: "True".to_string(),
        reason: "AllReplicasReady".to_string(),
        message: "3/3 replicas ready".to_string(),
        last_transition_time: "2026-01-01T00:00:00Z".to_string(),
        observed_generation: Some(7),
    }
}

#[test]
fn a_condition_keeps_its_wire_bytes() {
    both_ways(
        &ready(),
        r##"{"type":"Ready","status":"True","reason":"AllReplicasReady","message":"3/3 replicas ready","lastTransitionTime":"2026-01-01T00:00:00Z","observedGeneration":7}"##,
    );
}

#[test]
fn a_condition_without_a_generation_omits_the_field() {
    let condition = Condition {
        message: String::new(),
        observed_generation: None,
        ..ready()
    };
    both_ways(
        &condition,
        r##"{"type":"Ready","status":"True","reason":"AllReplicasReady","message":"","lastTransitionTime":"2026-01-01T00:00:00Z"}"##,
    );
}

#[test]
fn a_cluster_spec_keeps_its_wire_bytes() {
    let spec = ClusterSpec {
        image: "ghcr.io/axiom/lumen:1.2.3".to_string(),
        image_pull_policy: Some("IfNotPresent".to_string()),
        shard_count: 4,
        replicas_per_shard: 3,
        voter_count: 3,
        resources: ResourceSpec {
            cpu: "2".to_string(),
            memory: "8Gi".to_string(),
        },
    };
    let text = r##"{"image":"ghcr.io/axiom/lumen:1.2.3","imagePullPolicy":"IfNotPresent","shardCount":4,"replicasPerShard":3,"voterCount":3,"resources":{"cpu":"2","memory":"8Gi"}}"##;
    assert_eq!(serde_json::to_string(&spec).unwrap(), text, "encode");
    let decoded: ClusterSpec = serde_json::from_str(text).unwrap();
    assert_eq!(decoded.image, spec.image);
    assert_eq!(decoded.image_pull_policy, spec.image_pull_policy);
    assert_eq!(decoded.shard_count, spec.shard_count);
    assert_eq!(decoded.replicas_per_shard, spec.replicas_per_shard);
    assert_eq!(decoded.voter_count, spec.voter_count);
    assert_eq!(decoded.resources.cpu, spec.resources.cpu);
    assert_eq!(decoded.resources.memory, spec.resources.memory);
}

#[test]
fn a_minimal_cluster_spec_fills_its_defaults() {
    let decoded: ClusterSpec = serde_json::from_str(r##"{"image":"lumen:1"}"##).unwrap();
    assert_eq!(decoded.image, "lumen:1");
    assert_eq!(decoded.image_pull_policy, None);
    assert_eq!(
        (
            decoded.shard_count,
            decoded.replicas_per_shard,
            decoded.voter_count
        ),
        (1, 1, 1)
    );
    assert_eq!(
        serde_json::to_string(&decoded).unwrap(),
        r##"{"image":"lumen:1","shardCount":1,"replicasPerShard":1,"voterCount":1,"resources":{"cpu":"","memory":""}}"##
    );
}

#[test]
fn the_capacity_policies_keep_their_wire_bytes() {
    both_ways(
        &ReplicaLayerPolicy::default(),
        r##"{"minReplicasPerShard":1,"maxReplicasPerShard":1,"targetCpuUtilization":70,"targetMemoryUtilization":80}"##,
    );
    both_ways(
        &ShardSplitPolicy {
            split_threshold_bytes: 1 << 30,
            max_shards: Some(8),
        },
        r##"{"splitThresholdBytes":1073741824,"maxShards":8}"##,
    );
    both_ways(
        &ShardSplitPolicy {
            split_threshold_bytes: 1 << 30,
            max_shards: None,
        },
        r##"{"splitThresholdBytes":1073741824}"##,
    );
}

#[test]
fn the_lifecycle_policies_keep_their_wire_bytes() {
    both_ways(
        &ProbeTiming::default(),
        r##"{"periodSeconds":10,"timeoutSeconds":1,"failureThreshold":3,"successThreshold":1}"##,
    );
    both_ways(
        &LifecyclePolicy {
            prestop_cost_seconds: Some(3),
            ..LifecyclePolicy::default()
        },
        r##"{"totalGracePeriodSeconds":30,"runtimeDeadlineSeconds":25,"sigkillReserveSeconds":5,"minHookDurationSeconds":1,"prestopCostSeconds":3,"probeTiming":{"periodSeconds":10,"timeoutSeconds":1,"failureThreshold":3,"successThreshold":1}}"##,
    );
}

#[test]
fn the_replica_layer_policy_schema_is_pinned() {
    schema_both_ways(
        schemars::schema_for!(ReplicaLayerPolicy),
        r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "ReplicaLayerPolicy",
  "description": "Autoscaling policy for a whole replica layer.\n\n`min_replicas_per_shard` is the starting/floor value represented by an app's `replicasPerShard`. The maximum and utilization targets are policy; observation and stabilization belong to the operator control loop.",
  "type": "object",
  "required": [
    "maxReplicasPerShard",
    "minReplicasPerShard",
    "targetCpuUtilization",
    "targetMemoryUtilization"
  ],
  "properties": {
    "maxReplicasPerShard": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0.0
    },
    "minReplicasPerShard": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0.0
    },
    "targetCpuUtilization": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0.0
    },
    "targetMemoryUtilization": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0.0
    }
  }
}"##,
    );
}

#[test]
fn the_shard_split_policy_schema_is_pinned() {
    schema_both_ways(
        schemars::schema_for!(ShardSplitPolicy),
        r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "ShardSplitPolicy",
  "description": "Storage-pressure policy for adding physical shards.\n\nThe default is intentionally small enough for low-cost integration proof: a busiest shard must exceed one GiB. Production deployments may raise the threshold while preserving the same one-shard-at-a-time transition.",
  "type": "object",
  "required": [
    "splitThresholdBytes"
  ],
  "properties": {
    "maxShards": {
      "type": [
        "integer",
        "null"
      ],
      "format": "uint32",
      "minimum": 0.0
    },
    "splitThresholdBytes": {
      "type": "integer",
      "format": "uint64",
      "minimum": 0.0
    }
  }
}"##,
    );
}

#[test]
fn the_condition_schema_is_pinned() {
    schema_both_ways(
        schemars::schema_for!(Condition),
        r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Condition",
  "description": "One entry of a Kubernetes `status.conditions[]` array, in the shape every controller-aware tool already reads — `kubectl wait --for=condition=…`, Argo CD health assessment, Flux readiness gates.\n\nHand-written rather than reused from `k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition` because that type does not derive `JsonSchema`, so it cannot be embedded in a CRD schema generated by `kube`'s derive.",
  "type": "object",
  "required": [
    "lastTransitionTime",
    "reason",
    "status",
    "type"
  ],
  "properties": {
    "lastTransitionTime": {
      "description": "RFC3339 instant the status last *changed* — not the last time it was observed. Carried across reconciles by [`project`].",
      "type": "string"
    },
    "message": {
      "description": "Human-readable detail. May be empty.",
      "default": "",
      "type": "string"
    },
    "observedGeneration": {
      "description": "The `.metadata.generation` this condition was computed from.",
      "type": [
        "integer",
        "null"
      ],
      "format": "int64"
    },
    "reason": {
      "description": "CamelCase machine-readable cause of the current status.",
      "type": "string"
    },
    "status": {
      "description": "`\"True\" | \"False\" | \"Unknown\"`.",
      "type": "string"
    },
    "type": {
      "description": "CamelCase condition name, e.g. `Ready`.",
      "type": "string"
    }
  }
}"##,
    );
}

#[test]
fn the_probe_timing_schema_is_pinned() {
    schema_both_ways(
        schemars::schema_for!(ProbeTiming),
        r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "ProbeTiming",
  "description": "Configured timing for Kubernetes container probes.",
  "type": "object",
  "required": [
    "failureThreshold",
    "periodSeconds",
    "successThreshold",
    "timeoutSeconds"
  ],
  "properties": {
    "failureThreshold": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0.0
    },
    "periodSeconds": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0.0
    },
    "successThreshold": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0.0
    },
    "timeoutSeconds": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0.0
    }
  }
}"##,
    );
}

#[test]
fn the_lifecycle_policy_schema_is_pinned() {
    schema_both_ways(
        schemars::schema_for!(LifecyclePolicy),
        r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "LifecyclePolicy",
  "description": "Unvalidated input configuration for a workload's termination lifecycle policy.",
  "type": "object",
  "required": [
    "minHookDurationSeconds",
    "runtimeDeadlineSeconds",
    "sigkillReserveSeconds",
    "totalGracePeriodSeconds"
  ],
  "properties": {
    "minHookDurationSeconds": {
      "type": "integer",
      "format": "uint64",
      "minimum": 0.0
    },
    "prestopCostSeconds": {
      "type": [
        "integer",
        "null"
      ],
      "format": "uint64",
      "minimum": 0.0
    },
    "probeTiming": {
      "default": {
        "periodSeconds": 10,
        "timeoutSeconds": 1,
        "failureThreshold": 3,
        "successThreshold": 1
      },
      "allOf": [
        {
          "$ref": "#/definitions/ProbeTiming"
        }
      ]
    },
    "runtimeDeadlineSeconds": {
      "type": "integer",
      "format": "uint64",
      "minimum": 0.0
    },
    "sigkillReserveSeconds": {
      "type": "integer",
      "format": "uint64",
      "minimum": 0.0
    },
    "totalGracePeriodSeconds": {
      "type": "integer",
      "format": "uint64",
      "minimum": 0.0
    }
  },
  "definitions": {
    "ProbeTiming": {
      "description": "Configured timing for Kubernetes container probes.",
      "type": "object",
      "required": [
        "failureThreshold",
        "periodSeconds",
        "successThreshold",
        "timeoutSeconds"
      ],
      "properties": {
        "failureThreshold": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0.0
        },
        "periodSeconds": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0.0
        },
        "successThreshold": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0.0
        },
        "timeoutSeconds": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0.0
        }
      }
    }
  }
}"##,
    );
}

#[cfg(feature = "certificate")]
mod certificate_secrets {
    use chrono::TimeZone;
    use serde_json::Value;

    use service_k8s::certificate::projection::{material_secret, trust_bundle_secret};
    use service_k8s::certificate::{
        InstanceScope, IssuedMaterial, IssuerId, Owner, Purpose, TrustBundle,
    };

    fn scope() -> InstanceScope {
        InstanceScope::new("lumen", "lumen", "lumen-prod.svc.id.goog")
    }

    fn owner() -> Owner {
        Owner {
            api_version: "lumen.dev/v1".into(),
            kind: "Lumen".into(),
            name: "lumen".into(),
            uid: "0f7d1f4e-0000-4000-8000-000000000000".into(),
        }
    }

    fn bundle() -> TrustBundle {
        let mut bundle = TrustBundle::new();
        bundle.insert(
            IssuerId::new("pool-a"),
            "-----BEGIN CERTIFICATE-----\nQUFB\n-----END CERTIFICATE-----",
        );
        bundle.insert(
            IssuerId::new("pool-b"),
            "-----BEGIN CERTIFICATE-----\nQkJC\n-----END CERTIFICATE-----",
        );
        bundle
    }

    /// Encode gives `text`, and `text` decodes to the same object.
    fn both_ways(secret: &Value, text: &str) {
        assert_eq!(
            serde_json::to_string_pretty(secret).unwrap(),
            text,
            "encode"
        );
        assert_eq!(
            &serde_json::from_str::<Value>(text).unwrap(),
            secret,
            "decode"
        );
    }

    #[test]
    fn a_material_secret_keeps_its_bytes() {
        let material = IssuedMaterial {
            issuer: IssuerId::new("pool-a"),
            certificate_pem: "-----BEGIN CERTIFICATE-----\nTEVBRg==\n-----END CERTIFICATE-----\n"
                .to_string(),
            chain_pem: "-----BEGIN CERTIFICATE-----\nQUFB\n-----END CERTIFICATE-----\n".to_string(),
            not_before: chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            not_after: chrono::Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap(),
            fingerprint: "00ff".to_string(),
        };
        let secret = material_secret(
            &scope(),
            Purpose::Peer,
            &owner(),
            &material,
            "-----BEGIN PRIVATE KEY-----\nS0VZ\n-----END PRIVATE KEY-----\n",
            &bundle(),
            "digest-1",
        );
        both_ways(
            &secret,
            r##"{
  "apiVersion": "v1",
  "kind": "Secret",
  "type": "Opaque",
  "metadata": {
    "name": "lumen-peer-tls",
    "namespace": "lumen",
    "labels": {
      "app.kubernetes.io/component": "peer-tls",
      "app.kubernetes.io/managed-by": "service-k8s",
      "app.kubernetes.io/name": "lumen"
    },
    "ownerReferences": [
      {
        "apiVersion": "lumen.dev/v1",
        "kind": "Lumen",
        "name": "lumen",
        "uid": "0f7d1f4e-0000-4000-8000-000000000000",
        "controller": true,
        "blockOwnerDeletion": true
      }
    ],
    "annotations": {
      "service-k8s.axiom.dev/trust-bundle": "pool-a,pool-b",
      "service-k8s.axiom.dev/leaf-issuer": "pool-a",
      "service-k8s.axiom.dev/identity-digest": "digest-1"
    }
  },
  "stringData": {
    "tls.crt": "-----BEGIN CERTIFICATE-----\nTEVBRg==\n-----END CERTIFICATE-----\n",
    "tls.key": "-----BEGIN PRIVATE KEY-----\nS0VZ\n-----END PRIVATE KEY-----\n",
    "ca.crt": "-----BEGIN CERTIFICATE-----\nQUFB\n-----END CERTIFICATE-----\n-----BEGIN CERTIFICATE-----\nQkJC\n-----END CERTIFICATE-----\n"
  }
}"##,
        );
    }

    #[test]
    fn a_trust_bundle_secret_keeps_its_bytes() {
        let secret = trust_bundle_secret(&scope(), Purpose::Serving, &owner(), &bundle());
        both_ways(
            &secret,
            r##"{
  "apiVersion": "v1",
  "kind": "Secret",
  "type": "Opaque",
  "metadata": {
    "name": "lumen-serving-tls",
    "namespace": "lumen",
    "labels": {
      "app.kubernetes.io/component": "serving-tls",
      "app.kubernetes.io/managed-by": "service-k8s",
      "app.kubernetes.io/name": "lumen"
    },
    "ownerReferences": [
      {
        "apiVersion": "lumen.dev/v1",
        "kind": "Lumen",
        "name": "lumen",
        "uid": "0f7d1f4e-0000-4000-8000-000000000000",
        "controller": true,
        "blockOwnerDeletion": true
      }
    ],
    "annotations": {
      "service-k8s.axiom.dev/trust-bundle": "pool-a,pool-b"
    }
  },
  "stringData": {
    "ca.crt": "-----BEGIN CERTIFICATE-----\nQUFB\n-----END CERTIFICATE-----\n-----BEGIN CERTIFICATE-----\nQkJC\n-----END CERTIFICATE-----\n"
  }
}"##,
        );
    }
}
