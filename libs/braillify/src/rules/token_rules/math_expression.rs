//! Math expression token rule.
//!
//! Detects words that are math expressions (contain math operators,
//! function names, superscript/subscript chars, etc.) and encodes them
//! using the math braille engine instead of Korean character rules.

use crate::math_symbol_shortcut;
use crate::rules::context::EncoderState;
use crate::rules::math;
use crate::rules::token::{Token, WordMeta, WordToken};
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};
use std::borrow::Cow;

pub struct MathExpressionTokenRule;

/// Check if a character is a Unicode superscript.
fn is_superscript(c: char) -> bool {
    matches!(
        c,
        '\u{2070}' | '\u{00B9}' | '\u{00B2}' | '\u{00B3}'
            | '\u{2074}'..='\u{2079}'
            | '\u{207A}'
            | '\u{207B}'
            | '\u{207D}'
            | '\u{207E}'
            | '\u{207F}'
            | '\u{2071}'
            | '\u{02B0}'
            | '\u{02B2}'
            | '\u{02B3}'
            | '\u{02B7}'
            | '\u{02B8}'
            | '\u{02E1}'
            | '\u{02E2}'
            | '\u{02E3}'
            | '\u{1D43}'..='\u{1D58}'
            | '\u{1D5B}'
            | '\u{1D9C}'
            | '\u{1DA0}'
            | '\u{1DBB}'
    )
}

/// Check if a character is a Unicode subscript.
fn is_subscript(c: char) -> bool {
    matches!(
        c,
        '\u{2080}'..='\u{2089}'
            | '\u{208A}'
            | '\u{208B}'
            | '\u{208D}'
            | '\u{208E}'
            | '\u{2090}'..='\u{209C}'
            | '\u{1D62}'..='\u{1D65}'
    )
}

fn is_combining_math_mark(c: char) -> bool {
    matches!(
        c,
        '\u{0304}' | '\u{0305}' | '\u{0307}' | '\u{0308}' | '\u{0309}' | '\u{030A}' | '\u{0332}'
    )
}

fn is_middle_dot_numeric_word(chars: &[char]) -> bool {
    let middle_dot_count = chars
        .iter()
        .filter(|c| matches!(**c, '\u{00B7}' | '\u{22C5}'))
        .count();
    if middle_dot_count != 1 {
        return false;
    }
    chars
        .iter()
        .all(|c| c.is_ascii_digit() || matches!(*c, '\u{00B7}' | '\u{22C5}' | '\u{2212}' | '-'))
}

