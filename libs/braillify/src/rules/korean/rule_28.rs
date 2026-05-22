//! 제28항 — 로마자는 ｢통일영어점자 규정｣에 따라 다음과 같이 적는다.
//!
//! English letters are mapped to braille using the UEB (Unified English Braille) system.
//! Uppercase indicators: single ⠠(32), word ⠠⠠(32,32), passage ⠠⠠⠠(32,32,32).
//!
//! Encoding is delegated to `english::encode_english()`.
//!
//! Reference: 2024 Korean Braille Standard, Chapter 4, Section 10, Article 28

use crate::char_struct::CharType;
use crate::english;
use crate::rule_en::{rule_en_10_4, rule_en_10_5_whole_word, rule_en_10_6, rule_en_multi_cell};
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta { section: "28", subsection: None, name: "english_encoding", standard_ref: "2024 Korean Braille Standard, Ch.4 Sec.10 Art.28", description: "English letters encoded per UEB (Unified English Braille)" };

/// Single uppercase indicator (대문자 기호표).
pub const UPPERCASE_SINGLE: u8 = 32; // ⠠

/// Encode a single English letter to braille.
#[cfg(test)]
fn apply(ch: char) -> Result<u8, String> {
    english::encode_english(ch)
}

/// Returns a slice of indicator bytes to prepend.
#[cfg(test)]
fn uppercase_indicators(is_single_uppercase: bool, is_word_all_uppercase: bool, consecutive_uppercase_words: u8) -> &'static [u8] {
    if consecutive_uppercase_words >= 3 {
        &[32, 32, 32] // passage: ⠠⠠⠠
    } else if is_word_all_uppercase {
        &[32, 32] // word: ⠠⠠
    } else if is_single_uppercase {
        &[32] // single: ⠠
    } else {
        &[]
    }
}

/// Plugin struct for the rule engine.
///
/// Handles basic English letter encoding (제28항).
/// Uppercase indicators and English abbreviations are separate concerns
/// handled during ModeManagement and by rule_en rules.
pub struct Rule28;

