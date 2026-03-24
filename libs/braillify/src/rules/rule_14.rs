//! 제14항 — '나, 다, 마, 바, 자, 카, 타, 파, 하'에 모음이 붙어 나올 때에는 약자를 사용하지 않는다.
//!
//! When any of the 9 abbreviated syllables (나,다,마,바,자,카,타,파,하) is followed by
//! a syllable starting with silent ㅇ (i.e., vowel-initial), the abbreviation is NOT used.
//! Instead, the syllable is fully decomposed into choseong + jungseong.
//!
//! Note: 가 is not in this list (가 always uses abbreviation).
//!
//! Reference: 2024 Korean Braille Standard, Chapter 2, Section 6, Article 14

use crate::char_struct::CharType;
use crate::jauem::choseong::encode_choseong;
use crate::moeum::jungsong::encode_jungsong;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};
use crate::utils::has_choseong_o;

pub static META: RuleMeta = RuleMeta {
    section: "14",
    subsection: None,
    name: "no_abbrev_before_vowel",
    standard_ref: "2024 Korean Braille Standard, Ch.2 Sec.6 Art.14",
    description: "나,다,마,바,자,카,타,파,하 followed by vowel-initial syllable: no abbreviation",
};

/// The 9 syllables subject to this rule.
/// These syllables use abbreviation EXCEPT when followed by a vowel-initial syllable.
pub const NO_ABBREV_SYLLABLES: [char; 9] = ['나', '다', '마', '바', '자', '카', '타', '파', '하'];

/// When true, the encoder should use full decomposition (choseong + jungseong)
/// instead of the abbreviation shortcut.
#[cfg(test)]
fn should_suppress_abbreviation(current: char, next_has_choseong_o: bool) -> bool {
    is_no_abbrev_target(current) && next_has_choseong_o
}

/// Check if a character is subject to the no-abbreviation rule.
pub fn is_no_abbrev_target(ch: char) -> bool {
    NO_ABBREV_SYLLABLES.contains(&ch)
}

/// Plugin struct for the rule engine.
///
/// Suppresses abbreviation for 나,다,마,바,자,카,타,파,하 when followed
/// by a vowel-initial syllable (제14항). Emits full decomposition instead.
/// Runs at higher priority than rule_13 so it intercepts before abbreviation.
pub struct Rule14;

impl BrailleRule for Rule14 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        80 // Before rule_13 (priority 90) — intercepts abbreviation
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        if !matches!(ctx.char_type, CharType::Korean(_)) {
            return false;
        }
        if !is_no_abbrev_target(ctx.current_char()) {
            return false;
        }
        // Check if next character starts with ㅇ (vowel-initial)
        ctx.index < ctx.word_chars.len() - 1 && has_choseong_o(ctx.word_chars[ctx.index + 1])
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let CharType::Korean(korean) = ctx.char_type else {
            return Ok(RuleResult::Skip);
        };
        // Full decomposition: choseong + jungseong (no abbreviation)
        let cho_code = encode_choseong(korean.cho)?;
        ctx.emit(cho_code);
        ctx.emit_slice(encode_jungsong(korean.jung)?);
        Ok(RuleResult::Consumed)
    }
}

/// Check if this syllable should suppress its abbreviation.
///
/// Returns true when:
/// 1. Current char is one of the 9 target syllables
/// 2. Next char is a Korean syllable starting with ㅇ (vowel-initial)
///
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_all_target_syllables() {
        for &ch in &NO_ABBREV_SYLLABLES {
            assert!(is_no_abbrev_target(ch), "Expected {} to be target", ch);
        }
    }

    #[test]
    fn ga_is_not_target() {
        // 가 is NOT in the list — it always uses abbreviation
        assert!(!is_no_abbrev_target('가'));
    }

    #[test]
    fn suppresses_when_next_is_vowel_initial() {
        assert!(should_suppress_abbreviation('나', true));
        assert!(should_suppress_abbreviation('다', true));
        assert!(should_suppress_abbreviation('하', true));
    }

    #[test]
    fn does_not_suppress_when_next_is_consonant_initial() {
        assert!(!should_suppress_abbreviation('나', false));
        assert!(!should_suppress_abbreviation('하', false));
    }

    #[test]
    fn does_not_suppress_for_non_target() {
        assert!(!should_suppress_abbreviation('가', true));
        assert!(!should_suppress_abbreviation('곤', true));
    }

    #[test]
    fn golden_test_alignment() {
        // 나이: 나 + 이(ㅇ-initial) → no abbreviation for 나
        // 다음: 다 + 음(ㅇ-initial) → no abbreviation for 다
        let cases = vec![
            ("나이", "⠉⠣⠕"),  // full decomposition: ㄴ+ㅏ+ㅇ+ㅣ
            ("다음", "⠊⠣⠪⠢"), // full decomposition: ㄷ+ㅏ+ㅇ+ㅡ+ㅁ
            ("하얀", "⠚⠣⠜⠒"), // full decomposition: ㅎ+ㅏ+ㅇ+ㅑ+ㄴ
        ];
        for (input, expected) in cases {
            let result = crate::encode_to_unicode(input).unwrap();
            assert_eq!(
                result, expected,
                "Rule 14 golden test failed for: {}",
                input
            );
        }
    }
}
