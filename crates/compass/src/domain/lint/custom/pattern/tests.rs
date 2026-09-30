//! Expectations recorded from regex-lite 0.1.9, the engine custom rules used
//! before; a differential fuzz against it found no difference.

use super::{translate, RulePattern};

fn is_match(pattern: &str, haystack: &str) -> bool {
    RulePattern::new(pattern)
        .unwrap_or_else(|e| panic!("{pattern:?} should compile: {e}"))
        .is_match(haystack)
}

fn rejection(pattern: &str) -> String {
    match RulePattern::new(pattern) {
        Ok(_) => panic!("{pattern:?} should be rejected"),
        Err(e) => e.to_string(),
    }
}

fn check(cases: &[(&str, &str, bool)]) {
    for &(pattern, haystack, expected) in cases {
        assert_eq!(
            is_match(pattern, haystack),
            expected,
            "{pattern:?} on {haystack:?}"
        );
    }
}

#[test]
fn perl_classes_are_ascii() {
    check(&[
        (r"\d", "\u{660}", false),
        (r"\d", "7", true),
        (r"\D", "\u{660}", true),
        (r"\w", "é", false),
        (r"\w", "_", true),
        (r"\W", "é", true),
        (r"\s", "\u{A0}", false),
        (r"\s", "\u{2028}", false),
        (r"\s", "\u{B}", true),
        (r"\S", "\u{A0}", true),
        (r"^\w+$", "café", false),
    ]);
}

#[test]
fn bracket_classes_are_ascii() {
    check(&[
        (r"^[\w]+$", "café", false),
        (r"^[^\W]+$", "café", false),
        (r"^[\d\s]+$", "1 \u{660}", false),
        (r"[[:alpha:]]", "é", false),
    ]);
}

#[test]
fn word_boundaries_are_ascii() {
    check(&[
        (r"\bcaf", "écaf", true),
        (r"\bcaf", "_caf", false),
        (r"é\b", "é", false),
        (r"\<é", "é", false),
        (r"a\Bb", "ab", true),
        (r"\B", "zω0", false),
        (r"\b{start-half}", "a", true),
    ]);
}

#[test]
fn a_not_word_boundary_inside_a_character_does_not_hide_a_match() {
    // regex's own `is_match` answers false here.
    check(&[(r"\B|ω", "zω0", true)]);
}

#[test]
fn case_insensitivity_is_ascii() {
    check(&[
        (r"(?i)k", "\u{212A}", false),
        (r"(?i)s", "\u{17F}", false),
        (r"(?i)é", "É", false),
        (r"(?i)ß", "\u{1E9E}", false),
        (r"(?i)K", "k", true),
        (r"(?i)[a-z]", "\u{212A}", false),
        (r"(?i)[a-z]", "Q", true),
        (r"(?i)[^a-z]", "Q", false),
        (r"(?i)[^k]", "\u{212A}", true),
        (r"(?i)\x{212A}", "k", false),
    ]);
}

#[test]
fn flags_and_anchors() {
    check(&[
        (r".", "\n", false),
        (r"(?s).", "\n", true),
        (r"(?R).", "\r", false),
        (r"(?m)^b", "a\nb", true),
        (r"(?mR)a$", "a\r\n", true),
        (r"(?x) a b # comment", "ab", true),
        (r"(?U)a+", "a", true),
    ]);
}

#[test]
fn everyday_rules_match_as_before() {
    check(&[
        (r"TODO", "// TODO: fix", true),
        (r"println!\(", "    println!(\"x\");", true),
    ]);
}

#[test]
fn translation_spells_out_ascii_classes() {
    assert_eq!(translate(r"\d").unwrap(), r"[\x{30}-\x{39}]");
    assert_eq!(translate(r"(?i)k").unwrap(), r"[\x{4B}\x{6B}]");
    assert_eq!(translate(r"\bab").unwrap(), r"(?-u:\b)ab");
    assert_eq!(translate(r"a{2,}?").unwrap(), r"(?:a){2,}?");
}

#[test]
fn rejections_keep_regex_lite_messages() {
    let cases = [
        (r"\p{L}", "Unicode character classes are not supported"),
        (r"(?=a)", "look-around is not supported"),
        (
            r"a{2,1}",
            "found counted repetition with a min bigger than its max",
        ),
        (r"[a", "non-empty character class has no closing bracket"),
        (r"(?<n>a)(?<n>b)", "duplicate capture group name"),
        (r"\1", "backreferences are not supported"),
        (r"(?ii)a", "duplicate inline flag is not allowed"),
        (r"[[a]]", "nested character classes are not supported"),
        (r"[a&&b]", "character class intersection is not supported"),
        (r"\q", "unrecognized escape sequence"),
        (
            r"*",
            "uncounted repetition operator must be applied to a sub-expression",
        ),
        (r"(a", "found open group without closing ')'"),
        (r"a)", "found closing ')' without matching '('"),
        (
            r"\b{foo}",
            "special word boundary assertion is unrecognized",
        ),
        (r"\x{110000}", "got invalid hexadecimal number in braces"),
        (r"(?)", "empty flag directive '(?)' is not allowed"),
        (r"a{1000}{1000}", "compiled regex exceeded size limit"),
    ];
    for (pattern, message) in cases {
        assert_eq!(rejection(pattern), message, "{pattern:?}");
    }
}

#[test]
fn nesting_limit_is_regex_lite_s() {
    for (depth, accepted) in [(50, true), (51, false)] {
        let groups = format!("{}a{}", "(".repeat(depth), ")".repeat(depth));
        let stars = format!("a{}", "*".repeat(depth));
        for pattern in [groups, stars] {
            match RulePattern::new(&pattern) {
                Ok(_) => assert!(accepted, "{pattern:?} should be rejected"),
                Err(e) => {
                    assert!(!accepted, "{pattern:?}: {e}");
                    assert_eq!(e.to_string(), "pattern has too much nesting");
                }
            }
        }
    }
}
