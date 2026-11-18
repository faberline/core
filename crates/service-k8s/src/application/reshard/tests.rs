use super::*;

use crate::domain::capacity::ShardSplitPolicy;
use crate::domain::reshard::{BucketDataError, CopyPass, CopyReport, ReshardStep, ReshardWait};
use crate::domain::shard_map::{PodShardReport, ShardMap, ShardMapError};

type Log = std::rc::Rc<std::cell::RefCell<Vec<String>>>;

fn new_log() -> Log {
    std::rc::Rc::new(std::cell::RefCell::new(Vec::new()))
}

struct FakeData {
    log: Log,
    fail_on: Option<(&'static str, u32)>,
    usage: u64,
    other_usage: u64,
}

fn data(log: &Log) -> FakeData {
    FakeData {
        log: log.clone(),
        fail_on: None,
        usage: 500,
        other_usage: 10,
    }
}

impl FakeData {
    fn record(&self, entry: String, name: &str, bucket: u32) -> Result<(), BucketDataError> {
        self.log.borrow_mut().push(entry);
        if self.fail_on == Some((name, bucket)) {
            return Err(BucketDataError::Refused {
                detail: "boom".to_string(),
            });
        }
        Ok(())
    }
}

impl BucketData for FakeData {
    fn usage_bytes(&self, shard: u32) -> Result<u64, BucketDataError> {
        Ok(if shard == 0 {
            self.usage
        } else {
            self.other_usage
        })
    }

    fn fence(&self, shard: u32, bucket: u32) -> Result<(), BucketDataError> {
        self.record(format!("fence:{shard}:{bucket}"), "fence", bucket)
    }

    fn copy_pass(
        &self,
        from: u32,
        to: u32,
        bucket: u32,
        pass: CopyPass,
    ) -> Result<CopyReport, BucketDataError> {
        let name = match pass {
            CopyPass::First => "copy1",
            CopyPass::Second => "copy2",
        };
        self.record(format!("{name}:{from}:{to}:{bucket}"), name, bucket)?;
        Ok(CopyReport { keys: 1, bytes: 1 })
    }

    fn prune(&self, shard: u32, bucket: u32) -> Result<(), BucketDataError> {
        self.record(format!("prune:{shard}:{bucket}"), "prune", bucket)
    }

    fn checkpoint(&self, shard: u32) -> Result<(), BucketDataError> {
        self.log.borrow_mut().push(format!("checkpoint:{shard}"));
        Ok(())
    }

    fn unfence(&self, shard: u32, bucket: u32) -> Result<(), BucketDataError> {
        self.log
            .borrow_mut()
            .push(format!("unfence:{shard}:{bucket}"));
        Ok(())
    }
}

struct FakeCluster {
    log: Log,
    state: Option<ReshardState>,
    shard_count: u32,
    map: Option<ShardMap>,
    pods: Vec<PodShardReport>,
    pods_follow_map: bool,
    crash_after_state: Option<ReshardStep>,
    fail_pod_reports: bool,
}

impl ReshardCluster for FakeCluster {
    fn read_state(&mut self) -> Result<Option<ReshardState>, ReshardError> {
        Ok(self.state.clone())
    }

    fn write_state(&mut self, state: &ReshardState) -> Result<(), ReshardError> {
        self.state = Some(state.clone());
        self.log
            .borrow_mut()
            .push(format!("state:{:?}", state.step));
        if self.crash_after_state == Some(state.step) {
            return Err(ReshardError::Cluster("crash".to_string()));
        }
        Ok(())
    }

    fn clear_state(&mut self) -> Result<(), ReshardError> {
        self.state = None;
        self.log.borrow_mut().push("clear".to_string());
        Ok(())
    }

    fn shard_count(&mut self) -> Result<u32, ReshardError> {
        Ok(self.shard_count)
    }

    fn set_shard_count(&mut self, shard_count: u32) -> Result<(), ReshardError> {
        self.shard_count = shard_count;
        self.log.borrow_mut().push(format!("count:{shard_count}"));
        Ok(())
    }

    fn read_shard_map(&mut self) -> Result<Option<ShardMap>, ReshardError> {
        Ok(self.map.clone())
    }

    fn write_shard_map(&mut self, map: &ShardMap) -> Result<(), ReshardError> {
        self.map = Some(map.clone());
        self.log.borrow_mut().push(format!("map:v{}", map.version));
        if self.pods_follow_map {
            for pod in &mut self.pods {
                pod.map_version = map.version;
            }
        }
        Ok(())
    }

