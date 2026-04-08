//! 수학 제50항 — 특수 상수.
//!
//! 무한대(∞) 등 상수 기호를 단축표로 인코딩한다.

use crate::math_symbol_shortcut;

// Prepared for future direct encoder dispatch integration.
pub fn is_special_constant(c: char) -> bool {
    c == '\u{221E}'
}

// Prepared for future direct encoder dispatch integration.
pub fn encode_special_constant(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}
