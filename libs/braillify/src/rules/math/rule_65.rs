//! 수학 제65항 — 그러므로/왜냐하면.
//!
//! Therefore ∴ (U+2234) and because ∵ (U+2235).

use crate::math_symbol_shortcut;

pub fn is_therefore_because(c: char) -> bool {
    matches!(c, '\u{2234}' | '\u{2235}')
}

pub fn encode_therefore_because(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_therefore_because() {
        assert!(is_therefore_because('\u{2234}'));
    }
}
