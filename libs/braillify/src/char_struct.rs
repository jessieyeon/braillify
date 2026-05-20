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
    CombiningMark,
    Space(char),
}

impl CharType {
    pub fn new(c: char) -> Result<Self, String> {
        let code = c as u32;
        if (0x2800..=0x28FF).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        if c.is_ascii_alphabetic() {
            return Ok(Self::English(c));
        }
        if c.is_ascii_digit() {
            return Ok(Self::Number(c));
        }
        if is_symbol_char(c) {
            return Ok(Self::Symbol(c));
        }
        if c == '□' {
            return Ok(Self::Symbol(c));
        }
        if is_math_symbol_char(c) {
            return Ok(Self::MathSymbol(c));
        }
        if is_unicode_fraction(c) {
            return Ok(Self::Fraction(c));
        }
        if code == 0x0307 {
            return Ok(Self::CombiningMark);
        }
        if code == 0x0305 {
            return Ok(Self::CombiningMark);
        }
        if code == 0x0308 {
            return Ok(Self::CombiningMark);
        }
        if code == 0x0309 {
            return Ok(Self::CombiningMark);
        }
        if code == 0x030A {
            return Ok(Self::CombiningMark);
        }
        if code == 0x0332 {
            return Ok(Self::CombiningMark);
        }
        if (0x0300..=0x036F).contains(&code) {
            return Ok(Self::CombiningMark);
        }
        // Combining Diacritical Marks for Symbols (U+20D0–U+20FF),
        // includes U+20DE COMBINING ENCLOSING SQUARE which 제64항 attaches to a
        // preceding character. Rule 64 handles the wrap; the standalone mark
        // is consumed as a formatting annotation (제56항 path).
        if (0x20D0..=0x20FF).contains(&code) {
            return Ok(Self::CombiningMark);
        }
        if (0x3131..=0x318E).contains(&code) {
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
        // LaTeX delimiters — treat as symbols so partial LaTeX tokens
        // don't cause "Invalid character" errors
        if c == '$' || c == '\\' {
            return Ok(Self::Symbol(c));
        }

        // Old Hangul Jamo (initial consonants, medial vowels, final consonants)
        if (0x1100..=0x115F).contains(&code)
            || (0x1160..=0x11A7).contains(&code)
            || (0x11A8..=0x11FF).contains(&code)
        {
            return Ok(Self::KoreanPart(c));
        }
        // Hangul Jamo Extended-A (old initial consonants)
        if (0xA960..=0xA97C).contains(&code) {
            return Ok(Self::KoreanPart(c));
        }
        // Hangul Jamo Extended-B (old medial vowels + old final consonants)
        if (0xD7B0..=0xD7C6).contains(&code) || (0xD7CB..=0xD7FB).contains(&code) {
            return Ok(Self::KoreanPart(c));
        }
        // Extended Hangul Compatibility Jamo (ㆍ, ㆎ, ㆇ-ㆌ, etc.)
        // Current range is 0x3131-0x318E, extend to cover 0x318F-0x319F and 0x3200-0x321E
        if (0x318F..=0x319F).contains(&code) {
            return Ok(Self::KoreanPart(c));
        }
        // CJK Unified Ideographs (字, 君, 洪, 侵, 斗, 虛, 後, 狄, 人, 位, 烽, 火, 孟, 子, 禽, etc.)
        if (0x4E00..=0x9FFF).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // CJK Extension A
        if (0x3400..=0x4DBF).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // IPA Extensions
        if (0x0250..=0x02AF).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // Spacing Modifier Letters (ː for IPA long vowel mark etc.)
        if (0x02B0..=0x02FF).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // Latin Extended Additional (for accented characters in IPA contexts)
        if (0x1E00..=0x1EFF).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // Latin Extended-A and B (for ŋ, ə, ɛ, etc. used in IPA)
        if (0x0100..=0x024F).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // Latin-1 Supplement (for ·, ×, ÷, etc. - middle dot at 0x00B7 is important for 방점)
        if (0x00A0..=0x00FF).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // Greek and Coptic (θ for IPA, already partly handled via is_symbol_char, but ensure coverage)
        if (0x0370..=0x03FF).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // Fullwidth Forms (：fullwidth colon at U+FF1A for 상성)
        if (0xFF00..=0xFFEF).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // General Punctuation (various dashes, quotes, etc.)
        if (0x2000..=0x206F).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // Letterlike Symbols
        if (0x2100..=0x214F).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // Enclosed CJK Letters and Months
        if (0x3200..=0x32FF).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // CJK Compatibility
        if (0x3300..=0x33FF).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // Supplemental Punctuation
        if (0x2E00..=0x2E7F).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // CJK Symbols and Punctuation
        if (0x3000..=0x303F).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // Geometric Shapes (◇, ◆, ▷, ◁, etc.)
        if (0x25A0..=0x25FF).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // Miscellaneous Symbols
        if (0x2600..=0x26FF).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // Hangul syllables in the OLD range (U+D7A4-U+D7AF is gap between modern and extended-B)
        // Already handled by 0xAC00-0xD7A3 check above
        // Korean Unified Ideographic characters (supplement)
        if (0xF900..=0xFAFF).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // CJK Unified Ideographs Extension B-F and related supplementary planes
        if (0x20000..=0x2EBEF).contains(&code) || (0x2F800..=0x2FA1F).contains(&code) {
            return Ok(Self::Symbol(c));
        }
        // Private Use Area (legacy Hangul glyphs in historical corpora)
        if (0xE000..=0xF8FF).contains(&code)
            || (0xF0000..=0xFFFFD).contains(&code)
            || (0x100000..=0x10FFFD).contains(&code)
        {
            return Ok(Self::Symbol(c));
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
        assert!(matches!(CharType::new('□').unwrap(), CharType::Symbol('□')));
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
                    // KoreanPart는 다음 Hangul jamo/CJK 범위 중 하나여야 한다:
                    //  - U+1100..U+11FF (Hangul Jamo — modern initial/medial/final)
                    //  - U+3131..U+318E (Hangul Compatibility Jamo)
                    //  - U+318F..U+319F, U+3200..U+321E (확장 Hangul 호환 자모)
                    //  - U+A960..U+A97C (Hangul Jamo Extended-A)
                    //  - U+D7B0..U+D7C6, U+D7CB..U+D7FB (Hangul Jamo Extended-B)
                    //  - U+4E00..U+9FFF (CJK Unified Ideographs — historical Korean text)
                    assert!(
                        (0x1100..=0x11FF).contains(&code)
                            || (0x3131..=0x318E).contains(&code)
                            || (0x318F..=0x319F).contains(&code)
                            || (0x3200..=0x321E).contains(&code)
                            || (0xA960..=0xA97C).contains(&code)
                            || (0xD7B0..=0xD7C6).contains(&code)
                            || (0xD7CB..=0xD7FB).contains(&code)
                            || (0x4E00..=0x9FFF).contains(&code),
                        "KoreanPart char U+{:04X} not in expected ranges", code
                    );
                }
                CharType::English(ch) => {
                    assert!(ch.is_ascii_alphabetic());
                }
                CharType::Number(ch) => {
                    assert!(ch.is_ascii_digit());
                }
                CharType::Symbol(ch) => {
                    let code = ch as u32;
                    assert!(
                        is_symbol_char(ch)
                        || ch == '$'
                        || ch == '\\'
                        || ch == '□'
                        || (0x2800..=0x28FF).contains(&code)   // braille patterns
                        || (0x4E00..=0x9FFF).contains(&code)   // CJK
                        || (0x3400..=0x4DBF).contains(&code)   // CJK Ext A
                        || (0x0250..=0x02AF).contains(&code)   // IPA
                        || (0x02B0..=0x02FF).contains(&code)   // Spacing modifiers
                        || (0x1E00..=0x1EFF).contains(&code)   // Latin Extended Additional
                        || (0x0100..=0x024F).contains(&code)   // Latin Extended A/B
                        || (0x00A0..=0x00FF).contains(&code)   // Latin-1 Supplement
                        || (0x0370..=0x03FF).contains(&code)   // Greek
                        || (0xFF00..=0xFFEF).contains(&code)   // Fullwidth
                        || (0x2000..=0x206F).contains(&code)   // General Punctuation
                        || (0x2100..=0x214F).contains(&code)   // Letterlike
                        || (0x3200..=0x32FF).contains(&code)   // Enclosed CJK
                        || (0x3300..=0x33FF).contains(&code)   // CJK Compat
                        || (0x2E00..=0x2E7F).contains(&code)   // Supplemental Punct
                        || (0x25A0..=0x25FF).contains(&code)   // Geometric Shapes
                        || (0x2600..=0x26FF).contains(&code)   // Misc Symbols
                        || (0xF900..=0xFAFF).contains(&code)   // CJK Compat Ideographs
                        || (0x3000..=0x303F).contains(&code)   // CJK Symbols
                        || (0x20000..=0x2EBEF).contains(&code) // CJK Supplementary
                        || (0x2F800..=0x2FA1F).contains(&code) // CJK Compat Supplement
                        || (0xE000..=0xF8FF).contains(&code)   // PUA
                        || (0xF0000..=0xFFFFD).contains(&code) // Supplementary PUA-A
                        || (0x100000..=0x10FFFD).contains(&code) // Supplementary PUA-B
                    );
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
                CharType::CombiningMark => {}
            }
        }
    }
}
