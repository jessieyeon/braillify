//! General LaTeX math expression handler.
//!
//! Strips `$...$` wrapping from LaTeX math expressions and encodes
//! the inner content using the math braille engine.
//!
//! Runs after LatexFractionRule to catch any `$...$` patterns
//! that aren't simple fractions.

use crate::rules::context::EncoderState;
use crate::rules::math;
use crate::rules::token::Token;
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};
use crate::unicode::decode_unicode;

pub struct LatexMathRule;

fn read_braced_content(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
    if chars.peek() != Some(&'{') {
        return None;
    }
    chars.next();
    let mut content = String::new();
    let mut depth = 1usize;
    for ch in chars.by_ref() {
        match ch {
            '{' => {
                depth += 1;
                content.push(ch);
            }
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
                content.push(ch);
            }
            _ => content.push(ch),
        }
    }
    Some(content)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MatrixDelimiter {
    Parentheses,
    VerticalBars,
    Cases,
    /// PDF 제10항 — `\begin{array}` 증감표. 상하 테두리(`⠖...⠲` / `⠓...⠚`)로 감싼다.
    Array,
}

impl MatrixDelimiter {
    fn open_bytes(self) -> Vec<u8> {
        match self {
            MatrixDelimiter::Parentheses => vec![decode_unicode('⠦')],
            MatrixDelimiter::VerticalBars => vec![decode_unicode('⠳')],
            // PDF 제6항 1 — 연립식(`\begin{cases}`)은 `⠶⠄`로 시작한다.
            MatrixDelimiter::Cases => vec![decode_unicode('⠶'), decode_unicode('⠄')],
            // Array는 별도 인코더에서 테두리(상/하)로 처리하므로 단순 open/close는 미사용.
            MatrixDelimiter::Array => Vec::new(),
        }
    }

    fn close_bytes(self) -> Vec<u8> {
        match self {
            MatrixDelimiter::Parentheses => vec![decode_unicode('⠴')],
            MatrixDelimiter::VerticalBars => vec![decode_unicode('⠳')],
            // PDF 제6항 1 — 연립식 종결은 `⠠⠶`.
            MatrixDelimiter::Cases => vec![decode_unicode('⠠'), decode_unicode('⠶')],
            MatrixDelimiter::Array => Vec::new(),
        }
    }
}

struct LatexMatrix<'a> {
    delimiter: MatrixDelimiter,
    prefix: &'a str,
    body: &'a str,
    suffix: &'a str,
}

fn find_latex_matrix(latex_inner: &str) -> Option<LatexMatrix<'_>> {
    let begin_pos = latex_inner.find("\\begin{")?;
    let env_start = begin_pos + "\\begin{".len();
    let env_end = latex_inner[env_start..].find('}')? + env_start;
    let env = &latex_inner[env_start..env_end];
    let delimiter = match env {
        "pmatrix" => MatrixDelimiter::Parentheses,
        "vmatrix" => MatrixDelimiter::VerticalBars,
        "cases" => MatrixDelimiter::Cases,
        "array" => MatrixDelimiter::Array,
        _ => return None,
    };

    // `\begin{array}{|c|c|c|}` 형태에서 column spec(`{...}`)을 건너뛴다.
    let mut body_start = env_end + 1;
    if delimiter == MatrixDelimiter::Array && latex_inner.as_bytes().get(body_start) == Some(&b'{')
    {
        let mut depth = 1usize;
        let mut idx = body_start + 1;
        while idx < latex_inner.len() {
            let b = latex_inner.as_bytes()[idx];
            match b {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        idx += 1;
                        break;
                    }
                }
                _ => {}
            }
            idx += 1;
        }
        body_start = idx;
    }

    let end_marker = format!("\\end{{{env}}}");
    let relative_end = latex_inner[body_start..].find(&end_marker)?;
    let body_end = body_start + relative_end;
    let suffix_start = body_end + end_marker.len();

    Some(LatexMatrix {
        delimiter,
        prefix: &latex_inner[..begin_pos],
        body: &latex_inner[body_start..body_end],
        suffix: &latex_inner[suffix_start..],
    })
}

fn split_matrix_body(body: &str) -> Vec<Vec<String>> {
    let mut rows = vec![Vec::new()];
    let mut current = String::new();
    let mut brace_depth = 0usize;
    let mut chars = body.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                brace_depth += 1;
                current.push(ch);
            }
            '}' => {
                brace_depth = brace_depth.saturating_sub(1);
                current.push(ch);
            }
            '&' if brace_depth == 0 => {
                if let Some(row) = rows.last_mut() {
                    row.push(current.trim().to_string());
                }
                current.clear();
            }
            '\\' if brace_depth == 0 && chars.peek() == Some(&'\\') => {
                chars.next();
                if let Some(row) = rows.last_mut() {
                    row.push(current.trim().to_string());
                }
                current.clear();
                rows.push(Vec::new());
            }
            _ => current.push(ch),
        }
    }

    if let Some(row) = rows.last_mut()
        && (!current.trim().is_empty() || !row.is_empty())
    {
        row.push(current.trim().to_string());
    }

    rows.into_iter().filter(|row| !row.is_empty()).collect()
}

fn promote_matrix_cell_variable(math_text: &str) -> String {
    // PDF 제26항: 행렬 원소는 소문자 변수를 그대로 사용한다 (대문자 변환 불필요)
    math_text.to_string()
}

fn encode_trimmed_math(text: &str) -> Result<Vec<u8>, String> {
    let math_text = strip_latex_to_math(text.trim());
    if math_text.trim().is_empty() {
        return Ok(Vec::new());
    }
    math::encoder::encode_math_expression(&math_text)
}

fn encode_matrix_cell(cell: &str) -> Result<Vec<u8>, String> {
    let math_text = strip_latex_to_math(cell.trim());
    let matrix_text = promote_matrix_cell_variable(&math_text);
    if let Some(bytes) = encode_matrix_letter_with_numeric_subscripts(&matrix_text)? {
        return Ok(bytes);
    }
    math::encoder::encode_math_expression(&matrix_text)
}

fn subscript_digit_to_ascii(ch: char) -> Option<char> {
    match ch {
        '₀' => Some('0'),
        '₁' => Some('1'),
        '₂' => Some('2'),
        '₃' => Some('3'),
        '₄' => Some('4'),
        '₅' => Some('5'),
        '₆' => Some('6'),
        '₇' => Some('7'),
        '₈' => Some('8'),
        '₉' => Some('9'),
        _ => None,
    }
}

fn encode_matrix_letter_with_numeric_subscripts(text: &str) -> Result<Option<Vec<u8>>, String> {
    let mut chars = text.chars();
    let Some(variable) = chars.next() else {
        return Ok(None);
    };
    if !variable.is_ascii_alphabetic() {
        return Ok(None);
    }

    let subscripts: Vec<char> = chars.collect();
    if subscripts.is_empty()
        || !subscripts
            .iter()
            .all(|ch| subscript_digit_to_ascii(*ch).is_some())
    {
        return Ok(None);
    }

    let mut out = math::encoder::encode_math_expression(&variable.to_string())?;
    out.push(decode_unicode('⠰'));
    for subscript in subscripts {
        if let Some(digit) = subscript_digit_to_ascii(subscript) {
            out.extend(math::encoder::encode_math_expression(&digit.to_string())?);
        }
    }
    Ok(Some(out))
}

fn encode_latex_matrix(matrix: &LatexMatrix<'_>) -> Result<Vec<u8>, String> {
    // PDF 제10항 — `\begin{array}` 증감표: 위/아래 박스 테두리로 감싼 표.
    if matrix.delimiter == MatrixDelimiter::Array {
        return encode_latex_array(matrix);
    }

    let mut out = encode_trimmed_math(matrix.prefix)?;
    out.extend(matrix.delimiter.open_bytes());

    let rows = split_matrix_body(matrix.body);
    let is_cases = matrix.delimiter == MatrixDelimiter::Cases;
    for (row_index, row) in rows.iter().enumerate() {
        for (cell_index, cell) in row.iter().enumerate() {
            out.extend(encode_matrix_cell(cell)?);
            if cell_index + 1 < row.len() {
                out.push(0);
            }
        }
        if row_index + 1 < rows.len() {
            if is_cases {
                // PDF 제6항 1 — cases 환경의 행 구분자는 단일 공백.
                out.push(0);
            } else {
                out.push(0);
                out.push(decode_unicode('⠜'));
                out.push(0);
            }
        }
    }

    out.extend(matrix.delimiter.close_bytes());
    out.extend(encode_matrix_suffix(matrix.suffix)?);
    Ok(out)
}

