//! Bracketed character classes, parsed as regex-lite parses them.

use super::class::{self, Ranges};
use super::error::*;
use super::node::Node;
use super::Parser;

impl Parser<'_> {
    /// At a `[`: parse the class up to and including its `]`.
    pub(super) fn parse_class(&mut self) -> Result<Node, PatternError> {
        let mut union = Ranges::new();
        if !self.bump_and_bump_space() {
            return Err(CLASS_UNCLOSED);
        }
        let negate = self.char() == Some('^');
        if negate && !self.bump_and_bump_space() {
            return Err(CLASS_UNCLOSED_AFTER_NEGATION);
        }
        // Leading `-`s are literal, and so is a `]` that comes first.
        while self.char() == Some('-') {
            union.push(('-', '-'));
            if !self.bump_and_bump_space() {
                return Err(CLASS_UNCLOSED_AFTER_DASH);
            }
        }
        if union.is_empty() && self.char() == Some(']') {
            union.push((']', ']'));
            if !self.bump_and_bump_space() {
                return Err(CLASS_UNCLOSED_AFTER_CLOSING);
            }
        }
        loop {
            self.bump_space();
            let Some(c) = self.char() else {
                return Err(CLASS_UNCLOSED);
            };
            match c {
                '[' => match self.parse_posix_class() {
                    Some(ranges) => union.extend(ranges),
                    None => return Err(CLASS_NEST_UNSUPPORTED),
                },
                ']' => {
                    self.bump();
                    // Fold case before negating, as regex-lite does.
                    let mut ranges = class::canonicalize(union);
                    if self.flags.case_insensitive {
                        ranges = class::ascii_case_fold(ranges);
                    }
                    if negate {
                        ranges = class::negate(&ranges);
                    }
                    return Ok(Node::Class(ranges));
                }
                '&' if self.peek() == Some('&') => return Err(CLASS_INTERSECTION_UNSUPPORTED),
                '-' if self.peek() == Some('-') => return Err(CLASS_DIFFERENCE_UNSUPPORTED),
                '~' if self.peek() == Some('~') => return Err(CLASS_SYMDIFFERENCE_UNSUPPORTED),
                _ => self.parse_class_range(&mut union)?,
            }
        }
    }

    /// One item of a class: a character, a range `a-z`, or an escape such as
    /// `\d`.
    fn parse_class_range(&mut self, union: &mut Ranges) -> Result<(), PatternError> {
        let first = self.parse_class_item()?;
        self.bump_space();
        if self.is_done() {
            return Err(CLASS_UNCLOSED_AFTER_ITEM);
        }
        // A `-` before `]` is literal; `--` is a (rejected) difference.
        if self.char() != Some('-')
            || self.peek_space() == Some(']')
            || self.peek_space() == Some('-')
        {
            union.extend(item_ranges(first)?);
            return Ok(());
        }
        if !self.bump_and_bump_space() {
            return Err(CLASS_UNCLOSED_AFTER_DASH);
        }
        let last = self.parse_class_item()?;
        let (start, end) = (item_char(first)?, item_char(last)?);
        if start > end {
            return Err(CLASS_INVALID_RANGE);
        }
        union.push((start, end));
        Ok(())
    }

    fn parse_class_item(&mut self) -> Result<Node, PatternError> {
        let Some(c) = self.char() else {
            return Err(CLASS_UNCLOSED);
        };
        self.bump();
        if c == '\\' {
            self.parse_escape()
        } else {
            Ok(Node::Char(c))
        }
    }

    /// At a `[` inside a class: parse `[:name:]` or `[:^name:]`, or leave the
    /// parser where it was and return `None`.
    fn parse_posix_class(&mut self) -> Option<Ranges> {
        let start = self.pos;
        let ranges = self.parse_posix_class_body();
        if ranges.is_none() {
            self.pos = start;
        }
        ranges
    }

    fn parse_posix_class_body(&mut self) -> Option<Ranges> {
        if !self.bump() || self.char() != Some(':') || !self.bump() {
            return None;
        }
        let negated = self.char() == Some('^');
        if negated && !self.bump() {
            return None;
        }
        let name_start = self.pos;
        while self.char() != Some(':') && self.bump() {}
        if self.is_done() {
            return None;
        }
        let pattern = self.pattern;
        let name = &pattern[name_start..self.pos];
        if !self.bump_if(":]") {
            return None;
        }
        let ranges = class::posix(name)?;
        Some(if negated {
            class::negate(&ranges)
        } else {
            ranges
        })
    }
}

/// A class item used as a range end must be a single character.
fn item_char(item: Node) -> Result<char, PatternError> {
    match item {
        Node::Char(c) => Ok(c),
        _ => Err(CLASS_INVALID_RANGE_ITEM),
    }
}

/// A class item on its own may be a character or a class such as `\d`.
fn item_ranges(item: Node) -> Result<Ranges, PatternError> {
    match item {
        Node::Char(c) => Ok(vec![(c, c)]),
        Node::Class(ranges) => Ok(ranges),
        _ => Err(CLASS_INVALID_ITEM),
    }
}
