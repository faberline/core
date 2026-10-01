//! CFG-based type narrowing (R2.1)
//!
//! Integrates the Control Flow Graph from the PDG module with the TypeNarrower
//! to provide flow-sensitive type narrowing. For each basic block in the CFG,
//! computes the set of narrowed types that hold on entry to that block.
//!
//! This gives Pyright-level precision: after `if isinstance(x, Foo):`, the
//! type of `x` inside the block is `Foo`, not `Union[Foo, Bar]`.

pub use crate::domain::narrowing::block_env::{BlockNarrowEnv, CfgNarrowingResult};
pub use crate::domain::narrowing::cfg_pass::CfgNarrowingPass;
pub use crate::domain::narrowing::resolution::{
    apply_typevar_bindings, check_protocol_satisfaction, resolve_overload,
    resolve_typevar_bindings, ProtocolCheckResult, ProtocolMemberError,
};
