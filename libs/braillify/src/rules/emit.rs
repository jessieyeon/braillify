use crate::char_struct::{CharType, KoreanChar};
use crate::english_logic;
use crate::fraction;
use crate::rules::context::{EncoderState, RuleContext};
use crate::rules::engine::RuleEngine;
use crate::rules::korean::rule_69::parse_numeric_ascii_unit_prefix;
use crate::rules::traits::Phase;

use super::token::{DocumentIR, ModeEvent, SpaceKind, Token, WordToken};

/// 제39항 한글표 점형 (⠸⠷). 영어 어절 사이에 끼인 한글 어절을 감싼다.
pub(crate) const HANGUL_WRAP_START_BYTES: [u8; 2] = [56, 55];
/// 제39항 한글 종료표 점형 (⠸⠾).
pub(crate) const HANGUL_WRAP_END_BYTES: [u8; 2] = [56, 62];

struct WordContext<'a> {
    prev_word: &'a str,
    remaining_words: &'a [&'a str],
}

/// 토큰의 byte 슬라이스가 한글표(⠸⠷) 점형과 일치하는지.
fn is_hangul_wrap_start(token: &Token<'_>) -> bool {
    matches!(token, Token::PreEncoded(bytes) if bytes.as_slice() == HANGUL_WRAP_START_BYTES)
}

/// 토큰의 byte 슬라이스가 한글 종료표(⠸⠾) 점형과 일치하는지.
fn is_hangul_wrap_end(token: &Token<'_>) -> bool {
    matches!(token, Token::PreEncoded(bytes) if bytes.as_slice() == HANGUL_WRAP_END_BYTES)
}

/// 어떤 토큰 직후, 공백/PreEncoded(non-wrap)을 건너뛰고 만나는 첫 토큰이
/// 한글표 시작이면 true. 한글 wrap이 영어 모드 유지를 위한 신호이므로,
/// 단어 끝의 종료표 emit을 건너뛰는 데 사용된다.
fn next_non_space_is_hangul_wrap_start<'a>(tokens: &'a [Token<'a>], after_index: usize) -> bool {
    for token in tokens.iter().skip(after_index + 1) {
        match token {
            Token::Space(_) => continue,
            t => return is_hangul_wrap_start(t),
        }
    }
    false
}

/// 어떤 토큰 직전에, 공백을 건너뛰고 만나는 첫 비공백 토큰이 한글 종료표면 true.
/// 한글 wrap 종료 후 영어 컨텍스트가 자동 재개되는 점을 알리는 데 사용한다.
fn prev_non_space_is_hangul_wrap_end<'a>(tokens: &'a [Token<'a>], before_index: usize) -> bool {
    for token in tokens[..before_index].iter().rev() {
        match token {
            Token::Space(_) => continue,
            t => return is_hangul_wrap_end(t),
        }
    }
    false
}

