use super::*;
use crate::certificate::profile::{
    CertificateIdentity, CertificateProfile, InstanceScope, Purpose,
};

fn scope() -> InstanceScope {
    InstanceScope::new("lumen", "lumen", "lumen-prod.svc.id.goog")
}

fn profile() -> CertificateProfile {
    CertificateProfile::new(
        &scope(),
        Purpose::Serving,
        "lumen.lumen.svc.cluster.local",
        CertificateIdentity {
            dns_names: vec!["lumen.lumen.svc.cluster.local".into()],
            spiffe_uri: None,
        },
        Duration::from_secs(86_400),
        Duration::from_secs(21_600),
        Duration::from_secs(1_800),
    )
    .unwrap()
}

fn at(hours: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_800_000_000 + hours * 3_600, 0).unwrap()
}

fn leaf(issuer: &str, profile: &CertificateProfile) -> ObservedLeaf {
    ObservedLeaf {
        issuer: IssuerId::new(issuer),
        not_before: at(0),
        not_after: at(24),
        fingerprint: "aa".repeat(32),
        identity_digest: profile.identity_digest(),
    }
}

#[test]
fn trust_is_published_before_anything_is_issued() {
    let profile = profile();
    let desired = Desired {
        profile: &profile,
        issuer: IssuerId::new("pool-a"),
    };
    let action = next_action(&desired, &Observed::default(), at(1));
    assert_eq!(
        action,
        Action::PublishTrustBundle {
            issuers: vec![IssuerId::new("pool-a")]
        },
        "a leaf from an untrusted issuer fails on the far side, where the error names nothing"
    );
}

#[test]
fn a_healthy_leaf_waits_until_its_renewal_instant() {
    let profile = profile();
    let observed = Observed {
        leaf: Some(leaf("pool-a", &profile)),
        trust_bundle: vec![IssuerId::new("pool-a")],
        ..Observed::default()
    };
    let desired = Desired {
        profile: &profile,
        issuer: IssuerId::new("pool-a"),
    };
    let Action::Wait { until } = next_action(&desired, &observed, at(1)) else {
        panic!("expected Wait");
    };
    // 24h expiry, 6h window, up to 30m jitter.
    assert!(until >= at(18) && until <= at(18) + chrono::Duration::minutes(30));
}

#[test]
fn the_renewal_instant_does_not_move_when_the_controller_restarts() {
    let profile = profile();
    let leaf = leaf("pool-a", &profile);
    let first = renew_at(&profile, &leaf);
    let second = renew_at(&profile, &leaf);
    assert_eq!(
        first, second,
        "a jitter drawn at reconcile time would make every restart a different deadline"
    );
}

#[test]
fn different_certificates_still_spread_out() {
    let profile = profile();
    let mut a = leaf("pool-a", &profile);
    let mut b = a.clone();
    a.fingerprint = "11".repeat(32);
    b.fingerprint = "22".repeat(32);
    assert_ne!(
        renew_at(&profile, &a),
        renew_at(&profile, &b),
        "deterministic must not mean identical; the point of jitter is that a fleet does \
         not renew in lockstep"
    );
}

#[test]
fn an_identity_change_reissues_a_perfectly_valid_leaf() {
    let profile = profile();
    let mut observed = Observed {
        leaf: Some(leaf("pool-a", &profile)),
        trust_bundle: vec![IssuerId::new("pool-a")],
        ..Observed::default()
    };
    observed.leaf.as_mut().unwrap().identity_digest = "stale".into();
    let desired = Desired {
        profile: &profile,
        issuer: IssuerId::new("pool-a"),
    };
    assert_eq!(
        next_action(&desired, &observed, at(1)),
        Action::Issue {
            issuer: IssuerId::new("pool-a"),
            reason: IssueReason::IdentityChanged
        }
    );
}

#[test]
fn an_expired_leaf_is_distinguishable_from_a_due_one() {
    let profile = profile();
    let observed = Observed {
        leaf: Some(leaf("pool-a", &profile)),
        trust_bundle: vec![IssuerId::new("pool-a")],
        ..Observed::default()
    };
    let desired = Desired {
        profile: &profile,
        issuer: IssuerId::new("pool-a"),
    };
    assert_eq!(
        next_action(&desired, &observed, at(25)),
        Action::Issue {
            issuer: IssuerId::new("pool-a"),
            reason: IssueReason::Expired
        },
        "'expired' means something was already wrong; folding it into 'renewal' hides that"
    );
}

#[test]
fn no_action_can_remove_the_current_leaf() {
    // Enumerated rather than asserted on one case: the guarantee is about
    // the shape of `Action`, so the test that matters is that every
    // reachable variant leaves existing material alone.
    let profile = profile();
    let desired = Desired {
        profile: &profile,
        issuer: IssuerId::new("pool-b"),
    };
    let states = [
        Observed::default(),
        Observed {
            leaf: Some(leaf("pool-a", &profile)),
            trust_bundle: vec![IssuerId::new("pool-a")],
            ..Observed::default()
        },
        Observed {
            leaf: Some(leaf("pool-b", &profile)),
            trust_bundle: vec![IssuerId::new("pool-a"), IssuerId::new("pool-b")],
            activated_fingerprint: Some("aa".repeat(32)),
            ..Observed::default()
        },
    ];
    for observed in states {
        match next_action(&desired, &observed, at(1)) {
            Action::PublishTrustBundle { .. }
            | Action::Issue { .. }
            | Action::AwaitActivation { .. }
            | Action::RetireIssuers { .. }
            | Action::Wait { .. } => {}
        }
    }
}

#[test]
fn backoff_climbs_to_a_ceiling_and_stays_there() {
    assert_eq!(retry_after(0), Duration::from_secs(5));
    assert!(retry_after(1) < retry_after(4));
    assert_eq!(retry_after(20), Duration::from_secs(300));
    assert_eq!(
        retry_after(u32::MAX),
        Duration::from_secs(300),
        "a controller racing an expiry must not back off past the window it is racing"
    );
}
