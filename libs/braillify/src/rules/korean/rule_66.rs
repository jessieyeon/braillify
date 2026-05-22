use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta { section: "66", subsection: None, name: "literal_braille_cells", standard_ref: "2024 Korean Braille Standard, Ch.6 Art.66", description: "Unicode braille input is emitted literally when used as braille cells" };

fn is_braille_cell(c: char) -> bool {
    (0x2800..=0x28ff).contains(&(c as u32))
}

pub struct Rule66;

impl BrailleRule for Rule66 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        10
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::Symbol(c) if is_braille_cell(*c))
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        ctx.emit(crate::unicode::decode_unicode(ctx.current_char()));
        Ok(RuleResult::Consumed)
    }
}
