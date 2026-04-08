//! 수학 제33항 — 기하 연산 기호 (▷, ◁).
//!
//! 기하 연산에 쓰는 삼각 기호 `▷`(U+25B7), `◁`(U+25C1)을 판별하고
//! 수학 기호 단축 인코더를 통해 부호화한다.

use crate::math_symbol_shortcut;

pub fn is_geometric_operator(c: char) -> bool {
    matches!(c, '▷' | '◁')
}

pub fn encode_geometric_operator(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_geometric_operators() {
        assert!(is_geometric_operator('▷'));
        assert!(is_geometric_operator('◁'));
        assert!(!is_geometric_operator('△'));
    }
}
