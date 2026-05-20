//! 수학 제7항 — 분수선/슬래시 표기.
//!
//! 일반 나눗셈 슬래시와 분수 기호형 슬래시를 문맥으로 구분한다.

use crate::rules::math::parser::{BracketKind, MathToken};

use super::math_token_rule::{MathEncodeState, MathTokenEngine, MathTokenResult, MathTokenRule};
use super::{rule_1, rule_6, rule_12};

/// 분수 기호형 슬래시(_/)를 써야 하는 문맥인지 판별한다.
///
/// PDF 수학 제7항: 숫자 분자/분모로 구성된 N/M 분수는 분수 기호형(_/)로 적는다.
/// 알파벳 대문자(예: A/B) 분수도 동일.
pub fn slash_as_fraction_symbol(tokens: &[MathToken], i: usize) -> bool {
    let left = tokens.get(i.saturating_sub(1));
    let right = tokens.get(i + 1);

    matches!(
        (left, right),
        (
            Some(MathToken::UpperVariable(_)),
            Some(MathToken::UpperVariable(_))
        )
    ) || matches!(
        (left, right),
        (Some(MathToken::Number(l)), Some(MathToken::Number(r)))
            if l.chars().all(|c| c.is_ascii_digit())
                && r.chars().all(|c| c.is_ascii_digit())
    )
}

pub struct FractionReversalRule;

pub struct GroupedFractionReversalRule;

impl MathTokenRule for GroupedFractionReversalRule {
    fn name(&self) -> &'static str {
        "GroupedFractionReversalRule"
    }

    fn priority(&self) -> u16 {
        10
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::OpenParen(_)))
            && rule_6::find_matching_paren(tokens, index).is_some_and(|close| {
                matches!(
                    tokens.get(close + 1),
                    Some(MathToken::Operator('/') | MathToken::MathSymbol('\u{2044}'))
                )
            })
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        let Some(left_close) = rule_6::find_matching_paren(tokens, index) else {
            return Ok(MathTokenResult::Skip);
        };
        if !matches!(
            tokens.get(left_close + 1),
            Some(MathToken::Operator('/') | MathToken::MathSymbol('\u{2044}'))
        ) {
            return Ok(MathTokenResult::Skip);
        }
        let right_start = left_close + 2;

        // PDF 제7항 3: strip_latex_to_math이 (분모)/분자 형태로 출력한다.
        // 왼쪽(분모)을 묶음 괄호(Grouping)로 감싸서 먼저 출력하고,
        // 분수선 후 오른쪽(분자)을 출력한다.

        // 오른쪽(분자)이 괄호로 감싸진 경우: (분모)/(분자) 패턴
        if matches!(tokens.get(right_start), Some(MathToken::OpenParen(_))) {
            let Some(right_close) = rule_6::find_matching_paren(tokens, right_start) else {
                return Ok(MathTokenResult::Skip);
            };

            // 분모(왼쪽)를 묶음 괄호로 감싸서 먼저 출력
            rule_6::encode_open_paren(BracketKind::Grouping, result);
            engine.encode_tokens(&tokens[index + 1..left_close], result)?;
            rule_6::encode_close_paren(BracketKind::Grouping, result);
            result.push(12);
            // 분자(오른쪽) 출력 (괄호 포함)
            engine.encode_tokens(&tokens[right_start..=right_close], result)?;
            state.prev_was_number = false;
            return Ok(MathTokenResult::Consumed(right_close + 1 - index));
        }

        // 오른쪽(분자)이 단순 토큰인 경우: (분모)/단순식 패턴
        let right_end = find_simple_right_end(tokens, right_start);
        if right_end == right_start {
            return Ok(MathTokenResult::Skip);
        }

        // 분모(왼쪽)를 묶음 괄호로 감싸서 먼저 출력
        rule_6::encode_open_paren(BracketKind::Grouping, result);
        engine.encode_tokens(&tokens[index + 1..left_close], result)?;
        rule_6::encode_close_paren(BracketKind::Grouping, result);
        result.push(12);
        // 분자(오른쪽) 출력
        engine.encode_tokens(&tokens[right_start..right_end], result)?;
        state.prev_was_number = false;
        Ok(MathTokenResult::Consumed(right_end - index))
    }
}

/// 단순 오른쪽 피연산자의 끝 인덱스를 반환한다.
///
/// 단순 피연산자: 숫자, 변수, 첨자 등 단일 토큰 또는 연속된 단순 토큰.
/// 연산자(+, -, ×, ÷)나 괄호가 나오면 멈춘다.
fn find_simple_right_end(tokens: &[MathToken], start: usize) -> usize {
    let mut i = start;
    while i < tokens.len() {
        match &tokens[i] {
            MathToken::Number(_)
            | MathToken::Variable(_)
            | MathToken::UpperVariable(_)
            | MathToken::Superscript(_)
            | MathToken::Subscript(_)
            | MathToken::Prime => {
                i += 1;
            }
            _ => break,
        }
    }
    i
}

impl MathTokenRule for FractionReversalRule {
    fn name(&self) -> &'static str {
        "FractionReversalRule"
    }

    fn priority(&self) -> u16 {
        10
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::Number(_)))
            && matches!(tokens.get(index + 1), Some(MathToken::Operator('/')))
            && matches!(tokens.get(index + 2), Some(MathToken::Number(_)))
            && !slash_as_fraction_symbol(tokens, index + 1)
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        _engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        let (Some(MathToken::Number(left)), Some(MathToken::Number(right))) =
            (tokens.get(index), tokens.get(index + 2))
        else {
            return Ok(MathTokenResult::Skip);
        };

        rule_1::encode_number_with_prefix(right, false, result);
        result.push(12);
        rule_1::encode_number_with_prefix(left, false, result);
        state.prev_was_number = false;
        Ok(MathTokenResult::Consumed(3))
    }
}

pub struct ConditionalProbFractionRule;

impl MathTokenRule for ConditionalProbFractionRule {
    fn name(&self) -> &'static str {
        "ConditionalProbFractionRule"
    }

    fn priority(&self) -> u16 {
        10
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::Number(_)))
            && matches!(tokens.get(index + 1), Some(MathToken::Operator('/')))
            && matches!(tokens.get(index + 2), Some(MathToken::Number(_)))
            && matches!(
                rule_12::prev_non_space(tokens, index),
                Some(MathToken::Operator('='))
            )
            && tokens
                .iter()
                .any(|token| matches!(token, MathToken::MathSymbol('|')))
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        _engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        let (Some(MathToken::Number(left)), Some(MathToken::Number(right))) =
            (tokens.get(index), tokens.get(index + 2))
        else {
            return Ok(MathTokenResult::Skip);
        };

        rule_1::encode_number_literal(right, result);
        result.push(12);
        rule_1::encode_number_literal(left, result);
        state.prev_was_number = false;
        Ok(MathTokenResult::Consumed(3))
    }
}
