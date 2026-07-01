//! Document-level UEB Grade-2 engine.
//!
//! Walks the token stream, applies §8 capitalisation indicators, and delegates
//! intra-word contraction to the [`ContractionEngine`]. Returns `None` for any
//! construct not yet supported, so the caller can fall back to the legacy path
//! (this is what keeps the engine safe to grow rule-by-rule).

use super::contraction::ContractionEngine;
use super::rule_10_3::StrongContractionRule;
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

/// Determine the capitalisation pattern, or `None` for mixed-case words (internal
/// capitals, e.g. "McDonald") — these are split and re-encoded part-by-part by
/// [`EnglishUebEngine::encode_mixed_case`] (§8.2).
fn classify_caps(chars: &[char]) -> Option<Caps> {
    // Unicode case (not ASCII-only) so an accented or ligatured capital (`É`, `Œ`,
    // `ẞ`) counts as a capital — `ŒDIPUS`/`AOÛT` are whole-word caps, not mixed.
    let uppers = chars.iter().filter(|c| c.is_uppercase()).count();
    if uppers == 0 {
        Some(Caps::None)
    } else if uppers == chars.len() {
        if chars.len() == 1 {
            Some(Caps::Single)
        } else {
            Some(Caps::Word)
        }
    } else if uppers == 1 && chars[0].is_uppercase() {
        Some(Caps::Single)
    } else {
        None
    }
}

/// A word whose letters are all uppercase (≥1 upper, no lowercase) — `A`, `NEW`,
/// `AOÛT`. Uses Unicode case so accented/ligatured capitals count (§8.4).
fn is_caps_word(t: &EnglishToken) -> bool {
    matches!(t, EnglishToken::Word(c)
        if c.iter().any(|x| x.is_uppercase()) && !c.iter().any(|x| x.is_lowercase()))
}

/// A word containing any lowercase letter (breaks a §8.4 capitals passage).
fn has_lower_word(t: &EnglishToken) -> bool {
    matches!(t, EnglishToken::Word(c) if c.iter().any(|x| x.is_lowercase()))
}

/// §9.x: from the styled run at `i`, count consecutive same-`form` styled words
/// joined only by spaces/punctuation, and return that count with the passage end
/// (exclusive) — the last styled run plus any trailing punctuation (so `Cities.`
/// keeps its full stop inside the passage). A plain word or number ends the run.
///
/// A trailing *dash* is excluded: it separates the passage from following text
/// (e.g. an attribution `…𝑤𝑖𝑡.—Shakespeare`), so the terminator falls before the
/// dash (`…⠺⠊⠞⠲⠨⠄⠠⠤…`), not after it.
fn styled_passage_extent(
    tokens: &[EnglishToken],
    i: usize,
    form: super::token::Typeform,
) -> (usize, usize) {
    let mut words = 0usize;
    let mut last_styled_end = i;
    let mut k = i;
    loop {
        while matches!(tokens.get(k), Some(EnglishToken::Space)) {
            k += 1;
        }
        // A passage word must begin with a same-form styled token; a plain word,
        // number, or other-form styled token ends the run.
        if !matches!(tokens.get(k), Some(EnglishToken::Styled(_, f)) if *f == form) {
            break;
        }
        words += 1;
        // Consume the whole space-delimited word: same-form styled runs plus the
        // symbols attached within it, so a hyphen/apostrophe-joined word counts
        // once (`l'oeil-de-boeuf`) and a trailing mark stays attached (`Twist,`).
        while let Some(t) = tokens.get(k) {
            match t {
                EnglishToken::Styled(_, f) if *f == form => {
                    k += 1;
                    last_styled_end = k;
                }
                EnglishToken::Symbol(_) => k += 1,
                _ => break,
            }
        }
    }
    let mut end = last_styled_end;
    // A trailing dash or *closing bracket* separates the passage from following
    // matter, so the terminator falls *before* it (`…𝐽𝑢𝑙𝑖𝑒𝑡⠨⠄⠐⠜`,
    // `…𝑤𝑖𝑡.⠨⠄⠠⠤`); a sentence mark (`.`, `,`) that belongs to the emphasised
    // phrase stays inside (`𝐶𝑖𝑡𝑖𝑒𝑠⠲⠨⠄`).
    while matches!(tokens.get(end),
        Some(EnglishToken::Symbol(c))
            if !matches!(c, '-' | '\u{2013}' | '\u{2014}' | ')' | ']' | '}'))
    {
        end += 1;
    }
    (words, end)
}

/// §8.4 within a §9 typeform passage: whether every styled letter from `start` to
/// `end` (exclusive) is uppercase, so the passage is also a capitals passage
/// (`⠠⠠⠠ … ⠠⠄` nested inside the typeform `⠨⠶ … ⠨⠄`). A single lowercase styled
/// letter disqualifies it (each caps word then takes its own `⠠⠠`).
fn styled_passage_all_caps(
    tokens: &[EnglishToken],
    start: usize,
    end: usize,
    form: super::token::Typeform,
) -> bool {
    let mut saw_letter = false;
    for t in tokens.iter().take(end).skip(start) {
        if let EnglishToken::Styled(c, f) = t
            && *f == form
            && c.is_alphabetic()
        {
            saw_letter = true;
            if c.is_lowercase() {
                return false;
            }
        }
    }
    saw_letter
}

/// §9.5: whether the space-delimited word continues past index `j` with more
/// graphic content — a `Word`/`Number`, possibly after attached symbols (`/`) —
/// so a *word* typeform indicator needs an explicit terminator (`a̲n̲d̲/or` →
/// `⠸⠂⠯⠸⠄⠸⠌⠕⠗`). A trailing sentence mark alone (`𝐬𝐞𝐭.`) or a space does not
/// continue the word, so no terminator is emitted there.
fn word_continues_after(tokens: &[EnglishToken], j: usize) -> bool {
    let mut k = j;
    while let Some(t) = tokens.get(k) {
        match t {
            EnglishToken::Word(_) | EnglishToken::Number(_) => return true,
            EnglishToken::Symbol(_) => k += 1,
            EnglishToken::Space | EnglishToken::Styled(..) => return false,
        }
    }
    false
}

/// §7.1.3: whether the lower-cell punctuation mark `c` at `tokens[i]` needs a
/// grade-1 indicator — its braille cell collides with a lower groupsign/wordsign,
/// so it is guarded in the position where that contraction could be read instead:
/// a `?` (⠦ = "his") preceded by a boundary (standing alone), a `:` (⠒ = "con")
/// directly between two words, a `!` (⠖) run embedded inside a word (`Ai!!ams`),
/// and a *word-initial* `.` (⠲ = "dis") before a word (abbreviation dots like
/// `U.S.A.`, whose `.` follows a word, are excluded).
fn punctuation_grade1(tokens: &[EnglishToken], i: usize, c: char) -> bool {
    let prev = i.checked_sub(1).map(|p| &tokens[p]);
    let next = tokens.get(i + 1);
    match c {
        // §7.1.3: the `?` cell (⠦) is also the "his" groupsign, so a `?` referenced
        // in isolation takes the grade-1 indicator. That is any `?` not closing a
        // word: at an edge or space, or attached after an opening bracket or a dash
        // (`[?]`, `(?—1750)`, `10:30-?`). A `?` right after a word (`who?`) is a
        // genuine question mark and keeps the bare ⠦.
        '?' => matches!(
            prev,
            None | Some(EnglishToken::Space)
                | Some(EnglishToken::Symbol(
                    '(' | '[' | '{' | '-' | '\u{2013}' | '\u{2014}'
                ))
        ),
        ':' => {
            matches!(prev, Some(EnglishToken::Word(_)))
                && matches!(next, Some(EnglishToken::Word(_)))
        }
        // A `!` (or run of `!`) directly between letters takes the indicator once,
        // before the run: it follows a word and, past the run, a word continues.
        '!' => {
            matches!(prev, Some(EnglishToken::Word(_))) && {
                let mut k = i + 1;
                while matches!(tokens.get(k), Some(EnglishToken::Symbol('!'))) {
                    k += 1;
                }
                matches!(tokens.get(k), Some(EnglishToken::Word(_)))
            }
        }
        '.' => {
            matches!(prev, None | Some(EnglishToken::Space))
                && matches!(next, Some(EnglishToken::Word(_)))
        }
        _ => false,
    }
}

/// §9.5: the exclusive end of a *word*-level typeform extent starting at `i` — the
/// index just past the last same-`form` styled token reachable through only
/// same-form styled tokens and attached symbols (no space). A styled word with
/// internal punctuation (`𝑜𝑓-𝑡ℎ𝑒`, `𝑙'𝑜𝑒𝑖𝑙-𝑑𝑒-𝑏𝑜𝑒𝑢𝑓`) is therefore one extent,
/// while a following space or plain word ends it (a trailing symbol like the `/`
/// in `a̲n̲d̲/` is excluded — the span ends at its last styled token).
fn styled_word_span(tokens: &[EnglishToken], i: usize, form: super::token::Typeform) -> usize {
    let mut last_styled_end = i;
    let mut k = i;
    while let Some(t) = tokens.get(k) {
        match t {
            EnglishToken::Styled(_, f) if *f == form => {
                k += 1;
                last_styled_end = k;
            }
            EnglishToken::Symbol(_) => k += 1,
            _ => break,
        }
    }
    last_styled_end
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

/// §7.6 role of a single-quote glyph: an opening/closing single *quotation* mark
/// (`⠠⠦`/`⠠⠴`) or an *apostrophe* (`⠄`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SingleQuote {
    Apostrophe,
    Open,
    Close,
}

