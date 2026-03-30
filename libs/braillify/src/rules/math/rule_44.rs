//! 수학 제44항 — 평행 기호 (∥).
//!
//! Parallel ∥ (U+2225).

use crate::math_symbol_shortcut;

pub fn is_parallel_symbol(c: char) -> bool {
    c == '\u{2225}'
}

pub fn encode_parallel_symbol(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_parallel_symbol() {
        assert!(is_parallel_symbol('\u{2225}'));
    }
}
