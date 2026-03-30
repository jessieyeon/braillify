//! 수학 제13항 — 그리스 문자.
//!
//! 그리스 문자 기호는 수학기호 단축표를 사용한다.

use crate::math_symbol_shortcut;

// Prepared for future direct encoder dispatch integration.
pub fn is_greek_symbol(c: char) -> bool {
    matches!(
        c,
        'Δ' | 'α' | 'β' | 'γ' | 'π' | 'Π' | 'Σ' | 'θ' | 'λ' | 'μ' | 'Ω'
    )
}

// Prepared for future direct encoder dispatch integration.
pub fn encode_greek_symbol(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}
