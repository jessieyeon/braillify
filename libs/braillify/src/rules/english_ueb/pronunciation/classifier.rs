//! §10.6 restricted lower-groupsign classifier (`be`, `con`).
//!
//! Decides, from spelling + CMUdict pronunciation, whether the prefix forms the
//! first syllable of the word (RUEB 2024 §10.6.1). The rules are conservative:
//! anything the pronunciation cannot settle returns [`Decision::Unknown`], which
//! the caller spells out — a wrong groupsign is far worse than a missed one.

use super::{Phoneme, PronunciationProvider};

/// The restricted prefix under test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prefix {
    /// `be` lower groupsign (⠆).
    Be,
    /// `con` lower groupsign (⠒).
    Con,
    /// `dis` lower groupsign (⠲).
    Dis,
}

/// The classifier's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// The prefix is the first syllable — use the groupsign.
    Use,
    /// The prefix is not the first syllable — spell the letters out.
    SpellOut,
    /// Pronunciation is missing or ambiguous — spell out (never guess).
    Unknown,
}

/// Classify whether `word` (lowercase chars) uses the groupsign for `prefix`.
pub fn classify(word: &[char], prefix: Prefix, provider: &dyn PronunciationProvider) -> Decision {
    match prefix {
        Prefix::Be => classify_be(word, provider),
        Prefix::Con => classify_con(word, provider),
        Prefix::Dis => classify_dis(word, provider),
    }
}

fn word_string(word: &[char]) -> String {
    word.iter().collect()
}

fn is_vowel_char(c: char) -> bool {
    matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u')
}

/// Tense (free) vowels, which can end an open syllable without a coda — so a
/// primary-stressed one directly before another vowel keeps `be` open (`be·ing`
/// /biː-ɪŋ/), unlike a coda-closed `beat`/`bead`.
const TENSE_VOWELS: &[&str] = &["IY", "EY", "AY", "OW", "UW", "OY", "AW"];

/// `be`: a doubled consonant right after the prefix closes the first syllable
/// (`belligerent` = bel·lig…, CMUdict collapses the `ll`), so the prefix is not
/// a standalone syllable. Otherwise `be` is the first (open) syllable when the
/// pronunciation is `B` + a first vowel that is unstressed or secondary-stressed
/// (`become` /bɪ-/, `beneficent` /bə-/) — both pretonic — or a primary-stressed
/// tense vowel in hiatus (`be·ing`). A primary-stressed lax vowel closes the
/// syllable into `be{C}` (`beckon`, `benefit`, `bet`, `been`).
fn classify_be(word: &[char], provider: &dyn PronunciationProvider) -> Decision {
    if word.len() >= 4 && word[2] == word[3] && !is_vowel_char(word[2]) {
        return Decision::SpellOut;
    }
    // For a primary-stressed TENSE vowel, `be` is an open first syllable when a
    // CONSONANT letter follows (`be·ta` /B EY1 T…/, the `t` onsets the next
    // syllable), but a VOWEL after `be` makes a digraph the `be` is part of
    // (`bea·con`, `beat`) — those spell out.
    let consonant_follows = word.get(2).is_some_and(|c| !is_vowel_char(*c));
    decide_all(&provider.pronunciations(&word_string(word)), |p| {
        be_pron_uses(p, consonant_follows)
    })
}

fn be_pron_uses(p: &[Phoneme], consonant_follows: bool) -> bool {
    if p.first().map(|ph| ph.base.as_str()) != Some("B") {
        return false;
    }
    if p.iter().filter(|ph| ph.is_vowel()).count() < 2 {
        return false;
    }
    let Some(idx) = p.iter().position(|ph| ph.is_vowel()) else {
        return false;
    };
    match p[idx].stress {
        Some(0 | 2) => true,
        Some(1) if TENSE_VOWELS.contains(&p[idx].base.as_str()) => {
            // Open syllable: a following vowel (hiatus, `be·ing`) OR a consonant
            // letter after `be` (`be·ta`) — both keep `be` open.
            p.get(idx + 1).is_some_and(|n| n.is_vowel()) || consonant_follows
        }
        _ => false,
    }
}

