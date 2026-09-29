//! LLM topic provider for the shared service-backup contract.
//!
//! [`TOPIC`]/[`topic`] are the original static form and are unchanged —
//! every existing CLI consumer keeps compiling and behaving exactly as
//! before. [`SECTIONED_TOPICS`]/[`sectioned_topic`] are the parallel
//! `cli_std::llm::SectionedTopic` form (#2494): the destination-contract
//! block is a `TopicSection::Generated` section rendered from
//! `crate::SUPPORTED_SCHEMES` at call time instead of frozen into a
//! `&'static str`, so it can't drift from what
//! `BackupDestination::from_uri`/`sink_from_destination` actually accept in
//! this build. CLI composition can adopt either form; both describe the
//! same contract.

pub use crate::interfaces::{sectioned_topic, topic, SECTIONED_TOPICS, TOPIC};
