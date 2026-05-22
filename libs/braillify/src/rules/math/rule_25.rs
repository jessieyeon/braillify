//! 수학 제25항 — 합 기호(Σ).
//!
//! 합 기호 Σ는 대문자 그리스 문자 표지(`,.s`, ⠠⠨⠎) 계열로 인코딩하고,
//! 상한/하한은 첨자·위첨자 규칙에 따라 이어서 붙인다.

use crate::math_symbol_shortcut;

pub fn is_sigma_symbol(c: char) -> bool {
    c == '\u{03A3}'
}

pub fn encode_sigma_with_bounds(lower_bound_encoded: &[u8], upper_bound_encoded: &[u8], result: &mut Vec<u8>) -> Result<(), String> {
    let sigma = math_symbol_shortcut::encode_char_math_symbol_shortcut('\u{03A3}')?;
    result.extend_from_slice(sigma);

    if !lower_bound_encoded.is_empty() {
        result.extend_from_slice(lower_bound_encoded);
    }
    if !upper_bound_encoded.is_empty() {
        result.extend_from_slice(upper_bound_encoded);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math_symbol_shortcut;

    #[test]
    fn encodes_sigma_prefix() -> Result<(), String> {
        let mut result = Vec::new();
        encode_sigma_with_bounds(&[1], &[2], &mut result)?;

        let sigma = math_symbol_shortcut::encode_char_math_symbol_shortcut('\u{03A3}')?;
        assert!(result.starts_with(sigma));
        Ok(())
    }
}
