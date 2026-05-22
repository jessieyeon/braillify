//! 제12항 — 'ㅑ, ㅘ, ㅜ, ㅝ'에 '애'가 붙어 나올 때에는 두 모음자 사이에 구분표 ⠤을 적어 나타낸다.
//!
//! When specific vowels (ㅑ, ㅘ, ㅜ, ㅝ) are followed by '애' (ㅇ+ㅐ),
//! insert separator ⠤ (code 36) between them.
//! Condition: current syllable has no final consonant (jongseong).
//!
//! Reference: 2024 Korean Braille Standard, Chapter 1, Section 5, Article 12

use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta { section: "12", subsection: None, name: "vowel_ae_separator", standard_ref: "2024 Korean Braille Standard, Ch.1 Sec.5 Art.12", description: "Insert separator ⠤ between ㅑ/ㅘ/ㅜ/ㅝ and 애 (ㅇ+ㅐ)" };

const SEPARATOR: u8 = 36; // ⠤
const TRIGGERING_VOWELS: [char; 4] = ['ㅑ', 'ㅘ', 'ㅜ', 'ㅝ'];

/// Apply rule 12: insert ⠤ separator before 애 when preceded by ㅑ/ㅘ/ㅜ/ㅝ.
///
/// # Arguments
/// * `current` - The current Korean syllable (already decomposed)
/// * `next` - The next raw character in the word
/// * `result` - The braille output buffer to append to
#[cfg(test)]
fn apply(current: &crate::char_struct::KoreanChar, next: char, result: &mut Vec<u8>) -> Result<(), String> {
    if let CharType::Korean(korean) = CharType::new(next)?
        && current.jong.is_none()
        && TRIGGERING_VOWELS.contains(&current.jung)
        && korean.cho == 'ㅇ'
        && korean.jung == 'ㅐ'
    {
        result.push(SEPARATOR);
    }
    Ok(())
}

/// Plugin struct for the rule engine.
pub struct Rule12;

impl BrailleRule for Rule12 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::InterCharacter
    }

    fn priority(&self) -> u16 {
        110 // Runs after Rule11 (priority 100)
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        let Some(korean) = ctx.as_korean() else {
            return false;
        };
        if korean.jong.is_some() {
            return false;
        }
        if !TRIGGERING_VOWELS.contains(&korean.jung) {
            return false;
        }
        let Some(next) = ctx.next_char() else {
            return false;
        };
        let Ok(CharType::Korean(next_k)) = CharType::new(next) else {
            return false;
        };
        next_k.cho == 'ㅇ' && next_k.jung == 'ㅐ'
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        ctx.emit(SEPARATOR);
        Ok(RuleResult::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::char_struct::KoreanChar;

    fn make_korean(ch: char) -> KoreanChar {
        match CharType::new(ch).unwrap() {
            CharType::Korean(k) => k,
            _ => panic!("Expected Korean character: {}", ch),
        }
    }

    // ── ㅑ + 애 ──────────────────────────────────────────

    #[test]
    fn inserts_separator_for_ya_ae() {
        // 야애: 야 (ㅇ+ㅑ, no jong) + 애 (ㅇ+ㅐ)
        let current = make_korean('야');
        let mut result = Vec::new();
        apply(&current, '애', &mut result).unwrap();
        assert_eq!(result, vec![SEPARATOR]);
    }

    // ── ㅘ + 애 ──────────────────────────────────────────

    #[test]
    fn inserts_separator_for_hwa_ae() {
        // 화 (ㅎ+ㅘ, no jong) + 액 → 액's first is 애 (ㅇ+ㅐ+ㄱ)
        let current = make_korean('화');
        let mut result = Vec::new();
        apply(&current, '액', &mut result).unwrap();
        assert_eq!(result, vec![SEPARATOR]);
    }

    // ── ㅜ + 애 ──────────────────────────────────────────

    #[test]
    fn inserts_separator_for_su_ae() {
        // 수 (ㅅ+ㅜ, no jong) + 액
        let current = make_korean('수');
        let mut result = Vec::new();
        apply(&current, '액', &mut result).unwrap();
        assert_eq!(result, vec![SEPARATOR]);
    }

    // ── ㅝ + 애 ──────────────────────────────────────────

    #[test]
    fn inserts_separator_for_weo_ae() {
        // 워 (ㅇ+ㅝ, no jong) + 앰
        let current = make_korean('워');
        let mut result = Vec::new();
        apply(&current, '앰', &mut result).unwrap();
        assert_eq!(result, vec![SEPARATOR]);
    }

    // ── Non-triggering vowels ────────────────────────────

    #[test]
    fn skips_non_triggering_vowel_a() {
        // 가 (ㄱ+ㅏ) → ㅏ is not in [ㅑ, ㅘ, ㅜ, ㅝ]
        let current = make_korean('가');
        let mut result = Vec::new();
        apply(&current, '애', &mut result).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn skips_non_triggering_vowel_eo() {
        // 서 (ㅅ+ㅓ) → ㅓ is not in triggering set
        let current = make_korean('서');
        let mut result = Vec::new();
        apply(&current, '애', &mut result).unwrap();
        assert!(result.is_empty());
    }

    // ── Jong present → skip ──────────────────────────────

    #[test]
    fn skips_when_current_has_jongseong() {
        // 숙 (ㅅ+ㅜ+ㄱ) has jong → no separator
        let current = make_korean('숙');
        assert!(current.jong.is_some());
        let mut result = Vec::new();
        apply(&current, '애', &mut result).unwrap();
        assert!(result.is_empty());
    }

    // ── Next is not 애 → skip ────────────────────────────

    #[test]
    fn skips_when_next_is_not_ae() {
        let current = make_korean('야');
        let mut result = Vec::new();
        apply(&current, '이', &mut result).unwrap();
        assert!(result.is_empty());
    }

    // ── Golden tests ─────────────────────────────────────

    #[test]
    fn golden_test_alignment() {
        // From test_cases/rule_12.json
        let cases = vec![("야애", "⠜⠤⠗"), ("소화액", "⠠⠥⠚⠧⠤⠗⠁"), ("수액", "⠠⠍⠤⠗⠁")];
        for (input, expected_unicode) in cases {
            let result = crate::encode_to_unicode(input).unwrap();
            assert_eq!(result, expected_unicode, "Rule 12 golden test failed for input: {}", input);
        }
    }

    #[test]
    fn meta_is_correct() {
        assert_eq!(META.section, "12");
        assert_eq!(META.name, "vowel_ae_separator");
    }
}
