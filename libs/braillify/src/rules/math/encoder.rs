//! Math expression → braille byte encoding.
//!
//! Converts parsed math tokens into braille byte sequences
//! following the 2024 Korean Math Braille Standard.

use super::math_token_rule::{MathEncodeState, MathTokenEngine, MathTokenResult, MathTokenRule};
use super::parser::{BracketKind, MathToken};
use super::{
    rule_1, rule_2, rule_3, rule_4, rule_5, rule_6, rule_7, rule_8, rule_9, rule_10, rule_11,
    rule_12, rule_13, rule_14, rule_15, rule_16, rule_17, rule_18, rule_19, rule_20, rule_21,
    rule_22, rule_23, rule_24, rule_25, rule_26, rule_27, rule_28, rule_29, rule_30, rule_31,
    rule_32, rule_33, rule_36, rule_37, rule_38, rule_39, rule_40, rule_41, rule_42, rule_43,
    rule_44, rule_47, rule_50, rule_52, rule_53, rule_54, rule_55, rule_56, rule_57, rule_58,
    rule_59, rule_60, rule_61, rule_65,
};
use crate::math_symbol_shortcut;

struct DigitSeparatorRule;

fn encode_generic_math_symbol(
    c: char,
    _is_direct_shortcut_symbol: bool,
    result: &mut Vec<u8>,
) -> Result<(), String> {
    let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(c)?;
    result.extend_from_slice(encoded);
    Ok(())
}

impl MathTokenRule for DigitSeparatorRule {
    fn name(&self) -> &'static str {
        "DigitSeparatorRule"
    }

    fn priority(&self) -> u16 {
        50
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::DigitSeparator))
    }

    fn apply(
        &self,
        _tokens: &[MathToken],
        _index: usize,
        result: &mut Vec<u8>,
        _state: &mut MathEncodeState,
        _engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        result.push(2);
        Ok(MathTokenResult::Consumed(1))
    }
}

struct SpaceRule;

fn prev_non_space(tokens: &[MathToken], index: usize) -> Option<&MathToken> {
    tokens[..index]
        .iter()
        .rev()
        .find(|token| !matches!(token, MathToken::Space))
}

fn next_non_space(tokens: &[MathToken], index: usize) -> Option<&MathToken> {
    tokens[index + 1..]
        .iter()
        .find(|token| !matches!(token, MathToken::Space))
}

fn prev_non_space_index(tokens: &[MathToken], index: usize) -> Option<usize> {
    (0..index)
        .rev()
        .find(|&i| !matches!(tokens.get(i), Some(MathToken::Space)))
}

fn next_non_space_index(tokens: &[MathToken], index: usize) -> Option<usize> {
    (index + 1..tokens.len()).find(|&i| !matches!(tokens.get(i), Some(MathToken::Space)))
}

fn is_glue_operator(token: Option<&MathToken>) -> bool {
    matches!(
        token,
        Some(MathToken::Operator('+' | '-' | '×' | '=' | '/'))
    )
}

fn should_suppress_space(tokens: &[MathToken], index: usize) -> bool {
    let prev_idx = prev_non_space_index(tokens, index);
    let next_idx = next_non_space_index(tokens, index);

    if prev_idx.is_some_and(|i| should_suppress_after_operator(tokens, i))
        || next_idx.is_some_and(|i| should_suppress_before_operator(tokens, i))
    {
        return true;
    }

    // PDF — `=`(또는 글루 연산자) 한쪽에 그룹 피연산자(괄호/한국어 wrap/√)가 인접하면
    // 반대쪽 공백도 제거한다. 예: `f = (...)` → `⠋⠒⠒⠦`. 입력 공백을 그대로 두면
    // PDF 점역 결과와 어긋난다.
    let operator_with_grouped_neighbor = |op_idx: usize| -> bool {
        if !is_glue_operator(tokens.get(op_idx)) {
            return false;
        }
        let lhs_grouped = prev_non_space_index(tokens, op_idx)
            .is_some_and(|i| token_is_grouped_operand(tokens, i));
        let rhs_grouped = next_non_space_index(tokens, op_idx)
            .is_some_and(|i| token_is_grouped_operand(tokens, i));
        lhs_grouped || rhs_grouped
    };
    prev_idx.is_some_and(operator_with_grouped_neighbor)
        || next_idx.is_some_and(operator_with_grouped_neighbor)
}

