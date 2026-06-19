//! §10.6 Lower groupsigns — `en` and `in` (the position-unrestricted subset).
//!
//! Per RUEB 2024 §10.6, the lower groupsigns `en` (⠢) and `in` (⠔) may be used
//! **anywhere** in a word (they are exempt from the lower-sign-rule placement
//! restrictions that govern `be con dis ea bb cc ff gg`). Those restricted
//! groupsigns require the lower-sign-rule primitive and are added in a later
//! phase; this file implements only the two unrestricted ones.
//!
//! Note: as a whole word, `in` is also the §10.5 lower wordsign — encoded with
//! the same cell (⠔), so handling it here as a groupsign yields identical output.

use phf::phf_map;

use super::contraction::{ContractionMatch, ContractionRule, match_longest};
use crate::unicode::decode_unicode;

static LOWER_GROUPSIGNS: phf::Map<&'static str, u8> = phf_map! {
    "en" => decode_unicode('⠢'),
    "in" => decode_unicode('⠔'),
};

/// §10.6 lower groupsign rule (unrestricted subset: `en`, `in`).
pub struct LowerGroupsignRule;

impl ContractionRule for LowerGroupsignRule {
    fn name(&self) -> &'static str {
        "10.6 lower groupsigns (en, in)"
    }

    fn try_match(&self, word: &[char], pos: usize) -> Option<ContractionMatch> {
        match_longest(word, pos, &LOWER_GROUPSIGNS, 70)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case::en("en", 0, Some((decode_unicode('⠢'), 2)))]
    #[case::in_word("find", 1, Some((decode_unicode('⠔'), 2)))]
    #[case::no_match("cat", 0, None)]
    fn matches_lower_groupsigns(
        #[case] word: &str,
        #[case] pos: usize,
        #[case] expected: Option<(u8, usize)>,
    ) {
        let chars: Vec<char> = word.chars().collect();
        let got = LowerGroupsignRule
            .try_match(&chars, pos)
            .map(|m| (m.cells[0], m.consumed));
        assert_eq!(got, expected);
    }
}
