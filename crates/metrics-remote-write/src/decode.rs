use std::collections::BTreeSet;

use prost::Message;
use thiserror::Error;

use crate::proto;

/// Prometheus' reserved stale marker. Other NaN payloads are invalid.
pub const PROMETHEUS_STALE_NAN_BITS: u64 = 0x7ff0_0000_0000_0002;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("decoded remote write body is {actual} bytes; limit is {limit} bytes")]
    BodyTooLarge { actual: usize, limit: usize },
    #[error("invalid Snappy block: {0}")]
    InvalidSnappy(#[from] snap::Error),
    #[error("invalid Prometheus WriteRequest protobuf: {0}")]
    InvalidProtobuf(#[from] prost::DecodeError),
    #[error("remote write request contains no time series")]
    EmptyRequest,
    #[error("remote write series contains no labels")]
    EmptyLabels,
    #[error("remote write label names must not be empty")]
    EmptyLabelName,
    #[error("remote write labels must be sorted by name")]
    UnsortedLabels,
    #[error("remote write labels must be unique")]
    DuplicateLabels,
    #[error("remote write series contains no samples")]
    EmptySamples,
    #[error("remote write sample values must be finite or the Prometheus stale marker")]
    InvalidSampleValue,
    #[error("remote write sample timestamps must be strictly increasing")]
    NonIncreasingTimestamps,
    #[error("remote write exemplar value must be finite")]
    InvalidExemplarValue,
}

/// Decompress one Snappy block with a decoded-size limit.
pub fn decode_snappy(body: &[u8], max_decoded_bytes: usize) -> Result<Vec<u8>, DecodeError> {
    let decoded_len = snap::raw::decompress_len(body)?;
    if decoded_len > max_decoded_bytes {
        return Err(DecodeError::BodyTooLarge {
            actual: decoded_len,
            limit: max_decoded_bytes,
        });
    }
    let mut decoded = vec![0; decoded_len];
    let written = snap::raw::Decoder::new().decompress(body, &mut decoded)?;
    decoded.truncate(written);
    Ok(decoded)
}

/// Encode one Remote Write Snappy block.
pub fn encode_snappy(body: &[u8]) -> Result<Vec<u8>, snap::Error> {
    snap::raw::Encoder::new().compress_vec(body)
}

#[derive(Clone, Debug)]
pub struct ValidatedWrite {
    request: proto::WriteRequest,
    sample_count: usize,
}

impl ValidatedWrite {
    pub fn into_inner(self) -> proto::WriteRequest {
        self.request
    }

    pub fn sample_count(&self) -> usize {
        self.sample_count
    }
}

/// Decode and validate the product-neutral Remote Write 1.0 contract.
pub fn decode_write_request(body: &[u8]) -> Result<ValidatedWrite, DecodeError> {
    let request = proto::WriteRequest::decode(body)?;
    if request.timeseries.is_empty() {
        return Err(DecodeError::EmptyRequest);
    }

    let mut sample_count = 0;
    for series in &request.timeseries {
        validate_labels(&series.labels)?;
        if series.samples.is_empty() {
            return Err(DecodeError::EmptySamples);
        }
        let mut previous_timestamp = None;
        for sample in &series.samples {
            let stale = sample.value.to_bits() == PROMETHEUS_STALE_NAN_BITS;
            if !sample.value.is_finite() && !stale {
                return Err(DecodeError::InvalidSampleValue);
            }
            if previous_timestamp.is_some_and(|previous| sample.timestamp <= previous) {
                return Err(DecodeError::NonIncreasingTimestamps);
            }
            previous_timestamp = Some(sample.timestamp);
            sample_count += 1;
        }
        for exemplar in &series.exemplars {
            if !exemplar.labels.is_empty() {
                validate_labels(&exemplar.labels)?;
            }
            if !exemplar.value.is_finite() {
                return Err(DecodeError::InvalidExemplarValue);
            }
        }
    }

    Ok(ValidatedWrite {
        request,
        sample_count,
    })
}

fn validate_labels(labels: &[proto::Label]) -> Result<(), DecodeError> {
    if labels.is_empty() {
        return Err(DecodeError::EmptyLabels);
    }
    let mut previous: Option<&str> = None;
    let mut seen = BTreeSet::new();
    for label in labels {
        if label.name.is_empty() {
            return Err(DecodeError::EmptyLabelName);
        }
        if previous.is_some_and(|previous| previous > label.name.as_str()) {
            return Err(DecodeError::UnsortedLabels);
        }
        if !seen.insert(label.name.as_str()) {
            return Err(DecodeError::DuplicateLabels);
        }
        previous = Some(&label.name);
    }
    Ok(())
}
