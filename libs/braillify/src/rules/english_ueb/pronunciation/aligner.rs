//! Bounded grapheme→phoneme alignment over CMUdict, for §10.7 contraction gating.
//!
//! English spelling↔sound is irregular, so to decide whether a contraction's letters
//! form a pronounced unit we align the word's letters to each recorded pronunciation
//! with a small dynamic program (each letter voices 0, 1 or 2 phonemes; two letters
//! may share one phoneme as a digraph), then inspect one letter's role.
//!
//! The only question the §10.7 gate asks is whether a contraction's TRAILING `e`
//! (`one`, `some`, `name`, `time`, `here`, `there`, `where`) is *silent* or merged
//! into an `ey` digraph — in which case the letters form the `…one`/`…ere` unit and
//! the contraction is used (`cone`, `atonement`, `honey`). When that `e` instead
//! voices its OWN vowel — a full vowel (`Monet` /…neɪ/, `phonetic` /…nɛt…/) OR an
//! unstressed schwa (`krone` /…nə/, `demonetise` /…nɪ…/) — the unit is split and we
//! spell out. Reduced-vowel cases like `demonetise` that the PDF still contracts are
//! conservatively MISSED here: CMUdict gives no morpheme/syllable boundary to tell
//! `demonetise` (contract) from `krone` (spell out), and a missed contraction is far
//! better than a wrong one.

use super::Phoneme;

/// Cost of an impossible / disallowed alignment edge. Far above any real path so it
/// never wins, yet finite to keep the saturating arithmetic simple.
const IMPOSSIBLE: u32 = 1_000_000;

fn is_vowel_letter(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}

/// Plausibility cost of a single `letter` voicing a single `phoneme`. A vowel letter
/// may voice any vowel (English spelling is chaotic) but not a consonant, and vice
/// versa — except `r`, which voices the r-coloured vowel `ER`.
fn emit_cost(letter: char, ph: &Phoneme) -> u32 {
    let vowel_ph = ph.is_vowel();
    if is_vowel_letter(letter) {
        // a vowel letter voices a vowel — or, for `y`, the consonant `Y` (`yes`).
        if vowel_ph || (letter == 'y' && ph.base == "Y") {
            0
        } else {
            IMPOSSIBLE
        }
    } else if letter == 'r' && ph.base == "ER" {
        1 // r-coloured vowel (`fever`, `here`)
    } else if vowel_ph {
        IMPOSSIBLE
    } else {
        consonant_cost(letter, &ph.base)
    }
}

/// Cost of a consonant `letter` voicing consonant phoneme `base`: 0 for a natural
/// pairing, a mild penalty otherwise (English has odd spellings, but we never forbid
/// a consonant→consonant edge so alignment rarely fails outright).
fn consonant_cost(letter: char, base: &str) -> u32 {
    let natural: &[&str] = match letter {
        'b' => &["B"],
        'c' => &["K", "S", "CH"],
        'd' => &["D", "JH", "T"],
        'f' => &["F", "V"],
        'g' => &["G", "JH", "ZH"],
        'h' => &["HH"],
        'j' => &["JH", "Y", "HH"],
        'k' => &["K"],
        'l' => &["L"],
        'm' => &["M"],
        'n' => &["N", "NG"],
        'p' => &["P"],
        'q' => &["K"],
        'r' => &["R"],
        's' => &["S", "Z", "SH", "ZH"],
        't' => &["T", "CH", "SH", "DH", "TH"],
        'v' => &["V"],
        'w' => &["W"],
        'x' => &["K", "S", "Z"],
        'z' => &["Z", "S", "ZH"],
        _ => &[],
    };
    if natural.contains(&base) { 0 } else { 3 }
}

/// Cost of `letter` voicing TWO phonemes (`x`→`K S`, `u`→`Y UW`, `q`→`K W`).
fn emit2_cost(letter: char, a: &Phoneme, b: &Phoneme) -> u32 {
    match (letter, a.base.as_str(), b.base.as_str()) {
        ('x', "K", "S") => 0,
        ('u', "Y", "UW") => 1,
        ('q', "K", "W") => 1,
        _ => IMPOSSIBLE,
    }
}