/// PDF 수학 — `Word(math)+Space+Word(=/==/관계)+Space+Word(math)` 패턴에서
/// 등호 양옆 Space 토큰을 묵음 처리한다. 점역 결과는 `expr⠒⠒expr`로 인접한다.
fn is_math_operator_space_suppression<'a>(tokens: &'a [Token<'a>], space_idx: usize) -> bool {
    fn token_is_math_word(token: Option<&Token<'_>>) -> bool {
        match token {
            Some(Token::Word(w)) => {
                if w.meta.has_korean {
                    return false;
                }
                w.chars.iter().any(|c| {
                    c.is_ascii_alphabetic()
                        || matches!(*c,
                            '\u{2080}'..='\u{2089}'
                            | '\u{00B2}' | '\u{00B3}'
                            | '\u{2070}'..='\u{2079}'
                            | '∇' | '∂' | '∞' | '∫'
                            | 'α'..='ω' | 'Α'..='Ω'
                        )
                }) || w.chars.contains(&'(')
                    || w.chars.contains(&')')
                    || w.chars.contains(&'/')
            }
            Some(Token::PreEncoded(_)) => true,
            _ => false,
        }
    }
    fn token_is_relation_operator_word(token: Option<&Token<'_>>) -> bool {
        match token {
            Some(Token::Word(w)) => {
                w.chars.len() <= 2
                    && w.chars.iter().all(|c| {
                        matches!(*c, '=' | '<' | '>' | '\u{2260}' | '\u{2264}' | '\u{2265}')
                    })
            }
            // PDF — MathExpressionTokenRule이 관계연산자 Word를 PreEncoded로 변환한 결과.
            // 등호/부등호/관계기호의 점역 결과는 다음과 같다 (소스: rule_3, rule_4, math_symbol_shortcut).
            // 셀 시퀀스가 정확히 일치하면 관계연산자로 본다.
            // 향후 Token 메타데이터로 의미를 보존하는 방향이 더 안전하지만, 현 구조에서는
            // 점형이 짧고 충돌 가능성이 낮은 셀들만 골라 매칭한다.
            Some(Token::PreEncoded(bytes)) => matches!(
                bytes.as_slice(),
                [18, 18]                  // ⠒⠒ : =
                | [40, 18, 18]            // ⠨⠒⠒ : ≠
                | [16, 16]                // ⠐⠐ : ≤  
                | [16, 18]                // ⠐⠒ : <
                | [18, 16] // ⠒⠐ : >
            ),
            _ => false,
        }
    }
    // 케이스 1: Space 다음이 관계 연산자 Word, 이전이 math Word/PreEncoded.
    if space_idx + 1 < tokens.len()
        && token_is_relation_operator_word(tokens.get(space_idx + 1))
        && space_idx > 0
        && token_is_math_word(tokens.get(space_idx - 1))
    {
        return true;
    }
    // 케이스 2: Space 이전이 관계 연산자 Word, 다음이 math Word/PreEncoded.
    if space_idx > 0
        && token_is_relation_operator_word(tokens.get(space_idx - 1))
        && space_idx + 1 < tokens.len()
        && token_is_math_word(tokens.get(space_idx + 1))
    {
        return true;
    }
    false
}

pub fn emit(ir: &mut DocumentIR, char_engine: &mut RuleEngine) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    let word_texts = if ir.tokens.len() > 1 {
        collect_word_texts(&ir.tokens)
    } else {
        Vec::new()
    };
    let mut word_index = 0usize;

    for (idx, token) in ir.tokens.iter().enumerate() {
        match token {
            Token::Word(word) => {
                let context = if word_texts.is_empty() {
                    WordContext {
                        prev_word: "",
                        remaining_words: &[],
                    }
                } else {
                    word_context(&word_texts, word_index)
                };
                emit_word(
                    word,
                    idx,
                    &mut ir.state,
                    char_engine,
                    &ir.tokens,
                    context,
                    &mut result,
                )?;
                word_index += 1;
            }
            Token::Space(SpaceKind::Regular) => {
                if !is_math_operator_space_suppression(&ir.tokens, idx) {
                    result.push(0);
                }
            }
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
            Token::PreEncoded(bytes) => {
                // 제39항 한글 wrap 점형은 영어 모드를 자동으로 휴면(⠸⠷)·재개(⠸⠾)시킨다.
                // 이렇게 하면 wrap 사이의 한글 어절은 한국어 인코더로 처리되고,
                // wrap 종료 후 이어지는 영어 어절은 영자표시(⠴) 없이 모드를 이어간다.
                if bytes.as_slice() == HANGUL_WRAP_START_BYTES {
                    ir.state.is_english = false;
                    ir.state.needs_english_continuation = false;
                    ir.state.roman_number_chain = false;
                } else if bytes.as_slice() == HANGUL_WRAP_END_BYTES {
                    ir.state.is_english = true;
                    ir.state.needs_english_continuation = false;
                }
                result.extend(bytes);
            }
        }
    }

    // End-of-stream: close triple uppercase if active (Encoder::finish)
    if ir.state.triple_big_english {
        result.push(32);
        result.push(4);
    }

    Ok(result)
}

fn collect_word_texts<'tokens, 'source>(tokens: &'tokens [Token<'source>]) -> Vec<&'tokens str> {
    let mut word_texts = Vec::with_capacity(tokens.len().div_ceil(2));

    for token in tokens {
        if let Token::Word(word) = token {
            word_texts.push(word.text.as_ref());
        }
    }

    word_texts
}

