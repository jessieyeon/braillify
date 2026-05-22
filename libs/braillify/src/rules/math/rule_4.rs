//! 수학 제4항 — 대소/비교 기호.
//!
//! >, <, ≥, ≤ 및 부정 비교 기호를 단축표로 인코딩한다.

use crate::math_symbol_shortcut;

// Prepared for future direct encoder dispatch integration.
pub fn is_comparison_symbol(c: char) -> bool {
    matches!(
        c,
        '>' | '<' | '\u{2265}' | '\u{2264}' | '\u{226F}' | '\u{226E}'
    )
}

// Prepared for future direct encoder dispatch integration.
pub fn encode_comparison_symbol(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}
