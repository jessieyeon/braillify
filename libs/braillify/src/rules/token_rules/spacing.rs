use crate::rules::token::Token;
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};

pub struct AsteriskSpacingRule;

fn is_last_word_index(tokens: &[Token], index: usize) -> bool {
    !tokens
        .iter()
        .skip(index + 1)
        .any(|t| matches!(t, Token::Word(_)))
}

impl TokenRule for AsteriskSpacingRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::PostWord
    }

    fn priority(&self) -> u16 {
        400
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        _state: &mut crate::rules::context::EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        let Some(Token::Word(current)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };

        if !is_last_word_index(tokens, index) {
            return Ok(TokenAction::Noop);
        }

        let mut trailing_spaces = 0usize;

        if current.text == "*" || current.text.ends_with('*') {
            trailing_spaces += 1;
        }

        if trailing_spaces == 0 {
            return Ok(TokenAction::Noop);
        }

        let replacement = vec![
            Token::Word(current.clone()),
            Token::PreEncoded(vec![0; trailing_spaces]),
        ];
        Ok(TokenAction::ReplaceMany(replacement))
    }
}
