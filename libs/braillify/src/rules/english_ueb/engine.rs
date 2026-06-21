//! Document-level UEB Grade-2 engine.
//!
//! Walks the token stream, applies §8 capitalisation indicators, and delegates
//! intra-word contraction to the [`ContractionEngine`]. Returns `None` for any
//! construct not yet supported, so the caller can fall back to the legacy path
//! (this is what keeps the engine safe to grow rule-by-rule).

use super::contraction::ContractionEngine;
use super::rule_10_3::StrongContractionRule;
use super::rule_10_4::StrongGroupsignRule;
use super::rule_10_6::LowerGroupsignRule;
use super::standing_alone::{is_standing_alone, lower_wordsign_usable};
use super::token::EnglishToken;
use crate::unicode::decode_unicode;

/// ⠠ dot-6 — UEB capital indicator (§8).
const CAPITAL: u8 = decode_unicode('⠠');
/// ⠰ dots-5-6 — UEB grade-1 indicator (§5/§6.5).
const GRADE1: u8 = decode_unicode('⠰');
/// ⠦ — opening double quotation mark (§7.6).
const QUOTE_OPEN: u8 = decode_unicode('⠦');
/// ⠴ — closing double quotation mark (§7.6).
const QUOTE_CLOSE: u8 = decode_unicode('⠴');
/// Braille space cell.
const SPACE: u8 = 0;

/// Capitalisation pattern of a word (§8 subset currently supported).
enum Caps {
    /// All lowercase — no indicator.
    None,
    /// One leading capital, or a single capital letter — `⠠`.
    Single,
    /// Whole word uppercase (len ≥ 2) — `⠠⠠`.
    Word,
}

/// Determine the capitalisation pattern, or `None` for mixed-case words
/// (e.g. "McDonald") which need per-letter indicators not yet implemented.
fn classify_caps(chars: &[char]) -> Option<Caps> {
    let uppers = chars.iter().filter(|c| c.is_ascii_uppercase()).count();
    if uppers == 0 {
        Some(Caps::None)
    } else if uppers == chars.len() {
        if chars.len() == 1 {
            Some(Caps::Single)
        } else {
            Some(Caps::Word)
        }
    } else if uppers == 1 && chars[0].is_ascii_uppercase() {
        Some(Caps::Single)
    } else {
        None
    }
}

/// A word whose letters are all uppercase (≥1 upper, no lowercase) — `A`, `NEW`.
fn is_caps_word(t: &EnglishToken) -> bool {
    matches!(t, EnglishToken::Word(c)
        if c.iter().any(|x| x.is_ascii_uppercase()) && !c.iter().any(|x| x.is_ascii_lowercase()))
}

/// A word containing any lowercase letter (breaks a §8.4 capitals passage).
fn has_lower_word(t: &EnglishToken) -> bool {
    matches!(t, EnglishToken::Word(c) if c.iter().any(|x| x.is_ascii_lowercase()))
}

/// Identify §8.4 capitals passages: runs of three or more space-separated
/// all-caps "words". Returns per-token flags — emit `⠠⠠⠠` *before* a token,
/// emit the `⠠⠄` terminator *after* a token, and whether a token lies *inside*
/// a passage (so caps words drop their own indicator). Below the 3-word
/// threshold every flag stays false, so 1–2 caps-word inputs are untouched.
fn caps_passages(tokens: &[EnglishToken]) -> (Vec<bool>, Vec<bool>, Vec<bool>) {
    let n = tokens.len();
    let (mut starts, mut terms, mut inside) = (vec![false; n], vec![false; n], vec![false; n]);

    // Space-separated groups, as inclusive `[first, last]` token ranges.
    let mut groups: Vec<(usize, usize)> = Vec::new();
    let mut g0: Option<usize> = None;
    for (i, t) in tokens.iter().enumerate() {
        if matches!(t, EnglishToken::Space) {
            if let Some(s) = g0.take() {
                groups.push((s, i - 1));
            }
        } else if g0.is_none() {
            g0 = Some(i);
        }
    }
    if let Some(s) = g0 {
        groups.push((s, n - 1));
    }

    let group_has_lower = |&(s, e): &(usize, usize)| tokens[s..=e].iter().any(has_lower_word);
    let group_is_caps =
        |g: &(usize, usize)| !group_has_lower(g) && tokens[g.0..=g.1].iter().any(is_caps_word);

    let mut gi = 0;
    while gi < groups.len() {
        if group_is_caps(&groups[gi]) {
            let first = groups[gi].0;
            let mut last = groups[gi].1;
            let mut count = 1;
            let mut gj = gi + 1;
            while gj < groups.len() && !group_has_lower(&groups[gj]) {
                if group_is_caps(&groups[gj]) {
                    last = groups[gj].1;
                    count += 1;
                }
                gj += 1;
            }
            if count >= 3 {
                starts[first] = true;
                terms[last] = true;
                for f in &mut inside[first..=last] {
                    *f = true;
                }
            }
            gi = gj;
        } else {
            gi += 1;
        }
    }
    (starts, terms, inside)
}

