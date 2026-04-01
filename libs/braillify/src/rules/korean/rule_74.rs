use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta {
    section: "74",
    subsection: None,
    name: "digital_notation_symbols",
    standard_ref: "2024 Korean Braille Standard, Ch.6 Art.74",
    description: "Digital notation symbols such as slash and hash in URLs and filenames",
};

fn encode_digital_symbol(symbol: char) -> Option<Vec<u8>> {
    match symbol {
        '/' => Some(vec![
            crate::unicode::decode_unicode('⠸'),
            crate::unicode::decode_unicode('⠌'),
        ]),
        '#' => Some(vec![
            crate::unicode::decode_unicode('⠸'),
            crate::unicode::decode_unicode('⠹'),
        ]),
        '@' => Some(vec![
            crate::unicode::decode_unicode('⠈'),
            crate::unicode::decode_unicode('⠁'),
        ]),
        '.' => Some(vec![crate::unicode::decode_unicode('⠲')]),
        ':' => Some(vec![crate::unicode::decode_unicode('⠒')]),
        '_' => Some(vec![
            crate::unicode::decode_unicode('⠨'),
            crate::unicode::decode_unicode('⠤'),
        ]),
        _ => None,
    }
}

fn is_digital_notation_context(ctx: &RuleContext) -> bool {
    let text: String = ctx.word_chars.iter().collect();
    let has_ascii = ctx.word_chars.iter().any(|ch| ch.is_ascii_alphanumeric());

    has_ascii
        && (text.contains("//") || text.contains('@') || text.contains('#') || text.contains('_'))
}

pub struct Rule74;

impl BrailleRule for Rule74 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        176
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::Symbol(_))
            && is_digital_notation_context(ctx)
            && matches!(ctx.current_char(), '/' | '#' | '@' | '.' | ':' | '_')
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let Some(encoded) = encode_digital_symbol(ctx.current_char()) else {
            return Err("unsupported digital notation symbol".to_string());
        };
        ctx.emit_slice(&encoded);
        Ok(RuleResult::Consumed)
    }
}
