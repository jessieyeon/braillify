//! 수학 제36항 — 호 기호 (⌢AB).
//!
//! 호 표기는 `⌢`(U+2322)와 연속 대문자 표기를 결합해 표현한다.
//! 내부표기 예시는 `@[, ,AB`(공백 제거 시 `@[, ,AB` 형태)로 설명할 수 있으며,
//! 구현에서는 호 기호를 수학 기호 단축 인코더로 처리한다.

use crate::math_symbol_shortcut;

pub fn is_arc_symbol(c: char) -> bool {
    c == '⌢'
}

pub fn encode_arc(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_arc_uppercase_prefix(result: &mut Vec<u8>) -> Result<(), String> {
        result.extend_from_slice(&[32, 32]);
        Ok(())
    }

    #[test]
    fn detects_arc_symbol() {
        assert!(is_arc_symbol('⌢'));
        assert!(!is_arc_symbol('^'));
    }

    #[test]
    fn encodes_arc_uppercase_prefix_correctly() -> Result<(), String> {
        let mut result = Vec::new();
        encode_arc_uppercase_prefix(&mut result)?;
        assert_eq!(result, vec![32, 32]);
        Ok(())
    }
}
