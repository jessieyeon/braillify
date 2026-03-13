use unicode_normalization::UnicodeNormalization;

const FRACTION_SLASH: char = '\u{2044}';

fn consume_whitespace(iter: &mut std::iter::Peekable<std::str::Chars>) {
    while let Some(c) = iter.peek() {
        if c.is_whitespace() {
            iter.next();
        } else {
            break;
        }
    }
}

fn encode_number_string(s: &str, part_name: &str) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    for c in s.chars() {
        if !c.is_ascii_digit() {
            return Err(format!(
                "Invalid {} part (non-ascii digit): {}",
                part_name, c
            ));
        }
        result.extend(crate::number::encode_number(c));
    }
    Ok(result)
}

pub fn encode_fraction(numerator: &str, denominator: &str) -> Result<Vec<u8>, String> {
    let mut result = vec![60];
    result.extend(encode_number_string(denominator, "fraction denominator")?);
    result.push(12);
    result.push(60);
    result.extend(encode_number_string(numerator, "fraction numerator")?);
    Ok(result)
}

pub fn encode_fraction_in_context(numerator: &str, denominator: &str) -> Result<Vec<u8>, String> {
    let mut result = vec![60];
    result.extend(encode_number_string(numerator, "fraction numerator")?);
    result.push(56);
    result.push(12);
    result.push(60);
    result.extend(encode_number_string(denominator, "fraction denominator")?);
    Ok(result)
}

pub fn encode_mixed_fraction(
    whole: &str,
    numerator: &str,
    denominator: &str,
) -> Result<Vec<u8>, String> {
    let mut result = vec![60];
    result.extend(encode_number_string(whole, "whole number")?);
    result.extend(encode_fraction(numerator, denominator)?);
    Ok(result)
}

fn normalize_digit(c: char) -> Option<char> {
    match c {
        '0' | '⁰' | '₀' => Some('0'),
        '1' | '¹' | '₁' => Some('1'),
        '2' | '²' | '₂' => Some('2'),
        '3' | '³' | '₃' => Some('3'),
        '4' | '⁴' | '₄' => Some('4'),
        '5' | '⁵' | '₅' => Some('5'),
        '6' | '⁶' | '₆' => Some('6'),
        '7' | '⁷' | '₇' => Some('7'),
        '8' | '⁸' | '₈' => Some('8'),
        '9' | '⁹' | '₉' => Some('9'),
        _ => None,
    }
}

fn read_braced_content(iter: &mut std::iter::Peekable<std::str::Chars>) -> Option<String> {
    consume_whitespace(iter);

    if iter.next()? != '{' {
        return None;
    }

    let mut content = String::new();
    while let Some(c) = iter.peek() {
        match c {
            '}' => {
                iter.next();
                return if content.is_empty() {
                    None
                } else {
                    Some(content)
                };
            }
            _ if c.is_whitespace() => {
                iter.next();
            }
            _ => {
                if let Some(digit) = normalize_digit(*c) {
                    content.push(digit);
                    iter.next();
                } else {
                    return None;
                }
            }
        }
    }
    None
}

pub fn parse_latex_fraction(s: &str) -> Option<(Option<String>, String, String)> {
    let mut iter = s.trim().chars().peekable();

    if iter.next()? != '$' {
        return None;
    }

    consume_whitespace(&mut iter);

    let mut whole_part_str = String::new();
    while let Some(digit) = iter.peek().and_then(|c| normalize_digit(*c)) {
        whole_part_str.push(digit);
        iter.next();
    }
    let whole_part = if whole_part_str.is_empty() {
        None
    } else {
        Some(whole_part_str)
    };

    consume_whitespace(&mut iter);

    if iter.next() != Some('\\')
        || iter.next() != Some('f')
        || iter.next() != Some('r')
        || iter.next() != Some('a')
        || iter.next() != Some('c')
    {
        return None;
    }

    let numerator = read_braced_content(&mut iter)?;
    let denominator = read_braced_content(&mut iter)?;

    consume_whitespace(&mut iter);

    if iter.next()? != '$' {
        return None;
    }

    consume_whitespace(&mut iter);

    if iter.next().is_some() {
        return None;
    }

    Some((whole_part, numerator, denominator))
}

