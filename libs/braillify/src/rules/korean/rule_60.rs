//! 제60항 — 별표(*)는 앞뒤를 한 칸씩 띄어 쓴다.
//!
//! Asterisks require surrounding spaces. When the asterisk is a standalone word,
//! spaces are added before and after. The inter-word spacing mechanism handles
//! most cases, but explicit spacing is needed at word boundaries.
//!
//! Reference: 2024 Korean Braille Standard, Chapter 6, Section 13, Article 60

use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};
use crate::symbol_shortcut;

pub static META: RuleMeta = RuleMeta {
    section: "60",
    subsection: None,
    name: "asterisk_spacing",
    standard_ref: "2024 Korean Braille Standard, Ch.6 Sec.13 Art.60",
    description: "Asterisk (*) requires surrounding spaces",
};

/// Plugin struct for the rule engine.
///
/// Handles asterisk encoding with spacing.
/// When the asterisk is the first and only character in a word, and there's
/// a previous word, insert a space before it. The asterisk symbol encoding
/// is then emitted normally.
pub struct Rule60;

impl BrailleRule for Rule60 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        400 // Before rule_49 (500) — intercept * before generic symbol encoding
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::Symbol(c) if *c == '*')
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        // 제60항: asterisk as standalone word with previous word → prepend space
        if ctx.index == 0 && ctx.word_len() == 1 && !ctx.prev_word.is_empty() {
            ctx.emit(0); // Space before asterisk
        }
        let encoded = symbol_shortcut::encode_char_symbol_shortcut('*')?;
        ctx.emit_slice(encoded);
        Ok(RuleResult::Consumed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_is_correct() {
        assert_eq!(META.section, "60");
        assert_eq!(META.name, "asterisk_spacing");
    }
}
