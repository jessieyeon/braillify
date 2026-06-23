//! Parse English source text into a flat `EnglishToken` stream.

use super::token::{EnglishToken, Typeform};

/// A word letter: ASCII alphabetic or a supported accented letter (§4.2), so
/// `crème` tokenizes as one word rather than `cr` + `è` + `me`.
fn is_word_letter(c: char) -> bool {
    c.is_ascii_alphabetic() || super::rule_4::is_accented(c)
}

/// Tokenize `text`: runs of word letters become `Word`, runs of ASCII digits
/// become `Number`, a single space becomes `Space`, anything else `Symbol`.
pub fn parse_english(text: &str) -> Vec<EnglishToken> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // §9: a Mathematical-Alphanumeric styled letter is its own token.
        if let Some((base, form)) = super::rule_9::decode_styled(c) {
            tokens.push(EnglishToken::Styled(base, form));
            i += 1;
            continue;
        }
        // §9: any character (letter, digit, or symbol) immediately followed by a
        // combining low line (U+0332) is underlined → a Styled token that also
        // ends any current word. A space is excluded (an underlined space opens a
        // §9.x passage, handled separately) as is a lone combining mark, so a
        // styled digit (`3̲4̲`) or symbol (`.̲`, `%̲`) is captured alongside letters.
        if c != ' ' && c != '\u{0332}' && chars.get(i + 1) == Some(&'\u{0332}') {
            tokens.push(EnglishToken::Styled(c, Typeform::Underline));
            i += 2;
            continue;
        }
        if is_word_letter(c) {
            let start = i;
            // A styled letter is not a word letter (so it stops the run on its
            // own); stop one early before a letter that an upcoming U+0332 will
            // underline so it becomes its own Styled token.
            while i < chars.len()
                && is_word_letter(chars[i])
                && chars.get(i + 1) != Some(&'\u{0332}')
            {
                i += 1;
            }
            tokens.push(EnglishToken::Word(chars[start..i].to_vec()));
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            tokens.push(EnglishToken::Number(chars[start..i].to_vec()));
        } else if c == ' ' {
            tokens.push(EnglishToken::Space);
            i += 1;
        } else {
            tokens.push(EnglishToken::Symbol(c));
            i += 1;
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_words_spaces_numbers_symbols() {
        let toks = parse_english("ab 12!c");
        assert_eq!(
            toks,
            vec![
                EnglishToken::Word(vec!['a', 'b']),
                EnglishToken::Space,
                EnglishToken::Number(vec!['1', '2']),
                EnglishToken::Symbol('!'),
                EnglishToken::Word(vec!['c']),
            ]
        );
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(parse_english("").is_empty());
    }
}
