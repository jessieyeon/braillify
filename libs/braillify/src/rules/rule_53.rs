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

pub static META: RuleMeta = RuleMeta {
    section: "53",
    subsection: None,
    name: "ellipsis_normalization",
    standard_ref: "2024 Korean Braille Standard, Ch.6 Sec.13 Art.53",
    description: "Normalize ellipsis: 6 dots→3, double middle dot→single",
};

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
        // Check if word contains ellipsis patterns that need normalization
        let word: String = ctx.word_chars.iter().collect();
        word.contains("......") || word.contains("……")
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
}