    fn pod_reports(&mut self) -> Result<Vec<PodShardReport>, ReshardError> {
        if self.fail_pod_reports {
            return Err(ReshardError::Cluster("pods down".to_string()));
        }
        Ok(self.pods.clone())
    }

    fn create_shard(&mut self, shard: u32) -> Result<(), ReshardError> {
        self.log.borrow_mut().push(format!("create:{shard}"));
        Ok(())
    }

    fn delete_shard(&mut self, shard: u32) -> Result<(), ReshardError> {
        self.log.borrow_mut().push(format!("delete:{shard}"));
        Ok(())
    }
}

fn config() -> ReshardConfig {
    ReshardConfig {
        bucket_count: 8,
        policy: ShardSplitPolicy {
            split_threshold_bytes: 100,
            max_shards: None,
        },
    }
}

fn cluster(log: &Log) -> FakeCluster {
    FakeCluster {
        log: log.clone(),
        state: None,
        shard_count: 2,
        map: Some(ShardMap::balanced(8, 2)),
        pods: vec![
            PodShardReport {
                shard: 0,
                buckets: vec![0, 2, 4, 6],
                map_version: 1,
            },
            PodShardReport {
                shard: 1,
                buckets: vec![1, 3, 5, 7],
                map_version: 1,
            },
        ],
        pods_follow_map: true,
        crash_after_state: None,
        fail_pod_reports: false,
    }
}

/// The entries of the call log that name a step, in order, without repeats in a row.
fn kinds(log: &Log) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for entry in log.borrow().iter() {
        let kind = entry.split(':').next().unwrap();
        let wanted = ["fence", "copy1", "copy2", "prune", "checkpoint", "map"];
        if wanted.contains(&kind) && out.last().map(String::as_str) != Some(kind) {
            out.push(kind.to_string());
        }
    }
    out
}

#[test]
fn a_full_split_runs_the_steps_in_order_and_raises_the_map_version() {
    let log = new_log();
    let mut c = cluster(&log);
    let outcome = run_reshard(&mut c, &data(&log), config()).unwrap();
    assert_eq!(outcome, ReshardOutcome::Completed { map_version: 2 });
    assert_eq!(
        kinds(&log),
        ["fence", "copy1", "copy2", "prune", "checkpoint", "map"]
    );
    assert_eq!(c.shard_count, 3);
    let map = c.map.as_ref().unwrap();
    assert_eq!(map.version, 2);
    assert_eq!(map.buckets, vec![0, 1, 0, 1, 2, 1, 2, 1]);
    assert!(c.state.is_none());
    let entries = log.borrow();
    let create = entries.iter().position(|e| e == "create:2").unwrap();
    let first_fence = entries.iter().position(|e| e.starts_with("fence")).unwrap();
    assert!(create < first_fence);
    assert!(entries.contains(&"count:3".to_string()));
}

#[test]
fn a_new_controller_continues_at_copy_second_after_a_failover() {
    let log = new_log();
    let mut c = cluster(&log);
    c.crash_after_state = Some(ReshardStep::CopySecond);
    let first = run_reshard(&mut c, &data(&log), config());
    assert_eq!(first, Err(ReshardError::Cluster("crash".to_string())));
    assert_eq!(c.state.as_ref().unwrap().step, ReshardStep::CopySecond);

    c.crash_after_state = None;
    log.borrow_mut().clear();
    let second = run_reshard(&mut c, &data(&log), config());
    assert_eq!(second, Ok(ReshardOutcome::Completed { map_version: 2 }));
    assert_eq!(kinds(&log), ["copy2", "prune", "checkpoint", "map"]);
    assert_eq!(c.shard_count, 3);
}

#[test]
fn a_copy_error_at_copy_second_rolls_back_and_keeps_the_shard_count() {
    let log = new_log();
    let mut c = cluster(&log);
    let mut failing = data(&log);
    failing.fail_on = Some(("copy2", 4));
    let outcome = run_reshard(&mut c, &failing, config()).unwrap();
    assert_eq!(
        outcome,
        ReshardOutcome::RolledBack {
            from_step: ReshardStep::CopySecond,
            error: "the service refused the step: boom".to_string(),
        }
    );
    assert_eq!(c.shard_count, 2);
    assert!(c.state.is_none());
    let entries = log.borrow();
    assert!(entries.contains(&"delete:2".to_string()));
    assert!(entries.contains(&"unfence:0:4".to_string()));
    assert!(entries.contains(&"unfence:0:6".to_string()));
    assert!(!entries.contains(&"count:3".to_string()));
    assert!(!entries.iter().any(|e| e.starts_with("map:")));
    assert_eq!(c.map.as_ref().unwrap().version, 1);
    assert_eq!(c.map.as_ref().unwrap(), &ShardMap::balanced(8, 2));
}

