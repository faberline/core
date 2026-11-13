//! Asking the leader's read-index endpoint for its read index.

use reqwest::StatusCode;

use super::HttpPeerClient;
use crate::domain::{ReadIndexReply, READ_INDEX_PATH};
use crate::infrastructure::peer_wire::{ReadIndexEnvelope, ReadIndexResp};

impl HttpPeerClient {
    /// POST to the leader's read-index endpoint and classify the reply.
    #[allow(dead_code)] // used by #110 part 2
    pub(super) async fn read_index_from_leader(&self, leader_url: &str) -> ReadIndexReply {
        let resp = match self
            .http_client()
            .post(format!("{leader_url}{READ_INDEX_PATH}"))
            .timeout(self.propose_timeout)
            .json(&ReadIndexEnvelope {
                group_id: self.group_id.as_str().to_owned(),
            })
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return ReadIndexReply::Failed {
                    reason: e.to_string(),
                };
            }
        };
        let status = resp.status();
        if status == StatusCode::MISDIRECTED_REQUEST {
            return ReadIndexReply::NotLeader;
        }
        if status == StatusCode::SERVICE_UNAVAILABLE {
            let value: serde_json::Value = match resp.json().await {
                Ok(value) => value,
                Err(e) => {
                    return ReadIndexReply::Failed {
                        reason: e.to_string(),
                    };
                }
            };
            return match value.get("error").and_then(|error| error.as_str()) {
                Some(reason) => ReadIndexReply::Refused {
                    reason: reason.to_string(),
                },
                None => ReadIndexReply::Failed {
                    reason: "raft: leader read-index refusal missing error".to_string(),
                },
            };
        }
        if status != StatusCode::OK {
            return ReadIndexReply::Failed {
                reason: format!("raft: leader read-index returned {status}"),
            };
        }
        match resp.json::<ReadIndexResp>().await {
            Ok(body) => ReadIndexReply::Granted { index: body.index },
            Err(_) => ReadIndexReply::Failed {
                reason: "raft: leader reply missing index".to_string(),
            },
        }
    }
}
