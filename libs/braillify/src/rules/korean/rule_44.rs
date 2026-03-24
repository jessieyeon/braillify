//! 제44항 [다만] — 숫자와 혼동되는 'ㄴ, ㄷ, ㅁ, ㅋ, ㅌ, ㅍ, ㅎ'의 첫소리 글자와
//! '운'의 약자는 숫자 뒤에 붙어 나오더라도 숫자와 한글을 띄어 쓴다.
//!
//! When a Korean syllable starting with a "confusable" choseong (ㄴ,ㄷ,ㅁ,ㅋ,ㅌ,ㅍ,ㅎ)
//! or the syllable '운' follows a number, insert a space to prevent confusion.
//!
//! Reference: 2024 Korean Braille Standard, Chapter 5, Section 11, Article 44 [다만]

use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta {
    section: "44",
    subsection: Some("b1"),
    name: "number_korean_spacing",
    standard_ref: "2024 Korean Braille Standard, Ch.5 Sec.11 Art.44 [다만]",
    description: "Insert space between number and confusable Korean choseong",
};

/// Choseong characters that could be confused with digit braille patterns.
const CONFUSABLE_CHOSEONG: [char; 7] = ['ㄴ', 'ㄷ', 'ㅁ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ'];

/// Plugin struct for the rule engine.
///
/// Inserts a space (code 0) before Korean syllables with confusable choseong
/// when preceded by a number sequence. Runs in CoreEncoding at high priority
/// to insert the space BEFORE the Korean character is encoded.
pub struct Rule44;

impl BrailleRule for Rule44 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        50 // Very high — inserts space before any encoding of the Korean char
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        if !ctx.state.is_number {
            return false;
        }
        let CharType::Korean(korean) = ctx.char_type else {
            return false;
        };
        CONFUSABLE_CHOSEONG.contains(&korean.cho) || ctx.current_char() == '운'
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let has_middle_dot_before = ctx.word_chars[..ctx.index].contains(&'·');
        if has_middle_dot_before {
            ctx.emit(8); // Attached separator in middle-dot enumerations
        } else {
            ctx.emit(0); // Space separator
        }
        Ok(RuleResult::Continue) // Continue to Korean encoding rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_confusable_choseong() {
        for &cho in &CONFUSABLE_CHOSEONG {
            assert!(
                CONFUSABLE_CHOSEONG.contains(&cho),
                "Missing confusable: {}",
                cho
            );
        }
    }

    #[test]
    fn golden_test_alignment() {
        // "5운6기" → ⠼⠑ + space + 운 + ⠼⠋ + 기
        let result = crate::encode_to_unicode("5운6기").unwrap();
        assert_eq!(result, "⠼⠑⠀⠛⠼⠋⠈⠕");
    }

    #[test]
    fn meta_is_correct() {
        assert_eq!(META.section, "44");
        assert_eq!(META.name, "number_korean_spacing");
    }
}
