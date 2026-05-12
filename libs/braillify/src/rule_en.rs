use phf::phf_map;

use crate::unicode::decode_unicode;
static ENGLISH_SHORTCUT_MAP: phf::Map<&'static str, u8> = phf_map! {
    // 10.3
    "and" => decode_unicode('⠯'),
    "for" => decode_unicode('⠿'),
    "of" => decode_unicode('⠷'),
    "the" => decode_unicode('⠮'),
    "with" => decode_unicode('⠾'),
    // 10.4
    "ch" => decode_unicode('⠡'),
    "gh" => decode_unicode('⠣'),
    "sh" => decode_unicode('⠩'),
    "th" => decode_unicode('⠹'),
    "wh" => decode_unicode('⠱'),
    "ed" => decode_unicode('⠫'),
    "er" => decode_unicode('⠻'),
    "ou" => decode_unicode('⠳'),
    "ow" => decode_unicode('⠪'),
    "st" => decode_unicode('⠌'),
    "ing" => decode_unicode('⠬'),
    "ar" => decode_unicode('⠜'),


    // 10.6.1 - 하위 묶음 약자 - en, in
    "en" => decode_unicode('⠢'),
    "in" => decode_unicode('⠔'),
};

/// 10.3 - 온칸 약자
/// 10.4 - 온칸 묶음 약자
pub fn rule_en_10_4(current: &str) -> Option<(u8, usize)> {
    for key in ENGLISH_SHORTCUT_MAP.keys() {
        if current.starts_with(key) {
            return Some((*ENGLISH_SHORTCUT_MAP.get(key).unwrap(), key.len() - 1));
        }
    }
    None
}
static ENGLISH_SHORTCUT_MAP_10_6: phf::Map<&'static str, u8> = phf_map! {
    "ea" => decode_unicode('⠂'),
    "be" => decode_unicode('⠆'),
    "bb" => decode_unicode('⠆'),
    "con" => decode_unicode('⠒'),
    "cc" => decode_unicode('⠒'),
    "dis" => decode_unicode('⠲'),
    "en" => decode_unicode('⠢'),
    "ff" => decode_unicode('⠖'),
    "gg" => decode_unicode('⠶'),
    "in" => decode_unicode('⠔'),
};

static ENGLISH_WHOLE_WORD_MAP_10_5: phf::Map<&'static str, &'static [u8]> = phf_map! {
    "every" => &[decode_unicode('⠐'), decode_unicode('⠑'), decode_unicode('⠽')],
    "knowledge" => &[
        decode_unicode('⠐'),
        decode_unicode('⠅'),
        decode_unicode('⠇'),
        decode_unicode('⠫'),
        decode_unicode('⠛'),
        decode_unicode('⠑'),
    ],
    "rather" => &[decode_unicode('⠗'), decode_unicode('⠁'), decode_unicode('⠮'), decode_unicode('⠗')],
    "enough" => &[decode_unicode('⠢'), decode_unicode('⠳'), decode_unicode('⠣')],
    "were" => &[decode_unicode('⠺'), decode_unicode('⠻'), decode_unicode('⠑')],
    "part" => &[decode_unicode('⠐'), decode_unicode('⠏')],
};

pub fn rule_en_10_5_whole_word(word: &str) -> Option<&'static [u8]> {
    ENGLISH_WHOLE_WORD_MAP_10_5.get(word).copied()
}
/// 10.6.1 - 하위 묶음 약자 - 시작할 때 일치 해야만 함
pub fn rule_en_10_6(current: &str) -> Option<(u8, usize)> {
    for key in ENGLISH_SHORTCUT_MAP_10_6.keys() {
        if current.starts_with(key) {
            return Some((*ENGLISH_SHORTCUT_MAP_10_6.get(key).unwrap(), key.len() - 1));
        }
    }
    None
}
