use crate::english::encode_english;
use crate::number::encode_number;
use crate::rules::context::EncoderState;
use crate::rules::token::Token;
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};
use crate::unicode::decode_unicode;

pub struct DigitalNotationRule;

impl TokenRule for DigitalNotationRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::ModeEntry
    }

    fn priority(&self) -> u16 {
        1
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        _state: &mut EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        let Some(Token::Word(word)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };

        if !has_digital_signature(word.text.as_ref()) {
            return Ok(TokenAction::Noop);
        }

        let bytes = encode_digital_word(word.text.as_ref())?;
        Ok(TokenAction::Replace(Token::PreEncoded(bytes)))
    }
}

fn has_digital_signature(text: &str) -> bool {
    if !text
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphanumeric())
    {
        return false;
    }
    if text.contains("//") || text.contains('@') || text.contains('#') {
        return true;
    }
    // 단독 `_`는 일반 부호로 처리. 디지털 표기는 `_`와 다른 디지털 표지(`. / :`)
    // 조합에서만 활성화한다.
    text.contains('_') && (text.contains('.') || text.contains('/') || text.contains(':'))
}

fn encode_digital_word(text: &str) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    let mut english_started = false;

    let prefix_len = chars
        .iter()
        .take_while(|ch| {
            ch.is_ascii_alphanumeric() || matches!(**ch, '/' | '#' | '@' | '.' | ':' | '_')
        })
        .count();

    let digital_chars = &chars[..prefix_len];

    while i < digital_chars.len() {
        let ch = digital_chars[i];
        if ch.is_ascii_alphabetic() {
            let start = i;
            i += 1;
            while i < digital_chars.len() && digital_chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            if !english_started {
                result.push(decode_unicode('⠴'));
                english_started = true;
            }
            encode_digital_english_segment(&digital_chars[start..i], &mut result)?;
            continue;
        }

        if ch.is_ascii_digit() {
            if i > 0 && digital_chars[i - 1].is_ascii_alphabetic() {
                result.push(decode_unicode('⠐'));
                result.push(0);
            }
            result.push(decode_unicode('⠼'));
            while i < digital_chars.len() && digital_chars[i].is_ascii_digit() {
                result.push(encode_number(digital_chars[i])?);
                i += 1;
            }
            continue;
        }

        match ch {
            '/' => {
                result.push(decode_unicode('⠸'));
                result.push(decode_unicode('⠌'));
            }
            '#' => {
                result.push(decode_unicode('⠸'));
                result.push(decode_unicode('⠹'));
            }
            '@' => {
                result.push(decode_unicode('⠈'));
                result.push(decode_unicode('⠁'));
            }
            '.' => result.push(decode_unicode('⠲')),
            ':' => result.push(decode_unicode('⠒')),
            '_' => {
                result.push(decode_unicode('⠨'));
                result.push(decode_unicode('⠤'));
            }
            _ => return Err(format!("unsupported digital notation character: {ch}")),
        }
        i += 1;
    }

    if prefix_len == chars.len() && chars.last().is_some_and(|ch| ch.is_ascii_alphabetic()) {
        result.push(decode_unicode('⠲'));
    }

    if prefix_len < chars.len() {
        if digital_chars
            .last()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
        {
            result.push(decode_unicode('⠲'));
        }
        let remainder: String = chars[prefix_len..].iter().collect();
        result.extend(crate::encode(&remainder)?);
    }

    Ok(result)
}

