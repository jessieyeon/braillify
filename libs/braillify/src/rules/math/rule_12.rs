//! 수학 제12항 — 로마자 변수 표기.
//!
//! 소문자/대문자 변수, 대문자 이어쓰기, 수-문자 연결형을 처리한다.

use crate::math_symbol_shortcut;
use crate::rules::math::parser::{BracketKind, MathToken};

use super::math_token_rule::{MathEncodeState, MathTokenEngine, MathTokenResult, MathTokenRule};
use super::rule_1;
use super::rule_6;

pub fn prev_non_space(tokens: &[MathToken], mut idx: usize) -> Option<&MathToken> {
    while idx > 0 {
        idx -= 1;
        let token = tokens.get(idx)?;
        if !matches!(token, MathToken::Space) {
            return Some(token);
        }
    }
    None
}

pub fn encode_variable(
    c: char,
    tokens: &[MathToken],
    i: &mut usize,
    prev_was_number: &mut bool,
    result: &mut Vec<u8>,
    engine: &MathTokenEngine,
) -> Result<bool, String> {
    if c == 'y'
        && matches!(tokens.get(*i + 1), Some(MathToken::Superscript(_)))
        && matches!(tokens.get(*i + 2), Some(MathToken::Operator('=')))
        && let Some(MathToken::Superscript(content)) = tokens.get(*i + 1)
    {
        result.push(crate::english::encode_english('y')?);
        result.push(24);
        result.push(38);
        engine.encode_tokens(content, result)?;
        result.push(52);
        *prev_was_number = false;
        *i += 2;
        return Ok(true);
    }

    if c == 'd'
        && matches!(
            tokens.get(i.saturating_sub(1)),
            Some(MathToken::Operator('='))
        )
        && matches!(tokens.get(*i + 1), Some(MathToken::Variable(_)))
        && matches!(tokens.get(*i + 2), Some(MathToken::Operator('/')))
        && matches!(tokens.get(*i + 3), Some(MathToken::Variable('d')))
        && matches!(tokens.get(*i + 4), Some(MathToken::Variable(_)))
        && let (Some(MathToken::Variable(num_var)), Some(MathToken::Variable(den_var))) =
            (tokens.get(*i + 1), tokens.get(*i + 4))
        && *num_var == 'y'
        && *den_var == 'x'
    {
        result.push(crate::english::encode_english('d')?);
        result.push(crate::english::encode_english(
            den_var.to_ascii_lowercase(),
        )?);
        result.push(12);
        result.push(crate::english::encode_english('d')?);
        result.push(crate::english::encode_english(
            num_var.to_ascii_lowercase(),
        )?);
        *prev_was_number = false;
        *i += 5;
        return Ok(true);
    }

    if c == 'd'
        && matches!(tokens.get(*i + 1), Some(MathToken::Superscript(_)))
        && matches!(tokens.get(*i + 2), Some(MathToken::Variable(_)))
        && matches!(tokens.get(*i + 3), Some(MathToken::Operator('/')))
        && let (Some(MathToken::Superscript(content)), Some(MathToken::Variable(v))) =
            (tokens.get(*i + 1), tokens.get(*i + 2))
    {
        result.push(crate::english::encode_english('d')?);
        result.push(crate::english::encode_english(v.to_ascii_lowercase())?);
        result.push(24);
        engine.encode_tokens(content, result)?;
        *prev_was_number = false;
        *i += 3;
        return Ok(true);
    }

    if *prev_was_number
        && *i == 1
        && matches!(
            tokens.get(*i + 1),
            Some(MathToken::Variable(_) | MathToken::UpperVariable(_))
        )
    {
        result.push(16);
    }
    result.push(crate::english::encode_english(c.to_ascii_lowercase())?);
    *prev_was_number = false;
    *i += 1;
    Ok(false)
}

