use crate::english::encode_english;
use crate::number::encode_number;
use crate::rules::context::EncoderState;
use crate::rules::token::Token;
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};
use crate::unicode::decode_unicode;

pub struct DigitalNotationRule;

impl TokenRule for DigitalNotationRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::ModeEntry
    }

    fn priority(&self) -> u16 {
        1
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        _state: &mut EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        let Some(Token::Word(word)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };

        if !has_digital_signature(word.text.as_ref()) {
            return Ok(TokenAction::Noop);
        }

        Ok(TokenAction::Replace(Token::PreEncoded(
            encode_digital_word(word.text.as_ref())?,
        )))
    }
}

fn has_digital_signature(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphanumeric())
        && (text.contains("//") || text.contains('@') || text.contains('#') || text.contains('_'))
}

fn encode_digital_word(text: &str) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    let mut english_started = false;

    let prefix_len = chars
        .iter()
        .take_while(|ch| {
            ch.is_ascii_alphanumeric() || matches!(**ch, '/' | '#' | '@' | '.' | ':' | '_')
        })
        .count();

    let digital_chars = &chars[..prefix_len];

    while i < digital_chars.len() {
        let ch = digital_chars[i];
        if ch.is_ascii_alphabetic() {
            let start = i;
            i += 1;
            while i < digital_chars.len() && digital_chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            if !english_started {
                result.push(decode_unicode('⠴'));
                english_started = true;
            }
            encode_digital_english_segment(&digital_chars[start..i], &mut result)?;
            continue;
        }

        if ch.is_ascii_digit() {
            if i > 0 && digital_chars[i - 1].is_ascii_alphabetic() {
                result.push(decode_unicode('⠐'));
                result.push(0);
            }
            result.push(decode_unicode('⠼'));
            while i < digital_chars.len() && digital_chars[i].is_ascii_digit() {
                result.push(encode_number(digital_chars[i])?);
                i += 1;
            }
            continue;
        }

        match ch {
            '/' => {
                result.push(decode_unicode('⠸'));
                result.push(decode_unicode('⠌'));
            }
            '#' => {
                result.push(decode_unicode('⠸'));
                result.push(decode_unicode('⠹'));
            }
            '@' => {
                result.push(decode_unicode('⠈'));
                result.push(decode_unicode('⠁'));
            }
            '.' => result.push(decode_unicode('⠲')),
            ':' => result.push(decode_unicode('⠒')),
            '_' => {
                result.push(decode_unicode('⠨'));
                result.push(decode_unicode('⠤'));
            }
            _ => return Err(format!("unsupported digital notation character: {ch}")),
        }
        i += 1;
    }

    if prefix_len == chars.len() && chars.last().is_some_and(|ch| ch.is_ascii_alphabetic()) {
        result.push(decode_unicode('⠲'));
    }

    if prefix_len < chars.len() {
        if digital_chars
            .last()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
        {
            result.push(decode_unicode('⠲'));
        }
        let remainder: String = chars[prefix_len..].iter().collect();
        result.extend(crate::encode(&remainder)?);
    }

    Ok(result)
}

fn encode_digital_english_segment(chars: &[char], result: &mut Vec<u8>) -> Result<(), String> {
    let mut i = 0usize;
    while i < chars.len() {
        let rest: String = chars[i..].iter().collect();
        if rest.starts_with("ment") {
            result.push(decode_unicode('⠰'));
            result.push(decode_unicode('⠞'));
            i += 4;
            continue;
        }
        if rest.starts_with("ing") {
            result.push(decode_unicode('⠬'));
            i += 3;
            continue;
        }
        if rest.starts_with("con") {
            result.push(decode_unicode('⠒'));
            i += 3;
            continue;
        }
        if rest.starts_with("ea") && chars.get(i + 2).is_some_and(|ch| ch.is_ascii_alphabetic()) {
            result.push(decode_unicode('⠂'));
            i += 2;
            continue;
        }
        if rest.starts_with("en") {
            result.push(decode_unicode('⠢'));
            i += 2;
            continue;
        }
        if rest.starts_with("ar") {
            result.push(decode_unicode('⠜'));
            i += 2;
            continue;
        }
        if rest.starts_with("er") {
            result.push(decode_unicode('⠻'));
            i += 2;
            continue;
        }
        if rest.starts_with("ou") {
            result.push(decode_unicode('⠳'));
            i += 2;
            continue;
        }
        if rest.starts_with("ow") {
            result.push(decode_unicode('⠪'));
            i += 2;
            continue;
        }
        if rest.starts_with("th") {
            result.push(decode_unicode('⠹'));
            i += 2;
            continue;
        }
        result.push(encode_english(chars[i])?);
        i += 1;
    }
    Ok(())
}
