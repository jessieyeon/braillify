use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::EncodingMode;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta {
    section: "27",
    subsection: None,
    name: "middle_korean_tone_marks",
    standard_ref: "2024 Korean Braille Standard, Ch.3 Art.27",
    description: "Middle Korean tone marks: 거성(·), 상성(：)",
};

const GEOSEONG: [u8; 2] = [
    crate::unicode::decode_unicode('⠸'),
    crate::unicode::decode_unicode('⠂'),
];
const SANGSEONG: [u8; 2] = [
    crate::unicode::decode_unicode('⠸'),
    crate::unicode::decode_unicode('⠅'),
];

fn is_historical_context_word(word: &str) -> bool {
    word.chars().any(|c| {
        let code = c as u32;
        (0xE000..=0xF8FF).contains(&code)
            || (0x4E00..=0x9FFF).contains(&code)
            || matches!(c, '：' | '〔' | '〕')
    })
}

fn has_historical_context(ctx: &RuleContext) -> bool {
    if is_historical_context_word(&ctx.word_chars.iter().collect::<String>()) {
        return true;
    }
    if is_historical_context_word(ctx.prev_word) {
        return true;
    }
    ctx.remaining_words
        .iter()
        .take(2)
        .any(|word| is_historical_context_word(word))
}

fn is_middle_korean_geoseong(ctx: &RuleContext) -> bool {
    if !matches!(ctx.char_type, CharType::Symbol('·')) {
        return false;
    }

    if ctx.word_len() == 1 {
        return ctx.state.current_mode() == EncodingMode::MiddleKorean;
    }

    if ctx.prev_char().is_some_and(|prev| prev.is_ascii_digit())
        && ctx.next_char().is_some_and(|next| next.is_ascii_digit())
    {
        return false;
    }

    (ctx.index == 0 && ctx.next_char().is_some()) || has_historical_context(ctx)
}

fn is_middle_korean_particle_geoseong(ctx: &RuleContext) -> bool {
    matches!(ctx.char_type, CharType::Symbol('·'))
        && ctx.state.current_mode() == EncodingMode::MiddleKorean
        && ctx.next_char() == Some('에')
}

fn is_inline_gloss_separator(ctx: &RuleContext) -> bool {
    matches!(ctx.char_type, CharType::Symbol('·'))
        && ctx.state.current_mode() == EncodingMode::MiddleKorean
        && ctx.prev_char() == Some('字')
        && ctx.next_char() == Some('')
}

fn is_middle_korean_sangseong(ctx: &RuleContext) -> bool {
    matches!(ctx.char_type, CharType::Symbol('：'))
}

pub struct Rule27;

impl BrailleRule for Rule27 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        51
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        let is_potential_tone_mark = matches!(ctx.char_type, CharType::Symbol('·' | '：'));
        if !is_potential_tone_mark {
            return false;
        }

        if ctx.state.current_mode() == EncodingMode::MiddleKorean {
            return true;
        }

        is_middle_korean_geoseong(ctx) || is_middle_korean_sangseong(ctx)
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let CharType::Symbol(c) = ctx.char_type else {
            return Ok(RuleResult::Skip);
        };

        match c {
            '·' if is_inline_gloss_separator(ctx) => {}
            '·' if is_middle_korean_particle_geoseong(ctx) => {
                ctx.emit(0);
                ctx.emit_slice(&GEOSEONG);
            }
            '·' if ctx.state.current_mode() == EncodingMode::MiddleKorean => {
                ctx.emit_slice(&GEOSEONG);
            }
            '·' if is_middle_korean_geoseong(ctx) => ctx.emit_slice(&GEOSEONG),
            '：' => ctx.emit_slice(&SANGSEONG),
            _ => return Ok(RuleResult::Skip),
        }

        Ok(RuleResult::Consumed)
    }
}
