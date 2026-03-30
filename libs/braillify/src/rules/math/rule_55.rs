//! 수학 제55항 — 나블라/그래디언트 (∇).
//!
//! Nabla/gradient ∇ (U+2207).

use crate::math_symbol_shortcut;

pub fn is_nabla_symbol(c: char) -> bool {
    c == '\u{2207}'
}

pub fn encode_nabla_symbol(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_nabla_symbol() {
        assert!(is_nabla_symbol('\u{2207}'));
    }
}
