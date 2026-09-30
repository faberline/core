//! Forwarding a proposal to the leader's publish endpoint.

use reqwest::StatusCode;

use super::HttpPeerClient;
use crate::domain::{ForwardReply, PUBLISH_PATH};
use crate::infrastructure::PublishEnvelope;

impl HttpPeerClient {
    /// POST `command` to the leader's publish endpoint and classify the reply.
    pub(super) async fn forward_to_leader(&self, leader_url: &str, command: &[u8]) -> ForwardReply {
        let resp = match self
            .http_client()
            .post(format!("{leader_url}{PUBLISH_PATH}"))
            // Unlike Vote/AppendEntries, publish includes durable quorum
            // commit and local state-machine apply. Its request budget is the
            // proposal deadline, not the short peer transport timeout.
            .timeout(self.propose_timeout)
            .json(&PublishEnvelope {
                group_id: self.group_id.0.clone(),
                command: command.to_vec(),
            })
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return ForwardReply::Failed {
                    reason: e.to_string(),
                };
            }
        };
        let status = resp.status();
        if status == StatusCode::TOO_MANY_REQUESTS {
            let value: serde_json::Value = match resp.json().await {
                Ok(value) => value,
                Err(error) => {
                    return ForwardReply::Failed {
                        reason: error.to_string(),
                    }
                }
            };
            let Some(reason) = value.get("error").and_then(|value| value.as_str()) else {
                return ForwardReply::Failed {
                    reason: "raft: leader backpressure reply missing error".to_string(),
                };
            };
            let Some(retry_after_seconds) = value
                .get("retry_after_seconds")
                .and_then(|value| value.as_u64())
            else {
                return ForwardReply::Failed {
                    reason: "raft: leader backpressure reply missing retry_after_seconds"
                        .to_string(),
                };
            };
            return ForwardReply::Backpressure {
                reason: reason.to_string(),
                retry_after_seconds,
            };
        }
        if status == StatusCode::SERVICE_UNAVAILABLE {
            let v: serde_json::Value = match resp.json().await {
                Ok(v) => v,
                Err(e) => {
                    return ForwardReply::Failed {
                        reason: e.to_string(),
                    };
                }
            };
            if v.get("outcome").and_then(|outcome| outcome.as_str())
                == Some("rejected_before_admission")
            {
                if let Some(reason) = v.get("error").and_then(|error| error.as_str()) {
                    return ForwardReply::Rejected {
                        reason: reason.to_string(),
                    };
                }
            }
            return ForwardReply::Failed {
                reason: format!("raft: leader redirect returned {status}"),
            };
        }
        if status != StatusCode::OK {
            return ForwardReply::Failed {
                reason: format!("raft: leader redirect returned {status}"),
            };
        }
        let v: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                return ForwardReply::Failed {
                    reason: e.to_string(),
                };
            }
        };
        match v.get("seq").and_then(|s| s.as_u64()) {
            Some(seq) => ForwardReply::Accepted { seq },
            None => ForwardReply::Failed {
                reason: "raft: leader reply missing seq".to_string(),
            },
        }
    }
}
