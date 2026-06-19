//! Parse English source text into a flat `EnglishToken` stream.

use super::token::EnglishToken;

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
        if is_word_letter(c) {
            let start = i;
            while i < chars.len() && is_word_letter(chars[i]) {
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
