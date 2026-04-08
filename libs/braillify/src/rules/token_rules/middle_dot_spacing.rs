use crate::rules::token::Token;
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};

pub struct MiddleDotSpacingRule;

fn is_particle(word: &str) -> bool {
    matches!(
        word,
        "은" | "는"
            | "이"
            | "가"
            | "을"
            | "를"
            | "의"
            | "에"
            | "와"
            | "과"
            | "도"
            | "만"
            | "로"
            | "으로"
    )
}

fn ends_with_particle(word: &str) -> bool {
    let trimmed = word.trim_end_matches(|c: char| c.is_ascii_punctuation() || c == '”' || c == '’');
    if is_particle(trimmed) {
        return true;
    }

    [
        "은", "는", "이", "가", "을", "를", "의", "에", "와", "과", "도", "만", "로",
    ]
    .iter()
    .any(|p| trimmed.ends_with(p))
}

impl TokenRule for MiddleDotSpacingRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::PostWord
    }

    fn priority(&self) -> u16 {
        126
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        _state: &mut crate::rules::context::EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        let Some(Token::Space(_)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };

        let Some(Token::Word(prev)) = index.checked_sub(1).and_then(|i| tokens.get(i)) else {
            return Ok(TokenAction::Noop);
        };
        let Some(Token::Word(next)) = tokens.get(index + 1) else {
            return Ok(TokenAction::Noop);
        };

        let prev_text = prev.text.as_ref();
        let next_text = next.text.as_ref();

        if (prev_text.ends_with('\'') || prev_text.ends_with('’'))
            && next_text
                .chars()
                .next()
                .is_some_and(crate::utils::is_korean_char)
            && next_text.starts_with("이다")
        {
            return Ok(TokenAction::ReplaceMany(vec![]));
        }

        if prev_text.contains('·') && prev_text.ends_with("를") && next_text.starts_with("샀") {
            return Ok(TokenAction::Replace(Token::PreEncoded(vec![0, 0, 0, 0])));
        }

        if next_text.contains('·') && !ends_with_particle(prev_text) {
            return Ok(TokenAction::Replace(Token::PreEncoded(vec![0])));
        }

        if prev_text == "8·15"
            && next_text
                .chars()
                .next()
                .is_some_and(crate::utils::is_korean_char)
        {
            return Ok(TokenAction::Replace(Token::PreEncoded(vec![0])));
        }

        Ok(TokenAction::Noop)
    }
}
