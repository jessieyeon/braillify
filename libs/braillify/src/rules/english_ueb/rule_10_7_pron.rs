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
}

impl ContractionRule for InitialContractionPronunciationRule {
    fn try_match(&self, word: &[char], pos: usize) -> Option<ContractionMatch> {
        let full: String = word.iter().collect();
        let mut best: Option<(usize, [u8; 2])> = None;
        for (key, &cells) in PRON_CONTRACTIONS.entries() {
            let klen = key.chars().count();
            if pos + klen <= word.len()
                && key
                    .chars()
                    .zip(&word[pos..pos + klen])
                    .all(|(k, w)| k == *w)
                && best.is_none_or(|(bl, _)| klen > bl)
                && self.pronunciation_supports(&full, key)
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

    /// Pronunciation DIFFERS (spelling-only coincidence) → safely spelled out.
    #[rstest::rstest]
    #[case::money_not_one("money", 1)] // M AH N IY — no W AH N
    #[case::component_not_one("component", 4)] // K AH M P OW N AH N T — no W AH N
    #[case::acknowledge_not_know("acknowledge", 2)] // AE K N AA L … — `know` is N OW
    fn spells_out_when_pronunciation_differs(#[case] word: &str, #[case] pos: usize) {
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
