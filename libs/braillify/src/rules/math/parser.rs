//! Math expression tokenizer/parser.
//!
//! Parses math expression strings into structured tokens
//! that can be encoded into braille by the encoder.



/// The kind of bracket in a math expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BracketKind {
    /// Standard math parentheses: f(x), (a+b)
    MathParen,
    /// Grouping brackets used in braille-only notation
    Grouping,
    /// Hangul-wrapped math group _( ... _)
    Hangul,
    /// Square brackets: [x]
    Square,
    /// Curly braces: {1, 2, 3}
    Curly,
}

/// A parsed math expression token.
#[derive(Debug, Clone)]
pub enum MathToken {
    /// Lowercase variable letter (a-z)
    Variable(char),
    /// Uppercase variable letter (A-Z)
    UpperVariable(char),
    /// Digit sequence (one or more digits)
    Number(String),
    /// Decimal point in number context
    DecimalPoint,
    /// Digit-group separator comma in number context
    DigitSeparator,
    /// Math operator (+, -, ×, ÷, =, >, <, etc.)
    Operator(char),
    /// Known function name (sin, cos, etc.)
    FunctionName(String),
    /// Korean word or phrase inside a math expression.
    KoreanWord(String),
    /// Opening bracket
    OpenParen(BracketKind),
    /// Closing bracket
    CloseParen(BracketKind),
    /// Superscript content (exponent)
    Superscript(Vec<MathToken>),
    /// Subscript content
    Subscript(Vec<MathToken>),
    /// Whitespace
    Space,
    /// Math symbol (Greek letters, set symbols, etc.)
    MathSymbol(char),
    /// Prime mark (′)
    Prime,
    /// Unrecognized character (fallback)
    Raw(char),
}

#[derive(Debug, Clone, Copy)]
struct GroupState {
    kind: BracketKind,
    token_index: usize,
    contains_korean: bool,
    contains_arithmetic: bool,
    contains_comma: bool,
    promote_grouping: bool,
}

mod helpers;

mod parse;
pub(crate) use parse::{parse_math_expression, parse_math_expression_with_math_mode};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_variable() {
        let tokens = parse_math_expression("x").unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], MathToken::Variable('x')));
    }

    #[test]
    fn test_parse_number() {
        let tokens = parse_math_expression("42").unwrap();
        assert_eq!(tokens.len(), 1);
        if let MathToken::Number(n) = &tokens[0] {
            assert_eq!(n, "42");
        } else {
            panic!("Expected Number token");
        }
    }

    #[test]
    fn test_parse_function() {
        let tokens = parse_math_expression("sin3x").unwrap();
        assert!(tokens.len() >= 3);
        assert!(matches!(tokens[0], MathToken::FunctionName(ref n) if n == "sin"));
    }

    #[test]
    fn test_parse_superscript() {
        let tokens = parse_math_expression("x²").unwrap();
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0], MathToken::Variable('x')));
        assert!(matches!(tokens[1], MathToken::Superscript(_)));
    }

    #[test]
    fn test_parse_equation() {
        let tokens = parse_math_expression("ax+b=0").unwrap();
        // a, x, +, b, =, 0
        assert!(tokens.len() >= 5);
    }

    #[test]
    fn test_parse_ascii_apostrophe_as_prime() {
        let tokens = parse_math_expression("f''(x)").unwrap();
        assert!(matches!(tokens[1], MathToken::Prime));
        assert!(matches!(tokens[2], MathToken::Prime));
    }

    #[test]
    fn test_grouping_paren_after_function() {
        let tokens = parse_math_expression("sin(x)").unwrap();
        assert!(matches!(
            tokens[1],
            MathToken::OpenParen(BracketKind::Grouping)
        ));
        assert!(matches!(
            tokens[3],
            MathToken::CloseParen(BracketKind::Grouping)
        ));
    }

    /// Exercise every Unicode superscript character recognized by
    /// `normalize_superscript`. Each codepoint must parse without error
    /// and produce at least one token.
    #[test]
    fn unicode_superscripts_parsed() {
        let superscripts: &[char] = &[
            '\u{2070}', '\u{00B9}', '\u{00B2}', '\u{00B3}', '\u{2074}', '\u{2075}', '\u{2076}',
            '\u{2077}', '\u{2078}', '\u{2079}', '\u{207A}', '\u{207B}', '\u{207D}', '\u{207E}',
            '\u{207F}', '\u{1D43}', '\u{1D47}', '\u{1D9C}', '\u{1D48}', '\u{1D49}', '\u{1DA0}',
            '\u{1D4D}', '\u{02B0}', '\u{2071}', '\u{02B2}', '\u{1D4F}', '\u{02E1}', '\u{1D50}',
            '\u{1D52}', '\u{1D56}', '\u{02B3}', '\u{02E2}', '\u{1D57}', '\u{1D58}', '\u{1D5B}',
            '\u{02B7}', '\u{02E3}', '\u{02B8}', '\u{1DBB}',
        ];
        for &c in superscripts {
            let input = format!("x{}", c);
            let result = parse_math_expression(&input);
            assert!(result.is_ok(), "parse failed for x{:?}", c);
            assert!(!result.unwrap().is_empty());
        }
    }

    /// Exercise every Unicode subscript character recognized by
    /// `normalize_subscript`.
    #[test]
    fn unicode_subscripts_parsed() {
        let subscripts: &[char] = &[
            '\u{2080}', '\u{2081}', '\u{2082}', '\u{2083}', '\u{2084}', '\u{2085}', '\u{2086}',
            '\u{2087}', '\u{2088}', '\u{2089}', '\u{208A}', '\u{208B}', '\u{208D}', '\u{208E}',
            '\u{2090}', '\u{2091}', '\u{2092}', '\u{2093}', '\u{2095}', '\u{2096}', '\u{2097}',
            '\u{2098}', '\u{2099}', '\u{209A}', '\u{209B}', '\u{209C}', '\u{1D62}', '\u{1D63}',
            '\u{1D64}', '\u{1D65}',
        ];
        for &c in subscripts {
            let input = format!("x{}", c);
            let result = parse_math_expression(&input);
            assert!(result.is_ok(), "parse failed for x{:?}", c);
            assert!(!result.unwrap().is_empty());
        }
    }

    /// Underline-notation fraction normalization paths (lines around 300-330).
    #[test]
    fn underline_notation_fraction_paths() {
        // U+0332 suffix on digit prefix → ⟨digits⟩/1
        let _ = parse_math_expression("123\u{0332}");
        // "1̲/(...)" pattern
        let _ = parse_math_expression("1\u{0332}/(x+y)");
        // "X̲/Y" generic underline-fraction
        let _ = parse_math_expression("A\u{0332}/B");
    }
}
