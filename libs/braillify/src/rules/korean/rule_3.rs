//! 제3항 — 기본 자음자 14개가 받침으로 쓰일 때에는 다음과 같이 적는다.
//! 제4항 — 쌍받침 'ㄲ'은 ⠁⠁으로 적고, 쌍받침 'ㅆ'은 약자인 ⠌으로 적는다.
//! 제5항 — 겹받침은 각 받침 글자를 어울러 다음과 같이 적는다.
//!
//! Maps 28 final consonants (jongseong) to braille dot patterns.
//! Includes single, double, and compound final consonants.
//!
//! Encoding is delegated to `jauem::jongseong::encode_jongseong()` which uses a PHF map.
//!
//! Reference: 2024 Korean Braille Standard, Chapter 1, Section 2, Articles 3-5

use crate::jauem::jongseong;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META_3: RuleMeta = RuleMeta { section: "3", subsection: None, name: "basic_jongseong", standard_ref: "2024 Korean Braille Standard, Ch.1 Sec.2 Art.3", description: "Encode 14 basic final consonants (jongseong) to braille" };

/// Encode a jongseong character to its braille representation.
/// Re-exports `jauem::jongseong::encode_jongseong`.
#[cfg(test)]
fn apply(jong: char) -> Result<&'static [u8], String> {
    jongseong::encode_jongseong(jong)
}

/// Plugin struct for the rule engine.
///
/// Sub-component rule: encodes the final consonant (jongseong) of a Korean syllable.
/// Covers 제3항 (basic), 제4항 (double: ㄲ→⠁⠁, ㅆ→⠌), and 제5항 (compound).
/// Returns Continue since this is a sub-component of syllable encoding.
pub struct Rule3;

impl BrailleRule for Rule3 {
    fn meta(&self) -> &'static RuleMeta {
        &META_3
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        210 // Sub-component — runs after choseong (200) and jungseong
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        ctx.as_korean().is_some_and(|k| k.jong.is_some())
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let Some(korean) = ctx.as_korean() else {
            return Ok(RuleResult::Skip);
        };
        if let Some(jong) = korean.jong {
            let encoded = jongseong::encode_jongseong(jong)?;
            ctx.emit_slice(encoded);
        }
        Ok(RuleResult::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unicode::decode_unicode;

    // ── 제3항: basic 14 jongseong ──────────────────────

    #[test]
    fn encodes_basic_jongseong() {
        let cases = vec![('ㄱ', vec![decode_unicode('⠁')]), ('ㄴ', vec![decode_unicode('⠒')]), ('ㄷ', vec![decode_unicode('⠔')]), ('ㄹ', vec![decode_unicode('⠂')]), ('ㅁ', vec![decode_unicode('⠢')]), ('ㅂ', vec![decode_unicode('⠃')]), ('ㅅ', vec![decode_unicode('⠄')]), ('ㅇ', vec![decode_unicode('⠶')]), ('ㅈ', vec![decode_unicode('⠅')]), ('ㅊ', vec![decode_unicode('⠆')]), ('ㅋ', vec![decode_unicode('⠖')]), ('ㅌ', vec![decode_unicode('⠦')]), ('ㅍ', vec![decode_unicode('⠲')]), ('ㅎ', vec![decode_unicode('⠴')])];
        for (jong, expected) in cases {
            let result = apply(jong).unwrap();
            assert_eq!(result, &expected[..], "Failed for jongseong: {}", jong);
        }
    }

    // ── 제4항: double jongseong (ㄲ, ㅆ) ──────────────

    #[test]
    fn encodes_double_jongseong_gg() {
        let result = apply('ㄲ').unwrap();
        assert_eq!(result, &[decode_unicode('⠁'), decode_unicode('⠁')]);
    }

    #[test]
    fn encodes_double_jongseong_ss() {
        // ㅆ is abbreviated to ⠌
        let result = apply('ㅆ').unwrap();
        assert_eq!(result, &[decode_unicode('⠌')]);
    }

    // ── 제5항: compound jongseong ──────────────────────

    #[test]
    fn encodes_compound_jongseong() {
        let cases = vec![('ㄳ', vec![decode_unicode('⠁'), decode_unicode('⠄')]), ('ㄵ', vec![decode_unicode('⠒'), decode_unicode('⠅')]), ('ㄶ', vec![decode_unicode('⠒'), decode_unicode('⠴')]), ('ㄺ', vec![decode_unicode('⠂'), decode_unicode('⠁')]), ('ㅄ', vec![decode_unicode('⠃'), decode_unicode('⠄')])];
        for (jong, expected) in cases {
            let result = apply(jong).unwrap();
            assert_eq!(result, &expected[..], "Failed for compound jongseong: {}", jong);
        }
    }

    #[test]
    fn invalid_returns_error() {
        assert!(apply('A').is_err());
        assert!(apply('가').is_err());
    }

    #[test]
    fn golden_test_alignment() {
        let cases = vec![("국보", "⠈⠍⠁⠘⠥"), ("놋그릇", "⠉⠥⠄⠈⠪⠐⠪⠄")];
        for (input, expected) in cases {
            let result = crate::encode_to_unicode(input).unwrap();
            assert_eq!(result, expected, "Rule 3 golden test failed for: {}", input);
        }
    }
}
