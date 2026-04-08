//! 수학 제40항 — 도형 기호.
//!
//! 도형 표기에서 쓰는 △, □, ◯, ⊙, ☆, ○, ● 기호를 단축표로 인코딩한다.

use crate::math_symbol_shortcut;

pub fn is_geometric_shape(c: char) -> bool {
    matches!(
        c,
        '\u{25B3}' | '\u{25A1}' | '\u{25EF}' | '\u{2299}' | '\u{2606}' | '\u{25CB}' | '\u{25CF}'
    )
}

pub fn encode_geometric_shape(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geometric_shape_detect() {
        assert!(is_geometric_shape('△'));
        assert!(is_geometric_shape('□'));
        assert!(is_geometric_shape('◯'));
        assert!(is_geometric_shape('⊙'));
        assert!(is_geometric_shape('☆'));
        assert!(is_geometric_shape('○'));
        assert!(is_geometric_shape('●'));
        assert!(!is_geometric_shape('A'));
    }
}
