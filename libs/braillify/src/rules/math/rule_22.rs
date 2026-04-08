//! 수학 제22항 — 근호 표기.
//!
//! 제곱근(√)과 n제곱근 지수 표식(])을 처리한다.

// Prepared for future direct encoder dispatch integration.
pub const NTH_ROOT_INDEX_MARKER: u8 = 59;

// Prepared for future direct encoder dispatch integration.
pub fn is_root_symbol(c: char) -> bool {
    c == '\u{221A}'
}
