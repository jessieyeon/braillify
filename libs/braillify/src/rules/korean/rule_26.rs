use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta {
    section: "26",
    subsection: None,
    name: "middle_korean_after_hanja",
    standard_ref: "2024 Korean Braille Standard, Ch.3 Art.26",
    description: "Middle Korean legacy syllables used after Hanja readings",
};

const MAPPINGS: &[(char, &str)] = &[
    ('烽', "⠘⠿"),
    ('火', "⠚⠧"),
    ('孟', "⠑"),
    ('子', "⠨"),
    ('', "⠐⠐⠼⠂"),
    ('', "⠐⠨⠝"),
    ('', "⠐⠼⠗⠐⠲"),
    ('', "⠐⠼"),
    ('', "⠊⠐⠼⠗"),
    ('', "⠊⠐⠼"),
    ('', "⠐⠐⠼"),
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

fn is_standalone_i_after_hanja(ctx: &RuleContext) -> bool {
    ctx.current_char() == 'ㅣ' && matches!(ctx.prev_char(), Some('火' | ''))
}

pub struct Rule26;

impl BrailleRule for Rule26 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        60
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::Symbol(c) | CharType::KoreanPart(c) if encode_legacy(*c).is_some())
            || is_standalone_i_after_hanja(ctx)
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let c = match ctx.char_type {
            CharType::Symbol(c) | CharType::KoreanPart(c) => *c,
            _ => return Ok(RuleResult::Skip),
        };

        if is_standalone_i_after_hanja(ctx) {
            ctx.emit_slice(&encode_unicode_cells("⠸⠕"));
            return Ok(RuleResult::Consumed);
        }

        let Some(encoded) = encode_legacy(c) else {
            return Ok(RuleResult::Skip);
        };

        ctx.emit_slice(&encoded);
        Ok(RuleResult::Consumed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_skips_non_korean() {
        let mut owned = crate::test_helpers::CtxOwned::for_text("A", false);
        let mut ctx = owned.ctx_at(0);
        let outcome = Rule26.apply(&mut ctx).unwrap();
        assert!(matches!(outcome, RuleResult::Skip));
    }
}
