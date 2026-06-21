//! §10.9 Shortforms.
//!
//! The table is the UEB 2024 §10.9 / Appendix 1 base shortform list.  The value
//! is written in the same ASCII braille notation used by the rulebook examples:
//! letters are alphabet cells, while symbols such as `*`, `/`, `2`, `3`, `]`,
//! `\`, and `?` denote embedded UEB contractions in the abbreviation.

use phf::phf_map;

use super::contraction::ContractionEngine;
use crate::english::encode_english;
use crate::unicode::decode_unicode;

static SHORTFORMS: phf::Map<&'static str, &'static str> = phf_map! {
    "about" => "ab", "above" => "abv", "according" => "ac", "across" => "acr",
    "after" => "af", "afternoon" => "afn", "afterward" => "afw", "again" => "ag",
    "against" => "ag/", "almost" => "alm", "already" => "alr", "also" => "al",
    "although" => "al?", "altogether" => "alt", "always" => "alw",
    "because" => "2c", "before" => "2f", "behind" => "2h", "below" => "2l",
    "beneath" => "2n", "beside" => "2s", "between" => "2t", "beyond" => "2y",
    "blind" => "bl", "braille" => "brl", "children" => "*n",
    "conceive" => "3cv", "conceiving" => "3cvg", "could" => "cd",
    "deceive" => "dcv", "deceiving" => "dcvg", "declare" => "dcl", "declaring" => "dclg",
    "either" => "ei", "first" => "f/", "friend" => "fr", "good" => "gd", "great" => "grt",
    "herself" => "h]f", "him" => "hm", "himself" => "hmf", "immediate" => "imm",
    "its" => "xs", "itself" => "xf", "letter" => "lr", "little" => "ll",
    "much" => "m*", "must" => "m/", "myself" => "myf", "necessary" => "nec",
    "neither" => "nei", "o'clock" => "o'c", "oneself" => "\"of", "ourselves" => "\\rvs",
    "paid" => "pd", "perceive" => "p]cv", "perceiving" => "p]cvg", "perhaps" => "p]h",
    "quick" => "qk", "receive" => "rcv", "receiving" => "rcvg", "rejoice" => "rjc",
    "rejoicing" => "rjcg", "said" => "sd", "should" => "%d", "such" => "s*",
    "themselves" => "!mvs", "thyself" => "?yf", "today" => "td", "together" => "tgr",
    "tomorrow" => "tm", "tonight" => "tn", "would" => "wd", "your" => "yr",
    "yourself" => "yrf", "yourselves" => "yrvs",
};

/// UEB §10.9.1: a complete shortform word standing alone uses its abbreviation.
pub fn whole_word_cells(word: &str) -> Option<Vec<u8>> {
    notation_cells(SHORTFORMS.get(word).copied()?)
}

/// A literal all-letter abbreviation that collides with a pure-letter shortform
/// needs a grade-1 indicator before normal letter encoding (§10.9.7).
pub fn is_pure_shortform_abbreviation(word: &str) -> bool {
    SHORTFORMS
        .values()
        .any(|abbr| abbr.chars().all(|ch| ch.is_ascii_lowercase()) && *abbr == word)
}

/// Encode a word using the safe §10.9.2-§10.9.3 longer-word placements currently
/// expressible without a pronunciation dictionary. Returns `None` only when the
/// fallback letter/contraction engine cannot encode a source character.
pub fn encode_with_longer_shortforms(
    word: &[char],
    contractions: &ContractionEngine,
) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(word.len());
    let mut pos = 0;
    while pos < word.len() {
        if let Some((source_len, cells)) = longer_match(word, pos) {
            out.extend(cells);
            pos += source_len;
        } else {
            let (cells, consumed) = contractions.encode_at(word, pos)?;
            out.extend(cells);
            pos += consumed;
        }
    }
    Some(out)
}

fn longer_match(word: &[char], pos: usize) -> Option<(usize, Vec<u8>)> {
    let mut best: Option<(usize, Vec<u8>)> = None;
    for (shortform, notation) in SHORTFORMS.entries() {
        let len = shortform.len();
        if pos + len <= word.len()
            && starts_with(word, pos, shortform)
            && word.len() > len
            && longer_use_allowed(word, pos, shortform)
            && best.as_ref().is_none_or(|(best_len, _)| len > *best_len)
        {
            best = Some((len, notation_cells(notation)?));
        }
    }
    best
}

