//! 수학 제46항 — 삼각함수 표기.
//!
//! sin, cos, tan, csc, sec, cot 인코딩과 인수 정규화를 처리한다.

use crate::rules::math::function;
use crate::rules::math::parser::MathToken;

pub fn is_trig_function(name: &str) -> bool {
    matches!(name, "sin" | "cos" | "tan" | "csc" | "sec" | "cot")
}

pub fn encode_trig_function(
    name: &str,
    tokens: &[MathToken],
    i: &mut usize,
    result: &mut Vec<u8>,
    find_matching_paren: fn(&[MathToken], usize) -> Option<usize>,
) -> Result<bool, String> {
    if !is_trig_function(name) {
        return Ok(false);
    }
    let Some(encoded) = function::encode_function(name) else {
        return Ok(false);
    };
    result.extend_from_slice(encoded);

    if let (Some(MathToken::Number(n)), Some(MathToken::Variable(v))) =
        (tokens.get(*i + 1), tokens.get(*i + 2))
    {
        result.push(55);
        result.push(60);
        for ch in n.chars() {
            result.extend(crate::number::encode_number(ch));
        }
        result.push(crate::english::encode_english(v.to_ascii_lowercase())?);
        result.push(62);
        *i += 3;
        return Ok(true);
    }

    if let Some(MathToken::OpenParen(_)) = tokens.get(*i + 1)
        && let Some(close_idx) = find_matching_paren(tokens, *i + 1)
        && let [
            MathToken::Variable(v),
            MathToken::Operator('/'),
            MathToken::Number(n),
        ] = &tokens[*i + 2..close_idx]
    {
        result.push(55);
        result.push(60);
        for ch in n.chars() {
            result.extend(crate::number::encode_number(ch));
        }
        result.push(12);
        result.push(crate::english::encode_english(v.to_ascii_lowercase())?);
        result.push(62);
        *i = close_idx + 1;
        return Ok(true);
    }

    *i += 1;
    Ok(true)
}
