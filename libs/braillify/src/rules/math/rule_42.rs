//! 수학 제42항 — 닮음 기호 (∽).
//!
//! Similarity ∽ (U+223D).

use crate::math_symbol_shortcut;

pub fn is_similarity_symbol(c: char) -> bool {
    c == '\u{223D}'
}

pub fn encode_similarity_symbol(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_similarity_symbol() {
        assert!(is_similarity_symbol('\u{223D}'));
    }
}