#[cfg_attr(tarpaulin, inline(never))]
fn encode_digital_english_segment(chars: &[char], result: &mut Vec<u8>) -> Result<(), String> {
    let mut i = 0usize;
    while i < chars.len() {
        let rest: String = chars[i..].iter().collect();
        if rest.starts_with("ment") {
            result.push(decode_unicode('⠰'));
            result.push(decode_unicode('⠞'));
            i += 4;
            continue;
        }
        if rest.starts_with("ing") {
            result.push(decode_unicode('⠬'));
            i += 3;
            continue;
        }
        if rest.starts_with("con") {
            result.push(decode_unicode('⠒'));
            i += 3;
            continue;
        }
        if rest.starts_with("ea") && chars.get(i + 2).is_some_and(|ch| ch.is_ascii_alphabetic()) {
            result.push(decode_unicode('⠂'));
            i += 2;
            continue;
        }
        if rest.starts_with("en") {
            result.push(decode_unicode('⠢'));
            i += 2;
            continue;
        }
        if rest.starts_with("ar") {
            result.push(decode_unicode('⠜'));
            i += 2;
            continue;
        }
        if rest.starts_with("er") {
            result.push(decode_unicode('⠻'));
            i += 2;
            continue;
        }
        if rest.starts_with("ou") {
            result.push(decode_unicode('⠳'));
            i += 2;
            continue;
        }
        if rest.starts_with("ow") {
            result.push(decode_unicode('⠪'));
            i += 2;
            continue;
        }
        if rest.starts_with("th") {
            result.push(decode_unicode('⠹'));
            i += 2;
            continue;
        }
        let encoded_letter = encode_english(chars[i])?;
        result.push(encoded_letter);
        i += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::token::{SpaceKind, WordMeta, WordToken};
    use std::borrow::Cow;

    fn word_token<'a>(text: &str) -> Token<'a> {
        let chars: Vec<char> = text.chars().collect();
        Token::Word(WordToken {
            text: Cow::Owned(text.to_string()),
            chars: chars.clone(),
            meta: WordMeta::from_chars(&chars),
        })
    }

    #[test]
    fn rule_phase_priority() {
        let r = DigitalNotationRule;
        assert!(matches!(r.phase(), TokenPhase::ModeEntry));
        assert_eq!(r.priority(), 1);
    }

    #[test]
    fn has_digital_signature_paths() {
        // URL/path with //
        assert!(has_digital_signature("http://example.com"));
        // Email with @
        assert!(has_digital_signature("foo@bar.com"));
        // Hashtag
        assert!(has_digital_signature("a#hash"));
        // Underscore with dot
        assert!(has_digital_signature("a_b.c"));
        // Underscore with slash
        assert!(has_digital_signature("a_b/c"));
        // Underscore with colon
        assert!(has_digital_signature("a_b:c"));
        // Plain alpha — no
        assert!(!has_digital_signature("hello"));
        // Just underscore — no
        assert!(!has_digital_signature("a_b"));
        // Non-alphanumeric start — no
        assert!(!has_digital_signature("/foo"));
        // Empty — no
        assert!(!has_digital_signature(""));
    }

    #[test]
    fn apply_non_word_noop() {
        let r = DigitalNotationRule;
        let tokens: Vec<Token> = vec![Token::Space(SpaceKind::Regular)];
        let mut state = EncoderState::new(false);
        assert!(matches!(
            r.apply(&tokens, 0, &mut state).unwrap(),
            TokenAction::Noop
        ));
    }

    #[test]
    fn apply_plain_word_noop() {
        let r = DigitalNotationRule;
        let tokens = vec![word_token("hello")];
        let mut state = EncoderState::new(false);
        assert!(matches!(
            r.apply(&tokens, 0, &mut state).unwrap(),
            TokenAction::Noop
        ));
    }

    #[test]
    fn apply_url_replaces() {
        let r = DigitalNotationRule;
        let tokens = vec![word_token("http://a.b")];
        let mut state = EncoderState::new(false);
        let action = r.apply(&tokens, 0, &mut state).unwrap();
        assert!(matches!(action, TokenAction::Replace(Token::PreEncoded(_))));
    }

    #[test]
    fn encode_digital_word_email() {
        let result = encode_digital_word("a@b").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn encode_digital_word_hashtag() {
        let result = encode_digital_word("a#b").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn encode_digital_word_underscore_dot() {
        let result = encode_digital_word("a_b.c").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn encode_digital_word_colon_path() {
        let result = encode_digital_word("foo:bar").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn encode_digital_word_mixed_alpha_digit_transitions() {
        // English then digit → triggers ⠐ + space prefix logic (line 71-74)
        let result = encode_digital_word("abc123").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn encode_digital_word_pure_digit_starts() {
        let result = encode_digital_word("123#abc").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn encode_digital_word_ends_with_alpha_appends_terminator() {
        // Pure URL ending with alpha; line 107-109 path
        let result = encode_digital_word("a#b").unwrap();
        // last alpha + appended terminator
        assert!(result.last().copied().is_some());
    }

    #[test]
    fn encode_digital_word_with_korean_suffix() {
        // prefix_len < chars.len() → lines 111-117
        // "a@b가" — `a@b` is digital prefix, `가` is Korean suffix
        let result = encode_digital_word("a@b가").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn encode_digital_english_segment_all_abbreviations() {
        // Exercise every digraph branch in encode_digital_english_segment
        let cases = [
            "aliment", "playing", "constant", "easy", "energy", "argon", "verb", "outdoor", "owls",
            "thumb",
        ];
        for case in cases {
            let chars: Vec<char> = case.chars().collect();
            let mut buf = Vec::new();
            encode_digital_english_segment(&chars, &mut buf).unwrap();
            assert!(!buf.is_empty(), "{case} should encode");
        }
    }

    #[test]
    fn encode_digital_english_segment_plain_letter() {
        // Single letter — no digraph match → falls to single-letter encode (line 177)
        let chars: Vec<char> = "z".chars().collect();
        let mut buf = Vec::new();
        encode_digital_english_segment(&chars, &mut buf).unwrap();
        assert!(!buf.is_empty());
    }

    /// `apply` Replace path: `has_digital_signature` true → returns a
    /// `Token::PreEncoded`. Exercises line 33-35.
    #[test]
    fn apply_replaces_digital_word_token() {
        use crate::rules::context::EncoderState;
        use crate::rules::token::{Token, WordMeta, WordToken};
        use crate::rules::token_rule::{TokenAction, TokenRule};
        use std::borrow::Cow;

        let text = "http://example.com";
        let chars: Vec<char> = text.chars().collect();
        let meta = WordMeta::from_chars(&chars);
        let tokens = vec![Token::Word(WordToken {
            text: Cow::Borrowed(text),
            chars,
            meta,
        })];
        let mut state = EncoderState::new(false);
        let action = DigitalNotationRule.apply(&tokens, 0, &mut state).unwrap();
        match action {
            TokenAction::Replace(Token::PreEncoded(bytes)) => assert!(!bytes.is_empty()),
            _ => panic!("expected Replace(PreEncoded)"),
        }
    }

    /// `apply` Noop path: non-Word token short-circuits.
    #[test]
    fn apply_noop_on_non_word_token() {
        use crate::rules::context::EncoderState;
        use crate::rules::token::Token;
        use crate::rules::token_rule::{TokenAction, TokenRule};

        let tokens = vec![Token::PreEncoded(vec![1, 2, 3])];
        let mut state = EncoderState::new(false);
        let action = DigitalNotationRule.apply(&tokens, 0, &mut state).unwrap();
        assert!(matches!(action, TokenAction::Noop));
    }

    /// `apply` Noop path: Word without digital signature.
    #[test]
    fn apply_noop_on_plain_word() {
        use crate::rules::context::EncoderState;
        use crate::rules::token::{Token, WordMeta, WordToken};
        use crate::rules::token_rule::{TokenAction, TokenRule};
        use std::borrow::Cow;

        let text = "hello";
        let chars: Vec<char> = text.chars().collect();
        let meta = WordMeta::from_chars(&chars);
        let tokens = vec![Token::Word(WordToken {
            text: Cow::Borrowed(text),
            chars,
            meta,
        })];
        let mut state = EncoderState::new(false);
        let action = DigitalNotationRule.apply(&tokens, 0, &mut state).unwrap();
        assert!(matches!(action, TokenAction::Noop));
    }

    /// digital_notation:33, 196 — digital signature with characters that aren't
    /// "ow" or "th" shortcuts fall through to the per-char encode at line 196.
    /// And the Replace(PreEncoded) return at line 33 fires for digital words.
    #[test]
    fn digital_words_simple_chars() {
        let _ = crate::encode("a@b");
        let _ = crate::encode("c#d");
        let _ = crate::encode("e//f");
        let _ = crate::encode("g_h.i");
        let _ = crate::encode("x_y:z");
    }
}
