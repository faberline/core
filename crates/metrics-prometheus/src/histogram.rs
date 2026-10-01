use std::fmt::Write;

use crate::instrument::Counter;

/// One upper bound of a [`Histogram`], carrying the same bound twice on
/// purpose: `le` is the Prometheus label exactly as it must appear in the
/// exposition, and `max` is that bound in the integer unit observations are
/// recorded in.
///
/// Deriving either side from the other would mean formatting or parsing a
/// decimal at scrape or observe time. The label is a *display* value an SLO
/// is written against (`"0.5"`, never `"0.500"`), and bucket assignment must
/// stay exact integer comparison — so both are stated, and a test pins that
/// they agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bucket<'a> {
    /// The Prometheus `le` label value, in the metric name's published unit.
    pub le: &'a str,
    /// The same bound in the integer unit passed to [`Histogram::observe`].
    pub max: u64,
}

impl<'a> Bucket<'a> {
    pub const fn new(le: &'a str, max: u64) -> Self {
        Self { le, max }
    }
}

/// A Prometheus histogram: per-bucket observation counts plus `_sum` and
/// `_count`, all lock-free integer atomics.
///
/// Observations are recorded in an integer *base unit* (microseconds,
/// milliseconds, bytes) and published in the unit the metric name promises,
/// via the `divisor` given to [`Histogram::render`]. Accumulating integers
/// and dividing once at scrape time keeps the hot path free of floats and
/// makes `_sum` exactly reproducible — a float accumulator would drift by an
/// amount that depends on the order observations happened to arrive in.
///
/// Buckets are stored exclusively (bucket `i` counts `(bounds[i-1],
/// bounds[i]]`) and made cumulative at render time, which is the form
/// Prometheus requires. Observations above the last bound are not stored in
/// an extra counter: the `+Inf` row is by definition the total count.
#[derive(Debug)]
pub struct Histogram {
    bounds: &'static [Bucket<'static>],
    counts: Vec<Counter>,
    sum: Counter,
    count: Counter,
}

impl Histogram {
    /// Build a histogram over `bounds`, which must be sorted ascending by
    /// `max` (assignment scans for the first bound an observation fits, so an
    /// unsorted list would silently mis-bucket). Allocates once; every later
    /// operation touches only atomics.
    pub fn new(bounds: &'static [Bucket<'static>]) -> Self {
        Self {
            bounds,
            counts: bounds.iter().map(|_| Counter::new()).collect(),
            sum: Counter::new(),
            count: Counter::new(),
        }
    }

    /// Record one observation of `value`, expressed in the base unit the
    /// bounds' `max` fields use.
    pub fn observe(&self, value: u64) {
        if let Some(index) = self.bounds.iter().position(|bound| value <= bound.max) {
            self.counts[index].incr();
        }
        self.sum.add(value);
        self.count.incr();
    }

    /// Total observations recorded — the `_count` series and the `+Inf` bucket.
    pub fn count(&self) -> u64 {
        self.count.get()
    }

    /// Render this histogram as the three series Prometheus expects:
    /// `<name>_bucket{le=…}` (cumulative, including `+Inf`), `<name>_sum`
    /// scaled by `divisor`, and `<name>_count`.
    ///
    /// `divisor` converts the base unit to the published unit and should be a
    /// power of ten (1000 to publish seconds from milliseconds). It is applied
    /// with integer division plus a zero-padded remainder, so the rendered sum
    /// is exact rather than float-rounded. A `divisor` that is not a power of
    /// ten degrades to plain integer division rather than emitting a
    /// misleading fraction.
    pub fn render(&self, name: &str, help: &str, divisor: u64) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# HELP {name} {help}");
        let _ = writeln!(out, "# TYPE {name} histogram");
        let mut cumulative = 0u64;
        for (bound, counter) in self.bounds.iter().zip(&self.counts) {
            cumulative += counter.get();
            let _ = writeln!(out, "{name}_bucket{{le=\"{}\"}} {cumulative}", bound.le);
        }
        let total = self.count.get();
        let _ = writeln!(out, "{name}_bucket{{le=\"+Inf\"}} {total}");
        let _ = writeln!(out, "{name}_sum {}", scale_decimal(self.sum.get(), divisor));
        let _ = writeln!(out, "{name}_count {total}");
        out
    }
}

