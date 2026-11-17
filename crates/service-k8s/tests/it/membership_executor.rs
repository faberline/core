#![cfg(feature = "controller")]

//! The membership step executor: the planner runs through the port, a step
//! runs only while the Lease is held, and a new executor resumes from the
//! observed state.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use kube::client::Body;
use kube::Client;
use serde_json::{json, Value};

use service_k8s::{
    run_membership_step, run_membership_step_as_leader, DesiredMembership, Election,
    KubeMembershipExecutor, MemberRole, MemberState, MembershipError, MembershipExecutor,
    MembershipNaming, MembershipStep, ObservedMembership, Progress, RaftAdminClient,
    ReadScalingMode, ShardMembership, StepResult,
};

const NAMESPACE: &str = "acme";

type Log = Arc<Mutex<Vec<String>>>;
type Calls = Arc<Mutex<Vec<MembershipStep>>>;

/// A fake apiserver that records `METHOD path` for everything it is asked.
fn fake_apiserver(
    route: impl Fn(&str, &str) -> (u16, Value) + Send + Sync + 'static,
) -> (Client, Log) {
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let seen = log.clone();
    let route = Arc::new(route);
    let service = tower::service_fn(move |req: http::Request<Body>| {
        let seen = seen.clone();
        let route = route.clone();
        async move {
            let method = req.method().to_string();
            let path = req.uri().path().to_string();
            let _ = req.into_body().collect_bytes().await;
            let (code, response) = route(&method, &path);
            seen.lock().unwrap().push(format!("{method} {path}"));
            Ok::<_, std::convert::Infallible>(
                http::Response::builder()
                    .status(code)
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&response).unwrap()))
                    .unwrap(),
            )
        }
    });
    (Client::new(service, NAMESPACE), log)
}

/// A fake cluster for shard 0. Every port call is recorded, then applied to
/// the state unless it is an `add_learner` that is told to wait.
struct Cluster {
    state: Arc<Mutex<ObservedMembership>>,
    calls: Calls,
    waiting: Option<&'static str>,
}

impl Cluster {
    fn new(state: Arc<Mutex<ObservedMembership>>, calls: Calls) -> Self {
        Self {
            state,
            calls,
            waiting: None,
        }
    }

    fn observe(&self) -> ObservedMembership {
        self.state.lock().unwrap().clone()
    }

    fn record(&self, step: MembershipStep) {
        self.calls.lock().unwrap().push(step);
    }

    fn with_shard(&self, change: impl FnOnce(&mut ShardMembership)) {
        let mut state = self.state.lock().unwrap();
        change(state.shards.get_mut(&0).unwrap());
    }
}

impl MembershipExecutor for Cluster {
    fn scale_pods<'a>(
        &'a self,
        shard: u32,
        replicas: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            self.record(MembershipStep::ScalePods { shard, replicas });
            self.with_shard(|s| s.pods = replicas);
            Ok(StepResult::Applied)
        })
    }

    fn add_learner<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            self.record(MembershipStep::AddLearner { shard, replica });
            if let Some(reason) = self.waiting {
                return Ok(StepResult::Waiting(reason.to_string()));
            }
            self.with_shard(|s| {
                s.members.insert(
                    replica,
                    MemberState {
                        role: MemberRole::Learner,
                        caught_up: true,
                    },
                );
                s.pvcs.insert(replica);
            });
            Ok(StepResult::Applied)
        })
    }

    fn promote<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            self.record(MembershipStep::Promote { shard, replica });
            self.with_shard(|s| {
                if let Some(m) = s.members.get_mut(&replica) {
                    m.role = MemberRole::Voter;
                }
            });
            Ok(StepResult::Applied)
        })
    }

    fn transfer_leadership<'a>(
        &'a self,
        shard: u32,
        from: u32,
        to: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            self.record(MembershipStep::TransferLeadership { shard, from, to });
            self.with_shard(|s| s.leader = Some(to));
            Ok(StepResult::Applied)
        })
    }

    fn demote<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            self.record(MembershipStep::Demote { shard, replica });
            self.with_shard(|s| {
                if let Some(m) = s.members.get_mut(&replica) {
                    m.role = MemberRole::Learner;
                }
            });
            Ok(StepResult::Applied)
        })
    }

    fn remove_member<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            self.record(MembershipStep::RemoveMember { shard, replica });
            self.with_shard(|s| {
                s.members.remove(&replica);
            });
            Ok(StepResult::Applied)
        })
    }

    fn delete_pvc<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            self.record(MembershipStep::DeletePvc { shard, replica });
            self.with_shard(|s| {
                s.pvcs.remove(&replica);
            });
            Ok(StepResult::Applied)
        })
    }
}

