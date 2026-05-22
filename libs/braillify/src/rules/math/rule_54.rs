//! 수학 제54항 — 편미분 기호 (∂).
//!
//! Partial derivative ∂ (U+2202) → code 40 (⠫).

use crate::math_symbol_shortcut;

use super::math_token_rule::{MathEncodeState, MathTokenEngine, MathTokenResult, MathTokenRule};
use super::parser::MathToken;

fn next_non_space(tokens: &[MathToken], mut index: usize) -> Option<usize> {
    while matches!(tokens.get(index), Some(MathToken::Space)) {
        index += 1;
    }
    tokens.get(index).map(|_| index)
}

pub fn is_partial_derivative(c: char) -> bool {
    c == '\u{2202}'
}

pub fn encode_partial_derivative(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

pub struct PartialDerivativeFractionRule;

impl MathTokenRule for PartialDerivativeFractionRule {
    fn name(&self) -> &'static str {
        "PartialDerivativeFractionRule"
    }

    fn priority(&self) -> u16 {
        40
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        let Some(numerator_index) = next_non_space(tokens, index + 1) else {
            return false;
        };
        let Some(slash_index) = next_non_space(tokens, numerator_index + 1) else {
            return false;
        };
        let Some(second_partial_index) = next_non_space(tokens, slash_index + 1) else {
            return false;
        };
        let Some(denominator_index) = next_non_space(tokens, second_partial_index + 1) else {
            return false;
        };

        matches!(tokens.get(index), Some(MathToken::MathSymbol('\u{2202}'))) && matches!(tokens.get(numerator_index), Some(MathToken::Variable(_) | MathToken::UpperVariable(_))) && matches!(tokens.get(slash_index), Some(MathToken::Operator('/'))) && matches!(tokens.get(second_partial_index), Some(MathToken::MathSymbol('\u{2202}'))) && matches!(tokens.get(denominator_index), Some(MathToken::Variable(_) | MathToken::UpperVariable(_)))
    }

    fn apply(&self, tokens: &[MathToken], index: usize, result: &mut Vec<u8>, state: &mut MathEncodeState, engine: &MathTokenEngine) -> Result<MathTokenResult, String> {
        let numerator_index = next_non_space(tokens, index + 1).ok_or_else(|| "Missing numerator in partial derivative".to_string())?;
        let slash_index = next_non_space(tokens, numerator_index + 1).ok_or_else(|| "Missing slash in partial derivative".to_string())?;
        let second_partial_index = next_non_space(tokens, slash_index + 1).ok_or_else(|| "Missing denominator partial symbol".to_string())?;
        let denominator_index = next_non_space(tokens, second_partial_index + 1).ok_or_else(|| "Missing denominator in partial derivative".to_string())?;

        let numerator = tokens[numerator_index..numerator_index + 1].to_vec();
        let denominator = tokens[denominator_index..denominator_index + 1].to_vec();

        encode_partial_derivative('\u{2202}', result)?;
        engine.encode_tokens(&denominator, result)?;
        result.push(12);
        encode_partial_derivative('\u{2202}', result)?;
        engine.encode_tokens(&numerator, result)?;

        state.prev_was_number = false;
        Ok(MathTokenResult::Consumed(denominator_index + 1 - index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::math::encoder::encode_math_expression;

    #[test]
    fn test_is_partial_derivative() {
        assert!(is_partial_derivative('\u{2202}'));
    }

    #[test]
    fn encodes_partial_derivative_fraction() {
        assert_eq!(encode_math_expression("∂z/∂x").unwrap(), vec![43, 45, 12, 43, 53]);
    }

    #[test]
    fn encodes_partial_derivative_fraction_with_spaces() {
        assert_eq!(encode_math_expression("∂z / ∂x").unwrap(), vec![43, 45, 12, 43, 53]);
    }
}