impl BrailleRule for Rule28 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::English(_))
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let CharType::English(c) = ctx.char_type else {
            return Ok(RuleResult::Skip);
        };

        // Enter English mode (로마자표 / 연속표)
        // 제39항 영어 주도 문서에서는 영자표시/연속표를 emit하지 않는다.
        if ctx.state.english_indicator && !ctx.state.is_english && !ctx.state.english_dominant_no_indicator {
            if ctx.state.needs_english_continuation {
                ctx.emit(48);
            } else {
                ctx.emit(52);
            }
        }

        // Uppercase indicators (single/consecutive uppercase run)
        if (!ctx.is_all_uppercase || ctx.word_len() < 2 || !ctx.ascii_starts_at_beginning) && !ctx.state.is_big_english && c.is_uppercase() {
            ctx.state.is_big_english = true;
            for idx in 0..std::cmp::min(ctx.word_len() - ctx.index, 2) {
                if ctx.word_chars[ctx.index + idx].is_uppercase() {
                    ctx.emit(UPPERCASE_SINGLE);
                } else {
                    break;
                }
            }
        }

        // English abbreviation lookup + fallback letter encoding.
        //
        // Rule28 only fires when `ctx.char_type` is `CharType::English(_)`, so the
        // current character is ASCII. Non-ASCII trailing characters (e.g. Korean
        // following an English run) are not lowercase-affected by the lookup tables,
        // so `to_ascii_lowercase` per char is equivalent to the previous
        // `.collect::<String>().to_lowercase()` for any input that reaches the
        // lookup matchers — and avoids the second allocation + Unicode tables.
        let remaining: String = ctx.word_chars[ctx.index..].iter().map(|c| c.to_ascii_lowercase()).collect();
        let is_whole_lowercase_word = ctx.index == 0 && ctx.word_chars.iter().all(|ch| ch.is_ascii_lowercase());
        let be_boundary_non_alpha = remaining.starts_with("be") && remaining.chars().nth(2).is_none_or(|ch| !ch.is_ascii_alphabetic());
        let in_boundary_non_alpha = remaining.starts_with("in") && remaining.chars().nth(2).is_none_or(|ch| !ch.is_ascii_alphabetic());
        let prev_is_ascii_word = !ctx.prev_word.is_empty() && ctx.prev_word.chars().all(|ch| ch.is_ascii_alphabetic());
        let next_is_ascii_word = ctx.remaining_words.first().is_some_and(|w| !w.is_empty() && w.chars().all(|ch| ch.is_ascii_alphabetic()));

        if is_whole_lowercase_word && remaining == "you" && prev_is_ascii_word && next_is_ascii_word {
            ctx.emit(english::encode_english('y')?);
            *ctx.skip_count = ctx.word_len().saturating_sub(1);
            ctx.state.is_english = true;
            ctx.state.needs_english_continuation = false;
            return Ok(RuleResult::Consumed);
        }
        // Title case word ("Part", "Every") 도 whole-word contraction을 적용한다.
        // 모두 소문자 → contraction만 emit; 첫 대문자 + 나머지 소문자 → ⠠(대문자 표시) + contraction.
        // 모두 대문자(CD, KBS 등)는 약자 자체이므로 contraction 적용 안 함.
        let is_title_case_word = ctx.index == 0 && !ctx.is_all_uppercase && ctx.word_chars.first().is_some_and(|ch| ch.is_ascii_uppercase()) && ctx.word_chars.iter().skip(1).all(|ch| ch.is_ascii_lowercase()) && ctx.word_chars.len() >= 2;
        if ctx.index == 0
            && !ctx.is_all_uppercase
            && (is_whole_lowercase_word || is_title_case_word)
            && let Some(cells) = rule_en_10_5_whole_word(&remaining)
        {
            ctx.emit_slice(cells);
            *ctx.skip_count = ctx.word_len().saturating_sub(1);
            ctx.state.is_english = true;
            ctx.state.needs_english_continuation = false;
            return Ok(RuleResult::Consumed);
        }

        // 제39항 영-한 wrap 활성 컨텍스트에서는 단독 단어 "in", "be"도
        // UEB 약자를 적용한다 (예: "What is 김치 in English?"의 "in" → ⠔).
        let wrap_active = ctx.state.english_dominant_wrap_active;
        let allow_10_6 = !(ctx.is_all_uppercase || (!wrap_active && be_boundary_non_alpha) || (!wrap_active && in_boundary_non_alpha) || (!wrap_active && is_whole_lowercase_word && matches!(remaining.as_str(), "be" | "in")));
        let allow_10_4_entry = !(ctx.is_all_uppercase || (!wrap_active && in_boundary_non_alpha) || (!wrap_active && is_whole_lowercase_word && remaining == "in"));
        let allow_10_4_cont = !((!wrap_active && in_boundary_non_alpha) || (!wrap_active && is_whole_lowercase_word && remaining == "in"));

        if !ctx.state.is_english || ctx.index == 0 {
            if allow_10_6 && let Some((code, len)) = rule_en_10_6(&remaining) {
                ctx.emit(code);
                *ctx.skip_count = len;
            } else if allow_10_4_entry && let Some((code, len)) = rule_en_10_4(&remaining) {
                ctx.emit(code);
                *ctx.skip_count = len;
            } else if let Some((cells, len)) = rule_en_multi_cell(&remaining) {
                // multi-cell 약자 (예: 'ong' → ⠰⠛)는 영어 모드 진입 위치에서도 적용.
                ctx.emit_slice(cells);
                *ctx.skip_count = len;
            } else {
                ctx.emit(english::encode_english(*c)?);
            }
        } else if allow_10_4_cont && let Some((code, len)) = rule_en_10_4(&remaining) {
            ctx.emit(code);
            *ctx.skip_count = len;
        } else if let Some((cells, len)) = rule_en_multi_cell(&remaining) {
            // multi-cell 약자 ('ong' → ⠰⠛)는 word middle에서도 적용. 예: "along" → ⠁⠇⠰⠛.
            ctx.emit_slice(cells);
            *ctx.skip_count = len;
        } else if wrap_active
            && allow_10_6
            && let Some((code, len)) = rule_en_10_6(&remaining)
        {
            // 제39항 영-한 wrap context에서는 word middle에서도 1급 점자 기호표
            // 하위 약자(10.6: ea, be, con, en, in)를 적용한다.
            // 예: "Korean"의 'ea' → ⠂.
            ctx.emit(code);
            *ctx.skip_count = len;
        } else {
            ctx.emit(english::encode_english(*c)?);
        }

        ctx.state.is_english = true;
        ctx.state.needs_english_continuation = false;
        Ok(RuleResult::Consumed)
    }
}

/// Determine the uppercase indicator(s) needed.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::unicode::decode_unicode;

    #[test]
    fn encodes_lowercase_letters() {
        assert_eq!(apply('a').unwrap(), decode_unicode('⠁'));
        assert_eq!(apply('z').unwrap(), decode_unicode('⠵'));
    }

    #[test]
    fn encodes_uppercase_as_lowercase() {
        // encode_english lowercases internally
        assert_eq!(apply('A').unwrap(), decode_unicode('⠁'));
    }

    #[test]
    fn invalid_returns_error() {
        assert!(apply('1').is_err());
        assert!(apply('가').is_err());
    }

    #[test]
    fn uppercase_indicator_single() {
        assert_eq!(uppercase_indicators(true, false, 0), &[32]);
    }

    #[test]
    fn uppercase_indicator_word() {
        assert_eq!(uppercase_indicators(false, true, 0), &[32, 32]);
    }

    #[test]
    fn uppercase_indicator_passage() {
        assert_eq!(uppercase_indicators(false, true, 3), &[32, 32, 32]);
    }

    #[test]
    fn no_indicator_for_lowercase() {
        assert_eq!(uppercase_indicators(false, false, 0), &[] as &[u8]);
    }
}
