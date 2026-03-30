//! 수학 제54항 — 편미분 기호 (∂).
//!
//! Partial derivative ∂ (U+2202) → code 40 (⠫).

use crate::math_symbol_shortcut;

pub fn is_partial_derivative(c: char) -> bool {
    c == '\u{2202}'
}

pub fn encode_partial_derivative(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_partial_derivative() {
        assert!(is_partial_derivative('\u{2202}'));
    }
}
