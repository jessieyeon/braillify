use crate::{
    char_shortcut,
    char_struct::KoreanChar,
    jauem::{choseong::encode_choseong, jongseong::encode_jongseong},
    moeum::jungsong::encode_jungsong,
    split::split_korean_jauem,
    utils::build_char,
};

pub fn encode_korean_char(korean: &KoreanChar) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    let (cho0, cho1) = split_korean_jauem(korean.cho)?;
    if cho1.is_some() {
        // 쌍자음이라는 뜻, 초성은 반드시 쌍자음이다.
        result.push(32);
    }
    if let Some(jong) = korean.jong {
        let (jong0, jong1) = split_korean_jauem(jong)?;
        if let Ok(code) = char_shortcut::encode_char_shortcut(build_char('ㅇ', korean.jung, Some(jong0))) {
            // 초성 자체를 결합

            if cho0 != 'ㅇ' {
                result.push(encode_choseong(cho0)?);
            }
            result.extend(code);
            if let Some(code) = jong1 {
                // 이미 합쳐질 경우 종성을 더 추가한다.
                result.extend(encode_jongseong(code)?);
            }
        } else if let Ok(code) = char_shortcut::encode_char_shortcut(build_char(cho0, korean.jung, Some(jong0))) {
            result.extend(code);
            if let Some(code) = jong1 {
                // 이미 합쳐질 경우 종성을 더 추가한다.
                result.extend(encode_jongseong(code)?);
            }
        } else if let Ok(code) = char_shortcut::encode_char_shortcut(build_char(cho0, korean.jung, None)) {
            result.extend(code);
            // 종성 자체를 결합
            result.extend(encode_jongseong(jong)?);
        } else {
            // shortcut 이 없으므로 초성, 중성, 종성 모두 결합

            if cho0 != 'ㅇ' {
                result.push(encode_choseong(cho0)?);
            }
            result.extend(encode_jungsong(korean.jung)?);
            result.extend(encode_jongseong(jong)?);
        }
    } else if let Ok(code) = char_shortcut::encode_char_shortcut(build_char(cho0, korean.jung, None)) {
        result.extend(code);
    } else {
        // shortcut 이 없으므로 초성 중성, 모두 결합
        if cho0 != 'ㅇ' {
            result.push(encode_choseong(cho0)?);
        }
        result.extend(encode_jungsong(korean.jung)?);
    }

    Ok(result)
}
