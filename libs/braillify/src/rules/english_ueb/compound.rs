//! §10.11 compound-word seam data for bridging suppression.
//!
//! §10.11.1 forbids a contraction that would bridge the seam of an unhyphenated
//! compound word (`ant·hill` must spell `th`, not contract it). The seam is not
//! derivable from spelling or hyphenation alone — hyphenation cannot tell a
//! compound seam (`micro·film`) from a prefix boundary (`pro·file`), and misses
//! solid compounds entirely (`anthill` does not hyphenate). So we carry an
//! explicit list of English compound words with their seam positions.
//!
//! Source: the CompoundPiece dataset (MIT, Wiktionary-derived,
//! github.com/bminixhofer/compoundpiece) — its `positive` English entries, which
//! list genuine free-word compounds. Coincidental letter splits that are NOT
//! compounds (`father`=fat+her, `panther`=pant+her) are absent.
//!
//! A word *missing* from the table is left to contract — but that is for **lack of
//! evidence**, not because it is known safe: a genuine compound CompoundPiece omits
//! (`micro·film`) will then be *wrongly bridged*, which §10.11 treats as worse than a
//! missed contraction. So coverage is only ever **grown** toward completeness, never
//! pruned by a heuristic. Three additive sources, in order of authority:
//!  1. the static CompoundPiece table, minus an exact [`SEAM_DENYLIST`] of its
//!     individual source-data errors;
//!  2. an additive [`SUPPLEMENTAL`] list of irregular compounds the dataset omits
//!     (`twofold`, `insofar`);
//!  3. a productive [`combining_form_seam`] rule that derives the seam of a
//!     `combining-form + free-word` compound (`micro·film`, `retro·fit`) by checking
//!     the second component against the CMUdict word list ([`super::pronunciation`]).
//!
//! Every seam is derived from the word's morphology (spelling) — never from braille
//! test outputs.

use std::collections::HashMap;
use std::sync::LazyLock;

use super::pronunciation::cmudict::is_recorded_word;

/// Raw resource: `word\tseam,seam` per line, `#` comments. Compiled into the build
/// like the CMUdict table.
static COMPOUNDS_RAW: &str = include_str!("../../../resources/english_compounds.txt");

/// CompoundPiece source-data errors — words it wrongly marks as compounds. Their
/// spurious seams would suppress a *valid* contraction, so we drop them. Verified
/// against English morphology (NOT against braille outputs); each is synchronically a
/// single morpheme, so §10.11 bridging does not apply:
/// - `nightingale` (OE `nihtegale`): `nightin` is not a morpheme — `nightin·gale` is spurious.
/// - `sheriff` (`shire`+`reeve`, fully lexicalised to one morpheme): `she·riff` is spurious.
const SEAM_DENYLIST: &[&str] = &["nightingale", "sheriff"];

/// Irregular English compounds ABSENT from CompoundPiece whose seam is NOT produced
/// by the productive [`combining_form_seam`] rule (their first component is a numeral
/// or a fixed function word, not a Greek/Latin combining form). A manually-curated
/// extension of the compound lexicon — real compounds, seams from each word's
/// morphology (spelling), never from braille outputs.
const SUPPLEMENTAL: &[(&str, &[usize])] = &[
    ("twofold", &[3]),    // two·fold (numeral + `-fold`)
    ("insofar", &[2, 4]), // in·so·far (fixed adverbial compound)
];

/// Greek/Latin combining forms that productively build solid compounds with a free
/// word (`micro`+`film`, `bio`+`fuel`, `retro`+`fit`, `micro`+`wave`). Every form
/// ends in the combining vowel `o`, so the only contraction that can bridge the seam
/// is `of`/`ou`/`ow` (when the second component starts `f`/`u`/`w`) — and in that
/// position the word is a genuine compound, so suppressing the bridge is correct.
///
/// Generic prefixes that attach to *bound* roots are deliberately EXCLUDED, because
/// they over-fire on non-compounds where the contraction must stay: `pro`+`file`,
/// `con`+`fer`, `de`+`fer`, `re`+`fuse`, `in`+`fer`. The combining forms here do not
/// have that failure mode (no `micro`+`bound-root` word exists with a bridging seam).
const COMBINING_FORMS: &[&str] = &[
    "micro", "macro", "mega", "nano", "mono", "bio", "geo", "neo", "photo", "electro", "thermo",
    "hydro", "aero", "astro", "auto", "proto", "pseudo", "retro", "psycho", "socio", "ortho",
];

/// §10.11: the seam of a `combining-form + free-word` compound, or `None`. A word is
/// recognised as such a compound only when it begins with a [`COMBINING_FORMS`] entry
/// and the remaining ≥3-letter component is itself a CMUdict-recorded word
/// (`micro|film`, `retro|fit`). Conservative by design: an unrecorded remainder
/// yields `None` (the word simply falls through to contraction).
fn combining_form_seam(word: &str) -> Option<usize> {
    COMBINING_FORMS.iter().find_map(|&form| {
        // Only the matching form yields `Some(rest)`; the remainder must be a
        // substantial standalone word before we declare a seam.
        let rest = word.strip_prefix(form)?;
        (rest.len() >= 3 && is_recorded_word(rest)).then_some(form.len())
    })
}

