//! Token spacing helpers for LaTeX math encoding (extracted from latex_math.rs).

use crate::rules::token::Token;

pub(super) fn previous_content_needs_math_spacing(tokens: &[Token<'_>], index: usize) -> usize {
    let Some(previous_index) = index.checked_sub(1) else {
        return 0;
    };

    match tokens.get(previous_index) {
        Some(Token::Space(_)) => {
            let left = previous_index
                .checked_sub(1)
                .and_then(|left_index| tokens.get(left_index));
            match left {
                Some(Token::Word(word)) if word.meta.has_korean => 1,
                _ => 0,
            }
        }
        Some(Token::Word(_) | Token::PreEncoded(_) | Token::Fraction(_) | Token::Mode(_)) => 2,
        None => 0,
    }
}

pub(super) fn next_content_needs_math_spacing(tokens: &[Token<'_>], index: usize) -> usize {
    match tokens.get(index + 1) {
        Some(Token::Space(_)) => {
            let right = tokens.get(index + 2);
            match right {
                Some(Token::Word(word)) if word.meta.has_korean => 1,
                _ => 0,
            }
        }
        Some(Token::Word(_) | Token::PreEncoded(_) | Token::Fraction(_) | Token::Mode(_)) => 2,
        None => 0,
    }
}

/// PDF — 한국어 산문 내 단일 math letter는 따옴표(⠴...⠲)로 감싼다.
/// 본문이 1~2자 ASCII letter이고 좌우가 한국어 컨텍스트일 때 적용한다.
pub(crate) fn wrap_latex_math_tokens_with_inner<'a>(
    tokens: &[Token<'a>],
    index: usize,
    bytes: Vec<u8>,
    inner: &str,
) -> Vec<Token<'a>> {
    let mut replacement = Vec::new();
    // PDF — `$-2$`, `$0.3010$` 같이 부호+숫자/소수점만 있는 단순 수치 표기는
    // "본격적 수식"이 아니므로 한국어 단어 경계에서 추가 두칸 띄어쓰기를 적용하지 않는다.
    // Space token 1칸으로 충분하다.
    let inner_is_simple_numeric = !inner.is_empty()
        && inner
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '\u{2212}' | '.' | ','));

    // PDF — 한국어 산문 내 단일/소수 math letter는 따옴표(⠴...⠲)로 감싼다.
    // 검출 조건:
    // 1. inner가 모두 ASCII letter 또는 짧은 letter+숫자 첨자 패턴
    // 2. 좌측이 한국어 단어로 끝나거나 우측이 한국어 단어로 시작(또는 한국어 particle)
    let is_short_prose_letter = !inner.is_empty()
        && inner.chars().count() <= 2
        && inner.chars().all(|c| c.is_ascii_alphabetic());
    // 콤마-구분 letter 리스트 (예: `a, b, c`, `A, B, C`)
    let comma_separated_letter_list = !inner.is_empty()
        && inner.contains(',')
        && inner.split(',').map(str::trim).all(|part| {
            !part.is_empty()
                && part.chars().count() == 1
                && part.chars().all(|c| c.is_ascii_alphabetic())
        });
    let in_korean_prose = if is_short_prose_letter || comma_separated_letter_list {
        let prev_is_korean = index
            .checked_sub(1)
            .and_then(|prev_idx| tokens.get(prev_idx))
            .map(|tok| match tok {
                Token::Word(word) => word.meta.has_korean,
                Token::Space(_) => index
                    .checked_sub(2)
                    .and_then(|left_idx| tokens.get(left_idx))
                    .is_some_and(|t| matches!(t, Token::Word(w) if w.meta.has_korean)),
                _ => false,
            })
            .unwrap_or(false);
        let next_is_korean = tokens
            .get(index + 1)
            .map(|tok| match tok {
                Token::Word(word) => word.meta.has_korean,
                Token::Space(_) => tokens
                    .get(index + 2)
                    .is_some_and(|t| matches!(t, Token::Word(w) if w.meta.has_korean)),
                _ => false,
            })
            .unwrap_or(false);
        prev_is_korean || next_is_korean
    } else {
        false
    };

    // 따옴표 wrap 경우 자체적으로 경계 명시 → 추가 leading 공백 불필요.
    // 단순 수치 또한 leading 공백 없음.
    let leading_spaces = if inner_is_simple_numeric || in_korean_prose {
        0
    } else {
        previous_content_needs_math_spacing(tokens, index)
    };
    if leading_spaces > 0 {
        replacement.push(Token::PreEncoded(vec![0; leading_spaces]));
    }

    if in_korean_prose && comma_separated_letter_list {
        // 콤마-구분 letter 리스트: 각 letter를 quote/english marker로 감싼다.
        // 예: `a, b, c` → ⠴a⠂ ⠰b⠂ ⠰c⠲
        let letters: Vec<&str> = inner.split(',').map(str::trim).collect();
        let mut wrapped = Vec::new();
        for (i, letter) in letters.iter().enumerate() {
            if let Some(c) = letter.chars().next() {
                if i == 0 {
                    wrapped.push(52); // ⠴ open quote
                } else {
                    wrapped.push(0); // space
                    wrapped.push(48); // ⠰ english indicator
                }
                if c.is_ascii_uppercase() {
                    wrapped.push(32); // ⠠ capital marker
                    if let Ok(code) = crate::english::encode_english(c.to_ascii_lowercase()) {
                        wrapped.push(code);
                    }
                } else if let Ok(code) = crate::english::encode_english(c) {
                    wrapped.push(code);
                }
                if i + 1 < letters.len() {
                    wrapped.push(16); // ⠐ comma
                } else {
                    wrapped.push(50); // ⠲ close quote
                }
            }
        }
        replacement.push(Token::PreEncoded(wrapped));
    } else if in_korean_prose {
        // ⠴ (open quote, 52) + 본문 + ⠲ (close quote, 50)
        // 따옴표가 math/Korean 경계를 명시하므로 추가 trailing/leading 공백 불필요.
        let mut wrapped = Vec::with_capacity(bytes.len() + 2);
        wrapped.push(52);
        wrapped.extend(bytes);
        wrapped.push(50);
        replacement.push(Token::PreEncoded(wrapped));
        // Korean prose context에서는 trailing 공백을 emit하지 않는다 (Space token이 분리).
    } else {
        replacement.push(Token::PreEncoded(bytes));
        let trailing_spaces = if inner_is_simple_numeric {
            0
        } else {
            next_content_needs_math_spacing(tokens, index)
        };
        if trailing_spaces > 0 {
            replacement.push(Token::PreEncoded(vec![0; trailing_spaces]));
        }
    }
    replacement
}

