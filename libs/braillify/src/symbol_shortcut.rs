use phf::phf_map;

use crate::unicode::decode_unicode;

static SHORTCUT_MAP: phf::Map<char, &'static [u8]> = phf_map! {
    '"' => &[decode_unicode('⠦')],
    // '"' => &[decode_unicode('⠴')],
    '\'' => &[decode_unicode('⠠'), decode_unicode('⠦')],
    // '\'' => &[decode_unicode('⠴'), decode_unicode('⠄')],
    '~' => &[decode_unicode('⠈'), decode_unicode('⠔')],
    '…' => &[decode_unicode('⠠'), decode_unicode('⠠'), decode_unicode('⠠')],
    '⋯' => &[decode_unicode('⠠'), decode_unicode('⠠'), decode_unicode('⠠')],
    '!' => &[decode_unicode('⠖')],
    '.' => &[decode_unicode('⠲')],
    ',' => &[decode_unicode('⠐')],
    '?' => &[decode_unicode('⠦')],
    '“' => &[decode_unicode('⠦')],
    '”' => &[decode_unicode('⠴')],
    ':' => &[decode_unicode('⠐'), decode_unicode('⠂')],
    ';' => &[decode_unicode('⠰'), decode_unicode('⠆')],
    '_' => &[decode_unicode('⠤')],
    '*' => &[decode_unicode('⠐'), decode_unicode('⠔')],
    '(' => &[decode_unicode('⠦'), decode_unicode('⠄')],
    ')' => &[decode_unicode('⠠'), decode_unicode('⠴')],
    '{' => &[decode_unicode('⠦'), decode_unicode('⠂')],
    '}' => &[decode_unicode('⠐'), decode_unicode('⠴')],
    '[' => &[decode_unicode('⠦'), decode_unicode('⠆')],
    ']' => &[decode_unicode('⠰'), decode_unicode('⠴')],
    '·' => &[decode_unicode('⠐'), decode_unicode('⠆')],
    '「' => &[decode_unicode('⠐'), decode_unicode('⠦')],
    '」' => &[decode_unicode('⠴'), decode_unicode('⠂')],
    '『' => &[decode_unicode('⠰'), decode_unicode('⠦')],
    '』' => &[decode_unicode('⠴'), decode_unicode('⠆')],
    '/' => &[decode_unicode('⠸'), decode_unicode('⠌')],
    '〈' => &[decode_unicode('⠐'), decode_unicode('⠶')],
    '〉' => &[decode_unicode('⠶'), decode_unicode('⠂')],
    '《' => &[decode_unicode('⠰'), decode_unicode('⠶')],
    '》' => &[decode_unicode('⠶'), decode_unicode('⠆')],
    '―' => &[decode_unicode('⠤'), decode_unicode('⠤')],
    '-' => &[decode_unicode('⠤')],
    '∼' => &[decode_unicode('⠈'), decode_unicode('⠔')],
    '‘' => &[decode_unicode('⠠'), decode_unicode('⠦')],
    '’' => &[decode_unicode('⠴'), decode_unicode('⠄')],
    '○' => &[decode_unicode('⠸'),decode_unicode('⠴'), decode_unicode('⠇')],
    // '×' => &[decode_unicode('⠸'),decode_unicode('⠭'), decode_unicode('⠇')],
    '△' => &[decode_unicode('⠸'),decode_unicode('⠬'), decode_unicode('⠇')],
    '☆' => &[decode_unicode('⠸'),decode_unicode('⠔'), decode_unicode('⠇')],
    '◇' => &[decode_unicode('⠸'),decode_unicode('⠢'), decode_unicode('⠇')],
    '◆' => &[decode_unicode('⠸'),decode_unicode('⠕'), decode_unicode('⠇')],
    '□' => &[decode_unicode('⠸'),decode_unicode('⠶'), decode_unicode('⠇')],
    'ː' => &[decode_unicode('⠠'), decode_unicode('⠄')],
    '〃' => &[decode_unicode('⠴'), decode_unicode('⠴')],
};

static ENGLISH_SYMBOL_MAP: phf::Map<char, &'static [u8]> = phf_map! {
    '(' => &[decode_unicode('⠐'), decode_unicode('⠣')],
    ')' => &[decode_unicode('⠐'), decode_unicode('⠜')],
    ',' => &[decode_unicode('⠂')],
};

pub fn encode_char_symbol_shortcut(text: char) -> Result<&'static [u8], String> {
    if let Some(code) = SHORTCUT_MAP.get(&text) {
        Ok(code)
    } else {
        Err("Invalid symbol character".to_string())
    }
}

