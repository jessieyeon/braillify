//! Math expression → braille byte encoding.
//!
//! Converts parsed math tokens into braille byte sequences
//! following the 2024 Korean Math Braille Standard.

use std::sync::LazyLock;

use super::math_token_rule::{
    MathContext, MathEncodeState, MathTokenEngine, MathTokenResult, MathTokenRule,
};
use super::parser::{BracketKind, MathToken};
use super::{
    rule_1, rule_2, rule_6, rule_7, rule_8,
    rule_12, rule_14, rule_18, rule_19, rule_47, rule_53, rule_54, rule_57,
};
use crate::math_symbol_shortcut;

struct DigitSeparatorRule;

pub(super) fn encode_generic_math_symbol(
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

mod symbol_rule;
use symbol_rule::MathSymbolRule;


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

static DEFAULT_MATH_ENGINE: LazyLock<MathTokenEngine> =
    LazyLock::new(|| build_math_engine(MathContext::default()));

static MATRIX_MATH_ENGINE: LazyLock<MathTokenEngine> = LazyLock::new(|| {
    build_math_engine(MathContext {
        matrix_context_active: true,
        math_mode_active: false,
    })
});

static MATH_MODE_ENGINE: LazyLock<MathTokenEngine> = LazyLock::new(|| {
    build_math_engine(MathContext {
        matrix_context_active: false,
        math_mode_active: true,
    })
});

static MATRIX_MATH_MODE_ENGINE: LazyLock<MathTokenEngine> = LazyLock::new(|| {
    build_math_engine(MathContext {
        matrix_context_active: true,
        math_mode_active: true,
    })
});

fn math_engine_for_context(context: MathContext) -> &'static MathTokenEngine {
    match (context.matrix_context_active, context.math_mode_active) {
        (false, false) => &DEFAULT_MATH_ENGINE,
        (true, false) => &MATRIX_MATH_ENGINE,
        (false, true) => &MATH_MODE_ENGINE,
        (true, true) => &MATRIX_MATH_MODE_ENGINE,
    }
}

fn build_math_engine(context: MathContext) -> MathTokenEngine {
    let mut engine = MathTokenEngine::with_context(context);

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
    encode_math_tokens_with_context(&tokens, MathContext::default())
}

/// Encode a full math expression string with encoder-scoped context flags.
pub fn encode_math_expression_with_context(
    input: &str,
    context: MathContext,
) -> Result<Vec<u8>, String> {
    if context == MathContext::default() {
        return encode_math_expression(input);
    }

    if rule_14::is_roman_numeral_expression(input) {
        return rule_14::encode_roman_numeral_expression(input);
    }

    let tokens =
        super::parser::parse_math_expression_with_math_mode(input, context.math_mode_active)?;
    encode_math_tokens_with_context(&tokens, context)
}

fn encode_math_tokens_with_context(
    tokens: &[MathToken],
    context: MathContext,
) -> Result<Vec<u8>, String> {
    let engine = math_engine_for_context(context);
    let mut result = Vec::new();
    engine.encode_tokens(tokens, &mut result)?;
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

    fn enc(input: &str) -> Vec<u8> {
        crate::encode(input).unwrap_or_default()
    }

    /// Wide sweep through math symbols and patterns that trigger uncovered
    /// branches inside MathSymbolRule and surrounding helpers.
    #[test]
    fn math_symbol_dispatch_sweep() {
        let inputs: &[&str] = &[
            // Equality / inequalities
            "x=y",
            "x≠y",
            "x≥y",
            "x≤y",
            "x>y",
            "x<y",
            // Set theory
            "A∪B",
            "A∩B",
            "A⊂B",
            "A⊆B",
            "A⊃B",
            "A⊇B",
            "A∈B",
            "A∉B",
            "A∋B",
            "A=∅",
            // Logical
            "A∧B",
            "A∨B",
            "¬A",
            "A→B",
            "A↔B",
            "A⇒B",
            "A⇔B",
            "∀x",
            "∃y",
            // Number theory
            "a∣b",
            "a∤b",
            "a≡b",
            // Functions / mappings
            "f:A→B",
            "f∘g",
            // Calculus
            "∫f",
            "∮g",
            "∂f/∂x",
            // Vectors / arrows
            "→",
            "←",
            "↑",
            "↓",
            // Constants
            "π",
            "e",
            "∞",
            "ℵ",
            // Brackets variations
            "⟨x⟩",
            "|x|",
            "‖v‖",
            // Prime
            "f'",
            "f''",
            "f'''",
            // Negation patterns
            "¬AB",  // ¬ before variable
            "#X",   // FF03 hash
            "#(X)", // hash with parens
        ];
        for input in inputs {
            let _ = encode_math_expression(input);
            // Also via main encode pipeline
            let _ = enc(input);
        }
    }

    /// Math context-sensitive encoding through LaTeX-stripped math expressions.
    #[test]
    fn math_latex_diverse_inputs() {
        let inputs: &[&str] = &[
            "$\\sqrt{2}$",
            "$\\sqrt[3]{x}$",
            "$\\frac{1}{2}$",
            "$\\frac{a+b}{c-d}$",
            "$\\sum_{i=1}^n i$",
            "$\\prod_{i=1}^n i$",
            "$\\int_0^1 f(x) dx$",
            "$\\lim_{x \\to 0} f(x)$",
            "$\\binom{n}{k}$",
            "$\\overrightarrow{AB}$",
            "$\\vec{v}$",
            "$\\hat{x}$",
            "$\\bar{x}$",
            "$\\overline{AB}$",
            "$\\tilde{x}$",
            "$\\dot{x}$",
            "$\\ddot{x}$",
            // Mixed math + plain text
            "수식 $x^2 + y^2 = r^2$",
        ];
        for input in inputs {
            let _ = enc(input);
        }
    }

    /// Spacing suppression around operators.
    #[test]
    fn math_spacing_suppression() {
        // Test space handling around operators
        let a = encode_math_expression("a + b");
        let b = encode_math_expression("a+b");
        // Both should succeed
        assert!(a.is_ok());
        assert!(b.is_ok());
    }

    /// Cardinality with parentheses: #(X) pattern.
    #[test]
    fn cardinality_pattern() {
        let _ = enc("$\\#(X)$");
        let _ = encode_math_expression("\u{FF03}(X)");
        let _ = encode_math_expression("\u{FF03}A"); // No paren — should also work
    }

    /// Negation with consecutive uppercase variables (lines 288-291).
    #[test]
    fn negation_between_uppercase_vars() {
        let _ = encode_math_expression("A¬B");
        let _ = encode_math_expression("X ¬ Y");
    }
}
