//! §2.6 Standing alone.
//!
//! A wordsign/contraction "stands alone" when its letters are bounded only by a
//! space, a text boundary, or permitted punctuation — and crucially NOT attached
//! as a contraction suffix (e.g. the `t` in `don't` does not stand alone, so it
//! must not take the `that` wordsign). This predicate is consulted by the
//! wordsign rules (§10.1, §10.2) before they replace a whole word with its
//! single-cell wordsign.

use super::token::EnglishToken;

/// Whether a word with the given immediate neighbours stands alone (§2.6).
///
/// A *preceding* apostrophe marks a contraction suffix (`'t`, `'s`, `'d`), so the
/// fragment after it does NOT stand alone. A *following* apostrophe is permitted
/// (`it's`, `that's`). Spaces and text boundaries are always permitted; a directly
/// adjacent word or number means the run is not isolated.
pub fn is_standing_alone(prev: Option<&EnglishToken>, next: Option<&EnglishToken>) -> bool {
    // §2.6.2: only a restricted set of *preceding* punctuation permits standing
    // alone — an opening bracket/parenthesis or an opening quotation mark (cells
    // that carry upper dots and genuinely open a fresh run). Sentence-internal
    // marks such as ellipsis/period/comma/dash do NOT (e.g. the `but` in
    // `b...but` is attached to the ellipsis on its leading side → spelled out).
    // A preceding apostrophe marks a contraction suffix (`don't`) → not alone.
    let prev_ok = match prev {
        None | Some(EnglishToken::Space) => true,
        // §2.6.1: a hyphen or dash bounds a standing-alone word (e.g. the `more`
        // in `mmm-more`, the `like` in `child-like`).
        Some(EnglishToken::Symbol('-' | '–' | '—')) => true,
        // §2.6.2: opening bracket/parenthesis or opening quotation mark.
        Some(EnglishToken::Symbol('(' | '[' | '{' | '"' | '“' | '‘')) => true,
        // Sentence-internal marks (ellipsis/period/comma) and a preceding
        // apostrophe (contraction suffix, `don't`) do NOT permit standing alone.
        Some(EnglishToken::Symbol(_)) => false,
        Some(EnglishToken::Word(_) | EnglishToken::Number(_)) => false,
    };
    // §2.6.3: a wide set of *following* punctuation still permits standing alone
    // (period, ellipsis, comma, `?`, `!`, closing brackets/quotes, apostrophe …).
    let next_ok = match next {
        None | Some(EnglishToken::Space | EnglishToken::Symbol(_)) => true,
        Some(EnglishToken::Word(_) | EnglishToken::Number(_)) => false,
    };
    prev_ok && next_ok
}

/// §10.5 (with §2.6): whether a *lower* wordsign — `be, enough, his, in, was,
/// were` — may be used given its immediate neighbours.
///
/// Lower wordsigns occupy only the bottom of the cell, so — unlike the upper
/// wordsigns (§10.1/§10.2) — they may stand alone only between *anchoring*
/// boundaries: a space, a text edge, or a bracket/parenthesis (whose cells carry
/// upper dots). A neighbouring quotation mark, apostrophe, hyphen, dash or
/// sentence-final mark — all themselves lower signs — would be ambiguous, so the
/// word is spelled out instead (e.g. `be?` → `⠃⠑⠦`, not `⠆⠦`).
pub fn lower_wordsign_usable(prev: Option<&EnglishToken>, next: Option<&EnglishToken>) -> bool {
    is_lower_anchor(prev) && is_lower_anchor(next)
}

