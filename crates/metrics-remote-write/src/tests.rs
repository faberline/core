use crate::proto::{Label, Sample, TimeSeries, WriteRequest};
use crate::{
    decode_snappy, decode_write_request, encode_snappy, DecodeError, PROMETHEUS_STALE_NAN_BITS,
};
use prost::Message;

fn request(samples: Vec<Sample>) -> WriteRequest {
    WriteRequest {
        timeseries: vec![TimeSeries {
            labels: vec![Label {
                name: "__name__".into(),
                value: "requests_total".into(),
            }],
            samples,
            exemplars: Vec::new(),
        }],
        metadata: Vec::new(),
    }
}

#[test]
fn stale_marker_survives_the_bounded_wire_round_trip() {
    let original = request(vec![
        Sample {
            value: 3.0,
            timestamp: 1,
        },
        Sample {
            value: f64::from_bits(PROMETHEUS_STALE_NAN_BITS),
            timestamp: 2,
        },
    ]);
    let protobuf = original.encode_to_vec();
    let compressed = encode_snappy(&protobuf).unwrap();
    let decoded = decode_snappy(&compressed, protobuf.len()).unwrap();
    assert_eq!(decoded, protobuf);
    let validated = decode_write_request(&decoded).unwrap();
    assert_eq!(validated.sample_count(), 2);
    let decoded = validated.into_inner();
    assert_eq!(decoded.timeseries[0].samples[0].value, 3.0);
    assert_eq!(
        decoded.timeseries[0].samples[1].value.to_bits(),
        PROMETHEUS_STALE_NAN_BITS
    );
    assert_eq!(decoded.timeseries[0].samples[1].timestamp, 2);
    assert!(matches!(
        decode_snappy(&compressed, protobuf.len() - 1),
        Err(DecodeError::BodyTooLarge { actual, limit })
            if actual == protobuf.len() && limit == protobuf.len() - 1
    ));
}

#[test]
fn duplicate_or_reversed_timestamps_are_refused() {
    for timestamp in [1, 0] {
        let input = request(vec![
            Sample {
                value: 1.0,
                timestamp: 1,
            },
            Sample {
                value: 2.0,
                timestamp,
            },
        ]);
        assert!(matches!(
            decode_write_request(&input.encode_to_vec()),
            Err(DecodeError::NonIncreasingTimestamps)
        ));
    }
}

#[test]
fn ordinary_nan_and_infinity_are_not_the_stale_marker() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let input = request(vec![Sample {
            value,
            timestamp: 1,
        }]);
        assert!(matches!(
            decode_write_request(&input.encode_to_vec()),
            Err(DecodeError::InvalidSampleValue)
        ));
    }
}

#[test]
fn labels_must_be_unique_and_sorted() {
    let mut input = request(vec![Sample {
        value: 1.0,
        timestamp: 1,
    }]);
    let duplicate = input.timeseries[0].labels[0].clone();
    input.timeseries[0].labels.push(duplicate);
    assert!(matches!(
        decode_write_request(&input.encode_to_vec()),
        Err(DecodeError::DuplicateLabels)
    ));
    input.timeseries[0].labels = vec![
        Label {
            name: "job".into(),
            value: "api".into(),
        },
        Label {
            name: "__name__".into(),
            value: "requests_total".into(),
        },
    ];
    assert!(matches!(
        decode_write_request(&input.encode_to_vec()),
        Err(DecodeError::UnsortedLabels)
    ));
}
