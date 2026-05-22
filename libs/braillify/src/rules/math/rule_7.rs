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

    matches!((left, right), (Some(MathToken::UpperVariable(_)), Some(MathToken::UpperVariable(_))))
        || matches!(
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
        matches!(tokens.get(index), Some(MathToken::OpenParen(_))) && rule_6::find_matching_paren(tokens, index).is_some_and(|close| matches!(tokens.get(close + 1), Some(MathToken::Operator('/') | MathToken::MathSymbol('\u{2044}'))))
    }

    fn apply(&self, tokens: &[MathToken], index: usize, result: &mut Vec<u8>, state: &mut MathEncodeState, engine: &MathTokenEngine) -> Result<MathTokenResult, String> {
        let Some(left_close) = rule_6::find_matching_paren(tokens, index) else {
            return Ok(MathTokenResult::Skip);
        };
        if !matches!(tokens.get(left_close + 1), Some(MathToken::Operator('/') | MathToken::MathSymbol('\u{2044}'))) {
            return Ok(MathTokenResult::Skip);
        }
        let right_start = left_close + 2;

        // PDF 제7항 3: strip_latex_to_math이 (분모)/분자 형태로 출력한다.
        // 왼쪽(분모)을 묶음 괄호(Grouping)로 감싸서 먼저 출력하고,
        // 분수선 후 오른쪽(분자)을 출력한다.

        // 분모 측 OpenParen의 BracketKind를 보존한다 (Grouping/Hangul 구분).
        let left_kind = match tokens.get(index) {
            Some(MathToken::OpenParen(k)) => *k,
            _ => BracketKind::Grouping,
        };

        // 오른쪽(분자)이 괄호로 감싸진 경우: (분모)/(분자) 패턴
        if matches!(tokens.get(right_start), Some(MathToken::OpenParen(_))) {
            let Some(right_close) = rule_6::find_matching_paren(tokens, right_start) else {
                return Ok(MathTokenResult::Skip);
            };

            // 분모(왼쪽)를 원본 BracketKind로 감싸서 먼저 출력
            rule_6::encode_open_paren(left_kind, result);
            engine.encode_tokens(&tokens[index + 1..left_close], result)?;
            rule_6::encode_close_paren(left_kind, result);
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

        // 분모(왼쪽)를 원본 BracketKind로 감싸서 먼저 출력
        rule_6::encode_open_paren(left_kind, result);
        engine.encode_tokens(&tokens[index + 1..left_close], result)?;
        rule_6::encode_close_paren(left_kind, result);
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
            MathToken::Number(_) | MathToken::Variable(_) | MathToken::UpperVariable(_) | MathToken::Superscript(_) | MathToken::Subscript(_) | MathToken::Prime => {
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
        matches!(tokens.get(index), Some(MathToken::Number(_))) && matches!(tokens.get(index + 1), Some(MathToken::Operator('/'))) && matches!(tokens.get(index + 2), Some(MathToken::Number(_))) && !slash_as_fraction_symbol(tokens, index + 1)
    }

    fn apply(&self, tokens: &[MathToken], index: usize, result: &mut Vec<u8>, state: &mut MathEncodeState, _engine: &MathTokenEngine) -> Result<MathTokenResult, String> {
        let (Some(MathToken::Number(left)), Some(MathToken::Number(right))) = (tokens.get(index), tokens.get(index + 2)) else {
            return Ok(MathTokenResult::Skip);
        };

        rule_1::encode_number_with_prefix(right, false, result);
        result.push(12);
        rule_1::encode_number_with_prefix(left, false, result);
        state.prev_was_number = false;
        Ok(MathTokenResult::Consumed(3))
    }
}

/// PDF — `(f/x₁, f/x₂, ..., f/xₙ)` 같이 paren 안 comma-구분 fraction은 reverse.
/// `f/x` → `x/f` (분모 먼저). 안전을 위해 prev가 OpenParen 또는 comma일 때만 발동.
pub struct VariableFractionInListRule;

impl MathTokenRule for VariableFractionInListRule {
    fn name(&self) -> &'static str {
        "VariableFractionInListRule"
    }

    fn priority(&self) -> u16 {
        10
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        // 패턴: V '/' V (+ optional Subscript) AND prev is OpenParen/Operator(',')/None(독립 cell)
        matches!(tokens.get(index), Some(MathToken::Variable(_))) && matches!(tokens.get(index + 1), Some(MathToken::Operator('/'))) && matches!(tokens.get(index + 2), Some(MathToken::Variable(_))) && {
            let prev = rule_12::prev_non_space(tokens, index);
            matches!(prev, None | Some(MathToken::OpenParen(_)) | Some(MathToken::Operator(',')))
        }
    }

    fn apply(&self, tokens: &[MathToken], index: usize, result: &mut Vec<u8>, state: &mut MathEncodeState, engine: &MathTokenEngine) -> Result<MathTokenResult, String> {
        let (Some(MathToken::Variable(num)), Some(MathToken::Variable(den))) = (tokens.get(index), tokens.get(index + 2)) else {
            return Ok(MathTokenResult::Skip);
        };

        // 분모 right side는 V + optional Subscript까지 수집
        let mut den_end = index + 3;
        while matches!(tokens.get(den_end), Some(MathToken::Subscript(_)) | Some(MathToken::Superscript(_))) {
            den_end += 1;
        }

        // 분자(분모)/분모(분자)를 reverse: encode den + subscript first, then ⠌, then num
        result.push(crate::english::encode_english(den.to_ascii_lowercase())?);
        // den's subscripts/superscripts
        if den_end > index + 3 {
            engine.encode_tokens(&tokens[index + 3..den_end], result)?;
        }
        result.push(12); // ⠌ slash
        result.push(crate::english::encode_english(num.to_ascii_lowercase())?);
        state.prev_was_number = false;
        Ok(MathTokenResult::Consumed(den_end - index))
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
        matches!(tokens.get(index), Some(MathToken::Number(_))) && matches!(tokens.get(index + 1), Some(MathToken::Operator('/'))) && matches!(tokens.get(index + 2), Some(MathToken::Number(_))) && matches!(rule_12::prev_non_space(tokens, index), Some(MathToken::Operator('='))) && tokens.iter().any(|token| matches!(token, MathToken::MathSymbol('|')))
    }

    fn apply(&self, tokens: &[MathToken], index: usize, result: &mut Vec<u8>, state: &mut MathEncodeState, _engine: &MathTokenEngine) -> Result<MathTokenResult, String> {
        let (Some(MathToken::Number(left)), Some(MathToken::Number(right))) = (tokens.get(index), tokens.get(index + 2)) else {
            return Ok(MathTokenResult::Skip);
        };

        rule_1::encode_number_literal(right, result);
        result.push(12);
        rule_1::encode_number_literal(left, result);
        state.prev_was_number = false;
        Ok(MathTokenResult::Consumed(3))
    }
}
