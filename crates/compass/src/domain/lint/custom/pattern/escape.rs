//! Escapes: `\n`, `\x{..}`, `\d`, `\b{start}` and the rest, and the decimals
//! of a counted repetition.

use super::class;
use super::error::*;
use super::node::{Look, Node};
use super::Parser;

impl Parser<'_> {
    /// Parse the escape after a `\`.
    pub(super) fn parse_escape(&mut self) -> Result<Node, PatternError> {
        let Some(c) = self.char() else {
            return Err(ESCAPE_UNEXPECTED_EOF);
        };
        match c {
            '0'..='9' => return Err(BACKREF_UNSUPPORTED),
            'p' | 'P' => return Err(UNICODE_CLASS_UNSUPPORTED),
            'x' | 'u' | 'U' => return self.parse_hex(c),
            'd' | 's' | 'w' | 'D' | 'S' | 'W' => {
                self.bump();
                return Ok(Node::Class(class::perl(c)));
            }
            _ => {}
        }
        self.bump();
        if is_escapable(c) {
            return Ok(self.literal(c));
        }
        let look = match c {
            'a' => return Ok(self.literal('\x07')),
            'f' => return Ok(self.literal('\x0C')),
            't' => return Ok(self.literal('\t')),
            'n' => return Ok(self.literal('\n')),
            'r' => return Ok(self.literal('\r')),
            'v' => return Ok(self.literal('\x0B')),
            'A' => Look::Start,
            'z' => Look::End,
            'b' if self.char() == Some('{') => {
                self.parse_special_word_boundary()?.unwrap_or(Look::Word)
            }
            'b' => Look::Word,
            'B' => Look::NotWord,
            '<' => Look::WordStart,
            '>' => Look::WordEnd,
            _ => return Err(ESCAPE_UNRECOGNIZED),
        };
        Ok(Node::Look(look))
    }

    /// After `\b`, at a `{`: parse `{start}`, `{end}`, `{start-half}` or
    /// `{end-half}`, or leave the `{` for a counted repetition.
    fn parse_special_word_boundary(&mut self) -> Result<Option<Look>, PatternError> {
        let is_name_char = |c: char| c.is_ascii_alphabetic() || c == '-';
        let start = self.pos;
        if !self.bump_and_bump_space() {
            return Err(SPECIAL_WORD_OR_REP_UNEXPECTED_EOF);
        }
        if !self.char().is_some_and(is_name_char) {
            self.pos = start;
            return Ok(None);
        }
        let mut name = String::new();
        while let Some(c) = self.char().filter(|&c| is_name_char(c)) {
            name.push(c);
            self.bump_and_bump_space();
        }
        if self.char() != Some('}') {
            return Err(SPECIAL_WORD_BOUNDARY_UNCLOSED);
        }
        self.bump();
        match name.as_str() {
            "start" => Ok(Some(Look::WordStart)),
            "end" => Ok(Some(Look::WordEnd)),
            "start-half" => Ok(Some(Look::WordStartHalf)),
            "end-half" => Ok(Some(Look::WordEndHalf)),
            _ => Err(SPECIAL_WORD_BOUNDARY_UNRECOGNIZED),
        }
    }

    /// At the `x`, `u` or `U` of a hex escape.
    fn parse_hex(&mut self, kind: char) -> Result<Node, PatternError> {
        let width = match kind {
            'x' => 2,
            'u' => 4,
            _ => 8,
        };
        if !self.bump_and_bump_space() {
            return Err(HEX_UNEXPECTED_EOF);
        }
        let mut digits = String::new();
        let (invalid, value) = if self.char() == Some('{') {
            while self.bump_and_bump_space() && self.char() != Some('}') {
                match self.char() {
                    Some(c) if c.is_ascii_hexdigit() => digits.push(c),
                    _ => return Err(HEX_BRACE_INVALID_DIGIT),
                }
            }
            if self.is_done() {
                return Err(HEX_BRACE_UNEXPECTED_EOF);
            }
            self.bump_and_bump_space();
            if digits.is_empty() {
                return Err(HEX_BRACE_EMPTY);
            }
            (HEX_BRACE_INVALID, u32::from_str_radix(&digits, 16))
        } else {
            for i in 0..width {
                if i > 0 && !self.bump_and_bump_space() {
                    return Err(HEX_FIXED_UNEXPECTED_EOF);
                }
                match self.char() {
                    Some(c) if c.is_ascii_hexdigit() => digits.push(c),
                    _ => return Err(HEX_FIXED_INVALID_DIGIT),
                }
            }
            self.bump_and_bump_space();
            (HEX_FIXED_INVALID, u32::from_str_radix(&digits, 16))
        };
        match value.ok().and_then(char::from_u32) {
            Some(c) => Ok(self.literal(c)),
            None => Err(invalid),
        }
    }

    /// A decimal in a counted repetition, allowing whitespace around it.
    pub(super) fn parse_decimal(&mut self) -> Result<u32, PatternError> {
        while self.char().is_some_and(char::is_whitespace) {
            self.bump();
        }
        let mut digits = String::new();
        while let Some(c) = self.char().filter(char::is_ascii_digit) {
            digits.push(c);
            self.bump_and_bump_space();
        }
        while self.char().is_some_and(char::is_whitespace) {
            self.bump_and_bump_space();
        }
        if digits.is_empty() {
            return Err(DECIMAL_NO_DIGITS);
        }
        digits.parse().map_err(|_| DECIMAL_INVALID)
    }
}

/// Whether `\c` is `c` itself: every ASCII character but letters, digits,
/// `<` and `>`.
fn is_escapable(c: char) -> bool {
    c.is_ascii() && !c.is_ascii_alphanumeric() && c != '<' && c != '>'
}
