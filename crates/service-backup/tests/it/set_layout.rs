use std::collections::HashSet;
use std::time::{Duration, UNIX_EPOCH};

use service_backup::{plan_retention, SetInventory, SetKeyError, SetKeyLayout, SetState};

#[test]
fn key_shapes_with_and_without_prefix() {
    let layout_with = SetKeyLayout::new("backups/cluster1").unwrap();
    assert_eq!(layout_with.prefix(), "backups/cluster1");
    assert_eq!(
        layout_with.artifact_key("set-1", "data.tar").unwrap(),
        "backups/cluster1/sets/set-1/artifacts/data.tar"
    );
    assert_eq!(
        layout_with.manifest_key("set-1").unwrap(),
        "backups/cluster1/sets/set-1/manifest.json"
    );
    assert_eq!(layout_with.latest_key(), "backups/cluster1/latest.json");
    assert_eq!(
        layout_with.set_prefix("set-1").unwrap(),
        "backups/cluster1/sets/set-1/"
    );

    let layout_empty = SetKeyLayout::new("").unwrap();
    assert_eq!(layout_empty.prefix(), "");
    assert_eq!(
        layout_empty.artifact_key("set-1", "data.tar").unwrap(),
        "sets/set-1/artifacts/data.tar"
    );
    assert_eq!(
        layout_empty.manifest_key("set-1").unwrap(),
        "sets/set-1/manifest.json"
    );
    assert_eq!(layout_empty.latest_key(), "latest.json");
    assert_eq!(layout_empty.set_prefix("set-1").unwrap(), "sets/set-1/");
}

#[test]
fn invalid_prefix_set_id_and_name() {
    // Prefix rules
    assert!(matches!(
        SetKeyLayout::new("/backups"),
        Err(SetKeyError::LeadingSlashInPrefix(_))
    ));
    assert!(matches!(
        SetKeyLayout::new("backups/"),
        Err(SetKeyError::TrailingSlashInPrefix(_))
    ));
    assert!(matches!(
        SetKeyLayout::new("backups//prod"),
        Err(SetKeyError::EmptyPrefixSegment(_))
    ));
    assert!(matches!(
        SetKeyLayout::new("."),
        Err(SetKeyError::DotSegmentInPrefix(_))
    ));
    assert!(matches!(
        SetKeyLayout::new("backups/./prod"),
        Err(SetKeyError::DotSegmentInPrefix(_))
    ));
    assert!(matches!(
        SetKeyLayout::new(".."),
        Err(SetKeyError::DotSegmentInPrefix(_))
    ));
    assert!(matches!(
        SetKeyLayout::new("backups/../prod"),
        Err(SetKeyError::DotSegmentInPrefix(_))
    ));
    assert!(matches!(
        SetKeyLayout::new("backups/prod@1"),
        Err(SetKeyError::InvalidPrefixCharacter(_))
    ));
    assert!(matches!(
        SetKeyLayout::new("backups prod"),
        Err(SetKeyError::InvalidPrefixCharacter(_))
    ));

    let layout = SetKeyLayout::new("prefix").unwrap();

    // Set id rules
    assert_eq!(layout.manifest_key(""), Err(SetKeyError::EmptySetId));
    let long_id = "a".repeat(129);
    assert_eq!(
        layout.manifest_key(&long_id),
        Err(SetKeyError::SetIdTooLong(129))
    );
    assert!(matches!(
        layout.manifest_key("set/1"),
        Err(SetKeyError::SlashInSetId(_))
    ));
    assert!(matches!(
        layout.manifest_key("set:1"),
        Err(SetKeyError::InvalidSetIdCharacter(_))
    ));
    assert!(matches!(
        layout.manifest_key("set 1"),
        Err(SetKeyError::InvalidSetIdCharacter(_))
    ));
    assert!(matches!(
        layout.set_prefix("set/1"),
        Err(SetKeyError::SlashInSetId(_))
    ));

    // Artifact name rules
    assert_eq!(
        layout.artifact_key("s1", ""),
        Err(SetKeyError::EmptyArtifactName)
    );
    let long_name = "b".repeat(257);
    assert_eq!(
        layout.artifact_key("s1", &long_name),
        Err(SetKeyError::ArtifactNameTooLong(257))
    );
    assert!(matches!(
        layout.artifact_key("s1", "/data.tar"),
        Err(SetKeyError::LeadingSlashInArtifactName(_))
    ));
    assert!(matches!(
        layout.artifact_key("s1", "data.tar/"),
        Err(SetKeyError::TrailingSlashInArtifactName(_))
    ));
    assert!(matches!(
        layout.artifact_key("s1", "data//tar"),
        Err(SetKeyError::EmptyArtifactNameSegment(_))
    ));
    assert!(matches!(
        layout.artifact_key("s1", "."),
        Err(SetKeyError::DotSegmentInArtifactName(_))
    ));
    assert!(matches!(
        layout.artifact_key("s1", "a/./b"),
        Err(SetKeyError::DotSegmentInArtifactName(_))
    ));
    assert!(matches!(
        layout.artifact_key("s1", ".."),
        Err(SetKeyError::DotSegmentInArtifactName(_))
    ));
    assert!(matches!(
        layout.artifact_key("s1", "a/../b"),
        Err(SetKeyError::DotSegmentInArtifactName(_))
    ));
    assert!(matches!(
        layout.artifact_key("s1", "data:tar"),
        Err(SetKeyError::InvalidArtifactNameCharacter(_))
    ));
    assert!(matches!(
        layout.artifact_key("s1", "data tar"),
        Err(SetKeyError::InvalidArtifactNameCharacter(_))
    ));
}

