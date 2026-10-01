//! A parsed pattern, shaped as regex-lite shapes it, and its rendering as a
//! `regex` pattern that uses no Unicode-sensitive construct.

use std::fmt::Write as _;

use super::class::Ranges;
use super::error::{PatternError, TOO_MUCH_NESTING};

/// A zero-width assertion. Word boundaries are ASCII.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Look {
    Start,
    End,
    StartLf,
    EndLf,
    StartCrlf,
    EndCrlf,
    Word,
    NotWord,
    WordStart,
    WordEnd,
    WordStartHalf,
    WordEndHalf,
}

impl Look {
    fn as_regex(self) -> &'static str {
        match self {
            Look::Start => r"\A",
            Look::End => r"\z",
            Look::StartLf => "(?m:^)",
            Look::EndLf => "(?m:$)",
            Look::StartCrlf => "(?mR:^)",
            Look::EndCrlf => "(?mR:$)",
            Look::Word => r"(?-u:\b)",
            Look::NotWord => r"(?-u:\B)",
            Look::WordStart => r"(?-u:\b{start})",
            Look::WordEnd => r"(?-u:\b{end})",
            Look::WordStartHalf => r"(?-u:\b{start-half})",
            Look::WordEndHalf => r"(?-u:\b{end-half})",
        }
    }
}

/// One node of a parsed pattern. Case-insensitive letters and every class
/// are already resolved to explicit code point ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Node {
    Empty,
    Char(char),
    Class(Ranges),
    Look(Look),
    Repeat {
        min: u32,
        max: Option<u32>,
        greedy: bool,
        sub: Box<Node>,
    },
    Capture(Box<Node>),
    Concat(Vec<Node>),
    Alt(Vec<Node>),
}

impl Node {
    pub(super) fn concat(mut subs: Vec<Node>) -> Node {
        if subs.len() > 1 {
            Node::Concat(subs)
        } else {
            subs.pop().unwrap_or(Node::Empty)
        }
    }

    pub(super) fn alternation(mut subs: Vec<Node>) -> Node {
        if subs.len() > 1 {
            Node::Alt(subs)
        } else {
            subs.pop().unwrap_or(Node::Class(Vec::new()))
        }
    }

    pub(super) fn repetition(min: u32, max: Option<u32>, greedy: bool, sub: Node) -> Node {
        match (min, max) {
            (0, Some(0)) => Node::Empty,
            (1, Some(1)) => sub,
            _ => Node::Repeat {
                min,
                max,
                greedy,
                sub: Box::new(sub),
            },
        }
    }

    /// How many repetitions are stacked directly on top of each other here,
    /// counting no further than `limit + 1`.
    pub(super) fn repeat_chain(&self, limit: u32) -> u32 {
        let mut chain = 0;
        let mut node = self;
        while let Node::Repeat { sub, .. } = node {
            chain += 1;
            if chain > limit {
                break;
            }
            node = sub;
        }
        chain
    }

    /// Reject a tree deeper than `limit`, as regex-lite does after parsing.
    pub(super) fn check_nesting(&self, limit: u32, depth: u32) -> Result<(), PatternError> {
        if depth > limit {
            return Err(TOO_MUCH_NESTING);
        }
        match self {
            Node::Repeat { sub, .. } | Node::Capture(sub) => sub.check_nesting(limit, depth + 1),
            Node::Concat(subs) | Node::Alt(subs) => subs
                .iter()
                .try_for_each(|sub| sub.check_nesting(limit, depth + 1)),
            Node::Empty | Node::Char(_) | Node::Class(_) | Node::Look(_) => Ok(()),
        }
    }

    /// Write this node as a `regex` pattern with no flags.
    pub(super) fn write_regex(&self, out: &mut String) {
        match self {
            Node::Empty => out.push_str("(?:)"),
            Node::Char(c) => write_char(out, *c),
            Node::Class(ranges) => write_class(out, ranges),
            Node::Look(look) => out.push_str(look.as_regex()),
            Node::Repeat {
                min,
                max,
                greedy,
                sub,
            } => {
                write_group(out, sub);
                let _ = match max {
                    Some(max) if max == min => write!(out, "{{{min}}}"),
                    Some(max) => write!(out, "{{{min},{max}}}"),
                    None => write!(out, "{{{min},}}"),
                };
                if !greedy {
                    out.push('?');
                }
            }
            Node::Capture(sub) => write_group(out, sub),
            Node::Concat(subs) => subs.iter().for_each(|sub| sub.write_regex(out)),
            Node::Alt(subs) => {
                out.push_str("(?:");
                for (i, sub) in subs.iter().enumerate() {
                    if i > 0 {
                        out.push('|');
                    }
                    sub.write_regex(out);
                }
                out.push(')');
            }
        }
    }
}

fn write_group(out: &mut String, sub: &Node) {
    out.push_str("(?:");
    sub.write_regex(out);
    out.push(')');
}

fn write_char(out: &mut String, c: char) {
    if c.is_ascii_alphanumeric() {
        out.push(c);
    } else {
        let _ = write!(out, r"\x{{{:X}}}", u32::from(c));
    }
}

/// Write a class as explicit ranges. A range whose start is past its end
/// (regex-lite can produce one across the surrogate gap) matches nothing and
/// is left out; a class with no ranges left matches nothing.
fn write_class(out: &mut String, ranges: &[(char, char)]) {
    let mut ranges = ranges.iter().filter(|(start, end)| start <= end).peekable();
    if ranges.peek().is_none() {
        out.push_str(r"[^\x{0}-\x{10FFFF}]");
        return;
    }
    out.push('[');
    for &(start, end) in ranges {
        let _ = write!(out, r"\x{{{:X}}}", u32::from(start));
        if end != start {
            let _ = write!(out, r"-\x{{{:X}}}", u32::from(end));
        }
    }
    out.push(']');
}
