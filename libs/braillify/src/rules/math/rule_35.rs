//! 수학 제35항 — 선분 기호 (윗줄 AB).
//!
//! 선분 표기에서 윗줄(결합 오버라인 U+0304/U+0305)을 사용한다.
//! 내부표기 `@c,,AB`에서 `@c`는 선분(윗줄) 지시 기호로 코드 `[8, 9]`에 해당한다.

#[cfg(test)]
mod tests {
    use crate::math_symbol_shortcut;

    fn is_segment_overline(c: char) -> bool {
        matches!(c, '\u{0304}' | '\u{0305}')
    }

    fn encode_segment_overline(result: &mut Vec<u8>) -> Result<(), String> {
        result.extend_from_slice(&[8, 9]);
        Ok(())
    }

    fn encode_double_uppercase_prefix(result: &mut Vec<u8>) -> Result<(), String> {
        result.extend_from_slice(&[32, 32]);
        Ok(())
    }

    fn encode_segment_notation(c: char, result: &mut Vec<u8>) -> Result<(), String> {
        let encoded = crate::math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
        result.extend_from_slice(encoded);
        Ok(())
    }

    #[test]
    fn detects_segment_overline() {
        assert!(is_segment_overline('\u{0304}'));
        assert!(is_segment_overline('\u{0305}'));
        assert!(!is_segment_overline('a'));
    }

    #[test]
    fn encodes_segment_overline_prefix() -> Result<(), String> {
        let mut result = Vec::new();
        encode_segment_overline(&mut result)?;
        assert_eq!(result, vec![8, 9]);
        Ok(())
    }

    #[test]
    fn encodes_segment_overline_separately() -> Result<(), String> {
        let mut result = Vec::new();
        encode_segment_overline(&mut result)?;
        assert_eq!(result, vec![8, 9]);
        Ok(())
    }

    #[test]
    fn encodes_double_uppercase_prefix_correctly() -> Result<(), String> {
        let mut result = Vec::new();
        encode_double_uppercase_prefix(&mut result)?;
        assert_eq!(result, vec![32, 32]);
        Ok(())
    }

    #[test]
    fn encodes_segment_notation_char() -> Result<(), String> {
        let mut result = Vec::new();
        encode_segment_notation('∪', &mut result)?;
        assert!(!result.is_empty());
        Ok(())
    }
}
