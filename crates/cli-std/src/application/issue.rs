//! The issue verbs (create, comment, search, view), their flags, and the
//! diagnostics, labels and terminal output they share.

pub(crate) mod comment;
pub(crate) mod create;
pub(crate) mod diagnostics;
pub(crate) mod labels;
mod output;
pub(crate) mod repo;
pub(crate) mod search;
pub(crate) mod view;

#[cfg(test)]
mod tests;
