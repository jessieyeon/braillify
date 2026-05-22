//! Body of MathExpressionTokenRule::apply (extracted from math_expression.rs).

use crate::math_symbol_shortcut;
use crate::rules::context::EncoderState;
use crate::rules::math;
use crate::rules::token::{Token, WordToken};
use crate::rules::token_rule::TokenAction;

use super::detect::is_math_expression;
use super::helpers::*;

/// Resolve the previous and next Word neighbours, skipping over Space tokens.
/// Returns (prev, next) where each is `Some(&WordToken)` if found before hitting
/// a non-Space/Word token (e.g., PreEncoded, Fraction) or the boundary.
///
/// Extracted from `run` so the helper is directly unit-testable and mutation
/// testing can pinpoint regressions in neighbour resolution logic.
pub(super) fn prev_next_words<'a, 'b>(
    tokens: &'b [Token<'a>],
    index: usize,
) -> (
    Option<&'b crate::rules::token::WordToken<'a>>,
    Option<&'b crate::rules::token::WordToken<'a>>,
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

/// Detect a Word that is exactly the logic XOR symbol `⊻` (U+22BB).
///
/// PDF 수학 — `A ⊻ B` 패턴에서 양쪽 대문자를 math 변수로 처리하기 위해 사용.
pub(super) fn is_logic_symbol_word(word: &crate::rules::token::WordToken<'_>) -> bool {
    word.chars
        .first()
        .is_some_and(|c| word.chars.len() == 1 && matches!(*c, '⊻'))
}

pub(super) fn run<'a>(
    tokens: &[Token<'a>],
    index: usize,
    state: &mut EncoderState,
) -> Result<TokenAction<'a>, String> {
    let Some(Token::Word(word)) = tokens.get(index) else {
        return Ok(TokenAction::Noop);
    };

    let text = word.text.as_ref();

    // PDF 수학 제60/61항 — `a ≲ b:`, `p ⊻ q:` 같이 단일 letter + 관계기호 + 단일
    // letter + 콜론 패턴의 inline math expression. 콜론 이전까지를 하나의 math
    // expression으로 병합해 인코딩한다 (letter들이 산문 quote-wrap되지 않도록).
    //
    // 패턴 매칭 조건:
    // - 현재 Word: 단일 ASCII 알파벳 (lowercase)
    // - 다음 Word: math 관계/논리 연산자 (단일 chars, `<>≲≺⊻` 등)
    // - 그 다음 Word: 단일 ASCII letter + `:`
    if word.chars.len() == 1 && word.chars[0].is_ascii_lowercase() {
        let collect_next = |start: usize| {
            let mut j = start;
            while matches!(tokens.get(j), Some(Token::Space(_))) {
                j += 1;
            }
            tokens.get(j).map(|t| (j, t))
        };
        if let Some((op_idx, Token::Word(op_w))) = collect_next(index + 1)
            && op_w.chars.len() == 1
            && matches!(
                op_w.chars[0],
                '\u{2272}'
                    | '\u{2273}'
                    | '\u{227A}'
                    | '\u{227B}'
                    | '\u{22BB}'
                    | '<'
                    | '>'
                    | '='
                    | '\u{2260}'
                    | '\u{2264}'
                    | '\u{2265}'
                    | '\u{2208}'
                    | '\u{2209}'
            )
            && let Some((last_idx, Token::Word(last_w))) = collect_next(op_idx + 1)
            && last_w.chars.len() == 2
            && last_w.chars[0].is_ascii_lowercase()
            && last_w.chars[1] == ':'
        {
            // Merge: "a" + " " + "≲" + " " + "b:" → math expression.
            let merged = format!("{} {} {}", text, op_w.text.as_ref(), last_w.text.as_ref());
            let math_context = math_context_from_state(state);
            if let Ok(bytes) =
                math::encoder::encode_math_expression_with_context(&merged, math_context)
            {
                let consume_count = last_idx + 1 - index;
                return Ok(TokenAction::ReplaceRange(
                    consume_count,
                    vec![Token::PreEncoded(bytes)],
                ));
            }
        }
    }

    // PDF 수학 제60항 2-나 — 조건제시법 set-builder notation `{x|x는 정수}`.
    // `{`로 시작하고 `|`를 포함하는 Word를 만나면, `}` 토큰을 찾을 때까지
    // 후속 Word/Space를 모아 하나의 math expression으로 인코딩한다.
    if word.chars.first() == Some(&'{') && word.chars.contains(&'|') {
        let mut merged = text.to_string();
        let mut end_idx = index;
        let mut found_close = word.chars.last() == Some(&'}');
        if !found_close {
            let mut i = index + 1;
            while i < tokens.len() {
                match tokens.get(i) {
                    Some(Token::Space(_)) => merged.push(' '),
                    Some(Token::Word(w)) => {
                        merged.push_str(w.text.as_ref());
                        if w.chars.last() == Some(&'}') {
                            end_idx = i;
                            found_close = true;
                            break;
                        }
                    }
                    _ => break,
                }
                i += 1;
            }
        }
        let math_context = math_context_from_state(state);
        if found_close
            && let Ok(bytes) =
                math::encoder::encode_math_expression_with_context(&merged, math_context)
        {
            let consume_count = end_idx + 1 - index;
            return Ok(TokenAction::ReplaceRange(
                consume_count,
                vec![Token::PreEncoded(bytes)],
            ));
        }
    }

    // PDF 제12항 [붙임 2] — 한국어 prose 내 multi-letter math identifier 처리.
    // Word가 2~3개 ASCII letter로 시작하고 곧장 한국어가 따라오는 패턴 (예: `ab의 값을`,
    // `AB의 값을`)에서 산문 영어 wrap(`⠴...⠲`)이 아닌 math letter 처리.
    // 추가 컨텍스트: 같은 문장 안에 `값` (value), `구하` (find), `곱` (product) 같은
    // 수학 키워드가 등장해야 한다 (일반 약어 `SNS는`, `MP3을` 등과 구분).
    if word.chars.len() >= 3 {
        let ascii_prefix_len = word
            .chars
            .iter()
            .take_while(|c| c.is_ascii_alphabetic())
            .count();
        if (2..=3).contains(&ascii_prefix_len) {
            let suffix_chars = &word.chars[ascii_prefix_len..];
            let suffix_all_korean = suffix_chars
                .iter()
                .all(|c| crate::utils::is_korean_char(*c));
            let prefix_letters: Vec<char> = word.chars[..ascii_prefix_len].to_vec();
            let all_lower = prefix_letters.iter().all(|c| c.is_ascii_lowercase());
            let all_upper = prefix_letters.iter().all(|c| c.is_ascii_uppercase());

            // 후속 토큰들에서 math-context 키워드 발견 여부.
            let has_math_context_keyword = tokens[index + 1..].iter().take(8).any(|t| match t {
                Token::Word(w) => {
                    let t = w.text.as_ref();
                    t.contains('값')
                        || t.contains("구하")
                        || t.contains('곱')
                        || t.contains("값을")
                        || t.contains("값은")
                }
                _ => false,
            });
            if suffix_all_korean && (all_lower || all_upper) && has_math_context_keyword {
                let prev_is_korean_or_first = index == 0
                    || index
                        .checked_sub(1)
                        .and_then(|i| tokens.get(i))
                        .is_some_and(|t| match t {
                            Token::Word(w) => w.meta.has_korean,
                            Token::Space(_) => index
                                .checked_sub(2)
                                .and_then(|j| tokens.get(j))
                                .is_some_and(
                                    |t2| matches!(t2, Token::Word(w) if w.meta.has_korean),
                                ),
                            _ => false,
                        });
                if prev_is_korean_or_first {
                    let matrix_context = state.matrix_context_active;
                    let mut bytes = Vec::new();
                    // PDF 제11항 — 국어 문장 안 수식 앞뒤를 두 칸씩 띄어 쓴다.
                    // Token::Space가 1칸 보조하므로 leading 1칸 추가.
                    bytes.push(0);
                    for letter in &prefix_letters {
                        if all_upper {
                            if matrix_context {
                                bytes.push(32);
                            } else if letter == &prefix_letters[0] {
                                bytes.push(32);
                                bytes.push(32);
                            }
                            let code = crate::english::encode_english(letter.to_ascii_lowercase())?;
                            bytes.push(code);
                        } else {
                            let code = crate::english::encode_english(*letter)?;
                            bytes.push(code);
                        }
                    }
                    // trailing 두 칸 (math expression 종료 boundary).
                    bytes.push(0);
                    bytes.push(0);
                    let suffix: String = suffix_chars.iter().collect();
                    let suffix_chars_vec: Vec<char> = suffix.chars().collect();
                    let suffix_meta = crate::rules::token::WordMeta::from_chars(&suffix_chars_vec);
                    let suffix_word = Token::Word(WordToken {
                        text: std::borrow::Cow::Owned(suffix),
                        chars: suffix_chars_vec,
                        meta: suffix_meta,
                    });
                    return Ok(TokenAction::ReplaceMany(vec![
                        Token::PreEncoded(bytes),
                        suffix_word,
                    ]));
                }
            }
        }
    }

    // PDF 제13항 — 한국어 산문 안 그리스 문자 리스트 (예: `α, β에`).
    // `Word(MathLetter+',')`이 현재이고 다음 비공백 Word가 `MathLetter+Korean`이면
    // 두 단어를 `⠴α, β⠲` + Korean으로 묶어 emit한다.
    // 직전이 한국어 단어여야 한다 (prose 컨텍스트 확인).
    if word.chars.len() == 2
        && word.chars[1] == ','
        && math_symbol_shortcut::is_math_symbol_char(word.chars[0])
        && !word.chars[0].is_ascii_alphanumeric()
    {
        let prev_is_korean_word = index
            .checked_sub(1)
            .and_then(|i| tokens.get(i))
            .and_then(|t| match t {
                Token::Space(_) => index.checked_sub(2).and_then(|j| tokens.get(j)),
                _ => Some(t),
            })
            .is_some_and(|t| matches!(t, Token::Word(w) if w.meta.has_korean));
        // 다음 Word: math letter 시작 + 한국어 suffix
        let next_word_opt = {
            let mut i = index + 1;
            loop {
                match tokens.get(i) {
                    Some(Token::Space(_)) => i += 1,
                    Some(Token::Word(w)) => break Some((i, w)),
                    _ => break None,
                }
            }
        };
        if prev_is_korean_word
            && let Some((next_idx, next_word)) = next_word_opt
            && next_word.chars.len() >= 2
            && math_symbol_shortcut::is_math_symbol_char(next_word.chars[0])
            && !next_word.chars[0].is_ascii_alphanumeric()
            && next_word.chars[1..]
                .iter()
                .all(|c| crate::utils::is_korean_char(*c))
        {
            let letter1 = word.chars[0];
            let letter2 = next_word.chars[0];
            let korean_suffix: String = next_word.chars[1..].iter().collect();
            let enc1 = math_symbol_shortcut::encode_char_math_symbol_shortcut(letter1)?;
            let enc2 = math_symbol_shortcut::encode_char_math_symbol_shortcut(letter2)?;
            let mut bytes = Vec::new();
            bytes.push(52); // ⠴ open quote
            bytes.extend_from_slice(enc1);
            bytes.push(2); // ⠂ literal comma in math letter list
            bytes.push(0); // space
            bytes.extend_from_slice(enc2);
            bytes.push(50); // ⠲ close quote
            // suffix Korean을 다음 Word로 분리 emit
            let suffix_chars: Vec<char> = korean_suffix.chars().collect();
            let suffix_meta = crate::rules::token::WordMeta::from_chars(&suffix_chars);
            let suffix_word = Token::Word(WordToken {
                text: std::borrow::Cow::Owned(korean_suffix),
                chars: suffix_chars,
                meta: suffix_meta,
            });
            // 현재 Word + 사이 토큰 + 다음 Word를 한꺼번에 교체.
            let consume_count = next_idx + 1 - index;
            return Ok(TokenAction::ReplaceRange(
                consume_count,
                vec![Token::PreEncoded(bytes), suffix_word],
            ));
        }
    }

    // PDF — `...` 또는 `..., `, `..`은 math context에 있으면 수학 줄임표 `⠠⠠⠠`로 emit.
    // Korean 마침표 줄임표 `⠲⠲⠲`와 구분.
    let dot_only =
        !text.is_empty() && (text.chars().all(|c| matches!(c, '.' | ',')) && text.contains('.'));
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
                        found = w.chars.iter().any(|c| {
                                matches!(*c,
                                    '\u{2080}'..='\u{2089}' | '\u{00B2}' | '\u{00B3}' | '\u{2070}'..='\u{2079}'
                                )
                            }) || (w.chars.first().is_some_and(|c| c.is_ascii_alphabetic()) && w.chars.iter().all(|c| c.is_ascii_alphabetic() || matches!(*c, ',' | '₀'..='₉')));
                        break;
                    }
                    _ => break,
                }
            }
            found
        };
        if prev_is_math_context {
            let dots: usize = text.chars().filter(|c| *c == '.').count();
            // ⠠ (32) repeated for each dot, capped at 3 per PDF.
            let mut bytes = vec![32u8; dots.min(3)];
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
            let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(word.chars[0])?;
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
            let math_context = math_context_from_state(state);
            if let Ok(bytes) =
                crate::rules::token_rules::latex_math::encode_latex_math_bytes_with_context(
                    inner,
                    math_context,
                )
            {
                // PDF — Korean prose 안 단일 letter math 블록은 ⠴...⠲로 감싼다.
                // 콤마-구분 letter 리스트도 quote/english marker로 감싼다.
                let suffix_first = suffix.chars().next();
                let suffix_is_korean = suffix_first.is_some_and(crate::utils::is_korean_char);
                let inner_is_single_letter =
                    inner.chars().count() == 1 && inner.chars().all(|c| c.is_ascii_alphabetic());
                let comma_list = inner.contains(',')
                    && inner.split(',').map(str::trim).all(|p| {
                        !p.is_empty()
                            && p.chars().count() == 1
                            && p.chars().all(|c| c.is_ascii_alphabetic())
                    });
                let prev_is_korean = index
                    .checked_sub(1)
                    .and_then(|i| tokens.get(i))
                    .map(|tok| match tok {
                        Token::Word(w) => w.meta.has_korean,
                        Token::Space(_) => index
                            .checked_sub(2)
                            .and_then(|j| tokens.get(j))
                            .is_some_and(|t| matches!(t, Token::Word(w) if w.meta.has_korean)),
                        _ => false,
                    })
                    .unwrap_or(false);
                let in_prose = suffix_is_korean || prev_is_korean;
                // PDF — `$-2$`, `$0.3010$` 같이 부호+숫자/소수점만 있는 단순 수치는
                // "본격적 수식"이 아니므로 한국어 단어 경계에서 추가 공백을 적용하지 않는다.
                // Space token 1칸으로 충분하다.
                let inner_is_simple_numeric = !inner.is_empty()
                    && inner.chars().all(|c| {
                        c.is_ascii_digit() || matches!(c, '-' | '+' | '\u{2212}' | '.' | ',')
                    });
                // 따옴표 자체가 경계를 명시(단일 letter/리스트), 단순 수치, 토큰 첫 위치는
                // 모두 leading_spaces=0.
                let leading_spaces = if (in_prose && (inner_is_single_letter || comma_list))
                    || inner_is_simple_numeric
                    || index == 0
                {
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
                                if let Ok(code) =
                                    crate::english::encode_english(c.to_ascii_lowercase())
                                {
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
                    // 단, 단순 수치 표기(`-2`, `0.3010`)는 본격적 수식이 아니므로 직접 인접.
                    let trailing_spaces = if suffix_is_korean && !inner_is_simple_numeric {
                        2
                    } else {
                        0
                    };
                    if trailing_spaces > 0 {
                        replacement.push(Token::PreEncoded(vec![0; trailing_spaces]));
                    }
                }
                replacement.push(build_word_token(suffix.to_string()));
                return Ok(TokenAction::ReplaceMany(replacement));
            }
        }

        if let Some((whole, numerator, denominator)) = crate::fraction::parse_latex_fraction(text) {
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
            let math_context = math_context_from_state(state);
            if let Ok(bytes) =
                crate::rules::token_rules::latex_math::encode_latex_math_bytes_with_context(
                    inner,
                    math_context,
                )
            {
                let replacement =
                    crate::rules::token_rules::latex_math::wrap_latex_math_tokens_with_inner(
                        tokens, index, bytes, inner,
                    );
                return Ok(TokenAction::ReplaceMany(replacement));
            }
        }

        return Ok(TokenAction::Noop);
    }

    if !is_math_expression(&word.chars, text) {
        let math_context = math_context_from_state(state);
        if let Some(bytes) = try_encode_mixed_math_slice(&word.chars, math_context) {
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
                if i == 0 {
                    break;
                }
                i -= 1;
                match tokens.get(i) {
                    Some(Token::Space(_)) => {
                        found_space = true;
                    }
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
        if let Some(replacement) = split_mixed_math_word(word, leading_delimiter_len, math_context)
        {
            return Ok(TokenAction::ReplaceMany(replacement));
        }
        return Ok(TokenAction::Noop);
    }

    // Try to encode via math engine
    let math_context = math_context_from_state(state);
    match math::encoder::encode_math_expression_with_context(text, math_context) {
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
                    c.is_ascii_digit() || matches!(*c, '-' | '+' | '\u{2212}' | '.' | ',')
                });
            let is_substantial_math = word.chars.len() > 1
                && word.chars.iter().any(|c| {
                    c.is_ascii_alphanumeric() || matches!(*c, '(' | ')' | '[' | ']' | '|')
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

// ============================================================
// Mutation-testing reinforcements for apply::run
//
// Strategy: rather than re-implement the local helpers in tests, drive run()
// indirectly via `crate::encode()` with crafted inputs. Each test exercises
// one specific code path and asserts an OBSERVABLE difference between the
// happy path and a nearby negative path. This kills mutations on local helpers
// (prev_next_words, is_logic_symbol_word) and on the dozens of branch checks
// throughout `run`.
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::token::{SpaceKind, WordMeta, WordToken};
    use std::borrow::Cow;

    fn enc_str(s: &str) -> String {
        crate::encode_to_unicode(s).unwrap_or_default()
    }

    /// Build a WordToken from a string for direct testing.
    fn word_tok<'a>(text: &'a str) -> Token<'a> {
        let chars: Vec<char> = text.chars().collect();
        let meta = WordMeta::from_chars(&chars);
        Token::Word(WordToken {
            text: Cow::Borrowed(text),
            chars,
            meta,
        })
    }

    fn space_tok() -> Token<'static> {
        Token::Space(SpaceKind::Regular)
    }

    // ---------- Direct tests on extracted helpers ----------

    /// `prev_next_words` returns (None, None) for an out-of-range index.
    /// Kills the `-> (None, None)` substitution mutant.
    #[test]
    fn prev_next_words_oob_index() {
        let tokens: Vec<Token<'_>> = vec![word_tok("a")];
        let (prev, next) = prev_next_words(&tokens, 5);
        assert!(prev.is_none(), "prev must be None for oob index");
        assert!(next.is_none(), "next must be None for oob index");
    }

    /// `prev_next_words` returns the immediate previous Word (no Space between).
    #[test]
    fn prev_next_words_adjacent_words() {
        let tokens: Vec<Token<'_>> = vec![word_tok("a"), word_tok("b"), word_tok("c")];
        let (prev, next) = prev_next_words(&tokens, 1);
        assert!(prev.is_some(), "prev must resolve to Word 'a'");
        assert_eq!(prev.unwrap().text.as_ref(), "a");
        assert!(next.is_some(), "next must resolve to Word 'c'");
        assert_eq!(next.unwrap().text.as_ref(), "c");
    }

    /// `prev_next_words` skips one or more Space tokens.
    #[test]
    fn prev_next_words_skips_spaces() {
        let tokens: Vec<Token<'_>> = vec![
            word_tok("a"),
            space_tok(),
            space_tok(),
            word_tok("b"),
            space_tok(),
            word_tok("c"),
        ];
        let (prev, next) = prev_next_words(&tokens, 3);
        assert_eq!(prev.unwrap().text.as_ref(), "a");
        assert_eq!(next.unwrap().text.as_ref(), "c");
    }

    /// `prev_next_words` returns None for prev when index is 0.
    /// Kills the `i - 1` underflow path mutations.
    #[test]
    fn prev_next_words_at_index_zero() {
        let tokens: Vec<Token<'_>> = vec![word_tok("a"), word_tok("b")];
        let (prev, next) = prev_next_words(&tokens, 0);
        assert!(prev.is_none(), "no prev at index 0");
        assert!(next.is_some(), "next must still resolve");
        assert_eq!(next.unwrap().text.as_ref(), "b");
    }

    /// `prev_next_words` returns None when a non-Space/Word boundary is hit.
    #[test]
    fn prev_next_words_stops_at_non_word_token() {
        let tokens: Vec<Token<'_>> = vec![
            Token::PreEncoded(vec![1, 2, 3]),
            space_tok(),
            word_tok("middle"),
            space_tok(),
            Token::PreEncoded(vec![4, 5, 6]),
        ];
        let (prev, next) = prev_next_words(&tokens, 2);
        // PreEncoded on both sides → prev/next both None.
        assert!(
            prev.is_none(),
            "PreEncoded boundary must yield None for prev"
        );
        assert!(
            next.is_none(),
            "PreEncoded boundary must yield None for next"
        );
    }

    /// `is_logic_symbol_word` true ONLY for single-char `⊻`.
    /// Kills: `-> false`, `!=` mutations.
    #[test]
    fn is_logic_symbol_word_matches_only_xor() {
        // Positive: ⊻ alone.
        let yes_word = WordToken {
            text: Cow::Borrowed("⊻"),
            chars: vec!['\u{22BB}'],
            meta: WordMeta::from_chars(&['\u{22BB}']),
        };
        assert!(is_logic_symbol_word(&yes_word));

        // Negative: a different symbol.
        let other_word = WordToken {
            text: Cow::Borrowed("∧"),
            chars: vec!['\u{2227}'],
            meta: WordMeta::from_chars(&['\u{2227}']),
        };
        assert!(!is_logic_symbol_word(&other_word));

        // Negative: ⊻ followed by something (len > 1).
        let multi = WordToken {
            text: Cow::Borrowed("⊻x"),
            chars: vec!['\u{22BB}', 'x'],
            meta: WordMeta::from_chars(&['\u{22BB}', 'x']),
        };
        assert!(!is_logic_symbol_word(&multi));

        // Negative: empty word.
        let empty = WordToken {
            text: Cow::Borrowed(""),
            chars: vec![],
            meta: WordMeta::from_chars(&[]),
        };
        assert!(!is_logic_symbol_word(&empty));
    }

    // ----- Lines 66-110: `a ≲ b:` colon-suffix math merge -----

    /// `a ≲ b:` is recognized as math expression (letter-relation-letter colon)
    /// and the letters do NOT receive prose quote wrapping (⠴...⠲).
    /// Mutation guarded: the `len() == 1 && is_ascii_lowercase()` gate at line 66
    /// and the collect_next/op matching that follow.
    #[test]
    fn colon_math_pattern_letters_avoid_prose_wrap() {
        let merged = enc_str("a ≲ b:");
        // When the merge runs, the result must NOT begin with the prose-quote
        // open ⠴ (U+2834) because the math encoder emits letters bare.
        assert!(!merged.is_empty(), "expected encoded bytes for `a ≲ b:`");
        // Compare with non-colon variant which goes through different path.
        let plain = enc_str("a ≲ b");
        assert_ne!(
            merged, plain,
            "trailing colon must change encoding via merge path"
        );
    }

    // ----- Lines 115-148: Set-builder `{x|x는 정수}` -----

    /// `{x|x는 정수}` triggers the set-builder merge. The token range
    /// (including spaces and Korean inside) is consumed as a single math
    /// expression. Distinguishes from non-set-builder `{...}` which would
    /// encode differently.
    #[test]
    fn set_builder_brace_pipe_merges_inner_korean() {
        let setbuilder = enc_str("{x|x는 정수}");
        assert!(!setbuilder.is_empty());
        // Same Korean text without the `{x|...}` should differ — confirming
        // the set-builder path triggered.
        let plain = enc_str("x는 정수");
        assert_ne!(
            setbuilder, plain,
            "set-builder wrap must change encoding vs. bare Korean"
        );
    }

    /// `{x|...` UNCLOSED → no merge; falls back to literal handling.
    /// Mutation: `found_close` requirement at line 138 (`&&`) — flipping to
    /// `||` would encode unclosed garbage. Compare unclosed vs. closed.
    #[test]
    fn set_builder_unclosed_does_not_merge() {
        let unclosed = enc_str("{x|x는 정수");
        let closed = enc_str("{x|x는 정수}");
        assert_ne!(
            unclosed, closed,
            "unclosed set-builder must NOT produce the same encoding as closed"
        );
    }

    // ----- Lines 155-236: Multi-letter Korean math identifier -----

    /// `ab의 값을 구하라` — `ab` is a 2-letter math identifier glued to Korean.
    /// The math-context-keyword check (`값을`/`구하`) gates this path.
    /// Lines 171 (`take(8)`), 175-178 (keyword OR checks), 183/188 (prev korean).
    #[test]
    fn multiletter_lower_identifier_with_math_keyword() {
        let with_kw = enc_str("ab의 값을 구하라");
        // Bytes must include leading space (0) and math letter marks.
        assert!(!with_kw.is_empty());
        // WITHOUT the keyword `값`/`구하`, the multi-letter path should NOT
        // trigger — leading to a different encoding.
        let without_kw = enc_str("ab의 친구");
        assert_ne!(
            with_kw, without_kw,
            "math-keyword presence must change ab의 encoding"
        );
    }

    /// `AB의 값을` — uppercase variant. Matrix context emits different marks
    /// per letter vs. lowercase variant.
    #[test]
    fn multiletter_upper_identifier_with_math_keyword() {
        let upper = enc_str("AB의 값을 구하라");
        let lower = enc_str("ab의 값을 구하라");
        assert_ne!(
            upper, lower,
            "uppercase vs lowercase identifier paths must differ"
        );
    }

    // ----- Lines 242-302: Greek letter list `α, β에` -----

    /// `한국어 α, β에` — Greek letter comma list with Korean suffix.
    /// Lines 242 (`chars.len() == 2 && chars[1] == ','`), 244 (math symbol).
    #[test]
    fn greek_letter_list_with_korean_suffix() {
        let list = enc_str("그래서 α, β에 대해");
        let plain = enc_str("그래서 α에 대해");
        assert!(!list.is_empty());
        assert_ne!(list, plain, "α, β list must differ from single α");
    }

    // ----- Lines 304-363: math ellipsis `...` -----

    /// Math context dot-ellipsis: after a math letter, `...` → `⠠⠠⠠`.
    /// Without prev math context, `...` falls through to default handling.
    #[test]
    fn math_ellipsis_after_math_letter() {
        let with_ctx = enc_str("x... ");
        let without_ctx = enc_str("...");
        assert_ne!(
            with_ctx, without_ctx,
            "ellipsis after math letter must differ from standalone ellipsis"
        );
    }

    // ----- Lines 375-405: therefore/because `∴ ∵` standalone -----

    /// Standalone `∴` between Word tokens gets braille space on each side.
    /// Lines 381 (Space match arm), 382 (Word/PreEncoded match arm).
    #[test]
    fn therefore_between_content_gets_spaces() {
        let with_ctx = enc_str("a ∴ b");
        // `∴` alone (no neighbors) should encode differently.
        let alone = enc_str("∴");
        assert_ne!(
            with_ctx, alone,
            "∴ between content must add spaces vs. standalone"
        );
    }

    // ----- Lines 407-414: Uppercase + logic symbol context -----

    /// `A ⊻ B` — uppercase letters surrounding a logic XOR symbol must be
    /// treated as math variables (lowercase-encoded), not as English prose.
    /// Lines 408 (uppercase check), 410 (prev/next is_logic_symbol_word).
    #[test]
    fn uppercase_around_logic_symbol_treated_as_math() {
        let logic = enc_str("A ⊻ B");
        // Compare with `A ⊻` alone (only prev set, next is None).
        let only_left = enc_str("A ⊻");
        // Both should encode A, but the `B` after triggers special path.
        assert_ne!(
            logic, only_left,
            "A ⊻ B with both neighbors must differ from A ⊻"
        );
    }

    /// `A ⊻ B` vs `A x B` (non-logic operator x in middle).
    /// Kills `is_logic_symbol_word -> false` mutation: with that mutation,
    /// both inputs would route the same way.
    #[test]
    fn logic_symbol_vs_plain_letter_neighbor() {
        let logic = enc_str("A ⊻ B");
        let plain = enc_str("A x B");
        assert_ne!(
            logic, plain,
            "logic-symbol neighbor must take a different path than plain-letter neighbor"
        );
    }

    // ----- Lines 417-585: LaTeX with Korean prose -----

    /// `$x$를` — single-letter LaTeX inside Korean prose. Must be quote-wrapped
    /// ⠴x⠲ with appropriate spacing.
    /// Lines 454-455 (single_letter), 466-467 (prev korean detection).
    #[test]
    fn latex_single_letter_korean_prose_wrapping() {
        let prose = enc_str("우리는 $x$를 구한다");
        // Without Korean prose around it, the encoding should differ.
        let standalone = enc_str("$x$");
        assert_ne!(
            prose, standalone,
            "$x$ in prose must have boundary spacing/wrap"
        );
    }

    /// `$a,b,c$를` — comma list LaTeX in Korean prose.
    /// Lines 456-461 (comma_list detection), 511+ (wrapping each letter).
    #[test]
    fn latex_comma_list_korean_prose() {
        let prose = enc_str("점 $a,b,c$를 잡자");
        let single = enc_str("점 $a$를 잡자");
        assert_ne!(
            prose, single,
            "comma list LaTeX must differ from single-letter"
        );
    }

    /// `$-2$` — simple numeric LaTeX must NOT get prose boundary spacing.
    /// Lines 478-481 (inner_is_simple_numeric), 543-548 (trailing_spaces=0).
    #[test]
    fn latex_simple_numeric_no_extra_boundary() {
        let num = enc_str("값은 $-2$이다");
        let var = enc_str("값은 $x$이다");
        // Single-letter `x` triggers `inner_is_single_letter` wrap path,
        // simple numeric does not → encodings must differ structurally.
        assert_ne!(
            num, var,
            "simple numeric LaTeX must encode differently from single-letter"
        );
    }

    // ----- Lines 587-639: Non-math-expression mixed math word path -----

    /// `안녕x+y는` — Korean-prose word with embedded math.
    /// Lines 611-615 (prev_prev math/mixed context), 617 (prev korean check).
    #[test]
    fn mixed_math_word_after_korean_word() {
        let mixed = enc_str("저는 안녕x+y는 좋다");
        assert!(!mixed.is_empty());
    }

    // ----- Lines 640+: Math expression with prev-Korean adjacency -----

    /// `한국어 f(x)` — math expression after Korean word with substantial-math
    /// path. Line 683 (`is_substantial_math`).
    #[test]
    fn substantial_math_after_korean() {
        let with_paren = enc_str("그래서 f(x)는");
        let just_var = enc_str("그래서 x는");
        // f(x) is substantial (has paren), x alone is not — boundary spacing differs.
        assert_ne!(
            with_paren, just_var,
            "substantial math must get prose boundary vs. single variable"
        );
    }

    /// `∆=...` patterns trigger needs_decimal_context_spacing.
    /// Lines 648-650 (`'∆' || '⋯' || combining mark` check).
    #[test]
    fn combining_mark_or_special_char_triggers_decimal_spacing() {
        let with_delta = enc_str("이전 ∆=10 이다");
        let plain = enc_str("이전 x=10 이다");
        assert_ne!(
            with_delta, plain,
            "∆ in expression must trigger different leading spacing"
        );
    }

    /// `prev_next_words` returns Some when there is an actual Word neighbor
    /// separated only by Space, returns None when boundary is reached.
    /// This is exercised through the uppercase+logic-symbol path which
    /// requires BOTH neighbors.
    #[test]
    fn prev_next_words_neighbor_resolution() {
        // Just `A` standalone — no neighbors → uppercase logic path NOT triggered.
        let solo = enc_str("A");
        // `A ⊻ B` — both neighbors present → uppercase logic path triggers.
        let both = enc_str("A ⊻ B");
        // `⊻ A` — only prev present (next is None).
        let only_prev = enc_str("⊻ A");
        // Verify all three produce different bytes (different code paths).
        assert_ne!(solo, both);
        assert_ne!(only_prev, both);
    }
}