fn one_shard(
    members: &[(u32, MemberRole)],
    leader: Option<u32>,
    pods: u32,
    pvcs: &[u32],
) -> ObservedMembership {
    let members: BTreeMap<u32, MemberState> = members
        .iter()
        .map(|(id, role)| {
            (
                *id,
                MemberState {
                    role: *role,
                    caught_up: true,
                },
            )
        })
        .collect();
    let shard = ShardMembership {
        members,
        leader,
        pods,
        pvcs: pvcs.iter().copied().collect::<BTreeSet<u32>>(),
    };
    ObservedMembership {
        shards: BTreeMap::from([(0, shard)]),
    }
}

fn want(replicas: u32, voters: u32) -> DesiredMembership {
    DesiredMembership {
        replicas_per_shard: replicas,
        voters,
        read_scaling: ReadScalingMode::None,
    }
}

/// Shorthand for the planner steps of shard 0.
fn step_scale(replicas: u32) -> MembershipStep {
    MembershipStep::ScalePods { shard: 0, replicas }
}

fn step_add(replica: u32) -> MembershipStep {
    MembershipStep::AddLearner { shard: 0, replica }
}

fn step_promote(replica: u32) -> MembershipStep {
    MembershipStep::Promote { shard: 0, replica }
}

fn step_demote(replica: u32) -> MembershipStep {
    MembershipStep::Demote { shard: 0, replica }
}

fn step_remove(replica: u32) -> MembershipStep {
    MembershipStep::RemoveMember { shard: 0, replica }
}

fn step_pvc(replica: u32) -> MembershipStep {
    MembershipStep::DeletePvc { shard: 0, replica }
}

fn step_transfer(from: u32, to: u32) -> MembershipStep {
    MembershipStep::TransferLeadership { shard: 0, from, to }
}

/// Runs steps until `Done` (at most 30) and returns what each step answered.
async fn drive(cluster: &Cluster, desired: &DesiredMembership) -> Vec<Progress> {
    let mut out = Vec::new();
    for _ in 0..30 {
        let progress = run_membership_step(&cluster.observe(), desired, cluster)
            .await
            .unwrap();
        let done = progress == Progress::Done;
        out.push(progress);
        if done {
            return out;
        }
    }
    panic!("membership did not converge within 30 steps");
}

fn naming() -> MembershipNaming {
    MembershipNaming {
        statefulset: Box::new(|s| format!("svc-s{s}")),
        pvc: Box::new(|s, r| format!("data-svc-s{s}-{r}")),
    }
}

/// Records each Raft call as `"<name> <args>"`.
#[derive(Default)]
struct FakeAdmin {
    log: Log,
}

impl FakeAdmin {
    fn push(&self, line: String) {
        self.log.lock().unwrap().push(line);
    }
}

impl RaftAdminClient for FakeAdmin {
    fn add_learner<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            self.push(format!("add_learner {shard} {replica}"));
            Ok(StepResult::Applied)
        })
    }

    fn promote<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            self.push(format!("promote {shard} {replica}"));
            Ok(StepResult::Waiting("learner not caught up".to_string()))
        })
    }

    fn transfer_leadership<'a>(
        &'a self,
        shard: u32,
        from: u32,
        to: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            self.push(format!("transfer {shard} {from} {to}"));
            Ok(StepResult::Applied)
        })
    }

    fn demote<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            self.push(format!("demote {shard} {replica}"));
            Ok(StepResult::Applied)
        })
    }

    fn remove_member<'a>(
        &'a self,
        shard: u32,
        replica: u32,
    ) -> BoxFuture<'a, Result<StepResult, MembershipError>> {
        Box::pin(async move {
            self.push(format!("remove {shard} {replica}"));
            Ok(StepResult::Applied)
        })
    }
}

