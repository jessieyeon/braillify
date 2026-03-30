//! 수학 제60항 — 집합 기호.
//!
//! ∈, ∉, ∪, ∩, ⊂, ⊃, ∅ 계열 기호를 단축표로 인코딩한다.

// Prepared for future direct encoder dispatch integration.
pub fn is_set_symbol(c: char) -> bool {
    matches!(
        c,
        '\u{2208}' | '\u{2209}' | '\u{222A}' | '\u{2229}' | '\u{2282}' | '\u{2283}' | '\u{2205}'
    )
}
