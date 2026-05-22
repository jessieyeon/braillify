use phf::phf_map;

use crate::unicode::decode_unicode;

pub static SHORTCUT_MAP: phf::Map<&'static str, &'static [u8]> = phf_map! {
    "그래서" => &[decode_unicode('⠁'), decode_unicode('⠎')],
    "그러나" => &[decode_unicode('⠁'), decode_unicode('⠉')],
    "그러면" => &[decode_unicode('⠁'), decode_unicode('⠒')],
    "그러므로" => &[decode_unicode('⠁'), decode_unicode('⠢')],
    "그런데" => &[decode_unicode('⠁'), decode_unicode('⠝')],
    "그리고" => &[decode_unicode('⠁'), decode_unicode('⠥')],
    "그리하여" => &[decode_unicode('⠁'), decode_unicode('⠱')],
};

pub fn split_word_shortcut(text: &str) -> Option<(&'static str, &'static [u8], String)> {
    for (key, value) in SHORTCUT_MAP.entries() {
        if let Some(rest) = text.strip_prefix(key) {
            return Some((key, value, rest.to_string()));
        }
    }
    None
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_encode_word_shortcut() {
        assert_eq!(
            split_word_shortcut("그래서").unwrap(),
            (
                "그래서",
                &[decode_unicode('⠁'), decode_unicode('⠎')][..],
                "".to_string()
            )
        );
    }
}