impl MathTokenRule for SpaceRule {
    fn name(&self) -> &'static str {
        "SpaceRule"
    }

    fn priority(&self) -> u16 {
        50
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::Space))
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        _engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        if !should_suppress_space(tokens, index) {
            result.push(0);
        }
        state.prev_was_number = false;
        Ok(MathTokenResult::Consumed(1))
    }
}

struct KoreanWordRule;

impl KoreanWordRule {
    /// 토큰이 Curly 컨텍스트(`{...}`) 내부에 있는지 확인한다.
    /// set-builder notation `{x|x는 정수}`의 Korean 본문은 wrap 없이 직접 emit.
    fn is_inside_curly(tokens: &[MathToken], index: usize) -> bool {
        let mut depth: i32 = 0;
        for i in 0..index {
            match tokens.get(i) {
                Some(MathToken::OpenParen(BracketKind::Curly)) => depth += 1,
                Some(MathToken::CloseParen(BracketKind::Curly)) => depth -= 1,
                _ => {}
            }
        }
        depth > 0
    }

    fn wrap_kind(tokens: &[MathToken], index: usize) -> Option<BracketKind> {
        let prev = prev_non_space(tokens, index);
        let next = next_non_space(tokens, index);
        let Some(MathToken::KoreanWord(text)) = tokens.get(index) else {
            return None;
        };

        if matches!(prev, Some(MathToken::OpenParen(BracketKind::Hangul)))
            || matches!(next, Some(MathToken::CloseParen(BracketKind::Hangul)))
        {
            return None;
        }

        // PDF — 이미 괄호 토큰으로 둘러싸여 있으면 추가 wrap 불필요.
        // 예: `(원의 둘레)` → BracketRule이 `⠦...⠴`를 그리므로 KoreanWordRule은 본문만 emit.
        if matches!(prev, Some(MathToken::OpenParen(_)))
            && matches!(next, Some(MathToken::CloseParen(_)))
        {
            return None;
        }

        // PDF 제60항 2-나 — set-builder notation `{x|x는 정수}` 내부 Korean은
        // wrap 없이 직접 emit한다 (math 변수가 ⠴...⠲로 quote 처리되므로 Korean은 bare).
        if Self::is_inside_curly(tokens, index) {
            return None;
        }

        if matches!(prev, Some(MathToken::MathSymbol('\u{221A}'))) {
            return Some(BracketKind::Hangul);
        }

        if text.contains(' ')
            || matches!(prev, Some(MathToken::Operator('×')))
            || matches!(next, Some(MathToken::Operator('×')))
        {
            return Some(BracketKind::MathParen);
        }

        None
    }
}

fn token_is_grouped_operand(tokens: &[MathToken], index: usize) -> bool {
    match tokens.get(index) {
        Some(MathToken::OpenParen(_) | MathToken::CloseParen(_)) => true,
        Some(MathToken::KoreanWord(_)) => KoreanWordRule::wrap_kind(tokens, index).is_some(),
        Some(MathToken::MathSymbol('\u{221A}')) => true,
        // PDF — Subscript/Superscript는 변수와 결합된 단일 점역 단위로, 인접한 산술 연산자의
        // 공백 처리에 있어 그룹 피연산자처럼 동작한다.
        Some(MathToken::Subscript(_) | MathToken::Superscript(_)) => true,
        _ => false,
    }
}

fn is_plain_unwrapped_korean(tokens: &[MathToken], index: usize) -> bool {
    matches!(tokens.get(index), Some(MathToken::KoreanWord(_)))
        && KoreanWordRule::wrap_kind(tokens, index).is_none()
}

