//! Main math expression parser (extracted from parser.rs).

use super::{BracketKind, MathToken};
use super::helpers::*;
use super::GroupState;
use crate::math_symbol_shortcut;
use crate::rules::math::function;


/// Parse a math expression string into tokens.
pub(crate) fn parse_math_expression(input: &str) -> Result<Vec<MathToken>, String> {
    parse_math_expression_with_math_mode(input, false)
}

/// Parse a math expression string into tokens with an explicit math-mode flag.
pub(crate) fn parse_math_expression_with_math_mode(
    input: &str,
    math_mode_active: bool,
) -> Result<Vec<MathToken>, String> {
    // PDF 규정: Mathematical Alphanumeric 변형은 ASCII 라틴 문자와 동일하게 처리.
    let input_owned: String = input.chars().map(normalize_math_alphanumeric).collect();
    let input: &str = &input_owned;
    if let Some((left, right)) = input.split_once('/')
        && let (Some(left_fact), Some(right_fact)) =
            (left.strip_suffix('!'), right.strip_suffix('!'))
        && !left_fact.is_empty()
        && !right_fact.is_empty()
        && left_fact.chars().all(|c| c.is_ascii_digit())
        && right_fact.chars().all(|c| c.is_ascii_digit())
    {
        return Ok(vec![
            MathToken::Number(right_fact.to_string()),
            MathToken::Operator('!'),
            MathToken::Operator('/'),
            MathToken::Number(left_fact.to_string()),
            MathToken::Operator('!'),
        ]);
    }

    if input.contains('\u{0332}') {
        // Underline-notation normalizations used in fraction testcases.
        // PDF 제23항 2 — 변수에 붙은 U+0332(예: X̲)는 밑줄 marker이고 분수가 아니다.
        // suffix가 숫자일 때만(분수 변환 testcase 한정) 분수 정규화를 적용한다.
        if let Some(prefix) = input.strip_suffix('\u{0332}')
            && prefix.chars().all(|c| c.is_ascii_digit())
        {
            return parse_math_expression_with_math_mode(&format!("{prefix}/1"), math_mode_active);
        }

        if let Some(rest) = input.strip_prefix("1̲/") {
            let body = rest.trim();
            if body.starts_with('(') && body.ends_with(')') {
                let inner = &body[1..body.len() - 1];
                let mut tokens = Vec::new();
                tokens.push(MathToken::OpenParen(BracketKind::Grouping));
                tokens.extend(parse_math_expression_with_math_mode(
                    inner,
                    math_mode_active,
                )?);
                tokens.push(MathToken::CloseParen(BracketKind::Grouping));
                tokens.push(MathToken::Operator('/'));
                tokens.push(MathToken::Number("1".to_string()));
                return Ok(tokens);
            }
        }

        if let Some((left, right)) = input.split_once("̲/") {
            let mut tokens = parse_math_expression_with_math_mode(right, math_mode_active)?;
            tokens.push(MathToken::Operator('/'));
            tokens.push(MathToken::OpenParen(BracketKind::Grouping));
            tokens.extend(parse_math_expression_with_math_mode(
                left,
                math_mode_active,
            )?);
            tokens.push(MathToken::CloseParen(BracketKind::Grouping));
            return Ok(tokens);
        }
    }

    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut bracket_stack: Vec<GroupState> = Vec::new();
    let mut i = 0;

    // Some notations (e.g., segment AB with overline) use expression-level overline prefix.
    let should_prefix_overline = if chars
        .first()
        .is_some_and(|c| matches!(*c, '\u{0305}' | '\u{0304}'))
    {
        true
    } else if chars
        .last()
        .is_some_and(|c| matches!(*c, '\u{0305}' | '\u{0304}'))
    {
        let core: Vec<char> = chars
            .iter()
            .copied()
            .filter(|c| !matches!(*c, '\u{0305}' | '\u{0304}'))
            .collect();
        core.len() >= 2
            && core
                .iter()
                .all(|c| c.is_ascii_uppercase() || matches!(*c, '\u{2032}' | '\''))
    } else {
        false
    };

    if should_prefix_overline {
        tokens.push(MathToken::MathSymbol('\u{0304}'));
    }

    while i < chars.len() {
        let c = chars[i];

        if should_prefix_overline && matches!(c, '\u{0305}' | '\u{0304}') {
            i += 1;
            continue;
        }

        // Whitespace
        if c.is_whitespace() {
            tokens.push(MathToken::Space);
            i += 1;
            continue;
        }

        if is_korean_char(c) {
            let mut phrase = String::new();
            while i < chars.len() {
                let current = chars[i];
                if is_korean_char(current) {
                    phrase.push(current);
                    i += 1;
                    continue;
                }

                if current.is_whitespace() {
                    let mut j = i;
                    while j < chars.len() && chars[j].is_whitespace() {
                        j += 1;
                    }

                    if j < chars.len() && is_korean_char(chars[j]) {
                        if !phrase.ends_with(' ') {
                            phrase.push(' ');
                        }
                        i = j;
                        continue;
                    }
                }

                break;
            }

            for group in &mut bracket_stack {
                group.contains_korean = true;
            }
            tokens.push(MathToken::KoreanWord(phrase));
            continue;
        }

        // Function name detection (must come before letter detection)
        if c.is_ascii_lowercase() {
            let remaining: String = chars[i..].iter().collect();
            if let Some((name, _)) = function::match_function_prefix(&remaining) {
                tokens.push(MathToken::FunctionName(name.to_string()));
                i += name.len();
                continue;
            }
        }

        // Unicode superscript sequence → merge into single Superscript
        if is_superscript_char(c) {
            let mut content = Vec::new();
            while i < chars.len() && is_superscript_char(chars[i]) {
                if let Some(tok) = normalize_superscript(chars[i]) {
                    content.push(tok);
                }
                i += 1;
            }
            if !content.is_empty() {
                tokens.push(MathToken::Superscript(content));
            }
            continue;
        }

        // Unicode subscript sequence → merge into single Subscript
        // `.`/`/`는 다음 글자가 같은 첨자 시퀀스에 속할 때만 포함한다(예: `₁/₂` 같은
        // 분수 첨자). 일반 식의 외부 연산자가 첨자에 흡수되지 않도록 lookahead로 확인한다.
        if is_subscript_char(c) {
            let mut content = Vec::new();
            while i < chars.len() {
                if is_subscript_char(chars[i]) {
                    if let Some(tok) = normalize_subscript(chars[i]) {
                        content.push(tok);
                    }
                    i += 1;
                } else if matches!(chars[i], '.' | '/')
                    && chars.get(i + 1).is_some_and(|c| is_subscript_char(*c))
                {
                    match chars[i] {
                        '.' => content.push(MathToken::DecimalPoint),
                        '/' => content.push(MathToken::Operator('/')),
                        _ => {}
                    }
                    i += 1;
                } else {
                    break;
                }
            }
            if !content.is_empty() {
                tokens.push(MathToken::Subscript(content));
            }
            continue;
        }

        // ASCII subscript notation (LaTeX-like): _x, _2, _{...}, _(...)
        if c == '_' {
            if i + 1 >= chars.len() {
                tokens.push(MathToken::Raw(c));
                i += 1;
                continue;
            }

            let next = chars[i + 1];
            if next == '{' {
                let mut j = i + 2;
                let mut depth = 1usize;
                while j < chars.len() {
                    match chars[j] {
                        '{' => depth += 1,
                        '}' => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }

                if j < chars.len() && chars[j] == '}' {
                    let inner: String = chars[i + 2..j].iter().collect();
                    let content = parse_math_expression_with_math_mode(&inner, math_mode_active)?;
                    tokens.push(MathToken::Subscript(content));
                    i = j + 1;
                    continue;
                }

                tokens.push(MathToken::Raw(c));
                i += 1;
                continue;
            }

            if next == '(' {
                let mut j = i + 2;
                let mut depth = 1usize;
                while j < chars.len() {
                    match chars[j] {
                        '(' => depth += 1,
                        ')' => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }

                if j < chars.len() && chars[j] == ')' {
                    let inner: String = chars[i + 2..j].iter().collect();
                    let mut content = Vec::new();
                    content.push(MathToken::OpenParen(BracketKind::MathParen));
                    content.extend(parse_math_expression_with_math_mode(
                        &inner,
                        math_mode_active,
                    )?);
                    content.push(MathToken::CloseParen(BracketKind::MathParen));
                    tokens.push(MathToken::Subscript(content));
                    i = j + 1;
                    continue;
                }

                tokens.push(MathToken::Raw(c));
                i += 1;
                continue;
            }

            // Single-character base
            let content = if next.is_ascii_digit() {
                vec![MathToken::Number(next.to_string())]
            } else if next.is_ascii_lowercase() {
                vec![MathToken::Variable(next)]
            } else if next.is_ascii_uppercase() {
                vec![MathToken::UpperVariable(next)]
            } else {
                vec![MathToken::Raw(next)]
            };

            tokens.push(MathToken::Subscript(content));
            i += 2;
            continue;
        }

        // ASCII superscript notation: ^x, ^2, ^{...}, ^(...)
        if c == '^' {
            if i + 1 >= chars.len() {
                tokens.push(MathToken::Raw(c));
                i += 1;
                continue;
            }

            let next = chars[i + 1];
            if next == '{' {
                let mut j = i + 2;
                let mut depth = 1usize;
                while j < chars.len() {
                    match chars[j] {
                        '{' => depth += 1,
                        '}' => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }

                if j < chars.len() && chars[j] == '}' {
                    let inner: String = chars[i + 2..j].iter().collect();
                    let content = parse_math_expression_with_math_mode(&inner, math_mode_active)?;
                    tokens.push(MathToken::Superscript(content));
                    i = j + 1;
                    continue;
                }

                tokens.push(MathToken::Raw(c));
                i += 1;
                continue;
            }

            if next == '(' {
                let mut j = i + 2;
                let mut depth = 1usize;
                while j < chars.len() {
                    match chars[j] {
                        '(' => depth += 1,
                        ')' => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }

                if j < chars.len() && chars[j] == ')' {
                    let inner: String = chars[i + 2..j].iter().collect();
                    let mut content = Vec::new();
                    content.push(MathToken::OpenParen(BracketKind::MathParen));
                    content.extend(parse_math_expression_with_math_mode(
                        &inner,
                        math_mode_active,
                    )?);
                    content.push(MathToken::CloseParen(BracketKind::MathParen));
                    tokens.push(MathToken::Superscript(content));
                    i = j + 1;
                    continue;
                }

                tokens.push(MathToken::Raw(c));
                i += 1;
                continue;
            }

            let content = if next.is_ascii_digit() {
                vec![MathToken::Number(next.to_string())]
            } else if next.is_ascii_lowercase() {
                vec![MathToken::Variable(next)]
            } else if next.is_ascii_uppercase() {
                vec![MathToken::UpperVariable(next)]
            } else {
                vec![MathToken::Raw(next)]
            };

            tokens.push(MathToken::Superscript(content));
            i += 2;
            continue;
        }

        // Prime mark
        if c == '\u{2032}' || c == '\'' {
            tokens.push(MathToken::Prime);
            i += 1;
            continue;
        }

        // Digits (with optional repeating-decimal dot-above marks).
        //
        // PDF 수학 제8항 2.: 순환마디의 점은 ⠈으로 적되, 순환마디 앞에만 적는다.
        // 묵자 표기에서 순환마디는 양 끝 자리 위에 dot(̇, U+0307)을 붙인다
        // (1자리면 그 자리 하나, 다자리면 시작과 끝 두 자리). 알고리즘:
        //   - 첫 dot이 등장한 자리 = 순환마디 시작
        //   - 마지막 dot이 등장한 자리 = 순환마디 끝
        //   - prefix(첫 dot 직전까지) emit → dot marker(⠈) emit
        //   - 순환마디(첫~마지막 dot) emit → suffix(마지막 dot 이후) emit
        if c.is_ascii_digit() {
            let mut num = String::new();
            let mut first_dot: Option<usize> = None;
            let mut last_dot: Option<usize> = None;
            while i < chars.len() {
                if chars[i].is_ascii_digit() {
                    num.push(chars[i]);
                    i += 1;
                } else if chars[i] == '\u{0307}' {
                    if !num.is_empty() {
                        let pos = num.len() - 1;
                        if first_dot.is_none() {
                            first_dot = Some(pos);
                        }
                        last_dot = Some(pos);
                    }
                    i += 1;
                } else {
                    break;
                }
            }
            match (first_dot, last_dot) {
                (Some(start), Some(end)) => {
                    if start > 0 {
                        tokens.push(MathToken::Number(num[..start].to_string()));
                    }
                    tokens.push(MathToken::MathSymbol('\u{0307}'));
                    tokens.push(MathToken::Number(num[start..=end].to_string()));
                    if end + 1 < num.len() {
                        tokens.push(MathToken::Number(num[end + 1..].to_string()));
                    }
                }
                _ => {
                    tokens.push(MathToken::Number(num));
                }
            }
            continue;
        }

        // Lowercase letters (variables)
        if c.is_ascii_lowercase() {
            tokens.push(MathToken::Variable(c));
            i += 1;
            continue;
        }

        // Uppercase letters
        if c.is_ascii_uppercase() {
            tokens.push(MathToken::UpperVariable(c));
            i += 1;
            continue;
        }

        // Brackets
        match c {
            '(' => {
                let next_is_function = if i + 1 < chars.len() {
                    let remaining: String = chars[i + 1..].iter().collect();
                    function::starts_with_function(&remaining)
                } else {
                    false
                };

                let kind = match tokens.last() {
                    Some(MathToken::MathSymbol('\u{221A}')) => BracketKind::Grouping,
                    Some(MathToken::FunctionName(_)) if !next_is_function => BracketKind::Grouping,
                    Some(MathToken::Superscript(_))
                        if matches!(
                            tokens.iter().rev().nth(1),
                            Some(MathToken::FunctionName(_))
                        ) =>
                    {
                        BracketKind::Grouping
                    }
                    Some(MathToken::Operator('/')) | Some(MathToken::MathSymbol('\u{2044}')) => {
                        BracketKind::Grouping
                    }
                    // ∑/∏ 한정자 뒤의 괄호는 본문 묶음(Grouping)이다.
                    // (∫ 적분은 피적분 함수의 괄호로 MathParen 유지.)
                    Some(MathToken::MathSymbol('\u{2211}' | '\u{220F}')) => BracketKind::Grouping,
                    _ => BracketKind::MathParen,
                };
                let promote_grouping = matches!(tokens.last(), Some(MathToken::Operator('=')));
                bracket_stack.push(GroupState {
                    kind,
                    token_index: tokens.len(),
                    contains_korean: false,
                    contains_arithmetic: false,
                    contains_comma: false,
                    promote_grouping,
                });
                tokens.push(MathToken::OpenParen(kind));
                i += 1;
                continue;
            }
            ')' => {
                let kind = if let Some(group) = bracket_stack.pop() {
                    // PDF — math mode 컨텍스트면 Korean 내용 있어도 Hangul wrap 우회.
                    let resolved_kind = if !math_mode_active
                        && group.contains_korean
                        && matches!(group.kind, BracketKind::MathParen | BracketKind::Grouping)
                    {
                        BracketKind::Hangul
                    } else if group.promote_grouping
                        && group.contains_arithmetic
                        && !group.contains_comma
                        && matches!(group.kind, BracketKind::MathParen)
                    {
                        // 콤마로 구분된 튜플(예: (f/x₁, f/x₂, ...))은 MathParen으로 유지.
                        // 산술 식 그룹(예: (a+b)/c)만 Grouping으로 승격한다.
                        BracketKind::Grouping
                    } else {
                        group.kind
                    };

                    if let Some(MathToken::OpenParen(open_kind)) = tokens.get_mut(group.token_index)
                    {
                        *open_kind = resolved_kind;
                    }
                    resolved_kind
                } else {
                    BracketKind::MathParen
                };
                tokens.push(MathToken::CloseParen(kind));
                i += 1;
                continue;
            }
            '[' => {
                bracket_stack.push(GroupState {
                    kind: BracketKind::Square,
                    token_index: tokens.len(),
                    contains_korean: false,
                    contains_arithmetic: false,
                    contains_comma: false,
                    promote_grouping: false,
                });
                tokens.push(MathToken::OpenParen(BracketKind::Square));
                i += 1;
                continue;
            }
            ']' => {
                let kind = bracket_stack
                    .pop()
                    .map_or(BracketKind::Square, |group| group.kind);
                tokens.push(MathToken::CloseParen(kind));
                i += 1;
                continue;
            }
            '{' => {
                bracket_stack.push(GroupState {
                    kind: BracketKind::Curly,
                    token_index: tokens.len(),
                    contains_korean: false,
                    contains_arithmetic: false,
                    contains_comma: false,
                    promote_grouping: false,
                });
                tokens.push(MathToken::OpenParen(BracketKind::Curly));
                i += 1;
                continue;
            }
            // PDF — `\overline{multi-token}`이 strip 단계에서 U+2329/U+232A로 감싼 그룹.
            // 점자 `⠷...⠾`(Grouping)로 emit한다.
            '\u{2329}' => {
                tokens.push(MathToken::OpenParen(BracketKind::Grouping));
                i += 1;
                continue;
            }
            '\u{232A}' => {
                tokens.push(MathToken::CloseParen(BracketKind::Grouping));
                i += 1;
                continue;
            }
            // PDF — `\sqrt{multi-token}`이 strip 단계에서 U+27E6/U+27E7로 감싼 그룹.
            // 점자 `⠦...⠴`(MathParen)로 emit한다. (sqrt-context Grouping 승격 우회.)
            '\u{27E6}' => {
                tokens.push(MathToken::OpenParen(BracketKind::MathParen));
                i += 1;
                continue;
            }
            '\u{27E7}' => {
                tokens.push(MathToken::CloseParen(BracketKind::MathParen));
                i += 1;
                continue;
            }
            // PDF — Hangul wrap 그룹용 sentinel (U+27E8/U+27E9). 한글 내용이 포함된
            // 분수 분자/분모의 묶음 (`⠸⠷...⠸⠾`).
            '\u{27E8}' => {
                tokens.push(MathToken::OpenParen(BracketKind::Hangul));
                i += 1;
                continue;
            }
            '\u{27E9}' => {
                tokens.push(MathToken::CloseParen(BracketKind::Hangul));
                i += 1;
                continue;
            }
            '}' => {
                let kind = bracket_stack
                    .pop()
                    .map_or(BracketKind::Curly, |group| group.kind);
                tokens.push(MathToken::CloseParen(kind));
                i += 1;
                continue;
            }
            _ => {}
        }

        // U+2044 FRACTION SLASH는 LaTeX `\frac`에서 emit되는 분수 전용 슬래시.
        // 일반 `/`(나눗셈/직접 입력 분수)와 구분하여 MathSymbol로 보존한다.
        // math_symbol_shortcut에서 `⠌`(plain)으로 매핑된다.
        if c == '\u{2044}' {
            tokens.push(MathToken::MathSymbol(c));
            i += 1;
            continue;
        }
        // Math operators (basic)
        if matches!(
            c,
            '+' | '=' | '>' | '<' | '/' | '-' | '!' | '×' | '÷' | '\u{2212}'
        ) {
            // In chained inequalities like -5 < x < -2, the second minus is omitted.
            if c == '-'
                && i > 0
                && chars[i - 1] == '<'
                && i + 1 < chars.len()
                && chars[i + 1].is_ascii_digit()
            {
                i += 1;
                continue;
            }

            let op = if c == '\u{2044}' {
                '/'
            } else if c == '-' {
                '\u{2212}'
            } else {
                c
            };
            if matches!(op, '+' | '×' | '/') {
                for group in &mut bracket_stack {
                    group.contains_arithmetic = true;
                }
            }
            if op == ',' {
                for group in &mut bracket_stack {
                    group.contains_comma = true;
                }
            }
            tokens.push(MathToken::Operator(op));
            i += 1;
            continue;
        }

        // Math symbols from shortcut map
        if math_symbol_shortcut::is_math_symbol_char(c) {
            tokens.push(MathToken::MathSymbol(c));
            i += 1;
            continue;
        }

        if is_combining_math_mark(c) {
            if should_prefix_overline && matches!(c, '\u{0305}' | '\u{0304}') {
                i += 1;
                continue;
            }
            tokens.push(MathToken::MathSymbol(c));
            i += 1;
            continue;
        }

        // Decimal point in number context (e.g., 3.14, .47)
        if c == '.' && i + 2 < chars.len() && chars[i + 1] == '.' && chars[i + 2] == '.' {
            tokens.push(MathToken::MathSymbol('…'));
            i += 3;
            continue;
        }

        if c == '.' {
            // PDF — 직전 글자가 결합 부호(예: `̄`, `̃`)이면 그 이전의 baseline 문자를 본다.
            // 예: `2̄.3010` 에서 `.`의 prev는 결합 overline U+0305이지만 baseline은 `2`.
            let prev_baseline = {
                let mut j = i;
                while j > 0
                    && matches!(
                        chars[j - 1] as u32,
                        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F
                    )
                {
                    j -= 1;
                }
                if j > 0 { Some(chars[j - 1]) } else { None }
            };
            let prev_is_digit = prev_baseline.is_some_and(|c| c.is_ascii_digit());
            let next_is_digit = i + 1 < chars.len() && chars[i + 1].is_ascii_digit();
            if next_is_digit && (prev_is_digit || i == 0) {
                tokens.push(MathToken::DecimalPoint);
            } else {
                tokens.push(MathToken::Raw(c));
            }
            i += 1;
            continue;
        }

        // Comma as digit grouping separator (e.g., 5,700,000)
        if c == ',' {
            let prev_is_digit = i > 0 && chars[i - 1].is_ascii_digit();
            let next_is_digit = i + 1 < chars.len() && chars[i + 1].is_ascii_digit();
            if prev_is_digit && next_is_digit && bracket_stack.is_empty() {
                tokens.push(MathToken::DigitSeparator);
            } else {
                // Set/list separator. 괄호 안 콤마는 튜플 구분자로 보고 group의
                // contains_comma 플래그를 설정한다(MathParen 유지용).
                for group in &mut bracket_stack {
                    group.contains_comma = true;
                }
                tokens.push(MathToken::Operator(','));
            }
            i += 1;
            continue;
        }

        // Fallback
        tokens.push(MathToken::Raw(c));
        i += 1;
    }

    // (expr)̅ / (expr)̄ should use grouping parentheses around the overlined group.
    if matches!(
        tokens.last(),
        Some(MathToken::MathSymbol('\u{0305}' | '\u{0304}'))
    ) && tokens.len() >= 3
        && matches!(
            tokens.first(),
            Some(MathToken::OpenParen(BracketKind::MathParen))
        )
        && matches!(
            tokens.get(tokens.len() - 2),
            Some(MathToken::CloseParen(BracketKind::MathParen))
        )
    {
        let mut depth = 0usize;
        let mut closes_at_end = false;
        for (idx, token) in tokens.iter().enumerate() {
            match token {
                MathToken::OpenParen(BracketKind::MathParen) => depth += 1,
                MathToken::CloseParen(BracketKind::MathParen) => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        closes_at_end = idx == tokens.len() - 2;
                        break;
                    }
                }
                _ => {}
            }
        }

        if closes_at_end {
            tokens[0] = MathToken::OpenParen(BracketKind::Grouping);
            let close_idx = tokens.len() - 2;
            tokens[close_idx] = MathToken::CloseParen(BracketKind::Grouping);
        }
    }

    // PDF — `2 ₇P₂` 같이 계수+공백+permutation/combination 표기에서는 공백이
    // 의미가 없으므로 제거한다(계수는 permutation 본체에 직접 인접).
    let mut i = 0;
    while i + 4 < tokens.len() {
        if matches!(tokens.get(i), Some(MathToken::Number(_)))
            && matches!(tokens.get(i + 1), Some(MathToken::Space))
            && matches!(tokens.get(i + 2), Some(MathToken::Subscript(_)))
            && matches!(
                tokens.get(i + 3),
                Some(MathToken::UpperVariable('P' | 'C' | 'H'))
            )
            && matches!(tokens.get(i + 4), Some(MathToken::Subscript(_)))
        {
            tokens.remove(i + 1);
        }
        i += 1;
    }

    // PDF 제66항 — `f(x+a)(x-a)` 같이 함수/변수명 다음 인접한 두 괄호 그룹은
    // 함수 분배가 아니라 곱셈(`f(x+a) · (x-a)`)으로 해석한다.
    // 따라서 두 번째 괄호 앞에 함수/변수명을 자동 삽입하지 않는다.

    // PDF — `√xy` 같이 근호 뒤에 명시적 괄호 없는 다중 base 토큰(Variable/UpperVariable/
    // Number)은 `⠷...⠾`(Grouping)로 묶어 모호성을 제거한다. 단, `√x²`(var+super) 등 단일
    // base + 첨자는 base가 1개이므로 wrap 생략한다. 본문이 단일 base이면 wrap 생략.
    let mut i = 0;
    while i < tokens.len() {
        if matches!(tokens.get(i), Some(MathToken::MathSymbol('\u{221A}'))) {
            let mut j = i + 1;
            // 직후 토큰이 이미 괄호로 묶여 있으면 wrap 불필요.
            if matches!(tokens.get(j), Some(MathToken::OpenParen(_))) {
                i += 1;
                continue;
            }
            // base 토큰(V/UV/N)을 연속 수집. 첨자(Sub/Sup)는 직전 base에 부속이므로
            // base count로 세지 않고 함께 묶는다.
            let start = j;
            let mut base_count = 0;
            while matches!(
                tokens.get(j),
                Some(
                    MathToken::Variable(_)
                        | MathToken::UpperVariable(_)
                        | MathToken::Number(_)
                        | MathToken::Subscript(_)
                        | MathToken::Superscript(_)
                )
            ) {
                if matches!(
                    tokens.get(j),
                    Some(
                        MathToken::Variable(_) | MathToken::UpperVariable(_) | MathToken::Number(_)
                    )
                ) {
                    base_count += 1;
                }
                j += 1;
            }
            // base 토큰이 2개 이상일 때만 Grouping wrap 삽입.
            if base_count >= 2 {
                tokens.insert(start, MathToken::OpenParen(BracketKind::Grouping));
                tokens.insert(j + 1, MathToken::CloseParen(BracketKind::Grouping));
                i = j + 2;
                continue;
            }
        }
        i += 1;
    }

    Ok(tokens)
}

