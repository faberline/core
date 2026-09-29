use crate::domain::rust_type_system::types::{Lifetime, LifetimeId};

// ============================================================================
// R2d: Lifetime elision
// ============================================================================

/// Rust lifetime elision rule applied to a function or method signature.
///
/// See <https://doc.rust-lang.org/reference/lifetime-elision.html>.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElisionRule {
    /// Rule 1 — each elided input lifetime gets its own fresh lifetime.
    EachInputGetsOwn,
    /// Rule 2 — if there is exactly one input lifetime, that lifetime is
    ///           assigned to all elided output lifetimes.
    SingleInputToOutput,
    /// Rule 3 — if the function has `&self` or `&mut self`, the lifetime of
    ///           that reference is assigned to all elided output lifetimes.
    SelfToOutput,
}

/// The result of applying lifetime elision to a function signature.
#[derive(Debug, Clone, PartialEq)]
pub struct ElisionResult {
    /// The elision rule that was applied to determine the output lifetime.
    pub rule: ElisionRule,
    /// Assigned output lifetime (if any)
    pub output_lifetime: Option<Lifetime>,
    /// Input lifetimes, each assigned a unique identifier (rule 1)
    pub input_lifetimes: Vec<Lifetime>,
}

/// Apply Rust's three standard lifetime elision rules to a function signature.
///
/// # Parameters
/// - `has_self_ref` — `true` when the first parameter is `&self` or `&mut self`
/// - `input_elided_count` — number of elided (`'_` or anonymous) lifetimes in
///   input position
/// - `has_elided_output` — `true` when the return type contains an elided
///   lifetime
/// - `lifetime_counter` — counter used to generate fresh `LifetimeId`s
pub fn apply_lifetime_elision(
    has_self_ref: bool,
    input_elided_count: usize,
    has_elided_output: bool,
    lifetime_counter: &mut usize,
) -> ElisionResult {
    // Rule 1 — assign a unique lifetime to each elided input lifetime.
    let input_lifetimes: Vec<Lifetime> = (0..input_elided_count)
        .map(|_| {
            let id = LifetimeId(*lifetime_counter);
            *lifetime_counter += 1;
            Lifetime::Inferred(id)
        })
        .collect();

    if !has_elided_output {
        return ElisionResult {
            rule: ElisionRule::EachInputGetsOwn,
            output_lifetime: None,
            input_lifetimes,
        };
    }

    // Rule 3 — self reference takes priority over rule 2.
    if has_self_ref && !input_lifetimes.is_empty() {
        return ElisionResult {
            rule: ElisionRule::SelfToOutput,
            output_lifetime: Some(input_lifetimes[0].clone()),
            input_lifetimes,
        };
    }

    // Rule 2 — exactly one input lifetime ⇒ use it as output lifetime.
    if input_lifetimes.len() == 1 {
        return ElisionResult {
            rule: ElisionRule::SingleInputToOutput,
            output_lifetime: Some(input_lifetimes[0].clone()),
            input_lifetimes,
        };
    }

    // Cannot determine: multiple input lifetimes, no self ref, elided output.
    // This is a compile error in real Rust; return a fresh inferred lifetime
    // as a best-effort fallback.
    let fallback_id = LifetimeId(*lifetime_counter);
    *lifetime_counter += 1;

    ElisionResult {
        rule: ElisionRule::EachInputGetsOwn,
        output_lifetime: Some(Lifetime::Inferred(fallback_id)),
        input_lifetimes,
    }
}