pub fn encode_upper_variable(
    c: char,
    tokens: &[MathToken],
    i: &mut usize,
    prev_was_number: &mut bool,
    logic_context: bool,
    result: &mut Vec<u8>,
) -> Result<bool, String> {
    if matches!(
        tokens.get(*i + 1),
        Some(MathToken::OpenParen(BracketKind::MathParen))
    ) && matches!(tokens.get(*i + 2), Some(MathToken::Number(_)))
        && matches!(tokens.get(*i + 3), Some(MathToken::Operator(',')))
        && matches!(tokens.get(*i + 4), Some(MathToken::Number(_)))
        && matches!(
            tokens.get(*i + 5),
            Some(MathToken::CloseParen(BracketKind::MathParen))
        )
    {
        result.push(32);
        result.push(crate::english::encode_english(c.to_ascii_lowercase())?);
        result.push(55);
        if let Some(MathToken::Number(left)) = tokens.get(*i + 2) {
            rule_1::encode_number_literal(left, result);
        }
        result.push(0);
        if let Some(MathToken::Number(right)) = tokens.get(*i + 4) {
            rule_1::encode_number_literal(right, result);
        }
        result.push(62);
        *prev_was_number = false;
        *i += 6;
        return Ok(true);
    }

    if matches!(
        tokens.get(*i + 1),
        Some(MathToken::OpenParen(
            BracketKind::MathParen | BracketKind::Grouping
        ))
    ) && *i == 0
        && let Some(close_idx) = rule_6::find_matching_paren(tokens, *i + 1)
    {
        let inner = &tokens[*i + 2..close_idx];
        let simple_function_arg = !inner
            .iter()
            .any(|t| matches!(t, MathToken::UpperVariable(_) | MathToken::MathSymbol('|')));
        if simple_function_arg {
            result.push(crate::english::encode_english(c.to_ascii_lowercase())?);
            *prev_was_number = false;
            *i += 1;
            return Ok(true);
        }
    }

    let mut seq_end = *i;
    let mut uppercase_count = 0usize;
    while let Some(MathToken::UpperVariable(_)) = tokens.get(seq_end) {
        uppercase_count += 1;
        seq_end += 1;
        if matches!(tokens.get(seq_end), Some(MathToken::Prime)) {
            seq_end += 1;
        }
    }

    if uppercase_count >= 2 {
        result.push(32);
        result.push(32);
        for token in &tokens[*i..seq_end] {
            match token {
                MathToken::UpperVariable(upper) => {
                    result.push(crate::english::encode_english(upper.to_ascii_lowercase())?);
                }
                MathToken::Prime => result.push(36),
                _ => {}
            }
        }
        *i = seq_end;
        *prev_was_number = false;
        return Ok(true);
    }

    let omit_uppercase_indicator = *i == 0
        && matches!(
            tokens.get(*i + 1),
            Some(MathToken::MathSymbol('\u{2208}' | '\u{2209}'))
        );

    let overline_suffix_single = matches!(
        tokens.get(*i + 1),
        Some(MathToken::MathSymbol('\u{0304}' | '\u{0305}'))
    ) && !matches!(
        tokens.get(*i + 2),
        Some(MathToken::UpperVariable(_) | MathToken::Prime)
    );

    let logical_upper = logic_context
        && !matches!(
            (prev_non_space(tokens, *i), tokens.get(*i + 1)),
            (
                Some(MathToken::MathSymbol('\u{00AC}')),
                Some(MathToken::Variable(_))
            )
        )
        && !matches!(
            prev_non_space(tokens, *i),
            Some(MathToken::MathSymbol('\u{2192}'))
        )
        && !matches!(tokens.get(*i + 1), Some(MathToken::Subscript(_)))
        && !matches!(
            tokens.get(i.saturating_sub(1)),
            Some(MathToken::Subscript(_))
        );

    let predicate_form = matches!(
        (tokens.get(*i + 1), tokens.get(*i + 2), tokens.get(*i + 3)),
        (
            Some(MathToken::OpenParen(_)),
            Some(MathToken::Variable(_) | MathToken::UpperVariable(_)),
            Some(MathToken::CloseParen(_))
        )
    );

    if *i == 0
        && matches!(tokens.get(*i + 1), Some(MathToken::MathSymbol('\u{2228}')))
        && matches!(tokens.get(*i + 2), Some(MathToken::MathSymbol('\u{00AC}')))
        && let Some(MathToken::UpperVariable(right)) = tokens.get(*i + 3)
    {
        result.push(32);
        result.push(crate::english::encode_english(c.to_ascii_lowercase())?);
        result.push(0);
        let or_encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut('\u{2228}')?;
        result.extend_from_slice(or_encoded);
        result.push(0);
        let not_encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut('\u{00AC}')?;
        result.extend_from_slice(not_encoded);
        result.push(32);
        result.push(crate::english::encode_english(right.to_ascii_lowercase())?);
        *prev_was_number = false;
        *i += 4;
        return Ok(true);
    }

    if !(omit_uppercase_indicator
        || logical_upper
        || predicate_form && logic_context
        || overline_suffix_single)
    {
        result.push(32);
    }
    result.push(crate::english::encode_english(c.to_ascii_lowercase())?);

    *prev_was_number = false;
    *i += 1;
    Ok(false)
}

