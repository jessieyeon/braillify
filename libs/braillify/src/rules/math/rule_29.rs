//! 수학 제29항 — 근사 등호 (≈).
//!
//! 근사 등호 `≈`(U+2248)는 수학 내부표기에서 `@9@9`에 해당한다.

use crate::math_symbol_shortcut;

pub fn is_approximate_equal(c: char) -> bool {
    c == '≈'
}

pub fn encode_approximate_equal(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_approximate_equal_symbol() {
        assert!(is_approximate_equal('≈'));
        assert!(!is_approximate_equal('='));
    }
}
