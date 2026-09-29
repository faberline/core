/// Test seam for observing frames covered by an AOF trim.
#[doc(hidden)]
pub trait FramedLogTrimObserver: Send + Sync + std::fmt::Debug {
    fn covered_frame(&self, through: u64, seq: u64);

    /// Called after an EverySec sync has flushed its current bytes and just
    /// before the filesystem sync starts off the writer lock.  This is a
    /// test-only observation seam; it does not delay or change the sync.
    fn before_background_sync(&self) {}

    fn before_temp_sync(&self, _through: u64) {}
}
