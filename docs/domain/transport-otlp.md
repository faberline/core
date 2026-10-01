# transport-otlp

transport-otlp is the OpenTelemetry Protocol ingest shell. It owns media
negotiation, decoding with the official OTLP protobuf types, bounded gzip
decompression, partial-success responses, and a gRPC server for the logs,
metrics and trace services. What an accepted payload becomes is the product's
decision, made behind `OtlpConsumer`; which project a gRPC caller writes to is
the product's decision too, made behind `GrpcProjectAuthorizer`. The OTLP HTTP
route belongs to the product, which calls this crate's functions from it. No
core crate depends on it; downstream, sift ingests OTLP through it.

**Form:** whole-src interfaces · **Depends on:** — · **Crate:** [`crates/transport-otlp`](../../crates/transport-otlp)

## Model

- **Signal** — `OtlpSignal`: `Logs`, `Metrics` or `Traces`, each with the JSON
  field that counts its rejected items (`rejected_json_field`).
- **Media type** — `OtlpMediaType`: `Json` or `Protobuf`, parsed from a
  content type.
- **Wire messages** — the `proto` module: the official OTLP request, response,
  partial-success and data types.
- **Decoded payload** — `DecodedPayload`: a protobuf export request of one
  signal, or a JSON value tagged with its signal.
- **Partial success** — `PartialSuccess`: how many items the consumer rejected
  and why; an empty one means full success.
- **Encoded response** — `EncodedOtlpResponse`: a response body and its content
  type.
- **Transport error** — `TransportError`: unsupported media type or content
  encoding, a decoded body over the limit, invalid JSON, protobuf or gzip, an
  encoding failure, or a consumer failure.
- **gRPC server** — `serve_grpc`: the three OTLP collector services on one
  listener, with a maximum message size and a shutdown future.

## Ports

- `OtlpConsumer` — takes a project and a `DecodedPayload` and returns a
  `PartialSuccess` or a `TransportError`; sift implements it.
- `GrpcProjectAuthorizer` — turns gRPC request metadata into a project, or a
  gRPC status that refuses the call; sift implements it.

## Invariants

- A missing content type means JSON. `application/x-protobuf` and
  `application/protobuf` mean protobuf; anything else is
  `UnsupportedMediaType`.
- A protobuf body decodes into the official request type of its signal. A JSON
  body is only parsed as JSON and handed on with its signal; interpreting it is
  the consumer's job.
- `decode_content_encoding` accepts no encoding, `identity` or `gzip`. An
  identity body over the limit is refused; a gzip body is inflated to at most
  one byte past the limit, and refused if it reaches that byte.
- A response carries a partial success only when something was rejected or a
  message was set: JSON answers `{}` otherwise, and protobuf leaves the field
  unset.
- On gRPC the authorizer runs before the consumer, and a refusal never reaches
  the consumer. Both gRPC compression directions accept gzip.
- A consumer error maps to a gRPC status: a media, encoding or decoding error
  to `invalid_argument`, an oversized body to `resource_exhausted`, an encode
  failure to `internal`, and a consumer failure to `unavailable`.

## Published language

The whole public API; whole-src interfaces contexts have no application layer.
sift re-exports `proto` under its own name. A sift test reads sift's sources
and asserts the `proto` re-export and the paths `transport_otlp::serve_grpc`
and `transport_otlp::OtlpConsumer`, so those paths are a contract.

## Exceptions and debts

- **Checker exceptions (P1):** None.
- **Tracked for P2:** `anyhow` in the result of `serve_grpc` (ADR D4); the two
  ports return the crate's own `TransportError` and a gRPC status.
