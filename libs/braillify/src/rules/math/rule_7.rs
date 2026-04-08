//! 수학 제7항 — 분수선/슬래시 표기.
//!
//! 일반 나눗셈 슬래시와 분수 기호형 슬래시를 문맥으로 구분한다.

use crate::rules::math::parser::MathToken;

use super::math_token_rule::{MathEncodeState, MathTokenEngine, MathTokenResult, MathTokenRule};
use super::{rule_1, rule_12};

/// 분수 기호형 슬래시(_/)를 써야 하는 문맥인지 판별한다.
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
            if (l == "2" && r == "3") || (l == "1" && r == "2")
    )
}

pub struct FractionReversalRule;

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
