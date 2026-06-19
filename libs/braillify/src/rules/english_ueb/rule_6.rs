//! §6 Numeric Mode — numeric indicator `⠼` followed by digits.
//!
//! Per RUEB 2024 §6.1, a number is the numeric indicator `⠼` (dots 3-4-5-6)
//! once, then each digit rendered with the letters a–j: `1→a 2→b … 9→i 0→j`.
//! Numeric mode runs until a space or a non-digit symbol ends it (§6.3).
//!
//! The `⠰` grade-1 indicator needed when a lowercase letter a–j directly
//! follows a number (§6.5) is applied by the document engine, which has the
//! cross-token context.

use crate::english::encode_english;
use crate::unicode::decode_unicode;

/// `⠼` numeric indicator (dots 3-4-5-6).
pub const NUMERIC_INDICATOR: u8 = decode_unicode('⠼');

/// Map a single ASCII digit to its braille cell (the a–j letter cells).
pub fn digit_cell(d: char) -> Option<u8> {
    let letter = match d {
        '1'..='9' => char::from(b'a' + (d as u8 - b'1')),
        '0' => 'j',
        _ => return None,
    };
    encode_english(letter).ok()
}

/// Encode a run of ASCII digits as a UEB number: `⠼` + digit cells.
pub fn encode_number(digits: &[char]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(digits.len() + 1);
    out.push(NUMERIC_INDICATOR);
    for d in digits {
        out.push(digit_cell(*d)?);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case::ninety_five("95", vec![NUMERIC_INDICATOR, decode_unicode('⠊'), decode_unicode('⠑')])]
    #[case::one_zero("10", vec![NUMERIC_INDICATOR, decode_unicode('⠁'), decode_unicode('⠚')])]
    #[case::all_digits("1234567890", vec![
        NUMERIC_INDICATOR,
        decode_unicode('⠁'), decode_unicode('⠃'), decode_unicode('⠉'), decode_unicode('⠙'),
        decode_unicode('⠑'), decode_unicode('⠋'), decode_unicode('⠛'), decode_unicode('⠓'),
        decode_unicode('⠊'), decode_unicode('⠚'),
    ])]
    fn encodes_numbers(#[case] digits: &str, #[case] expected: Vec<u8>) {
        let chars: Vec<char> = digits.chars().collect();
        assert_eq!(encode_number(&chars), Some(expected));
    }
}
