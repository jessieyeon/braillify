//! 수학 제59항 — 선적분/경로적분 (∮).
//!
//! Contour integral ∮ (U+222E).

use crate::math_symbol_shortcut;

pub fn is_contour_integral(c: char) -> bool {
    c == '\u{222E}'
}

pub fn encode_contour_integral(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_contour_integral() {
        assert!(is_contour_integral('\u{222E}'));
    }
}
