use super::*;

/// The server's expiry wins. Asking for 600 seconds and being handed 300
/// is a supported answer, not an anomaly, and the refresh point has to
/// follow the answer.
#[tokio::test]
async fn the_refresh_point_follows_the_issued_expiry_not_the_requested_one() {
    let requested = MintedToken::new("t", 0, 600_000);
    assert_eq!(requested.refresh_at_millis(), 480_000);

    let issued_shorter = MintedToken::new("t", 0, 300_000);
    assert_eq!(issued_shorter.refresh_at_millis(), 240_000);

    // 20% of a minute is 12 seconds, less than the 30-second guard, so the
    // guard wins and the refresh happens earlier than the fraction alone
    // would put it.
    let very_short = MintedToken::new("t", 0, 60_000);
    assert_eq!(very_short.refresh_at_millis(), 30_000);

    // Shorter than the guard entirely: due immediately. A token that
    // cannot be held for the guard interval is one to replace on sight,
    // not one to hold and hope.
    let tiny = MintedToken::new("t", 0, 10_000);
    assert_eq!(tiny.refresh_at_millis(), 0);
}

/// AC5's first half: a long-running caller mints once, reuses, and mints
/// again before the token it holds stops working — never after.
#[tokio::test]
async fn a_long_running_caller_refreshes_before_expiry_and_not_on_every_call() {
    let clock = Arc::new(ManualClock::new(0));
    let minter = Arc::new(RecordingMinter::new(
        clock.clone(),
        Duration::from_secs(600),
    ));
    let source = TokenSource::with_clock(minter.clone(), target(), clock.clone());

    let first = source.token().await.expect("first mint");
    assert_eq!(minter.calls(), 1);

    // Anywhere inside the first four fifths, the same token comes back.
    for _ in 0..5 {
        clock.advance(Duration::from_secs(60));
        assert_eq!(
            source.token().await.expect("reuse").expose(),
            first.expose()
        );
    }
    assert_eq!(minter.calls(), 1, "a reused token must not be re-minted");

    // 480s in — the refresh point, two minutes before the token dies.
    clock.advance(Duration::from_secs(180));
    let second = source.token().await.expect("refresh");
    assert_ne!(
        second.expose(),
        first.expose(),
        "the refresh must produce a new token, not re-present the old one"
    );
    assert_eq!(minter.calls(), 2);
    assert!(
        clock.now_millis() < 600_000,
        "the refresh has to happen while the old token still works, or every in-flight \
             request between expiry and refresh fails"
    );
}

/// AC5's second half: when the grant goes away, the next refresh fails and
/// the failure is returned. Continuing to serve the token already in hand
/// would turn revocation into a delay.
#[tokio::test]
async fn a_revoked_grant_surfaces_at_the_next_refresh_rather_than_at_expiry() {
    let clock = Arc::new(ManualClock::new(0));
    let minter =
        Arc::new(RecordingMinter::new(clock.clone(), Duration::from_secs(600)).failing_after(1));
    let source = TokenSource::with_clock(minter.clone(), target(), clock.clone());

    source.token().await.expect("the first mint still works");
    clock.advance(Duration::from_secs(480));

    let err = source.token().await.expect_err("the refresh is refused");
    assert!(
        matches!(err, TokenRequestError::Forbidden { .. }),
        "{err:?}"
    );
}
