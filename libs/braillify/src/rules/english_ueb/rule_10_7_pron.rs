//! §10.7 initial-letter contractions, pronunciation-gated (feature
//! `english_ueb_cmudict`).
//!
//! Extends the safe spelling-only set in [`super::rule_10_7`] with the
//! contractions that module defers as pronunciation-dependent. A contraction is
//! applied at a position ONLY when the whole word's CMUdict pronunciation
//! contains the standalone contraction word's pronunciation as a contiguous
//! phoneme run (comparing ARPABET base symbols, ignoring stress) — i.e. the
//! letters keep their normal sound there: `someone` /-wʌn/ → use `one`;
//! `money` /-ʌni/ and `component` → spell out. All recorded pronunciations of
//! the word must agree, mirroring the conservative §10.6 classifier: a wrong
//! contraction is far worse than a missed one, so unknown words or non-matching
//! pronunciations spell out.
//!
//! Spelling-based occurrences whose sound differs (`acknowledge`→`know` /-nɒl-/,
//! `Germany`→`many` /-ʌnɪ/) are *safely missed* by this gate (no contraction),
//! never mis-encoded.

use phf::phf_map;

use super::contraction::{ContractionMatch, ContractionRule};
use super::pronunciation::{Phoneme, PronunciationProvider};
use crate::unicode::decode_unicode;

/// §10.7 deferred initial-letter contractions → (prefix cell, first-letter cell),
/// taken from the RUEB 2024 §10.7 examples in `test_cases/english/rule_10_7_*`.
/// The unambiguous ones (`right`, `cannot`, `world`, …) already live ungated in
/// [`super::rule_10_7`]; this table holds only the pronunciation-gated remainder.
static PRON_CONTRACTIONS: phf::Map<&'static str, [u8; 2]> = phf_map! {
    // ⠐ (dot 5) prefix.
    "day"    => [decode_unicode('⠐'), decode_unicode('⠙')],
    "ever"   => [decode_unicode('⠐'), decode_unicode('⠑')],
    "father" => [decode_unicode('⠐'), decode_unicode('⠋')],
    "here"   => [decode_unicode('⠐'), decode_unicode('⠓')],
    "know"   => [decode_unicode('⠐'), decode_unicode('⠅')],
    // `lord` deferred: its phonemes appear contiguously across a morpheme
    // boundary in `chlordane` (chlor|dane), a §10.11 bridging case this gate
    // cannot detect without syllabification — a wrong contraction is worse than
    // the few missed `lordship`/`Gaylord` wins.
    "mother" => [decode_unicode('⠐'), decode_unicode('⠍')],
    "part"   => [decode_unicode('⠐'), decode_unicode('⠏')],
    "some"   => [decode_unicode('⠐'), decode_unicode('⠎')],
    "under"  => [decode_unicode('⠐'), decode_unicode('⠥')],
    "where"  => [decode_unicode('⠐'), decode_unicode('⠱')],
    "work"   => [decode_unicode('⠐'), decode_unicode('⠺')],
    "name"   => [decode_unicode('⠐'), decode_unicode('⠝')],
    "one"    => [decode_unicode('⠐'), decode_unicode('⠕')],
    "there"  => [decode_unicode('⠐'), decode_unicode('⠮')],
    "time"   => [decode_unicode('⠐'), decode_unicode('⠞')],
    "young"  => [decode_unicode('⠐'), decode_unicode('⠽')],
    // ⠘ (dots 4-5) prefix.
    "word"   => [decode_unicode('⠘'), decode_unicode('⠺')],
    "these"  => [decode_unicode('⠘'), decode_unicode('⠮')],
    "those"  => [decode_unicode('⠘'), decode_unicode('⠹')],
    "upon"   => [decode_unicode('⠘'), decode_unicode('⠥')],
    // ⠸ (dots 4-5-6) prefix.
    "many"   => [decode_unicode('⠸'), decode_unicode('⠍')],
    "their"  => [decode_unicode('⠸'), decode_unicode('⠮')],
};

