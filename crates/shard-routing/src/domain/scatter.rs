//! Scatter a call to several shards in parallel and merge the answers with the app's hook.

use std::thread;

use crate::domain::request_routing::RequestRouting;

/// One shard that failed during a scatter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScatterFailure<E> {
    /// The shard that failed.
    pub shard: u32,
    /// What the call returned.
    pub error: E,
}

/// Call `call(shard)` for every shard in `shards`, in parallel, one thread each.
///
/// A shard that fails or has nothing to contribute does not fail the scatter while
/// another shard answers. The answers come back in the order of `shards`.
/// Only when every shard fails is the error returned, with one failure per shard in
/// the order of `shards`. An empty `shards` gives `Ok(vec![])`.
pub fn scatter<T, E, F>(shards: &[u32], call: F) -> Result<Vec<T>, Vec<ScatterFailure<E>>>
where
    T: Send,
    E: Send,
    F: Fn(u32) -> Result<T, E> + Sync,
{
    let results: Vec<Result<T, E>> = thread::scope(|scope| {
        let handles: Vec<_> = shards
            .iter()
            .map(|&shard| {
                let call = &call;
                scope.spawn(move || call(shard))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| match handle.join() {
                Ok(result) => result,
                Err(panic) => std::panic::resume_unwind(panic),
            })
            .collect()
    });

    let mut answers = Vec::new();
    let mut failures = Vec::new();
    for (&shard, result) in shards.iter().zip(results) {
        match result {
            Ok(answer) => answers.push(answer),
            Err(error) => failures.push(ScatterFailure { shard, error }),
        }
    }
    if answers.is_empty() && !failures.is_empty() {
        return Err(failures);
    }
    Ok(answers)
}

/// [`scatter`], then `routing.merge` on the answers.
pub fn scatter_merge<R, E, F>(
    routing: &R,
    shards: &[u32],
    call: F,
) -> Result<R::Response, Vec<ScatterFailure<E>>>
where
    R: RequestRouting,
    R::Response: Send,
    E: Send,
    F: Fn(u32) -> Result<R::Response, E> + Sync,
{
    scatter(shards, call).map(|parts| routing.merge(parts))
}

#[cfg(test)]
mod tests;
