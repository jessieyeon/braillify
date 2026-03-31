use crate::char_struct::CharType;
use crate::english;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta {
    section: "69",
    subsection: None,
    name: "measurement_symbols",
    standard_ref: "2024 Korean Braille Standard, Ch.6 Art.69",
    description: "Measurement and scientific unit symbols",
};

const SINGLE_MAPPINGS: &[(char, &str)] = &[
    ('㎎', "⠴⠍⠛"),
    ('㎗', "⠙⠇⠲"),
    ('㎠', "⠉⠍⠘⠼⠃"),
    ('㎞', "⠴⠅⠍⠲"),
    ('㎒', "⠴⠠⠍⠠⠓⠵⠲"),
    ('Ω', "⠴⠠⠨⠺⠲"),
    ('%', "⠴⠏"),
    ('‰', "⠴⠏⠍"),
    ('°', "⠴⠙"),
    ('℃', "⠴⠙⠠⠉"),
    ('℉', "⠴⠙⠠⠋"),
    ('′', "⠴⠤"),
    ('″', "⠴⠤⠤"),
    ('Å', "⠴⠡"),
];

fn encode_unicode_cells(unicode: &str) -> Vec<u8> {
    unicode
        .chars()
        .map(crate::unicode::decode_unicode)
        .collect()
}

pub fn is_rule_69_symbol(c: char) -> bool {
    SINGLE_MAPPINGS.iter().any(|(candidate, _)| *candidate == c) || c == 'μ'
}

pub struct Rule69;

impl BrailleRule for Rule69 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        165
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::Symbol(c) if is_rule_69_symbol(*c))
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        if ctx.current_char() == '%'
            && ctx.word_chars.get(ctx.index + 1) == Some(&'i')
            && ctx.word_chars.get(ctx.index + 2) == Some(&'l')
            && ctx.word_chars.get(ctx.index + 3) == Some(&'e')
        {
            let encoded = encode_unicode_cells("⠴⠏⠞");
            ctx.emit_slice(&encoded);
            *ctx.skip_count = 3;
            return Ok(RuleResult::Consumed);
        }

        if ctx.current_char() == 'μ' {
            let mut encoded = vec![
                crate::unicode::decode_unicode('⠨'),
                english::encode_english('m')?,
            ];
            let mut ascii_count = 0usize;
            for ch in &ctx.word_chars[ctx.index + 1..] {
                if ch.is_ascii_alphabetic() {
                    if ascii_count == 0 {
                        encoded.insert(0, crate::unicode::decode_unicode('⠴'));
                    }
                    encoded.push(english::encode_english(*ch)?);
                    ascii_count += 1;
                } else {
                    break;
                }
            }
            if ascii_count > 0 {
                encoded.push(crate::unicode::decode_unicode('⠲'));
            }
            ctx.emit_slice(&encoded);
            if ascii_count > 0 {
                *ctx.skip_count = ascii_count;
            }
            return Ok(RuleResult::Consumed);
        }

        let Some((_, unicode)) = SINGLE_MAPPINGS
            .iter()
            .find(|(candidate, _)| *candidate == ctx.current_char())
        else {
            return Ok(RuleResult::Skip);
        };
        let encoded = encode_unicode_cells(unicode);
        ctx.emit_slice(&encoded);
        Ok(RuleResult::Consumed)
    }
}
