//! English-context symbol handling.
//!
//! Handles symbol behavior that depends on English mode state:
//! - English symbol rendering for (, ), , when context requires
//! - Parenthesis stack push/pop for matching English parentheses
//! - Comma before Korean fallback preservation

use crate::char_struct::CharType;
use crate::english_logic;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};
use crate::symbol_shortcut;
use crate::utils;

pub static META: RuleMeta = RuleMeta {
    section: "49",
    subsection: Some("eng"),
    name: "english_symbol_context",
    standard_ref: "2024 Korean Braille Standard, Ch.4 Sec.10 + Ch.6 Sec.13",
    description: "English-context punctuation rendering with parenthesis tracking",
};

pub struct RuleEnglishSymbol;

impl BrailleRule for RuleEnglishSymbol {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        300
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::Symbol(_))
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let CharType::Symbol(sym) = ctx.char_type else {
            return Ok(RuleResult::Skip);
        };

        let mut use_english_symbol = english_logic::should_render_symbol_as_english(
            ctx.state.english_indicator,
            ctx.state.is_english,
            &ctx.state.parenthesis_stack,
            *sym,
            ctx.word_chars,
            ctx.index,
            ctx.remaining_words,
        );

        if *sym == '(' {
            ctx.state.parenthesis_stack.push(use_english_symbol);
        } else if *sym == ')' {
            use_english_symbol = ctx
                .state
                .parenthesis_stack
                .pop()
                .unwrap_or(use_english_symbol);
        }

        let has_ascii_alphabetic = ctx.word_chars.iter().any(|ch| ch.is_ascii_alphabetic());
        let can_use_english_symbol = ctx.state.is_english || has_ascii_alphabetic;

        if ctx.state.english_indicator && can_use_english_symbol && use_english_symbol {
            if !ctx.state.is_english && !ctx.state.needs_english_continuation {
                ctx.emit(52);
                ctx.state.is_english = true;
                ctx.state.needs_english_continuation = false;
            }
            if let Some(encoded) = symbol_shortcut::encode_english_char_symbol_shortcut(*sym) {
                ctx.emit_slice(encoded);
                return Ok(RuleResult::Consumed);
            }
        }

        if *sym == ',' {
            let next_char = ctx
                .next_char()
                .or_else(|| ctx.remaining_words.first().and_then(|w| w.chars().next()));
            if next_char.is_some_and(utils::is_korean_char) {
                ctx.emit_slice(symbol_shortcut::encode_char_symbol_shortcut(*sym)?);
                return Ok(RuleResult::Consumed);
            }
        }

        Ok(RuleResult::Continue)
    }
}
