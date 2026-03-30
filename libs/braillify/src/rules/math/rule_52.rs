//! 수학 제52항 — 델타 기호 (Δ).
//!
//! Delta Δ (U+0394) — uppercase Greek delta, encoded as ,.d.

use crate::math_symbol_shortcut;

pub fn is_delta_symbol(c: char) -> bool {
    c == '\u{0394}'
}

pub fn encode_delta_symbol(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_delta_symbol() {
        assert!(is_delta_symbol('\u{0394}'));
    }
}