/// Cost of a two-letter grapheme `l1 l2` voicing one `phoneme`: known vowel digraphs
/// to a vowel, known consonant digraphs to their consonant.
fn digraph_cost(l1: char, l2: char, ph: &Phoneme) -> u32 {
    let key = [l1, l2];
    let vowel_digraph = matches!(
        key,
        ['o', 'o']
            | ['e', 'e']
            | ['e', 'a']
            | ['a', 'i']
            | ['a', 'y']
            | ['o', 'a']
            | ['o', 'e']
            | ['u', 'e']
            | ['u', 'i']
            | ['i', 'e']
            | ['e', 'i']
            | ['a', 'u']
            | ['a', 'w']
            | ['e', 'w']
            | ['e', 'y']
            | ['o', 'y']
            | ['o', 'u']
            | ['o', 'w']
    );
    if vowel_digraph {
        return if ph.is_vowel() { 1 } else { IMPOSSIBLE };
    }
    let consonant_digraph: Option<&[&str]> = match key {
        ['t', 'h'] => Some(&["TH", "DH"]),
        ['s', 'h'] => Some(&["SH"]),
        ['c', 'h'] => Some(&["CH", "K", "SH"]),
        ['p', 'h'] => Some(&["F"]),
        ['g', 'h'] => Some(&["G", "F"]),
        ['c', 'k'] => Some(&["K"]),
        ['n', 'g'] => Some(&["NG"]),
        ['w', 'h'] => Some(&["W", "HH"]),
        ['k', 'n'] => Some(&["N"]),
        ['w', 'r'] => Some(&["R"]),
        ['g', 'n'] => Some(&["N"]),
        ['q', 'u'] => Some(&["K"]),
        _ => None,
    };
    match consonant_digraph {
        Some(bases) if bases.contains(&ph.base.as_str()) => 0,
        _ => IMPOSSIBLE,
    }
}

/// Cost of treating `letter` as silent (voicing no phoneme): cheap for a silent
/// final/medial `e` and the classic silent consonants (`kn`ow, `gh`ost, `wr`ite),
/// dearer otherwise. `n`/`r`/`d`/`p`… are virtually never silent in a rhotic
/// dictionary, so a high cost stops the aligner buying a bogus silent-`e` reading by
/// silencing a consonant instead (`colonel` /…N AH0 L/).
fn silent_cost(letter: char) -> u32 {
    match letter {
        'e' => 1,
        'h' | 'k' | 'g' | 'w' => 2,
        'b' | 'l' | 'u' | 't' => 3,
        _ => 5,
    }
}

/// Forward DP: `f[i][j]` = min cost to align `word[0..i]` to `phonemes[0..j]`.
fn forward(word: &[char], ph: &[Phoneme]) -> Vec<Vec<u32>> {
    let (l, p) = (word.len(), ph.len());
    let mut f = vec![vec![IMPOSSIBLE; p + 1]; l + 1];
    f[0][0] = 0;
    for i in 0..=l {
        for j in 0..=p {
            let cur = f[i][j];
            if cur >= IMPOSSIBLE {
                continue;
            }
            if i < l {
                relax(&mut f[i + 1][j], cur + silent_cost(word[i]));
            }
            if i < l && j < p {
                relax(
                    &mut f[i + 1][j + 1],
                    cur.saturating_add(emit_cost(word[i], &ph[j])),
                );
            }
            if i < l && j + 1 < p {
                relax(
                    &mut f[i + 1][j + 2],
                    cur.saturating_add(emit2_cost(word[i], &ph[j], &ph[j + 1])),
                );
            }
            if i + 1 < l && j < p {
                relax(
                    &mut f[i + 2][j + 1],
                    cur.saturating_add(digraph_cost(word[i], word[i + 1], &ph[j])),
                );
            }
        }
    }
    f
}

/// Backward DP: `b[i][j]` = min cost to align `word[i..]` to `phonemes[j..]`.
fn backward(word: &[char], ph: &[Phoneme]) -> Vec<Vec<u32>> {
    let (l, p) = (word.len(), ph.len());
    let mut b = vec![vec![IMPOSSIBLE; p + 1]; l + 1];
    b[l][p] = 0;
    for i in (0..=l).rev() {
        for j in (0..=p).rev() {
            if i == l && j == p {
                continue;
            }
            let mut best = IMPOSSIBLE;
            if i < l {
                best = best.min(b[i + 1][j].saturating_add(silent_cost(word[i])));
            }
            if i < l && j < p {
                best = best.min(b[i + 1][j + 1].saturating_add(emit_cost(word[i], &ph[j])));
            }
            if i < l && j + 1 < p {
                best = best.min(b[i + 1][j + 2].saturating_add(emit2_cost(
                    word[i],
                    &ph[j],
                    &ph[j + 1],
                )));
            }
            if i + 1 < l && j < p {
                best = best.min(b[i + 2][j + 1].saturating_add(digraph_cost(
                    word[i],
                    word[i + 1],
                    &ph[j],
                )));
            }
            b[i][j] = best;
        }
    }
    b
}

