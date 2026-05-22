use crate::unicode::decode_unicode;
// choseong map
use phf::phf_map;

pub static CHOSEONG_MAP: phf::Map<char, u8> = phf_map! {
    'ㄱ' => decode_unicode('⠈'),
    'ㄴ' => decode_unicode('⠉'),
    'ㄷ' => decode_unicode('⠊'),
    'ㄹ' => decode_unicode('⠐'),
    'ㅁ' => decode_unicode('⠑'),
    'ㅂ' => decode_unicode('⠘'),
    'ㅅ' => decode_unicode('⠠'),
    // 'ㅇ' => decode_unicode(''), // skip ㅇ of choseong
    'ㅈ' => decode_unicode('⠨'),
    'ㅊ' => decode_unicode('⠰'),
    'ㅋ' => decode_unicode('⠋'),
    'ㅌ' => decode_unicode('⠓'),
    'ㅍ' => decode_unicode('⠙'),
    'ㅎ' => decode_unicode('⠚'),
};

pub fn encode_choseong(text: char) -> Result<u8, String> {
    if let Some(code) = CHOSEONG_MAP.get(&text) {
        Ok(*code)
    } else {
        Err("Invalid Korean choseong character".to_string())
    }
}

#[cfg(test)]
mod test {
    use crate::unicode::decode_unicode;

    use super::*;
    #[test]
    pub fn test_encode_choseong() {
        assert_eq!(encode_choseong('ㄱ').unwrap(), decode_unicode('⠈'));
        assert_eq!(encode_choseong('ㄴ').unwrap(), decode_unicode('⠉'));
        assert_eq!(encode_choseong('ㄷ').unwrap(), decode_unicode('⠊'));
        assert_eq!(encode_choseong('ㄹ').unwrap(), decode_unicode('⠐'));
        assert_eq!(encode_choseong('ㅁ').unwrap(), decode_unicode('⠑'));
        assert_eq!(encode_choseong('ㅂ').unwrap(), decode_unicode('⠘'));
        assert_eq!(encode_choseong('ㅅ').unwrap(), decode_unicode('⠠'));
        assert_eq!(encode_choseong('ㅈ').unwrap(), decode_unicode('⠨'));
        assert_eq!(encode_choseong('ㅊ').unwrap(), decode_unicode('⠰'));
        assert_eq!(encode_choseong('ㅋ').unwrap(), decode_unicode('⠋'));
        assert_eq!(encode_choseong('ㅌ').unwrap(), decode_unicode('⠓'));
        assert_eq!(encode_choseong('ㅍ').unwrap(), decode_unicode('⠙'));
        assert_eq!(encode_choseong('ㅎ').unwrap(), decode_unicode('⠚'));
    }
}
