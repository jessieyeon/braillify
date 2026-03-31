use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta {
    section: "22",
    subsection: None,
    name: "middle_korean_fortis_series",
    standard_ref: "2024 Korean Braille Standard, Ch.3 Art.22",
    description: "Middle Korean fortis/cluster legacy syllable glyphs",
};

const MAPPINGS: &[(char, &str)] = &[
    ('', "⠐⠘⠈⠪"),
    ('', "⠐⠘⠊⠪"),
    ('', "⠐⠘⠠⠣"),
    ('', "⠉⠐⠼⠒"),
    ('', "⠐⠘⠨⠣⠁"),
    ('', "⠐⠘⠓⠎"),
    ('', "⠐⠘⠠⠈⠪⠢"),
    ('', "⠐⠘⠠⠊⠗"),
    ('', "⠐⠠⠈⠎"),
    ('', "⠐⠠⠉⠣"),
    ('', "⠐⠠⠊⠐⠼⠂"),
    ('', "⠐⠠⠘⠥⠐⠲"),
    ('', "⠐⠠⠨⠺"),
    ('', "⠊⠐⠼⠂⠁⠄"),
    ('禽', "⠈⠪⠢⠵"),
    ('', "⠉⠐⠼⠂"),
    ('', "⠐⠴⠨⠩⠐⠲"),
    ('', "⠠⠐⠼⠗⠐⠲"),
    ('', "⠨⠕⠢⠄"),
    ('', "⠠⠜⠐⠲⠄"),
    ('', "⠊⠐⠼⠂"),
    ('', "⠚⠐⠼⠂"),
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

pub struct Rule22;

impl BrailleRule for Rule22 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        58
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::Symbol(c) if encode_legacy(*c).is_some())
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        if ctx.current_char() == '禽' && ctx.next_char() == Some('은') {
            ctx.emit_slice(&encode_unicode_cells("⠈⠪⠢⠵"));
            *ctx.skip_count = 1;
            return Ok(RuleResult::Consumed);
        }

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
