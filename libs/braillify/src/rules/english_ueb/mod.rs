//! Unified English Braille (UEB) Grade-2 encoder.
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

pub mod compound;
pub mod contraction;
pub mod engine;
pub(crate) mod korean_context;
pub mod parser;
pub mod pronunciation;
pub mod rule_10_1;
pub mod rule_10_11;
pub mod rule_10_2;
pub mod rule_10_3;
pub mod rule_10_4;
pub mod rule_10_5;
pub mod rule_10_6;
pub mod rule_10_6_8;
pub mod rule_10_6_middle;
pub mod rule_10_6_restricted;
pub mod rule_10_7;
pub mod rule_10_7_pron;
pub mod rule_10_8;
pub mod rule_10_9;
pub mod rule_16;
pub mod rule_3;
pub mod rule_3_24;
pub mod rule_4;
pub mod rule_5_7;
pub mod rule_6;
pub mod rule_7;
pub mod rule_9;
pub mod span;
pub mod standing_alone;
pub mod token;

use engine::EnglishUebEngine;

/// Attempt to encode `text` as standalone UEB Grade-2. Returns `None` if the
/// input is empty or contains a construct the engine does not yet support, so
/// the caller can fall back to the legacy encoding path.
pub fn try_encode(text: &str) -> Option<Vec<u8>> {
    // The math pipeline NFD-decomposes accented Latin (제65항 combining marks),
    // turning `é` into `e`+◌́. UEB owns accents as precomposed letters (§4.2,
    // [`rule_4`]), so recompose (NFC) here to undo that split for the English
    // path only. Pure ASCII is NFC-stable, so non-accented inputs are unchanged.
    use unicode_normalization::UnicodeNormalization;
    let composed: String = text.nfc().collect();
    if !is_ueb_eligible(&composed) {
        return None;
    }
    let tokens = parser::parse_english(&composed);
    if tokens.is_empty() {
        return None;
    }
    EnglishUebEngine::new().encode(&tokens)
}