/// Classify each *curly* single quote — `‘` (U+2018) and `’` (U+2019) — as an
/// opening or closing single quotation mark, or an apostrophe (§7.6).
///
/// A left curly `‘` always opens. A right curly `’` is a *closing* quote when it
/// matches an open on the stack; an apostrophe when it sits between two words
/// (`o’clock`); and otherwise a word-final possessive/elision apostrophe
/// (`Jones’`, `be’`, `rock ’n’ roll`). This matched-pair test is what
/// distinguishes `’` in `mother-‘in-law’` (paired → closing quote) from `’` in
/// `Jones’` (unpaired → apostrophe).
///
/// The straight quote `'` (U+0027) is deliberately *not* classified here: it is
/// genuinely ambiguous in print — a quoted `'Hamlet'` and an apostrophe-delimited
/// `'display will minimise'` are indistinguishable — so it stays an apostrophe
/// (the dominant reading) on the default punctuation path.
fn single_quote_roles(tokens: &[EnglishToken]) -> Vec<SingleQuote> {
    let mut roles = vec![SingleQuote::Apostrophe; tokens.len()];
    // Indices of opening curly single quotes still awaiting their close (LIFO).
    let mut open_stack: Vec<usize> = Vec::new();
    let adjacent_text = |t: Option<&EnglishToken>| {
        matches!(
            t,
            Some(EnglishToken::Word(_) | EnglishToken::Number(_) | EnglishToken::Styled(..))
        )
    };
    for i in 0..tokens.len() {
        match &tokens[i] {
            EnglishToken::Symbol('\u{2018}') => {
                roles[i] = SingleQuote::Open;
                open_stack.push(i);
            }
            EnglishToken::Symbol('\u{2019}') => {
                let prev_text = i > 0 && adjacent_text(tokens.get(i - 1));
                let next_text = adjacent_text(tokens.get(i + 1));
                roles[i] = if prev_text && next_text {
                    // Between two words → apostrophe (`o'clock`).
                    SingleQuote::Apostrophe
                } else if open_stack.pop().is_some() {
                    // Closing side of a matched pair.
                    SingleQuote::Close
                } else if prev_text || next_text {
                    // Unmatched but touching a word → possessive/elision apostrophe
                    // (`Jones'`, `be'`, `'Tis`).
                    SingleQuote::Apostrophe
                } else {
                    // Unmatched and fully detached (space/edge both sides) → a
                    // standalone closing single quote referenced in isolation
                    // (§7.6.10), e.g. "forget the ' at the end".
                    SingleQuote::Close
                };
            }
            _ => {}
        }
    }
    roles
}

/// §3.27: detect a transcriber's-note marker `[open tn]` / `[close tn]` starting
/// at `i`. The print convention spells the boundary as those bracketed words; in
/// braille it is a single note indicator — `⠈⠨⠣` to open, `⠈⠨⠜` to close (the
/// square-bracket signs `⠨⠣`/`⠨⠜` under a dot-4 prefix). Returns `(is_open,
/// next_index)` on a match so the five marker tokens are replaced as a unit.
fn transcriber_note_at(tokens: &[EnglishToken], i: usize) -> Option<(bool, usize)> {
    let word_is = |t: Option<&EnglishToken>, s: &str| matches!(t, Some(EnglishToken::Word(w)) if w.iter().collect::<String>() == s);
    if !matches!(tokens.get(i), Some(EnglishToken::Symbol('['))) {
        return None;
    }
    let is_open = if word_is(tokens.get(i + 1), "open") {
        true
    } else if word_is(tokens.get(i + 1), "close") {
        false
    } else {
        return None;
    };
    if matches!(tokens.get(i + 2), Some(EnglishToken::Space))
        && word_is(tokens.get(i + 3), "tn")
        && matches!(tokens.get(i + 4), Some(EnglishToken::Symbol(']')))
    {
        Some((is_open, i + 5))
    } else {
        None
    }
}

/// §10.4.3: whether a word token preceded by `prev` begins a fresh word — `prev`
/// is the text edge, a space, or a hyphen/dash (`to-ing`, `Smith–Inge`). A token
/// after a mid-word connector (`(`, `'`, a base word, or a typeform run, as in
/// `brown(ing)`, `Ch'ing`, `flow𝑖𝑛𝑔`) is a continuation, not a word start, so a
/// leading `ing` there keeps its groupsign instead of spelling `in` + `g`.
fn word_initial_boundary(prev: Option<&EnglishToken>) -> bool {
    matches!(
        prev,
        None | Some(EnglishToken::Space)
            | Some(EnglishToken::Symbol('-' | '\u{2013}' | '\u{2014}'))
    )
}

/// §10.12.15: if `tokens[i]` is part of a letter-by-letter spelled run — three or
/// more single-letter words joined by single hyphens (`w-i-n-d-o-w`,
/// `M-a-c-L-e-a-n`, `U-N-I-T-E-D`) ending at a space/edge/sentence mark — return the
/// run's `(first, last)` letter-token indices. Such a run takes ONE grade-1
/// *passage* indicator `⠰⠰` at its first letter instead of a per-letter `⠰`. A run
/// continuing into a plain word (`s-s-s-super`) needs a passage terminator and is
/// deliberately excluded here.
fn spelled_letter_run(tokens: &[EnglishToken], i: usize) -> Option<(usize, usize)> {
    let single = |k: usize| matches!(tokens.get(k), Some(EnglishToken::Word(w)) if w.len() == 1);
    if !single(i) {
        return None;
    }
    let mut start = i;
    while start >= 2
        && matches!(tokens.get(start - 1), Some(EnglishToken::Symbol('-')))
        && single(start - 2)
    {
        start -= 2;
    }
    let mut last = i;
    while matches!(tokens.get(last + 1), Some(EnglishToken::Symbol('-'))) && single(last + 2) {
        last += 2;
    }
    // Three or more letters, and the run must stand alone — a hyphen on either
    // side glues it to a plain word: a trailing one (`s-s-s-super`) needs a
    // terminator, a leading one (`so-o-o-o`) is a stutter, not a spelling-out.
    if (last - start) / 2 + 1 < 3
        || matches!(tokens.get(last + 1), Some(EnglishToken::Symbol('-')))
        || (start >= 1 && matches!(tokens.get(start - 1), Some(EnglishToken::Symbol('-'))))
    {
        return None;
    }
    Some((start, last))
}

/// §2.6 / §10.12.12: whether the word at `i` continues into a larger
/// space-delimited unit across an *attached* bracket or double quote —
/// `child(ish)` = "childish", `(be)long` = "belong", `"just"ice` = "justice" — so
/// it does NOT stand alone and a wordsign/shortform must not consume it (`child`
/// keeps its full spelling, not the `child` shortform ⠡; `be` is spelled, not the
/// ⠆ wordsign; `just` is spelled, not the `just` shortform). A bracket or `"`
/// directly followed (no space) by a Word/Number means the mark is mid-word, not a
/// fresh boundary. The *apostrophe* `'` is deliberately excluded — `it's`/`that's`
/// legitimately keep the wordsign before a contraction-suffix apostrophe.
fn continues_across_bracket(tokens: &[EnglishToken], i: usize) -> bool {
    let is_bracket = |t: Option<&EnglishToken>| {
        matches!(
            t,
            Some(EnglishToken::Symbol('(' | ')' | '[' | ']' | '{' | '}' | '"'))
        )
    };
    let is_texty = |t: Option<&EnglishToken>| {
        matches!(
            t,
            Some(EnglishToken::Word(_) | EnglishToken::Number(_) | EnglishToken::Styled(..))
        )
    };
    // Forward: the word is followed by an attached bracket/`"` then more text
    // (`child(ish)`, `(be)long`, `"just"ice`).
    let forward = is_bracket(tokens.get(i + 1)) && is_texty(tokens.get(i + 2));
    // Backward (symmetric): the word follows an attached bracket/`"` that itself
    // follows text (`"be"friend` → `friend` continues "befriend", so it spells out
    // rather than taking the `friend` shortform).
    let backward = i.checked_sub(1).is_some_and(|p| is_bracket(tokens.get(p)))
        && i.checked_sub(2).is_some_and(|p| is_texty(tokens.get(p)));
    // §10.12.12: an apostrophe + a NON-suffix continuation keeps the word from
    // standing alone (`go'n` = "goin'", `out'a` = "outta" → spell `go`/`out`, not
    // their wordsigns). The standard contraction suffixes — `'s 't 'd 'm 'll 're
    // 've` (`it's`, `don't`, `we'll`, `they're`) — DO leave the word standing alone.
    let is_suffix = |w: &[char]| {
        let lc = |c: &char| c.to_ascii_lowercase();
        match w {
            // `'s 't 'd 'm` (`it's`, `don't`, `we'd`, `I'm`) — case-insensitive so an
            // all-caps contraction (`IT'S`, `HE'S`, `THAT'S`) is protected too.
            [a] => matches!(lc(a), 's' | 't' | 'd' | 'm'),
            // `'ll 're 've` (`we'll`, `they're`, `we've`).
            [a, b] => matches!((lc(a), lc(b)), ('l', 'l') | ('r', 'e') | ('v', 'e')),
            _ => false,
        }
    };
    let apostrophe = matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('\'')))
        && matches!(tokens.get(i + 2), Some(EnglishToken::Word(w)) if !is_suffix(w));
    forward || backward || apostrophe
}

