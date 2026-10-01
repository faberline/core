use crate::domain::{ProjectionCursor, ProjectionEventId, SourceGeneration};
use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;

use super::{LiveProjection, ProjectionHandle};
use crate::domain::{checkpoint, Projection, ProjectionLag, ProjectionRecord, RebuildComparison};

impl<Record, P> ProjectionHandle<Record, P>
where
    Record: ProjectionRecord,
    P: Projection<Record>,
{
    pub(super) fn catch_up_locked(
        &self,
        live: &mut LiveProjection<P>,
        target: ProjectionCursor,
        generation: SourceGeneration,
    ) -> Result<ProjectionCursor> {
        let mut session = self.source.open_read_session(live.checkpoint.cursor)?;
        while live.checkpoint.cursor < target {
            let records = match session.as_mut() {
                Some(session) => session.read_next(self.config.batch_size)?,
                None => self
                    .source
                    .read_after(live.checkpoint.cursor, self.config.batch_size)?,
            };
            if records.is_empty() {
                live.checkpoint.cursor = target;
                live.checkpoint.event_id = None;
                live.checkpoint.source_generation = generation;
                self.published.notify_waiters();
                break;
            }
            let mut last_event_id = None;
            for record in records
                .iter()
                .filter(|record| record.projection_cursor() <= target)
            {
                live.implementation.apply_idempotent(record)?;
                live.checkpoint.cursor = record.projection_cursor();
                last_event_id = Some(record.projection_event_id());
            }
            if last_event_id.is_none() {
                break;
            }
            live.checkpoint.event_id = last_event_id;
            live.checkpoint.source_generation = generation;
            self.published.notify_waiters();
        }
        let advanced = live.checkpoint.cursor.distance_since(live.persisted_cursor);
        if live.checkpoint.cursor > live.persisted_cursor
            && (live.persisted_cursor == ProjectionCursor::default()
                || advanced >= self.config.snapshot_interval_events)
        {
            self.persist_live(live, generation)?;
        }
        Ok(live.checkpoint.cursor)
    }

    pub(super) fn persist_live(
        &self,
        live: &mut LiveProjection<P>,
        generation: SourceGeneration,
    ) -> Result<()> {
        let state = live.implementation.snapshot()?;
        let checkpoint = checkpoint(
            &live.implementation.descriptor(),
            live.checkpoint.cursor,
            generation,
            live.checkpoint.event_id.clone(),
            &state,
            Utc::now(),
        );
        self.store.persist(&self.name, &checkpoint, &state)?;
        live.implementation.checkpoint_committed()?;
        live.persisted_cursor = checkpoint.cursor;
        live.checkpoint = checkpoint;
        Ok(())
    }

    pub(super) fn rebuild_locked(
        &self,
        live: &mut LiveProjection<P>,
        source_cursor: ProjectionCursor,
        source_generation: SourceGeneration,
    ) -> Result<RebuildComparison> {
        let live_digest = live.implementation.semantic_digest()?;
        let (rebuilt, last_event_id) = self.build_projection(source_cursor)?;
        let rebuilt_digest = rebuilt.semantic_digest()?;
        let descriptor = rebuilt.descriptor();
        let state = rebuilt.snapshot()?;
        let checkpoint = checkpoint(
            &descriptor,
            source_cursor,
            source_generation,
            last_event_id,
            &state,
            Utc::now(),
        );
        self.store.persist(&self.name, &checkpoint, &state)?;
        rebuilt.checkpoint_committed()?;
        live.implementation = rebuilt;
        live.checkpoint = checkpoint;
        live.persisted_cursor = source_cursor;
        self.published.notify_waiters();
        Ok(RebuildComparison {
            source_cursor,
            rebuilt_cursor: source_cursor,
            equal: live_digest == rebuilt_digest,
            live_digest,
            rebuilt_digest,
        })
    }

    pub(super) fn build_projection(
        &self,
        source_cursor: ProjectionCursor,
    ) -> Result<(Arc<P>, Option<ProjectionEventId>)> {
        let rebuilt = (self.factory)()?;
        let mut after = ProjectionCursor::default();
        let mut last_event_id = None;
        let mut session = self.source.open_read_session(after)?;
        while after < source_cursor {
            let records = match session.as_mut() {
                Some(session) => session.read_next(self.config.batch_size)?,
                None => self.source.read_after(after, self.config.batch_size)?,
            };
            if records.is_empty() {
                break;
            }
            let mut advanced = false;
            for record in records
                .iter()
                .filter(|record| record.projection_cursor() <= source_cursor)
            {
                rebuilt.apply_idempotent(record)?;
                after = record.projection_cursor();
                last_event_id = Some(record.projection_event_id());
                advanced = true;
            }
            if !advanced
                || records
                    .last()
                    .is_some_and(|record| record.projection_cursor() > source_cursor)
            {
                break;
            }
        }
        Ok((rebuilt, last_event_id))
    }

    pub(super) fn current_source_generation(&self) -> SourceGeneration {
        self.live
            .lock()
            .expect("projection state lock poisoned")
            .checkpoint
            .source_generation
    }

    pub(super) fn lag(
        &self,
        required_cursor: ProjectionCursor,
        current_cursor: ProjectionCursor,
    ) -> ProjectionLag {
        ProjectionLag::new(
            self.descriptor().name().clone(),
            required_cursor,
            current_cursor,
            self.config.retry_after_seconds,
        )
    }
}
