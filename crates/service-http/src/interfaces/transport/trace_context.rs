#[derive(Clone, Debug, PartialEq, Eq)]
/// Canonical correlation fields for one inbound HTTP request.
///
/// The context is always available, including in logging-only builds. A valid
/// W3C version 00 `traceparent` preserves its trace and parent span ids; absent
/// or invalid input creates a new local root context.
pub struct RequestTraceContext {
    trace_id: String,
    span_id: String,
    parent_span_id: Option<String>,
    trace_flags: String,
}

impl RequestTraceContext {
    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    pub fn span_id(&self) -> &str {
        &self.span_id
    }

    pub fn parent_span_id(&self) -> Option<&str> {
        self.parent_span_id.as_deref()
    }

    pub fn trace_flags(&self) -> &str {
        &self.trace_flags
    }
}

#[derive(Debug)]
struct ParsedTraceParent {
    trace_id: String,
    parent_span_id: String,
    trace_flags: String,
}

/// Parse the inbound W3C context and return a usable request correlation
/// context. Invalid input is deliberately equivalent to no input: business
/// routing continues on a fresh local root trace.
pub fn request_trace_context(headers: &axum::http::HeaderMap) -> RequestTraceContext {
    let parsed = parse_traceparent(headers);
    RequestTraceContext {
        trace_id: parsed
            .as_ref()
            .map(|parent| parent.trace_id.clone())
            .unwrap_or_else(|| fresh_hex_id(16)),
        span_id: fresh_hex_id(8),
        parent_span_id: parsed.as_ref().map(|parent| parent.parent_span_id.clone()),
        trace_flags: parsed
            .as_ref()
            .map(|parent| parent.trace_flags.clone())
            .unwrap_or_else(|| "00".to_string()),
    }
}

fn parse_traceparent(headers: &axum::http::HeaderMap) -> Option<ParsedTraceParent> {
    let mut values = headers.get_all("traceparent").iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() || !value.is_ascii() || value.len() != 55 {
        return None;
    }
    let bytes = value.as_bytes();
    if bytes[2] != b'-' || bytes[35] != b'-' || bytes[52] != b'-' {
        return None;
    }
    let version = &value[0..2];
    let trace_id = &value[3..35];
    let parent_span_id = &value[36..52];
    let trace_flags = &value[53..55];
    if version != "00"
        || !lower_hex(trace_id)
        || !lower_hex(parent_span_id)
        || !lower_hex(trace_flags)
        || all_zero(trace_id)
        || all_zero(parent_span_id)
    {
        return None;
    }
    Some(ParsedTraceParent {
        trace_id: trace_id.to_string(),
        parent_span_id: parent_span_id.to_string(),
        trace_flags: trace_flags.to_string(),
    })
}

fn lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn all_zero(value: &str) -> bool {
    value.bytes().all(|byte| byte == b'0')
}

fn fresh_hex_id(byte_len: usize) -> String {
    use sha2::{Digest as _, Sha256};
    use std::fmt::Write as _;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    loop {
        let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut hasher = Sha256::new();
        hasher.update(timestamp.to_le_bytes());
        hasher.update(std::process::id().to_le_bytes());
        hasher.update(sequence.to_le_bytes());
        let digest = hasher.finalize();
        let mut value = String::with_capacity(byte_len * 2);
        for byte in &digest[..byte_len] {
            write!(&mut value, "{byte:02x}").expect("write to String");
        }
        if !all_zero(&value) {
            return value;
        }
    }
}
