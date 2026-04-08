use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta {
    section: "21",
    subsection: None,
    name: "middle_korean_hieuh_series",
    standard_ref: "2024 Korean Braille Standard, Ch.3 Art.21",
    description: "Middle Korean aspirated old-consonant composites",
};

const MAPPINGS: &[(char, &str)] = &[
    ('', "⠐⠉⠉⠐⠼"),
    ('', "⠚⠐⠼⠗"),
    ('', "⠐⠛⠛⠱"),
    ('', "⠐⠐⠼"),
    ('', "⠐⠚⠚⠱"),
];

fn encode_unicode_cells(unicode: &str) -> Vec<u8> {
    unicode
        .chars()
        .map(crate::unicode::decode_unicode)
        .collect()
}

fn encode_legacy(c: char) -> Option<Vec<u8>> {
    MAPPINGS
        .iter()
        .find(|(candidate, _)| *candidate == c)
        .map(|(_, unicode)| encode_unicode_cells(unicode))
}

pub struct Rule21;

impl BrailleRule for Rule21 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        54
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::Symbol(c) if encode_legacy(*c).is_some())
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let CharType::Symbol(c) = ctx.char_type else {
            return Ok(RuleResult::Skip);
        };

        let Some(encoded) = encode_legacy(*c) else {
            return Ok(RuleResult::Skip);
        };

        ctx.emit_slice(&encoded);
        Ok(RuleResult::Consumed)
    }
}
