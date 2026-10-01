use super::*;

/// A leader has no capacity for a proposal before it allocated a Raft index.
/// Callers can downcast the error returned by [`RaftHost::propose`] to select a
/// retry policy without treating an uncertain append as safely retryable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalBackpressure {
    pub reason: String,
    pub retry_after_seconds: u64,
}

impl std::fmt::Display for ProposalBackpressure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}; retry after {} seconds",
            self.reason, self.retry_after_seconds
        )
    }
}

impl std::error::Error for ProposalBackpressure {}

const BACKPRESSURE_REASON_PREFIX: &str = "\u{1e}raft-proposal-backpressure:";

pub(super) fn encode_backpressure(backpressure: &ProposalBackpressure) -> String {
    format!(
        "{BACKPRESSURE_REASON_PREFIX}{}",
        serde_json::json!({
            "reason": backpressure.reason,
            "retry_after_seconds": backpressure.retry_after_seconds,
        })
    )
}

pub(crate) fn decode_backpressure(reason: &str) -> Option<ProposalBackpressure> {
    let body = reason.strip_prefix(BACKPRESSURE_REASON_PREFIX)?;
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    Some(ProposalBackpressure {
        reason: value.get("reason")?.as_str()?.to_owned(),
        retry_after_seconds: value.get("retry_after_seconds")?.as_u64()?,
    })
}

pub(super) fn rejected_admission(error: anyhow::Error) -> ProposalOutcome {
    if let Some(backpressure) = error.downcast_ref::<ProposalBackpressure>() {
        ProposalOutcome::RejectedBeforeAdmission {
            reason: encode_backpressure(backpressure),
        }
    } else {
        ProposalOutcome::RejectedBeforeAdmission {
            reason: error.to_string(),
        }
    }
}
