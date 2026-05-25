pub fn encode_unicode(text: u8) -> char {
    if text == 255 {
        return '\n';
    }
    char::from_u32(text as u32 + 0x2800).unwrap()
}

pub const fn decode_unicode(text: char) -> u8 {
    if (text as u32) < 0x2800 {
        panic!("Invalid unicode character");
    }
    (text as u32 - 0x2800) as u8
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    pub fn test_encode_unicode() {
        assert_eq!(encode_unicode(0), '⠀');
        assert_eq!(encode_unicode(1), '⠁');
        assert_eq!(encode_unicode(2), '⠂');
        assert_eq!(encode_unicode(3), '⠃');
        assert_eq!(encode_unicode(4), '⠄');
        assert_eq!(encode_unicode(5), '⠅');
        assert_eq!(encode_unicode(6), '⠆');
        assert_eq!(encode_unicode(7), '⠇');
        assert_eq!(encode_unicode(8), '⠈');
        assert_eq!(encode_unicode(9), '⠉');
        assert_eq!(encode_unicode(10), '⠊');
        assert_eq!(encode_unicode(11), '⠋');
        assert_eq!(encode_unicode(12), '⠌');
        assert_eq!(encode_unicode(13), '⠍');
        assert_eq!(encode_unicode(14), '⠎');
        assert_eq!(encode_unicode(15), '⠏');
        assert_eq!(encode_unicode(16), '⠐');
        assert_eq!(encode_unicode(17), '⠑');
        assert_eq!(encode_unicode(18), '⠒');
        assert_eq!(encode_unicode(19), '⠓');
        assert_eq!(encode_unicode(20), '⠔');
        assert_eq!(encode_unicode(21), '⠕');
        assert_eq!(encode_unicode(22), '⠖');
        assert_eq!(encode_unicode(23), '⠗');
        assert_eq!(encode_unicode(24), '⠘');
        assert_eq!(encode_unicode(25), '⠙');
        assert_eq!(encode_unicode(26), '⠚');
        assert_eq!(encode_unicode(27), '⠛');
        assert_eq!(encode_unicode(28), '⠜');
        assert_eq!(encode_unicode(29), '⠝');
        assert_eq!(encode_unicode(30), '⠞');
        assert_eq!(encode_unicode(31), '⠟');
        assert_eq!(encode_unicode(32), '⠠');
        assert_eq!(encode_unicode(33), '⠡');
        assert_eq!(encode_unicode(34), '⠢');
        assert_eq!(encode_unicode(35), '⠣');
        assert_eq!(encode_unicode(36), '⠤');
        assert_eq!(encode_unicode(37), '⠥');
        assert_eq!(encode_unicode(38), '⠦');
        assert_eq!(encode_unicode(39), '⠧');
        assert_eq!(encode_unicode(40), '⠨');
        assert_eq!(encode_unicode(41), '⠩');
        assert_eq!(encode_unicode(42), '⠪');
        assert_eq!(encode_unicode(43), '⠫');
        assert_eq!(encode_unicode(44), '⠬');
        assert_eq!(encode_unicode(45), '⠭');
        assert_eq!(encode_unicode(46), '⠮');
        assert_eq!(encode_unicode(47), '⠯');
        assert_eq!(encode_unicode(48), '⠰');
        assert_eq!(encode_unicode(49), '⠱');
        assert_eq!(encode_unicode(50), '⠲');
        assert_eq!(encode_unicode(51), '⠳');
        assert_eq!(encode_unicode(52), '⠴');
        assert_eq!(encode_unicode(53), '⠵');
        assert_eq!(encode_unicode(54), '⠶');
        assert_eq!(encode_unicode(55), '⠷');
        assert_eq!(encode_unicode(56), '⠸');
        assert_eq!(encode_unicode(57), '⠹');
        assert_eq!(encode_unicode(58), '⠺');
        assert_eq!(encode_unicode(59), '⠻');
        assert_eq!(encode_unicode(60), '⠼');
        assert_eq!(encode_unicode(61), '⠽');
        assert_eq!(encode_unicode(62), '⠾');
        assert_eq!(encode_unicode(63), '⠿');
        assert_eq!(encode_unicode(255), '\n');
    }

    /// unicode.rs line 10 - decode_unicode panics for chars before U+2800.
    #[test]
    #[should_panic(expected = "Invalid unicode character")]
    fn decode_unicode_panics_for_non_braille_char() {
        let _ = decode_unicode('A');
    }
}
