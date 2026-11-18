//! The HTTP side of [`RaftAdminClient`]: the five membership calls of a shard,
//! sent to the `/raft/admin/` routes of the shard leader (core#398).
//!
//! Wire contract of those routes: `POST` with a JSON body `{group_id, target}`
//! (`add-learner` also has `url`). `200` is done. `409` is a refusal with the body
//! `{refusal, message}`. `421` is "not the leader" with the body
//! `{error: "not-leader", leader: <node id or null>}`.

use futures::future::BoxFuture;
use serde_json::{json, Value};

use crate::domain::membership_port::{MembershipError, RaftAdminClient, StepResult};

/// How a shard and a replica map to the group id, the pod url and the Raft node id.
/// The per-shard naming is owned by the render, not by this client.
pub struct RaftAdminTopology {
    /// The Raft group id of a shard (the `group_id` of every request body).
    pub group_id: Box<dyn Fn(u32) -> String + Send + Sync>,
    /// The base url of the pod of a replica in a shard, for example `http://pod-0:8080`.
    pub pod_url: Box<dyn Fn(u32, u32) -> String + Send + Sync>,
    /// The Raft node id of a replica in a shard.
    pub node_id: Box<dyn Fn(u32, u32) -> u64 + Send + Sync>,
    /// The replica of a shard that has this Raft node id, or `None` when there is none.
    pub replica_of: Box<dyn Fn(u32, u64) -> Option<u32> + Send + Sync>,
}

/// Calls the membership admin routes over HTTP.
pub struct HttpRaftAdminClient {
    http: reqwest::Client,
    topology: RaftAdminTopology,
}

impl HttpRaftAdminClient {
    /// A client that speaks h2c (HTTP/2 with prior knowledge), as the raft-runtime peer port does.
    pub fn new(topology: RaftAdminTopology) -> Self {
        let http = reqwest::Client::builder()
            .http2_prior_knowledge()
            .build()
            .expect("a reqwest client with default settings builds");
        Self::with_client(http, topology)
    }

    /// A client that uses the given `reqwest::Client`.
    pub fn with_client(http: reqwest::Client, topology: RaftAdminTopology) -> Self {
        Self { http, topology }
    }

    /// The request body of `replica`: its group and its node id.
    fn target_body(&self, shard: u32, replica: u32) -> Value {
        json!({
            "group_id": (self.topology.group_id)(shard),
            "target": (self.topology.node_id)(shard, replica),
        })
    }

    /// POST `body` to `/raft/admin/<route>` on the pod of `entry`. A `421` that names
    /// another replica as the leader is followed once.
    async fn post(
        &self,
        shard: u32,
        entry: u32,
        route: &str,
        body: &Value,
    ) -> Result<StepResult, MembershipError> {
        let mut replica = entry;
        for attempt in 0..2 {
            let url = format!(
                "{}/raft/admin/{route}",
                (self.topology.pod_url)(shard, replica)
            );
            let response = self.http.post(&url).json(body).send().await.map_err(|e| {
                MembershipError::new(format!("{route} on shard {shard}: POST {url}: {e}"))
            })?;
            let status = response.status().as_u16();
            let answer: Value = response.json().await.unwrap_or(Value::Null);
            match status {
                200 => return Ok(StepResult::Applied),
                409 => return conflict(route, shard, &answer),
                421 => {
                    let leader = answer["leader"]
                        .as_u64()
                        .and_then(|id| (self.topology.replica_of)(shard, id));
                    match leader {
                        Some(next) if attempt == 0 && next != replica => replica = next,
                        _ => return Ok(StepResult::Waiting("not the leader".to_string())),
                    }
                }
                other => {
                    return Err(MembershipError::new(format!(
                        "{route} on shard {shard}: {url} answered {other}"
                    )))
                }
            }
        }
        Ok(StepResult::Waiting("not the leader".to_string()))
    }
}

/// A `409`: a refusal that goes away by itself is `Waiting`, any other refusal is an error.
fn conflict(route: &str, shard: u32, answer: &Value) -> Result<StepResult, MembershipError> {
    let message = answer["message"].as_str().unwrap_or("refused").to_string();
    let refusal = &answer["refusal"];
    let transient = refusal.get("NotCaughtUp").is_some()
        || matches!(
            refusal.as_str(),
            Some("TransitionInFlight" | "NotLeaderOrTransferInFlight")
        );
    if transient {
        Ok(StepResult::Waiting(message))
    } else {
        Err(MembershipError::new(format!(
            "{route} on shard {shard} refused: {message}"
        )))
    }
}

impl RaftAdminClient for HttpRaftAdminClient {
    fn add_learner<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            let mut body = self.target_body(shard, replica);
            body["url"] = json!((self.topology.pod_url)(shard, replica));
            self.post(shard, 0, "add-learner", &body).await
        })
    }

    fn promote<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            let body = self.target_body(shard, replica);
            self.post(shard, 0, "promote", &body).await
        })
    }

    fn transfer_leadership<'a>(
        &'a self,
        shard: u32,
        from: u32,
        to: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            let body = self.target_body(shard, to);
            self.post(shard, from, "transfer-leadership", &body).await
        })
    }

    fn demote<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            let body = self.target_body(shard, replica);
            self.post(shard, 0, "demote", &body).await
        })
    }

    fn remove_member<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            let body = self.target_body(shard, replica);
            self.post(shard, 0, "remove", &body).await
        })
    }
}

#[cfg(test)]
mod tests;
