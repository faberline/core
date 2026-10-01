//! Semantic analysis: symbols, scopes, program dependence graphs and Go types.

pub(crate) mod pdg;
pub(crate) mod scope;
pub(crate) mod symbols;
pub(crate) mod types;

#[cfg(test)]
mod tests;
