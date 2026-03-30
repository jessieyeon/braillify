//! 수학 제28항 — 노름 기호 ‖x‖.
//!
//! 이 항은 노름을 나타내는 이중 절댓값 막대(‖, U+2016)를 다룬다.
//! 여는 막대와 닫는 막대는 동일한 점형 코드 `[51, 51]`을 사용한다.

use crate::math_symbol_shortcut;

pub fn is_norm_symbol(c: char) -> bool {
    c == '‖'
}

pub fn encode_norm_open(result: &mut Vec<u8>) -> Result<(), String> {
    result.extend_from_slice(&[51, 51]);
    Ok(())
}

pub fn encode_norm_close(result: &mut Vec<u8>) -> Result<(), String> {
    result.extend_from_slice(&[51, 51]);
    Ok(())
}

pub fn encode_norm_symbol(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_norm_open_and_close() -> Result<(), String> {
        let mut result = Vec::new();
        encode_norm_open(&mut result)?;
        encode_norm_close(&mut result)?;
        assert_eq!(result, vec![51, 51, 51, 51]);
        Ok(())
    }
}
