use std::borrow::Cow;

use crate::rules::token::{Token, WordMeta, WordToken};
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};
use crate::utils::is_korean_char;

/// PDF 제39항: 영어가 주된 문장 안에 나온 한글 어절을 한글표/한글 종료표로 감싼다.
///
/// 이 규칙은 이전 회귀를 피하기 위해 의도적으로 좁게 동작한다.
/// - 문장 전체에서 영어 어절이 2개 이상이어야 한다.
/// - 첫 번째 비자명 어절의 시작 문자가 영어여야 한다.
/// - 감싸는 대상은 한글 어절 본체이며, 앞뒤에 붙은 문장부호만 분리 허용한다.
/// - ASCII가 섞인 `www.대통령.kr` 같은 혼합 어절은 여기서 건드리지 않는다.
const HANGUL_WRAP_START: [u8; 2] = [56, 55]; // internal: "_("
const HANGUL_WRAP_END: [u8; 2] = [56, 62]; // internal: "_)"

pub struct EnglishDominantKoreanWrapRule;

#[derive(Debug, Default, Clone, Copy)]
struct SentenceProfile {
    english_words: usize,
    korean_words: usize,
    first_non_trivial_is_english: bool,
    has_first_non_trivial: bool,
}

fn build_word_token<'a>(text: &str) -> Token<'a> {
    let chars: Vec<char> = text.chars().collect();

    Token::Word(WordToken {
        text: Cow::Owned(text.to_string()),
        chars: chars.clone(),
        meta: WordMeta::from_chars(&chars),
    })
}

fn first_non_trivial_char(word: &WordToken<'_>) -> Option<char> {
    word.chars
        .iter()
        .copied()
        .find(|ch| ch.is_ascii_alphanumeric() || is_korean_char(*ch))
}

fn is_punctuation_only(text: &str) -> bool {
    text.chars()
        .all(|ch| !ch.is_ascii_alphanumeric() && !is_korean_char(ch))
}

fn scan_sentence_profile<'a>(tokens: &[Token<'a>]) -> SentenceProfile {
    let mut profile = SentenceProfile::default();

    for token in tokens {
        let Token::Word(word) = token else {
            continue;
        };

        let Some(first_char) = first_non_trivial_char(word) else {
            continue;
        };

        if !profile.has_first_non_trivial {
            profile.first_non_trivial_is_english = first_char.is_ascii_alphabetic();
            profile.has_first_non_trivial = true;
        }

        if first_char.is_ascii_alphabetic() {
            profile.english_words += 1;
        }

        if is_korean_char(first_char) {
            profile.korean_words += 1;
        }
    }

    profile
}

fn should_activate<'a>(tokens: &[Token<'a>]) -> bool {
    let profile = scan_sentence_profile(tokens);

    profile.english_words >= 2 && profile.korean_words > 0 && profile.first_non_trivial_is_english
}

fn split_wrappable_korean_word(text: &str) -> Option<(&str, &str, &str)> {
    let mut start = None;
    let mut end = 0usize;

    for (idx, ch) in text.char_indices() {
        if is_korean_char(ch) {
            if start.is_none() {
                start = Some(idx);
            }
            end = idx + ch.len_utf8();
            continue;
        }

        if start.is_some() {
            break;
        }
    }

    let start = start?;
    let prefix = &text[..start];
    let korean = &text[start..end];
    let suffix = &text[end..];

    if korean.is_empty() || !is_punctuation_only(prefix) || !is_punctuation_only(suffix) {
        return None;
    }

    Some((prefix, korean, suffix))
}

impl TokenRule for EnglishDominantKoreanWrapRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::ModeEntry
    }

    fn priority(&self) -> u16 {
        10
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        _state: &mut crate::rules::context::EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        let Some(Token::Word(word)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };

        if !word.meta.has_korean || !should_activate(tokens) {
            return Ok(TokenAction::Noop);
        }

        let Some((prefix, korean, suffix)) = split_wrappable_korean_word(word.text.as_ref()) else {
            return Ok(TokenAction::Noop);
        };

        let mut replacement = Vec::new();

        if !prefix.is_empty() {
            replacement.push(build_word_token(prefix));
        }

        replacement.push(Token::PreEncoded(HANGUL_WRAP_START.to_vec()));
        replacement.push(build_word_token(korean));
        replacement.push(Token::PreEncoded(HANGUL_WRAP_END.to_vec()));

        if !suffix.is_empty() {
            replacement.push(build_word_token(suffix));
        }

        Ok(TokenAction::ReplaceMany(replacement))
    }
}
