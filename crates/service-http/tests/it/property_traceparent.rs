use axum::http::{HeaderMap, HeaderValue};
use proptest::prelude::*;
use proptest::test_runner::TestRunner;
use service_http::transport::request_trace_context;

fn is_lower_hex(b: u8) -> bool {
    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
}

// Accepts every value of the right length. It is a deliberately broken
// parser, used only by the negative test below.
fn broken_accepts(value: &str) -> bool {
    value.len() == 55
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn valid_traceparent_is_preserved(
        trace in "[0-9a-f]{32}",
        parent in "[0-9a-f]{16}",
        flags in "[0-9a-f]{2}",
    ) {
        prop_assume!(trace.bytes().any(|b| b != b'0') && parent.bytes().any(|b| b != b'0'));
        let mut headers = HeaderMap::new();
        headers.insert(
            "traceparent",
            HeaderValue::from_str(&format!("00-{trace}-{parent}-{flags}")).expect("ascii header"),
        );
        let ctx = request_trace_context(&headers);
        prop_assert_eq!(ctx.trace_id(), trace.as_str());
        prop_assert_eq!(ctx.parent_span_id(), Some(parent.as_str()));
        prop_assert_eq!(ctx.trace_flags(), flags.as_str());
    }

    #[test]
    fn arbitrary_header_never_panics(value in "[ -~]{0,80}") {
        prop_assume!(HeaderValue::from_str(&value).is_ok());
        let mut headers = HeaderMap::new();
        headers.insert("traceparent", HeaderValue::from_str(&value).expect("checked above"));
        let ctx = request_trace_context(&headers);
        prop_assert_eq!(ctx.trace_id().len(), 32);
        prop_assert_eq!(ctx.span_id().len(), 16);
        prop_assert!(ctx.trace_id().bytes().all(is_lower_hex));
        prop_assert!(ctx.span_id().bytes().all(is_lower_hex));
    }
}

#[test]
fn seeded_bug_is_found() {
    let mut runner = TestRunner::default();
    let result = runner.run(&"[ -~]{55}", |v| {
        prop_assume!(HeaderValue::from_str(&v).is_ok());
        let mut headers = HeaderMap::new();
        headers.insert(
            "traceparent",
            HeaderValue::from_str(&v).expect("checked above"),
        );
        let kept = request_trace_context(&headers).parent_span_id().is_some();
        prop_assert_eq!(broken_accepts(&v), kept);
        Ok(())
    });
    assert!(result.is_err());
}
