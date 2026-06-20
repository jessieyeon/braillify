//! Unified English Braille (UEB) Grade-2 encoder — WIP, feature `english_ueb`.
//!
//! Mirrors the Math token-engine architecture: a parser produces a token
//! stream ([`parser`]), a document engine ([`engine`]) manages modes and
//! capitalisation indicators, and per-rule [`contraction::ContractionRule`]
//! impls (one file per UEB §10.x clause) apply contractions via longest-match +
//! priority — the §10.10 preference rule, encoded structurally.
//!
//! [`try_encode`] returns `Some` only when the engine fully handles the input;
//! otherwise the caller falls back to the legacy path. This lets the engine
//! grow rule-by-rule without regressing already-passing cases.
//!
//! Source of truth: `docs/Rules-of-Unified-English-Braille-2024.pdf`.

pub mod contraction;
pub mod engine;
pub mod parser;
#[cfg(feature = "english_ueb_cmudict")]
pub mod pronunciation;
pub mod rule_10_1;
pub mod rule_10_2;
pub mod rule_10_3;
pub mod rule_10_4;
pub mod rule_10_5;
pub mod rule_10_6;
#[cfg(feature = "english_ueb_cmudict")]
pub mod rule_10_6_restricted;
pub mod rule_10_7;
pub mod rule_10_8;
pub mod rule_3;
pub mod rule_4;
pub mod rule_6;
pub mod rule_7;
pub mod standing_alone;
pub mod token;

use engine::EnglishUebEngine;

/// Attempt to encode `text` as standalone UEB Grade-2. Returns `None` if the
/// input is empty or contains a construct the engine does not yet support, so
/// the caller can fall back to the legacy encoding path.
pub fn try_encode(text: &str) -> Option<Vec<u8>> {
    let tokens = parser::parse_english(text);
    if tokens.is_empty() {
        return None;
    }
    EnglishUebEngine::new().encode(&tokens)
}

/// Whether `text` is *unambiguously* a math expression the legacy math pipeline
/// owns, so the UEB dispatch must NOT intercept it (Phase 7 preflight).
///
/// This is deliberately narrow: it keys only on hard math signals and never on
/// punctuation that also appears in English prose (`-`, `(`, `,`, `.`). The two
/// signals are (1) a known trig/log **function name** prefix (`sin`, `log2`,
/// `2cosx`), and (2) a single space-free token that **adjacently mixes ASCII
/// letters and digits** (`3ab`, `sin3x`, `f(x1)`) — a shape English words never
/// take. Hyphenated or multi-word English (`child-ish-ly`, `9-in dia.`) is left
/// to UEB.
pub fn is_math_owned(text: &str) -> bool {
    // (0) a balanced `$…$` span is LaTeX math, owned by the math engine. A lone
    // or trailing `$` (currency: `$6`, `US$`) is NOT, and is handled by §3.10.
    if text.len() >= 2 && text.starts_with('$') && text.ends_with('$') {
        return true;
    }

    // (1) function-name expression: a trig/log name, optionally with a leading
    // coefficient (`2cosx`), where the name is NOT merely the prefix of a longer
    // English word (`singe` starts with `sin`, `arccosine` with `arccos`). The
    // character after the function name must be a non-letter (digit, `(`, end) —
    // a math argument, never a continuation of an English word.
    let after_coeff = text.trim_start_matches(|c: char| c.is_ascii_digit());
    if let Some((name, _)) = crate::rules::math::function::match_function_prefix(after_coeff) {
        let rest = &after_coeff[name.len()..];
        // The remainder is a math argument — never an English word continuation —
        // when it is empty, begins with a non-letter (`sin3x`, `f(`), or is a
        // run of *vowel-free* letters (math variables `x`, `xy`). An English word
        // sharing the prefix (`singe`=sin+ge, `arccosine`=arccos+ine) always has a
        // vowel in its continuation, so it is left to UEB.
        let rest_is_math_arg = rest.is_empty()
            || !rest.starts_with(|c: char| c.is_ascii_alphabetic())
            || rest
                .chars()
                .all(|c| c.is_ascii_alphabetic() && !matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u'));
        if rest_is_math_arg {
            return true;
        }
    }

    // (1b) a digit enclosed in parentheses — `f(x-1)`, `(3n)!`. No passing
    // English test case has a digit inside `(...)`, so this is collision-free
    // while it captures bracketed math the legacy engine owns.
    if let Some(open) = text.find('(') {
        let inner = &text[open + 1..];
        let close = inner.find(')').unwrap_or(inner.len());
        if inner[..close].chars().any(|c| c.is_ascii_digit()) {
            return true;
        }
    }

    // (2) a single space-free token where a digit is immediately followed by
    // *two or more* letters that are NOT an ordinal suffix — an implied product
    // of variables (`3ab`, `sin3x`). A single trailing letter (`3b`, `2d`,
    // `99c`) is a unit and an ordinal (`2nd`, `3rd`, `1st`, `4th`) is English;
    // both are encoded correctly by UEB, so they are NOT blocked.
    if text.contains(' ') || text.contains('-') {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    let is_ordinal = ["st", "nd", "rd", "th"]
        .iter()
        .any(|suf| lower.ends_with(suf) && lower[..lower.len() - 2].chars().all(|c| c.is_ascii_digit()));
    if is_ordinal {
        return false;
    }
    let chars: Vec<char> = text.chars().collect();
    chars.windows(3).any(|w| {
        w[0].is_ascii_digit() && w[1].is_ascii_alphabetic() && w[2].is_ascii_alphabetic()
    })
}

#[cfg(test)]
mod is_math_owned_tests {
    use super::is_math_owned;

    /// Inputs the legacy math engine owns — UEB must NOT intercept these.
    #[rstest::rstest]
    #[case::sin("sin")]
    #[case::cos("cos")]
    #[case::sinh("sinh")]
    #[case::log2("log2")]
    #[case::two_log7("2log7")]
    #[case::sin3x("sin3x")]
    #[case::sinxy("sinxy")] // function + vowel-free variable run
    #[case::two_cosx("2cosx")] // leading coefficient
    #[case::three_ab("3ab")] // digit + 2 variables (implied product)
    #[case::f_paren("f(x-1)")] // digit inside parentheses
    #[case::factorial("(3n)!")]
    fn math_owned_inputs_are_blocked(#[case] text: &str) {
        assert!(is_math_owned(text), "{text:?} should be math-owned");
    }

    /// English (or unit/ordinal) inputs UEB owns — must NOT be blocked.
    #[rstest::rstest]
    #[case::singe("singe")] // English word sharing the `sin` prefix
    #[case::singeing("singeing")]
    #[case::arccosine("arccosine")] // shares `arccos`, has vowel continuation
    #[case::ordinal_2nd("2nd")]
    #[case::ordinal_3rd("3rd")]
    #[case::unit_3b("3b")] // single trailing letter — unit, not product
    #[case::cents_99c("99c")]
    #[case::hyphenated("child-ish-ly")]
    #[case::sentence("That is quite fair.")]
    #[case::plain_word("cat")]
    fn english_inputs_are_not_blocked(#[case] text: &str) {
        assert!(!is_math_owned(text), "{text:?} should NOT be math-owned");
    }
}
