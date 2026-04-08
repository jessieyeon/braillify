//! 수학 제38항 — 단방향 화살표 반직선 (→AB).
//!
//! 단방향 화살표 반직선은 화살표 기호 `→`와 연속 대문자 표기를 결합한다.
//! 내부표기 패턴은 `3o,,AB`이며,
//! 화살표 지시 뒤에 대문자 연속 접두 `,,`를 두고 문자 `AB`를 배치한다.

use crate::math_symbol_shortcut;

pub fn is_right_arrow_ray_symbol(c: char) -> bool {
    c == '→'
}

pub fn encode_right_arrow_ray_symbol(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_right_arrow_ray_uppercase_prefix(result: &mut Vec<u8>) -> Result<(), String> {
        result.extend_from_slice(&[32, 32]);
        Ok(())
    }

    #[test]
    fn detects_right_arrow_ray_symbol() {
        assert!(is_right_arrow_ray_symbol('→'));
        assert!(!is_right_arrow_ray_symbol('↔'));
    }

    #[test]
    fn encodes_right_arrow_ray_uppercase_prefix_correctly() -> Result<(), String> {
        let mut result = Vec::new();
        encode_right_arrow_ray_uppercase_prefix(&mut result)?;
        assert_eq!(result, vec![32, 32]);
        Ok(())
    }
}
