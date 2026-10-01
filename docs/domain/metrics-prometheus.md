# metrics-prometheus

metrics-prometheus holds the metric primitives every faberline service needs
and the Prometheus text encoder (format 0.0.4) that turns them into a scrape
body: a counter, a gauge, a sum-and-count latency pair, a bucketed histogram,
and unlabeled and labeled samples. It has no registry, no macros and no
dependencies; a caller owns its metric struct and its metric names. It was
lifted from lumen. In core, service-auth, service-k8s and
service-observability use it; downstream, defer, lumen, pgpool, relay, sift
and tape do.

**Form:** whole-src infrastructure · **Depends on:** — · **Crate:** [`crates/metrics-prometheus`](../../crates/metrics-prometheus)

## Model

- **Counter** — `Counter`: a monotonic count on one atomic, with `incr`, `add`
  and `get`.
- **Gauge** — `Gauge`: a point-in-time value on one atomic, with `set` and
  `get`.
- **Latency** — `Latency`: a `sum` and a `count` counter, read through the
  `sum()` and `count()` getters; `observe` records one value in whatever unit
  the metric name promises.
- **Bucket** — `Bucket`: one histogram bound stated twice, as the `le` label
  text and as the integer `max` that observations are compared against. Its
  const `new` and getters work in a const bucket list.
- **Histogram** — `Histogram`: per-bucket counts plus `_sum` and `_count` over a
  fixed list of buckets, observed in an integer base unit and published in the
  metric's unit through a divisor at render time.
- **Sample** — `Sample`: one unlabeled metric with its name, kind token, HELP
  text and value.
- **Labeled family** — `SampleGroup`: one name, kind and HELP text over a list
  of `LabeledSample` rows, each a set of `Label` pairs and a value.
- `Sample`, `Label`, `Bucket` and `SampleGroup` are built with a const `new`
  and read through const getters named after their parts. `LabeledSample`
  has a plain `new(labels, value)` and getters.

## Ports

None.

## Invariants

- Counters and gauges use relaxed atomic ordering; each is a single value with
  no other state to keep consistent. Both deref to the raw atomic for callers
  that need it.
- `render` emits `# HELP`, `# TYPE` and the value for each sample, in the order
  given, so the same input always yields the same body.
- `render_labeled` keeps the caller's group and row order, sorts the labels of
  a row by name and then value, and escapes backslash, double quote and
  newline in label values. `escape_label_value` applies the same rule for
  custom families.
- Metric names and HELP text are written as given; they are not escaped.
- Histogram bounds must be sorted ascending by `max`; this is not checked, and
  an unsorted list mis-buckets. An observation goes to the first bound it does
  not exceed.
- A histogram stores exclusive bucket counts and renders them cumulatively. The
  `+Inf` bucket is the total count, so an observation above the last bound is
  counted without a bucket of its own.
- `_sum` is rendered exactly with integer arithmetic when the divisor is a
  power of ten; any other divisor falls back to plain integer division.

## Published language

The whole public API; whole-src infrastructure contexts have no application
layer. Everything is re-exported at the crate root, and no module is public.

## Exceptions and debts

- **Checker exceptions:** None.
- **Debts:** none tracked. P2 made the `Sample`, `Label`, `Bucket`,
  `SampleGroup`, `LabeledSample` and `Latency` fields private (D2).
