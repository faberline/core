//! `<tool> llm` — offline agent-facing self-documentation.
//!
//! Each CLI supplies its own `&[Topic]` (the single in-code source of truth for
//! its domain docs); this module renders the standard `outline` / topic / JSON
//! shapes so the command is uniform across the ecosystem.
//!
//! Phase 1 of #2494 adds [`SectionedTopic`]: a topic whose body is composed of
//! [`TopicSection`]s, some of which are computed at *call time* rather than
//! frozen into a `&'static str` at compile time. Facts that drift out from
//! under a hand-written body (command inventories, config surfaces, feature
//! flags, ...) belong in a `TopicSection::Generated` section so `<tool> llm`
//! always reports what's true right now. `Topic` + `render` are unchanged —
//! every existing CLI keeps compiling and behaving exactly as before.

pub use crate::application::llm::{
    assert_topics_render, render, render_sectioned, Format, RenderableTopic, RenderedSection,
    SectionedTopic, Topic, TopicSection,
};

/// Typed, task-navigation protocol for tools that need stronger machine
/// semantics than a static topic body.
pub mod v2;