/// Pronunciation-gated §10.7 initial-letter contraction rule.
pub struct InitialContractionPronunciationRule {
    provider: Box<dyn PronunciationProvider>,
}

impl InitialContractionPronunciationRule {
    /// Build the rule with the pronunciation source used to gate each contraction.
    pub fn new(provider: Box<dyn PronunciationProvider>) -> Self {
        Self { provider }
    }

    /// True iff EVERY recorded pronunciation of `full_word` contains SOME recorded
    /// pronunciation of `contraction` as a contiguous phoneme run.
    fn pronunciation_supports(&self, full_word: &str, contraction: &str) -> bool {
        let full = self.provider.pronunciations(full_word);
        let sub = self.provider.pronunciations(contraction);
        if full.is_empty() || sub.is_empty() {
            return false;
        }
        full.iter()
            .all(|fp| sub.iter().any(|sp| contains_contiguous(fp, sp)))
    }

    /// §10.7: a SUFFICIENT acceptance for a contraction whose letters end in a silent
    /// `e` (`one`, `some`, `name`, `time`, `here`, `there`, `where`, `these`,
    /// `those`). The letters form the `…one`/`…ere` unit — word-final OR medial —
    /// when the trailing `e` is silent or `ey`-merged in every recorded pronunciation
    /// (`cone`, `atonement`, `honey`); a `e` voicing its own vowel splits the unit
    /// (`krone` /…ə/, `Monet` /…eɪ/, `phonetic` /…ɛ…/) — see [`super::pronunciation::aligner`].
    /// A preceding vowel letter is rejected so the first letter never continues a
    /// vowel digraph (`B[oo]ne`, `R[oo]ney`).
    fn silent_final_e_supports(&self, word: &[char], pos: usize, key: &str, full: &str) -> bool {
        if !key.ends_with('e') {
            return false;
        }
        if pos > 0 && matches!(word[pos - 1], 'a' | 'e' | 'i' | 'o' | 'u') {
            return false;
        }
        let e_idx = pos + key.chars().count() - 1;
        // §10.10: a following `ed`/`er` strong groupsign claims the trailing `e` with
        // the next letter (`stone·d`→⠫, `ton·er`→⠻, `on·er·ous`→⠻), so the silent `e`
        // must not be swallowed into the contraction: `stoned`=st·o·n·ed,
        // `toner`=t·o·n·er — not st·one·d / t·one·r.
        if matches!(word.get(e_idx + 1), Some('d' | 'r')) {
            return false;
        }
        let prons = self.provider.pronunciations(full);
        super::pronunciation::aligner::trailing_letter_is_silent_or_merged(word, e_idx, &prons)
    }

    /// §10.7 `ever`: used where the letters `e·v·e·r` form the unstressed `-er` unit
    /// (`fever`, `several`, `beverage`) — the trailing `er` carrying no stress — but
    /// not a stressed `e·VER·sion`/`se·VER·ity`, nor across a preceding vowel digraph
    /// (`belie·ver`'s `ie`). Restricted to `ever`; `father`/`mother`/`under` are
    /// deferred (no test corpus, and `under` risks coincidental substrings like
    /// `th·under`).
    fn ever_shape_supports(&self, word: &[char], pos: usize, key: &str, full: &str) -> bool {
        if key != "ever" {
            return false;
        }
        if pos > 0 && matches!(word[pos - 1], 'a' | 'e' | 'i' | 'o' | 'u') {
            return false;
        }
        let er_e_idx = pos + 2; // the `e` of the trailing `er` in `e·v·e·r`
        let prons = self.provider.pronunciations(full);
        super::pronunciation::aligner::trailing_er_is_unstressed(word, er_e_idx, &prons)
    }
}

