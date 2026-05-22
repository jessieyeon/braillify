//! 제13항 — 다음 글자들은 약자를 사용하여 적는다.
//! (가, 나, 다, 마, 바, 사, 자, 카, 타, 파, 하, 것, 억, 언, 얼, 연, 열, 영, 옥, 온, 옹, 운, 울, 은, 을, 인, 성, 정, 청)
//!
//! 제15항 — 추가 약자 목록 (억, 언, 얼, 연, 열, 영, 옥, 온, 옹, 운, 울, 은, 을, 인, 것)
//!
//! Abbreviations are looked up from `char_shortcut::SHORTCUT_MAP` (PHF).
//! Encoding is delegated to `char_shortcut::encode_char_shortcut()`.
//!
//! Reference: 2024 Korean Braille Standard, Chapter 2, Section 6, Articles 13, 15

use crate::char_shortcut;
use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META_13: RuleMeta = RuleMeta { section: "13", subsection: None, name: "syllable_abbreviation", standard_ref: "2024 Korean Braille Standard, Ch.2 Sec.6 Art.13", description: "Common syllable abbreviations (가,나,다,...,하)" };

/// Try to encode a character using the abbreviation shortcut table.
/// Returns the abbreviated braille encoding, or Err if no abbreviation exists.
#[cfg(test)]
fn apply(ch: char) -> Result<&'static [u8], String> {
    char_shortcut::encode_char_shortcut(ch)
}

/// Check if a character has an abbreviation in the shortcut table.
pub fn has_abbreviation(ch: char) -> bool {
    char_shortcut::SHORTCUT_MAP.contains_key(&ch)
}

/// Plugin struct for the rule engine.
///
/// Handles syllable abbreviation lookup (제13항, 제15항).
/// Runs after rule_14 (which may suppress abbreviation). If a Korean syllable
/// has a shortcut entry, this rule emits the abbreviated form and Consumes.
pub struct Rule13;

impl BrailleRule for Rule13 {
    fn meta(&self) -> &'static RuleMeta {
        &META_13
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        90 // Before generic Korean encoding, after rule_14 (priority 80)
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        if let CharType::Korean(_) = ctx.char_type { has_abbreviation(ctx.current_char()) } else { false }
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let encoded = char_shortcut::encode_char_shortcut(ctx.current_char())?;
        ctx.emit_slice(encoded);
        Ok(RuleResult::Consumed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unicode::decode_unicode;

    #[test]
    fn encodes_basic_syllable_abbreviations() {
        // 제13항: 가, 나, 다, ... 하
        assert_eq!(apply('가').unwrap(), &[decode_unicode('⠫')]);
        assert_eq!(apply('나').unwrap(), &[decode_unicode('⠉')]);
        assert_eq!(apply('다').unwrap(), &[decode_unicode('⠊')]);
        assert_eq!(apply('사').unwrap(), &[decode_unicode('⠇')]);
        assert_eq!(apply('하').unwrap(), &[decode_unicode('⠚')]);
    }

    #[test]
    fn encodes_extended_abbreviations() {
        // 제15항: 것, 억, 언, 영, etc.
        assert_eq!(apply('것').unwrap(), &[decode_unicode('⠸'), decode_unicode('⠎')]);
        assert_eq!(apply('영').unwrap(), &[decode_unicode('⠻')]);
        assert_eq!(apply('은').unwrap(), &[decode_unicode('⠵')]);
        assert_eq!(apply('인').unwrap(), &[decode_unicode('⠟')]);
    }

    #[test]
    fn has_abbreviation_returns_true_for_known() {
        assert!(has_abbreviation('가'));
        assert!(has_abbreviation('것'));
        assert!(has_abbreviation('영'));
    }

    #[test]
    fn has_abbreviation_returns_false_for_unknown() {
        assert!(!has_abbreviation('곤'));
        assert!(!has_abbreviation('A'));
        assert!(!has_abbreviation('1'));
    }

    #[test]
    fn non_abbreviated_char_returns_error() {
        assert!(apply('곤').is_err());
    }

    #[test]
    fn golden_test_alignment() {
        let cases = vec![("가지", "⠫⠨⠕"), ("나비", "⠉⠘⠕"), ("것이다", "⠸⠎⠕⠊")];
        for (input, expected) in cases {
            let result = crate::encode_to_unicode(input).unwrap();
            assert_eq!(result, expected, "Rule 13 golden test failed for: {}", input);
        }
    }
}
