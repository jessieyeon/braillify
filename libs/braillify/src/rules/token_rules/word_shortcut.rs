use std::borrow::Cow;

use crate::rules::token::{Token, WordMeta, WordToken};
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};
use crate::word_shortcut;

pub struct WordShortcutRule;

impl TokenRule for WordShortcutRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::WordShortcut
    }

    fn priority(&self) -> u16 {
        100
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        _state: &mut crate::rules::context::EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        let Some(Token::Word(word)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };

        let Some((_, code, rest)) = word_shortcut::split_word_shortcut(word.text.as_ref()) else {
            return Ok(TokenAction::Noop);
        };

        if rest.is_empty() {
            return Ok(TokenAction::Replace(Token::PreEncoded(code.to_vec())));
        }

        let rest_chars: Vec<char> = rest.chars().collect();
        Ok(TokenAction::ReplaceMany(vec![
            Token::PreEncoded(code.to_vec()),
            Token::Word(WordToken {
                text: Cow::Owned(rest),
                chars: rest_chars.clone(),
                meta: WordMeta::from_chars(&rest_chars),
            }),
        ]))
    }
}
