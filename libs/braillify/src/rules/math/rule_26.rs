//! 수학 제26항 — (예약).
//!
//! 본 항목은 2024 한국 점자 수학 규정에서 예약되어 있으며,
//! 현재 구현 단계에서는 규칙 확장을 위한 자리만 유지한다.

pub fn is_reserved_rule_26() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_reserved_marker() {
        assert!(is_reserved_rule_26());
    }
}