pub fn is_symbol_char(text: char) -> bool {
    SHORTCUT_MAP.contains_key(&text)
        || crate::rules::korean::rule_64::is_enclosed_symbol(text)
        || crate::rules::korean::rule_65::is_currency_symbol(text)
        || crate::rules::korean::rule_23::is_historical_letter_symbol(text)
        || crate::rules::korean::rule_25::is_rule_25_symbol(text)
        || crate::rules::korean::rule_31::is_greek_letter(text)
        || crate::rules::korean::rule_68::is_rule_68_symbol(text)
        || crate::rules::korean::rule_69::is_rule_69_symbol(text)
        || crate::rules::korean::rule_70::is_arrow_symbol(text)
        || crate::rules::korean::rule_71::is_rule_71_symbol(text)
        || crate::rules::korean::rule_72::is_rule_72_symbol(text)
}

pub fn encode_english_char_symbol_shortcut(text: char) -> Option<&'static [u8]> {
    ENGLISH_SYMBOL_MAP.get(&text).copied()
}

pub fn is_english_symbol_char(text: char) -> bool {
    ENGLISH_SYMBOL_MAP.contains_key(&text)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_is_symbol_char() {
        assert!(is_symbol_char('"'));
        assert!(is_symbol_char('\''));
        assert!(is_symbol_char('~'));
        assert!(is_symbol_char('…'));
        assert!(is_symbol_char('!'));
        assert!(is_symbol_char('.'));
        assert!(is_symbol_char(','));
        assert!(is_symbol_char('?'));
        assert!(is_symbol_char(':'));
        assert!(is_symbol_char(';'));
        assert!(is_symbol_char('_'));
        assert!(is_symbol_char('*'));
        assert!(is_symbol_char('('));
        assert!(is_symbol_char(')'));
        assert!(is_symbol_char('{'));
        assert!(is_symbol_char('}'));
        assert!(is_symbol_char('①'));
        assert!(is_symbol_char('ⓐ'));
        assert!(is_symbol_char('￦'));
    }

    #[test]
    pub fn test_encode_char_symbol_shortcut() {
        assert_eq!(
            encode_char_symbol_shortcut('"').unwrap(),
            &[decode_unicode('⠦')]
        );
        assert_eq!(
            encode_char_symbol_shortcut('\'').unwrap(),
            &[decode_unicode('⠠'), decode_unicode('⠦')]
        );
        assert_eq!(
            encode_char_symbol_shortcut('~').unwrap(),
            &[decode_unicode('⠈'), decode_unicode('⠔')]
        );
        assert_eq!(
            encode_char_symbol_shortcut('…').unwrap(),
            &[
                decode_unicode('⠠'),
                decode_unicode('⠠'),
                decode_unicode('⠠')
            ]
        );
        assert_eq!(
            encode_char_symbol_shortcut('⋯').unwrap(),
            &[
                decode_unicode('⠠'),
                decode_unicode('⠠'),
                decode_unicode('⠠')
            ]
        );
        assert_eq!(
            encode_char_symbol_shortcut('!').unwrap(),
            &[decode_unicode('⠖')]
        );
        assert_eq!(
            encode_char_symbol_shortcut('.').unwrap(),
            &[decode_unicode('⠲')]
        );
        assert_eq!(
            encode_char_symbol_shortcut(',').unwrap(),
            &[decode_unicode('⠐')]
        );
        assert_eq!(
            encode_char_symbol_shortcut('?').unwrap(),
            &[decode_unicode('⠦')]
        );
        assert_eq!(
            encode_char_symbol_shortcut(':').unwrap(),
            &[decode_unicode('⠐'), decode_unicode('⠂')]
        );
        assert_eq!(
            encode_char_symbol_shortcut(';').unwrap(),
            &[decode_unicode('⠰'), decode_unicode('⠆')]
        );
        assert_eq!(
            encode_char_symbol_shortcut('_').unwrap(),
            &[decode_unicode('⠤')]
        );
        assert_eq!(
            encode_char_symbol_shortcut('*').unwrap(),
            &[decode_unicode('⠐'), decode_unicode('⠔')]
        );
        assert_eq!(
            encode_char_symbol_shortcut('(').unwrap(),
            &[decode_unicode('⠦'), decode_unicode('⠄')]
        );
        assert_eq!(
            encode_char_symbol_shortcut(')').unwrap(),
            &[decode_unicode('⠠'), decode_unicode('⠴')]
        );
    }

    #[test]
    fn test_encode_english_char_symbol_shortcut_variants() {
        assert_eq!(
            encode_english_char_symbol_shortcut('(').unwrap(),
            &[decode_unicode('⠐'), decode_unicode('⠣')]
        );
        assert_eq!(
            encode_english_char_symbol_shortcut(')').unwrap(),
            &[decode_unicode('⠐'), decode_unicode('⠜')]
        );
        assert_eq!(encode_english_char_symbol_shortcut('?'), None);
    }
}