fn is_mixed_times_context(tokens: &[MathToken], index: usize) -> bool {
    let Some(MathToken::Operator('×')) = tokens.get(index) else {
        return false;
    };

    let prev_idx = prev_non_space_index(tokens, index);
    let next_idx = next_non_space_index(tokens, index);
    let plain_korean_both_sides = prev_idx.is_some_and(|i| is_plain_unwrapped_korean(tokens, i))
        && next_idx.is_some_and(|i| is_plain_unwrapped_korean(tokens, i));

    if plain_korean_both_sides {
        return false;
    }

    tokens.iter().enumerate().any(|(i, token)| {
        matches!(token, MathToken::KoreanWord(_)) && KoreanWordRule::wrap_kind(tokens, i).is_some()
    })
}

fn should_suppress_before_operator(tokens: &[MathToken], index: usize) -> bool {
    let Some(MathToken::Operator(op)) = tokens.get(index) else {
        return false;
    };

    if *op == '×' {
        return is_mixed_times_context(tokens, index);
    }

    if !is_glue_operator(tokens.get(index)) {
        return false;
    }

    prev_non_space_index(tokens, index).is_some_and(|i| token_is_grouped_operand(tokens, i))
}

fn should_suppress_after_operator(tokens: &[MathToken], index: usize) -> bool {
    let Some(MathToken::Operator(op)) = tokens.get(index) else {
        return false;
    };

    if *op == '×' {
        return is_mixed_times_context(tokens, index);
    }

    if !is_glue_operator(tokens.get(index)) {
        return false;
    }

    next_non_space_index(tokens, index).is_some_and(|i| token_is_grouped_operand(tokens, i))
}

impl MathTokenRule for KoreanWordRule {
    fn name(&self) -> &'static str {
        "KoreanWordRule"
    }

    fn priority(&self) -> u16 {
        50
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::KoreanWord(_)))
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        _engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        let Some(MathToken::KoreanWord(text)) = tokens.get(index) else {
            return Ok(MathTokenResult::Skip);
        };

        if let Some(kind) = Self::wrap_kind(tokens, index) {
            rule_6::encode_open_paren(kind, result);
            result.extend(crate::encode(text)?);
            rule_6::encode_close_paren(kind, result);
        } else {
            result.extend(crate::encode(text)?);
        }

        state.prev_was_number = false;
        Ok(MathTokenResult::Consumed(1))
    }
}

struct MathSymbolRule;

impl MathSymbolRule {
    fn next_non_space(tokens: &[MathToken], mut idx: usize) -> Option<&MathToken> {
        while let Some(token) = tokens.get(idx) {
            if !matches!(token, MathToken::Space) {
                return Some(token);
            }
            idx += 1;
        }
        None
    }
}

