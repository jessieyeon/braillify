use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::{EncodingMode, RuleContext};
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta {
    section: "71",
    subsection: None,
    name: "information_symbols",
    standard_ref: "2024 Korean Braille Standard, Ch.6 Art.71",
    description: "Keyboard, copyright, and information symbols",
};

const MAPPINGS: &[(char, &str)] = &[
    ('@', "⠈⠁"),
    ('^', "⠈⠢"),
    ('#', "⠸⠹"),
    ('|', "⠸⠳"),
    ('\\', "⠸⠡"),
    ('&', "⠈⠯"),
    ('§', "⠘⠎"),
    ('¶', "⠘⠏"),
    ('©', "⠘⠉"),
    ('®', "⠘⠗"),
    ('™', "⠘⠞"),
];

fn encode_unicode_cells(unicode: &str) -> Vec<u8> {
    unicode
        .chars()
        .map(crate::unicode::decode_unicode)
        .collect()
}

fn should_wrap_information_symbol(ctx: &RuleContext) -> bool {
    if ctx.word_len() > 1 {
        return true;
    }

    let prev_has_korean =
        !ctx.prev_word.is_empty() && ctx.prev_word.chars().any(crate::utils::is_korean_char);
    let next_has_korean = ctx
        .remaining_words
        .first()
        .is_some_and(|word| !word.is_empty() && word.chars().any(crate::utils::is_korean_char));

    prev_has_korean || next_has_korean
}

pub fn is_rule_71_symbol(c: char) -> bool {
    MAPPINGS.iter().any(|(candidate, _)| *candidate == c)
}

pub struct Rule71;

impl BrailleRule for Rule71 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        175
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        ctx.state.current_mode() != EncodingMode::Math
            && matches!(ctx.char_type, CharType::Symbol(c) if is_rule_71_symbol(*c))
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        if ctx.current_char() == '§' {
            if should_wrap_information_symbol(ctx) {
                let mut encoded = vec![crate::unicode::decode_unicode('⠴')];
                encoded.extend(encode_unicode_cells("⠘⠎"));
                if !ctx.next_char().is_some_and(|ch| ch.is_ascii_digit()) {
                    encoded.push(crate::unicode::decode_unicode('⠲'));
                }
                if ctx.index > 0 {
                    ctx.emit(0);
                }
                ctx.emit_slice(&encoded);
                return Ok(RuleResult::Consumed);
            }

            let encoded = encode_unicode_cells("⠘⠎");
            ctx.emit_slice(&encoded);
            return Ok(RuleResult::Consumed);
        }

        let Some((_, unicode)) = MAPPINGS
            .iter()
            .find(|(candidate, _)| *candidate == ctx.current_char())
        else {
            return Ok(RuleResult::Skip);
        };

        let mut encoded = Vec::new();
        if should_wrap_information_symbol(ctx)
            && matches!(ctx.current_char(), '&' | '¶' | '©' | '®' | '™')
        {
            encoded.push(crate::unicode::decode_unicode('⠴'));
            encoded.extend(encode_unicode_cells(unicode));
            encoded.push(crate::unicode::decode_unicode('⠲'));
        } else {
            encoded = encode_unicode_cells(unicode);
        }
        ctx.emit_slice(&encoded);
        Ok(RuleResult::Consumed)
    }
}
