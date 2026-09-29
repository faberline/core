use anyhow::Result;
use async_trait::async_trait;

use crate::domain::{DeliveryFailure, DeliveryReceipt};

#[async_trait]
pub trait BatchSink<T>: Send + Sync {
    async fn send(&self, records: &[T]) -> Result<DeliveryReceipt, DeliveryFailure>;
}
