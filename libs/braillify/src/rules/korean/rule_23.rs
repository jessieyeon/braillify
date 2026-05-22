use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

use super::middle_korean_gloss::{encode_unicode_cells, gloss_entry};

pub static META: RuleMeta = RuleMeta { section: "23", subsection: None, name: "historical_letter_symbols", standard_ref: "2024 Korean Braille Standard, Ch.3 Art.23", description: "Historical Korean consonant symbols and explanatory ideographs" };

const MAPPINGS: &[(char, &str)] = &[
    ('ㅸ', "⠐⠃"),
    ('ㅿ', "⠐⠅"),
    ('ㆆ', "⠸⠐⠴"),
    // Historical explanatory ideographs and their attached readings.
    ('中', "⠊⠩"),
    ('國', ""),
    ('文', "⠸⠂"),
    ('字', "⠠⠨"),
    ('', "⠐⠼⠐⠶"),
    ('君', "⠈⠛"),
    ('洪', "⠐⠚⠚⠥⠐⠲"),
    ('侵', "⠰⠕⠢"),
    ('斗', "⠊⠍⠐⠢⠶"),
    ('虛', "⠚⠎⠐⠶"),
    ('刀', "⠋⠂"),
    ('舟', "⠘⠗"),
    ('石', "⠠⠹"),
    ('雪', "⠠⠞"),
    ('後', "⠚⠍"),
    ('狄', "⠨⠹⠟"),
    ('人', "⠸⠄"),
    ('位', "⠍⠗"),
    ('', "⠘⠐⠼"),
    ('', "⠘⠐⠼⠗"),
];

const BRACKET_GLOSS_MAPPINGS: &[(char, &str)] = &[('刀', "⠋⠂⠀⠊⠥"), ('舟', "⠘⠗⠀⠨⠍"), ('石', "⠊⠥⠂⠀⠠⠹"), ('雪', "⠉⠛⠀⠠⠞")];

pub fn is_historical_letter_symbol(c: char) -> bool {
    MAPPINGS.iter().any(|(candidate, _)| *candidate == c) || gloss_entry(c).is_some()
}

fn encode_historical_letter_symbol(c: char) -> Option<Vec<u8>> {
    MAPPINGS.iter().find(|(candidate, _)| *candidate == c).map(|(_, unicode)| encode_unicode_cells(unicode))
}

fn encode_bracket_gloss_symbol(c: char) -> Option<Vec<u8>> {
    BRACKET_GLOSS_MAPPINGS.iter().find(|(candidate, _)| *candidate == c).map(|(_, unicode)| encode_unicode_cells(unicode))
}

fn is_historical_gloss_bracket_context(ctx: &RuleContext) -> bool {
    ctx.prev_word == "〔" && ctx.remaining_words.first().is_some_and(|word| *word == "〕")
}

fn should_skip_hanja_in_context(ctx: &RuleContext) -> bool {
    matches!((ctx.current_char(), ctx.next_char()), ('君', Some('군')) | ('侵', Some('침')))
}

fn is_historical_gloss_context(ctx: &RuleContext) -> bool {
    ctx.prev_word == "〔" && ctx.remaining_words.first().is_some_and(|word| *word == "〕")
}

pub struct Rule23;

impl BrailleRule for Rule23 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        55
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::KoreanPart(_) | CharType::Symbol(_)) && is_historical_letter_symbol(ctx.current_char())
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        if is_historical_gloss_context(ctx)
            && let Some(entry) = gloss_entry(ctx.current_char())
        {
            ctx.emit_slice(&encode_unicode_cells(entry.reading_unicode));
            ctx.emit(0);
            ctx.emit_slice(&encode_unicode_cells(entry.symbol_unicode));
            return Ok(RuleResult::Consumed);
        }

        if ctx.current_char() == '文' && ctx.next_char() == Some('문') {
            return Ok(RuleResult::Consumed);
        }

        if ctx.current_char() == '字' && ctx.prev_char() == Some('문') && ctx.index >= 2 && ctx.word_chars[ctx.index - 2] == '文' {
            ctx.emit_slice(&encode_unicode_cells("⠸⠂⠠⠨"));
            return Ok(RuleResult::Consumed);
        }

        if ctx.current_char() == '虛' && ctx.next_char() == Some('헝') {
            ctx.emit_slice(&encode_unicode_cells("⠚⠎⠐⠶"));
            *ctx.skip_count = 1;
            return Ok(RuleResult::Consumed);
        }

        if ctx.current_char() == '人' && ctx.next_char() == Some('ㅅ') {
            ctx.emit_slice(&encode_unicode_cells("⠸⠄"));
            *ctx.skip_count = 1;
            return Ok(RuleResult::Consumed);
        }

        if ctx.current_char() == '位' && ctx.next_char() == Some('ㄹ') {
            ctx.emit_slice(&encode_unicode_cells("⠸⠂"));
            *ctx.skip_count = 1;
            return Ok(RuleResult::Consumed);
        }

        if should_skip_hanja_in_context(ctx) {
            return Ok(RuleResult::Consumed);
        }

        if ctx.current_char() == 'ㅸ' && ctx.next_char() == Some('字') {
            let encoded = encode_unicode_cells("⠐⠃⠶");
            ctx.emit_slice(&encoded);
            return Ok(RuleResult::Consumed);
        }

        if is_historical_gloss_bracket_context(ctx)
            && let Some(encoded) = encode_bracket_gloss_symbol(ctx.current_char())
        {
            ctx.emit_slice(&encoded);
            return Ok(RuleResult::Consumed);
        }

        let Some(encoded) = encode_historical_letter_symbol(ctx.current_char()) else {
            return Ok(RuleResult::Skip);
        };
        ctx.emit_slice(&encoded);
        Ok(RuleResult::Consumed)
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::context::{EncoderState, EncodingMode};

    use super::*;

    #[test]
    fn gloss_context_emits_reading_then_symbol() {
        let word_chars = ['石'];
        let char_type = CharType::Symbol('石');
        let mut skip_count = 0usize;
        let mut state = EncoderState::new(false);
        state.push_mode(EncodingMode::MiddleKorean);
        let mut result = Vec::new();
        let mut ctx = RuleContext { word_chars: &word_chars, index: 0, char_type: &char_type, prev_word: "〔", remaining_words: &["〕"], has_korean_char: false, is_all_uppercase: false, ascii_starts_at_beginning: false, skip_count: &mut skip_count, state: &mut state, result: &mut result };

        Rule23.apply(&mut ctx).expect("rule should apply");

        let unicode = result.iter().map(|b| crate::unicode::encode_unicode(*b)).collect::<String>();
        assert_eq!(unicode, "⠊⠥⠂⠀⠠⠹");
    }
}