/// word → ascending char-index seam positions (1-based offsets into the word).
static SEAMS: LazyLock<HashMap<&'static str, Vec<usize>>> = LazyLock::new(|| {
    let mut map: HashMap<&'static str, Vec<usize>> = HashMap::new();
    for line in COMPOUNDS_RAW.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((word, seams)) = line.split_once('\t') else {
            continue;
        };
        let seams: Vec<usize> = seams.split(',').filter_map(|s| s.parse().ok()).collect();
        if !word.is_empty() && !seams.is_empty() {
            map.insert(word, seams);
        }
    }
    // Delete CompoundPiece source-data errors (exact, never heuristic).
    for &bad in SEAM_DENYLIST {
        map.remove(bad);
    }
    // Add genuine compounds the dataset omits (additive only — the table only grows).
    for &(word, seams) in SUPPLEMENTAL {
        map.insert(word, seams.to_vec());
    }
    map
});

/// §10.11.1: the compound seams of `word` (lowercase) — char indices where two
/// components meet — or an empty vec when `word` is not a known compound. The static
/// table (CompoundPiece + [`SUPPLEMENTAL`]) is authoritative; only when a word is
/// absent there do we fall back to the productive [`combining_form_seam`] rule.
pub fn compound_seams(word: &str) -> Vec<usize> {
    if let Some(seams) = SEAMS.get(word) {
        return seams.clone();
    }
    combining_form_seam(word).into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known compounds expose their seam(s); their bridging digraph straddles one.
    #[rstest::rstest]
    #[case::anthill("anthill", 3)] // ant|hill — th (pos 2..4) straddles seam 3
    #[case::carthorse("carthorse", 4)] // cart|horse
    #[case::foghorn("foghorn", 3)] // fog|horn
    #[case::sweetheart("sweetheart", 5)] // sweet|heart
    fn known_compound_has_seam(#[case] word: &str, #[case] seam: usize) {
        assert!(
            compound_seams(word).contains(&seam),
            "{word} should have seam {seam}, got {:?}",
            compound_seams(word)
        );
    }

    /// Coincidental letter splits that are NOT compounds — and CompoundPiece's bogus
    /// `SEAM_DENYLIST` entries — are absent → no seam, so the contraction is never
    /// suppressed.
    #[rstest::rstest]
    #[case::father("father")]
    #[case::panther("panther")]
    #[case::profile("profile")]
    #[case::mother("mother")]
    #[case::nightingale("nightingale")] // denylisted: spurious CompoundPiece seam
    #[case::sheriff("sheriff")] // denylisted: spurious CompoundPiece seam
    fn non_compound_has_no_seam(#[case] word: &str) {
        assert!(
            compound_seams(word).is_empty(),
            "{word} must not be treated as a compound: {:?}",
            compound_seams(word)
        );
    }

    /// Irregular compounds supplied by the hand-curated `SUPPLEMENTAL` list (first
    /// component is a numeral / fixed word, not a combining form).
    #[rstest::rstest]
    #[case::twofold("twofold", 3)] // two|fold
    #[case::insofar("insofar", 4)] // in·so|far
    fn supplemental_compound_has_seam(#[case] word: &str, #[case] seam: usize) {
        assert!(
            compound_seams(word).contains(&seam),
            "{word} should have supplemental seam {seam}, got {:?}",
            compound_seams(word)
        );
    }

    /// The productive combining-form rule derives the seam of `combining-form +
    /// free-word` compounds — including ones outside any test corpus (`retrofit`,
    /// `microwave`), proving it generalises rather than fitting known cases.
    #[rstest::rstest]
    #[case::microfilm("microfilm", 5)] // micro|film — `of` bridge
    #[case::biofeedback("biofeedback", 3)] // bio|feedback
    #[case::biofuel("biofuel", 3)] // bio|fuel
    #[case::retrofit("retrofit", 5)] // retro|fit — `of` bridge
    #[case::microwave("microwave", 5)] // micro|wave — `ow` bridge
    fn combining_form_seam_is_derived(#[case] word: &str, #[case] seam: usize) {
        assert_eq!(
            combining_form_seam(word),
            Some(seam),
            "{word} should derive seam {seam}"
        );
    }

    /// The combining-form rule must NOT fire on (a) generic prefixes attaching to
    /// bound roots — `pro`/`con` are not combining forms (`pro·file` must contract);
    /// nor (b) a too-short remainder (`neo`+`n`). These keep the rule conservative.
    #[rstest::rstest]
    #[case::profile("profile")] // pro- is not a combining form
    #[case::confer("confer")] // con- is not a combining form
    #[case::neon("neon")] // neo + "n": remainder < 3 letters
    fn combining_form_rule_does_not_overfire(#[case] word: &str) {
        assert_eq!(
            combining_form_seam(word),
            None,
            "{word} must not be split by the combining-form rule"
        );
    }
}
