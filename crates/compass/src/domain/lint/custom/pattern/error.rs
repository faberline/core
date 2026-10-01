//! Why a custom rule's pattern is rejected. The messages are regex-lite's,
//! so a rejected rule reports the same reason it always has.

use std::borrow::Cow;
use std::fmt;

/// A pattern the custom-rule regex dialect does not accept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::domain::lint::custom) struct PatternError(Cow<'static, str>);

impl PatternError {
    /// A `regex` compile error for a translated pattern. Only the size limit
    /// can fail there; it keeps regex-lite's wording.
    pub(super) fn from_regex(error: regex::Error) -> Self {
        match error {
            regex::Error::CompiledTooBig(_) => SIZE_LIMIT,
            other => PatternError(Cow::Owned(other.to_string())),
        }
    }
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PatternError {}

const fn err(message: &'static str) -> PatternError {
    PatternError(Cow::Borrowed(message))
}

pub(super) const SIZE_LIMIT: PatternError = err("compiled regex exceeded size limit");

pub(super) const TOO_MUCH_NESTING: PatternError = err("pattern has too much nesting");
pub(super) const DUPLICATE_CAPTURE_NAME: PatternError = err("duplicate capture group name");
pub(super) const UNCLOSED_GROUP: PatternError = err("found open group without closing ')'");
pub(super) const UNCLOSED_GROUP_QUESTION: PatternError =
    err("expected closing ')', but got end of pattern");
pub(super) const UNOPENED_GROUP: PatternError = err("found closing ')' without matching '('");
pub(super) const LOOK_UNSUPPORTED: PatternError = err("look-around is not supported");
pub(super) const EMPTY_FLAGS: PatternError = err("empty flag directive '(?)' is not allowed");
pub(super) const MISSING_GROUP_NAME: PatternError =
    err("expected capture group name, but got end of pattern");
pub(super) const INVALID_GROUP_NAME: PatternError = err("invalid group name");
pub(super) const UNCLOSED_GROUP_NAME: PatternError =
    err("expected end of capture group name, but got end of pattern");
pub(super) const EMPTY_GROUP_NAME: PatternError = err("empty capture group names are not allowed");
pub(super) const FLAG_UNRECOGNIZED: PatternError = err("unrecognized inline flag");
pub(super) const FLAG_REPEATED_NEGATION: PatternError =
    err("inline flag negation cannot be repeated");
pub(super) const FLAG_DUPLICATE: PatternError = err("duplicate inline flag is not allowed");
pub(super) const FLAG_UNEXPECTED_EOF: PatternError =
    err("expected ':' or ')' to end inline flags, but got end of pattern");
pub(super) const FLAG_DANGLING_NEGATION: PatternError =
    err("inline flags cannot end with negation directive");
pub(super) const DECIMAL_NO_DIGITS: PatternError =
    err("expected decimal number, but found no digits");
pub(super) const DECIMAL_INVALID: PatternError = err("got invalid decimal number");
pub(super) const HEX_BRACE_INVALID_DIGIT: PatternError =
    err("expected hexadecimal number in braces, but got non-hex digit");
pub(super) const HEX_BRACE_UNEXPECTED_EOF: PatternError =
    err("expected hexadecimal number, but saw end of pattern before closing brace");
pub(super) const HEX_BRACE_EMPTY: PatternError =
    err("expected hexadecimal number in braces, but got no digits");
pub(super) const HEX_BRACE_INVALID: PatternError = err("got invalid hexadecimal number in braces");
pub(super) const HEX_FIXED_UNEXPECTED_EOF: PatternError =
    err("expected fixed length hexadecimal number, but saw end of pattern first");
pub(super) const HEX_FIXED_INVALID_DIGIT: PatternError =
    err("expected fixed length hexadecimal number, but got non-hex digit");
pub(super) const HEX_FIXED_INVALID: PatternError =
    err("got invalid fixed length hexadecimal number");
pub(super) const HEX_UNEXPECTED_EOF: PatternError =
    err("expected hexadecimal number, but saw end of pattern first");
pub(super) const ESCAPE_UNEXPECTED_EOF: PatternError =
    err("saw start of escape sequence, but saw end of pattern before it finished");
pub(super) const BACKREF_UNSUPPORTED: PatternError = err("backreferences are not supported");
pub(super) const UNICODE_CLASS_UNSUPPORTED: PatternError =
    err("Unicode character classes are not supported");
pub(super) const ESCAPE_UNRECOGNIZED: PatternError = err("unrecognized escape sequence");
pub(super) const UNCOUNTED_REP_SUB_MISSING: PatternError =
    err("uncounted repetition operator must be applied to a sub-expression");
pub(super) const COUNTED_REP_SUB_MISSING: PatternError =
    err("counted repetition operator must be applied to a sub-expression");
pub(super) const COUNTED_REP_UNCLOSED: PatternError =
    err("found unclosed counted repetition operator");
pub(super) const COUNTED_REP_MIN_UNCLOSED: PatternError =
    err("found incomplete and unclosed counted repetition operator");
pub(super) const COUNTED_REP_COMMA_UNCLOSED: PatternError =
    err("found counted repetition operator with a comma that is unclosed");
pub(super) const COUNTED_REP_MIN_MAX_UNCLOSED: PatternError =
    err("found counted repetition with min and max that is unclosed");
pub(super) const COUNTED_REP_INVALID: PatternError =
    err("expected closing brace for counted repetition, but got something else");
pub(super) const COUNTED_REP_INVALID_RANGE: PatternError =
    err("found counted repetition with a min bigger than its max");
pub(super) const CLASS_UNCLOSED_AFTER_ITEM: PatternError =
    err("non-empty character class has no closing bracket");
pub(super) const CLASS_INVALID_RANGE_ITEM: PatternError =
    err("character class ranges must start and end with a single character");
pub(super) const CLASS_INVALID_ITEM: PatternError =
    err("invalid escape sequence in character class");
pub(super) const CLASS_UNCLOSED_AFTER_DASH: PatternError =
    err("non-empty character class has no closing bracket after dash");
pub(super) const CLASS_UNCLOSED_AFTER_NEGATION: PatternError =
    err("negated character class has no closing bracket");
pub(super) const CLASS_UNCLOSED_AFTER_CLOSING: PatternError =
    err("character class begins with literal ']' but has no closing bracket");
pub(super) const CLASS_INVALID_RANGE: PatternError = err("invalid range in character class");
pub(super) const CLASS_UNCLOSED: PatternError = err("found unclosed character class");
pub(super) const CLASS_NEST_UNSUPPORTED: PatternError =
    err("nested character classes are not supported");
pub(super) const CLASS_INTERSECTION_UNSUPPORTED: PatternError =
    err("character class intersection is not supported");
pub(super) const CLASS_DIFFERENCE_UNSUPPORTED: PatternError =
    err("character class difference is not supported");
pub(super) const CLASS_SYMDIFFERENCE_UNSUPPORTED: PatternError =
    err("character class symmetric difference is not supported");
pub(super) const SPECIAL_WORD_BOUNDARY_UNCLOSED: PatternError =
    err("special word boundary assertion is unclosed or has an invalid character");
pub(super) const SPECIAL_WORD_BOUNDARY_UNRECOGNIZED: PatternError =
    err("special word boundary assertion is unrecognized");
pub(super) const SPECIAL_WORD_OR_REP_UNEXPECTED_EOF: PatternError =
    err("found start of special word boundary or repetition without an end");
