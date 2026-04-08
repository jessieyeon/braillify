//! 수학 제64항 — 모자 기호 (p̂).
//!
//! Hat/circumflex combining U+0302 notation.

pub fn is_hat_notation(c: char) -> bool {
    c == '\u{0302}'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_hat_notation() {
        assert!(is_hat_notation('\u{0302}'));
    }
}