pub struct CombinatoricsRule;

impl MathTokenRule for CombinatoricsRule {
    fn name(&self) -> &'static str {
        "CombinatoricsRule"
    }

    fn priority(&self) -> u16 {
        10
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::Number(_)))
            && matches!(
                tokens.get(index + 1),
                Some(MathToken::UpperVariable('P' | 'C'))
            )
            && matches!(tokens.get(index + 2), Some(MathToken::Number(_)))
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        _engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        let (
            Some(MathToken::Number(left)),
            Some(MathToken::UpperVariable(mark)),
            Some(MathToken::Number(right)),
        ) = (
            tokens.get(index),
            tokens.get(index + 1),
            tokens.get(index + 2),
        )
        else {
            return Ok(MathTokenResult::Skip);
        };

        result.push(32);
        result.push(crate::english::encode_english(mark.to_ascii_lowercase())?);
        result.push(55);
        rule_1::encode_number_literal(left, result);
        result.push(0);
        rule_1::encode_number_literal(right, result);
        result.push(62);
        state.prev_was_number = false;
        Ok(MathTokenResult::Consumed(3))
    }
}

pub struct VariableRule;

impl MathTokenRule for VariableRule {
    fn name(&self) -> &'static str {
        "VariableRule"
    }

    fn priority(&self) -> u16 {
        50
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::Variable(_)))
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        let Some(MathToken::Variable(c)) = tokens.get(index) else {
            return Ok(MathTokenResult::Skip);
        };

        let mut cursor = index;
        let _ = encode_variable(
            *c,
            tokens,
            &mut cursor,
            &mut state.prev_was_number,
            result,
            engine,
        )?;
        Ok(MathTokenResult::Consumed(cursor - index))
    }
}

pub struct UpperVariableRule;

impl MathTokenRule for UpperVariableRule {
    fn name(&self) -> &'static str {
        "UpperVariableRule"
    }

    fn priority(&self) -> u16 {
        50
    }

    fn matches(&self, tokens: &[MathToken], index: usize, _state: &MathEncodeState) -> bool {
        matches!(tokens.get(index), Some(MathToken::UpperVariable(_)))
    }

    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        _engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String> {
        let Some(MathToken::UpperVariable(c)) = tokens.get(index) else {
            return Ok(MathTokenResult::Skip);
        };

        let mut cursor = index;
        let _ = encode_upper_variable(
            *c,
            tokens,
            &mut cursor,
            &mut state.prev_was_number,
            state.logic_context,
            result,
        )?;
        Ok(MathTokenResult::Consumed(cursor - index))
    }
}