#[tokio::test]
async fn scale_up_1_to_3_calls_scale_pods_then_add_learner_then_promote() {
    let state = Arc::new(Mutex::new(one_shard(
        &[(0, MemberRole::Voter)],
        Some(0),
        1,
        &[0],
    )));
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let cluster = Cluster::new(state, calls.clone());

    let progress = drive(&cluster, &want(3, 3)).await;

    let recorded = calls.lock().unwrap().clone();
    assert_eq!(
        recorded,
        vec![
            step_scale(2),
            step_add(1),
            step_promote(1),
            step_scale(3),
            step_add(2),
            step_promote(2),
        ]
    );
    let mut expected = vec![Progress::Advanced; 6];
    expected.push(Progress::Done);
    assert_eq!(progress, expected);
}

#[tokio::test]
async fn scale_down_of_the_leader_transfers_first_and_deletes_the_pvc_last() {
    let state = Arc::new(Mutex::new(one_shard(
        &[
            (0, MemberRole::Voter),
            (1, MemberRole::Voter),
            (2, MemberRole::Voter),
        ],
        Some(2),
        3,
        &[0, 1, 2],
    )));
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let cluster = Cluster::new(state, calls.clone());

    drive(&cluster, &want(2, 2)).await;

    let recorded = calls.lock().unwrap().clone();
    assert_eq!(
        recorded,
        vec![
            step_transfer(2, 0),
            step_demote(2),
            step_remove(2),
            step_scale(2),
            step_pvc(2),
        ]
    );
}

#[tokio::test]
async fn a_new_executor_resumes_after_an_interrupted_scale_up() {
    let state = Arc::new(Mutex::new(one_shard(
        &[(0, MemberRole::Voter)],
        Some(0),
        1,
        &[0],
    )));
    let desired = want(2, 2);

    // Executor A runs two steps, then is dropped (the controller lost the process).
    let calls_a: Calls = Arc::new(Mutex::new(Vec::new()));
    let a = Cluster::new(state.clone(), calls_a.clone());
    for _ in 0..2 {
        run_membership_step(&a.observe(), &desired, &a)
            .await
            .unwrap();
    }
    assert_eq!(
        calls_a.lock().unwrap().clone(),
        vec![step_scale(2), step_add(1),]
    );
    drop(a);

    // Executor B reads the observed state again and continues with Promote.
    let calls_b: Calls = Arc::new(Mutex::new(Vec::new()));
    let b = Cluster::new(state.clone(), calls_b.clone());
    let progress = run_membership_step(&b.observe(), &desired, &b)
        .await
        .unwrap();

    assert_eq!(progress, Progress::Advanced);
    assert_eq!(calls_b.lock().unwrap().clone(), vec![step_promote(1)]);
    let observed = b.observe();
    assert_eq!(observed.shards[&0].members[&1].role, MemberRole::Voter);
}

#[tokio::test]
async fn a_waiting_executor_answers_waiting_and_changes_nothing() {
    let state = Arc::new(Mutex::new(one_shard(
        &[(0, MemberRole::Voter)],
        Some(0),
        2,
        &[0, 1],
    )));
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let mut cluster = Cluster::new(state.clone(), calls.clone());
    cluster.waiting = Some("learner not caught up");
    let start = cluster.observe();

    let progress = run_membership_step(&cluster.observe(), &want(2, 2), &cluster)
        .await
        .unwrap();

    assert_eq!(
        progress,
        Progress::Waiting("learner not caught up".to_string())
    );
    assert_eq!(cluster.observe(), start);
}

