//! Pure retention planning for backup sets in storage sinks.

use std::time::{Duration, SystemTime};

/// Completion state of a backup set in inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetState {
    /// The set's manifest object exists.
    Complete { created_at: SystemTime },
    /// Objects exist under the set prefix, but no manifest exists.
    Partial,
}

/// Description of one backup set prefix discovered in a sink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetInventory {
    pub set_id: String,
    pub state: SetState,
    pub newest_object_at: SystemTime,
}

/// Decision on which backup sets to retain, prune, or delete as orphans.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RetentionPlan {
    pub keep: Vec<String>,
    pub delete_complete: Vec<String>,
    pub delete_orphans: Vec<String>,
}

/// Plan retention across discovered sets deterministically.
///
/// Rules:
/// - The `keep_latest` newest `Complete` sets are kept, with `keep_latest` raised
///   to at least 1 so the plan never deletes the last complete set.
/// - Older `Complete` sets go to `delete_complete`.
/// - With zero `Complete` sets, nothing in `delete_complete` is produced.
/// - A `Partial` set goes to `delete_orphans` only when `now - newest_object_at >= orphan_grace`,
///   otherwise it is neither kept nor deleted (left out of all three lists).
/// - An empty inventory produces an empty plan.
/// - Ties on `created_at` break by `set_id`.
/// - All output lists (`keep`, `delete_complete`, `delete_orphans`) are sorted by `set_id`.
pub fn plan_retention(
    inventory: &[SetInventory],
    keep_latest: usize,
    now: SystemTime,
    orphan_grace: Duration,
) -> RetentionPlan {
    let mut complete_sets: Vec<(&str, SystemTime)> = Vec::new();
    let mut delete_orphans: Vec<String> = Vec::new();

    for item in inventory {
        match &item.state {
            SetState::Complete { created_at } => {
                complete_sets.push((&item.set_id, *created_at));
            }
            SetState::Partial => {
                if let Ok(age) = now.duration_since(item.newest_object_at) {
                    if age >= orphan_grace {
                        delete_orphans.push(item.set_id.clone());
                    }
                }
            }
        }
    }

    // Sort ascending by (created_at, set_id); newest sets are at the end.
    complete_sets.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(b.0)));

    let effective_keep = keep_latest.max(1);
    let mut keep = Vec::new();
    let mut delete_complete = Vec::new();

    if !complete_sets.is_empty() {
        let keep_count = effective_keep.min(complete_sets.len());
        let split_idx = complete_sets.len() - keep_count;
        for (id, _) in &complete_sets[split_idx..] {
            keep.push((*id).to_string());
        }
        for (id, _) in &complete_sets[..split_idx] {
            delete_complete.push((*id).to_string());
        }
    }

    // Never delete a partial set that has the same id as a complete set.
    delete_orphans.retain(|orphan| !keep.contains(orphan) && !delete_complete.contains(orphan));

    keep.sort();
    delete_complete.sort();
    delete_orphans.sort();
    delete_orphans.dedup();

    RetentionPlan {
        keep,
        delete_complete,
        delete_orphans,
    }
}