fn adjacent_korean_word_flags(tokens: &[Token<'_>], index: usize) -> (bool, bool) {
    let prev_has_korean = index
        .checked_sub(1)
        .and_then(|mut i| {
            loop {
                match tokens.get(i) {
                    Some(Token::Space(_)) => {
                        i = i.checked_sub(1)?;
                    }
                    Some(Token::Word(w)) => return Some(w.meta.has_korean),
                    _ => return None,
                }
            }
        })
        .unwrap_or(false);

    let next_has_korean = {
        let mut i = index + 1;
        loop {
            match tokens.get(i) {
                Some(Token::Space(_)) => i += 1,
                Some(Token::Word(w)) => break w.meta.has_korean,
                _ => break false,
            }
        }
    };

    (prev_has_korean, next_has_korean)
}

fn has_adjacent_korean_word(tokens: &[Token<'_>], index: usize) -> bool {
    let (prev_has_korean, next_has_korean) = adjacent_korean_word_flags(tokens, index);
    prev_has_korean || next_has_korean
}

fn is_korean_char(c: char) -> bool {
    let code = c as u32;
    (0xAC00..=0xD7A3).contains(&code) || (0x3131..=0x3163).contains(&code)
}

fn is_korean_suffix_char(c: char) -> bool {
    is_korean_char(c) || matches!(c, ')' | ']' | '}' | '.' | ',' | '!' | '?')
}

/// PDF 제44항 [다만]: 숫자와 혼동되는 'ㄴ, ㄷ, ㅁ, ㅋ, ㅌ, ㅍ, ㅎ'의 첫소리 글자와
/// '운'의 약자는 숫자 뒤에 붙어 나오더라도 숫자와 한글을 띄어 쓴다.
///
/// 즉, 수식·숫자 토큰 직후 한국어 음절이 위 7개 자음 초성으로 시작하거나
/// 첫 글자가 '운'이면 사이에 띄어쓰기를 추가한다.
///
/// 예: `$\frac{2}{5}$는` (는 = ㄴ 초성) → 분수 + 공백 + 는
///     `$\frac{3}{5}$은` (은 = ㅇ 초성) → 분수 + 은 (붙여쓰기)
fn rule_44_requires_space_before_korean(s: &str) -> bool {
    let Some(first_char) = s.chars().next() else {
        return false;
    };
    let code = first_char as u32;
    // 한글 음절 (AC00-D7A3) 외 한글 자모는 검사하지 않음.
    if !(0xAC00..=0xD7A3).contains(&code) {
        return false;
    }
    // 한글 음절 → 초성 추출. (음절 코드 - 0xAC00) / (21 * 28).
    // 초성 인덱스: ㄱ(0), ㄲ(1), ㄴ(2), ㄷ(3), ㄸ(4), ㄹ(5), ㅁ(6), ㅂ(7), ㅃ(8),
    //              ㅅ(9), ㅆ(10), ㅇ(11), ㅈ(12), ㅉ(13), ㅊ(14), ㅋ(15), ㅌ(16),
    //              ㅍ(17), ㅎ(18)
    let cho_index = (code - 0xAC00) / (21 * 28);
    if matches!(cho_index, 2 | 3 | 6 | 15 | 16 | 17 | 18) {
        return true;
    }
    // '운' 약자: '운' = U+C6B4 (오십칠항). 단일 음절이 '운'으로 시작.
    first_char == '운'
}

fn build_word_token(text: String) -> Token<'static> {
    let chars: Vec<char> = text.chars().collect();
    Token::Word(WordToken {
        text: Cow::Owned(text),
        chars: chars.clone(),
        meta: WordMeta::from_chars(&chars),
    })
}

fn is_strong_mixed_math_candidate(chars: &[char], text: &str) -> bool {
    if chars.len() <= 1 {
        return false;
    }

    let has_superscript = chars.iter().any(|c| is_superscript(*c));
    let has_subscript = chars.iter().any(|c| is_subscript(*c));
    let has_combining_mark = chars.iter().any(|c| is_combining_math_mark(*c));
    let starts_with_function = math::function::starts_with_function(text);
    let starts_with_root = chars.first() == Some(&'√');
    let is_absolute_value_form = chars.first() == Some(&'|') && chars.last() == Some(&'|');

    // 제11항: 등호 포함 수식 (예: "y=x+2는") — 한국어와 결합된 mixed math 토큰
    // 으로 분리 가능. 등호 + 변수 + 산술 연산자 형태.
    let has_equation = chars.contains(&'=')
        && chars.iter().any(|c| c.is_ascii_alphabetic())
        && chars
            .iter()
            .any(|c| matches!(*c, '+' | '-' | '×' | '÷' | '\u{2212}'));

    // PDF 수학 제12항 — 단일 영문자 + `(` 함수 호출 패턴(예: g(x), f(x)).
    // BMI 같은 약어와 구분하기 위해 첫 글자가 단일 영문자이고 두 번째가 `(`인 경우로 제한.
    let has_function_call = chars.len() >= 3
        && chars[0].is_ascii_alphabetic()
        && chars[1] == '('
        && chars.iter().filter(|c| c.is_ascii_alphabetic()).count() <= 3;

    starts_with_function
        || starts_with_root
        || is_absolute_value_form
        || has_superscript
        || has_subscript
        || has_combining_mark
        || has_equation
        || has_function_call
}

fn is_rule_68_compact_notation(chars: &[char]) -> bool {
    if chars.len() < 2 || !chars[0].is_ascii_uppercase() {
        return false;
    }

    if chars.len() == 2 && chars[1] == '-' {
        return true;
    }

    chars[1..]
        .iter()
        .all(|c| matches!(*c, '⁺' | '⁻' | '₀'..='₉'))
        && chars[1..]
            .iter()
            .any(|c| is_superscript(*c) || is_subscript(*c))
}

fn try_encode_math_slice(chars: &[char]) -> Option<Vec<u8>> {
    if chars.is_empty() || chars.iter().any(|c| is_korean_char(*c)) {
        return None;
    }

    let text: String = chars.iter().collect();
    if !is_strong_mixed_math_candidate(chars, &text) {
        return None;
    }
    if !is_math_expression(chars, &text) {
        return None;
    }
    // math engine이 처리하지 못하는 패턴(예: combining macron이 있는 순환소수
    // `2̄.3010`)은 일반 encode로 fallback한다. 일반 encode는 char-level 룰을
    // 거쳐 같은 결과를 산출한다.
    if let Ok(bytes) = math::encoder::encode_math_expression(&text) {
        return Some(bytes);
    }
    crate::encode(&text).ok()
}

fn is_mixed_math_expression(chars: &[char], text: &str) -> bool {
    let has_korean = chars.iter().any(|c| is_korean_char(*c));
    let has_root = chars.contains(&'√');
    let has_parens = chars.iter().any(|c| matches!(*c, '(' | ')'));
    let has_math_op = chars
        .iter()
        .any(|c| matches!(*c, '=' | '+' | '/' | '×' | '÷'));

    // 좁힌 trigger:
    // (1) 분수 패턴: 분수 묶음 안에 한글 있을 때만 mixed math 분수 처리 (라인 17 자연수).
    //     `tan의 값은 2/(3+√5)`처럼 괄호 안 숫자만 있는 분수는 baseline 일반 path가 더 정답.
    // (2) √ 한글 직접 인접 패턴 (라인 18 `√분산`).
    // (3) 한글 명사구 + 수식 연산: `원의 둘레 = 반지름 × ...` (라인 12).
    //     — 한글 명사구는 공백으로 구분된 한글 단어. 일반 산식 `5개−3개=2개`은 공백 없음.
    let fraction_with_korean =
        has_parens && has_math_op && (text.contains("/(") || text.contains(")/")) && {
            // 괄호 안 한글 여부 확인 — `(`와 매칭되는 `)` 사이 한글 있어야
            let mut depth = 0i32;
            let mut korean_in_parens = false;
            for c in chars {
                match *c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ if depth > 0 && is_korean_char(*c) => korean_in_parens = true,
                    _ => {}
                }
            }
            korean_in_parens
        };

    let root_with_korean = has_root
        && chars
            .windows(2)
            .any(|w| w[0] == '√' && is_korean_char(w[1]));

    let multi_word_korean_phrase = chars
        .windows(3)
        .any(|w| is_korean_char(w[0]) && w[1] == ' ' && is_korean_char(w[2]));

    // BMI 같은 영문자 + 한글 mixed 입력은 baseline의 일반 한국어 점역이 옳다.
    // multi-word Korean 분기는 한글 명사구만 있는 입력으로 제한.
    let has_english_letter = chars.iter().any(|c| c.is_ascii_alphabetic());

    has_korean
        && (fraction_with_korean
            || root_with_korean
            || (multi_word_korean_phrase && has_math_op && !has_english_letter))
}

