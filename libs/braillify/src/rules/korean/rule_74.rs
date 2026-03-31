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
            && matches!(ctx.current_char(), '/' | '#' | '@' | '.')
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let encoded: Vec<u8> =
            match crate::symbol_shortcut::encode_char_symbol_shortcut(ctx.current_char()) {
                Ok(bytes) => bytes.to_vec(),
                Err(_) => match ctx.current_char() {
                    '#' => vec![
                        crate::unicode::decode_unicode('⠸'),
                        crate::unicode::decode_unicode('⠹'),
                    ],
                    '@' => vec![
                        crate::unicode::decode_unicode('⠈'),
                        crate::unicode::decode_unicode('⠁'),
                    ],
                    _ => return Err("unsupported digital notation symbol".to_string()),
                },
            };
        ctx.emit_slice(&encoded);
        Ok(RuleResult::Consumed)
    }
}
