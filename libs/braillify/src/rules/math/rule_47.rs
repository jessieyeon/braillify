//! 수학 제47항 — 로그와 극한 표기.
//!
//! log의 밑 첨자, lim의 아래첨자(수렴 대상) 인코딩을 처리한다.

use crate::rules::math::parser::{BracketKind, MathToken};

use super::function;
use super::math_token_rule::{MathEncodeState, MathTokenEngine, MathTokenResult, MathTokenRule};
use super::{rule_6, rule_46};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogBaseKind {
    None,
    Digit,
    Variable,
    Complex,
}

fn is_single_digit_base(content: &[MathToken]) -> Option<char> {
    match content {
        [MathToken::Number(n)] if n.len() == 1 => n.chars().next(),
        _ => None,
    }
}

fn is_single_variable_base(content: &[MathToken]) -> Option<char> {
    match content {
        [MathToken::Variable(c)] => Some(*c),
        [MathToken::UpperVariable(c)] => Some(c.to_ascii_lowercase()),
        _ => None,
    }
}

fn encode_log_base_digit(digit: char) -> Option<u8> {
    Some(match digit {
        '1' => 3,
        '2' => 6,
        '3' => 18,
        '4' => 50,
        '5' => 34,
        '6' => 22,
        '7' => 55,
        '8' => 41,
        '9' => 10,
        '0' => 26,
        _ => return None,
    })
}

fn encode_log_base(
    content: &[MathToken],
    result: &mut Vec<u8>,
    engine: &MathTokenEngine,
) -> Result<LogBaseKind, String> {
    if let Some(digit) = is_single_digit_base(content) {
        result.push(32);
        result.push(encode_log_base_digit(digit).ok_or("Invalid log base digit")?);
        return Ok(LogBaseKind::Digit);
    }

    if let Some(var) = is_single_variable_base(content) {
        result.push(48);
        result.push(crate::english::encode_english(var)?);
        return Ok(LogBaseKind::Variable);
    }

    let base_content = if content.len() >= 2
        && matches!(
            content.first(),
            Some(MathToken::OpenParen(BracketKind::MathParen))
        )
        && matches!(
            content.last(),
            Some(MathToken::CloseParen(BracketKind::MathParen))
        ) {
        &content[1..content.len() - 1]
    } else {
        content
    };

    result.push(48);
    result.push(55);
    let normalized_base: Vec<MathToken> = base_content
        .iter()
        .map(|t| match t {
            MathToken::Operator('/') => MathToken::MathSymbol('\u{2044}'),
            other => other.clone(),
        })
        .collect();
    engine.encode_tokens(&normalized_base, result)?;
    result.push(62);
    Ok(LogBaseKind::Complex)
}

pub fn encode_log_token(
    tokens: &[MathToken],
    i: &mut usize,
    result: &mut Vec<u8>,
    engine: &MathTokenEngine,
) -> Result<(), String> {
    result.push(56);
    *i += 1;

    let mut base_kind = LogBaseKind::None;
    if let Some(MathToken::Subscript(content)) = tokens.get(*i) {
        base_kind = encode_log_base(content, result, engine)?;
        *i += 1;
    }

    if *i >= tokens.len() {
        return Ok(());
    }

    if let Some(MathToken::OpenParen(_)) = tokens.get(*i) {
        let Some(close_idx) = rule_6::find_matching_paren(tokens, *i) else {
            return Err("Unmatched parenthesis in log argument".to_string());
        };

        if base_kind == LogBaseKind::None {
            // PDF 수학 제46항/47항 — 진수가 다항식인 log(x+...)는 ⠦...⠴ (math paren).
            result.push(38); // ⠦
            engine.encode_tokens(&tokens[*i + 1..close_idx], result)?;
            result.push(52); // ⠴
        } else if base_kind == LogBaseKind::Digit {
            result.push(55);
            engine.encode_tokens(&tokens[*i..=close_idx], result)?;
            result.push(62);
        } else if base_kind == LogBaseKind::Variable {
            let inner = &tokens[*i + 1..close_idx];
            let needs_normalized_group = inner
                .iter()
                .any(|t| matches!(t, MathToken::UpperVariable(_) | MathToken::Operator('/')));

            if needs_normalized_group {
                let normalized_arg: Vec<MathToken> = inner
                    .iter()
                    .map(|t| match t {
                        MathToken::UpperVariable(c) => MathToken::Variable(c.to_ascii_lowercase()),
                        MathToken::Operator('/') => MathToken::MathSymbol('\u{2044}'),
                        other => other.clone(),
                    })
                    .collect();
                result.push(55);
                engine.encode_tokens(&normalized_arg, result)?;
                result.push(62);
            } else {
                engine.encode_tokens(&tokens[*i..=close_idx], result)?;
            }
        } else {
            engine.encode_tokens(&tokens[*i..=close_idx], result)?;
        }

        *i = close_idx + 1;
        return Ok(());
    }

    // PDF 수학 제47항 — log 인수가 분수(괄호 없는 V/V 또는 N/N 등)일 때는 ⠷...⠾로 묶는다.
    if matches!(
        tokens.get(*i),
        Some(MathToken::Number(_) | MathToken::Variable(_))
    ) && matches!(
        tokens.get(*i + 1),
        Some(MathToken::Operator('/') | MathToken::MathSymbol('\u{2044}'))
    ) && matches!(
        tokens.get(*i + 2),
        Some(MathToken::Number(_) | MathToken::Variable(_))
    ) {
        result.push(55); // ⠷
        engine.encode_tokens(&tokens[*i..*i + 3], result)?;
        result.push(62); // ⠾
        *i += 3;
        return Ok(());
    }

    if let Some(arg) = tokens.get(*i) {
        if base_kind == LogBaseKind::Complex && matches!(arg, MathToken::Variable(_)) {
            result.push(32);
        }
        engine.encode_tokens(std::slice::from_ref(arg), result)?;
        *i += 1;
    }
    Ok(())
}

