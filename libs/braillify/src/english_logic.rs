use crate::{symbol_shortcut, utils};

/// 규칙 33~35에서 종료표(⠲)를 생략해야 하는 기호 모음.
/// 기호 앞뒤에서는 로마자 종료표를 생략한다.
pub(crate) fn should_skip_terminator_for_symbol(symbol: char) -> bool {
    matches!(
        symbol,
        '.' | '?'
            | '!'
            | '…'
            | '⋯'
            | '"'
            | '\''
            | '”'
            | '’'
            | '」'
            | '』'
            | '〉'
            | '》'
            | '('
            | ')'
            | ']'
            | '}'
            | ','
            | ':'
            | ';'
            | '―'
    )
}

/// 종료표를 생략한 뒤에도 연속표(⠐)로 이어야 하는 기호 모음.
/// 여는 괄호 '(' 는 새 영어 구간을 열게 되므로 제외한다.
/// 종료표를 생략했지만 이어지는 로마자에 연속표를 붙여야 하는지 판단한다.
pub(crate) fn should_request_continuation(symbol: char) -> bool {
    matches!(
        symbol,
        '.' | '?'
            | '!'
            | '…'
            | '⋯'
            | '"'
            | '\''
            | '”'
            | '’'
            | '」'
            | '』'
            | '〉'
            | '》'
            | ')'
            | ']'
            | '}'
            | ','
            | ':'
            | ';'
            | '―'
    )
}

/// 제33항 [다만] : '/', '-', '~' 앞에는 종료표를 강제로 붙인다.
pub(crate) fn should_force_terminator_before_symbol(symbol: char) -> bool {
    matches!(symbol, '/' | '-' | '~' | '∼')
}

/// 영어 점자 전용 기호인지 확인.[외국어 점자 일람표의 문장 부호 참고]
pub(crate) fn is_english_symbol(symbol: char) -> bool {
    symbol_shortcut::is_english_symbol_char(symbol)
}

/// 단일 소문자 단어가 연속될 때 연속표가 필요한지 판단한다.
/// [통일 영어 점자 - 5.2 1급 점자 기호표(⠰)] : 글자 a, i, o 앞에는 1급 점자 기호표가 필요하지 않다.
pub(crate) fn requires_single_letter_continuation(letter: char) -> bool {
    letter.is_ascii_lowercase() && !matches!(letter, 'a' | 'i' | 'o')
}

fn is_ascii_letter_or_digit(ch: Option<char>) -> bool {
    ch.is_some_and(|c| c.is_ascii_alphanumeric())
}

pub(crate) fn prev_ascii_letter_or_digit(word_chars: &[char], index: usize) -> bool {
    let mut j = index;
    while j > 0 {
        let ch = word_chars[j - 1];
        if ch.is_ascii_alphanumeric() {
            return true;
        }
        if symbol_shortcut::is_english_symbol_char(ch) {
            j -= 1;
            continue;
        }
        break;
    }
    false
}

pub(crate) fn next_ascii_letter_or_digit(
    word_chars: &[char],
    index: usize,
    remaining_words: &[&str],
) -> bool {
    let mut j = index + 1;
    while j < word_chars.len() {
        let ch = word_chars[j];
        if ch.is_ascii_alphanumeric() {
            return true;
        }
        if symbol_shortcut::is_english_symbol_char(ch) {
            j += 1;
            continue;
        }
        return false;
    }

    for word in remaining_words {
        for ch in word.chars() {
            if ch.is_ascii_alphanumeric() {
                return true;
            }
            if symbol_shortcut::is_english_symbol_char(ch) {
                continue;
            }
            return false;
        }
    }

    false
}

