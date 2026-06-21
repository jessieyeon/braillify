//! §10.4 Strong groupsigns — `ch sh th wh ou ow st ar ing ed er gh`.
//!
//! These one-cell groupsigns apply wherever the letter sequence occurs within a
//! word (no standing-alone requirement), per RUEB 2024 §10.4. Longest match
//! wins, so `ing` (3) beats any 2-letter prefix at the same position.

use phf::phf_map;

use super::contraction::{ContractionMatch, ContractionRule, match_longest};
use crate::unicode::decode_unicode;

static GROUPSIGNS: phf::Map<&'static str, u8> = phf_map! {
    "ch"  => decode_unicode('⠡'),
    "gh"  => decode_unicode('⠣'),
    "sh"  => decode_unicode('⠩'),
    "th"  => decode_unicode('⠹'),
    "wh"  => decode_unicode('⠱'),
    "ed"  => decode_unicode('⠫'),
    "er"  => decode_unicode('⠻'),
    "ou"  => decode_unicode('⠳'),
    "ow"  => decode_unicode('⠪'),
    "st"  => decode_unicode('⠌'),
    "ar"  => decode_unicode('⠜'),
    "ing" => decode_unicode('⠬'),
};

/// §10.4 strong groupsign rule.
pub struct StrongGroupsignRule;

impl ContractionRule for StrongGroupsignRule {
    fn try_match(&self, word: &[char], pos: usize) -> Option<ContractionMatch> {
        match_longest(word, pos, &GROUPSIGNS, 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case::ch("ch", 0, Some((decode_unicode('⠡'), 2)))]
    #[case::th("th", 0, Some((decode_unicode('⠹'), 2)))]
    #[case::ing("ing", 0, Some((decode_unicode('⠬'), 3)))]
    #[case::st_mid("must", 2, Some((decode_unicode('⠌'), 2)))]
    #[case::no_match("xyz", 0, None)]
    fn matches_strong_groupsigns(
        #[case] word: &str,
        #[case] pos: usize,
        #[case] expected: Option<(u8, usize)>,
    ) {
        let chars: Vec<char> = word.chars().collect();
        let got = StrongGroupsignRule
            .try_match(&chars, pos)
            .map(|m| (m.cells[0], m.consumed));
        assert_eq!(got, expected);
    }
}
