//! Golden pins for the `schemars` JSON schema of every backup CRD wire type,
//! taken before the DDD P2 changes. Downstream CRDs (lumen, tape, relay, defer,
//! keep) embed these schemas, so the text must not change.

use service_backup::{BackupDestination, BackupPolicy, RetentionPolicy, ScheduledBackupPolicy};

fn schema_text<T: schemars::JsonSchema>() -> String {
    serde_json::to_string_pretty(&schemars::schema_for!(T)).unwrap()
}

const BACKUP_DESTINATION_SCHEMA: &str = r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "BackupDestination",
  "description": "Backup destination declared by a service CR or runner config.",
  "oneOf": [
    {
      "description": "Local filesystem path, primarily for dev/tests and PVC-backed local runs.",
      "type": "object",
      "required": [
        "path",
        "type"
      ],
      "properties": {
        "path": {
          "type": "string"
        },
        "prefix": {
          "type": [
            "string",
            "null"
          ]
        },
        "type": {
          "type": "string",
          "enum": [
            "local"
          ]
        }
      }
    },
    {
      "description": "Amazon S3-compatible object store. Upload implementation is a crate feature; the schema is stable regardless of whether that feature is linked into the runner.",
      "type": "object",
      "required": [
        "bucket",
        "type"
      ],
      "properties": {
        "bucket": {
          "type": "string"
        },
        "credentials_secret": {
          "type": [
            "string",
            "null"
          ]
        },
        "endpoint": {
          "type": [
            "string",
            "null"
          ]
        },
        "prefix": {
          "default": "",
          "type": "string"
        },
        "region": {
          "type": [
            "string",
            "null"
          ]
        },
        "type": {
          "type": "string",
          "enum": [
            "s3"
          ]
        }
      }
    },
    {
      "description": "Google Cloud Storage. The adapter uses workload identity by default and Vat's `STORAGE_EMULATOR_HOST` for integration tests.",
      "type": "object",
      "required": [
        "bucket",
        "type"
      ],
      "properties": {
        "bucket": {
          "type": "string"
        },
        "credentials_secret": {
          "type": [
            "string",
            "null"
          ]
        },
        "prefix": {
          "default": "",
          "type": "string"
        },
        "type": {
          "type": "string",
          "enum": [
            "gcs"
          ]
        }
      }
    }
  ]
}"##;

const SCHEDULED_BACKUP_POLICY_SCHEMA: &str = r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "ScheduledBackupPolicy",
  "description": "Kubernetes structural-schema-safe scheduled backup projection.\n\nThe runtime [`BackupPolicy`] uses a tagged [`BackupDestination`] enum whose variant schemas cannot be embedded directly in a CRD. This flat shape keeps the public `schedule`, `destination`, and `retentionSecs` fields shared by every service operator and validates them through one conversion path.",
  "type": "object",
  "required": [
    "destination",
    "schedule"
  ],
  "properties": {
    "destination": {
      "description": "Destination URI accepted by [`BackupDestination::from_uri`].",
      "type": "string"
    },
    "retentionSecs": {
      "description": "Drop objects older than this many seconds after a successful put. `None` keeps every object.",
      "type": [
        "integer",
        "null"
      ],
      "format": "uint64",
      "minimum": 0.0
    },
    "schedule": {
      "description": "Cron expression rendered into `CronJob.spec.schedule`.",
      "type": "string"
    }
  }
}"##;

const BACKUP_POLICY_SCHEMA: &str = r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "BackupPolicy",
  "description": "Operator/runner-facing backup policy.",
  "type": "object",
  "required": [
    "destination",
    "schedule"
  ],
  "properties": {
    "destination": {
      "$ref": "#/definitions/BackupDestination"
    },
    "retention": {
      "default": {},
      "allOf": [
        {
          "$ref": "#/definitions/RetentionPolicy"
        }
      ]
    },
    "schedule": {
      "description": "Cron expression for the runner. The operator owns translating this into a Kubernetes CronJob schedule.",
      "type": "string"
    }
  },
  "definitions": {
    "BackupDestination": {
      "description": "Backup destination declared by a service CR or runner config.",
      "oneOf": [
        {
          "description": "Local filesystem path, primarily for dev/tests and PVC-backed local runs.",
          "type": "object",
          "required": [
            "path",
            "type"
          ],
          "properties": {
            "path": {
              "type": "string"
            },
            "prefix": {
              "type": [
                "string",
                "null"
              ]
            },
            "type": {
              "type": "string",
              "enum": [
                "local"
              ]
            }
          }
        },
        {
          "description": "Amazon S3-compatible object store. Upload implementation is a crate feature; the schema is stable regardless of whether that feature is linked into the runner.",
          "type": "object",
          "required": [
            "bucket",
            "type"
          ],
          "properties": {
            "bucket": {
              "type": "string"
            },
            "credentials_secret": {
              "type": [
                "string",
                "null"
              ]
            },
            "endpoint": {
              "type": [
                "string",
                "null"
              ]
            },
            "prefix": {
              "default": "",
              "type": "string"
            },
            "region": {
              "type": [
                "string",
                "null"
              ]
            },
            "type": {
              "type": "string",
              "enum": [
                "s3"
              ]
            }
          }
        },
        {
          "description": "Google Cloud Storage. The adapter uses workload identity by default and Vat's `STORAGE_EMULATOR_HOST` for integration tests.",
          "type": "object",
          "required": [
            "bucket",
            "type"
          ],
          "properties": {
            "bucket": {
              "type": "string"
            },
            "credentials_secret": {
              "type": [
                "string",
                "null"
              ]
            },
            "prefix": {
              "default": "",
              "type": "string"
            },
            "type": {
              "type": "string",
              "enum": [
                "gcs"
              ]
            }
          }
        }
      ]
    },
    "RetentionPolicy": {
      "description": "Retention applied after a successful put.",
      "type": "object",
      "properties": {
        "maxAgeSeconds": {
          "description": "Drop objects older than this many seconds. `None` disables age pruning.",
          "type": [
            "integer",
            "null"
          ],
          "format": "uint64",
          "minimum": 0.0
        }
      }
    }
  }
}"##;

const RETENTION_POLICY_SCHEMA: &str = r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "RetentionPolicy",
  "description": "Retention applied after a successful put.",
  "type": "object",
  "properties": {
    "maxAgeSeconds": {
      "description": "Drop objects older than this many seconds. `None` disables age pruning.",
      "type": [
        "integer",
        "null"
      ],
      "format": "uint64",
      "minimum": 0.0
    }
  }
}"##;

#[test]
fn backup_destination_schema_is_pinned() {
    assert_eq!(
        schema_text::<BackupDestination>(),
        BACKUP_DESTINATION_SCHEMA
    );
}

#[test]
fn scheduled_backup_policy_schema_is_pinned() {
    assert_eq!(
        schema_text::<ScheduledBackupPolicy>(),
        SCHEDULED_BACKUP_POLICY_SCHEMA
    );
}

#[test]
fn backup_policy_schema_is_pinned() {
    assert_eq!(schema_text::<BackupPolicy>(), BACKUP_POLICY_SCHEMA);
}

#[test]
fn retention_policy_schema_is_pinned() {
    assert_eq!(schema_text::<RetentionPolicy>(), RETENTION_POLICY_SCHEMA);
}
