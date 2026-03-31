//! 수학 제19항 — 아래첨자 표기.
//!
//! 아래첨자 지시(;)와 조합수 표기(₃P₁, ₃C₂)를 처리한다.

use crate::rules::math::parser::{BracketKind, MathToken};

use super::math_token_rule::{MathEncodeState, MathTokenEngine, MathTokenResult, MathTokenRule};
use super::rule_1;

fn single_numeric(content: &[MathToken]) -> Option<String> {
    match content {
        [MathToken::Number(n)] => Some(n.clone()),
        _ => None,
    }
}

fn prev_non_space(tokens: &[MathToken], mut idx: usize) -> Option<&MathToken> {
    while idx > 0 {
        idx -= 1;
        let token = tokens.get(idx)?;
        if !matches!(token, MathToken::Space) {
            return Some(token);
        }
    }
    None
}

fn is_plain_numeric_subscript(content: &[MathToken]) -> bool {
    content
        .iter()
        .all(|token| matches!(token, MathToken::Number(_) | MathToken::DecimalPoint))
}

pub fn should_group_subscript(content: &[MathToken]) -> bool {
    if content.len() <= 1 {
        return false;
    }
    if matches!(
        (content.first(), content.last()),
        (
            Some(MathToken::OpenParen(BracketKind::MathParen)),
            Some(MathToken::CloseParen(BracketKind::MathParen))
        )
    ) {
        return false;
    }
    !is_plain_numeric_subscript(content)
}

pub fn encode_subscript(
    tokens: &[MathToken],
    i: &mut usize,
    content: &[MathToken],
    result: &mut Vec<u8>,
    engine: &MathTokenEngine,
) -> Result<bool, String> {
    if let Some(left) = single_numeric(content)
        && matches!(
            tokens.get(*i + 1),
            Some(MathToken::UpperVariable('P' | 'C'))
        )
        && let Some(MathToken::Subscript(right_content)) = tokens.get(*i + 2)
        && let Some(right) = single_numeric(right_content)
        && let Some(MathToken::UpperVariable(mark)) = tokens.get(*i + 1)
    {
        result.push(32);
        result.push(crate::english::encode_english(mark.to_ascii_lowercase())?);
        result.push(38);
        rule_1::encode_number_literal(&left, result);
        result.push(0);
        rule_1::encode_number_literal(&right, result);
        result.push(52);
        *i += 3;
        return Ok(true);
    }

    if let Some(base) = single_numeric(content)
        && matches!(prev_non_space(tokens, *i), Some(MathToken::Number(_)))
    {
        result.push(48);
        result.push(38);
        rule_1::encode_number_literal(&base, result);
        result.push(52);
        *i += 1;
        return Ok(false);
    }

    result.push(48);
    if should_group_subscript(content) {
        result.push(55);
        if let [MathToken::Number(n), MathToken::Variable(v)] = content {
            rule_1::encode_number_literal(n, result);
            result.push(16);
            result.push(crate::english::encode_english(v.to_ascii_lowercase())?);
        } else if let [MathToken::Number(n), MathToken::UpperVariable(v)] = content {
            rule_1::encode_number_literal(n, result);
            result.push(16);
            result.push(crate::english::encode_english(v.to_ascii_lowercase())?);
        } else {
            engine.encode_tokens(content, result)?;
        }
        result.push(62);
    } else {
        engine.encode_tokens(content, result)?;
    }
    *i += 1;
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::super::encoder::encode_math_expression;

    #[test]
    fn encodes_number_base_notation_without_explicit_subscript_parentheses() {
        assert_eq!(
            encode_math_expression("1010₂").expect("math encoding should succeed"),
            vec![60, 1, 26, 1, 26, 48, 38, 60, 3, 52]
        );
    }

    #[test]
    fn encodes_number_base_notation_with_explicit_subscript_parentheses() {
        assert_eq!(
            encode_math_expression("1101₍₂₎").expect("math encoding should succeed"),
            vec![60, 1, 1, 26, 1, 48, 38, 60, 3, 52]
        );
    }
}

pub struct SubscriptRule;

impl MathTokenRule for SubscriptRule {
    fn name(&self) -> &'static str {
        "SubscriptRule"
    }

    fn priority(&self) -> u16 {
        50
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::Subscript(_)))
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        let Some(MathToken::Subscript(content)) = tokens.get(index) else {
            return Ok(MathTokenResult::Skip);
        };
        let mut cursor = index;
        let _ = encode_subscript(tokens, &mut cursor, content, result, engine)?;
        state.prev_was_number = false;
        Ok(MathTokenResult::Consumed(cursor - index))
    }
}
