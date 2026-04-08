//! 수학 제61항 — 논리 기호.
//!
//! ¬, ∧, ∨, →, ⇒, ∀, ∃ 기호를 단축표와 이항 간격 규칙으로 처리한다.

// Prepared for future direct encoder dispatch integration.
pub fn is_logic_symbol(c: char) -> bool {
    matches!(
        c,
        '\u{00AC}'
            | '\u{2192}'
            | '\u{21D2}'
            | '\u{2194}'
            | '\u{21D4}'
            | '\u{21C4}'
            | '\u{2227}'
            | '\u{2228}'
            | '\u{22BB}'
            | '\u{2193}'
            | '\u{2191}'
            | '\u{2200}'
            | '\u{2203}'
            | '\u{2204}'
    )
}
