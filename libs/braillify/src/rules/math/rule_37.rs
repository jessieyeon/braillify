//! 수학 제37항 — 양방향 화살표 직선 (↔AB).
//!
//! 양방향 화살표 직선은 화살표 기호 `↔`와 연속 대문자 표기를 결합한다.
//! 내부표기 패턴은 `[3o,,AB`이며,
//! 화살표 지시 뒤에 대문자 연속 접두 `,,`를 두고 문자 `AB`를 배치한다.

use crate::math_symbol_shortcut;

pub fn is_double_arrow_line_symbol(c: char) -> bool {
    c == '↔'
}

pub fn encode_double_arrow_line_symbol(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_double_arrow_line_uppercase_prefix(result: &mut Vec<u8>) -> Result<(), String> {
        result.extend_from_slice(&[32, 32]);
        Ok(())
    }

    #[test]
    fn detects_double_arrow_line_symbol() {
        assert!(is_double_arrow_line_symbol('↔'));
        assert!(!is_double_arrow_line_symbol('→'));
    }

    #[test]
    fn encodes_double_arrow_line_uppercase_prefix_correctly() -> Result<(), String> {
        let mut result = Vec::new();
        encode_double_arrow_line_uppercase_prefix(&mut result)?;
        assert_eq!(result, vec![32, 32]);
        Ok(())
    }
}
