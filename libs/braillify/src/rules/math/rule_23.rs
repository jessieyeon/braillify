//! 수학 제23항 — 윗줄 기호(오버라인).
//!
//! 변수 뒤에 결합형 오버라인(U+0304/U+0305)이 오는 표기(예: `x̄`)를 처리한다.
//! 점자 내부표기에서는 오버라인 표지를 `@c`(코드 8, 9 / ⠈⠉)로 둔다.

pub fn is_overline_mark(c: char) -> bool {
    matches!(c, '\u{0304}' | '\u{0305}')
}

pub fn encode_overline(result: &mut Vec<u8>) -> Result<(), String> {
    result.extend_from_slice(&[8, 9]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_overline_marker() -> Result<(), String> {
        let mut result = Vec::new();
        encode_overline(&mut result)?;
        assert_eq!(result, vec![8, 9]);
        assert!(is_overline_mark('\u{0304}'));
        Ok(())
    }
}
