//! 수학 제39항 — 각도 기호.
//!
//! 각 표기(∠ABC)에서 쓰는 각도 기호 ∠(U+2220)를 단축표로 인코딩한다.

use crate::math_symbol_shortcut;

pub fn is_angle_symbol(c: char) -> bool {
    c == '\u{2220}'
}

pub fn encode_angle_symbol(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_angle_symbol() {
        assert!(is_angle_symbol('∠'));
        assert!(!is_angle_symbol('A'));
    }
}
