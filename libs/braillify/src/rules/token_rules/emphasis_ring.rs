use std::borrow::Cow;

use crate::rules::token::{Token, WordToken};
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};
use crate::unicode::decode_unicode;

pub struct EmphasisRingRule;

/// 드러냄표(제56항)에 쓰이는 결합 부호.
/// - U+030A `◌̊`(combining ring above): 「훈민정음̊」 등 PDF 예시
/// - U+0307 `◌̇`(combining dot above): 한국어 본문에서 강조용으로 쓰이는 결합 부호
///
/// 주의: U+0307은 수학 표기에서 「결합 윗점」(반복 소수, 도함수 등)으로도 사용되므로
/// 단어가 한글을 포함할 때에 한해 강조 마커로 해석한다.
fn is_ring_mark(ch: char) -> bool {
    matches!(ch, '\u{030A}' | '\u{0307}')
}

fn is_ring_mark_only(text: &str) -> bool {
    !text.is_empty() && text.chars().all(is_ring_mark)
}

fn is_emphasis_word(text: &str) -> bool {
    // 텍스트 어딘가에 결합 부호가 있어야 한다.
    if !text.chars().any(is_ring_mark) {
        return false;
    }
    // 결합 부호(U+0307·U+030A)는 NFD 분해로 Latin 단위/기호(Å 등)에도 등장하므로
    // 단어에 한글이 포함된 경우에만 강조 마커로 해석한다. 그렇지 않으면 수학/단위
    // 결합 부호로 보고 통과시킨다.
    text.chars().any(crate::utils::is_korean_char)
}

fn trim_ring_marks(text: &str) -> String {
    text.chars().filter(|ch| !is_ring_mark(*ch)).collect()
}

impl TokenRule for EmphasisRingRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::Normalization
    }

    fn priority(&self) -> u16 {
        120
    }

    fn apply<'a>(&self, tokens: &[Token<'a>], index: usize, _state: &mut crate::rules::context::EncoderState) -> Result<TokenAction<'a>, String> {
        match tokens.get(index) {
            Some(Token::Word(word)) => {
                let text = word.text.as_ref();

                if is_ring_mark_only(text) {
                    return Ok(TokenAction::ReplaceMany(vec![]));
                }

                if !is_emphasis_word(text) {
                    return Ok(TokenAction::Noop);
                }

                let trimmed = trim_ring_marks(text);
                if trimmed.is_empty() {
                    return Ok(TokenAction::ReplaceMany(vec![]));
                }

                let trimmed_chars: Vec<char> = trimmed.chars().collect();
                Ok(TokenAction::ReplaceMany(vec![Token::PreEncoded(vec![decode_unicode('⠠'), decode_unicode('⠤')]), Token::Word(WordToken { text: Cow::Owned(trimmed), chars: trimmed_chars.clone(), meta: crate::rules::token::WordMeta::from_chars(&trimmed_chars) }), Token::PreEncoded(vec![decode_unicode('⠤'), decode_unicode('⠄')])]))
            }
            Some(Token::Space(_)) => {
                let prev_word = index.checked_sub(1).and_then(|i| tokens.get(i)).and_then(|t| match t {
                    Token::Word(w) => Some(w.text.as_ref()),
                    _ => None,
                });
                let next_word = tokens.get(index + 1).and_then(|t| match t {
                    Token::Word(w) => Some(w.text.as_ref()),
                    _ => None,
                });

                // 직전 토큰이 강조 종료 마커(⠤⠄)인 경우: 강조 끝과 다음 단어 사이의
                // 분리용 공백은 종료 마커가 이미 흡수했으므로 제거한다(rule_49 예시
                // 「훈민정음̊」 + 이다 → 종료 후 공백 없이 「이다」가 이어진다).
                let prev_is_emphasis_close = index.checked_sub(1).and_then(|i| tokens.get(i)).is_some_and(|t| match t {
                    Token::PreEncoded(bytes) => bytes.as_slice() == [decode_unicode('⠤'), decode_unicode('⠄')].as_slice(),
                    _ => false,
                });
                if prev_is_emphasis_close && next_word.is_some_and(|w| !is_ring_mark_only(w)) {
                    return Ok(TokenAction::ReplaceMany(vec![]));
                }

                // Remove spacing around standalone combining-emphasis words.
                if prev_word.is_some_and(is_ring_mark_only) || next_word.is_some_and(is_ring_mark_only) {
                    return Ok(TokenAction::ReplaceMany(vec![]));
                }

                // Close emphasis immediately before the next real word.
                if prev_word.is_some_and(|w| is_emphasis_word(w) || is_ring_mark_only(w)) && next_word.is_some_and(|w| !is_ring_mark_only(w)) {
                    return Ok(TokenAction::Replace(Token::PreEncoded(vec![decode_unicode('⠤'), decode_unicode('⠄')])));
                }

                Ok(TokenAction::Noop)
            }
            _ => Ok(TokenAction::Noop),
        }
    }
}
