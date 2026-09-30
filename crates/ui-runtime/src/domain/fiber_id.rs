//! The identity of a fiber.

/// Identifies one mounted component's fiber. The runtime allocates ids in
/// increasing order.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct FiberId(pub u64);
