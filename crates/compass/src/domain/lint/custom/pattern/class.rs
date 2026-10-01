//! Code point sets for character classes: regex-lite's canonical form, its
//! ASCII-only case folding, and its POSIX and Perl classes.

/// Inclusive code point ranges.
pub(super) type Ranges = Vec<(char, char)>;

/// Sort the ranges and merge the ones that overlap or touch.
pub(super) fn canonicalize(mut ranges: Ranges) -> Ranges {
    ranges.sort_unstable();
    let mut merged: Ranges = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        if let Some(last) = merged.last_mut() {
            if u32::from(start.max(last.0)) <= u32::from(end.min(last.1)).saturating_add(1) {
                *last = (last.0.min(start), last.1.max(end));
                continue;
            }
        }
        merged.push((start, end));
    }
    merged
}

/// The other-case range of the ASCII letters in `start..=end`, as regex-lite
/// computes it: lowercase letters win, and only one range is returned.
pub(super) fn fold_range(start: char, end: char) -> Option<(char, char)> {
    if start <= 'z' && end >= 'a' {
        return Some((shift(start.max('a'), false), shift(end.min('z'), false)));
    }
    if start <= 'Z' && end >= 'A' {
        return Some((shift(start.max('A'), true), shift(end.min('Z'), true)));
    }
    None
}

/// Swap the case of an ASCII letter.
fn shift(letter: char, up: bool) -> char {
    let byte = letter as u8;
    char::from(if up { byte + 32 } else { byte - 32 })
}

/// Add the ASCII other-case letters of every range, then canonicalize.
pub(super) fn ascii_case_fold(ranges: Ranges) -> Ranges {
    let mut folded = ranges.clone();
    folded.extend(
        ranges
            .iter()
            .filter_map(|&(start, end)| fold_range(start, end)),
    );
    canonicalize(folded)
}

/// The complement of canonical `ranges` over all code points.
pub(super) fn negate(ranges: &[(char, char)]) -> Ranges {
    let (Some(&(first, _)), Some(&(_, last))) = (ranges.first(), ranges.last()) else {
        return vec![('\0', char::MAX)];
    };
    let mut out = Vec::with_capacity(ranges.len() + 1);
    if first > '\0' {
        out.push(('\0', prev_char(first)));
    }
    for pair in ranges.windows(2) {
        out.push((next_char(pair[0].1), prev_char(pair[1].0)));
    }
    if last < char::MAX {
        out.push((next_char(last), char::MAX));
    }
    out
}

fn next_char(c: char) -> char {
    if c == '\u{D7FF}' {
        return '\u{E000}';
    }
    char::from_u32(u32::from(c) + 1).unwrap_or(char::MAX)
}

fn prev_char(c: char) -> char {
    if c == '\u{E000}' {
        return '\u{D7FF}';
    }
    char::from_u32(u32::from(c).saturating_sub(1)).unwrap_or('\0')
}

/// The ranges of a POSIX class such as `alnum`, all ASCII.
pub(super) fn posix(name: &str) -> Option<Ranges> {
    let ranges: &[(u8, u8)] = match name {
        "alnum" => &[(b'0', b'9'), (b'A', b'Z'), (b'a', b'z')],
        "alpha" => &[(b'A', b'Z'), (b'a', b'z')],
        "ascii" => &[(b'\x00', b'\x7F')],
        "blank" => &[(b'\t', b'\t'), (b' ', b' ')],
        "cntrl" => &[(b'\x00', b'\x1F'), (b'\x7F', b'\x7F')],
        "digit" => &[(b'0', b'9')],
        "graph" => &[(b'!', b'~')],
        "lower" => &[(b'a', b'z')],
        "print" => &[(b' ', b'~')],
        "punct" => &[(b'!', b'/'), (b':', b'@'), (b'[', b'`'), (b'{', b'~')],
        "space" => &[(b'\t', b'\r'), (b' ', b' ')],
        "upper" => &[(b'A', b'Z')],
        "word" => &[(b'0', b'9'), (b'A', b'Z'), (b'_', b'_'), (b'a', b'z')],
        "xdigit" => &[(b'0', b'9'), (b'A', b'F'), (b'a', b'f')],
        _ => return None,
    };
    Some(
        ranges
            .iter()
            .map(|&(start, end)| (char::from(start), char::from(end)))
            .collect(),
    )
}

/// The ASCII ranges of `\d`, `\s` or `\w`, negated for `\D`, `\S` or `\W`.
pub(super) fn perl(name: char) -> Ranges {
    let base = match name.to_ascii_lowercase() {
        'd' => "digit",
        's' => "space",
        _ => "word",
    };
    let ranges = posix(base).unwrap_or_default();
    if name.is_ascii_uppercase() {
        negate(&ranges)
    } else {
        ranges
    }
}
