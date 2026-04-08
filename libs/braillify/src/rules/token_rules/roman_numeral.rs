use std::borrow::Cow;

use crate::rules::token::{Token, WordMeta, WordToken};
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};

const ROMAN_INDICATOR: u8 = 52; // ⠴
const ROMAN_CONTINUATION: u8 = 48; // ⠰
const UPPERCASE_SIGN: u8 = 32; // ⠠
const ROMAN_TERMINATOR: u8 = 50; // ⠲
const HYPHEN: u8 = crate::unicode::decode_unicode('⠤');

pub struct RomanNumeralRule;

fn is_upper_roman_char(c: char) -> bool {
    matches!(c, 'I' | 'V' | 'X')
}

fn is_lower_roman_char(c: char) -> bool {
    matches!(c, 'i' | 'v' | 'x')
}

fn roman_case(text: &str) -> Option<bool> {
    if text.chars().all(is_upper_roman_char) {
        return Some(true);
    }
    if text.chars().all(is_lower_roman_char) {
        return Some(false);
    }
    None
}

fn is_valid_roman_1_to_39(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    if roman_case(text).is_none() {
        return false;
    }

    let upper = text.to_ascii_uppercase();
    let mut chars = upper.chars().peekable();
    let mut x_count = 0usize;
    while chars.peek().is_some_and(|c| *c == 'X') {
        x_count += 1;
        chars.next();
    }
    if x_count > 3 {
        return false;
    }

    let rest: String = chars.collect();
    let ones_ok = matches!(
        rest.as_str(),
        "" | "I" | "II" | "III" | "IV" | "V" | "VI" | "VII" | "VIII" | "IX"
    );

    (x_count > 0 || !rest.is_empty()) && ones_ok
}

fn split_roman_prefix(text: &str) -> (&str, &str) {
    let mut split_at = 0usize;
    for (idx, ch) in text.char_indices() {
        if !is_upper_roman_char(ch) && !is_lower_roman_char(ch) {
            break;
        }
        split_at = idx + ch.len_utf8();
    }
    text.split_at(split_at)
}

fn split_after_hyphen(text: &str) -> Option<(&str, &str)> {
    if !text.starts_with('-') {
        return None;
    }
    let without_hyphen = &text['-'.len_utf8()..];
    let (roman, rest) = split_roman_prefix(without_hyphen);
    if roman.is_empty() {
        return None;
    }
    Some((roman, rest))
}

fn starts_with_ascii_alpha(text: &str) -> bool {
    text.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

fn has_prev_ascii_word<'a>(tokens: &[Token<'a>], index: usize) -> bool {
    tokens[..index].iter().rev().find_map(|token| match token {
        Token::Word(word) => Some(word.meta.has_ascii_alphabetic),
        _ => None,
    }) == Some(true)
}

fn build_word_token<'a>(text: &str) -> Token<'a> {
    let chars: Vec<char> = text.chars().collect();
    Token::Word(WordToken {
        text: Cow::Owned(text.to_string()),
        chars: chars.clone(),
        meta: WordMeta::from_chars(&chars),
    })
}

fn encode_roman_segment(text: &str, entry: u8, with_terminator: bool) -> Result<Vec<u8>, String> {
    let is_upper = roman_case(text).ok_or_else(|| format!("invalid roman case: {text}"))?;

    let mut out = Vec::new();
    out.push(entry);

    if is_upper {
        if text.chars().count() >= 2 {
            out.push(UPPERCASE_SIGN);
            out.push(UPPERCASE_SIGN);
        } else {
            out.push(UPPERCASE_SIGN);
        }
    }

    for ch in text.chars() {
        out.push(crate::english::encode_english(ch.to_ascii_lowercase())?);
    }

    if with_terminator {
        out.push(ROMAN_TERMINATOR);
    }

    Ok(out)
}

impl TokenRule for RomanNumeralRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::ModeEntry
    }

    fn priority(&self) -> u16 {
        5
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        state: &mut crate::rules::context::EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        let Some(Token::Word(word)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };

        let text = word.text.as_ref();

        let allow_standalone = state.english_indicator || text.chars().count() >= 2;
        if allow_standalone && is_valid_roman_1_to_39(text) {
            let bytes = encode_roman_segment(text, ROMAN_INDICATOR, true)?;
            return Ok(TokenAction::Replace(Token::PreEncoded(bytes)));
        }

        if !state.english_indicator {
            return Ok(TokenAction::Noop);
        }

        let (first, rest) = split_roman_prefix(text);
        if first.is_empty() || !is_valid_roman_1_to_39(first) {
            return Ok(TokenAction::Noop);
        }

        if let Some((second, suffix)) = split_after_hyphen(rest)
            && is_valid_roman_1_to_39(second)
            && !starts_with_ascii_alpha(suffix)
        {
            let first_entry = if has_prev_ascii_word(tokens, index) {
                ROMAN_CONTINUATION
            } else {
                ROMAN_INDICATOR
            };
            let mut bytes = encode_roman_segment(first, first_entry, false)?;
            bytes.push(HYPHEN);
            bytes.extend(encode_roman_segment(second, ROMAN_CONTINUATION, true)?);

            if suffix.is_empty() {
                return Ok(TokenAction::Replace(Token::PreEncoded(bytes)));
            }

            return Ok(TokenAction::ReplaceMany(vec![
                Token::PreEncoded(bytes),
                build_word_token(suffix),
            ]));
        }

        if starts_with_ascii_alpha(rest) {
            return Ok(TokenAction::Noop);
        }

        let entry = if has_prev_ascii_word(tokens, index) {
            ROMAN_CONTINUATION
        } else {
            ROMAN_INDICATOR
        };

        let bytes = encode_roman_segment(first, entry, true)?;
        if rest.is_empty() {
            return Ok(TokenAction::Replace(Token::PreEncoded(bytes)));
        }

        Ok(TokenAction::ReplaceMany(vec![
            Token::PreEncoded(bytes),
            build_word_token(rest),
        ]))
    }
}