fn longer_use_allowed(word: &[char], pos: usize, shortform: &str) -> bool {
    match shortform {
        "braille" | "great" => true,
        "children" => !is_followed_by_vowel_or_y(word, pos, shortform),
        "above" | "afternoon" | "afterward" => true,
        "paid" => pos > 0,
        "about" => is_about_compound(word, pos),
        "good" => pos == 0 && good_prefix_allowed(word),
        "quick" => pos == 0 && quick_prefix_allowed(word),
        "such" => pos == 0,
        "after" | "blind" | "first" | "friend" | "letter" | "little" => {
            pos == 0 && !is_followed_by_vowel_or_y(word, pos, shortform)
        }
        "below" => pos == 0 && !is_followed_by_vowel_or_y(word, pos, shortform),
        _ => false,
    }
}

fn is_about_compound(word: &[char], pos: usize) -> bool {
    matches!(
        suffix_after(word, pos, "about").as_deref(),
        Some("face" | "faced" | "facer" | "facing" | "turn" | "turned")
    ) || (pos > 0 && matches!(suffix_after(word, pos, "about").as_deref(), Some("s")))
}

fn good_prefix_allowed(word: &[char]) -> bool {
    !is_followed_by_vowel_or_y(word, 0, "good")
        || matches!(suffix_after(word, 0, "good").as_deref(), Some("afternoon"))
}

fn quick_prefix_allowed(word: &[char]) -> bool {
    !is_followed_by_vowel_or_y(word, 0, "quick")
        || matches!(suffix_after(word, 0, "quick").as_deref(), Some("er" | "ly"))
}

fn suffix_after(word: &[char], pos: usize, prefix: &str) -> Option<String> {
    let end = pos + prefix.len();
    (end <= word.len()).then(|| word[end..].iter().collect())
}

fn is_followed_by_vowel_or_y(word: &[char], pos: usize, shortform: &str) -> bool {
    word.get(pos + shortform.len())
        .is_some_and(|ch| matches!(ch, 'a' | 'e' | 'i' | 'o' | 'u' | 'y'))
}

fn starts_with(word: &[char], pos: usize, needle: &str) -> bool {
    needle
        .chars()
        .zip(&word[pos..])
        .all(|(expected, actual)| expected == *actual)
}

fn notation_cells(notation: &str) -> Option<Vec<u8>> {
    notation.chars().map(notation_cell).collect()
}

fn notation_cell(ch: char) -> Option<u8> {
    match ch {
        'a'..='z' => encode_english(ch).ok(),
        '*' => Some(decode_unicode('⠡')),
        '%' => Some(decode_unicode('⠩')),
        '?' => Some(decode_unicode('⠹')),
        '\\' => Some(decode_unicode('⠳')),
        '/' => Some(decode_unicode('⠌')),
        '2' => Some(decode_unicode('⠆')),
        '3' => Some(decode_unicode('⠒')),
        ']' => Some(decode_unicode('⠻')),
        '!' => Some(decode_unicode('⠮')),
        '"' => Some(decode_unicode('⠐')),
        '\'' => Some(decode_unicode('⠄')),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case::about("about", "⠁⠃")]
    #[case::against("against", "⠁⠛⠌")]
    #[case::because("because", "⠆⠉")]
    #[case::should("should", "⠩⠙")]
    #[case::ourselves("ourselves", "⠳⠗⠧⠎")]
    fn whole_shortform_cells_follow_standard_notation(#[case] word: &str, #[case] expected: &str) {
        let expected_cells: Vec<u8> = expected.chars().map(decode_unicode).collect();
        assert_eq!(whole_word_cells(word), Some(expected_cells));
    }

    #[rstest::rstest]
    #[case::gd("gd")]
    #[case::wd("wd")]
    #[case::rjc("rjc")]
    fn pure_abbreviations_are_collisions(#[case] word: &str) {
        assert!(is_pure_shortform_abbreviation(word));
    }
}
