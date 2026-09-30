//! `sink_from_destination`: the one place a destination becomes a sink.

use anyhow::Result;

use crate::domain::{BackupDestination, BackupSink};
#[cfg(feature = "s3")]
use crate::infrastructure::S3Sink;
#[cfg(not(feature = "s3"))]
use crate::infrastructure::UnsupportedCloudSink;
use crate::infrastructure::{GcsSink, LocalFsSink};

/// The sink for `destination`: local and GCS always; S3 with the `s3`
/// feature, otherwise an [`UnsupportedCloudSink`](crate::UnsupportedCloudSink)
/// that fails loud on `put` and `prune`.
pub fn sink_from_destination(destination: &BackupDestination) -> Result<Box<dyn BackupSink>> {
    match destination {
        BackupDestination::Local { .. } => {
            Ok(Box::new(LocalFsSink::from_destination(destination)?))
        }
        BackupDestination::Gcs { .. } => Ok(Box::new(GcsSink::from_destination(destination)?)),
        #[cfg(feature = "s3")]
        BackupDestination::S3 { .. } => Ok(Box::new(S3Sink::from_destination(destination)?)),
        #[cfg(not(feature = "s3"))]
        BackupDestination::S3 { .. } => Ok(Box::new(UnsupportedCloudSink {
            destination: destination.clone(),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gcs_destination_constructs_real_sink_without_network_io() {
        let dest = BackupDestination::from_uri("gs://bucket/prefix").unwrap();
        let sink = sink_from_destination(&dest).unwrap();
        assert_eq!(sink.identity(), "gs://bucket/prefix");
    }

    #[cfg(not(feature = "s3"))]
    #[test]
    fn s3_sink_reports_feature_action_when_unlinked() {
        let dest = BackupDestination::from_uri("s3://bucket/prefix").unwrap();
        let sink = sink_from_destination(&dest).unwrap();
        let err = sink
            .put(std::time::SystemTime::now(), b"x")
            .unwrap_err()
            .to_string();
        assert!(err.contains("`s3` feature"));
        assert!(err.contains("--features s3"));
    }
}