/// `con`: the prefix is the first syllable when the pronunciation is `K`, a
/// vowel, then `N`/`NG` followed by a *consonant*, AND a second syllable follows
/// (`con·cept`, `con·trol`, `con·gress` /…NG G…/). A vowel after the `N` makes the
/// split `co·n…` (`coney`); a single syllable (`cone`, `conch` /K AA NG K/) is not
/// the prefix; in all those cases the groupsign is not used.
fn classify_con(word: &[char], provider: &dyn PronunciationProvider) -> Decision {
    // §10.6: like `dis`+`t`, `con` before `t`/`g` is its own first syllable
    // (`con·trol`, `con·tain`, `con·gress`, `con·gruous`) — the letter test also
    // settles the monosyllabic abbreviation `cont`/`cont.` and CMUdict gaps
    // (`congee`, `congruous`) that the multisyllable pronunciation rule rejects.
    // No `con…t`/`con…g` word in the corpus spells out (the con-monosyllables are
    // `conch`/`conk`/`cone`, i.e. con+`c`/`k`/`e`).
    if matches!(word.get(3), Some('t' | 'g')) {
        return Decision::Use;
    }
    decide_all(&provider.pronunciations(&word_string(word)), con_pron_uses)
}

fn con_pron_uses(p: &[Phoneme]) -> bool {
    let multisyllable = p.iter().filter(|ph| ph.is_vowel()).count() >= 2;
    p.len() >= 4
        && p[0].base == "K"
        && p[1].is_vowel()
        && matches!(p[2].base.as_str(), "N" | "NG")
        && !p[3].is_vowel()
        && multisyllable
}

/// `dis`: the prefix forms the first syllable when (1) the remainder after `dis`
/// is itself a standalone word — a spelling test prefixing cannot misjudge
/// (`dis·like`, `dis·honest`, `dis·play`), which settles the S-coda cases the
/// pronunciation alone cannot (`dis·like` vs `di·spirited`); or (2) the
/// pronunciation is `D IH S` then a vowel — the closed first syllable
/// (`dis·aster`, `dis·cipline`). Requiring the first vowel be `IH` excludes the
/// `di-` words (`di·sulphide` = D AY S …), and requiring a vowel after the `S`
/// excludes `di·spirited`/`disc`.
fn classify_dis(word: &[char], provider: &dyn PronunciationProvider) -> Decision {
    // A ≥2-letter remainder rules out 1-letter codas (`disc`→`c`, `dish`→`h`)
    // that some single-letter dictionary entries would otherwise match.
    if word.len() > 4 && !provider.pronunciations(&word_string(&word[3..])).is_empty() {
        return Decision::Use;
    }
    // §10.6: `dis` before `t` is conventionally its own first syllable — the `s`
    // is read as the coda of `dis` (`dis·tinct`, `dis·turb`, `dis·tance`, `dist.`),
    // unlike the `s`-cluster onset of `di·spirited` (`sp`). Spelling settles what
    // stress cannot: `distinct`/`disturbed` are pretonic (`D IH0 S T…`) just like
    // `dispirited`, yet take `dis`. No `dis…t` word in the corpus spells out.
    if word.get(3) == Some(&'t') {
        return Decision::Use;
    }
    decide_all(&provider.pronunciations(&word_string(word)), dis_pron_uses)
}

