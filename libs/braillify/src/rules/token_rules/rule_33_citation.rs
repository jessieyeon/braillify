//! PDF 한국어 제33항 — 학술 인용 형식 (저자, 연도a, 연도b; ...).
//!
//! `1998a,`, `1998b;` 같이 4자리 연도 + 단일 알파벳 suffix + 구두점 토큰을 감지해
//! 영어 모드 점역(⠴ begin / ⠰ continue + letter + 영어 구두점)으로 emit한다.
//! 단어 시작 이외 위치의 영어 모드 진입은 char-level emit에서 미지원이므로 token-level
//! 으로 처리한다.

use crate::english::encode_english;
use crate::number::encode_number;
use crate::rules::context::EncoderState;
use crate::rules::token::Token;
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};
use crate::unicode::decode_unicode;

pub struct Rule33CitationYearSuffixRule;

/// Rule33가 emit한 PreEncoded인지 구조적으로 확인한다.
/// Pattern: `⠼(60)` + 4 digit bytes + (`⠴`(52) | `⠰`(48)) + letter byte + suffix.
fn is_rule33_emission(bytes: &[u8]) -> bool {
    // 최소 길이: 60 + 4 digits + marker + letter = 7. + suffix (1 or 2 bytes).
    if bytes.len() < 8 || bytes.len() > 9 {
        return false;
    }
    if bytes[0] != 60 {
        return false;
    }
    // bytes[1..5]는 number digits — `encode_number`가 emit하는 정확한 셀 값만 허용한다.
    // NUMBER_MAP 값: {1,3,9,10,11,17,19,25,26,27}
    let is_digit_byte = |b: &u8| matches!(*b, 1 | 3 | 9 | 10 | 11 | 17 | 19 | 25 | 26 | 27);
    if !bytes[1..5].iter().all(is_digit_byte) {
        return false;
    }
    // bytes[5] is marker: 48 (⠰ continue) or 52 (⠴ begin)
    if !matches!(bytes[5], 48 | 52) {
        return false;
    }
    // bytes[6] is encoded letter (1..=63)
    if !(1..=63).contains(&bytes[6]) {
        return false;
    }
    // suffix: bytes[7..]
    match &bytes[7..] {
        [2] => true,     // ⠂ comma
        [50] => true,    // ⠲ period
        [48, 6] => true, // ⠰⠆ semicolon
        _ => false,
    }
}

fn match_year_suffix(text: &str) -> Option<(&str, char, char)> {
    let chars: Vec<char> = text.chars().collect();
    // 정확히 6자: 4 digits + 1 lowercase + 1 punctuation (',' or ';' or '.')
    if chars.len() != 6 {
        return None;
    }
    if !chars[..4].iter().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !chars[4].is_ascii_lowercase() {
        return None;
    }
    if !matches!(chars[5], ',' | ';' | '.') {
        return None;
    }
    let year_end = text.char_indices().nth(4).map(|(i, _)| i)?;
    Some((&text[..year_end], chars[4], chars[5]))
}

impl TokenRule for Rule33CitationYearSuffixRule {
    fn phase(&self) -> TokenPhase {
        // Normalization 단계 — 다른 토큰 변환 전에 처리. 토큰 엔진은 Normalization
        // phase에서 Noop 시에도 다음 rule을 시도하므로 안전하다.
        TokenPhase::Normalization
    }

    fn priority(&self) -> u16 {
        // LatexMergeRule(10) 이후, EmphasisRingRule(120)보다 먼저
        50
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
        let text = word.text.as_ref();
        let Some((year_str, letter, punct)) = match_year_suffix(text) else {
            return Ok(TokenAction::Noop);
        };

        // 직전 비공백 토큰이 동일 패턴(연속 인용)인지 확인 → ⠰ (continue) 마커.
        // 아니면 ⠴ (begin) 마커.
        // - 직전이 같은 year-suffix Word이면 continue.
        // - 직전이 Rule33가 emit한 PreEncoded(끝이 영어 모드 종료 마커)이면 continue.
        let prev_is_same_pattern = {
            let mut i = index;
            loop {
                if i == 0 {
                    break false;
                }
                i -= 1;
                match tokens.get(i) {
                    Some(Token::Space(_)) => continue,
                    Some(Token::Word(w)) => {
                        break match_year_suffix(w.text.as_ref()).is_some();
                    }
                    Some(Token::PreEncoded(bytes)) => {
                        // Rule33이 자체 emit한 PreEncoded인지 구조적으로 확인한다.
                        // 패턴: `⠼(60)` + 4 digits(1..=10) + 마커(48|52) + letter(1..=26) + suffix.
                        // suffix: `⠂(2)` | `⠰⠆(48,6)` | `⠲(50)`
                        break is_rule33_emission(bytes);
                    }
                    _ => break false,
                }
            }
        };

        let mut bytes = Vec::new();
        // ⠼ + 연도 숫자
        bytes.push(decode_unicode('⠼'));
        for c in year_str.chars() {
            bytes.push(encode_number(c)?);
        }
        // ⠴ (begin) 또는 ⠰ (continue)
        bytes.push(if prev_is_same_pattern {
            decode_unicode('⠰')
        } else {
            decode_unicode('⠴')
        });
        // letter
        bytes.push(encode_english(letter)?);
        // 구두점 — 영어 모드 inside
        match punct {
            ',' => bytes.push(decode_unicode('⠂')), // 영어 모드 comma
            ';' => {
                bytes.push(decode_unicode('⠰'));
                bytes.push(decode_unicode('⠆'));
            }
            '.' => bytes.push(decode_unicode('⠲')),
            _ => {}
        }

        Ok(TokenAction::Replace(Token::PreEncoded(bytes)))
    }
}
