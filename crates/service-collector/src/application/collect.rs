use anyhow::{anyhow, Result};

use super::config::{DeliveryRetryMode, RetryPolicy, RuntimeConfig};
use super::report::RunReport;
use super::sink::BatchSink;
use super::source::{CollectorSource, RecordDecoder};
use crate::domain::{
    CollectorRecord, CollectorRejection, CommitStats, DeliveryReceipt, QuarantineSink, ReadOutcome,
};

pub async fn run_collector<S, D, B, Q>(
    source: &mut S,
    decoder: &D,
    sink: &B,
    quarantine: &mut Q,
    config: RuntimeConfig,
) -> Result<RunReport>
where
    S: CollectorSource + ?Sized,
    D: RecordDecoder<S::Record, Rejection = <S::Rejection as CollectorRejection>::Entry>,
    B: BatchSink<D::Item>,
    Q: QuarantineSink<<S::Rejection as CollectorRejection>::Entry>,
{
    run_collector_with_delivery_mode(
        source,
        decoder,
        sink,
        quarantine,
        config,
        DeliveryRetryMode::Bounded,
    )
    .await
}

pub async fn run_collector_with_delivery_mode<S, D, B, Q>(
    source: &mut S,
    decoder: &D,
    sink: &B,
    quarantine: &mut Q,
    config: RuntimeConfig,
    delivery_mode: DeliveryRetryMode,
) -> Result<RunReport>
where
    S: CollectorSource + ?Sized,
    D: RecordDecoder<S::Record, Rejection = <S::Rejection as CollectorRejection>::Entry>,
    B: BatchSink<D::Item>,
    Q: QuarantineSink<<S::Rejection as CollectorRejection>::Entry>,
{
    let config = config.validate()?;
    RetryPolicy::new(
        config.retry.max_retries,
        config.retry.initial_backoff,
        config.retry.max_backoff,
    )?;
    let mut report = RunReport {
        progress: source.progress(),
        ..RunReport::default()
    };

    loop {
        let mut records = Vec::with_capacity(config.batch_size);
        let mut rejections = Vec::new();
        let mut cursors = Vec::with_capacity(config.batch_size);
        let mut reached_end = false;

        for _ in 0..config.batch_size {
            match source
                .next_record(config.max_record_bytes)
                .map_err(|error| anyhow!("collector source read failed: {error}"))?
            {
                ReadOutcome::Record(record) => {
                    cursors.push(record.cursor().clone());
                    match decoder.decode(record) {
                        Ok(record) => records.push(record),
                        Err(rejection) => rejections.push(rejection),
                    }
                }
                ReadOutcome::Rejection(rejection) => {
                    let (entry, cursor) = rejection.into_parts();
                    rejections.push(entry);
                    cursors.push(cursor);
                }
                ReadOutcome::Pending | ReadOutcome::Exhausted => {
                    reached_end = true;
                    break;
                }
            }
        }

        if cursors.is_empty() {
            if config.follow {
                tokio::time::sleep(config.follow_poll_interval).await;
                source
                    .refresh()
                    .map_err(|error| anyhow!("collector source refresh failed: {error}"))?;
                continue;
            }
            break;
        }

        let delivered = deliver_with_retry(sink, &records, config.retry, delivery_mode).await?;
        quarantine
            .append(&rejections)
            .map_err(|error| anyhow!("collector quarantine append failed: {error}"))?;
        source
            .commit(
                &cursors,
                CommitStats {
                    accepted: delivered.accepted,
                    duplicates: delivered.duplicates,
                    rejected: rejections.len() as u64,
                },
            )
            .map_err(|error| anyhow!("collector checkpoint commit failed: {error}"))?;

        report.lines += cursors.len() as u64;
        report.accepted += delivered.accepted;
        report.duplicates += delivered.duplicates;
        report.rejected += rejections.len() as u64;
        report.progress = source.progress();

        if reached_end {
            if config.follow {
                tokio::time::sleep(config.follow_poll_interval).await;
                source
                    .refresh()
                    .map_err(|error| anyhow!("collector source refresh failed: {error}"))?;
            } else {
                break;
            }
        }
    }

    report.progress = source.progress();
    Ok(report)
}

async fn deliver_with_retry<B, T>(
    sink: &B,
    records: &[T],
    retry: RetryPolicy,
    delivery_mode: DeliveryRetryMode,
) -> Result<DeliveryReceipt>
where
    B: BatchSink<T>,
{
    if records.is_empty() {
        return Ok(DeliveryReceipt::default());
    }
    let mut attempt = 0_usize;
    loop {
        match sink.send(records).await {
            Ok(receipt) => {
                let covered = receipt
                    .accepted
                    .checked_add(receipt.duplicates)
                    .ok_or_else(|| anyhow!("collector delivery receipt counters overflowed"))?;
                let expected = u64::try_from(records.len())
                    .map_err(|_| anyhow!("collector batch size does not fit in u64"))?;
                if covered != expected {
                    return Err(anyhow!(
                        "collector delivery receipt covered {covered} of {expected} records \
                         (accepted={}, duplicates={}); refusing to commit source cursors",
                        receipt.accepted,
                        receipt.duplicates
                    ));
                }
                return Ok(receipt);
            }
            Err(error) if !error.is_retryable() => {
                return Err(anyhow!("collector delivery failed permanently: {error}"));
            }
            Err(error)
                if delivery_mode == DeliveryRetryMode::Bounded && attempt >= retry.max_retries =>
            {
                return Err(anyhow!(
                    "collector delivery exhausted after {} attempt(s): {error}",
                    retry.max_retries.saturating_add(1)
                ));
            }
            Err(_) => tokio::time::sleep(retry.delay(attempt)).await,
        }
        attempt = attempt.saturating_add(1);
    }
}
