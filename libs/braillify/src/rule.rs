use crate::char_struct::{CharType, KoreanChar};

/// 5절 11항 - 모음자에 ‘예’가 붙어 나올 때에는 그 사이에 구분표 ⠤을 적어 나타낸다.
pub fn rule_11(current: &KoreanChar, next: char, result: &mut Vec<u8>) -> Result<(), String> {
    if let CharType::Korean(korean) = CharType::new(next)?
        && current.jong.is_none()
        && korean.cho == 'ㅇ'
        && korean.jung == 'ㅖ'
    {
        result.push(36);
    }
    Ok(())
}

/// 5절 12항 - ‘ㅑ, ㅘ, ㅜ, ㅝ’에 ‘애’가 붙어 나올 때에는 두 모음자 사이에 구분표 ⠤을 적어 나타낸다.
pub fn rule_12(current: &KoreanChar, next: char, result: &mut Vec<u8>) -> Result<(), String> {
    if let CharType::Korean(korean) = CharType::new(next)?
        && current.jong.is_none()
        && ['ㅑ', 'ㅘ', 'ㅜ', 'ㅝ'].contains(&current.jung)
        && korean.cho == 'ㅇ'
        && korean.jung == 'ㅐ'
    {
        result.push(36);
    }
    Ok(())
}
