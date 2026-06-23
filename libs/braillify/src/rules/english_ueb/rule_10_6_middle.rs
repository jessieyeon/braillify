//! §10.6.5 middle lower groupsigns `ea bb cc ff gg`, morpheme-gated (feature
//! `english_ueb_cmudict`).
//!
//! These one-cell signs are used in the *middle* of a word — a letter on both
//! sides ([`super::rule_10_6::middle_lower_groupsign`]) — but NOT when the two
//! letters straddle a morpheme boundary where each is sounded in its own
//! component (RUEB 2024 §10.6.5): `pine·apple`, `hide·away`, `lime·ade`,
//! `dumb·bell`, `sub·basement` spell out, while `oceanic`, `head`, `bubble`,
//! `accept` contract.
//!
//! Boundaries are detected from word *structure* via the CMUdict word list (not
//! phoneme identity, which cannot tell `oceanic` /…IY AE…/ from `react`
//! /…IY AE…/). A lower groupsign at `pos` is suppressed when:
//!  * §10.10 preference — a strong contraction (§10.3), strong groupsign (§10.4)
//!    or final-letter groupsign (§10.8) begins at the shared second letter and
//!    outranks it: `ear`→`ar`, `m·eander`→`and`, `aff·ord`→`for`,
//!    `bacc·hanal`→`ch`, `veng·eance`→`ance`;
//!  * (`ea`) the prefix through the `e` is a word ending in a *silent* `e`
//!    (`pine`, `hide`, `lime`), a magic-`e` boundary; or the suffix from the `a`
//!    is a standalone root word of ≥4 letters (`re·action`, `pre·amble`,
//!    `de·activate`) — short codas (`be·at`, `he·ad`) are one word;
//!  * (doubled) the word splits at the pair into two words (`dumb`+`bell`).
//!
//! Conservative by design: an unknown prefix/suffix is treated as non-boundary
//! (contract). A missed contraction is far better than a wrong one.

use super::contraction::{ContractionMatch, ContractionRule};
use super::pronunciation::PronunciationProvider;
use super::rule_10_3::StrongContractionRule;
use super::rule_10_4::StrongGroupsignRule;
use super::rule_10_6::middle_lower_groupsign;
use super::rule_10_8::FinalGroupsignRule;

/// Morpheme-gated §10.6.5 middle lower groupsign rule.
pub struct MiddleLowerGroupsignRule {
    provider: Box<dyn PronunciationProvider>,
}

impl MiddleLowerGroupsignRule {
    /// Build the rule with the word/pronunciation source used to detect boundaries.
    pub fn new(provider: Box<dyn PronunciationProvider>) -> Self {
        Self { provider }
    }

    /// `ea`: spell out on a §10.10 overlap, a magic-`e` prefix boundary, or a
    /// standalone root word from the `a`.
    fn ea_allowed(&self, word: &[char], pos: usize) -> bool {
        !outranked_at(word, pos + 1)
            && !self.has_trailing_silent_e(&word[..=pos])
            && !self.suffix_is_root_word(&word[pos + 1..])
    }

    /// Doubled letter (`bb cc ff gg`): spell out on a §10.10 overlap or at a
    /// compound boundary — the prefix completes a word AND the remainder is a
    /// real second component: both halves are dictionary words (`dumb`+`bell`),
    /// or a ≥3 prefix with a substantial ≥4 remainder whose root is outside the
    /// dictionary (`arc`+`cosine`). `bub`+`ble` (short remainder) stays one word.
    fn doubled_allowed(&self, word: &[char], pos: usize) -> bool {
        if outranked_at(word, pos + 1) {
            return false;
        }
        let prefix = &word[..=pos];
        let suffix = &word[pos + 1..];
        let boundary = self.is_word(prefix)
            && (self.is_word(suffix) || (prefix.len() >= 3 && suffix.len() >= 4));
        !boundary
    }

    /// True iff `prefix` is a recorded word spelled with a final `e` that is
    /// silent — every recorded pronunciation ends in a consonant phoneme.
    fn has_trailing_silent_e(&self, prefix: &[char]) -> bool {
        if prefix.last() != Some(&'e') {
            return false;
        }
        let prons = self.provider.pronunciations(&collect(prefix));
        !prons.is_empty()
            && prons
                .iter()
                .all(|p| p.last().is_some_and(|ph| !ph.is_vowel()))
    }

    /// True iff `suffix` is a standalone root word of ≥4 letters — a prefix+root
    /// boundary (`re·action`), as opposed to a short single-morpheme coda (`be·at`).
    fn suffix_is_root_word(&self, suffix: &[char]) -> bool {
        suffix.len() >= 4 && self.is_word(suffix)
    }

