use std::borrow::Cow;

use crate::rules::token::{Token, WordToken};
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};
use crate::unicode::decode_unicode;

pub struct EmphasisRingRule;

fn is_ring_mark_only(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|ch| ch == '\u{030A}')
}

fn trim_ring_marks(text: &str) -> String {
    text.chars().filter(|ch| *ch != '\u{030A}').collect()
}

impl TokenRule for EmphasisRingRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::Normalization
    }

    fn priority(&self) -> u16 {
        120
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        _state: &mut crate::rules::context::EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        match tokens.get(index) {
            Some(Token::Word(word)) => {
                let text = word.text.as_ref();

                if is_ring_mark_only(text) {
                    return Ok(TokenAction::ReplaceMany(vec![]));
                }

                if !text.contains('\u{030A}') {
                    return Ok(TokenAction::Noop);
                }

                let trimmed = trim_ring_marks(text);
                if trimmed.is_empty() {
                    return Ok(TokenAction::ReplaceMany(vec![]));
                }

                let trimmed_chars: Vec<char> = trimmed.chars().collect();
                Ok(TokenAction::ReplaceMany(vec![
                    Token::PreEncoded(vec![decode_unicode('⠠'), decode_unicode('⠤')]),
                    Token::Word(WordToken {
                        text: Cow::Owned(trimmed),
                        chars: trimmed_chars.clone(),
                        meta: crate::rules::token::WordMeta::from_chars(&trimmed_chars),
                    }),
                    Token::PreEncoded(vec![decode_unicode('⠤'), decode_unicode('⠄')]),
                ]))
            }
            Some(Token::Space(_)) => {
                let prev_word = index
                    .checked_sub(1)
                    .and_then(|i| tokens.get(i))
                    .and_then(|t| match t {
                        Token::Word(w) => Some(w.text.as_ref()),
                        _ => None,
                    });
                let next_word = tokens.get(index + 1).and_then(|t| match t {
                    Token::Word(w) => Some(w.text.as_ref()),
                    _ => None,
                });

                // Remove spacing around standalone combining-ring words.
                if prev_word.is_some_and(is_ring_mark_only)
                    || next_word.is_some_and(is_ring_mark_only)
                {
                    return Ok(TokenAction::ReplaceMany(vec![]));
                }

                // Close emphasis immediately before the next real word.
                if prev_word.is_some_and(|w| w.contains('\u{030A}') || is_ring_mark_only(w))
                    && next_word.is_some_and(|w| !is_ring_mark_only(w))
                {
                    return Ok(TokenAction::Replace(Token::PreEncoded(vec![
                        decode_unicode('⠤'),
                        decode_unicode('⠄'),
                    ])));
                }

                Ok(TokenAction::Noop)
            }
            _ => Ok(TokenAction::Noop),
        }
    }
}
