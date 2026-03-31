//! 제49항 — 문장 부호는 다음과 같이 적는다.
//!
//! Symbol/punctuation encoding via `symbol_shortcut::encode_char_symbol_shortcut()` (PHF).
//! Includes: period, comma, question mark, exclamation, quotes, brackets, etc.
//!
//! English-specific symbol variants via `symbol_shortcut::encode_english_char_symbol_shortcut()`.
//!
//! Reference: 2024 Korean Braille Standard, Chapter 6, Section 13, Article 49

use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};
use crate::symbol_shortcut;
use crate::unicode::decode_unicode;

pub static META: RuleMeta = RuleMeta {
    section: "49",
    subsection: None,
    name: "punctuation_encoding",
    standard_ref: "2024 Korean Braille Standard, Ch.6 Sec.13 Art.49",
    description: "Punctuation marks encoded to braille dot patterns",
};

/// Encode a punctuation/symbol character to braille (Korean context).
#[cfg(test)]
fn apply(ch: char) -> Result<&'static [u8], String> {
    symbol_shortcut::encode_char_symbol_shortcut(ch)
}

/// Encode a punctuation/symbol in English context (different dot patterns for (, ), ,).
#[cfg(test)]
fn apply_english(ch: char) -> Option<&'static [u8]> {
    symbol_shortcut::encode_english_char_symbol_shortcut(ch)
}

/// Check if a character is a recognized symbol.
#[cfg(test)]
fn is_symbol(ch: char) -> bool {
    symbol_shortcut::is_symbol_char(ch)
}

/// Plugin struct for the rule engine.
///
/// Handles the base case of symbol/punctuation encoding (제49항).
/// Special cases (comma in numbers, blank marks, asterisks) are handled
/// by dedicated rules (rule_41, rule_58, rule_60) which run at higher priority.
pub struct Rule49;

impl BrailleRule for Rule49 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        500 // Low priority — fallback after special-case symbol rules
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::Symbol(_))
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let CharType::Symbol(c) = ctx.char_type else {
            return Ok(RuleResult::Skip);
        };

        // 제49항 [붙임]: 문장 부호를 낱말처럼 설명할 때
        // 물음표는 기호표(⠸) + 해당 기호 + 점역자 주표(⠠⠄ ... ⠠⠄)를 사용한다.
        if *c == '?' && ctx.index == 0 {
            let prev_word_is_korean = ctx.prev_word.chars().any(crate::utils::is_korean_char);
            let next_word_is_korean = ctx
                .remaining_words
                .first()
                .is_some_and(|w| w.chars().any(crate::utils::is_korean_char));

            if !ctx.has_korean_char && !prev_word_is_korean && !next_word_is_korean {
                let encoded = symbol_shortcut::encode_char_symbol_shortcut(*c)?;
                ctx.emit_slice(encoded);
                return Ok(RuleResult::Consumed);
            }

            let next_is_korean_or_end = ctx.next_char().is_none_or(crate::utils::is_korean_char);
            if next_is_korean_or_end {
                ctx.emit(decode_unicode('⠸'));
                let encoded = symbol_shortcut::encode_char_symbol_shortcut(*c)?;
                ctx.emit_slice(encoded);
                ctx.emit(0);
                ctx.emit(decode_unicode('⠠'));
                ctx.emit(decode_unicode('⠄'));
                // "물음표"
                ctx.emit_slice(&[
                    decode_unicode('⠑'),
                    decode_unicode('⠯'),
                    decode_unicode('⠪'),
                    decode_unicode('⠢'),
                    decode_unicode('⠙'),
                    decode_unicode('⠬'),
                ]);
                ctx.emit(decode_unicode('⠠'));
                ctx.emit(decode_unicode('⠄'));
                return Ok(RuleResult::Consumed);
            }
        }

        // ASCII apostrophe context-sensitive open/close rendering.
        // open:  ⠠⠦, close: ⠴⠄
        if *c == '\'' {
            let is_close = ctx.prev_char().is_some();
            if is_close {
                ctx.emit_slice(&[decode_unicode('⠴'), decode_unicode('⠄')]);
            } else {
                ctx.emit_slice(&[decode_unicode('⠠'), decode_unicode('⠦')]);
            }
            return Ok(RuleResult::Consumed);
        }

        // ASCII double quote context-sensitive open/close rendering.
        // open: ⠦, close: ⠴
        if *c == '"' && ctx.next_char() != Some('˙') {
            let is_close = ctx.prev_char().is_some();
            if is_close {
                ctx.emit(decode_unicode('⠴'));
            } else {
                ctx.emit(decode_unicode('⠦'));
            }
            return Ok(RuleResult::Consumed);
        }

        // 제56항 입력 표기(인쇄 부호 잔존) 호환:
        // "˙,  __" 형태를 강조 시작/종결 표기로 해석한다.
        if *c == '"' && ctx.next_char() == Some('˙') {
            ctx.emit_slice(&[decode_unicode('⠠'), decode_unicode('⠤')]);
            *ctx.skip_count = 2; // skip ˙,
            return Ok(RuleResult::Consumed);
        }
        if *c == '_'
            && ctx.next_char() == Some('_')
            && ctx.word_chars.get(ctx.index + 2) == Some(&'"')
        {
            ctx.emit_slice(&[decode_unicode('⠤'), decode_unicode('⠄')]);
            *ctx.skip_count = 2; // skip _"
            return Ok(RuleResult::Consumed);
        }

        if *c == '×'
            && ctx.word_len() == 1
            && ctx.prev_word.is_empty()
            && ctx.remaining_words.is_empty()
        {
            ctx.emit_slice(&[
                decode_unicode('⠸'),
                decode_unicode('⠭'),
                decode_unicode('⠇'),
            ]);
            return Ok(RuleResult::Consumed);
        }

        let encoded = symbol_shortcut::encode_char_symbol_shortcut(*c)?;
        ctx.emit_slice(encoded);

        if ctx.word_len() == 1
            && ctx.prev_word.is_empty()
            && ctx.remaining_words.is_empty()
            && matches!(*c, '(' | '〈' | '―' | '-')
        {
            ctx.emit(0);
        }

        Ok(RuleResult::Consumed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unicode::decode_unicode;

    #[test]
    fn encodes_basic_punctuation() {
        assert_eq!(apply('.').unwrap(), &[decode_unicode('⠲')]);
        assert_eq!(apply(',').unwrap(), &[decode_unicode('⠐')]);
        assert_eq!(apply('?').unwrap(), &[decode_unicode('⠦')]);
        assert_eq!(apply('!').unwrap(), &[decode_unicode('⠖')]);
    }

    #[test]
    fn encodes_brackets() {
        assert_eq!(
            apply('(').unwrap(),
            &[decode_unicode('⠦'), decode_unicode('⠄')]
        );
        assert_eq!(
            apply(')').unwrap(),
            &[decode_unicode('⠠'), decode_unicode('⠴')]
        );
    }

    #[test]
    fn english_parentheses_different() {
        let eng = apply_english('(').unwrap();
        let kor = apply('(').unwrap();
        assert_ne!(eng, kor, "English and Korean parentheses should differ");
    }

    #[test]
    fn is_symbol_detection() {
        assert!(is_symbol('.'));
        assert!(is_symbol('?'));
        assert!(is_symbol('('));
        assert!(!is_symbol('A'));
        assert!(!is_symbol('가'));
    }

    #[test]
    fn unknown_symbol_returns_error() {
        assert!(apply('@').is_err());
    }
}
