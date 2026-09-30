/// What a sink reports for one delivered batch.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeliveryReceipt {
    accepted: u64,
    duplicates: u64,
}

impl DeliveryReceipt {
    pub fn new(accepted: u64, duplicates: u64) -> Self {
        Self {
            accepted,
            duplicates,
        }
    }

    pub fn accepted(&self) -> u64 {
        self.accepted
    }

    pub fn duplicates(&self) -> u64 {
        self.duplicates
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct DeliveryFailure {
    retryable: bool,
    message: String,
}

impl DeliveryFailure {
    pub fn retryable(message: impl Into<String>) -> Self {
        Self {
            retryable: true,
            message: message.into(),
        }
    }

    pub fn permanent(message: impl Into<String>) -> Self {
        Self {
            retryable: false,
            message: message.into(),
        }
    }

    pub fn is_retryable(&self) -> bool {
        self.retryable
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}
