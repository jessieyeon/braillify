//! 제18항 — 다음 단어들은 약어를 사용하여 적는다.
//! (그래서, 그러나, 그러면, 그러므로, 그런데, 그리고, 그리하여)
//!
//! Word-level abbreviations: entire words are replaced with short braille sequences.
//! Lookup is delegated to `word_shortcut::split_word_shortcut()`.
//!
//! [다만] 약어 앞에 다른 글자가 붙어 나올 때에는 약어를 사용하지 않는다.
//!
//! Reference: 2024 Korean Braille Standard, Chapter 2, Section 7, Article 18

use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};
use crate::word_shortcut;

pub static META: RuleMeta = RuleMeta {
    section: "18",
    subsection: None,
    name: "word_abbreviation",
    standard_ref: "2024 Korean Braille Standard, Ch.2 Sec.7 Art.18",
    description: "Word abbreviations: 그래서,그러나,그러면,그러므로,그런데,그리고,그리하여",
};

/// Try to match a word against the abbreviation table.
/// Returns Some((matched_str, braille_codes, remaining_str)) if matched.
#[cfg(test)]
fn apply(text: &str) -> Option<(&'static str, &'static [u8], String)> {
    word_shortcut::split_word_shortcut(text)
}

/// Plugin struct for the rule engine.
///
/// Handles word-level abbreviations (제18항): 그래서, 그러나, 그러면, etc.
/// Runs in the WordShortcut phase at index 0 (word start).
/// When matched, emits the abbreviated braille codes and Consumes.
///
/// Note: Handling the "rest" (suffix after abbreviation, e.g., "그래서인지" → "인지")
/// requires re-entering the encoding pipeline. In Phase 3, the engine-driven
/// encode_word() will handle this recursion.
pub struct Rule18;

impl BrailleRule for Rule18 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::WordShortcut
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        // Word shortcuts only apply at the beginning of a word
        if ctx.index != 0 {
            return false;
        }
        let word: String = ctx.word_chars.iter().collect();
        word_shortcut::split_word_shortcut(&word).is_some()
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let word: String = ctx.word_chars.iter().collect();
        if let Some((_, codes, _rest)) = word_shortcut::split_word_shortcut(&word) {
            ctx.emit_slice(codes);
            // TODO(Phase 3): handle `rest` by re-entering encoding pipeline
            // For now, the remaining characters are handled by the caller.
            Ok(RuleResult::Consumed)
        } else {
            Ok(RuleResult::Skip)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_all_word_abbreviations() {
        let words = vec![
            "그래서",
            "그러나",
            "그러면",
            "그러므로",
            "그런데",
            "그리고",
            "그리하여",
        ];
        for word in words {
            let result = apply(word);
            assert!(result.is_some(), "Expected abbreviation for: {}", word);
            let (matched, codes, rest) = result.unwrap();
            assert_eq!(matched, word);
            assert!(!codes.is_empty());
            assert!(rest.is_empty());
        }
    }

    #[test]
    fn matches_with_suffix() {
        // 그래서인지 → matches 그래서, rest = "인지"
        let result = apply("그래서인지").unwrap();
        assert_eq!(result.0, "그래서");
        assert_eq!(result.2, "인지");
    }

    #[test]
    fn no_match_for_non_abbreviation() {
        assert!(apply("안녕하세요").is_none());
        assert!(apply("hello").is_none());
    }

    #[test]
    fn golden_test_alignment() {
        let cases = vec![("그래서", "⠁⠎"), ("그러나", "⠁⠉"), ("그리고", "⠁⠥")];
        for (input, expected) in cases {
            let result = crate::encode_to_unicode(input).unwrap();
            assert_eq!(
                result, expected,
                "Rule 18 golden test failed for: {}",
                input
            );
        }
    }
}