fn try_encode_mixed_math_slice(chars: &[char]) -> Option<Vec<u8>> {
    if chars.is_empty() {
        return None;
    }

    let text: String = chars.iter().collect();
    if !is_mixed_math_expression(chars, &text) {
        return None;
    }

    math::encoder::encode_math_expression(&text).ok()
}

fn try_encode_mixed_math_prefix(prefix: &[char], suffix: &[char]) -> Option<Vec<u8>> {
    if let Some(bytes) = try_encode_math_slice(prefix) {
        let text: String = prefix.iter().collect();
        if !suffix.is_empty()
            && suffix.iter().all(|c| is_korean_suffix_char(*c))
            && suffix.iter().any(|c| is_korean_char(*c))
            && math::rule_46::is_trig_function(&text)
        {
            return math::encoder::encode_math_expression(&format!("{text}x")).ok();
        }
        return Some(bytes);
    }

    None
}

fn split_mixed_math_word(
    word: &crate::rules::token::WordToken<'_>,
    leading_delimiter_len: usize,
) -> Option<Vec<Token<'static>>> {
    if !word.meta.has_korean || word.chars.iter().all(|c| is_korean_char(*c)) {
        return None;
    }

    let chars = &word.chars;
    let len = chars.len();

    for end in (1..=len).rev() {
        let Some(bytes) = try_encode_mixed_math_prefix(&chars[..end], &chars[end..]) else {
            continue;
        };

        if end == len {
            return None;
        }

        if !chars[end..].iter().all(|c| is_korean_suffix_char(*c))
            || !chars[end..].iter().any(|c| is_korean_char(*c))
        {
            continue;
        }

        let suffix: String = chars[end..].iter().collect();
        return Some(vec![
            Token::PreEncoded(vec![0; leading_delimiter_len]),
            Token::PreEncoded(bytes),
            Token::PreEncoded(vec![0, 0]),
            build_word_token(suffix),
        ]);
    }

    None
}