impl MathTokenRule for MathSymbolRule {
    fn name(&self) -> &'static str {
        "MathSymbolRule"
    }

    fn priority(&self) -> u16 {
        100
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::MathSymbol(_)))
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        let Some(MathToken::MathSymbol(c)) = tokens.get(index) else {
            return Ok(MathTokenResult::Skip);
        };

        let _ = rule_26::is_reserved_rule_26();
        let _ = rule_22::NTH_ROOT_INDEX_MARKER;

        if *c == '\u{00AC}'
            && index > 0
            && matches!(
                rule_12::prev_non_space(tokens, index),
                Some(MathToken::Variable(_) | MathToken::UpperVariable(_))
            )
            && matches!(
                Self::next_non_space(tokens, index + 1),
                Some(MathToken::UpperVariable(_))
            )
        {
            result.push(40);
            state.prev_was_number = false;
            return Ok(MathTokenResult::Consumed(1));
        }

        if *c == '\u{FF03}'
            && matches!(
                Self::next_non_space(tokens, index + 1),
                Some(MathToken::UpperVariable(_))
            )
        {
            let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(*c)?;
            result.extend_from_slice(encoded);
            result.push(38);
            let mut i = index + 1;
            while matches!(tokens.get(i), Some(MathToken::Space)) {
                i += 1;
            }
            if let Some(MathToken::UpperVariable(upper)) = tokens.get(i) {
                result.push(32);
                result.push(crate::english::encode_english(upper.to_ascii_lowercase())?);
                i += 1;
            }
            result.push(52);
            state.prev_was_number = false;
            return Ok(MathTokenResult::Consumed(i - index));
        }

        // PDF 수학 제65항 1 — `＃(UpperVar)` 패턴: 기수 표기.
        // `＃` + `(` + UpperVariable + `)` 형태를 `⠸⠹⠦⠠letter⠴`로 emit.
        if *c == '\u{FF03}'
            && matches!(
                Self::next_non_space(tokens, index + 1),
                Some(MathToken::OpenParen(_))
            )
        {
            // ＃ 다음 ( 다음 UpperVariable 다음 ) 패턴 확인
            let mut i = index + 1;
            while matches!(tokens.get(i), Some(MathToken::Space)) {
                i += 1;
            }
            // OpenParen
            if !matches!(tokens.get(i), Some(MathToken::OpenParen(_))) {
                // fall through to default handling
            } else {
                let open_idx = i;
                i += 1;
                while matches!(tokens.get(i), Some(MathToken::Space)) {
                    i += 1;
                }
                if let Some(MathToken::UpperVariable(upper)) = tokens.get(i) {
                    let upper_char = *upper;
                    i += 1;
                    while matches!(tokens.get(i), Some(MathToken::Space)) {
                        i += 1;
                    }
                    if matches!(tokens.get(i), Some(MathToken::CloseParen(_))) {
                        // 패턴 매칭 성공: ⠸⠹⠦⠠X⠴
                        let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(*c)?;
                        result.extend_from_slice(encoded);
                        result.push(38); // ⠦ (MathParen open)
                        result.push(32); // ⠠ (capital marker)
                        result.push(crate::english::encode_english(
                            upper_char.to_ascii_lowercase(),
                        )?);
                        result.push(52); // ⠴ (MathParen close)
                        state.prev_was_number = false;
                        let consumed = i + 1 - index;
                        let _ = open_idx;
                        return Ok(MathTokenResult::Consumed(consumed));
                    }
                }
            }
        }

        // PDF 수학 제61항 — 한정자(∀/∃) + 변수 형태의 식에서, 한정자-변수 다음
        // 또 다른 식(변수/괄호/함수)이 이어지면 한 칸을 띄어 쓴다.
        // 예: `∀x p(x)` → ⠨⠄⠭⠀⠏⠦⠭⠴
        if matches!(*c, '\u{2200}' | '\u{2203}')
            && matches!(
                tokens.get(index + 1),
                Some(MathToken::Variable(_) | MathToken::UpperVariable(_))
            )
        {
            let after_var = index + 2;
            let needs_space = matches!(
                tokens.get(after_var),
                Some(
                    MathToken::Variable(_)
                        | MathToken::UpperVariable(_)
                        | MathToken::Number(_)
                        | MathToken::OpenParen(_)
                        | MathToken::FunctionName(_)
                        | MathToken::MathSymbol(_)
                )
            );
            if needs_space {
                let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(*c)?;
                result.extend_from_slice(encoded);
                if let Some(MathToken::Variable(v)) = tokens.get(index + 1) {
                    result.push(crate::english::encode_english(*v)?);
                } else if let Some(MathToken::UpperVariable(v)) = tokens.get(index + 1) {
                    result.push(32);
                    result.push(crate::english::encode_english(v.to_ascii_lowercase())?);
                }
                result.push(0); // PDF 제61항 ∀x/∃x 다음 한 칸 띄움
                state.prev_was_number = false;
                return Ok(MathTokenResult::Consumed(2));
            }
        }

        if rule_25::is_sigma_symbol(*c)
            && matches!(tokens.get(index + 1), Some(MathToken::OpenParen(_)))
        {
            let Some(close_idx) = rule_6::find_matching_paren(tokens, index + 1) else {
                return Err("Unmatched parenthesis in sigma bounds".to_string());
            };
            rule_25::encode_sigma_with_bounds(&[], &[], result)?;
            result.push(48);

            let normalized_inner: Vec<MathToken> = tokens[index + 2..close_idx]
                .iter()
                .map(|token| {
                    if matches!(token, MathToken::Operator(',')) {
                        MathToken::Space
                    } else {
                        token.clone()
                    }
                })
                .collect();

            let has_bound_separators = tokens[index + 2..close_idx]
                .iter()
                .any(|token| matches!(token, MathToken::Operator('=' | ',')));

            if has_bound_separators {
                engine.encode_tokens(&normalized_inner, result)?;
            } else {
                result.pop();
                result.push(55);
                engine.encode_tokens(&normalized_inner, result)?;
                result.push(62);
            }

            if !matches!(tokens.get(close_idx + 1), Some(MathToken::Space) | None) {
                result.push(0);
            }

            state.prev_was_number = false;
            return Ok(MathTokenResult::Consumed(close_idx + 1 - index));
        }

        if *c == '\u{03A0}'
            && matches!(
                tokens.get(index + 1),
                Some(MathToken::OpenParen(BracketKind::MathParen))
            )
            && matches!(tokens.get(index + 2), Some(MathToken::Number(_)))
            && matches!(tokens.get(index + 3), Some(MathToken::Operator(',')))
            && matches!(tokens.get(index + 4), Some(MathToken::Number(_)))
            && matches!(
                tokens.get(index + 5),
                Some(MathToken::CloseParen(BracketKind::MathParen))
            )
        {
            let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(*c)?;
            result.extend_from_slice(encoded);
            result.push(55);
            if let Some(MathToken::Number(left)) = tokens.get(index + 2) {
                rule_1::encode_number_literal(left, result);
            }
            result.push(0);
            if let Some(MathToken::Number(right)) = tokens.get(index + 4) {
                rule_1::encode_number_literal(right, result);
            }
            result.push(62);
            state.prev_was_number = false;
            return Ok(MathTokenResult::Consumed(6));
        }

        // In derivative/product formulas (제53항), middle dot is used as
        // multiplication sign when the same expression also contains
        // arithmetic composition (= or +).
        if *c == '\u{00B7}'
            && tokens
                .iter()
                .any(|t| matches!(t, MathToken::Operator('=' | '+')))
        {
            rule_2::encode_operator('\u{00D7}', tokens, index, result)?;
            state.prev_was_number = false;
            return Ok(MathTokenResult::Consumed(1));
        }

        let should_pad = rule_2::needs_binary_spacing(*c)
            && index > 0
            && rule_2::is_algebraic_neighbor(rule_12::prev_non_space(tokens, index))
            && (rule_2::is_algebraic_neighbor(Self::next_non_space(tokens, index + 1))
                || matches!(
                    Self::next_non_space(tokens, index + 1),
                    Some(MathToken::MathSymbol('\u{00AC}'))
                ));

        // PDF 수학 제65항 2~3 — ∴/∵는 앞뒤 두 칸씩 띄어 쓴다.
        // 입력에 Space 토큰이 있으면 +1, 없으면 +2 출력해 합계 2를 맞춘다.
        if matches!(*c, '\u{2234}' | '\u{2235}') {
            let prev_is_space =
                matches!(tokens.get(index.saturating_sub(1)), Some(MathToken::Space));
            // Avoid duplicate padding when previous token has already emitted spacing.
            let prev_emits_trailing_space = matches!(
                tokens.get(index.saturating_sub(1)),
                Some(MathToken::Operator(_))
            );
            if !prev_emits_trailing_space {
                if prev_is_space {
                    result.push(0);
                } else if index > 0 {
                    result.push(0);
                    result.push(0);
                }
            }
        } else if should_pad && !matches!(tokens.get(index - 1), Some(MathToken::Space)) {
            // PDF — `\xrightarrow{f}` 같이 라벨 직후 화살표는 공백 없이 인접한다.
            // 라벨 컨텍스트 조건: 화살표이고, 직전이 Variable/UpperVariable이며,
            // 그 직전이 Space (즉, V가 라벨 단독 위치). 일반 `X→Y`는 V 직전이 Space가
            // 아니므로 padding이 유지된다.
            let is_horizontal_arrow = matches!(
                *c,
                '\u{2192}' | '\u{2190}' | '\u{2194}' | '\u{21C4}' | '\u{21CC}'
            );
            let prev_is_label = matches!(
                tokens.get(index - 1),
                Some(MathToken::Variable(_) | MathToken::UpperVariable(_))
            ) && (index >= 2
                && matches!(tokens.get(index - 2), Some(MathToken::Space)));
            if !(is_horizontal_arrow && prev_is_label) {
                result.push(0);
            }
        }

        if rule_3::is_equality_symbol(*c) {
            rule_3::encode_equality_symbol(*c, result)?;
        } else if rule_4::is_comparison_symbol(*c) {
            rule_4::encode_comparison_symbol(*c, result)?;
        } else if rule_5::is_proportion_symbol(*c) {
            rule_5::encode_proportion_symbol(*c, result)?;
        } else if rule_37::is_double_arrow_line_symbol(*c) {
            rule_37::encode_double_arrow_line_symbol(*c, result)?;
        } else if rule_38::is_right_arrow_ray_symbol(*c) {
            rule_38::encode_right_arrow_ray_symbol(*c, result)?;
        } else if rule_10::is_arrow_symbol(*c) {
            rule_10::encode_arrow_symbol(*c, result)?;
        } else if rule_13::is_greek_symbol(*c) {
            rule_13::encode_greek_symbol(*c, result)?;
        } else if rule_15::is_custom_binary_operator(*c) {
            rule_15::encode_custom_binary_operator(*c, result)?;
        } else if rule_17::is_prime_mark(*c) {
            rule_17::encode_prime(*c, result)?;
        } else if rule_20::is_approximation_symbol(*c) {
            rule_20::encode_approximation_symbol(*c, result)?;
        } else if rule_21::is_absolute_value_bar(*c) {
            if matches!(
                rule_12::prev_non_space(tokens, index),
                Some(MathToken::Operator(_))
            ) || index == 0
            {
                rule_21::encode_absolute_value_open(result)?;
            } else {
                rule_21::encode_absolute_value_close(result)?;
            }
        } else if rule_23::is_overline_mark(*c) {
            rule_23::encode_overline(result)?;
        } else if rule_24::is_sequence_brace(*c) {
            rule_24::encode_sequence_brace(*c, result)?;
        } else if rule_27::is_divisibility_symbol(*c) {
            if *c == '|' {
                rule_27::encode_divisibility(*c, result)?;
            } else {
                let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(*c)?;
                result.extend_from_slice(encoded);
            }
        } else if rule_28::is_norm_symbol(*c) {
            if index == 0 {
                rule_28::encode_norm_open(result)?;
            } else if index + 1 >= tokens.len() {
                rule_28::encode_norm_close(result)?;
            } else {
                rule_28::encode_norm_symbol(*c, result)?;
            }
        } else if rule_29::is_approximate_equal(*c) {
            rule_29::encode_approximate_equal(*c, result)?;
        } else if rule_30::is_dot_congruence(*c) {
            rule_30::encode_dot_congruence(*c, result)?;
        } else if rule_31::is_asymptotic_equal(*c) {
            rule_31::encode_asymptotic_equal(*c, result)?;
        } else if rule_32::is_congruence_symbol(*c) {
            rule_32::encode_congruence_symbol(*c, result)?;
        } else if rule_33::is_geometric_operator(*c) {
            rule_33::encode_geometric_operator(*c, result)?;
        } else if rule_36::is_arc_symbol(*c) {
            rule_36::encode_arc(*c, result)?;
        } else if rule_39::is_angle_symbol(*c) {
            rule_39::encode_angle_symbol(*c, result)?;
        } else if rule_40::is_geometric_shape(*c) {
            rule_40::encode_geometric_shape(*c, result)?;
        } else if rule_41::is_perpendicular_symbol(*c) {
            rule_41::encode_perpendicular(*c, result)?;
        } else if rule_42::is_similarity_symbol(*c) {
            rule_42::encode_similarity_symbol(*c, result)?;
        } else if rule_43::is_identity_symbol(*c) {
            rule_43::encode_identity_symbol(*c, result)?;
        } else if rule_44::is_parallel_symbol(*c) {
            rule_44::encode_parallel_symbol(*c, result)?;
        } else if rule_50::is_special_constant(*c) {
            rule_50::encode_special_constant(*c, result)?;
        } else if rule_52::is_delta_symbol(*c) {
            rule_52::encode_delta_symbol(*c, result)?;
        } else if rule_54::is_partial_derivative(*c) {
            rule_54::encode_partial_derivative(*c, result)?;
        } else if rule_55::is_nabla_symbol(*c) {
            rule_55::encode_nabla_symbol(*c, result)?;
        } else if rule_56::is_integral_symbol(*c) {
            rule_56::encode_integral_symbol(*c, result)?;
        } else if rule_58::is_double_integral(*c) {
            rule_58::encode_double_integral(*c, result)?;
        } else if rule_59::is_contour_integral(*c) {
            rule_59::encode_contour_integral(*c, result)?;
        } else if rule_65::is_therefore_because(*c) {
            rule_65::encode_therefore_because(*c, result)?;
        } else if *c == '\u{0307}'
            && matches!(
                rule_12::prev_non_space(tokens, index),
                Some(MathToken::Variable(_) | MathToken::UpperVariable(_))
            )
        {
            // PDF 수학 제65항 5 — 문자 뒤 결합 윗 한 점 (ȧ 등). 숫자 뒤 순환소수와 구분.
            result.push(crate::unicode::decode_unicode('⠈'));
            result.push(crate::unicode::decode_unicode('⠲'));
        } else {
            let is_direct_shortcut_symbol = rule_11::is_math_sentence_delimiter(*c)
                || rule_16::is_base_notation_subscript(*c)
                || rule_22::is_root_symbol(*c)
                || rule_60::is_set_symbol(*c)
                || rule_61::is_logic_symbol(*c)
                || super::rule_64::is_hat_notation(*c);
            encode_generic_math_symbol(*c, is_direct_shortcut_symbol, result)?;
        }

        if matches!(*c, '\u{2234}' | '\u{2235}') {
            let next_is_space = matches!(tokens.get(index + 1), Some(MathToken::Space));
            let next_emits_leading_space =
                matches!(tokens.get(index + 1), Some(MathToken::Operator(_)));
            if !next_emits_leading_space {
                if next_is_space {
                    result.push(0);
                } else if index + 1 < tokens.len() {
                    result.push(0);
                    result.push(0);
                }
            }
        } else if should_pad && !matches!(tokens.get(index + 1), Some(MathToken::Space)) {
            // PDF — `\xrightleftharpoons[g]{f}` 같이 화살표 뒤 below 라벨도 공백 없이 인접.
            // 라벨 컨텍스트 조건: 화살표이고, 직후가 Variable/UpperVariable이며,
            // 그 직후가 Space (즉, V가 below 라벨 단독 위치).
            let is_horizontal_arrow = matches!(
                *c,
                '\u{2192}' | '\u{2190}' | '\u{2194}' | '\u{21C4}' | '\u{21CC}'
            );
            let next_is_label = matches!(
                tokens.get(index + 1),
                Some(MathToken::Variable(_) | MathToken::UpperVariable(_))
            ) && matches!(tokens.get(index + 2), Some(MathToken::Space));
            if !(is_horizontal_arrow && next_is_label) {
                result.push(0);
            }
        }

        state.prev_was_number = rule_9::is_repeating_decimal_mark(*c);
        Ok(MathTokenResult::Consumed(1))
    }
}