impl ContractionRule for InitialContractionPronunciationRule {
    fn try_match(&self, word: &[char], pos: usize) -> Option<ContractionMatch> {
        let full: String = word.iter().collect();
        let mut best: Option<(usize, [u8; 2])> = None;
        for (key, &cells) in PRON_CONTRACTIONS.entries() {
            let klen = key.chars().count();
            let end = pos + klen;
            if end > word.len()
                || !key.chars().zip(&word[pos..end]).all(|(k, w)| k == *w)
                || best.is_some_and(|(bl, _)| klen <= bl)
            {
                continue;
            }
            // The letters keep their sound here when EITHER the whole word's
            // pronunciation contains the contraction's run (`part`/`work`), OR — for
            // a silent-`e` contraction standing word-finally — the trailing `e` is
            // proven silent (`cone`, `phone`, `adhere`). Note: these §10.7
            // initial-letter contractions are whole-unit signs (priority 55), exempt
            // from §10.11 compound-seam bridging — `up·on`/`there·up·on` legitimately
            // span their own etymological seams.
            if self.pronunciation_supports(&full, key)
                || self.silent_final_e_supports(word, pos, key, &full)
                || self.ever_shape_supports(word, pos, key, &full)
            {
                best = Some((klen, cells));
            }
        }
        best.map(|(klen, cells)| ContractionMatch {
            cells: cells.to_vec(),
            consumed: klen,
            // Same band as the spelling-only §10.7 set so longest-match drives
            // the choice; below §10.4 strong groupsigns (60) on equal length.
            priority: 55,
            // §10.10.1: pronunciation-validated (`part` in `apartheid`), so the
            // cell-minimiser must not split it with a cheaper generic contraction.
            protect_span: true,
        })
    }
}

/// Whether `needle` occurs as a contiguous run inside `haystack`, comparing only
/// ARPABET base symbols (lexical stress shifts in context: `one` is W AH1 N alone
/// but W AH0 N in `anyone`).
fn contains_contiguous(haystack: &[Phoneme], needle: &[Phoneme]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|w| w.iter().zip(needle).all(|(a, b)| a.base == b.base))
}

#[cfg(test)]
mod tests {
    use super::super::pronunciation::cmudict::CmuDictProvider;
    use super::*;

    fn rule() -> InitialContractionPronunciationRule {
        InitialContractionPronunciationRule::new(Box::new(CmuDictProvider::new()))
    }

    fn try_at(word: &str, pos: usize) -> Option<(Vec<u8>, usize)> {
        let chars: Vec<char> = word.chars().collect();
        rule().try_match(&chars, pos).map(|m| (m.cells, m.consumed))
    }

    /// Pronunciation MATCHES → contraction used (the letters keep their sound).
    #[rstest::rstest]
    // partake = P AA R T EY K — `part` (P AA R T) at pos 0.
    #[case::partake("partake", 0, Some((vec![decode_unicode('⠐'), decode_unicode('⠏')], 4)))]
    // apartheid = AH P AA R T … — `part` (P AA R T) at pos 1.
    #[case::apartheid("apartheid", 1, Some((vec![decode_unicode('⠐'), decode_unicode('⠏')], 4)))]
    // network = N EH T W ER K — `work` (W ER K) at pos 3.
    #[case::network("network", 3, Some((vec![decode_unicode('⠐'), decode_unicode('⠺')], 4)))]
    fn applies_when_pronunciation_matches(
        #[case] word: &str,
        #[case] pos: usize,
        #[case] expected: Option<(Vec<u8>, usize)>,
    ) {
        assert_eq!(try_at(word, pos), expected);
    }

    /// §10.7 `ever`: the unstressed `-er` unit contracts (`fever` /…V ER0/,
    /// `several` /…V R AH0…/); a stressed `e·VER` (`eversion`) or a full vowel at the
    /// `e` (`severity`) does not.
    #[rstest::rstest]
    #[case::fever("fever", 1, Some((vec![decode_unicode('⠐'), decode_unicode('⠑')], 4)))]
    #[case::several("several", 1, Some((vec![decode_unicode('⠐'), decode_unicode('⠑')], 4)))]
    #[case::beverage("beverage", 1, Some((vec![decode_unicode('⠐'), decode_unicode('⠑')], 4)))]
    #[case::eversion("eversion", 0, None)] // e·VER stressed → use `er`
    #[case::severity("severity", 1, None)] // se·VER·ity — e is a full vowel
    fn applies_ever_when_unstressed(
        #[case] word: &str,
        #[case] pos: usize,
        #[case] expected: Option<(Vec<u8>, usize)>,
    ) {
        assert_eq!(try_at(word, pos), expected);
    }

