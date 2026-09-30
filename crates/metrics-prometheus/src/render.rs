use std::fmt::Write;

/// One named metric sample ready to render: the Prometheus metric
/// `name`, its `kind` token (`"counter"` or `"gauge"`), the `# HELP`
/// text, and the current `value`.
#[derive(Debug, Clone, Copy)]
pub struct Sample<'a> {
    name: &'a str,
    kind: &'a str,
    help: &'a str,
    value: u64,
}

impl<'a> Sample<'a> {
    pub const fn new(name: &'a str, kind: &'a str, help: &'a str, value: u64) -> Self {
        Self {
            name,
            kind,
            help,
            value,
        }
    }

    /// The Prometheus metric name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// The `# TYPE` token, such as `"counter"` or `"gauge"`.
    pub const fn kind(&self) -> &'a str {
        self.kind
    }

    /// The `# HELP` text.
    pub const fn help(&self) -> &'a str {
        self.help
    }

    /// The sample value.
    pub const fn value(&self) -> u64 {
        self.value
    }
}

/// One Prometheus label name/value pair. The renderer canonicalizes label
/// order and escapes values, so callers only own label semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Label<'a> {
    name: &'a str,
    value: &'a str,
}

impl<'a> Label<'a> {
    pub const fn new(name: &'a str, value: &'a str) -> Self {
        Self { name, value }
    }

    /// The label name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// The label value, unescaped.
    pub const fn value(&self) -> &'a str {
        self.value
    }
}

/// One value row within a labeled metric family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabeledSample<'a> {
    labels: Vec<Label<'a>>,
    value: u64,
}

impl<'a> LabeledSample<'a> {
    pub fn new(labels: Vec<Label<'a>>, value: u64) -> Self {
        Self { labels, value }
    }

    /// The row's labels, in the order given to `new`.
    pub fn labels(&self) -> &[Label<'a>] {
        &self.labels
    }

    /// The row value.
    pub fn value(&self) -> u64 {
        self.value
    }
}

/// A metric family whose HELP and TYPE declarations are shared by one or more
/// labeled value rows.
#[derive(Debug, Clone, Copy)]
pub struct SampleGroup<'a> {
    name: &'a str,
    kind: &'a str,
    help: &'a str,
    samples: &'a [LabeledSample<'a>],
}

impl<'a> SampleGroup<'a> {
    pub const fn new(
        name: &'a str,
        kind: &'a str,
        help: &'a str,
        samples: &'a [LabeledSample<'a>],
    ) -> Self {
        Self {
            name,
            kind,
            help,
            samples,
        }
    }

    /// The metric family name shared by every row.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// The `# TYPE` token shared by every row.
    pub const fn kind(&self) -> &'a str {
        self.kind
    }

    /// The `# HELP` text shared by every row.
    pub const fn help(&self) -> &'a str {
        self.help
    }

    /// The labeled rows, in render order.
    pub const fn samples(&self) -> &'a [LabeledSample<'a>] {
        self.samples
    }
}