/// Whether `text` carries content the UEB path may own. The legacy/math path
/// keeps every input with no such signal — in particular a lone accented letter
/// (`ã`, `ä`) is an ambiguous standalone diacritic owned by 수학 제65항, and a
/// pure number/symbol run (`7:30`, `9-10`) is legacy numeric.
///
/// Two signals qualify: (1) an ASCII letter — UEB §4.2 accents only arise inside
/// an alphabetic word (`café`), which always has one; and (2) a §9 typeform
/// signal — a Mathematical-Alphanumeric styled char or a combining underline
/// (U+0332) — so a *letterless* but emphasised input (`3̲4̲`, `27.̲9`, `83%̲`) is
/// still UEB's. Korean is excluded by the callers' own `is_korean_char` guard.
pub fn is_ueb_eligible(text: &str) -> bool {
    text.chars().any(|c| {
        c.is_ascii_alphabetic()
            || c == '\u{0332}'
            || rule_9::decode_styled(c).is_some()
            // §3.10 currency: the regular-width cent/pound/yen signs are UEB's. The
            // Korean 제65항 currency rule owns the *fullwidth* forms (`￠`/`￡`/`￥`)
            // and the shared `$`/`€`/`₣`, so only these three regular code points
            // are unambiguously English and safe to route here.
            || matches!(c, '\u{00A2}' | '\u{00A3}' | '\u{00A5}')
            // §3.26 transcriber-defined symbols `฿`/`❀`. These are English-exclusive
            // (Korean 제65항 owns the shared per-mille `‰`, which is excluded).
            || matches!(c, '\u{0E3F}' | '\u{2740}')
    })
    // §16.2: a run of two or more adjacent box-drawing characters is a horizontal
    // line. A lone box char (a single mathematical `≡` or `─`) is left to the
    // legacy/math path so its non-line meaning is preserved.
    || {
        let chars: Vec<char> = text.chars().collect();
        chars
            .windows(2)
            .any(|w| rule_16::is_line_char(w[0]) && rule_16::is_line_char(w[1]))
    }
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

    // (0b) a comparison/equation operator (`=`, `<`, `>`) bound *tightly* to an
    // operand — no space on at least one side — is a math relation the legacy
    // math pipeline owns (`ax=b`, `a>b`, `x<0`, `y=f(x)`, `A={2, 4, …}`). English
    // prose always spaces these operators (`2 + 2 = 4`, `positron < posi`), so a
    // space on *both* sides leaves the input to the UEB §3.17 signs of comparison.
    let cells: Vec<char> = text.chars().collect();
    if cells.iter().enumerate().any(|(i, &c)| {
        matches!(c, '=' | '<' | '>')
            && (i.checked_sub(1).is_some_and(|j| cells[j] != ' ')
                || cells.get(i + 1).is_some_and(|&n| n != ' '))
    }) {
        return true;
    }

    // (0c) §3.24: a single space-free token carrying a Unicode super/subscript is a
    // math expression (`c²`, `x₂`, `³√x`, `log₂(x+1)`) — the same code points take
    // the 제18/19항 point shape there, not the UEB §3.24 indicator. Only a
    // multi-word English prose usage (`vitamin B₁₂`, `3 yd³`) reaches the UEB path.
    if !text.contains(' ') && text.chars().any(rule_3_24::is_script_char) {
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
        // when it is empty, begins with a non-letter (`sin3x`, `f(`), begins with
        // an *uppercase* letter (a math variable, `arcsinA`, `cosX` — an English
        // word never has a mid-word capital after a function prefix), or is a run
        // of *vowel-free* letters (math variables `x`, `xy`). An English word
        // sharing the prefix (`singe`=sin+ge, `arccosine`=arccos+ine) always has a
        // lowercase, vowel-bearing continuation, so it is left to UEB.
        let rest_is_math_arg = rest.is_empty()
            || !rest.starts_with(|c: char| c.is_ascii_alphabetic())
            || rest.starts_with(|c: char| c.is_ascii_uppercase())
            || rest.chars().all(|c| {
                c.is_ascii_alphabetic()
                    && !matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u')
            });
        if rest_is_math_arg {
            return true;
        }
    }

    // (1b) a digit enclosed in parentheses — `f(x-1)`, `(3n)!` — is bracketed math
    // the legacy engine owns, but ONLY when the content is a single space-free
    // token. A parenthetical *phrase* that contains spaces is English prose, not a
    // math argument (`(2 billion dollars)`, `(20 escudos)`, `(3 million pounds)`),
    // so it is left to UEB §3.10 currency handling.
    if let Some(open) = text.find('(') {
        let inner = &text[open + 1..];
        let close = inner.find(')').unwrap_or(inner.len());
        let bracketed = &inner[..close];
        if bracketed.chars().any(|c| c.is_ascii_digit()) && !bracketed.contains(' ') {
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
    let is_ordinal = ["st", "nd", "rd", "th"].iter().any(|suf| {
        lower.ends_with(suf) && lower[..lower.len() - 2].chars().all(|c| c.is_ascii_digit())
    });
    if is_ordinal {
        return false;
    }
    let chars: Vec<char> = text.chars().collect();
    chars
        .windows(3)
        .any(|w| w[0].is_ascii_digit() && w[1].is_ascii_alphabetic() && w[2].is_ascii_alphabetic())
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
    // §3.17: a comparison/equation operator bound tightly (no surrounding space)
    // is a math relation, not English prose.
    #[case::eq_relation("ax=b")]
    #[case::gt_relation("a>b")]
    #[case::lt_relation("x<0")]
    #[case::eq_func("y=f(x)")]
    #[case::set_eq("A={2, 4, 6, ...}")] // tight `=` even though spaces follow
    #[case::interval("-1<x<3")]
    // §3.24: a single space-free token with a Unicode super/subscript is math.
    #[case::script_c_squared("c\u{00B2}")]
    #[case::script_x_sub2("x\u{2082}")]
    #[case::script_cube_root("\u{00B3}\u{221A}x\u{00B3}")]
    #[case::script_log_sub2("log\u{2082}(x+1)")]
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
    // §3.17 prose: operators with a space on BOTH sides are English signs of
    // comparison, not a math relation (`2 + 2 = 4`, `positron < posi`).
    #[case::spaced_eq("a = b")]
    #[case::spaced_lt("positron < posi")]
    #[case::spaced_sum("as easy as 2 + 2 = 4")]
    // §3.10: a parenthetical *phrase* (spaces inside `(...)`) is prose, even with a
    // digit — not a bracketed math argument. Currency amounts gloss this way.
    #[case::paren_phrase_billion("$2bn (2 billion dollars)")]
    #[case::paren_phrase_escudos("20$00 (20 escudos)")]
    #[case::paren_phrase_pounds("\u{00A3}3m (3 million pounds)")]
    #[case::paren_phrase_enough("Buy meat (enough for 2).")]
    fn english_inputs_are_not_blocked(#[case] text: &str) {
        assert!(!is_math_owned(text), "{text:?} should NOT be math-owned");
    }
}

#[cfg(test)]
mod is_ueb_eligible_tests {
    use super::is_ueb_eligible;

    /// §3.10: regular-width cent/pound/yen are English-exclusive (Korean 제65항
    /// owns the fullwidth forms), so a letterless currency run routes to UEB.
    #[rstest::rstest]
    #[case::cent_amount("10\u{00A2}")]
    #[case::pound_amount("\u{00A3}24")]
    #[case::yen_amount("\u{00A5}360")]
    #[case::ascii_letter("cat")]
    #[case::styled_digit("3\u{0332}4")] // §9 combining-underline typeform
    fn ueb_owned_inputs_are_eligible(#[case] text: &str) {
        assert!(is_ueb_eligible(text), "{text:?} should be UEB-eligible");
    }

    /// The shared `$`/`€`/`₣` and the *fullwidth* `￠`/`￡`/`￥`/`￦` are owned by
    /// Korean 제65항 (`⠴⠈ + letter`); a letterless run of them must stay in the
    /// legacy path, so it must NOT become UEB-eligible.
    #[rstest::rstest]
    #[case::dollar("$50")]
    #[case::euro("\u{20AC}75")]
    #[case::franc("\u{20A3}1")]
    #[case::fullwidth_cent("25\u{FFE0}")]
    #[case::fullwidth_pound("\u{FFE1}88")]
    #[case::fullwidth_yen("\u{FFE5}1")]
    #[case::fullwidth_won("\u{FFE6}100")]
    fn korean_owned_currency_is_not_eligible(#[case] text: &str) {
        assert!(
            !is_ueb_eligible(text),
            "{text:?} must stay in the legacy path"
        );
    }
}
