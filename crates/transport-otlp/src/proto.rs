pub use opentelemetry_proto::tonic::{
    collector::{
        logs::v1::{ExportLogsPartialSuccess, ExportLogsServiceRequest, ExportLogsServiceResponse},
        metrics::v1::{
            ExportMetricsPartialSuccess, ExportMetricsServiceRequest, ExportMetricsServiceResponse,
        },
        trace::v1::{
            ExportTracePartialSuccess, ExportTraceServiceRequest, ExportTraceServiceResponse,
        },
    },
    common::v1::{any_value, AnyValue, ArrayValue, InstrumentationScope, KeyValue, KeyValueList},
    logs::v1::{LogRecord, ResourceLogs, ScopeLogs},
    metrics::v1::{
        exemplar, metric, number_data_point, Exemplar, Gauge, Histogram, HistogramDataPoint,
        Metric, NumberDataPoint, ResourceMetrics, ScopeMetrics, Sum,
    },
    resource::v1::Resource,
    trace::v1::{ResourceSpans, ScopeSpans, Span},
};
