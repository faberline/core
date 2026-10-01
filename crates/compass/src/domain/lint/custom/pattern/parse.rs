//! Concatenation, alternation, repetition and the single-character
//! primitives.

use super::class;
use super::error::*;
use super::node::{Look, Node};
use super::{Parser, NEST_LIMIT};

impl Parser<'_> {
    pub(super) fn parse(mut self) -> Result<Node, PatternError> {
        let node = self.parse_inner()?;
        if self.too_deep {
            return Err(TOO_MUCH_NESTING);
        }
        node.check_nesting(NEST_LIMIT, 0)?;
        Ok(node)
    }

    /// Parse up to the end of the pattern or the `)` that closes the current
    /// group.
    pub(super) fn parse_inner(&mut self) -> Result<Node, PatternError> {
        let depth = self.depth;
        if depth > NEST_LIMIT {
            return Err(TOO_MUCH_NESTING);
        }
        self.depth += 1;
        let mut alternates = Vec::new();
        let mut concat = Vec::new();
        loop {
            self.bump_space();
            let Some(c) = self.char() else { break };
            match c {
                '(' => {
                    let outer = self.flags;
                    if let Some(sub) = self.parse_group()? {
                        concat.push(sub);
                        self.flags = outer;
                    }
                    if self.char() != Some(')') {
                        return Err(UNCLOSED_GROUP);
                    }
                    self.bump();
                }
                ')' => {
                    if depth == 0 {
                        return Err(UNOPENED_GROUP);
                    }
                    break;
                }
                '|' => {
                    alternates.push(Node::concat(std::mem::take(&mut concat)));
                    self.bump();
                }
                '[' => concat.push(self.parse_class()?),
                '?' | '*' | '+' => self.parse_uncounted_repetition(c, &mut concat)?,
                '{' => self.parse_counted_repetition(&mut concat)?,
                _ => concat.push(self.parse_primitive(c)?),
            }
        }
        self.depth -= 1;
        alternates.push(Node::concat(concat));
        Ok(Node::alternation(alternates))
    }

    fn parse_primitive(&mut self, c: char) -> Result<Node, PatternError> {
        self.bump();
        Ok(match c {
            '\\' => return self.parse_escape(),
            '.' => self.dot(),
            '^' => self.anchor(true),
            '$' => self.anchor(false),
            c => self.literal(c),
        })
    }

    fn parse_uncounted_repetition(
        &mut self,
        op: char,
        concat: &mut Vec<Node>,
    ) -> Result<(), PatternError> {
        let sub = concat.pop().ok_or(UNCOUNTED_REP_SUB_MISSING)?;
        let (min, max) = match op {
            '?' => (0, Some(1)),
            '*' => (0, None),
            _ => (1, None),
        };
        let mut greedy = true;
        if self.bump() && self.char() == Some('?') {
            greedy = false;
            self.bump();
        }
        concat.push(self.repeat(min, max, greedy, sub));
        Ok(())
    }

    fn parse_counted_repetition(&mut self, concat: &mut Vec<Node>) -> Result<(), PatternError> {
        let sub = concat.pop().ok_or(COUNTED_REP_SUB_MISSING)?;
        if !self.bump_and_bump_space() {
            return Err(COUNTED_REP_UNCLOSED);
        }
        let min = self.parse_decimal()?;
        let mut max = Some(min);
        if self.is_done() {
            return Err(COUNTED_REP_MIN_UNCLOSED);
        }
        if self.char() == Some(',') {
            if !self.bump_and_bump_space() {
                return Err(COUNTED_REP_COMMA_UNCLOSED);
            }
            max = if self.char() != Some('}') {
                Some(self.parse_decimal()?)
            } else {
                None
            };
            if self.is_done() {
                return Err(COUNTED_REP_MIN_MAX_UNCLOSED);
            }
        }
        if self.char() != Some('}') {
            return Err(COUNTED_REP_INVALID);
        }
        let mut greedy = true;
        if self.bump_and_bump_space() && self.char() == Some('?') {
            greedy = false;
            self.bump();
        }
        if max.is_some_and(|max| min > max) {
            return Err(COUNTED_REP_INVALID_RANGE);
        }
        concat.push(self.repeat(min, max, greedy, sub));
        Ok(())
    }

    fn repeat(&mut self, min: u32, max: Option<u32>, greedy: bool, sub: Node) -> Node {
        let greedy = greedy != self.flags.swap_greed;
        let node = Node::repetition(min, max, greedy, sub);
        // regex-lite rejects this tree once parsed; stop it growing.
        if node.repeat_chain(NEST_LIMIT) > NEST_LIMIT {
            self.too_deep = true;
            return Node::Empty;
        }
        node
    }

    /// A literal character, as a two-letter class under `(?i)`.
    pub(super) fn literal(&self, c: char) -> Node {
        if self.flags.case_insensitive {
            if let Some(other) = class::fold_range(c, c) {
                return Node::Class(class::canonicalize(vec![(c, c), other]));
            }
        }
        Node::Char(c)
    }

    fn dot(&self) -> Node {
        Node::Class(if self.flags.dot_all {
            vec![('\0', char::MAX)]
        } else if self.flags.crlf {
            vec![('\0', '\x09'), ('\x0B', '\x0C'), ('\x0E', char::MAX)]
        } else {
            vec![('\0', '\x09'), ('\x0B', char::MAX)]
        })
    }

    fn anchor(&self, start: bool) -> Node {
        Node::Look(match (self.flags.multi_line, self.flags.crlf, start) {
            (false, _, true) => Look::Start,
            (false, _, false) => Look::End,
            (true, false, true) => Look::StartLf,
            (true, false, false) => Look::EndLf,
            (true, true, true) => Look::StartCrlf,
            (true, true, false) => Look::EndCrlf,
        })
    }
}
