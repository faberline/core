//! Performance gate: a caller gives the thresholds and the measured results.
//! The crate holds no threshold of its own.

/// A bound on one metric.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bound {
    /// The value must be less than or equal to the limit.
    AtMost(f64),
    /// The value must be greater than or equal to the limit.
    AtLeast(f64),
}

/// One threshold: a metric and its bound.
#[derive(Debug, Clone, PartialEq)]
pub struct Threshold {
    /// The metric name, as the measurement names it.
    pub metric: String,
    /// The bound the measured value must meet.
    pub bound: Bound,
}

impl Threshold {
    /// A threshold that the measured value must not exceed.
    pub fn at_most(metric: &str, limit: f64) -> Self {
        Self {
            metric: metric.to_string(),
            bound: Bound::AtMost(limit),
        }
    }

    /// A threshold that the measured value must not fall below.
    pub fn at_least(metric: &str, limit: f64) -> Self {
        Self {
            metric: metric.to_string(),
            bound: Bound::AtLeast(limit),
        }
    }
}

/// One measured result.
#[derive(Debug, Clone, PartialEq)]
pub struct Measurement {
    /// The metric name.
    pub metric: String,
    /// The measured value.
    pub value: f64,
}

impl Measurement {
    /// A result for `metric` with `value`.
    pub fn new(metric: &str, value: f64) -> Self {
        Self {
            metric: metric.to_string(),
            value,
        }
    }
}

/// A set of thresholds that measured results must meet.
#[derive(Debug, Clone, PartialEq)]
pub struct PerfGate {
    thresholds: Vec<Threshold>,
}

impl PerfGate {
    /// A gate with the given thresholds.
    pub fn new(thresholds: Vec<Threshold>) -> Self {
        Self { thresholds }
    }

    /// One message per violation, in threshold order. An empty list means the gate passes.
    /// A gate with no thresholds never passes.
    pub fn violations(&self, results: &[Measurement]) -> Vec<String> {
        if self.thresholds.is_empty() {
            return vec!["perf gate has no thresholds".to_string()];
        }
        let mut out = Vec::new();
        for threshold in &self.thresholds {
            let metric = threshold.metric.as_str();
            let Some(found) = results.iter().find(|m| m.metric == metric) else {
                out.push(format!("perf gate: metric `{metric}` has no result"));
                continue;
            };
            let value = found.value;
            match threshold.bound {
                Bound::AtMost(limit) => {
                    if value.is_nan() || limit.is_nan() || value > limit {
                        out.push(format!(
                            "perf gate: metric `{metric}` is {value} but must be at most {limit}"
                        ));
                    }
                }
                Bound::AtLeast(limit) => {
                    if value.is_nan() || limit.is_nan() || value < limit {
                        out.push(format!(
                            "perf gate: metric `{metric}` is {value} but must be at least {limit}"
                        ));
                    }
                }
            }
        }
        out
    }

    /// Panic with every violation, joined by `"; "`, when the gate does not pass.
    pub fn assert_within(&self, results: &[Measurement]) {
        let violations = self.violations(results);
        if !violations.is_empty() {
            panic!("{}", violations.join("; "));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate() -> PerfGate {
        PerfGate::new(vec![
            Threshold::at_most("p99_ms", 10.0),
            Threshold::at_least("rps", 1000.0),
        ])
    }

    #[test]
    fn within_threshold_passes() {
        let results = [
            Measurement::new("p99_ms", 9.5),
            Measurement::new("rps", 1200.0),
        ];
        assert!(gate().violations(&results).is_empty());
        gate().assert_within(&results);
    }

    #[test]
    fn value_equal_to_the_limit_passes() {
        let g = PerfGate::new(vec![
            Threshold::at_most("p99_ms", 10.0),
            Threshold::at_least("rps", 1000.0),
        ]);
        let results = [
            Measurement::new("p99_ms", 10.0),
            Measurement::new("rps", 1000.0),
        ];
        assert!(g.violations(&results).is_empty());
    }

    #[test]
    fn at_most_violation_names_metric_and_both_values() {
        let g = PerfGate::new(vec![Threshold::at_most("p99_ms", 10.0)]);
        let v = g.violations(&[Measurement::new("p99_ms", 12.5)]);
        assert_eq!(
            v,
            vec!["perf gate: metric `p99_ms` is 12.5 but must be at most 10".to_string()]
        );
    }

    #[test]
    fn at_least_violation_names_metric_and_both_values() {
        let g = PerfGate::new(vec![Threshold::at_least("rps", 1000.0)]);
        let v = g.violations(&[Measurement::new("rps", 800.0)]);
        assert_eq!(
            v,
            vec!["perf gate: metric `rps` is 800 but must be at least 1000".to_string()]
        );
    }

    #[test]
    fn missing_result_is_a_violation() {
        let g = PerfGate::new(vec![Threshold::at_most("p99_ms", 10.0)]);
        assert_eq!(
            g.violations(&[]),
            vec!["perf gate: metric `p99_ms` has no result".to_string()]
        );
    }

    #[test]
    fn nan_result_is_a_violation() {
        let g = PerfGate::new(vec![Threshold::at_most("p99_ms", 10.0)]);
        let v = g.violations(&[Measurement::new("p99_ms", f64::NAN)]);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].contains("metric `p99_ms` "), "{v:?}");
    }

    #[test]
    fn empty_gate_never_passes() {
        let g = PerfGate::new(vec![]);
        assert_eq!(
            g.violations(&[Measurement::new("p99_ms", 1.0)]),
            vec!["perf gate has no thresholds".to_string()]
        );
    }

    #[test]
    fn all_violations_are_listed_in_order() {
        let g = PerfGate::new(vec![
            Threshold::at_most("a", 1.0),
            Threshold::at_most("b", 1.0),
        ]);
        let v = g.violations(&[Measurement::new("a", 2.0), Measurement::new("b", 3.0)]);
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(v[0].contains("`a`"), "{v:?}");
        assert!(v[1].contains("`b`"), "{v:?}");
    }

    #[test]
    #[should_panic(expected = "metric `p99_ms` is 12.5 but must be at most 10")]
    fn assert_within_panics_with_the_messages() {
        let g = PerfGate::new(vec![Threshold::at_most("p99_ms", 10.0)]);
        g.assert_within(&[Measurement::new("p99_ms", 12.5)]);
    }
}