/// Render `samples` as Prometheus text format (0.0.4 compatible): each
/// sample emits `# HELP <name> <help>`, `# TYPE <name> <kind>`, then
/// `<name> <value>`, in the order given. Always emits the same set of
/// lines for the same input so scrape configs stay stable.
pub fn render(samples: &[Sample<'_>]) -> String {
    let mut out = String::new();
    for sample in samples {
        let _ = writeln!(out, "# HELP {} {}", sample.name, sample.help);
        let _ = writeln!(out, "# TYPE {} {}", sample.name, sample.kind);
        let _ = writeln!(out, "{} {}", sample.name, sample.value);
    }
    out
}

/// Render labeled metric families as Prometheus text format 0.0.4.
///
/// Groups and rows preserve caller order. Labels within a row are sorted by
/// name then value, and label values escape backslash, double quote, and
/// newline as required by the Prometheus exposition format.
pub fn render_labeled(groups: &[SampleGroup<'_>]) -> String {
    let mut out = String::new();
    for group in groups {
        let _ = writeln!(out, "# HELP {} {}", group.name, group.help);
        let _ = writeln!(out, "# TYPE {} {}", group.name, group.kind);
        for sample in group.samples {
            let _ = write!(out, "{}", group.name);
            if !sample.labels.is_empty() {
                let mut labels = sample.labels.iter().collect::<Vec<_>>();
                labels.sort_unstable_by(|left, right| {
                    left.name
                        .cmp(right.name)
                        .then_with(|| left.value.cmp(right.value))
                });
                out.push('{');
                for (index, label) in labels.into_iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    let _ = write!(out, "{}=\"", label.name);
                    write_escaped_label_value(&mut out, label.value);
                    out.push('"');
                }
                out.push('}');
            }
            let _ = writeln!(out, " {}", sample.value);
        }
    }
    out
}

/// Escapes a label value for Prometheus text exposition. Custom metric
/// families should use this instead of duplicating the escaping rules.
pub fn escape_label_value(value: &str) -> String {
    let mut escaped = String::new();
    write_escaped_label_value(&mut escaped, value);
    escaped
}

fn write_escaped_label_value(out: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instrument::{Counter, Gauge, Latency};

    #[test]
    fn render_emits_help_type_value_per_sample() {
        let samples = [
            Sample::new("demo_total", "counter", "A demo counter.", 3),
            Sample::new("demo_bytes", "gauge", "A demo gauge.", 100),
        ];
        let first = samples[0];
        assert_eq!(
            (first.name(), first.kind(), first.help(), first.value()),
            ("demo_total", "counter", "A demo counter.", 3)
        );
        let out = render(&samples);
        assert_eq!(
            out,
            "# HELP demo_total A demo counter.\n\
             # TYPE demo_total counter\n\
             demo_total 3\n\
             # HELP demo_bytes A demo gauge.\n\
             # TYPE demo_bytes gauge\n\
             demo_bytes 100\n"
        );
    }

    #[test]
    fn labeled_render_sorts_and_escapes_labels() {
        let rows = [LabeledSample::new(
            vec![Label::new("zone", "a\\b\nc"), Label::new("pool", "x\"y")],
            7,
        )];
        let groups = [SampleGroup::new(
            "demo_active",
            "gauge",
            "Active demo resources.",
            &rows,
        )];

        assert_eq!(
            render_labeled(&groups),
            "# HELP demo_active Active demo resources.\n\
# TYPE demo_active gauge\n\
demo_active{pool=\"x\\\"y\",zone=\"a\\\\b\\nc\"} 7\n"
        );
        assert_eq!(rows[0].labels()[0].name(), "zone");
        assert_eq!(rows[0].value(), 7);
        assert_eq!(groups[0].name(), "demo_active");
        assert_eq!(groups[0].kind(), "gauge");
        assert_eq!(groups[0].help(), "Active demo resources.");
        assert_eq!(groups[0].samples().len(), 1);
    }

    /// Golden-render test derived from lumen's `src/metrics.rs` (#974):
    /// reproduces lumen's exact metric set (names, HELP text, `# TYPE`
    /// kinds, ordering) at fixed counter states and asserts the encoder
    /// output is byte-identical to the pre-refactor capture. lumen's own
    /// `Metrics::render` test asserts the same string against the live
    /// `Metrics` struct so the two stay locked together.
    #[test]
    fn golden_render_matches_lumen_metrics_capture() {
        let index_writes_total = Counter::new();
        let index_bytes_total = Counter::new();
        let search = Latency::new();
        let duplicates_requests_total = Counter::new();
        let collections_created_total = Counter::new();
        let schema_fields_total = Counter::new();
        let storage_bytes = Gauge::new();
        let posting_cache_hits_total = Counter::new();
        let posting_cache_misses_total = Counter::new();

        index_writes_total.add(3);
        index_bytes_total.add(100);
        search.observe(7);
        search.observe(9);
        duplicates_requests_total.incr();
        collections_created_total.incr();
        schema_fields_total.add(4);
        storage_bytes.set(2048);
        posting_cache_hits_total.add(5);
        posting_cache_misses_total.add(2);

        let samples = [
            Sample::new(
                "lumen_index_writes_total",
                "counter",
                "Total index items applied.",
                index_writes_total.get(),
            ),
            Sample::new(
                "lumen_index_bytes_total",
                "counter",
                "Total bytes written across all field indexes.",
                index_bytes_total.get(),
            ),
            Sample::new(
                "lumen_search_requests_total",
                "counter",
                "Total search requests served.",
                search.count.get(),
            ),
            Sample::new(
                "lumen_search_latency_ms_sum",
                "counter",
                "Sum of search latencies in milliseconds.",
                search.sum.get(),
            ),
            Sample::new(
                "lumen_search_latency_ms_count",
                "counter",
                "Count of search latency observations.",
                search.count.get(),
            ),
            Sample::new(
                "lumen_duplicates_requests_total",
                "counter",
                "Total duplicate-detection requests.",
                duplicates_requests_total.get(),
            ),
            Sample::new(
                "lumen_collections_created_total",
                "counter",
                "Total collections created or extended.",
                collections_created_total.get(),
            ),
            Sample::new(
                "lumen_schema_fields_total",
                "counter",
                "Total field declarations registered.",
                schema_fields_total.get(),
            ),
            Sample::new(
                "lumen_storage_bytes",
                "gauge",
                "Approximate bytes held by all in-memory field indexes.",
                storage_bytes.get(),
            ),
            Sample::new(
                "lumen_posting_cache_hits_total",
                "counter",
                "Posting cache hit count (0 until LSM cache is wired).",
                posting_cache_hits_total.get(),
            ),
            Sample::new(
                "lumen_posting_cache_misses_total",
                "counter",
                "Posting cache miss count.",
                posting_cache_misses_total.get(),
            ),
        ];

        let out = render(&samples);
        let golden = "# HELP lumen_index_writes_total Total index items applied.\n\
# TYPE lumen_index_writes_total counter\n\
lumen_index_writes_total 3\n\
# HELP lumen_index_bytes_total Total bytes written across all field indexes.\n\
# TYPE lumen_index_bytes_total counter\n\
lumen_index_bytes_total 100\n\
# HELP lumen_search_requests_total Total search requests served.\n\
# TYPE lumen_search_requests_total counter\n\
lumen_search_requests_total 2\n\
# HELP lumen_search_latency_ms_sum Sum of search latencies in milliseconds.\n\
# TYPE lumen_search_latency_ms_sum counter\n\
lumen_search_latency_ms_sum 16\n\
# HELP lumen_search_latency_ms_count Count of search latency observations.\n\
# TYPE lumen_search_latency_ms_count counter\n\
lumen_search_latency_ms_count 2\n\
# HELP lumen_duplicates_requests_total Total duplicate-detection requests.\n\
# TYPE lumen_duplicates_requests_total counter\n\
lumen_duplicates_requests_total 1\n\
# HELP lumen_collections_created_total Total collections created or extended.\n\
# TYPE lumen_collections_created_total counter\n\
lumen_collections_created_total 1\n\
# HELP lumen_schema_fields_total Total field declarations registered.\n\
# TYPE lumen_schema_fields_total counter\n\
lumen_schema_fields_total 4\n\
# HELP lumen_storage_bytes Approximate bytes held by all in-memory field indexes.\n\
# TYPE lumen_storage_bytes gauge\n\
lumen_storage_bytes 2048\n\
# HELP lumen_posting_cache_hits_total Posting cache hit count (0 until LSM cache is wired).\n\
# TYPE lumen_posting_cache_hits_total counter\n\
lumen_posting_cache_hits_total 5\n\
# HELP lumen_posting_cache_misses_total Posting cache miss count.\n\
# TYPE lumen_posting_cache_misses_total counter\n\
lumen_posting_cache_misses_total 2\n";
        assert_eq!(
            out, golden,
            "encoder output diverged from lumen's pre-refactor capture"
        );
    }
}
