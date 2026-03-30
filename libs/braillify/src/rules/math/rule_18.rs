//! 수학 제18항 — 위첨자 표기.
//!
//! 위첨자 지시(^)와 다중 토큰 묶음, 지수 특수형을 처리한다.

use crate::rules::math::parser::{BracketKind, MathToken};

use super::math_token_rule::{MathEncodeState, MathTokenEngine, MathTokenResult, MathTokenRule};
use super::rule_1;

fn is_simple_signed_number(content: &[MathToken]) -> bool {
    if content.len() != 2 {
        return false;
    }
    matches!(content[0], MathToken::Operator('\u{2212}'))
        && matches!(content[1], MathToken::Number(_))
}

pub fn should_group_superscript(content: &[MathToken]) -> bool {
    if content.len() <= 1 {
        return false;
    }
    if is_simple_signed_number(content) {
        return false;
    }
    content.iter().any(|token| {
        matches!(
            token,
            MathToken::Operator(_)
                | MathToken::OpenParen(_)
                | MathToken::CloseParen(_)
                | MathToken::Space
        )
    }) || content.len() >= 3
}

pub fn encode_superscript(
    tokens: &[MathToken],
    i: &mut usize,
    content: &[MathToken],
    result: &mut Vec<u8>,
    engine: &MathTokenEngine,
) -> Result<bool, String> {
    if *i >= 2
        && matches!(tokens.get(*i - 1), Some(MathToken::Subscript(_)))
        && matches!(
            tokens.get(*i - 2),
            Some(MathToken::MathSymbol('\u{222B}' | '\u{222C}' | '\u{222E}'))
        )
    {
        result.push(0);
        engine.encode_tokens(content, result)?;
        result.push(0);
        *i += 1;
        return Ok(true);
    }

    if matches!(tokens.get(*i + 1), Some(MathToken::MathSymbol('\u{221A}'))) {
        if content.len() > 1 {
            result.push(55);
            engine.encode_tokens(content, result)?;
            result.push(62);
        } else {
            engine.encode_tokens(content, result)?;
        }
        result.push(59);
        *i += 2;
        return Ok(true);
    }

    if let [MathToken::Number(left)] = content
        && matches!(tokens.get(*i + 1), Some(MathToken::MathSymbol('\u{00B7}')))
        && let Some(MathToken::Superscript(right_content)) = tokens.get(*i + 2)
        && let [MathToken::Number(right)] = right_content.as_slice()
    {
        result.push(24);
        result.push(60);
        for ch in left.chars() {
            result.extend(crate::number::encode_number(ch));
        }
        result.push(50);
        for ch in right.chars() {
            result.extend(crate::number::encode_number(ch));
        }
        *i += 3;
        return Ok(true);
    }

    if let [MathToken::Number(left)] = content
        && matches!(tokens.get(*i + 1), Some(MathToken::Operator('/')))
        && let Some(MathToken::Superscript(right_content)) = tokens.get(*i + 2)
        && let [MathToken::Number(right)] = right_content.as_slice()
    {
        result.push(24);
        result.push(55);
        rule_1::encode_number_literal(left, result);
        result.push(12);
        rule_1::encode_number_literal(right, result);
        result.push(62);
        *i += 3;
        return Ok(true);
    }

    let (sup_content, force_group) = if content.len() >= 2
        && matches!(
            (content.first(), content.last()),
            (
                Some(MathToken::OpenParen(BracketKind::MathParen)),
                Some(MathToken::CloseParen(BracketKind::MathParen))
            )
        ) {
        (&content[1..content.len() - 1], true)
    } else {
        (content, false)
    };

    result.push(24);
    if force_group || should_group_superscript(sup_content) {
        result.push(55);
        engine.encode_tokens(sup_content, result)?;
        result.push(62);
    } else {
        engine.encode_tokens(sup_content, result)?;
    }
    *i += 1;
    Ok(false)
}

pub struct SuperscriptRule;

impl MathTokenRule for SuperscriptRule {
    fn name(&self) -> &'static str {
        "SuperscriptRule"
    }

    fn priority(&self) -> u16 {
        50
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::Superscript(_)))
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        let Some(MathToken::Superscript(content)) = tokens.get(index) else {
            return Ok(MathTokenResult::Skip);
        };
        let mut cursor = index;
        let _ = encode_superscript(tokens, &mut cursor, content, result, engine)?;
        state.prev_was_number = false;
        Ok(MathTokenResult::Consumed(cursor - index))
    }
}