/// A boundary that lets an adjacent lower wordsign stand alone (§10.5).
fn is_lower_anchor(boundary: Option<&EnglishToken>) -> bool {
    matches!(
        boundary,
        None | Some(EnglishToken::Space)
            | Some(EnglishToken::Symbol('(' | ')' | '[' | ']' | '{' | '}'))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_by_spaces_stands_alone() {
        // PDF §2.6.1: a word bounded by spaces stands alone.
        assert!(is_standing_alone(
            Some(&EnglishToken::Space),
            Some(&EnglishToken::Space)
        ));
    }

    #[test]
    fn text_boundaries_stand_alone() {
        assert!(is_standing_alone(None, None));
    }

    #[test]
    fn following_apostrophe_s_still_stands_alone() {
        // PDF §10.1.2: `it's` — `it` keeps the wordsign before `'s`.
        assert!(is_standing_alone(
            Some(&EnglishToken::Space),
            Some(&EnglishToken::Symbol('\''))
        ));
    }

    #[test]
    fn after_apostrophe_does_not_stand_alone() {
        // `don't` — the `t` after the apostrophe is a contraction suffix.
        assert!(!is_standing_alone(Some(&EnglishToken::Symbol('\'')), None));
    }

    #[test]
    fn adjacent_number_breaks_standing_alone() {
        // `b1` — the `b` is attached to a number, so no `but` wordsign.
        assert!(!is_standing_alone(
            None,
            Some(&EnglishToken::Number(vec!['1']))
        ));
    }

    /// §2.6.2: leading punctuation asymmetry. Opening bracket/quote and a
    /// hyphen/dash permit standing alone; sentence-internal marks (ellipsis,
    /// period, comma) preceding the word do not (the `but` in `b...but`).
    #[rstest::rstest]
    #[case::open_paren(Some(EnglishToken::Symbol('(')), true)]
    #[case::open_quote(Some(EnglishToken::Symbol('"')), true)]
    #[case::hyphen(Some(EnglishToken::Symbol('-')), true)]
    #[case::en_dash(Some(EnglishToken::Symbol('–')), true)]
    #[case::leading_period(Some(EnglishToken::Symbol('.')), false)]
    #[case::leading_comma(Some(EnglishToken::Symbol(',')), false)]
    #[case::leading_slash(Some(EnglishToken::Symbol('/')), false)]
    fn leading_punctuation_asymmetry(#[case] prev: Option<EnglishToken>, #[case] expected: bool) {
        assert_eq!(
            is_standing_alone(prev.as_ref(), Some(&EnglishToken::Space)),
            expected
        );
    }

    /// §10.5 lower-sign rule: anchored by space/edge/bracket → usable; any other
    /// lower-sign neighbour (quote, hyphen, dash, `?`, `.`) → spell out.
    #[rstest::rstest]
    #[case::spaces(Some(EnglishToken::Space), Some(EnglishToken::Space), true)]
    #[case::text_edges(None, None, true)]
    #[case::parens(Some(EnglishToken::Symbol('(')), Some(EnglishToken::Symbol(')')), true)]
    #[case::brackets(Some(EnglishToken::Symbol('[')), Some(EnglishToken::Symbol(']')), true)]
    #[case::paren_then_space(Some(EnglishToken::Symbol('(')), Some(EnglishToken::Space), true)]
    #[case::open_quote_before(Some(EnglishToken::Symbol('"')), Some(EnglishToken::Space), false)]
    #[case::question_after(Some(EnglishToken::Space), Some(EnglishToken::Symbol('?')), false)]
    #[case::hyphen_after(Some(EnglishToken::Space), Some(EnglishToken::Symbol('-')), false)]
    #[case::period_after(Some(EnglishToken::Space), Some(EnglishToken::Symbol('.')), false)]
    #[case::apostrophe_after(Some(EnglishToken::Space), Some(EnglishToken::Symbol('\'')), false)]
    fn lower_wordsign_anchoring(
        #[case] prev: Option<EnglishToken>,
        #[case] next: Option<EnglishToken>,
        #[case] expected: bool,
    ) {
        assert_eq!(
            lower_wordsign_usable(prev.as_ref(), next.as_ref()),
            expected
        );
    }
}
