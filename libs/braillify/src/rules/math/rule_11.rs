//! 수학 제11항 — 수식이 포함된 문장.
//!
//! 한글 문장 안에 수식을 삽입할 때, 수식 구간은 점자 빈칸(⠀, U+2800)을
//! 두 칸(`⠀⠀`) 사용해 경계를 나타내는 전처리 규칙을 따른다.

pub fn is_math_sentence_delimiter(c: char) -> bool {
    c == '\u{2800}'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_math_sentence_delimiter_pair(left: char, right: char) -> bool {
        is_math_sentence_delimiter(left) && is_math_sentence_delimiter(right)
    }

    #[test]
    fn detects_math_sentence_delimiter() {
        assert!(is_math_sentence_delimiter('\u{2800}'));
    }

    #[test]
    fn detects_math_sentence_delimiter_pair() {
        assert!(is_math_sentence_delimiter_pair('\u{2800}', '\u{2800}'));
        assert!(!is_math_sentence_delimiter_pair('a', '\u{2800}'));
        assert!(!is_math_sentence_delimiter_pair('\u{2800}', 'b'));
    }
}
