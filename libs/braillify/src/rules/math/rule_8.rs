//! 수학 제8항 — 소수점 표기.
//!
//! 소수점(⠲)과 선행 소수(.47)의 수표 부여를 처리한다.

use crate::rules::math::parser::MathToken;

use super::math_token_rule::{MathEncodeState, MathTokenEngine, MathTokenResult, MathTokenRule};

pub fn encode_decimal_point(
    tokens: &[MathToken],
    i: usize,
    prev_was_number: &mut bool,
    result: &mut Vec<u8>,
) {
    // PDF — 직전이 결합 부호(예: `̄` overline)면 그 앞 baseline이 Number인지 본다.
    // 예: `2̄.3010` 에서 overline U+0305 사이를 건너뛰고 `2` (Number)를 인식한다.
    let prev_baseline_is_number = {
        let mut j = i;
        while j > 0
            && matches!(
                tokens.get(j - 1),
                Some(MathToken::MathSymbol(
                    '\u{0300}'..='\u{036F}'
                    | '\u{1AB0}'..='\u{1AFF}'
                    | '\u{1DC0}'..='\u{1DFF}'
                    | '\u{20D0}'..='\u{20FF}'
                    | '\u{FE20}'..='\u{FE2F}',
                ))
            )
        {
            j -= 1;
        }
        j > 0 && matches!(tokens.get(j - 1), Some(MathToken::Number(_)))
    };
    if !*prev_was_number && !prev_baseline_is_number {
        let has_next_number = match tokens.get(i + 1) {
            Some(MathToken::Number(_)) => true,
            Some(MathToken::MathSymbol('\u{0307}')) => {
                matches!(tokens.get(i + 2), Some(MathToken::Number(_)))
            }
            _ => false,
        };
        if has_next_number {
            result.push(60);
            *prev_was_number = true;
        }
    } else if prev_baseline_is_number {
        // 직전 baseline이 Number이면 그 number context를 유지한다.
        *prev_was_number = true;
    }
    result.push(50);
}

pub struct DecimalPointRule;

impl MathTokenRule for DecimalPointRule {
    fn name(&self) -> &'static str {
        "DecimalPointRule"
    }

    fn priority(&self) -> u16 {
        50
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::DecimalPoint))
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        _engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        encode_decimal_point(tokens, index, &mut state.prev_was_number, result);
        Ok(MathTokenResult::Consumed(1))
    }
}
