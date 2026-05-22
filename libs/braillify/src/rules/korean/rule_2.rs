//! 제2항 — 된소리 글자 'ㄲ, ㄸ, ㅃ, ㅆ, ㅉ'이 첫소리로 쓰일 때에는
//! 'ㄱ, ㄷ, ㅂ, ㅅ, ㅈ' 앞에 된소리표 ⠠을 적어 나타낸다.
//!
//! Double consonants as initial (choseong) are written as 된소리표 (⠠, code 32)
//! followed by the base consonant.
//!
//! The decomposition is handled by `split::split_korean_jauem()`.
//!
//! Reference: 2024 Korean Braille Standard, Chapter 1, Section 1, Article 2

use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};
use crate::split;

pub static META: RuleMeta = RuleMeta { section: "2", subsection: None, name: "double_choseong", standard_ref: "2024 Korean Braille Standard, Ch.1 Sec.1 Art.2", description: "Double consonants (ㄲ,ㄸ,ㅃ,ㅆ,ㅉ) as choseong: 된소리표 ⠠ + base consonant" };

const DOUBLE_CONSONANT_INDICATOR: u8 = 32; // ⠠ (된소리표)

/// The 5 double consonants that trigger this rule.
pub const DOUBLE_CHOSEONG: [char; 5] = ['ㄲ', 'ㄸ', 'ㅃ', 'ㅆ', 'ㅉ'];

/// Check if a choseong is a double consonant.
pub fn is_double_choseong(cho: char) -> bool {
    DOUBLE_CHOSEONG.contains(&cho)
}

/// Decompose a double choseong into (된소리표, base consonant).
/// Returns None if not a double consonant.
pub fn decompose(cho: char) -> Option<(u8, char)> {
    if !is_double_choseong(cho) {
        return None;
    }
    let (base, _) = split::split_korean_jauem(cho).ok()?;
    Some((DOUBLE_CONSONANT_INDICATOR, base))
}

/// Plugin struct for the rule engine.
///
/// Sub-component rule: handles double consonant (된소리) choseong encoding.
/// When a Korean syllable has a double initial consonant (ㄲ,ㄸ,ㅃ,ㅆ,ㅉ),
/// this rule emits 된소리표 ⠠ followed by the base consonant code.
/// Returns Continue for jungseong/jongseong processing.
pub struct Rule2;

impl BrailleRule for Rule2 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        195 // Sub-component — runs before Rule1 (200) for double consonant check
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        ctx.as_korean().is_some_and(|k| is_double_choseong(k.cho))
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let Some(korean) = ctx.as_korean() else {
            return Ok(RuleResult::Skip);
        };
        if let Some((indicator, _base)) = decompose(korean.cho) {
            ctx.emit(indicator);
        }
        Ok(RuleResult::Continue) // Continue to Rule1 for base consonant + jungseong/jongseong
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_all_double_consonants() {
        assert!(is_double_choseong('ㄲ'));
        assert!(is_double_choseong('ㄸ'));
        assert!(is_double_choseong('ㅃ'));
        assert!(is_double_choseong('ㅆ'));
        assert!(is_double_choseong('ㅉ'));
    }

    #[test]
    fn rejects_single_consonants() {
        assert!(!is_double_choseong('ㄱ'));
        assert!(!is_double_choseong('ㄷ'));
        assert!(!is_double_choseong('ㅂ'));
        assert!(!is_double_choseong('ㅅ'));
        assert!(!is_double_choseong('ㅈ'));
    }

    #[test]
    fn decomposes_correctly() {
        assert_eq!(decompose('ㄲ'), Some((32, 'ㄱ')));
        assert_eq!(decompose('ㄸ'), Some((32, 'ㄷ')));
        assert_eq!(decompose('ㅃ'), Some((32, 'ㅂ')));
        assert_eq!(decompose('ㅆ'), Some((32, 'ㅅ')));
        assert_eq!(decompose('ㅉ'), Some((32, 'ㅈ')));
    }

    #[test]
    fn decompose_returns_none_for_single() {
        assert_eq!(decompose('ㄱ'), None);
        assert_eq!(decompose('ㅎ'), None);
    }

    #[test]
    fn golden_test_alignment() {
        let cases = vec![("꾸러미", "⠠⠈⠍⠐⠎⠑⠕"), ("쓰기", "⠠⠠⠪⠈⠕")];
        for (input, expected) in cases {
            let result = crate::encode_to_unicode(input).unwrap();
            assert_eq!(result, expected, "Rule 2 golden test failed for: {}", input);
        }
    }
}
