//! §5.7 Grade 1 mode avoids confusion with contractions.
//!
//! §5.7.1: a grade-1 indicator (`⠰`) is required before a single letter that
//! would otherwise be misread as an alphabetic wordsign (§10.1) — that is, when
//! the letter "stands alone" per §2.6. §5.8.1 places the indicator before any
//! capital indicator (handled by the caller).
//!
//! "Standing alone" (§2.6): after stripping the §2.6.2 opening set on the left
//! and the §2.6.3 closing/terminal set on the right, both sides reach a §2.6.1
//! boundary — a space, hyphen, dash or text edge.
//!
//! Two project guards refine pure §2.6, matching the Korean PDF §28 roman
//! handling and the maths section:
//! 1. A *bare* space/edge-bounded letter keeps the plain §4.1 cell (a punctuation
//!    neighbour is required), so `a b` / a lone `b` are not indicated.
//! 2. `!`/`?` are NOT treated as transparent, so a maths factorial (`x!`, `n!`)
//!    keeps the bare variable cell.
//!
//! The §2.6 left/right asymmetry (a period is right- but not left-transparent)
//! keeps `U.S.A.`/`p.7` bare while `p. 7`, `Dr J. F.`, `b)`, `-s`, `J----` and
//! `"g"` are indicated; §2.6.4 keeps a following contraction apostrophe (`X'll`,
//! `p's`, `maitre d'`) standing alone while a contraction *prefix* (`d'you`,
//! `e'er`, `G'day`) is not.

use super::token::EnglishToken;

/// §10.1: the alphabetic wordsign letters are every letter except a, i, o (whose
/// cells carry no contraction, so they never need a grade-1 indicator).
pub fn is_wordsign_letter(c: char) -> bool {
    c.is_ascii_alphabetic() && !matches!(c.to_ascii_lowercase(), 'a' | 'i' | 'o')
}

/// §2.6.1: a boundary that lets a letter stand alone — space, hyphen, dash, edge.
fn is_boundary(t: Option<&EnglishToken>) -> bool {
    matches!(
        t,
        None | Some(EnglishToken::Space)
            | Some(EnglishToken::Symbol('-' | '\u{2013}' | '\u{2014}'))
    )
}

/// §2.6.2: symbols transparent on the LEFT of the letter (opening brackets and
/// quotation marks, plus the apostrophe).
fn is_left_transparent(c: char) -> bool {
    matches!(c, '(' | '[' | '{' | '"' | '\u{201C}' | '\u{2018}' | '\'')
}

/// §2.6.3: symbols transparent on the RIGHT (closing brackets/quotes and terminal
/// punctuation). `!` and `?` are deliberately excluded (project guard 2).
fn is_right_transparent(c: char) -> bool {
    matches!(
        c,
        ')' | ']' | '}' | '"' | '\u{201D}' | '\u{2019}' | '.' | ',' | ':' | ';' | '\u{2026}'
    )
}

/// A standard English contraction suffix after an apostrophe (`'s`, `'ll`, `'re`,
/// `'ve`, `'d`, `'t`, `'m`) — §2.6.4 leaves the preceding letter standing alone.
fn is_contraction_suffix(w: &[char]) -> bool {
    let lower: Vec<char> = w.iter().map(|c| c.to_ascii_lowercase()).collect();
    matches!(
        lower.as_slice(),
        ['s'] | ['l', 'l'] | ['r', 'e'] | ['v', 'e'] | ['d'] | ['t'] | ['m']
    )
}

