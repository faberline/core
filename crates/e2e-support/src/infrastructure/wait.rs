//! Waits for pods and for probe results, with a timeout.

use super::kube::Kube;
use serde_json::Value;
use std::time::{Duration, Instant};

/// Calls `probe` every 2 seconds until it succeeds or `timeout` passes.
///
/// # Panics
///
/// Panics with the last error when the timeout passes.
pub fn wait_for<T>(what: &str, timeout: Duration, probe: impl FnMut() -> Result<T, String>) -> T {
    wait_for_with(what, timeout, Duration::from_secs(2), probe)
}

fn wait_for_with<T>(
    what: &str,
    timeout: Duration,
    interval: Duration,
    mut probe: impl FnMut() -> Result<T, String>,
) -> T {
    let started = Instant::now();
    loop {
        match probe() {
            Ok(v) => return v,
            Err(e) => {
                if started.elapsed() >= timeout {
                    panic!(
                        "timed out after {}s waiting for {what}: {e}",
                        timeout.as_secs()
                    );
                }
                std::thread::sleep(interval);
            }
        }
    }
}

/// Waits until at least `expected` pods with `label` exist.
pub fn wait_pods_exist(kube: &Kube, label: &str, expected: usize, timeout: Duration) {
    wait_for(
        &format!("{expected} pod(s) with label {label}"),
        timeout,
        || pods_exist_error(&kube.try_pods(label)?, expected).map_or(Ok(()), Err),
    );
}

/// Waits until at least `expected` pods with `label` are ready.
pub fn wait_ready(kube: &Kube, label: &str, expected: usize, timeout: Duration) {
    wait_for(
        &format!("{expected} ready pod(s) with label {label}"),
        timeout,
        || pods_ready_error(&kube.try_pods(label)?, expected).map_or(Ok(()), Err),
    );
}

fn pods_exist_error(pods: &Value, expected: usize) -> Option<String> {
    let count = pods["items"].as_array().map_or(0, Vec::len);
    (count < expected).then(|| format!("found {count}"))
}

fn pods_ready_error(pods: &Value, expected: usize) -> Option<String> {
    let ready = pods["items"]
        .as_array()
        .map_or(0, |items| items.iter().filter(|pod| is_ready(pod)).count());
    (ready < expected).then(|| format!("{ready} of {expected} ready"))
}

fn is_ready(pod: &Value) -> bool {
    pod["status"]["conditions"]
        .as_array()
        .is_some_and(|conditions| {
            conditions
                .iter()
                .any(|c| c["type"] == "Ready" && c["status"] == "True")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::Cell;
    use std::time::Duration;

    #[test]
    fn wait_for_with_returns_the_first_success() {
        let v = wait_for_with(
            "x",
            Duration::from_secs(1),
            Duration::from_millis(5),
            || Ok::<_, String>(7),
        );
        assert_eq!(v, 7);
    }

    #[test]
    fn wait_for_with_retries_until_the_probe_succeeds() {
        let calls = Cell::new(0);
        let v = wait_for_with(
            "x",
            Duration::from_secs(5),
            Duration::from_millis(1),
            || {
                calls.set(calls.get() + 1);
                if calls.get() < 3 {
                    Err("not yet".to_string())
                } else {
                    Ok("done")
                }
            },
        );
        assert_eq!(v, "done");
        assert_eq!(calls.get(), 3);
    }

    #[test]
    #[should_panic(expected = "waiting for things: not yet")]
    fn wait_for_with_times_out_with_the_last_error() {
        wait_for_with::<()>(
            "things",
            Duration::from_millis(50),
            Duration::from_millis(10),
            || Err("not yet".to_string()),
        );
    }

    #[test]
    fn wait_for_with_runs_the_probe_once_when_the_timeout_is_zero() {
        let v = wait_for_with("x", Duration::ZERO, Duration::from_millis(1), || {
            Ok::<_, String>(1)
        });
        assert_eq!(v, 1);
    }

    #[test]
    fn pods_exist_error_counts_items() {
        let pods = json!({"items": [{}, {}]});
        assert_eq!(pods_exist_error(&pods, 2), None);
        assert_eq!(pods_exist_error(&pods, 3), Some("found 2".to_string()));
        assert_eq!(pods_exist_error(&json!({}), 1), Some("found 0".to_string()));
    }

    #[test]
    fn pods_ready_error_counts_ready_pods() {
        let pods = json!({"items": [
            {"status": {"conditions": [{"type": "Ready", "status": "True"}]}},
            {"status": {"conditions": [{"type": "Ready", "status": "False"}]}}
        ]});
        assert_eq!(pods_ready_error(&pods, 1), None);
        assert_eq!(pods_ready_error(&pods, 2), Some("1 of 2 ready".to_string()));
    }
}
