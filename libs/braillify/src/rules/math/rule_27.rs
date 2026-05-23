//! 수학 제27항 — 약수/배수 관계 기호.
//!
//! `|`(나눔 관계)와 `∤`(나누지 않음)를 처리한다.
//! `|`는 코드 51(⠳), `∤`는 부정 표지 + 51 순서로 인코딩한다.

pub fn is_divisibility_symbol(c: char) -> bool {
    matches!(c, '|' | '\u{2224}')
}

pub fn encode_divisibility(c: char, result: &mut Vec<u8>) -> Result<(), String> {
    match c {
        '|' => {
            result.push(51);
            Ok(())
        }
        '\u{2224}' => {
            result.extend_from_slice(&[24, 51]);
            Ok(())
        }
        _ => Err(format!("unsupported divisibility symbol: {c}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_not_divides_symbol() -> Result<(), String> {
        let mut result = Vec::new();
        encode_divisibility('\u{2224}', &mut result)?;
        assert_eq!(result, vec![24, 51]);
        Ok(())
    }

    #[test]
    fn encodes_divides_symbol() -> Result<(), String> {
        let mut result = Vec::new();
        encode_divisibility('|', &mut result)?;
        assert_eq!(result, vec![51]);
        Ok(())
    }

    #[test]
    fn rejects_unsupported_symbol() {
        let mut result = Vec::new();
        assert!(encode_divisibility('@', &mut result).is_err());
        assert!(!is_divisibility_symbol('a'));
    }
}