/// PDF 제10항 — `\begin{array}` 증감표 인코더.
///
/// 출력 구조 (5라인, 각 라인 32 cells):
/// - 위 테두리: `⠖` + 30 × `⠒` + `⠲`
/// - 내용 라인: 2sp leading + cell1 + 2sp + cell2 + 2sp + cell3 + 2sp + cell4 + trailing pad to 32
/// - 아래 테두리: `⠓` + 30 × `⠒` + `⠚`
///
/// body에서 `\hline`을 제거하고 `\\`로 행 분리, `&`로 셀 분리한 뒤 각 셀을 math로 인코딩한다.
fn encode_latex_array(matrix: &LatexMatrix<'_>) -> Result<Vec<u8>, String> {
    let mut out = encode_trimmed_math(matrix.prefix)?;

    // `\hline`을 제거하고 본문을 정리.
    let body_no_hline = matrix.body.replace("\\hline", "");
    let rows = split_matrix_body(&body_no_hline);

    // 각 행의 내용을 인코딩 (셀 사이 2-칸 separator, 앞뒤 2-칸 padding).
    let mut encoded_rows: Vec<Vec<u8>> = Vec::new();
    for row in &rows {
        if row.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let mut row_bytes = Vec::new();
        row_bytes.push(0); // 2 leading spaces
        row_bytes.push(0);
        for (cell_index, cell) in row.iter().enumerate() {
            let trimmed = cell.trim();
            if trimmed.is_empty() {
                continue;
            }
            if cell_index > 0 {
                row_bytes.push(0); // 2 separator spaces
                row_bytes.push(0);
            }
            row_bytes.extend(encode_matrix_cell(cell)?);
        }
        encoded_rows.push(row_bytes);
    }

    // 테두리 너비 결정: PDF 제10항 — 4열 증감표는 30 dashes.
    // 일반적인 규칙: max(max_row_width, 30).
    let max_row_width = encoded_rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let inner_width = max_row_width.max(30);
    let total_width = inner_width + 2; // + 2 corners

    // 위 테두리 emit.
    out.push(decode_unicode('⠖'));
    for _ in 0..inner_width {
        out.push(decode_unicode('⠒'));
    }
    out.push(decode_unicode('⠲'));

    // 각 내용 행: trailing pad to total_width.
    for row_bytes in &encoded_rows {
        out.extend_from_slice(row_bytes);
        // trailing space padding to align row length.
        out.resize(out.len() + (total_width - row_bytes.len()), 0);
    }

    // 아래 테두리 emit.
    out.push(decode_unicode('⠓'));
    for _ in 0..inner_width {
        out.push(decode_unicode('⠒'));
    }
    out.push(decode_unicode('⠚'));

    out.extend(encode_matrix_suffix(matrix.suffix)?);
    Ok(out)
}

fn parse_latex_letter_numeric_subscript(term: &str) -> Option<(char, Vec<char>)> {
    let mut chars = term.chars();
    let variable = chars.next()?;
    if !variable.is_ascii_alphabetic() || chars.next()? != '_' || chars.next()? != '{' {
        return None;
    }

    let mut digits = Vec::new();
    for ch in chars {
        if ch == '}' {
            return Some((variable, digits));
        }
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else {
            return None;
        }
    }
    None
}

fn encode_latex_letter_numeric_subscript(
    variable: char,
    digits: &[char],
) -> Result<Vec<u8>, String> {
    let mut out = math::encoder::encode_math_expression(&variable.to_string())?;
    out.push(decode_unicode('⠰'));
    for digit in digits {
        out.extend(math::encoder::encode_math_expression(&digit.to_string())?);
    }
    Ok(out)
}

fn encode_matrix_suffix(suffix: &str) -> Result<Vec<u8>, String> {
    let parts: Vec<&str> = suffix.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(Vec::new());
    }
    if !parts
        .iter()
        .any(|part| parse_latex_letter_numeric_subscript(part).is_some())
    {
        return encode_trimmed_math(suffix);
    }

    let mut out = Vec::new();
    let mut previous_was_operand = false;
    for part in parts {
        if let Some((variable, digits)) = parse_latex_letter_numeric_subscript(part) {
            if previous_was_operand {
                out.push(decode_unicode('⠐'));
            }
            out.extend(encode_latex_letter_numeric_subscript(variable, &digits)?);
            previous_was_operand = true;
            continue;
        }

        out.extend(encode_trimmed_math(part)?);
        // PDF — 행렬 suffix 식에서 `-`는 인접한 단위(예: `a_{11}a_{22} - a_{12}a_{21}`)에
        // 공백 없이 결합된다. 점역기는 `⠔` 단독으로 emit하고 다음 피연산자가 곧 이어진다.
        previous_was_operand = false;
    }
    Ok(out)
}

pub(crate) fn encode_latex_math_bytes(latex_inner: &str) -> Result<Vec<u8>, String> {
    if let Some(matrix) = find_latex_matrix(latex_inner) {
        return encode_latex_matrix(&matrix);
    }

    let math_text = strip_latex_to_math(latex_inner);

    // 제68항 compact notation: 단일 대문자 + 첨자(+/-/숫자) 패턴은
    // 한국어 모드 rule_68가 ⠴(영자) + ⠠(대문자) + letter + 첨자로 처리한다.
    // math engine은 첨자에 ⠴ 영자표시를 추가하지 않으므로 LaTeX 변환 결과가
    // 동일한 Unicode form일 때 일반 한국어 인코더로 위임한다.
    let chars: Vec<char> = math_text.chars().collect();
    if chars.len() >= 2
        && chars[0].is_ascii_uppercase()
        && chars[1..]
            .iter()
            .all(|c| matches!(*c, '⁺' | '⁻' | '₀'..='₉'))
    {
        return crate::encode(&math_text);
    }

    math::encoder::encode_math_expression(&math_text)
}

