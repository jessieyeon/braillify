use std::borrow::Cow;

use super::context::EncoderState;

pub struct DocumentIR<'a> {
    pub tokens: Vec<Token<'a>>,
    pub state: EncoderState,
}

#[derive(Debug, Clone)]
pub enum Token<'a> {
    Word(WordToken<'a>),
    Space(SpaceKind),
    Fraction(FractionToken),
    Mode(ModeEvent),
    PreEncoded(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct WordToken<'a> {
    pub text: Cow<'a, str>,
    pub chars: Vec<char>,
    pub meta: WordMeta,
}

#[derive(Debug, Clone, Copy)]
pub struct WordMeta {
    pub has_korean: bool,
    pub is_all_uppercase: bool,
    pub starts_with_ascii: bool,
    pub has_ascii_alphabetic: bool,
}

impl WordMeta {
    pub fn from_chars(chars: &[char]) -> WordMeta {
        let mut has_korean = false;
        let mut has_ascii_alphabetic = false;
        let mut ascii_letter_count = 0u16;
        let mut uppercase_count = 0u16;

        for ch in chars {
            let code = *ch as u32;
            if (0xAC00..=0xD7A3).contains(&code) {
                has_korean = true;
            }

            if ch.is_ascii_alphabetic() {
                has_ascii_alphabetic = true;
                ascii_letter_count = ascii_letter_count.saturating_add(1);
                if ch.is_ascii_uppercase() {
                    uppercase_count = uppercase_count.saturating_add(1);
                }
            }
        }

        let starts_with_ascii = chars.first().is_some_and(char::is_ascii_alphabetic);
        let is_all_uppercase = ascii_letter_count >= 2 && ascii_letter_count == uppercase_count;

        WordMeta {
            has_korean,
            is_all_uppercase,
            starts_with_ascii,
            has_ascii_alphabetic,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceKind {
    Regular,
}

#[derive(Debug, Clone)]
pub struct FractionToken {
    pub whole: Option<String>,
    pub numerator: String,
    pub denominator: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeEvent {
    EnterEnglish,
    EnterEnglishContinue,
    CapsWord,
    CapsPassageStart,
    CapsPassageEnd,
}

impl<'a> DocumentIR<'a> {
    pub fn parse(text: &'a str, english_indicator: bool) -> Self {
        let words: Vec<&str> = text.split(' ').filter(|w| !w.is_empty()).collect();
        let mut tokens = Vec::new();

        for (idx, word) in words.iter().enumerate() {
            let chars: Vec<char> = word.chars().collect();
            let meta = WordMeta::from_chars(&chars);
            tokens.push(Token::Word(WordToken {
                text: Cow::Borrowed(word),
                chars,
                meta,
            }));

            if idx < words.len() - 1 {
                tokens.push(Token::Space(SpaceKind::Regular));
            }
        }

        DocumentIR {
            tokens,
            state: EncoderState::new(english_indicator),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_meta_korean_only() {
        let chars: Vec<char> = "안녕".chars().collect();
        let meta = WordMeta::from_chars(&chars);
        assert!(meta.has_korean);
        assert!(!meta.has_ascii_alphabetic);
        assert!(!meta.starts_with_ascii);
        assert!(!meta.is_all_uppercase);
    }

    #[test]
    fn word_meta_english_uppercase() {
        let chars: Vec<char> = "ATM".chars().collect();
        let meta = WordMeta::from_chars(&chars);
        assert!(!meta.has_korean);
        assert!(meta.has_ascii_alphabetic);
        assert!(meta.starts_with_ascii);
        assert!(meta.is_all_uppercase);
    }

    #[test]
    fn word_meta_mixed() {
        let chars: Vec<char> = "A한b".chars().collect();
        let meta = WordMeta::from_chars(&chars);
        assert!(meta.has_korean);
        assert!(meta.has_ascii_alphabetic);
        assert!(meta.starts_with_ascii);
        assert!(!meta.is_all_uppercase);
    }

    #[test]
    fn token_debug_clone_works() {
        let token = Token::Word(WordToken {
            text: Cow::Borrowed("hello"),
            chars: vec!['h', 'e', 'l', 'l', 'o'],
            meta: WordMeta::from_chars(&['h', 'e', 'l', 'l', 'o']),
        });
        let cloned = token.clone();
        assert!(format!("{cloned:?}").contains("Word"));
    }

    #[test]
    fn parse_simple_words() {
        let ir = DocumentIR::parse("hello world", false);
        assert_eq!(ir.tokens.len(), 3);

        match &ir.tokens[0] {
            Token::Word(w) => assert_eq!(w.text, "hello"),
            _ => panic!("expected first token to be word"),
        }
        assert!(matches!(ir.tokens[1], Token::Space(SpaceKind::Regular)));
        match &ir.tokens[2] {
            Token::Word(w) => assert_eq!(w.text, "world"),
            _ => panic!("expected third token to be word"),
        }
    }

    #[test]
    fn parse_empty() {
        let ir = DocumentIR::parse("", false);
        assert!(ir.tokens.is_empty());
    }

    #[test]
    fn parse_sets_meta() {
        let ir = DocumentIR::parse("ATM 한A", true);
        match &ir.tokens[0] {
            Token::Word(w) => {
                assert!(w.meta.is_all_uppercase);
                assert!(w.meta.starts_with_ascii);
            }
            _ => panic!("expected word"),
        }
        match &ir.tokens[2] {
            Token::Word(w) => {
                assert!(w.meta.has_korean);
                assert!(w.meta.has_ascii_alphabetic);
                assert!(!w.meta.is_all_uppercase);
            }
            _ => panic!("expected word"),
        }
    }
}
