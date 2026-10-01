/// The maximum size of a snapshot chunk streamed during snapshot generation.
pub const SNAPSHOT_CHUNK_SIZE: usize = 64 * 1024;

/// A bounded chunk sink that consumes streaming writes in chunks of up to `SNAPSHOT_CHUNK_SIZE`.
pub struct ChunkSink<F = Box<dyn FnMut(&[u8]) -> std::io::Result<()> + Send>> {
    chunk_size: usize,
    buffer: Vec<u8>,
    handler: Option<F>,
    collected: Option<Vec<u8>>,
}

impl ChunkSink<Box<dyn FnMut(&[u8]) -> std::io::Result<()> + Send>> {
    /// Create a collecting sink that accumulates all chunks into an internal buffer.
    pub fn new(chunk_size: usize) -> Self {
        Self::collecting(chunk_size)
    }

    /// Create a collecting sink that accumulates all chunks into an internal buffer.
    pub fn collecting(chunk_size: usize) -> Self {
        Self {
            chunk_size,
            buffer: Vec::with_capacity(chunk_size),
            handler: None,
            collected: Some(Vec::new()),
        }
    }

    /// Create a streaming chunk sink that emits chunks of up to `chunk_size` to `handler`
    /// without unbounded memory retention.
    pub fn streaming<H>(
        chunk_size: usize,
        mut handler: H,
    ) -> ChunkSink<Box<dyn FnMut(&[u8]) -> std::io::Result<()> + Send>>
    where
        H: FnMut(&[u8]) + Send + 'static,
    {
        ChunkSink {
            chunk_size,
            buffer: Vec::with_capacity(chunk_size),
            handler: Some(Box::new(move |chunk| {
                handler(chunk);
                Ok(())
            })),
            collected: None,
        }
    }

    /// Return the accumulated bytes (for collecting mode).
    pub fn into_bytes(mut self) -> Vec<u8> {
        if !self.buffer.is_empty() {
            if let Some(ref mut c) = self.collected {
                c.extend_from_slice(&self.buffer);
            }
        }
        self.collected.unwrap_or(self.buffer)
    }
}

impl<F> std::io::Write for ChunkSink<F>
where
    F: FnMut(&[u8]) -> std::io::Result<()>,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let space = self.chunk_size.saturating_sub(self.buffer.len());
        let to_copy = buf.len().min(space);
        if to_copy > 0 {
            self.buffer.extend_from_slice(&buf[..to_copy]);
        }
        if self.buffer.len() >= self.chunk_size {
            self.flush_chunk()?;
        }
        if to_copy == 0 && !buf.is_empty() {
            self.flush_chunk()?;
            let to_copy = buf.len().min(self.chunk_size);
            self.buffer.extend_from_slice(&buf[..to_copy]);
            return Ok(to_copy);
        }
        Ok(to_copy)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if !self.buffer.is_empty() {
            self.flush_chunk()?;
        }
        Ok(())
    }
}

impl<F> ChunkSink<F>
where
    F: FnMut(&[u8]) -> std::io::Result<()>,
{
    fn flush_chunk(&mut self) -> std::io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        if let Some(ref mut handler) = self.handler {
            handler(&self.buffer)?;
            self.buffer.clear();
        } else if let Some(ref mut c) = self.collected {
            c.extend_from_slice(&self.buffer);
            self.buffer.clear();
        }
        Ok(())
    }
}
