//! 수학 제34항 — 관계 표기 (aRb, a~b).
//!
//! 관계 표기에서는 소문자 변수 사이의 대문자 관계 문자(예: `R`)를 사용한다.
//! 이때 `R`은 대문자 표기 규칙에 따라 `,` 접두를 붙여 인코딩한다.

#[cfg(test)]
mod tests {
    fn is_relation_notation(prev: char, middle: char, next: char) -> bool {
        prev.is_ascii_lowercase() && middle.is_ascii_uppercase() && next.is_ascii_lowercase()
    }

    fn encode_relation_uppercase(middle: char, result: &mut Vec<u8>) -> Result<(), String> {
        if !middle.is_ascii_uppercase() {
            return Err("relation symbol must be an uppercase ASCII letter".to_string());
        }

        result.push(32); // , prefix (⠠)
        let code = crate::english::encode_english(middle.to_ascii_lowercase())?;
        result.push(code);
        Ok(())
    }

    #[test]
    fn validates_relation_notation_pattern() {
        assert!(is_relation_notation('a', 'R', 'b'));
        assert!(!is_relation_notation('A', 'R', 'b'));
        assert!(!is_relation_notation('a', 'r', 'b'));
    }

    #[test]
    fn detects_relation_notation_correctly() {
        assert!(is_relation_notation('x', 'S', 'y'));
        assert!(!is_relation_notation('1', 'R', '2'));
        assert!(!is_relation_notation('a', 'b', 'c'));
    }

    #[test]
    fn encodes_uppercase_relation_symbol_with_prefix() -> Result<(), String> {
        let mut result = Vec::new();
        encode_relation_uppercase('R', &mut result)?;
        assert_eq!(result.first().copied(), Some(32));
        Ok(())
    }

    #[test]
    fn encodes_relation_uppercase_correctly() -> Result<(), String> {
        let mut result = Vec::new();
        encode_relation_uppercase('X', &mut result)?;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], 32); // uppercase prefix
        Ok(())
    }
}
