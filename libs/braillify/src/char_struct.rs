use crate::{
    fraction::is_unicode_fraction, math_symbol_shortcut::is_math_symbol_char,
    symbol_shortcut::is_symbol_char,
};

/// Character in Korean
#[derive(Debug)]
pub struct KoreanChar {
    /// 초성
    pub cho: char,
    /// 중성
    pub jung: char,
    /// 종성
    pub jong: Option<char>,
}

impl KoreanChar {
    pub fn new(c: char) -> Result<Self, String> {
        let code = c as u32;
        if !(0xAC00..=0xD7A3).contains(&code) {
            return Err("Invalid Korean character".to_string());
        }

        const CHOSEONG: [char; 19] = [
            'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ',
            'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
        ];
        const JUNGSEONG: [char; 21] = [
            'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ',
            'ㅝ', 'ㅞ', 'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
        ];
        const JONGSEONG: [char; 28] = [
            ' ', 'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ',
            'ㄿ', 'ㅀ', 'ㅁ', 'ㅂ', 'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
        ];

        let code = c as u32;

        let uni = code - 0xAC00;
        let fn_idx = (uni / 588) as usize;
        let sn_idx = ((uni - (fn_idx as u32 * 588)) / 28) as usize;
        let tn_idx = (uni % 28) as usize;

        Ok(Self {
            cho: CHOSEONG[fn_idx],
            jung: JUNGSEONG[sn_idx],
            jong: if JONGSEONG[tn_idx] != ' ' {
                Some(JONGSEONG[tn_idx])
            } else {
                None
            },
        })
    }
}

#[derive(Debug)]
pub enum CharType {
    Korean(KoreanChar),
    KoreanPart(char),
    English(char),
    Number(char),
    Symbol(char),
    MathSymbol(char),
    Fraction(char),
    Space(char),
}

impl CharType {
    pub fn new(c: char) -> Result<Self, String> {
        if c.is_ascii_alphabetic() {
            return Ok(Self::English(c));
        }
        if c.is_ascii_digit() {
            return Ok(Self::Number(c));
        }
        if is_symbol_char(c) {
            return Ok(Self::Symbol(c));
        }
        if is_math_symbol_char(c) {
            return Ok(Self::MathSymbol(c));
        }
        if is_unicode_fraction(c) {
            return Ok(Self::Fraction(c));
        }
        let code = c as u32;
        if (0x3131..=0x3163).contains(&code) {
            return Ok(Self::KoreanPart(c));
        }
        // if !(0xAC00 <= code && code <= 0xD7A3) {
        //     return Ok(Self::Char(c));
        // }
        if (0xAC00..=0xD7A3).contains(&code) {
            return Ok(Self::Korean(KoreanChar::new(c)?));
        }
        if c.is_whitespace() {
            return Ok(Self::Space(c));
        }
        Err("Invalid character".to_string())
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use proptest::prelude::*;

    #[test]
    pub fn test_char_type() {
        assert!(matches!(
            CharType::new('A').unwrap(),
            CharType::English('A')
        ));
        assert!(matches!(CharType::new('1').unwrap(), CharType::Number('1')));
        assert!(matches!(CharType::new('!').unwrap(), CharType::Symbol('!')));
        assert!(matches!(
            CharType::new('ㄱ').unwrap(),
            CharType::KoreanPart('ㄱ')
        ));
        assert!(matches!(CharType::new(' ').unwrap(), CharType::Space(' ')));
        assert!(matches!(
            CharType::new('½').unwrap(),
            CharType::Fraction('½')
        ));
    }

    proptest! {
        #[test]
        fn test_char_type_proptest(c: char) {
            let Ok(c) = CharType::new(c) else {
                // 지원하지 않는 문자이므로
                return Ok(());
            };
            match c {
                CharType::Korean(korean_char) => {
                    assert!(korean_char.cho != '\0');
                    assert!(korean_char.jung != '\0');
                }
                CharType::KoreanPart(ch) => {
                    let code = ch as u32;
                    assert!(0x3131 <= code && code <= 0x3163);
                }
                CharType::English(ch) => {
                    assert!(ch.is_ascii_alphabetic());
                }
                CharType::Number(ch) => {
                    assert!(ch.is_ascii_digit());
                }
                CharType::Symbol(ch) => {
                    assert!(is_symbol_char(ch));
                }
                CharType::MathSymbol(ch) => {
                    assert!(is_math_symbol_char(ch));
                }
                CharType::Space(ch) => {
                    assert!(ch.is_whitespace());
                }
                CharType::Fraction(ch) => {
                    assert!(is_unicode_fraction(ch));
                }
            }
        }
    }
}
