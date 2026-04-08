//! 제64항 — 둘러싼 문자(원문자/동그라미 문자).
//!
//! Handles enclosed Unicode forms that semantically represent an existing
//! number, standalone jamo, syllable, or Latin letter.

use crate::char_struct::CharType;
use crate::english;
use crate::korean_part;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::korean::rule_8::ONTAB;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta {
    section: "64",
    subsection: None,
    name: "enclosed_symbols",
    standard_ref: "2024 Korean Braille Standard, Ch.6 Art.64",
    description: "Encode enclosed/circled numbers, jamo, syllables, and latin letters",
};

const CIRCLE: u8 = 54; // ⠶
const LETTER_MARKER: u8 = 52; // ⠴
const NUMBER_MARKER: u8 = 60; // ⠼

const CIRCLED_SYLLABLES: &[(char, char)] = &[
    ('㉮', '가'),
    ('㉯', '나'),
    ('㉰', '다'),
    ('㉱', '라'),
    ('㉲', '마'),
    ('㉳', '바'),
    ('㉴', '사'),
    ('㉵', '아'),
    ('㉶', '자'),
    ('㉷', '차'),
    ('㉸', '카'),
    ('㉹', '타'),
    ('㉺', '파'),
    ('㉻', '하'),
];

const CIRCLED_JAMO: &[(char, char)] = &[
    ('㉠', 'ㄱ'),
    ('㉡', 'ㄴ'),
    ('㉢', 'ㄷ'),
    ('㉣', 'ㄹ'),
    ('㉤', 'ㅁ'),
    ('㉥', 'ㅂ'),
    ('㉦', 'ㅅ'),
    ('㉧', 'ㅇ'),
    ('㉨', 'ㅈ'),
    ('㉩', 'ㅊ'),
    ('㉪', 'ㅋ'),
    ('㉫', 'ㅌ'),
    ('㉬', 'ㅍ'),
    ('㉭', 'ㅎ'),
];

pub fn is_enclosed_symbol(c: char) -> bool {
    matches!(c, '①'..='⑳' | 'ⓐ'..='ⓩ')
        || CIRCLED_SYLLABLES.iter().any(|(enclosed, _)| *enclosed == c)
        || CIRCLED_JAMO.iter().any(|(enclosed, _)| *enclosed == c)
}

fn encode_number_string(digits: &str) -> Result<Vec<u8>, String> {
    let mut result = vec![NUMBER_MARKER];
    for digit in digits.chars() {
        result.push(encode_enclosed_digit(digit)?);
    }
    Ok(result)
}

fn encode_enclosed_digit(digit: char) -> Result<u8, String> {
    match digit {
        '1' => Ok(2),
        '2' => Ok(6),
        '3' => Ok(18),
        '4' => Ok(50),
        '5' => Ok(34),
        '6' => Ok(22),
        '7' => Ok(54),
        '8' => Ok(38),
        '9' => Ok(20),
        '0' => Ok(52),
        _ => Err("Invalid enclosed number digit".to_string()),
    }
}

fn wrap_circle(inner: Vec<u8>) -> Vec<u8> {
    let mut result = Vec::with_capacity(inner.len() + 2);
    result.push(CIRCLE);
    result.extend(inner);
    result.push(CIRCLE);
    result
}

pub fn encode_enclosed_symbol(c: char) -> Result<Vec<u8>, String> {
    if ('①'..='⑳').contains(&c) {
        let value = (c as u32) - ('①' as u32) + 1;
        return encode_number_string(&value.to_string());
    }

    if ('ⓐ'..='ⓩ').contains(&c) {
        let letter = char::from_u32((c as u32) - ('ⓐ' as u32) + ('a' as u32))
            .ok_or_else(|| "Invalid enclosed latin letter".to_string())?;
        return Ok(wrap_circle(vec![
            LETTER_MARKER,
            english::encode_english(letter)?,
        ]));
    }

    if let Some((_, syllable)) = CIRCLED_SYLLABLES
        .iter()
        .find(|(enclosed, _)| *enclosed == c)
    {
        return Ok(wrap_circle(crate::encode(&syllable.to_string())?));
    }

    if let Some((_, jamo)) = CIRCLED_JAMO.iter().find(|(enclosed, _)| *enclosed == c) {
        let mut inner = vec![ONTAB];
        inner.extend_from_slice(korean_part::encode_korean_part(*jamo)?);
        return Ok(wrap_circle(inner));
    }

    Err("Invalid enclosed symbol character".to_string())
}

pub struct Rule64;

impl BrailleRule for Rule64 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        350
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::Symbol(c) if is_enclosed_symbol(*c))
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let CharType::Symbol(c) = ctx.char_type else {
            return Ok(RuleResult::Skip);
        };

        let encoded = encode_enclosed_symbol(*c)?;
        ctx.emit_slice(&encoded);
        Ok(RuleResult::Consumed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_unicode(bytes: &[u8]) -> String {
        bytes
            .iter()
            .copied()
            .map(crate::unicode::encode_unicode)
            .collect()
    }

    #[test]
    fn encodes_circled_numbers() {
        assert_eq!(to_unicode(&encode_enclosed_symbol('①').unwrap()), "⠼⠂");
        assert_eq!(to_unicode(&encode_enclosed_symbol('⑳').unwrap()), "⠼⠆⠴");
    }

    #[test]
    fn encodes_circled_jamo() {
        assert_eq!(to_unicode(&encode_enclosed_symbol('㉠').unwrap()), "⠶⠿⠁⠶");
    }

    #[test]
    fn encodes_circled_latin() {
        assert_eq!(to_unicode(&encode_enclosed_symbol('ⓐ').unwrap()), "⠶⠴⠁⠶");
    }

    #[test]
    fn encodes_circled_syllable() {
        assert_eq!(to_unicode(&encode_enclosed_symbol('㉮').unwrap()), "⠶⠫⠶");
    }

    #[test]
    fn detects_supported_chars() {
        assert!(is_enclosed_symbol('①'));
        assert!(is_enclosed_symbol('㉠'));
        assert!(is_enclosed_symbol('㉮'));
        assert!(is_enclosed_symbol('ⓩ'));
        assert!(!is_enclosed_symbol('가'));
    }
}