/// Per-word encoding context derived from a word's surrounding tokens: the §2.6
/// standing-alone status and the §8/§10 boundary flags. Bundled so the word
/// encoder takes one value instead of a long boolean argument list.
struct WordContext {
    /// §2.6: the word stands alone, so §10.1/§10.2/§10.5 wordsigns may apply.
    standing_alone: bool,
    /// §10.9: a shortform abbreviation may be used for this word.
    shortform_usable: bool,
    /// §10.5: the stricter lower-wordsign boundary is also satisfied.
    lower_usable: bool,
    /// §8.4: inside a caps passage — per-word capital indicators are suppressed.
    suppress_caps: bool,
    /// §10.4.3: this token begins a fresh word (after a space/hyphen/dash/edge),
    /// so a word-initial `ing` spells out as `in` (⠔) + `g`.
    word_initial: bool,
    /// §10.12.1: the word directly abuts a digit (`CH6`, `6CH`), so an all-caps run
    /// is an initialism "used as letters" and takes no contractions.
    digit_adjacent: bool,
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
        // §10.11: the bridge-aware strong groupsign suppresses `th`/`wh`/`sh`
        // that cross a compound boundary (hyphenation-detected).
        contractions.register(Box::new(super::rule_10_11::BridgeAwareStrongGroupsignRule));
        // §10.6.8: `en`/`in` are pronunciation-gated — suppressed where they
        // overlap a word-final `ness` whose `n` onsets the syllable (`busi·ness`,
        // `fi·ness·e`), kept where the `n` closes it (`citi·zen·ess`).
        contractions.register(Box::new(super::rule_10_6_8::EnInBeforeNessRule::new(
            Box::new(super::pronunciation::cmudict::CmuDictProvider::new()),
        )));
        contractions.register(Box::new(super::rule_10_7::InitialContractionRule));
        contractions.register(Box::new(super::rule_10_8::FinalGroupsignRule));
        // §10.6 restricted groupsigns (be/con/dis) judge the first syllable from
        // pronunciation/word-structure (CMUdict).
        contractions.register(Box::new(
            super::rule_10_6_restricted::RestrictedLowerGroupsignRule::new(Box::new(
                super::pronunciation::cmudict::CmuDictProvider::new(),
            )),
        ));
        // §10.6.5 middle lower groupsigns (ea/bb/cc/ff/gg) need the word list to
        // detect morpheme boundaries (pine|apple, dumb|bell).
        contractions.register(Box::new(
            super::rule_10_6_middle::MiddleLowerGroupsignRule::new(Box::new(
                super::pronunciation::cmudict::CmuDictProvider::new(),
            )),
        ));
        // §10.7 deferred initial-letter contractions (part/work/some/where/…) are
        // pronunciation-gated.
        contractions.register(Box::new(
            super::rule_10_7_pron::InitialContractionPronunciationRule::new(Box::new(
                super::pronunciation::cmudict::CmuDictProvider::new(),
            )),
        ));
        // §10.7/§10.11 structure-gated initial-letter contractions (`lord`, `work`):
        // applied only where the letters START a real word component.
        contractions.register(Box::new(
            super::rule_10_7_struct::StructuralInitialContractionRule::new(Box::new(
                super::pronunciation::cmudict::CmuDictProvider::new(),
            )),
        ));
        Self { contractions }
    }

    /// Encode a token stream. Returns `None` if any token is unsupported
    /// (a number, a symbol, or a mixed-case word), so the legacy path — which
    /// handles those — takes over. `explicit_english` is true only under an
    /// explicit `EncodingMode::English` (testcase `context: english`); it threads
    /// to §5.7.1 so an isolated single letter is grade-1-indicated only then.
    pub fn encode(&self, tokens: &[EnglishToken], explicit_english: bool) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        let mut prev_was_number = false;
        // §6.3: numeric mode continues across a `,` or `.` that separates digits
        // (e.g. `5,70`, `4.2`), so the numeric indicator `⠼` is emitted only once.
        let mut numeric_mode = false;
        let mut quote_open = false;
        // §9: index past a styled run already emitted as a word indicator, so its
        // member tokens are not re-emitted individually.
        let mut skip_to = 0usize;
        // §9.x active typeform passage: (end index exclusive, form, caps) where
        // `caps` marks a passage whose every styled word is all-caps (§8.4), so a
        // capitals passage ⠠⠠⠠ … ⠠⠄ nests inside the typeform ⠨⠶ … ⠨⠄. Its
        // terminator is emitted once the walk passes the styled span.
        let mut passage: Option<(usize, super::token::Typeform, bool)> = None;
        // §8.4 capitals passage: ⠠⠠⠠ … ⠠⠄ around runs of 3+ all-caps words.
        let (cap_start, cap_term, in_passage) = caps_passages(tokens);
        // §7.6 single-quote vs apostrophe role per token (matched-pair analysis).
        let sq_roles = single_quote_roles(tokens);
        for i in 0..tokens.len() {
            if let Some((end, form, caps)) = passage
                && i >= end
            {
                // §8.4: close the nested capitals passage before the typeform one.
                if caps {
                    out.extend([CAPITAL, decode_unicode('⠄')]);
                }
                out.extend(super::rule_9::terminator(form));
                passage = None;
            }
            if i < skip_to {
                continue;
            }
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
                    let prev = i.checked_sub(1).map(|p| &tokens[p]);
                    let next = tokens.get(i + 1);
                    let standing_alone =
                        is_standing_alone(prev, next) && !continues_across_bracket(tokens, i);
                    // §6.5: a lowercase letter a–j immediately after a number needs
                    // the grade-1 indicator ⠰ so it is not misread as a digit.
                    let after_number_grade1 = prev_was_number
                        && chars
                            .first()
                            .is_some_and(|c| c.is_ascii_lowercase() && ('a'..='j').contains(c));
                    // §5.7.1: a single wordsign-letter standing alone (§2.6) takes a
                    // grade-1 indicator ⠰ so it is not read as the wordsign; §5.8.1
                    // places it before any capital. Full rule in `rule_5_7`.
                    // §10.12.15: a letter-by-letter spelled run (`w-i-n-d-o-w`) takes
                    // one grade-1 *passage* ⠰⠰ at its first letter; its members then
                    // suppress the per-letter grade-1 ⠰.
                    let spelled_run = spelled_letter_run(tokens, i);
                    if matches!(spelled_run, Some((start, _)) if start == i) {
                        out.extend([GRADE1, GRADE1]);
                    }
                    let letter_grade1 = spelled_run.is_none()
                        && super::rule_5_7::needs_grade1_indicator(tokens, i, explicit_english);
                    if after_number_grade1 || letter_grade1 {
                        out.push(GRADE1);
                    }
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
                        WordContext {
                            standing_alone,
                            shortform_usable,
                            lower_usable,
                            suppress_caps: in_passage[i],
                            word_initial: word_initial_boundary(prev),
                            digit_adjacent: matches!(prev, Some(EnglishToken::Number(_)))
                                || matches!(next, Some(EnglishToken::Number(_))),
                        },
                        &mut out,
                    )?;
                    prev_was_number = false;
                    numeric_mode = false;
                }
                EnglishToken::Symbol('"') => {
                    // §7.6.10: a double quotation mark standing alone (a space or
                    // text edge on both sides) is the mark referenced in isolation
                    // → grade-1 + the nondirectional double-quote sign ⠰⠠⠶, and it
                    // does not flip the open/close alternation.
                    let standalone = (i == 0
                        || matches!(tokens.get(i - 1), Some(EnglishToken::Space)))
                        && matches!(tokens.get(i + 1), None | Some(EnglishToken::Space));
                    if standalone {
                        out.extend([GRADE1, decode_unicode('⠠'), decode_unicode('⠶')]);
                    } else {
                        // §7.6 double quotation mark: open ⠦ / close ⠴, alternating.
                        out.push(if quote_open { QUOTE_CLOSE } else { QUOTE_OPEN });
                        quote_open = !quote_open;
                    }
                    prev_was_number = false;
                    numeric_mode = false;
                }
                EnglishToken::Symbol('\u{2018}' | '\u{2019}') => {
                    // §7.6 curly single quotation mark vs apostrophe, resolved by
                    // the matched-pair analysis in `single_quote_roles`: an opening
                    // mark → ⠠⠦, a closing mark → ⠠⠴, an apostrophe → ⠄. The straight
                    // `'` is ambiguous and stays an apostrophe on the default path.
                    match sq_roles[i] {
                        SingleQuote::Open => {
                            // §7.6.10: a detached opening single quote (a space sits
                            // between it and the text it bounds) takes a grade-1
                            // indicator ⠰ so the ⠠⠦ is not misread.
                            if matches!(tokens.get(i + 1), Some(EnglishToken::Space)) {
                                out.push(GRADE1);
                            }
                            out.extend([decode_unicode('⠠'), decode_unicode('⠦')]);
                        }
                        SingleQuote::Close => {
                            // §7.6.10: likewise a detached closing single quote (a
                            // space precedes it) takes the grade-1 indicator.
                            if i > 0 && matches!(tokens.get(i - 1), Some(EnglishToken::Space)) {
                                out.push(GRADE1);
                            }
                            out.extend([decode_unicode('⠠'), decode_unicode('⠴')]);
                        }
                        SingleQuote::Apostrophe => out.push(decode_unicode('⠄')),
                    }
                    prev_was_number = false;
                    numeric_mode = false;
                }
                EnglishToken::Symbol(c)
                    if super::rule_16::is_line_char(*c)
                        && (i.checked_sub(1).is_some_and(|p| {
                            matches!(&tokens[p], EnglishToken::Symbol(s) if super::rule_16::is_line_char(*s))
                        }) || matches!(tokens.get(i + 1), Some(EnglishToken::Symbol(s)) if super::rule_16::is_line_char(*s))) =>
                {
                    // §16.2 horizontal line mode: a run of two or more box-drawing
                    // characters opens with the indicator `⠐⠒` (whose `⠒` is the
                    // first solid segment, so a leading `─` folds into it); each
                    // further char maps to its segment/corner/crossing cell. A lone
                    // box char never reaches here (the guard requires a neighbour),
                    // so a mathematical `≡`/`─` keeps its legacy meaning.
                    let prev_is_line = i.checked_sub(1).is_some_and(|p| {
                        matches!(&tokens[p], EnglishToken::Symbol(s) if super::rule_16::is_line_char(*s))
                    });
                    if prev_is_line {
                        out.push(super::rule_16::line_segment(*c)?);
                    } else {
                        out.push(decode_unicode('⠐'));
                        out.push(decode_unicode('⠒'));
                        if *c != super::rule_16::SIMPLE_SEGMENT {
                            out.push(super::rule_16::line_segment(*c)?);
                        }
                    }
                    // §16.2.5: a horizontal line interrupted by text mid-line takes
                    // the line mode terminator `⠄` before the text (a following space
                    // ends the line naturally, needing none). The next box run
                    // re-opens with its own `⠐⠒` indicator (§16.4.2).
                    if matches!(tokens.get(i + 1), Some(EnglishToken::Word(_))) {
                        out.push(decode_unicode('⠄'));
                    }
                    prev_was_number = false;
                    numeric_mode = false;
                }
                EnglishToken::Symbol('[') if transcriber_note_at(tokens, i).is_some() => {
                    // §3.27: a `[open tn]` / `[close tn]` print marker becomes a
                    // single note indicator — `⠈⠨⠣` open, `⠈⠨⠜` close — replacing
                    // the five bracketed tokens.
                    let (is_open, end) = transcriber_note_at(tokens, i)?;
                    out.push(decode_unicode('⠈'));
                    out.push(decode_unicode('⠨'));
                    out.push(decode_unicode(if is_open { '⠣' } else { '⠜' }));
                    skip_to = end;
                    prev_was_number = false;
                    numeric_mode = false;
                }
                EnglishToken::Symbol(c) if super::rule_3_24::is_script_char(*c) => {
                    // §3.24 super/subscript: a digit run following a base takes the
                    // level indicator (`⠔`/`⠢`). The grade-1 indicator `⠰` is added
                    // for a letter base (`B₁₂`, `clarion¹`) but not after a number,
                    // whose numeric mode already covers it (`1682.³`). A *leading*
                    // script (no base, e.g. `¹ clarion` or combinatorics `₇𝑃₂`) or a
                    // non-digit script (`ᵐ`, `⁺`) fails the whole UEB attempt so the
                    // legacy/math path (제18/19항) keeps ownership.
                    let (kind, first) = super::rule_3_24::script_digit(*c)?;
                    let base_is_number = match i.checked_sub(1).map(|p| &tokens[p]) {
                        Some(EnglishToken::Word(_)) => false,
                        Some(EnglishToken::Number(_)) => true,
                        // A base reached across a single period (`1682.³`, `knowledge.³`).
                        Some(EnglishToken::Symbol('.')) => {
                            match i.checked_sub(2).map(|p| &tokens[p]) {
                                Some(EnglishToken::Word(_)) => false,
                                Some(EnglishToken::Number(_)) => true,
                                _ => return None,
                            }
                        }
                        _ => return None,
                    };
                    let mut digits = vec![first];
                    let mut j = i + 1;
                    while let Some(EnglishToken::Symbol(sc)) = tokens.get(j) {
                        if !super::rule_3_24::is_script_char(*sc) {
                            break;
                        }
                        match super::rule_3_24::script_digit(*sc) {
                            Some((k, d)) if k == kind => {
                                digits.push(d);
                                j += 1;
                            }
                            // a mixed-kind or non-digit script char is unsupported.
                            _ => return None,
                        }
                    }
                    if !base_is_number {
                        out.push(GRADE1);
                    }
                    out.push(kind.indicator());
                    out.push(decode_unicode('⠼'));
                    for d in &digits {
                        out.push(super::rule_6::digit_cell(*d)?);
                    }
                    skip_to = j;
                    prev_was_number = false;
                    numeric_mode = false;
                }
                EnglishToken::Symbol(c @ ('.' | ','))
                    if matches!(
                        i.checked_sub(1).map(|p| &tokens[p]),
                        None | Some(EnglishToken::Space)
                    ) && matches!(tokens.get(i + 1), Some(EnglishToken::Number(_))) =>
                {
                    // §6: a leading decimal point or comma (`.375`, `,7`) opens a
                    // number — the numeric indicator ⠼ then the separator (`.`→⠲,
                    // `,`→⠂), with numeric mode carrying the following digits (no
                    // second ⠼). A `.`/`,` *after* a digit (`3.14`, `8,93`) is the
                    // §6.3 digit-separator handled in the general Symbol arm below.
                    out.push(super::rule_6::NUMERIC_INDICATOR);
                    out.push(decode_unicode(if *c == '.' { '⠲' } else { '⠂' }));
                    prev_was_number = false;
                    numeric_mode = true;
                }
                EnglishToken::Symbol(c) => {
                    // §7.1.3: a lower-cell punctuation mark whose cell collides with
                    // a lower contraction takes a grade-1 indicator ⠰ where that
                    // contraction could be read instead (a standing-alone `?`, a
                    // word-internal `:`, a word-initial `.`).
                    if punctuation_grade1(tokens, i, *c) {
                        out.push(GRADE1);
                    }
                    let cells = super::rule_7::encode_punctuation(*c)
                        .or_else(|| super::rule_3::encode_symbol(*c))
                        .or_else(|| super::rule_6::encode_vulgar_fraction(*c))?;
                    out.extend(cells);
                    prev_was_number = false;
                    // §6.3: a `,` or `.` between two numbers is a digit separator —
                    // numeric mode (and thus the single `⠼`) carries across it. Any
                    // other symbol, or a `,`/`.` not flanked by digits, ends it.
                    numeric_mode = numeric_mode
                        && matches!(c, ',' | '.')
                        && matches!(tokens.get(i + 1), Some(EnglishToken::Number(_)));
                }
                EnglishToken::Styled(_, form) => {
                    // §9 typeform extent: a single styled letter takes a *symbol*
                    // indicator (`⠨⠆`); a run of 2+ styled letters a *word* indicator
                    // (`⠨⠂`); and 3+ same-form styled words joined by spaces or
                    // punctuation one *passage* indicator + terminator (`⠨⠶…⠨⠄`). A
                    // styled number or a single styled symbol takes a *symbol*
                    // indicator over the whole item. §5.8.1 keeps it before caps.
                    let mut j = i + 1;
                    while matches!(tokens.get(j), Some(EnglishToken::Styled(_, f)) if f == form) {
                        j += 1;
                    }
                    let chars: Vec<char> = tokens[i..j]
                        .iter()
                        .map(|t| match t {
                            EnglishToken::Styled(c, _) => *c,
                            _ => unreachable!("run is all Styled"),
                        })
                        .collect();
                    // The walk resumes past the contiguous run, unless a
                    // multi-segment styled word extends it to its span end.
                    let mut run_end = j;
                    if chars.iter().all(char::is_ascii_digit) {
                        // §9: a styled digit run that is only PART of a larger number
                        // — plain digits sit immediately before or after it — takes a
                        // *word* indicator when it spans 2+ digits, with a terminator
                        // if plain digits continue after it (`45̲6̲7` → ⠼⠙⠸⠂⠼⠑⠋⠸⠄⠼⠛,
                        // `13.8𝟔𝟔𝟔𝟔` → …⠓⠘⠂⠼⠋⠋⠋⠋). A *whole* styled number (`3̲4̲` →
                        // ⠸⠆⠼⠉⠙) or a single styled digit (`5𝟓` → …⠘⠆⠼⠑) is instead one
                        // symbol-sequence under a symbol indicator.
                        let prev_is_number = i.checked_sub(1).is_some_and(|p| {
                            matches!(tokens.get(p), Some(EnglishToken::Number(_)))
                        });
                        let next_is_number =
                            matches!(tokens.get(j), Some(EnglishToken::Number(_)));
                        if chars.len() >= 2 && (prev_is_number || next_is_number) {
                            out.extend(super::rule_9::word_indicator(*form));
                            out.extend(super::rule_6::encode_number(&chars)?);
                            if next_is_number {
                                out.extend(super::rule_9::terminator(*form));
                            }
                        } else {
                            out.extend(super::rule_9::symbol_indicator(*form));
                            out.extend(super::rule_6::encode_number(&chars)?);
                        }
                    } else if chars.len() == 1 && !chars[0].is_ascii_alphabetic() {
                        // §9: a single styled punctuation/symbol mark (`.̲` → `⠸⠆⠲`,
                        // `%̲` → `⠸⠆⠨⠴`).
                        out.extend(super::rule_9::symbol_indicator(*form));
                        let cells = super::rule_7::encode_punctuation(chars[0])
                            .or_else(|| super::rule_3::encode_symbol(chars[0]))?;
                        out.extend(cells);
                    } else {
                        // Styled letters: passage / word / symbol level. The word
                        // span may reach past the contiguous run across attached
                        // punctuation (`𝑙'𝑜𝑒𝑖𝑙…`), so it distinguishes a true single
                        // styled letter from a multi-segment styled word. Passage
                        // detection opens a §9.x span before the per-word emit below.
                        let span_end = styled_word_span(tokens, i, *form);
                        if passage.is_none() {
                            let (words, end) = styled_passage_extent(tokens, i, *form);
                            if words >= 3 {
                                out.extend(super::rule_9::passage_indicator(*form));
                                // §8.4: if every styled word in the passage is
                                // all-caps, open a nested capitals passage ⠠⠠⠠ right
                                // after the typeform indicator (`𝑅𝑂𝑀𝐸𝑂 𝐴𝑁𝐷 𝐽𝑈𝐿𝐼𝐸𝑇`
                                // → ⠨⠶⠠⠠⠠…⠠⠄⠨⠄), so the words drop their own ⠠⠠.
                                let caps = styled_passage_all_caps(tokens, i, end, *form);
                                if caps {
                                    out.extend([CAPITAL, CAPITAL, CAPITAL]);
                                }
                                passage = Some((end, *form, caps));
                            }
                        }
                        if passage.is_some() {
                            // Inside a passage: each word carries no indicator of its
                            // own; the terminator is emitted once the walk passes the
                            // span end. A caps passage also suppresses per-word caps.
                            let caps_active = matches!(passage, Some((_, _, true)));
                            self.encode_styled_word(
                                &chars,
                                i,
                                j,
                                tokens,
                                in_passage[i] || caps_active,
                                &mut out,
                            )?;
                        } else if chars.len() == 1 && span_end == j {
                            out.extend(super::rule_9::symbol_indicator(*form));
                            // §5.7.1/§5.8.1: a single styled wordsign-letter standing
                            // alone (§2.6) takes a grade-1 indicator ⠰ — before any
                            // capital — so it is not read as the §10.1 wordsign (`𝑦`
                            // → `⠨⠆⠰⠽`); a/i/o letters carry no wordsign so are exempt
                            // (`𝑖` → `⠨⠆⠊`).
                            let prev = i.checked_sub(1).map(|p| &tokens[p]);
                            let next = tokens.get(j);
                            if super::rule_5_7::is_wordsign_letter(chars[0])
                                && is_standing_alone(prev, next)
                            {
                                out.push(GRADE1);
                            }
                            if chars[0].is_ascii_uppercase() {
                                out.push(CAPITAL);
                            }
                            out.push(
                                crate::english::encode_english(chars[0].to_ascii_lowercase())
                                    .ok()?,
                            );
                        } else {
                            // 2+ styled letters → one word indicator covering the
                            // whole space-delimited word. A hyphen/apostrophe-joined
                            // run of styled segments (`𝑜𝑓-𝑡ℎ𝑒`, `𝑙'𝑜𝑒𝑖𝑙-𝑑𝑒-𝑏𝑜𝑒𝑢𝑓`)
                            // stays under a single indicator (§9.5); a terminator
                            // closes it if the word continues plain (`𝐭𝐞𝐱𝐭book`,
                            // `a̲n̲d̲/or`).
                            out.extend(super::rule_9::word_indicator(*form));
                            if span_end > j {
                                self.encode_styled_span(
                                    i,
                                    span_end,
                                    *form,
                                    tokens,
                                    in_passage[i],
                                    &mut out,
                                )?;
                                run_end = span_end;
                            } else {
                                self.encode_styled_word(
                                    &chars,
                                    i,
                                    j,
                                    tokens,
                                    in_passage[i],
                                    &mut out,
                                )?;
                            }
                            if word_continues_after(tokens, run_end) {
                                out.extend(super::rule_9::terminator(*form));
                            }
                        }
                    }
                    skip_to = run_end;
                    prev_was_number = false;
                    numeric_mode = false;
                }
            }
            if cap_term[i] {
                // §8.4 capitals terminator ⠠⠄.
                out.extend([CAPITAL, decode_unicode('⠄')]);
            }
        }
        // §9.x: a passage reaching the end of the input still needs its terminator.
        if let Some((_, form, caps)) = passage {
            if caps {
                out.extend([CAPITAL, decode_unicode('⠄')]);
            }
            out.extend(super::rule_9::terminator(form));
        }
        Some(out)
    }

    /// §9: encode a styled word's base letters as an ordinary word (caps +
    /// contractions, with its standing-alone context taken from `tokens[i-1]` and
    /// `tokens[j]`) — the typeform indicator is emitted separately by the caller.
    fn encode_styled_word(
        &self,
        chars: &[char],
        i: usize,
        j: usize,
        tokens: &[EnglishToken],
        suppress_caps: bool,
        out: &mut Vec<u8>,
    ) -> Option<()> {
        let prev = i.checked_sub(1).map(|p| &tokens[p]);
        let next = tokens.get(j);
        let standing_alone = is_standing_alone(prev, next);
        let shortform_usable =
            standing_alone && !matches!(next, Some(EnglishToken::Symbol('@' | '/')));
        let lower_usable = standing_alone && lower_wordsign_usable(prev, next);
        self.encode_word(
            chars,
            WordContext {
                standing_alone,
                shortform_usable,
                lower_usable,
                suppress_caps,
                word_initial: word_initial_boundary(prev),
                digit_adjacent: false,
            },
            out,
        )
    }

    /// §9.5: encode a *multi-segment* styled word — its same-`form` styled letter
    /// runs (each as an ordinary word with its own §2.6 standing-alone context)
    /// and the symbols attached between them (`𝑜𝑓-𝑡ℎ𝑒` → `⠷⠤⠮`, `ℎ𝑡𝑡𝑝://…` →
    /// `⠓⠞⠞⠏⠒⠸⠌⠸⠌…`) — under the single typeform indicator emitted by the caller.
    fn encode_styled_span(
        &self,
        start: usize,
        span_end: usize,
        form: super::token::Typeform,
        tokens: &[EnglishToken],
        suppress_caps: bool,
        out: &mut Vec<u8>,
    ) -> Option<()> {
        let mut k = start;
        while k < span_end {
            match &tokens[k] {
                EnglishToken::Styled(_, f) if *f == form => {
                    let seg_start = k;
                    let mut seg_chars = Vec::new();
                    while k < span_end
                        && matches!(&tokens[k], EnglishToken::Styled(_, g) if *g == form)
                    {
                        if let EnglishToken::Styled(c, _) = &tokens[k] {
                            seg_chars.push(*c);
                        }
                        k += 1;
                    }
                    self.encode_styled_word(&seg_chars, seg_start, k, tokens, suppress_caps, out)?;
                }
                EnglishToken::Symbol(c) => {
                    let cells = super::rule_7::encode_punctuation(*c)
                        .or_else(|| super::rule_3::encode_symbol(*c))?;
                    out.extend(cells);
                    k += 1;
                }
                _ => return None,
            }
        }
        Some(())
    }

    /// §8 capital prefix + §10.1/§10.2 wordsigns (when standing alone) +
    /// §4.1/§10 contracted letters.
    fn encode_word(&self, chars: &[char], ctx: WordContext, out: &mut Vec<u8>) -> Option<()> {
        let WordContext {
            standing_alone,
            shortform_usable,
            lower_usable,
            suppress_caps,
            word_initial,
            digit_adjacent,
        } = ctx;
        // Unicode lowercase (so an accented/ligatured capital folds to its base —
        // `Œ`→`œ`, `À`→`à`), letting the §8 capital come from `classify_caps` while
        // the letter encodes without its own indicator (avoids a doubled `⠠`).
        let lower: Vec<char> = chars.iter().flat_map(|c| c.to_lowercase()).collect();
        let word: String = lower.iter().collect();
        // §8.2: a word with internal capitals (`HarperCollins`, `verY`) has no
        // single §8 caps pattern. Split it at each lower→upper boundary and give
        // every Title-case / all-caps part its own capital indicator. Parts not yet
        // modelled (a capital run *followed by* lowercase, e.g. `founDAtion`) leave
        // the whole word to the legacy path. Not applied inside a §8.4 caps passage.
        if !suppress_caps && classify_caps(chars).is_none() {
            return self.encode_mixed_case(chars, out);
        }
        if shortform_usable && super::rule_10_9::is_pure_shortform_abbreviation(&word) {
            out.push(GRADE1);
        }
        // Inside a §8.4 passage the ⠠⠠⠠ … ⠠⠄ carry capitalisation; `?` still guards
        // any residual mixed-case word there (→ legacy fallback).
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
        // §10.12.1: an all-caps initialism directly abutting a digit (`CH6`,
        // `W2N 6CH`) is "used as letters" — no contractions, each letter spelled.
        // Ordinal suffixes (`6TH`, `1ST`) keep their groupsign; a lowercase
        // digit-neighbour (`3rd`, `21st`) is not all-caps and never reaches here.
        // (A bare short acronym like `WHO`/`OED` also qualifies under §10.12.1, but
        // is structurally indistinguishable from a short §8 all-caps emphasis word
        // that DOES contract (`THE`/`SHE`) — a heuristic that suppresses contractions
        // there was measured to regress 9 passing cases for 11, so it is left out.)
        let acronym_as_letters = matches!(classify_caps(chars), Some(Caps::Word))
            && !matches!(
                lower.as_slice(),
                ['s', 't'] | ['n', 'd'] | ['r', 'd'] | ['t', 'h']
            )
            && digit_adjacent;
        if acronym_as_letters {
            for &c in &lower {
                match super::rule_4::accent_cells(c) {
                    Some(cells) => out.extend(cells),
                    None => out.push(crate::english::encode_english(c).ok()?),
                }
            }
            return Some(());
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
            word_initial,
        )?);
        Some(())
    }

    /// §8.2: encode a mixed-case word by splitting it at each lower→upper boundary
    /// (the start of a new Title-case / all-caps part) and giving every part its
    /// own capital indicator (`⠠` Title-case, `⠠⠠` all-caps). Contractions are
    /// computed per part, but the split is only used when it does **not** change
    /// them versus the whole word: a part that is itself a capital run + lowercase
    /// tail (`founDAtion`), or a part whose contraction context differs from the
    /// whole word (a restricted `dis`/`con`/`be` or a final groupsign that depends
    /// on word position), returns `None` so the legacy path handles the word.
    fn encode_mixed_case(&self, chars: &[char], out: &mut Vec<u8>) -> Option<()> {
        let whole_lower: Vec<char> = chars.iter().flat_map(|c| c.to_lowercase()).collect();
        // §8.2 mixed-case parts (`WALK`+`ing`) are mid-word continuations, never
        // word starts, so the §10.4.3 word-initial `ing` rule does not apply here.
        let whole = super::rule_10_9::encode_with_longer_shortforms(
            &whole_lower,
            &self.contractions,
            false,
        )?;

        let mut bounds = vec![0usize];
        for i in 1..chars.len() {
            // §8.2: a new Title-case / all-caps part begins at each lower→upper.
            let low_to_up = chars[i - 1].is_ascii_lowercase() && chars[i].is_ascii_uppercase();
            // §8.6.3: split a *caps word* (≥2 capitals) from a following lowercase
            // run so its `⠠⠄` terminator can be emitted (`ABCs`, `unSELFish`). A lone
            // Title-case capital keeps its lowercase tail, so contractions there keep
            // their context (`Deaf`'s `ea`, `Perfect`'s `er`).
            let capsword_to_low = chars[i].is_ascii_lowercase()
                && chars[i - 1].is_ascii_uppercase()
                && i >= 2
                && chars[i - 2].is_ascii_uppercase();
            if low_to_up || capsword_to_low {
                bounds.push(i);
            }
        }
        bounds.push(chars.len());

        let mut buf = Vec::new();
        let mut concat = Vec::new();
        let mut prev_caps_word = false;
        let mut has_caps_word_segment = false;
        for w in bounds.windows(2) {
            let seg = &chars[w[0]..w[1]];
            let seg_lower: Vec<char> = seg.iter().flat_map(|c| c.to_lowercase()).collect();
            let cells = super::rule_10_9::encode_with_longer_shortforms(
                &seg_lower,
                &self.contractions,
                false,
            )?;
            let caps = classify_caps(seg)?;
            has_caps_word_segment |= matches!(caps, Caps::Word);
            // §8.6.3: a §8.4 caps word (`⠠⠠`) is terminated by `⠠⠄` before lowercase
            // letters that continue the same word (`ABCs`, `WALKing`, `unSELFish`).
            if prev_caps_word && matches!(caps, Caps::None) {
                buf.push(CAPITAL);
                buf.push(decode_unicode('⠄'));
            }
            match caps {
                Caps::None => {}
                Caps::Single => buf.push(CAPITAL),
                Caps::Word => {
                    buf.push(CAPITAL);
                    buf.push(CAPITAL);
                }
            }
            buf.extend(&cells);
            concat.extend(cells);
            prev_caps_word = matches!(caps, Caps::Word);
        }
        // §8.2 / §10.12.12: a lower→upper case boundary breaks a contraction that
        // would span it, so the per-part split is the correct reading (`NorthEast`
        // → `North`+`East`, not the boundary-spanning `the`; `CliffEdge` → component
        // `f·f`, not the medial `ff` groupsign). When the split differs from the
        // whole word it is therefore *preferred* — UNLESS a part is an all-caps run
        // (`founDAtion`'s `DA`), whose caps pattern and position-sensitive groupsigns
        // (`tion`) are not yet modelled part-wise: those defer to the legacy path.
        if concat != whole && has_caps_word_segment {
            return None;
        }
        out.extend(buf);
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
    // §10.12.2: the lone wordsign letter `b` in running text takes a grade-1 ⠰.
    #[case::two_words("a b", vec![decode_unicode('⠁'), SPACE, GRADE1, decode_unicode('⠃')])]
    #[case::number_then_az_letter("5a", vec![decode_unicode('⠼'), decode_unicode('⠑'), GRADE1, decode_unicode('⠁')])]
    #[case::word_space_number("a 50", vec![decode_unicode('⠁'), SPACE, decode_unicode('⠼'), decode_unicode('⠑'), decode_unicode('⠚')])]
    fn encodes_supported_words(#[case] text: &str, #[case] expected: Vec<u8>) {
        assert_eq!(enc(text), Some(expected));
    }

    /// `founDAtion` still defers to the legacy path (`None`): it has an internal
    /// all-caps run before lowercase (`DA`) whose caps pattern and the
    /// position-sensitive `tion` groupsign are not yet modelled part-wise (§8.2).
    /// A mixed-case word whose parts are all Title-case is split instead — see
    /// `CliffEdge`/`NorthEast` in `encodes_mixed_case_words_8_2`.
    #[rstest::rstest]
    #[case::caps_run_then_lower("founDAtion")]
    fn unsupported_inputs_return_none(#[case] text: &str) {
        assert_eq!(enc(text), None);
    }

    /// §8.2: a mixed-case word (internal capitals) is split at each lower→upper
    /// boundary and each Title-case / all-caps part takes its own capital
    /// indicator (`⠠` single, `⠠⠠` all-caps), contractions applying within each.
    #[rstest::rstest]
    #[case::mcd("McD", "⠠⠍⠉⠠⠙")]
    #[case::trailing_single_cap("verY", "⠧⠻⠠⠽")]
    #[case::trailing_caps_word("grandEST", "⠛⠗⠯⠠⠠⠑⠌")]
    // §8.2/§10.12.12: Title-case parts split even when a contraction would span the
    // boundary in the whole word (`ff` in `cliffedge`, `the` in `northeast`).
    #[case::cliff_edge_title_split("CliffEdge", "⠠⠉⠇⠊⠋⠋⠠⠫⠛⠑")]
    #[case::north_east_title_split("NorthEast", "⠠⠝⠕⠗⠹⠠⠑⠁⠌")]
    fn encodes_mixed_case_words_8_2(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §8.6.3: a §8.4 caps word (`⠠⠠`) followed by lowercase letters continuing the
    /// same word takes the capitals terminator `⠠⠄` before the lowercase part
    /// (`ABCs`, `WALKing`, `unSELFish`, `OKd`); a lone Title-case capital does not
    /// (covered by `verY`/`CliffEdge` above, which keep their lowercase context).
    #[rstest::rstest]
    #[case::abcs("ABCs", "⠠⠠⠁⠃⠉⠠⠄⠎")]
    #[case::walking("WALKing", "⠠⠠⠺⠁⠇⠅⠠⠄⠬")]
    #[case::un_self_ish("unSELFish", "⠥⠝⠠⠠⠎⠑⠇⠋⠠⠄⠊⠩")]
    #[case::okd("OKd", "⠠⠠⠕⠅⠠⠄⠙")]
    fn encodes_caps_word_terminator_8_6_3(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §3.24: a digit super/subscript following a base takes the level indicator
    /// (`⠔`/`⠢`). The grade-1 `⠰` precedes it after a letter base (`yd³`, `B₁₂`,
    /// `clarion¹`) but not after a number (`1682.³`), whose numeric mode covers it.
    #[rstest::rstest]
    #[case::super_after_word("3 yd\u{00B3}", "⠼⠉⠀⠽⠙⠰⠔⠼⠉")]
    #[case::sub_after_letter("vitamin B\u{2081}\u{2082}", "⠧⠊⠞⠁⠍⠔⠀⠠⠃⠰⠢⠼⠁⠃")]
    #[case::super_after_number("born in 1682.\u{00B3}", "⠃⠕⠗⠝⠀⠔⠀⠼⠁⠋⠓⠃⠲⠔⠼⠉")]
    #[case::super_after_word_inline("the clarion\u{00B9} horn", "⠮⠀⠉⠇⠜⠊⠕⠝⠰⠔⠼⠁⠀⠓⠕⠗⠝")]
    fn encodes_script_3_24(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §3.24 boundary: a *leading* super/subscript (no base before it) fails the
    /// whole UEB attempt so the legacy/math path keeps ownership — this is what
    /// protects combinatorics like `₇𝑃₂` (제18/19항) from being misread as §3.24.
    #[rstest::rstest]
    #[case::leading_superscript("\u{00B9} clarion")]
    #[case::leading_subscript_combinatorics("2 \u{2087}\u{1D443}\u{2082}")]
    fn leading_script_delegates_to_legacy(#[case] text: &str) {
        assert_eq!(enc(text), None);
    }

    /// §3.27: `[open tn]` / `[close tn]` markers become the note indicators
    /// `⠈⠨⠣` / `⠈⠨⠜`; a plain bracket that is not the marker keeps its sign.
    #[rstest::rstest]
    #[case::wrapped_note("[open tn]cat[close tn]", "⠈⠨⠣⠉⠁⠞⠈⠨⠜")]
    #[case::plain_bracket_unchanged("[cat]", "⠨⠣⠉⠁⠞⠨⠜")]
    fn encodes_transcriber_notes_3_27(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §16.2 horizontal line mode: a run of box-drawing characters opens with
    /// `⠐⠒` (a leading `─` folding into the indicator's `⠒`) and maps each further
    /// char to its segment/corner/crossing cell.
    #[rstest::rstest]
    #[case::solid("\u{2500}\u{2500}\u{2500}\u{2500}", "⠐⠒⠒⠒⠒")]
    #[case::triple("\u{2261}\u{2261}\u{2261}", "⠐⠒⠿⠿⠿")]
    #[case::corners(
        "\u{2500}\u{2500}\u{2500}\u{2500}\u{2534}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}",
        "⠐⠒⠒⠒⠒⠚⠒⠒⠒⠒⠲"
    )]
    #[case::diagonals("\u{2572}\u{2500}\u{2571}", "⠐⠒⠣⠒⠜")]
    // §16.2.5: text mid-line takes the terminator `⠄`; the next run re-opens `⠐⠒`.
    #[case::text_midpoint(
        "\u{2500}\u{2500}\u{2500}\u{2500}cat\u{2500}\u{2500}\u{2500}\u{2500}",
        "⠐⠒⠒⠒⠒⠄⠉⠁⠞⠐⠒⠒⠒⠒"
    )]
    fn encodes_box_drawing_16_2(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §16.2: a lone box-drawing char (a single mathematical `≡` or `─`) is not a
    /// line run, so the UEB engine declines it and the legacy/math meaning stands.
    #[rstest::rstest]
    #[case::lone_hline("\u{2500}")]
    #[case::lone_triple("\u{2261}")]
    fn lone_box_char_is_not_line_mode(#[case] text: &str) {
        assert_eq!(enc(text), None);
    }

    /// §5.7.1: a single letter that is an alphabetic wordsign takes a grade-1
    /// indicator ⠰ when it stands alone abutting a dash or a *free-standing*
    /// bracket, so it is not misread as the wordsign (§5.8.1 places it before any
    /// capital). Space/edge bounds (`a b`, covered above), abbreviation dots
    /// (`U.S.A.`) and brackets attached to an adjacent word (`noun(s)`) keep the
    /// bare cell. Expected cells are taken from RUEB §5.7.1 / §7.1 examples.
    #[rstest::rstest]
    #[case::after_hyphen("b-1", "⠰⠃⠤⠼⠁")]
    #[case::free_standing_paren("(h)", "⠐⠣⠰⠓⠐⠜")]
    #[case::attached_paren("noun(s)", "⠝⠳⠝⠐⠣⠎⠐⠜")]
    #[case::abbreviation_dots("U.S.A.", "⠠⠥⠲⠠⠎⠲⠠⠁⠲")]
    #[case::period_ends_run("p. 7", "⠰⠏⠲⠀⠼⠛")]
    #[case::abbreviation_dot_digit("p.7", "⠏⠲⠼⠛")]
    fn grade1_single_letter_5_7_1(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §9: a styled letter takes a symbol-level typeform indicator before its base
    /// cell (italic ⠨⠆, bold ⠘⠆, underline ⠸⠆) and is a contraction boundary, so
    /// the plain neighbours still contract (`story̲` keeps its `st` groupsign).
    #[rstest::rstest]
    #[case::italic_math_alpha("\u{1D45D}neumonia", "⠨⠆⠏⠝⠑⠥⠍⠕⠝⠊⠁")]
    #[case::bold_math_alpha("\u{1D41B}at", "⠘⠆⠃⠁⠞")]
    #[case::underline_combining("story\u{0332}", "⠌⠕⠗⠸⠆⠽")]
    fn typeform_symbol_indicator_9(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §9.x: a run of 2+ styled letters takes a word indicator (`⠨⠂`) with the
    /// word contracted normally (`𝑅𝑎𝑑𝑎𝑟` → `⠨⠂⠠⠗⠁⠙⠜`, `ar` groupsign); a partial
    /// run ending mid-word adds a terminator (`𝐭𝐞𝐱𝐭book` → `⠘⠂⠞⠑⠭⠞⠘⠄…`).
    #[rstest::rstest]
    #[case::italic_whole_word("the \u{1D445}\u{1D44E}\u{1D451}\u{1D44E}\u{1D45F}", "⠮⠀⠨⠂⠠⠗⠁⠙⠜")]
    #[case::bold_partial_then_plain("\u{1D42D}\u{1D41E}\u{1D431}\u{1D42D}book", "⠘⠂⠞⠑⠭⠞⠘⠄⠃⠕⠕⠅")]
    fn typeform_word_indicator_9(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §9.x: three or more same-form styled words take a single passage indicator
    /// (`⠨⠶`) and terminator (`⠨⠄`), each word encoded normally between them — the
    /// shortforms `ab`/`cd` (about/could) still keep their §5.7.2 grade-1 `⠰`.
    #[rstest::rstest]
    #[case::italic_passage(
        "go \u{1D44E}\u{1D44F} \u{1D450}\u{1D451} \u{1D452}\u{1D453}",
        "⠛⠀⠨⠶⠰⠁⠃⠀⠰⠉⠙⠀⠑⠋⠨⠄"
    )]
    fn typeform_passage_indicator_9(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §9 over digits/symbols: a styled *number* is one symbol-sequence — a single
    /// symbol indicator then the whole number (`3̲4̲` → `⠸⠆⠼⠉⠙`, bold `5𝟓` →
    /// `⠼⠑⠘⠆⠼⠑`); a single styled punctuation/symbol mark takes the symbol
    /// indicator then its cells, restarting numeric mode after (`27.̲9` →
    /// `⠼⠃⠛⠸⠆⠲⠼⠊`, `83%̲` → `⠼⠓⠉⠸⠆⠨⠴`). U+0332 underlines the preceding char.
    #[rstest::rstest]
    #[case::underline_digit_run("3\u{0332}4\u{0332}", "⠸⠆⠼⠉⠙")]
    #[case::bold_digit_after_plain("5\u{1D7D3}", "⠼⠑⠘⠆⠼⠑")]
    #[case::underline_period_between_digits("27.\u{0332}9", "⠼⠃⠛⠸⠆⠲⠼⠊")]
    #[case::underline_percent("83%\u{0332}", "⠼⠓⠉⠸⠆⠨⠴")]
    fn typeform_styled_digits_symbols_9(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §9 + §5.7.1: a single styled letter standing alone (§2.6) takes a grade-1
    /// indicator after its typeform symbol indicator when it is an alphabetic
    /// wordsign (`𝑦` italic → `⠨⠆⠰⠽`), but not for the a/i/o letters which carry no
    /// wordsign (`𝑖` → `⠨⠆⠊`).
    #[rstest::rstest]
    #[case::italic_y_wordsign("\u{1D466}", "⠨⠆⠰⠽")]
    #[case::italic_i_exempt("\u{1D456}", "⠨⠆⠊")]
    fn typeform_single_letter_grade1_9(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §9.5: a *word* typeform indicator is terminated when the emphasis ends
    /// before the space-delimited word does — including across attached
    /// punctuation, so the underlined `and` in `a̲n̲d̲/or` closes with `⠸⠄` before
    /// the plain `/or` completes the word.
    #[rstest::rstest]
    #[case::underline_word_then_slash_word("a\u{0332}n\u{0332}d\u{0332}/or", "⠸⠂⠯⠸⠄⠸⠌⠕⠗")]
    fn typeform_word_terminator_continues_9(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §9.x: a typeform *passage* keeps a trailing full stop inside its span, but a
    /// trailing dash separates it from following text, so the terminator falls
    /// after the stop and *before* the dash (`…𝑒𝑓.—` → `…⠑⠋⠲⠨⠄⠠⠤`).
    #[rstest::rstest]
    #[case::passage_stop_then_dash(
        "go \u{1D44E}\u{1D44F} \u{1D450}\u{1D451} \u{1D452}\u{1D453}.\u{2014}",
        "⠛⠀⠨⠶⠰⠁⠃⠀⠰⠉⠙⠀⠑⠋⠲⠨⠄⠠⠤"
    )]
    fn typeform_passage_dash_boundary_9(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §9.5: a styled word whose emphasis spans internal punctuation (hyphen,
    /// apostrophe) takes ONE word indicator over the whole space-delimited word —
    /// even when the first styled segment is a single letter (`𝑙'𝑜…`). The plain
    /// neighbours (`out-…-way`) stay outside, with a terminator where it ends.
    #[rstest::rstest]
    #[case::hyphen_joined_of_the(
        "out-\u{1D45C}\u{1D453}-\u{1D461}\u{210E}\u{1D452}-way",
        "⠳⠤⠨⠂⠷⠤⠮⠨⠄⠤⠺⠁⠽"
    )]
    // §10.12.2: the trailing lone wordsign letter `z` in running text takes grade-1 ⠰.
    #[case::apostrophe_single_first_segment("\u{1D459}'\u{1D45C} z", "⠨⠂⠇⠄⠕⠀⠰⠵")]
    fn typeform_multi_segment_word_9(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// `try_encode` owns letter-containing input and §9-styled input; a *plain*
    /// number/symbol run with no ASCII letter and no styling (no math-alphanumeric
    /// or combining underline) is delegated to the legacy path — the `encode()`
    /// precondition in `encoder.rs` mirrors this via `is_ueb_eligible` — so the
    /// engine returns `None`. Number/symbol encoding itself is covered by `5a`/
    /// `a 50`, the styled-digit cases above, and the testcase suite.
    #[rstest::rstest]
    #[case::number("95")]
    #[case::percent("5%")]
    #[case::comma_grouped("5,70")]
    #[case::decimal("4.2")]
    fn non_letter_input_delegated_to_legacy(#[case] text: &str) {
        assert_eq!(enc(text), None);
    }

    #[rstest::rstest]
    #[case::word_period("cat.", vec![decode_unicode('⠉'), decode_unicode('⠁'), decode_unicode('⠞'), decode_unicode('⠲')])]
    #[case::wordsign_us_question("us?", vec![decode_unicode('⠥'), decode_unicode('⠦')])]
    #[case::double_quotes("\"a\"", vec![QUOTE_OPEN, decode_unicode('⠁'), QUOTE_CLOSE])]
    fn encodes_punctuation_and_symbols(#[case] text: &str, #[case] expected: Vec<u8>) {
        assert_eq!(enc(text), Some(expected));
    }

    /// §7.1.3: a lower-cell punctuation mark whose cell collides with a lower
    /// contraction takes a grade-1 indicator ⠰ where that contraction could be read
    /// — a standing-alone `?` (⠦/his), a word-internal `:` (⠒/con), a word-initial
    /// `.` (⠲/dis). It stays bare in its plain terminal position (`us?`, `cat.`
    /// above) and as an abbreviation dot (`U.S.A.`, covered by §5.7.1 tests).
    #[rstest::rstest]
    #[case::colon_between_words("a:o", "⠁⠰⠒⠕")]
    #[case::colon_in_word("lang:uk", "⠇⠁⠝⠛⠰⠒⠥⠅")]
    #[case::word_initial_period(".doc", "⠰⠲⠙⠕⠉")]
    #[case::standalone_question("cat ? dog", "⠉⠁⠞⠀⠰⠦⠀⠙⠕⠛")]
    #[case::embedded_exclamation("Ai!!ams", "⠠⠁⠊⠰⠖⠖⠁⠍⠎")]
    fn punctuation_grade1_7_1_3(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §7.6: a *curly* single quote is an opening (`⠠⠦`) or closing (`⠠⠴`) single
    /// quotation mark only as part of a matched pair; an unmatched right curly is a
    /// word-final apostrophe (`⠄`). The straight `'` is ambiguous in print
    /// (`'Hamlet'` vs `'display will minimise'`) so it always stays an apostrophe.
    /// §7.6.10: a single quote *detached* from its text by a space (or referenced
    /// in isolation) takes a leading grade-1 indicator `⠰`.
    #[rstest::rstest]
    #[case::curly_pair_is_quotation("\u{2018}cat\u{2019}", "⠠⠦⠉⠁⠞⠠⠴")]
    #[case::unmatched_curly_close_is_apostrophe("cats\u{2019}", "⠉⠁⠞⠎⠄")]
    #[case::straight_quotes_stay_apostrophe("'cat'", "⠄⠉⠁⠞⠄")]
    #[case::detached_open_takes_grade1("\u{2018} cat\u{2019}", "⠰⠠⠦⠀⠉⠁⠞⠠⠴")]
    #[case::detached_close_takes_grade1("\u{2018}cat \u{2019}", "⠠⠦⠉⠁⠞⠀⠰⠠⠴")]
    #[case::standalone_close_takes_grade1("cat \u{2019} dog", "⠉⠁⠞⠀⠰⠠⠴⠀⠙⠕⠛")]
    fn encodes_single_quotes_7_6(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §7.6.10: a double quotation mark standing alone (space/edge both sides) is
    /// the mark referenced in isolation → ⠰⠠⠶, without flipping the open/close
    /// alternation; a normal dialogue pair still toggles ⠦ … ⠴.
    #[rstest::rstest]
    #[case::standalone_double_quote("cat \" dog", "⠉⠁⠞⠀⠰⠠⠶⠀⠙⠕⠛")]
    #[case::dialogue_double_quote_toggles("\"cat\"", "⠦⠉⠁⠞⠴")]
    fn encodes_standalone_double_quote_7_6_10(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §7.6 matched-pair classification: a left curly `‘` opens; a right curly `’`
    /// closes when it matches an open, otherwise is an apostrophe; a `’` between two
    /// words is an apostrophe.
    #[test]
    fn single_quote_roles_classifies_curly_pairs() {
        // ‘cat’ → Open … Close.
        let roles = single_quote_roles(&[
            EnglishToken::Symbol('\u{2018}'),
            EnglishToken::Word(vec!['c', 'a', 't']),
            EnglishToken::Symbol('\u{2019}'),
        ]);
        assert_eq!(roles[0], SingleQuote::Open);
        assert_eq!(roles[2], SingleQuote::Close);
        // cats’ (unmatched right curly) → Apostrophe.
        let roles = single_quote_roles(&[
            EnglishToken::Word(vec!['c', 'a', 't', 's']),
            EnglishToken::Symbol('\u{2019}'),
        ]);
        assert_eq!(roles[1], SingleQuote::Apostrophe);
        // o’clock (right curly between two words) → Apostrophe.
        let roles = single_quote_roles(&[
            EnglishToken::Word(vec!['o']),
            EnglishToken::Symbol('\u{2019}'),
            EnglishToken::Word(vec!['c', 'l', 'o', 'c', 'k']),
        ]);
        assert_eq!(roles[1], SingleQuote::Apostrophe);
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

    /// §6.3: within letter-containing input the numeric indicator `⠼` restarts
    /// after a letter splits a digit run. (Pure-number inputs with `,`/`.`
    /// separators have no ASCII letter and are delegated to the legacy path — see
    /// `non_letter_input_delegated_to_legacy`.)
    #[rstest::rstest]
    // `4x4` — a letter splits the run, so each number keeps its own `⠼`
    // (grade-1 ⠰ guards an a–j letter; `x` is not a–j, so no ⠰).
    #[case::letter_split("4x4", vec![decode_unicode('⠼'), decode_unicode('⠙'), decode_unicode('⠭'), decode_unicode('⠼'), decode_unicode('⠙')])]
    fn numeric_indicator_restarts_after_letter(#[case] text: &str, #[case] expected: Vec<u8>) {
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

    /// §8.4 with §4.2: caps detection is Unicode-aware, so a word whose capitals
    /// include an accented or ligatured letter is still a whole-word caps (`⠠⠠`),
    /// and the letter folds to its lowercase base before encoding — no doubled
    /// capital (`AOÛT` → `⠠⠠⠁⠕⠘⠩⠥⠞`, `ŒDIPUS` → `⠠⠠⠕⠘⠖⠑⠙⠊⠏⠥⠎`).
    #[rstest::rstest]
    #[case::accented_caps_word("AOÛT", "⠠⠠⠁⠕⠘⠩⠥⠞")]
    #[case::ligature_caps_word("ŒDIPUS", "⠠⠠⠕⠘⠖⠑⠙⠊⠏⠥⠎")]
    fn unicode_caps_word_8_4(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }
}
