use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta {
    section: "23",
    subsection: None,
    name: "historical_letter_symbols",
    standard_ref: "2024 Korean Braille Standard, Ch.3 Art.23",
    description: "Historical Korean consonant symbols and explanatory ideographs",
};

const MAPPINGS: &[(char, &str)] = &[
    ('ㅸ', "⠐⠃"),
    ('ㅿ', "⠐⠅"),
    ('ㆆ', "⠸⠐⠴"),
    ('字', "⠠⠨"),
    ('', "⠐⠼⠐⠶"),
    ('君', "⠈⠛"),
    ('洪', "⠐⠚⠚⠥⠐⠲"),
    ('侵', "⠰⠕⠢"),
    ('斗', "⠊⠍⠐⠢⠶"),
    ('虛', "⠚⠎⠐⠶"),
    ('後', "⠚⠍"),
    ('狄', "⠨⠹⠟"),
    ('人', "⠸⠄"),
    ('位', "⠍⠗"),
    ('', "⠘⠐⠼"),
];

fn encode_unicode_cells(unicode: &str) -> Vec<u8> {
    unicode
        .chars()
        .map(crate::unicode::decode_unicode)
        .collect()
}

pub fn is_historical_letter_symbol(c: char) -> bool {
    MAPPINGS.iter().any(|(candidate, _)| *candidate == c)
}

fn encode_historical_letter_symbol(c: char) -> Option<Vec<u8>> {
    MAPPINGS
        .iter()
        .find(|(candidate, _)| *candidate == c)
        .map(|(_, unicode)| encode_unicode_cells(unicode))
}

fn should_skip_hanja_in_context(ctx: &RuleContext) -> bool {
    matches!(
        (ctx.current_char(), ctx.next_char()),
        ('君', Some('군')) | ('侵', Some('침'))
    )
}

pub struct Rule23;

impl BrailleRule for Rule23 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        55
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::KoreanPart(_) | CharType::Symbol(_))
            && is_historical_letter_symbol(ctx.current_char())
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        if ctx.current_char() == '虛' && ctx.next_char() == Some('헝') {
            ctx.emit_slice(&encode_unicode_cells("⠚⠎⠐⠶"));
            *ctx.skip_count = 1;
            return Ok(RuleResult::Consumed);
        }

        if ctx.current_char() == '人' && ctx.next_char() == Some('ㅅ') {
            ctx.emit_slice(&encode_unicode_cells("⠸⠄"));
            *ctx.skip_count = 1;
            return Ok(RuleResult::Consumed);
        }

        if ctx.current_char() == '位' && ctx.next_char() == Some('ㄹ') {
            ctx.emit_slice(&encode_unicode_cells("⠸⠂"));
            *ctx.skip_count = 1;
            return Ok(RuleResult::Consumed);
        }

        if should_skip_hanja_in_context(ctx) {
            return Ok(RuleResult::Consumed);
        }

        if ctx.current_char() == 'ㅸ' && ctx.next_char() == Some('字') {
            let encoded = encode_unicode_cells("⠐⠃⠶");
            ctx.emit_slice(&encoded);
            return Ok(RuleResult::Consumed);
        }

        let Some(encoded) = encode_historical_letter_symbol(ctx.current_char()) else {
            return Ok(RuleResult::Skip);
        };
        ctx.emit_slice(&encoded);
        Ok(RuleResult::Consumed)
    }
}
