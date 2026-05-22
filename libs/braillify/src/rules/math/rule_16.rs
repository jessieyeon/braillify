//! 수학 제16항 — 진법 표기 (아래 괄호 첨자).
//!
//! 진법 표기는 `수 + 아래첨자 괄호 진법` 패턴(예: `1010₍₂₎`, `11₍₁₀₎`)을 따른다.

pub fn is_base_notation_subscript(c: char) -> bool {
    is_subscript_parenthesis(c) || is_subscript_digit(c)
}

pub fn is_subscript_parenthesis(c: char) -> bool {
    matches!(c, '\u{208D}' | '\u{208E}')
}

pub fn is_subscript_digit(c: char) -> bool {
    matches!(c, '\u{2080}' | '\u{2081}' | '\u{2082}' | '\u{2083}' | '\u{2084}' | '\u{2085}' | '\u{2086}' | '\u{2087}' | '\u{2088}' | '\u{2089}')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalize_subscript_digit(c: char) -> Option<char> {
        match c {
            '\u{2080}' => Some('0'),
            '\u{2081}' => Some('1'),
            '\u{2082}' => Some('2'),
            '\u{2083}' => Some('3'),
            '\u{2084}' => Some('4'),
            '\u{2085}' => Some('5'),
            '\u{2086}' => Some('6'),
            '\u{2087}' => Some('7'),
            '\u{2088}' => Some('8'),
            '\u{2089}' => Some('9'),
            _ => None,
        }
    }

    #[test]
    fn detects_base_notation_subscript_chars() {
        assert!(is_base_notation_subscript('\u{208D}'));
        assert!(is_base_notation_subscript('\u{2082}'));
    }

    #[test]
    fn normalizes_subscript_digits() {
        assert_eq!(normalize_subscript_digit('\u{2080}'), Some('0'));
        assert_eq!(normalize_subscript_digit('\u{2082}'), Some('2'));
        assert_eq!(normalize_subscript_digit('\u{2089}'), Some('9'));
        assert_eq!(normalize_subscript_digit('a'), None);
    }
}
