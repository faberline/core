use std::io::Write;

use flate2::{write::GzEncoder, Compression};
use prost::Message;
use serde_json::json;

use crate::{
    decode_content_encoding, decode_payload, encode_response, proto, DecodedPayload, OtlpMediaType,
    OtlpSignal, PartialSuccess, TransportError,
};

#[test]
fn compressed_and_identity_bodies_enforce_the_decoded_limit() {
    let body = vec![b'x'; 4096];
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&body).unwrap();
    let gzip = encoder.finish().unwrap();
    assert!(gzip.len() < body.len());
    for (encoding, wire) in [(Some("gzip"), gzip.as_slice()), (None, body.as_slice())] {
        assert_eq!(
            decode_content_encoding(encoding, wire, body.len()).unwrap(),
            body
        );
        assert!(matches!(
            decode_content_encoding(encoding, wire, body.len() - 1),
            Err(TransportError::DecodedBodyTooLarge {
                maximum_bytes: 4095
            })
        ));
    }
}

#[test]
fn unsupported_or_malformed_encoding_is_refused() {
    assert!(matches!(
        decode_content_encoding(Some("br"), b"body", 100),
        Err(TransportError::UnsupportedContentEncoding { .. })
    ));
    assert!(matches!(
        decode_content_encoding(Some("gzip"), b"not gzip", 100),
        Err(TransportError::InvalidGzip { .. })
    ));
}

#[test]
fn partial_success_uses_each_signal_wire_field() {
    for (signal, field) in [
        (OtlpSignal::Logs, "rejectedLogRecords"),
        (OtlpSignal::Metrics, "rejectedDataPoints"),
        (OtlpSignal::Traces, "rejectedSpans"),
    ] {
        let encoded = encode_response(
            signal,
            OtlpMediaType::Json,
            &PartialSuccess::new(2, "refused"),
        )
        .unwrap();
        let actual: serde_json::Value = serde_json::from_slice(&encoded.body).unwrap();
        assert_eq!(
            actual,
            json!({"partialSuccess": {field: 2, "errorMessage": "refused"}})
        );
        assert_eq!(encoded.content_type, "application/json");
        let empty =
            encode_response(signal, OtlpMediaType::Json, &PartialSuccess::default()).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&empty.body).unwrap(),
            json!({})
        );
    }
}

#[test]
fn protobuf_response_and_request_keep_the_official_logs_contract() {
    let response = encode_response(
        OtlpSignal::Logs,
        OtlpMediaType::Protobuf,
        &PartialSuccess::new(3, "capacity"),
    )
    .unwrap();
    assert_eq!(response.content_type, "application/x-protobuf");
    let decoded = proto::ExportLogsServiceResponse::decode(response.body.as_slice()).unwrap();
    let partial = decoded.partial_success.unwrap();
    assert_eq!(partial.rejected_log_records, 3);
    assert_eq!(partial.error_message, "capacity");
    let input = proto::ExportLogsServiceRequest::default().encode_to_vec();
    assert!(matches!(
        decode_payload(OtlpSignal::Logs, OtlpMediaType::Protobuf, &input).unwrap(),
        DecodedPayload::Logs(_)
    ));
    assert!(matches!(
        decode_payload(OtlpSignal::Logs, OtlpMediaType::Protobuf, &[0xff]),
        Err(TransportError::InvalidProtobuf { .. })
    ));
}
