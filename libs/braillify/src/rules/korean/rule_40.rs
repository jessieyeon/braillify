//! 제40항 — 숫자는 수표 ⠼(60)을 앞세워 다음과 같이 적는다.
//!
//! 제43항 — 숫자 사이에 마침표, 쉼표, 연결표가 붙어 나올 때에는 뒤의 숫자에 수표를 적지 않는다.
//!
//! The number indicator ⠼ (code 60) is prepended before the first digit in a number sequence.
//! Within a sequence, if separated by . or , the indicator is NOT repeated.
//!
//! Digit encoding is delegated to `number::encode_number()`.
//!
//! Reference: 2024 Korean Braille Standard, Chapter 5, Section 11, Articles 40, 43

use crate::char_struct::CharType;
use crate::number;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::korean::rule_69::parse_numeric_ascii_unit_prefix;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META_40: RuleMeta = RuleMeta {
    section: "40",
    subsection: None,
    name: "number_prefix",
    standard_ref: "2024 Korean Braille Standard, Ch.5 Sec.11 Art.40",
    description: "Number indicator ⠼ (60) before first digit in number sequence",
};

/// Number indicator (수표).
pub const NUMBER_INDICATOR: u8 = 60; // ⠼

/// Encode a digit character to braille.
#[cfg(test)]
fn encode_digit(ch: char) -> Result<u8, String> {
    number::encode_number(ch)
}

/// Plugin struct for the rule engine.
///
/// Handles number encoding with prefix indicator (제40항, 제43항).
/// Emits 수표 ⠼ before the first digit in a sequence. Subsequent digits
/// after continuation characters (`.`, `,`) do not repeat the prefix.
/// Fraction detection and complex numeric formatting are separate concerns.
pub struct Rule40;

impl BrailleRule for Rule40 {
    fn meta(&self) -> &'static RuleMeta {
        &META_40
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::Number(_))
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let CharType::Number(c) = ctx.char_type else {
            return Ok(RuleResult::Skip);
        };

        if ctx.index == 0
            && let Some((numeric, unit, consumed)) = parse_numeric_ascii_unit_prefix(ctx.word_chars)
        {
            let mut encoded = crate::encode(&numeric)?;
            encoded.extend(unit);
            ctx.emit_slice(&encoded);
            ctx.state.is_number = false;
            *ctx.skip_count = consumed.saturating_sub(1);
            return Ok(RuleResult::Consumed);
        }

        if !ctx.state.is_number {
            // 제43항: skip prefix after continuation characters (. or ,)
            let needs_prefix = ctx
                .prev_char()
                .is_none_or(|prev| !is_number_continuation(prev));
            if needs_prefix {
                ctx.emit(NUMBER_INDICATOR);
                // 제61항: apostrophe/right single quote before number emits ⠄ after 수표
                if ctx
                    .prev_char()
                    .is_some_and(|prev| prev == '\'' || prev == '\u{2019}')
                {
                    ctx.emit(4);
                }
            }
            ctx.state.is_number = true;
        }
        let digit = number::encode_number(*c)?;
        ctx.emit(digit);
        Ok(RuleResult::Consumed)
    }
}

/// Check if the previous character is a continuation character (. or ,)
/// that should suppress the number indicator on the next digit.
pub fn is_number_continuation(prev: char) -> bool {
    prev == '.' || prev == ','
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unicode::decode_unicode;

    #[test]
    fn encodes_digits() {
        assert_eq!(encode_digit('1').unwrap(), decode_unicode('⠁'));
        assert_eq!(encode_digit('0').unwrap(), decode_unicode('⠚'));
        assert_eq!(encode_digit('9').unwrap(), decode_unicode('⠊'));
    }

    #[test]
    fn invalid_digit() {
        assert!(encode_digit('a').is_err());
    }

    #[test]
    fn continuation_chars() {
        assert!(is_number_continuation('.'));
        assert!(is_number_continuation(','));
        assert!(!is_number_continuation(' '));
        assert!(!is_number_continuation('-'));
    }

    #[test]
    fn golden_test_alignment() {
        let cases = vec![("1", "⠼⠁"), ("10", "⠼⠁⠚"), ("0.48", "⠼⠚⠲⠙⠓")];
        for (input, expected) in cases {
            let result = crate::encode_to_unicode(input).unwrap();
            assert_eq!(
                result, expected,
                "Rule 40 golden test failed for: {}",
                input
            );
        }
    }
}
