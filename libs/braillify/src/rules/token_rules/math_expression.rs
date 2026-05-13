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
        '\u{2070}' | '\u{00B9}' | '\u{00B2}' | '\u{00B3}' | '\u{2074}'
            ..='\u{2079}'
                | '\u{207A}'
                | '\u{207B}'
                | '\u{207D}'
                | '\u{207E}'
                | '\u{207F}'
                | '\u{1D4F}'
                | '\u{1D50}'
                | '\u{02E3}'
                | '\u{1D9C}'
    )
}

/// Check if a character is a Unicode subscript.
fn is_subscript(c: char) -> bool {
    matches!(
        c,
        '\u{2080}'
            ..='\u{2089}'
                | '\u{208A}'
                | '\u{208B}'
                | '\u{208D}'
                | '\u{208E}'
                | '\u{2090}'
                | '\u{2093}'
                | '\u{2098}'
                | '\u{2099}'
    )
}

fn is_combining_math_mark(c: char) -> bool {
    matches!(
        c,
        '\u{0305}' | '\u{0307}' | '\u{0308}' | '\u{0309}' | '\u{030A}' | '\u{0332}'
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

    starts_with_function
        || starts_with_root
        || is_absolute_value_form
        || has_superscript
        || has_subscript
        || has_combining_mark
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

    math::encoder::encode_math_expression(&text).ok()
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
    let fraction_with_korean = has_parens
        && has_math_op
        && (text.contains("/(") || text.contains(")/"))
        && {
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

    let multi_word_korean_phrase = chars.windows(3).any(|w| {
        is_korean_char(w[0]) && w[1] == ' ' && is_korean_char(w[2])
    });

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
                    return Ok(TokenAction::ReplaceMany(vec![
                        Token::Fraction(crate::rules::token::FractionToken {
                            whole,
                            numerator,
                            denominator,
                        }),
                        build_word_token(suffix.to_string()),
                    ]));
                }

                let inner = &latex[1..latex.len() - 1];
                let math_text = crate::rules::token_rules::latex_math::strip_latex_to_math(inner);
                if let Ok(bytes) = math::encoder::encode_math_expression(&math_text) {
                    return Ok(TokenAction::ReplaceMany(vec![
                        Token::PreEncoded(bytes),
                        build_word_token(suffix.to_string()),
                    ]));
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
                let math_text = crate::rules::token_rules::latex_math::strip_latex_to_math(inner);
                if let Ok(bytes) = math::encoder::encode_math_expression(&math_text) {
                    return Ok(TokenAction::Replace(Token::PreEncoded(bytes)));
                }
            }

            return Ok(TokenAction::Noop);
        }

        if !is_math_expression(&word.chars, text) {
            if let Some(bytes) = try_encode_mixed_math_slice(&word.chars) {
                return Ok(TokenAction::Replace(Token::PreEncoded(bytes)));
            }
            // 제11항: 한글 문장 안의 수학적 표기는 앞뒤를 두 칸씩 띄어 쓴다.
            // 다만 문서의 맨 앞(index == 0)에서는 앞쪽 띄어쓰기를 생략한다.
            // - index == 0          → 0칸 (leading space 없음)
            // - 이전 토큰이 Space    → 1칸 추가 (기존 1칸 + 새 1칸 = 2칸)
            // - 그 외 (content)     → 2칸 (경계 표시)
            let leading_delimiter_len = if index == 0 {
                0
            } else if matches!(tokens.get(index - 1), Some(Token::Space(_))) {
                1
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
                let (prev_has_korean, _next_has_korean) =
                    adjacent_korean_word_flags(tokens, index);
                let mut wrapped = Vec::with_capacity(bytes.len() + 2);

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

                wrapped.extend_from_slice(&bytes);

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