#[test]
fn parse_set_id_round_trip_and_rejection() {
    let layout = SetKeyLayout::new("backup/v1").unwrap();
    let key = layout.artifact_key("set.1-a_b", "a/b.bin").unwrap();
    assert_eq!(layout.parse_set_id(&key), Some("set.1-a_b"));
    assert_eq!(layout.parse_set_id(&layout.latest_key()), None);
    assert_eq!(
        layout.parse_set_id("other/sets/set.1-a_b/manifest.json"),
        None
    );
    assert_eq!(layout.parse_set_id("backup/v1/sets/"), None);
    assert_eq!(layout.parse_set_id("backup/v1/sets/set.1-a_b"), None);

    let layout_empty = SetKeyLayout::new("").unwrap();
    let key_empty = layout_empty.artifact_key("set.1-a_b", "a/b.bin").unwrap();
    assert_eq!(layout_empty.parse_set_id(&key_empty), Some("set.1-a_b"));
    assert_eq!(layout_empty.parse_set_id(&layout_empty.latest_key()), None);
    assert_eq!(
        layout_empty.parse_set_id("other/sets/set.1-a_b/manifest.json"),
        None
    );
}

#[test]
fn collision_cross_product() {
    let layouts = [
        SetKeyLayout::new("").unwrap(),
        SetKeyLayout::new("backup").unwrap(),
    ];
    let set_ids = ["s1", "s2", "set-a.1_b"];
    let names = [
        "manifest.json",
        "latest.json",
        "data.bin",
        "nested/part.0",
        "artifacts",
    ];

    for layout in &layouts {
        let mut seen = HashSet::new();
        let latest = layout.latest_key();

        for set_id in &set_ids {
            let manifest = layout.manifest_key(set_id).unwrap();
            assert_ne!(manifest, latest);

            for name in &names {
                let artifact = layout.artifact_key(set_id, name).unwrap();
                assert_ne!(artifact, manifest);
                assert_ne!(artifact, latest);
                assert!(
                    seen.insert(artifact),
                    "collision detected for set_id={set_id}, name={name}"
                );
            }
        }
    }
}

#[test]
fn retention_keeps_newest_n() {
    let t0 = UNIX_EPOCH + Duration::from_secs(1_000_000);
    let inventory = vec![
        SetInventory {
            set_id: "set-1".to_string(),
            state: SetState::Complete {
                created_at: t0 + Duration::from_secs(10),
            },
            newest_object_at: t0 + Duration::from_secs(10),
        },
        SetInventory {
            set_id: "set-2".to_string(),
            state: SetState::Complete {
                created_at: t0 + Duration::from_secs(20),
            },
            newest_object_at: t0 + Duration::from_secs(20),
        },
        SetInventory {
            set_id: "set-3".to_string(),
            state: SetState::Complete {
                created_at: t0 + Duration::from_secs(30),
            },
            newest_object_at: t0 + Duration::from_secs(30),
        },
        SetInventory {
            set_id: "set-4".to_string(),
            state: SetState::Complete {
                created_at: t0 + Duration::from_secs(40),
            },
            newest_object_at: t0 + Duration::from_secs(40),
        },
        SetInventory {
            set_id: "set-5".to_string(),
            state: SetState::Complete {
                created_at: t0 + Duration::from_secs(50),
            },
            newest_object_at: t0 + Duration::from_secs(50),
        },
    ];

    let plan = plan_retention(
        &inventory,
        3,
        t0 + Duration::from_secs(100),
        Duration::from_secs(3600),
    );
    assert_eq!(plan.keep, vec!["set-3", "set-4", "set-5"]);
    assert_eq!(plan.delete_complete, vec!["set-1", "set-2"]);
    assert!(plan.delete_orphans.is_empty());
}

