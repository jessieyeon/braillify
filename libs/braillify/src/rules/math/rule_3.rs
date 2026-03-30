//! 수학 제3항 — 등식/동치 기호.
//!
//! =, ≠, ≒, ≈ 등 등식 계열 기호를 단축표에서 인코딩한다.

use crate::math_symbol_shortcut;

// Prepared for future direct encoder dispatch integration.
pub fn is_equality_symbol(c: char) -> bool {
    matches!(c, '=' | '\u{2260}' | '\u{2252}' | '\u{2248}')
}

// Prepared for future direct encoder dispatch integration.
pub fn encode_equality_symbol(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}