fn dis_pron_uses(p: &[Phoneme]) -> bool {
    if !(p.len() >= 4 && p[0].base == "D" && p[1].base == "IH" && p[2].base == "S") {
        return false;
    }
    // `dis` is the first syllable when a vowel onsets the next syllable after the
    // `S` (`dis·aster`, `dis·cipline`), OR the `IH` is stressed AND a second
    // syllable follows — the stressed `dis` closes its own syllable (`dis·tance`
    // /D IH1 S T…/, `dis·trict`), unlike an unstressed `di·spirited` (/D IH0 S P…/).
    let multisyllable = p.iter().filter(|ph| ph.is_vowel()).count() >= 2;
    p[3].is_vowel() || (matches!(p[1].stress, Some(1 | 2)) && multisyllable)
}

/// Every pronunciation must agree for a definite `Use`/`SpellOut`; disagreement
/// or no data yields `Unknown`.
fn decide_all<F: Fn(&[Phoneme]) -> bool>(prons: &[Vec<Phoneme>], uses: F) -> Decision {
    let Some((first, rest)) = prons.split_first() else {
        return Decision::Unknown;
    };
    let verdict = uses(first);
    if rest.iter().all(|p| uses(p) == verdict) {
        if verdict {
            Decision::Use
        } else {
            Decision::SpellOut
        }
    } else {
        Decision::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::super::{NoPronunciationProvider, Phoneme, PronunciationProvider, parse_phoneme};
    use super::*;
    use std::collections::HashMap;

    /// Test provider seeded with real CMUdict pronunciations (linguistic facts,
    /// not braille outputs) so the classifier logic is exercised standalone.
    struct Mock(HashMap<&'static str, Vec<&'static str>>);

    impl PronunciationProvider for Mock {
        fn pronunciations(&self, word: &str) -> Vec<Vec<Phoneme>> {
            self.0
                .get(word)
                .map(|v| {
                    v.iter()
                        .map(|s| s.split_whitespace().map(parse_phoneme).collect())
                        .collect()
                })
                .unwrap_or_default()
        }
    }

    fn mock() -> Mock {
        Mock(HashMap::from([
            ("become", vec!["B IH0 K AH1 M"]),
            ("begin", vec!["B IH0 G IH1 N"]),
            ("beckon", vec!["B EH1 K AH0 N"]),
            ("benefit", vec!["B EH1 N AH0 F IH0 T"]),
            ("beneficent", vec!["B EH2 N AH0 F IH1 SH AH0 N T"]),
            ("being", vec!["B IY1 IH0 NG"]),
            ("beat", vec!["B IY1 T"]),
            ("been", vec!["B IH1 N"]),
            ("belligerent", vec!["B AH0 L IH1 JH ER0 AH0 N T"]),
            ("concept", vec!["K AA1 N S EH0 P T"]),
            ("control", vec!["K AH0 N T R OW1 L"]),
            ("cone", vec!["K OW1 N"]),
            ("coney", vec!["K OW1 N IY0"]),
            ("dislike", vec!["D IH0 S L AY1 K"]),
            ("like", vec!["L AY1 K"]),
            ("discipline", vec!["D IH1 S AH0 P L IH0 N"]),
            ("dispirited", vec!["D IH0 S P IH1 R IH0 T IH0 D"]),
            ("disulphide", vec!["D AY0 S AH1 L F AY2 D"]),
            ("disc", vec!["D IH1 S K"]),
            ("congress", vec!["K AA1 NG G R AH0 S"]),
            ("conch", vec!["K AA1 NG K"]),
            ("congo", vec!["K AA1 NG G OW0"]),
            ("connor", vec!["K AA1 N ER0"]),
            ("distance", vec!["D IH1 S T AH0 N S"]),
            ("beta", vec!["B EY1 T AH0"]),
            ("beacon", vec!["B IY1 K AH0 N"]),
        ]))
    }

    fn chars(w: &str) -> Vec<char> {
        w.chars().collect()
    }

    /// Adversarial pairs whose decision is settled by pronunciation, not
    /// spelling. `become`/`beckon` and `concept`/`cone` are the canonical
    /// first-syllable contrasts. `benefit`/`beneficent` share the prefix `benef`
    /// yet differ; the conservative rule spells both out (a safe miss for
    /// `beneficent`) rather than risk contracting `benefit`.
    #[rstest::rstest]
    #[case::become_word("become", Prefix::Be, Decision::Use)]
    #[case::begin("begin", Prefix::Be, Decision::Use)]
    #[case::beckon("beckon", Prefix::Be, Decision::SpellOut)]
    #[case::benefit("benefit", Prefix::Be, Decision::SpellOut)]
    #[case::beneficent_secondary("beneficent", Prefix::Be, Decision::Use)]
    #[case::being_tense_hiatus("being", Prefix::Be, Decision::Use)]
    #[case::beat_tense_coda("beat", Prefix::Be, Decision::SpellOut)]
    #[case::been_monosyllable("been", Prefix::Be, Decision::SpellOut)]
    #[case::belligerent_doubled("belligerent", Prefix::Be, Decision::SpellOut)]
    #[case::concept("concept", Prefix::Con, Decision::Use)]
    #[case::control("control", Prefix::Con, Decision::Use)]
    #[case::cone("cone", Prefix::Con, Decision::SpellOut)]
    #[case::coney("coney", Prefix::Con, Decision::SpellOut)]
    // `dis`: rest-is-word (dis·like) and `D IH S`+vowel (dis·cipline) use it;
    // di·spirited (S+consonant), di·sulphide (first vowel AY), and disc
    // (monosyllable) spell out.
    #[case::dislike_rest_word("dislike", Prefix::Dis, Decision::Use)]
    #[case::discipline_pron("discipline", Prefix::Dis, Decision::Use)]
    #[case::dispirited("dispirited", Prefix::Dis, Decision::SpellOut)]
    #[case::disulphide("disulphide", Prefix::Dis, Decision::SpellOut)]
    #[case::disc_monosyllable("disc", Prefix::Dis, Decision::SpellOut)]
    // §10.6 NG/multisyllable con + stressed dis (regression locks):
    #[case::congress_ng("congress", Prefix::Con, Decision::Use)] // K AA NG G… multisyllable
    #[case::conch_monosyllable("conch", Prefix::Con, Decision::SpellOut)] // K AA NG K — 1 syllable
    #[case::congo_ng("congo", Prefix::Con, Decision::Use)]
    #[case::connor_vowel_after_n("connor", Prefix::Con, Decision::SpellOut)] // K AA N ER — coney split
    #[case::distance_stressed("distance", Prefix::Dis, Decision::Use)] // D IH1 S T… stressed dis
    #[case::distinct_dis_t("distinct", Prefix::Dis, Decision::Use)] // dis·t spelling rule (pretonic)
    #[case::disturb_dis_t("disturb", Prefix::Dis, Decision::Use)] // dis·t, not in mock — letter rule
    #[case::cont_con_t("cont", Prefix::Con, Decision::Use)] // con·t abbreviation, monosyllable
    #[case::congee_con_g("congee", Prefix::Con, Decision::Use)] // con·g spelling rule (dict gap)
    #[case::dispirited_not_dis_t("dispirited", Prefix::Dis, Decision::SpellOut)] // dis·p, spelled
    #[case::beta_open_consonant("beta", Prefix::Be, Decision::Use)] // be·ta — t after `be`
    #[case::beacon_digraph("beacon", Prefix::Be, Decision::SpellOut)] // bea·con — vowel digraph
    fn classifies_restricted_prefixes(
        #[case] word: &str,
        #[case] prefix: Prefix,
        #[case] expected: Decision,
    ) {
        assert_eq!(classify(&chars(word), prefix, &mock()), expected);
    }

    /// Without pronunciation data every word is `Unknown` (→ spell out), except
    /// the spelling-only doubled-consonant guard which can still say SpellOut.
    #[test]
    fn unknown_without_pronunciation() {
        assert_eq!(
            classify(&chars("become"), Prefix::Be, &NoPronunciationProvider),
            Decision::Unknown
        );
    }
}
