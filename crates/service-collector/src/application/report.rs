use crate::domain::SourceProgress;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RunReport {
    pub lines: u64,
    pub accepted: u64,
    pub duplicates: u64,
    pub rejected: u64,
    pub progress: SourceProgress,
}
