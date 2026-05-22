use crate::rules::english_shortform::requires_grade1_indicator;
use crate::rules::token::{ModeEvent, Token, WordToken};
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};

pub struct UppercasePassageRule;

fn prev_word<'a>(tokens: &'a [Token<'a>], index: usize) -> Option<&'a WordToken<'a>> {
    tokens[..index].iter().rev().find_map(|t| {
        if let Token::Word(w) = t {
            Some(w)
        } else {
            None
        }
    })
}

/// Return the next two Word tokens after `index`, in order, lazily.
///
/// The caller only needs to know whether the next 1 and 2 upcoming Word
/// tokens exist and whether they look like ASCII passages. We never need
/// the full tail of upcoming words, so avoid materializing a `Vec`. This
/// turns what was previously an O(N²) scan (one full tail-collect per
/// token application) into O(1) amortized lookahead.
fn next_two_words<'a>(
    tokens: &'a [Token<'a>],
    index: usize,
) -> (Option<&'a WordToken<'a>>, Option<&'a WordToken<'a>>) {
    let mut iter = tokens.iter().skip(index + 1).filter_map(|t| {
        if let Token::Word(w) = t {
            Some(w)
        } else {
            None
        }
    });
    let first = iter.next();
    let second = iter.next();
    (first, second)
}

fn is_ascii_word(word: &WordToken) -> bool {
    word.text.chars().all(|c| c.is_ascii_alphabetic())
}

impl TokenRule for UppercasePassageRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::UppercasePassage
    }

    fn priority(&self) -> u16 {
        100
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        state: &mut crate::rules::context::EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        let Some(Token::Word(word)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };

        let mut prefix = Vec::new();
        let mut suffix = Vec::new();

        let (upcoming_first, upcoming_second) = next_two_words(tokens, index);
        let word_len = word.chars.len();
        let ascii_starts_at_beginning = word.meta.starts_with_ascii;

        let needs_inline_entry = state.english_indicator
            && !state.is_english
            && word.meta.has_ascii_alphabetic
            && ascii_starts_at_beginning;

        if word.meta.is_all_uppercase && !state.triple_big_english && ascii_starts_at_beginning {
            if needs_inline_entry {
                let entry = if state.needs_english_continuation {
                    ModeEvent::EnterEnglishContinue
                } else {
                    ModeEvent::EnterEnglish
                };
                prefix.push(Token::Mode(entry));
                state.is_english = true;
                state.needs_english_continuation = false;
            }

            let prev_ascii = prev_word(tokens, index).is_some_and(is_ascii_word);
            let can_start_passage = (!state.has_processed_word || !prev_ascii)
                && upcoming_first.is_some_and(is_ascii_word)
                && upcoming_second.is_some_and(is_ascii_word);

            // UEB §5.7.2 + §10.9: prepend Grade-1 indicator (⠰) when the uppercase
            // letters spell a multi-letter shortform (e.g. CD = "could"). This forces
            // literal letter reading and prevents shortform mis-interpretation.
            let needs_grade1 = requires_grade1_indicator(word.text.as_ref());
            if can_start_passage {
                if needs_grade1 {
                    prefix.push(Token::Mode(ModeEvent::Grade1Indicator));
                }
                prefix.push(Token::Mode(ModeEvent::CapsPassageStart));
                state.triple_big_english = true;
            } else if word_len >= 2 {
                if needs_grade1 {
                    prefix.push(Token::Mode(ModeEvent::Grade1Indicator));
                }
                prefix.push(Token::Mode(ModeEvent::CapsWord));
            }
        }

        let next_is_ascii = upcoming_first.is_some_and(is_ascii_word);
        if state.triple_big_english && !next_is_ascii {
            suffix.push(Token::Mode(ModeEvent::CapsPassageEnd));
            state.triple_big_english = false;
        }

        if !state.has_processed_word {
            state.has_processed_word = true;
        }

        if prefix.is_empty() && suffix.is_empty() {
            return Ok(TokenAction::Noop);
        }

        let mut replacement = Vec::with_capacity(prefix.len() + 1 + suffix.len());
        replacement.extend(prefix);
        replacement.push(Token::Word(word.clone()));
        replacement.extend(suffix);
        Ok(TokenAction::ReplaceMany(replacement))
    }
}
