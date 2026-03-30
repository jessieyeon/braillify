//! 수학 제32항 — 합동 기호 (≅).
//!
//! 합동 기호 `≅`(U+2245)는 수학 내부표기에서 `@933`에 해당한다.

use crate::math_symbol_shortcut;

pub fn is_congruence_symbol(c: char) -> bool {
    c == '≅'
}

pub fn encode_congruence_symbol(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_congruence_symbol() {
        assert!(is_congruence_symbol('≅'));
        assert!(!is_congruence_symbol('≃'));
    }
}