#[test]
fn keep_latest_zero_still_keeps_one() {
    let t0 = UNIX_EPOCH + Duration::from_secs(1_000_000);
    let inventory = vec![
        SetInventory {
            set_id: "set-1".to_string(),
            state: SetState::Complete {
                created_at: t0 + Duration::from_secs(10),
            },
            newest_object_at: t0 + Duration::from_secs(10),
        },
        SetInventory {
            set_id: "set-2".to_string(),
            state: SetState::Complete {
                created_at: t0 + Duration::from_secs(20),
            },
            newest_object_at: t0 + Duration::from_secs(20),
        },
    ];

    let plan = plan_retention(
        &inventory,
        0,
        t0 + Duration::from_secs(100),
        Duration::from_secs(3600),
    );
    assert_eq!(plan.keep, vec!["set-2"]);
    assert_eq!(plan.delete_complete, vec!["set-1"]);
    assert!(plan.delete_orphans.is_empty());
}

#[test]
fn young_partial_set_is_untouched() {
    let now = UNIX_EPOCH + Duration::from_secs(1_000_000);
    let grace = Duration::from_secs(3600);
    let inventory = vec![SetInventory {
        set_id: "partial-young".to_string(),
        state: SetState::Partial,
        newest_object_at: now - Duration::from_secs(300), // 5 min old
    }];

    let plan = plan_retention(&inventory, 1, now, grace);
    assert!(plan.keep.is_empty());
    assert!(plan.delete_complete.is_empty());
    assert!(plan.delete_orphans.is_empty());
}

#[test]
fn aged_partial_set_is_orphan() {
    let now = UNIX_EPOCH + Duration::from_secs(1_000_000);
    let grace = Duration::from_secs(3600);
    let inventory = vec![SetInventory {
        set_id: "partial-aged".to_string(),
        state: SetState::Partial,
        newest_object_at: now - Duration::from_secs(7200), // 2 hours old
    }];

    let plan = plan_retention(&inventory, 1, now, grace);
    assert!(plan.keep.is_empty());
    assert!(plan.delete_complete.is_empty());
    assert_eq!(plan.delete_orphans, vec!["partial-aged"]);
}

#[test]
fn input_order_does_not_change_plan() {
    let now = UNIX_EPOCH + Duration::from_secs(1_000_000);
    let grace = Duration::from_secs(3600);

    let item1 = SetInventory {
        set_id: "complete-a".to_string(),
        state: SetState::Complete {
            created_at: now - Duration::from_secs(500),
        },
        newest_object_at: now - Duration::from_secs(500),
    };
    let item2 = SetInventory {
        set_id: "complete-b".to_string(),
        state: SetState::Complete {
            created_at: now - Duration::from_secs(500), // tie with complete-a
        },
        newest_object_at: now - Duration::from_secs(500),
    };
    let item3 = SetInventory {
        set_id: "complete-c".to_string(),
        state: SetState::Complete {
            created_at: now - Duration::from_secs(200),
        },
        newest_object_at: now - Duration::from_secs(200),
    };
    let item4 = SetInventory {
        set_id: "partial-young".to_string(),
        state: SetState::Partial,
        newest_object_at: now - Duration::from_secs(100),
    };
    let item5 = SetInventory {
        set_id: "partial-aged".to_string(),
        state: SetState::Partial,
        newest_object_at: now - Duration::from_secs(5000),
    };

    let order1 = vec![
        item1.clone(),
        item2.clone(),
        item3.clone(),
        item4.clone(),
        item5.clone(),
    ];
    let mut order2 = order1.clone();
    order2.reverse();
    let order3 = vec![
        item3.clone(),
        item5.clone(),
        item1.clone(),
        item4.clone(),
        item2.clone(),
    ];

    let plan1 = plan_retention(&order1, 2, now, grace);
    let plan2 = plan_retention(&order2, 2, now, grace);
    let plan3 = plan_retention(&order3, 2, now, grace);

    assert_eq!(plan1, plan2);
    assert_eq!(plan1, plan3);
}

#[test]
fn inventory_with_only_partial_sets_deletes_no_complete_set() {
    let now = UNIX_EPOCH + Duration::from_secs(1_000_000);
    let grace = Duration::from_secs(3600);

    let inventory = vec![
        SetInventory {
            set_id: "p1-young".to_string(),
            state: SetState::Partial,
            newest_object_at: now - Duration::from_secs(60),
        },
        SetInventory {
            set_id: "p2-aged".to_string(),
            state: SetState::Partial,
            newest_object_at: now - Duration::from_secs(4000),
        },
        SetInventory {
            set_id: "p3-aged".to_string(),
            state: SetState::Partial,
            newest_object_at: now - Duration::from_secs(8000),
        },
    ];

    let plan = plan_retention(&inventory, 2, now, grace);
    assert!(plan.keep.is_empty());
    assert!(plan.delete_complete.is_empty());
    assert_eq!(plan.delete_orphans, vec!["p2-aged", "p3-aged"]);
}
