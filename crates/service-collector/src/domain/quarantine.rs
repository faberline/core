use std::fmt;

pub trait QuarantineSink<T> {
    type Error: fmt::Display + Send + Sync + 'static;

    fn append(&mut self, entries: &[T]) -> Result<(), Self::Error>;
}
