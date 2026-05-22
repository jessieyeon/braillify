use crate::fraction;
use crate::rules::token::{FractionToken, Token};
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};

pub struct LatexFractionRule;

impl TokenRule for LatexFractionRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::FractionDetection
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

        let word_text = word.text.as_ref();
        if !(word_text.starts_with('$') && word_text.ends_with('$')) {
            return Ok(TokenAction::Noop);
        }

        let Some((whole, numerator, denominator)) = fraction::parse_latex_fraction(word_text)
        else {
            return Ok(TokenAction::Noop);
        };

        Ok(TokenAction::Replace(Token::Fraction(FractionToken {
            whole,
            numerator,
            denominator,
        })))
    }
}