fn relax(slot: &mut u32, candidate: u32) {
    if candidate < *slot {
        *slot = candidate;
    }
}

/// Whether, in this single pronunciation, letter `e_idx` is most cheaply aligned as a
/// SILENT letter or the first half of a vowel digraph (`ey`) — strictly cheaper than
/// it voicing its own vowel phoneme. A failed overall alignment ⇒ false (reject).
fn silent_or_merged_in(word: &[char], e_idx: usize, ph: &[Phoneme]) -> bool {
    let (l, p) = (word.len(), ph.len());
    let f = forward(word, ph);
    let b = backward(word, ph);
    if f[l][p] >= IMPOSSIBLE {
        return false;
    }
    let mut c_vowel = IMPOSSIBLE;
    let mut c_silent = IMPOSSIBLE;
    let mut c_merged = IMPOSSIBLE;
    for j in 0..=p {
        c_silent = c_silent.min(
            f[e_idx][j]
                .saturating_add(silent_cost(word[e_idx]))
                .saturating_add(b[e_idx + 1][j]),
        );
        if j < p && ph[j].is_vowel() {
            c_vowel = c_vowel.min(
                f[e_idx][j]
                    .saturating_add(emit_cost(word[e_idx], &ph[j]))
                    .saturating_add(b[e_idx + 1][j + 1]),
            );
            if e_idx + 1 < l {
                let d = digraph_cost(word[e_idx], word[e_idx + 1], &ph[j]);
                c_merged = c_merged.min(
                    f[e_idx][j]
                        .saturating_add(d)
                        .saturating_add(b[e_idx + 2][j + 1]),
                );
            }
        }
    }
    c_silent.min(c_merged) < c_vowel
}

/// §10.7: true iff the trailing letter at `e_idx` is silent / `ey`-merged in EVERY
/// recorded pronunciation (`prons`) — i.e. the contraction's letters form their
/// pronounced unit, so the sign is safe. Empty `prons` (unknown word) ⇒ false.
pub fn trailing_letter_is_silent_or_merged(
    word: &[char],
    e_idx: usize,
    prons: &[Vec<Phoneme>],
) -> bool {
    e_idx < word.len()
        && !prons.is_empty()
        && prons.iter().all(|p| silent_or_merged_in(word, e_idx, p))
}

/// Whether, in this single pronunciation, the `er` at (`e_idx`, `e_idx+1`) is most
/// cheaply read as a single UNSTRESSED `ER0` — strictly cheaper than a stressed `ER`
/// or the `e` voicing its own vowel.
fn er_unstressed_in(word: &[char], e_idx: usize, ph: &[Phoneme]) -> bool {
    let (l, p) = (word.len(), ph.len());
    let f = forward(word, ph);
    let b = backward(word, ph);
    if f[l][p] >= IMPOSSIBLE {
        return false;
    }
    // The `er` reads `e` silent + `r` → one phoneme: an unstressed `ER0` (`fever`) or
    // a bare `R` with the `e` elided (`several` /…V R AH0…/) — vs a STRESSED `ER1`
    // (`eversion`). Or the `e` voices its own vowel (`severity` /…EH1 R…/) — a split.
    let mut c_accept = IMPOSSIBLE;
    let mut c_reject = IMPOSSIBLE;
    for j in 0..p {
        let er = f[e_idx][j]
            .saturating_add(silent_cost('e'))
            .saturating_add(emit_cost('r', &ph[j]))
            .saturating_add(b[e_idx + 2][j + 1]);
        if matches!(ph[j].stress, Some(1) | Some(2)) {
            c_reject = c_reject.min(er);
        } else {
            c_accept = c_accept.min(er);
        }
        if ph[j].is_vowel() {
            let split = f[e_idx][j]
                .saturating_add(emit_cost('e', &ph[j]))
                .saturating_add(b[e_idx + 1][j + 1]);
            c_reject = c_reject.min(split);
        }
    }
    c_accept < c_reject
}

/// §10.7 `ever`-shape: true iff the trailing `er` (`e` at `e_idx`, `r` at `e_idx+1`)
/// voices a single UNSTRESSED `ER0` in EVERY pronunciation — the reduced `-er` ending
/// the `ever`/`father`/`mother` signs stand for (`fever` /…V ER0/). A stressed `ER1`
/// (`eversion`) or a full vowel at the `e` (`severity` /…EH1 R…/) splits the unit.
pub fn trailing_er_is_unstressed(word: &[char], e_idx: usize, prons: &[Vec<Phoneme>]) -> bool {
    e_idx + 1 < word.len()
        && !prons.is_empty()
        && prons.iter().all(|p| er_unstressed_in(word, e_idx, p))
}

