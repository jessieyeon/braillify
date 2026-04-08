//! 수학 제9항 — 순환소수 표기.
//!
//! 결합점(◌̇, U+0307)을 수문자 문맥에서 유지하도록 처리한다.

pub fn is_repeating_decimal_mark(c: char) -> bool {
    c == '\u{0307}'
}
