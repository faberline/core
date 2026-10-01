//! Groups, capture names and inline flags.

use super::error::*;
use super::node::Node;
use super::{Flags, Parser};

impl Parser<'_> {
    /// At a `(`: a group, or a flag directive such as `(?i)` (`None`).
    pub(super) fn parse_group(&mut self) -> Result<Option<Node>, PatternError> {
        self.bump_and_bump_space();
        if self.bump_if("?=") || self.bump_if("?!") || self.bump_if("?<=") || self.bump_if("?<!") {
            return Err(LOOK_UNSUPPORTED);
        }
        if self.bump_if("?P<") || self.bump_if("?<") {
            self.parse_capture_name()?;
            return Ok(Some(Node::Capture(Box::new(self.parse_inner()?))));
        }
        if !self.bump_if("?") {
            return Ok(Some(Node::Capture(Box::new(self.parse_inner()?))));
        }
        if self.is_done() {
            return Err(UNCLOSED_GROUP_QUESTION);
        }
        let start = self.pos;
        self.flags = self.parse_flags()?;
        if self.char() == Some(')') {
            if self.pos == start {
                return Err(EMPTY_FLAGS);
            }
            return Ok(None);
        }
        self.bump();
        self.parse_inner().map(Some)
    }

    fn parse_capture_name(&mut self) -> Result<(), PatternError> {
        if self.is_done() {
            return Err(MISSING_GROUP_NAME);
        }
        let start = self.pos;
        while let Some(c) = self.char().filter(|&c| c != '>') {
            let first = self.pos == start;
            let valid = c == '_'
                || if first {
                    c.is_alphabetic()
                } else {
                    matches!(c, '.' | '[' | ']') || c.is_alphanumeric()
                };
            if !valid {
                return Err(INVALID_GROUP_NAME);
            }
            if !self.bump() {
                break;
            }
        }
        let end = self.pos;
        if self.is_done() {
            return Err(UNCLOSED_GROUP_NAME);
        }
        self.bump();
        let pattern = self.pattern;
        let name = &pattern[start..end];
        if name.is_empty() {
            return Err(EMPTY_GROUP_NAME);
        }
        if self.names.contains(&name) {
            return Err(DUPLICATE_CAPTURE_NAME);
        }
        self.names.push(name);
        Ok(())
    }

    /// Parse flags up to the `:` or `)` that ends them.
    fn parse_flags(&mut self) -> Result<Flags, PatternError> {
        let mut flags = self.flags;
        let mut negate = false;
        let mut last_was_negation = false;
        let mut seen = [false; 128];
        while let Some(c) = self.char().filter(|&c| c != ':' && c != ')') {
            if c == '-' {
                last_was_negation = true;
                if negate {
                    return Err(FLAG_REPEATED_NEGATION);
                }
                negate = true;
            } else {
                last_was_negation = false;
                let enabled = !negate;
                match c {
                    'i' => flags.case_insensitive = enabled,
                    'm' => flags.multi_line = enabled,
                    's' => flags.dot_all = enabled,
                    'U' => flags.swap_greed = enabled,
                    'R' => flags.crlf = enabled,
                    'x' => flags.verbose = enabled,
                    // regex-lite accepts `u` and ignores it.
                    'u' => {}
                    _ => return Err(FLAG_UNRECOGNIZED),
                }
                let seen = &mut seen[usize::from(c as u8)];
                if *seen {
                    return Err(FLAG_DUPLICATE);
                }
                *seen = true;
            }
            if !self.bump() {
                return Err(FLAG_UNEXPECTED_EOF);
            }
        }
        if last_was_negation {
            return Err(FLAG_DANGLING_NEGATION);
        }
        Ok(flags)
    }
}
