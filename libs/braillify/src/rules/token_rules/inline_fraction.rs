use once_cell::sync::Lazy;
use regex::Regex;

use crate::fraction;
use crate::rules::token::{Token, WordMeta, WordToken};
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};

static FRACTION_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\d+)\/(\d+)").expect("Failed to compile FRACTION_REGEX"));

pub struct InlineFractionRule;

impl TokenRule for InlineFractionRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::FractionDetection
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
        let Some(Token::Word(word)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };

        let chars = &word.chars;
        let word_len = chars.len();

        for (i, ch) in chars.iter().enumerate() {
            if !ch.is_ascii_digit() {
                continue;
            }

            let remaining: String = chars[i..].iter().collect();
            let Some(captures) = FRACTION_REGEX.captures(&remaining) else {
                continue;
            };

            let numerator = &captures[1];
            let denominator = &captures[2];
            let match_len = captures[0].len();
            let k = i + match_len;
            let is_date_or_range = (numerator.len() > 1 || denominator.len() > 1)
                || (k < word_len && chars[k] == '/')
                || (k < word_len && chars[k] == '~');

            if is_date_or_range {
                continue;
            }

            let mut replacement = Vec::new();

            if i > 0 {
                let prefix: String = chars[..i].iter().collect();
                let prefix_chars: Vec<char> = prefix.chars().collect();
                replacement.push(Token::Word(WordToken {
                    text: std::borrow::Cow::Owned(prefix),
                    chars: prefix_chars.clone(),
                    meta: WordMeta::from_chars(&prefix_chars),
                }));
            }

            replacement.push(Token::PreEncoded(fraction::encode_fraction_in_context(
                numerator,
                denominator,
            )?));

            if k < word_len {
                let suffix: String = chars[k..].iter().collect();
                let suffix_chars: Vec<char> = suffix.chars().collect();
                replacement.push(Token::Word(WordToken {
                    text: std::borrow::Cow::Owned(suffix),
                    chars: suffix_chars.clone(),
                    meta: WordMeta::from_chars(&suffix_chars),
                }));
            }

            return Ok(TokenAction::ReplaceMany(replacement));
        }

        Ok(TokenAction::Noop)
    }
}