fn word_context<'a>(word_texts: &'a [&'a str], word_index: usize) -> WordContext<'a> {
    let prev_word = word_index
        .checked_sub(1)
        .map_or("", |prev_index| word_texts[prev_index]);
    let remaining_words = &word_texts[word_index + 1..];

    WordContext {
        prev_word,
        remaining_words,
    }
}

fn emit_mode_event(event: ModeEvent, state: &mut EncoderState, result: &mut Vec<u8>) {
    match event {
        ModeEvent::EnterEnglish => {
            result.push(52);
            state.is_english = true;
            state.needs_english_continuation = false;
            state.roman_number_chain = false;
        }
        ModeEvent::EnterEnglishContinue => {
            result.push(48);
            state.is_english = true;
            state.needs_english_continuation = false;
            state.roman_number_chain = false;
        }
        ModeEvent::CapsWord => {
            result.push(32);
            result.push(32);
        }
        ModeEvent::Grade1Indicator => {
            // ⠰ (dots 5+6, byte 48): UEB Grade-1 indicator that forces literal letter
            // reading and prevents shortform/contraction collision (UEB 5.7.2 + 10.9).
            result.push(48);
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
    state.roman_number_chain = false;
}

fn enter_english(state: &mut EncoderState, result: &mut Vec<u8>) {
    if state.needs_english_continuation {
        result.push(48);
    } else {
        result.push(52);
    }
    state.is_english = true;
    state.needs_english_continuation = false;
    state.roman_number_chain = false;
}

fn exit_english_for_roman_number_chain(state: &mut EncoderState) {
    exit_english(state, false);
    state.roman_number_chain = true;
}

fn resume_english_from_roman_number_chain(state: &mut EncoderState) {
    state.is_english = true;
    state.needs_english_continuation = false;
    state.roman_number_chain = false;
}

fn emit_word(
    word: &WordToken,
    token_index: usize,
    state: &mut EncoderState,
    char_engine: &mut RuleEngine,
    all_tokens: &[Token],
    context: WordContext<'_>,
    result: &mut Vec<u8>,
) -> Result<(), String> {
    let prev_word = context.prev_word;
    let remaining_words = context.remaining_words;
    // 다음 비공백 토큰이 한글표(⠸⠷)이면 영어 모드를 끊지 않는다 (제39항).
    let next_is_hangul_wrap = next_non_space_is_hangul_wrap_start(all_tokens, token_index);
    // 직전 비공백 토큰이 한글 종료표(⠸⠾)이면 이 토큰의 시작 문장부호도
    // 영어 컨텍스트의 일부로 본다 (제39항 wrap 재개 직후).
    let prev_is_hangul_wrap_end = prev_non_space_is_hangul_wrap_end(all_tokens, token_index);

    // ── [D] Per-character loop (encoder.rs:201-409) ──
    let word_chars = word.chars.as_slice();
    let word_len = word_chars.len();

    if word_len > 0 {
        let meta = word.meta;
        let is_all_uppercase = meta.is_all_uppercase;
        let has_korean_char = meta.has_korean;
        let has_ascii_alphabetic = meta.has_ascii_alphabetic;

        if word_chars.first().is_some_and(|ch| ch.is_ascii_digit())
            && let Some((numeric, unit, consumed)) = parse_numeric_ascii_unit_prefix(word_chars)
            && consumed == word_chars.len()
        {
            let mut encoded = crate::encode(&numeric)?;
            encoded.extend(unit);
            result.extend(encoded);
            return Ok(());
        }

        // English entry (encoder.rs:216-223)
        if state.english_indicator
            && !state.is_english
            && has_ascii_alphabetic
            && word_chars[0].is_ascii_alphabetic()
        {
            if state.roman_number_chain {
                resume_english_from_roman_number_chain(state);
            } else if state.english_dominant_no_indicator {
                // 영어 주도 문서: 영자표시 ⠴ 생략, state만 영어 모드로 전환.
                state.is_english = true;
                state.needs_english_continuation = false;
                state.roman_number_chain = false;
            } else {
                enter_english(state, result);
            }
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
                        exit_english_for_roman_number_chain(state);
                    }
                    CharType::Symbol(sym) => {
                        // 한글 wrap 직후의 첫 디지털 표기 기호(. / @ # _ : -)는
                        // 영어 컨텍스트의 연속으로 본다. 예) "www.대통령.kr"에서
                        // wrap 종료 직후의 '.'는 ".kr" 영어 도메인 일부.
                        let prev_wrap_eng_continuation = i == 0
                            && prev_is_hangul_wrap_end
                            && matches!(*sym, '.' | '/' | '@' | '#' | '_' | ':' | '-')
                            && english_logic::next_ascii_letter_or_digit(
                                word_chars,
                                i,
                                remaining_words,
                            );

                        // 단어 끝의 영어 모드 유지 가능 기호(. , : ;) 직후 한글표(⠸⠷)가
                        // 이어지면, 그 기호도 영어 컨텍스트의 연속으로 본다 (제39항 wrap
                        // 직전). 예) "(Korean:" 끝의 ':'은 다음 wrap된 한글에 이어지므로
                        // 영어 점자(⠒)로 처리.
                        let next_wrap_eng_continuation = i == word_chars.len() - 1
                            && next_is_hangul_wrap
                            && matches!(*sym, '.' | ',' | ':' | ';');

                        if prev_wrap_eng_continuation
                            || next_wrap_eng_continuation
                            || english_logic::should_render_symbol_as_english(
                                state.english_indicator,
                                state.is_english,
                                &state.parenthesis_stack,
                                *sym,
                                word_chars,
                                i,
                                remaining_words,
                            )
                            || english_logic::should_keep_english_mode_for_symbol(
                                *sym,
                                word_chars,
                                i,
                                remaining_words,
                            )
                        {
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
            if state.roman_number_chain && !state.is_english {
                match &char_type {
                    CharType::English(_) => {
                        // PDF — roman_number_chain 안 digit 뒤 letter는 영어 연속 표지(⠰)를
                        // 부착해 letter임을 명시한다 (digit과 혼동 방지).
                        result.push(48);
                        resume_english_from_roman_number_chain(state);
                    }
                    CharType::Number(_) => {}
                    _ => {
                        state.roman_number_chain = false;
                    }
                }
            }

            match &char_type {
                CharType::Korean(_) | CharType::KoreanPart(_) => {
                    state.needs_english_continuation = false;
                }
                CharType::Number(_) => {}
                _ => {}
            }

            // CoreEncoding via engine (encoder.rs:330-360)
            state.is_number = is_number;
            state.is_big_english = is_big_english;
            apply_core_encoding_rules(
                char_engine,
                &char_type,
                word_chars,
                i,
                is_all_uppercase,
                has_korean_char,
                ascii_starts_at_beginning,
                state,
                &mut skip_count,
                remaining_words,
                prev_word,
                result,
            )?;
            is_number = state.is_number;
            is_big_english = state.is_big_english;

            // InterCharacter via engine (encoder.rs:362-402)
            if let CharType::Korean(ref korean) = char_type
                && i < word_len - 1
            {
                let recon_type = CharType::Korean(KoreanChar {
                    cho: korean.cho,
                    jung: korean.jung,
                    jong: korean.jong,
                });
                state.is_number = is_number;
                state.is_big_english = is_big_english;
                apply_inter_character_rules(
                    char_engine,
                    &recon_type,
                    word_chars,
                    i,
                    is_all_uppercase,
                    has_korean_char,
                    ascii_starts_at_beginning,
                    state,
                    &mut skip_count,
                    remaining_words,
                    prev_word,
                    result,
                )?;
                is_number = state.is_number;
                is_big_english = state.is_big_english;
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
    // 제39항: 다음 토큰이 한글표(⠸⠷)이면 영어 모드를 끊지 않는다.
    // 한글표 emit 시점에 영어 모드가 자동 휴면되고, 한글 종료표(⠸⠾)에서 재개된다.
    if state.english_indicator && state.is_english && next_is_hangul_wrap {
        // 한글 wrap이 영어 모드 전환을 책임지므로 여기서는 아무 것도 emit하지 않는다.
    } else if state.english_dominant_no_indicator && state.english_indicator && state.is_english {
        // 영어 주도 문서: 영어 단어 사이의 종료표 ⠲ 모두 생략하고 영어 모드를 유지.
    } else if state.english_indicator && state.is_english {
        if remaining_words.is_empty() {
            result.push(50);
            exit_english(state, false);
        } else if let Some(next_word) = remaining_words.first() {
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
                                exit_english(
                                    state,
                                    english_logic::should_request_continuation(sym),
                                );
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
                Token::Mode(ModeEvent::Grade1Indicator),
            ],
            state: EncoderState::new(false),
        };
        let mut engine = make_char_engine();
        let out = emit(&mut ir, &mut engine).unwrap();
        assert_eq!(out, vec![52, 48, 32, 32, 32, 32, 32, 32, 4, 48]);
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

        let word_texts = collect_word_texts(&tokens);
        let context = word_context(&word_texts, 1);
        assert_eq!(context.prev_word, "A");
        assert_eq!(context.remaining_words, ["C"]);
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
