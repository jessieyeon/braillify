//! 수학 제43항 — 합동 기호 (≡).
//!
//! Identity/congruence ≡ (U+2261).

use crate::math_symbol_shortcut;

pub fn is_identity_symbol(c: char) -> bool {
    c == '\u{2261}'
}

pub fn encode_identity_symbol(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_identity_symbol() {
        assert!(is_identity_symbol('\u{2261}'));
    }
}
