use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta {
    section: "20",
    subsection: None,
    name: "middle_korean_bieup_series",
    standard_ref: "2024 Korean Braille Standard, Ch.3 Art.20",
    description: "Middle Korean ㅸ-series and legacy syllable glyphs",
};

const OLD_BIEUP: [u8; 3] = [
    crate::unicode::decode_unicode('⠐'),
    crate::unicode::decode_unicode('⠃'),
    crate::unicode::decode_unicode('⠶'),
];

const LEGACY_MAPPINGS: &[(char, &str)] = &[
    ('', "⠸"),
    ('', "⠐⠘⠶"),
    ('', "⠐⠼⠐⠨⠣"),
    ('', "⠐⠨⠐⠼⠐⠃⠶"),
    ('', "⠐⠘⠘⠶⠣⠐⠲"),
];

fn encode_unicode_cells(unicode: &str) -> Vec<u8> {
    unicode
        .chars()
        .map(crate::unicode::decode_unicode)
        .collect()
}

fn legacy_symbol_bytes(c: char) -> Option<Vec<u8>> {
    LEGACY_MAPPINGS
        .iter()
        .find(|(candidate, _)| *candidate == c)
        .map(|(_, unicode)| encode_unicode_cells(unicode))
}

pub struct Rule20;

impl BrailleRule for Rule20 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        53
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::KoreanPart('ㅸ'))
            || matches!(ctx.char_type, CharType::Symbol(c) if legacy_symbol_bytes(*c).is_some())
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        if matches!(ctx.char_type, CharType::KoreanPart('ㅸ')) {
            let is_symbol_fn = |ch: char| matches!(CharType::new(ch), Ok(CharType::Symbol(_)));
            let prefix = crate::rules::korean::rule_8::determine_prefix(
                ctx.word_len(),
                ctx.index,
                ctx.word_chars,
                ctx.has_korean_char,
                is_symbol_fn,
            );
            ctx.emit(prefix);
            ctx.emit_slice(&OLD_BIEUP);
            return Ok(RuleResult::Consumed);
        }

        if let CharType::Symbol(c) = ctx.char_type
            && let Some(encoded) = legacy_symbol_bytes(*c)
        {
            ctx.emit_slice(&encoded);
            return Ok(RuleResult::Consumed);
        }

        Ok(RuleResult::Skip)
    }
}
