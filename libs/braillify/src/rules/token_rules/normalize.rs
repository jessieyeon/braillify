use std::borrow::Cow;

use crate::rules::token::{Token, WordToken};
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};

pub struct NormalizeEllipsis;

impl TokenRule for NormalizeEllipsis {
    fn phase(&self) -> TokenPhase {
        TokenPhase::Normalization
    }

    fn priority(&self) -> u16 {
        100
    }

    fn apply<'a>(&self, tokens: &[Token<'a>], index: usize, _state: &mut crate::rules::context::EncoderState) -> Result<TokenAction<'a>, String> {
        let Some(Token::Word(word)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };

        let has_literal_quote_context = word.text.contains('‘') || word.text.contains('’');
        let normalized = if has_literal_quote_context { word.text.to_string() } else { word.text.replace("......", "...").replace("……", "…") };
        if normalized == word.text {
            return Ok(TokenAction::Noop);
        }

        let chars: Vec<char> = normalized.chars().collect();
        Ok(TokenAction::Replace(Token::Word(WordToken { text: Cow::Owned(normalized), chars: chars.clone(), meta: crate::rules::token::WordMeta::from_chars(&chars) })))
    }
}
