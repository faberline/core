use std::borrow::Cow;

use super::schema::Analyzer;

pub const DEFAULT_NGRAM_MIN: usize = 2;
pub const DEFAULT_NGRAM_MAX: usize = 3;

pub fn tokenize(text: &str, analyzer: Analyzer) -> Vec<String> {
    match analyzer {
        Analyzer::WhitespaceLower => {
            let mut out = Vec::new();
            for_whitespace_lower(text, |token| out.push(token));
            out
        }
        Analyzer::Jieba => jieba(text),
        Analyzer::Ngram => ngram(text, DEFAULT_NGRAM_MIN, DEFAULT_NGRAM_MAX),
    }
}

pub fn for_whitespace_lower(text: &str, mut emit: impl FnMut(String)) -> u32 {
    for_whitespace_lower_cow(text, |token| emit(token.into_owned()))
}

pub fn for_whitespace_lower_cow<'a>(mut text: &'a str, mut emit: impl FnMut(Cow<'a, str>)) -> u32 {
    let mut emitted = 0u32;
    while !text.is_empty() {
        let trimmed_start = text.trim_start();
        if trimmed_start.is_empty() {
            break;
        }
        text = trimmed_start;
        let end = text.find(char::is_whitespace).unwrap_or(text.len());
        let raw = &text[..end];
        text = &text[end..];
        let token = raw.trim_matches(|character: char| !character.is_alphanumeric());
        if token.is_empty() {
            continue;
        }
        emitted += 1;
        if token
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        {
            emit(Cow::Borrowed(token));
        } else {
            emit(Cow::Owned(token.to_lowercase()));
        }
    }
    emitted
}

#[cfg(feature = "jieba")]
fn jieba_dictionary() -> &'static jieba_rs::Jieba {
    use std::sync::OnceLock;
    static JIEBA: OnceLock<jieba_rs::Jieba> = OnceLock::new();
    JIEBA.get_or_init(jieba_rs::Jieba::new)
}

/// Storage for the exact no-HMM dictionary route. Callers may use a bounded
/// file cache instead of keeping a route and graph for the whole input in RAM.
#[cfg(feature = "jieba")]
pub use jieba_rs::RouteStore as JiebaRouteStore;

/// Visit the same lowercase, nonempty tokens as `tokenize(text, Jieba)`.
/// The shared dictionary is initialized once. The caller supplies route storage
/// and owns each emitted token only for the duration of its callback.
#[cfg(feature = "jieba")]
pub fn for_jieba_no_hmm(
    text: &str,
    route: &mut impl JiebaRouteStore,
    mut emit: impl FnMut(&str) -> std::io::Result<()>,
) -> std::io::Result<u32> {
    let mut count = 0u32;
    jieba_dictionary().cut_no_hmm_visit(text, route, |raw| {
        let token = raw.to_lowercase();
        if !token.trim().is_empty() {
            count = count.checked_add(1).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Jieba document length exceeds u32",
                )
            })?;
            emit(&token)?;
        }
        Ok(())
    })?;
    Ok(count)
}

#[cfg(all(test, feature = "jieba"))]
mod jieba_stream_tests {
    use super::*;
    use std::io;

    #[derive(Default)]
    struct Route(Vec<(f64, usize)>);
    impl JiebaRouteStore for Route {
        fn reset(&mut self, slots: usize) -> io::Result<()> {
            self.0.clear();
            // Initialization deliberately does not supply the terminal score.
            self.0.resize(slots, (f64::NAN, usize::MAX));
            Ok(())
        }
        fn get(&mut self, index: usize) -> io::Result<(f64, usize)> {
            Ok(self.0[index])
        }
        fn set(&mut self, index: usize, value: (f64, usize)) -> io::Result<()> {
            self.0[index] = value;
            Ok(())
        }
    }

    #[test]
    fn stream_matches_owned_jieba_tokens_counts_and_unicode_normalization() {
        let mut route = Route::default();
        for text in [
            "南京市长江大桥 ΣΟΣ İSTANBUL, Straße",
            "\r\n\t  中文。👪𠀀 ABC123+_#&.%-XYZ  ",
            "",
        ] {
            let mut tokens = Vec::new();
            let count = for_jieba_no_hmm(text, &mut route, |token| {
                tokens.push(token.to_owned());
                Ok(())
            })
            .unwrap();
            assert_eq!(tokens, tokenize(text, Analyzer::Jieba));
            assert_eq!(count as usize, tokens.len());
            assert!(route.0.is_empty());
        }
    }

    #[test]
    fn stream_returns_callback_error_without_emitting_another_token() {
        let mut route = Route::default();
        let mut calls = 0;
        let error = for_jieba_no_hmm("南京市长江大桥", &mut route, |_| {
            calls += 1;
            Err(io::Error::new(io::ErrorKind::Interrupted, "stop callback"))
        })
        .unwrap_err();
        assert_eq!(calls, 1);
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert!(route.0.is_empty());
    }
}

#[cfg(feature = "jieba")]
fn jieba(text: &str) -> Vec<String> {
    jieba_dictionary()
        .cut(text, false)
        .into_iter()
        .map(str::to_lowercase)
        .filter(|token| !token.trim().is_empty())
        .collect()
}

#[cfg(not(feature = "jieba"))]
fn jieba(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut tokens = Vec::new();
    let characters = trimmed.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < characters.len() {
        let cjk = is_cjk_char(characters[index]);
        let start = index;
        while index < characters.len() && is_cjk_char(characters[index]) == cjk {
            index += 1;
        }
        if cjk {
            let run = &characters[start..index];
            if run.len() == 1 {
                tokens.push(run[0].to_string());
            } else {
                for offset in 0..run.len() - 1 {
                    tokens.push(run[offset..offset + 2].iter().collect());
                }
            }
        } else {
            let run = characters[start..index].iter().collect::<String>();
            for_whitespace_lower(&run, |token| tokens.push(token));
        }
    }
    tokens
}

fn is_cjk_char(character: char) -> bool {
    let code = character as u32;
    (0x4E00..=0x9FFF).contains(&code)
        || (0x3400..=0x4DBF).contains(&code)
        || (0x3040..=0x309F).contains(&code)
        || (0x30A0..=0x30FF).contains(&code)
        || (0xAC00..=0xD7A3).contains(&code)
}

fn ngram(text: &str, min: usize, max: usize) -> Vec<String> {
    let characters = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect::<Vec<_>>();
    let mut tokens = Vec::new();
    for window in min..=max {
        if characters.len() < window {
            continue;
        }
        for start in 0..=characters.len() - window {
            tokens.push(characters[start..start + window].iter().collect());
        }
    }
    tokens
}