/// Check if a word is a math expression.
fn is_math_expression(chars: &[char], text: &str) -> bool {
    if is_rule_68_compact_notation(chars) {
        return false;
    }

    if chars.len() == 1
        && matches!(
            chars[0],
            '+' | '=' | '−' | '×' | '÷' | '<' | '>' | '≠' | '≥' | '≤'
        )
    {
        return true;
    }

    if chars.len() == 1 && crate::fraction::is_unicode_fraction(chars[0]) {
        return true;
    }

    if chars.len() == 2 && matches!(chars[0], '-' | '\u{2212}') && chars[1] == '\u{221E}' {
        return true;
    }

    // Must NOT contain Korean characters
    for c in chars {
        let code = *c as u32;
        if (0xAC00..=0xD7A3).contains(&code) || (0x3131..=0x3163).contains(&code) {
            return false;
        }
    }

    let has_letters = chars.iter().any(|c| c.is_ascii_alphabetic());
    let has_digits = chars.iter().any(|c| c.is_ascii_digit());
    let has_math_symbol = chars
        .iter()
        .any(|c| math_symbol_shortcut::is_math_symbol_char(*c));
    let has_strong_math_symbol = chars.iter().any(|c| {
        math_symbol_shortcut::is_math_symbol_char(*c)
            && !matches!(*c, '\u{00B7}' | '\u{22C5}' | '/')
    });
    let has_superscript = chars.iter().any(|c| is_superscript(*c));
    let has_subscript = chars.iter().any(|c| is_subscript(*c));
    let has_combining_mark = chars.iter().any(|c| is_combining_math_mark(*c));
    let has_function = math::function::starts_with_function(text);
    let has_math_operator = chars.iter().any(|c| {
        matches!(
            c,
            '+' | '=' | '>' | '<' | '.' | ',' | '-' | '\u{2212}' | '/' | '!'
        )
    });
    let has_brackets = chars
        .iter()
        .any(|c| matches!(c, '(' | ')' | '[' | ']' | '{' | '}'));
    let starts_with_math_symbol = chars
        .first()
        .is_some_and(|c| math_symbol_shortcut::is_math_symbol_char(*c));

    // Number-base notation like 1010₂ is a math expression and should use the math engine.
    if chars.first().is_some_and(|c| c.is_ascii_digit())
        && chars.iter().any(|c| matches!(*c, '\u{2080}'..='\u{2089}'))
        && chars
            .iter()
            .all(|c| c.is_ascii_digit() || matches!(*c, '\u{2080}'..='\u{2089}'))
    {
        return true;
    }

    // Common phone/date/range tokens like 02-799-1000 should stay non-math.
    if !has_letters
        && chars
            .iter()
            .all(|c| c.is_ascii_digit() || matches!(c, '-' | '~' | '(' | ')' | ','))
        && !chars
            .first()
            .is_some_and(|c| matches!(*c, '-' | '\u{2212}'))
    {
        return false;
    }

    // PDF 제43항: 숫자 사이에 마침표(소수점)는 일반 수표(⠼)로 처리.
    // 첫 글자가 숫자인 순수 소수(96.7, 3.14 등)는 한글 점자 number rule로 처리.
    // ".47"처럼 점으로 시작하는 형태는 math expression으로 처리.
    if !has_letters
        && chars.first().is_some_and(|c| c.is_ascii_digit())
        && chars.iter().all(|c| c.is_ascii_digit() || *c == '.')
    {
        return false;
    }

    // Slash-only numeric tokens: 2-part (N/M) is a fraction expression for any digit count;
    // 3-or-more parts (e.g. 2024/12/31) is a date/range and stays non-math.
    if !has_letters && chars.contains(&'/') && chars.iter().all(|c| c.is_ascii_digit() || *c == '/')
    {
        let parts: Vec<&str> = text.split('/').collect();
        if parts.len() == 2
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        {
            return true;
        }
        return false;
    }

    // Function names are math expressions when they have additional content after them.
    if has_function
        && let Some((name, _)) = math::function::match_function_prefix(text)
        && (chars.len() > name.len() || text == name)
    {
        return true;
    }

    // Inverse trig text forms like arcsinA / arccosx
    if let Some(rest) = text.strip_prefix("arc")
        && ["sin", "cos", "tan"]
            .iter()
            .any(|name| rest.starts_with(name))
    {
        return true;
    }

    // Relation shorthand like aRb should be treated as math.
    if chars.len() == 3
        && chars[0].is_ascii_lowercase()
        && chars[1].is_ascii_uppercase()
        && chars[2].is_ascii_lowercase()
    {
        return true;
    }

    // Plain English list tokens/punctuation in prose should remain non-math.
    if has_letters
        && !has_digits
        && !has_strong_math_symbol
        && !has_superscript
        && !has_subscript
        && chars
            .iter()
            .all(|c| c.is_ascii_alphabetic() || matches!(*c, ',' | '.' | '\'' | '"'))
    {
        return false;
    }

    // Parenthesized single-letter list item in prose: (a), (b), ...
    if chars.len() >= 3
        && chars.first() == Some(&'(')
        && chars.get(1).is_some_and(|c| c.is_ascii_alphabetic())
        && chars.get(2) == Some(&')')
        && chars
            .get(3)
            .is_none_or(|c| matches!(*c, ',' | '.' | '\'' | '"'))
    {
        return false;
    }

    // Superscript/subscript with letters or digits (like "x²", "aₙ")
    if (has_superscript || has_subscript) && (has_letters || has_digits) {
        return true;
    }

    // PDF 수학 제62항 — 첨자가 수학 기호(Greek 등)와 함께 등장하면 수식이다.
    // 예: ₙΠᵣ (중복순열).
    if (has_superscript || has_subscript) && has_strong_math_symbol {
        return true;
    }

    if has_combining_mark && (has_letters || has_digits) {
        return true;
    }

    // Math operators mixed with letters/digits.
    if has_math_operator && has_letters {
        let trailing_slash_word = chars.last() == Some(&'/')
            && chars
                .iter()
                .all(|c| c.is_ascii_alphabetic() || matches!(*c, '/' | '\''));
        if trailing_slash_word {
            return false;
        }
        return true;
    }
    if has_math_operator && has_digits {
        return true;
    }

    // Math symbols with letters/digits and symbol-leading tokens.
    if has_math_symbol && has_letters && has_digits {
        return true;
    }
    if has_math_symbol && has_digits {
        return true;
    }

    if has_strong_math_symbol && has_letters {
        return true;
    }
    if has_strong_math_symbol && has_digits {
        return true;
    }
    if starts_with_math_symbol && has_digits {
        return true;
    }
    // Slash between letters indicates fraction (F/N, a/b) — but not trailing slash (a/)
    if has_letters && chars.contains(&'/') {
        let has_letter_before_slash = chars
            .windows(2)
            .any(|w| w[0].is_ascii_alphabetic() && w[1] == '/');
        let has_letter_after_slash = chars
            .windows(2)
            .any(|w| w[0] == '/' && w[1].is_ascii_alphabetic());
        if has_letter_before_slash && has_letter_after_slash {
            return true;
        }
    }

    // Signed numeric/math tokens (e.g. -3, -1<x) should be handled as math.
    if has_digits && chars.iter().any(|c| matches!(*c, '-' | '\u{2212}')) {
        return true;
    }

    // Bracket-containing words with digits are math.
    if has_brackets && has_digits {
        return true;
    }
    // Bracket-containing words with letters + other math indicators are math.
    if has_brackets
        && has_letters
        && (has_math_operator || has_math_symbol || has_superscript || has_subscript)
    {
        return true;
    }
    // Function-call patterns like f(x), p(x), g(n) — letter(s) followed by open paren.
    // But NOT purely parenthesized words like "(kg)" where paren wraps the whole word.
    if has_brackets && has_letters {
        let text_bytes = text.as_bytes();
        // Check if there's a letter immediately before '(' — function call pattern
        let has_func_call = text_bytes
            .windows(2)
            .any(|w| (w[0] as char).is_ascii_alphabetic() && w[1] == b'(');
        if has_func_call {
            return true;
        }
    }

    // Digit-then-letter transition at start of word (like "3ab" → math multiplication)
    // But NOT letter-then-digit (like "MP3" which is NOT math)
    if chars.len() >= 2 && chars[0].is_ascii_digit() {
        // PDF 제69항: 숫자+단위 (180cm, 5kg, 1in 등)은 math가 아닌 단위 표기로 처리.
        if let Some((_, _, consumed)) =
            crate::rules::korean::rule_69::parse_numeric_ascii_unit_prefix(chars)
            && consumed == chars.len()
        {
            return false;
        }
        let has_letter_after_digit = chars.iter().skip(1).any(|c| c.is_ascii_lowercase());
        if has_letter_after_digit {
            return true;
        }
    }

    false
}

