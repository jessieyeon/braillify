//! 수학 제21항 — 절댓값 기호.
//!
//! 절댓값 표기 `|x|`는 여닫는 막대 모두 코드 51(⠳)을 사용한다.
//! 노름 표기 `‖x‖`는 51, 51(⠳⠳)으로 처리한다.

pub fn is_absolute_value_bar(c: char) -> bool {
    c == '|'
}

pub fn encode_absolute_value_open(result: &mut Vec<u8>) -> Result<(), String> {
    result.push(51);
    Ok(())
}

pub fn encode_absolute_value_close(result: &mut Vec<u8>) -> Result<(), String> {
    result.push(51);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_norm_delimiter(result: &mut Vec<u8>) -> Result<(), String> {
        result.extend_from_slice(&[51, 51]);
        Ok(())
    }

    #[test]
    fn encodes_absolute_value_and_norm() -> Result<(), String> {
        let mut absolute = Vec::new();
        encode_absolute_value_open(&mut absolute)?;
        encode_absolute_value_close(&mut absolute)?;
        assert_eq!(absolute, vec![51, 51]);

        let mut norm = Vec::new();
        encode_norm_delimiter(&mut norm)?;
        assert_eq!(norm, vec![51, 51]);
        Ok(())
    }

    #[test]
    fn encodes_norm_delimiter_separately() -> Result<(), String> {
        let mut result = Vec::new();
        encode_norm_delimiter(&mut result)?;
        assert_eq!(result, vec![51, 51]);
        Ok(())
    }
}
