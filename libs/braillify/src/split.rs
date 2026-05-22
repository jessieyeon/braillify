use phf::phf_map;

#[derive(Debug, PartialEq)]
pub enum KoreanChar {
    Choseong(char),
    Jongseong(char),
    Jungseong(char),
}
impl KoreanChar {
    pub fn get_char(&self) -> char {
        match self {
            KoreanChar::Choseong(c) => *c,
            KoreanChar::Jongseong(c) => *c,
            KoreanChar::Jungseong(c) => *c,
        }
    }
}

// ㄱ, ㄲ, ㄳ, ㄴ, ㄵ, ㄶ, ㄷ, ㄸ, ㄹ, ㄺ, ㄻ, ㄼ, ㄽ, ㄾ, ㄿ, ㅀ, ㅁ, ㅂ, ㅃ, ㅄ, ㅅ, ㅆ, ㅇ, ㅈ, ㅉ, ㅊ, ㅋ, ㅌ, ㅍ, ㅎ
pub static KOREAN_JAUEM_MAP: phf::Map<char, (char, Option<char>)> = phf_map! {
    'ㄱ' => ('ㄱ', None),
    'ㄲ' => ('ㄱ', Some('ㄱ')),
    'ㄳ' => ('ㄱ', Some('ㅅ')),
    'ㄴ' => ('ㄴ', None),
    'ㄵ' => ('ㄴ', Some('ㅈ')),
    'ㄶ' => ('ㄴ', Some('ㅎ')),
    'ㄷ' => ('ㄷ', None),
    'ㄸ' => ('ㄷ', Some('ㄷ')),
    'ㄹ' => ('ㄹ', None),
    'ㄺ' => ('ㄹ', Some('ㄱ')),
    'ㄻ' => ('ㄹ', Some('ㅁ')),
    'ㄼ' => ('ㄹ', Some('ㅂ')),
    'ㄽ' => ('ㄹ', Some('ㅅ')),
    'ㄾ' => ('ㄹ', Some('ㅌ')),
    'ㄿ' => ('ㄹ', Some('ㅍ')),
    'ㅀ' => ('ㄹ', Some('ㅎ')),
    'ㅁ' => ('ㅁ', None),
    'ㅂ' => ('ㅂ', None),
    'ㅃ' => ('ㅂ', Some('ㅂ')),
    'ㅄ' => ('ㅂ', Some('ㅅ')),
    'ㅅ' => ('ㅅ', None),
    'ㅆ' => ('ㅅ', Some('ㅅ')),
    'ㅇ' => ('ㅇ', None),
    'ㅈ' => ('ㅈ', None),
    'ㅉ' => ('ㅈ', Some('ㅈ')),
    'ㅊ' => ('ㅊ', None),
    'ㅋ' => ('ㅋ', None),
    'ㅌ' => ('ㅌ', None),
    'ㅍ' => ('ㅍ', None),
    'ㅎ' => ('ㅎ', None),
};

/// 자음을 분리합니다.
pub fn split_korean_jauem(text: char) -> Result<(char, Option<char>), String> {
    if let Some((cho, jong)) = KOREAN_JAUEM_MAP.get(&text) {
        return Ok((*cho, *jong));
    }
    Err("Invalid Korean character".to_string())
}
pub fn split_korean_char(text: char) -> Result<Vec<KoreanChar>, String> {
    // check korean char
    let code = text as u32;
    if (0x3131..=0x314E).contains(&code) {
        return Ok(vec![KoreanChar::Choseong(text)]);
    }
    if (0x314F..=0x3163).contains(&code) {
        return Ok(vec![KoreanChar::Jungseong(text)]);
    }
    if !(0xAC00..=0xD7A3).contains(&code) {
        return Err("Invalid Korean character".to_string());
    }

    const CHOSEONG: [char; 19] = [
        'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ',
        'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
    ];
    const JUNGSEONG: [char; 21] = [
        'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ',
        'ㅞ', 'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
    ];
    const JONGSEONG: [char; 28] = [
        ' ', 'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ',
        'ㅀ', 'ㅁ', 'ㅂ', 'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
    ];

    let code = text as u32;

    let uni = code - 0xAC00;
    let fn_idx = (uni / 588) as usize;
    let sn_idx = ((uni - (fn_idx as u32 * 588)) / 28) as usize;
    let tn_idx = (uni % 28) as usize;

    let mut result = Vec::new();
    result.push(KoreanChar::Choseong(CHOSEONG[fn_idx]));
    result.push(KoreanChar::Jungseong(JUNGSEONG[sn_idx]));
    if JONGSEONG[tn_idx] != ' ' {
        result.push(KoreanChar::Jongseong(JONGSEONG[tn_idx]));
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split() {
        assert_eq!(
            split_korean_char('강'),
            Ok(vec![
                KoreanChar::Choseong('ㄱ'),
                KoreanChar::Jungseong('ㅏ'),
                KoreanChar::Jongseong('ㅇ')
            ])
        );
        assert_eq!(
            split_korean_char('한'),
            Ok(vec![
                KoreanChar::Choseong('ㅎ'),
                KoreanChar::Jungseong('ㅏ'),
                KoreanChar::Jongseong('ㄴ')
            ])
        );
        assert_eq!(
            split_korean_char('글'),
            Ok(vec![
                KoreanChar::Choseong('ㄱ'),
                KoreanChar::Jungseong('ㅡ'),
                KoreanChar::Jongseong('ㄹ')
            ])
        );
        assert_eq!(
            split_korean_char('안'),
            Ok(vec![
                KoreanChar::Choseong('ㅇ'),
                KoreanChar::Jungseong('ㅏ'),
                KoreanChar::Jongseong('ㄴ')
            ])
        );
        assert_eq!(
            split_korean_char('녕'),
            Ok(vec![
                KoreanChar::Choseong('ㄴ'),
                KoreanChar::Jungseong('ㅕ'),
                KoreanChar::Jongseong('ㅇ')
            ])
        );

        assert_eq!(
            split_korean_char('나'),
            Ok(vec![
                KoreanChar::Choseong('ㄴ'),
                KoreanChar::Jungseong('ㅏ'),
            ])
        );

        assert_eq!(
            split_korean_char('라'),
            Ok(vec![
                KoreanChar::Choseong('ㄹ'),
                KoreanChar::Jungseong('ㅏ'),
            ])
        );
    }

    #[test]
    fn test_split_choseong() {
        for c in [
            'ㄱ', 'ㄴ', 'ㄷ', 'ㅁ', 'ㅂ', 'ㅅ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
        ] {
            assert_eq!(split_korean_char(c), Ok(vec![KoreanChar::Choseong(c),]));
        }
    }

    #[test]
    fn test_split_jungseong() {
        for c in [
            'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ',
            'ㅝ', 'ㅞ', 'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
        ] {
            assert_eq!(split_korean_char(c), Ok(vec![KoreanChar::Jungseong(c),]));
        }
    }

    #[test]
    fn test_split_wrong() {
        assert_eq!(
            split_korean_char('a'),
            Err("Invalid Korean character".to_string())
        );
        assert_eq!(
            split_korean_char('1'),
            Err("Invalid Korean character".to_string())
        );
    }
}
