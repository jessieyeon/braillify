//! 수학 제14항 — 로마 숫자 표기.
//!
//! 로마 숫자열(I,V,X,L,C,D,M)을 로마숫자 지시(0)로 감싸 인코딩한다.

pub fn is_roman_numeral_expression(input: &str) -> bool {
    !input.is_empty()
        && input
            .chars()
            .all(|c| matches!(c, 'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M'))
}

pub fn encode_roman_numeral_expression(input: &str) -> Result<Vec<u8>, String> {
    // 0 + ,(단일) 또는 ,,(복수) + 소문자 로마문자 + 4
    let chars: Vec<char> = input.chars().collect();
    let mut result = vec![52, 32];
    if chars.len() >= 2 {
        result.push(32);
    }
    for ch in chars {
        result.push(crate::english::encode_english(ch.to_ascii_lowercase())?);
    }
    result.push(50);
    Ok(result)
}
