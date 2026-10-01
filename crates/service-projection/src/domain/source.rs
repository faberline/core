use anyhow::Result;

pub trait ProjectionRecord: Clone + Send + Sync + 'static {
    fn projection_cursor(&self) -> u64;
    fn projection_event_id(&self) -> &str;
}

/// One forward-only retained-source scan.
///
/// A source can keep catalog cursors, decoded segment buffers, or remote
/// stream state here. The projection runtime opens one session for the whole
/// catch-up or rebuild instead of asking the source to restart every page.
pub trait ProjectionReadSession<Record>: Send
where
    Record: ProjectionRecord,
{
    fn read_next(&mut self, limit: usize) -> Result<Vec<Record>>;
}

pub trait ProjectionSource<Record>: Send + Sync + 'static
where
    Record: ProjectionRecord,
{
    fn current_cursor(&self) -> u64;
    fn read_after(&self, after: u64, limit: usize) -> Result<Vec<Record>>;

    /// Open a stateful forward scan when the source can avoid stateless page
    /// restarts. Existing sources keep the default and use `read_after`.
    fn open_read_session(
        &self,
        _after: u64,
    ) -> Result<Option<Box<dyn ProjectionReadSession<Record>>>> {
        Ok(None)
    }

    /// Monotonic identity for the retained source set.
    ///
    /// Append-only sources can keep the default value. A source increments the
    /// generation when retention or repair removes or replaces records without
    /// moving its cursor high-water mark.
    fn generation(&self) -> u64 {
        0
    }
}
