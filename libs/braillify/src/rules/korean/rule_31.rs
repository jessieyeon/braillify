use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta {
    section: "31",
    subsection: None,
    name: "greek_letters",
    standard_ref: "2024 Korean Braille Standard, Ch.4 Art.31",
    description: "Greek letters in Korean context use Roman indicators and Greek braille cells",
};

fn greek_braille(c: char) -> Option<&'static str> {
    match c {
        'σ' => Some("⠨⠎"),
        'Φ' => Some("⠨⠋"),
        'Β' => Some("⠨⠃"),
        'Κ' => Some("⠨⠅"),
        'Ω' => Some("⠨⠺"),
        'μ' => Some("⠨⠍"),
        _ => None,
    }
}

fn encode_unicode_cells(unicode: &str) -> Vec<u8> {
    unicode
        .chars()
        .map(crate::unicode::decode_unicode)
        .collect()
}

fn korean_context(ctx: &RuleContext) -> bool {
    ctx.has_korean_char
        || ctx.prev_word.chars().any(crate::utils::is_korean_char)
        || ctx
            .remaining_words
            .first()
            .is_some_and(|word| word.chars().any(crate::utils::is_korean_char))
}

pub fn is_greek_letter(c: char) -> bool {
    greek_braille(c).is_some()
}

pub struct Rule31;

impl BrailleRule for Rule31 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        145
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::Symbol(c) if is_greek_letter(*c))
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let mut run = Vec::new();
        for ch in &ctx.word_chars[ctx.index..] {
            if is_greek_letter(*ch) {
                run.push(*ch);
            } else {
                break;
            }
        }

        if run.is_empty() {
            return Ok(RuleResult::Skip);
        }

        let korean_context = korean_context(ctx);
        if korean_context {
            ctx.emit(crate::unicode::decode_unicode('⠴'));
            if run.len() > 1 && run.iter().all(|c| c.is_uppercase()) {
                ctx.emit(crate::unicode::decode_unicode('⠠'));
                ctx.emit(crate::unicode::decode_unicode('⠠'));
            } else if run.len() == 1 && run[0].is_uppercase() {
                ctx.emit(crate::unicode::decode_unicode('⠠'));
            }
        } else if run.len() == 1 && run[0].is_uppercase() {
            ctx.emit(crate::unicode::decode_unicode('⠠'));
        }

        for ch in &run {
            let Some(unicode) = greek_braille(*ch) else {
                continue;
            };
            ctx.emit_slice(&encode_unicode_cells(unicode));
        }
        if korean_context {
            ctx.emit(crate::unicode::decode_unicode('⠲'));
        }

        if run.len() > 1 {
            *ctx.skip_count = run.len() - 1;
        }

        Ok(RuleResult::Consumed)
    }
}
