use crate::char_struct::{CharType, KoreanChar};
use crate::english_logic;
use crate::fraction;
use crate::rules::context::{EncoderState, RuleContext};
use crate::rules::engine::RuleEngine;
use crate::rules::traits::Phase;

use super::token::{DocumentIR, ModeEvent, SpaceKind, Token, WordMeta, WordToken};

pub fn emit(ir: &mut DocumentIR, char_engine: &mut RuleEngine) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();

    for token in &ir.tokens {
        match token {
            Token::Word(word) => {
                emit_word(word, &mut ir.state, char_engine, &ir.tokens, &mut result)?;
            }
            Token::Space(SpaceKind::Regular) => result.push(0),
            Token::Mode(event) => emit_mode_event(*event, &mut ir.state, &mut result),
            Token::Fraction(frac) => {
                if let Some(ref w) = frac.whole {
                    result.extend(fraction::encode_mixed_fraction(
                        w,
                        &frac.numerator,
                        &frac.denominator,
                    )?);
                } else {
                    result.extend(fraction::encode_fraction(
                        &frac.numerator,
                        &frac.denominator,
                    )?);
                }
                ir.state.is_number = true;
            }
            Token::PreEncoded(bytes) => result.extend(bytes),
        }
    }

    // End-of-stream: close triple uppercase if active (Encoder::finish)
    if ir.state.triple_big_english {
        result.push(32);
        result.push(4);
    }

    Ok(result)
}