#[allow(clippy::too_many_arguments)]
/// 괄호/쉼표가 영어 점자로 이어져야 하는지 판정한다.
/// - '(' 는 뒤에 올 문자가 ASCII 영숫자여야 하고, 앞은 한글이 아니어야 한다.
/// - ')' 는 여는 괄호가 영어 기호로 열렸던 경우에만 영어 기호로 닫는다.
/// - ',' 는 앞뒤 모두 ASCII 영숫자가 이어지는 경우에만 영어 점자로 유지한다.
pub(crate) fn should_render_symbol_as_english(
    english_indicator: bool,
    is_english: bool,
    parenthesis_stack: &[bool],
    symbol: char,
    word_chars: &[char],
    index: usize,
    remaining_words: &[&str],
) -> bool {
    if !english_indicator {
        return false;
    }

    let prev_char = if index > 0 {
        Some(word_chars[index - 1])
    } else {
        None
    };
    let next_char = if index + 1 < word_chars.len() {
        Some(word_chars[index + 1])
    } else {
        remaining_words.first().and_then(|w| w.chars().next())
    };

    match symbol {
        '(' => is_ascii_letter_or_digit(next_char) && !prev_char.is_some_and(utils::is_korean_char),
        ')' => parenthesis_stack.last().copied().unwrap_or(false),
        ',' => {
            if !is_english {
                return false;
            }

            let prev_ascii = prev_ascii_letter_or_digit(word_chars, index);
            let next_ascii = next_ascii_letter_or_digit(word_chars, index, remaining_words);

            prev_ascii && next_ascii
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_single_letter_continuation_distinguishes_letters() {
        assert!(requires_single_letter_continuation('b'));
        assert!(!requires_single_letter_continuation('a'));
        assert!(!requires_single_letter_continuation('A'));
    }

    #[test]
    fn skip_and_force_terminator_sets_are_separate() {
        for symbol in ['.', '?', '!', ')', ']', ','] {
            assert!(should_skip_terminator_for_symbol(symbol));
        }
        for symbol in ['/', '-', '~'] {
            assert!(should_force_terminator_before_symbol(symbol));
            assert!(!should_skip_terminator_for_symbol(symbol));
        }
        assert!(should_request_continuation('.'));
        assert!(!should_request_continuation('('));
    }

    #[test]
    fn english_symbol_detection_matches_lookup_table() {
        assert!(is_english_symbol('('));
        assert!(is_english_symbol(')'));
        assert!(is_english_symbol(','));
        assert!(!is_english_symbol('?'));
    }

    #[test]
    fn prev_ascii_letter_or_digit_skips_english_symbols() {
        let word: Vec<char> = "A(,B".chars().collect();
        assert!(prev_ascii_letter_or_digit(&word, 2));

        let hangul: Vec<char> = "가,".chars().collect();
        assert!(!prev_ascii_letter_or_digit(&hangul, 1));
    }

    #[test]
    fn next_ascii_letter_or_digit_checks_future_ascii() {
        let contiguous: Vec<char> = "A,B".chars().collect();
        assert!(next_ascii_letter_or_digit(&contiguous, 1, &[]));

        let with_symbol: Vec<char> = "A,(B".chars().collect();
        assert!(next_ascii_letter_or_digit(&with_symbol, 1, &[]));

        let with_remaining: Vec<char> = "A,".chars().collect();
        assert!(next_ascii_letter_or_digit(&with_remaining, 1, &["B"]));

        let hangul: Vec<char> = "A,가".chars().collect();
        assert!(!next_ascii_letter_or_digit(&hangul, 1, &[]));

        let only_symbols: Vec<char> = "A,".chars().collect();
        assert!(next_ascii_letter_or_digit(&only_symbols, 1, &["(B"]));
        assert!(!next_ascii_letter_or_digit(&only_symbols, 1, &["()"]));
    }

    #[test]
    fn should_render_symbol_as_english_for_parentheses() {
        let opener: Vec<char> = "(Hello".chars().collect();
        assert!(should_render_symbol_as_english(
            true,
            false,
            &[],
            '(',
            &opener,
            0,
            &[]
        ));

        let korean_before: Vec<char> = "가(".chars().collect();
        assert!(!should_render_symbol_as_english(
            true,
            false,
            &[],
            '(',
            &korean_before,
            1,
            &["A"]
        ));

        assert!(!should_render_symbol_as_english(
            false,
            false,
            &[],
            '(',
            &opener,
            0,
            &[]
        ));
    }

    #[test]
    fn should_render_symbol_as_english_for_closing_parenthesis() {
        let closer: Vec<char> = ")".chars().collect();
        assert!(should_render_symbol_as_english(
            true,
            true,
            &[true],
            ')',
            &closer,
            0,
            &[]
        ));

        assert!(!should_render_symbol_as_english(
            true,
            true,
            &[false],
            ')',
            &closer,
            0,
            &[]
        ));
    }

    #[test]
    fn should_render_symbol_as_english_for_comma_requires_ascii_neighbors() {
        let word: Vec<char> = "A,B".chars().collect();
        assert!(should_render_symbol_as_english(
            true,
            true,
            &[],
            ',',
            &word,
            1,
            &[]
        ));

        let no_english_context: Vec<char> = "A,B".chars().collect();
        assert!(!should_render_symbol_as_english(
            true,
            false,
            &[],
            ',',
            &no_english_context,
            1,
            &[]
        ));

        let korean_neighbor: Vec<char> = "가,B".chars().collect();
        assert!(!should_render_symbol_as_english(
            true,
            true,
            &[],
            ',',
            &korean_neighbor,
            1,
            &[]
        ));
    }
}
