//! 수학 제31항 — 점근 등호 (≃).
//!
//! 점근 등호 `≃`(U+2243)는 수학 내부표기에서 `@93`에 해당한다.

use crate::math_symbol_shortcut;

pub fn is_asymptotic_equal(c: char) -> bool {
    c == '≃'
}

pub fn encode_asymptotic_equal(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_asymptotic_equal_symbol() {
        assert!(is_asymptotic_equal('≃'));
        assert!(!is_asymptotic_equal('≈'));
    }
}