pub fn parse_unicode_fraction(c: char) -> Option<(String, String)> {
    let decomposed = c.nfkd().collect::<String>();
    parse_decomposed_fraction(&decomposed)
}

fn parse_decomposed_fraction(decomposed: &str) -> Option<(String, String)> {
    if !decomposed.contains(FRACTION_SLASH) {
        return None;
    }

    let parts: Vec<&str> = decomposed.split(FRACTION_SLASH).collect();

    if parts.len() == 2 {
        let num_str = parts[0].trim();
        let den_str = parts[1].trim();
        if num_str.is_empty() || den_str.is_empty() {
            return None;
        }
        if !num_str.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        if !den_str.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        Some((num_str.to_string(), den_str.to_string()))
    } else {
        None
    }
}

pub fn is_unicode_fraction(c: char) -> bool {
    parse_unicode_fraction(c).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_number_string_valid() {
        let result = encode_number_string("123", "test").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_encode_number_string_invalid_non_digit() {
        let result = encode_number_string("a", "test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid test part"));
    }

    #[test]
    fn test_encode_number_string_invalid_mixed() {
        let result = encode_number_string("1a2", "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_number_string_empty() {
        let result = encode_number_string("", "test").unwrap();
        assert_eq!(result, Vec::<u8>::new());
    }

    #[test]
    fn test_encode_fraction_simple() {
        let result = encode_fraction("3", "4").unwrap();
        assert_eq!(result, vec![60, 25, 12, 60, 9]);
    }

    #[test]
    fn test_encode_fraction_double_digit() {
        let result = encode_fraction("12", "34").unwrap();
        assert!(result.starts_with(&[60]));
    }

    #[test]
    fn test_encode_fraction_invalid_numerator() {
        let result = encode_fraction("a", "4");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("numerator"));
    }

    #[test]
    fn test_encode_fraction_invalid_denominator() {
        let result = encode_fraction("3", "b");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("denominator"));
    }

    #[test]
    fn test_encode_fraction_in_context_simple() {
        let result = encode_fraction_in_context("2", "3").unwrap();
        assert_eq!(result, vec![60, 3, 56, 12, 60, 9]);
    }

    #[test]
    fn test_encode_fraction_in_context_invalid_numerator() {
        let result = encode_fraction_in_context("x", "3");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("numerator"));
    }

    #[test]
    fn test_encode_fraction_in_context_invalid_denominator() {
        let result = encode_fraction_in_context("2", "y");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("denominator"));
    }

    #[test]
    fn test_encode_mixed_fraction_simple() {
        let result = encode_mixed_fraction("3", "1", "6").unwrap();
        assert_eq!(result, vec![60, 9, 60, 11, 12, 60, 1]);
    }

    #[test]
    fn test_encode_mixed_fraction_invalid_whole() {
        let result = encode_mixed_fraction("a", "1", "6");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("whole"));
    }

    #[test]
    fn test_encode_mixed_fraction_invalid_numerator() {
        let result = encode_mixed_fraction("3", "b", "6");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("numerator"));
    }

    #[test]
    fn test_encode_mixed_fraction_invalid_denominator() {
        let result = encode_mixed_fraction("3", "1", "c");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("denominator"));
    }

    #[test]
    fn test_normalize_digit_regular() {
        assert_eq!(normalize_digit('0'), Some('0'));
        assert_eq!(normalize_digit('1'), Some('1'));
        assert_eq!(normalize_digit('9'), Some('9'));
    }

    #[test]
    fn test_normalize_digit_superscript() {
        assert_eq!(normalize_digit('⁰'), Some('0'));
        assert_eq!(normalize_digit('¹'), Some('1'));
        assert_eq!(normalize_digit('²'), Some('2'));
        assert_eq!(normalize_digit('³'), Some('3'));
        assert_eq!(normalize_digit('⁴'), Some('4'));
        assert_eq!(normalize_digit('⁵'), Some('5'));
        assert_eq!(normalize_digit('⁶'), Some('6'));
        assert_eq!(normalize_digit('⁷'), Some('7'));
        assert_eq!(normalize_digit('⁸'), Some('8'));
        assert_eq!(normalize_digit('⁹'), Some('9'));
    }

    #[test]
    fn test_normalize_digit_subscript() {
        assert_eq!(normalize_digit('₀'), Some('0'));
        assert_eq!(normalize_digit('₁'), Some('1'));
        assert_eq!(normalize_digit('₂'), Some('2'));
        assert_eq!(normalize_digit('₃'), Some('3'));
        assert_eq!(normalize_digit('₄'), Some('4'));
        assert_eq!(normalize_digit('₅'), Some('5'));
        assert_eq!(normalize_digit('₆'), Some('6'));
        assert_eq!(normalize_digit('₇'), Some('7'));
        assert_eq!(normalize_digit('₈'), Some('8'));
        assert_eq!(normalize_digit('₉'), Some('9'));
    }

    #[test]
    fn test_normalize_digit_invalid() {
        assert_eq!(normalize_digit('a'), None);
        assert_eq!(normalize_digit('A'), None);
        assert_eq!(normalize_digit('!'), None);
        assert_eq!(normalize_digit(' '), None);
    }

    #[test]
    fn test_parse_latex_fraction_simple() {
        let result = parse_latex_fraction("$\\frac{3}{4}$");
        assert_eq!(result, Some((None, "3".to_string(), "4".to_string())));
    }

    #[test]
    fn test_parse_latex_fraction_mixed() {
        let result = parse_latex_fraction("$3\\frac{1}{6}$");
        assert_eq!(
            result,
            Some((Some("3".to_string()), "1".to_string(), "6".to_string()))
        );
    }

    #[test]
    fn test_parse_latex_fraction_mixed_double_digit() {
        let result = parse_latex_fraction("$12\\frac{34}{56}$");
        assert_eq!(
            result,
            Some((Some("12".to_string()), "34".to_string(), "56".to_string()))
        );
    }

    #[test]
    fn test_parse_latex_fraction_with_whitespace() {
        let result = parse_latex_fraction("$ \\frac{ 3 }{ 4 } $");
        assert_eq!(result, Some((None, "3".to_string(), "4".to_string())));
    }

    #[test]
    fn test_parse_latex_fraction_with_leading_trailing_whitespace() {
        let result = parse_latex_fraction("  $\\frac{3}{4}$  ");
        assert_eq!(result, Some((None, "3".to_string(), "4".to_string())));
    }

    #[test]
    fn test_parse_latex_fraction_with_superscript() {
        let result = parse_latex_fraction("$\\frac{³}{⁴}$");
        assert_eq!(result, Some((None, "3".to_string(), "4".to_string())));
    }

    #[test]
    fn test_parse_latex_fraction_with_subscript() {
        let result = parse_latex_fraction("$\\frac{₃}{₄}$");
        assert_eq!(result, Some((None, "3".to_string(), "4".to_string())));
    }

    #[test]
    fn test_parse_latex_fraction_mixed_with_superscript() {
        let result = parse_latex_fraction("$³\\frac{¹}{⁶}$");
        assert_eq!(
            result,
            Some((Some("3".to_string()), "1".to_string(), "6".to_string()))
        );
    }

    #[test]
    fn test_parse_latex_fraction_no_starting_dollar() {
        assert_eq!(parse_latex_fraction("\\frac{3}{4}$"), None);
    }

    #[test]
    fn test_parse_latex_fraction_no_ending_dollar() {
        assert_eq!(parse_latex_fraction("$\\frac{3}{4}"), None);
    }

    #[test]
    fn test_parse_latex_fraction_no_backslash() {
        assert_eq!(parse_latex_fraction("$frac{3}{4}$"), None);
    }

    #[test]
    fn test_parse_latex_fraction_wrong_command() {
        assert_eq!(
            parse_latex_fraction("$\\frac{3}{4}$"),
            Some((None, "3".to_string(), "4".to_string()))
        );
        assert_eq!(parse_latex_fraction("$\\fracx{3}{4}$"), None);
    }

    #[test]
    fn test_parse_latex_fraction_empty_numerator() {
        assert_eq!(parse_latex_fraction("$\\frac{}{4}$"), None);
    }

    #[test]
    fn test_parse_latex_fraction_empty_denominator() {
        assert_eq!(parse_latex_fraction("$\\frac{3}{}$"), None);
    }

    #[test]
    fn test_parse_latex_fraction_non_digit_numerator() {
        assert_eq!(parse_latex_fraction("$\\frac{a}{4}$"), None);
    }

    #[test]
    fn test_parse_latex_fraction_non_digit_denominator() {
        assert_eq!(parse_latex_fraction("$\\frac{3}{b}$"), None);
    }

    #[test]
    fn test_parse_latex_fraction_no_opening_brace_numerator() {
        assert_eq!(parse_latex_fraction("$\\frac3}{4}$"), None);
    }

    #[test]
    fn test_parse_latex_fraction_no_closing_brace_numerator() {
        assert_eq!(parse_latex_fraction("$\\frac{3{4}$"), None);
    }

    #[test]
    fn test_parse_latex_fraction_extra_content_after() {
        assert_eq!(parse_latex_fraction("$\\frac{3}{4}$ extra"), None);
    }

    #[test]
    fn test_parse_latex_fraction_empty_string() {
        assert_eq!(parse_latex_fraction(""), None);
    }

    #[test]
    fn test_parse_latex_fraction_only_dollars() {
        assert_eq!(parse_latex_fraction("$$"), None);
    }

    #[test]
    fn test_parse_unicode_fraction_common() {
        assert_eq!(
            parse_unicode_fraction('½'),
            Some(("1".to_string(), "2".to_string()))
        );
        assert_eq!(
            parse_unicode_fraction('⅓'),
            Some(("1".to_string(), "3".to_string()))
        );
        assert_eq!(
            parse_unicode_fraction('⅔'),
            Some(("2".to_string(), "3".to_string()))
        );
        assert_eq!(
            parse_unicode_fraction('¼'),
            Some(("1".to_string(), "4".to_string()))
        );
        assert_eq!(
            parse_unicode_fraction('¾'),
            Some(("3".to_string(), "4".to_string()))
        );
        assert_eq!(
            parse_unicode_fraction('⅕'),
            Some(("1".to_string(), "5".to_string()))
        );
        assert_eq!(
            parse_unicode_fraction('⅖'),
            Some(("2".to_string(), "5".to_string()))
        );
        assert_eq!(
            parse_unicode_fraction('⅗'),
            Some(("3".to_string(), "5".to_string()))
        );
        assert_eq!(
            parse_unicode_fraction('⅘'),
            Some(("4".to_string(), "5".to_string()))
        );
        assert_eq!(
            parse_unicode_fraction('⅙'),
            Some(("1".to_string(), "6".to_string()))
        );
        assert_eq!(
            parse_unicode_fraction('⅚'),
            Some(("5".to_string(), "6".to_string()))
        );
        assert_eq!(
            parse_unicode_fraction('⅛'),
            Some(("1".to_string(), "8".to_string()))
        );
        assert_eq!(
            parse_unicode_fraction('⅜'),
            Some(("3".to_string(), "8".to_string()))
        );
        assert_eq!(
            parse_unicode_fraction('⅝'),
            Some(("5".to_string(), "8".to_string()))
        );
        assert_eq!(
            parse_unicode_fraction('⅞'),
            Some(("7".to_string(), "8".to_string()))
        );
    }

    #[test]
    fn test_parse_unicode_fraction_regular_chars() {
        assert_eq!(parse_unicode_fraction('a'), None);
        assert_eq!(parse_unicode_fraction('1'), None);
        assert_eq!(parse_unicode_fraction('!'), None);
        assert_eq!(parse_unicode_fraction(' '), None);
    }

    #[test]
    fn test_parse_unicode_fraction_slash() {
        assert_eq!(parse_unicode_fraction('/'), None);
    }

    #[test]
    fn test_parse_unicode_fraction_korean() {
        assert_eq!(parse_unicode_fraction('가'), None);
    }

    #[test]
    fn test_is_unicode_fraction_true() {
        assert!(is_unicode_fraction('½'));
        assert!(is_unicode_fraction('⅓'));
        assert!(is_unicode_fraction('⅔'));
        assert!(is_unicode_fraction('¼'));
        assert!(is_unicode_fraction('¾'));
        assert!(is_unicode_fraction('⅕'));
        assert!(is_unicode_fraction('⅙'));
        assert!(is_unicode_fraction('⅛'));
    }

    #[test]
    fn test_is_unicode_fraction_false() {
        assert!(!is_unicode_fraction('a'));
        assert!(!is_unicode_fraction('1'));
        assert!(!is_unicode_fraction('!'));
        assert!(!is_unicode_fraction('/'));
        assert!(!is_unicode_fraction(' '));
        assert!(!is_unicode_fraction('가'));
    }

    #[test]
    fn test_parse_unicode_fraction_slash_only() {
        assert_eq!(parse_decomposed_fraction(&FRACTION_SLASH.to_string()), None);
    }

    #[test]
    fn test_parse_unicode_fraction_non_ascii_digit_numerator() {
        let non_ascii_case = format!("a{}1", FRACTION_SLASH);
        assert_eq!(parse_decomposed_fraction(&non_ascii_case), None);
    }

    #[test]
    fn test_parse_unicode_fraction_non_ascii_digit_denominator() {
        let non_ascii_case = format!("1{}b", FRACTION_SLASH);
        assert_eq!(parse_decomposed_fraction(&non_ascii_case), None);
    }

    #[test]
    fn test_parse_unicode_fraction_multi_slash() {
        let multi_slash_case = format!("1{}2{}3", FRACTION_SLASH, FRACTION_SLASH);
        assert_eq!(parse_decomposed_fraction(&multi_slash_case), None);
    }

    #[test]
    fn test_consume_whitespace() {
        let s = "   abc";
        let mut iter = s.chars().peekable();
        consume_whitespace(&mut iter);
        assert_eq!(iter.next(), Some('a'));
    }

    #[test]
    fn test_consume_whitespace_no_whitespace() {
        let s = "abc";
        let mut iter = s.chars().peekable();
        consume_whitespace(&mut iter);
        assert_eq!(iter.next(), Some('a'));
    }

    #[test]
    fn test_consume_whitespace_only_whitespace() {
        let s = "   ";
        let mut iter = s.chars().peekable();
        consume_whitespace(&mut iter);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_consume_whitespace_tabs_newlines() {
        let s = "\t\n  abc";
        let mut iter = s.chars().peekable();
        consume_whitespace(&mut iter);
        assert_eq!(iter.next(), Some('a'));
    }

    #[test]
    fn test_read_braced_content_valid() {
        let s = "{123}";
        let mut iter = s.chars().peekable();
        let result = read_braced_content(&mut iter);
        assert_eq!(result, Some("123".to_string()));
    }

    #[test]
    fn test_read_braced_content_with_whitespace() {
        let s = "{ 1 2 3 }";
        let mut iter = s.chars().peekable();
        let result = read_braced_content(&mut iter);
        assert_eq!(result, Some("123".to_string()));
    }

    #[test]
    fn test_read_braced_content_with_superscript() {
        let s = "{¹²³}";
        let mut iter = s.chars().peekable();
        let result = read_braced_content(&mut iter);
        assert_eq!(result, Some("123".to_string()));
    }

    #[test]
    fn test_read_braced_content_empty() {
        let s = "{}";
        let mut iter = s.chars().peekable();
        let result = read_braced_content(&mut iter);
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_braced_content_no_opening_brace() {
        let s = "123}";
        let mut iter = s.chars().peekable();
        let result = read_braced_content(&mut iter);
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_braced_content_no_closing_brace() {
        let s = "{123";
        let mut iter = s.chars().peekable();
        let result = read_braced_content(&mut iter);
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_braced_content_with_non_digit() {
        let s = "{1a3}";
        let mut iter = s.chars().peekable();
        let result = read_braced_content(&mut iter);
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_braced_content_with_leading_whitespace() {
        let s = "  {123}";
        let mut iter = s.chars().peekable();
        let result = read_braced_content(&mut iter);
        assert_eq!(result, Some("123".to_string()));
    }
}