#[test]
fn a_cluster_read_error_keeps_the_stored_state_for_the_next_leader() {
    let log = new_log();
    let mut c = cluster(&log);
    c.shard_count = 3;
    c.fail_pod_reports = true;
    let stored = ReshardState {
        step: ReshardStep::Converge,
        from_shard: 0,
        to_shard: 2,
        buckets: vec![4, 6],
        map_version: 2,
    };
    c.state = Some(stored.clone());
    let outcome = run_reshard(&mut c, &data(&log), config());
    assert_eq!(outcome, Err(ReshardError::Cluster("pods down".to_string())));
    assert_eq!(c.state, Some(stored));
    let entries = log.borrow();
    assert!(!entries.iter().any(|e| e.starts_with("delete:")));
    assert!(!entries.iter().any(|e| e.starts_with("unfence:")));
}

#[test]
fn a_lost_shard_map_is_rebuilt_from_pod_reports_and_the_flow_continues() {
    let log = new_log();
    let mut c = cluster(&log);
    c.map = None;
    c.shard_count = 2;
    c.state = Some(ReshardState {
        step: ReshardStep::Cutover,
        from_shard: 0,
        to_shard: 2,
        buckets: vec![4, 6],
        map_version: 2,
    });
    let outcome = run_reshard(&mut c, &data(&log), config());
    assert_eq!(outcome, Ok(ReshardOutcome::Completed { map_version: 2 }));
    let map = c.map.as_ref().unwrap();
    assert_eq!(map.version, 2);
    assert_eq!(map.buckets, vec![0, 1, 0, 1, 2, 1, 2, 1]);
    let entries = log.borrow();
    let rebuilt = entries.iter().position(|e| e == "map:v1").unwrap();
    let raised = entries.iter().position(|e| e == "map:v2").unwrap();
    assert!(rebuilt < raised);
    assert_eq!(c.shard_count, 3);
}

#[test]
fn a_lost_map_and_no_pod_reports_is_an_error() {
    let log = new_log();
    let mut c = cluster(&log);
    c.map = None;
    c.pods = Vec::new();
    c.state = Some(ReshardState {
        step: ReshardStep::Cutover,
        from_shard: 0,
        to_shard: 2,
        buckets: vec![4, 6],
        map_version: 2,
    });
    let outcome = run_reshard(&mut c, &data(&log), config());
    assert_eq!(outcome, Err(ReshardError::Map(ShardMapError::NoReports)));
}

#[test]
fn a_hold_decision_changes_nothing() {
    let log = new_log();
    let mut c = cluster(&log);
    let mut quiet = data(&log);
    quiet.usage = 100;
    quiet.other_usage = 10;
    let outcome = run_reshard(&mut c, &quiet, config()).unwrap();
    assert_eq!(outcome, ReshardOutcome::NoSplit);
    assert!(c.state.is_none());
    assert_eq!(c.shard_count, 2);
    assert!(!log.borrow().iter().any(|e| e.starts_with("create:")));
}

#[test]
fn pods_on_the_old_map_version_make_the_controller_wait() {
    let log = new_log();
    let mut c = cluster(&log);
    c.pods_follow_map = false;
    let first = run_reshard(&mut c, &data(&log), config()).unwrap();
    assert_eq!(
        first,
        ReshardOutcome::Waiting(ReshardWait::PodsOnOldMapVersion { stale: 2 })
    );
    assert_eq!(c.state.as_ref().unwrap().step, ReshardStep::Converge);
    assert_eq!(c.shard_count, 3);

    for pod in c.pods.iter_mut() {
        pod.map_version = 2;
    }
    let second = run_reshard(&mut c, &data(&log), config());
    assert_eq!(second, Ok(ReshardOutcome::Completed { map_version: 2 }));
    assert!(c.state.is_none());
}

#[test]
fn a_shard_with_one_bucket_is_not_split() {
    let log = new_log();
    let mut c = cluster(&log);
    c.map = Some(ShardMap {
        version: 1,
        buckets: vec![0, 1, 1, 1, 1, 1, 1, 1],
    });
    let outcome = run_reshard(&mut c, &data(&log), config()).unwrap();
    assert_eq!(outcome, ReshardOutcome::NoSplit);
    assert!(!log.borrow().iter().any(|e| e.starts_with("create:")));
}
