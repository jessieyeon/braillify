//! 수학 제20항 — 약동호.
//!
//! 약동호 ≒(U+2252)를 단축표 인코딩으로 처리한다.

use crate::math_symbol_shortcut;

pub fn is_approximation_symbol(c: char) -> bool {
    c == '\u{2252}'
}

pub fn encode_approximation_symbol(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_approximation_symbol() {
        assert!(is_approximation_symbol('\u{2252}'));
    }
}
