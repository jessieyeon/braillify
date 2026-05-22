//! 수학 제15항 — 사용자 정의 이항연산 기호.
//!
//! ⊕, ⊖, ⊗, ⊙, ⊛, ∗, ∙, □, △ 계열 기호를 단축표에서 인코딩한다.

use crate::math_symbol_shortcut;

pub fn is_custom_binary_operator(c: char) -> bool {
    matches!(c, '\u{2295}' | '\u{2296}' | '\u{2297}' | '\u{2299}' | '\u{29BE}' | '\u{2217}' | '\u{2219}' | '\u{25A1}' | '\u{2206}')
}

pub fn encode_custom_binary_operator(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_custom_binary_operator() {
        assert!(is_custom_binary_operator('\u{2295}'));
    }
}