    /// True iff `chars` form a word recorded in the pronunciation source.
    fn is_word(&self, chars: &[char]) -> bool {
        !self.provider.pronunciations(&collect(chars)).is_empty()
    }
}

fn collect(chars: &[char]) -> String {
    chars.iter().collect()
}

/// §10.10 preference: whether a strong contraction (§10.3), strong groupsign
/// (§10.4) or final-letter groupsign (§10.8) begins at `at` — any of which
/// outranks a §10.6 lower groupsign and must claim the shared letter.
fn outranked_at(word: &[char], at: usize) -> bool {
    StrongContractionRule.try_match(word, at).is_some()
        || StrongGroupsignRule.try_match(word, at).is_some()
        || FinalGroupsignRule.try_match(word, at).is_some()
}

impl ContractionRule for MiddleLowerGroupsignRule {
    fn try_match(&self, word: &[char], pos: usize) -> Option<ContractionMatch> {
        let m = middle_lower_groupsign(word, pos)?;
        let allowed = match (word[pos], word[pos + 1]) {
            ('e', 'a') => self.ea_allowed(word, pos),
            (a, b) if a == b => self.doubled_allowed(word, pos),
            _ => false,
        };
        allowed.then_some(m)
    }
}

#[cfg(test)]
mod tests {
    use super::super::pronunciation::cmudict::CmuDictProvider;
    use super::*;
    use crate::unicode::decode_unicode;

    fn rule() -> MiddleLowerGroupsignRule {
        MiddleLowerGroupsignRule::new(Box::new(CmuDictProvider::new()))
    }

    fn try_at(word: &str, pos: usize) -> Option<(Vec<u8>, usize)> {
        let chars: Vec<char> = word.chars().collect();
        rule().try_match(&chars, pos).map(|m| (m.cells, m.consumed))
    }

    /// `ea` keeps its sound mid-component → contracted (⠂).
    #[rstest::rstest]
    #[case::oceanic("oceanic", 2)] // `oce` is not a word → not a boundary
    #[case::head("head", 1)] // prefix `he` ends in a vowel sound; coda `ad` < 4
    #[case::beat("beat", 1)]
    #[case::peanut("peanut", 1)] // `anut` is not a word
    fn ea_contracts(#[case] word: &str, #[case] pos: usize) {
        assert_eq!(try_at(word, pos), Some((vec![decode_unicode('⠂')], 2)));
    }

    /// `ea` across a morpheme boundary or §10.10 overlap → spelled out.
    #[rstest::rstest]
    #[case::pineapple("pineapple", 3)] // pine|apple — magic-e prefix
    #[case::hideaway("hideaway", 3)] // hide|away — magic-e prefix
    #[case::limeade("limeade", 3)] // lime|ade — magic-e prefix
    #[case::bear("bear", 1)] // ear → strong groupsign `ar`
    #[case::meander("meander", 1)] // eand → strong contraction `and`
    #[case::vengeance("vengeance", 4)] // eance → final groupsign `ance`
    #[case::reaction("reaction", 1)] // re|action — root word ≥4
    #[case::preamble("preamble", 3)] // pre|amble — root word ≥4
    fn ea_spells_out(#[case] word: &str, #[case] pos: usize) {
        assert_eq!(try_at(word, pos), None);
    }

    /// Doubled letters mid-morpheme → contracted.
    #[rstest::rstest]
    #[case::bubble("bubble", 2, '⠆')]
    #[case::accept("accept", 1, '⠒')]
    fn doubled_contracts(#[case] word: &str, #[case] pos: usize, #[case] cell: char) {
        assert_eq!(try_at(word, pos), Some((vec![decode_unicode(cell)], 2)));
    }

    /// Doubled letters across a compound boundary or §10.10 overlap → spelled out.
    #[rstest::rstest]
    #[case::dumbbell("dumbbell", 3)] // dumb|bell — compound
    #[case::subbasement("subbasement", 2)] // sub|basement — compound
    #[case::arccosine("arccosine", 2)] // arc|cosine — prefix word ≥3 (cosine not in dict)
    #[case::afford("afford", 1)] // ff|or → strong contraction `for`
    #[case::bacchanal("bacchanal", 2)] // cc|h → strong groupsign `ch`
    fn doubled_spells_out(#[case] word: &str, #[case] pos: usize) {
        assert_eq!(try_at(word, pos), None);
    }
}
