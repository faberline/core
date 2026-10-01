//! Data Flow Analysis for PDG
//!
//! Implements reaching definitions and def-use chain tracking
//! for data dependency analysis.

pub use crate::domain::semantic::pdg::data_flow::{
    DataDependencies, DefUseChain, Definition, Use, UseDefChain,
};
