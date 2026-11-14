//! Step lines on stderr and evidence lines on stdout.

use std::time::{Duration, Instant};

/// Writes one progress line to stderr.
pub fn say(line: impl AsRef<str>) {
    eprintln!("[e2e] {}", line.as_ref());
}

/// Writes one evidence line to stdout.
pub fn evidence(suite: &str, step: &str, data: serde_json::Value) {
    println!("{}", crate::evidence_line(suite, step, &data));
}

/// A named step. `begin` logs the start; `end` logs the duration.
pub struct Step {
    suite: &'static str,
    name: String,
    started: Instant,
}

impl Step {
    /// Starts a step and logs its begin line.
    pub fn begin(suite: &'static str, name: impl Into<String>) -> Self {
        let name = name.into();
        say(format!("step {suite}/{name}: begin"));
        Self {
            suite,
            name,
            started: Instant::now(),
        }
    }

    /// Ends the step and logs its duration.
    pub fn end(self) {
        say(step_end_line(
            self.suite,
            &self.name,
            self.started.elapsed(),
        ));
    }
}

fn step_end_line(suite: &str, name: &str, elapsed: Duration) -> String {
    format!("step {suite}/{name}: done in {}ms", elapsed.as_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_end_line_has_the_exact_shape() {
        assert_eq!(
            step_end_line("lumen", "restore", std::time::Duration::from_millis(1500)),
            "step lumen/restore: done in 1500ms"
        );
    }
}
