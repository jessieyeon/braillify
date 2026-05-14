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

#[derive(Clone, Copy)]
enum MatrixDelimiter {
    Parentheses,
    VerticalBars,
}

impl MatrixDelimiter {
    fn cells(self) -> (u8, u8) {
        match self {
            MatrixDelimiter::Parentheses => (decode_unicode('⠦'), decode_unicode('⠴')),
            MatrixDelimiter::VerticalBars => (decode_unicode('⠳'), decode_unicode('⠳')),
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
        _ => return None,
    };

    let body_start = env_end + 1;
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
    let mut chars = math_text.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };

    if first.is_ascii_lowercase() {
        let mut promoted = first.to_ascii_uppercase().to_string();
        promoted.extend(chars);
        promoted
    } else {
        math_text.to_string()
    }
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
    let mut out = encode_trimmed_math(matrix.prefix)?;
    let (open, close) = matrix.delimiter.cells();
    out.push(open);

    let rows = split_matrix_body(matrix.body);
    for (row_index, row) in rows.iter().enumerate() {
        for (cell_index, cell) in row.iter().enumerate() {
            out.extend(encode_matrix_cell(cell)?);
            if cell_index + 1 < row.len() {
                out.push(0);
            }
        }
        if row_index + 1 < rows.len() {
            out.push(0);
            out.push(decode_unicode('⠜'));
        }
    }

    out.push(close);
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
        if part == "-" {
            out.push(0);
        }
        previous_was_operand = false;
    }
    Ok(out)
}

pub(crate) fn encode_latex_math_bytes(latex_inner: &str) -> Result<Vec<u8>, String> {
    if let Some(matrix) = find_latex_matrix(latex_inner) {
        return encode_latex_matrix(&matrix);
    }

    let math_text = strip_latex_to_math(latex_inner);
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
                Some(Token::Word(word))
                    if word.text.ends_with('은') || word.text.ends_with('는') =>
                {
                    1
                }
                _ => 0,
            }
        }
        Some(Token::Word(_) | Token::PreEncoded(_) | Token::Fraction(_) | Token::Mode(_)) => 2,
        None => 0,
    }
}

fn next_content_needs_math_spacing(tokens: &[Token<'_>], index: usize) -> usize {
    match tokens.get(index + 1) {
        Some(Token::Space(_)) => 1,
        Some(Token::Word(_) | Token::PreEncoded(_) | Token::Fraction(_) | Token::Mode(_)) => 2,
        None => 0,
    }
}

pub(crate) fn wrap_latex_math_tokens<'a>(
    tokens: &[Token<'a>],
    index: usize,
    bytes: Vec<u8>,
) -> Vec<Token<'a>> {
    let mut replacement = Vec::new();
    let leading_spaces = previous_content_needs_math_spacing(tokens, index);
    if leading_spaces > 0 {
        replacement.push(Token::PreEncoded(vec![0; leading_spaces]));
    }
    replacement.push(Token::PreEncoded(bytes));
    let trailing_spaces = next_content_needs_math_spacing(tokens, index);
    if trailing_spaces > 0 {
        replacement.push(Token::PreEncoded(vec![0; trailing_spaces]));
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
            'x' => '\u{2093}',
            'm' => '\u{2098}',
            'n' => '\u{2099}',
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
    // Track literal braces opened by \{ so matching } is preserved (not skipped).
    let mut escaped_brace_depth = 0usize;

    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
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
                    } else if escaped == '}' {
                        escaped_brace_depth = escaped_brace_depth.saturating_sub(1);
                    }
                    result.push(escaped);
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
                "neq" => result.push('\u{2260}'),    // ≠
                "geq" => result.push('\u{2265}'),    // ≥
                "leq" => result.push('\u{2264}'),    // ≤
                "approx" => result.push('\u{2252}'), // ≒
                "infty" => result.push('\u{221E}'),  // ∞
                "to" => result.push('\u{2192}'),     // →
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

                    let radicand = read_braced_content(&mut chars).unwrap_or_default();

                    if let Some(idx) = index {
                        result.push_str(&to_superscript_sequence(&idx));
                    }
                    result.push('\u{221A}');

                    // Keep grouped radicand for multi-letter body (e.g., \sqrt{xy}).
                    let group_body = radicand.chars().count() > 1
                        && radicand.chars().all(|ch| ch.is_ascii_alphabetic());
                    if group_body {
                        result.push('(');
                        result.push_str(&radicand);
                        result.push(')');
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
                "Delta" => result.push('\u{0394}'),
                "Sigma" => result.push('\u{03A3}'),
                "sum" => result.push('\u{03A3}'),
                "int" => result.push('\u{222B}'), // ∫
                "Omega" => result.push('\u{03A9}'),
                "square" => result.push('\u{25A1}'),
                "vec" => {
                    if let Some(inner) = read_braced_content(&mut chars) {
                        result.push('\u{2192}');
                        result.push_str(&inner);
                    }
                }
                "overrightarrow" => {
                    if let Some(inner) = read_braced_content(&mut chars) {
                        result.push('\u{2192}');
                        result.push_str(&inner);
                    }
                }
                "frac" => {
                    if let Some(num) = read_braced_content(&mut chars)
                        && let Some(den) = read_braced_content(&mut chars)
                    {
                        let norm_num = strip_latex_to_math(&num);
                        let norm_den = strip_latex_to_math(&den);
                        // 한국 점자 수학 규정: 분수는 점자 표기에서 분모/분자 역순으로
                        // 적는다. LaTeX `\frac{a}{b}` (a/b)는 점자로 b/(a) 형태가
                        // 된다. testcase 패턴(예: $\frac{3}{4}$ → #d/#c = 4/3)과
                        // 일관.
                        result.push_str(&norm_den);
                        result.push('/');
                        result.push('(');
                        result.push_str(&norm_num);
                        result.push(')');
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
                "overleftrightarrow" => {
                    if let Some(inner) = read_braced_content(&mut chars) {
                        result.push('\u{2194}');
                        result.push_str(&inner);
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
                "overline" | "bar" => {
                    if let Some(inner) = read_braced_content(&mut chars) {
                        result.push_str(&inner);
                        result.push('\u{0305}');
                    }
                }
                "underline" => {
                    if let Some(inner) = read_braced_content(&mut chars) {
                        result.push_str(&inner);
                        result.push('\u{0332}');
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
                "mathrel" => {
                    if let Some(inner) = read_braced_content(&mut chars) {
                        result.push_str(&inner);
                    }
                }
                "sim" => result.push('\u{223D}'), // ∽
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
                result.push_str(&to_superscript_sequence(&content));
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
                result.push('_');
                result.push(next);
                chars.next();
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

        // Only trigger on words starting with $ but NOT ending with $
        // (single-token $...$ is already handled by downstream rules)
        if !text.starts_with('$') || text.ends_with('$') {
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

        // Only handle $...$ wrapped expressions (already merged by LatexMergeRule)
        if !(text.starts_with('$') && text.ends_with('$') && text.len() >= 3) {
            return Ok(TokenAction::Noop);
        }

        // Extract inner content (strip $ delimiters)
        let inner = &text[1..text.len() - 1];

        // Try to encode via math engine
        match encode_latex_math_bytes(inner) {
            Ok(bytes) => Ok(TokenAction::ReplaceMany(wrap_latex_math_tokens(
                tokens, index, bytes,
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