#[cfg(test)]
mod tests {
    use super::super::PronunciationProvider;
    use super::super::cmudict::CmuDictProvider;
    use super::*;

    /// Verdict for the contraction's trailing `e` at `e_idx` (the span's last letter),
    /// from the real CMUdict — exactly what the §10.7 gate asks.
    fn e_silent(word: &str, e_idx: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        assert_eq!(chars[e_idx], 'e', "{word}[{e_idx}] is not the trailing e");
        let prons = CmuDictProvider::new().pronunciations(word);
        trailing_letter_is_silent_or_merged(&chars, e_idx, &prons)
    }

    /// Silent or `ey`-merged trailing `e` → the `…one`/`…ere` unit holds (contract).
    /// `e_idx` is the last letter of the contraction span.
    #[rstest::rstest]
    #[case::cone("cone", 3)] // c·one — K OW1 N, e silent
    #[case::done("done", 3)] // D AH1 N
    #[case::phone("phone", 4)] // F OW1 N
    #[case::atonement("atonement", 4)] // at·one·ment — OW1 N M, e silent before m
    #[case::lonesome("lonesome", 3)] // l·one·some — e silent before s
    #[case::honey("honey", 3)] // h·one·y — HH AH1 N IY0, ey merge
    #[case::baloney("baloney", 5)] // bal·one·y — ey merge
    #[case::adhere("adhere", 5)] // ad·here — AH0 D HH IH1 R, e silent
    fn trailing_e_is_silent(#[case] word: &str, #[case] e_idx: usize) {
        assert!(
            e_silent(word, e_idx),
            "{word}: trailing e should read silent/merged"
        );
    }

    /// A trailing `e` that voices its own vowel — a full vowel OR an unstressed schwa
    /// — splits the unit and must spell out (conservative: schwa words like
    /// `demonetise` that the PDF still contracts are safely missed, never mis-signed).
    #[rstest::rstest]
    #[case::krone("krone", 4)] // K R OW1 N AH0 — e is a (schwa) vowel
    #[case::monet("monet", 3)] // M OW0 N EY1 — e is a full vowel
    #[case::phonetic("phonetic", 4)] // … N EH1 T … — e is a stressed vowel
    #[case::anemone("anemone", 6)] // … N IY0 — e voices /i/
    #[case::demonetise("demonetise", 5)] // … N AH0 … — schwa: conservatively spelled out
    #[case::colonel("colonel", 5)] // K ER1 N AH0 L — irregular; e voices the schwa
    fn trailing_e_voices_vowel(#[case] word: &str, #[case] e_idx: usize) {
        assert!(
            !e_silent(word, e_idx),
            "{word}: trailing e voices a vowel — must spell out"
        );
    }

    /// `ever`-shape verdict for the trailing `er` at `e_idx` (its `e`), from CMUdict.
    fn er_unstressed(word: &str, e_idx: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        assert_eq!(chars[e_idx], 'e');
        let prons = CmuDictProvider::new().pronunciations(word);
        trailing_er_is_unstressed(&chars, e_idx, &prons)
    }

    /// Unstressed trailing `er` (`ER0`) → the `ever` unit holds (contract).
    #[rstest::rstest]
    #[case::fever("fever", 3)] // F IY1 V ER0
    #[case::several("several", 3)] // S EH1 V ER0 AH0 L
    #[case::beverage("beverage", 3)] // B EH1 V ER0 IH0 JH
    #[case::reverend("reverend", 3)] // R EH1 V ER0 AH0 N D
    fn ever_er_is_unstressed(#[case] word: &str, #[case] e_idx: usize) {
        assert!(
            er_unstressed(word, e_idx),
            "{word}: trailing er should be unstressed ER0"
        );
    }

    /// A stressed `ER1` (`eversion`) or a full vowel at the `e` (`severity`, `revere`)
    /// splits the unit → spell out / use `er`.
    #[rstest::rstest]
    #[case::eversion("eversion", 2)] // IH0 V ER1 … — stressed er
    #[case::severity("severity", 3)] // S AH0 V EH1 R … — e is a full vowel
    #[case::revere("revere", 3)] // R IH0 V IH1 R — e is a full vowel
    fn ever_er_splits(#[case] word: &str, #[case] e_idx: usize) {
        assert!(
            !er_unstressed(word, e_idx),
            "{word}: trailing er splits — must spell out"
        );
    }
}
