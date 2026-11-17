use super::*;
use crate::domain::request_routing::RouteDecision;

struct SumRouting;

impl RequestRouting for SumRouting {
    type Request = Option<u32>;
    type Response = u32;

    fn classify(&self, request: &Option<u32>) -> RouteDecision {
        match request {
            Some(shard) => RouteDecision::Owner(*shard),
            None => RouteDecision::Scatter,
        }
    }

    fn merge(&self, parts: Vec<u32>) -> u32 {
        parts.into_iter().sum()
    }
}

#[test]
fn a_scatter_over_three_shards_returns_the_merged_result() {
    let merged = scatter_merge(&SumRouting, &[0, 1, 2], |shard| {
        Ok::<u32, String>(10 * (shard + 1))
    });
    assert_eq!(merged, Ok(60));
}

#[test]
fn the_answers_come_back_in_the_order_of_the_shards() {
    let answers = scatter(&[2, 0, 1], |shard| Ok::<u32, String>(shard + 100)).unwrap();
    assert_eq!(answers, vec![102, 100, 101]);
}

#[test]
fn one_failing_shard_does_not_fail_the_scatter_when_another_answers() {
    let merged = scatter_merge(&SumRouting, &[0, 1, 2], |shard| {
        if shard == 1 {
            Err("not found".to_string())
        } else {
            Ok(10 * (shard + 1))
        }
    });
    assert_eq!(merged, Ok(40));
}

#[test]
fn all_shards_failing_returns_the_error() {
    let merged = scatter_merge(&SumRouting, &[0, 1, 2], |shard| {
        Err::<u32, String>(format!("shard {shard} down"))
    });
    assert_eq!(
        merged,
        Err(vec![
            ScatterFailure {
                shard: 0,
                error: "shard 0 down".to_string()
            },
            ScatterFailure {
                shard: 1,
                error: "shard 1 down".to_string()
            },
            ScatterFailure {
                shard: 2,
                error: "shard 2 down".to_string()
            },
        ])
    );
}

#[test]
fn an_empty_shard_list_merges_no_answers() {
    let merged = scatter_merge(&SumRouting, &[], |_| {
        Err::<u32, String>("unused".to_string())
    });
    assert_eq!(merged, Ok(0));
}

#[test]
fn the_classify_hook_names_the_route() {
    assert_eq!(SumRouting.classify(&Some(2)), RouteDecision::Owner(2));
    assert_eq!(SumRouting.classify(&None), RouteDecision::Scatter);
}