#[tokio::test]
async fn a_follower_runs_no_step() {
    let state = Arc::new(Mutex::new(one_shard(
        &[(0, MemberRole::Voter)],
        Some(0),
        1,
        &[0],
    )));
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let cluster = Cluster::new(state, calls.clone());
    let observed = cluster.observe();
    let desired = want(3, 3);
    let election = Election::new("replica-b".to_string());

    let progress = run_membership_step_as_leader(&election, &observed, &desired, &cluster)
        .await
        .unwrap();
    assert_eq!(progress, Progress::Waiting("not the leader".to_string()));
    assert!(calls.lock().unwrap().is_empty());

    election.is_leader.store(true, Ordering::Relaxed);
    let progress = run_membership_step_as_leader(&election, &observed, &desired, &cluster)
        .await
        .unwrap();
    assert_eq!(progress, Progress::Advanced);
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn the_adapter_scales_the_statefulset_through_the_scale_subresource() {
    let (client, log) = fake_apiserver(|method, path| {
        if method == "PATCH" && path.ends_with("/scale") {
            (
                200,
                json!({
                    "apiVersion": "autoscaling/v1",
                    "kind": "Scale",
                    "metadata": {"name": "svc-s0"},
                    "spec": {"replicas": 2}
                }),
            )
        } else {
            (500, json!({}))
        }
    });
    let exec = KubeMembershipExecutor::new(
        client,
        NAMESPACE.to_string(),
        FakeAdmin::default(),
        naming(),
    );

    let result = exec.scale_pods(0, 2).await.unwrap();

    assert_eq!(result, StepResult::Applied);
    assert_eq!(
        log.lock().unwrap().clone(),
        vec!["PATCH /apis/apps/v1/namespaces/acme/statefulsets/svc-s0/scale".to_string()]
    );
}

#[tokio::test]
async fn the_adapter_deletes_the_pvc_and_treats_not_found_as_done() {
    let (client, log) = fake_apiserver(|method, path| {
        if method != "DELETE" {
            return (500, json!({}));
        }
        match path {
            "/api/v1/namespaces/acme/persistentvolumeclaims/data-svc-s0-2" => (
                200,
                json!({"apiVersion": "v1", "kind": "Status", "status": "Success", "code": 200}),
            ),
            "/api/v1/namespaces/acme/persistentvolumeclaims/data-svc-s0-3" => (
                404,
                json!({
                    "apiVersion": "v1",
                    "kind": "Status",
                    "status": "Failure",
                    "reason": "NotFound",
                    "code": 404,
                    "message": "gone"
                }),
            ),
            _ => (
                500,
                json!({
                    "apiVersion": "v1",
                    "kind": "Status",
                    "status": "Failure",
                    "code": 500,
                    "message": "boom"
                }),
            ),
        }
    });
    let exec = KubeMembershipExecutor::new(
        client,
        NAMESPACE.to_string(),
        FakeAdmin::default(),
        naming(),
    );

    assert_eq!(exec.delete_pvc(0, 2).await.unwrap(), StepResult::Applied);
    assert_eq!(exec.delete_pvc(0, 3).await.unwrap(), StepResult::Applied);
    assert!(exec.delete_pvc(0, 4).await.is_err());

    assert_eq!(
        log.lock().unwrap().clone(),
        vec![
            "DELETE /api/v1/namespaces/acme/persistentvolumeclaims/data-svc-s0-2".to_string(),
            "DELETE /api/v1/namespaces/acme/persistentvolumeclaims/data-svc-s0-3".to_string(),
            "DELETE /api/v1/namespaces/acme/persistentvolumeclaims/data-svc-s0-4".to_string(),
        ]
    );
}

#[tokio::test]
async fn the_adapter_sends_raft_steps_to_the_admin_port_and_makes_no_kube_call() {
    let (client, kube_log) = fake_apiserver(|_, _| (500, json!({})));
    let admin = FakeAdmin::default();
    let admin_log = admin.log.clone();
    let exec = KubeMembershipExecutor::new(client, NAMESPACE.to_string(), admin, naming());

    assert_eq!(exec.add_learner(0, 1).await.unwrap(), StepResult::Applied);
    assert_eq!(
        exec.promote(0, 1).await.unwrap(),
        StepResult::Waiting("learner not caught up".to_string())
    );
    assert_eq!(
        exec.transfer_leadership(0, 2, 0).await.unwrap(),
        StepResult::Applied
    );
    assert_eq!(exec.demote(0, 2).await.unwrap(), StepResult::Applied);
    assert_eq!(exec.remove_member(0, 2).await.unwrap(), StepResult::Applied);

    assert_eq!(
        admin_log.lock().unwrap().clone(),
        vec![
            "add_learner 0 1".to_string(),
            "promote 0 1".to_string(),
            "transfer 0 2 0".to_string(),
            "demote 0 2".to_string(),
            "remove 0 2".to_string(),
        ]
    );
    assert!(kube_log.lock().unwrap().is_empty());
}
