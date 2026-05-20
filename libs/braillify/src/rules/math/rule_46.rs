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

    // Check if the next token(s) form a compound argument that needs brackets
    // (multiple variables, or a fraction)
    let next_idx = *i + 1;
    if next_idx < tokens.len() {
        // Two consecutive variables: sinxy -> sin(xy)
        if matches!(tokens.get(next_idx), Some(MathToken::Variable(_)))
            && matches!(tokens.get(next_idx + 1), Some(MathToken::Variable(_)))
        {
            result.push(55); // Grouping open
            result.push(crate::english::encode_english(
                if let Some(MathToken::Variable(v)) = tokens.get(next_idx) {
                    *v
                } else {
                    'x'
                },
            )?);
            result.push(crate::english::encode_english(
                if let Some(MathToken::Variable(v)) = tokens.get(next_idx + 1) {
                    *v
                } else {
                    'y'
                },
            )?);
            result.push(62); // Grouping close
            *i += 3;
            return Ok(true);
        }
        // Fraction without parens: sin(6/x) or sin(x/6). U+2044 (LaTeX \frac slash)도 매칭.
        if matches!(
            tokens.get(next_idx),
            Some(MathToken::Number(_) | MathToken::Variable(_))
        ) && matches!(
            tokens.get(next_idx + 1),
            Some(MathToken::Operator('/') | MathToken::MathSymbol('\u{2044}'))
        ) && matches!(
                tokens.get(next_idx + 2),
                Some(MathToken::Number(_) | MathToken::Variable(_))
            )
        {
            result.push(55); // Grouping open
            // Encode the fraction tokens
            // We need to use the engine but we don't have it here
            // For now, just encode the 3 tokens directly
            match tokens.get(next_idx) {
                Some(MathToken::Number(n)) => {
                    result.push(60);
                    for ch in n.chars() {
                        result.extend(crate::number::encode_number(ch));
                    }
                }
                Some(MathToken::Variable(v)) => {
                    result.push(crate::english::encode_english(*v)?);
                }
                _ => {}
            }
            result.push(12); // fraction slash
            match tokens.get(next_idx + 2) {
                Some(MathToken::Number(n)) => {
                    result.push(60);
                    for ch in n.chars() {
                        result.extend(crate::number::encode_number(ch));
                    }
                }
                Some(MathToken::Variable(v)) => {
                    result.push(crate::english::encode_english(*v)?);
                }
                _ => {}
            }
            result.push(62); // Grouping close
            *i += 4;
            return Ok(true);
        }
    }
    *i += 1;
    Ok(true)
}
