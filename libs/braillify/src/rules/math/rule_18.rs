//! 수학 제18항 — 위첨자 표기.
//!
//! 위첨자 지시(^)와 다중 토큰 묶음, 지수 특수형을 처리한다.

use crate::rules::math::parser::{BracketKind, MathToken};

use super::math_token_rule::{MathEncodeState, MathTokenEngine, MathTokenResult, MathTokenRule};
use super::rule_1;

fn prev_non_space(tokens: &[MathToken], mut idx: usize) -> Option<&MathToken> {
    while idx > 0 {
        idx -= 1;
        let t = tokens.get(idx)?;
        if !matches!(t, MathToken::Space) {
            return Some(t);
        }
    }
    None
}

fn next_non_space(tokens: &[MathToken], mut idx: usize) -> Option<&MathToken> {
    loop {
        idx += 1;
        let t = tokens.get(idx)?;
        if !matches!(t, MathToken::Space) {
            return Some(t);
        }
    }
}

/// PDF 수학 제18항 2 — 좌상첨자: 위첨자가 변수 앞에 단독 위치할 때.
/// 앞에 피첨자(변수/숫자/괄호닫기)가 없고 뒤에 변수가 이어지면 좌상첨자다.
/// 단, 합/적분/극한 등 한정자 뒤의 첨자(예: ∑_{k=0}^{∞} 의 ^∞)는 좌상첨자가 아니다.
fn is_left_superscript_position(tokens: &[MathToken], index: usize) -> bool {
    let prev_blocks = matches!(
        prev_non_space(tokens, index),
        Some(MathToken::Variable(_))
            | Some(MathToken::UpperVariable(_))
            | Some(MathToken::Number(_))
            | Some(MathToken::CloseParen(_))
            | Some(MathToken::Prime)
            | Some(MathToken::FunctionName(_))
            // Subscript 뒤의 Superscript는 같은 base에 붙는 위첨자 (좌상첨자 아님)
            | Some(MathToken::Subscript(_))
    );
    if prev_blocks {
        return false;
    }
    // PDF — 알파벳적 수학 기호(∂ ∇ ℏ 등)는 피첨자로 동작한다.
    // `∂²z`의 `²`는 ∂의 위첨자이지 z의 좌상첨자가 아니다.
    if let Some(MathToken::MathSymbol('\u{2202}' | '\u{2207}' | '\u{210F}' | '\u{2135}')) =
        prev_non_space(tokens, index)
    {
        return false;
    }
    // 한정자(∫/∑/Π 등) 토큰을 좌측 두번째에서 발견하면 좌상첨자가 아님.
    let mut i = index;
    while i > 0 {
        i -= 1;
        match tokens.get(i) {
            Some(MathToken::Space | MathToken::Subscript(_)) => continue,
            Some(MathToken::MathSymbol(
                '\u{222B}' | '\u{222C}' | '\u{222D}' | '\u{222E}' | '\u{2211}' | '\u{220F}'
                | '\u{2200}' | '\u{2203}',
            )) => {
                return false;
            }
            Some(MathToken::FunctionName(_)) => return false,
            _ => break,
        }
    }
    matches!(
        next_non_space(tokens, index),
        Some(MathToken::Variable(_)) | Some(MathToken::UpperVariable(_))
    )
}

fn is_simple_signed_number(content: &[MathToken]) -> bool {
    if content.len() != 2 {
        return false;
    }
    // 부호: ASCII `-` 또는 수학 마이너스 `\u{2212}`. 둘 다 첨자에서 단순 부호로 본다.
    let is_minus = matches!(
        content[0],
        MathToken::Operator('\u{2212}') | MathToken::Operator('-')
    );
    // 부호 뒤 단일 숫자 또는 단일 변수. 예: `e^{-x}`, `x^{-1}`.
    let is_simple_term = matches!(content[1], MathToken::Number(_) | MathToken::Variable(_));
    is_minus && is_simple_term
}

pub fn should_group_superscript(content: &[MathToken]) -> bool {
    if content.len() <= 1 {
        return false;
    }
    if is_simple_signed_number(content) {
        return false;
    }
    // PDF — 위첨자 본문이 여러 토큰을 포함(연산자/괄호/공백/첨자 등)하면 그룹으로 묶는다.
    // `^{ℵ_0}` 같이 MathSymbol+Subscript 조합도 그룹 대상이다.
    content.iter().any(|token| {
        matches!(
            token,
            MathToken::Operator(_)
                | MathToken::OpenParen(_)
                | MathToken::CloseParen(_)
                | MathToken::Space
                | MathToken::Subscript(_)
                | MathToken::Superscript(_)
        )
    }) || content.len() >= 3
}