impl TokenRule for MathExpressionTokenRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::FractionDetection
    }

    fn priority(&self) -> u16 {
        50 // Before InlineFractionRule (120) and LatexFractionRule
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        _state: &mut EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        fn prev_next_words<'a>(
            tokens: &'a [Token<'a>],
            index: usize,
        ) -> (
            Option<&'a crate::rules::token::WordToken<'a>>,
            Option<&'a crate::rules::token::WordToken<'a>>,
        ) {
            let prev = index.checked_sub(1).and_then(|mut i| {
                loop {
                    match tokens.get(i) {
                        Some(Token::Space(_)) => i = i.checked_sub(1)?,
                        Some(Token::Word(w)) => return Some(w),
                        _ => return None,
                    }
                }
            });
            let next = {
                let mut i = index + 1;
                loop {
                    match tokens.get(i) {
                        Some(Token::Space(_)) => i += 1,
                        Some(Token::Word(w)) => break Some(w),
                        _ => break None,
                    }
                }
            };
            (prev, next)
        }

        fn is_logic_symbol_word(word: &crate::rules::token::WordToken<'_>) -> bool {
            word.chars
                .first()
                .is_some_and(|c| word.chars.len() == 1 && matches!(*c, '⊻'))
        }

        let Some(Token::Word(word)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };

        let text = word.text.as_ref();

        // PDF — `...` 또는 `..., `, `..`은 math context에 있으면 수학 줄임표 `⠠⠠⠠`로 emit.
        // Korean 마침표 줄임표 `⠲⠲⠲`와 구분.
        let dot_only = !text.is_empty()
            && (text.chars().all(|c| matches!(c, '.' | ','))
                && text.contains('.'));
        if dot_only {
            // PDF — 앞 토큰이 math letter Word 또는 이미 인코딩된 PreEncoded(math 컨텍스트)면
            // 수학 줄임표로 emit. PreEncoded는 이전 math 처리 결과로 본다.
            let prev_is_math_context = {
                let mut i = index;
                let mut found = false;
                loop {
                    if i == 0 {
                        break;
                    }
                    i -= 1;
                    match tokens.get(i) {
                        Some(Token::Space(_)) => continue,
                        Some(Token::PreEncoded(_)) => {
                            found = true;
                            break;
                        }
                        Some(Token::Word(w)) => {
                            found = w.chars.iter().any(|c| matches!(*c,
                                '\u{2080}'..='\u{2089}' | '\u{00B2}' | '\u{00B3}' | '\u{2070}'..='\u{2079}'
                            )) || (w.chars.first().is_some_and(|c| c.is_ascii_alphabetic())
                                && w.chars.iter().all(|c| c.is_ascii_alphabetic() || matches!(*c, ',' | '₀'..='₉')));
                            break;
                        }
                        _ => break,
                    }
                }
                found
            };
            if prev_is_math_context {
                let mut bytes = Vec::new();
                let dots: usize = text.chars().filter(|c| *c == '.').count();
                for _ in 0..dots.min(3) {
                    bytes.push(32); // ⠠
                }
                // 다음 토큰이 Korean Word면 math+Korean 경계로 trailing space 추가.
                let next_is_korean = {
                    let mut i = index + 1;
                    loop {
                        match tokens.get(i) {
                            Some(Token::Space(_)) => i += 1,
                            Some(Token::Word(w)) => break w.meta.has_korean,
                            _ => break false,
                        }
                    }
                };
                if text.ends_with(',') {
                    // PDF — math 식 안 comma는 ⠐, prose math letter 리스트의 comma는 ⠂.
                    // 다음이 math 또는 PreEncoded면 ⠐, Korean이면 ⠂.
                    bytes.push(if next_is_korean { 2 } else { 16 });
                }
                if next_is_korean {
                    bytes.push(0);
                }
                return Ok(TokenAction::Replace(Token::PreEncoded(bytes)));
            }
        }

        // Numeric middle-dot forms in Korean prose (e.g. 3·1 운동) should stay non-math,
        // while standalone numeric expressions like 6·9 should be routed to math.
        if is_middle_dot_numeric_word(&word.chars) && has_adjacent_korean_word(tokens, index) {
            return Ok(TokenAction::Noop);
        }

        // Standalone therefore/because between content tokens (Word or PreEncoded)
        // should add one braille space on each side. Combined with the Space tokens
        // already present between words, this produces the double-space delimiter
        // required by 제11항.
        if matches!(word.chars.as_slice(), ['∴' | '∵']) {
            let has_prev_content = index
                .checked_sub(1)
                .and_then(|mut i| {
                    loop {
                        match tokens.get(i) {
                            Some(Token::Space(_)) => i = i.checked_sub(1)?,
                            Some(Token::Word(_) | Token::PreEncoded(_)) => return Some(true),
                            _ => return None,
                        }
                    }
                })
                .unwrap_or(false);
            let has_next_content = {
                let mut i = index + 1;
                loop {
                    match tokens.get(i) {
                        Some(Token::Space(_)) => i += 1,
                        Some(Token::Word(_) | Token::PreEncoded(_)) => break true,
                        _ => break false,
                    }
                }
            };
            if has_prev_content && has_next_content {
                let encoded =
                    math_symbol_shortcut::encode_char_math_symbol_shortcut(word.chars[0])?;
                let mut out = vec![0];
                out.extend_from_slice(encoded);
                out.push(0);
                return Ok(TokenAction::Replace(Token::PreEncoded(out)));
            }
        }

        // Logical symbols separated by spaces should still treat uppercase letters as variables.
        if word.chars.len() == 1 && word.chars[0].is_ascii_uppercase() {
            let (prev, next) = prev_next_words(tokens, index);
            if prev.is_some_and(is_logic_symbol_word) || next.is_some_and(is_logic_symbol_word) {
                let code = crate::english::encode_english(word.chars[0].to_ascii_lowercase())?;
                return Ok(TokenAction::Replace(Token::PreEncoded(vec![code])));
            }
        }

        // Skip if already processed (PreEncoded) or if it's a fraction
        if let Some(stripped) = text.strip_prefix('$') {
            if let Some(close_idx) = stripped.find('$')
                && close_idx + 1 < stripped.len()
            {
                let latex = &text[..=close_idx + 1];
                let suffix = &stripped[close_idx + 1..];

                if let Some((whole, numerator, denominator)) =
                    crate::fraction::parse_latex_fraction(latex)
                {
                    // 제44항 [다만]: 분수 직후 한국어 조사의 첫 초성이 ㄴ/ㄷ/ㅁ/ㅋ/ㅌ/ㅍ/ㅎ
                    // 또는 '운'으로 시작하면 띄어 쓴다.
                    let mut replacement: Vec<Token<'a>> =
                        vec![Token::Fraction(crate::rules::token::FractionToken {
                            whole,
                            numerator,
                            denominator,
                        })];
                    if !suffix.is_empty() && rule_44_requires_space_before_korean(suffix) {
                        replacement.push(Token::Space(crate::rules::token::SpaceKind::Regular));
                    }
                    replacement.push(build_word_token(suffix.to_string()));
                    return Ok(TokenAction::ReplaceMany(replacement));
                }

                let inner = &latex[1..latex.len() - 1];
                if let Ok(bytes) =
                    crate::rules::token_rules::latex_math::encode_latex_math_bytes(inner)
                {
                    // PDF — Korean prose 안 단일 letter math 블록은 ⠴...⠲로 감싼다.
                    // 콤마-구분 letter 리스트도 quote/english marker로 감싼다.
                    let suffix_first = suffix.chars().next();
                    let suffix_is_korean = suffix_first.is_some_and(crate::utils::is_korean_char);
                    let inner_is_single_letter = inner.chars().count() == 1
                        && inner.chars().all(|c| c.is_ascii_alphabetic());
                    let comma_list = inner.contains(',')
                        && inner
                            .split(',')
                            .map(str::trim)
                            .all(|p| !p.is_empty() && p.chars().count() == 1 && p.chars().all(|c| c.is_ascii_alphabetic()));
                    let prev_is_korean = index
                        .checked_sub(1)
                        .and_then(|i| tokens.get(i))
                        .map(|tok| match tok {
                            Token::Word(w) => w.meta.has_korean,
                            Token::Space(_) => index.checked_sub(2)
                                .and_then(|j| tokens.get(j))
                                .is_some_and(|t| matches!(t, Token::Word(w) if w.meta.has_korean)),
                            _ => false,
                        })
                        .unwrap_or(false);
                    let in_prose = suffix_is_korean || prev_is_korean;
                    let leading_spaces = if in_prose && (inner_is_single_letter || comma_list) {
                        0 // 따옴표 자체가 경계를 명시.
                    } else if index == 0 {
                        0
                    } else if matches!(tokens.get(index - 1), Some(Token::Space(_))) {
                        // Space token이 이미 1칸 공백을 제공. prev-prev가 math/PreEncoded면
                        // 추가 공백 없이, Korean Word면 1칸 추가하여 총 2칸 boundary.
                        let prev_prev = index.checked_sub(2).and_then(|i| tokens.get(i));
                        match prev_prev {
                            Some(Token::Word(w)) if w.meta.has_korean => 1,
                            _ => 0,
                        }
                    } else {
                        2
                    };
                    let mut replacement = Vec::new();
                    if leading_spaces > 0 {
                        replacement.push(Token::PreEncoded(vec![0; leading_spaces]));
                    }
                    if in_prose && inner_is_single_letter {
                        let mut wrapped = Vec::with_capacity(bytes.len() + 2);
                        wrapped.push(52); // ⠴
                        wrapped.extend(bytes);
                        wrapped.push(50); // ⠲
                        replacement.push(Token::PreEncoded(wrapped));
                    } else if in_prose && comma_list {
                        let letters: Vec<&str> = inner.split(',').map(str::trim).collect();
                        let mut wrapped = Vec::new();
                        for (i, letter) in letters.iter().enumerate() {
                            if let Some(c) = letter.chars().next() {
                                if i == 0 {
                                    wrapped.push(52);
                                } else {
                                    wrapped.push(0);
                                    wrapped.push(48); // ⠰ english
                                }
                                if c.is_ascii_uppercase() {
                                    wrapped.push(32);
                                    if let Ok(code) = crate::english::encode_english(c.to_ascii_lowercase()) {
                                        wrapped.push(code);
                                    }
                                } else if let Ok(code) = crate::english::encode_english(c) {
                                    wrapped.push(code);
                                }
                                if i + 1 < letters.len() {
                                    wrapped.push(2); // ⠂ literal comma (in math letter list)
                                } else {
                                    wrapped.push(50);
                                }
                            }
                        }
                        replacement.push(Token::PreEncoded(wrapped));
                    } else {
                        replacement.push(Token::PreEncoded(bytes));
                        // PDF — math + Korean prose 경계는 두 칸. 구두점/기호 suffix는 인접.
                        let trailing_spaces = if suffix_is_korean { 2 } else { 0 };
                        if trailing_spaces > 0 {
                            replacement.push(Token::PreEncoded(vec![0; trailing_spaces]));
                        }
                    }
                    replacement.push(build_word_token(suffix.to_string()));
                    return Ok(TokenAction::ReplaceMany(replacement));
                }
            }

            if let Some((whole, numerator, denominator)) =
                crate::fraction::parse_latex_fraction(text)
            {
                return Ok(TokenAction::Replace(Token::Fraction(
                    crate::rules::token::FractionToken {
                        whole,
                        numerator,
                        denominator,
                    },
                )));
            }

            if text.ends_with('$') && text.len() >= 3 {
                let inner = &text[1..text.len() - 1];
                if let Ok(bytes) =
                    crate::rules::token_rules::latex_math::encode_latex_math_bytes(inner)
                {
                    let mut replacement =
                        crate::rules::token_rules::latex_math::wrap_latex_math_tokens_with_inner(
                            tokens, index, bytes, inner,
                        );
                    if inner.contains("\\begin{vmatrix}")
                        && matches!(
                            index.checked_sub(1).and_then(|i| tokens.get(i)),
                            Some(Token::Space(_))
                        )
                    {
                        replacement.insert(0, Token::PreEncoded(vec![0]));
                    }
                    return Ok(TokenAction::ReplaceMany(replacement));
                }
            }

            return Ok(TokenAction::Noop);
        }

        if !is_math_expression(&word.chars, text) {
            if let Some(bytes) = try_encode_mixed_math_slice(&word.chars) {
                return Ok(TokenAction::Replace(Token::PreEncoded(bytes)));
            }
            // 제11항: 한글 문장 안의 수학적 표기는 앞뒤를 두 칸씩 띄어 쓴다.
            // - index == 0           → 0칸 (문서 맨 앞)
            // - 이전 토큰이 Space    → 1칸 추가 (Token::Space 1칸 + 새 1칸 = 2칸)
            //   다만 prev-prev가 같은 math/mixed math 단어이면 0 (1칸 유지)
            // - 그 외 (content)     → 2칸 (경계 표시)
            let prev_prev_is_math_or_mixed = {
                let mut i = index;
                let mut found_space = false;
                let mut result = false;
                loop {
                    if i == 0 { break; }
                    i -= 1;
                    match tokens.get(i) {
                        Some(Token::Space(_)) => { found_space = true; }
                        // PreEncoded는 이미 math/mixed가 인코딩된 결과이므로 math 컨텍스트로 본다.
                        Some(Token::PreEncoded(_) | Token::Fraction(_)) if found_space => {
                            result = true;
                            break;
                        }
                        Some(Token::Word(w)) if found_space => {
                            let is_math = is_math_expression(&w.chars, w.text.as_ref())
                                || (w.meta.has_korean
                                    && is_strong_mixed_math_candidate(&w.chars, w.text.as_ref()));
                            result = is_math;
                            break;
                        }
                        _ => break,
                    }
                }
                result
            };
            let leading_delimiter_len = if index == 0 {
                0
            } else if matches!(tokens.get(index - 1), Some(Token::Space(_))) {
                if prev_prev_is_math_or_mixed { 0 } else { 1 }
            } else {
                2
            };
            if let Some(replacement) = split_mixed_math_word(word, leading_delimiter_len) {
                return Ok(TokenAction::ReplaceMany(replacement));
            }
            return Ok(TokenAction::Noop);
        }

        // Try to encode via math engine
        match math::encoder::encode_math_expression(text) {
            Ok(bytes) => {
                let (prev_has_korean, _next_has_korean) = adjacent_korean_word_flags(tokens, index);
                let mut wrapped = Vec::with_capacity(bytes.len() + 2);

                let needs_decimal_context_spacing = text.contains('')
                    || text.contains('⋯')
                    || word.chars.iter().any(|ch| is_combining_math_mark(*ch));
                if needs_decimal_context_spacing
                    && matches!(
                        index.checked_sub(1).and_then(|i| tokens.get(i)),
                        Some(Token::Space(_))
                    )
                {
                    wrapped.push(0);
                }

                // 특수 패턴(증분 + 등호 + 다항식 조합)에만 prefix space 두 칸 추가.
                // 일반적인 한글 + math 인접 케이스는 Token::Space가 단일 공백을 처리하므로
                // 추가 prefix/suffix space를 emit하지 않는다.
                // 문서 맨 앞(index == 0)에서는 제11조에 따라 leading 띄어쓰기를 생략한다.
                if index != 0
                    && !prev_has_korean
                    && text.contains('\u{2206}')
                    && text.contains('=')
                    && text.contains(")+(")
                {
                    wrapped.push(0);
                    wrapped.push(0);
                }

                // PDF 수학 제11항 — 국어 문장 안 "수식"은 앞뒤 두 칸씩 띄어쓴다.
                // 단일 연산자/기호(+, =, ×, ÷, /, - 등)는 일반 산식 일부이므로 제외한다.
                // 변수/숫자/괄호 등 실질적 수식(`f(x)`, `a²`, `2x+3` 등)일 때만 적용.
                // 단순 부호+숫자(`-2`, `+3`, `0.5` 등)는 일반 숫자 표기이므로
                // 추가 띄어쓰기를 적용하지 않는다. 첨자/괄호/문자가 있으면 실질적 수식.
                let only_simple_digits = !word.chars.is_empty()
                    && word.chars.iter().all(|c| {
                        c.is_ascii_digit()
                            || matches!(*c, '-' | '+' | '\u{2212}' | '.' | ',')
                    });
                let is_substantial_math = word.chars.len() > 1
                    && word.chars.iter().any(|c| {
                        c.is_ascii_alphanumeric()
                            || matches!(*c, '(' | ')' | '[' | ']' | '|')
                    })
                    && !only_simple_digits;
                let needs_korean_leading = index != 0
                    && prev_has_korean
                    && matches!(tokens.get(index - 1), Some(Token::Space(_)))
                    && !needs_decimal_context_spacing
                    && is_substantial_math;
                if needs_korean_leading {
                    wrapped.push(0);
                }

                wrapped.extend_from_slice(&bytes);

                if needs_decimal_context_spacing
                    && matches!(tokens.get(index + 1), Some(Token::Space(_)))
                {
                    wrapped.push(0);
                }

                // trailing은 다음 단어가 순수 한글일 때만 추가. (인접 단어가 math+korean
                // 혼합이면 다음 단어 측에서 leading을 추가하므로 중복 방지.)
                let next_is_pure_korean = {
                    let mut i = index + 1;
                    loop {
                        match tokens.get(i) {
                            Some(Token::Space(_)) => i += 1,
                            Some(Token::Word(w)) => {
                                let has_kor = w.meta.has_korean;
                                let all_kor = w.chars.iter().all(|c| {
                                    let code = *c as u32;
                                    (0xAC00..=0xD7A3).contains(&code)
                                        || (0x3131..=0x3163).contains(&code)
                                        || matches!(*c, '.' | ',' | '!' | '?' | ' ')
                                });
                                break has_kor && all_kor;
                            }
                            _ => break false,
                        }
                    }
                };
                if next_is_pure_korean
                    && matches!(tokens.get(index + 1), Some(Token::Space(_)))
                    && !needs_decimal_context_spacing
                    && is_substantial_math
                {
                    wrapped.push(0);
                }

                Ok(TokenAction::Replace(Token::PreEncoded(wrapped)))
            }
            Err(_) => {
                // If math encoding fails, let the character-level rules handle it
                Ok(TokenAction::Noop)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::token::WordMeta;
    use std::borrow::Cow;

    #[test]
    fn test_is_math_with_operator() {
        let chars: Vec<char> = "ax+b=0".chars().collect();
        assert!(is_math_expression(&chars, "ax+b=0"));
    }

    #[test]
    fn test_is_math_with_function() {
        let chars: Vec<char> = "sin3x".chars().collect();
        assert!(is_math_expression(&chars, "sin3x"));
    }

    #[test]
    fn test_is_math_with_standalone_function_name() {
        let chars: Vec<char> = "sin".chars().collect();
        assert!(is_math_expression(&chars, "sin"));
    }

    #[test]
    fn test_is_not_math_korean() {
        let chars: Vec<char> = "안녕".chars().collect();
        assert!(!is_math_expression(&chars, "안녕"));
    }

    #[test]
    fn test_is_not_math_plain_english() {
        let chars: Vec<char> = "hello".chars().collect();
        assert!(!is_math_expression(&chars, "hello"));
    }

    #[test]
    fn test_is_math_with_superscript() {
        let chars: Vec<char> = "x²".chars().collect();
        assert!(is_math_expression(&chars, "x²"));
    }

    #[test]
    fn test_is_math_digit_letter_with_operator() {
        // "3a+b" has digit-letter AND operator → math
        let chars: Vec<char> = "3a+b".chars().collect();
        assert!(is_math_expression(&chars, "3a+b"));
    }

    #[test]
    fn test_is_math_digit_then_letter() {
        // "3ab" starts with digit then letters → math multiplication
        let chars: Vec<char> = "3ab".chars().collect();
        assert!(is_math_expression(&chars, "3ab"));
    }

    #[test]
    fn test_is_not_math_letter_then_digit() {
        // "MP3" starts with letters then digit → NOT math (avoids false positive)
        let chars: Vec<char> = "MP3".chars().collect();
        assert!(!is_math_expression(&chars, "MP3"));
    }

    #[test]
    fn test_is_math_symbol_digit_combo() {
        let chars: Vec<char> = "≠0".chars().collect();
        assert!(is_math_expression(&chars, "≠0"));
    }

    #[test]
    fn test_decimal_starting_with_digit_is_not_math() {
        // PDF 제43항: 첫 글자가 숫자인 순수 소수는 한글 number rule로 처리.
        let chars: Vec<char> = "0.17".chars().collect();
        assert!(!is_math_expression(&chars, "0.17"));
        let chars: Vec<char> = "96.7".chars().collect();
        assert!(!is_math_expression(&chars, "96.7"));
    }

    #[test]
    fn test_decimal_starting_with_dot_is_math() {
        // ".47"처럼 점으로 시작하는 형태는 math expression.
        let chars: Vec<char> = ".47".chars().collect();
        assert!(is_math_expression(&chars, ".47"));
    }

    #[test]
    fn test_is_math_relation_shorthand() {
        let chars: Vec<char> = "aRb".chars().collect();
        assert!(is_math_expression(&chars, "aRb"));
    }

    #[test]
    fn test_is_math_negative_infinity() {
        let chars: Vec<char> = "-∞".chars().collect();
        assert!(is_math_expression(&chars, "-∞"));
    }

    #[test]
    fn test_is_math_unicode_fraction_char() {
        let chars: Vec<char> = "⅔".chars().collect();
        assert!(is_math_expression(&chars, "⅔"));
    }

    #[test]
    fn test_is_math_base_notation() {
        let chars: Vec<char> = "1010₂".chars().collect();
        assert!(is_math_expression(&chars, "1010₂"));
    }

    #[test]
    fn split_mixed_math_word_extracts_math_prefix() {
        let chars: Vec<char> = "tan의".chars().collect();
        let word = crate::rules::token::WordToken {
            text: Cow::Borrowed("tan의"),
            chars: chars.clone(),
            meta: WordMeta::from_chars(&chars),
        };

        let replacement = split_mixed_math_word(&word, 2).expect("expected split");
        assert!(matches!(replacement[0], Token::PreEncoded(ref bytes) if bytes == &vec![0, 0]));
        assert!(matches!(replacement[1], Token::PreEncoded(_)));
        assert!(matches!(replacement[2], Token::PreEncoded(ref bytes) if bytes == &vec![0, 0]));
        assert!(matches!(&replacement[3], Token::Word(w) if w.text == "의"));
    }

    #[test]
    fn split_mixed_math_word_keeps_plain_mixed_english_korean() {
        let chars: Vec<char> = "ATM에서".chars().collect();
        let word = crate::rules::token::WordToken {
            text: Cow::Borrowed("ATM에서"),
            chars: chars.clone(),
            meta: WordMeta::from_chars(&chars),
        };

        assert!(split_mixed_math_word(&word, 2).is_none());
    }
}
