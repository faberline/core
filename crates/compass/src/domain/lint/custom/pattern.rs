//! The pattern dialect of custom regex rules.
//!
//! Custom rules used to be compiled with regex-lite, whose `\d`, `\s`, `\w`,
//! word boundaries and `(?i)` are ASCII-only; the `regex` crate's are
//! Unicode-aware. Rewriting only the Perl escapes is not enough: under `(?i)`
//! regex also folds `k` to the Kelvin sign and `s` to the long s, and it
//! accepts syntax regex-lite rejects. So [`RulePattern`] parses a pattern
//! exactly as regex-lite 0.1 does and writes the same language as a `regex`
//! pattern that uses nothing Unicode-aware: a case-insensitive letter becomes
//! an explicit class, every class becomes explicit code point ranges, and
//! word boundaries become `(?-u:\b)` and friends. A pattern regex-lite
//! rejected is rejected with regex-lite's message.

mod bracket;
mod class;
mod error;
mod escape;
mod group;
mod node;
mod parse;

pub(super) use error::PatternError;

/// regex-lite's default nesting limit.
const NEST_LIMIT: u32 = 50;

/// A compiled custom-rule regex.
#[derive(Debug, Clone)]
pub(super) struct RulePattern {
    regex: regex::Regex,
}

impl RulePattern {
    /// Compile a custom-rule pattern.
    pub(super) fn new(pattern: &str) -> Result<Self, PatternError> {
        let translated = translate(pattern)?;
        let regex = regex::Regex::new(&translated).map_err(PatternError::from_regex)?;
        Ok(Self { regex })
    }

    /// Whether the pattern matches anywhere in `haystack`.
    pub(super) fn is_match(&self, haystack: &str) -> bool {
        // Not `Regex::is_match`: when an ASCII `\B` or half word boundary can
        // match between the bytes of a multi-byte character, regex 1.12's
        // `is_match` can miss a match that `find` reports (`(?-u:\B)|ω` on
        // "zω0"). `find` agrees with regex-lite.
        self.regex.find(haystack).is_some()
    }
}

/// Translate a custom-rule pattern into an equivalent `regex` pattern.
fn translate(pattern: &str) -> Result<String, PatternError> {
    let node = Parser::new(pattern).parse()?;
    let mut out = String::with_capacity(pattern.len() * 2);
    node.write_regex(&mut out);
    Ok(out)
}

#[derive(Debug, Clone, Copy, Default)]
struct Flags {
    case_insensitive: bool,
    multi_line: bool,
    dot_all: bool,
    swap_greed: bool,
    crlf: bool,
    verbose: bool,
}

/// A port of regex-lite's recursive-descent parser. The parsing methods live
/// in the child modules.
struct Parser<'a> {
    pattern: &'a str,
    pos: usize,
    depth: u32,
    flags: Flags,
    names: Vec<&'a str>,
    /// Set when repetitions stack deeper than the nesting limit; the pattern
    /// is rejected once it has parsed.
    too_deep: bool,
}

impl<'a> Parser<'a> {
    fn new(pattern: &'a str) -> Self {
        Parser {
            pattern,
            pos: 0,
            depth: 0,
            flags: Flags::default(),
            names: Vec::new(),
            too_deep: false,
        }
    }

    fn char(&self) -> Option<char> {
        self.pattern[self.pos..].chars().next()
    }

    fn is_done(&self) -> bool {
        self.pos == self.pattern.len()
    }

    /// Move past the current character; false when that reaches the end.
    fn bump(&mut self) -> bool {
        if let Some(c) = self.char() {
            self.pos += c.len_utf8();
        }
        !self.is_done()
    }

    fn bump_if(&mut self, prefix: &str) -> bool {
        let found = self.pattern[self.pos..].starts_with(prefix);
        if found {
            self.pos += prefix.len();
        }
        found
    }

    fn bump_and_bump_space(&mut self) -> bool {
        if !self.bump() {
            return false;
        }
        self.bump_space();
        !self.is_done()
    }

    /// In verbose mode (`x`), skip whitespace and `#` comments.
    fn bump_space(&mut self) {
        if !self.flags.verbose {
            return;
        }
        while let Some(c) = self.char() {
            if c.is_whitespace() {
                self.bump();
            } else if c == '#' {
                self.bump();
                while let Some(c) = self.char() {
                    self.bump();
                    if c == '\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        let c = self.char()?;
        self.pattern[self.pos + c.len_utf8()..].chars().next()
    }

    /// Like `peek`, skipping whitespace in verbose mode the way regex-lite
    /// does.
    fn peek_space(&self) -> Option<char> {
        if !self.flags.verbose {
            return self.peek();
        }
        let c = self.char()?;
        let mut start = self.pos + c.len_utf8();
        let mut in_comment = false;
        for (i, ch) in self.pattern[start..].char_indices() {
            if ch.is_whitespace() {
                continue;
            } else if !in_comment && ch == '#' {
                in_comment = true;
            } else if in_comment && ch == '\n' {
                in_comment = false;
            } else {
                start += i;
                break;
            }
        }
        self.pattern[start..].chars().next()
    }
}

#[cfg(test)]
mod tests;
