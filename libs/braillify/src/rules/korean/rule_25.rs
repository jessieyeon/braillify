use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta {
    section: "25",
    subsection: None,
    name: "middle_korean_vowels",
    standard_ref: "2024 Korean Braille Standard, Ch.3 Art.25",
    description: "Middle Korean standalone vowels such as ㆍ, ㆎ, ㆇ-ㆌ",
};

const MAPPINGS: &[(char, &str)] = &[
    ('轉', "⠊"),
    ('榮', "⠐⠙"),
    ('ㆍ', "⠐⠼"),
    ('ㆎ', "⠐⠼⠗"),
    ('ㆇ', "⠸⠬⠜"),
    ('ㆈ', "⠸⠬⠜⠗"),
    ('ㆉ', "⠸⠬⠕"),
    ('ㆊ', "⠸⠩⠱"),
    ('ㆋ', "⠸⠩⠌"),
    ('ㆌ', "⠸⠩⠕"),
    ('', "⠈⠐⠼"),
    ('', "⠐⠨⠐⠼⠂"),
    ('', "⠐⠼⠗"),
    ('', "⠑⠐⠼⠗"),
    ('', "⠚⠐⠼⠒"),
    ('', "⠠⠸⠬⠕"),
    ('', "⠸⠩⠱⠒"),
    ('', "⠐⠙⠧⠐⠲"),
    ('', "⠸⠩⠱⠐⠲"),
    ('', "⠜⠐⠲"),
    ('', "⠰⠸⠩⠌"),
    ('', "⠸⠩⠕"),
    ('', "⠰⠸⠩⠕"),
    ('', "⠚⠐⠼"),
];

const SILENT_HANJA: &[char] = &['輪', '王', '養', '砌'];

fn encode_unicode_cells(unicode: &str) -> Vec<u8> {
    unicode
        .chars()
        .map(crate::unicode::decode_unicode)
        .collect()
}

fn is_middle_korean_vowel(c: char) -> bool {
    MAPPINGS.iter().any(|(candidate, _)| *candidate == c) || SILENT_HANJA.contains(&c)
}

pub fn is_rule_25_symbol(c: char) -> bool {
    is_middle_korean_vowel(c)
}

pub struct Rule25;

impl BrailleRule for Rule25 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        56
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::KoreanPart(c) if is_middle_korean_vowel(*c))
            || matches!(ctx.char_type, CharType::Symbol(c) if is_middle_korean_vowel(*c))
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let c = match ctx.char_type {
            CharType::KoreanPart(c) | CharType::Symbol(c) => *c,
            _ => return Ok(RuleResult::Skip),
        };
        if SILENT_HANJA.contains(&c) {
            return Ok(RuleResult::Consumed);
        }
        let Some((_, unicode)) = MAPPINGS.iter().find(|(candidate, _)| *candidate == c) else {
            return Ok(RuleResult::Skip);
        };
        let encoded = encode_unicode_cells(unicode);
        ctx.emit_slice(&encoded);
        Ok(RuleResult::Consumed)
    }
}
