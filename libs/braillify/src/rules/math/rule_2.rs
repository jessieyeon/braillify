//! 수학 제2항 — 사칙연산 기호.
//!
//! +, -, ×, ÷, · 및 팩토리얼, 쉼표, 슬래시 연산자 처리를 담당한다.

use crate::math_symbol_shortcut;
use crate::rules::math::parser::MathToken;

use super::math_token_rule::{MathEncodeState, MathTokenEngine, MathTokenResult, MathTokenRule};
use super::rule_7;

pub fn is_algebraic_neighbor(token: Option<&MathToken>) -> bool {
    matches!(
        token,
        Some(
            MathToken::Variable(_)
                | MathToken::UpperVariable(_)
                | MathToken::Number(_)
                | MathToken::Subscript(_)
                | MathToken::Superscript(_)
                | MathToken::OpenParen(_)
                | MathToken::CloseParen(_)
                | MathToken::MathSymbol('\u{221E}')
        )
    )
}

pub fn needs_binary_spacing(c: char) -> bool {
    matches!(
        c,
        '\u{2192}'
            | '\u{21D2}'
            | '\u{2194}'
            | '\u{21D4}'
            | '\u{21C4}'
            | '\u{2227}'
            | '\u{2228}'
            | '\u{22BB}'
            | '\u{2193}'
            | '\u{2191}'
            | '\u{2229}'
            | '\u{222A}'
            | '\u{25A1}'
            | '\u{2295}'
            | '\u{2296}'
            | '\u{2297}'
            | '\u{29BE}'
            | '\u{2217}'
            | '\u{2219}'
            | '\u{2206}'
            | '\u{2234}'
            | '\u{2235}'
    )
}

pub fn encode_operator(
    token: char,
    tokens: &[MathToken],
    i: usize,
    result: &mut Vec<u8>,
) -> Result<(), String> {
    if token == '+' {
        let prev = tokens[..i]
            .iter()
            .rev()
            .find(|t| !matches!(t, MathToken::Space));
        let next = tokens[i + 1..]
            .iter()
            .find(|t| !matches!(t, MathToken::Space));
        let has_set_triangle = tokens
            .iter()
            .any(|t| matches!(t, MathToken::MathSymbol('\u{2206}')));

        if has_set_triangle
            && matches!(prev, Some(MathToken::CloseParen(_)))
            && matches!(next, Some(MathToken::OpenParen(_)))
        {
            result.push(0);
            result.push(44);
            result.push(0);
            return Ok(());
        }
    }

    if token == '!' {
        result.push(22);
        return Ok(());
    }

    if token == ',' {
        let divisibility_context = matches!(
            (
                tokens.get(i.saturating_sub(1)),
                tokens.get(i.saturating_sub(2))
            ),
            (Some(MathToken::Number(_)), Some(MathToken::MathSymbol('|')))
        );

        if tokens.get(i + 1).is_none() && divisibility_context {
            return Ok(());
        }
        if matches!(tokens.get(i + 1), Some(MathToken::Space)) && divisibility_context {
            result.push(0);
            return Ok(());
        }

        result.push(16);
        if matches!(
            tokens.get(i + 1),
            Some(
                MathToken::UpperVariable(_)
                    | MathToken::Variable(_)
                    | MathToken::Number(_)
                    | MathToken::Subscript(_)
                    | MathToken::Superscript(_)
                    | MathToken::MathSymbol(_)
            )
        ) {
            result.push(0);
        }
        return Ok(());
    }

    if token == '/' {
        if rule_7::slash_as_fraction_symbol(tokens, i) {
            let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(token)?;
            result.extend_from_slice(encoded);
        } else {
            result.push(12);
        }
        return Ok(());
    }

    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(token)?;
    result.extend_from_slice(encoded);
    Ok(())
}

pub struct OperatorRule;

impl MathTokenRule for OperatorRule {
    fn name(&self) -> &'static str {
        "OperatorRule"
    }

    fn priority(&self) -> u16 {
        50
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::Operator(_)))
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        _engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        let Some(MathToken::Operator(c)) = tokens.get(index) else {
            return Ok(MathTokenResult::Skip);
        };

        let korean_group_operator = matches!(*c, '+' | '×')
            && matches!(tokens.get(index.saturating_sub(1)), Some(MathToken::KoreanWord(_)))
            && matches!(tokens.get(index + 1), Some(MathToken::KoreanWord(_)));
        if korean_group_operator {
            result.push(0);
            encode_operator(*c, tokens, index, result)?;
            result.push(0);
            state.prev_was_number = false;
            return Ok(MathTokenResult::Consumed(1));
        }

        let label_equation = *c == '='
            && matches!(tokens.get(index.saturating_sub(1)), Some(MathToken::KoreanWord(_)))
            && matches!(tokens.get(index + 1), Some(MathToken::MathSymbol('\u{221A}')));
        if label_equation {
            result.push(0);
            encode_operator(*c, tokens, index, result)?;
            state.prev_was_number = false;
            return Ok(MathTokenResult::Consumed(1));
        }

        let should_pad = needs_binary_spacing(*c)
            && index > 0
            && is_algebraic_neighbor(tokens.get(index - 1))
            && is_algebraic_neighbor(tokens.get(index + 1));
        if should_pad && !matches!(tokens.get(index - 1), Some(MathToken::Space)) {
            result.push(0);
        }
        encode_operator(*c, tokens, index, result)?;
        if should_pad && !matches!(tokens.get(index + 1), Some(MathToken::Space)) {
            result.push(0);
        }
        state.prev_was_number = false;
        Ok(MathTokenResult::Consumed(1))
    }
}
