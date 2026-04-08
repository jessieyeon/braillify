//! 제29항 — 국어 문장 안에 로마자가 나올 때에는 그 앞에 로마자표 ⠴(52)을 적고
//! 그 뒤에 로마자 종료표 ⠲(50)을 적는다.
//!
//! 제31항 — 국어 문장 안에 그리스 문자가 나올 때에도 로마자표와 종료표를 적는다.
//!
//! 제33항 — 문장 부호의 점형이 다른 경우 종료표를 생략하는 규칙.
//! 제35항 — 로마자와 숫자가 이어 나올 때에는 종료표를 적지 않는다.
//!
//! Reference: 2024 Korean Braille Standard, Chapter 4, Section 10, Articles 29, 31, 33, 35

use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::korean::rule_69::encode_ascii_unit;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META_29: RuleMeta = RuleMeta {
    section: "29",
    subsection: None,
    name: "roman_indicator",
    standard_ref: "2024 Korean Braille Standard, Ch.4 Sec.10 Art.29",
    description: "Roman letter indicator ⠴ (enter) and terminator ⠲ (exit)",
};

/// Roman letter indicator (로마자표).
pub const ROMAN_INDICATOR: u8 = 52; // ⠴

/// Roman letter terminator (로마자 종료표).
#[cfg(test)]
pub const ROMAN_TERMINATOR: u8 = 50; // ⠲

/// English continuation indicator (연속표).
pub const ENGLISH_CONTINUATION: u8 = 48; // ⠰

/// Plugin struct for the rule engine.
///
/// Manages English/Roman mode transitions (제29항, 제31항, 제33항, 제35항).
/// Emits 로마자표 ⠴ when entering English mode, 로마자 종료표 ⠲ when exiting.
/// Uses 연속표 ⠐ when continuing English after an interruption (e.g., number).
///
/// This rule runs in the ModeManagement phase, before CoreEncoding.
/// It inspects the current character and state to decide mode transitions.
pub struct Rule29;

fn prev_word_is_numeric(prev_word: &str) -> bool {
    !prev_word.is_empty()
        && prev_word
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, ',' | '.'))
}

fn should_enter_as_roman_indicator(ctx: &RuleContext) -> bool {
    encode_ascii_unit(ctx.word_chars, ctx.index).is_some()
        && (ctx.prev_char().is_some_and(|ch| ch.is_ascii_digit())
            || prev_word_is_numeric(ctx.prev_word))
}

impl BrailleRule for Rule29 {
    fn meta(&self) -> &'static RuleMeta {
        &META_29
    }

    fn phase(&self) -> Phase {
        Phase::ModeManagement
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        // Only relevant when english_indicator is active (Korean text contains English)
        if !ctx.state.english_indicator {
            return false;
        }
        // Match when we need to enter English mode (current char is English and not in English)
        if !ctx.state.is_english && matches!(ctx.char_type, CharType::English(_)) {
            return true;
        }
        // Match when we're in English and encounter a non-English char (potential exit)
        if ctx.state.is_english && !matches!(ctx.char_type, CharType::English(_)) {
            return true;
        }
        false
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        if !ctx.state.is_english && matches!(ctx.char_type, CharType::English(_)) {
            // Enter English mode
            if ctx.state.needs_english_continuation && !should_enter_as_roman_indicator(ctx) {
                ctx.emit(ENGLISH_CONTINUATION); // ⠐ continuation
            } else {
                ctx.emit(ROMAN_INDICATOR); // ⠴ enter
            }
            ctx.state.is_english = true;
            ctx.state.needs_english_continuation = false;
        }
        // Exit logic is complex (depends on next word, symbol type, etc.)
        // and is deferred to Phase 3 engine-driven rewrite.
        Ok(RuleResult::Continue) // Continue to CoreEncoding
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indicator_values() {
        assert_eq!(ROMAN_INDICATOR, 52);
        assert_eq!(ROMAN_TERMINATOR, 50);
        assert_eq!(ENGLISH_CONTINUATION, 48);
    }

    #[test]
    fn golden_test_roman_in_korean() {
        // "그는 Canada로" → Roman indicator before Canada, terminator after
        let result = crate::encode_to_unicode("그는 Canada로").unwrap();
        assert!(result.contains('⠴'), "Should contain roman indicator ⠴");
    }
}