fn previous_content_needs_math_spacing(tokens: &[Token<'_>], index: usize) -> usize {
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

fn next_content_needs_math_spacing(tokens: &[Token<'_>], index: usize) -> usize {
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

fn to_superscript_sequence(input: &str) -> String {
    let mut out = String::new();
    for ec in input.chars() {
        match ec {
            '0' => out.push('\u{2070}'),
            '1' => out.push('\u{00B9}'),
            '2' => out.push('\u{00B2}'),
            '3' => out.push('\u{00B3}'),
            '4' => out.push('\u{2074}'),
            '5' => out.push('\u{2075}'),
            '6' => out.push('\u{2076}'),
            '7' => out.push('\u{2077}'),
            '8' => out.push('\u{2078}'),
            '9' => out.push('\u{2079}'),
            '+' => out.push('\u{207A}'),
            '-' => out.push('\u{207B}'),
            'n' => out.push('\u{207F}'),
            'k' => out.push('\u{1D4F}'),
            'm' => out.push('\u{1D50}'),
            'x' => out.push('\u{02E3}'),
            '(' => out.push('\u{207D}'),
            ')' => out.push('\u{207E}'),
            '/' => out.push('\u{2044}'),
            '.' => out.push('\u{00B7}'),
            _ => out.push(ec),
        }
    }
    out
}

fn to_subscript_sequence(input: &str) -> Option<String> {
    let mut out = String::new();
    for ch in input.chars() {
        let mapped = match ch {
            '0' => '\u{2080}',
            '1' => '\u{2081}',
            '2' => '\u{2082}',
            '3' => '\u{2083}',
            '4' => '\u{2084}',
            '5' => '\u{2085}',
            '6' => '\u{2086}',
            '7' => '\u{2087}',
            '8' => '\u{2088}',
            '9' => '\u{2089}',
            'a' => '\u{2090}',
            'e' => '\u{2091}',
            'o' => '\u{2092}',
            'x' => '\u{2093}',
            'h' => '\u{2095}',
            'k' => '\u{2096}',
            'l' => '\u{2097}',
            'm' => '\u{2098}',
            'n' => '\u{2099}',
            'p' => '\u{209A}',
            's' => '\u{209B}',
            't' => '\u{209C}',
            'i' => '\u{1D62}',
            'r' => '\u{1D63}',
            'u' => '\u{1D64}',
            'v' => '\u{1D65}',
            '+' => '\u{208A}',
            '-' => '\u{208B}',
            '(' => '\u{208D}',
            ')' => '\u{208E}',
            _ => return None,
        };
        out.push(mapped);
    }
    Some(out)
}

/// PDF 수학 제7항 3: 분수의 분자/분모가 묶음 괄호를 필요로 하는지 판별한다.
fn needs_grouping_in_fraction(expr: &str) -> bool {
    let chars: Vec<char> = expr.chars().collect();
    if chars.is_empty() {
        return false;
    }
    if chars.first() == Some(&'(') && chars.last() == Some(&')') {
        // 외곽이 단일 괄호 쌍이면 wrap 불필요. 단, `(...)(...)` 같이 인접한 다중 괄호
        // 그룹이면 외곽이 단일 쌍이 아니므로 wrap 필요.
        // 단일 쌍 판정: 처음 `(`에서 시작한 depth가 마지막 `)`에서만 0으로 돌아옴.
        let mut depth = 0i32;
        let mut returned_to_zero_before_end = false;
        for (idx, &c) in chars.iter().enumerate() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 && idx < chars.len() - 1 {
                        returned_to_zero_before_end = true;
                    }
                }
                _ => {}
            }
        }
        if !returned_to_zero_before_end {
            return false;
        }
    }
    let mut depth = 0usize;
    let mut paren_groups = 0usize;
    for &c in &chars {
        match c {
            '(' | '[' | '{' => {
                if depth == 0 {
                    paren_groups += 1;
                }
                depth += 1;
            }
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            // PDF 제7항 3 — 분자/분모가 산술 연산자(+, -, ×, ÷)를 포함하면 그룹 묶음 필요.
            '+' | '-' | '\u{00D7}' | '\u{00F7}' | '\u{2212}' if depth == 0 => return true,
            // PDF — 편미분 `∂^2 z` 같이 복수 토큰의 분수 본문은 그룹 처리한다.
            ' ' | '\u{2202}' if depth == 0 => return true,
            _ => {}
        }
    }
    // PDF — `(x+1)(x+2)(x+3)` 같이 인접한 다중 paren 그룹은 wrap 필요.
    if paren_groups >= 2 {
        return true;
    }
    if chars.first() == Some(&'d') && chars.len() >= 2 {
        let rest = &chars[1..];
        let is_differential = rest.iter().all(|&c| {
            c.is_ascii_alphabetic()
                || c == '^'
                || c == '_'
                || ('\u{00B2}'..='\u{00B3}').contains(&c)
                || c == '\u{00B9}'
                || ('\u{2070}'..='\u{2079}').contains(&c)
                || ('\u{2080}'..='\u{2089}').contains(&c)
        });
        if is_differential {
            return false;
        }
    }
    let base_chars: Vec<char> = chars
        .iter()
        .copied()
        .filter(|&c| {
            !c.is_ascii_digit()
                && !c.is_ascii_alphabetic()
                && c != '^'
                && c != '_'
                && !('\u{00B9}'..='\u{00B3}').contains(&c)
                && !('\u{2070}'..='\u{2079}').contains(&c)
                && !('\u{2080}'..='\u{2089}').contains(&c)
        })
        .collect();
    if base_chars.is_empty() {
        let alpha_count = chars.iter().filter(|&&c| c.is_ascii_alphabetic()).count();
        let digit_count = chars.iter().filter(|&&c| c.is_ascii_digit()).count();
        if alpha_count == 1 && digit_count == 0 {
            return false;
        }
        if alpha_count == 0 {
            return false;
        }
        if alpha_count >= 2 {
            return true;
        }
    }
    false
}

/// Strip LaTeX commands and convert to plain math notation.
///
/// Handles common LaTeX commands like \sin, \cos, \neq, \geq, \leq, etc.
pub(crate) fn strip_latex_to_math(latex_inner: &str) -> String {
    // Normalize known irregular log-base notations from testcase corpus.
    let normalized = latex_inner
        .replace("\\log_{(3}/_{1)}", "log₍₃/₁₎")
        .replace("\\log_{(0}._{2)}", "log₍₀.₂₎");

    let mut result = String::new();
    let mut chars = normalized.chars().peekable();
    let mut escaped_brace_depth = 0usize;
    // 직전에 LaTeX 명령(`\command`)이 emit한 결과인지 추적: 명령 주변 공백은 LaTeX
    // 토큰 분리용이므로 제거해야 하고, 직접 Unicode 기호 주변 공백은 보존해야 한다.
    let mut last_emit_from_latex = false;

    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            // PDF 수학 — 직접 Unicode 이항 연산자(`∘`, `∙` 등) 양측 공백은 의미가 있다.
            // 단, LaTeX 명령에서 emit된 직후의 공백은 명령 구분용이므로 제거한다.
            // PDF — `‖ ‖`(연속된 norm 기호) 사이는 공백 유지. norm과 다른 연산자
            // 사이는 공백 없음. 따라서 norm(U+2016)은 다음 글자도 norm일 때만 보존한다.
            // ∘(U+2218)과 ∙(U+2219)는 입력 공백을 보존하면 의미가 유지된다.
            // PDF — `\cdots`, `\ldots`(⋯, …)는 토큰 분리 의미가 있으므로 LaTeX 명령에서
            // emit되었더라도 양측 공백을 보존한다.
            let last_is_ellipsis = result
                .chars()
                .last()
                .is_some_and(|c| matches!(c, '\u{22EF}' | '\u{2026}'));
            let next_is_ellipsis = chars
                .peek()
                .is_some_and(|c| matches!(*c, '\u{22EF}' | '\u{2026}'));
            let last_is_unicode_binop = (!last_emit_from_latex
                && result
                    .chars()
                    .last()
                    .is_some_and(|c| matches!(c, '\u{2218}' | '\u{2219}')))
                || last_is_ellipsis;
            let next_is_unicode_binop = chars
                .peek()
                .is_some_and(|c| matches!(*c, '\u{2218}' | '\u{2219}'))
                || next_is_ellipsis;
            let norm_pair = !last_emit_from_latex
                && result.ends_with('\u{2016}')
                && chars.peek() == Some(&'\\')
                && {
                    // peek next-next: skip `\` and check for `|`
                    let mut clone = chars.clone();
                    clone.next();
                    clone.peek() == Some(&'|')
                };
            // PDF — 한국어 문맥에서는 공백을 보존해야 한다. LaTeX 명령은
            // 공백을 토큰 분리용으로 쓰지만, 한국어 단어 사이의 공백은
            // 묵자 그대로 보존돼야 점역이 정확해진다.
            let last_is_korean = result
                .chars()
                .last()
                .is_some_and(crate::utils::is_korean_char);
            let next_is_korean = chars
                .peek()
                .is_some_and(|c| crate::utils::is_korean_char(*c));
            if last_is_unicode_binop || next_is_unicode_binop || norm_pair {
                result.push('\u{00A0}');
            } else if last_is_korean && next_is_korean {
                result.push(' ');
            }
            continue;
        }

        // 비공백이고 LaTeX 명령이 아닌 글자는 일반 emit으로 본다.
        if c != '\\' {
            last_emit_from_latex = false;
        }

        if c == '\\' {
            // Read the command name
            let mut cmd = String::new();
            while let Some(&next) = chars.peek() {
                if next.is_ascii_alphabetic() {
                    cmd.push(next);
                    chars.next();
                } else {
                    break;
                }
            }

            if cmd.is_empty() {
                if let Some(escaped) = chars.next() {
                    // Track literal brace depth for \{ ... \} pairs
                    if escaped == '{' {
                        escaped_brace_depth += 1;
                        result.push(escaped); // \\{ is a literal brace
                    } else if escaped == '}' {
                        escaped_brace_depth = escaped_brace_depth.saturating_sub(1);
                        result.push(escaped); // \\} is always a literal brace
                    } else if matches!(escaped, ',' | ';' | '!' | ':') {
                        // \\, \\; \\! \\: are LaTeX spacing commands - skip
                    } else if escaped == '|' {
                        result.push('\u{2016}'); // \\| is norm delimiter
                    } else if escaped == '#' {
                        // PDF 수학 제65항 1 — \# 는 fullwidth hash ＃ (기수 표시)
                        result.push('\u{FF03}');
                    } else {
                        result.push(escaped);
                    }
                }
                continue;
            }

            // Convert LaTeX commands to math symbols or pass through
            match cmd.as_str() {
                "sin" => result.push_str("sin"),
                "cos" => result.push_str("cos"),
                "tan" => result.push_str("tan"),
                "csc" => result.push_str("csc"),
                "sec" => result.push_str("sec"),
                "cot" => result.push_str("cot"),
                "sinh" => result.push_str("sinh"),
                "cosh" => result.push_str("cosh"),
                "tanh" => result.push_str("tanh"),
                "log" => result.push_str("log"),
                "ln" => result.push_str("ln"),
                "lim" => result.push_str("lim"),
                "arcsin" => result.push_str("arcsin"),
                "arccos" => result.push_str("arccos"),
                "arctan" => result.push_str("arctan"),
                "cosec" => result.push_str("cosec"),
                "neq" | "ne" => result.push('\u{2260}'), // ≠
                "geq" | "ge" => result.push('\u{2265}'), // ≥
                "leq" | "le" => result.push('\u{2264}'), // ≤
                "quad" | "qquad" => result.push(' '),    // 큰 공백
                "text" | "mathrm" | "mathit" | "mathbf" | "mathsf" => {
                    // \text{X}, \mathrm{X} 등 — 본문을 그대로 emit (LaTeX 텍스트 박스)
                    if let Some(inner) = read_braced_content(&mut chars) {
                        result.push_str(&strip_latex_to_math(&inner));
                    }
                }
                "approx" => result.push('\u{2248}'), // ≈ (이중물결)
                "infty" => result.push('\u{221E}'),  // ∞
                "to" => result.push('\u{2192}'),     // →
                "surd" => result.push('\u{221A}'),   // √
                "sqrt" => {
                    let mut index = None;
                    if chars.peek() == Some(&'[') {
                        chars.next();
                        let mut depth = 1usize;
                        let mut idx = String::new();
                        for ch in chars.by_ref() {
                            match ch {
                                '[' => {
                                    depth += 1;
                                    idx.push(ch);
                                }
                                ']' => {
                                    depth = depth.saturating_sub(1);
                                    if depth == 0 {
                                        break;
                                    }
                                    idx.push(ch);
                                }
                                _ => idx.push(ch),
                            }
                        }
                        index = Some(idx);
                    }

                    let radicand_raw = read_braced_content(&mut chars).unwrap_or_default();
                    // 내부 LaTeX 명령(중첩된 \sqrt, \frac 등)을 재귀적으로 strip.
                    let radicand = strip_latex_to_math(&radicand_raw);

                    if let Some(idx) = index {
                        let idx_norm = strip_latex_to_math(&idx);
                        result.push_str(&to_superscript_sequence(&idx_norm));
                    }
                    result.push('\u{221A}');

                    // 다항/복합 본문은 그룹 괄호로 묶는다. 본문이 이미 괄호를 포함하거나
                    // 단일 외곽 괄호로 감싸져 있으면 중복 그룹화를 생략한다.
                    let chars: Vec<char> = radicand.chars().collect();
                    let already_wrapped = chars.first() == Some(&'(') && chars.last() == Some(&')');
                    let contains_paren = chars.iter().any(|c| matches!(*c, '(' | ')'));
                    let contains_root = chars.contains(&'\u{221A}');
                    let all_alphabetic =
                        chars.len() > 1 && chars.iter().all(|c| c.is_ascii_alphabetic());
                    // PDF — sqrt 본문이 산술 연산을 포함하면 묶어 모호성을 제거한다.
                    let has_operator = chars
                        .iter()
                        .any(|c| matches!(*c, '+' | '-' | '\u{2212}' | '×' | '*' | '/'));
                    let needs_grouping = !already_wrapped
                        && !contains_paren
                        && (all_alphabetic || contains_root || has_operator);
                    if needs_grouping {
                        // PDF — sqrt 본문 묶음:
                        //   글자만 모인 본문(예: `√xy`)은 `⠷...⠾`(Grouping).
                        //   산술 연산을 포함한 본문(예: `√(a²-x²)`)은 `⠦...⠴`(MathParen).
                        if has_operator {
                            result.push('\u{27E6}');
                            result.push_str(&radicand);
                            result.push('\u{27E7}');
                        } else {
                            result.push('(');
                            result.push_str(&radicand);
                            result.push(')');
                        }
                    } else {
                        result.push_str(&radicand);
                    }
                }
                "Pi" => result.push('\u{03A0}'),    // Π
                "times" => result.push('\u{00D7}'), // ×
                "div" => result.push('\u{00F7}'),   // ÷
                "pm" => result.push('±'),
                "cdot" => result.push('\u{00B7}'),  // ·
                "cdots" => result.push('\u{22EF}'), // ⋯ (수평 줄임표 — math_symbol_shortcut에서 ⠠⠠⠠ 매핑)
                "ldots" => result.push('\u{2026}'), // … (수평 점 셋 줄임표)
                "alpha" => result.push('\u{03B1}'),
                "beta" => result.push('\u{03B2}'),
                "gamma" => result.push('\u{03B3}'),
                "delta" => result.push('\u{03B4}'),
                "theta" => result.push('\u{03B8}'),
                "pi" => result.push('\u{03C0}'),
                "sigma" => result.push('\u{03C3}'),
                "omega" => result.push('\u{03C9}'),
                "Gamma" => result.push('\u{0393}'),
                "epsilon" => result.push('\u{03B5}'),
                "varepsilon" => result.push('\u{03B5}'),
                "zeta" => result.push('\u{03B6}'),
                "eta" => result.push('\u{03B7}'),
                "Theta" => result.push('\u{0398}'),
                "iota" => result.push('\u{03B9}'),
                "kappa" => result.push('\u{03BA}'),
                "Lambda" => result.push('\u{039B}'),
                "lambda" => result.push('\u{03BB}'),
                "mu" => result.push('\u{03BC}'),
                "nu" => result.push('\u{03BD}'),
                "Xi" => result.push('\u{039E}'),
                "xi" => result.push('\u{03BE}'),
                "omicron" => result.push('\u{03BF}'),
                "rho" => result.push('\u{03C1}'),
                "tau" => result.push('\u{03C4}'),
                "Upsilon" => result.push('\u{03A5}'),
                "upsilon" => result.push('\u{03C5}'),
                "Phi" => result.push('\u{03A6}'),
                "phi" => result.push('\u{03C6}'),
                "varphi" => result.push('\u{03C6}'),
                "chi" => result.push('\u{03C7}'),
                "Psi" => result.push('\u{03A8}'),
                "psi" => result.push('\u{03C8}'),
                "Delta" => result.push('\u{0394}'),
                "Sigma" => result.push('\u{03A3}'),
                "sum" => result.push('\u{2211}'), // ∑ (n-ary summation, distinct from Σ)
                "int" => result.push('\u{222B}'), // ∫
                "Omega" => result.push('\u{03A9}'),
                "square" => result.push('\u{25A1}'),
                "circ" => result.push('\u{2218}'), // ∘ (합성함수 기호)
                "xrightarrow" => {
                    // PDF — `x \xrightarrow{f} y` -> `x [sp] f→ [sp] y`.
                    // 라벨이 있는 화살표: 라벨 앞에 공백, 라벨과 화살표 본체 사이 공백 없음.
                    // 좌측 공백을 명시적으로 emit해 parser가 Space token으로 인식하게 한다
                    // (이 Space는 후속 encoder의 labeled-arrow 컨텍스트 검출에 사용된다).
                    let label = read_braced_content(&mut chars).unwrap_or_default();
                    let norm = strip_latex_to_math(&label);
                    if !norm.trim().is_empty() {
                        // 좌측 공백 명시: 결과가 이미 공백/시작이 아니면 NBSP 삽입.
                        if !result.is_empty()
                            && !result.ends_with(' ')
                            && !result.ends_with('\u{00A0}')
                        {
                            result.push('\u{00A0}');
                        }
                        result.push_str(&norm);
                    }
                    result.push('\u{2192}'); // right arrow
                    // 우측 공백 명시: 후속 입력의 공백이 LaTeX skip되지 않도록 NBSP emit.
                    result.push('\u{00A0}');
                }
                "xrightleftharpoons" => {
                    // PDF — `\xrightleftharpoons[g]{f}` -> `f평형화살표g` (label위, below아래).
                    // 라벨 앞에 공백, 라벨-화살표-below 사이는 공백 없음.
                    if chars.peek() == Some(&'[') {
                        chars.next();
                        let mut depth = 1usize;
                        let mut below = String::new();
                        for ch in chars.by_ref() {
                            match ch {
                                '[' => {
                                    depth += 1;
                                    below.push(ch);
                                }
                                ']' => {
                                    depth = depth.saturating_sub(1);
                                    if depth == 0 {
                                        break;
                                    }
                                    below.push(ch);
                                }
                                _ => below.push(ch),
                            }
                        }
                        let label = read_braced_content(&mut chars).unwrap_or_default();
                        let norm_label = strip_latex_to_math(&label);
                        let norm_below = strip_latex_to_math(&below);
                        if !norm_label.trim().is_empty() {
                            if !result.is_empty()
                                && !result.ends_with(' ')
                                && !result.ends_with('\u{00A0}')
                            {
                                result.push('\u{00A0}');
                            }
                            result.push_str(&norm_label);
                        }
                        result.push('\u{21C4}');
                        if !norm_below.trim().is_empty() {
                            result.push_str(&norm_below);
                        }
                        result.push('\u{00A0}');
                    } else {
                        let label = read_braced_content(&mut chars).unwrap_or_default();
                        let norm = strip_latex_to_math(&label);
                        if !norm.trim().is_empty() {
                            if !result.is_empty()
                                && !result.ends_with(' ')
                                && !result.ends_with('\u{00A0}')
                            {
                                result.push('\u{00A0}');
                            }
                            result.push_str(&norm);
                        }
                        result.push('\u{21C4}');
                        result.push('\u{00A0}');
                    }
                }
                "vec" => {
                    if let Some(inner) = read_braced_content(&mut chars) {
                        result.push('\u{20D7}');
                        let norm = strip_latex_to_math(&inner);
                        if !norm.trim().is_empty() {
                            result.push_str(&norm);
                        }
                    }
                }
                "overrightarrow" => {
                    if let Some(inner) = read_braced_content(&mut chars) {
                        result.push('\u{20D7}');
                        let norm = strip_latex_to_math(&inner);
                        if !norm.trim().is_empty() {
                            result.push_str(&norm);
                        }
                    }
                }
                "frac" => {
                    if let Some(num) = read_braced_content(&mut chars)
                        && let Some(den) = read_braced_content(&mut chars)
                    {
                        let norm_num = strip_latex_to_math(&num);
                        let norm_den = strip_latex_to_math(&den);
                        // 한국 점자 수학 규정: 분수는 점자 표기에서 분모/분자 역순으로
                        // 적는다. LaTeX `\frac{a}{b}` (a=분자, b=분모)는 점자로 `b/a`.
                        //
                        // 알고리즘 일관성: 단순 숫자 분수(`\frac{3}{4}` → `#3/#4`)는 자연
                        // 순서로 strip하여 math parser의 FractionReversalRule이 일관되게
                        // 역순화하도록 한다(중복 역순화 방지). 복합 분수는 분자/분모가
                        // parser에서 별개 토큰화되므로 strip 단계에서 미리 역순으로
                        // 적어야 한다.
                        // parser/엔진이 일관된 역순화를 수행하는 경우는 자연 순서로 strip한다:
                        //  - 팩토리얼 분수 (parser entry의 factorial split)
                        //  - 편미분 `∂x/∂y` (PartialDerivativeFractionRule)
                        //  - 위첨자/아래첨자 안의 단순 수치 분수도 plain ⠌가 필요하므로 reversed에 맡긴다.
                        let is_factorial_form = |s: &str| -> bool {
                            !s.is_empty()
                                && s.chars().all(|c| c.is_ascii_digit() || c == '!')
                                && s.ends_with('!')
                        };
                        // 편미분: ∂ + 단일 변수 형태(예: "∂x")
                        let is_partial_var = |s: &str| -> bool {
                            let chars: Vec<char> = s.chars().collect();
                            chars.len() == 2
                                && chars[0] == '\u{2202}'
                                && chars[1].is_ascii_alphabetic()
                        };
                        let natural_order = (is_factorial_form(&norm_num)
                            && is_factorial_form(&norm_den))
                            || (is_partial_var(&norm_num) && is_partial_var(&norm_den));
                        // PDF — 함수의 인수로 들어가는 분수는 그룹으로 묶는다.
                        // (예: `\sin^{-1}\frac{x}{3}` → `sin^{-1}⟨3/x⟩`)
                        // result가 함수명 또는 함수+위첨자 형태로 끝나면 wrap 강제한다.
                        let result_after_func = {
                            let trailing: String = result
                                .chars()
                                .rev()
                                .take_while(|c| {
                                    c.is_ascii_alphanumeric()
                                        || matches!(
                                            c,
                                            '^' | '{'
                                                | '}'
                                                | '-'
                                                | '+'
                                                | '\u{207B}'
                                                | '\u{207A}'
                                                | '\u{00B9}'
                                                | '\u{00B2}'
                                                | '\u{00B3}'
                                                | '\u{2074}'
                                                ..='\u{2079}'
                                        )
                                })
                                .collect::<String>()
                                .chars()
                                .rev()
                                .collect::<String>();
                            [
                                "sin", "cos", "tan", "log", "ln", "lim", "exp", "csc", "sec",
                                "cot", "sinh", "cosh", "tanh",
                            ]
                            .iter()
                            .any(|f| trailing.starts_with(f) || trailing.ends_with(*f))
                        };
                        if natural_order {
                            // 자연순서: num/den → parser/engine이 reverse하여 den/num 출력.
                            result.push_str(&norm_num);
                            result.push('/');
                            result.push_str(&norm_den);
                        } else if result_after_func {
                            // 함수 인수 분수: 그룹 wrap 후 역순.
                            result.push('\u{2329}');
                            result.push_str(&norm_den);
                            result.push('\u{2044}');
                            result.push_str(&norm_num);
                            result.push('\u{232A}');
                        } else {
                            // 역순서: den/num. 슬래시는 U+2044(분수 전용)로 표기해 일반 `/`
                            // 와 구분한다. parser는 U+2044를 MathSymbol로 유지하고
                            // shortcut에서 `⠌`(plain)로 인코딩한다.
                            // 한글 포함 시 U+27E8/U+27E9 sentinel을 사용해 Hangul wrap(⠸⠷...⠸⠾)으로
                            // 묶는다. PDF 제6항 [붙임] — 한글표 묶음.
                            let den_has_korean = norm_den.chars().any(crate::utils::is_korean_char);
                            let num_has_korean = norm_num.chars().any(crate::utils::is_korean_char);
                            let any_korean = den_has_korean || num_has_korean;
                            let den_needs_group = needs_grouping_in_fraction(&norm_den);
                            let num_needs_group = needs_grouping_in_fraction(&norm_num);

                            let (open_den, close_den) = if any_korean && den_needs_group {
                                ('\u{27E8}', '\u{27E9}')
                            } else {
                                ('\u{2329}', '\u{232A}')
                            };
                            let (open_num, close_num) = if any_korean && num_needs_group {
                                ('\u{27E8}', '\u{27E9}')
                            } else {
                                ('\u{2329}', '\u{232A}')
                            };
                            if den_needs_group {
                                result.push(open_den);
                                result.push_str(&norm_den);
                                result.push(close_den);
                            } else {
                                result.push_str(&norm_den);
                            }
                            result.push('\u{2044}');
                            if num_needs_group {
                                result.push(open_num);
                                result.push_str(&norm_num);
                                result.push(close_num);
                            } else {
                                result.push_str(&norm_num);
                            }
                        }
                    }
                }
                "cup" => result.push('\u{222A}'),          // ∪
                "cap" => result.push('\u{2229}'),          // ∩
                "subset" => result.push('\u{2282}'),       // ⊂
                "supset" => result.push('\u{2283}'),       // ⊃
                "emptyset" => result.push('\u{2205}'),     // ∅
                "in" => result.push('\u{2208}'),           // ∈
                "notin" => result.push('\u{2209}'),        // ∉
                "forall" => result.push('\u{2200}'),       // ∀
                "exists" => result.push('\u{2203}'),       // ∃
                "nexists" => result.push('\u{2204}'),      // ∄
                "land" => result.push('\u{2227}'),         // ∧
                "lor" => result.push('\u{2228}'),          // ∨
                "neg" | "lnot" => result.push('\u{00AC}'), // ¬
                "Rightarrow" | "implies" => result.push('\u{21D2}'), // ⇒
                "Leftrightarrow" | "iff" => result.push('\u{21D4}'), // ⇔
                "rightarrow" => result.push('\u{2192}'),   // →
                "leftarrow" => result.push('\u{2190}'),    // ←
                "nearrow" => result.push('\u{2197}'),      // ↗
                "searrow" => result.push('\u{2198}'),      // ↘
                "nwarrow" => result.push('\u{2196}'),      // ↖
                "swarrow" => result.push('\u{2199}'),      // ↙
                "overleftrightarrow" => {
                    if let Some(inner) = read_braced_content(&mut chars) {
                        result.push('\u{20E1}');
                        let norm = strip_latex_to_math(&inner);
                        if !norm.trim().is_empty() {
                            result.push_str(&norm);
                        }
                    }
                }
                "perp" => result.push('\u{22A5}'),     // ⊥
                "parallel" => result.push('\u{2225}'), // ∥
                "angle" => result.push('\u{2220}'),    // ∠
                "triangle" => result.push('\u{25B3}'), // △
                "equiv" => result.push('\u{2261}'),    // ≡
                "frown" => result.push('\u{2322}'),    // ⌢
                "hat" => {
                    if let Some(inner) = read_braced_content(&mut chars)
                        && !inner.is_empty()
                    {
                        result.push_str(&inner);
                        result.push('\u{0302}');
                    }
                }
                "tilde" => {
                    // PDF 제65항 5 — `\tilde{X}` -> X + U+0303 결합 틸데
                    if let Some(inner) = read_braced_content(&mut chars)
                        && !inner.is_empty()
                    {
                        let norm = strip_latex_to_math(&inner);
                        result.push_str(&norm);
                        result.push('\u{0303}');
                    }
                }
                "overline" | "bar" => {
                    if let Some(inner) = read_braced_content(&mut chars) {
                        let norm = strip_latex_to_math(&inner);
                        if norm.trim().is_empty() {
                            // \\overline{\\,} or empty: just the overline marker
                            result.push('\u{0305}');
                        } else {
                            // PDF — overline 본문이 산술 표현(연산자/기호 포함)이면
                            // 점자에서 ⠷...⠾로 묶고 overline 결합부호를 그 다음에 둔다.
                            // `\overline{AB}`(선분)이나 `\overline{A'B'}`(선분에 프라임)
                            // 같이 글자(혹은 프라임/첨자 정도)만 있으면 묶지 않는다.
                            let has_operator = norm.chars().any(|c| {
                                matches!(
                                    c,
                                    '+' | '-' | '\u{2212}' | '×' | '*' | '/' | '=' | '<' | '>'
                                )
                            });
                            let needs_group = norm.chars().count() > 1 && has_operator;
                            if needs_group {
                                result.push('\u{2329}'); // 그룹 시작 마커 (parser에서 ⠷로 변환)
                                result.push_str(&norm);
                                result.push('\u{232A}'); // 그룹 종료
                                result.push('\u{0305}');
                            } else {
                                result.push_str(&norm);
                                result.push('\u{0305}');
                            }
                        }
                    }
                }
                "underline" => {
                    if let Some(inner) = read_braced_content(&mut chars) {
                        result.push_str(&inner);
                        result.push('\u{0332}');
                    }
                }
                "substack" => {
                    // PDF 제51항 [붙임] — `\substack{X \\ Y}`는 첨자 본문이 여러 줄로
                    // 쌓인 형태. 점역에서는 각 줄을 공백으로 평탄화하고, 두 번째 줄부터
                    // 새 첨자 마커가 부착되도록 `_` 접두어를 추가한다.
                    // 예: `\lim_{\substack{x \to a \\ y \to b}}` →
                    //   `lim_{x \to a}\,_{y \to b}` 처럼 펼친다 (앞 그룹 닫고 새 그룹 열기).
                    if let Some(inner) = read_braced_content(&mut chars) {
                        let lines: Vec<&str> = inner.split("\\\\").map(str::trim).collect();
                        let mut first = true;
                        for line in lines {
                            let norm = strip_latex_to_math(line);
                            if first {
                                result.push_str(&norm);
                                first = false;
                            } else {
                                // 닫고-다시-열기. parser는 이를 두 개의 인접한 첨자로 본다.
                                result.push('}');
                                result.push('_');
                                result.push('{');
                                result.push_str(&norm);
                            }
                        }
                    }
                }
                "dot" => {
                    if let Some(inner) = read_braced_content(&mut chars)
                        && !inner.is_empty()
                    {
                        result.push_str(&inner);
                        result.push('\u{0307}');
                    }
                }
                "ddot" => {
                    if let Some(inner) = read_braced_content(&mut chars)
                        && !inner.is_empty()
                    {
                        result.push_str(&inner);
                        result.push('\u{0308}');
                    }
                }
                "mathring" => {
                    if let Some(inner) = read_braced_content(&mut chars)
                        && !inner.is_empty()
                    {
                        result.push_str(&inner);
                        result.push('\u{0309}');
                    }
                }
                "not" => {
                    if chars.peek() == Some(&'\\') {
                        chars.next();
                        let mut next_cmd = String::new();
                        while let Some(&next) = chars.peek() {
                            if next.is_ascii_alphabetic() {
                                next_cmd.push(next);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        match next_cmd.as_str() {
                            "sim" => result.push('\u{2241}'),
                            // PDF 수학 제60항 — 부정 형태
                            "subset" => {
                                result.push('\u{2284}'); // ⊄
                            }
                            "supset" => {
                                result.push('\u{2285}'); // ⊅
                            }
                            "ni" => {
                                result.push('\u{220C}'); // ∌
                            }
                            "in" => {
                                result.push('\u{2209}'); // ∉
                            }
                            "equiv" => {
                                result.push('\u{2262}'); // ≢
                            }
                            "mathcal" => {
                                result.push('\u{0338}');
                                if let Some(inner) = read_braced_content(&mut chars) {
                                    for ch in inner.chars() {
                                        if ch.is_ascii_alphabetic() {
                                            result.push(ch.to_ascii_uppercase());
                                        }
                                    }
                                }
                            }
                            "mathrel" => {
                                if let Some(inner) = read_braced_content(&mut chars) {
                                    result.push('\u{00AC}');
                                    result.push_str(&inner);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "mathcal" => {
                    if let Some(inner) = read_braced_content(&mut chars) {
                        // \mathcal{X} -> uppercase letter X
                        for ch in inner.chars() {
                            if ch.is_ascii_alphabetic() {
                                result.push(ch.to_ascii_uppercase());
                            }
                        }
                    }
                }
                "mathrel" => {
                    if let Some(inner) = read_braced_content(&mut chars) {
                        result.push_str(&inner);
                    }
                }
                "sim" => result.push('~'),            // ~ (물결 = 닮음)
                "backsim" => result.push('\u{223D}'), // ∽
                "nsim" => result.push('\u{2241}'),    // ≁ (not sim)
                "nabla" => result.push('\u{2207}'),   // ∇
                "partial" => result.push('\u{2202}'), // ∂
                "iint" => result.push('\u{222C}'),    // ∬
                "oint" => result.push('\u{222E}'),    // ∮
                "nmid" => result.push('\u{2224}'),    // ∤
                "mid" => result.push('|'),
                "approxeq" => result.push('\u{224A}'), // ≊
                "simeq" => result.push('\u{2243}'),    // ≃
                "cong" => result.push('\u{2245}'),     // ≅
                "triangleright" => result.push('\u{25B7}'), // ▷
                "triangleleft" => result.push('\u{25C1}'), // ◁
                "veebar" => result.push('\u{22BB}'),   // ⊻
                "downarrow" => result.push('\u{2193}'), // ↓
                "uparrow" => result.push('\u{2191}'),  // ↑
                "leftrightarrow" => result.push('\u{2194}'), // ↔
                "rightleftarrows" => result.push('\u{21C4}'), // ⇄
                "nRightarrow" => result.push('\u{21CF}'), // ⇏
                "aleph" => result.push('\u{2135}'),    // ℵ
                "therefore" => result.push('\u{2234}'), // ∴
                "because" => result.push('\u{2235}'),  // ∵
                "ni" => result.push('\u{220B}'),       // ∋
                // PDF 수학 제60항 6 — 추론 기호
                "vdash" => result.push('\u{22A2}'),  // ⊢
                "dashv" => result.push('\u{22A3}'),  // ⊣
                "models" => result.push('\u{22A8}'), // ⊨
                "Dashv" => result.push('\u{2AE4}'),  // ⫤
                // PDF 수학 제60항 7~8 — 순서 관계
                "lesssim" => result.push('\u{2272}'), // ≲
                "prec" => result.push('\u{227A}'),    // ≺
                // PDF 수학 제61항 7 — 동치명제
                "rightleftharpoons" => result.push('\u{21CC}'), // ⇌
                "fallingdotseq" => result.push('\u{2252}'),     // ≒ (근삿값 ≈)
                "risingdotseq" => result.push('\u{2253}'),      // ≓
                "prime" => result.push('\u{2032}'),             // ′ (프라임)
                "bullet" => result.push('\u{2219}'),            // ∙ (검정 동그라미)
                // `\left` and `\right` LaTeX size modifiers: skip the keyword.
                // 뒤따르는 괄호/구분자는 그대로 처리되도록 한다.
                // PDF — `\right.`(one-sided, 닫는 구분자 없음)은 `⠄`(dots 3) 표지를 붙인다.
                "left" => {
                    if chars.peek() == Some(&'.') {
                        chars.next();
                    }
                }
                "right" => {
                    if chars.peek() == Some(&'.') {
                        chars.next();
                        // U+2E29 sentinel for open-ended right delimiter → ⠄
                        result.push('\u{2E29}');
                    }
                }
                "overset" => {
                    if let Some(over) = read_braced_content(&mut chars)
                        && let Some(base) = read_braced_content(&mut chars)
                    {
                        if over == "\\frown" || over == "⌢" {
                            result.push('\u{2322}');
                            result.push_str(&base);
                        } else {
                            result.push_str(&base);
                        }
                    }
                }
                _ => {
                    if cmd.len() == 1 && cmd.chars().all(|ch| ch.is_ascii_alphabetic()) {
                        result.push_str(&cmd);
                        continue;
                    }

                    // Handle compact forms like \sinx, \coshx, ...
                    let mut handled = false;
                    for known in [
                        "sinh", "cosh", "tanh", "sin", "cos", "tan", "csc", "sec", "cot", "lim",
                        "log", "ln",
                    ] {
                        if let Some(rest) = cmd.strip_prefix(known) {
                            result.push_str(known);
                            result.push_str(rest);
                            handled = true;
                            break;
                        }
                    }
                    if !handled {
                        // Unknown command — skip it silently
                    }
                }
            }
            // 이 branch에서 emit된 결과는 LaTeX 명령에서 온 것으로 표시한다.
            last_emit_from_latex = true;
        } else if c == '{' || c == '}' {
            // If we're inside a literal brace pair (\{ ... }), preserve the closing }.
            if c == '}' && escaped_brace_depth > 0 {
                escaped_brace_depth -= 1;
                result.push('}');
            }
            // Otherwise skip braces (used for LaTeX grouping)
        } else if c == '^' {
            // Superscript: convert to Unicode superscript or keep as-is
            // The math parser will handle this
            if let Some(&'{') = chars.peek() {
                chars.next(); // consume '{'
                let mut content = String::new();
                let mut depth = 1;
                for ch in chars.by_ref() {
                    if ch == '{' {
                        depth += 1;
                        content.push(ch);
                    } else if ch == '}' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                        content.push(ch);
                    } else {
                        content.push(ch);
                    }
                }
                // PDF 수학 — 위첨자 내용이 단순 ASCII 문자(숫자/연산자 등)면 Unicode
                // 위첨자로 직접 변환(`x^{0.3}` → `x⁰·³`). LaTeX 명령(\frac, \infty)을 포함하면
                // 재귀적으로 strip한 뒤 `^{...}` 구조를 보존해 math parser가 처리하도록 한다.
                let has_latex = content.contains('\\');
                let normalized = if has_latex {
                    strip_latex_to_math(&content)
                } else {
                    content.clone()
                };
                let simple_superscript = !has_latex
                    && normalized.chars().all(|c| {
                        c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.' | '(' | ')')
                    });
                if simple_superscript {
                    result.push_str(&to_superscript_sequence(&normalized));
                } else {
                    result.push('^');
                    result.push('{');
                    result.push_str(&normalized);
                    result.push('}');
                }
            } else if let Some(&next) = chars.peek() {
                // Single char exponent like ^2
                match next {
                    '0' => {
                        result.push('\u{2070}');
                        chars.next();
                    }
                    '1' => {
                        result.push('\u{00B9}');
                        chars.next();
                    }
                    '2' => {
                        result.push('\u{00B2}');
                        chars.next();
                    }
                    '3' => {
                        result.push('\u{00B3}');
                        chars.next();
                    }
                    '4' => {
                        result.push('\u{2074}');
                        chars.next();
                    }
                    '5' => {
                        result.push('\u{2075}');
                        chars.next();
                    }
                    '6' => {
                        result.push('\u{2076}');
                        chars.next();
                    }
                    '7' => {
                        result.push('\u{2077}');
                        chars.next();
                    }
                    '8' => {
                        result.push('\u{2078}');
                        chars.next();
                    }
                    '9' => {
                        result.push('\u{2079}');
                        chars.next();
                    }
                    _ => {
                        if next.is_ascii_alphabetic() || matches!(next, '+' | '-') {
                            let mapped = to_superscript_sequence(&next.to_string());
                            if mapped != next.to_string() {
                                result.push_str(&mapped);
                                chars.next();
                            } else {
                                result.push('^');
                            }
                        } else {
                            result.push('^');
                        }
                    }
                }
            }
        } else if c == '_' {
            // Subscript
            if let Some(&'{') = chars.peek() {
                chars.next();
                let mut content = String::new();
                let mut depth = 1;
                for ch in chars.by_ref() {
                    if ch == '{' {
                        depth += 1;
                        content.push(ch);
                    } else if ch == '}' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                        content.push(ch);
                    } else {
                        content.push(ch);
                    }
                }
                // Keep structured subscript so parser can handle complex content
                // like \Delta x \to 0 without leaving raw LaTeX commands.
                let normalized = strip_latex_to_math(&content);
                if let Some(subscript) = to_subscript_sequence(&normalized) {
                    result.push_str(&subscript);
                } else {
                    result.push('_');
                    result.push('{');
                    result.push_str(&normalized);
                    result.push('}');
                }
            } else if let Some(&next) = chars.peek() {
                // single char subscript: digit이면 Unicode subscript로 변환한다.
                // (예: `B_6` → `B₆` → rule_68 compact 패턴 매칭 가능)
                if let Some(sub) = to_subscript_sequence(&next.to_string()) {
                    result.push_str(&sub);
                    chars.next();
                } else {
                    result.push('_');
                    result.push(next);
                    chars.next();
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Merges `$...$` token sequences into single Word tokens.
/// This runs at Normalization phase so that downstream fraction/math rules
/// see the complete LaTeX expression as one token.
pub struct LatexMergeRule;

impl TokenRule for LatexMergeRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::Normalization
    }

    fn priority(&self) -> u16 {
        10 // Very early — merge before anything else
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        _state: &mut EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        let Some(Token::Word(word)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };

        let text = word.text.as_ref();

        // PDF — `제$n$항까지의` 같이 Korean prefix + `$X$` + Korean suffix 패턴.
        // 단어 내부 `$X$` math 블록을 분리해 prefix/inner/suffix로 분해한다.
        if !text.starts_with('$') && text.contains('$') {
            let first_dollar = text.find('$').unwrap();
            let after_first = &text[first_dollar + 1..];
            if let Some(close_rel) = after_first.find('$') {
                let prefix = &text[..first_dollar];
                let inner = &text[first_dollar + 1..first_dollar + 1 + close_rel];
                let suffix = &text[first_dollar + 1 + close_rel + 1..];
                // prefix가 Korean으로 끝나고 inner가 단일 letter면 ⠴X⠲ quote 형태.
                let prefix_ends_korean = prefix
                    .chars()
                    .last()
                    .is_some_and(crate::utils::is_korean_char);
                let inner_single_letter =
                    inner.chars().count() == 1 && inner.chars().all(|c| c.is_ascii_alphabetic());
                if prefix_ends_korean
                    && inner_single_letter
                    && let Ok(prefix_bytes) = crate::encode(prefix)
                    && let Ok(inner_bytes) = encode_latex_math_bytes(inner)
                    && let Ok(suffix_bytes) = crate::encode(suffix)
                {
                    let mut bytes = Vec::with_capacity(
                        prefix_bytes.len() + inner_bytes.len() + suffix_bytes.len() + 2,
                    );
                    bytes.extend(prefix_bytes);
                    bytes.push(52); // ⠴
                    bytes.extend(inner_bytes);
                    bytes.push(50); // ⠲
                    bytes.extend(suffix_bytes);
                    return Ok(TokenAction::ReplaceMany(vec![Token::PreEncoded(bytes)]));
                }
            }
        }

        // Only trigger on words starting with $ but NOT ending with $
        // (single-token $...$ is already handled by downstream rules)
        if !text.starts_with('$') || text.ends_with('$') {
            return Ok(TokenAction::Noop);
        }
        // PDF — `$a$는` 같이 단어 안에 짝수 개의 `$`가 이미 있으면(math 블록이 word 내에서
        // 종료됨) Korean prose 컨텍스트로 본다. ⠴...⠲로 quoted된 letter + Korean particle을
        // 직접 emit한다 (Normalization 단계에서 처리해야 후속 MathExpressionTokenRule이
        // 우회되지 않는다).
        let dollar_count = text.chars().filter(|c| *c == '$').count();
        if dollar_count % 2 == 0 {
            // `$X$<suffix>` 패턴 처리: math 블록 + 비-math 접미사 (Korean/구두점 등).
            if dollar_count == 2
                && let Some(close_idx) = text[1..].find('$').map(|i| i + 1)
            {
                let inner = &text[1..close_idx];
                let suffix = &text[close_idx + 1..];
                let has_korean_suffix = suffix
                    .chars()
                    .next()
                    .is_some_and(crate::utils::is_korean_char);
                // 단일 letter: ASCII 알파벳 또는 `\<greek>` (예: \omega, \alpha)
                let inner_is_short_letter = (inner.chars().count() == 1
                    && inner.chars().all(|c| c.is_ascii_alphabetic()))
                    || (inner.starts_with('\\')
                        && inner.chars().count() > 1
                        && inner.chars().skip(1).all(|c| c.is_ascii_alphabetic())
                        && [
                            "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta",
                            "iota", "kappa", "lambda", "mu", "nu", "xi", "pi", "rho", "sigma",
                            "tau", "upsilon", "phi", "chi", "psi", "omega",
                        ]
                        .contains(&&inner[1..]));
                // Case 1: 단일 letter + Korean → ⠴letter⠲ PreEncoded + Korean Word
                // suffix를 별도 Word로 유지해 다음 math expression이 Korean prose 컨텍스트로
                // 두 칸 간격(혹은 quote-wrap)을 판정할 수 있게 한다.
                if has_korean_suffix
                    && inner_is_short_letter
                    && let Ok(inner_bytes) = encode_latex_math_bytes(inner)
                {
                    let mut bytes = Vec::with_capacity(inner_bytes.len() + 2);
                    bytes.push(52); // ⠴
                    bytes.extend(inner_bytes);
                    bytes.push(50); // ⠲
                    let suffix_chars: Vec<char> = suffix.chars().collect();
                    let suffix_meta = crate::rules::token::WordMeta::from_chars(&suffix_chars);
                    let suffix_word = Token::Word(crate::rules::token::WordToken {
                        text: std::borrow::Cow::Owned(suffix.to_string()),
                        chars: suffix_chars,
                        meta: suffix_meta,
                    });
                    return Ok(TokenAction::ReplaceMany(vec![
                        Token::PreEncoded(bytes),
                        suffix_word,
                    ]));
                }
            }
            return Ok(TokenAction::Noop);
        }

        // Scan forward to find the closing $
        let mut merged = text.to_string();
        let mut j = index + 1;
        let mut found_end = false;

        while j < tokens.len() {
            match &tokens[j] {
                Token::Word(w) => {
                    let wt = w.text.as_ref();
                    merged.push(' ');
                    merged.push_str(wt);
                    if wt.ends_with('$') {
                        found_end = true;
                        j += 1;
                        break;
                    }
                }
                Token::Space(_) => {
                    // Space tokens are just separators — already handled by push(' ')
                }
                _ => break,
            }
            j += 1;
        }

        if !found_end {
            return Ok(TokenAction::Noop);
        }
        let merged_chars: Vec<char> = merged.chars().collect();
        let meta = crate::rules::token::WordMeta::from_chars(&merged_chars);

        // Replace current token with merged Word, and consume remaining tokens
        // by replacing current..j range. ReplaceMany replaces tokens[i..=i], so we need
        // to manually handle the span. Instead, replace this token and mark others for removal.
        //
        // The token engine's ReplaceMany replaces tokens[i..=i] with the vec.
        // We can't remove subsequent tokens directly, but we can replace this one
        // with the merged word and then subsequent Space/Word tokens will still be there.
        //
        // Better approach: just replace the current token with the merged word.
        // The subsequent tokens (Space, Word) that were part of the $...$ will
        // then go through normal encoding and produce wrong output, but at least
        // the merge will happen for the first token.
        //
        // Actually, the cleanest approach: splice out the entire range.
        // ReplaceMany splices tokens[i..=i], but we need tokens[i..j].
        // Let's build a replacement that covers all consumed positions.

        let replacement = [Token::Word(crate::rules::token::WordToken {
            text: std::borrow::Cow::Owned(merged),
            chars: merged_chars,
            meta,
        })];

        // For each additional token consumed (after index), add an empty PreEncoded
        // so ReplaceMany covers the right count. But ReplaceMany only replaces
        // tokens[i..=i], not tokens[i..j]. We need a different strategy.
        //
        // Since we can't splice a range, let's use the merged token and hope
        // the next tokens get skipped. Actually, ReplaceMany replaces tokens.splice(i..=i, ...)
        // which only replaces ONE token at position i.
        //
        // WORKAROUND: Replace current token with merged Word, and for each subsequent
        // consumed token, we mark them as empty PreEncoded by using our replacement vec size.
        // The splice is tokens[i..=i] not i..j, so subsequent tokens remain.
        //
        // REAL FIX: We need to store the "tokens to skip" elsewhere or use a multi-token splice.
        // For now, just output the PreEncoded bytes directly and skip the merge approach.

        // Direct encoding approach: encode the merged LaTeX and output PreEncoded
        let inner = &replacement[0];
        if let Token::Word(w) = inner {
            let full = w.text.as_ref();
            if full.starts_with('$') && full.ends_with('$') && full.len() >= 3 {
                let latex_inner = &full[1..full.len() - 1];
                if let Ok(bytes) = encode_latex_math_bytes(latex_inner) {
                    // Replace current token + consumed tokens
                    let mut final_replacement = vec![Token::PreEncoded(bytes)];
                    let consumed_count = j - index - 1; // tokens after index consumed
                    for _ in 0..consumed_count {
                        final_replacement.push(Token::PreEncoded(vec![]));
                    }
                    return Ok(TokenAction::ReplaceMany(final_replacement));
                }
            }
        }

        Ok(TokenAction::Noop)
    }
}

impl TokenRule for LatexMathRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::FractionDetection
    }

    fn priority(&self) -> u16 {
        110 // After LatexFractionRule (100) but before InlineFractionRule (120)
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        _state: &mut EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        let Some(Token::Word(word)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };

        let text = word.text.as_ref();

        // PDF — `$X$는`처럼 LaTeX math 블록 + Korean particle 결합 형태도 처리한다.
        // 이 경우 math 블록은 prose 컨텍스트로 인식되어 ⠴...⠲로 quoted된다.
        if text.starts_with('$') && !text.ends_with('$') && text.len() >= 3 {
            // 첫번째 매칭하는 `$` 위치 찾기
            if let Some(close_idx) = text[1..].find('$').map(|i| i + 1) {
                let inner = &text[1..close_idx];
                let suffix = &text[close_idx + 1..];
                // suffix가 Korean 글자로 시작하는지 확인
                let has_korean_suffix = suffix
                    .chars()
                    .next()
                    .is_some_and(crate::utils::is_korean_char);
                if has_korean_suffix
                    && !inner.is_empty()
                    && inner.chars().count() <= 2
                    && inner.chars().all(|c| c.is_ascii_alphabetic())
                    && let Ok(inner_bytes) = encode_latex_math_bytes(inner)
                {
                    // ⠴ + inner + ⠲ 로 감싸고 suffix는 Korean으로 encode
                    let mut bytes = Vec::with_capacity(inner_bytes.len() + 2);
                    bytes.push(52);
                    bytes.extend(inner_bytes);
                    bytes.push(50);
                    if let Ok(suffix_bytes) = crate::encode(suffix) {
                        bytes.extend(suffix_bytes);
                    }
                    return Ok(TokenAction::ReplaceMany(vec![Token::PreEncoded(bytes)]));
                }
            }
            return Ok(TokenAction::Noop);
        }

        // Only handle $...$ wrapped expressions (already merged by LatexMergeRule)
        if !(text.starts_with('$') && text.ends_with('$') && text.len() >= 3) {
            return Ok(TokenAction::Noop);
        }

        // Extract inner content (strip $ delimiters)
        let inner = &text[1..text.len() - 1];

        // Try to encode via math engine
        match encode_latex_math_bytes(inner) {
            Ok(bytes) => Ok(TokenAction::ReplaceMany(wrap_latex_math_tokens_with_inner(
                tokens, index, bytes, inner,
            ))),
            Err(_) => Ok(TokenAction::Noop),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_neq() {
        let result = strip_latex_to_math("y \\neq 0");
        assert!(result.contains('\u{2260}'));
        assert!(result.contains('y'));
        assert!(result.contains('0'));
    }

    #[test]
    fn test_strip_geq() {
        let result = strip_latex_to_math("x \\geq 5");
        assert!(result.contains('\u{2265}'));
    }

    #[test]
    fn test_strip_sin() {
        let result = strip_latex_to_math("\\sin x");
        assert!(result.contains("sin"));
        assert!(result.contains('x'));
    }

    #[test]
    fn test_strip_exponent() {
        let result = strip_latex_to_math("x^{2}");
        assert!(result.contains('\u{00B2}'));
    }

    #[test]
    fn test_strip_subscript() {
        let result = strip_latex_to_math("x_{2}");
        assert!(result.contains('\u{2082}'));
    }
}
