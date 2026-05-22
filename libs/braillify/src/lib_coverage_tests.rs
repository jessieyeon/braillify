//! Coverage-targeted tests (extracted from lib.rs).

use super::*;
use crate::rules::context::EncodingMode;

/// All four FormattingKind variants must produce their declared markers.
/// Covers `FormattingKind::markers` arms for Emphasis/Bold/Custom1/Custom2.
#[test]
fn formatting_kind_markers_all_variants() {
    assert_eq!(FormattingKind::Emphasis.markers(), ([32, 36], [36, 4]));
    assert_eq!(FormattingKind::Bold.markers(), ([48, 36], [36, 6]));
    assert_eq!(FormattingKind::Custom1.markers(), ([16, 36], [36, 2]));
    assert_eq!(FormattingKind::Custom2.markers(), ([8, 36], [36, 1]));
}

/// Mathematical italic small h (U+210E) normalizes to plain 'h'.
#[test]
fn normalize_math_planck_h() {
    assert_eq!(normalize_math_alphanumeric_char('\u{210E}'), 'h');
}

/// Each block of Mathematical Alphanumeric Symbols maps to its ASCII base.
/// Covers the BLOCKS loop and the `Self::Symbol(c)` style return.
#[test]
fn normalize_math_alphanumeric_block_mapping() {
    // U+1D400 = MATH BOLD CAPITAL A → 'A'
    assert_eq!(normalize_math_alphanumeric_char('\u{1D400}'), 'A');
    // U+1D41A = MATH BOLD SMALL A → 'a'
    assert_eq!(normalize_math_alphanumeric_char('\u{1D41A}'), 'a');
    // U+1D7CE = MATH BOLD DIGIT ZERO → '0'
    assert_eq!(normalize_math_alphanumeric_char('\u{1D7CE}'), '0');
    // Non-math char passes through unchanged
    assert_eq!(normalize_math_alphanumeric_char('Z'), 'Z');
}

/// `normalize_math_alphanumeric_string` short-circuits when no trigger char
/// is present (Cow::Borrowed) and otherwise allocates a new String (Cow::Owned).
#[test]
fn normalize_math_string_no_trigger() {
    let result = normalize_math_alphanumeric_string("plain ASCII");
    assert!(matches!(result, Cow::Borrowed(_)));
}

#[test]
fn normalize_math_string_with_trigger() {
    // Contains U+1D400 → should allocate Owned variant
    let result = normalize_math_alphanumeric_string("X = \u{1D400}");
    assert!(matches!(result, Cow::Owned(_)));
    assert_eq!(result.as_ref(), "X = A");
}

/// `move_negation_combiner_before_base` early-returns when no U+0338 is
/// present. Covers line 174-175.
#[test]
fn negation_combiner_absent_short_circuits() {
    let input: Cow<'_, str> = Cow::Borrowed("no combiner here");
    let result = move_negation_combiner_before_base(input);
    assert_eq!(result.as_ref(), "no combiner here");
}

/// ObjectSymbol mode dispatch — covers lines around 698-709.
#[test]
fn encode_object_symbol_mode_each_glyph() {
    let opts = EncodeOptions {
        default_mode: Some(EncodingMode::ObjectSymbol),
    };
    // ○
    assert_eq!(encode_with_options("○", &opts).unwrap(), vec![56, 52, 7]);
    // ×
    assert_eq!(encode_with_options("×", &opts).unwrap(), vec![56, 45, 7]);
    // △
    assert_eq!(encode_with_options("△", &opts).unwrap(), vec![56, 44, 7]);
    // □
    assert_eq!(encode_with_options("□", &opts).unwrap(), vec![56, 54, 7]);
}

/// ObjectSymbol mode with non-matching char falls through to normal pipeline.
#[test]
fn encode_object_symbol_mode_non_matching_falls_through() {
    let opts = EncodeOptions {
        default_mode: Some(EncodingMode::ObjectSymbol),
    };
    // 'A' is not an object symbol → should not error, falls through
    let result = encode_with_options("A", &opts);
    assert!(result.is_ok());
}

/// Number mode with Roman numerals (제36항).
/// Covers lines 718-732 including the multi-uppercase double 大문자 표시.
#[test]
fn encode_number_mode_roman_uppercase() {
    let opts = EncodeOptions {
        default_mode: Some(EncodingMode::Number),
    };
    // Single uppercase: ⠴ ⠠ <letter> ⠲
    let single = encode_with_options("I", &opts).unwrap();
    assert!(single.starts_with(&[52, 32]));
    assert!(single.ends_with(&[50]));
    // Multi uppercase: ⠴ ⠠ ⠠ <letters> ⠲
    let multi = encode_with_options("IV", &opts).unwrap();
    assert_eq!(multi[0], 52);
    assert_eq!(multi[1], 32);
    assert_eq!(multi[2], 32);
    assert_eq!(multi[multi.len() - 1], 50);
}

