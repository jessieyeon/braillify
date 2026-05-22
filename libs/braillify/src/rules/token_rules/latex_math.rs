//! General LaTeX math expression handler.
//!
//! Strips `$...$` wrapping from LaTeX math expressions and encodes
//! the inner content using the math braille engine.
//!
//! Runs after LatexFractionRule to catch any `$...$` patterns
//! that aren't simple fractions.

use crate::rules::context::EncoderState;
use crate::rules::math;
use crate::rules::math::math_token_rule::MathContext;
use crate::rules::token::Token;
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};

pub struct LatexMathRule;

fn math_context_from_state(state: &EncoderState) -> MathContext {
    MathContext {
        matrix_context_active: state.matrix_context_active,
        math_mode_active: state.math_mode_active,
    }
}

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

mod matrix;
#[cfg(test)]
use matrix::subscript_digit_to_ascii;
use matrix::{encode_latex_matrix, find_latex_matrix};

pub(crate) fn encode_latex_math_bytes_with_context(
    latex_inner: &str,
    math_context: MathContext,
) -> Result<Vec<u8>, String> {
    if let Some(matrix) = find_latex_matrix(latex_inner) {
        return encode_latex_matrix(&matrix, math_context);
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

    math::encoder::encode_math_expression_with_context(&math_text, math_context)
}

mod spacing;
pub(crate) use spacing::wrap_latex_math_tokens_with_inner;

mod grouping;

mod strip;
pub(crate) use strip::strip_latex_to_math;

/// Merges `$...$` token sequences into single Word tokens.
/// This runs at Normalization phase so that downstream fraction/math rules
/// see the complete LaTeX expression as one token.
mod merge_rule;
pub use merge_rule::LatexMergeRule;

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
        state: &mut EncoderState,
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
                let math_context = math_context_from_state(state);
                if has_korean_suffix
                    && !inner.is_empty()
                    && inner.chars().count() <= 2
                    && inner.chars().all(|c| c.is_ascii_alphabetic())
                    && let Ok(inner_bytes) =
                        encode_latex_math_bytes_with_context(inner, math_context)
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
        let math_context = math_context_from_state(state);

        // Try to encode via math engine
        match encode_latex_math_bytes_with_context(inner, math_context) {
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

    /// Exercise subscript_digit_to_ascii for each codepoint.
    #[test]
    fn subscript_digit_to_ascii_table() {
        for (sub, ascii) in [
            ('\u{2080}', '0'),
            ('\u{2081}', '1'),
            ('\u{2082}', '2'),
            ('\u{2083}', '3'),
            ('\u{2084}', '4'),
            ('\u{2085}', '5'),
            ('\u{2086}', '6'),
            ('\u{2087}', '7'),
            ('\u{2088}', '8'),
            ('\u{2089}', '9'),
        ] {
            assert_eq!(subscript_digit_to_ascii(sub), Some(ascii), "sub={sub:?}");
        }
        assert!(subscript_digit_to_ascii('a').is_none());
        assert!(subscript_digit_to_ascii('0').is_none());
    }

    fn enc(input: &str) -> Vec<u8> {
        crate::encode(input).unwrap_or_default()
    }

    /// Comprehensive LaTeX matrix variants — every Begin/End matrix env.
    #[test]
    fn latex_matrix_environments() {
        let inputs: &[&str] = &[
            // matrix family
            "$\\begin{matrix} 1 & 2 \\\\ 3 & 4 \\end{matrix}$",
            "$\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}$",
            "$\\begin{bmatrix} 1 \\\\ 2 \\end{bmatrix}$",
            "$\\begin{Bmatrix} x & y \\end{Bmatrix}$",
            "$\\begin{vmatrix} a & b \\\\ c & d \\end{vmatrix}$",
            "$\\begin{Vmatrix} 1 & 0 \\\\ 0 & 1 \\end{Vmatrix}$",
            // arrays
            "$\\begin{array}{cc} x & y \\\\ z & w \\end{array}$",
            "$\\begin{array}{ll} a & b \\\\ c & d \\end{array}$",
            // determinant
            "$\\begin{vmatrix} a & b \\\\ c & d \\end{vmatrix}$",
        ];
        for input in inputs {
            let _ = enc(input);
        }
    }

    /// Various LaTeX command stripping cases.
    #[test]
    fn latex_command_stripping_diverse() {
        let inputs: &[&str] = &[
            "$\\alpha$",
            "$\\beta$",
            "$\\gamma$",
            "$\\delta$",
            "$\\theta$",
            "$\\lambda$",
            "$\\mu$",
            "$\\nu$",
            "$\\pi$",
            "$\\sigma$",
            "$\\tau$",
            "$\\phi$",
            "$\\chi$",
            "$\\psi$",
            "$\\omega$",
            "$\\Alpha$",
            "$\\Gamma$",
            "$\\Delta$",
            "$\\Theta$",
            "$\\infty$",
            "$\\partial$",
            "$\\nabla$",
            "$\\forall$",
            "$\\exists$",
            "$\\emptyset$",
            "$\\in$",
            "$\\notin$",
            "$\\subset$",
            "$\\supset$",
            "$\\cup$",
            "$\\cap$",
            "$\\land$",
            "$\\lor$",
            "$\\neg$",
            "$\\Rightarrow$",
            "$\\Leftrightarrow$",
            "$\\rightarrow$",
            "$\\cdot$",
            "$\\times$",
            "$\\div$",
            "$\\le$",
            "$\\ge$",
            "$\\equiv$",
            "$\\approx$",
            "$\\sum$",
            "$\\prod$",
            "$\\int$",
            "$\\oint$",
            // Compound
            "$x \\to \\infty$",
            "$a \\equiv b \\pmod{n}$",
            "$\\sqrt{a^2 + b^2}$",
            "$\\sqrt[n]{x}$",
        ];
        for input in inputs {
            let _ = enc(input);
        }
    }

    /// LaTeX with combining marks and accents.
    #[test]
    fn latex_accents_and_marks() {
        let inputs: &[&str] = &[
            "$\\bar{x}$",
            "$\\overline{AB}$",
            "$\\underline{x}$",
            "$\\vec{v}$",
            "$\\overrightarrow{AB}$",
            "$\\hat{x}$",
            "$\\widehat{ABC}$",
            "$\\tilde{x}$",
            "$\\widetilde{xy}$",
            "$\\dot{x}$",
            "$\\ddot{x}$",
            "$\\acute{a}$",
            "$\\grave{a}$",
            "$\\check{x}$",
            "$\\breve{x}$",
        ];
        for input in inputs {
            let _ = enc(input);
        }
    }

    /// LaTeX fraction variants.
    #[test]
    fn latex_fractions_diverse() {
        let inputs: &[&str] = &[
            "$\\frac{1}{2}$",
            "$\\frac{a}{b}$",
            "$\\frac{a+b}{c-d}$",
            "$\\frac{x^2}{y^2}$",
            "$\\frac{\\sqrt{2}}{2}$",
            "$\\frac{\\sin x}{\\cos x}$",
            "$\\dfrac{1}{2}$",
            "$\\tfrac{1}{2}$",
            "$\\cfrac{1}{2}$",
            "$\\binom{n}{k}$",
            "$\\dbinom{n}{k}$",
        ];
        for input in inputs {
            let _ = enc(input);
        }
    }

    /// LaTeX paren / bracket variations.
    #[test]
    fn latex_brackets_diverse() {
        let inputs: &[&str] = &[
            "$(x)$",
            "$[x]$",
            "$\\{x\\}$",
            "$\\langle x \\rangle$",
            "$\\left(x\\right)$",
            "$\\left[x\\right]$",
            "$\\left\\{x\\right\\}$",
            "$\\left| x \\right|$",
            "$\\lfloor x \\rfloor$",
            "$\\lceil x \\rceil$",
        ];
        for input in inputs {
            let _ = enc(input);
        }
    }
}
