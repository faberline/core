//! Rust symbol extraction visitor methods
//!
//! Implements R1-R7 from spec rust-symbol-analysis:
//! - R1: Extract Rust Functions (function_item)
//! - R2: Extract Rust Structs (struct_item)
//! - R3: Extract Rust Traits (trait_item)
//! - R4: Extract Rust Impls (impl_item)
//! - R5: Extract Rust Constants (const_item, static_item)
//! - R6: Extract Rust Doc Comments (///, //!)
//! - R7: Parse Rust Types into TypeInfo