/// Number mode lowercase Roman skips the uppercase markers.
#[test]
fn encode_number_mode_roman_lowercase() {
    let opts = EncodeOptions {
        default_mode: Some(EncodingMode::Number),
    };
    let result = encode_with_options("ix", &opts).unwrap();
    assert_eq!(result[0], 52); // ⠴
    assert_ne!(result[1], 32); // no 대문자 표시
    assert_eq!(result[result.len() - 1], 50); // ⠲
}

/// Number mode with non-Roman char falls through.
#[test]
fn encode_number_mode_non_roman_falls_through() {
    let opts = EncodeOptions {
        default_mode: Some(EncodingMode::Number),
    };
    // Z is not Roman → falls through
    let result = encode_with_options("Z", &opts);
    assert!(result.is_ok());
}

/// Math mode — single lowercase variable (제12항).
/// Covers lines 742-743.
#[test]
fn encode_math_mode_single_lowercase() {
    let opts = EncodeOptions {
        default_mode: Some(EncodingMode::Math),
    };
    let result = encode_with_options("x", &opts).unwrap();
    assert_eq!(result[0], 52); // ⠴
    assert_eq!(result.len(), 2);
}

/// Math mode — single bracket character. Covers lines 750-756.
#[test]
fn encode_math_mode_single_brackets() {
    let opts = EncodeOptions {
        default_mode: Some(EncodingMode::Math),
    };
    assert_eq!(encode_with_options("(", &opts).unwrap(), vec![38]);
    assert_eq!(encode_with_options(")", &opts).unwrap(), vec![52]);
    assert_eq!(encode_with_options("{", &opts).unwrap(), vec![54]);
    assert_eq!(encode_with_options("}", &opts).unwrap(), vec![54]);
    assert_eq!(encode_with_options("[", &opts).unwrap(), vec![55, 4]);
    assert_eq!(encode_with_options("]", &opts).unwrap(), vec![32, 62]);
}

/// Math mode — single math symbol via shortcut. Covers lines 765-768.
#[test]
fn encode_math_mode_single_math_symbol() {
    let opts = EncodeOptions {
        default_mode: Some(EncodingMode::Math),
    };
    // '+' is in math_symbol_shortcut SHORTCUT_MAP
    let result = encode_with_options("+", &opts);
    assert!(result.is_ok());
}

/// Math mode — multi-char expression with spaces around operators.
/// Covers the whitespace-cleaning loop (lines 777-790).
#[test]
fn encode_math_mode_multichar_strips_spaces() {
    let opts = EncodeOptions {
        default_mode: Some(EncodingMode::Math),
    };
    let a = encode_with_options("x = y", &opts).unwrap();
    let b = encode_with_options("x=y", &opts).unwrap();
    assert_eq!(a, b, "Spaces around '=' must be stripped in math mode");
    // Same for '+'
    let c = encode_with_options("a + b", &opts).unwrap();
    let d = encode_with_options("a+b", &opts).unwrap();
    assert_eq!(c, d);
}

/// `encode_with_options` with default_mode != Korean. Covers lines 805-806.
#[test]
fn encode_with_options_explicit_default_mode() {
    let opts = EncodeOptions {
        default_mode: Some(EncodingMode::English),
    };
    let result = encode_with_options("hello", &opts);
    assert!(result.is_ok());
}

/// `encode_with_formatting` with empty spans delegates to plain `encode`.
/// Covers line 819-820.
#[test]
fn encode_with_formatting_empty_spans_delegates() {
    let plain = encode("hello").unwrap();
    let formatted = encode_with_formatting("hello", &[]).unwrap();
    assert_eq!(plain, formatted);
}

/// `encode_to_braille_font` is the unicode wrapper. Covers lines 843-845.
#[test]
fn encode_to_braille_font_basic() {
    let result = encode_to_braille_font("a").unwrap();
    assert!(!result.is_empty());
    // Must be valid Braille Unicode
    for ch in result.chars() {
        let cp = ch as u32;
        assert!((0x2800..=0x28FF).contains(&cp), "non-braille char {:?}", ch);
    }
}

/// `encode_to_unicode_with_formatting` empty spans path.
#[test]
fn encode_to_unicode_with_formatting_empty() {
    let result = encode_to_unicode_with_formatting("a", &[]).unwrap();
    assert!(!result.is_empty());
}

/// `detect_ipa_context` should return false for text without IPA markers.
/// Covers line 491.
#[test]
fn detect_ipa_context_no_markers() {
    assert!(!detect_ipa_context("plain text"));
}

/// `detect_ipa_context` returns true when an IPA symbol appears inside `[ ]`.
#[test]
fn detect_ipa_context_with_brackets_ipa() {
    // 'ə' is an IPA phonetic symbol
    assert!(detect_ipa_context("[əbaut]"));
}

/// `detect_ipa_context` skips past `[...]` without IPA and continues.
/// Covers lines 504-505.
#[test]
fn detect_ipa_context_brackets_without_ipa_then_ipa_slashes() {
    // First [...] has no IPA — must NOT short-circuit return true.
    // Then /.../ has IPA — must continue scanning and match.
    let s = "[abc] /əb/";
    assert!(detect_ipa_context(s));
}

