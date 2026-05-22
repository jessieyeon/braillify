//! 제8항 — 자음자나 모음자가 단독으로 쓰일 때에는 해당 글자 앞에 온표 ⠿(63)을 적어 나타내며,
//! 자음자는 받침으로 적는다.
//!
//! 제9항 — 한글의 자음자가 번호로 쓰일 때에는 온표를 앞세워 받침으로 적는다.
//! (e.g., ㄱ. → 온표 + jongseong encoding)
//!
//! 제10항 — 단독으로 쓰인 자음자가 단어에 붙어 나올 때에는 ⠸(56)을 앞세워 받침으로 적는다.
//!
//! Reference: 2024 Korean Braille Standard, Chapter 1, Section 4, Articles 8-10

use crate::char_struct::CharType;
use crate::korean_part;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META_8: RuleMeta = RuleMeta {
    section: "8",
    subsection: None,
    name: "standalone_jamo",
    standard_ref: "2024 Korean Braille Standard, Ch.1 Sec.4 Art.8",
    description: "Standalone jamo: prefix with 온표 ⠿ (63), consonants as jongseong",
};

/// Indicator prefix for standalone jamo (온표).
pub const ONTAB: u8 = 63; // ⠿

/// Indicator prefix for jamo attached to a word.
pub const WORD_ATTACHED_PREFIX: u8 = 56; // ⠸

/// Determine which prefix to use for a standalone jamo (KoreanPart).
///
/// Returns the prefix byte (63 for standalone/제8항, 56 for word-attached/제10항).
///
/// # Arguments
/// * `word_len` - total characters in current word
/// * `char_index` - index of the current KoreanPart character
/// * `word_chars` - all characters in the word
/// * `has_korean_char` - whether the word contains Korean syllable characters
/// * `is_symbol` - closure to check if a char is a symbol
pub fn determine_prefix<F>(
    word_len: usize,
    char_index: usize,
    word_chars: &[char],
    has_korean_char: bool,
    is_symbol: F,
) -> u8
where
    F: Fn(char) -> bool,
{
    match word_len {
        1 => ONTAB, // 제8항: standalone
        2 => ONTAB, // 제8항/제9항: standalone in 2-char word
        _ => {
            // Multi-char word: check context
            let is_first_with_ja = char_index == 0 && word_len > 1 && word_chars[1] == '자';

            let is_bordered_by_symbols = {
                let prev_is_symbol_or_start =
                    char_index == 0 || (char_index > 0 && is_symbol(word_chars[char_index - 1]));
                let next_is_symbol_or_end = word_len - 1 == char_index
                    || (char_index < word_len - 1 && is_symbol(word_chars[char_index + 1]));
                prev_is_symbol_or_start && next_is_symbol_or_end
            };

            if (is_first_with_ja || is_bordered_by_symbols) || !has_korean_char {
                ONTAB // 제8항: standalone context
            } else {
                WORD_ATTACHED_PREFIX // 제10항: attached to Korean word
            }
        }
    }
}

/// Plugin struct for the rule engine.
///
/// Handles standalone jamo encoding (제8항, 제9항, 제10항).
/// Determines the appropriate prefix (온표 ⠿ or ⠸) based on context,
/// then encodes the jamo character.
pub struct Rule8;

impl BrailleRule for Rule8 {
    fn meta(&self) -> &'static RuleMeta {
        &META_8
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::KoreanPart(_))
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let CharType::KoreanPart(c) = ctx.char_type else {
            return Ok(RuleResult::Skip);
        };

        let is_symbol_fn = |ch: char| matches!(CharType::new(ch), Ok(CharType::Symbol(_)));

        // 제9항: jamo used as numbering (ㄱ.) — uses jongseong encoding
        if is_jamo_numbering(ctx.index, ctx.word_chars) {
            ctx.emit(ONTAB);
            ctx.emit_slice(crate::jauem::jongseong::encode_jongseong(*c)?);
            return Ok(RuleResult::Consumed);
        }

        let prefix = determine_prefix(
            ctx.word_len(),
            ctx.index,
            ctx.word_chars,
            ctx.has_korean_char,
            is_symbol_fn,
        );
        ctx.emit(prefix);
        ctx.emit_slice(korean_part::encode_korean_part(*c)?);
        Ok(RuleResult::Consumed)
    }
}

