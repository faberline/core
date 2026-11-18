use super::*;

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// What one fake pod saw: the request line and the JSON body of each request.
type Seen = Arc<Mutex<Vec<(String, Value)>>>;

/// A fake pod. It answers the n-th connection with the n-th canned reply
/// (status, JSON body) and records what it was asked.
async fn pod(replies: Vec<(u16, &'static str)>) -> (String, Seen) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    tokio::spawn(async move {
        for (status, body) in replies {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf: Vec<u8> = Vec::new();
            let mut chunk = [0u8; 1024];
            let (head_len, content_len) = loop {
                let n = stream.read(&mut chunk).await.unwrap();
                assert!(n > 0, "the client closed before it sent a full request");
                buf.extend_from_slice(&chunk[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..pos]).to_lowercase();
                    let len = head
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .map(|v| v.trim().parse::<usize>().unwrap())
                        .unwrap_or(0);
                    break (pos + 4, len);
                }
            };
            while buf.len() < head_len + content_len {
                let n = stream.read(&mut chunk).await.unwrap();
                assert!(n > 0, "the client closed before it sent the whole body");
                buf.extend_from_slice(&chunk[..n]);
            }
            let text = String::from_utf8_lossy(&buf).to_string();
            let request_line = text.lines().next().unwrap().to_string();
            let body_json: Value =
                serde_json::from_slice(&buf[head_len..head_len + content_len]).unwrap();
            log.lock().unwrap().push((request_line, body_json));
            let reply = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    (url, seen)
}

/// Group `g<shard>`, pod `urls[replica]`, node id `replica + 10`.
fn client(urls: Vec<String>) -> HttpRaftAdminClient {
    let count = urls.len() as u32;
    HttpRaftAdminClient::with_client(
        reqwest::Client::new(),
        RaftAdminTopology {
            group_id: Box::new(|shard| format!("g{shard}")),
            pod_url: Box::new(move |_, replica| urls[replica as usize].clone()),
            node_id: Box::new(|_, replica| u64::from(replica) + 10),
            replica_of: Box::new(move |_, id| {
                id.checked_sub(10)
                    .map(|replica| replica as u32)
                    .filter(|replica| *replica < count)
            }),
        },
    )
}

const NOT_LEADER_12: &str = r#"{"error":"not-leader","leader":12}"#;

#[tokio::test]
async fn a_200_is_applied_and_the_body_names_the_group_and_the_node() {
    let (url, seen) = pod(vec![(200, r#"{"index":7}"#)]).await;
    let client = client(vec![url.clone(), url]);
    let result = client.promote(0, 1).await.unwrap();
    assert_eq!(result, StepResult::Applied);
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0, "POST /raft/admin/promote HTTP/1.1");
    assert_eq!(seen[0].1, json!({ "group_id": "g0", "target": 11 }));
}

#[tokio::test]
async fn add_learner_also_sends_the_url_of_the_new_pod() {
    let (url, seen) = pod(vec![(200, r#"{"index":3}"#)]).await;
    let client = client(vec![url, "http://pod-1:8080".to_string()]);
    let result = client.add_learner(0, 1).await.unwrap();
    assert_eq!(result, StepResult::Applied);
    let seen = seen.lock().unwrap();
    assert_eq!(seen[0].0, "POST /raft/admin/add-learner HTTP/1.1");
    assert_eq!(
        seen[0].1,
        json!({ "group_id": "g0", "target": 11, "url": "http://pod-1:8080" })
    );
}

#[tokio::test]
async fn demote_and_remove_use_their_own_routes() {
    let (url, seen) = pod(vec![(200, "{}"), (200, "{}")]).await;
    let client = client(vec![url.clone(), url]);
    assert_eq!(client.demote(0, 1).await.unwrap(), StepResult::Applied);
    assert_eq!(
        client.remove_member(0, 1).await.unwrap(),
        StepResult::Applied
    );
    let seen = seen.lock().unwrap();
    assert_eq!(seen[0].0, "POST /raft/admin/demote HTTP/1.1");
    assert_eq!(seen[1].0, "POST /raft/admin/remove HTTP/1.1");
}

#[tokio::test]
async fn a_409_not_caught_up_waits_with_the_message() {
    let reply = r#"{"refusal":{"NotCaughtUp":{"matched":1,"target":5}},"message":"NotCaughtUp { matched: 1, target: 5 }"}"#;
    let (url, _) = pod(vec![(409, reply)]).await;
    let client = client(vec![url.clone(), url]);
    let result = client.promote(0, 1).await.unwrap();
    assert_eq!(
        result,
        StepResult::Waiting("NotCaughtUp { matched: 1, target: 5 }".to_string())
    );
}

#[tokio::test]
async fn any_other_409_is_an_error_that_carries_the_message() {
    let reply = r#"{"refusal":"NotALearner","message":"node 11 is not a learner of this group"}"#;
    let (url, _) = pod(vec![(409, reply)]).await;
    let client = client(vec![url.clone(), url]);
    let error = client.promote(0, 1).await.unwrap_err();
    assert!(
        error
            .message
            .contains("node 11 is not a learner of this group"),
        "{}",
        error.message
    );
    assert!(error.message.contains("promote"), "{}", error.message);
}

#[tokio::test]
async fn a_421_follows_the_leader_once() {
    let (first, first_seen) = pod(vec![(421, NOT_LEADER_12)]).await;
    let (second, second_seen) = pod(vec![(200, r#"{"index":9}"#)]).await;
    let client = client(vec![first, "http://unused:1".to_string(), second]);
    let result = client.remove_member(0, 1).await.unwrap();
    assert_eq!(result, StepResult::Applied);
    let first_seen = first_seen.lock().unwrap();
    let second_seen = second_seen.lock().unwrap();
    assert_eq!(first_seen.len(), 1);
    assert_eq!(second_seen.len(), 1);
    assert_eq!(second_seen[0].0, "POST /raft/admin/remove HTTP/1.1");
    assert_eq!(second_seen[0].1, json!({ "group_id": "g0", "target": 11 }));
}

#[tokio::test]
async fn a_second_421_is_not_followed_and_waits() {
    let (first, first_seen) = pod(vec![(421, NOT_LEADER_12)]).await;
    let (second, second_seen) = pod(vec![(421, r#"{"error":"not-leader","leader":10}"#)]).await;
    let client = client(vec![first, "http://unused:1".to_string(), second]);
    let result = client.remove_member(0, 1).await.unwrap();
    assert_eq!(result, StepResult::Waiting("not the leader".to_string()));
    assert_eq!(first_seen.lock().unwrap().len(), 1);
    assert_eq!(second_seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn a_421_that_names_no_leader_waits() {
    let (url, seen) = pod(vec![(421, r#"{"error":"not-leader","leader":null}"#)]).await;
    let client = client(vec![url.clone(), url]);
    let result = client.demote(0, 1).await.unwrap();
    assert_eq!(result, StepResult::Waiting("not the leader".to_string()));
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn transfer_leadership_asks_the_from_pod_and_names_the_to_node() {
    let (zero, zero_seen) = pod(vec![]).await;
    let (one, one_seen) = pod(vec![]).await;
    let (two, two_seen) = pod(vec![(200, r#"{"transferring_to":11}"#)]).await;
    let client = client(vec![zero, one, two]);
    let result = client.transfer_leadership(0, 2, 1).await.unwrap();
    assert_eq!(result, StepResult::Applied);
    assert!(zero_seen.lock().unwrap().is_empty());
    assert!(one_seen.lock().unwrap().is_empty());
    let two_seen = two_seen.lock().unwrap();
    assert_eq!(
        two_seen[0].0,
        "POST /raft/admin/transfer-leadership HTTP/1.1"
    );
    assert_eq!(two_seen[0].1, json!({ "group_id": "g0", "target": 11 }));
}

#[tokio::test]
async fn an_unexpected_status_is_an_error() {
    let (url, _) = pod(vec![(500, "{}")]).await;
    let client = client(vec![url.clone(), url]);
    let error = client.demote(0, 1).await.unwrap_err();
    assert!(error.message.contains("500"), "{}", error.message);
}

#[tokio::test]
async fn a_pod_that_cannot_be_reached_is_an_error() {
    let client = client(vec!["http://127.0.0.1:1".to_string(); 2]);
    let error = client.demote(0, 1).await.unwrap_err();
    assert!(error.message.contains("demote"), "{}", error.message);
}