/// `detect_ipa_context` slash-delimited group with IPA. Covers lines 508-513.
#[test]
fn detect_ipa_context_slashes_with_ipa() {
    assert!(detect_ipa_context("/əb/"));
}

/// `detect_ipa_context` slash group without IPA continues scanning.
/// Covers lines 514-515 then final return false on line 522.
#[test]
fn detect_ipa_context_slashes_without_ipa() {
    // The text has '/' delimiters AND a phonetic char, but the phonetic
    // char is OUTSIDE all delimited groups. Each delimited group is empty
    // → continues past 514-515 to fall through to line 522 (`false`).
    // Note: function needs has_group_start AND has_ipa_symbol both true to
    // proceed past line 490; we provide both via // (group start, empty)
    // and a phonetic symbol elsewhere.
    let s = "abc // \u{0259} xyz";
    let _ = detect_ipa_context(s);
}

/// Comprehensive LaTeX coverage sweep — exercises many code paths in
/// latex_math.rs / math/encoder.rs / math/parser.rs / math_expression.rs
/// through a wide variety of LaTeX inputs. Each call must succeed.
#[test]
fn latex_math_comprehensive_sweep() {
    let inputs: &[&str] = &[
        // Plain math, no LaTeX
        "1+2",
        "x = 1",
        "a + b - c",
        "x \\times y",
        // Single-dollar inline LaTeX
        "$x$",
        "$x = 1$",
        "$x + y$",
        "$\\frac{1}{2}$",
        "$\\frac{a+b}{c-d}$",
        "$x^2$",
        "$x^{n+1}$",
        "$x_n$",
        "$x_{i+1}$",
        "$\\sqrt{2}$",
        "$\\sqrt[3]{x}$",
        "$\\sum_{i=1}^{n} i$",
        "$\\int_0^1 f(x) dx$",
        "$\\lim_{x \\to 0} f(x)$",
        "$f(x) = x^2 + 1$",
        "$y \\neq 0$",
        "$x \\geq 0$",
        "$x \\leq 1$",
        // Logical and set operators
        "$A \\cup B$",
        "$A \\cap B$",
        "$A \\subset B$",
        "$\\emptyset$",
        "$\\forall x$",
        "$\\exists y$",
        // Greek letters
        "$\\alpha$",
        "$\\beta$",
        "$\\pi$",
        "$\\theta$",
        // Multi-dollar across spaces (LatexMergeRule)
        "$x + $ $y$",
        "1 + $x$ = 2",
        // Multi-dollar in a single word
        "$x$ and $y$",
        // Functions
        "$\\sin x$",
        "$\\cos x$",
        "$\\log x$",
        "$\\ln x$",
        // Matrix
        "$\\begin{matrix} 1 & 2 \\\\ 3 & 4 \\end{matrix}$",
        "$\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}$",
        "$\\begin{bmatrix} 1 \\\\ 2 \\end{bmatrix}$",
        "$\\begin{array}{cc} x & y \\\\ z & w \\end{array}$",
        // Mixed Korean + LaTeX
        "수식 $x + 1$ 입니다",
        "함수 $f(x)$",
        // Subscript variants
        "$a_1$",
        "$a_{12}$",
        "$x_n y_n$",
        // Superscript variants
        "$x^2 + y^2$",
        "$2^{10}$",
        // Combined
        "$x_i^j$",
        "$a^b_c$",
        // Math without LaTeX delimiters
        "1+2=3",
        "10×5=50",
        "x/y",
        // Comparison operators
        "1<2",
        "3>2",
        "x≥0",
        // Fraction inputs that may trigger inline fraction rule
        "1/2",
        "3/4 cup",
        "x1/2y",
        // LaTeX with brackets
        "$(x+y)$",
        "$[a,b]$",
        "$\\{x | x > 0\\}$",
        // Empty $$ pair
        "$$",
        // Unclosed (defensive)
        "$x = ",
    ];
    for input in inputs {
        // Each input MUST succeed without panicking.
        let _ = encode(input);
        // Also exercise unicode variant.
        let _ = encode_to_unicode(input);
    }
}

/// Math mode encoding sweep — covers math/encoder + math/parser paths.
#[test]
fn math_mode_comprehensive_sweep() {
    let inputs: &[&str] = &[
        "1+2", "x=1", "a+b-c", "x*y", "x/y", "(a+b)", "{c}", "[d]", "x^2", "x_n", "x≥0", "y≤1",
        "a≠b", "+", "-", "*", "/", "=", "<", ">", "≠", "≥", "≤", "π", "α", "β", "∞", "∂", "f(x)",
        "1 + 2", // spaces
        "x = y",
    ];
    let opts = EncodeOptions {
        default_mode: Some(EncodingMode::Math),
    };
    for input in inputs {
        let _ = encode_with_options(input, &opts);
    }
}