fn emit_mode_event(event: ModeEvent, state: &mut EncoderState, result: &mut Vec<u8>) {
    match event {
        ModeEvent::EnterEnglish => {
            result.push(52);
            state.is_english = true;
            state.needs_english_continuation = false;
        }
        ModeEvent::EnterEnglishContinue => {
            result.push(48);
            state.is_english = true;
            state.needs_english_continuation = false;
        }
        ModeEvent::CapsWord => {
            result.push(32);
            result.push(32);
        }
        ModeEvent::CapsPassageStart => {
            result.push(32);
            result.push(32);
            result.push(32);
            state.triple_big_english = true;
        }
        ModeEvent::CapsPassageEnd => {
            result.push(32);
            result.push(4);
            state.triple_big_english = false;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_core_encoding_rules(
    engine: &mut RuleEngine,
    char_type: &CharType,
    word_chars: &[char],
    index: usize,
    is_all_uppercase: bool,
    has_korean_char: bool,
    ascii_starts_at_beginning: bool,
    state: &mut EncoderState,
    skip_count: &mut usize,
    remaining_words: &[&str],
    prev_word: &str,
    result: &mut Vec<u8>,
) -> Result<crate::rules::traits::RuleResult, String> {
    let mut ctx = RuleContext {
        word_chars,
        index,
        char_type,
        prev_word,
        remaining_words,
        has_korean_char,
        is_all_uppercase,
        ascii_starts_at_beginning,
        skip_count,
        state,
        result,
    };
    engine.apply_phase(Phase::CoreEncoding, &mut ctx)
}

#[allow(clippy::too_many_arguments)]
fn apply_inter_character_rules(
    engine: &mut RuleEngine,
    char_type: &CharType,
    word_chars: &[char],
    index: usize,
    is_all_uppercase: bool,
    has_korean_char: bool,
    ascii_starts_at_beginning: bool,
    state: &mut EncoderState,
    skip_count: &mut usize,
    remaining_words: &[&str],
    prev_word: &str,
    result: &mut Vec<u8>,
) -> Result<(), String> {
    let mut ctx = RuleContext {
        word_chars,
        index,
        char_type,
        prev_word,
        remaining_words,
        has_korean_char,
        is_all_uppercase,
        ascii_starts_at_beginning,
        skip_count,
        state,
        result,
    };
    engine.apply_phase(Phase::InterCharacter, &mut ctx)?;
    Ok(())
}

fn exit_english(state: &mut EncoderState, needs_continuation: bool) {
    state.is_english = false;
    state.needs_english_continuation = needs_continuation;
}

fn enter_english(state: &mut EncoderState, result: &mut Vec<u8>) {
    if state.needs_english_continuation {
        result.push(48);
    } else {
        result.push(52);
    }
    state.is_english = true;
    state.needs_english_continuation = false;
}

fn extract_word_context<'a>(
    word: &WordToken<'a>,
    all_tokens: &'a [Token<'a>],
) -> (&'a str, Vec<&'a str>) {
    let mut prev_word = "";
    let mut remaining_words = Vec::new();
    let mut seen_current = false;

    for token in all_tokens {
        if let Token::Word(candidate) = token {
            if !seen_current {
                if std::ptr::eq(candidate, word) {
                    seen_current = true;
                } else {
                    prev_word = candidate.text.as_ref();
                }
            } else {
                remaining_words.push(candidate.text.as_ref());
            }
        }
    }

    (prev_word, remaining_words)
}

fn emit_word(
    word: &WordToken,
    state: &mut EncoderState,
    char_engine: &mut RuleEngine,
    all_tokens: &[Token],
    result: &mut Vec<u8>,
) -> Result<(), String> {
    let (prev_word, remaining_words_vec) = extract_word_context(word, all_tokens);
    let remaining_words = remaining_words_vec.as_slice();

    let word_text = word.text.as_ref();

    // ── [D] Per-character loop (encoder.rs:201-409) ──
    let word_chars: Vec<char> = word_text.chars().collect();
    let word_len = word_chars.len();

    if word_len > 0 {
        let meta = WordMeta::from_chars(&word_chars);
        let is_all_uppercase = meta.is_all_uppercase;
        let has_korean_char = meta.has_korean;
        let has_ascii_alphabetic = meta.has_ascii_alphabetic;

        // English entry (encoder.rs:216-223)
        if state.english_indicator
            && !state.is_english
            && has_ascii_alphabetic
            && word_chars[0].is_ascii_alphabetic()
        {
            enter_english(state, result);
        }

        let first_ascii_index = word_chars.iter().position(|c| c.is_ascii_alphabetic());
        let ascii_starts_at_beginning = matches!(first_ascii_index, Some(0));

        let mut is_number = false;
        let mut is_big_english = false;
        let mut skip_count = 0usize;

        // Per-char loop (encoder.rs:251-409)
        for (i, c) in word_chars.iter().enumerate() {
            if skip_count > 0 {
                skip_count -= 1;
                continue;
            }

            let char_type = CharType::new(*c)?;

            // English exit state machine (encoder.rs:259-294)
            if state.english_indicator && state.is_english {
                match &char_type {
                    CharType::English(_) => {}
                    CharType::Number(_) => {
                        exit_english(state, true);
                    }
                    CharType::Symbol(sym) => {
                        if english_logic::should_render_symbol_as_english(
                            state.english_indicator,
                            state.is_english,
                            &state.parenthesis_stack,
                            *sym,
                            &word_chars,
                            i,
                            remaining_words,
                        ) {
                        } else if english_logic::should_force_terminator_before_symbol(*sym)
                            || !english_logic::should_skip_terminator_for_symbol(*sym)
                        {
                            result.push(50);
                            exit_english(state, false);
                        } else {
                            exit_english(state, english_logic::should_request_continuation(*sym));
                        }
                    }
                    _ => {
                        result.push(50);
                        exit_english(state, false);
                    }
                }
            }

            // Pre-engine type-specific checks (encoder.rs:296-327)
            match &char_type {
                CharType::Korean(_) | CharType::KoreanPart(_) => {
                    state.needs_english_continuation = false;
                }
                CharType::Number(_) => {}
                _ => {}
            }

            // CoreEncoding via engine (encoder.rs:330-360)
            let mut core_state = EncoderState {
                mode_stack: state.mode_stack.clone(),
                is_english: state.is_english,
                english_indicator: state.english_indicator,
                triple_big_english: state.triple_big_english,
                has_processed_word: state.has_processed_word,
                needs_english_continuation: state.needs_english_continuation,
                parenthesis_stack: state.parenthesis_stack.clone(),
                is_number,
                is_big_english,
            };
            apply_core_encoding_rules(
                char_engine,
                &char_type,
                &word_chars,
                i,
                is_all_uppercase,
                has_korean_char,
                ascii_starts_at_beginning,
                &mut core_state,
                &mut skip_count,
                remaining_words,
                prev_word,
                result,
            )?;
            state.is_english = core_state.is_english;
            state.triple_big_english = core_state.triple_big_english;
            state.has_processed_word = core_state.has_processed_word;
            state.needs_english_continuation = core_state.needs_english_continuation;
            state.parenthesis_stack = core_state.parenthesis_stack;
            state.mode_stack = core_state.mode_stack;
            is_number = core_state.is_number;
            is_big_english = core_state.is_big_english;

            // InterCharacter via engine (encoder.rs:362-402)
            if let CharType::Korean(ref korean) = char_type
                && i < word_len - 1
            {
                let recon_type = CharType::Korean(KoreanChar {
                    cho: korean.cho,
                    jung: korean.jung,
                    jong: korean.jong,
                });
                let mut inter_state = EncoderState {
                    mode_stack: state.mode_stack.clone(),
                    is_english: state.is_english,
                    english_indicator: state.english_indicator,
                    triple_big_english: state.triple_big_english,
                    has_processed_word: state.has_processed_word,
                    needs_english_continuation: state.needs_english_continuation,
                    parenthesis_stack: state.parenthesis_stack.clone(),
                    is_number,
                    is_big_english,
                };
                apply_inter_character_rules(
                    char_engine,
                    &recon_type,
                    &word_chars,
                    i,
                    is_all_uppercase,
                    has_korean_char,
                    ascii_starts_at_beginning,
                    &mut inter_state,
                    &mut skip_count,
                    remaining_words,
                    prev_word,
                    result,
                )?;
                state.is_english = inter_state.is_english;
                state.triple_big_english = inter_state.triple_big_english;
                state.has_processed_word = inter_state.has_processed_word;
                state.needs_english_continuation = inter_state.needs_english_continuation;
                state.parenthesis_stack = inter_state.parenthesis_stack;
                state.mode_stack = inter_state.mode_stack;
                is_number = inter_state.is_number;
                is_big_english = inter_state.is_big_english;
            }

            // Post-char state reset (encoder.rs:403-408)
            if !c.is_numeric() {
                is_number = false;
            }
            if c.is_ascii_alphabetic() && !c.is_uppercase() {
                is_big_english = false;
            }
        }
    }

    // ── [F] Post-loop: English termination for next word (encoder.rs:424-482) ──
    // Space between words is handled by Token::Space, NOT emitted here.
    if !remaining_words.is_empty()
        && state.english_indicator
        && state.is_english
        && let Some(next_word) = remaining_words.first()
    {
        let ascii_letters = next_word
            .chars()
            .filter(|c| c.is_ascii_alphabetic())
            .collect::<Vec<_>>();
        let has_invalid_symbol = next_word.chars().any(|ch| {
            !(ch.is_ascii_alphabetic()
                || english_logic::is_english_symbol(ch)
                || crate::symbol_shortcut::is_symbol_char(ch)
                || crate::utils::is_korean_char(ch))
        });
        let is_single_letter_word = ascii_letters.len() == 1
            && !next_word.chars().any(|ch| ch.is_ascii_digit())
            && !has_invalid_symbol;

        if is_single_letter_word
            && english_logic::requires_single_letter_continuation(ascii_letters[0])
        {
            exit_english(state, true);
        } else if let Some(next_char) = next_word.chars().next() {
            if let Ok(next_type) = CharType::new(next_char) {
                match next_type {
                    CharType::English(_) | CharType::Number(_) => {}
                    CharType::Symbol(sym) => {
                        if state.english_indicator
                            && state.is_english
                            && english_logic::is_english_symbol(sym)
                        {
                            // 연속되는 영어 구절 사이에 오는 영어 문장 부호는
                            // 로마자 구간을 유지한다.
                        } else if english_logic::should_force_terminator_before_symbol(sym)
                            || !english_logic::should_skip_terminator_for_symbol(sym)
                        {
                            result.push(50);
                            exit_english(state, false);
                        } else {
                            exit_english(state, english_logic::should_request_continuation(sym));
                        }
                    }
                    _ => {
                        result.push(50);
                        exit_english(state, false);
                    }
                }
            } else {
                result.push(50);
                exit_english(state, false);
            }
        }
    }

    // ── [G] has_processed_word (encoder.rs:501-504) ──
    if !state.has_processed_word {
        state.has_processed_word = true;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::encode;
    use crate::rules::korean::rule_1::Rule1;
    use crate::utils;

    use super::*;

    fn english_indicator(text: &str) -> bool {
        text.split(' ')
            .filter(|word| !word.is_empty())
            .any(|word| word.chars().any(utils::is_korean_char))
    }

    fn make_char_engine() -> RuleEngine {
        let mut engine = RuleEngine::new();
        engine.register(Box::new(crate::rules::korean::rule_53::Rule53));
        engine.register(Box::new(crate::rules::korean::rule_18::Rule18));
        engine.register(Box::new(crate::rules::korean::rule_29::Rule29));
        engine.register(Box::new(crate::rules::korean::rule_44::Rule44));
        engine.register(Box::new(crate::rules::korean::rule_16::Rule16));
        engine.register(Box::new(crate::rules::korean::rule_14::Rule14));
        engine.register(Box::new(crate::rules::korean::rule_13::Rule13));
        engine.register(Box::new(crate::rules::korean::rule_korean::RuleKorean));
        engine.register(Box::new(crate::rules::korean::rule_28::Rule28));
        engine.register(Box::new(crate::rules::korean::rule_40::Rule40));
        engine.register(Box::new(crate::rules::korean::rule_8::Rule8));
        engine.register(Box::new(Rule1));
        engine.register(Box::new(crate::rules::korean::rule_2::Rule2));
        engine.register(Box::new(crate::rules::korean::rule_3::Rule3));
        engine.register(Box::new(
            crate::rules::korean::rule_english_symbol::RuleEnglishSymbol,
        ));
        engine.register(Box::new(crate::rules::korean::rule_61::Rule61));
        engine.register(Box::new(crate::rules::korean::rule_41::Rule41));
        engine.register(Box::new(crate::rules::korean::rule_56::Rule56));
        engine.register(Box::new(crate::rules::korean::rule_57::Rule57));
        engine.register(Box::new(crate::rules::korean::rule_58::Rule58));
        engine.register(Box::new(crate::rules::korean::rule_60::Rule60));
        engine.register(Box::new(crate::rules::korean::rule_49::Rule49));
        engine.register(Box::new(crate::rules::korean::rule_space::RuleSpace));
        engine.register(Box::new(crate::rules::korean::rule_math::RuleMath));
        engine.register(Box::new(crate::rules::korean::rule_fraction::RuleFraction));
        engine.register(Box::new(crate::rules::korean::rule_11::Rule11));
        engine.register(Box::new(crate::rules::korean::rule_12::Rule12));
        engine
    }

    fn make_token_engine() -> crate::rules::token_engine::TokenRuleEngine {
        let mut engine = crate::rules::token_engine::TokenRuleEngine::new();
        engine.register(Box::new(
            crate::rules::token_rules::normalize::NormalizeEllipsis,
        ));
        engine.register(Box::new(
            crate::rules::token_rules::emphasis_ring::EmphasisRingRule,
        ));
        engine.register(Box::new(
            crate::rules::token_rules::latex_fraction::LatexFractionRule,
        ));
        engine.register(Box::new(
            crate::rules::token_rules::inline_fraction::InlineFractionRule,
        ));
        engine.register(Box::new(
            crate::rules::token_rules::word_shortcut::WordShortcutRule,
        ));
        engine.register(Box::new(
            crate::rules::token_rules::uppercase_passage::UppercasePassageRule,
        ));
        engine.register(Box::new(
            crate::rules::token_rules::middle_dot_spacing::MiddleDotSpacingRule,
        ));
        engine.register(Box::new(
            crate::rules::token_rules::quote_attachment::QuoteAttachmentRule,
        ));
        engine.register(Box::new(
            crate::rules::token_rules::spacing::AsteriskSpacingRule,
        ));
        engine
    }

    /// Helper: round-trip test via emit(parse(text)) == encode(text)
    fn assert_round_trip(text: &str) {
        let mut ir = DocumentIR::parse(text, english_indicator(text));
        let mut engine = make_char_engine();
        let mut token_engine = make_token_engine();
        let state_before_token_rules = ir.state.clone();
        token_engine
            .apply_all(&mut ir.tokens, &mut ir.state)
            .unwrap();
        ir.state = state_before_token_rules;
        let emitted = emit(&mut ir, &mut engine).unwrap();
        let expected = encode(text).unwrap();
        assert_eq!(
            emitted, expected,
            "round-trip mismatch for {:?}\n  emit:   {:?}\n  encode: {:?}",
            text, emitted, expected
        );
    }

    // ── Step 1-3: Basic token tests ──

    #[test]
    fn emit_round_trip_korean() {
        assert_round_trip("안녕하세요");
    }

    #[test]
    fn emit_round_trip_english_words() {
        assert_round_trip("hello world");
    }

    #[test]
    fn mode_events_emit_expected_bytes() {
        let mut ir = DocumentIR {
            tokens: vec![
                Token::Mode(ModeEvent::EnterEnglish),
                Token::Mode(ModeEvent::EnterEnglishContinue),
                Token::Mode(ModeEvent::CapsWord),
                Token::Mode(ModeEvent::CapsPassageStart),
                Token::Mode(ModeEvent::CapsPassageEnd),
            ],
            state: EncoderState::new(false),
        };
        let mut engine = make_char_engine();
        let out = emit(&mut ir, &mut engine).unwrap();
        assert_eq!(out, vec![52, 48, 32, 32, 32, 32, 32, 32, 4]);
    }

    #[test]
    fn fraction_token_encodes() {
        let mut ir = DocumentIR {
            tokens: vec![
                Token::Fraction(super::super::token::FractionToken {
                    whole: None,
                    numerator: "1".to_string(),
                    denominator: "2".to_string(),
                }),
                Token::Space(SpaceKind::Regular),
                Token::Fraction(super::super::token::FractionToken {
                    whole: Some("3".to_string()),
                    numerator: "1".to_string(),
                    denominator: "4".to_string(),
                }),
            ],
            state: EncoderState::new(false),
        };
        let mut engine = make_char_engine();
        let out = emit(&mut ir, &mut engine).unwrap();

        let mut expected = fraction::encode_fraction("1", "2").unwrap();
        expected.push(0);
        expected.extend(fraction::encode_mixed_fraction("3", "1", "4").unwrap());
        assert_eq!(out, expected);
    }

    #[test]
    fn extract_context_uses_prev_and_remaining_words() {
        let words = ["A", "B", "C"];
        let tokens = words
            .iter()
            .map(|w| {
                let chars: Vec<char> = w.chars().collect();
                Token::Word(WordToken {
                    text: Cow::Borrowed(w),
                    chars: chars.clone(),
                    meta: super::super::token::WordMeta::from_chars(&chars),
                })
            })
            .collect::<Vec<_>>();

        let target = match &tokens[1] {
            Token::Word(w) => w,
            _ => panic!("expected word"),
        };

        let (prev, rem) = extract_word_context(target, &tokens);
        assert_eq!(prev, "A");
        assert_eq!(rem, vec!["C"]);
    }

    // ── Post-loop parity tests ──

    #[test]
    fn emit_round_trip_triple_uppercase() {
        // 제28항 [붙임] 대문자 구절표
        assert_round_trip("WELCOME TO KOREA");
    }

    #[test]
    fn emit_round_trip_english_indicator_with_korean() {
        // 로마자표 + 종료표 tests
        assert_round_trip("SNS에서");
        assert_round_trip("ATM 기기");
        assert_round_trip("BMI(지수)");
    }

    #[test]
    fn emit_round_trip_mixed_uppercase_word() {
        assert_round_trip("ATM");
        assert_round_trip("Contents");
        assert_round_trip("Table of Contents");
    }

    #[test]
    fn emit_round_trip_numbers() {
        assert_round_trip("1,000");
        assert_round_trip("0.48");
    }

    #[test]
    fn emit_round_trip_multi_word_korean() {
        assert_round_trip("상상이상의 ");
    }

    #[test]
    fn emit_round_trip_korean_with_newline() {
        // parse() splits on spaces; newlines within words are handled by per-char
        assert_round_trip("안녕\n반가워");
    }

    #[test]
    fn emit_round_trip_word_shortcut() {
        // 제18항 약어 (그래서, 그러나, etc.)
        assert_round_trip("그래서");
        assert_round_trip("그러나");
    }

    #[test]
    fn emit_round_trip_latex_fraction() {
        assert_round_trip("$\\frac{1}{2}$");
    }

    #[test]
    fn emit_round_trip_math_symbols() {
        assert_round_trip("나루 + 배 = 나룻배");
    }

    #[test]
    fn emit_round_trip_phone_number() {
        assert_round_trip("02-2669-9775~6");
    }

    #[test]
    fn emit_round_trip_parenthesized_english() {
        assert_round_trip("지수(BMI)");
        assert_round_trip("체질량 지수(BMI)");
    }

    #[test]
    fn emit_round_trip_standalone_jamo() {
        assert_round_trip("삼각형 ㄱㄴㄷ");
    }

    #[test]
    fn emit_round_trip_kg_parenthesized() {
        assert_round_trip("(kg)");
        assert_round_trip("kg");
    }

    #[test]
    fn emit_round_trip_roma_bracket() {
        assert_round_trip("Roma [ㄹㄹ로마]");
    }
}
