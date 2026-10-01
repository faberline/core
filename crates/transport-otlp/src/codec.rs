use std::io::Read;

use prost::Message;
use serde_json::{json, Map};

use crate::error::{Result, TransportError};
use crate::media::OtlpMediaType;
use crate::payload::{DecodedPayload, EncodedOtlpResponse, PartialSuccess};
use crate::proto;
use crate::signal::OtlpSignal;

pub fn decode_payload(
    signal: OtlpSignal,
    media_type: OtlpMediaType,
    body: &[u8],
) -> Result<DecodedPayload> {
    match media_type {
        OtlpMediaType::Json => serde_json::from_slice(body)
            .map(|value| DecodedPayload::Json { signal, value })
            .map_err(|error| TransportError::InvalidJson {
                signal,
                message: error.to_string(),
            }),
        OtlpMediaType::Protobuf => match signal {
            OtlpSignal::Logs => proto::ExportLogsServiceRequest::decode(body)
                .map(DecodedPayload::Logs)
                .map_err(|error| TransportError::InvalidProtobuf {
                    signal,
                    message: error.to_string(),
                }),
            OtlpSignal::Metrics => proto::ExportMetricsServiceRequest::decode(body)
                .map(DecodedPayload::Metrics)
                .map_err(|error| TransportError::InvalidProtobuf {
                    signal,
                    message: error.to_string(),
                }),
            OtlpSignal::Traces => proto::ExportTraceServiceRequest::decode(body)
                .map(DecodedPayload::Traces)
                .map_err(|error| TransportError::InvalidProtobuf {
                    signal,
                    message: error.to_string(),
                }),
        },
    }
}

pub fn encode_response(
    signal: OtlpSignal,
    media_type: OtlpMediaType,
    partial: &PartialSuccess,
) -> Result<EncodedOtlpResponse> {
    let body = match media_type {
        OtlpMediaType::Json => {
            let value = if partial.is_empty() {
                json!({})
            } else {
                let mut fields = Map::new();
                fields.insert(
                    signal.rejected_json_field().to_string(),
                    json!(partial.rejected_items),
                );
                fields.insert("errorMessage".to_string(), json!(partial.error_message));
                json!({"partialSuccess": fields})
            };
            serde_json::to_vec(&value).map_err(|error| TransportError::Encode {
                message: error.to_string(),
            })?
        }
        OtlpMediaType::Protobuf => match signal {
            OtlpSignal::Logs => proto::ExportLogsServiceResponse {
                partial_success: (!partial.is_empty()).then_some(proto::ExportLogsPartialSuccess {
                    rejected_log_records: partial.rejected_items as i64,
                    error_message: partial.error_message.clone(),
                }),
            }
            .encode_to_vec(),
            OtlpSignal::Metrics => proto::ExportMetricsServiceResponse {
                partial_success: (!partial.is_empty()).then_some(
                    proto::ExportMetricsPartialSuccess {
                        rejected_data_points: partial.rejected_items as i64,
                        error_message: partial.error_message.clone(),
                    },
                ),
            }
            .encode_to_vec(),
            OtlpSignal::Traces => proto::ExportTraceServiceResponse {
                partial_success: (!partial.is_empty()).then_some(
                    proto::ExportTracePartialSuccess {
                        rejected_spans: partial.rejected_items as i64,
                        error_message: partial.error_message.clone(),
                    },
                ),
            }
            .encode_to_vec(),
        },
    };
    Ok(EncodedOtlpResponse {
        content_type: media_type.content_type(),
        body,
    })
}

pub fn decode_content_encoding(
    content_encoding: Option<&str>,
    body: &[u8],
    maximum_bytes: usize,
) -> Result<Vec<u8>> {
    match content_encoding
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        None | Some("identity") => {
            if body.len() > maximum_bytes {
                return Err(TransportError::DecodedBodyTooLarge { maximum_bytes });
            }
            Ok(body.to_vec())
        }
        Some("gzip") => {
            let mut output = Vec::new();
            flate2::read::GzDecoder::new(body)
                .take(maximum_bytes.saturating_add(1) as u64)
                .read_to_end(&mut output)
                .map_err(|error| TransportError::InvalidGzip {
                    message: error.to_string(),
                })?;
            if output.len() > maximum_bytes {
                return Err(TransportError::DecodedBodyTooLarge { maximum_bytes });
            }
            Ok(output)
        }
        Some(encoding) => Err(TransportError::UnsupportedContentEncoding {
            encoding: encoding.to_string(),
        }),
    }
}