    /// The letters split across the contraction's sound → safely spelled out.
    /// `component` voices a schwa at the `e` (`…N AH0 N T`), so the `one` unit is
    /// split; `acknowledge`'s `know` is /N AA…/, not /N OW/.
    #[rstest::rstest]
    #[case::component_not_one("component", 4)] // K AH M P OW N AH0 N T — e voices schwa
    #[case::acknowledge_not_know("acknowledge", 2)] // AE K N AA L … — `know` is N OW
    fn spells_out_when_pronunciation_differs(#[case] word: &str, #[case] pos: usize) {
        assert_eq!(try_at(word, pos), None);
    }

    /// Silent word-final `e` → the e-ending contraction is used even though the
    /// in-word vowel differs from the standalone sound (`cone` /…N/ ≠ `one` /wʌn/).
    #[rstest::rstest]
    #[case::cone("cone", 1, decode_unicode('⠕'), 3)] // c·one — word-final
    #[case::done("done", 1, decode_unicode('⠕'), 3)]
    #[case::phone("phone", 2, decode_unicode('⠕'), 3)]
    #[case::everyone("everyone", 5, decode_unicode('⠕'), 3)] // y precedes — not a digraph
    #[case::adhere("adhere", 2, decode_unicode('⠓'), 4)] // ad·here
    #[case::atonement("atonement", 2, decode_unicode('⠕'), 3)] // medial: at·one·ment
    #[case::honey("honey", 1, decode_unicode('⠕'), 3)] // medial: h·one·y (ey merge)
    #[case::money("money", 1, decode_unicode('⠕'), 3)] // m·one·y — identical to honey
    fn applies_silent_final_e(
        #[case] word: &str,
        #[case] pos: usize,
        #[case] letter: u8,
        #[case] consumed: usize,
    ) {
        assert_eq!(
            try_at(word, pos),
            Some((vec![decode_unicode('⠐'), letter], consumed))
        );
    }

    /// The silent-`e` path stays conservative: a SOUNDED final `e` (`krone` /…ə/,
    /// `anemone` /…i/), a preceding vowel digraph (`b[oo]ne`), or a medial occurrence
    /// (`money`) all spell out rather than risk a wrong contraction.
    #[rstest::rstest]
    #[case::krone("krone", 2)] // K R OW N AH0 — sounded final e
    #[case::anemone("anemone", 4)] // … N IY0 — sounded final e
    #[case::boone("boone", 2)] // b·oo·ne — o starts inside the `oo` digraph
    #[case::abalone("abalone", 4)] // … OW N IY0 — sounded final e
    #[case::colonel("colonel", 3)] // irregular K ER1 N AH0 L — e voices schwa
    #[case::stoned("stoned", 2)] // st·o·n·ed — `ed` groupsign claims e-d
    #[case::toner("toner", 1)] // t·o·n·er — `er` groupsign claims e-r
    fn silent_final_e_stays_conservative(#[case] word: &str, #[case] pos: usize) {
        assert_eq!(try_at(word, pos), None);
    }

    /// The contiguous-run helper compares ARPABET base only (stress-insensitive)
    /// and requires a true contiguous run.
    #[test]
    fn contiguous_run_compares_base_only() {
        use super::super::pronunciation::parse_phoneme;
        let hay: Vec<Phoneme> = "S AH0 M W AH1 N"
            .split_whitespace()
            .map(parse_phoneme)
            .collect();
        // `W AH N` matches despite stress differing (AH1 needle vs AH1 hay here).
        let need: Vec<Phoneme> = "W AH0 N".split_whitespace().map(parse_phoneme).collect();
        assert!(contains_contiguous(&hay, &need));
        // `M AH N` is present as letters but not as a contiguous phoneme run.
        let nope: Vec<Phoneme> = "M AH1 N".split_whitespace().map(parse_phoneme).collect();
        assert!(!contains_contiguous(&hay, &nope));
    }
}
