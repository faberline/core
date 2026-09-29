use serde_json::Value;

use crate::proto;
use crate::signal::OtlpSignal;

#[derive(Clone, Debug)]
pub enum DecodedPayload {
    Logs(proto::ExportLogsServiceRequest),
    Metrics(proto::ExportMetricsServiceRequest),
    Traces(proto::ExportTraceServiceRequest),
    Json { signal: OtlpSignal, value: Value },
}

impl DecodedPayload {
    pub fn signal(&self) -> OtlpSignal {
        match self {
            Self::Logs(_) => OtlpSignal::Logs,
            Self::Metrics(_) => OtlpSignal::Metrics,
            Self::Traces(_) => OtlpSignal::Traces,
            Self::Json { signal, .. } => *signal,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PartialSuccess {
    pub rejected_items: usize,
    pub error_message: String,
}

impl PartialSuccess {
    pub fn new(rejected_items: usize, error_message: impl Into<String>) -> Self {
        Self {
            rejected_items,
            error_message: error_message.into(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rejected_items == 0 && self.error_message.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedOtlpResponse {
    pub content_type: &'static str,
    pub body: Vec<u8>,
}
