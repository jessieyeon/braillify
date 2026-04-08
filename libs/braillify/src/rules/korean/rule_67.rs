use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta {
    section: "67",
    subsection: None,
    name: "braille_cell_mentions",
    standard_ref: "2024 Korean Braille Standard, Ch.6 Art.67",
    description: "Braille cells mentioned inside prose are prefixed with ⠸⠿",
};

fn is_braille_cell(c: char) -> bool {
    (0x2800..=0x28ff).contains(&(c as u32))
}

fn explanatory_context(ctx: &RuleContext) -> bool {
    let braille_run_len = ctx.word_chars[ctx.index..]
        .iter()
        .take_while(|&&c| is_braille_cell(c))
        .count();
    let mentions_symbol_name = ctx
        .remaining_words
        .iter()
        .take(3)
        .any(|word| word.contains("기호"));

    (ctx.has_korean_char && braille_run_len == 1) || (!ctx.has_korean_char && mentions_symbol_name)
}

pub struct Rule67;

impl BrailleRule for Rule67 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        9
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::Symbol(c) if is_braille_cell(*c))
            && explanatory_context(ctx)
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let is_start = ctx.prev_char().is_none_or(|c| !is_braille_cell(c));
        if is_start {
            ctx.emit(crate::unicode::decode_unicode('⠸'));
            ctx.emit(crate::unicode::decode_unicode('⠿'));
        }
        ctx.emit(crate::unicode::decode_unicode(ctx.current_char()));

        // 제67항: 본문 중 점형을 설명할 때는 점형 뒤 설명어를 띄어 쓴다.
        if is_start && ctx.next_char().is_some_and(crate::utils::is_korean_char) {
            ctx.emit(0);
        }

        Ok(RuleResult::Consumed)
    }
}