/// Check if a word of length 2 is in "jamo as numbering" format (제9항).
/// e.g., "ㄱ." — jamo followed by period.
pub fn is_jamo_numbering(char_index: usize, word_chars: &[char]) -> bool {
    word_chars.len() == 2 && char_index == 0 && word_chars[1] == '.'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn not_symbol(_: char) -> bool {
        false
    }

    fn is_sym(c: char) -> bool {
        matches!(c, '.' | ',' | '(' | ')' | '[' | ']')
    }

    #[test]
    fn standalone_single_char() {
        assert_eq!(determine_prefix(1, 0, &['ㄱ'], false, not_symbol), ONTAB);
    }

    #[test]
    fn jamo_numbering_format() {
        let chars = ['ㄱ', '.'];
        assert!(is_jamo_numbering(0, &chars));
        assert_eq!(determine_prefix(2, 0, &chars, false, not_symbol), ONTAB);
    }

    #[test]
    fn non_numbering_two_char() {
        let chars = ['ㄱ', 'ㄴ'];
        assert!(!is_jamo_numbering(0, &chars));
    }

    #[test]
    fn attached_to_korean_word() {
        let chars = ['가', 'ㄱ', '나'];
        assert_eq!(
            determine_prefix(3, 1, &chars, true, not_symbol),
            WORD_ATTACHED_PREFIX
        );
    }

    #[test]
    fn bordered_by_symbols_uses_ontab() {
        let chars = ['(', 'ㄱ', ')'];
        assert_eq!(determine_prefix(3, 1, &chars, true, is_sym), ONTAB);
    }

    #[test]
    fn first_with_ja_uses_ontab() {
        let chars = ['ㄱ', '자', '도'];
        assert_eq!(determine_prefix(3, 0, &chars, true, not_symbol), ONTAB);
    }

    #[test]
    fn golden_test_alignment() {
        let cases = vec![("ㄱ", "⠿⠁"), ("ㅏ", "⠿⠣")];
        for (input, expected) in cases {
            let result = crate::encode_to_unicode(input).unwrap();
            assert_eq!(result, expected, "Rule 8 golden test failed for: {}", input);
        }
    }

    use rstest::rstest;

    #[rstest]
    #[case("ㄱ", true)] // KoreanPart consonant
    #[case("ㅏ", true)] // KoreanPart vowel
    #[case("가", false)] // Korean syllable, not part
    #[case("A", false)] // English
    fn rule8_matches_korean_part_only(#[case] input: &str, #[case] expected: bool) {
        let mut owned = crate::test_helpers::CtxOwned::for_text(input, false);
        let ctx = owned.ctx_at(0);
        assert_eq!(Rule8.matches(&ctx), expected, "input={input}");
    }

    #[rstest]
    #[case("ㄱ")]
    #[case("ㄴ")]
    #[case("ㅏ")]
    #[case("ㅎ")]
    fn rule8_apply_emits_for_korean_part(#[case] input: &str) {
        let mut owned = crate::test_helpers::CtxOwned::for_text(input, false);
        let mut ctx = owned.ctx_at(0);
        let outcome = Rule8.apply(&mut ctx).unwrap();
        assert!(matches!(outcome, RuleResult::Consumed));
        assert!(!owned.result.is_empty());
    }

    #[test]
    fn rule8_apply_skips_non_korean_part() {
        let mut owned = crate::test_helpers::CtxOwned::for_text("A", false);
        let mut ctx = owned.ctx_at(0);
        let outcome = Rule8.apply(&mut ctx).unwrap();
        assert!(matches!(outcome, RuleResult::Skip));
    }
}