/// Format `value / divisor` exactly, without floating point.
///
/// Returns a plain integer when `divisor` is 1 or is not a power of ten;
/// otherwise an integer part and a remainder zero-padded to the divisor's
/// number of decimal places (`scale_decimal(1_500, 1000) == "1.500"`).
fn scale_decimal(value: u64, divisor: u64) -> String {
    let mut places = 0usize;
    let mut remaining = divisor;
    while remaining > 1 && remaining % 10 == 0 {
        remaining /= 10;
        places += 1;
    }
    if divisor <= 1 || remaining != 1 {
        return (value / divisor.max(1)).to_string();
    }
    format!(
        "{}.{:0width$}",
        value / divisor,
        value % divisor,
        width = places
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: [Bucket<'static>; 3] = [
        Bucket::new("0.1", 100),
        Bucket::new("0.5", 500),
        Bucket::new("1", 1_000),
    ];

    /// Each bound's `le` label and its integer `max` are written by hand, so
    /// nothing but a test stops them from disagreeing — and a disagreement is
    /// invisible in the scrape body while quietly assigning observations to
    /// the wrong bucket.
    #[test]
    fn bucket_label_and_integer_bound_agree() {
        for bound in MS {
            let expected = scale_decimal(bound.max, 1_000);
            let matches = expected == bound.le
                || expected.trim_end_matches('0').trim_end_matches('.') == bound.le;
            assert!(
                matches,
                "le={:?} does not denote {} in the base unit (got {expected})",
                bound.le, bound.max
            );
        }
    }

    /// Prometheus buckets are cumulative, but storing them that way would mean
    /// touching every counter above an observation on the hot path. Counts are
    /// stored exclusively and summed at render.
    #[test]
    fn buckets_render_cumulatively_from_exclusive_counts() {
        let h = Histogram::new(&MS);
        h.observe(50); // → le=0.1
        h.observe(300); // → le=0.5
        h.observe(300); // → le=0.5
        let out = h.render("op_seconds", "help", 1_000);
        assert!(out.contains("op_seconds_bucket{le=\"0.1\"} 1"), "{out}");
        assert!(out.contains("op_seconds_bucket{le=\"0.5\"} 3"), "{out}");
        assert!(out.contains("op_seconds_bucket{le=\"1\"} 3"), "{out}");
    }

    /// An observation past the last bound belongs to `+Inf` only. It must
    /// still reach `_sum`/`_count`, or a histogram would under-report exactly
    /// the outliers it exists to expose.
    #[test]
    fn an_observation_past_the_last_bound_lands_only_in_inf() {
        let h = Histogram::new(&MS);
        h.observe(9_999);
        let out = h.render("op_seconds", "help", 1_000);
        assert!(out.contains("op_seconds_bucket{le=\"1\"} 0"), "{out}");
        assert!(out.contains("op_seconds_bucket{le=\"+Inf\"} 1"), "{out}");
        assert!(out.contains("op_seconds_sum 9.999"), "{out}");
        assert!(out.contains("op_seconds_count 1"), "{out}");
    }

    /// The whole reason observations accumulate as integers: the rendered sum
    /// is exact and order-independent, where a float accumulator drifts by an
    /// amount that depends on arrival order.
    #[test]
    fn sum_scaling_is_exact_integer_arithmetic() {
        assert_eq!(scale_decimal(1_500, 1_000), "1.500");
        assert_eq!(scale_decimal(7, 1_000), "0.007");
        assert_eq!(scale_decimal(0, 1_000), "0.000");
        // Not a power of ten → integer division rather than a bogus fraction.
        assert_eq!(scale_decimal(3_000, 1_024), "2");
        // No scaling requested.
        assert_eq!(scale_decimal(42, 1), "42");

        let forward = Histogram::new(&MS);
        let backward = Histogram::new(&MS);
        for value in [1u64, 33, 250, 999] {
            forward.observe(value);
        }
        for value in [999u64, 250, 33, 1] {
            backward.observe(value);
        }
        assert_eq!(
            forward.render("op_seconds", "h", 1_000),
            backward.render("op_seconds", "h", 1_000)
        );
    }

    /// An empty histogram must still expose every series, otherwise a rate()
    /// over a freshly started process reports "no data" instead of zero and
    /// an absence alert cannot distinguish the two.
    #[test]
    fn a_histogram_with_no_observations_still_renders_every_series() {
        let out = Histogram::new(&MS).render("op_seconds", "help", 1_000);
        assert_eq!(
            out,
            "# HELP op_seconds help\n\
             # TYPE op_seconds histogram\n\
             op_seconds_bucket{le=\"0.1\"} 0\n\
             op_seconds_bucket{le=\"0.5\"} 0\n\
             op_seconds_bucket{le=\"1\"} 0\n\
             op_seconds_bucket{le=\"+Inf\"} 0\n\
             op_seconds_sum 0.000\n\
             op_seconds_count 0\n"
        );
    }
}
