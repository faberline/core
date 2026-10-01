#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeliveryReceipt {
    pub accepted: u64,
    pub duplicates: u64,
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
