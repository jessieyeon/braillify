//! 수학 제30항 — 점 합동 기호 (≊).
//!
//! 점 합동 기호 `≊`(U+224A)를 판별하고 수학 기호 단축 인코더로 처리한다.

use crate::math_symbol_shortcut;

pub fn is_dot_congruence(c: char) -> bool {
    c == '≊'
}

pub fn encode_dot_congruence(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_dot_congruence_symbol() {
        assert!(is_dot_congruence('≊'));
        assert!(!is_dot_congruence('≅'));
    }
}
