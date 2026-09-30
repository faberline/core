//! Semantic analysis: symbols, scopes and program dependence graphs.

pub(crate) mod pdg;
pub(crate) mod scope;
pub(crate) mod symbols;

#[cfg(test)]
mod tests;
