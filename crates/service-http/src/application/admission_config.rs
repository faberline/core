use std::error::Error;
use std::fmt;
use std::time::Duration;

use super::admission::{AdmissionController, AdmissionPolicy};

/// Validated, opt-in admission settings shared by service adopters.
///
/// The parser owns the common `<SVC>_ADMISSION_*` environment grammar while
/// callers retain their public route-class names when building a controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionConfig {
    read_capacity: Option<u32>,
    write_capacity: Option<u32>,
    admin_capacity: Option<u32>,
    refill_window: Duration,
    max_keys: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionConfigError {
    InvalidValue { key: String, value: String },
    OrphanedCommonSetting { key: String },
    InvalidPolicy { key: String, message: String },
}

impl fmt::Display for AdmissionConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue { key, value } => {
                write!(f, "{key} must be a positive integer, got {value:?}")
            }
            Self::OrphanedCommonSetting { key } => {
                write!(f, "{key} requires at least one admission class capacity")
            }
            Self::InvalidPolicy { key, message } => write!(f, "{key}: {message}"),
        }
    }
}

impl Error for AdmissionConfigError {}

impl AdmissionConfig {
    pub const DEFAULT_REFILL_SECS: u64 = 60;
    pub const DEFAULT_MAX_KEYS: usize = 1024;

    /// Parse configuration from process environment.
    pub fn from_env(prefix: &str) -> Result<Self, AdmissionConfigError> {
        Self::from_lookup(prefix, |key| std::env::var(key).ok())
    }

    /// Parse configuration from an injected lookup seam. This keeps tests and
    /// embedding callers independent of process-global environment mutation.
    pub fn from_lookup<F>(prefix: &str, lookup: F) -> Result<Self, AdmissionConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let key = |suffix: &str| format!("{prefix}_ADMISSION_{suffix}");
        let read_key = key("READ_CAPACITY");
        let write_key = key("WRITE_CAPACITY");
        let admin_key = key("ADMIN_CAPACITY");
        let refill_key = key("REFILL_SECS");
        let max_keys_key = key("MAX_KEYS");
        let parse = |key: &str| match lookup(key) {
            None => Ok(None),
            Some(value) => value
                .parse::<u32>()
                .ok()
                .filter(|value| *value > 0)
                .map(Some)
                .ok_or_else(|| AdmissionConfigError::InvalidValue {
                    key: key.to_owned(),
                    value,
                }),
        };
        let read_capacity = parse(&read_key)?;
        let write_capacity = parse(&write_key)?;
        let admin_capacity = parse(&admin_key)?;
        let enabled =
            read_capacity.is_some() || write_capacity.is_some() || admin_capacity.is_some();
        let common_present = lookup(&refill_key).is_some() || lookup(&max_keys_key).is_some();
        if !enabled && common_present {
            return Err(AdmissionConfigError::OrphanedCommonSetting {
                key: if lookup(&refill_key).is_some() {
                    refill_key
                } else {
                    max_keys_key
                },
            });
        }
        let refill_secs = parse(&key("REFILL_SECS"))?
            .map(u64::from)
            .unwrap_or(Self::DEFAULT_REFILL_SECS);
        let max_keys = parse(&key("MAX_KEYS"))?
            .map(|value| value as usize)
            .unwrap_or(Self::DEFAULT_MAX_KEYS);
        let refill_window = Duration::from_secs(refill_secs);
        for (class_key, capacity) in [
            (&read_key, read_capacity),
            (&write_key, write_capacity),
            (&admin_key, admin_capacity),
        ] {
            if let Some(capacity) = capacity {
                AdmissionPolicy::new(capacity, refill_window, max_keys).map_err(|error| {
                    AdmissionConfigError::InvalidPolicy {
                        key: class_key.clone(),
                        message: error.to_string(),
                    }
                })?;
            }
        }
        Ok(Self {
            read_capacity,
            write_capacity,
            admin_capacity,
            refill_window,
            max_keys,
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.read_capacity.is_some()
            || self.write_capacity.is_some()
            || self.admin_capacity.is_some()
    }

    /// Build a controller using the consuming service's route-class names.
    pub fn controller(
        &self,
        read_class: impl Into<String>,
        write_class: impl Into<String>,
        admin_class: impl Into<String>,
    ) -> Option<AdmissionController> {
        let classes = [
            (read_class.into(), self.read_capacity),
            (write_class.into(), self.write_capacity),
            (admin_class.into(), self.admin_capacity),
        ];
        let policies = classes.into_iter().filter_map(|(class, capacity)| {
            capacity.map(|capacity| {
                let policy = AdmissionPolicy::new(capacity, self.refill_window, self.max_keys)
                    .expect("AdmissionConfig validates policies at construction");
                (class, policy)
            })
        });
        self.is_enabled()
            .then(|| AdmissionController::new(policies))
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use crate::application::{AdmissionInput, AdmissionOutcome};
    use std::collections::BTreeMap;

    fn config(entries: &[(&str, &str)]) -> Result<AdmissionConfig, AdmissionConfigError> {
        let values = entries
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect::<BTreeMap<_, _>>();
        AdmissionConfig::from_lookup("TAPE", |key| values.get(key).cloned())
    }

    #[test]
    fn config_without_capacities_is_disabled() {
        let config = config(&[]).unwrap();
        assert!(!config.is_enabled());
        assert!(config
            .controller("tape.read", "tape.write", "tape.admin")
            .is_none());
    }

    #[test]
    fn config_builds_multi_class_controller() {
        let config = config(&[
            ("TAPE_ADMISSION_READ_CAPACITY", "2"),
            ("TAPE_ADMISSION_WRITE_CAPACITY", "1"),
            ("TAPE_ADMISSION_ADMIN_CAPACITY", "3"),
            ("TAPE_ADMISSION_REFILL_SECS", "10"),
            ("TAPE_ADMISSION_MAX_KEYS", "4"),
        ])
        .unwrap();
        let controller = config
            .controller("tape.read", "tape.write", "tape.admin")
            .unwrap();
        let input = AdmissionInput::new("tape.read", b"principal");
        assert_eq!(
            controller.admit_at(&input, Duration::ZERO).outcome,
            AdmissionOutcome::Allow
        );
        assert_eq!(
            controller.admit_at(&input, Duration::ZERO).outcome,
            AdmissionOutcome::Allow
        );
        assert_eq!(
            controller.admit_at(&input, Duration::ZERO).outcome,
            AdmissionOutcome::Deny
        );
        assert_eq!(controller.tracked_keys("tape.read"), 1);
    }

    #[test]
    fn config_rejects_invalid_or_orphaned_common_values() {
        let error = config(&[("TAPE_ADMISSION_READ_CAPACITY", "many")]).unwrap_err();
        assert!(error.to_string().contains("TAPE_ADMISSION_READ_CAPACITY"));
        let error = config(&[("TAPE_ADMISSION_REFILL_SECS", "10")]).unwrap_err();
        assert!(error.to_string().contains("TAPE_ADMISSION_REFILL_SECS"));
    }
}