pub fn encode_lim_token(
    tokens: &[MathToken],
    i: &mut usize,
    result: &mut Vec<u8>,
    engine: &MathTokenEngine,
) -> Result<(), String> {
    fn encode_lim_target(
        content: &[MathToken],
        result: &mut Vec<u8>,
        engine: &MathTokenEngine,
    ) -> Result<(), String> {
        if let Some(arrow_idx) = content.iter().position(|t| {
            matches!(
                t,
                MathToken::MathSymbol('\u{2192}') | MathToken::MathSymbol('\u{21D2}')
            )
        }) {
            engine.encode_tokens(&content[..arrow_idx], result)?;
            result.push(0);
            engine.encode_tokens(&content[arrow_idx..arrow_idx + 1], result)?;
            result.push(0);
            engine.encode_tokens(&content[arrow_idx + 1..], result)?;
            return Ok(());
        }
        engine.encode_tokens(content, result)
    }

    result.push(crate::english::encode_english('l')?);
    result.push(crate::english::encode_english('i')?);
    result.push(crate::english::encode_english('m')?);
    *i += 1;

    if let Some(MathToken::Subscript(content)) = tokens.get(*i) {
        result.push(48);
        encode_lim_target(content, result, engine)?;
        *i += 1;
        // PDF 수학 제51항 — lim의 첨자 뒤에 다음 식이 이어지면 한 칸 띄움.
        // (LaTeX strip이 공백을 제거하므로 명시적으로 삽입한다.)
        if next_is_lim_body(tokens, *i) {
            result.push(0);
        }
        return Ok(());
    }

    if let Some(MathToken::OpenParen(_)) = tokens.get(*i) {
        let Some(close_idx) = rule_6::find_matching_paren(tokens, *i) else {
            return Err("Unmatched parenthesis in lim argument".to_string());
        };
        result.push(48);
        encode_lim_target(&tokens[*i + 1..close_idx], result, engine)?;
        *i = close_idx + 1;
    }

    Ok(())
}

/// lim 첨자 직후가 함수 본문(변수/괄호/숫자 등)이면 한 칸 띄움이 필요하다.
fn next_is_lim_body(tokens: &[MathToken], idx: usize) -> bool {
    let mut cursor = idx;
    // 이미 공백 토큰이 있으면 별도 삽입 불필요.
    if matches!(tokens.get(cursor), Some(MathToken::Space)) {
        return false;
    }
    while cursor < tokens.len() {
        match &tokens[cursor] {
            MathToken::Space => return false,
            MathToken::Variable(_)
            | MathToken::UpperVariable(_)
            | MathToken::Number(_)
            | MathToken::OpenParen(_)
            | MathToken::FunctionName(_)
            | MathToken::MathSymbol(_)
            | MathToken::Superscript(_) => return true,
            _ => cursor += 1,
        }
    }
    false
}

pub struct FunctionNameRule;

impl MathTokenRule for FunctionNameRule {
    fn name(&self) -> &'static str {
        "FunctionNameRule"
    }

    fn priority(&self) -> u16 {
        50
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::FunctionName(_)))
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        let Some(MathToken::FunctionName(name)) = tokens.get(index) else {
            return Ok(MathTokenResult::Skip);
        };

        let mut cursor = index;
        if name == "log" {
            encode_log_token(tokens, &mut cursor, result, engine)?;
            state.prev_was_number = false;
            return Ok(MathTokenResult::Consumed(cursor - index));
        }

        if name == "lim" {
            encode_lim_token(tokens, &mut cursor, result, engine)?;
            state.prev_was_number = false;
            return Ok(MathTokenResult::Consumed(cursor - index));
        }

        if rule_46::encode_trig_function(
            name,
            tokens,
            &mut cursor,
            result,
            rule_6::find_matching_paren,
        )? {
            state.prev_was_number = false;
            return Ok(MathTokenResult::Consumed(cursor - index));
        }

        if let Some(encoded) = function::encode_function(name) {
            result.extend_from_slice(encoded);
        } else {
            for ch in name.chars() {
                if let Ok(code) = crate::english::encode_english(ch) {
                    result.push(code);
                }
            }
        }
        state.prev_was_number = false;
        Ok(MathTokenResult::Consumed(1))
    }
}