pub fn encode_superscript(
    tokens: &[MathToken],
    i: &mut usize,
    content: &[MathToken],
    result: &mut Vec<u8>,
    engine: &MathTokenEngine,
) -> Result<bool, String> {
    if *i >= 2
        && matches!(tokens.get(*i - 1), Some(MathToken::Subscript(_)))
        && matches!(
            tokens.get(*i - 2),
            Some(MathToken::MathSymbol(
                '\u{222B}' | '\u{222C}' | '\u{222D}' | '\u{222E}' | '\u{2211}' | '\u{220F}'
            ))
        )
    {
        result.push(0);
        engine.encode_tokens(content, result)?;
        // 다음 토큰이 Space면 이미 한 칸 띄움이 보장되므로 중복 출력하지 않는다.
        if !matches!(tokens.get(*i + 1), Some(MathToken::Space) | None) {
            result.push(0);
        }
        *i += 1;
        return Ok(true);
    }

    if *i >= 2
        && matches!(tokens.get(*i - 1), Some(MathToken::Subscript(_)))
        && matches!(
            tokens.get(*i - 2),
            Some(MathToken::CloseParen(BracketKind::Square))
        )
    {
        result.push(0);
        engine.encode_tokens(content, result)?;
        if !matches!(tokens.get(*i + 1), Some(MathToken::Space) | None) {
            result.push(0);
        }
        *i += 1;
        return Ok(true);
    }

    if matches!(tokens.get(*i + 1), Some(MathToken::MathSymbol('\u{221A}'))) {
        if content.len() > 1 {
            result.push(55);
            engine.encode_tokens(content, result)?;
            result.push(62);
        } else {
            engine.encode_tokens(content, result)?;
        }
        result.push(59);
        *i += 2;
        return Ok(true);
    }

    if let [MathToken::Number(left)] = content
        && matches!(tokens.get(*i + 1), Some(MathToken::MathSymbol('\u{00B7}')))
        && let Some(MathToken::Superscript(right_content)) = tokens.get(*i + 2)
        && let [MathToken::Number(right)] = right_content.as_slice()
    {
        result.push(24);
        result.push(60);
        for ch in left.chars() {
            result.extend(crate::number::encode_number(ch));
        }
        result.push(50);
        for ch in right.chars() {
            result.extend(crate::number::encode_number(ch));
        }
        *i += 3;
        return Ok(true);
    }

    if let [MathToken::Number(left)] = content
        && matches!(tokens.get(*i + 1), Some(MathToken::Operator('/')))
        && let Some(MathToken::Superscript(right_content)) = tokens.get(*i + 2)
        && let [MathToken::Number(right)] = right_content.as_slice()
    {
        result.push(24);
        result.push(55);
        rule_1::encode_number_literal(left, result);
        result.push(12);
        rule_1::encode_number_literal(right, result);
        result.push(62);
        *i += 3;
        return Ok(true);
    }

    // PDF 수학 — 위첨자가 (단순한 단일 항목)을 괄호로 감싼 형태일 때는 도함수 차수 등
    // 인덱스 표기로 보고 MathParen(⠦⠴)을 그대로 보존한다(예: y⁽⁴⁾, y⁽ⁿ⁾).
    // 복합 식이 괄호 안에 있으면 PDF 위첨자 그룹 규칙대로 ⠷⠾로 묶고 외곽 괄호는 떼낸다.
    let wrapped_simple_index = content.len() == 3
        && matches!(
            (content.first(), content.get(1), content.last()),
            (
                Some(MathToken::OpenParen(BracketKind::MathParen)),
                Some(MathToken::Number(_) | MathToken::Variable(_) | MathToken::UpperVariable(_)),
                Some(MathToken::CloseParen(BracketKind::MathParen))
            )
        );

    let (sup_content, force_group) = if !wrapped_simple_index
        && content.len() >= 2
        && matches!(
            (content.first(), content.last()),
            (
                Some(MathToken::OpenParen(BracketKind::MathParen)),
                Some(MathToken::CloseParen(BracketKind::MathParen))
            )
        ) {
        (&content[1..content.len() - 1], true)
    } else {
        (content, false)
    };

    // PDF 수학 제18항 2 — 좌상첨자(left superscript): 변수 앞에 위치한 위첨자.
    // 좌상첨자는 단일 토큰이라도 그룹 괄호로 묶는다.
    let is_left_superscript = is_left_superscript_position(tokens, *i);

    result.push(24);
    if wrapped_simple_index {
        // 본문 그대로 emit하여 ⠦⠴(MathParen) 보존.
        engine.encode_tokens(content, result)?;
    } else if force_group || should_group_superscript(sup_content) || is_left_superscript {
        result.push(55);
        engine.encode_tokens(sup_content, result)?;
        result.push(62);
    } else {
        engine.encode_tokens(sup_content, result)?;
    }
    *i += 1;
    Ok(false)
}

pub struct SuperscriptRule;

impl MathTokenRule for SuperscriptRule {
    fn name(&self) -> &'static str {
        "SuperscriptRule"
    }

    fn priority(&self) -> u16 {
        50
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::Superscript(_)))
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        let Some(MathToken::Superscript(content)) = tokens.get(index) else {
            return Ok(MathTokenResult::Skip);
        };
        let mut cursor = index;
        let _ = encode_superscript(tokens, &mut cursor, content, result, engine)?;
        state.prev_was_number = false;
        Ok(MathTokenResult::Consumed(cursor - index))
    }
}