struct RawTokenRule;

impl MathTokenRule for RawTokenRule {
    fn name(&self) -> &'static str {
        "RawTokenRule"
    }

    fn priority(&self) -> u16 {
        500
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::Raw(_)))
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        _state: &mut MathEncodeState,
        _engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        let Some(MathToken::Raw(c)) = tokens.get(index) else {
            return Ok(MathTokenResult::Skip);
        };
        // PDF — 수학 컨텍스트 내 일반 구두점 중 PDF 65항 등에서 정의된 것만 처리한다.
        // 무차별 fallback은 다른 컨텍스트(예: 인용 부호)와 충돌하므로 명시적 매핑으로 한정.
        if matches!(*c, ':' | ';' | '?' | '!')
            && let Ok(encoded) = crate::symbol_shortcut::encode_char_symbol_shortcut(*c)
        {
            result.extend_from_slice(encoded);
            return Ok(MathTokenResult::Consumed(1));
        }
        Err(format!("Unrecognized math character: '{}'", c))
    }
}

fn build_math_engine() -> MathTokenEngine {
    let mut engine = MathTokenEngine::new();

    // Priority 10 — lookahead rules
    engine.register(Box::new(rule_7::ConditionalProbFractionRule));
    engine.register(Box::new(rule_7::GroupedFractionReversalRule));
    engine.register(Box::new(rule_7::FractionReversalRule));
    engine.register(Box::new(rule_7::VariableFractionInListRule));
    engine.register(Box::new(rule_12::CombinatoricsRule));
    engine.register(Box::new(rule_54::PartialDerivativeFractionRule));
    engine.register(Box::new(rule_57::DefiniteIntegralRule));

    // Priority 50 — core token rules
    engine.register(Box::new(rule_1::NumberRule));
    engine.register(Box::new(rule_12::VariableRule));
    engine.register(Box::new(rule_12::UpperVariableRule));
    engine.register(Box::new(KoreanWordRule));
    engine.register(Box::new(rule_2::OperatorRule));
    engine.register(Box::new(rule_47::FunctionNameRule));
    engine.register(Box::new(rule_6::BracketRule));
    engine.register(Box::new(rule_18::SuperscriptRule));
    engine.register(Box::new(rule_19::SubscriptRule));
    engine.register(Box::new(rule_8::DecimalPointRule));
    engine.register(Box::new(DigitSeparatorRule));
    engine.register(Box::new(SpaceRule));
    engine.register(Box::new(rule_53::PrimeRule));

    // Priority 100 — math symbol dispatch
    engine.register(Box::new(MathSymbolRule));
    engine.register(Box::new(RawTokenRule));

    engine.finalize();
    engine
}

/// Encode a full math expression string into braille bytes.
pub fn encode_math_expression(input: &str) -> Result<Vec<u8>, String> {
    if rule_14::is_roman_numeral_expression(input) {
        return rule_14::encode_roman_numeral_expression(input);
    }

    let tokens = super::parser::parse_math_expression(input)?;
    let engine = build_math_engine();
    let mut result = Vec::new();
    engine.encode_tokens(&tokens, &mut result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_equation() {
        // ax+b=0 → internal "ax5b33#j"
        // a=1, x=45, 5(+)=34, b=3, 33(=)=18,18, #j=60,26
        let result = encode_math_expression("ax+b=0");
        assert!(result.is_ok(), "Should encode ax+b=0: {:?}", result);
    }

    #[test]
    fn test_number_encoding() {
        // Pure number should get # prefix
        let result = encode_math_expression("37+25").unwrap();
        // #cg5#be = 60,9,27,34,60,3,17
        assert!(!result.is_empty());
    }
}
