//! 제53항 — 가운뎃점으로 쓴 줄임표(…… , …)는 ⠠⠠⠠으로,
//! 마침표로 쓴 줄임표(...... , ...)는 ⠲⠲⠲으로 적는다.
//!
//! Ellipsis normalization: multiple dots/middle dots are collapsed before encoding.
//! This rule is applied during preprocessing (before character-level encoding).
//!
//! Reference: 2024 Korean Braille Standard, Chapter 6, Section 13, Article 53

use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta { section: "53", subsection: None, name: "ellipsis_normalization", standard_ref: "2024 Korean Braille Standard, Ch.6 Sec.13 Art.53", description: "Normalize ellipsis: 6 dots→3, double middle dot→single" };

/// Normalize ellipsis patterns in a word.
///
/// - `......` (6 periods) → `...` (3 periods)
/// - `……` (2 middle dots) → `…` (1 middle dot)
#[cfg(test)]
fn normalize(word: &str) -> String {
    word.replace("......", "...").replace("……", "…")
}

/// Plugin struct for the rule engine.
///
/// Word-level preprocessing: normalizes ellipsis patterns (제53항).
/// This rule operates at the Preprocessing phase, which runs BEFORE the
/// per-character loop. In the engine-driven pipeline, the engine would
/// call this at index 0 and the rule would signal that word normalization
/// is needed. The actual text mutation occurs outside the per-character model.
///
/// Note: The `normalize()` function is the primary API. The BrailleRule trait
/// is provided for trait completeness and rule-engine discoverability.
pub struct Rule53;

impl BrailleRule for Rule53 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::Preprocessing
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        // Only meaningful at the start of word processing
        if ctx.index != 0 {
            return false;
        }
        // Detect either pattern in a single forward pass over `&[char]` — no
        // intermediate `String` allocation. Tracks the longest run of '.' and
        // whether two consecutive '…' have appeared back-to-back.
        let mut dot_run = 0u8;
        let mut prev_ellipsis = false;
        for &ch in ctx.word_chars {
            if ch == '.' {
                dot_run += 1;
                if dot_run >= 6 {
                    return true;
                }
                prev_ellipsis = false;
            } else if ch == '…' {
                if prev_ellipsis {
                    return true;
                }
                prev_ellipsis = true;
                dot_run = 0;
            } else {
                dot_run = 0;
                prev_ellipsis = false;
            }
        }
        false
    }

    fn apply(&self, _ctx: &mut RuleContext) -> Result<RuleResult, String> {
        // Word normalization happens outside the per-character pipeline.
        // This rule signals that preprocessing was needed but doesn't emit bytes.
        // The engine-driven encode_word() will call normalize() on the word
        // before entering the character loop.
        Ok(RuleResult::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_six_periods() {
        assert_eq!(normalize("hello......world"), "hello...world");
    }

    #[test]
    fn normalizes_double_middle_dot() {
        assert_eq!(normalize("hello……world"), "hello…world");
    }

    #[test]
    fn no_change_for_three_periods() {
        assert_eq!(normalize("hello...world"), "hello...world");
    }

    #[test]
    fn no_change_for_single_middle_dot() {
        assert_eq!(normalize("hello…world"), "hello…world");
    }

    #[test]
    fn no_change_for_normal_text() {
        assert_eq!(normalize("안녕하세요"), "안녕하세요");
    }

    #[test]
    fn empty_string() {
        assert_eq!(normalize(""), "");
    }

    fn make_ctx<'a>(word_chars: &'a [char], index: usize, char_type: &'a crate::char_struct::CharType, skip_count: &'a mut usize, state: &'a mut crate::rules::context::EncoderState, result: &'a mut Vec<u8>) -> RuleContext<'a> {
        RuleContext { word_chars, index, char_type, prev_word: "", remaining_words: &[], has_korean_char: false, is_all_uppercase: false, ascii_starts_at_beginning: false, skip_count, state, result }
    }

    #[test]
    fn rule53_meta_and_phase() {
        let r = Rule53;
        assert_eq!(r.meta().section, "53");
        assert!(matches!(r.phase(), Phase::Preprocessing));
    }

    #[test]
    fn rule53_matches_six_periods_run() {
        use crate::char_struct::CharType;
        let word_chars: Vec<char> = "......".chars().collect();
        let ct = CharType::new(word_chars[0]).unwrap();
        let mut skip = 0usize;
        let mut state = crate::rules::context::EncoderState::new(false);
        let mut out = Vec::new();
        let ctx = make_ctx(&word_chars, 0, &ct, &mut skip, &mut state, &mut out);
        assert!(Rule53.matches(&ctx));
    }

    #[test]
    fn rule53_matches_double_ellipsis() {
        use crate::char_struct::CharType;
        let word_chars: Vec<char> = "……".chars().collect();
        let ct = CharType::new(word_chars[0]).unwrap();
        let mut skip = 0usize;
        let mut state = crate::rules::context::EncoderState::new(false);
        let mut out = Vec::new();
        let ctx = make_ctx(&word_chars, 0, &ct, &mut skip, &mut state, &mut out);
        assert!(Rule53.matches(&ctx));
    }

    #[test]
    fn rule53_does_not_match_three_periods() {
        use crate::char_struct::CharType;
        let word_chars: Vec<char> = "...".chars().collect();
        let ct = CharType::new(word_chars[0]).unwrap();
        let mut skip = 0usize;
        let mut state = crate::rules::context::EncoderState::new(false);
        let mut out = Vec::new();
        let ctx = make_ctx(&word_chars, 0, &ct, &mut skip, &mut state, &mut out);
        assert!(!Rule53.matches(&ctx));
    }

    #[test]
    fn rule53_match_resets_on_other_char() {
        use crate::char_struct::CharType;
        // "...a..." has two runs of three; should NOT trigger six-period match
        let word_chars: Vec<char> = "...a...".chars().collect();
        let ct = CharType::new(word_chars[0]).unwrap();
        let mut skip = 0usize;
        let mut state = crate::rules::context::EncoderState::new(false);
        let mut out = Vec::new();
        let ctx = make_ctx(&word_chars, 0, &ct, &mut skip, &mut state, &mut out);
        assert!(!Rule53.matches(&ctx));
    }

    #[test]
    fn rule53_match_false_when_not_at_word_start() {
        use crate::char_struct::CharType;
        let word_chars: Vec<char> = "......".chars().collect();
        let ct = CharType::new(word_chars[0]).unwrap();
        let mut skip = 0usize;
        let mut state = crate::rules::context::EncoderState::new(false);
        let mut out = Vec::new();
        let ctx = make_ctx(&word_chars, 1, &ct, &mut skip, &mut state, &mut out);
        assert!(!Rule53.matches(&ctx));
    }

    #[test]
    fn rule53_apply_just_continues() {
        use crate::char_struct::CharType;
        let word_chars: Vec<char> = "......".chars().collect();
        let ct = CharType::new(word_chars[0]).unwrap();
        let mut skip = 0usize;
        let mut state = crate::rules::context::EncoderState::new(false);
        let mut out = Vec::new();
        let mut ctx = make_ctx(&word_chars, 0, &ct, &mut skip, &mut state, &mut out);
        let res = Rule53.apply(&mut ctx).unwrap();
        assert!(matches!(res, RuleResult::Continue));
        assert!(out.is_empty());
    }
}
