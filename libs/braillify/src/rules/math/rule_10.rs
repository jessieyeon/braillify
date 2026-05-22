//! 수학 제10항 — 화살표 기호.
//!
//! →, ←, ↔, ↑, ↓ 및 이중 화살표를 단축표로 인코딩한다.

use crate::math_symbol_shortcut;

// Prepared for future direct encoder dispatch integration.
pub fn is_arrow_symbol(c: char) -> bool {
    matches!(
        c,
        '\u{2192}' | '\u{2190}' | '\u{2194}' | '\u{21D2}' | '\u{21D4}' | '\u{2191}' | '\u{2193}'
    )
}

// Prepared for future direct encoder dispatch integration.
pub fn encode_arrow_symbol(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}