/// §5.7.1: whether the single-letter word at `tokens[i]` needs a grade-1
/// indicator. Returns `false` for any token that is not a single wordsign letter.
pub fn needs_grade1_indicator(tokens: &[EnglishToken], i: usize) -> bool {
    let Some(EnglishToken::Word(chars)) = tokens.get(i) else {
        return false;
    };
    if chars.len() != 1 || !is_wordsign_letter(chars[0]) {
        return false;
    }
    let prev = i.checked_sub(1).map(|p| &tokens[p]);
    let next = tokens.get(i + 1);
    // Guard 1: a bare space/edge-bounded letter is the §4.1 letter, not a wordsign
    // risk, so require at least one punctuation neighbour.
    if !matches!(prev, Some(EnglishToken::Symbol(_)))
        && !matches!(next, Some(EnglishToken::Symbol(_)))
    {
        return false;
    }
    // Left boundary: strip §2.6.2 openers, then require a §2.6.1 boundary.
    let mut l = i;
    while l > 0 && matches!(&tokens[l - 1], EnglishToken::Symbol(c) if is_left_transparent(*c)) {
        l -= 1;
    }
    if !is_boundary(l.checked_sub(1).map(|p| &tokens[p])) {
        return false;
    }
    // §2.6.4: a following apostrophe that introduces a contraction suffix (or ends
    // the text) leaves the letter standing alone; an apostrophe before a full word
    // is a contraction prefix, so the letter does not stand alone.
    if matches!(next, Some(EnglishToken::Symbol('\''))) {
        return match tokens.get(i + 2) {
            None => true,
            Some(EnglishToken::Word(w)) => is_contraction_suffix(w),
            _ => false,
        };
    }
    // Right boundary: strip §2.6.3 closers/terminal punctuation, then require a
    // §2.6.1 boundary.
    let mut r = i;
    while r + 1 < tokens.len()
        && matches!(&tokens[r + 1], EnglishToken::Symbol(c) if is_right_transparent(*c))
    {
        r += 1;
    }
    is_boundary(tokens.get(r + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(s: &str) -> EnglishToken {
        EnglishToken::Word(s.chars().collect())
    }
    fn num(s: &str) -> EnglishToken {
        EnglishToken::Number(s.chars().collect())
    }
    fn sym(c: char) -> EnglishToken {
        EnglishToken::Symbol(c)
    }

    /// The grade-1 indicator fires for a single wordsign letter standing alone
    /// abutting punctuation, and stays off for bare letters, `!`/`?`, abbreviation
    /// dots, attached brackets and contraction prefixes. Index `1` is the letter.
    #[rstest::rstest]
    // Indicated:
    #[case::after_hyphen(vec![word("b"), sym('-'), num("1")], 0, true)]
    #[case::before_close_paren(vec![word("in"), EnglishToken::Space, word("b"), sym(')')], 2, true)]
    #[case::free_paren(vec![sym('('), word("h"), sym(')')], 1, true)]
    #[case::period_then_space(vec![word("p"), sym('.'), EnglishToken::Space, num("7")], 0, true)]
    #[case::colon_ends_run(vec![word("d"), sym(':')], 0, true)]
    #[case::free_quote(vec![sym('"'), word("g"), sym('"')], 1, true)]
    #[case::apostrophe_suffix_ll(vec![word("X"), sym('\''), word("ll")], 0, true)]
    #[case::apostrophe_suffix_s(vec![word("p"), sym('\''), word("s")], 0, true)]
    #[case::apostrophe_at_end(vec![word("d"), sym('\'')], 0, true)]
    // Not indicated:
    #[case::bare_space_bounded(vec![word("a"), EnglishToken::Space, word("b")], 2, false)]
    #[case::bare_alone(vec![word("b")], 0, false)]
    #[case::aio_excluded(vec![sym('('), word("i"), sym(')')], 1, false)]
    #[case::factorial(vec![word("x"), sym('!')], 0, false)]
    #[case::abbreviation_dot_digit(vec![word("p"), sym('.'), num("7")], 0, false)]
    #[case::abbreviation_dot_letter(vec![word("U"), sym('.'), word("S"), sym('.')], 0, false)]
    #[case::attached_paren(vec![word("noun"), sym('('), word("s"), sym(')')], 2, false)]
    #[case::before_open_paren(vec![word("p"), sym('('), word("en"), sym(')')], 0, false)]
    #[case::contraction_prefix(vec![word("d"), sym('\''), word("you")], 0, false)]
    #[case::poetic_prefix(vec![word("e"), sym('\''), word("er")], 0, false)]
    fn grade1_indicator_matches_2_6(
        #[case] tokens: Vec<EnglishToken>,
        #[case] index: usize,
        #[case] expected: bool,
    ) {
        assert_eq!(needs_grade1_indicator(&tokens, index), expected);
    }
}
