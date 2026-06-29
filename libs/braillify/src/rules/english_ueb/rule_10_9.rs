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

/// Encode a word as the §10.10.2 cell-minimising contraction sequence.
///
/// §10.10.2 ("give preference to the groupsign which causes a word to occupy
/// fewer cells") is global cell minimisation, solved as a shortest-path DP from
/// the end of the word: `cost[pos]` is the fewest cells that can encode
/// `word[pos..]`, and `back[pos]` records the move (cells + characters consumed)
/// achieving it. Greedy longest-match fails the overlap cases — in `bastion` the
/// 2-letter `st` blocks the cheaper `s`+`tion`.
///
/// §10.10.1 ("unless other rules apply") makes the structural rules primary, so
/// before cell counts are compared [`candidate_moves`] removes the contractions
/// they forbid: the word-initial `ing` (§10.4.3), the `en` overlapping an `ence`
/// in encea/enced/encer (§10.10.6), and any generic contraction that would split
/// a morpheme/pronunciation-validated span (§10.11, via `protect_span`). Among
/// equal-cost moves the lower `priority` wins (§10.10.3–.7), then the longer match.
pub fn encode_with_longer_shortforms(
    word: &[char],
    contractions: &ContractionEngine,
    suppress_initial_ing: bool,
) -> Option<Vec<u8>> {
    let n = word.len();
    // §10.11 (§10.10.1): positions strictly inside a protected span — a
    // morpheme/pronunciation-validated contraction (`part`, a kept `in`, `con`…)
    // must not be split by a cheaper generic contraction starting inside it.
    let mut inside_protected = vec![false; n];
    let mut max_reach = vec![0usize; n];
    for start in 0..n {
        // A gated contraction pre-empted by a longer contraction starting earlier
        // and covering this position is never used, so it must not protect
        // anything: in `hea·the·nesse` the `the` (pos 3–5) covers the `e` of the
        // `e·n` at pos 5, so that `en` is never chosen and the overlapping `ness`
        // must stay free.
        let preempted = (0..start).any(|s| max_reach[s] > start);
        for m in contractions.matches_at(word, start) {
            max_reach[start] = max_reach[start].max(start + m.consumed);
            if m.protect_span && !preempted {
                let end = (start + m.consumed).min(n);
                inside_protected[(start + 1)..end].fill(true);
            }
        }
    }
    // §10.10.2 cell-minimising DP from the end of the word.
    let mut cost = vec![usize::MAX; n + 1];
    let mut back: Vec<Option<(Vec<u8>, usize)>> = vec![None; n + 1];
    cost[n] = 0;
    for pos in (0..n).rev() {
        // Best candidate so far: (total cells, priority, consumed, cells).
        let mut best: Option<(usize, u16, usize, Vec<u8>)> = None;
        for (cells, consumed, priority) in candidate_moves(
            word,
            pos,
            contractions,
            suppress_initial_ing,
            &inside_protected,
        ) {
            let next = pos + consumed;
            if next > n || cost[next] == usize::MAX {
                continue;
            }
            let total = cells.len() + cost[next];
            let better = best.as_ref().is_none_or(|(bt, bp, bc, _)| {
                total < *bt
                    || (total == *bt && priority < *bp)
                    || (total == *bt && priority == *bp && consumed > *bc)
            });
            if better {
                best = Some((total, priority, consumed, cells));
            }
        }
        let (total, _, consumed, cells) = best?;
        cost[pos] = total;
        back[pos] = Some((cells, consumed));
    }
    // Reconstruct the chosen sequence from the start.
    let mut out = Vec::with_capacity(cost[0]);
    let mut pos = 0;
    while pos < n {
        let (cells, consumed) = back[pos].as_ref()?;
        out.extend(cells.iter().copied());
        pos += consumed;
    }
    Some(out)
}

/// The candidate moves at `pos` for the §10.10.2 DP, after the §10.10.1
/// structural filters: an embedded §10.9 shortform, every §10.x contraction match
/// (minus those a structural rule forbids), and the §4.1/§4.2 single-letter
/// fallback. Each is `(cells, characters consumed, priority)`.
fn candidate_moves(
    word: &[char],
    pos: usize,
    contractions: &ContractionEngine,
    suppress_initial_ing: bool,
    inside_protected: &[bool],
) -> Vec<(Vec<u8>, usize, u16)> {
    let mut moves = Vec::new();
    // §10.9 longer-word shortform placement (preferred on a cost tie → priority 0).
    if let Some((len, cells)) = longer_match(word, pos) {
        moves.push((cells, len, 0));
    }
    let protected_here = inside_protected[pos];
    for m in contractions.matches_at(word, pos) {
        // §10.4.3: the `ing` groupsign is never used at the start of a word — drop
        // it so the `in` lower groupsign (⠔) + `g` is chosen instead.
        if suppress_initial_ing && pos == 0 && m.consumed == 3 && word.starts_with(&['i', 'n', 'g'])
        {
            continue;
        }
        // §10.10.6: use the `ence` final groupsign in `encea`/`enced`/`encer` —
        // drop the overlapping 2-cell `en` lower groupsign so `ence` (a cell tie)
        // is the one the minimiser keeps (`Sp·ence·r`, `comm·ence·d`).
        if m.consumed == 2
            && word.get(pos) == Some(&'e')
            && word.get(pos + 1) == Some(&'n')
            && is_ence_context(word, pos)
        {
            continue;
        }
        // §10.11: a generic contraction may not start strictly inside a protected
        // span, so a morpheme/pronunciation-validated contraction is never split
        // by a cheaper one (`a·part·heid`, `captain·ess`).
        if protected_here && !m.protect_span && m.consumed >= 2 {
            continue;
        }
        moves.push((m.cells, m.consumed, m.priority));
    }
    // §4.2 accent / §4.1 single letter — always available so the DP never stalls.
    if let Some(cells) = super::rule_4::accent_cells(word[pos]) {
        moves.push((cells, 1, u16::MAX));
    } else if let Ok(cell) = encode_english(word[pos]) {
        moves.push((vec![cell], 1, u16::MAX));
    }
    moves
}

/// §10.10.6: whether `pos` begins the letters-sequence `encea`/`enced`/`encer`.
fn is_ence_context(word: &[char], pos: usize) -> bool {
    word.get(pos..pos + 5).is_some_and(|s| {
        s[0] == 'e' && s[1] == 'n' && s[2] == 'c' && s[3] == 'e' && matches!(s[4], 'a' | 'd' | 'r')
    })
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