/// Document-level UEB Grade-2 encoder.
pub struct EnglishUebEngine {
    contractions: ContractionEngine,
}

impl Default for EnglishUebEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl EnglishUebEngine {
    /// Build the engine with the currently-implemented contraction rules.
    pub fn new() -> Self {
        let mut contractions = ContractionEngine::default();
        contractions.register(Box::new(StrongContractionRule));
        contractions.register(Box::new(StrongGroupsignRule));
        contractions.register(Box::new(LowerGroupsignRule));
        contractions.register(Box::new(super::rule_10_7::InitialContractionRule));
        contractions.register(Box::new(super::rule_10_8::FinalGroupsignRule));
        // §10.6 restricted groupsigns (be/con) need pronunciation data to judge
        // the first syllable, so they exist only under `english_ueb_cmudict`.
        #[cfg(feature = "english_ueb_cmudict")]
        contractions.register(Box::new(
            super::rule_10_6_restricted::RestrictedLowerGroupsignRule::new(Box::new(
                super::pronunciation::cmudict::CmuDictProvider::new(),
            )),
        ));
        Self { contractions }
    }

    /// Encode a token stream. Returns `None` if any token is unsupported
    /// (a number, a symbol, or a mixed-case word), so the legacy path — which
    /// handles those — takes over.
    pub fn encode(&self, tokens: &[EnglishToken]) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        let mut prev_was_number = false;
        // §6.3: numeric mode continues across a `,` or `.` that separates digits
        // (e.g. `5,70`, `4.2`), so the numeric indicator `⠼` is emitted only once.
        let mut numeric_mode = false;
        let mut quote_open = false;
        // §8.4 capitals passage: ⠠⠠⠠ … ⠠⠄ around runs of 3+ all-caps words.
        let (cap_start, cap_term, in_passage) = caps_passages(tokens);
        for i in 0..tokens.len() {
            if cap_start[i] {
                out.extend([CAPITAL, CAPITAL, CAPITAL]);
            }
            match &tokens[i] {
                EnglishToken::Space => {
                    out.push(SPACE);
                    prev_was_number = false;
                    numeric_mode = false;
                }
                EnglishToken::Number(digits) => {
                    if numeric_mode {
                        // §6.3: already in numeric mode (digit-separator `,`/`.`
                        // bridged us here) — emit digits only, no second `⠼`.
                        for d in digits {
                            out.push(super::rule_6::digit_cell(*d)?);
                        }
                    } else {
                        out.extend(super::rule_6::encode_number(digits)?);
                    }
                    prev_was_number = true;
                    numeric_mode = true;
                }
                EnglishToken::Word(chars) => {
                    // §6.5: a lowercase letter a–j immediately after a number needs
                    // the grade-1 indicator ⠰ so it is not misread as a digit.
                    if prev_was_number
                        && chars
                            .first()
                            .is_some_and(|c| c.is_ascii_lowercase() && ('a'..='j').contains(c))
                    {
                        out.push(GRADE1);
                    }
                    let prev = i.checked_sub(1).map(|p| &tokens[p]);
                    let next = tokens.get(i + 1);
                    let standing_alone = is_standing_alone(prev, next);
                    let shortform_usable =
                        standing_alone && !matches!(next, Some(EnglishToken::Symbol('@' | '/')));
                    // §10.5 lower wordsigns need a stricter boundary than §10.1/§10.2.
                    let mut lower_usable = standing_alone && lower_wordsign_usable(prev, next);
                    // §10.5.2: "enough's" keeps the wordsign (its interior apostrophe is
                    // "standing alone" per §2.6.4) — an explicit exception to the
                    // lower-dot-contact bar that spells out his'/was'/be'.
                    if !lower_usable
                        && chars
                            .iter()
                            .map(|c| c.to_ascii_lowercase())
                            .eq("enough".chars())
                        && matches!(next, Some(EnglishToken::Symbol('\'')))
                        && matches!(tokens.get(i + 2),
                            Some(EnglishToken::Word(w)) if w.len() == 1 && w[0].eq_ignore_ascii_case(&'s'))
                    {
                        lower_usable = true;
                    }
                    self.encode_word(
                        chars,
                        standing_alone,
                        shortform_usable,
                        lower_usable,
                        in_passage[i],
                        &mut out,
                    )?;
                    prev_was_number = false;
                    numeric_mode = false;
                }
                EnglishToken::Symbol('"') => {
                    // §7.6 double quotation mark: open ⠦ / close ⠴, alternating.
                    out.push(if quote_open { QUOTE_CLOSE } else { QUOTE_OPEN });
                    quote_open = !quote_open;
                    prev_was_number = false;
                    numeric_mode = false;
                }
                EnglishToken::Symbol(c) => {
                    let cells = super::rule_7::encode_punctuation(*c)
                        .or_else(|| super::rule_3::encode_symbol(*c))?;
                    out.extend(cells);
                    prev_was_number = false;
                    // §6.3: a `,` or `.` between two numbers is a digit separator —
                    // numeric mode (and thus the single `⠼`) carries across it. Any
                    // other symbol, or a `,`/`.` not flanked by digits, ends it.
                    numeric_mode = numeric_mode
                        && matches!(c, ',' | '.')
                        && matches!(tokens.get(i + 1), Some(EnglishToken::Number(_)));
                }
            }
            if cap_term[i] {
                // §8.4 capitals terminator ⠠⠄.
                out.extend([CAPITAL, decode_unicode('⠄')]);
            }
        }
        Some(out)
    }

    /// §8 capital prefix + §10.1/§10.2 wordsigns (when standing alone) +
    /// §4.1/§10 contracted letters.
    fn encode_word(
        &self,
        chars: &[char],
        standing_alone: bool,
        shortform_usable: bool,
        lower_usable: bool,
        suppress_caps: bool,
        out: &mut Vec<u8>,
    ) -> Option<()> {
        let lower: Vec<char> = chars.iter().map(char::to_ascii_lowercase).collect();
        let word: String = lower.iter().collect();
        if shortform_usable && super::rule_10_9::is_pure_shortform_abbreviation(&word) {
            out.push(GRADE1);
        }
        // `classify_caps(...)?` still guards mixed-case words (→ legacy fallback)
        // even inside a §8.4 passage, where the ⠠⠠⠠ … ⠠⠄ carry the capitalisation.
        match classify_caps(chars)? {
            _ if suppress_caps => {}
            Caps::None => {}
            Caps::Single => out.push(CAPITAL),
            Caps::Word => {
                // §8.7 / UEB §5.7.2: a *standing-alone* all-caps acronym whose
                // lowercase letters form a multi-letter shortform (e.g. `CD` =
                // "could", `WD` = "would") would, as `⠠⠠cd`, be misread as the
                // capitalised shortform. The grade-1 indicator `⠰` precedes the
                // caps marker to force the literal-letters reading. When the word
                // is attached to other context (after a number, apostrophe, or
                // closing bracket) the shortform reading does not arise, so the
                // indicator is not needed (e.g. `N12 7BT`, `SHE'LL`, `(R)AC`).
                let uppercase_word: String = chars.iter().collect();
                if standing_alone
                    && !super::rule_10_9::is_pure_shortform_abbreviation(&word)
                    && crate::rules::english_shortform::requires_grade1_indicator(&uppercase_word)
                {
                    out.push(GRADE1);
                }
                out.push(CAPITAL);
                out.push(CAPITAL);
            }
        }
        // §10.1/§10.2 (upper) and §10.5 (lower) wordsigns: a whole word that
        // stands alone (§2.6) becomes its wordsign. Lower wordsigns additionally
        // require the stricter `lower_usable` boundary. All are suppressed inside
        // Korean text via `standing_alone = false` (한국 점자 제37항).
        if standing_alone {
            let cell = super::rule_10_1::wordsign(&word)
                .or_else(|| super::rule_10_2::wordsign(&word))
                .or_else(|| {
                    lower_usable
                        .then(|| super::rule_10_5::wordsign(&word))
                        .flatten()
                });
            if let Some(cell) = cell {
                out.push(cell);
                return Some(());
            }
            if shortform_usable && let Some(cells) = super::rule_10_9::whole_word_cells(&word) {
                out.extend(cells);
                return Some(());
            }
        }
        out.extend(super::rule_10_9::encode_with_longer_shortforms(
            &lower,
            &self.contractions,
        )?);
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(text: &str) -> Option<Vec<u8>> {
        super::super::try_encode(text)
    }

    /// Build the expected cell vector from a unicode-braille string (`⠀` = space).
    fn cells(s: &str) -> Vec<u8> {
        s.chars().map(decode_unicode).collect()
    }

    #[rstest::rstest]
    #[case::lower_letters("cat", vec![decode_unicode('⠉'), decode_unicode('⠁'), decode_unicode('⠞')])]
    #[case::single_capital("A", vec![CAPITAL, decode_unicode('⠁')])]
    #[case::leading_capital("Cat", vec![CAPITAL, decode_unicode('⠉'), decode_unicode('⠁'), decode_unicode('⠞')])]
    // `XY` is all-caps but not a shortform collision, so no §8.7 grade-1 indicator.
    #[case::caps_word("XY", vec![CAPITAL, CAPITAL, decode_unicode('⠭'), decode_unicode('⠽')])]
    #[case::strong_contraction("the", vec![decode_unicode('⠮')])]
    #[case::groupsign_then_letters("show", vec![decode_unicode('⠩'), decode_unicode('⠪')])]
    #[case::lower_groupsign_in("find", vec![decode_unicode('⠋'), decode_unicode('⠔'), decode_unicode('⠙')])]
    #[case::lower_groupsign_en("send", vec![decode_unicode('⠎'), decode_unicode('⠢'), decode_unicode('⠙')])]
    #[case::enough_lower_wordsign("enough", vec![decode_unicode('⠢')])]
    #[case::two_words("a b", vec![decode_unicode('⠁'), SPACE, decode_unicode('⠃')])]
    #[case::number("95", vec![decode_unicode('⠼'), decode_unicode('⠊'), decode_unicode('⠑')])]
    #[case::number_then_az_letter("5a", vec![decode_unicode('⠼'), decode_unicode('⠑'), GRADE1, decode_unicode('⠁')])]
    #[case::word_space_number("a 50", vec![decode_unicode('⠁'), SPACE, decode_unicode('⠼'), decode_unicode('⠑'), decode_unicode('⠚')])]
    fn encodes_supported_words(#[case] text: &str, #[case] expected: Vec<u8>) {
        assert_eq!(enc(text), Some(expected));
    }

    #[rstest::rstest]
    #[case::has_unknown_symbol("a~")]
    #[case::mixed_case("McD")]
    fn unsupported_inputs_return_none(#[case] text: &str) {
        assert_eq!(enc(text), None);
    }

    #[rstest::rstest]
    #[case::word_period("cat.", vec![decode_unicode('⠉'), decode_unicode('⠁'), decode_unicode('⠞'), decode_unicode('⠲')])]
    #[case::wordsign_us_question("us?", vec![decode_unicode('⠥'), decode_unicode('⠦')])]
    #[case::percent("5%", vec![decode_unicode('⠼'), decode_unicode('⠑'), decode_unicode('⠨'), decode_unicode('⠴')])]
    #[case::double_quotes("\"a\"", vec![QUOTE_OPEN, decode_unicode('⠁'), QUOTE_CLOSE])]
    fn encodes_punctuation_and_symbols(#[case] text: &str, #[case] expected: Vec<u8>) {
        assert_eq!(enc(text), Some(expected));
    }

    /// §8.7 / UEB §5.7.2: a standing-alone all-caps acronym whose letters form a
    /// multi-letter shortform takes the grade-1 indicator `⠰` before `⠠⠠` to
    /// block the shortform reading; non-colliding caps words do not.
    #[rstest::rstest]
    // `CD` = "could" shortform → ⠰⠠⠠CD.
    #[case::cd_collides("CD", vec![GRADE1, CAPITAL, CAPITAL, decode_unicode('⠉'), decode_unicode('⠙')])]
    // `XY` is not a shortform → plain ⠠⠠XY.
    #[case::xy_no_collision("XY", vec![CAPITAL, CAPITAL, decode_unicode('⠭'), decode_unicode('⠽')])]
    fn caps_shortform_grade1(#[case] text: &str, #[case] expected: Vec<u8>) {
        assert_eq!(enc(text), Some(expected));
    }

    /// §6.3: numeric mode continues across a `,`/`.` digit separator, so the
    /// numeric indicator `⠼` appears once; any letter between digits restarts it.
    #[rstest::rstest]
    // `5,70` — comma between digits: single `⠼`, then ⠑ ⠂ ⠛ ⠚.
    #[case::comma_grouped("5,70", vec![decode_unicode('⠼'), decode_unicode('⠑'), decode_unicode('⠂'), decode_unicode('⠛'), decode_unicode('⠚')])]
    // `4.2` — decimal point between digits: single `⠼`.
    #[case::decimal("4.2", vec![decode_unicode('⠼'), decode_unicode('⠙'), decode_unicode('⠲'), decode_unicode('⠃')])]
    // `4x4` — a letter splits the run, so each number keeps its own `⠼`
    // (grade-1 ⠰ guards the a–j letter `x`? `x` is not a–j, so no ⠰).
    #[case::letter_split("4x4", vec![decode_unicode('⠼'), decode_unicode('⠙'), decode_unicode('⠭'), decode_unicode('⠼'), decode_unicode('⠙')])]
    fn numeric_mode_spans_digit_separators(#[case] text: &str, #[case] expected: Vec<u8>) {
        assert_eq!(enc(text), Some(expected));
    }

    /// §10.1/§10.2 wordsigns (standing alone) and the §2.6 standing-alone guard.
    #[rstest::rstest]
    #[case::but_wordsign("but", vec![decode_unicode('⠃')])]
    #[case::knowledge_wordsign("knowledge", vec![decode_unicode('⠅')])]
    #[case::this_wordsign("this", vec![decode_unicode('⠹')])]
    #[case::child_wordsign("child", vec![decode_unicode('⠡')])]
    #[case::it_apostrophe_s("it's", vec![decode_unicode('⠭'), decode_unicode('⠄'), decode_unicode('⠎')])]
    fn encodes_wordsigns(#[case] text: &str, #[case] expected: Vec<u8>) {
        assert_eq!(enc(text), Some(expected));
    }

    /// §10.9 shortforms: whole shortform words contract only in standalone
    /// pure-English UEB, while a literal abbreviation gets a grade-1 guard.
    #[rstest::rstest]
    #[case::good("good", "⠛⠙")]
    #[case::would("would", "⠺⠙")]
    #[case::rejoice("rejoice", "⠗⠚⠉")]
    #[case::literal_gd("gd", "⠰⠛⠙")]
    fn encodes_shortforms(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §10.5 lower wordsigns: used between anchoring boundaries (space/edge/
    /// bracket), spelled out when touching a lower-sign neighbour (`?`, hyphen).
    #[rstest::rstest]
    #[case::be_alone("be", vec![decode_unicode('⠆')])]
    #[case::was_alone("was", vec![decode_unicode('⠴')])]
    #[case::his_alone("his", vec![decode_unicode('⠦')])]
    #[case::was_in_parens("(was)", vec![decode_unicode('⠐'), decode_unicode('⠣'), decode_unicode('⠴'), decode_unicode('⠐'), decode_unicode('⠜')])]
    #[case::be_before_question("be?", vec![decode_unicode('⠃'), decode_unicode('⠑'), decode_unicode('⠦')])]
    // §10.5.2: enough's keeps the wordsign; his' (lower-dot contact) spells out.
    #[case::enoughs("Enough's", vec![CAPITAL, decode_unicode('⠢'), decode_unicode('⠄'), decode_unicode('⠎')])]
    #[case::his_apostrophe_n("his'n", vec![decode_unicode('⠓'), decode_unicode('⠊'), decode_unicode('⠎'), decode_unicode('⠄'), decode_unicode('⠝')])]
    fn lower_wordsigns_respect_boundaries(#[case] text: &str, #[case] expected: Vec<u8>) {
        assert_eq!(enc(text), Some(expected));
    }

    /// §8.4 capitals passage (3+ all-caps words) vs §8.3 capital word (1–2).
    #[rstest::rstest]
    #[case::passage_four("THE BBC AFRICA NEWS", "⠠⠠⠠⠮⠀⠃⠃⠉⠀⠁⠋⠗⠊⠉⠁⠀⠝⠑⠺⠎⠠⠄")]
    #[case::two_caps_no_passage("NEW YORK", "⠠⠠⠝⠑⠺⠀⠠⠠⠽⠕⠗⠅")]
    #[case::single_caps_word("DOG", "⠠⠠⠙⠕⠛")]
    fn caps_passage_threshold(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }
}
