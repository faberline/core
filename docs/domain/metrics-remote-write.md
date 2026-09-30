# metrics-remote-write

metrics-remote-write owns the wire contract of Prometheus Remote Write 1.0:
the transport headers, the Snappy block framing, the protobuf `WriteRequest`,
and the validation a request must pass before a product may store it. Turning
a validated request into the product's own records is the product's job, done
behind `RemoteWriteConsumer`. The HTTP route is the product's too. No core
crate depends on it; downstream, sift ingests Prometheus metrics through it.

**Form:** whole-src interfaces · **Depends on:** — · **Crate:** [`crates/metrics-remote-write`](../../crates/metrics-remote-write)

## Model

- **Wire messages** — the `proto` module: `WriteRequest`, `TimeSeries`,
  `Label`, `Sample`, `Exemplar` and `MetricMetadata`, as prost messages.
- **Header check** — `validate_headers` over the content type, content encoding
  and protocol version, failing with a `HeaderError` (`RemoteWriteTwo`,
  `UnsupportedMediaType`, `UnsupportedEncoding`, `UnsupportedVersion`).
- **Snappy block** — `decode_snappy` with a decoded-size limit, and
  `encode_snappy`.
- **Validated write** — `ValidatedWrite`: a `WriteRequest` that passed every
  check, with its sample count; only `decode_write_request` produces one.
- **Stale marker** — `PROMETHEUS_STALE_NAN_BITS`: the one NaN bit pattern
  Prometheus uses to mark a series stale.
- **Decode error** — `DecodeError`: why a body was refused.
- **Consumer** — `RemoteWriteConsumer`: the product hook that converts a
  `ValidatedWrite` into its own output. `consume_write` decodes and then
  consumes, failing with a `ConsumeError` that separates a decode failure
  from a consumer failure.

## Ports

- `RemoteWriteConsumer` — implemented by the product; sift implements it.

## Invariants

- Headers are validated before any payload is decoded. Remote Write 2.0,
  recognised by its media type or a version starting with `2`, is refused
  outright.
- The media type must be `application/x-protobuf` and the encoding `snappy`. A
  version header, when present, must be `0.1.0`, `1.0` or `1.0.0`.
- `decode_snappy` reads the decoded length from the block and refuses a body
  over the limit before allocating for it.
- A valid request has at least one series; every series has at least one label
  and one sample. Label names are non-empty, sorted and unique, and the same
  holds for an exemplar's labels when it has any.
- Sample values are finite, except the Prometheus stale marker; any other NaN
  is refused. Timestamps within a series strictly increase. Exemplar values are
  finite.
- `consume_write` never hands the consumer a request that failed validation.

## Published language

The whole public API; whole-src interfaces contexts have no application layer.
sift re-exports `proto` under its own name. A sift test reads sift's sources
and asserts the paths `metrics_remote_write::RemoteWriteConsumer`,
`metrics_remote_write::validate_headers` and
`metrics_remote_write::decode_snappy`, and the `proto` re-export, so those
paths are a contract.

## Exceptions and debts

- **Checker exceptions:** None.
- **Debts:** none tracked. P2 deleted the unused
  `ValidatedWrite::request` borrow; `into_inner` takes the request (D7).
