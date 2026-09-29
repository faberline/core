#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OtlpSignal {
    Logs,
    Metrics,
    Traces,
}

impl OtlpSignal {
    pub fn rejected_json_field(self) -> &'static str {
        match self {
            Self::Logs => "rejectedLogRecords",
            Self::Metrics => "rejectedDataPoints",
            Self::Traces => "rejectedSpans",
        }
    }
}
