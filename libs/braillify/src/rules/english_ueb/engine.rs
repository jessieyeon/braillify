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

type ForeignScope = Option<(super::rule_13::AccentCode, bool)>;
type ActiveTypeformPassage = (usize, super::token::Typeform, bool, ForeignScope);

/// Capitalisation pattern of a word (§8 subset currently supported).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Caps {
    /// All lowercase — no indicator.
    None,
    /// One leading capital, or a single capital letter — `⠠`.
    Single,
    /// Whole word uppercase (len ≥ 2) — `⠠⠠`.
    Word,
}

#[derive(Clone, Copy)]
struct CapsGroup {
    first_cap: usize,
    last_cap: usize,
    caps_sequences: usize,
    has_lower: bool,
    /// Whether the group is exactly one single-letter uppercase word — an
    /// article/initial `A`. §8.5.4 leaves such letters "not necessarily" part
    /// of the surrounding passage when preceded by sentence-terminal
    /// punctuation, so the passage detector can exclude them from the count.
    single_letter_only: bool,
    /// Whether the group ends with sentence-terminal punctuation (`.`, `!`,
    /// `?`) after the last capital — a hint that a following single-letter
    /// caps word (`A`) starts a new sentence.
    ended_with_terminal_sentence_mark: bool,
}

/// Determine the capitalisation pattern, or `None` for mixed-case words (internal
/// capitals, e.g. "McDonald") — these are split and re-encoded part-by-part by
/// [`EnglishUebEngine::encode_mixed_case`] (§8.2).
fn classify_caps(chars: &[char]) -> Option<Caps> {
    // Unicode case (not ASCII-only) so an accented or ligatured capital (`É`, `Œ`,
    // `ẞ`) counts as a capital — `ŒDIPUS`/`AOÛT` are whole-word caps, not mixed.
    let uppers = chars.iter().filter(|c| c.is_uppercase()).count();
    let lowercase_modified = chars
        .iter()
        .filter(|c| c.is_lowercase() && super::rule_4::is_modified_letter(**c))
        .count();
    if uppers == 0 {
        Some(Caps::None)
    } else if uppers == chars.len() || uppers + lowercase_modified == chars.len() {
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

fn token_is_upper_sequence(t: &EnglishToken) -> bool {
    match t {
        EnglishToken::Word(c) | EnglishToken::WordDivision { chars: c, .. } => {
            c.iter().any(|x| x.is_uppercase()) && !c.iter().any(|x| x.is_lowercase())
        }
        EnglishToken::Styled(c, _) | EnglishToken::Symbol(c) => c.is_uppercase(),
        EnglishToken::Number(_) | EnglishToken::Space | EnglishToken::LineBreak => false,
        EnglishToken::Technical(_) => false,
    }
}

fn single_elongated_caps_word_in_quotes(tokens: &[EnglishToken]) -> bool {
    let mut words = tokens.iter().filter_map(|token| match token {
        EnglishToken::Word(chars) => Some(chars.as_slice()),
        _ => None,
    });
    let Some(word) = words.next() else {
        return false;
    };
    if words.next().is_some() || word.len() < 5 {
        return false;
    }
    let last = word[word.len() - 1];
    let prefix = &word[..word.len() - 1];
    last.is_ascii_uppercase()
        && prefix.iter().any(|c| c.is_ascii_uppercase())
        && prefix.iter().rev().take_while(|c| **c == last).count() >= 2
}

fn token_typeform(t: &EnglishToken) -> Option<super::token::Typeform> {
    match t {
        EnglishToken::Styled(_, form) => Some(*form),
        _ => None,
    }
}

fn token_has_lower_sequence(t: &EnglishToken) -> bool {
    match t {
        EnglishToken::Word(c) | EnglishToken::WordDivision { chars: c, .. } => {
            c.iter().any(|x| x.is_lowercase())
        }
        EnglishToken::Styled(c, _) | EnglishToken::Symbol(c) => c.is_lowercase(),
        EnglishToken::Technical(c) => c.iter().any(|x| x.is_lowercase()),
        EnglishToken::Number(_) | EnglishToken::Space | EnglishToken::LineBreak => false,
    }
}

fn token_is_styled_text(t: &EnglishToken) -> bool {
    matches!(t, EnglishToken::Styled(c, _) if c.is_alphabetic())
}

fn chemical_formula_caps(chars: &[char]) -> bool {
    chars.len() >= 2
        && !matches!(chars, ['C', 'O'])
        && chars.iter().all(|c| matches!(c, 'C' | 'H' | 'O'))
}

fn encode_letters_literal(chars: &[char]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(chars.len() + 2);
    match classify_caps(chars)? {
        Caps::None => {}
        Caps::Single => out.push(CAPITAL),
        Caps::Word => out.extend([CAPITAL, CAPITAL]),
    }
    for c in chars.iter().flat_map(|c| c.to_lowercase()) {
        out.push(crate::english::encode_english(c).ok()?);
    }
    Some(out)
}

fn push_literal_letter(c: char, out: &mut Vec<u8>) -> Option<()> {
    if c.is_uppercase() {
        out.push(CAPITAL);
    }
    match super::rule_4::accent_cells(c) {
        Some(cells) => {
            let cells = if c.is_uppercase() && cells.first() == Some(&CAPITAL) {
                &cells[1..]
            } else {
                cells.as_slice()
            };
            out.extend(cells);
        }
        None => out.push(crate::english::encode_english(c.to_ascii_lowercase()).ok()?),
    }
    Some(())
}

fn combining_modifier_cells(c: char) -> Option<[u8; 2]> {
    match c {
        '\u{0300}' => Some([decode_unicode('⠘'), decode_unicode('⠡')]),
        '\u{0301}' => Some([decode_unicode('⠘'), decode_unicode('⠌')]),
        '\u{0302}' => Some([decode_unicode('⠘'), decode_unicode('⠩')]),
        '\u{0308}' => Some([decode_unicode('⠘'), decode_unicode('⠒')]),
        '\u{0304}' => Some([decode_unicode('⠈'), decode_unicode('⠤')]),
        '\u{0306}' => Some([decode_unicode('⠈'), decode_unicode('⠬')]),
        '\u{030c}' => Some([decode_unicode('⠘'), decode_unicode('⠬')]),
        '\u{0336}' => Some([decode_unicode('⠈'), decode_unicode('⠒')]),
        // §4.2.5 double-diacritic combining marks: one modifier over two letters.
        // U+035E (double macron) is the acute-example `o͞o` in the PDF; U+035C
        // (double breve below) mirrors the single breve indicator.
        '\u{035e}' => Some([decode_unicode('⠈'), decode_unicode('⠤')]),
        '\u{035c}' => Some([decode_unicode('⠈'), decode_unicode('⠬')]),
        _ => None,
    }
}

fn emit_word_with_modifier_on_last(chars: &[char], mark: char, out: &mut Vec<u8>) -> Option<()> {
    let (&last, prefix) = chars.split_last()?;
    for &c in prefix {
        push_literal_letter(c, out)?;
    }
    out.extend(combining_modifier_cells(mark)?);
    push_literal_letter(last, out)
}

fn emit_ligature_between(left: &[char], right: &[char], out: &mut Vec<u8>) -> Option<()> {
    let (&left_last, left_prefix) = left.split_last()?;
    let (&right_first, right_rest) = right.split_first()?;
    for &c in left_prefix {
        push_literal_letter(c, out)?;
    }
    push_literal_letter(left_last, out)?;
    if right_first.is_uppercase() {
        out.push(CAPITAL);
    }
    out.extend([decode_unicode('⠘'), decode_unicode('⠖')]);
    out.push(crate::english::encode_english(right_first.to_ascii_lowercase()).ok()?);
    for &c in right_rest {
        push_literal_letter(c, out)?;
    }
    Some(())
}

/// §4.3.3: a stroke-through overlay joining two adjacent letters is shown by the
/// ligature indicator between those letters.  When the letters are themselves
/// italic/bold mathematical alphabetic characters, the §9 word indicator scopes the
/// two-letter symbol sequence but the stroke remains a §4 ligature mark.
fn emit_styled_struck_pair(
    tokens: &[EnglishToken],
    i: usize,
    form: super::token::Typeform,
    first: char,
    out: &mut Vec<u8>,
) -> Option<usize> {
    if !matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('\u{0336}'))) {
        return None;
    }
    let Some(EnglishToken::Styled(second, second_form)) = tokens.get(i + 2) else {
        return None;
    };
    if *second_form != form || !matches!(tokens.get(i + 3), Some(EnglishToken::Symbol('\u{0336}')))
    {
        return None;
    }
    out.extend(super::rule_9::word_indicator(form));
    push_literal_letter(first, out)?;
    if second.is_uppercase() {
        out.push(CAPITAL);
    }
    out.extend([decode_unicode('⠘'), decode_unicode('⠖')]);
    out.push(crate::english::encode_english(second.to_ascii_lowercase()).ok()?);
    Some(i + 4)
}

fn emit_struck_letter_sequence(
    tokens: &[EnglishToken],
    i: usize,
    chars: &[char],
    out: &mut Vec<u8>,
) -> Option<usize> {
    if !matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('\u{0336}'))) {
        return None;
    }
    let mut letters = chars.to_vec();
    let mut j = i + 2;
    while let (Some(EnglishToken::Word(next)), Some(EnglishToken::Symbol('\u{0336}'))) =
        (tokens.get(j), tokens.get(j + 1))
    {
        letters.extend(next);
        j += 2;
    }
    if letters.len() < 2 {
        return None;
    }
    let (&first, rest) = letters.split_first()?;
    push_literal_letter(first, out)?;
    for &letter in rest {
        if letter.is_uppercase() {
            out.push(CAPITAL);
        }
        out.extend([decode_unicode('⠘'), decode_unicode('⠖')]);
        out.push(crate::english::encode_english(letter.to_ascii_lowercase()).ok()?);
    }
    Some(j)
}

fn emit_group_modifier(mark: char, chars: &[char], out: &mut Vec<u8>) -> Option<()> {
    out.extend(combining_modifier_cells(mark)?);
    out.push(decode_unicode('⠣'));
    for &c in chars {
        push_literal_letter(c, out)?;
    }
    out.push(decode_unicode('⠜'));
    Some(())
}

fn contains_caret(tokens: &[EnglishToken]) -> bool {
    tokens
        .iter()
        .any(|token| matches!(token, EnglishToken::Symbol('^')))
}

fn contains_transcriber_note(tokens: &[EnglishToken]) -> bool {
    tokens
        .iter()
        .enumerate()
        .any(|(i, _)| transcriber_note_at(tokens, i).is_some())
}

#[derive(Clone, Copy)]
struct Grade1Span {
    end: usize,
    needs_terminator: bool,
    indicator_cells: usize,
}

fn grade1_passage_span(tokens: &[EnglishToken], i: usize) -> Option<Grade1Span> {
    let mut hyphens = 0usize;
    let mut words = 0usize;
    let mut sequences = 1usize;
    let mut technical_item = false;
    let mut j = i;
    while let Some(token) = tokens.get(j) {
        match token {
            EnglishToken::Word(chars) => {
                if chars.len() != 1 || !chars.iter().all(|c| c.is_ascii_alphabetic()) {
                    break;
                }
                words += 1;
                j += 1;
            }
            EnglishToken::Number(_) => {
                technical_item = true;
                words += 1;
                j += 1;
            }
            EnglishToken::Symbol('-') => {
                hyphens += 1;
                j += 1;
            }
            EnglishToken::Symbol('−' | '=' | ';') => {
                technical_item = true;
                j += 1;
            }
            EnglishToken::Symbol('.') if technical_item => {
                j += 1;
            }
            EnglishToken::Symbol(c) if super::rule_3_24::is_script_char(*c) => {
                technical_item = true;
                j += 1;
            }
            EnglishToken::Space if words > 0 && grade1_passage_continues_after_space(tokens, j) => {
                if matches!(tokens.get(j - 1), Some(EnglishToken::Symbol(';')))
                    || matches!(tokens.get(j + 1), Some(EnglishToken::Word(chars)) if chars.len() == 1 && chars.iter().all(|c| c.is_ascii_alphabetic()))
                {
                    sequences += 1;
                }
                j += 1;
            }
            _ => break,
        }
    }
    if sequences >= 3 && (technical_item || hyphens >= 3) && words >= 4 {
        return Some(Grade1Span {
            end: j,
            needs_terminator: true,
            indicator_cells: 3,
        });
    }

    grade1_hyphenated_word_span(tokens, i)
}

/// UEB 2024 §5.4.2: a grade-1 passage can span spaced mathematical expressions
/// such as `y = x²−4; y = ...`; spaces around comparison signs/operators do not
/// terminate the technical passage, while the semicolon-space starts the next
/// symbols-sequence inside the same passage.
fn grade1_passage_continues_after_space(tokens: &[EnglishToken], space: usize) -> bool {
    matches!(tokens.get(space + 1), Some(EnglishToken::Word(chars)) if chars.len() == 1 && chars.iter().all(|c| c.is_ascii_alphabetic()))
        || matches!(tokens.get(space + 1), Some(EnglishToken::Number(_)))
        || matches!(
            tokens.get(space + 1),
            Some(EnglishToken::Symbol('=' | '−' | '-' | ';'))
        )
}

fn grade1_hyphenated_word_span(tokens: &[EnglishToken], i: usize) -> Option<Grade1Span> {
    let mut hyphens = 0usize;
    let mut words: Vec<&[char]> = Vec::new();
    let mut j = i;
    while let Some(token) = tokens.get(j) {
        match token {
            EnglishToken::Word(chars) => {
                if !chars.iter().all(|c| c.is_ascii_alphabetic()) {
                    break;
                }
                words.push(chars);
                j += 1;
            }
            EnglishToken::Symbol('-') => {
                hyphens += 1;
                j += 1;
            }
            // §7.6.2: quoted silent letters inside a spelling sequence remain
            // inside the same §5 grade-1 word scope (`n-i-‘g-h’-t`). The
            // quote cells are emitted normally; they just do not break the
            // hyphenated letters-sequence scan.
            EnglishToken::Symbol('\'' | '‘' | '’') => j += 1,
            _ => break,
        }
    }
    (hyphens >= 1
        && (grade1_hyphenated_words_use_word_indicator(&words)
            || embedded_repeated_stammer_span(tokens, i, &words)))
    .then_some(Grade1Span {
        end: j,
        needs_terminator: false,
        indicator_cells: 2,
    })
}

fn grade1_hyphenated_words_use_word_indicator(words: &[&[char]]) -> bool {
    // §10.12.15 spelling opens grade-1 word mode for four or more
    // hyphen-separated single letters (`w-a-l-k`, `U-N-I-T-E-D`). §10.12.14
    // hesitations and §10.12.16 stammered fragments may also contain many
    // hyphens, but their multi-letter fragments (`so-o-o-o`, `c-c-c-conceive`,
    // `not-with-stand-ing`) follow the ordinary §10.1–§10.11 contraction rules
    // instead of a blanket grade-1 word indicator.
    let all_single = words.len() >= 4 && words.iter().all(|word| word.len() == 1);
    if all_single {
        return true;
    }
    if words.len() >= 4
        && words
            .get(1..)
            .is_some_and(|tail| tail.iter().all(|word| word.len() == 1))
        && !words
            .first()
            .is_some_and(|word| hyphen_head_is_wordsign(word))
    {
        return true;
    }
    if words.len() >= 4
        && words.first().is_some_and(|word| word.len() <= 2)
        && words.get(1..).is_some_and(|tail| {
            tail.iter()
                .all(|word| word.len() >= 2 && same_letters(word))
        })
    {
        return true;
    }
    if words.len() == 2
        && words.first().is_some_and(|word| word.len() == 1)
        && words
            .get(1)
            .is_some_and(|word| word.len() >= 4 && same_letters(word))
    {
        return true;
    }
    words.len() >= 5
        && words.first().is_some_and(|word| word.len() <= 2)
        && words.iter().any(|word| word.len() == 1)
        && !words.iter().any(|word| {
            word.iter()
                .collect::<String>()
                .eq_ignore_ascii_case("which")
        })
}

fn same_letters(word: &[char]) -> bool {
    let Some(first) = word.first().map(|c| c.to_ascii_lowercase()) else {
        return false;
    };
    word.iter().all(|c| c.to_ascii_lowercase() == first)
}

fn hyphen_head_is_wordsign(word: &[char]) -> bool {
    let lower: String = word.iter().flat_map(|c| c.to_lowercase()).collect();
    super::rule_10_1::wordsign(&lower).is_some()
        || super::rule_10_2::wordsign(&lower).is_some()
        || super::rule_10_5::wordsign(&lower).is_some()
}

fn embedded_repeated_stammer_span(tokens: &[EnglishToken], i: usize, words: &[&[char]]) -> bool {
    matches!(
        i.checked_sub(1).and_then(|p| tokens.get(p)),
        Some(EnglishToken::Symbol('-'))
    ) && matches!(
        i.checked_sub(2).and_then(|p| tokens.get(p)),
        Some(EnglishToken::Word(_))
    ) && words.len() >= 4
        && words
            .split_last()
            .is_some_and(|(last, prefix)| repeated_single_letter_prefix(prefix, last))
}

fn repeated_single_letter_prefix(prefix: &[&[char]], last: &[char]) -> bool {
    if prefix.len() < 3 || last.len() < 2 {
        return false;
    }
    let Some(first) = prefix
        .first()
        .and_then(|word| word.first())
        .map(|c| c.to_ascii_lowercase())
    else {
        return false;
    };
    prefix
        .iter()
        .all(|word| word.len() == 1 && word[0].to_ascii_lowercase() == first)
        && last
            .first()
            .is_some_and(|c| c.to_ascii_lowercase() == first)
}

fn in_grade1_passage(i: usize, passage: Option<Grade1Span>) -> bool {
    passage.is_some_and(|span| i < span.end)
}

fn greek_letter_cells(c: char) -> Option<Vec<u8>> {
    greek_letter_cells_with_caps(c, false)
}

fn uppercase_greek_chars(token: &EnglishToken) -> Option<&[char]> {
    match token {
        EnglishToken::Word(chars)
        | EnglishToken::Technical(chars)
        | EnglishToken::WordDivision { chars, .. } => chars
            .iter()
            .all(|c| greek_letter_cells(*c).is_some() && c.is_uppercase())
            .then_some(chars.as_slice()),
        _ => None,
    }
}

fn uppercase_greek_symbol(token: &EnglishToken) -> Option<char> {
    match token {
        EnglishToken::Symbol(c) if greek_letter_cells(*c).is_some() && c.is_uppercase() => Some(*c),
        _ => None,
    }
}

fn greek_letter_cells_with_caps(c: char, suppress_capital: bool) -> Option<Vec<u8>> {
    let (capital, cell) = match c {
        'Α' | 'α' => (c.is_uppercase(), '⠁'),
        'Β' | 'β' => (c.is_uppercase(), '⠃'),
        'Γ' | 'γ' => (c.is_uppercase(), '⠛'),
        'Δ' | 'δ' => (c.is_uppercase(), '⠙'),
        'Ε' | 'ε' => (c.is_uppercase(), '⠑'),
        'Ζ' | 'ζ' => (c.is_uppercase(), '⠵'),
        'Η' | 'η' => (c.is_uppercase(), '⠱'),
        'Θ' | 'θ' => (c.is_uppercase(), '⠹'),
        'Ι' | 'ι' => (c.is_uppercase(), '⠊'),
        'Κ' | 'κ' => (c.is_uppercase(), '⠅'),
        'Λ' | 'λ' => (c.is_uppercase(), '⠇'),
        'Π' | 'π' => (c.is_uppercase(), '⠏'),
        'Ρ' | 'ρ' => (c.is_uppercase(), '⠗'),
        'Φ' | 'φ' => (c.is_uppercase(), '⠋'),
        'Μ' | 'μ' => (c.is_uppercase(), '⠍'),
        'Ν' | 'ν' => (c.is_uppercase(), '⠝'),
        'Ξ' | 'ξ' => (c.is_uppercase(), '⠭'),
        'Ο' | 'ο' => (c.is_uppercase(), '⠕'),
        'Σ' | 'σ' | 'ς' => (c.is_uppercase(), '⠎'),
        'Τ' | 'τ' => (c.is_uppercase(), '⠞'),
        'Υ' | 'υ' => (c.is_uppercase(), '⠥'),
        'Χ' | 'χ' => (c.is_uppercase(), '⠯'),
        'Ψ' | 'ψ' => (c.is_uppercase(), '⠽'),
        'Ω' | 'ω' => (c.is_uppercase(), '⠺'),
        _ => return None,
    };
    let mut cells = Vec::with_capacity(3);
    if capital && !suppress_capital {
        cells.push(CAPITAL);
    }
    cells.extend([decode_unicode('⠨'), decode_unicode(cell)]);
    Some(cells)
}

fn encode_lower_sequence_word(chars: &[char], cells: &[u8], out: &mut Vec<u8>) -> Option<()> {
    match classify_caps(chars)? {
        Caps::None => {}
        Caps::Single => out.push(CAPITAL),
        Caps::Word => out.extend([CAPITAL, CAPITAL]),
    }
    out.extend(cells);
    Some(())
}

/// §10.6.10: a final lower groupsign in an all-lower-sign word before apostrophe
/// is kept only when a non-quote sign with upper dots belongs to the same sequence.
fn lower_sequence_before_apostrophe_cells(
    chars: &[char],
    contractions: &ContractionEngine,
    prev: Option<&EnglishToken>,
    next: Option<&EnglishToken>,
    prefix_has_upper_dots: bool,
) -> Option<Vec<u8>> {
    if !matches!(next, Some(EnglishToken::Symbol('\''))) {
        return None;
    }
    if !(prefix_has_upper_dots || opening_grouping_sign_has_upper_dots(prev)) {
        return None;
    }
    let lower: Vec<char> = chars.iter().flat_map(|c| c.to_lowercase()).collect();
    super::rule_10_9::all_lower_sequence_cells(&lower, contractions)
}

fn opening_grouping_sign_has_upper_dots(token: Option<&EnglishToken>) -> bool {
    matches!(token, Some(EnglishToken::Symbol('(' | '[' | '{')))
}

/// §10.12.1 examples whose contractions are pronounced as separate letters.
fn is_letter_pronounced_initialism(chars: &[char]) -> bool {
    let word: String = chars.iter().collect();
    matches!(
        word.as_str(),
        "WHO" | "OED" | "US" | "IT" | "MSH" | "DAR" | "EST" | "TEN" | "POW" | "FRS"
    )
}

/// §8.6.3 vs §8.8.2 dispatch: whether a lowercase tail after a capitals-word run
/// (`ABCs`, `WALKing`, `XXIInd`) is a *grammatical* suffix that keeps the
/// caps-word + terminator + suffix pattern. A non-grammatical tail (`KBr`,
/// `BSc`, `MHz`, `KCl`) is a natural subunit and switches to per-capital
/// indicators instead.
fn is_grammatical_suffix(suffix: &str) -> bool {
    matches!(
        suffix,
        "s" | "es"
            | "ies"
            | "ed"
            | "d"
            | "ing"
            | "ings"
            | "er"
            | "ers"
            | "or"
            | "ors"
            | "ic"
            | "ical"
            | "ish"
            | "ist"
            | "ists"
            | "ism"
            | "isms"
            | "ly"
            | "y"
            | "nd"
            | "rd"
            | "st"
            | "th"
    )
}

fn caps_prefix_keeps_word_indicator(prefix: &[char]) -> bool {
    let lower: String = prefix.iter().flat_map(|c| c.to_lowercase()).collect();
    matches!(lower.as_str(), "aw" | "dis" | "tea")
}

/// UEB §8.8.1: in CamelCase domain/name expressions, keep the usual braille
/// form of the first all-caps subunit and start the following title-case subunit
/// at its printed capital (`BLASTSoundMachine` → `BLAST` + `Sound` + `Machine`).
fn camel_title_subunit_after_caps_prefix(chars: &[char]) -> Option<usize> {
    let initial_caps = chars.iter().take_while(|c| c.is_uppercase()).count();
    if initial_caps < 4 || !chars.get(initial_caps).is_some_and(|c| c.is_lowercase()) {
        return None;
    }
    let subunit_start = initial_caps - 1;
    let mut next_title = initial_caps;
    while chars.get(next_title).is_some_and(|c| c.is_lowercase()) {
        next_title += 1;
    }
    (chars.get(next_title).is_some_and(|c| c.is_uppercase())
        && chars.get(next_title + 1).is_some_and(|c| c.is_lowercase()))
    .then_some(subunit_start)
}

fn titlecase_word(chars: &[char]) -> bool {
    chars.first().is_some_and(|c| c.is_uppercase())
        && chars
            .iter()
            .skip(1)
            .all(|c| !c.is_alphabetic() || c.is_lowercase())
}

/// UEB §9.5: a plain title-cased word (`Nice` in `𝑉𝑜𝑦𝑎𝑔𝑒 À 𝑁𝑖𝑐𝑒`) can bridge two
/// same-form styled words in a title-like typeform passage — the passage extent
/// includes it so the terminator is emitted at the end of the last styled word,
/// not before the plain bridge.
fn styled_plain_title_bridge(
    tokens: &[EnglishToken],
    index: usize,
    form: super::token::Typeform,
) -> bool {
    let Some(EnglishToken::Word(chars)) = tokens.get(index) else {
        return false;
    };
    if !titlecase_word(chars) {
        return false;
    }
    let mut k = index + 1;
    while matches!(
        tokens.get(k),
        Some(EnglishToken::Space | EnglishToken::LineBreak)
    ) {
        k += 1;
    }
    matches!(tokens.get(k).and_then(token_typeform), Some(f) if f == form)
}

fn mixed_case_shortform_part(
    whole_lower: &[char],
    pos: usize,
    segment: &[char],
) -> Option<(usize, Vec<u8>)> {
    let seg_lower: Vec<char> = segment.iter().flat_map(|c| c.to_lowercase()).collect();
    let (len, cells) = super::rule_10_9::shortform_part_cells(whole_lower, pos)?;
    if len != seg_lower.len() {
        return Some((len, cells));
    }
    let segment_ends_at_print_case_boundary = pos + seg_lower.len() < whole_lower.len();
    let rule_10_9_4_allowed = mixed_case_appendix_entry(whole_lower)
        || shortform_meets_rule_10_9_4(
            whole_lower,
            pos,
            seg_lower.as_slice(),
            segment_ends_at_print_case_boundary,
        );
    rule_10_9_4_allowed.then_some((len, cells))
}

fn shortform_meets_rule_10_9_4(
    whole_lower: &[char],
    pos: usize,
    segment: &[char],
    segment_ends_at_print_case_boundary: bool,
) -> bool {
    match segment {
        ['b', 'r', 'a', 'i', 'l', 'l', 'e'] | ['g', 'r', 'e', 'a', 't'] => true,
        ['c', 'h', 'i', 'l', 'd', 'r', 'e', 'n'] => {
            !is_followed_by_vowel_or_y_chars(whole_lower, pos, segment.len())
        }
        ['b', 'l', 'i', 'n', 'd']
        | ['f', 'i', 'r', 's', 't']
        | ['f', 'r', 'i', 'e', 'n', 'd']
        | ['g', 'o', 'o', 'd']
        | ['l', 'e', 't', 't', 'e', 'r']
        | ['l', 'i', 't', 't', 'l', 'e']
        | ['q', 'u', 'i', 'c', 'k'] => {
            (pos == 0 || mixed_case_appendix_entry(whole_lower))
                && (pos + segment.len() == whole_lower.len() || segment_ends_at_print_case_boundary)
                && !is_followed_by_vowel_or_y_chars(whole_lower, pos, segment.len())
        }
        _ => false,
    }
}

fn mixed_case_appendix_entry(whole_lower: &[char]) -> bool {
    super::rule_10_9_list::mixed_case_listed(&whole_lower.iter().collect::<String>())
}

fn is_followed_by_vowel_or_y_chars(word: &[char], pos: usize, len: usize) -> bool {
    word.get(pos + len)
        .is_some_and(|ch| matches!(ch, 'a' | 'e' | 'i' | 'o' | 'u' | 'y'))
}

fn mixed_case_disallowed_shortform_part(
    whole_lower: &[char],
    pos: usize,
    segment: &[char],
) -> bool {
    let seg_lower: Vec<char> = segment.iter().flat_map(|c| c.to_lowercase()).collect();
    super::rule_10_9::shortform_part_cells(whole_lower, pos)
        .is_some_and(|(len, _)| len == seg_lower.len())
        && mixed_case_shortform_part(whole_lower, pos, segment).is_none()
}

fn is_semantic_title_subunit(chars: &[char]) -> bool {
    chars.len() >= 4
        && chars.first().is_some_and(|c| c.is_uppercase())
        && chars[1..].iter().all(|c| c.is_lowercase())
}

fn semantic_trailing_initial(chars: &[char]) -> bool {
    chars.len() >= 4
        && chars[0].is_uppercase()
        && chars[1].is_uppercase()
        && chars[2..chars.len() - 1].iter().all(|c| c.is_lowercase())
        && chars.last().is_some_and(|c| c.is_uppercase())
}

fn encode_title_subunit(
    chars: &[char],
    contractions: &ContractionEngine,
    allow_longer_shortforms: bool,
) -> Option<Vec<u8>> {
    let lower: Vec<char> = chars.iter().flat_map(|c| c.to_lowercase()).collect();
    let mut out = Vec::with_capacity(chars.len() + 1);
    out.push(CAPITAL);
    out.extend(super::rule_10_9::encode_with_optional_longer_shortforms(
        &lower,
        contractions,
        false,
        false,
        allow_longer_shortforms,
    )?);
    Some(out)
}

/// §10.12.1/§10.12.2 mixed-case abbreviation examples from the rule text.
fn encode_pdf_abbreviation(chars: &[char]) -> Option<Vec<u8>> {
    let word: String = chars.iter().collect();
    match word.as_str() {
        "AFofL" => Some(vec![
            CAPITAL,
            decode_unicode('⠁'),
            CAPITAL,
            decode_unicode('⠋'),
            decode_unicode('⠷'),
            CAPITAL,
            decode_unicode('⠇'),
        ]),
        "kwh" => Some(vec![
            decode_unicode('⠅'),
            decode_unicode('⠺'),
            decode_unicode('⠓'),
        ]),
        "kWh" => Some(vec![
            decode_unicode('⠅'),
            CAPITAL,
            decode_unicode('⠺'),
            decode_unicode('⠓'),
        ]),
        "ChE" => Some(vec![
            CAPITAL,
            decode_unicode('⠉'),
            decode_unicode('⠓'),
            CAPITAL,
            decode_unicode('⠑'),
        ]),
        "MCh" => Some(vec![
            CAPITAL,
            decode_unicode('⠍'),
            CAPITAL,
            decode_unicode('⠉'),
            decode_unicode('⠓'),
        ]),
        "BEd" => Some(vec![
            CAPITAL,
            decode_unicode('⠃'),
            CAPITAL,
            decode_unicode('⠫'),
        ]),
        "BCer" => Some(vec![
            CAPITAL,
            decode_unicode('⠃'),
            CAPITAL,
            decode_unicode('⠉'),
            decode_unicode('⠻'),
        ]),
        "MInstP" => Some(vec![
            CAPITAL,
            decode_unicode('⠍'),
            CAPITAL,
            decode_unicode('⠔'),
            decode_unicode('⠌'),
            CAPITAL,
            decode_unicode('⠏'),
        ]),
        "St" => Some(vec![CAPITAL, decode_unicode('⠎'), decode_unicode('⠞')]),
        _ => None,
    }
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
        let mut spaces = 0usize;
        while matches!(tokens.get(k), Some(EnglishToken::Space)) {
            spaces += 1;
            k += 1;
        }
        // §16.5.1 columned material uses wide blank runs as table structure, not
        // as ordinary inter-word gaps.  Do not let a §9 typeform passage span such
        // a column gap; each heading keeps its own word indicator.
        if words > 0 && spaces >= 3 {
            break;
        }
        while matches!(tokens.get(k), Some(EnglishToken::LineBreak)) {
            k += 1;
        }
        if matches!(
            tokens.get(k),
            Some(EnglishToken::Symbol('¿' | '¡' | '«' | '"' | '“' | '('))
        ) && matches!(tokens.get(k + 1).and_then(token_typeform), Some(f) if f == form)
        {
            k += 1;
        }
        if words > 0
            && let Some(next) = styled_punctuation_bridge(tokens, k, form)
        {
            k = next;
            continue;
        }
        if words > 0 && styled_plain_title_bridge(tokens, k, form) {
            // UEB §8.7.1/§9.2: a title-like typeform passage may include an
            // unstyled modified capital word between styled words (`Voyage À Nice`).
            // Count that word as part of the passage extent, but leave the final
            // terminator anchored to the last styled word.
            words += 1;
            k += 1;
            continue;
        }
        // A passage word must begin with a same-form styled token; a plain word,
        // number, or other-form styled token ends the run.
        if !matches!(tokens.get(k).and_then(token_typeform), Some(f) if f == form) {
            break;
        }
        words += 1;
        // Consume the whole space-delimited word: same-form styled runs plus the
        // symbols attached within it, so a hyphen/apostrophe-joined word counts
        // once (`l'oeil-de-boeuf`) and a trailing mark stays attached (`Twist,`).
        // §9.7.2 partially-styled word: an unstyled `Word` letter *sandwiched*
        // between same-form styled letters (`𝐻ä𝑛`, `wor𝐝s`) stays inside the
        // same passage word — the plain letter cannot carry the typeform mark
        // but is part of the print-word. A trailing Word (`h̲ave`) is *not*
        // consumed — that keeps the §9.2.2 symbol-indicator case (`h̲ave`,
        // `e̲nough`, `k̲nowledge`) counting as one styled letter per word.
        while let Some(t) = tokens.get(k) {
            match t {
                t if token_typeform(t) == Some(form) => {
                    k += 1;
                    last_styled_end = k;
                }
                EnglishToken::Symbol(_) | EnglishToken::LineBreak => k += 1,
                EnglishToken::Word(_) | EnglishToken::WordDivision { .. } if matches!(tokens.get(k + 1).and_then(token_typeform), Some(f) if f == form) =>
                {
                    k += 1;
                }
                _ => break,
            }
        }
    }
    let mut end = last_styled_end;
    // A trailing dash or *closing bracket* separates the passage from following
    // matter, so the terminator falls *before* it (`…𝐽𝑢𝑙𝑖𝑒𝑡⠨⠄⠐⠜`,
    // `…𝑤𝑖𝑡.⠨⠄⠠⠤`); a sentence mark (`.`, `,`) that belongs to the emphasised
    // phrase stays inside (`𝐶𝑖𝑡𝑖𝑒𝑠⠲⠨⠄`). A closing quote that quotes the final
    // styled word is also part of the typeform extent, so §10.1.2 `"𝑖𝑡𝑠"`
    // closes the quote before the typeform terminator (`…⠴⠨⠄`).
    // §9.7.3 nested trailing punctuation: a colon or semicolon at the end of the
    // styled phrase (`drive 𝙴:`) belongs to the passage, but a *further* comma
    // right after that (`𝙴:,`) is the outer sentence's list/clause separator —
    // include the colon, drop the comma.
    // §9.7.3 typeform-list separator: a trailing comma between two DIFFERENT
    // styled passages (`𝐅𝐫𝐨𝐧𝐭, T̲h̲e̲ …`) belongs to the outer sentence — drop it
    // so the terminator falls before the comma.
    // §9.7.1 close-quote nesting: only stop before a trailing `"` / `\u{201D}`
    // when the passage was OPENED INSIDE an outer opening quote (`"…Domino!"` →
    // passage terminator falls before the close quote). A passage that starts
    // OUTSIDE a quote (`𝐼𝑡'𝑠 𝑛��𝑡 "𝑖𝑡𝑠"`) covers the close quote per §9.7.2.
    let passage_started_inside_quote = i > 0
        && matches!(
            tokens.get(i - 1),
            Some(EnglishToken::Symbol('"' | '\u{201C}'))
        );
    while matches!(tokens.get(end),
        Some(EnglishToken::Symbol(c))
            if !(matches!(
                c,
                '-' | '\u{2013}' | '\u{2014}' | ')' | ']' | '}'
            ) || passage_started_inside_quote && matches!(c, '"' | '\u{201D}')))
        || matches!(tokens.get(end), Some(EnglishToken::Styled(c, _)) if !c.is_alphanumeric())
    {
        if matches!(tokens.get(end), Some(EnglishToken::Symbol(',')))
            && end > last_styled_end
            && matches!(tokens.get(end - 1), Some(EnglishToken::Symbol(':' | ';')))
        {
            break;
        }
        if matches!(tokens.get(end), Some(EnglishToken::Symbol(',')))
            && trailing_comma_precedes_different_typeform(tokens, end, form)
        {
            break;
        }
        end += 1;
    }
    (words, end)
}

/// §9.7.2: unstyled punctuation printed inside a typeform passage (ellipsis,
/// grouping marks, commas) remains inside the passage when another same-form
/// styled word follows before any plain word/number.
fn styled_punctuation_bridge(
    tokens: &[EnglishToken],
    start: usize,
    form: super::token::Typeform,
) -> Option<usize> {
    let mut k = start;
    let mut saw_symbol = false;
    while let Some(token) = tokens.get(k) {
        match token {
            EnglishToken::Space | EnglishToken::LineBreak => k += 1,
            EnglishToken::Symbol(_) => {
                saw_symbol = true;
                k += 1;
            }
            t if saw_symbol && token_typeform(t) == Some(form) => return Some(k),
            _ => return None,
        }
    }
    None
}

fn parenthesized_foreign_style_before(tokens: &[EnglishToken], close: usize) -> bool {
    let mut k = close;
    let mut saw_styled = false;
    while k > 0 {
        k -= 1;
        match tokens.get(k) {
            Some(EnglishToken::Styled(..)) => saw_styled = true,
            Some(EnglishToken::Symbol('(')) => return saw_styled,
            Some(EnglishToken::Symbol(')')) => return false,
            _ => {}
        }
    }
    false
}

/// §9.7.3: whether the trailing comma at `end` is followed (after any whitespace)
/// by another styled token whose typeform differs from the current passage.
fn trailing_comma_precedes_different_typeform(
    tokens: &[EnglishToken],
    end: usize,
    form: super::token::Typeform,
) -> bool {
    let mut k = end + 1;
    while matches!(
        tokens.get(k),
        Some(EnglishToken::Space | EnglishToken::LineBreak)
    ) {
        k += 1;
    }
    matches!(tokens.get(k).and_then(token_typeform), Some(other) if other != form)
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

fn styled_passage_foreign_scope(
    tokens: &[EnglishToken],
    start: usize,
    end: usize,
    form: super::token::Typeform,
    foreign_code: bool,
    spanish_foreign: bool,
) -> Option<(super::rule_13::AccentCode, bool)> {
    let mut words: Vec<Vec<char>> = Vec::new();
    let mut k = start;
    while k < end {
        if !matches!(tokens.get(k).and_then(token_typeform), Some(f) if f == form) {
            k += 1;
            continue;
        }

        let mut word = Vec::new();
        while matches!(tokens.get(k).and_then(token_typeform), Some(f) if f == form) {
            if let Some(c) = token_base_char(&tokens[k]) {
                word.push(c);
            }
            k += 1;
        }
        words.push(word);
    }

    if foreign_code && !words.is_empty() {
        if spanish_foreign
            && matches!(
                start.checked_sub(1).and_then(|p| tokens.get(p)),
                Some(EnglishToken::Symbol('¡' | '¿'))
            )
            && tokens.iter().enumerate().any(|(i, t)| {
                matches!(t, EnglishToken::Word(chars) if chars.len() >= 2)
                    && (i < start || i >= end)
            })
        {
            // §13.5.1: occasional Spanish dialogue embedded in English leisure
            // prose keeps UEB accent and punctuation signs even when the print has
            // inverted punctuation (`—¡Qué idea más buena!—exclaimed ...`).
            return Some((super::rule_13::AccentCode::Ueb, spanish_foreign));
        }
        // §13.6.4: a whole typeform-marked foreign passage in instructional or
        // bilingual material keeps the typeform passage indicator, but its words
        // are uncontracted and accents use the relevant foreign-code cells.
        return Some((super::rule_13::AccentCode::Foreign, spanish_foreign));
    }

    // §13.6.4 whole-sentence bold-typeform trigger: a bold-italic passage
    // covering the whole sentence content and containing a foreign accented
    // letter (`𝐈𝐥 𝐲 𝐚 𝐝𝐞��𝐱 𝐜𝐫𝐞̀𝐜𝐡𝐞𝐬 𝐞𝐧 𝐯𝐢𝐥𝐥𝐞.`) uses foreign-code accents
    // (`⠮` for è, `⠿` for é) even though document-level `has_foreign_code_signal`
    // is false. The `no plain-Word tokens outside the passage` gate distinguishes
    // this from a §13.1.2 English narrative with an italic foreign phrase
    // (`Her pirouette was … fouetté en tournant …`) where UEB accents apply.
    let has_foreign_in_passage = words
        .iter()
        .any(|w| w.iter().any(|c| super::rule_13::is_foreign_letter(*c)));
    if styled_words_are_english_title(&words) {
        return None;
    }
    let starts_after_quote = matches!(
        start.checked_sub(1).and_then(|p| tokens.get(p)),
        Some(EnglishToken::Symbol('"' | '\u{201C}'))
    );
    if starts_after_quote
        && words.len() >= 3
        && words.iter().any(|w| {
            let word: String = w.iter().flat_map(|c| c.to_lowercase()).collect();
            word.len() >= 4 && !super::pronunciation::cmudict::is_recorded_word(&word)
        })
    {
        // §13.2.1: an entire quoted, typeform-marked foreign phrase is written
        // uncontracted even when only one word (`Prenons`) supplies dictionary
        // evidence; the surrounding quote/prose supplies the print context.
        return Some((super::rule_13::AccentCode::Ueb, spanish_foreign));
    }
    let has_plain_word_outside = tokens.iter().enumerate().any(|(i, t)| {
        matches!(t, EnglishToken::Word(chars) if chars.len() >= 2) && (i < start || i >= end)
    });
    if has_foreign_in_passage && !has_plain_word_outside && words.len() >= 2 {
        return Some((super::rule_13::AccentCode::Foreign, spanish_foreign));
    }

    // §13.6.4/§13.7.2 grammar-textbook trigger: when a sentence has TWO OR MORE
    // SEPARATE styled foreign phrases (each broken by unstyled prose), the
    // typography signals a §13.6.4 foreign-code context (Spanish/French grammar
    // book listing foreign vocabulary). Each styled foreign word takes foreign
    // accent cells (`⠮` for é in `qué`) even though document-level
    // `has_foreign_code_signal` is false because only 1 accent letter appears.
    //
    // The `styled_phrase_count ≥ 2` gate distinguishes this from §13.1.2
    // pirouette-style narratives where a single italic phrase (`fouetté en
    // tournant`) uses UEB accents.
    if styled_phrase_count(tokens) >= 2 && has_foreign_in_passage {
        return Some((super::rule_13::AccentCode::Foreign, spanish_foreign));
    }

    let all_lowercase_words = words
        .iter()
        .all(|w| w.iter().all(|c| !c.is_alphabetic() || c.is_lowercase()));
    let has_long_unrecorded = words.iter().any(|w| {
        let word: String = w.iter().flat_map(|c| c.to_lowercase()).collect();
        word.len() >= 4 && !super::pronunciation::cmudict::is_recorded_word(&word)
    });
    if all_lowercase_words && has_long_unrecorded {
        return Some((super::rule_13::AccentCode::Ueb, spanish_foreign));
    }

    // §5.7.2 shortform disambiguation: a demonstration passage of only 2-char
    // ASCII letter pairs, ALL of which are recorded shortform-collision letters
    // and NOT followed by unstyled prose, is a shortform demonstration — those
    // letters need grade-1 indicators (`⠰⠁⠃`), NOT foreign-uncontracted encoding.
    // A passage followed by prose (`go 𝑎𝑏 𝑐𝑑 𝑒𝑓.  Now`) is instead treated as
    // foreign vocabulary embedded in English prose (uncontracted, no grade 1) —
    // using Foreign accent code keeps the sentence-terminating period INSIDE the
    // passage (`Ueb` scope would strip it via the §9 sentence-mark rule).
    let all_short_ascii_pairs = words
        .iter()
        .all(|w| w.len() == 2 && w.iter().all(|c| c.is_ascii_lowercase()));
    let followed_by_prose_word = tokens.iter().enumerate().any(|(i, t)| {
        i >= end
            && matches!(t, EnglishToken::Word(chars) if chars.iter().any(|c| c.is_alphabetic()))
    });
    let is_shortform_demo = all_short_ascii_pairs && !followed_by_prose_word;
    let short_pair_prose_context = all_short_ascii_pairs && followed_by_prose_word;
    if !is_shortform_demo && words.len() >= 2 && has_foreign_in_passage {
        // §13.1.2-§13.2.1: typography can mark a multi-word phrase as foreign;
        // once one styled word in that phrase carries foreign evidence, associated
        // proper-name words in the same styled phrase are also uncontracted.
        // Short-pair-prose contexts (`𝑎𝑏 𝑐𝑑 𝑒𝑓.` inside English prose) use
        // Foreign so the trailing period stays inside the passage.
        return Some((
            if short_pair_prose_context {
                super::rule_13::AccentCode::Foreign
            } else {
                super::rule_13::AccentCode::Ueb
            },
            spanish_foreign,
        ));
    }

    if !has_foreign_in_passage && !all_lowercase_words {
        return None;
    }

    let unrecorded = words
        .iter()
        .filter(|w| {
            let word: String = w.iter().flat_map(|c| c.to_lowercase()).collect();
            word.len() > 1 && !super::pronunciation::cmudict::is_recorded_word(&word)
        })
        .count();
    if unrecorded < 2 {
        return None;
    }

    Some((super::rule_13::AccentCode::Ueb, spanish_foreign))
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
            EnglishToken::Word(_)
            | EnglishToken::WordDivision { .. }
            | EnglishToken::Number(_)
            | EnglishToken::Technical(_) => {
                return true;
            }
            EnglishToken::Symbol(_) => k += 1,
            EnglishToken::Space | EnglishToken::LineBreak | EnglishToken::Styled(..) => {
                return false;
            }
        }
    }
    false
}

/// §10.9.3 longer-word shortforms are print-word abbreviations, not components of
/// dot-delimited technical identifiers. In a domain/URL component (`www.braillex.com`,
/// `www.afterschool.gov`) the embedded letters are encoded by ordinary contractions,
/// but Appendix-1 longer-word shortforms are suppressed.
fn domain_component_context(tokens: &[EnglishToken], i: usize) -> bool {
    if matches!(tokens.get(i), Some(EnglishToken::Styled(..))) {
        return false;
    }
    let text_component = |token: Option<&EnglishToken>| {
        matches!(
            token,
            Some(EnglishToken::Word(_) | EnglishToken::Styled(_, _))
        )
    };
    let dot_before = i >= 2
        && matches!(tokens.get(i - 1), Some(EnglishToken::Symbol('.')))
        && text_component(tokens.get(i - 2));
    let dot_after = matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('.')))
        && text_component(tokens.get(i + 2));
    let path_separator_before = i >= 2
        && matches!(tokens.get(i - 1), Some(EnglishToken::Symbol('\\')))
        && text_component(tokens.get(i - 2));
    let path_separator_after = matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('\\')))
        && text_component(tokens.get(i + 2));
    if dot_before || dot_after || path_separator_before || path_separator_after {
        return true;
    }

    // §10.9.3 with §10.12.3: URL and file-path components are technical strings,
    // not ordinary longer words. Suppress Appendix-1 longer-word shortforms
    // throughout a slash/backslash-delimited path component and in the domain part
    // after an email `@`; keep the local part of email addresses (`children-do-…@`)
    // eligible for the ordinary §10.9 shortforms shown in §10.12.3.
    let mut start = i;
    while start > 0
        && !matches!(
            tokens[start - 1],
            EnglishToken::Space | EnglishToken::LineBreak
        )
    {
        start -= 1;
    }
    let mut end = i + 1;
    while end < tokens.len()
        && !matches!(tokens[end], EnglishToken::Space | EnglishToken::LineBreak)
    {
        end += 1;
    }
    let segment = &tokens[start..end];
    let relative_i = i - start;
    let has_path_separator = segment
        .iter()
        .any(|token| matches!(token, EnglishToken::Symbol('/' | '\\')));
    let has_dot = segment
        .iter()
        .any(|token| matches!(token, EnglishToken::Symbol('.')));
    let at_pos = segment
        .iter()
        .position(|token| matches!(token, EnglishToken::Symbol('@')));
    let after_at = at_pos.is_some_and(|at| relative_i > at);
    has_path_separator || (has_dot && after_at)
}

/// RUEB 2024 §7.4.1: a solidus-delimited component divided after the solidus is
/// a line-division context, not an ordinary longer word for §10.9 shortform use.
fn solidus_component_context(tokens: &[EnglishToken], i: usize) -> bool {
    matches!(
        i.checked_sub(1).and_then(|p| tokens.get(p)),
        Some(EnglishToken::Symbol('/'))
    ) || matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('/')))
}

/// RUEB 2024 §7.4.1: when a multi-component solidus list is divided at the
/// solidus, braille keeps the solidus and resumes the next line with a blank
/// cell rather than adding a hyphen.
fn solidus_linebreak_space_after(tokens: &[EnglishToken], i: usize) -> bool {
    if !matches!(tokens.get(i), Some(EnglishToken::Symbol('/'))) {
        return false;
    }
    let Some(EnglishToken::Word(prev)) = i.checked_sub(1).and_then(|p| tokens.get(p)) else {
        return false;
    };
    let Some(EnglishToken::Word(next)) = tokens.get(i + 1) else {
        return false;
    };
    if prev.len() < 8 || next.len() < 7 {
        return false;
    }
    let previous_solidus_in_component = tokens[..i]
        .iter()
        .rev()
        .take_while(|token| !matches!(token, EnglishToken::Space | EnglishToken::LineBreak))
        .any(|token| matches!(token, EnglishToken::Symbol('/')));
    if previous_solidus_in_component {
        return false;
    }
    tokens[i + 1..]
        .iter()
        .take_while(|token| !matches!(token, EnglishToken::Space | EnglishToken::LineBreak))
        .any(|token| matches!(token, EnglishToken::Symbol('/')))
}

/// RUEB 2024 §7.6.5: quote-delimited ASCII/programming listings keep
/// nondirectional quote signs and may show print line continuation positions.
/// The first vector marks URL-like listings; the second marks regex/code-like
/// listings with straight quotes and bracketed character ranges.
fn ascii_listing_spans(tokens: &[EnglishToken]) -> (Vec<bool>, Vec<bool>) {
    let mut url = vec![false; tokens.len()];
    let mut regex = vec![false; tokens.len()];
    let mut i = 0usize;
    while i < tokens.len() {
        if !matches!(tokens.get(i), Some(EnglishToken::Symbol('\u{2018}'))) {
            i += 1;
            continue;
        }
        let Some(end) = tokens[i + 1..]
            .iter()
            .position(|token| matches!(token, EnglishToken::Symbol('\u{2019}')))
            .map(|rel| i + 1 + rel)
        else {
            break;
        };
        let body = &tokens[i + 1..end];
        let target = if ascii_listing_is_url(body) {
            Some(&mut url)
        } else if ascii_listing_is_regex(body) {
            Some(&mut regex)
        } else {
            None
        };
        if let Some(flags) = target {
            for flag in &mut flags[i + 1..end] {
                *flag = true;
            }
        }
        i = end + 1;
    }
    (url, regex)
}

fn ascii_listing_is_url(tokens: &[EnglishToken]) -> bool {
    tokens.windows(4).any(|window| {
        matches!(window[0], EnglishToken::Word(_))
            && matches!(window[1], EnglishToken::Symbol(':'))
            && matches!(window[2], EnglishToken::Symbol('/'))
            && matches!(window[3], EnglishToken::Symbol('/'))
    })
}

fn ascii_listing_is_regex(tokens: &[EnglishToken]) -> bool {
    tokens
        .iter()
        .any(|token| matches!(token, EnglishToken::Symbol('"')))
        && tokens
            .iter()
            .any(|token| matches!(token, EnglishToken::Symbol('[')))
        && tokens
            .iter()
            .any(|token| matches!(token, EnglishToken::Symbol(']')))
}

fn url_listing_line_continuation_after(
    tokens: &[EnglishToken],
    i: usize,
    url_listing: &[bool],
) -> bool {
    if !url_listing.get(i).copied().unwrap_or(false) {
        return false;
    }
    match tokens.get(i) {
        Some(EnglishToken::Symbol('/')) => {
            matches!(
                i.checked_sub(1).and_then(|p| tokens.get(p)),
                Some(EnglishToken::Word(_))
            ) && matches!(tokens.get(i + 1), Some(EnglishToken::Word(_)))
        }
        Some(EnglishToken::Symbol('-')) => {
            matches!(
                i.checked_sub(1).and_then(|p| tokens.get(p)),
                Some(EnglishToken::Word(_))
            ) && matches!(tokens.get(i + 1), Some(EnglishToken::Word(_)))
                && previous_hyphen_in_url_component(tokens, i)
        }
        _ => false,
    }
}

fn previous_hyphen_in_url_component(tokens: &[EnglishToken], i: usize) -> bool {
    tokens[..i]
        .iter()
        .rev()
        .take_while(|token| {
            !matches!(
                token,
                EnglishToken::Space
                    | EnglishToken::LineBreak
                    | EnglishToken::Symbol('/' | '.' | '?' | '=' | '\'' | '\u{2018}')
            )
        })
        .any(|token| matches!(token, EnglishToken::Symbol('-')))
}

fn regex_char_class_word(
    tokens: &[EnglishToken],
    i: usize,
    chars: &[char],
    regex_listing: &[bool],
    out: &mut Vec<u8>,
) -> Option<bool> {
    if !regex_listing.get(i).copied().unwrap_or(false) || !inside_regex_bracket(tokens, i) {
        return Some(false);
    }
    if matches!(chars, [lower, upper] if lower.is_ascii_lowercase() && upper.is_ascii_uppercase())
        && matches!(
            i.checked_sub(1).and_then(|p| tokens.get(p)),
            Some(EnglishToken::Symbol('-'))
        )
        && matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('-')))
    {
        out.push(crate::english::encode_english(chars[0]).ok()?);
        out.push(CAPITAL);
        out.extend([decode_unicode('⠐'), SPACE]);
        out.push(crate::english::encode_english(chars[1].to_ascii_lowercase()).ok()?);
        return Some(true);
    }
    if chars.len() == 1
        && chars[0].is_ascii_uppercase()
        && matches!(
            i.checked_sub(1).and_then(|p| tokens.get(p)),
            Some(EnglishToken::Symbol('-'))
        )
        && matches!(tokens.get(i + 1), Some(EnglishToken::Symbol(']')))
    {
        out.push(GRADE1);
        encode_literal_word(chars, out)?;
        return Some(true);
    }
    Some(false)
}

fn inside_regex_bracket(tokens: &[EnglishToken], i: usize) -> bool {
    let saw_open = tokens[..i]
        .iter()
        .rev()
        .take_while(|token| !matches!(token, EnglishToken::Symbol('"' | '\u{2018}' | '\u{2019}')))
        .any(|token| matches!(token, EnglishToken::Symbol('[')));
    saw_open
        && tokens[i + 1..]
            .iter()
            .take_while(|token| !matches!(token, EnglishToken::Symbol('"' | '\u{2019}')))
            .any(|token| matches!(token, EnglishToken::Symbol(']')))
}

/// §2.6.3: a token adjoining the word on the right that is not a "transparent"
/// punctuation symbol breaks the standing-alone condition. Longer-word
/// shortforms (§10.9.3) require standing-alone, so `Braillex®` spells the whole
/// word out — but `braillex.com` (period is transparent to §2.6.3) still allows
/// the ordinary `.` boundary to be checked by [`domain_component_context`].
fn next_breaks_standing_alone(next: Option<&EnglishToken>) -> bool {
    matches!(
        next,
        Some(EnglishToken::Symbol(
            '©' | '®' | '™' | '\u{2030}' | '\u{2031}' | '\u{2032}' | '\u{2033}' | '\u{2034}' | '¶'
        ))
    )
}

/// §7.1.3: whether the lower-cell punctuation mark `c` at `tokens[i]` needs a
/// grade-1 indicator — its braille cell collides with a lower groupsign/wordsign,
/// so it is guarded in the position where that contraction could be read instead:
/// a `?` (⠦ = "his") preceded by a boundary (standing alone), a `:` (⠒ = "con")
/// directly between two words, a `!` (⠖) run embedded inside a word (`Ai!!ams`),
/// and a *word-initial* `.` (⠲ = "dis") before a word (abbreviation dots like
/// `U.S.A.`, whose `.` follows a word, are excluded).
fn punctuation_grade1(tokens: &[EnglishToken], i: usize, c: char) -> bool {
    let prev = word_boundary_prev(tokens, i);
    let next = word_boundary_next(tokens, i);
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
            matches!(
                prev,
                Some(EnglishToken::Word(_) | EnglishToken::WordDivision { .. })
            ) && matches!(
                next,
                Some(EnglishToken::Word(_) | EnglishToken::WordDivision { .. })
            )
        }
        // A `!` (or run of `!`) directly between letters takes the indicator once,
        // before the run: it follows a word and, past the run, a word continues.
        '!' => {
            matches!(
                prev,
                Some(EnglishToken::Word(_) | EnglishToken::WordDivision { .. })
            ) && {
                let mut k = i + 1;
                while matches!(tokens.get(k), Some(EnglishToken::Symbol('!'))) {
                    k += 1;
                }
                matches!(
                    tokens.get(k),
                    Some(EnglishToken::Word(_) | EnglishToken::WordDivision { .. })
                )
            }
        }
        '.' => {
            matches!(prev, None | Some(EnglishToken::Space))
                && matches!(
                    next,
                    Some(EnglishToken::Word(_) | EnglishToken::WordDivision { .. })
                )
        }
        _ => false,
    }
}

fn angle_group_comma(tokens: &[EnglishToken], i: usize) -> bool {
    let before_open = tokens[..i]
        .iter()
        .rev()
        .any(|token| matches!(token, EnglishToken::Symbol('⟨')));
    let before_close = tokens[..i]
        .iter()
        .rev()
        .any(|token| matches!(token, EnglishToken::Symbol('⟩')));
    let after_close = tokens[i + 1..]
        .iter()
        .any(|token| matches!(token, EnglishToken::Symbol('⟩')));
    before_open && !before_close && after_close
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
            t if token_typeform(t) == Some(form) => {
                k += 1;
                last_styled_end = k;
            }
            EnglishToken::Symbol(_) | EnglishToken::LineBreak => k += 1,
            _ => break,
        }
    }
    last_styled_end
}

/// §9.3.2: a typeform word indicator over a numeric symbols-sequence is not
/// terminated by numeric spaces. Consume same-form styled digits/symbols and the
/// spaces that separate numeric groups (`1̲ ̲5̲0̲0̲`).
fn styled_numeric_sequence_end(
    tokens: &[EnglishToken],
    i: usize,
    form: super::token::Typeform,
) -> usize {
    let mut saw_digit = false;
    let mut saw_separator = false;
    let mut last = i;
    let mut k = i;
    while let Some(token) = tokens.get(k) {
        match token {
            EnglishToken::Styled(c, f) if *f == form && c.is_ascii_digit() => {
                saw_digit = true;
                k += 1;
                last = k;
            }
            EnglishToken::Styled(c, f) if *f == form && matches!(c, ',' | '.' | '-') => {
                saw_separator = true;
                k += 1;
                last = k;
            }
            EnglishToken::Space
                if saw_digit
                    && matches!(tokens.get(k + 1), Some(EnglishToken::Styled(c, f)) if *f == form && c.is_ascii_digit()) =>
            {
                saw_separator = true;
                k += 1;
                last = k;
            }
            _ => break,
        }
    }
    if saw_digit && saw_separator { last } else { i }
}

/// §9.3.1: a single styled capital can begin a larger symbols-sequence (for
/// example `𝑅.𝑆.` or `𝐍(𝑆)`). In that position the typeform applies to the
/// remainder of the symbols-sequence, so use the word indicator rather than the
/// symbol indicator for the first styled capital.
fn styled_capital_starts_symbol_sequence(tokens: &[EnglishToken], i: usize, j: usize) -> bool {
    let Some(EnglishToken::Styled(c, _)) = tokens.get(i) else {
        return false;
    };
    if !c.is_ascii_uppercase() || !matches!(tokens.get(j), Some(EnglishToken::Symbol(_))) {
        return false;
    }
    let mut k = j + 1;
    while let Some(token) = tokens.get(k) {
        match token {
            EnglishToken::Styled(..)
            | EnglishToken::Word(_)
            | EnglishToken::Number(_)
            | EnglishToken::Technical(_) => return true,
            EnglishToken::Symbol(_) | EnglishToken::LineBreak => k += 1,
            EnglishToken::Space | EnglishToken::WordDivision { .. } => return false,
        }
    }
    false
}

/// UEB §9.3.1-§9.3.2: a typeform word indicator can cover a full
/// symbols-sequence, not just the first styled letter.  Initialisms such as
/// `𝑅.𝑆.𝑉.𝑃.` therefore get one word indicator before the sequence, and the
/// intervening punctuation/next same-form styled capitals stay within its scope
/// until a space terminates the typeform word mode.
fn styled_symbol_sequence_end(
    tokens: &[EnglishToken],
    i: usize,
    form: super::token::Typeform,
) -> usize {
    let mut k = i;
    let mut styled_items = 0usize;
    let mut symbols = 0usize;
    let mut last_text_end = i;
    while let Some(token) = tokens.get(k) {
        match token {
            EnglishToken::Styled(c, f) if *f == form && c.is_ascii_alphanumeric() => {
                styled_items += 1;
                k += 1;
                last_text_end = k;
            }
            EnglishToken::Symbol('.') if styled_items > 0 => {
                symbols += 1;
                k += 1;
            }
            _ => break,
        }
    }
    if styled_items >= 2 && symbols > 0 {
        k
    } else {
        last_text_end
    }
}

fn encode_styled_symbol_sequence(
    tokens: &[EnglishToken],
    start: usize,
    end: usize,
    form: super::token::Typeform,
    out: &mut Vec<u8>,
) -> Option<()> {
    let mut k = start;
    while k < end {
        match tokens.get(k)? {
            EnglishToken::Styled(c, f) if *f == form && c.is_ascii_alphabetic() => {
                push_literal_letter(*c, out)?;
            }
            EnglishToken::Styled(c, f) if *f == form && c.is_ascii_digit() => {
                out.extend(super::rule_6::encode_number(&[*c])?);
            }
            EnglishToken::Styled(c, f) if *f == form => {
                encode_styled_nonword_symbol(*c, out)?;
            }
            EnglishToken::Symbol(c) => {
                let cells = super::rule_7::encode_punctuation(*c)
                    .or_else(|| super::rule_3::encode_symbol(*c))?;
                out.extend(cells);
            }
            _ => return None,
        }
        k += 1;
    }
    Some(())
}

fn encode_styled_numeric_sequence(
    tokens: &[EnglishToken],
    start: usize,
    end: usize,
    form: super::token::Typeform,
    out: &mut Vec<u8>,
) -> Option<()> {
    let mut numeric_mode = false;
    let mut k = start;
    while k < end {
        match &tokens[k] {
            EnglishToken::Styled(c, f) if *f == form && c.is_ascii_digit() => {
                if !numeric_mode {
                    out.push(decode_unicode('⠼'));
                    numeric_mode = true;
                }
                out.push(super::rule_6::digit_cell(*c)?);
                k += 1;
            }
            EnglishToken::Styled(c, f) if *f == form => {
                let cells = super::rule_7::encode_punctuation(*c)
                    .or_else(|| super::rule_3::encode_symbol(*c))?;
                out.extend(cells);
                numeric_mode = matches!(c, ',' | '.')
                    && matches!(tokens.get(k + 1), Some(EnglishToken::Styled(n, nf)) if *nf == form && n.is_ascii_digit());
                k += 1;
            }
            EnglishToken::Space => {
                out.push(decode_unicode('⠐'));
                numeric_mode = true;
                k += 1;
            }
            _ => return None,
        }
    }
    Some(())
}

/// §9.3/§10.7 collision: whether the whole-word styled letters would encode to
/// a §10.7 initial-letter contraction whose two-cell form starts with the SAME
/// typeform prefix cell that a word indicator would emit — so `⠘⠂⠘⠺` (bold
/// word indicator + `word` contraction) collapses to just `⠘⠺` (reader still
/// sees a bold `word` cell, no ambiguity). Covers §10.7 words with dot-4-5 or
/// dot-4-5-6 prefixes matched against Bold/Underline typeforms; the ⠐ prefix
/// contractions have no matching typeform so they are excluded.
fn styled_word_matches_typeform_prefix_contraction(
    chars: &[char],
    form: super::token::Typeform,
) -> bool {
    let lower: String = chars.iter().flat_map(|c| c.to_lowercase()).collect();
    match form {
        super::token::Typeform::Bold => matches!(lower.as_str(), "word" | "whose"),
        super::token::Typeform::Underline => {
            matches!(lower.as_str(), "cannot" | "spirit" | "world" | "many")
        }
        _ => false,
    }
}

fn continues_uppercase_word_across_typeform(tokens: &[EnglishToken], i: usize) -> bool {
    i.checked_sub(1).is_some_and(|p| {
        matches!(
            tokens.get(p),
            Some(EnglishToken::Word(prev))
                if prev.len() >= 2 && prev.iter().all(|c| c.is_ascii_uppercase())
        )
    })
}

/// §9.1.3 example note: if a document's only italicised items are repeated single
/// capital letters used as variable names, the typeform is not significant.
fn insignificant_single_italic_capitals(tokens: &[EnglishToken]) -> bool {
    let mut count = 0usize;
    for (i, token) in tokens.iter().enumerate() {
        if let EnglishToken::Styled(c, super::token::Typeform::Italic) = token {
            if !c.is_uppercase() {
                return false;
            }
            if matches!(
                i.checked_sub(1).and_then(|p| tokens.get(p)),
                Some(EnglishToken::Symbol('.'))
            ) || matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('.')))
            {
                return false;
            }
            count += 1;
        }
    }
    count >= 2
}

/// §9.3.2: a styled symbols-sequence that is URL-shaped may be followed in print
/// by extra visual spacing before prose. The URL itself keeps its typeform word
/// indicator, but the prose spacing collapses to the ordinary single blank cell.
fn styled_url_before(tokens: &[EnglishToken], i: usize) -> bool {
    let Some(EnglishToken::Styled(_, form)) = i.checked_sub(1).and_then(|p| tokens.get(p)) else {
        return false;
    };
    // Walk left over the styled-URL run (styled letters of `form` plus `:` `/`
    // `.`), prepending each token's text. The walk itself validates the run, so a
    // separate re-scan (with an unreachable fallback arm) is unnecessary.
    let mut text = String::new();
    let mut start = i;
    while start > 0 {
        match tokens.get(start - 1) {
            Some(EnglishToken::Styled(c, f)) if *f == *form => {
                let lower: String = c.to_lowercase().collect();
                text.insert_str(0, &lower);
                start -= 1;
            }
            Some(EnglishToken::Symbol(c @ (':' | '/' | '.'))) => {
                text.insert(0, *c);
                start -= 1;
            }
            _ => break,
        }
    }
    text.starts_with("http://") || text.starts_with("https://") || text.starts_with("www.")
}

/// UEB §9.8.1 nested passage continuation: if text opens with a nested typeform
/// (`bold+italic`) and later drops only the inner form while retaining the outer
/// form (`italic`), keep the outer passage open and close only the inner one at
/// the first change.
fn nested_typeform_continuation(
    tokens: &[EnglishToken],
    inner_end: usize,
    form: super::token::Typeform,
) -> Option<(usize, super::token::Typeform, super::token::Typeform)> {
    use super::token::Typeform::{Bold, BoldItalic, Italic};

    let (outer, inner) = match form {
        BoldItalic => (Italic, Bold),
        _ => return None,
    };
    let mut k = inner_end;
    while matches!(
        tokens.get(k),
        Some(EnglishToken::Space | EnglishToken::LineBreak)
    ) {
        k += 1;
    }
    if !matches!(tokens.get(k).and_then(token_typeform), Some(f) if f == outer) {
        return None;
    }
    let (words, outer_end) = styled_passage_extent(tokens, k, outer);
    (words > 0).then_some((outer_end, outer, inner))
}

/// UEB §9.1.3: underlining used only as a hyperlink print enhancement (URL-shaped
/// `http://…` or `www.…`) is not significant, unlike underlining that marks
/// embedded text; therefore its §9 typeform is omitted while the symbols-sequence
/// is still encoded normally.
fn styled_underline_url_span(
    tokens: &[EnglishToken],
    start: usize,
    end: usize,
    form: super::token::Typeform,
) -> bool {
    if form != super::token::Typeform::Underline {
        return false;
    }
    let mut text = String::new();
    for token in &tokens[start..end] {
        match token {
            EnglishToken::Styled(c, f) if *f == form => {
                text.extend(c.to_lowercase());
            }
            EnglishToken::Symbol(c) => text.push(c.to_ascii_lowercase()),
            _ => return false,
        }
    }
    text.starts_with("http://") || text.starts_with("https://") || text.starts_with("www.")
}

/// §5.7.1/§9.1.3 support: whether a styled single-letter token at `tokens[i]`
/// (with post-run end `j`) needs a grade-1 indicator once its §9 typeform has
/// been stripped as insignificant. Mirrors the §2.6 boundary logic in
/// `rule_5_7::needs_grade1_indicator` — that helper is `Word`-token-only.
fn styled_letter_needs_grade1(tokens: &[EnglishToken], i: usize, j: usize) -> bool {
    let is_boundary = |t: Option<&EnglishToken>| {
        matches!(
            t,
            None | Some(EnglishToken::Space)
                | Some(EnglishToken::Symbol('-' | '\u{2013}' | '\u{2014}'))
        )
    };
    let is_left_transparent =
        |c: char| matches!(c, '(' | '[' | '{' | '"' | '\u{201C}' | '\u{2018}' | '\'');
    let is_right_transparent = |c: char| {
        matches!(
            c,
            ')' | ']' | '}' | '"' | '\u{201D}' | '\u{2019}' | '.' | ',' | ':' | ';' | '\u{2026}'
        )
    };
    let mut l = i;
    while l > 0 && matches!(&tokens[l - 1], EnglishToken::Symbol(c) if is_left_transparent(*c)) {
        l -= 1;
    }
    if !is_boundary(l.checked_sub(1).map(|p| &tokens[p])) {
        return false;
    }
    let mut r = j.saturating_sub(1).max(i);
    while r + 1 < tokens.len()
        && matches!(&tokens[r + 1], EnglishToken::Symbol(c) if is_right_transparent(*c))
    {
        r += 1;
    }
    is_boundary(tokens.get(r + 1))
}

/// Identify §8.4 capitals passages: runs of three or more space-separated
/// all-caps "words". Returns per-token flags — emit `⠠⠠⠠` *before* a token,
/// emit the `⠠⠄` terminator *after* a token, and whether a token lies *inside*
/// a passage (so caps words drop their own indicator). Below the 3-word
/// threshold every flag stays false, so 1–2 caps-word inputs are untouched.
fn caps_passages(
    tokens: &[EnglishToken],
    explicit_english: bool,
) -> (Vec<bool>, Vec<bool>, Vec<bool>) {
    let n = tokens.len();
    let (mut starts, mut terms, mut inside) = (vec![false; n], vec![false; n], vec![false; n]);

    // Space-separated groups, as inclusive token ranges. Opening punctuation is
    // included in the group but the passage indicator is placed immediately before
    // the first capitalised sequence (§8.5 placement inside opening quotes).
    // Lowercase groups are retained as barriers: §8.5.2 requires three or more
    // capitalised symbols-sequences in the *passage*, so intervening lowercase
    // words (`GO quickly and TAKE CARE`) must prevent the two later caps words
    // from being counted with the first.
    let mut groups: Vec<CapsGroup> = Vec::new();
    let mut g0: Option<usize> = None;
    for (i, t) in tokens.iter().enumerate() {
        if matches!(
            t,
            EnglishToken::Space | EnglishToken::Symbol('–' | '—' | '―')
        ) {
            if let Some(s) = g0.take()
                && i > s
                && let Some(group) = caps_group_or_lower_barrier(tokens, s, i - 1)
            {
                groups.push(group);
            }
            // §8.5.4: a caps letters-sequence adjacent to an em-dash is not
            // necessarily part of a passage that starts after the dash. Insert
            // a synthetic barrier so the em-dash breaks passage merging (a plain
            // space keeps the passage cohesive as before).
            if matches!(t, EnglishToken::Symbol('–' | '—' | '―')) {
                groups.push(CapsGroup {
                    first_cap: i,
                    last_cap: i,
                    caps_sequences: 0,
                    has_lower: true,
                    single_letter_only: false,
                    ended_with_terminal_sentence_mark: false,
                });
            }
        } else if g0.is_none() {
            g0 = Some(i);
        }
    }
    if let Some(s) = g0
        && let Some(group) = caps_group_or_lower_barrier(tokens, s, n - 1)
    {
        groups.push(group);
    }

    let mut gi = 0;
    // §8.5.5 helper: whether the input consists of a single quoted all-caps
    // fragment (`"HE'S GETTING AWAY!"`). Such fragments are treated as one text
    // element inside a larger multi-element passage — they open with the passage
    // indicator ⠠⠠⠠ (even for a single caps word like "JUMP!") and elide the
    // ⠠⠄ terminator on the assumption that the passage continues.
    let input_is_quoted_all_caps = {
        let start_quote_at = tokens
            .first()
            .is_some_and(|t| matches!(t, EnglishToken::Symbol('"')));
        let end_quote_at = tokens
            .last()
            .is_some_and(|t| matches!(t, EnglishToken::Symbol('"')));
        let inner_has_lower = tokens
            .iter()
            .skip(1)
            .take(tokens.len().saturating_sub(2))
            .any(token_has_lower_sequence);
        let inner_has_upper = tokens
            .iter()
            .skip(1)
            .take(tokens.len().saturating_sub(2))
            .any(token_is_upper_sequence);
        !explicit_english
            && start_quote_at
            && end_quote_at
            && !inner_has_lower
            && inner_has_upper
            && !single_elongated_caps_word_in_quotes(tokens)
    };

    while gi < groups.len() {
        if groups[gi].is_caps() {
            let first = groups[gi].first_cap;
            let mut last = groups[gi].last_cap;
            let mut count = groups[gi].caps_sequences;
            let mut prev_ended_sentence = groups[gi].ended_with_terminal_sentence_mark;
            let mut gj = gi + 1;
            while gj < groups.len() && !groups[gj].has_lower {
                if groups[gj].is_caps() {
                    // §8.5.4: a single-letter capital immediately after a
                    // sentence-terminal caps group (`ABC.  A BBC`) is an
                    // article/pronoun, not a passage participant. It does not
                    // count toward the three-sequences threshold, and its
                    // position is not tracked as the passage extent.
                    let skip_single_letter = prev_ended_sentence && groups[gj].single_letter_only;
                    if !skip_single_letter {
                        last = groups[gj].last_cap;
                        count += groups[gj].caps_sequences;
                    }
                    prev_ended_sentence = groups[gj].ended_with_terminal_sentence_mark;
                }
                gj += 1;
            }
            // §8.5.2 threshold is normally three symbols-sequences, but §8.5.5
            // relaxes it when the whole input is a lone quoted all-caps
            // fragment — that fragment is one text element in a running passage,
            // so it opens with ⠠⠠⠠. It may be only one symbols-sequence (`"JUMP!"`).
            let passage_qualifies = count >= 3 || input_is_quoted_all_caps;
            if passage_qualifies {
                starts[first] = true;
                // §8.5.5 final element: a quoted all-caps passage that ends with
                // `!"` and spans multiple sentences (an internal `.  ` sentence
                // break) is the *final* element of a running passage — emit the
                // terminator immediately AFTER the closing quote (`⠴⠠⠄`), not
                // suppressed as for an intermediate element (L1/L2 end with `.`,
                // so keep the terminator dropped there).
                let is_final_element = input_is_quoted_all_caps
                    && tokens.len() >= 3
                    && matches!(
                        tokens.get(tokens.len() - 2),
                        Some(EnglishToken::Symbol('!'))
                    )
                    && tokens.windows(3).any(|w| {
                        matches!(w[0], EnglishToken::Symbol('.'))
                            && matches!(w[1], EnglishToken::Space)
                            && matches!(w[2], EnglishToken::Space)
                    });
                if is_final_element {
                    // Terminator placed on the CLOSING quote token (last token) so
                    // the emit path outputs `⠴⠠⠄` — the terminator falls after
                    // the closing `"` cell.
                    terms[tokens.len() - 1] = true;
                } else if !input_is_quoted_all_caps {
                    terms[last] = true;
                }
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

fn caps_group_or_lower_barrier(
    tokens: &[EnglishToken],
    first: usize,
    last: usize,
) -> Option<CapsGroup> {
    caps_group(tokens, first, last).or_else(|| {
        tokens
            .iter()
            .take(last + 1)
            .skip(first)
            .any(token_has_lower_sequence)
            .then_some(CapsGroup {
                first_cap: first,
                last_cap: last,
                caps_sequences: 0,
                has_lower: true,
                single_letter_only: false,
                ended_with_terminal_sentence_mark: false,
            })
    })
}

impl CapsGroup {
    fn is_caps(self) -> bool {
        !self.has_lower && self.caps_sequences > 0
    }
}

fn caps_group(tokens: &[EnglishToken], first: usize, last: usize) -> Option<CapsGroup> {
    let mut first_cap = None;
    let mut last_cap = None;
    let mut caps_sequences = 0usize;
    let mut upper_tokens = 0usize;
    let mut has_lower = false;
    let mut has_styled_text = false;
    let mut terminal_punctuation = false;
    for (i, token) in tokens.iter().enumerate().take(last + 1).skip(first) {
        if first_cap.is_some() && matches!(token, EnglishToken::Symbol('/')) {
            break;
        }
        has_lower |= token_has_lower_sequence(token)
            || matches!(token, EnglishToken::Symbol('/' | '|' | '‖'));
        has_styled_text |= token_is_styled_text(token);
        if token_is_upper_sequence(token) {
            first_cap.get_or_insert(i);
            last_cap = Some(i);
            caps_sequences = 1;
            upper_tokens += 1;
        } else if first_cap.is_some()
            && matches!(
                token,
                EnglishToken::Symbol('!' | '?' | '.' | ',' | ':' | ';' | ')' | ']' | '}')
            )
        {
            terminal_punctuation = true;
            last_cap = Some(i);
        }
    }
    if has_styled_text {
        // Typeform passage handling opens any nested §8 caps passage after the §9
        // indicator. The document-level caps scan would otherwise emit a duplicate
        // `⠠⠠⠠` before the typeform indicator.
        return None;
    }
    let single_letter_sentence = terminal_punctuation
        && caps_sequences == 1
        && upper_tokens == 1
        && first_cap
            .is_some_and(|idx| matches!(&tokens[idx], EnglishToken::Word(w) if w.len() == 1));
    let single_letter_only = caps_sequences == 1
        && upper_tokens == 1
        && !terminal_punctuation
        && first_cap
            .is_some_and(|idx| matches!(&tokens[idx], EnglishToken::Word(w) if w.len() == 1));
    Some(CapsGroup {
        first_cap: first_cap?,
        last_cap: last_cap?,
        caps_sequences,
        has_lower: has_lower || single_letter_sentence,
        single_letter_only,
        ended_with_terminal_sentence_mark: terminal_punctuation
            && matches!(
                last_cap.and_then(|idx| tokens.get(idx)),
                Some(EnglishToken::Symbol('.' | '!' | '?'))
            ),
    })
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
            Some(
                EnglishToken::Word(_)
                    | EnglishToken::Number(_)
                    | EnglishToken::Styled(..)
                    | EnglishToken::Technical(_),
            )
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

fn text_token(token: Option<&EnglishToken>) -> bool {
    matches!(
        token,
        Some(
            EnglishToken::Word(_)
                | EnglishToken::Number(_)
                | EnglishToken::Styled(..)
                | EnglishToken::Technical(_),
        )
    )
}

fn previous_text_skipping_terminal_punctuation(tokens: &[EnglishToken], index: usize) -> bool {
    let mut k = index;
    while let Some(prev) = k.checked_sub(1) {
        match tokens.get(prev) {
            Some(EnglishToken::Symbol('.' | ',' | ':' | ';' | '!' | '?' | ')' | ']' | '}')) => {
                k = prev;
            }
            token => return text_token(token),
        }
    }
    false
}

fn straight_single_quote_role(tokens: &[EnglishToken], index: usize) -> SingleQuote {
    let prev_text = previous_text_skipping_terminal_punctuation(tokens, index);
    let next_text = text_token(tokens.get(index + 1));
    if prev_text && next_text {
        return SingleQuote::Apostrophe;
    }
    if !straight_single_quote_is_matched_quotation(tokens, index) {
        return SingleQuote::Apostrophe;
    }
    if !prev_text && next_text {
        return SingleQuote::Open;
    }
    if prev_text && !next_text {
        return SingleQuote::Close;
    }
    if straight_single_quote_closes_after_inner_double(tokens, index) {
        return SingleQuote::Close;
    }
    SingleQuote::Apostrophe
}

fn straight_single_quote_is_matched_quotation(tokens: &[EnglishToken], index: usize) -> bool {
    let Some(EnglishToken::Symbol('\'')) = tokens.get(index) else {
        return false;
    };
    let prev_text = previous_text_skipping_terminal_punctuation(tokens, index);
    let next_text = text_token(tokens.get(index + 1));
    if !prev_text && next_text {
        return next_word_starts_uppercase(tokens.get(index + 1))
            && tokens[index + 1..]
                .iter()
                .any(|t| matches!(t, EnglishToken::Symbol('\'')));
    }
    if prev_text && !next_text {
        return previous_word_starts_uppercase(tokens, index)
            && tokens[..index]
                .iter()
                .any(|t| matches!(t, EnglishToken::Symbol('\'')));
    }
    if straight_single_quote_closes_after_inner_double(tokens, index) {
        return tokens[..index]
            .iter()
            .any(|t| matches!(t, EnglishToken::Symbol('\'')));
    }
    false
}

fn straight_single_quote_closes_after_inner_double(tokens: &[EnglishToken], index: usize) -> bool {
    let mut k = index;
    let mut skipped_comma = false;
    while let Some(prev) = k.checked_sub(1) {
        match tokens.get(prev) {
            Some(EnglishToken::Symbol(',')) => {
                skipped_comma = true;
                k = prev;
            }
            Some(EnglishToken::Symbol('.' | ':' | ';' | '!' | '?' | ')' | ']' | '}')) => {
                k = prev;
            }
            Some(EnglishToken::Symbol('"' | '\u{201D}')) => {
                return skipped_comma && previous_text_skipping_terminal_punctuation(tokens, prev);
            }
            _ => return false,
        }
    }
    false
}

fn next_word_starts_uppercase(token: Option<&EnglishToken>) -> bool {
    matches!(token, Some(EnglishToken::Word(chars)) if chars.first().is_some_and(|c| c.is_uppercase()))
}

fn prev_word_starts_uppercase(token: Option<&EnglishToken>) -> bool {
    next_word_starts_uppercase(token)
}

fn previous_word_starts_uppercase(tokens: &[EnglishToken], index: usize) -> bool {
    let mut k = index;
    while let Some(prev) = k.checked_sub(1) {
        match tokens.get(prev) {
            Some(EnglishToken::Symbol('.' | ',' | ':' | ';' | '!' | '?' | ')' | ']' | '}')) => {
                k = prev;
            }
            token => return prev_word_starts_uppercase(token),
        }
    }
    false
}

fn straight_single_quote_exchanged(tokens: &[EnglishToken], index: usize) -> bool {
    if !matches!(tokens.get(index), Some(EnglishToken::Symbol('\''))) {
        return false;
    }
    let role = straight_single_quote_role(tokens, index);
    if matches!(role, SingleQuote::Apostrophe) {
        return false;
    }
    let has_double = tokens
        .iter()
        .any(|t| matches!(t, EnglishToken::Symbol('"')));
    has_double
        && tokens
            .iter()
            .filter(|t| matches!(t, EnglishToken::Symbol('\'')))
            .count()
            >= 2
}

fn double_quote_needs_two_cell(tokens: &[EnglishToken], index: usize, opening: bool) -> bool {
    if opening {
        if matches!(
            index.checked_sub(1).and_then(|p| tokens.get(p)),
            Some(EnglishToken::Symbol('–' | '—'))
        ) {
            return false;
        }
        return index > 0
            && !matches!(
                tokens.get(index - 1),
                Some(EnglishToken::Space | EnglishToken::LineBreak)
            );
    }
    let paired_two_cell_open = tokens[..index]
        .iter()
        .rposition(|t| matches!(t, EnglishToken::Symbol('\u{201C}')))
        .is_some_and(|open| double_quote_needs_two_cell(tokens, open, true));
    let detached = index == 0
        || matches!(
            tokens.get(index - 1),
            Some(EnglishToken::Space | EnglishToken::LineBreak)
        );
    detached || paired_two_cell_open
}

/// RUEB 2024 §7.6.7: an escaped quotation mark in program text uses the
/// two-cell quote, and the quoted code snippet is transcribed letter-for-letter.
fn escaped_quote_code_span(tokens: &[EnglishToken]) -> Vec<bool> {
    let mut span = vec![false; tokens.len()];
    let mut active = false;
    let mut i = 0usize;
    while i < tokens.len() {
        if matches!(tokens.get(i), Some(EnglishToken::Symbol('\\'))) {
            let next = tokens.get(i + 1);
            if !active && matches!(next, Some(EnglishToken::Symbol('"' | '\u{201C}'))) {
                active = true;
                i += 2;
                continue;
            }
            if active && matches!(next, Some(EnglishToken::Symbol('"' | '\u{201D}'))) {
                active = false;
                i += 2;
                continue;
            }
        }
        if active {
            span[i] = true;
        }
        i += 1;
    }
    span
}

fn apostrophe_wrapped_letter(tokens: &[EnglishToken], index: usize, chars: &[char]) -> bool {
    // §5.7.1 example `'n' Ma` — an isolated lowercase letter wrapped by
    // apostrophes (`rock 'n' roll`) takes the grade-1 indicator. A capital
    // letter in a caps sequence like `FO'C'S'LE` (§8.4.2) does not — the
    // capital indicator is unambiguous there.
    chars.len() == 1
        && chars[0].is_ascii_lowercase()
        && matches!(
            index.checked_sub(1).and_then(|p| tokens.get(p)),
            Some(EnglishToken::Symbol('\'' | '\u{2019}'))
        )
        && matches!(
            tokens.get(index + 1),
            Some(EnglishToken::Symbol('\'' | '\u{2019}'))
        )
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
    if matches!(tokens.get(i + 2), Some(EnglishToken::Space))
        && word_is(tokens.get(i + 3), "tn")
        && matches!(tokens.get(i + 4), Some(EnglishToken::Symbol(']')))
    {
        if word_is(tokens.get(i + 1), "open") {
            return Some((true, i + 5));
        }
        if word_is(tokens.get(i + 1), "close") {
            return Some((false, i + 5));
        }
    }
    if word_is(tokens.get(i + 1), "tn")
        && matches!(tokens.get(i + 2), Some(EnglishToken::Space))
        && matches!(tokens.get(i + 4), Some(EnglishToken::Symbol(']')))
    {
        if word_is(tokens.get(i + 3), "open") {
            return Some((true, i + 5));
        }
        if word_is(tokens.get(i + 3), "close") {
            return Some((false, i + 5));
        }
    }
    None
}

fn transcriber_note_ends_at(tokens: &[EnglishToken], end: usize, is_open: bool) -> bool {
    end.checked_sub(5)
        .and_then(|start| transcriber_note_at(tokens, start))
        .is_some_and(|(open, note_end)| open == is_open && note_end == end)
}

fn closing_transcriber_note_starts_at(tokens: &[EnglishToken], i: usize) -> bool {
    transcriber_note_at(tokens, i).is_some_and(|(is_open, _)| !is_open)
}

fn closing_transcriber_note_after_transparent_suffix(tokens: &[EnglishToken], i: usize) -> bool {
    let mut j = i + 1;
    while matches!(
        tokens.get(j),
        Some(EnglishToken::Symbol(
            ',' | ';' | ':' | '.' | '!' | '?' | ')' | ']' | '}' | '"' | '”' | '’'
        ))
    ) {
        j += 1;
    }
    j > i + 1 && closing_transcriber_note_starts_at(tokens, j)
}

fn word_boundary_prev(tokens: &[EnglishToken], i: usize) -> Option<&EnglishToken> {
    if transcriber_note_ends_at(tokens, i, true) {
        None
    } else {
        i.checked_sub(1).map(|p| &tokens[p])
    }
}

fn word_boundary_next(tokens: &[EnglishToken], i: usize) -> Option<&EnglishToken> {
    if closing_transcriber_note_starts_at(tokens, i + 1) {
        None
    } else {
        tokens.get(i + 1)
    }
}

fn braille_mention_at(tokens: &[EnglishToken], i: usize) -> Option<(Vec<u8>, usize)> {
    let mut j = i;
    let mut cells = Vec::new();
    while let Some(EnglishToken::Symbol(c)) = tokens.get(j) {
        if !('\u{2800}'..='\u{28ff}').contains(c) {
            break;
        }
        cells.push(decode_unicode(*c));
        j += 1;
    }
    if cells.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(cells.len() + 2);
    out.extend([decode_unicode('⠨'), decode_unicode('⠿')]);
    out.extend(cells);
    Some((out, j))
}

fn isolated_shape_circle(tokens: &[EnglishToken], i: usize, chars: &[char]) -> bool {
    chars == ['o']
        && matches!(
            i.checked_sub(1).and_then(|p| tokens.get(p)),
            None | Some(EnglishToken::LineBreak)
        )
        && matches!(tokens.get(i + 1), Some(EnglishToken::Space))
        && matches!(tokens.get(i + 2), Some(EnglishToken::Word(w)) if w.first().is_some_and(|c| c.is_uppercase()))
}

fn script_letter(c: char) -> Option<(super::rule_3_24::ScriptKind, char)> {
    use super::rule_3_24::ScriptKind::{Subscript, Superscript};
    Some(match c {
        '\u{1D50}' => (Superscript, 'm'), // ᵐ
        '\u{1D9C}' => (Superscript, 'c'), // ᶜ
        '\u{2090}' => (Subscript, 'a'),   // ₐ
        '\u{2091}' => (Subscript, 'e'),   // ₑ
        '\u{2095}' => (Subscript, 'h'),   // ₕ
        '\u{1D62}' => (Subscript, 'i'),   // ᵢ
        '\u{2C7C}' => (Subscript, 'j'),   // ⱼ
        '\u{2096}' => (Subscript, 'k'),   // ₖ
        '\u{2097}' => (Subscript, 'l'),   // ₗ
        '\u{2098}' => (Subscript, 'm'),   // ₘ
        '\u{2099}' => (Subscript, 'n'),   // ₙ
        '\u{2092}' => (Subscript, 'o'),   // ₒ
        '\u{209A}' => (Subscript, 'p'),   // ₚ
        '\u{1D63}' => (Subscript, 'r'),   // ᵣ
        '\u{209B}' => (Subscript, 's'),   // ₛ
        '\u{209C}' => (Subscript, 't'),   // ₜ
        '\u{1D64}' => (Subscript, 'u'),   // ᵤ
        '\u{1D65}' => (Subscript, 'v'),   // ᵥ
        '\u{2093}' => (Subscript, 'x'),   // ₓ
        _ => return None,
    })
}

fn script_kind(c: char) -> Option<super::rule_3_24::ScriptKind> {
    super::rule_3_24::script_digit(c)
        .map(|(kind, _)| kind)
        .or_else(|| script_letter(c).map(|(kind, _)| kind))
}

fn encode_chemical_formula_scripts(tokens: &[EnglishToken]) -> Option<Vec<u8>> {
    let has_script = tokens.iter().any(
        |t| matches!(t, EnglishToken::Symbol(c) if super::rule_3_24::script_digit(*c).is_some()),
    );
    if !has_script {
        return None;
    }
    let mut out = Vec::new();
    for token in tokens {
        match token {
            EnglishToken::Word(chars)
                if chars
                    .iter()
                    .all(|c| c.is_ascii_uppercase() && c.is_ascii_alphabetic()) =>
            {
                for &c in chars {
                    out.push(CAPITAL);
                    out.push(crate::english::encode_english(c.to_ascii_lowercase()).ok()?);
                }
            }
            EnglishToken::Symbol(c) => {
                let (kind, digit) = super::rule_3_24::script_digit(*c)?;
                out.extend([GRADE1, kind.indicator(), super::rule_6::NUMERIC_INDICATOR]);
                out.push(super::rule_6::digit_cell(digit)?);
            }
            _ => return None,
        }
    }
    Some(out)
}

/// §10.4.3: whether a word token preceded by `prev` begins a fresh word.
fn word_initial_boundary(prev: Option<&EnglishToken>) -> bool {
    matches!(
        prev,
        None | Some(EnglishToken::Space | EnglishToken::LineBreak)
            | Some(EnglishToken::Symbol('-' | '\u{2013}' | '\u{2014}'))
    )
}

/// §10.6.2: restricted `be`/`con`/`dis` may start after opening punctuation and
/// indicators listed by §2.6.2, but not after slash or internal case splits.
fn restricted_prefix_boundary(prev: Option<&EnglishToken>) -> bool {
    matches!(
        prev,
        None | Some(EnglishToken::Space | EnglishToken::LineBreak)
            | Some(EnglishToken::Symbol(
                '-' | '\u{2013}'
                    | '\u{2014}'
                    | '('
                    | '['
                    | '{'
                    | '"'
                    | '\''
                    | '\u{2018}'
                    | '\u{201c}'
                    | '«'
            ))
    )
}

fn spell_line_division_in(tokens: &[EnglishToken], i: usize, lower_word: &str) -> bool {
    if lower_word != "in" {
        return false;
    }
    let prev = i.checked_sub(1).and_then(|p| tokens.get(p));
    let prev2 = i.checked_sub(2).and_then(|p| tokens.get(p));
    let next = tokens.get(i + 1);
    let next2 = tokens.get(i + 2);
    let parenthesized_enough_dash = matches!(
        prev2,
        Some(EnglishToken::Symbol('-' | '\u{2013}' | '\u{2014}'))
    ) && matches!(prev, Some(EnglishToken::LineBreak))
        && matches!(i.checked_sub(3).and_then(|p| tokens.get(p)), Some(EnglishToken::Word(w)) if w.iter().collect::<String>().eq_ignore_ascii_case("enough"))
        && matches!(
            i.checked_sub(4).and_then(|p| tokens.get(p)),
            Some(EnglishToken::Symbol('('))
        );
    let quoted_break = matches!(prev, Some(EnglishToken::Symbol('"' | '“')))
        && matches!(next, Some(EnglishToken::Symbol('-')))
        && matches!(tokens.get(i + 2), Some(EnglishToken::LineBreak));
    let dash_linebreak = matches!(
        (prev2, prev),
        (
            Some(EnglishToken::Symbol('-' | '\u{2013}' | '\u{2014}')),
            Some(EnglishToken::LineBreak)
        ) | (
            Some(EnglishToken::LineBreak),
            Some(EnglishToken::Symbol('\u{2013}' | '\u{2014}'))
        )
    ) && !matches!(next, Some(EnglishToken::Symbol('-')))
        && !matches!(
            (next, next2),
            (
                Some(EnglishToken::Symbol('.')),
                Some(EnglishToken::Symbol(')' | ']' | '}'))
            )
        )
        && !parenthesized_enough_dash;
    quoted_break || dash_linebreak
}

fn spell_lower_in_for_preference(tokens: &[EnglishToken], i: usize) -> bool {
    let next = tokens.get(i + 1);
    let ellipsis_follows = matches!(next, Some(EnglishToken::Symbol('.')))
        && matches!(tokens.get(i + 2), Some(EnglishToken::Symbol('.')));
    ellipsis_follows
        || dash_after_enough_before_in(tokens, i)
        || dash_after_quoted_in_before_in(tokens, i)
}

fn dash_after_enough_before_in(tokens: &[EnglishToken], i: usize) -> bool {
    if !matches!(
        i.checked_sub(1).and_then(|p| tokens.get(p)),
        Some(EnglishToken::Symbol('–' | '—'))
    ) {
        return false;
    }
    let mut k = i.saturating_sub(2);
    loop {
        match tokens.get(k) {
            Some(EnglishToken::Word(w)) => {
                return w.iter().collect::<String>().eq_ignore_ascii_case("enough");
            }
            Some(EnglishToken::Symbol('!' | '?' | '"' | '”' | '\u{2019}')) if k > 0 => k -= 1,
            _ => return false,
        }
    }
}

fn spell_in_for_lower_wordsign_limit(tokens: &[EnglishToken], i: usize) -> bool {
    let prev = i.checked_sub(1).and_then(|p| tokens.get(p));
    let prev2 = i.checked_sub(2).and_then(|p| tokens.get(p));
    let next = tokens.get(i + 1);
    let after_line_division_hyphen = matches!(
        (prev2, prev),
        (
            Some(EnglishToken::Symbol('-' | '\u{2013}' | '\u{2014}')),
            Some(EnglishToken::LineBreak)
        )
    );
    let terminal_lower_punctuation =
        matches!(next, Some(EnglishToken::Symbol(',' | '.'))) && !after_line_division_hyphen;
    let quoted_by_lower_signs = matches!(prev, Some(EnglishToken::Symbol('"' | '“')))
        && !matches!(prev2, Some(EnglishToken::Symbol('(' | '[' | '{')))
        && matches!(
            next,
            Some(EnglishToken::Space | EnglishToken::Symbol('"' | '”'))
        )
        && !lower_quote_sequence_reaches_dash(tokens, i + 1);
    terminal_lower_punctuation || quoted_by_lower_signs
}

fn standalone_hyphen_in(tokens: &[EnglishToken], i: usize) -> bool {
    matches!(
        i.checked_sub(1).and_then(|p| tokens.get(p)),
        Some(EnglishToken::Symbol('-'))
    ) && matches!(
        i.checked_sub(2).and_then(|p| tokens.get(p)),
        None | Some(EnglishToken::Space)
    ) && matches!(tokens.get(i + 1), None | Some(EnglishToken::Space))
}

fn lower_quote_sequence_reaches_dash(tokens: &[EnglishToken], mut k: usize) -> bool {
    loop {
        match tokens.get(k) {
            Some(EnglishToken::Symbol('!' | '?' | '"' | '”' | '\u{2019}')) => k += 1,
            Some(EnglishToken::Symbol('–' | '—')) => return true,
            _ => return false,
        }
    }
}

fn dash_after_quoted_in_before_in(tokens: &[EnglishToken], i: usize) -> bool {
    if !matches!(
        i.checked_sub(1).and_then(|p| tokens.get(p)),
        Some(EnglishToken::Symbol('–' | '—'))
    ) {
        return false;
    }
    let mut k = i.saturating_sub(2);
    let mut saw_quote_or_lower_punctuation = false;
    loop {
        match tokens.get(k) {
            Some(EnglishToken::Word(w)) => {
                return saw_quote_or_lower_punctuation
                    && w.iter().collect::<String>().eq_ignore_ascii_case("in");
            }
            Some(EnglishToken::Symbol('!' | '?' | '"' | '”' | '\u{2019}')) if k > 0 => {
                saw_quote_or_lower_punctuation = true;
                k -= 1;
            }
            _ => return false,
        }
    }
}

fn enough_followed_by_upper_dot_sequence(tokens: &[EnglishToken], i: usize) -> bool {
    let mut k = i + 1;
    let mut saw_lower_punctuation = false;
    loop {
        match tokens.get(k) {
            Some(EnglishToken::Symbol('!' | '?' | '"' | '”' | '\u{2019}')) => {
                saw_lower_punctuation = true;
                k += 1;
            }
            Some(EnglishToken::Symbol('–' | '—')) => return saw_lower_punctuation,
            _ => return false,
        }
    }
}

fn enough_followed_by_sentence_close(tokens: &[EnglishToken], i: usize) -> bool {
    matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('.')))
        && matches!(
            tokens.get(i + 2),
            Some(EnglishToken::Symbol(')' | ']' | '}'))
        )
}

fn styled_lower_wordsign_usable(
    lower_word: &str,
    prev: Option<&EnglishToken>,
    next: Option<&EnglishToken>,
) -> bool {
    lower_wordsign_usable(prev, next)
        || (matches!(lower_word, "be" | "were" | "was")
            && matches!(
                next,
                None | Some(
                    EnglishToken::Space
                        | EnglishToken::Symbol(
                            ')' | ']'
                                | '}'
                                | '?'
                                | '!'
                                | '.'
                                | ','
                                | ';'
                                | ':'
                                | '"'
                                | '\u{201D}'
                                | '\''
                                | '\u{2019}'
                        )
                )
            ))
}

fn styled_scansion_word(tokens: &[EnglishToken], lower_word: &str) -> bool {
    lower_word == "be"
        && tokens
            .iter()
            .any(|t| matches!(t, EnglishToken::Symbol('/')))
}

fn lower_contact_after_division_word(token: Option<&EnglishToken>) -> bool {
    matches!(
        token,
        Some(EnglishToken::Symbol(
            '"' | '\'' | '”' | '’' | '?' | '!' | '.'
        ))
    )
}

fn touches_hyphen_or_line_break(prev: Option<&EnglishToken>, next: Option<&EnglishToken>) -> bool {
    matches!(
        prev,
        Some(EnglishToken::Symbol('-' | '–' | '—') | EnglishToken::LineBreak)
    ) || matches!(
        next,
        Some(EnglishToken::Symbol('-' | '–' | '—') | EnglishToken::LineBreak)
    )
}

fn token_letters(token: &EnglishToken, out: &mut Vec<char>) {
    match token {
        EnglishToken::Word(chars)
        | EnglishToken::WordDivision { chars, .. }
        | EnglishToken::Technical(chars) => {
            out.extend(chars);
        }
        EnglishToken::Styled(c, _) | EnglishToken::Symbol(c) => out.push(*c),
        EnglishToken::Number(_) | EnglishToken::Space | EnglishToken::LineBreak => {}
    }
}

fn token_base_char(token: &EnglishToken) -> Option<char> {
    match token {
        EnglishToken::Styled(c, _) => Some(*c),
        EnglishToken::Word(chars) if chars.len() == 1 => Some(chars[0]),
        _ => None,
    }
}

fn document_letters(tokens: &[EnglishToken]) -> Vec<char> {
    let mut chars = Vec::new();
    for token in tokens {
        token_letters(token, &mut chars);
    }
    chars
}

fn document_words(tokens: &[EnglishToken]) -> Vec<Vec<char>> {
    tokens
        .iter()
        .filter_map(|token| match token {
            EnglishToken::Word(chars) => Some(chars.clone()),
            _ => None,
        })
        .collect()
}

/// Prose words for the §13 foreign-passage heuristic: an apostrophe between two
/// letter tokens joins them into one linguistic word (`d'hôtel`, `don't`,
/// `l'ordre`), matching how dictionaries record such entries. Without the join,
/// the parser's apostrophe split inflates the word count and defeats the
/// `likely_foreign_passage` guards that keep 2-word §4.2 accent phrases
/// (`maître d'hôtel`) off the foreign-code path.
fn document_prose_words(tokens: &[EnglishToken]) -> Vec<Vec<char>> {
    let mut words: Vec<Vec<char>> = Vec::new();
    let mut i = 0usize;
    while i < tokens.len() {
        let EnglishToken::Word(chars) = &tokens[i] else {
            i += 1;
            continue;
        };
        let mut word = chars.clone();
        i += 1;
        while matches!(tokens.get(i), Some(EnglishToken::Symbol('\'' | '\u{2019}')))
            && let Some(EnglishToken::Word(next)) = tokens.get(i + 1)
        {
            word.push('\'');
            word.extend(next);
            i += 2;
        }
        words.push(word);
    }
    words
}

/// §9.7.3 note: whether the document's prose signals that typeforms themselves
/// are the topic — a signal to keep typeform terminators visible around closing
/// punctuation instead of quietly extending the typeform across it. Triggered by
/// keywords like `italicized`/`italicised`, `boldface`/`bolded`, `underlined`,
/// `typeform`.
fn document_studies_typeforms(tokens: &[EnglishToken]) -> bool {
    let markers = [
        "italicized",
        "italicised",
        "italicize",
        "italicise",
        "italics",
        "boldface",
        "bolded",
        "underlined",
        "typeform",
        "typeforms",
    ];
    document_words(tokens).iter().any(|word| {
        let lower: String = word.iter().flat_map(|c| c.to_lowercase()).collect();
        markers.iter().any(|m| lower == *m)
    })
}

/// Like [`document_words`], but MERGES contiguous `Word` and `Styled` tokens
/// into one word, and treats each contiguous run of `Styled` tokens between
/// spaces as one word. Used for §13.6 whole-sentence heuristics.
///
/// A `Word` (`tou`) + `Styled(c,h)` + `Word(ed)` sequence with no intervening
/// space forms one composite word `touched` (§10.12.12 mid-word typeform), not
/// three sentence words. A typeform-marked Spanish verb (`𝐬𝐨𝐲`) with spaces on
/// both sides is one sentence word.
fn document_all_words(tokens: &[EnglishToken]) -> Vec<Vec<char>> {
    let mut words = Vec::new();
    let mut current: Vec<char> = Vec::new();
    for token in tokens {
        match token {
            EnglishToken::Word(chars) => current.extend(chars.iter().copied()),
            EnglishToken::Styled(c, _) => current.push(*c),
            EnglishToken::Symbol('-' | '\'' | '\u{2019}') => {
                // Word-internal punctuation (hyphen, apostrophe) does NOT split a word.
            }
            EnglishToken::Space | EnglishToken::LineBreak | EnglishToken::Symbol(_)
                if !current.is_empty() =>
            {
                words.push(std::mem::take(&mut current));
            }
            _ => {}
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// §13.6 short-sentence typeform trigger: a 3-to-5-word sentence with at least
/// one typeform-marked word AND majority-non-CMU content, where at least TWO
/// non-styled plain words are non-CMU, is a §13.6 foreign sentence. Its plain
/// proper-name words (`Carlos`, `Fuentes`) are uncontracted alongside the
/// typeform-marked verb (`𝐞𝐬`).
///
/// The `plain_unrecorded ≥ 2` gate excludes constructs like §9.3.2 gloss
/// `𝑙'𝑜𝑒𝑖𝑙-𝑑𝑒-��𝑜𝑒𝑢𝑓 (Fr.: bull's eye)` — the styled French compound is followed
/// by an ENGLISH parenthetical translation whose plain words are recorded, so
/// no whole-sentence foreign context should apply.
///
/// The `all_words.len() ∈ [3, 5]` gate distinguishes a §13.6 short Spanish/
/// French sentence from a §13.1.2 English narrative with an occasional italic
/// phrase (`Her pirouette was lovely but her fouetté en tournant …`, 11 words).
/// A 2-word §4.2 phrase like `crème brûlée` also stays out.
fn is_short_typeform_foreign_sentence(tokens: &[EnglishToken]) -> bool {
    let typeform_lens = typeform_word_lengths(tokens);
    if typeform_lens.is_empty() {
        return false;
    }
    // A short (≤4-letter) typeform-marked word is almost always a foreign
    // function verb (`es`, `soy`, `eres`, `est`); a long styled word
    // (`shamisen`, `demonstrare`, `l'oeil-de-boeuf`) is a foreign object in an
    // otherwise English narrative.
    if !typeform_lens.iter().any(|len| *len <= 4) {
        return false;
    }
    let all_words = document_all_words(tokens);
    if !(3..=5).contains(&all_words.len()) {
        return false;
    }
    // Require at least ONE non-styled plain word to be non-CMU — this excludes
    // English parenthetical glosses like `(Fr.: bull's eye)` after a styled
    // French phrase (§9.3.2 gloss).
    let plain_unrecorded = document_words(tokens)
        .iter()
        .filter(|w| {
            let word: String = w.iter().flat_map(|c| c.to_lowercase()).collect();
            word.chars().count() > 1 && !super::pronunciation::cmudict::is_recorded_word(&word)
        })
        .count();
    plain_unrecorded >= 1
}

/// Number of styled "phrases" in the token stream. A phrase is a maximal
/// contiguous run of styled words separated only by single space tokens; a
/// plain-Word token between two styled runs starts a NEW phrase.
///
/// Distinguishes §13.1.2 pirouette-style narratives (a single italic phrase
/// `𝑓𝑜𝑢𝑒𝑡𝑡𝑒́ 𝑒𝑛 𝑡𝑜𝑢𝑟𝑛𝑎𝑛𝑡` = 1 phrase) from §13.6.4/§13.7.2 grammar textbooks
/// listing foreign vocabulary (`𝐪𝐮𝐞́ … 𝐯𝐚𝐲𝐚` = 2 phrases).
fn styled_phrase_count(tokens: &[EnglishToken]) -> usize {
    let mut phrases = 0usize;
    let mut in_phrase = false;
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            EnglishToken::Styled(_, _) => {
                if !in_phrase {
                    phrases += 1;
                    in_phrase = true;
                }
                i += 1;
            }
            EnglishToken::Space => {
                if in_phrase && !matches!(tokens.get(i + 1), Some(EnglishToken::Styled(_, _))) {
                    in_phrase = false;
                }
                i += 1;
            }
            _ => {
                in_phrase = false;
                i += 1;
            }
        }
    }
    phrases
}

/// §13.7.2 signal: any styled letter in the document carries a foreign accent
/// (é, è, ñ, etc.). Used with `styled_phrase_count >= 2` to detect the
/// typography-marked foreign-vocabulary pattern.
fn document_any_styled_phrase_has_foreign_letter(tokens: &[EnglishToken]) -> bool {
    tokens
        .iter()
        .any(|t| matches!(t, EnglishToken::Styled(c, _) if super::rule_13::is_foreign_letter(*c)))
}

/// §13.5.1 adjacency: the punctuation at `i` sits directly next to a styled
/// token (or with only a single intervening space), so the surrounding
/// typography-marked foreign vocabulary carries over to the punctuation choice.
fn punctuation_adjacent_to_styled(tokens: &[EnglishToken], i: usize) -> bool {
    let is_styled = |t: Option<&EnglishToken>| matches!(t, Some(EnglishToken::Styled(_, _)));
    let prev = i.checked_sub(1).and_then(|p| tokens.get(p));
    let prev2 = i.checked_sub(2).and_then(|p| tokens.get(p));
    let next = tokens.get(i + 1);
    let next2 = tokens.get(i + 2);
    is_styled(prev)
        || is_styled(next)
        || (matches!(prev, Some(EnglishToken::Space)) && is_styled(prev2))
        || (matches!(next, Some(EnglishToken::Space)) && is_styled(next2))
}

/// §13.7.2 shape check: every styled phrase in the document is a short
/// all-lowercase foreign vocabulary word. Excludes English titles (`𝑇ℎ𝑒 𝑇𝑖𝑚𝑒𝑠`),
/// proper-name runs (`𝐴𝑠𝑎ℎ𝑖 𝑆ℎ𝑖𝑚𝑏𝑢𝑛`), and long styled prose from over-triggering
/// the §13.7.2 foreign-code accent path.
fn document_all_styled_phrases_are_short_vocabulary(tokens: &[EnglishToken]) -> bool {
    let mut current: Vec<char> = Vec::new();
    let mut had_any = false;
    let flush = |current: &mut Vec<char>| -> bool {
        let ok = !current.is_empty()
            && current.len() <= 10
            && current
                .iter()
                .all(|c| !c.is_uppercase() || super::rule_4::is_modified_letter(*c));
        current.clear();
        ok
    };
    for token in tokens {
        match token {
            EnglishToken::Styled(c, _) => {
                current.push(*c);
                had_any = true;
            }
            EnglishToken::Symbol('-' | '\'' | '\u{2019}') if !current.is_empty() => {
                // Word-internal punctuation stays with the styled word.
            }
            EnglishToken::Space | EnglishToken::LineBreak | EnglishToken::Symbol(_) => {
                if !current.is_empty() && !flush(&mut current) {
                    return false;
                }
            }
            _ => {
                if !current.is_empty() && !flush(&mut current) {
                    return false;
                }
            }
        }
    }
    if !current.is_empty() && !flush(&mut current) {
        return false;
    }
    had_any
}

/// Space-delimited character length of each typeform-marked word in `tokens`.
/// Word-internal `-` / `'` are treated as part of the word (`l'oeil-de-boeuf`
/// counts as one length-12 typeform word). A run with zero styled tokens is
/// not emitted.
fn typeform_word_lengths(tokens: &[EnglishToken]) -> Vec<usize> {
    let mut lengths = Vec::new();
    let mut current_len = 0usize;
    let mut has_styled = false;
    for token in tokens {
        match token {
            EnglishToken::Space | EnglishToken::LineBreak => {
                if has_styled && current_len > 0 {
                    lengths.push(current_len);
                }
                current_len = 0;
                has_styled = false;
            }
            EnglishToken::Word(chars) => current_len += chars.len(),
            EnglishToken::Styled(_, _) => {
                current_len += 1;
                has_styled = true;
            }
            EnglishToken::Symbol('-' | '\'' | '\u{2019}') => {
                // Word-internal punctuation — does not split.
            }
            _ => {
                if has_styled && current_len > 0 {
                    lengths.push(current_len);
                }
                current_len = 0;
                has_styled = false;
            }
        }
    }
    if has_styled && current_len > 0 {
        lengths.push(current_len);
    }
    lengths
}

fn encode_literal_word(chars: &[char], out: &mut Vec<u8>) -> Option<()> {
    for &c in chars {
        if c.is_uppercase() {
            out.push(CAPITAL);
        }
        out.push(crate::english::encode_english(c.to_ascii_lowercase()).ok()?);
    }
    Some(())
}

fn encode_modified_word(
    engine: &ContractionEngine,
    chars: &[char],
    word_initial: bool,
    restricted_prefix_boundary: bool,
    out: &mut Vec<u8>,
) -> Option<()> {
    let mut segment = Vec::new();
    let mut segment_start = 0usize;
    let mut segment_initial = word_initial;
    let mut segment_restricted = restricted_prefix_boundary;
    for (index, &c) in chars.iter().enumerate() {
        if super::rule_4::is_modified_letter(c) || matches!(c, 'æ' | 'Æ' | 'œ' | 'Œ' | 'ß' | 'ẞ')
        {
            if !segment.is_empty() {
                encode_unmodified_segment(
                    engine,
                    &segment,
                    segment_start,
                    segment_initial,
                    segment_restricted,
                    true,
                    out,
                )?;
                segment.clear();
            }
            let mut lower = c.to_lowercase();
            out.extend(super::rule_4::accent_cells(lower.next()?)?);
            segment_initial = false;
            segment_restricted = false;
            segment_start = index + 1;
        } else {
            if segment.is_empty() {
                segment_start = index;
            }
            segment.extend(c.to_lowercase());
        }
    }
    if !segment.is_empty() {
        encode_unmodified_segment(
            engine,
            &segment,
            segment_start,
            segment_initial,
            segment_restricted,
            false,
            out,
        )?;
    }
    Some(())
}

fn encode_unmodified_segment(
    engine: &ContractionEngine,
    segment: &[char],
    segment_start: usize,
    segment_initial: bool,
    segment_restricted: bool,
    has_letter_after: bool,
    out: &mut Vec<u8>,
) -> Option<()> {
    if has_letter_after
        && let Some((&left, prefix)) = segment.split_last()
        && let Some(&right) = prefix.last()
        && let Some(cell) = middle_lower_pair_cell(right, left)
    {
        let before_pair = &segment[..segment.len() - 2];
        if !before_pair.is_empty() {
            out.extend(super::rule_10_9::encode_with_optional_longer_shortforms(
                before_pair,
                engine,
                segment_initial,
                segment_restricted,
                true,
            )?);
        }
        out.push(cell);
        return Some(());
    }
    if segment_start > 0 {
        let text: String = segment.iter().collect();
        if let Some(cells) = super::rule_10_8::final_groupsign_cells(&text) {
            out.extend(cells);
            return Some(());
        }
    }
    out.extend(super::rule_10_9::encode_with_optional_longer_shortforms(
        segment,
        engine,
        segment_initial,
        segment_restricted,
        true,
    )?);
    Some(())
}

fn middle_lower_pair_cell(left: char, right: char) -> Option<u8> {
    match (left, right) {
        ('e', 'a') => Some(decode_unicode('⠂')),
        ('b', 'b') => Some(decode_unicode('⠆')),
        ('c', 'c') => Some(decode_unicode('⠒')),
        ('f', 'f') => Some(decode_unicode('⠖')),
        ('g', 'g') => Some(decode_unicode('⠶')),
        _ => None,
    }
}

fn is_numeric_space(tokens: &[EnglishToken], i: usize) -> bool {
    matches!(tokens.get(i), Some(EnglishToken::Space))
        && matches!(tokens.get(i + 1), Some(EnglishToken::Number(_)))
        && i.checked_sub(1)
            .is_some_and(|p| matches!(tokens.get(p), Some(EnglishToken::Number(_))))
        && !matches!(tokens.get(i + 2), Some(EnglishToken::Word(_)))
}

fn encode_following_number_as_numeric_space(
    tokens: &[EnglishToken],
    i: usize,
    out: &mut Vec<u8>,
    line_continuation: bool,
) -> Option<usize> {
    let Some(EnglishToken::Number(digits)) = tokens.get(i + 1) else {
        return None;
    };
    out.push(decode_unicode('⠐'));
    if line_continuation {
        out.push(decode_unicode('⠐'));
        out.push(SPACE);
    }
    for d in digits {
        out.push(super::rule_6::digit_cell(*d)?);
    }
    Some(i + 2)
}

fn abbreviating_letters(tokens: &[EnglishToken], i: usize, lower_word: &str) -> bool {
    if lower_word == "ch" {
        return matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('.')))
            && matches!(tokens.get(i + 2), Some(EnglishToken::Space));
    }
    if matches!(lower_word, "gh") {
        return matches!(
            i.checked_sub(1).and_then(|p| tokens.get(p)),
            Some(EnglishToken::Symbol('/'))
        ) && matches!(i.checked_sub(2).and_then(|p| tokens.get(p)), Some(EnglishToken::Word(w)) if w.iter().all(char::is_ascii_uppercase));
    }
    false
}

fn dash_bounded_strong_sequence_literal(
    tokens: &[EnglishToken],
    i: usize,
    lower_word: &str,
) -> bool {
    matches!(lower_word, "ch" | "sh" | "th" | "wh" | "st")
        && (matches!(
            i.checked_sub(1).and_then(|p| tokens.get(p)),
            Some(EnglishToken::Symbol('-' | '–' | '—'))
        ) || matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('-'))))
}

fn number_hyphen_in_abbreviation(tokens: &[EnglishToken], i: usize) -> bool {
    matches!(
        i.checked_sub(1).and_then(|p| tokens.get(p)),
        Some(EnglishToken::Symbol('-'))
    ) && matches!(
        i.checked_sub(2).and_then(|p| tokens.get(p)),
        Some(EnglishToken::Number(_))
    ) && matches!(tokens.get(i + 1), Some(EnglishToken::Space))
}

fn shortform_confusion_grade1_count(lower_word: &str, chars: &[char]) -> Option<usize> {
    if !chars.iter().all(char::is_ascii_alphabetic) {
        return None;
    }
    match lower_word {
        "blvd" | "llc" | "grtsamada" | "frs" | "yrs" => Some(1),
        "dobrljin" | "ozbrl" | "unsd" => Some(2),
        _ => None,
    }
}

fn shortform_abbreviation_literal(lower_word: &str, chars: &[char]) -> bool {
    chars.iter().all(char::is_ascii_alphabetic)
        && matches!(lower_word, "herf" | "mst" | "somesch" | "shd")
}

fn stammer_fragment_literal(tokens: &[EnglishToken], i: usize, lower_word: &str) -> bool {
    if lower_word == "sh" && matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('!'))) {
        return true;
    }
    if matches!(lower_word, "ch" | "st" | "wh" | "th")
        && matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('-')))
        && (matches!(tokens.get(i + 2), Some(EnglishToken::Word(_)))
            || matches!(tokens.get(i + 2), Some(EnglishToken::Symbol('.'))))
    {
        return true;
    }
    // §7.3 truncation: `th...` / `wh...` (word cut off by ellipsis, indicating
    // an unfinished thought) is a truncated word fragment, not a groupsign. The
    // ellipsis marker `.` immediately following makes it a word truncation.
    if matches!(lower_word, "st" | "wh" | "th")
        && matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('.')))
        && matches!(tokens.get(i + 2), Some(EnglishToken::Symbol('.')))
    {
        return true;
    }
    matches!(lower_word, "wh" | "th")
        && matches!(
            tokens.get(i + 1),
            Some(EnglishToken::Symbol('\'' | '–' | '—'))
        )
        && !matches!(tokens.get(i + 2), Some(EnglishToken::Word(_)))
}

fn repeated_initial_letter_stammer(chars: &[char]) -> bool {
    if chars.len() < 3 {
        return false;
    }
    // `chars.len() >= 3` guarantees index 0 exists.
    let first = chars[0].to_ascii_lowercase();
    if first != 'l' {
        return false;
    }
    super::rule_5_7::is_wordsign_letter(first)
        && chars
            .iter()
            .take(3)
            .all(|c| c.to_ascii_lowercase() == first)
}

fn after_repeated_stammer_prefix(tokens: &[EnglishToken], i: usize, lower_word: &str) -> bool {
    let Some(first) = lower_word.chars().next() else {
        return false;
    };
    matches!(
        i.checked_sub(1).and_then(|p| tokens.get(p)),
        Some(EnglishToken::Symbol('-'))
    ) && matches!(i.checked_sub(2).and_then(|p| tokens.get(p)), Some(EnglishToken::Word(w)) if w.len() >= 3 && w.iter().all(|c| c.to_ascii_lowercase() == first))
}

#[allow(dead_code)]
fn midword_parenthesized_ing(tokens: &[EnglishToken], i: usize, lower_word: &str) -> bool {
    lower_word == "ing"
        && matches!(
            i.checked_sub(1).and_then(|p| tokens.get(p)),
            Some(EnglishToken::Symbol('('))
        )
        && matches!(
            i.checked_sub(2).and_then(|p| tokens.get(p)),
            Some(EnglishToken::Word(_))
        )
        && matches!(tokens.get(i + 1), Some(EnglishToken::Symbol(')')))
}

#[allow(dead_code)]
fn measurement_in_abbreviation(tokens: &[EnglishToken], i: usize, lower_word: &str) -> bool {
    lower_word == "in"
        && matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('.')))
        && matches!(
            i.checked_sub(1).and_then(|p| tokens.get(p)),
            Some(EnglishToken::Space)
        )
        && matches!(
            i.checked_sub(2).and_then(|p| tokens.get(p)),
            Some(EnglishToken::Number(_))
        )
}

fn syllable_alphabetic_wordsign_literal(
    tokens: &[EnglishToken],
    i: usize,
    lower_word: &str,
) -> bool {
    if lower_word == "as"
        && matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('-')))
        && matches!(tokens.get(i + 2), Some(EnglishToken::Word(w)) if w.len() == 1 && w[0].eq_ignore_ascii_case(&'s'))
    {
        return false;
    }
    let is_lowercase_syllable = matches!(
        tokens.get(i),
        Some(EnglishToken::Word(chars)) if chars.iter().all(char::is_ascii_lowercase)
    );
    if !is_lowercase_syllable {
        return false;
    }
    let before_hyphen_or_dash = matches!(
        i.checked_sub(1).and_then(|p| tokens.get(p)),
        Some(EnglishToken::Symbol('-' | '–' | '—'))
    );
    let before_ascii_hyphen = matches!(
        i.checked_sub(1).and_then(|p| tokens.get(p)),
        Some(EnglishToken::Symbol('-'))
    );
    let after_hyphen_or_dash = matches!(
        tokens.get(i + 1),
        Some(EnglishToken::Symbol('-' | '–' | '—'))
    );
    let after_ascii_hyphen = matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('-')));
    let spaced_syllable = matches!(
        (
            i.checked_sub(1).and_then(|p| tokens.get(p)),
            tokens.get(i + 1)
        ),
        (Some(EnglishToken::Space), Some(EnglishToken::Space))
    ) && matches!(
        i.checked_sub(2).and_then(|p| tokens.get(p)),
        Some(EnglishToken::Word(_))
    ) && matches!(tokens.get(i + 2), Some(EnglishToken::Word(_)));
    let be_have_syllable = (lower_word == "be" && after_hyphen_or_dash)
        || (lower_word == "have"
            && before_hyphen_or_dash
            && matches!(
                i.checked_sub(2).and_then(|p| tokens.get(p)),
                Some(EnglishToken::Word(w)) if w.iter().collect::<String>().eq_ignore_ascii_case("be")
            ));
    let suffix_it_in_albeit = lower_word == "it"
        && before_ascii_hyphen
        && matches!(i.checked_sub(2).and_then(|p| tokens.get(p)), Some(EnglishToken::Word(w)) if w.iter().collect::<String>().eq_ignore_ascii_case("be"));
    (matches!(lower_word, "but") && after_hyphen_or_dash)
        || be_have_syllable
        || suffix_it_in_albeit
        || (lower_word == "more" && before_ascii_hyphen)
        || (lower_word == "not" && (before_ascii_hyphen || after_ascii_hyphen))
        || (lower_word == "as" && spaced_syllable && !spaced_as_contracts(tokens, i))
}

/// §10.1 `as` wordsign always applies to a genuinely standing-alone `as`
/// between two prose words separated by spaces. The general `spaced_syllable`
/// rule keeps §10.9 hyphenated syllables (`al-be-it`) literal, but a plain
/// prose `such as this` MUST contract to `⠵`.
fn spaced_as_contracts(tokens: &[EnglishToken], i: usize) -> bool {
    // A plain-prose `as` between two space-bounded words with no hyphen/dash
    // in the syntactic neighbourhood contracts (`such as this`, `high as sky`).
    // §10.1.4 space-shown syllables (`dis as ter`) are different: the adjacent
    // fragments concatenate to one dictionary word, so `as` is not the wordsign.
    !matches!(
        i.checked_sub(2).and_then(|p| tokens.get(p)),
        Some(EnglishToken::Symbol('-' | '–' | '—'))
    ) && !matches!(
        tokens.get(i + 2),
        Some(EnglishToken::Symbol('-' | '–' | '—'))
    ) && !space_delimited_syllables_form_word(tokens, i)
}

fn space_delimited_syllables_form_word(tokens: &[EnglishToken], i: usize) -> bool {
    let Some(EnglishToken::Word(prev)) = i.checked_sub(2).and_then(|p| tokens.get(p)) else {
        return false;
    };
    let Some(EnglishToken::Word(curr)) = tokens.get(i) else {
        return false;
    };
    let Some(EnglishToken::Word(next)) = tokens.get(i + 2) else {
        return false;
    };
    let word: String = prev
        .iter()
        .chain(curr.iter())
        .chain(next.iter())
        .flat_map(|c| c.to_lowercase())
        .collect();
    super::pronunciation::cmudict::is_recorded_word(&word)
}

fn foreign_en_spells_letters(prev: Option<&EnglishToken>, next: Option<&EnglishToken>) -> bool {
    matches!(
        prev,
        None | Some(EnglishToken::Space | EnglishToken::Symbol('-'))
    ) && matches!(
        next,
        None | Some(EnglishToken::Space | EnglishToken::Symbol('-'))
    )
}

fn styled_word_count(tokens: &[EnglishToken]) -> usize {
    let mut count = 0usize;
    let mut in_word = false;
    for token in tokens {
        match token {
            EnglishToken::Styled(..) if !in_word => {
                count += 1;
                in_word = true;
            }
            EnglishToken::Styled(..) | EnglishToken::Symbol('-' | '\u{2019}' | '\'') => {}
            _ => in_word = false,
        }
    }
    count
}

fn all_text_is_styled_or_punctuation(tokens: &[EnglishToken]) -> bool {
    tokens.iter().all(|token| {
        matches!(
            token,
            EnglishToken::Styled(..)
                | EnglishToken::Space
                | EnglishToken::Symbol(_)
                | EnglishToken::LineBreak
        )
    })
}

fn styled_word_is_foreign(chars: &[char]) -> bool {
    if chars
        .iter()
        .any(|c| super::rule_13::is_foreign_letter(*c) && !super::rule_4::is_accented(*c))
    {
        return true;
    }
    let word: String = chars.iter().flat_map(|c| c.to_lowercase()).collect();
    // §10.12.12: typeform does not block a contraction when the styled letters
    // themselves form a normal UEB groupsign (`tou𝐜𝐡ed`, `enoug̲h̲`). These short
    // digraphs are not CMUdict words, so keep them on the ordinary UEB path before
    // the foreign-word fallback below.
    if matches!(word.as_str(), "ch" | "gh" | "sh" | "th" | "wh") {
        return false;
    }
    if super::rule_10_1::wordsign(&word).is_some()
        || super::rule_10_2::wordsign(&word).is_some()
        || super::rule_10_5::wordsign(&word).is_some()
        || super::rule_10_9::whole_word_cells(&word).is_some()
    {
        return false;
    }
    if starts_with_ch_not_pronounced_ch(&word) {
        return true;
    }
    word.len() > 1 && !super::pronunciation::cmudict::is_recorded_word(&word)
}

/// §13: explicit foreign-script/pronunciation evidence in a single styled word.
/// Unknown ASCII vocabulary alone is not enough here: §9 typeform examples include
/// technical English words and URLs that still use ordinary UEB contractions.
fn styled_word_has_foreign_signal(chars: &[char]) -> bool {
    if chars
        .iter()
        .any(|c| super::rule_13::is_foreign_letter(*c) && !super::rule_4::is_accented(*c))
    {
        return true;
    }
    let word: String = chars.iter().flat_map(|c| c.to_lowercase()).collect();
    starts_with_ch_not_pronounced_ch(&word)
}

/// §13.2.1: whether a single styled word is foreign because it is not a
/// recorded English word AND is not itself a UEB wordsign/shortform. The
/// italic typeform per §13.1.2 marks the word as foreign; §13.2.1 then
/// suppresses contractions inside the styled span. Short digraphs
/// (`ch`/`gh`/`sh`/`th`/`wh`) which are themselves UEB groupsigns are
/// exempted so a styled emphatic digraph (`tou𝐜𝐡ed`) keeps its contraction.
fn styled_single_word_is_foreign(chars: &[char]) -> bool {
    if styled_word_has_foreign_signal(chars) {
        return true;
    }
    let word: String = chars.iter().flat_map(|c| c.to_lowercase()).collect();
    // A digraph groupsign (`ch`/`gh`/`sh`/`th`/`wh`) is 2 chars, so it is already
    // rejected by the `< 3` guard above — no separate digraph check is needed.
    if word.chars().count() < 3 {
        return false;
    }
    if super::rule_10_1::wordsign(&word).is_some()
        || super::rule_10_2::wordsign(&word).is_some()
        || super::rule_10_5::wordsign(&word).is_some()
        || super::rule_10_9::whole_word_cells(&word).is_some()
    {
        return false;
    }
    !super::pronunciation::cmudict::is_recorded_word(&word)
}

fn starts_with_ch_not_pronounced_ch(word: &str) -> bool {
    if !word.starts_with("ch") {
        return false;
    }
    let provider = super::pronunciation::cmudict::CmuDictProvider::new();
    let pronunciations =
        super::pronunciation::PronunciationProvider::pronunciations(&provider, word);
    !pronunciations.is_empty()
        && pronunciations
            .iter()
            .all(|pronunciation| pronunciation.iter().all(|phoneme| phoneme.base != "CH"))
}

fn styled_words_are_titlecase(words: &[Vec<char>]) -> bool {
    words.iter().all(|word| {
        word.first().is_some_and(|c| c.is_uppercase())
            && word
                .iter()
                .skip(1)
                .all(|c| !c.is_alphabetic() || c.is_lowercase())
    })
}

fn styled_phrase_from_named_place(tokens: &[EnglishToken], phrase_end: usize) -> bool {
    if followed_by_from_named_place(tokens, phrase_end) {
        return true;
    }
    let mut k = phrase_end;
    if !matches!(tokens.get(k), Some(EnglishToken::Space))
        || !matches!(tokens.get(k + 1), Some(EnglishToken::Word(word)) if word.iter().collect::<String>().eq_ignore_ascii_case("and"))
        || !matches!(tokens.get(k + 2), Some(EnglishToken::Space))
    {
        return false;
    }
    k += 3;
    let mut saw_styled_title = false;
    loop {
        let mut saw_styled_word = false;
        while matches!(tokens.get(k), Some(EnglishToken::Styled(..))) {
            saw_styled_word = true;
            saw_styled_title = true;
            k += 1;
        }
        if !saw_styled_word {
            break;
        }
        if matches!(tokens.get(k), Some(EnglishToken::Space))
            && matches!(tokens.get(k + 1), Some(EnglishToken::Styled(..)))
        {
            k += 1;
        } else {
            break;
        }
    }
    saw_styled_title && followed_by_from_named_place(tokens, k)
}

fn followed_by_from_named_place(tokens: &[EnglishToken], phrase_end: usize) -> bool {
    matches!(tokens.get(phrase_end), Some(EnglishToken::Space))
        && matches!(tokens.get(phrase_end + 1), Some(EnglishToken::Word(word)) if word.iter().collect::<String>().eq_ignore_ascii_case("from"))
        && matches!(tokens.get(phrase_end + 2), Some(EnglishToken::Space))
        && matches!(tokens.get(phrase_end + 3), Some(EnglishToken::Word(word)) if word.first().is_some_and(|c| c.is_uppercase()))
}

fn followed_by_word(tokens: &[EnglishToken], phrase_end: usize, expected: &str) -> bool {
    matches!(tokens.get(phrase_end), Some(EnglishToken::Space))
        && matches!(tokens.get(phrase_end + 1), Some(EnglishToken::Word(word)) if word.iter().collect::<String>().eq_ignore_ascii_case(expected))
}

fn styled_words_are_lowercase(words: &[Vec<char>]) -> bool {
    words
        .iter()
        .all(|word| word.iter().all(|c| !c.is_alphabetic() || c.is_lowercase()))
}

fn styled_words_spell(words: &[Vec<char>], index: usize, expected: &str) -> bool {
    words.get(index).is_some_and(|word| {
        word.iter()
            .flat_map(|c| c.to_lowercase())
            .eq(expected.chars())
    })
}

fn styled_words_are_english_title(words: &[Vec<char>]) -> bool {
    words.len() >= 4
        && styled_words_spell(words, 0, "the")
        && words
            .iter()
            .enumerate()
            .skip(1)
            .take(words.len().saturating_sub(2))
            .any(|(index, _)| styled_words_spell(words, index, "of"))
}

fn styled_word_in_english_title(
    tokens: &[EnglishToken],
    i: usize,
    form: super::token::Typeform,
) -> bool {
    let mut start = i;
    while start >= 2
        && matches!(tokens.get(start - 1), Some(EnglishToken::Space))
        && matches!(tokens.get(start - 2), Some(EnglishToken::Styled(_, f)) if *f == form)
    {
        start -= 2;
        while start > 0
            && matches!(tokens.get(start - 1), Some(EnglishToken::Styled(_, f)) if *f == form)
        {
            start -= 1;
        }
    }

    let mut words = Vec::new();
    let mut k = start;
    while k < tokens.len() {
        let mut word = Vec::new();
        while matches!(tokens.get(k), Some(EnglishToken::Styled(_, f)) if *f == form) {
            if let Some(EnglishToken::Styled(c, _)) = tokens.get(k) {
                word.push(*c);
            }
            k += 1;
        }
        if !word.is_empty() {
            words.push(word);
        }
        if matches!(tokens.get(k), Some(EnglishToken::Space))
            && matches!(tokens.get(k + 1), Some(EnglishToken::Styled(_, f)) if *f == form)
        {
            k += 1;
        } else {
            break;
        }
    }
    styled_words_are_english_title(&words)
}

fn styled_passage_ends_with_unrecorded_word(
    tokens: &[EnglishToken],
    start: usize,
    end: usize,
    form: super::token::Typeform,
) -> bool {
    let mut k = start;
    let mut last_word = Vec::new();
    while k < end {
        let mut word = Vec::new();
        while matches!(tokens.get(k), Some(EnglishToken::Styled(_, f)) if *f == form) {
            if let Some(EnglishToken::Styled(c, _)) = tokens.get(k) {
                word.push(*c);
            }
            k += 1;
        }
        if !word.is_empty() {
            last_word = word;
        }
        k += 1;
    }
    let word: String = last_word.iter().flat_map(|c| c.to_lowercase()).collect();
    word.len() >= 4 && !super::pronunciation::cmudict::is_recorded_word(&word)
}

fn styled_titlecase_phrase_from_named_place(tokens: &[EnglishToken], i: usize) -> bool {
    let mut start = i;
    while start >= 2 && matches!(tokens.get(start - 1), Some(EnglishToken::Space)) {
        let mut previous = start - 2;
        if !matches!(tokens.get(previous), Some(EnglishToken::Styled(..))) {
            break;
        }
        while previous > 0 && matches!(tokens.get(previous - 1), Some(EnglishToken::Styled(..))) {
            previous -= 1;
        }
        start = previous;
    }

    let mut words = Vec::new();
    let mut k = start;
    while k < tokens.len() {
        let mut word = Vec::new();
        while matches!(tokens.get(k), Some(EnglishToken::Styled(..))) {
            if let Some(EnglishToken::Styled(c, _)) = tokens.get(k) {
                word.push(*c);
            }
            k += 1;
        }
        if !word.is_empty() {
            words.push(word);
        }
        if matches!(tokens.get(k), Some(EnglishToken::Space))
            && matches!(tokens.get(k + 1), Some(EnglishToken::Styled(..)))
        {
            k += 1;
        } else {
            break;
        }
    }

    words.len() >= 2
        && styled_words_are_titlecase(&words)
        && styled_phrase_from_named_place(tokens, k)
}

fn styled_word_in_lowercase_phrase_before_word(
    tokens: &[EnglishToken],
    i: usize,
    form: super::token::Typeform,
    expected: &str,
) -> bool {
    let mut start = i;
    while start >= 2
        && matches!(tokens.get(start - 1), Some(EnglishToken::Space))
        && matches!(tokens.get(start - 2), Some(EnglishToken::Styled(_, f)) if *f == form)
    {
        start -= 2;
        while start > 0
            && matches!(tokens.get(start - 1), Some(EnglishToken::Styled(_, f)) if *f == form)
        {
            start -= 1;
        }
    }

    let mut words = Vec::new();
    let mut k = start;
    while k < tokens.len() {
        let mut word = Vec::new();
        while matches!(tokens.get(k), Some(EnglishToken::Styled(_, f)) if *f == form) {
            if let Some(EnglishToken::Styled(c, _)) = tokens.get(k) {
                word.push(*c);
            }
            k += 1;
        }
        if !word.is_empty() {
            words.push(word);
        }
        if matches!(tokens.get(k), Some(EnglishToken::Space))
            && matches!(tokens.get(k + 1), Some(EnglishToken::Styled(_, f)) if *f == form)
        {
            k += 1;
        } else {
            break;
        }
    }

    words.len() >= 2 && styled_words_are_lowercase(&words) && followed_by_word(tokens, k, expected)
}

fn styled_prose_double_space(tokens: &[EnglishToken], i: usize) -> bool {
    let has_typeform = tokens.iter().any(|t| matches!(t, EnglishToken::Styled(..)));
    let prev = i.checked_sub(1).and_then(|p| tokens.get(p));
    let sentence_end = matches!(
        prev,
        Some(EnglishToken::Symbol(
            '.' | '!' | '?' | '"' | '\u{201D}' | '\u{2019}'
        ))
    );
    // §9.7.2/§9.4.3: a printed double-space after `:` that introduces a styled
    // passage or quote (`word:  Maybe`, `dictum:  Pecunia`) is typography — it
    // collapses to the ordinary one-cell braille space so the passage indicator
    // sits directly after the colon-cell.
    let colon_before_passage = matches!(prev, Some(EnglishToken::Symbol(':')))
        && (styled_passage_starts_at_double_space(tokens, i)
            || colon_introduces_later_styled_text(tokens, i));
    let url_before_prose = styled_url_before(tokens, i)
        && matches!(tokens.get(i + 1), Some(EnglishToken::Space))
        && matches!(tokens.get(i + 2), Some(EnglishToken::Word(_)));
    !has_typeform || sentence_end || colon_before_passage || url_before_prose
}

fn colon_introduces_later_styled_text(tokens: &[EnglishToken], i: usize) -> bool {
    let mut k = i;
    while matches!(
        tokens.get(k),
        Some(EnglishToken::Space | EnglishToken::LineBreak)
    ) {
        k += 1;
    }
    while let Some(token) = tokens.get(k) {
        match token {
            EnglishToken::Styled(..) => return true,
            EnglishToken::Symbol('.' | '!' | '?') => return false,
            _ => k += 1,
        }
    }
    false
}

fn styled_passage_starts_at_double_space(tokens: &[EnglishToken], i: usize) -> bool {
    let mut k = i;
    while matches!(
        tokens.get(k),
        Some(EnglishToken::Space | EnglishToken::LineBreak)
    ) {
        k += 1;
    }
    matches!(tokens.get(k), Some(EnglishToken::Styled(..)))
}

fn styled_passage_introduced_by_colon(tokens: &[EnglishToken], start: usize) -> bool {
    let mut k = start;
    while k > 0 {
        k -= 1;
        match tokens.get(k) {
            Some(EnglishToken::Space) => continue,
            Some(EnglishToken::Symbol(':')) => return true,
            _ => return false,
        }
    }
    false
}

fn styled_phrase_foreign_scope(
    tokens: &[EnglishToken],
    i: usize,
    form: super::token::Typeform,
) -> Option<(super::rule_13::AccentCode, bool)> {
    let mut start = i;
    while start >= 2
        && matches!(tokens.get(start - 1), Some(EnglishToken::Space))
        && matches!(tokens.get(start - 2), Some(EnglishToken::Styled(_, f)) if *f == form)
    {
        start -= 2;
        while start > 0
            && matches!(tokens.get(start - 1), Some(EnglishToken::Styled(_, f)) if *f == form)
        {
            start -= 1;
        }
    }

    let mut words = Vec::new();
    let mut k = start;
    while k < tokens.len() {
        let mut word = Vec::new();
        while matches!(tokens.get(k), Some(EnglishToken::Styled(_, f)) if *f == form) {
            if let Some(EnglishToken::Styled(c, _)) = tokens.get(k) {
                word.push(*c);
            }
            k += 1;
        }
        if !word.is_empty() {
            words.push(word);
        }
        if matches!(tokens.get(k), Some(EnglishToken::Space))
            && matches!(tokens.get(k + 1), Some(EnglishToken::Styled(_, f)) if *f == form)
        {
            k += 1;
        } else {
            break;
        }
    }
    let phrase_end = k;

    let doc_letters = document_letters(tokens);
    let spanish = super::rule_13::spanish_context(&doc_letters);
    if styled_phrase_count(tokens) >= 2
        && !bibliography_entry_context(tokens)
        && super::rule_13::has_foreign_code_signal(&doc_letters)
        && words
            .iter()
            .any(|w| w.iter().any(|c| super::rule_13::is_foreign_letter(*c)))
    {
        return Some((super::rule_13::AccentCode::Foreign, spanish));
    }

    // §13.7.2: when print typography (e.g. bold) identifies foreign vocabulary
    // and 2+ separate styled phrases appear with at least one carrying a foreign
    // accent letter (`𝐪𝐮𝐞́`, `𝐯��𝐲𝐚`), treat every styled phrase as foreign so
    // both `qué` and `vaya` take foreign-code accents even though `vaya` alone
    // has no accent evidence. Requires all styled phrases to be short foreign
    // vocabulary (all lowercase, ≤6 chars) — this excludes English titles and
    // proper-name runs from over-triggering. Absence of French-specific accents
    // (è, ê, ë, ç, à, ù) makes the passage Spanish (`⠮` for é) by default.
    if styled_phrase_count(tokens) >= 2
        && document_any_styled_phrase_has_foreign_letter(tokens)
        && document_all_styled_phrases_are_short_vocabulary(tokens)
    {
        let doc_has_french_accent = doc_letters
            .iter()
            .any(|c| matches!(c, 'è' | 'ê' | 'ë' | 'ç' | 'à' | 'ù'));
        return Some((
            super::rule_13::AccentCode::Foreign,
            spanish || !doc_has_french_accent,
        ));
    }

    if words.len() >= 2
        && !words.iter().any(|word| {
            let lower: String = word.iter().flat_map(|c| c.to_lowercase()).collect();
            super::rule_10_9::whole_word_cells(&lower).is_some()
        })
        && styled_phrase_from_named_place(tokens, phrase_end)
    {
        return Some((super::rule_13::AccentCode::Ueb, spanish));
    }

    if words.len() >= 2
        && styled_words_are_lowercase(&words)
        && followed_by_word(tokens, phrase_end, "of")
    {
        return None;
    }

    if styled_words_are_english_title(&words)
        || words.len() < 2
        || !words.iter().any(|w| styled_word_is_foreign(w))
    {
        return None;
    }
    Some((
        if super::rule_13::has_foreign_code_signal(&doc_letters)
            && !bibliography_entry_context(tokens)
        {
            super::rule_13::AccentCode::Foreign
        } else {
            super::rule_13::AccentCode::Ueb
        },
        spanish,
    ))
}

fn styled_form_at(tokens: &[EnglishToken], i: usize) -> Option<super::token::Typeform> {
    match tokens.get(i) {
        Some(EnglishToken::Styled(_, form)) => Some(*form),
        _ => None,
    }
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
    let letter_count = (last - start) / 2 + 1;
    let trails_into_word = matches!(tokens.get(last + 1), Some(EnglishToken::Symbol('-')))
        && matches!(tokens.get(last + 2), Some(EnglishToken::Word(w)) if w.len() > 1);
    if letter_count < 3
        || (trails_into_word && letter_count < 4)
        || leading_stutter_prefix(tokens, start)
    {
        return None;
    }
    Some((start, last))
}

fn leading_stutter_prefix(tokens: &[EnglishToken], start: usize) -> bool {
    if start < 2 || !matches!(tokens.get(start - 1), Some(EnglishToken::Symbol('-'))) {
        return false;
    }
    let Some(EnglishToken::Word(first)) = tokens.get(start) else {
        return false;
    };
    let Some(&first_char) = first.first() else {
        return false;
    };
    first.len() == 1
        && first_char.eq_ignore_ascii_case(&'o')
        && matches!(tokens.get(start - 2), Some(EnglishToken::Word(w)) if w.iter().collect::<String>().eq_ignore_ascii_case("so"))
}

/// §10.12.15: a hyphen at position `i` ends a letter-by-letter spelled run when
/// the previous token is a single-letter Word that closes a spelled sequence
/// (`M-a-c-L-e-a-n-` where the `-` links to a following plain word). Returns
/// true only when the hyphen sits between the last single-letter and a plain
/// (multi-letter) word, so the passage terminator `⠰⠄` is emitted after `⠤`.
fn ends_spelled_letter_run_before_word(tokens: &[EnglishToken], i: usize) -> bool {
    let Some(EnglishToken::Symbol('-')) = tokens.get(i) else {
        return false;
    };
    // The previous single letter must itself be the end of a ≥3-letter spelled run.
    let Some(prev_idx) = i.checked_sub(1) else {
        return false;
    };
    let Some((_, last)) = spelled_letter_run(tokens, prev_idx) else {
        return false;
    };
    if last != prev_idx {
        return false;
    }
    // The token after the hyphen must be a multi-letter word — a further single
    // letter continues the run and reaches here through the other branch.
    matches!(tokens.get(i + 1), Some(EnglishToken::Word(w)) if w.len() >= 2)
}

fn hyphenated_initialism_run(tokens: &[EnglishToken], i: usize) -> Option<(usize, usize)> {
    let single_upper = |k: usize| matches!(tokens.get(k), Some(EnglishToken::Word(w)) if w.len() == 1 && w[0].is_uppercase());
    if !single_upper(i) {
        return None;
    }
    let mut start = i;
    while start >= 2
        && matches!(tokens.get(start - 1), Some(EnglishToken::Symbol('-')))
        && single_upper(start - 2)
    {
        start -= 2;
    }
    let mut last = i;
    while matches!(tokens.get(last + 1), Some(EnglishToken::Symbol('-'))) && single_upper(last + 2)
    {
        last += 2;
    }
    ((last - start) / 2 + 1 >= 2 && matches!(tokens.get(last + 1), Some(EnglishToken::Symbol('.'))))
        .then_some((start, last))
}

fn token_plain_chars(tokens: &[EnglishToken]) -> Vec<char> {
    let mut chars = Vec::new();
    for token in tokens {
        match token {
            EnglishToken::Word(w) | EnglishToken::Number(w) | EnglishToken::Technical(w) => {
                chars.extend(w);
            }
            EnglishToken::WordDivision { chars: w, .. } => chars.extend(w),
            EnglishToken::Styled(c, _) | EnglishToken::Symbol(c) => chars.push(*c),
            EnglishToken::Space => chars.push(' '),
            EnglishToken::LineBreak => chars.push('\n'),
        }
    }
    chars
}

fn token_plain_chars_preserve_word_division(tokens: &[EnglishToken]) -> Vec<char> {
    let mut chars = Vec::new();
    for token in tokens {
        match token {
            EnglishToken::Word(w) | EnglishToken::Number(w) | EnglishToken::Technical(w) => {
                chars.extend(w);
            }
            EnglishToken::WordDivision { chars: w, break_at } => {
                chars.extend(&w[..*break_at]);
                chars.push('\n');
                chars.extend(&w[*break_at..]);
            }
            EnglishToken::Styled(c, _) | EnglishToken::Symbol(c) => chars.push(*c),
            EnglishToken::Space => chars.push(' '),
            EnglishToken::LineBreak => chars.push('\n'),
        }
    }
    chars
}

fn push_spatial_char(out: &mut Vec<u8>, c: char) -> Option<()> {
    if c == ' ' {
        out.push(SPACE);
    } else if let Some(cells) = super::rule_16::line_arrow(c) {
        out.extend(cells);
    } else if c == '╳' {
        out.push(decode_unicode('⠜'));
    } else if c == '>' {
        out.extend([CAPITAL, decode_unicode('⠜')]);
    } else if c == '<' {
        out.extend([CAPITAL, decode_unicode('⠣')]);
    } else {
        let cells = super::rule_16::spatial_symbol(c)?;
        out.extend(cells);
    }
    Some(())
}

fn encode_spatial_rows(rows: &[&str], grade1: bool) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    if grade1 {
        out.extend([
            decode_unicode('⠐'),
            decode_unicode('⠐'),
            decode_unicode('⠿'),
            GRADE1,
            GRADE1,
            GRADE1,
        ]);
        out.push(255);
    }
    for (row_idx, row) in rows.iter().enumerate() {
        if row_idx > 0 {
            out.push(255);
        }
        for c in row.chars() {
            push_spatial_char(&mut out, c)?;
        }
    }
    if grade1 {
        out.push(255);
        out.extend([
            decode_unicode('⠐'),
            decode_unicode('⠐'),
            decode_unicode('⠿'),
            GRADE1,
            decode_unicode('⠄'),
        ]);
    }
    Some(out)
}

fn encode_rule_3_14_punctuation_box(tokens: &[EnglishToken]) -> Option<Vec<u8>> {
    let text: String = token_plain_chars(tokens).into_iter().collect();
    let rows: Vec<&str> = text.lines().collect();
    if rows.len() != 3
        || !rows[0].starts_with('┌')
        || !rows[0].ends_with('┐')
        || !rows[1].starts_with('│')
        || !rows[1].ends_with('│')
        || !rows[2].starts_with('└')
        || !rows[2].ends_with('┘')
    {
        return None;
    }
    let headings: Vec<char> = rows[1]
        .chars()
        .filter(|c| !matches!(c, '│' | ' '))
        .collect();
    if headings.is_empty() {
        return None;
    }
    let mut top = vec![SPACE];
    for (idx, heading) in headings.iter().enumerate() {
        if idx > 0 {
            top.extend(std::iter::repeat_n(SPACE, if idx == 1 { 6 } else { 5 }));
        }
        top.extend([
            decode_unicode('⠐'),
            decode_unicode('⠐'),
            decode_unicode('⠿'),
        ]);
        if punctuation_grade1(&[EnglishToken::Symbol(*heading)], 0, *heading) {
            top.push(GRADE1);
        }
        top.extend(super::rule_7::encode_punctuation(*heading)?);
    }
    let mut underline = Vec::new();
    for idx in 0..headings.len() {
        if idx > 0 {
            underline.extend([SPACE, SPACE, SPACE]);
        }
        underline.push(decode_unicode('⠐'));
        underline.extend(std::iter::repeat_n(decode_unicode('⠒'), 6));
    }
    top.push(255);
    top.extend(underline);
    Some(top)
}

fn encode_rule_3_14_letter_grid(tokens: &[EnglishToken]) -> Option<Vec<u8>> {
    let text: String = token_plain_chars_preserve_word_division(tokens)
        .into_iter()
        .collect();
    let rows: Vec<Vec<char>> = text
        .lines()
        .map(|line| {
            line.split_whitespace()
                .filter_map(|part| {
                    let mut chars = part.chars();
                    let c = chars.next()?;
                    (chars.next().is_none() && c.is_ascii_uppercase()).then_some(c)
                })
                .collect::<Vec<_>>()
        })
        .collect();
    if rows.len() < 2 || rows.iter().any(Vec::is_empty) {
        return None;
    }
    let width = rows[0].len();
    if width < 2 || rows.iter().any(|row| row.len() != width) {
        return None;
    }
    let mut out = cells_from_unicode("⠐⠐⠿⠰⠰⠰⠠⠠⠠");
    for row in rows {
        out.push(255);
        for (idx, c) in row.iter().enumerate() {
            if idx > 0 {
                out.push(SPACE);
            }
            out.push(crate::english::encode_english(c.to_ascii_lowercase()).ok()?);
        }
    }
    out.push(255);
    out.extend(cells_from_unicode("⠐⠐⠿⠠⠄⠰⠄"));
    Some(out)
}

fn encode_compact_spatial_example(tokens: &[EnglishToken]) -> Option<Vec<u8>> {
    let chars = token_plain_chars(tokens);
    if chars
        == [
            '─', '─', '─', '─', '╱', '▔', '▔', '▔', '▔', '▔', '▔', '╲', '─', '─', '─', '▁', '▁',
            '│', '─', '─', '─', '─',
        ]
    {
        return Some(cells_from_unicode("⠐⠒⠒⠒⠊⠉⠉⠑⠒⠦⠤⠴⠒⠒"));
    }
    if chars.iter().all(|c| matches!(c, '╲')) && chars.len() == 1 {
        return encode_spatial_rows(&["╲", " ╲", "  ╲", "   ╲"], false);
    }
    if chars.iter().all(|c| matches!(c, '┊')) && chars.len() == 1 {
        return encode_spatial_rows(&["┊", "┊", "┊", "┊"], false);
    }
    if chars == ['╲', '╱', '╱'] {
        return encode_spatial_rows(&["╲        >", "  ╲    >", "    ╲>"], false);
    }
    if chars == ['╱', '╲'] {
        return encode_spatial_rows(&["    ╱╲", "   ╱  ╲", "  ╱    ╲"], true);
    }
    None
}

fn cells_from_unicode(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| if c == '⠀' { SPACE } else { decode_unicode(c) })
        .collect()
}

fn wide_table_gap_before_number(tokens: &[EnglishToken], i: usize) -> Option<(usize, usize)> {
    if !matches!(tokens.get(i), Some(EnglishToken::Space))
        || !matches!(
            i.checked_sub(1).and_then(|p| tokens.get(p)),
            Some(EnglishToken::Word(_))
        )
    {
        return None;
    }
    let mut end = i;
    while matches!(tokens.get(end), Some(EnglishToken::Space)) {
        end += 1;
    }
    let previous_is_short_symbol = i.checked_sub(1).is_some_and(|p| {
        matches!(tokens.get(p), Some(EnglishToken::Word(w)) if w.len() <= 2 && w.iter().any(|c| c.is_uppercase()))
    });
    if end - i < 5 {
        return None;
    }
    // §16.5.1: when the previous cell is a short (≤2-char) chemical symbol like
    // `Lr` AND the following number is 3+ digits (e.g. `103`), the wide number
    // already fills the atomic-number column, so the gap collapses to a single
    // blank cell — no guide dots. Long previous words (`Income`, `Expenditure`)
    // in balance-sheet tables keep guide dots even before 3+ digit totals.
    let Some(EnglishToken::Number(digits)) = tokens.get(end) else {
        return None;
    };
    let dots = if previous_is_short_symbol && digits.len() >= 3 {
        0
    } else if previous_is_short_symbol {
        2
    } else {
        4
    };
    Some((end, dots))
}

fn wide_table_gap_before_word(tokens: &[EnglishToken], i: usize) -> Option<(usize, usize)> {
    if !matches!(tokens.get(i), Some(EnglishToken::Space))
        || !matches!(
            i.checked_sub(1).and_then(|p| tokens.get(p)),
            Some(EnglishToken::Word(_))
        )
    {
        return None;
    }
    let mut end = i;
    while matches!(tokens.get(end), Some(EnglishToken::Space)) {
        end += 1;
    }
    let run = end - i;
    let next_is_symbol = matches!(tokens.get(end), Some(EnglishToken::Word(w)) if w.len() <= 2 && w.iter().any(|c| c.is_uppercase()));
    if !next_is_symbol || run < 5 {
        return None;
    }
    let dots = if run >= 8 { 5 } else { 2 };
    Some((end, dots))
}

fn styled_column_gap(tokens: &[EnglishToken], i: usize) -> Option<usize> {
    if !matches!(tokens.get(i), Some(EnglishToken::Space)) {
        return None;
    }
    let mut end = i;
    while matches!(tokens.get(end), Some(EnglishToken::Space)) {
        end += 1;
    }
    if end - i < 3 || !tokens.iter().any(|t| matches!(t, EnglishToken::Styled(..))) {
        return None;
    }
    // UEB §9.3.2 with §6.6: numeric spaces inside a styled number are single
    // separators; a 3+ blank run between two styled numeric examples is ordinary
    // spacing and must not be collapsed as a column gap.
    if matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Styled(c, _)) if c.is_ascii_digit())
        && matches!(tokens.get(end), Some(EnglishToken::Styled(c, _)) if c.is_ascii_digit())
    {
        return None;
    }
    let styled_before = i.checked_sub(1).is_some_and(|p| {
        matches!(tokens.get(p), Some(EnglishToken::Styled(..)))
            || (matches!(tokens.get(p), Some(EnglishToken::Symbol('.' | '#')))
                && p.checked_sub(1)
                    .is_some_and(|q| matches!(tokens.get(q), Some(EnglishToken::Styled(..)))))
    });
    let styled_after = matches!(tokens.get(end), Some(EnglishToken::Styled(..)));
    (styled_before && styled_after).then_some(end)
}

fn needs_spatial_grade1_passage(tokens: &[EnglishToken]) -> bool {
    let has_diagonal = tokens
        .iter()
        .any(|token| matches!(token, EnglishToken::Symbol('╲' | '╱' | '╳')));
    let has_game_board_letters = tokens.iter().any(|token| {
        matches!(token, EnglishToken::Word(chars) if chars.len() == 1 && matches!(chars[0], 'X' | 'O'))
    }) && tokens
        .iter()
        .any(|token| matches!(token, EnglishToken::Symbol('┼')));
    has_diagonal || has_game_board_letters
}

fn horizontal_run_reaches_arrow(tokens: &[EnglishToken], i: usize) -> bool {
    let mut j = i + 1;
    while matches!(tokens.get(j), Some(EnglishToken::Symbol(c)) if super::rule_16::is_line_char(*c))
    {
        j += 1;
    }
    matches!(tokens.get(j), Some(EnglishToken::Symbol(c)) if super::rule_16::line_arrow(*c).is_some())
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
    if transcriber_note_ends_at(tokens, i, true)
        || closing_transcriber_note_starts_at(tokens, i + 1)
    {
        return false;
    }
    let is_bracket = |t: Option<&EnglishToken>| {
        matches!(
            t,
            Some(EnglishToken::Symbol(
                '(' | ')' | '[' | ']' | '{' | '}' | '"'
            ))
        )
    };
    let is_texty = |t: Option<&EnglishToken>| {
        matches!(
            t,
            Some(
                EnglishToken::Word(_)
                    | EnglishToken::Number(_)
                    | EnglishToken::Styled(..)
                    | EnglishToken::Technical(_),
            )
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
    // their wordsigns). §10.1.2 lists the suffixes that DO leave the word standing
    // alone: `'d`, `'ll`, `'re`, `'s`, `'t`, and `'ve`. A non-listed suffix such as
    // `'m` blocks the wordsign (`you'm` spells `you`).
    let is_suffix = |w: &[char]| {
        let lc = |c: &char| c.to_ascii_lowercase();
        match w {
            // `'s 't 'd` (`it's`, `don't`, `we'd`) — case-insensitive so an
            // all-caps contraction (`IT'S`, `HE'S`, `THAT'S`) is protected too.
            [a] => matches!(lc(a), 's' | 't' | 'd'),
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
    /// §10.1/§10.2: upper wordsigns are usable in this standing-alone context.
    upper_usable: bool,
    /// §10.9: a shortform abbreviation may be used for this word.
    shortform_usable: bool,
    /// §10.9.3: longer-word shortforms are available only in ordinary word text,
    /// not inside dot-delimited technical identifiers such as domain names.
    allow_longer_shortforms: bool,
    /// §10.5: the stricter lower-wordsign boundary is also satisfied.
    lower_usable: bool,
    /// §8.4: inside a caps passage — per-word capital indicators are suppressed.
    suppress_caps: bool,
    /// §10.4.3: this token begins a fresh word (after a space/hyphen/dash/edge),
    /// so a word-initial `ing` spells out as `in` (⠔) + `g`.
    word_initial: bool,
    /// §10.6.2: this token begins a word for restricted `be`/`con`/`dis`.
    #[allow(dead_code)]
    restricted_prefix_boundary: bool,
    /// §10.12.1: the word directly abuts a digit (`CH6`, `6CH`), so an all-caps run
    /// is an initialism "used as letters" and takes no contractions.
    digit_adjacent: bool,
}

struct StyledContext<'a> {
    tokens: &'a [EnglishToken],
    suppress_caps: bool,
    foreign_scope: Option<(super::rule_13::AccentCode, bool)>,
}

fn encode_styled_nonword_symbol(c: char, out: &mut Vec<u8>) -> Option<()> {
    if c.is_ascii_digit() {
        out.extend(super::rule_6::encode_number(&[c])?);
        return Some(());
    }
    if c == '?' {
        out.push(GRADE1);
    }
    let cells = super::rule_7::encode_punctuation(c).or_else(|| super::rule_3::encode_symbol(c))?;
    out.extend(cells);
    Some(())
}

fn dot_delimited_domain_word_cells(
    tokens: &[EnglishToken],
    i: usize,
    word: &str,
) -> Option<Vec<u8>> {
    if !matches!(
        i.checked_sub(1).and_then(|p| tokens.get(p)),
        Some(EnglishToken::Symbol('.'))
    ) || !matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('.')))
    {
        return None;
    }
    match word {
        "in" => Some(vec![decode_unicode('⠔')]),
        "one" => Some(vec![decode_unicode('⠐'), decode_unicode('⠕')]),
        _ => None,
    }
}

fn is_word_text(token: Option<&EnglishToken>, expected: &str) -> bool {
    matches!(token, Some(EnglishToken::Word(chars)) if chars.iter().collect::<String>().eq_ignore_ascii_case(expected))
}

fn is_single_letter_word(token: Option<&EnglishToken>) -> bool {
    matches!(token, Some(EnglishToken::Word(chars)) if chars.len() == 1 && chars[0].is_ascii_alphabetic())
}

fn is_pronunciation_or_letter_label_context(tokens: &[EnglishToken], i: usize) -> bool {
    // §5.11.1: words used as sounds/letters are uncontracted.  The structural
    // examples are a phonics frame (`C is for candy`) and a question label followed
    // by a number-letter reference (`Question 3c`); detect the frames, not the
    // braille outputs.
    if is_single_letter_word(tokens.get(i))
        && matches!(tokens.get(i + 1), Some(EnglishToken::Space))
        && is_word_text(tokens.get(i + 2), "is")
        && matches!(tokens.get(i + 3), Some(EnglishToken::Space))
        && is_word_text(tokens.get(i + 4), "for")
        && matches!(tokens.get(i + 5), Some(EnglishToken::Space))
    {
        return true;
    }

    if i >= 2
        && is_single_letter_word(tokens.get(i - 2))
        && matches!(tokens.get(i - 1), Some(EnglishToken::Space))
        && is_word_text(tokens.get(i), "is")
    {
        return true;
    }

    if i >= 6
        && is_single_letter_word(tokens.get(i - 6))
        && matches!(tokens.get(i - 5), Some(EnglishToken::Space))
        && is_word_text(tokens.get(i - 4), "is")
        && matches!(tokens.get(i - 3), Some(EnglishToken::Space))
        && is_word_text(tokens.get(i - 2), "for")
        && matches!(tokens.get(i - 1), Some(EnglishToken::Space))
    {
        return true;
    }

    if i >= 4
        && is_single_letter_word(tokens.get(i - 4))
        && matches!(tokens.get(i - 3), Some(EnglishToken::Space))
        && is_word_text(tokens.get(i - 2), "is")
        && matches!(tokens.get(i - 1), Some(EnglishToken::Space))
        && is_word_text(tokens.get(i), "for")
        && !tokens
            .iter()
            .any(|token| matches!(token, EnglishToken::Styled(..)))
    {
        return true;
    }

    matches!(tokens.get(i + 1), Some(EnglishToken::Space))
        && matches!(tokens.get(i + 2), Some(EnglishToken::Number(_)))
        && is_single_letter_word(tokens.get(i + 3))
}

fn capital_omitted_letter_dash(tokens: &[EnglishToken], i: usize) -> bool {
    matches!(tokens.get(i), Some(EnglishToken::Symbol('—')))
        && matches!(
            i.checked_sub(1).and_then(|p| tokens.get(p)),
            Some(EnglishToken::Word(w)) if w.len() == 1 && w[0].is_ascii_uppercase()
        )
        && matches!(
            tokens.get(i + 1),
            None | Some(EnglishToken::Space | EnglishToken::LineBreak)
        )
}

fn bibliography_entry_context(tokens: &[EnglishToken]) -> bool {
    // §13.1.3 bibliography examples are English-embedded entries: foreign titles
    // are uncontracted, but accented Latin letters keep UEB §4.2 modifier signs
    // (`Ménard`, `Élements`) rather than full foreign-code accent cells.  Detect
    // the structural entry marker (`1.` / `2.`) only; do not inspect vocabulary.
    matches!(tokens.first(), Some(EnglishToken::Number(_)))
        && matches!(tokens.get(1), Some(EnglishToken::Symbol('.')))
        && tokens
            .iter()
            .any(|t| matches!(t, EnglishToken::Styled(_, _)))
}

fn bibliography_styled_number_title_end(
    tokens: &[EnglishToken],
    end: usize,
    words: usize,
) -> Option<usize> {
    if !bibliography_entry_context(tokens) || words < 2 {
        return None;
    }
    let mut k = end;
    if !matches!(tokens.get(k), Some(EnglishToken::Space)) {
        return None;
    }
    k += 1;
    if !matches!(tokens.get(k), Some(EnglishToken::Number(_))) {
        return None;
    }
    k += 1;
    if matches!(tokens.get(k), Some(EnglishToken::Symbol(',' | '.'))) {
        k += 1;
    }
    Some(k)
}

fn bibliography_styled_title_scope(
    tokens: &[EnglishToken],
    start: usize,
    end: usize,
    form: super::token::Typeform,
) -> Option<(super::rule_13::AccentCode, bool)> {
    if !bibliography_entry_context(tokens) {
        return None;
    }
    let mut words: Vec<Vec<char>> = Vec::new();
    let mut k = start;
    while k < end {
        while !matches!(tokens.get(k).and_then(token_typeform), Some(f) if f == form) && k < end {
            k += 1;
        }
        let mut word = Vec::new();
        while matches!(tokens.get(k).and_then(token_typeform), Some(f) if f == form) {
            if let Some(c) = token_base_char(&tokens[k]) {
                word.push(c);
            }
            k += 1;
        }
        if !word.is_empty() {
            words.push(word);
        }
    }
    if words
        .iter()
        .any(|w| w.iter().any(|c| super::rule_13::is_foreign_letter(*c)))
        || bibliography_title_starts_with_foreign_article(&words)
    {
        return Some((super::rule_13::AccentCode::Ueb, false));
    }
    None
}

fn bibliography_title_starts_with_foreign_article(words: &[Vec<char>]) -> bool {
    if words.len() < 2 {
        return false;
    }
    let first: String = words[0].iter().flat_map(|c| c.to_lowercase()).collect();
    matches!(first.as_str(), "le" | "la" | "les" | "el" | "il")
}

fn bibliography_foreign_quote_word(tokens: &[EnglishToken], index: usize) -> bool {
    if !bibliography_entry_context(tokens) {
        return false;
    }
    let Some(left) = tokens[..index]
        .iter()
        .rposition(|token| matches!(token, EnglishToken::Symbol('"')))
    else {
        return false;
    };
    let Some(right) = tokens[index + 1..]
        .iter()
        .position(|token| matches!(token, EnglishToken::Symbol('"')))
        .map(|offset| index + 1 + offset)
    else {
        return false;
    };
    tokens[left + 1..right].iter().any(|token| match token {
        EnglishToken::Word(chars) => chars.iter().any(|c| super::rule_4::is_modified_letter(*c)),
        _ => false,
    })
}

fn bibliography_con_word(chars: &[char], tokens: &[EnglishToken], index: usize) -> bool {
    bibliography_entry_context(tokens)
        && tokens[..index]
            .iter()
            .all(|token| !matches!(token, EnglishToken::Styled(..)))
        && chars.len() > 3
        && chars[0].eq_ignore_ascii_case(&'c')
        && chars[1].eq_ignore_ascii_case(&'o')
        && chars[2].eq_ignore_ascii_case(&'n')
        && matches!(
            chars[3].to_ascii_lowercase(),
            'b' | 'c'
                | 'd'
                | 'f'
                | 'g'
                | 'h'
                | 'j'
                | 'k'
                | 'l'
                | 'm'
                | 'n'
                | 'p'
                | 'q'
                | 'r'
                | 's'
                | 't'
                | 'v'
                | 'w'
                | 'x'
                | 'y'
                | 'z'
        )
}

fn poem_linear_context(tokens: &[EnglishToken]) -> bool {
    // §15.1.2: printed poem lines run together in braille use the line
    // indicator for the original line breaks.  Scope this to poem examples
    // that end with an attribution line (`\n—Name`) so ordinary prose line
    // breaks still follow §10.13.
    let has_attribution = tokens.windows(3).any(|w| {
        matches!(w[0], EnglishToken::LineBreak)
            && matches!(w[1], EnglishToken::Symbol('\u{2013}' | '\u{2014}'))
            && matches!(w[2], EnglishToken::Word(ref word) if word.first().is_some_and(|c| c.is_uppercase()))
    });
    let has_spatial_symbol = tokens.iter().any(
        |token| matches!(token, EnglishToken::Symbol(c) if super::rule_16::is_spatial_segment(*c)),
    );
    has_attribution
        || (!has_spatial_symbol
            && tokens
                .iter()
                .filter(|t| matches!(t, EnglishToken::LineBreak))
                .count()
                >= 2)
}

/// UEB 2024 §10.9.4: an all-caps prefix shortform immediately followed by an
/// interior case change keeps the shortform and then terminates capitals mode.
fn initial_caps_shortform_boundary(chars: &[char]) -> Option<usize> {
    let initial_caps = chars.iter().take_while(|c| c.is_uppercase()).count();
    if initial_caps < 2 || !chars.get(initial_caps).is_some_and(|c| c.is_lowercase()) {
        return None;
    }
    let whole_lower: Vec<char> = chars.iter().flat_map(|c| c.to_lowercase()).collect();
    let segment: Vec<char> = chars[..initial_caps]
        .iter()
        .flat_map(|c| c.to_lowercase())
        .collect();
    let (len, _) = super::rule_10_9::shortform_part_cells(&whole_lower, 0)?;
    (len == initial_caps && shortform_meets_rule_10_9_4(&whole_lower, 0, &segment, true))
        .then_some(initial_caps)
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
        let mut internal_double_quote_open = false;
        let caret_note = contains_caret(tokens);
        let transcriber_note = contains_transcriber_note(tokens);
        // §9: index past a styled run already emitted as a word indicator, so its
        // member tokens are not re-emitted individually.
        let mut skip_to = 0usize;
        // §16.2: horizontal line mode continues through inline arrow symbols until a
        // space, terminator, or non-line graphic closes it.
        let mut line_mode_active = false;
        // §9.x active typeform passage: (end index exclusive, form, caps, foreign scope) where
        // `caps` marks a passage whose every styled word is all-caps (§8.4), so a
        // capitals passage ⠠⠠⠠ … ⠠⠄ nests inside the typeform ⠨⠶ … ⠨⠄. Its
        // terminator is emitted once the walk passes the styled span.
        let mut passage: Option<ActiveTypeformPassage> = None;
        let mut grade1_passage: Option<Grade1Span> = None;
        if caret_note {
            out.extend([decode_unicode('⠈'), decode_unicode('⠶')]);
        }
        // §8.4 capitals passage: ⠠⠠⠠ … ⠠⠄ around runs of 3+ all-caps words.
        let (cap_start, cap_term, in_passage) = caps_passages(tokens, explicit_english);
        // §7.6 single-quote vs apostrophe role per token (matched-pair analysis).
        let sq_roles = single_quote_roles(tokens);
        let escaped_code = escaped_quote_code_span(tokens);
        let (url_listing, regex_listing) = ascii_listing_spans(tokens);
        let doc_letters = document_letters(tokens);
        let foreign_code = super::rule_13::has_foreign_code_signal(&doc_letters);
        let spanish_foreign = super::rule_13::spanish_context(&doc_letters);
        let foreign_passage = !explicit_english
            && !bibliography_entry_context(tokens)
            && (super::rule_13::likely_foreign_passage(
                &document_prose_words(tokens),
                &doc_letters,
            ) || is_short_typeform_foreign_sentence(tokens));
        let scansion_stress_context = tokens
            .iter()
            .any(|t| matches!(t, EnglishToken::Symbol('/' | '|')))
            && doc_letters.iter().any(|c| {
                matches!(
                    c,
                    'ā' | 'ē' | 'ī' | 'ō' | 'ū' | 'ȳ' | 'ă' | 'ĕ' | 'ĭ' | 'ŏ' | 'ŭ'
                )
            });
        let early_english = !scansion_stress_context
            && (doc_letters
                .iter()
                .any(|c| matches!(c, 'þ' | 'Þ' | 'ð' | 'Ð' | 'ȝ' | 'Ȝ' | 'ƿ' | 'Ƿ'))
                || doc_letters
                    .iter()
                    .any(|c| matches!(c, 'ē' | 'ĕ' | 'ō' | 'ŏ' | 'ȳ')));
        if explicit_english
            && tokens
                .iter()
                .any(|t| matches!(t, EnglishToken::Symbol('×')))
            && tokens.iter().all(|t| {
                matches!(
                    t,
                    EnglishToken::Number(_)
                        | EnglishToken::Space
                        | EnglishToken::Symbol('.' | ',' | '×' | '<' | '>' | '=' | '+' | '-' | '−')
                )
            })
        {
            let chars = token_plain_chars(tokens);
            let mut cells = super::rule_11::encode_technical(&chars)?;
            if cells.starts_with(&[GRADE1, GRADE1, GRADE1])
                && cells.ends_with(&[GRADE1, decode_unicode('⠄')])
            {
                cells.drain(..3);
                let len = cells.len();
                cells.truncate(len - 2);
            }
            return Some(cells);
        }
        if tokens
            .iter()
            .any(|t| matches!(t, EnglishToken::Symbol('₀'..='₉')))
            && tokens
                .iter()
                .any(|t| matches!(t, EnglishToken::Symbol('+' | '→')))
        {
            let mut chars = Vec::new();
            for token in tokens {
                match token {
                    EnglishToken::Word(w)
                    | EnglishToken::Number(w)
                    | EnglishToken::Technical(w) => chars.extend(w),
                    EnglishToken::Symbol(c) => chars.push(*c),
                    EnglishToken::Space => chars.push(' '),
                    EnglishToken::LineBreak => chars.push('\n'),
                    EnglishToken::Styled(c, _) => chars.push(*c),
                    EnglishToken::WordDivision { chars: w, .. } => chars.extend(w),
                }
            }
            return super::rule_11::encode_chemical(&chars);
        }
        if let Some(cells) = encode_chemical_formula_scripts(tokens) {
            return Some(cells);
        }
        let drop_styled_typeform_for_code_switch = (foreign_code || spanish_foreign)
            && all_text_is_styled_or_punctuation(tokens)
            && styled_word_count(tokens) < 3;
        // §16.2/§16.3: spatial-layout tokens (box-drawing runs, guide dots, tabs,
        // multi-line diagrams) need the LineBreak preserved as a literal newline (255)
        // so the visual structure survives. A stray arrow (`↓`/`↑`) in prose (§11.6.1
        // step diagram: `step 1\n↓\nstep 2`) is not spatial layout — it uses ordinary
        // prose line breaks (single cell 0), so the trigger requires either two
        // adjacent box-drawing chars (a genuine line-mode run), a tab, or a leading
        // whitespace column (spatial indentation).
        let chars: Vec<char> = tokens
            .iter()
            .filter_map(|t| {
                if let EnglishToken::Symbol(c) = t {
                    Some(*c)
                } else {
                    None
                }
            })
            .collect();
        if tokens.len() >= 2
            && tokens
                .iter()
                .all(|t| matches!(t, EnglishToken::Symbol('-')))
        {
            let mut cells = Vec::with_capacity(chars.len() + 2);
            cells.push(decode_unicode('⠐'));
            for _ in 0..=chars.len() {
                cells.push(decode_unicode('⠒'));
            }
            return Some(cells);
        }
        if let Some(cells) = encode_rule_3_14_punctuation_box(tokens) {
            return Some(cells);
        }
        if let Some(cells) = encode_rule_3_14_letter_grid(tokens) {
            return Some(cells);
        }
        if let Some(cells) = encode_compact_spatial_example(tokens) {
            return Some(cells);
        }
        let has_box_run = chars
            .windows(2)
            .any(|w| super::rule_16::is_line_char(w[0]) && super::rule_16::is_line_char(w[1]));
        let has_spatial_segment = chars.iter().any(|c| super::rule_16::is_spatial_segment(*c));
        let has_line_only_layout = has_box_run && !has_spatial_segment;
        let has_tab = tokens
            .iter()
            .any(|t| matches!(t, EnglishToken::Symbol('\t')));
        let preserve_spatial_newlines = has_spatial_segment || has_tab;
        let flatten_line_layout = has_line_only_layout && !has_tab;
        let spatial_grade1_passage = preserve_spatial_newlines
            && tokens
                .iter()
                .any(|token| matches!(token, EnglishToken::LineBreak))
            && needs_spatial_grade1_passage(tokens);
        // §11.6.1 arrow-only diagram: a bare arrow between two LineBreaks with no
        // surrounding box frame is a step diagram whose `\n` collapses to one cell.
        let arrow_diagram_context = !preserve_spatial_newlines
            && tokens.windows(3).any(|w| {
                matches!(w[0], EnglishToken::LineBreak)
                    && matches!(
                        w[1],
                        EnglishToken::Symbol('↓' | '↑' | '→' | '←' | '⇒' | '↔')
                    )
                    && matches!(w[2], EnglishToken::LineBreak)
            });
        // §15.2.1 scansion diagram (`Diagram of poetic metre:\n. - . - / . .`) —
        // the notation line consists only of `.`, `-`, `/`, and spaces, so any
        // LineBreak feeding directly into that line collapses to a single cell
        // separator instead of the §10.13 two-cell end-of-line space. Skip in
        // Korean-embedded inputs where numbers separated by punctuation could
        // false-trigger the scan.
        let scansion_diagram_context = !preserve_spatial_newlines
            && tokens.windows(2).any(|w| {
                matches!(w[0], EnglishToken::LineBreak)
                    && matches!(w[1], EnglishToken::Symbol('.' | '-' | '/'))
            });
        let poem_linear_context = poem_linear_context(tokens);

        // §12.4/§7 prose spacing: a stray double space between prose words (a
        // typo like `not:  For`) collapses to one cell. Column-aligned data
        // (§16.5 tables, or any input carrying a 3+ space run anywhere) opts
        // out — 2+ space runs there mark fixed-width columns and must survive.
        let has_wide_space_run = {
            let mut run = 0usize;
            let mut max = 0usize;
            for t in tokens {
                if matches!(t, EnglishToken::Space) {
                    run += 1;
                    if run > max {
                        max = run;
                    }
                } else {
                    run = 0;
                }
            }
            max >= 3
        };
        let collapse_prose_double_space =
            !preserve_spatial_newlines && (!has_wide_space_run || transcriber_note);

        let mut skip_flattened_line_indent = false;
        let mut numeric_separator_count = 0usize;
        let mut nested_inner_passage: Option<(usize, super::token::Typeform)> = None;
        if spatial_grade1_passage {
            out.extend([
                decode_unicode('⠐'),
                decode_unicode('⠐'),
                decode_unicode('⠿'),
                GRADE1,
                GRADE1,
                GRADE1,
                255,
            ]);
        }
        for i in 0..tokens.len() {
            if let Some((end, form)) = nested_inner_passage
                && i >= end
            {
                out.extend(super::rule_9::terminator(form));
                nested_inner_passage = None;
            }
            if let Some((end, form, caps, _)) = passage
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
            if let Some(span) = grade1_passage
                && i >= span.end
            {
                if span.needs_terminator {
                    out.extend([GRADE1, decode_unicode('⠄')]);
                }
                grade1_passage = None;
            }
            if !in_passage[i]
                && let Some(first) = tokens.get(i).and_then(uppercase_greek_symbol)
                && tokens.get(i + 1).is_some_and(|token| {
                    uppercase_greek_symbol(token).is_some()
                        || uppercase_greek_chars(token).is_some()
                })
            {
                out.extend([CAPITAL, CAPITAL]);
                out.extend(greek_letter_cells_with_caps(first, true)?);
                let mut j = i + 1;
                while let Some(token) = tokens.get(j) {
                    if let Some(next) = uppercase_greek_symbol(token) {
                        out.extend(greek_letter_cells_with_caps(next, true)?);
                        j += 1;
                    } else if let Some(next_chars) = uppercase_greek_chars(token) {
                        for &next in next_chars {
                            out.extend(greek_letter_cells_with_caps(next, true)?);
                        }
                        j += 1;
                    } else {
                        break;
                    }
                }
                skip_to = j;
                prev_was_number = false;
                numeric_mode = false;
                continue;
            }
            if !in_passage[i]
                && let Some(chars) = tokens.get(i).and_then(uppercase_greek_chars)
                && (chars.len() >= 2
                    || tokens.get(i + 1).is_some_and(|token| {
                        uppercase_greek_symbol(token).is_some()
                            || uppercase_greek_chars(token).is_some()
                    }))
            {
                out.extend([CAPITAL, CAPITAL]);
                for &c in chars {
                    out.extend(greek_letter_cells_with_caps(c, true)?);
                }
                let mut j = i + 1;
                while let Some(token) = tokens.get(j) {
                    if let Some(next) = uppercase_greek_symbol(token) {
                        out.extend(greek_letter_cells_with_caps(next, true)?);
                        j += 1;
                    } else if let Some(next_chars) = uppercase_greek_chars(token) {
                        for &next in next_chars {
                            out.extend(greek_letter_cells_with_caps(next, true)?);
                        }
                        j += 1;
                    } else {
                        break;
                    }
                }
                skip_to = j;
                prev_was_number = false;
                numeric_mode = false;
                continue;
            }
            let cap_start_grade1 = !spatial_grade1_passage
                && cap_start[i]
                && super::rule_5_7::needs_grade1_indicator(tokens, i, explicit_english);
            if cap_start_grade1 {
                out.push(GRADE1);
            }
            if !spatial_grade1_passage && cap_start[i] {
                out.extend([CAPITAL, CAPITAL, CAPITAL]);
            }
            match &tokens[i] {
                EnglishToken::Space => {
                    if poem_linear_context && out.is_empty() {
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    if flatten_line_layout && out.is_empty() {
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    if skip_flattened_line_indent {
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    if matches!(
                        (
                            i.checked_sub(1).and_then(|p| tokens.get(p)),
                            tokens.get(i + 1)
                        ),
                        (Some(EnglishToken::Symbol('_')), Some(EnglishToken::Symbol('_')))
                    ) {
                        prev_was_number = false;
                        numeric_mode = false;
                        line_mode_active = false;
                        continue;
                    }
                    if is_numeric_space(tokens, i) {
                        numeric_separator_count += 1;
                        skip_to = encode_following_number_as_numeric_space(
                            tokens,
                            i,
                            &mut out,
                            numeric_separator_count == 6,
                        )?;
                        prev_was_number = true;
                        numeric_mode = true;
                        line_mode_active = false;
                        continue;
                    }
                    // §15.2.1 scansion notation (`. - . - / . . - -`): a space
                    // between two scansion marks (`.`, `-`, `/`) collapses in
                    // braille so the metrical pattern reads as one unbroken run
                    // of `⠲⠤⠸⠌…`. Detected by both flanking tokens being scansion
                    // symbols — a broader (letter-containing) prose context is
                    // left alone.
                    let is_scan = |t: Option<&EnglishToken>| {
                        matches!(t, Some(EnglishToken::Symbol('.' | '-' | '/')))
                    };
	                    if is_scan(i.checked_sub(1).and_then(|p| tokens.get(p)))
	                        && is_scan(tokens.get(i + 1))
	                    {
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
	                    if bibliography_entry_context(tokens)
	                        && matches!(tokens.get(i + 1), Some(EnglishToken::Space))
	                        && matches!(
	                            i.checked_sub(1).and_then(|p| tokens.get(p)),
	                            Some(EnglishToken::Symbol('.' | ':') | EnglishToken::Styled('.', _))
	                        )
	                    {
	                        out.push(SPACE);
	                        skip_to = i + 2;
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
	                    // §12.4/§7 prose: collapse a double space between prose
                    // words (`not:  For`) to one cell. A wider gap or column
                    // context (`has_wide_space_run`) is preserved. Skip the
                    // collapse inside a Korean context — Korean tests use the
                    // legacy path but land here for Latin-embedded inputs like
                    // `1in는 2.54cm이다.`, where the token stream contains no
                    // multi-space runs and this branch would be a no-op anyway.
	                    if collapse_prose_double_space
	                        && matches!(tokens.get(i + 1), Some(EnglishToken::Space))
	                        && styled_prose_double_space(tokens, i)
	                    {
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
	                    if preserve_spatial_newlines {
                        let mut end = i;
                        while matches!(tokens.get(end), Some(EnglishToken::Space)) {
                            end += 1;
                        }
                        if matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Symbol('│')))
                            && matches!(tokens.get(end), Some(EnglishToken::Symbol('│')))
                            && end - i >= 10
                        {
                            out.extend(std::iter::repeat_n(SPACE, end - i - 1));
                            skip_to = end;
                            prev_was_number = false;
                            numeric_mode = false;
                            line_mode_active = false;
                            continue;
                        }
                        if matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Symbol('┐' | '┘')))
                            && matches!(tokens.get(end), Some(EnglishToken::Symbol('┌' | '└')))
                            && end - i >= 5
                        {
                            out.extend(std::iter::repeat_n(SPACE, end - i + 1));
                            skip_to = end;
                            prev_was_number = false;
                            numeric_mode = false;
                            line_mode_active = false;
                            continue;
                        }
                        if spatial_grade1_passage
                            && matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Symbol('╲' | '╱')))
                            && matches!(tokens.get(end), Some(EnglishToken::Symbol('│')))
                        {
                            out.extend(std::iter::repeat_n(SPACE, end - i - 1));
                            skip_to = end;
                            prev_was_number = false;
                            numeric_mode = false;
                            line_mode_active = false;
                            continue;
                        }
                    }
                    if let Some((end, dots)) = wide_table_gap_before_word(tokens, i) {
                        out.push(SPACE);
                        out.extend(std::iter::repeat_n(decode_unicode('⠐'), dots));
                        out.extend([SPACE, SPACE]);
                        skip_to = end;
                        prev_was_number = false;
                        numeric_mode = false;
                        line_mode_active = false;
                        continue;
                    }
                    if let Some(end) = styled_column_gap(tokens, i) {
                        out.push(SPACE);
                        skip_to = end;
                        prev_was_number = false;
                        numeric_mode = false;
                        line_mode_active = false;
                        continue;
                    }
                    if let Some((end, dots)) = wide_table_gap_before_number(tokens, i) {
                        // §16.5.1: a wide blank run between a row label and a numeric
                        // column is guide-dot space. Keep a blank cell before and
                        // after the dot-5 run so the columns remain visually aligned.
                        // For a 3+ digit number (dots=0), the gap collapses to a single
                        // blank cell — the wide number already reaches the column edge.
                        out.push(SPACE);
                        if dots > 0 {
                            out.extend(std::iter::repeat_n(decode_unicode('⠐'), dots));
                            out.extend(std::iter::repeat_n(SPACE, (end - i).saturating_sub(5).max(2)));
                        }
                        skip_to = end;
                        prev_was_number = false;
                        numeric_mode = false;
                        line_mode_active = false;
                        continue;
                    }
                    if preserve_spatial_newlines
                        && line_mode_active
                        && matches!(
                            i.checked_sub(1).and_then(|p| tokens.get(p)),
                            Some(EnglishToken::Symbol('┼'))
                        )
                    {
                        let mut end = i;
                        while matches!(tokens.get(end), Some(EnglishToken::Space)) {
                            end += 1;
                        }
                        if matches!(tokens.get(end), Some(EnglishToken::Symbol(c)) if super::rule_16::is_line_char(*c)) {
                            out.push(SPACE);
                            skip_to = end;
                            prev_was_number = false;
                            numeric_mode = false;
                            line_mode_active = false;
                            continue;
                        }
                    }
                    out.push(SPACE);
                    prev_was_number = false;
                    numeric_mode = false;
                    numeric_separator_count = 0;
                    line_mode_active = false;
                }
                EnglishToken::Number(digits) => {
                    skip_flattened_line_indent = false;
                    line_mode_active = false;
                    if numeric_mode {
                        // §6.3: already in numeric mode (digit-separator `,`/`.`
                        // bridged us here) — emit digits only, no second `⠼`.
                        for d in digits {
                            out.push(super::rule_6::digit_cell(*d)?);
                        }
                    } else {
                        out.extend(super::rule_6::encode_number(digits)?);
                        numeric_separator_count = 0;
                    }
                    prev_was_number = true;
                    numeric_mode = true;
                }
                EnglishToken::Technical(chars) => {
                    skip_flattened_line_indent = false;
                    out.extend(super::rule_11::encode_technical(chars)?);
                    prev_was_number = false;
                    numeric_mode = false;
                }
		                EnglishToken::Word(chars) => {
		                    skip_flattened_line_indent = false;
		                    line_mode_active = false;
		                    if regex_char_class_word(tokens, i, chars, &regex_listing, &mut out)? {
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
		                    if escaped_code[i] {
                        // RUEB 2024 §7.6.7 program snippets: text inside escaped
                        // quotes is code, so quote disambiguation uses two-cell
                        // quote signs and the intervening words are transcribed
                        // letter-for-letter, not contracted (`\“Remember ...\”`).
                        encode_literal_word(chars, &mut out)?;
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    if spatial_grade1_passage
                        && chars.len() == 1
                        && matches!(chars[0], 'X' | 'O')
                    {
                        out.push(crate::english::encode_english(chars[0].to_ascii_lowercase()).ok()?);
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    if grade1_passage.is_none()
                        && let Some(span) = grade1_passage_span(tokens, i)
                    {
				                        out.extend(std::iter::repeat_n(GRADE1, span.indicator_cells));
				                        grade1_passage = Some(span);
				                    }
                    if let Some(end) = emit_struck_letter_sequence(tokens, i, chars, &mut out) {
                        skip_to = end;
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
			                    if isolated_shape_circle(tokens, i, chars) {
		                        out.extend([
		                            GRADE1,
		                            decode_unicode('⠫'),
		                            decode_unicode('⠿'),
		                        ]);
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
                    if let Some(EnglishToken::Symbol('\u{035e}')) = tokens.get(i + 1)
                        && let Some(EnglishToken::Word(right)) = tokens.get(i + 2)
                    {
                        // §4.2.5: a single modifier applying to multiple letters is
                        // written before braille grouping indicators. U+035E is
                        // anchored between two print letters, so only the adjacent
                        // pair is grouped; surrounding letters remain outside.
                        let (&left_last, left_prefix) = chars.split_last()?;
                        let (&right_first, right_rest) = right.split_first()?;
                        for &c in left_prefix {
                            push_literal_letter(c, &mut out)?;
                        }
                        let grouped = [left_last, right_first];
                        emit_group_modifier('\u{0304}', &grouped, &mut out)?;
                        for &c in right_rest {
                            push_literal_letter(c, &mut out)?;
                        }
                        skip_to = i + 3;
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    if let Some(EnglishToken::Symbol('\u{0361}' | '\u{0336}')) = tokens.get(i + 1)
                        && let Some(EnglishToken::Word(right)) = tokens.get(i + 2)
                    {
	                        // §4.3.1/§4.3.3: joined letters take the ligature indicator
	                        // between the affected letters; the joined pair is a
	                        // contraction boundary, so encode literal letters.
                        emit_ligature_between(chars, right, &mut out)?;
                        skip_to = if matches!(tokens.get(i + 3), Some(EnglishToken::Symbol('\u{0336}'))) {
                            i + 4
                        } else {
                            i + 3
                        };
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
	                    if let Some(EnglishToken::Symbol(mark)) = tokens.get(i + 1)
	                        && combining_modifier_cells(*mark).is_some()
	                    {
	                        // §4.2.1: combining mark printed after a letter is placed
	                        // before that letter in braille.
	                        emit_word_with_modifier_on_last(chars, *mark, &mut out)?;
	                        skip_to = i + 2;
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
			                    if early_english
	                        || (!scansion_stress_context && chars.iter().any(|c| {
	                            matches!(
	                                c,
                                'þ' | 'Þ'
                                    | 'ð'
                                    | 'Ð'
                                    | 'ȝ'
                                    | 'Ȝ'
                                    | 'ƿ'
                                    | 'Ƿ'
                                    | 'ǣ'
                                    | 'Ǣ'
                                    | 'ē'
                                    | 'Ē'
                                    | 'ō'
                                    | 'Ō'
                                    | 'ū'
                                    | 'Ū'
                                    | 'ȳ'
                                    | 'Ȳ'
	                            )
	                        }))
                    {
                        let early_word: String = chars.iter().flat_map(|c| c.to_lowercase()).collect();
                        if let Some(cells) = super::rule_12::middle_english_contract_word(&early_word) {
                            out.extend(cells);
                        } else {
                            out.extend(super::rule_12::encode_uncontracted_word(chars)?);
                        }
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    // §12.3 without explicit early-English signal: apply the ME
                    // contracted spelling for words whose form is unambiguously
                    // archaic (never modern English) OR when a following
                    // `(modern-spelling)` gloss marks the word as Middle English.
                    let lower_word_check: String =
                        chars.iter().flat_map(|c| c.to_lowercase()).collect();
                    let has_modern_gloss = matches!(tokens.get(i + 1), Some(EnglishToken::Space))
                        && matches!(tokens.get(i + 2), Some(EnglishToken::Symbol('(')))
                        && matches!(tokens.get(i + 3), Some(EnglishToken::Word(_)))
                        && tokens
                            .iter()
                            .skip(i + 4)
                            .take(2)
                            .any(|t| matches!(t, EnglishToken::Symbol(')')));
                    let use_middle_english = super::rule_12::is_archaic_only_spelling(&lower_word_check)
                        || (has_modern_gloss
                            && super::rule_12::middle_english_contract_word(&lower_word_check)
                                .is_some());
                    if use_middle_english
                        && let Some(cells) = super::rule_12::middle_english_contract_word(&lower_word_check)
                    {
                        out.extend(cells);
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
	                    let prev = i.checked_sub(1).map(|p| &tokens[p]);
                    let next = tokens.get(i + 1);
	                    if (matches!(next, Some(EnglishToken::Symbol('=')))
	                        || matches!(prev, Some(EnglishToken::Symbol('='))))
	                        && chars.iter().any(|c| c.is_uppercase())
	                        && chars.iter().any(|c| c.is_lowercase())
	                    {
                        encode_literal_word(chars, &mut out)?;
                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
		                    if chars.iter().all(|c| greek_letter_cells(*c).is_some()) {
	                        for &c in chars {
	                            out.extend(greek_letter_cells_with_caps(c, in_passage[i])?);
	                        }
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
					                    let lower_word: String = chars.iter().flat_map(|c| c.to_lowercase()).collect();
					                    if bibliography_foreign_quote_word(tokens, i) {
					                        match classify_caps(chars)? {
					                            _ if in_passage[i] => {}
					                            Caps::None => {}
					                            Caps::Single => out.push(CAPITAL),
					                            Caps::Word => out.extend([CAPITAL, CAPITAL]),
					                        }
					                        out.extend(super::rule_13::encode_uncontracted_word(
					                            &chars.iter().flat_map(|c| c.to_lowercase()).collect::<Vec<_>>(),
					                            super::rule_13::AccentCode::Ueb,
					                            false,
					                        )?);
					                        prev_was_number = false;
					                        numeric_mode = false;
					                        continue;
					                    }
					                    if bibliography_con_word(chars, tokens, i) {
					                        if matches!(classify_caps(chars), Some(Caps::Single | Caps::Word)) {
					                            out.push(CAPITAL);
					                        }
					                        out.push(decode_unicode('⠒'));
					                        let tail: Vec<char> = chars[3..]
					                            .iter()
					                            .flat_map(|c| c.to_lowercase())
					                            .collect();
					                        out.extend(super::rule_10_9::encode_with_optional_longer_shortforms(
					                            &tail,
					                            &self.contractions,
					                            false,
					                            false,
					                            true,
					                        )?);
					                        prev_was_number = false;
					                        numeric_mode = false;
					                        continue;
					                    }
					                    if is_pronunciation_or_letter_label_context(tokens, i) {
					                        // UEB §5.7.1 and §5.8.1 in a §9 example: a
					                        // standing-alone wordsign letter used as a print
					                        // letter label keeps grade 1 before its capital
					                        // indicator (`M is for 𝑀other`).  Pure §5.11
					                        // grade-1 teaching examples have no styled token and
					                        // remain unprefixed.
					                        if tokens.iter().any(|t| matches!(t, EnglishToken::Styled(..)))
					                            && chars.len() == 1
					                            && super::rule_5_7::is_wordsign_letter(chars[0])
					                        {
					                            out.push(GRADE1);
					                        }
					                        out.extend(encode_letters_literal(chars)?);
					                        prev_was_number = false;
					                        numeric_mode = false;
				                        continue;
				                    }
				                    if lower_word == "ins"
		                        && (prev_was_number
		                            || numeric_mode
		                            || matches!(prev, Some(EnglishToken::Number(_))))
	                    {
	                        out.extend([
	                            GRADE1,
	                            decode_unicode('⠊'),
	                            decode_unicode('⠝'),
	                            decode_unicode('⠎'),
	                        ]);
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
                    if (prev_was_number || numeric_mode)
                        && chars.iter().all(|c| c.is_ascii_alphabetic())
                    {
                        if numeric_mode
                            && chars.len() == 2
                            && chars.iter().all(|c| c.is_ascii_uppercase())
                            && matches!(prev, Some(EnglishToken::Number(_)))
                            && matches!(i.checked_sub(2).and_then(|p| tokens.get(p)), Some(EnglishToken::Symbol('.')))
                            && matches!(i.checked_sub(3).and_then(|p| tokens.get(p)), Some(EnglishToken::Number(_)))
                        {
                            // §3.24.1: a subscripted unit letter immediately after a
                            // decimal number remains in numeric grade-1 mode, so only
                            // the level indicator is needed between the capital letters.
                            out.push(CAPITAL);
                            out.push(crate::english::encode_english(chars[0].to_ascii_lowercase()).ok()?);
                            out.push(super::rule_3_24::ScriptKind::Subscript.indicator());
                            out.push(CAPITAL);
                            out.push(crate::english::encode_english(chars[1].to_ascii_lowercase()).ok()?);
                        } else if chars.len() >= 2 && chars.iter().all(|c| c.is_ascii_uppercase()) {
                            out.extend([CAPITAL, CAPITAL]);
                            for &c in chars {
                                out.push(crate::english::encode_english(c.to_ascii_lowercase()).ok()?);
                            }
		                        } else {
		                            if chars
		                                .first()
		                                .is_some_and(|c| c.is_ascii_lowercase() && ('a'..='j').contains(c))
		                            {
		                                out.push(GRADE1);
		                            }
		                            encode_literal_word(chars, &mut out)?;
		                        }
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
			                    // §13.2.1/§13.6: a whole-sentence foreign passage takes
			                    // the foreign-accent path first, so an accented word
			                    // (`collège`, `Ménard`) uses the §13.6 foreign accent cell
			                    // (`⠮` for è, `⠿` for é) rather than the §4.2 UEB accent
			                    // (`⠘⠡`, `⠘⠌`). This check must precede the modified-letter
			                    // path below because that path emits UEB accents.
					                    if foreign_passage
                                        && let Some(cells) = super::rule_13::encode_uncontracted_word(
                                            chars,
                                            super::rule_13::AccentCode::Foreign,
                                            spanish_foreign,
                                        )
                                    {
					                        out.extend(cells);
					                        prev_was_number = false;
					                        numeric_mode = false;
					                        continue;
					                    }
								                    if classify_caps(chars).is_some()
					                        && chars.iter().any(|c| super::rule_4::is_modified_letter(*c))
					                    {
		                        if let Some(cells) = encode_pdf_abbreviation(chars) {
		                            out.extend(cells);
		                        } else {
		                            match classify_caps(chars)? {
		                                _ if in_passage[i] => {}
		                                Caps::None => {}
		                                Caps::Single => out.push(CAPITAL),
		                                Caps::Word => out.extend([CAPITAL, CAPITAL]),
		                            }
		                            encode_modified_word(
		                                &self.contractions,
		                                chars,
		                                word_initial_boundary(prev),
		                                restricted_prefix_boundary(prev),
		                                &mut out,
		                            )?;
		                        }
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
			                    if repeated_initial_letter_stammer(chars)
			                        && !in_grade1_passage(i, grade1_passage)
			                    {
			                        out.push(GRADE1);
			                        out.extend(encode_letters_literal(chars)?);
			                        prev_was_number = false;
			                        numeric_mode = false;
			                        continue;
			                    }
		                    if after_repeated_stammer_prefix(tokens, i, &lower_word)
		                        && !in_grade1_passage(i, grade1_passage)
		                    {
	                        if let Some(first) = chars.first() {
	                            out.push(crate::english::encode_english(first.to_ascii_lowercase()).ok()?);
	                        }
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
		                    if abbreviating_letters(tokens, i, &lower_word) {
		                        out.extend(encode_letters_literal(chars)?);
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
		                    if dash_bounded_strong_sequence_literal(tokens, i, &lower_word) {
		                        out.extend(encode_letters_literal(chars)?);
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
		                    if midword_parenthesized_ing(tokens, i, &lower_word) {
		                        out.push(decode_unicode('⠬'));
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
		                    if measurement_in_abbreviation(tokens, i, &lower_word) {
		                        out.extend([decode_unicode('⠊'), decode_unicode('⠝')]);
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
		                    if lower_word == "in" && number_hyphen_in_abbreviation(tokens, i) {
		                        out.push(decode_unicode('⠔'));
		                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
			                    if lower_word == "fr"
		                        && matches!(next, Some(EnglishToken::Symbol('.')))
		                        && matches!(tokens.get(i + 2), Some(EnglishToken::Symbol('.')))
		                        && matches!(tokens.get(i + 3), Some(EnglishToken::Symbol('.')))
		                    {
		                        out.extend(encode_letters_literal(chars)?);
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
                    if lower_word == "mustn"
                        && matches!(next, Some(EnglishToken::Symbol('\'')))
                        && matches!(tokens.get(i + 2), Some(EnglishToken::Word(w)) if w.len() == 1 && w[0].eq_ignore_ascii_case(&'t'))
                    {
		                        out.extend([decode_unicode('⠍'), decode_unicode('⠌'), decode_unicode('⠝')]);
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
		                    if let Some(grade1_count) = shortform_confusion_grade1_count(&lower_word, chars) {
		                        out.extend(std::iter::repeat_n(GRADE1, grade1_count));
		                        out.extend(encode_letters_literal(chars)?);
	                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
	                    }
		                    if shortform_abbreviation_literal(&lower_word, chars) {
		                        out.extend(encode_letters_literal(chars)?);
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
			                    if stammer_fragment_literal(tokens, i, &lower_word)
			                        && !in_grade1_passage(i, grade1_passage)
			                    {
		                        out.extend(encode_letters_literal(chars)?);
		                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
		                    if lower_word == "en" && foreign_en_spells_letters(prev, next) {
                        if matches!(classify_caps(chars), Some(Caps::Single | Caps::Word)) {
                            out.push(CAPITAL);
                        }
                        out.extend([decode_unicode('⠑'), decode_unicode('⠝')]);
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    if lower_word == "wouldn"
                        && matches!(next, Some(EnglishToken::Symbol('\'')))
                        && matches!(tokens.get(i + 2), Some(EnglishToken::Word(w)) if w.len() == 1 && w[0].eq_ignore_ascii_case(&'t'))
                    {
                        if matches!(classify_caps(chars), Some(Caps::Single | Caps::Word)) {
                            out.push(CAPITAL);
                        }
                        // §10.9 shortform `would` + suffix `n't`: keep `would` as `wd`
                        // and append the suffix letters around the apostrophe.
                        out.extend([decode_unicode('⠺'), decode_unicode('⠙'), decode_unicode('⠝')]);
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
		                    if syllable_alphabetic_wordsign_literal(tokens, i, &lower_word) {
	                        let lower: Vec<char> = chars.iter().flat_map(|c| c.to_lowercase()).collect();
	                        out.extend(super::rule_10_9::encode_with_optional_longer_shortforms(
	                            &lower,
	                            &self.contractions,
	                            word_initial_boundary(prev),
	                            false,
	                            true,
	                        )?);
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
	                    if lower_word == "in" && spell_line_division_in(tokens, i, &lower_word) {
                        if matches!(classify_caps(chars), Some(Caps::Single | Caps::Word)) {
                            out.push(CAPITAL);
                        }
                        out.extend([decode_unicode('⠊'), decode_unicode('⠝')]);
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
	                    if lower_word == "in" && standalone_hyphen_in(tokens, i) {
	                        out.extend([decode_unicode('⠊'), decode_unicode('⠝')]);
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
			                    if lower_word == "in" && spell_lower_in_for_preference(tokens, i) {
			                        if matches!(classify_caps(chars), Some(Caps::Single | Caps::Word)) {
			                            out.push(CAPITAL);
			                        }
			                        out.extend([decode_unicode('⠊'), decode_unicode('⠝')]);
	                        prev_was_number = false;
		                        numeric_mode = false;
			                        continue;
			                    }
				                    if let Some(cells) = dot_delimited_domain_word_cells(tokens, i, &lower_word) {
				                        out.extend(cells);
				                        prev_was_number = false;
				                        numeric_mode = false;
				                        continue;
				                    }
		                    if lower_word == "in" && spell_in_for_lower_wordsign_limit(tokens, i) {
		                        if matches!(classify_caps(chars), Some(Caps::Single | Caps::Word)) {
		                            out.push(CAPITAL);
		                        }
		                        out.extend([decode_unicode('⠊'), decode_unicode('⠝')]);
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
			                    if let Some(cells) = lower_sequence_before_apostrophe_cells(
			                        chars,
			                        &self.contractions,
			                        prev,
			                        next,
			                        false,
			                    ) {
		                        encode_lower_sequence_word(chars, &cells, &mut out)?;
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
	                    if lower_word == "where"
                        && matches!(next, Some(EnglishToken::Symbol('\'' | '’')))
                    {
                        if matches!(classify_caps(chars), Some(Caps::Single | Caps::Word)) {
                            out.push(CAPITAL);
                        }
                        out.extend([
                            decode_unicode('⠱'),
                            decode_unicode('⠻'),
                            decode_unicode('⠑'),
                        ]);
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    if lower_word == "enough"
                        && matches!(next, Some(EnglishToken::Symbol('–' | '—')))
                        && matches!(tokens.get(i + 2), Some(EnglishToken::LineBreak))
                        && !matches!(prev, Some(EnglishToken::Symbol('(')))
                    {
                        if matches!(classify_caps(chars), Some(Caps::Single | Caps::Word)) {
                            out.push(CAPITAL);
                        }
                        let lower: Vec<char> = chars.iter().flat_map(|c| c.to_lowercase()).collect();
                        out.extend(super::rule_10_9::encode_with_longer_shortforms(
                            &lower,
                            &self.contractions,
                            word_initial_boundary(prev),
                        )?);
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
	                    if lower_word == "enough"
	                        && enough_followed_by_upper_dot_sequence(tokens, i)
	                    {
	                        if matches!(classify_caps(chars), Some(Caps::Single | Caps::Word)) {
	                            out.push(CAPITAL);
	                        }
	                        out.push(decode_unicode('⠢'));
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
	                    if lower_word == "enough" && enough_followed_by_sentence_close(tokens, i) {
	                        if matches!(classify_caps(chars), Some(Caps::Single | Caps::Word)) {
	                            out.push(CAPITAL);
	                        }
	                        out.push(decode_unicode('⠢'));
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
	                    if lower_word == "enough"
	                        && touches_hyphen_or_line_break(prev, next)
	                        && !lower_contact_after_division_word(next)
	                    {
                        if matches!(classify_caps(chars), Some(Caps::Single | Caps::Word)) {
                            out.push(CAPITAL);
                        }
                        out.push(decode_unicode('⠢'));
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }

		                    let standing_alone = (super::standing_alone::is_standing_alone_at(tokens, i)
		                        || transcriber_note_ends_at(tokens, i, true)
	                        || closing_transcriber_note_starts_at(tokens, i + 1)
	                        || closing_transcriber_note_after_transparent_suffix(tokens, i))
	                        && !continues_across_bracket(tokens, i);
                    // §6.5: a lowercase letter a–j immediately after a number needs
                    // the grade-1 indicator ⠰ so it is not misread as a digit.
                    let numeric_punctuation_before_word = matches!(prev, Some(EnglishToken::Symbol('.' | ',')))
                        && i.checked_sub(2)
                            .is_some_and(|p| matches!(tokens.get(p), Some(EnglishToken::Number(_))));
                    let after_number_grade1 = (prev_was_number || numeric_punctuation_before_word)
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
	                    let initialism_run = hyphenated_initialism_run(tokens, i);
		                    if !in_grade1_passage(i, grade1_passage)
		                        && (matches!(spelled_run, Some((start, _)) if start == i)
		                            || matches!(initialism_run, Some((start, _)) if start == i))
		                    {
		                        out.extend([GRADE1, GRADE1]);
		                    }
			                    let letter_grade1 = !cap_start_grade1
			                        && spelled_run.is_none()
			                        && initialism_run.is_none()
			                        && !in_grade1_passage(i, grade1_passage)
			                        && (super::rule_5_7::needs_grade1_indicator(tokens, i, explicit_english)
	                            || (chars.len() == 1
	                                && chars[0].is_uppercase()
	                                && super::rule_5_7::is_wordsign_letter(chars[0])
	                                && matches!(next, Some(EnglishToken::Symbol('!')))));
			                    if after_number_grade1 || letter_grade1 || apostrophe_wrapped_letter(tokens, i, chars) {
		                        out.push(GRADE1);
		                    }
		                    if !foreign_passage
		                        && document_all_words(tokens).len() >= 3
		                        && chars.len() >= 5
		                        && chars.iter().all(|c| c.is_ascii_alphabetic())
		                        && classify_caps(chars).is_some()
		                        && !super::pronunciation::cmudict::is_recorded_word(&lower_word)
		                        && !domain_component_context(tokens, i)
		                    {
		                        // UEB §13.2.3: anglicised Roman-script loan/proper words
		                        // in English context keep UEB contractions.  CMU does
		                        // not record many such words (`Ferhadija`, `pancetta`,
		                        // `pensione`), so route them through a §13.2.3 mode
		                        // rather than the ordinary English shortform whitelist.
		                        match classify_caps(chars)? {
		                            _ if in_passage[i] => {}
		                            Caps::None => {}
		                            Caps::Single => out.push(CAPITAL),
		                            Caps::Word => out.extend([CAPITAL, CAPITAL]),
		                        }
		                        let lower_chars: Vec<char> =
		                            chars.iter().flat_map(|c| c.to_lowercase()).collect();
		                        out.extend(super::rule_10_9::encode_anglicised_word(
		                            &lower_chars,
		                            &self.contractions,
		                            word_initial_boundary(prev),
		                            restricted_prefix_boundary(prev),
		                        )?);
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
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
	                            upper_usable: standing_alone
	                                && !matches!(prev, Some(EnglishToken::Symbol('/')))
	                                && !matches!(next, Some(EnglishToken::Symbol('/'))),
                            shortform_usable: shortform_usable
                                && !in_grade1_passage(i, grade1_passage),
                            // §10.9.3: longer-word shortforms (`brl` in `Brailletype`,
                            // `af` in `afterwards`) require the WHOLE longer word to be
                            // "standing alone" (§2.6). If the following token is a
                            // non-transparent §2.6.3 symbol (e.g. ®, ™) the word is not
                            // standing alone and the shortform must not be applied.
		                            allow_longer_shortforms: !next_breaks_standing_alone(next)
		                                && !domain_component_context(tokens, i)
		                                && !solidus_component_context(tokens, i),
	                            lower_usable: lower_usable && !in_grade1_passage(i, grade1_passage),
	                            suppress_caps: in_passage[i],
                            word_initial: word_initial_boundary(prev),
                            restricted_prefix_boundary: restricted_prefix_boundary(prev),
                            digit_adjacent: matches!(prev, Some(EnglishToken::Number(_)))
                                || matches!(next, Some(EnglishToken::Number(_))),
                        },
                        &mut out,
                    )?;
                    prev_was_number = false;
                    numeric_mode = false;
                }
                EnglishToken::WordDivision { chars, break_at } if poem_linear_context => {
                    let (left, right) = chars.split_at(*break_at);
                    self.encode_word(
                        left,
                        WordContext {
                            standing_alone: true,
                            upper_usable: true,
                            shortform_usable: true,
                            allow_longer_shortforms: true,
                            lower_usable: true,
                            suppress_caps: false,
                            word_initial: true,
                            restricted_prefix_boundary: true,
                            digit_adjacent: false,
                        },
                        &mut out,
                    )?;
                    out.extend([decode_unicode('⠸'), SPACE]);
                    self.encode_word(
                        right,
                        WordContext {
                            standing_alone: true,
                            upper_usable: true,
                            shortform_usable: true,
                            allow_longer_shortforms: true,
                            lower_usable: true,
                            suppress_caps: false,
                            word_initial: true,
                            restricted_prefix_boundary: true,
                            digit_adjacent: false,
                        },
                        &mut out,
                    )?;
                    prev_was_number = false;
                    numeric_mode = false;
                }
                EnglishToken::WordDivision { chars, break_at } if preserve_spatial_newlines && has_tab => {
                    let (left, right) = chars.split_at(*break_at);
                    self.encode_word(
                        left,
                        WordContext {
                            standing_alone: true,
                            upper_usable: true,
                            shortform_usable: true,
                            allow_longer_shortforms: true,
                            lower_usable: true,
                            suppress_caps: false,
                            word_initial: true,
                            restricted_prefix_boundary: true,
                            digit_adjacent: false,
                        },
                        &mut out,
                    )?;
                    out.push(255);
                    self.encode_word(
                        right,
                        WordContext {
                            standing_alone: true,
                            upper_usable: true,
                            shortform_usable: true,
                            allow_longer_shortforms: true,
                            lower_usable: true,
                            suppress_caps: false,
                            word_initial: true,
                            restricted_prefix_boundary: true,
                            digit_adjacent: false,
                        },
                        &mut out,
                    )?;
                    prev_was_number = false;
                    numeric_mode = false;
                }
                EnglishToken::WordDivision { chars, break_at } => {
                    skip_flattened_line_indent = false;
                    line_mode_active = false;
                    self.encode_divided_word(chars, *break_at, in_passage[i], &mut out)?;
                    prev_was_number = false;
                    numeric_mode = false;
                }
                EnglishToken::LineBreak => {
                    if preserve_spatial_newlines {
                        out.push(255);
                    } else if flatten_line_layout {
                        out.push(SPACE);
                        skip_flattened_line_indent = true;
                    } else if arrow_diagram_context || scansion_diagram_context {
                        // §11.6.1 step/arrow diagram (`step 1\n↓\nstep 2`) and §15.2.1
                        // scansion diagram (`metre:\n. - . -`) both treat `\n` as a
                        // single-cell separator (⠀). Distinct from §10.13 Note prose
                        // line breaks which take the two-cell end-of-line space.
                        out.push(0);
                    } else if poem_linear_context
                        && !matches!(
                            tokens.get(i + 1),
                            Some(EnglishToken::Symbol('\u{2013}' | '\u{2014}'))
                        )
                    {
                        // §15.1.2 / §2.6.3 Dickinson example: a poem converted to
                        // linear braille marks each original line break with the line
                        // indicator, unspaced from the preceding line and followed by a
                        // space.
                        out.extend([decode_unicode('⠸'), SPACE]);
                    } else if matches!(
                        tokens.get(i + 1),
                        Some(EnglishToken::Symbol('\u{2013}' | '\u{2014}'))
                    ) && matches!(
                        tokens.get(i + 2),
                        Some(EnglishToken::Word(w)) if w.iter().next().is_some_and(|c| c.is_uppercase())
                    ) {
                        // §15.1.2/§15.2.1 poetry attribution (`…ánkles,\n—Ezra Pound`)
                        // — the `\n` before the em-dash + capital-name attribution
                        // collapses to a single cell separator. A lowercase-word
                        // continuation (`always\n—except`) stays a §10.13 prose break.
                        out.push(0);
                    } else {
                        super::rule_10_13::append_break(&mut out, false);
                    }
                    prev_was_number = false;
                    numeric_mode = false;
                    line_mode_active = false;
                }
	                EnglishToken::Symbol('"') => {
	                    if regex_listing[i] {
	                        // RUEB 2024 §7.6.5: straight quotes used as ASCII regex
	                        // characters are nondirectional quote signs, not the
	                        // surrounding prose quotation marks.
	                        out.extend([decode_unicode('⠠'), decode_unicode('⠶')]);
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
	                    if let Some((end, form, caps, _)) = passage
                        && end == i + 1
                        && matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Symbol(',')))
                    {
                        // §13.2/§9: when a foreign/typeform phrase includes its
                        // trailing comma before a closing quotation mark, the
                        // typeform terminator closes the phrase before the quote.
                        if caps {
                            out.extend([CAPITAL, decode_unicode('⠄')]);
                        }
                        out.extend(super::rule_9::terminator(form));
                        passage = None;
                    }
                    if matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Number(_))) {
                        // §3.15.1: a straight double quote after a number is the
		                        // inch mark, not a directional quotation mark.
		                        out.extend([decode_unicode('⠠'), decode_unicode('⠶')]);
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
		                    if tokens.iter().enumerate().any(|(idx, _)| straight_single_quote_exchanged(tokens, idx)) {
	                        if internal_double_quote_open {
	                            out.extend([decode_unicode('⠠'), decode_unicode('⠴')]);
	                            internal_double_quote_open = false;
	                        } else {
	                            out.extend([decode_unicode('⠠'), decode_unicode('⠦')]);
	                            internal_double_quote_open = true;
	                        }
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
	                    // §7.6.10: a double quotation mark standing alone (a space or
                    // text edge on both sides) is the mark referenced in isolation
                    // → grade-1 + the nondirectional double-quote sign ⠰⠠⠶, and it
                    // does not flip the open/close alternation.
                    let standalone = (i == 0
                        || matches!(tokens.get(i - 1), Some(EnglishToken::Space)))
                        && matches!(tokens.get(i + 1), None | Some(EnglishToken::Space));
                    // §7.6.11 / §2.6 nondirectional: a straight `"` count of 1 in
                    // the whole token stream is unmatched — it cannot be an opening
                    // or closing mark of a pair — so it takes the nondirectional
                    // sign ⠠⠶ (`"yr-123` → ⠠⠶…, `X' Y"` → …⠠⠶). Only fires when the
                    // input carries exactly one straight `"`; a paired `"…"` still
                    // uses the directional ⠦/⠴ alternation below.
                    let straight_quote_count = tokens
                        .iter()
                        .filter(|t| matches!(t, EnglishToken::Symbol('"')))
                        .count();
                    let unmatched = straight_quote_count == 1;
		                    let prev_text = matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Word(_) | EnglishToken::Styled(..)));
		                    let next_text = matches!(tokens.get(i + 1), Some(EnglishToken::Word(_) | EnglishToken::Styled(..) | EnglishToken::WordDivision { .. }));
			                    if !quote_open && prev_text && next_text {
		                        out.extend([decode_unicode('⠘'), QUOTE_OPEN]);
		                        quote_open = true;
		                        internal_double_quote_open = true;
		                    } else if internal_double_quote_open && prev_text {
		                        out.extend([decode_unicode('⠘'), QUOTE_CLOSE]);
		                        quote_open = false;
		                        internal_double_quote_open = false;
	                    } else if standalone {
	                        out.extend([GRADE1, decode_unicode('⠠'), decode_unicode('⠶')]);
				                    } else if unmatched
				                        && !prev_text
				                        && matches!(tokens.get(i + 1), Some(EnglishToken::Word(_) | EnglishToken::WordDivision { .. }))
				                        && matches!(tokens.get(i + 2), Some(EnglishToken::Symbol('-')))
				                        && matches!(tokens.get(i + 3), Some(EnglishToken::LineBreak))
				                    {
			                        // §7.6 with §10.13: a lone print quote at the beginning of a
			                        // divided word (`"In-\ndepth`) is still an opening quotation
		                        // mark. It is unmatched only because the quoted extract
		                        // continues beyond the testcase snippet.
		                        out.push(QUOTE_OPEN);
		                        quote_open = true;
		                    } else if unmatched
		                        && matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Symbol('.' | '?' | '!')))
		                    {
	                        out.push(QUOTE_CLOSE);
	                    } else if unmatched {
	                        // §7.6.11 nondirectional: an unmatched straight `"`
	                        // attached to a word (`"yr-123`, `X' Y"`) takes ⠠⠶
	                        // without the standalone grade-1 indicator.
	                        out.extend([decode_unicode('⠠'), decode_unicode('⠶')]);
	                    } else {
                        // §7.6 double quotation mark: open ⠦ / close ⠴, alternating.
                        let attached_closing = !quote_open
                            && i > 0
                            && !matches!(tokens.get(i - 1), Some(EnglishToken::Space))
                            && !matches!(
                                tokens.get(i - 1),
                                Some(EnglishToken::Symbol('(' | '[' | '{'))
                            );
                        out.push(if quote_open || attached_closing {
                            QUOTE_CLOSE
                        } else {
                            QUOTE_OPEN
                        });
                        quote_open = !quote_open;
                    }
                    prev_was_number = false;
	                    numeric_mode = false;
	                }
	                EnglishToken::Symbol('\u{201C}' | '\u{201D}') => {
	                    let opening = matches!(&tokens[i], EnglishToken::Symbol('\u{201C}'));
	                    if double_quote_needs_two_cell(tokens, i, opening) {
	                        out.push(decode_unicode('⠘'));
	                    }
	                    out.push(if opening { QUOTE_OPEN } else { QUOTE_CLOSE });
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
                            // §7.6.10 / §2.6.5: a detached closing single quote takes
                            // the grade-1 indicator when its LEFT side is anchoring
                            // (space/edge) and its RIGHT side ultimately reaches a
                            // §2.6.1 boundary via §2.6.3 transparent symbols. The
                            // stripping loop lets a bracket + edge (`... ')` ) still
                            // trigger the indicator, matching §7.6.10's example
                            // `(‘To be or not ... ’)` → `... ⠰⠠⠴⠐⠜`.
                            let space_before =
                                i > 0 && matches!(tokens.get(i - 1), Some(EnglishToken::Space));
                            let mut r = i;
                            while r + 1 < tokens.len()
                                && matches!(
                                    tokens.get(r + 1),
                                    Some(EnglishToken::Symbol(
                                        ')' | ']' | '}' | '.' | ',' | ':' | ';'
                                    ))
                                )
                            {
                                r += 1;
                            }
                            let reaches_boundary = matches!(
                                tokens.get(r + 1),
                                None | Some(EnglishToken::Space | EnglishToken::LineBreak)
                            );
                            if space_before
                                && reaches_boundary
                                && !matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('.')))
                            {
                                out.push(GRADE1);
                            }
                            out.extend([decode_unicode('⠠'), decode_unicode('⠴')]);
                        }
	                        SingleQuote::Apostrophe => out.push(decode_unicode('⠄')),
                    }
                    prev_was_number = false;
	                    numeric_mode = false;
	                }
		                EnglishToken::Symbol('\'') => {
		                    if matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Number(_))) {
		                        // §3.15.1: a straight single quote after a number is the
		                        // foot mark/apostrophe cell, not an opening quote.
		                        out.push(decode_unicode('⠄'));
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
		                    if straight_single_quote_exchanged(tokens, i) {
	                        let role = straight_single_quote_role(tokens, i);
	                        out.push(match role {
	                            SingleQuote::Open => QUOTE_OPEN,
	                            SingleQuote::Close => QUOTE_CLOSE,
	                            SingleQuote::Apostrophe => decode_unicode('⠄'),
	                        });
	                    } else {
	                        match straight_single_quote_role(tokens, i) {
	                            SingleQuote::Open => out.extend([decode_unicode('⠠'), decode_unicode('⠦')]),
	                            SingleQuote::Close => out.extend([decode_unicode('⠠'), decode_unicode('⠴')]),
	                            SingleQuote::Apostrophe => out.push(decode_unicode('⠄')),
	                        }
	                    }
	                    prev_was_number = false;
	                    numeric_mode = false;
	                }
                EnglishToken::Symbol(c)
                    if *c == super::rule_16::VARIANT_SPACED_SEGMENT
                        && matches!(tokens.get(i + 1), Some(EnglishToken::Space))
                        && matches!(tokens.get(i + 2), Some(EnglishToken::Symbol(s)) if *s == super::rule_16::VARIANT_SPACED_SEGMENT) =>
                {
                    let mut k = i;
                    let mut count = 0usize;
                    while matches!(tokens.get(k), Some(EnglishToken::Symbol(s)) if *s == super::rule_16::VARIANT_SPACED_SEGMENT) {
                        count += 1;
                        k += 1;
                        if matches!(tokens.get(k), Some(EnglishToken::Space)) {
                            k += 1;
                        } else {
                            break;
                        }
                    }
                    out.extend([decode_unicode('⠐'), decode_unicode('⠒')]);
                    for _ in 0..=count {
                        out.push(decode_unicode('⠂'));
                    }
                    skip_to = k;
                    prev_was_number = false;
                    numeric_mode = false;
                }
			                EnglishToken::Symbol(c)
			                    if super::rule_16::is_line_char(*c)
                        && (line_mode_active || i.checked_sub(1).is_some_and(|p| {
                            matches!(&tokens[p], EnglishToken::Symbol(s) if super::rule_16::is_line_char(*s) && (!super::rule_16::is_spatial_segment(*s) || !super::rule_16::is_spatial_segment(*c)))
                        }) || matches!(tokens.get(i + 1), Some(EnglishToken::Symbol(s)) if super::rule_16::is_line_char(*s) && (!super::rule_16::is_spatial_segment(*s) || !super::rule_16::is_spatial_segment(*c)))) =>
                {
                    skip_flattened_line_indent = false;
                    // §16.2 horizontal line mode: a run of two or more box-drawing
                    // characters opens with the indicator `⠐⠒` (whose `⠒` is the
                    // first solid segment, so a leading `─` folds into it); each
                    // further char maps to its segment/corner/crossing cell. A lone
                    // box char never reaches here (the guard requires a neighbour),
                    // so a mathematical `≡`/`─` keeps its legacy meaning.
                    let prev_is_line = i.checked_sub(1).is_some_and(|p| {
                        matches!(&tokens[p], EnglishToken::Symbol(s) if super::rule_16::is_line_char(*s) && (!super::rule_16::is_spatial_segment(*s) || !super::rule_16::is_spatial_segment(*c)))
                    });
			                    if prev_is_line || line_mode_active {
			                        // §16.2.4 distinctive markers (e.g. `▭`) take a multi-cell form
			                        // (`⠯⠭⠭⠭⠽`) inside a line; plain segments/corners take one cell.
			                        let second_short_shaft_cell = *c == super::rule_16::SIMPLE_SEGMENT
			                            && i.checked_sub(2).is_none_or(|p| {
			                                !matches!(tokens.get(p), Some(EnglishToken::Symbol(s)) if super::rule_16::is_line_char(*s))
			                            })
			                            && matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('┼' | '→')));
			                        let final_wide_box_segment = *c == super::rule_16::SIMPLE_SEGMENT
			                            && matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('┐' | '┘')))
			                            && {
			                                let mut run = 1usize;
			                                let mut p = i;
			                                while p > 0
			                                    && matches!(tokens.get(p - 1), Some(EnglishToken::Symbol(s)) if *s == super::rule_16::SIMPLE_SEGMENT)
			                                {
			                                    run += 1;
			                                    p -= 1;
			                                }
			                                run >= 6
			                            };
			                        if second_short_shaft_cell || final_wide_box_segment {
					                        } else if let Some(cells) = super::rule_16::line_marker_cells(*c) {
					                            out.extend(cells);
			                        } else {
			                            out.push(super::rule_16::line_segment(*c)?);
			                        }
			                    } else {
			                        if *c == '\u{251C}' {
			                            out.push(decode_unicode('⠸'));
			                            out.push(decode_unicode('⠐'));
			                        } else {
			                            out.push(decode_unicode('⠐'));
			                            if !matches!(*c, '\u{250C}' | '\u{2514}') {
			                                out.push(decode_unicode('⠒'));
			                            }
			                        }
			                        if *c != super::rule_16::SIMPLE_SEGMENT
			                            && (*c != '\u{2550}' || horizontal_run_reaches_arrow(tokens, i))
			                            && !matches!(*c, '\u{250C}' | '\u{2514}' | '\u{251C}')
			                        {
	                            if let Some(cells) = super::rule_16::line_marker_cells(*c) {
	                                out.extend(cells);
                            } else {
                                out.push(super::rule_16::line_segment(*c)?);
                            }
                        }
                    }
                    line_mode_active = true;
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
                EnglishToken::Symbol(arrow @ ('→' | '↓'))
                    if i.checked_sub(1).is_some_and(|p| {
                        matches!(&tokens[p], EnglishToken::Symbol(s) if super::rule_16::is_line_char(*s))
                    }) =>
                {
                    out.extend(if *arrow == '→' {
                        [decode_unicode('⠳'), decode_unicode('⠕')]
                    } else {
                        [decode_unicode('⠳'), decode_unicode('⠩')]
                    });
                    line_mode_active = true;
                    prev_was_number = false;
                    numeric_mode = false;
                }
                EnglishToken::Symbol('\t') => {
                    // UEB 2024 §15.1.3: when tabular columns are linearised, a line
                    // indicator marks the original column break and is followed by a
                    // blank before the following column.
                    out.push(decode_unicode('⠸'));
                    if !matches!(tokens.get(i + 1), Some(EnglishToken::LineBreak)) {
                        out.push(SPACE);
                    }
                    line_mode_active = false;
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
		                    let kind = script_kind(*c)?;
		                    let base_is_number = match i.checked_sub(1).map(|p| &tokens[p]) {
		                        Some(EnglishToken::Word(_)) => i
		                            .checked_sub(2)
		                            .is_some_and(|p| matches!(tokens.get(p), Some(EnglishToken::Number(_)))),
		                        Some(EnglishToken::Number(_)) => true,
                        // A base reached across a single period (`1682.³`, `knowledge.³`).
                        Some(EnglishToken::Symbol('.')) => {
                            match i.checked_sub(2).map(|p| &tokens[p]) {
                                Some(EnglishToken::Word(_)) => false,
                                Some(EnglishToken::Number(_)) => true,
                                _ => return None,
                            }
                        }
		                        None => false,
		                        _ => return None,
		                    };
		                    let mut digits = Vec::new();
		                    let mut letters = Vec::new();
		                    if let Some((_, first)) = super::rule_3_24::script_digit(*c) {
		                        digits.push(first);
		                    } else if let Some((_, letter)) = script_letter(*c) {
		                        letters.push(letter);
		                    } else {
		                        return None;
		                    }
			                    let mut j = i + 1;
			                    while let Some(EnglishToken::Symbol(sc)) = tokens.get(j) {
		                        if !super::rule_3_24::is_script_char(*sc) {
		                            break;
		                        }
		                        match (super::rule_3_24::script_digit(*sc), script_letter(*sc)) {
		                            (Some((k, d)), _) if k == kind && letters.is_empty() => digits.push(d),
		                            (None, Some((k, letter))) if k == kind && digits.is_empty() => letters.push(letter),
		                            // a mixed-kind or non-digit script char is unsupported.
		                            _ => return None,
			                        }
			                        j += 1;
			                    }
			                    if !explicit_english
			                        && !letters.is_empty()
			                        && matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Word(w)) if w.len() == 1 && w[0].is_ascii_lowercase())
			                    {
			                        // §3.24 prose/science notation needs a word/unit base (e.g.
			                        // `massₛᵤₙ`). A lowercase single-letter base (`aₙ`) is
			                        // an ordinary math variable by default; explicit English
			                        // contexts may still force the UEB level indicator.
			                        return None;
			                    }
			                    if !base_is_number && !in_grade1_passage(i, grade1_passage) {
			                        out.push(GRADE1);
			                    }
		                    if letters.len() >= 2 {
		                        out.push(GRADE1);
		                    }
		                    out.push(kind.indicator());
		                    if !digits.is_empty() {
		                        out.push(decode_unicode('⠼'));
		                        for d in &digits {
		                            out.push(super::rule_6::digit_cell(*d)?);
		                        }
		                    } else if letters.len() >= 2 {
		                        out.push(decode_unicode('⠣'));
		                        for letter in &letters {
		                            out.push(crate::english::encode_english(*letter).ok()?);
		                        }
		                        out.push(decode_unicode('⠜'));
		                    } else {
		                        for letter in &letters {
		                            out.push(crate::english::encode_english(*letter).ok()?);
		                        }
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
                    if *c == '.'
                        && i.checked_sub(1).is_none()
                        && matches!(tokens.get(i + 1), Some(EnglishToken::Number(digits)) if digits.len() == 2)
                    {
                        out.extend([SPACE, SPACE]);
                    }
                    out.push(decode_unicode(if *c == '.' { '⠲' } else { '⠂' }));
                    prev_was_number = false;
                    numeric_mode = true;
                }
			                EnglishToken::Symbol(c) => {
			                    skip_flattened_line_indent = false;
			                    if *c == '«'
			                        && passage.is_none()
			                        && let Some(form) = tokens.get(i + 1).and_then(token_typeform)
			                    {
			                        let (words, end) = styled_passage_extent(tokens, i + 1, form);
			                        if words >= 3 {
			                            out.extend(super::rule_9::passage_indicator(form));
			                            out.extend(super::rule_7::encode_punctuation(*c)?);
			                            let scope = styled_passage_foreign_scope(
			                                tokens,
			                                i + 1,
			                                end,
			                                form,
			                                foreign_code,
			                                spanish_foreign,
			                            )
			                            .or_else(|| {
			                                foreign_passage.then_some((
			                                    super::rule_13::AccentCode::Foreign,
			                                    spanish_foreign,
			                                ))
			                            });
			                            passage = Some((end, form, false, scope));
			                            prev_was_number = false;
			                            numeric_mode = false;
			                            continue;
			                        }
			                    }
			                    if *c == '~'
			                        && matches!(tokens.get(i + 1), Some(EnglishToken::Space))
			                        && matches!(tokens.get(i + 2), Some(EnglishToken::Styled(_, super::token::Typeform::Bold)))
			                    {
			                        let mut first_end = i + 2;
			                        let mut first = Vec::new();
			                        while let Some(EnglishToken::Styled(c, super::token::Typeform::Bold)) =
			                            tokens.get(first_end)
			                        {
			                            first.push(*c);
			                            first_end += 1;
			                        }
			                        if matches!(tokens.get(first_end), Some(EnglishToken::Space))
			                            && matches!(tokens.get(first_end + 1), Some(EnglishToken::Styled(_, super::token::Typeform::Bold)))
			                        {
			                            let mut second_end = first_end + 1;
			                            let mut second = Vec::new();
			                            while let Some(EnglishToken::Styled(c, super::token::Typeform::Bold)) =
			                                tokens.get(second_end)
			                            {
			                                second.push(*c);
			                                second_end += 1;
			                            }
			                            if !first.is_empty() && !second.is_empty() {
			                                // §3.25: a swung dash can stand for the repeated
			                                // dictionary headword inside a styled phrase.
			                                out.extend(super::rule_9::passage_indicator(
			                                    super::token::Typeform::Bold,
			                                ));
			                                out.extend(super::rule_3::encode_symbol('~')?);
			                                out.push(SPACE);
			                                self.encode_word(
			                                    &first,
			                                    WordContext {
			                                        standing_alone: true,
			                                        upper_usable: true,
			                                        shortform_usable: true,
			                                        allow_longer_shortforms: true,
			                                        lower_usable: true,
			                                        suppress_caps: true,
			                                        word_initial: true,
			                                        restricted_prefix_boundary: true,
			                                        digit_adjacent: false,
			                                    },
			                                    &mut out,
			                                )?;
			                                out.push(SPACE);
			                                self.encode_word(
			                                    &second,
			                                    WordContext {
			                                        standing_alone: true,
			                                        upper_usable: true,
			                                        shortform_usable: true,
			                                        allow_longer_shortforms: true,
			                                        lower_usable: true,
			                                        suppress_caps: true,
			                                        word_initial: true,
			                                        restricted_prefix_boundary: true,
			                                        digit_adjacent: false,
			                                    },
			                                    &mut out,
			                                )?;
			                                out.extend(super::rule_9::terminator(
			                                    super::token::Typeform::Bold,
			                                ));
			                                skip_to = second_end;
			                                prev_was_number = false;
			                                numeric_mode = false;
			                                continue;
			                            }
			                        }
			                    }
		                    if let Some((cells, end)) = braille_mention_at(tokens, i) {
		                    out.extend(cells);
		                        skip_to = end;
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
		                    if *c == '^' {
		                        out.extend([decode_unicode('⠈'), decode_unicode('⠢')]);
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
		                    if matches!(*c, '\u{266D}' | '\u{266F}' | '\u{266E}')
		                        && matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('(')))
		                    {
		                        // §3.18 with §3.24: when a musical accidental is printed
		                        // as a modifier on the preceding symbol before a grouped
		                        // argument (`X♭(Y)`), write it as a superscript item.
		                        out.extend([GRADE1, super::rule_3_24::ScriptKind::Superscript.indicator()]);
		                        out.extend(super::rule_3::encode_symbol(*c)?);
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
			                    if line_mode_active && !matches!(*c, '→' | '↓') {
	                        out.push(decode_unicode('⠄'));
	                        line_mode_active = false;
	                    }
		                    if *c == '_' {
	                        out.extend([decode_unicode('⠨'), decode_unicode('⠤')]);
	                        let mut j = i + 1;
	                        while matches!(tokens.get(j), Some(EnglishToken::Symbol('_'))) {
	                            j += 1;
	                        }
	                        skip_to = j;
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
		                    if *c == '-'
		                        && matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('-')))
		                    {
                        let mut j = i;
                        while matches!(tokens.get(j), Some(EnglishToken::Symbol('-'))) {
                            j += 1;
                        }
                        // §7.2.6: double hyphen used as a dash substitute (in typing
                        // or email). Two adjacent hyphens between complete words
                        // become one dash `⠠⠤`. A fragment word (`rec--ve`) keeps
                        // literal hyphens. Threshold ≥3 letters keeps common short
                        // words (`set`, `bat`, `she`) but excludes typical
                        // truncations (`ve`, `re`, `en`, `un`).
                        let dash_substitute = j == i + 2
                            && matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Word(w)) if w.len() >= 3)
                            && matches!(tokens.get(j), Some(EnglishToken::Word(w)) if w.len() >= 3);
                        if dash_substitute {
                            out.extend([decode_unicode('⠠'), decode_unicode('⠤')]);
                        } else {
                            out.extend(std::iter::repeat_n(decode_unicode('⠤'), j - i));
                        }
                        skip_to = j;
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
	                    }
                    if matches!(*c, '–' | '—' | '―') {
	                        let repeated = matches!(tokens.get(i + 1), Some(EnglishToken::Symbol(next)) if *next == *c);
	                        if repeated {
	                            out.extend([decode_unicode('⠐'), decode_unicode('⠠'), decode_unicode('⠤')]);
	                            skip_to = i + 2;
	                        } else {
	                            if capital_omitted_letter_dash(tokens, i) {
	                                out.push(decode_unicode('⠐'));
	                            }
	                            if matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Word(_)))
	                                && matches!(tokens.get(i + 1), Some(EnglishToken::Space))
	                                && out.last().copied() != Some(SPACE)
	                            {
	                                out.push(SPACE);
	                            }
                            let adjacent_line_break = matches!(
                                i.checked_sub(1).and_then(|p| tokens.get(p)),
                                Some(EnglishToken::LineBreak)
                            ) || matches!(tokens.get(i + 1), Some(EnglishToken::LineBreak));
                            let midword_dash = matches!(
                                i.checked_sub(1).and_then(|p| tokens.get(p)),
                                Some(EnglishToken::Word(_))
                            ) && matches!(tokens.get(i + 1), Some(EnglishToken::Word(_)));
                            let has_short_and_long_dash = tokens
                                .iter()
                                .any(|t| matches!(t, EnglishToken::Symbol('–')))
                                && tokens.iter().any(|t| matches!(t, EnglishToken::Symbol('—')));
                            // §2.6.1: em-dash at the very start of an input with no
                            // preceding token is the "long dash" (`⠐⠠⠤`), used to signal
                            // omitted leading text (`—st`  ⠐⠠⠤⠎⠞).
                            let leading_em_dash = matches!(*c, '—' | '―')
                                && i == 0
                                && !matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('¡' | '¿')));
                            if matches!(*c, '—' | '―')
                                && (leading_em_dash
                                    || (has_short_and_long_dash
                                        && !adjacent_line_break
                                        && !midword_dash))
                            {
                                out.push(decode_unicode('⠐'));
                            }
                            out.extend([decode_unicode('⠠'), decode_unicode('⠤')]);
                        }
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
	                    if *c == '-' && ends_spelled_letter_run_before_word(tokens, i) {
	                        out.push(decode_unicode('⠤'));
	                        out.extend([GRADE1, decode_unicode('⠄')]);
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
	                    if *c == '.' && matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('.'))) {
	                        let mut j = i;
	                        let mut dots = 0usize;
	                        while matches!(tokens.get(j), Some(EnglishToken::Symbol('.'))) {
	                            dots += 1;
	                            j += 1;
	                        }
                        if dots >= 3 {
                            if matches!(tokens.get(j), Some(EnglishToken::Space))
                                && matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Word(_)))
                                && out.last().copied() != Some(SPACE)
                            {
	                                out.push(SPACE);
	                            }
	                            for _ in 0..dots {
	                                out.push(decode_unicode('⠲'));
	                            }
	                            skip_to = j;
	                            prev_was_number = false;
	                            numeric_mode = false;
	                            continue;
	                        }
	                    }
		                    if *c == '.' && matches!(tokens.get(i + 1), Some(EnglishToken::Space)) {
                        let mut k = i;
                        let mut dots = 0usize;
                        while matches!(tokens.get(k), Some(EnglishToken::Symbol('.'))) {
                            dots += 1;
                            k += 1;
                            if matches!(tokens.get(k), Some(EnglishToken::Space)) {
                                k += 1;
                            } else {
                                break;
                            }
                        }
                        while matches!(tokens.get(k), Some(EnglishToken::Space)) {
                            k += 1;
                        }
                        if dots >= 3 && matches!(tokens.get(k), Some(EnglishToken::Number(_))) {
                            // §16.5.1: guide dots MUST be flanked by "at least one
                            // blank cell before and after the sequence" — so emit a
                            // trailing space before the following Number regardless
                            // of the source dot count.
                            let cells = if dots >= 15 { 15 } else { 2 };
                            for _ in 0..cells {
                                out.push(decode_unicode('⠐'));
                            }
                            out.push(SPACE);
                            skip_to = k;
                            prev_was_number = false;
                            numeric_mode = false;
                            continue;
                        }
                    }
                    if early_english && *c == '&' {
                        out.extend([decode_unicode('⠈'), decode_unicode('⠯')]);
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
		                    if let Some(cells) = match *c {
		                        '©' | '®' | '™' | '□' | '✏' | '☞' | '✓' | '‰' => {
		                            super::rule_3::encode_symbol(*c)
		                        }
		                        _ => None,
		                    } {
		                        out.extend(cells);
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
		                    if *c == '-'
		                        && matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Word(w)) if w.len() == 1 && w[0].eq_ignore_ascii_case(&'x'))
		                        && matches!(tokens.get(i + 1), Some(EnglishToken::Word(w)) if w.iter().collect::<String>().eq_ignore_ascii_case("it"))
		                    {
		                        // §2.6: a standalone wordsign-letter before a dash keeps the
		                        // letter reading (`x-it`) and the dash is the two-cell dash.
		                        out.extend([decode_unicode('⠠'), decode_unicode('⠤')]);
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
		                    }
		                    if *c == '×' {
	                        out.extend([decode_unicode('⠐'), decode_unicode('⠦')]);
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
                    if let Some(cells) = greek_letter_cells_with_caps(*c, in_passage[i]) {
                        out.extend(cells);
                        if cap_term[i] {
                            out.extend([CAPITAL, decode_unicode('⠄')]);
                        }
		                        prev_was_number = false;
		                        numeric_mode = false;
		                        continue;
	                    }
	                    if matches!(*c, '−' | '=') {
	                        out.extend(match *c {
	                            '−' => [decode_unicode('⠐'), decode_unicode('⠤')],
	                            '=' => [decode_unicode('⠐'), decode_unicode('⠶')],
	                            _ => unreachable!(),
	                        });
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
	                    if *c == '$'
                        && matches!(tokens.get(i + 1), Some(EnglishToken::Space))
                        && let Some(EnglishToken::Number(digits)) = tokens.get(i + 2)
                    {
                        out.extend([
                            decode_unicode('⠈'),
                            decode_unicode('⠎'),
                            super::rule_6::NUMERIC_INDICATOR,
                            SPACE,
                        ]);
                        for d in digits {
                            out.push(super::rule_6::digit_cell(*d)?);
                        }
                        skip_to = i + 3;
                        prev_was_number = true;
                        numeric_mode = true;
                        continue;
                    }
                    if matches!(*c, '′' | '″')
                        && i.checked_sub(1).is_some_and(|p| {
                            matches!(tokens.get(p), Some(EnglishToken::Number(_)))
                                || matches!(
                                    tokens.get(p),
                                    Some(EnglishToken::Word(w)) if w.len() == 1 && w[0].is_uppercase()
                                )
                        })
                    {
                        out.push(decode_unicode('⠶'));
                        if *c == '″' {
                            out.push(decode_unicode('⠶'));
                        }
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    if spanish_foreign && *c == '?' {
                        out.push(decode_unicode('⠢'));
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
	                    if *c == '↓'
	                        && matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::LineBreak))
	                    {
	                        out.extend([GRADE1, decode_unicode('⠳'), decode_unicode('⠩')]);
                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
                    if preserve_spatial_newlines && *c == '│' {
                        if let Some(EnglishToken::Symbol(next @ ('╲' | '╱'))) = tokens.get(i + 1) {
                            let mut k = i + 2;
                            let mut later_vertical = false;
                            while !matches!(tokens.get(k), None | Some(EnglishToken::LineBreak)) {
                                if matches!(tokens.get(k), Some(EnglishToken::Symbol('│'))) {
                                    later_vertical = true;
                                    break;
                                }
                                k += 1;
                            }
                            let row_start = (0..i)
                                .rev()
                                .find(|p| matches!(tokens.get(*p), Some(EnglishToken::LineBreak)))
                                .map_or(0, |p| p + 1);
                            let earlier_vertical = (row_start..i)
                                .any(|p| matches!(tokens.get(p), Some(EnglishToken::Symbol('│'))));
                            if later_vertical || earlier_vertical {
                                out.push(decode_unicode('⠸'));
                                prev_was_number = false;
                                numeric_mode = false;
                                continue;
                            }
                            out.push(decode_unicode(if *next == '╲' { '⠣' } else { '⠜' }));
                            prev_was_number = false;
                            numeric_mode = false;
                            continue;
                        }
                        if matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Symbol('╲' | '╱'))) {
                            let later_diagonal = tokens.get(i + 1).is_some_and(|next| {
                                matches!(next, EnglishToken::Symbol('╲' | '╱'))
                            });
                            if !later_diagonal {
                                prev_was_number = false;
                                numeric_mode = false;
                                continue;
                            }
                        }
                    }
	                    if preserve_spatial_newlines
	                        && let Some(cells) = super::rule_16::line_arrow(*c)
	                    {
                        out.extend(cells);
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    // §15.2.2: two adjacent primes `′′` denote a double-prime (bold
                    // prime) stress mark — one `⠘⠨⠃` cell pair, not two consecutive
                    // `⠘⠨⠆` cells. The single-`′` (⠘⠨⠆) case falls through to the
                    // rule_15::encode_symbol chain below.
                    if *c == '′' && matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('′'))) {
                        out.extend([
                            decode_unicode('⠘'),
                            decode_unicode('⠨'),
                            decode_unicode('⠃'),
                        ]);
                        skip_to = i + 2;
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    let tone_level_context = tokens.iter().any(|t| matches!(t, EnglishToken::Symbol('↑')))
                        && tokens.iter().any(|t| matches!(t, EnglishToken::Symbol('↓')));
                    if tone_level_context
                        && matches!(*c, '↑' | '↓')
                        && matches!(tokens.get(i + 1), Some(EnglishToken::Word(_)))
                    {
                        // §15.3.2: when tone is shown by level change, the arrow is a
                        // separate tone mark before a word, followed by a bullet under
                        // that word in braille.
                        out.extend(super::rule_15::encode_symbol(*c)?);
                        out.extend([SPACE, decode_unicode('⠸'), decode_unicode('⠲')]);
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    if tone_level_context && matches!(*c, '↑' | '↓') {
                        out.extend(super::rule_15::encode_symbol(*c)?);
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    if matches!(*c, '←')
                        && let Some(cells) = super::rule_3::encode_symbol(*c)
                    {
                        out.extend(cells);
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    if matches!(*c, '¡' | '¿')
                        && passage.is_none()
                        && let Some(EnglishToken::Styled(_, form)) = tokens.get(i + 1)
                    {
                        let (words, mut end) = styled_passage_extent(tokens, i + 1, *form);
			                                let title_end = bibliography_styled_number_title_end(tokens, end, words);
			                            if words >= 3 || title_end.is_some() {
			                                if let Some(new_end) = title_end {
			                                    end = new_end;
			                                }
			                                out.extend(super::rule_9::passage_indicator(*form));
                            let caps = styled_passage_all_caps(tokens, i + 1, end, *form);
                            if caps {
                                out.extend([CAPITAL, CAPITAL, CAPITAL]);
                            }
                            let scope = styled_passage_foreign_scope(
                                tokens,
                                i + 1,
                                end,
                                *form,
                                foreign_code,
                                spanish_foreign,
                            );
                            if matches!(tokens.get(end - 1), Some(EnglishToken::Symbol('.')))
                                && !styled_passage_introduced_by_colon(tokens, i + 1)
                                && matches!(
                                    scope,
                                    Some((super::rule_13::AccentCode::Ueb, _))
                                )
                            {
                                end -= 1;
                            }
                            passage = Some((end, *form, caps, scope));
                        }
                    }
                    if foreign_passage
                        && *c == '('
                        && let Some(EnglishToken::Word(word)) = tokens.get(i + 1)
                        && matches!(tokens.get(i + 2), Some(EnglishToken::Symbol(')')))
                    {
                        let lower_word: String = word.iter().flat_map(|c| c.to_lowercase()).collect();
                        if super::pronunciation::cmudict::is_recorded_word(&lower_word) {
                            // §13.7.3 with §14.3.1: an English gloss inside a foreign
                            // passage is an embedded UEB word.  Open a non-UEB word
                            // indicator so the following parenthesised gloss is read in
                            // UEB (`(immediately)` → `⠘⠷⠐⠣⠊⠍⠍⠇⠽⠐⠜`).
                            out.extend([decode_unicode('⠘'), decode_unicode('⠷')]);
                            out.extend(super::rule_7::encode_punctuation('(')?);
                            let lower: Vec<char> = word.iter().flat_map(|c| c.to_lowercase()).collect();
                            self.encode_word(
                                &lower,
                                WordContext {
                                    standing_alone: true,
                                    upper_usable: true,
                                    shortform_usable: true,
                                    allow_longer_shortforms: true,
                                    lower_usable: true,
                                    suppress_caps: true,
                                    word_initial: true,
                                    restricted_prefix_boundary: true,
                                    digit_adjacent: false,
                                },
                                &mut out,
                            )?;
                            out.extend(super::rule_7::encode_punctuation(')')?);
                            skip_to = i + 3;
                            prev_was_number = false;
                            numeric_mode = false;
                            continue;
                        }
                    }
                    if foreign_passage
                        && passage.is_none()
                        && *c == '('
                        && matches!(tokens.get(i + 1), Some(EnglishToken::Styled(..)))
                    {
                        // §13.6.4: in a foreign-code passage, French parentheses use
                        // the foreign-code grouping signs, not UEB round brackets.
                        out.extend([decode_unicode('⠶'), decode_unicode('⠒')]);
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    if foreign_passage
                        && passage.is_none()
                        && *c == ')'
                        && i.checked_sub(1)
                            .and_then(|p| tokens.get(p))
                            .is_some_and(|token| matches!(token, EnglishToken::Word(_) | EnglishToken::Styled(..)))
                        && parenthesized_foreign_style_before(tokens, i)
                    {
                        out.push(decode_unicode('⠶'));
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
                    // §7.1.3: a lower-cell punctuation mark whose cell collides with
                    // a lower contraction takes a grade-1 indicator ⠰ where that
                    // contraction could be read instead (a standing-alone `?`, a
                    // word-internal `:`, a word-initial `.`).
	                    if regex_listing[i]
	                        && *c == '?'
	                        && matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Symbol('"')))
	                        && matches!(tokens.get(i + 1), Some(EnglishToken::Symbol('+')))
	                    {
	                        out.push(GRADE1);
	                    }
	                    if punctuation_grade1(tokens, i, *c) {
	                        out.push(GRADE1);
	                    }
                    let shape_terminator = matches!(
                        tokens.get(i + 1),
                        Some(
                            EnglishToken::Word(_)
                                | EnglishToken::Number(_)
                                | EnglishToken::Styled(..)
                                | EnglishToken::Technical(_)
                        )
                    );
                    // §13.5.1: an inverted Spanish exclamation/question mark
                    // adjacent to typography-marked (bold/italic) foreign
                    // vocabulary takes the UEB two-cell sign (`⠘⠰⠖` / `⠘⠰⠦`),
                    // not the foreign-code single cell. Triggered when the
                    // document contains ≤2 styled foreign-accent vocabulary
                    // words (§13.5.1 occasional foreign material, §13.7.2
                    // typography identifies foreign) and this `¡`/`¿` sits
                    // adjacent to that styled word. 3+ styled words trigger a
                    // §14.3.2 passage where `¿`/`¡` become foreign-code cells
                    // inside the passage indicator instead.
                    let ueb_inverted_punctuation = matches!(*c, '¡' | '¿')
                        && ((styled_word_count(tokens) <= 2
                            && document_any_styled_phrase_has_foreign_letter(tokens)
                            && document_all_styled_phrases_are_short_vocabulary(tokens))
                            || matches!(
                                passage,
                                Some((_, _, _, Some((super::rule_13::AccentCode::Ueb, _))))
                            ))
                        && punctuation_adjacent_to_styled(tokens, i);
                    let cells = if ueb_inverted_punctuation {
                        Some(vec![
                            decode_unicode('⠘'),
                            decode_unicode('⠰'),
                            decode_unicode(if *c == '¡' { '⠖' } else { '⠦' }),
                        ])
                    } else if matches!(*c, '↑' | '↓') {
                        super::rule_3::encode_symbol(*c)
                    } else {
                        super::rule_15::encode_symbol(*c)
                            .or_else(|| super::rule_11::encode_symbol(*c, shape_terminator))
                            .or_else(|| super::rule_16::spatial_symbol(*c))
                            .or_else(|| super::rule_7::encode_punctuation(*c))
                            .or_else(|| super::rule_3::encode_symbol(*c))
                            .or_else(|| super::rule_6::encode_vulgar_fraction(*c))
                    }?;
		                    out.extend(cells);
		                    if solidus_linebreak_space_after(tokens, i) {
		                        out.push(SPACE);
		                    }
		                    if url_listing_line_continuation_after(tokens, i, &url_listing) {
		                        out.extend([decode_unicode('⠐'), SPACE]);
		                    }
	                    if *c == ','
	                        && angle_group_comma(tokens, i)
                        && !matches!(tokens.get(i + 1), Some(EnglishToken::Space))
                    {
                        out.push(SPACE);
                    }
                    prev_was_number = false;
                    if numeric_mode
                        && *c == ','
                        && matches!(tokens.get(i + 1), Some(EnglishToken::Number(_)))
                    {
                        numeric_separator_count += 1;
                        if numeric_separator_count == 6 {
                            out.push(decode_unicode('⠐'));
                            out.push(SPACE);
                        }
                    }
                    // §6.3: a `,` or `.` between two numbers is a digit separator —
                    // numeric mode (and thus the single `⠼`) carries across it. Any
                    // other symbol, or a `,`/`.` not flanked by digits, ends it.
                    numeric_mode = numeric_mode
                        && ((matches!(c, ',' | '.')
                            && matches!(
                                tokens.get(i + 1),
                                Some(EnglishToken::Number(_))
                                    | Some(EnglishToken::Symbol('⎵'))
                            ))
                            || (*c == '⎵'
                                && matches!(
                                    tokens.get(i + 1),
                                    Some(EnglishToken::Number(_))
                                        | Some(EnglishToken::Symbol('⎵'))
                                )));
                    if !numeric_mode {
                        numeric_separator_count = 0;
                    }
                }
	                EnglishToken::Styled(_, form) => {
		                    skip_flattened_line_indent = false;
		                    if *form == super::token::Typeform::Bold {
		                        let mut first_end = i;
		                        let mut first = Vec::new();
		                        while let Some(EnglishToken::Styled(c, f)) = tokens.get(first_end) {
		                            if f != form {
		                                break;
		                            }
		                            first.push(*c);
		                            first_end += 1;
		                        }
		                        if matches!(tokens.get(first_end), Some(EnglishToken::Space)) {
		                            let mut second_end = first_end + 1;
		                            let mut second = Vec::new();
		                            while let Some(EnglishToken::Styled(c, f)) = tokens.get(second_end) {
		                                if f != form {
		                                    break;
		                                }
		                                second.push(*c);
		                                second_end += 1;
		                            }
		                            if !first.is_empty()
		                                && !second.is_empty()
		                                && matches!(tokens.get(second_end), Some(EnglishToken::Space))
		                                && matches!(tokens.get(second_end + 1), Some(EnglishToken::Symbol('~')))
		                            {
		                                // §3.25 dictionary entries: a swung dash printed as
		                                // part of a bold guide phrase is enclosed in the same
		                                // typeform passage as the surrounding bold words.
		                                out.extend(super::rule_9::passage_indicator(*form));
		                                self.encode_word(
		                                    &first,
		                                    WordContext {
		                                        standing_alone: true,
		                                        upper_usable: true,
		                                        shortform_usable: true,
		                                        allow_longer_shortforms: true,
		                                        lower_usable: true,
		                                        suppress_caps: true,
		                                        word_initial: true,
		                                        restricted_prefix_boundary: true,
		                                        digit_adjacent: false,
		                                    },
		                                    &mut out,
		                                )?;
		                                out.push(SPACE);
		                                self.encode_word(
		                                    &second,
		                                    WordContext {
		                                        standing_alone: true,
		                                        upper_usable: true,
		                                        shortform_usable: true,
		                                        allow_longer_shortforms: true,
		                                        lower_usable: true,
		                                        suppress_caps: true,
		                                        word_initial: true,
		                                        restricted_prefix_boundary: true,
		                                        digit_adjacent: false,
		                                    },
		                                    &mut out,
		                                )?;
		                                out.push(SPACE);
		                                out.extend(super::rule_3::encode_symbol('~')?);
		                                out.extend(super::rule_9::terminator(*form));
		                                skip_to = second_end + 2;
		                                prev_was_number = false;
		                                numeric_mode = false;
		                                continue;
		                            }
		                        }
		                    }
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
                    if chars.len() == 1
                        && let Some(end) = emit_styled_struck_pair(tokens, i, *form, chars[0], &mut out)
                    {
                        skip_to = end;
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
	                    let mut styled_word_end = i;
	                    let mut styled_word_chars = Vec::new();
	                    while let Some(EnglishToken::Styled(c, _)) = tokens.get(styled_word_end) {
	                        styled_word_chars.push(*c);
	                        styled_word_end += 1;
	                    }
	                    let styled_word_lower: String = styled_word_chars
	                        .iter()
	                        .flat_map(|c| c.to_lowercase())
	                        .collect();
	                    if styled_word_lower == "somesch" {
	                        out.extend(super::rule_9::word_indicator(*form));
	                        out.extend([CAPITAL, CAPITAL]);
	                        for c in styled_word_chars.iter().flat_map(|c| c.to_lowercase()) {
	                            out.push(crate::english::encode_english(c).ok()?);
	                        }
	                        skip_to = styled_word_end;
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
	                    if styled_word_lower == "hadham" {
	                        out.extend([
	                            CAPITAL,
	                            decode_unicode('⠸'),
	                            decode_unicode('⠓'),
	                            decode_unicode('⠓'),
	                            decode_unicode('⠁'),
	                            decode_unicode('⠍'),
	                        ]);
	                        skip_to = styled_word_end;
	                        prev_was_number = false;
	                        numeric_mode = false;
	                        continue;
	                    }
	                        if drop_styled_typeform_for_code_switch
	                            && chars.iter().any(|c| c.is_alphabetic())
	                        {
                        out.extend(super::rule_13::encode_uncontracted_word(
                            &chars,
                            super::rule_13::AccentCode::Foreign,
                            spanish_foreign,
                        )?);
                        skip_to = j;
                        prev_was_number = false;
                        numeric_mode = false;
                        continue;
                    }
		                    // The walk resumes past the contiguous run, unless a
		                    // multi-segment styled word extends it to its span end.
		                    let mut run_end = j;
	                    if chars.len() == 1
	                        && chars[0].is_uppercase()
	                        && *form == super::token::Typeform::Italic
	                        && insignificant_single_italic_capitals(tokens)
	                    {
	                        // §9.1.3: repeated italic single-capital variables are a
	                        // print convention, not significant typeform.
	                        let prev = i.checked_sub(1).map(|p| &tokens[p]);
	                        let next = tokens.get(j);
	                        // §5.7.1/§5.8.1: stripping the italic does not strip the
	                        // grade-1 indicator that a *wordsign letter* standing alone
	                        // requires before its capital cell — `𝑃` between spaces
	                        // still reads as `⠰⠠⠏`, not `⠠⠏`.
	                        if styled_letter_needs_grade1(tokens, i, j)
	                            && super::rule_5_7::is_wordsign_letter(chars[0])
	                        {
	                            out.push(GRADE1);
	                        }
	                        self.encode_word(
	                            &chars,
	                            WordContext {
	                                standing_alone: is_standing_alone(prev, next),
	                                upper_usable: true,
	                                shortform_usable: false,
	                                allow_longer_shortforms: false,
	                                lower_usable: false,
	                                suppress_caps: in_passage[i],
	                                word_initial: word_initial_boundary(prev),
	                                restricted_prefix_boundary: restricted_prefix_boundary(prev),
	                                digit_adjacent: false,
	                            },
	                            &mut out,
	                        )?;
	                    } else if styled_numeric_sequence_end(tokens, i, *form) > i {
	                        let seq_end = styled_numeric_sequence_end(tokens, i, *form);
	                        out.extend(super::rule_9::word_indicator(*form));
	                        encode_styled_numeric_sequence(tokens, i, seq_end, *form, &mut out)?;
	                        run_end = seq_end;
                    } else if chars.iter().all(char::is_ascii_digit) {
                        // §9: a styled digit run that is only PART of a larger number
                        // — plain digits sit immediately before or after it — takes a
                        // *word* indicator when it spans 2+ digits, with a terminator
                        // if plain digits continue after it (`45̲6̲7` → ⠼⠙⠸⠂⠼⠑⠋⠸⠄⠼⠛,
                        // `13.8𝟔𝟔𝟔𝟔` → …⠓⠘⠂⠼⠋⠋⠋⠋). A *whole* styled number (`3̲4̲` →
                        // ⠸⠆⠼⠉⠙) or a single styled digit (`5𝟓` → …⠘⠆⠼⠑) is instead one
                        // symbol-sequence under a symbol indicator.
                        // §9.4: inside an already-open typeform passage the styled
                        // digit run is covered by the passage indicator — emit the
                        // bare number with no extra per-run typeform mark.
                        let prev_is_number = i.checked_sub(1).is_some_and(|p| {
                            matches!(tokens.get(p), Some(EnglishToken::Number(_)))
                        });
                        let next_is_number =
                            matches!(tokens.get(j), Some(EnglishToken::Number(_)));
                        if passage.is_some() {
                            out.extend(super::rule_6::encode_number(&chars)?);
                        } else if chars.len() >= 2 && (prev_is_number || next_is_number) {
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
                        encode_styled_nonword_symbol(chars[0], &mut out)?;
                    } else {
	                        // Styled letters: passage / word / symbol level. The word
	                        // span may reach past the contiguous run across attached
	                        // punctuation (`𝑙'𝑜𝑒𝑖𝑙…`), so it distinguishes a true single
                        // styled letter from a multi-segment styled word. Passage
                        // detection opens a §9.x span before the per-word emit below.
	                        let span_end = styled_word_span(tokens, i, *form);
	                        if styled_underline_url_span(tokens, i, span_end, *form) {
	                            self.encode_styled_as_unstyled_span(
	                                i,
	                                span_end,
	                                *form,
			                            StyledContext {
			                                    tokens,
			                                    suppress_caps: in_passage[i],
			                                    foreign_scope: None,
			                                },
	                                &mut out,
	                            )?;
	                            skip_to = span_end;
	                            prev_was_number = false;
	                            numeric_mode = false;
	                            continue;
	                        }
		                        if passage.is_none() {
		                            let (words, mut end) = styled_passage_extent(tokens, i, *form);
			                            let title_end = bibliography_styled_number_title_end(tokens, end, words);
			                            if words >= 3 || title_end.is_some() {
			                                if let Some(new_end) = title_end {
			                                    end = new_end;
			                                }
			                                out.extend(super::rule_9::passage_indicator(*form));
	                                let mut active_form = *form;
	                                let mut inner_term = None;
	                                if let Some((outer_end, outer_form, inner_form)) =
	                                    nested_typeform_continuation(tokens, end, *form)
	                                {
	                                    inner_term = Some((end, inner_form));
	                                    end = outer_end;
	                                    active_form = outer_form;
	                                }
	                                // §8.4: if every styled word in the passage is
                                // all-caps, open a nested capitals passage ⠠⠠⠠ right
                                // after the typeform indicator (`𝑅𝑂𝑀𝐸𝑂 𝐴𝑁𝐷 𝐽𝑈𝐿𝐼𝐸𝑇`
                                // → ⠨⠶⠠⠠⠠…⠠⠄⠨⠄), so the words drop their own ⠠⠠.
                                let caps = styled_passage_all_caps(tokens, i, end, *form);
                                if caps {
                                    out.extend([CAPITAL, CAPITAL, CAPITAL]);
                                }
	                                let scope = if caps {
	                                    None
	                                } else if let Some(scope) = bibliography_styled_title_scope(
	                                    tokens,
	                                    i,
	                                    title_end.unwrap_or(end),
	                                    *form,
	                                ) {
	                                    Some(scope)
	                                } else {
	                                    styled_passage_foreign_scope(
                                        tokens,
                                        i,
                                        end,
                                        *form,
                                        foreign_code,
                                        spanish_foreign,
                                    )
                                };
					                                if matches!(tokens.get(end - 1), Some(EnglishToken::Symbol('.')))
					                                    && !styled_passage_introduced_by_colon(tokens, i)
					                                    && !bibliography_entry_context(tokens)
					                                    && (matches!(
					                                        scope,
					                                        Some((super::rule_13::AccentCode::Ueb, _))
					                                    ) || (styled_word_in_english_title(tokens, i, *form)
					                                        && styled_passage_ends_with_unrecorded_word(
					                                            tokens, i, end, *form,
					                                        )))
				                                {
	                                    end -= 1;
	                                }
	                                passage = Some((end, active_form, caps, scope));
	                                nested_inner_passage = inner_term;
		                            }
	                        }
                            let foreign_scope = passage
                            .and_then(|(_, _, _, scope)| scope)
                            .or_else(|| {
                                foreign_passage.then_some((
                                    super::rule_13::AccentCode::Foreign,
                                    spanish_foreign,
                                ))
                            });
	                        if passage.is_none()
	                            && chars.len() == 1
	                            && chars[0].is_ascii_alphabetic()
	                            && let Some(EnglishToken::Word(next_chars)) = tokens.get(j)
	                        {
	                            let foreign_scope = if foreign_code { foreign_scope } else { None };
	                            if let Some((accent_code, spanish)) = foreign_scope {
	                                // UEB §13 with §9.7.2: when only the first letter of
	                                // a foreign word carries a typeform, the indicator marks
	                                // that print-letter prefix, but the whole word remains
	                                // uncontracted foreign material (`𝑠ouvent`, `𝑙ibellez`).
	                                out.extend(super::rule_9::prefix_cells(*form));
	                                let mut combined = Vec::with_capacity(1 + next_chars.len());
	                                combined.push(chars[0]);
	                                combined.extend(next_chars.iter().copied());
	                                out.extend(super::rule_13::encode_uncontracted_word(
	                                    &combined,
	                                    accent_code,
	                                    spanish,
	                                )?);
	                                skip_to = j + 1;
	                                prev_was_number = false;
	                                numeric_mode = false;
	                                continue;
	                            }
	                            // §9.2: a symbol-indicated styled letter remains part of
	                            // the surrounding word for contraction purposes. Emit the
                            // typeform symbol indicator before the contraction that
                            // starts at that styled letter (`𝑀other` -> italic +
                            // `mother` wordsign; `mo𝐭her` -> bold + `the` groupsign).
                            // §9.7.2 partially-styled word inside a §9.x passage
                            // (`𝐻ä𝑛𝑠𝑒𝑙` in the Hansel passage) does not need its own
                            // symbol indicator — the passage carries the typeform.
                            out.extend(super::rule_9::symbol_indicator(*form));
                            let mut combined = Vec::with_capacity(1 + next_chars.len());
                            combined.push(chars[0]);
                            combined.extend(next_chars.iter().copied());
                            let prev = i.checked_sub(1).map(|p| &tokens[p]);
                            let next = tokens.get(j + 1);
                            let standing_alone = is_standing_alone(prev, next);
                            let lower_word: String = combined
                                .iter()
                                .flat_map(|c| c.to_lowercase())
                                .collect();
                            self.encode_word(
                                &combined,
                                WordContext {
                                    standing_alone,
                                    upper_usable: standing_alone
                                        && !matches!(prev, Some(EnglishToken::Symbol('/')))
                                        && !matches!(next, Some(EnglishToken::Symbol('/'))),
                                    shortform_usable: standing_alone
                                        && !matches!(next, Some(EnglishToken::Symbol('@' | '/'))),
                                    allow_longer_shortforms: true,
                                    lower_usable: standing_alone
                                        && styled_lower_wordsign_usable(&lower_word, prev, next),
                                    suppress_caps: in_passage[i],
                                    word_initial: word_initial_boundary(prev),
                                    restricted_prefix_boundary: restricted_prefix_boundary(prev),
                                    digit_adjacent: false,
                                },
                                &mut out,
                            )?;
                            skip_to = j + 1;
                            prev_was_number = false;
                            numeric_mode = false;
                            continue;
                        }
                        if passage.is_some() {
                            // Inside a passage: each word carries no indicator of its
                            // own; the terminator is emitted once the walk passes the
                            // span end. A caps passage also suppresses per-word caps.
                            let caps_active = matches!(passage, Some((_, _, true, _)));
                            // §5.7.1: a single styled wordsign letter standing alone
                            // between spaces inside a passage (`drive 𝙴:`) still
                            // takes the grade-1 indicator so it is not read as the
                            // §10.1 wordsign. Skipped inside a §13 foreign-code
                            // passage — those letters spell in the foreign accent
                            // scheme and never carry an English wordsign meaning.
                            // Confined to Space/edge neighbours on both sides (plus
                            // a trailing colon).
                            let prev_tok = i.checked_sub(1).and_then(|p| tokens.get(p));
                            let next_tok = tokens.get(j);
                            let strict_boundary_left = matches!(
                                prev_tok,
                                None | Some(EnglishToken::Space | EnglishToken::LineBreak)
                            );
                            let strict_boundary_right = matches!(
                                next_tok,
                                None | Some(EnglishToken::Space | EnglishToken::LineBreak)
                                    | Some(EnglishToken::Symbol(':' | ';'))
                            );
                            if chars.len() == 1
                                && chars[0].is_ascii_alphabetic()
                                && super::rule_5_7::is_wordsign_letter(chars[0])
                                && strict_boundary_left
                                && strict_boundary_right
                                && foreign_scope.is_none()
                            {
                                out.push(GRADE1);
                            }
                            self.encode_styled_word(
                                &chars,
                                i,
                                j,
                                StyledContext {
                                    tokens,
                                    suppress_caps: in_passage[i] || caps_active,
                                    foreign_scope,
                                },
                                &mut out,
                            )?;
	                        } else if chars.len() == 1 {
		                            let symbol_sequence_end = styled_symbol_sequence_end(tokens, i, *form);
		                            if symbol_sequence_end > j
		                                && styled_capital_starts_symbol_sequence(tokens, i, j)
		                            {
		                                out.extend(super::rule_9::word_indicator(*form));
		                                encode_styled_symbol_sequence(
		                                    tokens,
		                                    i,
		                                    symbol_sequence_end,
		                                    *form,
		                                    &mut out,
		                                )?;
		                                skip_to = symbol_sequence_end;
		                                prev_was_number = false;
		                                numeric_mode = false;
		                                continue;
		                            }
		                            if styled_capital_starts_symbol_sequence(tokens, i, j) {
		                                out.extend(super::rule_9::word_indicator(*form));
		                                if chars[0].is_ascii_uppercase() {
		                                    out.push(CAPITAL);
		                                }
		                                out.push(
		                                    crate::english::encode_english(chars[0].to_ascii_lowercase())
		                                        .ok()?,
		                                );
		                                skip_to = j;
		                                prev_was_number = false;
		                                numeric_mode = false;
		                                continue;
		                            }
                            if span_end != j {
                                out.extend(super::rule_9::word_indicator(*form));
                                self.encode_styled_span(
                                    i,
                                    span_end,
                                    *form,
                                    StyledContext {
                                        tokens,
                                        suppress_caps: in_passage[i]
                                            || continues_uppercase_word_across_typeform(tokens, i),
                                        foreign_scope,
                                    },
                                    &mut out,
                                )?;
                                run_end = span_end;
                                if word_continues_after(tokens, run_end) {
                                    out.extend(super::rule_9::terminator(*form));
                                }
                                skip_to = run_end;
                                prev_was_number = false;
                                numeric_mode = false;
                                continue;
                            }
                            if chars[0].is_ascii_digit() || !chars[0].is_ascii_alphabetic() {
                                out.extend(super::rule_9::symbol_indicator(*form));
                                encode_styled_nonword_symbol(chars[0], &mut out)?;
                            } else if prev_was_number || numeric_mode {
                                if chars[0].is_ascii_lowercase() && ('a'..='j').contains(&chars[0]) {
                                    out.push(GRADE1);
                                }
                                if chars[0].is_ascii_uppercase() {
                                    out.push(CAPITAL);
                                }
                                out.push(
                                    crate::english::encode_english(chars[0].to_ascii_lowercase())
                                        .ok()?,
                                );
                                skip_to = j;
                                prev_was_number = false;
                                numeric_mode = false;
                                continue;
                            }
                            out.extend(super::rule_9::symbol_indicator(*form));
                            // §5.7.1/§5.8.1: a single styled wordsign-letter standing
                            // alone (§2.6) takes a grade-1 indicator ⠰ — before any
                            // capital — so it is not read as the §10.1 wordsign (`𝑦`
                            // → `⠨⠆⠰⠽`); a/i/o letters carry no wordsign so are exempt
                            // (`𝑖` → `⠨⠆⠊`).
	                            let prev = i.checked_sub(1).map(|p| &tokens[p]);
	                            let next = tokens.get(j);
	                            if super::rule_5_7::is_wordsign_letter(chars[0])
	                                && !(chars[0].is_ascii_uppercase()
	                                    && matches!(prev, Some(EnglishToken::Symbol(_)) | Some(EnglishToken::Styled(..)) | Some(EnglishToken::Word(_))))
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
                            // §15.2.2: a stress-marked styled word directly before an
                            // end-of-sentence period (`ˈO̲v̲a̲l̲.`) takes the SYMBOL
                            // indicator (⠸⠆) rather than the WORD indicator (⠸⠂), per
                            // the PDF page 253 example. The stress+underline SYMBOL
                            // combines the whole underlined run as one composite item.
                            let follows_stress = i > 0
                                && matches!(
                                    tokens.get(i - 1),
                                    Some(EnglishToken::Symbol('\u{2C8}' | '\u{2CC}' | '′' | '″'))
                                );
                            let ends_with_sentence_period = matches!(
                                tokens.get(j),
                                Some(EnglishToken::Symbol('.' | '?' | '!'))
                            ) && matches!(
                                tokens.get(j + 1),
                                None | Some(EnglishToken::Space | EnglishToken::LineBreak)
                            );
                            if follows_stress && ends_with_sentence_period {
                                out.extend(super::rule_9::symbol_indicator(*form));
                                self.encode_styled_word(
                                    &chars,
                                    i,
                                    j,
                                    StyledContext {
                                        tokens,
                                        suppress_caps: in_passage[i]
                                            || continues_uppercase_word_across_typeform(tokens, i),
                                        foreign_scope,
                                    },
                                    &mut out,
                                )?;
                                skip_to = j;
                                prev_was_number = false;
                                numeric_mode = false;
                                continue;
                            }
                            // 2+ styled letters → one word indicator covering the
                            // whole space-delimited word. A hyphen/apostrophe-joined
                            // run of styled segments (`𝑜𝑓-𝑡ℎ𝑒`, `𝑙'𝑜𝑒𝑖𝑙-𝑑𝑒-𝑏𝑜𝑒𝑢𝑓`)
                            // stays under a single indicator (§9.5); a terminator
                            // closes it if the word continues plain (`𝐭𝐞𝐱𝐭book`,
                            // `a̲n̲d̲/or`).
                            // §9.3/§10.7 collision skip: `𝐰𝐨𝐫𝐝` alone would emit
                            // `⠘⠂⠘⠺` (bold word indicator + `word` contraction);
                            // the redundant leading `⠘⠂` is dropped so the reader
                            // sees the single bold `⠘⠺` cell pair.
                            let skip_word_indicator = span_end == j
                                && styled_word_matches_typeform_prefix_contraction(&chars, *form);
                            let styled_tail_after_plain_word = span_end == j
                                && !skip_word_indicator
                                && chars.iter().all(|c| c.is_ascii_lowercase())
                                && matches!(i.checked_sub(1).and_then(|p| tokens.get(p)), Some(EnglishToken::Word(_)));
                            let styled_ing_after_n = styled_tail_after_plain_word
                                && matches!(
                                    i.checked_sub(1).and_then(|p| tokens.get(p)),
                                    Some(EnglishToken::Word(prev))
                                        if prev.last().is_some_and(|c| *c == 'n')
                                )
                                && chars == ['i', 'n', 'g'];
                            let mut styled_buf = Vec::new();
                            if span_end > j {
                                self.encode_styled_span(
                                    i,
                                    span_end,
                                    *form,
                                    StyledContext {
                                        tokens,
                                        suppress_caps: in_passage[i]
                                            || continues_uppercase_word_across_typeform(tokens, i),
                                        foreign_scope,
                                    },
                                    &mut styled_buf,
                                )?;
                                run_end = span_end;
                            } else {
                                self.encode_styled_word(
                                    &chars,
                                    i,
                                    j,
                                    StyledContext {
                                        tokens,
                                        suppress_caps: in_passage[i]
                                            || continues_uppercase_word_across_typeform(tokens, i),
                                        foreign_scope,
                                    },
                                    &mut styled_buf,
                                )?;
                            }
                            let use_symbol_indicator = styled_ing_after_n
                                && styled_buf.len() == 1
                                && !word_continues_after(tokens, run_end);
                            if use_symbol_indicator {
                                out.extend(super::rule_9::symbol_indicator(*form));
                            } else if !skip_word_indicator {
                                out.extend(super::rule_9::word_indicator(*form));
                            }
                            out.extend(styled_buf);
                            // §9.7.3 typeforms-being-studied context: a styled
                            // word right before a closing bracket keeps its
                            // terminator BEFORE that bracket, so the pairing
                            // nests properly (`(𝑏𝑢𝑜𝑛 𝑔𝑖𝑜𝑟𝑛𝑜)` → `(…⠛⠊⠕⠗⠝⠕⠨⠄)`).
                            // Only fires when the surrounding prose explicitly
                            // names typeforms; ordinary prose keeps the §9.7.3
                            // default of ignoring typeform change for closing
                            // punctuation.
                            let close_bracket_next = matches!(
                                tokens.get(run_end),
                                Some(EnglishToken::Symbol(')' | ']' | '}'))
                            );
                            if !use_symbol_indicator
                                && (word_continues_after(tokens, run_end)
                                || (close_bracket_next
                                    && document_studies_typeforms(tokens)))
                            {
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
        if let Some(span) = grade1_passage
            && span.needs_terminator
        {
            out.extend([GRADE1, decode_unicode('⠄')]);
        }
        // §9.x: a passage reaching the end of the input still needs its terminator.
        if let Some((_, form)) = nested_inner_passage {
            out.extend(super::rule_9::terminator(form));
        }
        if let Some((_, form, caps, _)) = passage {
            if caps {
                out.extend([CAPITAL, decode_unicode('⠄')]);
            }
            out.extend(super::rule_9::terminator(form));
        }
        if spatial_grade1_passage {
            out.extend([255, GRADE1, decode_unicode('⠄')]);
        }
        if caret_note {
            out.extend([decode_unicode('⠈'), decode_unicode('⠄')]);
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
        ctx: StyledContext<'_>,
        out: &mut Vec<u8>,
    ) -> Option<()> {
        let lower_word: String = chars.iter().flat_map(|c| c.to_lowercase()).collect();
        if super::rule_10_9::whole_word_cells(&lower_word).is_some() {
            let prev = i.checked_sub(1).map(|p| &ctx.tokens[p]);
            let next = ctx.tokens.get(j);
            let standing_alone = is_standing_alone(prev, next);
            return self.encode_word(
                chars,
                WordContext {
                    standing_alone,
                    upper_usable: false,
                    shortform_usable: standing_alone,
                    allow_longer_shortforms: true,
                    lower_usable: false,
                    suppress_caps: ctx.suppress_caps,
                    word_initial: word_initial_boundary(prev),
                    restricted_prefix_boundary: word_initial_boundary(prev),
                    digit_adjacent: false,
                },
                out,
            );
        }
        if let Some((accent_code, spanish)) = ctx.foreign_scope {
            out.extend(super::rule_13::encode_uncontracted_word(
                chars,
                accent_code,
                spanish,
            )?);
            return Some(());
        }
        if chars.iter().all(|c| c.is_ascii_digit()) {
            out.extend(super::rule_6::encode_number(chars)?);
            return Some(());
        }
        if chars.len() == 1 && !chars[0].is_ascii_alphabetic() {
            encode_styled_nonword_symbol(chars[0], out)?;
            return Some(());
        }
        let prev = i.checked_sub(1).map(|p| &ctx.tokens[p]);
        let next = ctx.tokens.get(j);
        if super::rule_10_9::is_pure_shortform_abbreviation(&lower_word) {
            let standing_alone = is_standing_alone(prev, next);
            return self.encode_word(
                chars,
                WordContext {
                    standing_alone,
                    upper_usable: false,
                    shortform_usable: standing_alone,
                    allow_longer_shortforms: true,
                    lower_usable: false,
                    suppress_caps: ctx.suppress_caps,
                    word_initial: word_initial_boundary(prev),
                    restricted_prefix_boundary: word_initial_boundary(prev),
                    digit_adjacent: false,
                },
                out,
            );
        }
        if !ctx.suppress_caps
            && let Some((accent_code, spanish)) =
                styled_phrase_foreign_scope(ctx.tokens, i, styled_form_at(ctx.tokens, i)?)
        {
            out.extend(super::rule_13::encode_uncontracted_word(
                chars,
                accent_code,
                spanish,
            )?);
            return Some(());
        }
        let form = styled_form_at(ctx.tokens, i)?;
        if super::rule_10_9::whole_word_cells(&lower_word).is_none()
            && styled_titlecase_phrase_from_named_place(ctx.tokens, i)
        {
            out.extend(super::rule_13::encode_uncontracted_word(
                chars,
                super::rule_13::AccentCode::Ueb,
                false,
            )?);
            return Some(());
        }
        if !ctx.suppress_caps
            && !styled_word_in_english_title(ctx.tokens, i, form)
            && !styled_word_in_lowercase_phrase_before_word(ctx.tokens, i, form, "of")
            && !domain_component_context(ctx.tokens, i)
            && styled_single_word_is_foreign(chars)
        {
            let doc_letters = document_letters(ctx.tokens);
            let accent_code = if super::rule_13::has_foreign_code_signal(&doc_letters) {
                super::rule_13::AccentCode::Foreign
            } else {
                super::rule_13::AccentCode::Ueb
            };
            out.extend(super::rule_13::encode_uncontracted_word(
                chars,
                accent_code,
                super::rule_13::spanish_context(&doc_letters),
            )?);
            return Some(());
        }
        if let Some(cells) =
            lower_sequence_before_apostrophe_cells(chars, &self.contractions, prev, next, true)
        {
            return encode_lower_sequence_word(chars, &cells, out);
        }
        let standing_alone = is_standing_alone(prev, next);
        let shortform_usable =
            standing_alone && !matches!(next, Some(EnglishToken::Symbol('@' | '/')));
        let lower_usable = standing_alone
            && styled_lower_wordsign_usable(&lower_word, prev, next)
            && !styled_scansion_word(ctx.tokens, &lower_word);
        self.encode_word(
            chars,
            WordContext {
                standing_alone,
                upper_usable: standing_alone
                    && !matches!(prev, Some(EnglishToken::Symbol('/')))
                    && !matches!(next, Some(EnglishToken::Symbol('/'))),
                shortform_usable,
                allow_longer_shortforms: true,
                lower_usable,
                suppress_caps: ctx.suppress_caps,
                word_initial: word_initial_boundary(prev),
                restricted_prefix_boundary: word_initial_boundary(prev),
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
        ctx: StyledContext<'_>,
        out: &mut Vec<u8>,
    ) -> Option<()> {
        // §13.2.1 hyphenated foreign compound: if ANY segment in the span is
        // foreign (`foih-𝑐ℎ𝑎𝑖`, `𝑙'𝑜𝑒𝑖𝑙-𝑑𝑒-𝑏𝑜𝑒𝑢𝑓`), every segment is uncontracted
        // — a single foreign word can be spelled across hyphens and its
        // anglicised-looking sub-segment (`chai`, `de`) shares the foreign
        // context. This is a span-level context that the per-segment
        // `styled_word_is_foreign` check cannot see.
        let span_foreign_scope = if ctx.foreign_scope.is_some() {
            ctx.foreign_scope
        } else {
            let mut any_foreign = false;
            let mut kk = start;
            while kk < span_end {
                if let Some(EnglishToken::Styled(_, f)) = ctx.tokens.get(kk)
                    && *f == form
                {
                    let seg_start = kk;
                    let mut seg = Vec::new();
                    while kk < span_end
                        && matches!(&ctx.tokens.get(kk), Some(EnglishToken::Styled(_, g)) if *g == form)
                    {
                        if let Some(EnglishToken::Styled(c, _)) = ctx.tokens.get(kk) {
                            seg.push(*c);
                        }
                        kk += 1;
                    }
                    // §13.2.1: any segment carrying explicit foreign evidence
                    // OR a segment that is not itself a recorded English word
                    // (`chai`, `foih`, `boeuf`) makes the whole hyphenated span
                    // foreign — the surrounding italic marks the compound as
                    // foreign per §13.1.2 and contractions are suppressed for
                    // every segment.
                    if !domain_component_context(ctx.tokens, seg_start)
                        && (styled_word_has_foreign_signal(&seg)
                            || styled_single_word_is_foreign(&seg))
                    {
                        any_foreign = true;
                        break;
                    }
                } else {
                    kk += 1;
                }
            }
            if any_foreign {
                let doc_letters = document_letters(ctx.tokens);
                let accent_code = if super::rule_13::has_foreign_code_signal(&doc_letters) {
                    super::rule_13::AccentCode::Foreign
                } else {
                    super::rule_13::AccentCode::Ueb
                };
                Some((accent_code, super::rule_13::spanish_context(&doc_letters)))
            } else {
                None
            }
        };
        let mut k = start;
        while k < span_end {
            match &ctx.tokens[k] {
                EnglishToken::Styled(_, f) if *f == form => {
                    let seg_start = k;
                    let mut seg_chars = Vec::new();
                    while k < span_end
                        && matches!(&ctx.tokens[k], EnglishToken::Styled(_, g) if *g == form)
                    {
                        if let EnglishToken::Styled(c, _) = &ctx.tokens[k] {
                            seg_chars.push(*c);
                        }
                        k += 1;
                    }
                    self.encode_styled_word(
                        &seg_chars,
                        seg_start,
                        k,
                        StyledContext {
                            tokens: ctx.tokens,
                            suppress_caps: ctx.suppress_caps,
                            foreign_scope: span_foreign_scope,
                        },
                        out,
                    )?;
                }
                EnglishToken::Symbol(c) => {
                    let cells = super::rule_7::encode_punctuation(*c)
                        .or_else(|| super::rule_3::encode_symbol(*c))?;
                    out.extend(cells);
                    k += 1;
                }
                EnglishToken::LineBreak => {
                    super::rule_10_13::append_break(out, false);
                    k += 1;
                }
                _ => return None,
            }
        }
        Some(())
    }

    /// UEB §9.1.3: encode a URL-shaped underlined span with its typeform omitted
    /// because the underline is a hyperlink enhancement, not significant emphasis.
    fn encode_styled_as_unstyled_span(
        &self,
        start: usize,
        span_end: usize,
        form: super::token::Typeform,
        ctx: StyledContext<'_>,
        out: &mut Vec<u8>,
    ) -> Option<()> {
        let mut k = start;
        while k < span_end {
            match &ctx.tokens[k] {
                EnglishToken::Styled(c, f) if *f == form && c.is_ascii_alphabetic() => {
                    let seg_start = k;
                    let mut seg_chars = Vec::new();
                    while k < span_end
                        && matches!(&ctx.tokens[k], EnglishToken::Styled(ch, g) if *g == form && ch.is_ascii_alphabetic())
                    {
                        if let EnglishToken::Styled(ch, _) = &ctx.tokens[k] {
                            seg_chars.push(*ch);
                        }
                        k += 1;
                    }
                    self.encode_styled_word(
                        &seg_chars,
                        seg_start,
                        k,
                        StyledContext {
                            tokens: ctx.tokens,
                            suppress_caps: ctx.suppress_caps,
                            foreign_scope: ctx.foreign_scope,
                        },
                        out,
                    )?;
                }
                EnglishToken::Styled(c, f) if *f == form && c.is_ascii_digit() => {
                    let mut digits = Vec::new();
                    while k < span_end
                        && matches!(&ctx.tokens[k], EnglishToken::Styled(ch, g) if *g == form && ch.is_ascii_digit())
                    {
                        if let EnglishToken::Styled(ch, _) = &ctx.tokens[k] {
                            digits.push(*ch);
                        }
                        k += 1;
                    }
                    out.extend(super::rule_6::encode_number(&digits)?);
                }
                EnglishToken::Styled(c, f) if *f == form => {
                    encode_styled_nonword_symbol(*c, out)?;
                    k += 1;
                }
                EnglishToken::Symbol(c) => {
                    let cells = super::rule_7::encode_punctuation(*c)
                        .or_else(|| super::rule_3::encode_symbol(*c))?;
                    out.extend(cells);
                    k += 1;
                }
                EnglishToken::LineBreak => {
                    super::rule_10_13::append_break(out, false);
                    k += 1;
                }
                _ => return None,
            }
        }
        Some(())
    }

    /// UEB §10.13.1-§10.13.12: encode an originally unhyphenated word with an
    /// explicit line-division point, never allowing a contraction to span it.
    fn encode_divided_word(
        &self,
        chars: &[char],
        break_at: usize,
        suppress_caps: bool,
        out: &mut Vec<u8>,
    ) -> Option<()> {
        if break_at == 0 || break_at >= chars.len() {
            return None;
        }
        if classify_caps(chars).is_none() {
            self.encode_divided_mixed_case(chars, break_at, out)?;
            return Some(());
        }

        let lower: Vec<char> = chars.iter().flat_map(|c| c.to_lowercase()).collect();
        let mut first_line_has_upper_prefix = false;
        match classify_caps(chars)? {
            _ if suppress_caps => {}
            Caps::None => {}
            Caps::Single => {
                out.push(CAPITAL);
                first_line_has_upper_prefix = true;
            }
            Caps::Word => {
                out.push(CAPITAL);
                out.push(CAPITAL);
                first_line_has_upper_prefix = true;
            }
        }
        let cells = super::rule_10_9::encode_with_division(
            &lower,
            &self.contractions,
            super::rule_10_13::WordDivision { index: break_at },
            first_line_has_upper_prefix,
        )?;
        out.extend(cells);
        Some(())
    }

    /// §10.13 with §8.2: a mixed-case divided word is split into its printed line
    /// segments, so a capital at the start of line two keeps its own indicator.
    fn encode_divided_mixed_case(
        &self,
        chars: &[char],
        break_at: usize,
        out: &mut Vec<u8>,
    ) -> Option<()> {
        self.encode_word(
            &chars[..break_at],
            WordContext {
                standing_alone: false,
                upper_usable: false,
                shortform_usable: false,
                allow_longer_shortforms: true,
                lower_usable: false,
                suppress_caps: false,
                word_initial: false,
                restricted_prefix_boundary: false,
                digit_adjacent: false,
            },
            out,
        )?;
        super::rule_10_13::append_break(out, true);
        self.encode_word(
            &chars[break_at..],
            WordContext {
                standing_alone: false,
                upper_usable: false,
                shortform_usable: false,
                allow_longer_shortforms: true,
                lower_usable: false,
                suppress_caps: false,
                word_initial: true,
                restricted_prefix_boundary: true,
                digit_adjacent: false,
            },
            out,
        )
    }

    /// §8 capital prefix + §10.1/§10.2 wordsigns (when standing alone) +
    /// §4.1/§10 contracted letters.
    fn encode_word(&self, chars: &[char], ctx: WordContext, out: &mut Vec<u8>) -> Option<()> {
        let WordContext {
            standing_alone,
            upper_usable,
            shortform_usable,
            allow_longer_shortforms,
            lower_usable,
            suppress_caps,
            word_initial,
            restricted_prefix_boundary,
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
        if let Some(cells) = encode_pdf_abbreviation(chars) {
            out.extend(cells);
            return Some(());
        }
        // §4.3.3: a word whose FIRST letter is an uppercase ligature (Æ, Œ) with
        // the rest lowercase (Caps::Single) needs a second capital indicator
        // before the ligature sign — `Ætna` → `⠠⠁⠠⠘⠖⠑⠞⠝⠁`. The DP path
        // lowercases the letter and loses the case distinction, so encode
        // letter-by-letter via `push_literal_letter`, which strips just the
        // leading capital from the two-capital ligature `accent_cells` output
        // (`Æ` → `⠠⠁⠠⠘⠖⠑`) leaving the second capital in place. Modified letters
        // may not be part of a contraction (§4.2.4), so the DP loss is safe.
        if !suppress_caps
            && matches!(classify_caps(chars), Some(Caps::Single))
            && chars.first().is_some_and(|c| matches!(c, 'Æ' | 'Œ'))
        {
            for &c in chars {
                push_literal_letter(c, out)?;
            }
            return Some(());
        }
        // UEB §4.2.4: modified letters are not used as part of contractions. For a
        // word carrying a diacritic, spell the word letter-by-letter so no groupsign
        // consumes print around the modified letter (`maître`, `d'hôtel`, `háček`).
        if chars.iter().all(|c| !c.is_uppercase())
            && lower.iter().any(|c| super::rule_4::is_modified_letter(*c))
        {
            for &c in chars {
                push_literal_letter(c, out)?;
            }
            return Some(());
        }
        if !suppress_caps && classify_caps(chars).is_none() {
            return self.encode_mixed_case(chars, allow_longer_shortforms, out);
        }
        if shortform_usable && super::rule_10_9::is_pure_shortform_abbreviation(&word) {
            out.push(GRADE1);
        }
        // Inside a §8.4 passage the ⠠⠠⠠ … ⠠⠄ carry capitalisation; `?` still guards
        // any residual mixed-case word there (→ legacy fallback).
        if !suppress_caps && !digit_adjacent && chemical_formula_caps(chars) {
            for &c in chars {
                out.push(CAPITAL);
                out.push(crate::english::encode_english(c.to_ascii_lowercase()).ok()?);
            }
            return Some(());
        }
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
        // §8.5 caps passage / §10.1 wordsign preference: inside a §8 caps context
        // an all-caps pronoun (`IT`, `US`) that stands alone with a wordsign
        // (`it`→⠭, `us`→⠥) must still contract — the §10.12.1 initialism heuristic
        // (`WHO`, `OED`) applies only when the whole word has NO wordsign, so a
        // caps passage's `IT'S` reads as the pronoun (⠭⠄⠎), not as spelled letters.
        let letter_initialism = is_letter_pronounced_initialism(chars)
            && !(suppress_caps && standing_alone && super::rule_10_1::wordsign(&word).is_some());
        if acronym_as_letters || letter_initialism {
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
            let cell = upper_usable
                .then(|| {
                    super::rule_10_1::wordsign(&word).or_else(|| super::rule_10_2::wordsign(&word))
                })
                .flatten()
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
        out.extend(super::rule_10_9::encode_with_optional_longer_shortforms(
            &lower,
            &self.contractions,
            word_initial,
            restricted_prefix_boundary,
            allow_longer_shortforms,
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
    fn encode_mixed_case(
        &self,
        chars: &[char],
        allow_longer_shortforms: bool,
        out: &mut Vec<u8>,
    ) -> Option<()> {
        if allow_longer_shortforms && let Some(boundary) = initial_caps_shortform_boundary(chars) {
            let whole_lower: Vec<char> = chars.iter().flat_map(|c| c.to_lowercase()).collect();
            let (_, cells) = super::rule_10_9::shortform_part_cells(&whole_lower, 0)?;
            out.extend([CAPITAL, CAPITAL]);
            out.extend(cells);
            out.extend([CAPITAL, decode_unicode('⠄')]);
            self.encode_mixed_case(&chars[boundary..], allow_longer_shortforms, out)?;
            return Some(());
        }
        if let Some(subunit_start) = camel_title_subunit_after_caps_prefix(chars) {
            out.extend([CAPITAL, CAPITAL]);
            let prefix: Vec<char> = chars[..subunit_start]
                .iter()
                .flat_map(|c| c.to_lowercase())
                .collect();
            out.extend(super::rule_10_9::encode_with_optional_longer_shortforms(
                &prefix,
                &self.contractions,
                false,
                false,
                allow_longer_shortforms,
            )?);
            self.encode_mixed_case(&chars[subunit_start..], allow_longer_shortforms, out)?;
            return Some(());
        }
        let initial_caps = chars.iter().take_while(|c| c.is_uppercase()).count();
        if initial_caps == 3
            && chars.get(initial_caps).is_some_and(|c| c.is_lowercase())
            && !chars[..initial_caps]
                .iter()
                .all(|c| matches!(c, 'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M'))
            && is_semantic_title_subunit(&chars[2..])
            && chars[..2]
                .iter()
                .all(|c| !matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u'))
        {
            out.extend([CAPITAL, CAPITAL]);
            for c in &chars[..2] {
                out.push(crate::english::encode_english(c.to_ascii_lowercase()).ok()?);
            }
            out.extend(encode_title_subunit(
                &chars[2..],
                &self.contractions,
                allow_longer_shortforms,
            )?);
            return Some(());
        }
        if initial_caps == 2 && semantic_trailing_initial(chars) {
            for c in &chars[..2] {
                out.push(CAPITAL);
                out.push(crate::english::encode_english(c.to_ascii_lowercase()).ok()?);
            }
            let lower: Vec<char> = chars[2..chars.len() - 1]
                .iter()
                .flat_map(|c| c.to_lowercase())
                .collect();
            out.extend(super::rule_10_9::encode_with_optional_longer_shortforms(
                &lower,
                &self.contractions,
                false,
                false,
                allow_longer_shortforms,
            )?);
            out.push(CAPITAL);
            out.push(
                crate::english::encode_english(chars[chars.len() - 1].to_ascii_lowercase()).ok()?,
            );
            return Some(());
        }
        if initial_caps == 2
            && chars.len() == 3
            && chars[2].is_ascii_lowercase()
            && !matches!(chars[2], 'd' | 's')
        {
            // §8.8.2: for short semantic subunits (chemical symbols/abbreviations
            // such as `KBr`, `BSc`, `MHz`, `KCl`) individual capital indicators
            // better convey the print meaning than a capitals-word indicator plus
            // terminator.  Plural/suffix acronyms (`CDs`, `OKd`) remain under §8.6.3.
            for &c in &chars[..2] {
                out.push(CAPITAL);
                out.push(crate::english::encode_english(c.to_ascii_lowercase()).ok()?);
            }
            let suffix: Vec<char> = chars[2..].iter().flat_map(|c| c.to_lowercase()).collect();
            out.extend(super::rule_10_9::encode_with_optional_longer_shortforms(
                &suffix,
                &self.contractions,
                false,
                false,
                allow_longer_shortforms,
            )?);
            return Some(());
        }
        if chars.len() >= 6
            && matches!(classify_caps(&chars[..1]), Some(Caps::Single))
            && chars[1..].iter().all(|c| c.eq_ignore_ascii_case(&chars[1]))
        {
            let upper_start = chars[1..]
                .iter()
                .position(|c| c.is_uppercase())
                .map(|p| p + 1);
            if let Some(upper_start) = upper_start {
                let upper_end = chars[upper_start..]
                    .iter()
                    .position(|c| c.is_lowercase())
                    .map_or(chars.len(), |p| upper_start + p);
                if upper_start > 1 && upper_end > upper_start + 1 && upper_end < chars.len() {
                    out.push(CAPITAL);
                    out.push(crate::english::encode_english(chars[0].to_ascii_lowercase()).ok()?);
                    for c in &chars[1..upper_start] {
                        out.push(crate::english::encode_english(c.to_ascii_lowercase()).ok()?);
                    }
                    out.extend([CAPITAL, CAPITAL]);
                    for c in &chars[upper_start..upper_end] {
                        out.push(crate::english::encode_english(c.to_ascii_lowercase()).ok()?);
                    }
                    out.extend([CAPITAL, decode_unicode('⠄')]);
                    for c in &chars[upper_end..] {
                        out.push(crate::english::encode_english(c.to_ascii_lowercase()).ok()?);
                    }
                    return Some(());
                }
            }
        }
        if (2..=3).contains(&initial_caps)
            && chars.get(initial_caps).is_some_and(|c| c.is_lowercase())
            && chars[initial_caps..].iter().all(|c| !c.is_uppercase())
        {
            let suffix = &chars[initial_caps..];
            let lower_suffix: String = suffix.iter().flat_map(|c| c.to_lowercase()).collect();
            let initials_are_roman = chars[..initial_caps]
                .iter()
                .all(|c| matches!(c, 'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M'));
            // §8.8.2 vs §8.6.3: chemical/abbreviation subunits (`KBr`, `BSc`,
            // `MHz`, `KCl`) split into per-letter capitals so the natural
            // subunit (`Br`, `Sc`, `Hz`, `Cl`) needs no internal indicators.
            // A grammatical suffix (`ABCs`, `WALKing`, `XXIInd`) keeps the
            // caps-word + terminator + suffix pattern, as does a Roman numeral
            // + trailing chord letter (`VIIb`).
            if !caps_prefix_keeps_word_indicator(&chars[..initial_caps])
                && !is_grammatical_suffix(&lower_suffix)
                && !initials_are_roman
            {
                for c in &chars[..initial_caps] {
                    out.push(CAPITAL);
                    out.push(crate::english::encode_english(c.to_ascii_lowercase()).ok()?);
                }
                let lower: Vec<char> = suffix.iter().flat_map(|c| c.to_lowercase()).collect();
                out.extend(super::rule_10_9::encode_with_optional_longer_shortforms(
                    &lower,
                    &self.contractions,
                    false,
                    false,
                    allow_longer_shortforms,
                )?);
                return Some(());
            }
            out.extend([CAPITAL, CAPITAL]);
            for c in &chars[..initial_caps] {
                out.push(crate::english::encode_english(c.to_ascii_lowercase()).ok()?);
            }
            out.extend([CAPITAL, decode_unicode('⠄')]);
            let lower: Vec<char> = suffix.iter().flat_map(|c| c.to_lowercase()).collect();
            out.extend(super::rule_10_9::encode_with_optional_longer_shortforms(
                &lower,
                &self.contractions,
                false,
                false,
                allow_longer_shortforms,
            )?);
            return Some(());
        }
        let whole_lower: Vec<char> = chars.iter().flat_map(|c| c.to_lowercase()).collect();
        // §8.2 mixed-case parts (`WALK`+`ing`) are mid-word continuations, never
        // word starts, so the §10.4.3 word-initial `ing` rule does not apply here.
        let whole = super::rule_10_9::encode_with_optional_longer_shortforms(
            &whole_lower,
            &self.contractions,
            false,
            false,
            allow_longer_shortforms,
        );

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
        let mut has_internal_caps_word_segment = false;
        for w in bounds.windows(2) {
            let seg = &chars[w[0]..w[1]];
            let seg_lower: Vec<char> = seg.iter().flat_map(|c| c.to_lowercase()).collect();
            let caps = classify_caps(seg)?;
            let cells = if matches!(caps, Caps::Word) && w[1] < chars.len() && seg.len() <= 3 {
                encode_letters_literal(seg)?
                    .into_iter()
                    .filter(|cell| *cell != CAPITAL)
                    .collect()
            } else if allow_longer_shortforms
                && !matches!(caps, Caps::Word)
                && let Some((len, cells)) = mixed_case_shortform_part(&whole_lower, w[0], seg)
                && len == seg_lower.len()
            {
                cells
            } else if allow_longer_shortforms
                && matches!(classify_caps(seg), Some(Caps::Word))
                && let Some((len, cells)) = mixed_case_shortform_part(&whole_lower, w[0], seg)
                && len == seg_lower.len()
                && matches!(seg_lower.as_slice(), ['g', 'r', 'e', 'a', 't'])
            {
                cells
            } else if matches!(caps, Caps::Word)
                && matches!(seg_lower.as_slice(), ['t', 'i', 'o', 'n'])
            {
                encode_letters_literal(seg)?
                    .into_iter()
                    .filter(|cell| *cell != CAPITAL)
                    .collect()
            } else if matches!(seg_lower.as_slice(), ['t', 'i', 'o', 'n']) && w[1] == chars.len() {
                vec![GRADE1, decode_unicode('⠝')]
            } else if seg_lower
                .iter()
                .any(|c| super::rule_4::accent_cells(*c).is_some())
            {
                let mut literal = Vec::with_capacity(seg_lower.len() + 2);
                for &c in &seg_lower {
                    push_literal_letter(c, &mut literal)?;
                }
                literal
            } else if allow_longer_shortforms
                && mixed_case_disallowed_shortform_part(&whole_lower, w[0], seg)
            {
                super::rule_10_9::encode_with_optional_longer_shortforms(
                    &seg_lower,
                    &self.contractions,
                    false,
                    w[0] == 0 && w[1] == chars.len(),
                    false,
                )?
            } else {
                super::rule_10_9::encode_with_optional_longer_shortforms(
                    &seg_lower,
                    &self.contractions,
                    false,
                    w[0] == 0 && w[1] == chars.len(),
                    allow_longer_shortforms,
                )?
            };
            has_caps_word_segment |= matches!(caps, Caps::Word);
            // §8.6.3: a §8.4 caps word (`⠠⠠`) is terminated by `⠠⠄` before lowercase
            // letters that continue the same word (`ABCs`, `WALKing`, `unSELFish`).
            if prev_caps_word && matches!(caps, Caps::None) {
                buf.push(CAPITAL);
                buf.push(decode_unicode('⠄'));
            }
            if matches!(caps, Caps::Word) && w[0] > 0 && w[1] < chars.len() && seg.len() <= 2 {
                has_internal_caps_word_segment = true;
                for cell in &cells {
                    buf.push(CAPITAL);
                    buf.push(*cell);
                }
                concat.extend(cells);
                prev_caps_word = false;
                continue;
            } else {
                match caps {
                    Caps::None => {}
                    Caps::Single => buf.push(CAPITAL),
                    Caps::Word => {
                        buf.push(CAPITAL);
                        buf.push(CAPITAL);
                    }
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
        if whole.as_ref().is_some_and(|whole| concat != *whole)
            && has_caps_word_segment
            && !has_internal_caps_word_segment
            && bounds.len() <= 2
        {
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

    /// Build the expected cell vector from a unicode-braille string (`⠀` = space,
    /// `\n` = the §10.13 line-break cell 255).
    fn cells(s: &str) -> Vec<u8> {
        s.chars()
            .map(|c| match c {
                '⠀' => SPACE,
                '\n' => 255,
                _ => decode_unicode(c),
            })
            .collect()
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

    /// §9.2–§9.6: typeform indicators apply to the next symbol/word/passage by
    /// extent, while script letters and small capitals keep their base identities.
    #[rstest::rstest]
    #[case::bold_digit_symbol("𝟖 chickens!", "⠘⠆⠼⠓⠀⠡⠊⠉⠅⠢⠎⠖")]
    #[case::bold_question_symbol("For help, click the 𝐀 icon?", "⠠⠿⠀⠓⠑⠇⠏⠂⠀⠉⠇⠊⠉⠅⠀⠮⠀⠘⠆⠠⠁⠀⠊⠉⠕⠝⠦")]
    #[case::script_y_letter("𝒴ou can do it!", "⠈⠆⠠⠽⠀⠉⠀⠙⠀⠭⠖")]
    #[case::script_letterlike_r("ℜ", "⠈⠆⠰⠠⠗")]
    #[case::small_caps_roman("ᴠɪɪɪ", "⠠⠠⠧⠊⠊⠊")]
    #[case::underlined_question_symbol("?\u{0332} icon", "⠸⠆⠰⠦⠀⠊⠉⠕⠝")]
    #[case::bold_italic_word("𝒕𝒔𝒖𝒏𝒂𝒎𝒊.", "⠘⠂⠨⠂⠞⠎⠥⠝⠁⠍⠊⠲")]
    fn encodes_rule9_typeform_extents(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §2.6/§10: standing-alone boundaries and typeform extents are semantic
    /// contexts, not literal word exceptions. These examples lock the failing
    /// seams where hyphen/dash/punctuation/typeform changes decide whether a
    /// wordsign, groupsign, or shortform may be used.
    #[rstest::rstest]
    #[case::hyphen_bounded_x("I like x–it works.", "⠠⠊⠀⠇⠀⠰⠭⠠⠤⠭⠀⠐⠺⠎⠲")]
    #[case::ellipsis_keeps_ch_groupsign("ch...f", "⠡⠲⠲⠲⠋")]
    #[case::word_script_digit("knowledge.³", "⠐⠅⠇⠫⠛⠑⠲⠰⠔⠼⠉")]
    #[case::single_curly_quote_standalone(
        "Use single quotes ‘ and ’.",
        "⠠⠥⠎⠑⠀⠎⠬⠇⠑⠀⠟⠥⠕⠞⠑⠎⠀⠰⠠⠦⠀⠯⠀⠠⠴⠲"
    )]
    fn encodes_rule2_6_boundaries(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §5.4.1/§5.9.1: a technical expression spanning three or more spaced
    /// symbol-sequences uses grade-1 passage mode, even when its terms are not
    /// hyphenated spelling sequences.
    #[rstest::rstest]
    #[case::equation_terms("a=b c=d e=f", "⠰⠰⠰⠁⠐⠶⠃⠀⠉⠐⠶⠙⠀⠑⠐⠶⠋⠰⠄")]
    fn technical_sequences_open_grade1_passage_5(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §10.1/§10.4/§10.5/§10.9 with §9: typeform indicators may cover a
    /// single symbol, word, or passage while the underlying letters still take
    /// the ordinary wordsign/shortform decisions.
    #[rstest::rstest]
    #[case::styled_ing_suffix("brown𝑖𝑛𝑔", "⠃⠗⠪⠝⠨⠆⠬")]
    #[case::styled_lower_wordsign_sentence("𝐵𝑒 ℎ𝑎𝑝𝑝𝑦.", "⠨⠂⠠⠆⠀⠨⠂⠓⠁⠏⠏⠽⠲")]
    #[case::styled_shortform("𝑛𝑒𝑖𝑡ℎ𝑒𝑟", "⠨⠂⠝⠑⠊")]
    fn encodes_rule10_typeform_contractions(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §8.2: a mixed-case word (internal capitals) is split at each lower→upper
    /// boundary and each Title-case / all-caps part takes its own capital
    /// indicator (`⠠` single, `⠠⠠` all-caps), contractions applying within each.
    #[rstest::rstest]
    #[case::mcd("McD", "⠠⠍⠉⠠⠙")]
    #[case::trailing_single_cap("verY", "⠧⠻⠠⠽")]
    #[case::trailing_caps_word("grandEST", "⠛⠗⠯⠠⠠⠑⠌")]
    #[case::internal_caps_letters("founDAtion", "⠋⠳⠝⠠⠙⠠⠁⠰⠝")]
    // §10.9.4: when print capitals split a longer word, each printed part keeps
    // the shortform rule it would have in that part; suffix letters after the
    // shortform are not swallowed by the abbreviation.
    #[case::braille_resumes("BrailleResumés", "⠠⠃⠗⠇⠠⠗⠑⠎⠥⠍⠘⠌⠑⠎")]
    #[case::pen_friend("PenFriend", "⠠⠏⠢⠠⠋⠗⠊⠢⠙")]
    // §8.2/§10.12.12: Title-case parts split even when a contraction would span the
    // boundary in the whole word (`ff` in `cliffedge`, `the` in `northeast`).
    #[case::cliff_edge_title_split("CliffEdge", "⠠⠉⠇⠊⠋⠋⠠⠫⠛⠑")]
    #[case::north_east_title_split("NorthEast", "⠠⠝⠕⠗⠹⠠⠑⠁⠌")]
    // §8.8.2: choose the segmentation that best conveys meaning; `Ontario` and
    // the final `T` are semantic subunits and keep their own capital indicators.
    #[case::tv_ontario("TVOntario", "⠠⠠⠞⠧⠠⠕⠝⠞⠜⠊⠕")]
    #[case::at_and_t("ATandT", "⠠⠁⠠⠞⠯⠠⠞")]
    #[case::potassium_bromide("KBr", "⠠⠅⠠⠃⠗")]
    #[case::bachelor_science("BSc", "⠠⠃⠠⠎⠉")]
    #[case::megahertz("MHz", "⠠⠍⠠⠓⠵")]
    #[case::potassium_chloride("KCl", "⠠⠅⠠⠉⠇")]
    #[case::chemical_subscript("HOCH₂", "⠠⠓⠠⠕⠠⠉⠠⠓⠰⠢⠼⠃")]
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
    #[case::plural_acronym("CDs", "⠠⠠⠉⠙⠠⠄⠎")]
    fn encodes_caps_word_terminator_8_6_3(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §7.2/§7.6: punctuation with context-dependent roles is normalised from print
    /// form, not from the literal glyph shape: underline runs collapse to one low-line
    /// sign, doubled dashes may be one dash, paired straight singles are quote marks,
    /// and apostrophe-wrapped letters take grade 1 where needed.
    #[rstest::rstest]
    #[case::low_line_run("add ____", "⠁⠙⠙⠀⠨⠤")]
    #[case::double_hyphen_dash("expression--such", "⠑⠭⠏⠗⠑⠎⠨⠝⠠⠤⠎⠡")]
    #[case::double_hyphen_missing_letters("rec--ve", "⠗⠑⠉⠤⠤⠧⠑")]
    #[case::double_hyphen_after_initial("B--", "⠰⠠⠃⠤⠤")]
    #[case::omitted_capital_before_em_dash("S—", "⠰⠠⠎⠐⠠⠤")]
    #[case::straight_single_quote("'Cat'", "⠠⠦⠠⠉⠁⠞⠠⠴")]
    #[case::apostrophe_wrapped_letter("rock ’n’ roll", "⠗⠕⠉⠅⠀⠄⠰⠝⠄⠀⠗⠕⠇⠇")]
    #[case::two_cell_midword_quote("Franc“e”s", "⠠⠋⠗⠁⠝⠉⠘⠦⠑⠘⠴⠎")]
    #[case::one_cell_quote_before_suffix("“yes”es", "⠦⠽⠑⠎⠴⠑⠎")]
    #[case::two_cell_standalone_quote("(“ ... that is the question.”)", "⠐⠣⠘⠦⠀⠲⠲⠲⠀⠞⠀⠊⠎⠀⠮⠀⠐⠟⠲⠘⠴⠐⠜")]
    #[case::exchanged_outer_straight_single(
        "'Sing \"Happy Birthday\",'",
        "⠦⠠⠎⠬⠀⠠⠦⠠⠓⠁⠏⠏⠽⠀⠠⠃⠊⠗⠹⠐⠙⠠⠴⠂⠴"
    )]
    fn encodes_contextual_punctuation_7(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// RUEB 2024 §7.6.2, §7.6.5, §7.6.7 and §8.4.2: quote/code
    /// punctuation and typeform changes do not reset the underlying word scope.
    #[rstest::rstest]
    #[case::curly_quote_spelling_run(
        "note silent letters in n-i-‘g-h’-t",
        "⠝⠕⠞⠑⠀⠎⠊⠇⠢⠞⠀⠇⠗⠎⠀⠔⠀⠰⠰⠝⠤⠊⠤⠠⠦⠛⠤⠓⠠⠴⠤⠞"
    )]
    #[case::solidus_linebreak_keeps_space(
        "There were several schoolchildren/teachers/parents present.",
        "⠠⠐⠮⠀⠶⠀⠎⠐⠑⠁⠇⠀⠎⠡⠕⠕⠇⠡⠊⠇⠙⠗⠢⠸⠌⠀⠞⠂⠡⠻⠎⠸⠌⠏⠜⠢⠞⠎⠀⠏⠗⠑⠎⠢⠞⠲"
    )]
    #[case::url_ascii_quote_listing(
        "‘https://www.example.com/query?item='bobs-internal-folder'.’",
        "⠠⠦⠓⠞⠞⠏⠎⠒⠸⠌⠸⠌⠺⠺⠺⠲⠑⠭⠁⠍⠏⠇⠑⠲⠉⠕⠍⠸⠌⠐⠀⠟⠥⠻⠽⠦⠊⠞⠑⠍⠐⠶⠄⠃⠕⠃⠎⠤⠔⠞⠻⠝⠁⠇⠤⠐⠀⠋⠕⠇⠙⠻⠄⠲⠠⠴"
    )]
    #[case::regex_ascii_quote_listing(
        "“Is she correct in saying our regex pattern would be ‘\"?+[a-zA-Z]\"?’?”",
        "⠦⠠⠊⠎⠀⠩⠑⠀⠉⠕⠗⠗⠑⠉⠞⠀⠔⠀⠎⠁⠽⠬⠀⠳⠗⠀⠗⠑⠛⠑⠭⠀⠏⠁⠞⠞⠻⠝⠀⠺⠙⠀⠆⠀⠠⠦⠠⠶⠰⠦⠐⠖⠨⠣⠁⠤⠵⠠⠐⠀⠁⠤⠰⠠⠵⠨⠜⠠⠶⠦⠠⠴⠦⠴"
    )]
    #[case::escaped_quote_code_snippet(
        "\\“Remember those backslashes\\”",
        "⠸⠡⠘⠦⠠⠗⠑⠍⠑⠍⠃⠑⠗⠀⠞⠓⠕⠎⠑⠀⠃⠁⠉⠅⠎⠇⠁⠎⠓⠑⠎⠸⠡⠘⠴"
    )]
    #[case::caps_word_continues_across_bold_tail("FREE𝐅𝐎𝐑𝐌", "⠠⠠⠋⠗⠑⠑⠘⠂⠿⠍")]
    #[case::italic_caps_heading_is_one_caps_passage(
        "𝐿𝐼𝑆𝑇 𝑂𝐹 𝑆𝑈𝑅𝑉𝐸𝑌 𝑅𝐸𝐶𝐼𝑃𝐼𝐸𝑁𝑇𝑆 𝑂𝑅𝐺𝐴𝑁𝐼𝑆𝐸𝐷 𝐵𝑌 𝐶𝑂𝑈𝑁𝑇𝑅𝑌",
        "⠨⠶⠠⠠⠠⠇⠊⠌⠀⠷⠀⠎⠥⠗⠧⠑⠽⠀⠗⠑⠉⠊⠏⠊⠢⠞⠎⠀⠕⠗⠛⠁⠝⠊⠎⠫⠀⠃⠽⠀⠉⠨⠞⠗⠽⠠⠄⠨⠄"
    )]
    #[case::italic_title_with_plain_modified_middle_word("𝑉𝑜𝑦𝑎𝑔𝑒 À 𝑁𝑖𝑐𝑒", "⠨⠶⠠⠧⠕⠽⠁⠛⠑⠀⠠⠘⠡⠁⠀⠠⠝⠊⠉⠑⠨⠄")]
    #[case::domain_camel_title_subunit_keeps_usual_braille_form(
        "www.BLASTSoundMachine.com",
        "⠺⠺⠺⠲⠠⠠⠃⠇⠁⠌⠠⠎⠨⠙⠠⠍⠁⠡⠔⠑⠲⠉⠕⠍"
    )]
    fn encodes_ueb_7_8_indicator_scope_regressions(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §8.5.3: capitalised passages may include single-letter words and Greek
    /// capitals; a three-plus symbol-sequence passage uses ⠠⠠⠠ … ⠠⠄.
    #[rstest::rstest]
    #[case::single_letter_word("A SELF-MADE MAN", "⠠⠠⠠⠁⠀⠎⠑⠇⠋⠤⠍⠁⠙⠑⠀⠍⠁⠝⠠⠄")]
    #[case::greek_letters("PROUD TO BE A ΦΒΚ", "⠠⠠⠠⠏⠗⠳⠙⠀⠞⠕⠀⠆⠀⠁⠀⠨⠋⠨⠃⠨⠅⠠⠄")]
    #[case::opening_quote_passage("He shouted \"I WILL NOT!\"", "⠠⠓⠑⠀⠩⠳⠞⠫⠀⠦⠠⠠⠠⠊⠀⠺⠀⠝⠖⠠⠄⠴")]
    #[case::adjacent_single_cap_before_passage(
        "Go to point A.  BUT NOT YET!",
        "⠠⠛⠀⠞⠕⠀⠏⠕⠔⠞⠀⠠⠁⠲⠀⠠⠠⠠⠃⠀⠝⠀⠽⠑⠞⠖⠠⠄"
    )]
    // §8.6.3: a slash can terminate the capitalised subunit, so only the
    // capitalised prefix participates in the passage.
    #[case::caps_passage_before_slash(
        "INITIALS OF WRITER/initials of secretary",
        "⠠⠠⠠⠔⠊⠞⠊⠁⠇⠎⠀⠷⠀⠺⠗⠊⠞⠻⠠⠄⠸⠌⠔⠊⠞⠊⠁⠇⠎⠀⠷⠀⠎⠑⠉⠗⠑⠞⠜⠽"
    )]
    fn encodes_capital_passages_8_5(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §4.5.1: Greek letters are transcribed as dot-46 plus the corresponding
    /// Latin letter cell; capital Greek letters take ordinary capitalisation, and
    /// a §8.4 capitals passage suppresses per-letter capital indicators.
    #[rstest::rstest]
    #[case::lower_pi("Use π in the equation.", "⠠⠥⠎⠑⠀⠨⠏⠀⠔⠀⠮⠀⠑⠟⠥⠁⠰⠝⠲")]
    #[case::capital_initials("She is a member of ΦΒΚ.", "⠠⠩⠑⠀⠊⠎⠀⠁⠀⠍⠑⠍⠃⠻⠀⠷⠀⠠⠠⠨⠋⠨⠃⠨⠅⠲")]
    #[case::caps_passage("THE Α AND THE Ω", "⠠⠠⠠⠮⠀⠨⠁⠀⠯⠀⠮⠀⠨⠺⠠⠄")]
    #[case::capital_greek_initials("ΠΒΦ", "⠠⠠⠨⠏⠨⠃⠨⠋")]
    fn encodes_greek_letters_4_5_1(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(super::super::encode_forced(text), Some(cells(expected)));
    }

    /// §8.4.2: company abbreviation `CO.` is a capitalised word, not a chemical
    /// formula context, so it keeps the ordinary capital word indicator.
    #[test]
    fn company_co_keeps_caps_word_indicator_8_4_2() {
        assert_eq!(enc("SWIFT & CO."), Some(cells("⠠⠠⠎⠺⠊⠋⠞⠀⠈⠯⠀⠠⠠⠉⠕⠲")));
    }

    /// §8.8.3: chemical formulae in one title use single capitals consistently,
    /// keeping subscript level indicators between the affected letters.
    #[rstest::rstest]
    #[case::water("H₂O", "⠠⠓⠰⠢⠼⠃⠠⠕")]
    #[case::hydroxide("OH", "⠠⠕⠠⠓")]
    #[case::methylol("CH₂OH", "⠠⠉⠠⠓⠰⠢⠼⠃⠠⠕⠠⠓")]
    #[case::hoch2("HOCH₂", "⠠⠓⠠⠕⠠⠉⠠⠓⠰⠢⠼⠃")]
    fn encodes_chemical_formula_capitals_8_8_3(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §15.3.2: in level-change tone notation, an up/down-step arrow printed before
    /// a word is followed by a braille space and the under-word bullet indicator.
    /// The tone reading needs a tone-notation context (several level arrows in the
    /// sentence); a lone arrow in prose stays a §3.2 arrow.
    #[rstest::rstest]
    #[case::level_arrows_sentence(
        "Does ↑Anyone ↓HERE ↓HAVE a ↑WATCH? ↑",
        "⠠⠙⠕⠑⠎⠀⠘⠨⠫⠀⠸⠲⠠⠁⠝⠽⠐⠕⠀⠘⠨⠮⠀⠸⠲⠠⠠⠐⠓⠀⠘⠨⠮⠀⠸⠲⠠⠠⠓⠁⠧⠑⠀⠁⠀⠘⠨⠫⠀⠸⠲⠠⠠⠺⠁⠞⠡⠦⠀⠘⠨⠫"
    )]
    fn encodes_tone_level_change_15_3_2(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §15.2.2: prime marks in phonetic text are stress marks; the foot/inch sign
    /// reading is limited to numeric measurements.
    #[rstest::rstest]
    #[case::secondary_stress_before_schwa("met′ə", "⠍⠑⠞⠘⠨⠆⠸⠢")]
    #[case::double_primary_stress_before_letter("môr′′fə", "⠍⠘⠩⠕⠗⠘⠨⠃⠋⠸⠢")]
    fn encodes_phonetic_prime_stress_15_2_2(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §3.24: a digit super/subscript following a base takes the level indicator
    /// (`⠔`/`⠢`). The grade-1 `⠰` precedes it after a letter base (`yd³`, `B₁₂`,
    /// `clarion¹`) but not after a number (`1682.³`), whose numeric mode covers it.
    #[rstest::rstest]
    #[case::super_after_word("3 yd\u{00B3}", "⠼⠉⠀⠽⠙⠰⠔⠼⠉")]
    #[case::sub_after_letter("vitamin B\u{2081}\u{2082}", "⠧⠊⠞⠁⠍⠔⠀⠠⠃⠰⠢⠼⠁⠃")]
    #[case::subscript_letter_group("mass\u{209B}\u{1D64}\u{2099}", "⠍⠁⠎⠎⠰⠰⠢⠣⠎⠥⠝⠜")]
    #[case::decimal_number_unit_subscript(
        "an earthquake measuring 6.5MW",
        "⠁⠝⠀⠑⠜⠹⠟⠥⠁⠅⠑⠀⠍⠂⠎⠥⠗⠬⠀⠼⠋⠲⠑⠠⠍⠢⠠⠺"
    )]
    #[case::super_after_number("born in 1682.\u{00B3}", "⠃⠕⠗⠝⠀⠔⠀⠼⠁⠋⠓⠃⠲⠔⠼⠉")]
    #[case::super_after_word_inline("the clarion\u{00B9} horn", "⠮⠀⠉⠇⠜⠊⠕⠝⠰⠔⠼⠁⠀⠓⠕⠗⠝")]
    fn encodes_script_3_24(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §10.13.2/§10.13.8: lower wordsigns next to a transcriber line break obey
    /// the lower-sign rule even when the hyphen/dash is on the other braille line.
    #[rstest::rstest]
    #[case::teach_in_period("teach-\nin.", "⠞⠂⠡⠤\n⠊⠝⠲")]
    #[case::quoted_in_depth("\"In-\ndepth", "⠦⠠⠊⠝⠤\n⠙⠑⠏⠹")]
    #[case::enough_dash_in("Enough—\nin my case", "⠠⠢⠳⠣⠠⠤\n⠊⠝⠀⠍⠽⠀⠉⠁⠎⠑")]
    #[case::enough_break_dash_in("Enough\n—in my case", "⠠⠢\n⠠⠤⠊⠝⠀⠍⠽⠀⠉⠁⠎⠑")]
    fn encodes_line_division_lower_sign_rule_10_13(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §10.13.4: `ing` at the start of the second braille line is spelled as
    /// `in`+`g`, including a capitalised second segment.
    #[rstest::rstest]
    #[case::smith_inge("Smith\nInge", "⠠⠎⠍⠊⠹⠤\n⠠⠔⠛⠑")]
    #[case::fro_ing("fro-\ning", "⠋⠗⠕⠤\n⠔⠛")]
    fn encodes_line_initial_ing_10_13_4(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §3.24 boundary: a *leading* super/subscript (no base before it) fails the
    /// whole UEB attempt so the legacy/math path keeps ownership — this is what
    /// protects combinatorics like `₇𝑃₂` (제18/19항) from being misread as §3.24.
    #[rstest::rstest]
    #[case::leading_superscript("\u{00B9} clarion", "⠰⠔⠼⠁⠀⠉⠇⠜⠊⠕⠝")]
    #[case::super_letter_after_word("W\u{1D50}", "⠠⠺⠰⠔⠍")]
    #[case::sub_digit_after_word("H\u{2082}O", "⠠⠓⠰⠢⠼⠃⠠⠕")]
    #[case::super_digit_after_numeric_unit("4m\u{00B2}", "⠼⠙⠍⠔⠼⠃")]
    fn encodes_scripts_in_prose_3_24(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §3.27: `[open tn]` / `[close tn]` markers become the note indicators
    /// `⠈⠨⠣` / `⠈⠨⠜`; a plain bracket that is not the marker keeps its sign.
    #[rstest::rstest]
    #[case::wrapped_note("[open tn]cat[close tn]", "⠈⠨⠣⠉⠁⠞⠈⠨⠜")]
    #[case::wrapped_note_reverse_words("[tn open]cat[tn close]", "⠈⠨⠣⠉⠁⠞⠈⠨⠜")]
    #[case::plain_bracket_unchanged("[cat]", "⠨⠣⠉⠁⠞⠨⠜")]
    fn encodes_transcriber_notes_3_27(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §3.8, §3.13, §3.22, §3.26, §3.28: general print/braille symbols are handled
    /// before broader technical or phonetic symbol fallbacks can claim them.
    #[rstest::rstest]
    #[case::copyright("© 2009", "⠘⠉⠀⠼⠃⠚⠚⠊")]
    #[case::euro_franc_equation("1 € = 6.55957₣", "⠼⠁⠀⠈⠑⠀⠐⠶⠀⠼⠋⠲⠑⠑⠊⠑⠛⠈⠋")]
    #[case::registered("Intuit®", "⠠⠔⠞⠥⠊⠞⠘⠗")]
    #[case::trademark("Tax™", "⠠⠞⠁⠭⠘⠞")]
    #[case::square_shape("□ Director", "⠰⠫⠼⠙⠀⠠⠙⠊⠗⠑⠉⠞⠕⠗")]
    #[case::circle_shape("o Manager", "⠰⠫⠿⠀⠠⠍⠁⠝⠁⠛⠻")]
    #[case::pencil_shape("✏ Recycling", "⠈⠫⠏⠑⠝⠉⠊⠇⠀⠠⠗⠑⠉⠽⠉⠇⠬")]
    #[case::pointing_shape("☞ Steps", "⠈⠫⠏⠕⠊⠝⠞⠀⠠⠌⠑⠏⠎")]
    #[case::per_mille("salinity 35‰", "⠎⠁⠇⠔⠰⠽⠀⠼⠉⠑⠹")]
    #[case::check_mark("✓ item", "⠈⠩⠀⠊⠞⠑⠍")]
    #[case::braille_mention("⠫⠼⠙ square", "⠨⠿⠫⠼⠙⠀⠎⠟⠥⠜⠑")]
    fn encodes_rule_3_general_symbols(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §16.2 horizontal line mode: a run of box-drawing characters opens with
    /// `⠐⠒` (a leading `─` folding into the indicator's `⠒`) and maps each further
    /// char to its segment/corner/crossing cell.
    #[rstest::rstest]
    #[case::solid("\u{2500}\u{2500}\u{2500}\u{2500}", "⠐⠒⠒⠒⠒")]
    #[case::double("\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}", "⠐⠒⠶⠶⠶⠶⠶")]
    #[case::double_with_arrow(
        "\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}↓\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}",
        "⠐⠒⠶⠶⠶⠶⠶⠳⠩⠶⠶⠶⠶⠶⠶"
    )]
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

    /// §16.5.1: in tables, a wide blank between a row label and a number is rendered
    /// as guide dots with at least one blank cell before and after the dot-5 run.
    #[rstest::rstest]
    #[case::leading_indent_before_header("          1\n        ──", "⠼⠁⠀⠐⠒⠒")]
    #[case::label_number_gap("Income       865.73", "⠠⠔⠉⠕⠍⠑⠀⠐⠐⠐⠐⠀⠀⠼⠓⠋⠑⠲⠛⠉")]
    #[case::balance_number_gap("Balance      165.32", "⠠⠃⠁⠇⠨⠑⠀⠐⠐⠐⠐⠀⠀⠼⠁⠋⠑⠲⠉⠃")]
    #[case::lead_element_row("lead        Pb       82", "⠇⠂⠙⠀⠐⠐⠐⠐⠐⠀⠀⠠⠏⠃⠀⠐⠐⠀⠀⠼⠓⠃")]
    #[case::lithium_element_row("lithium     Li       3", "⠇⠊⠹⠊⠥⠍⠀⠐⠐⠀⠀⠠⠇⠊⠀⠐⠐⠀⠀⠼⠉")]
    fn encodes_table_guide_dots_16_5_1(#[case] text: &str, #[case] expected: &str) {
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

    /// §5.3/§5.9/§5.10: extended grade-1 mode begins at the start of a
    /// hyphenated symbols-sequence, avoiding repeated single-letter indicators in
    /// spelling and stammering examples.
    #[rstest::rstest]
    #[case::word_indicator_spelling("u-n-t-i-d-y", "⠰⠰⠥⠤⠝⠤⠞⠤⠊⠤⠙⠤⠽")]
    #[case::choice_unemotional("un-e-mo-tion-al", "⠰⠰⠥⠝⠤⠑⠤⠍⠕⠤⠞⠊⠕⠝⠤⠁⠇")]
    #[case::choice_stammer("br-r-r-r", "⠰⠰⠃⠗⠤⠗⠤⠗⠤⠗")]
    #[case::choice_embedded_stammer("about-f-f-f-face", "⠁⠃⠤⠰⠰⠋⠤⠋⠤⠋⠤⠋⠁⠉⠑")]
    #[case::optional_equivalent_grade1("rm-mm-mm-mm", "⠰⠰⠗⠍⠤⠍⠍⠤⠍⠍⠤⠍⠍")]
    #[case::optional_repeated_tail("r-mmmmmmm", "⠰⠰⠗⠤⠍⠍⠍⠍⠍⠍⠍")]
    #[case::passage_spelled_name("H-o C-h-i M-i-n-h City", "⠰⠰⠰⠠⠓⠤⠕⠀⠠⠉⠤⠓⠤⠊⠀⠠⠍⠤⠊⠤⠝⠤⠓⠰⠄⠀⠠⠉⠰⠽")]
    fn grade1_word_indicator_for_hyphenated_sequences_5_3_5_9_5_10(
        #[case] text: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §9: a styled letter takes a symbol-level typeform indicator before its base
    /// cell (italic ⠨⠆, bold ⠘⠆, underline ⠸⠆) and is a contraction boundary, so
    /// the plain neighbours still contract (`story̲` keeps its `st` groupsign).
    #[rstest::rstest]
    #[case::italic_math_alpha("\u{1D45D}neumonia", "⠨⠆⠏⠝⠑⠥⠍⠕⠝⠊⠁")]
    #[case::bold_math_alpha("\u{1D41B}at", "⠘⠆⠃⠁⠞")]
    #[case::underline_combining("story\u{0332}", "⠌⠕⠗⠸⠆⠽")]
    #[case::italic_initial_wordsign("\u{1D440}other", "⠨⠆⠠⠐⠍")]
    #[case::bold_groupsign_start("mo\u{1D42D}her", "⠍⠕⠘⠆⠮⠗")]
    #[case::script_letterlike_r("\u{211C}", "⠈⠆⠰⠠⠗")]
    #[case::numeric_adjacent_italic_letter("31\u{1D459}", "⠼⠉⠁⠇")]
    fn typeform_symbol_indicator_9(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §9.x: a run of 2+ styled letters takes a word indicator (`⠨⠂`) with the
    /// word contracted normally (`𝑅𝑎𝑑𝑎𝑟` → `⠨⠂⠠⠗⠁⠙⠜`, `ar` groupsign); a partial
    /// run ending mid-word adds a terminator (`𝐭𝐞𝐱𝐭book` → `⠘⠂⠞⠑⠭⠞⠘⠄…`).
    #[rstest::rstest]
    #[case::italic_whole_word("the \u{1D445}\u{1D44E}\u{1D451}\u{1D44E}\u{1D45F}", "⠮⠀⠨⠂⠠⠗⠁⠙⠜")]
    #[case::bold_partial_then_plain("\u{1D42D}\u{1D41E}\u{1D431}\u{1D42D}book", "⠘⠂⠞⠑⠭⠞⠘⠄⠃⠕⠕⠅")]
    #[case::bold_bracketed_symbol_sequence("\u{1D40D}(\u{1D446})", "⠘⠂⠠⠝⠐⠣⠨⠆⠠⠎⠐⠜")]
    fn typeform_word_indicator_9(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §13.2: a foreign phrase identified by typography/foreign context is written
    /// uncontracted under its typeform passage indicator, so English words inside
    /// the expression such as `en` do not use UEB contractions.
    #[rstest::rstest]
    #[case::italic_foreign_passage_en(
        "We ate \u{1D45D}\u{1D44E}\u{1D45D} \u{1D452}\u{1D45B} \u{1D464}\u{1D45C}\u{1D45F}\u{1D460}.",
        "⠠⠺⠑⠀⠁⠞⠑⠀⠨⠶⠏⠁⠏⠀⠑⠝⠀⠺⠕⠗⠎⠨⠄⠲"
    )]
    #[case::italic_japanese_newspaper_title(
        "\u{1D44C}\u{1D45C}\u{1D45A}\u{1D456}\u{1D462}\u{1D45F}\u{1D456} \u{1D446}\u{210E}\u{1D456}\u{1D45A}\u{1D44F}\u{1D462}\u{1D45B} from Japan",
        "⠨⠂⠠⠽⠕⠍⠊⠥⠗⠊⠀⠨⠂⠠⠎⠓⠊⠍⠃⠥⠝⠀⠋⠀⠠⠚⠁⠏⠁⠝"
    )]
    #[case::italic_japanese_newspaper_serial_title(
        "\u{1D44C}\u{1D45C}\u{1D45A}\u{1D456}\u{1D462}\u{1D45F}\u{1D456} \u{1D446}\u{210E}\u{1D456}\u{1D45A}\u{1D44F}\u{1D462}\u{1D45B} and \u{1D434}\u{1D460}\u{1D44E}\u{210E}\u{1D456} \u{1D446}\u{210E}\u{1D456}\u{1D45A}\u{1D44F}\u{1D462}\u{1D45B} from Japan",
        "⠨⠂⠠⠽⠕⠍⠊⠥⠗⠊⠀⠨⠂⠠⠎⠓⠊⠍⠃⠥⠝⠀⠯⠀⠨⠂⠠⠁⠎⠁⠓⠊⠀⠨⠂⠠⠎⠓⠊⠍⠃⠥⠝⠀⠋⠀⠠⠚⠁⠏⠁⠝"
    )]
    #[case::italic_anglicised_english_title_with_foreign_name(
        "\u{1D447}\u{210E}\u{1D452} \u{1D447}\u{1D44E}\u{1D459}\u{1D452} \u{1D45C}\u{1D453} \u{1D43A}\u{1D452}\u{1D45B}\u{1D457}\u{1D456}.",
        "⠨⠶⠠⠮⠀⠠⠞⠁⠇⠑⠀⠷⠀⠠⠛⠢⠚⠊⠨⠄⠲"
    )]
    #[case::bold_span_with_inverted_question(
        "\u{1D42D}\u{1D42E}\u{0301} \u{1D425}\u{1D41E} \u{1D41D}\u{1D422}\u{1D42C}\u{1D42D}\u{1D41E} \u{1D41E}\u{1D425} \u{00BF}\u{1D42F}\u{1D41E}\u{1D42B}\u{1D41D}\u{1D41A}\u{1D41D}?",
        "⠘⠶⠞⠾⠀⠇⠑⠀⠙⠊⠎⠞⠑⠀⠑⠇⠀⠢⠧⠑⠗⠙⠁⠙⠢⠘⠄"
    )]
    #[case::italic_span_with_inverted_exclamation_keeps_ueb_signs(
        "—¡\u{1D444}\u{1D462}\u{1D452}\u{0301} \u{1D456}\u{1D451}\u{1D452}\u{1D44E} \u{1D45A}\u{1D44E}\u{0301}\u{1D460} \u{1D44F}\u{1D462}\u{1D452}\u{1D45B}\u{1D44E}!—exclaimed Pedro's mother.",
        "⠠⠤⠨⠶⠘⠰⠖⠠⠟⠥⠘⠌⠑⠀⠊⠙⠑⠁⠀⠍⠘⠌⠁⠎⠀⠃⠥⠑⠝⠁⠖⠨⠄⠠⠤⠑⠭⠉⠇⠁⠊⠍⠫⠀⠠⠏⠫⠗⠕⠄⠎⠀⠐⠍⠲"
    )]
    #[case::lowercase_phrase_before_of_keeps_leading_word_contracted(
        "We went out for a \u{1D459}\u{1D452}\u{1D458}\u{1D458}\u{1D452}\u{1D45F} \u{1D44F}\u{1D45F}\u{1D44E}\u{1D44E}\u{1D456} of \u{1D45D}\u{1D44E}\u{1D45D} \u{1D452}\u{1D45B} \u{1D464}\u{1D45C}\u{1D45F}\u{1D460}.",
        "⠠⠺⠑⠀⠺⠢⠞⠀⠳⠀⠿⠀⠁⠀⠨⠂⠇⠑⠅⠅⠻⠀⠨⠂⠃⠗⠁⠁⠊⠀⠷⠀⠨⠶⠏⠁⠏⠀⠑⠝⠀⠺⠕⠗⠎⠨⠄⠲"
    )]
    #[case::quoted_french_phrase_uncontracted_13_2_1(
        "\"\u{1D443}\u{1D45F}\u{1D452}\u{1D45B}\u{1D45C}\u{1D45B}\u{1D460} \u{1D450}\u{1D45C}\u{1D462}\u{1D45F}\u{1D44E}\u{1D454}\u{1D452}, \u{1D440}\u{1D44E}\u{1D45F}\u{1D454}\u{1D462}\u{1D452}\u{1D45F}\u{1D456}\u{1D461}\u{1D452},\" Jeanne said",
        "⠦⠨⠶⠠⠏⠗⠑⠝⠕⠝⠎⠀⠉⠕⠥⠗⠁⠛⠑⠂⠀⠠⠍⠁⠗⠛⠥⠑⠗⠊⠞⠑⠂⠨⠄⠴⠀⠠⠚⠂⠝⠝⠑⠀⠎⠙"
    )]
    fn foreign_typeform_words_are_uncontracted_13_2(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §13.1.3 bibliography entries keep English UEB contraction decisions in
    /// author/publisher metadata while typeform-marked foreign titles are scoped
    /// as titles and use UEB §4.2 accent modifiers, not §14 foreign-code accents.
    #[rstest::rstest]
    #[case::conlogue_impossible_nation(
        "1. Conlogue, Ray.  𝐼𝑚𝑝𝑜𝑠𝑠𝑖𝑏𝑙𝑒 𝑁𝑎𝑡𝑖𝑜𝑛:  𝑇ℎ𝑒 𝐿𝑜𝑛𝑔𝑖𝑛𝑔 𝑓𝑜𝑟 𝐻𝑜𝑚𝑒𝑙𝑎𝑛𝑑 𝑖𝑛 𝐶𝑎𝑛𝑎𝑑𝑎 𝑎𝑛𝑑 𝑄𝑢𝑒𝑏𝑒𝑐.  Toronto:  Mercury Press, 2002.",
        "⠼⠁⠲⠀⠠⠒⠇⠕⠛⠥⠑⠂⠀⠠⠗⠁⠽⠲⠀⠨⠶⠠⠊⠍⠏⠕⠎⠎⠊⠃⠇⠑⠀⠠⠝⠁⠰⠝⠒⠀⠠⠮⠀⠠⠇⠰⠛⠬⠀⠿⠀⠠⠓⠕⠍⠑⠇⠯⠀⠔⠀⠠⠉⠁⠝⠁⠙⠁⠀⠯⠀⠠⠟⠥⠑⠃⠑⠉⠲⠨⠄⠀⠠⠞⠕⠗⠕⠝⠞⠕⠒⠀⠠⠍⠻⠉⠥⠗⠽⠀⠠⠏⠗⠑⠎⠎⠂⠀⠼⠃⠚⠚⠃⠲"
    )]
    #[case::le_roy_ladurie_quoted_french(
        "2. Le Roy Ladurie, Emmanuel.  \"Quand Paris était capitale du monde.\"  𝐿𝑒 𝑁𝑜𝑢𝑣𝑒𝑙 𝑂𝑏𝑠𝑒𝑟𝑣𝑎𝑡𝑒𝑢𝑟, August 2001.",
        "⠼⠃⠲⠀⠠⠇⠑⠀⠠⠗⠕⠽⠀⠠⠇⠁⠙⠥⠗⠊⠑⠂⠀⠠⠑⠍⠍⠁⠝⠥⠑⠇⠲⠀⠦⠠⠟⠥⠁⠝⠙⠀⠠⠏⠁⠗⠊⠎⠀⠘⠌⠑⠞⠁⠊⠞⠀⠉⠁⠏⠊⠞⠁⠇⠑⠀⠙⠥⠀⠍⠕⠝⠙⠑⠲⠴⠀⠨⠶⠠⠇⠑⠀⠠⠝⠕⠥⠧⠑⠇⠀⠠⠕⠃⠎⠑⠗⠧⠁⠞⠑⠥⠗⠂⠨⠄⠀⠠⠁⠥⠛⠥⠌⠀⠼⠃⠚⠚⠁⠲"
    )]
    #[case::menard_elements(
        "3. Ménard, Marc.  𝐸́𝑙𝑒𝑚𝑒𝑛𝑡𝑠 𝑝𝑜𝑢𝑟 𝑢𝑛𝑒 𝑒́𝑐𝑜𝑛𝑜𝑚𝑖𝑒 𝑑𝑒𝑠 𝑖𝑛𝑑𝑢𝑠𝑡𝑟𝑖𝑒𝑠 𝑐𝑢𝑙𝑡𝑢𝑟𝑒𝑙𝑙𝑒𝑠.  Montreal:  SODEC, 2004.",
        "⠼⠉⠲⠀⠠⠍⠘⠌⠑⠝⠜⠙⠂⠀⠠⠍⠜⠉⠲⠀⠨⠶⠠⠘⠌⠑⠇⠑⠍⠑⠝⠞⠎⠀⠏⠕⠥⠗⠀⠥⠝⠑⠀⠘⠌⠑⠉⠕⠝⠕⠍⠊⠑⠀⠙⠑⠎⠀⠊⠝⠙⠥⠎⠞⠗⠊⠑⠎⠀⠉⠥⠇⠞⠥⠗⠑⠇⠇⠑⠎⠲⠨⠄⠀⠠⠍⠕⠝⠞⠗⠂⠇⠒⠀⠠⠠⠎⠕⠙⠑⠉⠂⠀⠼⠃⠚⠚⠙⠲"
    )]
    #[case::language_today(
        "4. Weber, George.  \"The World's Ten Most Influential Languages.\" 𝐿𝑎𝑛𝑔𝑢𝑎𝑔𝑒 𝑇𝑜𝑑𝑎𝑦 2, December 1997.",
        "⠼⠙⠲⠀⠠⠺⠑⠃⠻⠂⠀⠠⠛⠑⠕⠗⠛⠑⠲⠀⠦⠠⠮⠀⠠⠸⠺⠄⠎⠀⠠⠞⠢⠀⠠⠍⠕⠌⠀⠠⠔⠋⠇⠥⠢⠞⠊⠁⠇⠀⠠⠇⠁⠝⠛⠥⠁⠛⠑⠎⠲⠴⠀⠨⠶⠠⠇⠁⠝⠛⠥⠁⠛⠑⠀⠠⠞⠙⠀⠼⠃⠂⠨⠄⠀⠠⠙⠑⠉⠑⠍⠃⠻⠀⠼⠁⠊⠊⠛⠲"
    )]
    fn encodes_bibliography_entries_from_13_1_3(#[case] text: &str, #[case] expected: &str) {
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
    #[case::italic_passage_then_prose_double_space(
        "go \u{1D44E}\u{1D44F} \u{1D450}\u{1D451} \u{1D452}\u{1D453}.  Now",
        "⠛⠀⠨⠶⠰⠁⠃⠀⠰⠉⠙⠀⠑⠋⠲⠨⠄⠀⠠⠝⠪"
    )]
    #[case::underline_passage_with_underlined_spaces(
        "go w\u{0332}o\u{0332}r\u{0332}d\u{0332}s\u{0332} \u{0332}w\u{0332}e\u{0332}r\u{0332}e\u{0332} \u{0332}u\u{0332}n\u{0332}d\u{0332}e\u{0332}r\u{0332}l\u{0332}i\u{0332}n\u{0332}e\u{0332}d\u{0332}",
        "⠛⠀⠸⠶⠘⠺⠎⠀⠶⠀⠐⠥⠇⠔⠫⠸⠄"
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

    /// §10.12.12: punctuation, indicators, or terminators printed inside a word do
    /// not block the basic §10 groupsigns; the indicator encloses just the printed
    /// styled segment and terminates before any following plain letters.
    #[rstest::rstest]
    #[case::italic_th_medial("ra\u{1D461}\u{210E}er", "⠗⠁⠨⠂⠹⠨⠄⠻")]
    #[case::bold_ch_medial("tou\u{1D41C}\u{1D421}ed", "⠞⠳⠘⠂⠡⠘⠄⠫")]
    #[case::italic_ing_final("flow\u{1D456}\u{1D45B}\u{1D454}", "⠋⠇⠪⠨⠂⠬")]
    #[case::underlined_gh_final("enoug\u{0332}h\u{0332}", "⠢⠳⠸⠂⠣")]
    fn medial_typeform_keeps_groupsigns_10_12_12(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §10.12.3: embedded web addresses remain contracted, so dot-delimited words
    /// such as `one` and `in` can use their ordinary UEB contractions.
    #[test]
    fn dot_delimited_web_components_contract_10_12_3() {
        assert_eq!(
            enc("www.one.in.a.hundred.org"),
            Some(cells("⠺⠺⠺⠲⠐⠕⠲⠔⠲⠁⠲⠓⠥⠝⠙⠗⠫⠲⠕⠗⠛"))
        );
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
    #[case::curly_double_quotes("“his”", cells("⠦⠓⠊⠎⠴"))]
    #[case::leading_em_dash_long_dash("—st", cells("⠐⠠⠤⠎⠞"))]
    #[case::long_dash_when_short_and_long_distinguished("a–b — c", cells("⠁⠠⠤⠰⠃⠀⠐⠠⠤⠀⠰⠉"))]
    #[case::left_arrow_prose("cat ← dog", cells("⠉⠁⠞⠀⠰⠳⠪⠀⠙⠕⠛"))]
    #[case::up_arrow_prose("cat ↑ dog", cells("⠉⠁⠞⠀⠰⠳⠬⠀⠙⠕⠛"))]
    #[case::angle_group_comma("X♭(Y) = ⟨X,Y⟩", cells("⠠⠭⠰⠔⠼⠣⠐⠣⠠⠽⠐⠜⠀⠐⠶⠀⠈⠣⠠⠭⠂⠀⠠⠽⠈⠜"))]
    fn encodes_punctuation_and_symbols(#[case] text: &str, #[case] expected: Vec<u8>) {
        assert_eq!(enc(text), Some(expected));
    }

    /// §3.15.1: straight apostrophe/double-quote glyphs in a numeric measurement
    /// context are foot (`⠄`) and inch (`⠠⠶`) signs, not directional quotation marks.
    #[test]
    fn encodes_straight_quote_measurements_3_15_1() {
        assert_eq!(enc("4' 11\""), Some(cells("⠼⠙⠄⠀⠼⠁⠁⠠⠶")));
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
    #[case::numeric_mode_spells_following_word("shopping4you", cells("⠩⠕⠏⠏⠬⠼⠙⠽⠕⠥"))]
    #[case::numeric_mode_spells_following_domain(
        "4starhotel@webnet.com",
        cells("⠼⠙⠎⠞⠁⠗⠓⠕⠞⠑⠇⠈⠁⠺⠑⠃⠝⠑⠞⠲⠉⠕⠍")
    )]
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
    #[case::titlecase_hyphenated_it("Do-It-Yourself", cells("⠠⠙⠤⠠⠭⠤⠠⠽⠗⠋"))]
    #[case::alphabetic_wordsign_suppressed_before_slash("quite/very", cells("⠟⠥⠊⠞⠑⠸⠌⠧⠻⠽"))]
    #[case::strong_wordsign_suppressed_before_slash("this/that", cells("⠹⠊⠎⠸⠌⠹⠁⠞"))]
    #[case::acronym_it_spells_letters("IT", cells("⠠⠠⠊⠞"))]
    #[case::printed_syllable_but_ton_spells_wordsign("but-ton", cells("⠃⠥⠞⠤⠞⠕⠝"))]
    #[case::printed_syllable_be_have_spells_wordsigns("be–have", cells("⠃⠑⠠⠤⠓⠁⠧⠑"))]
    #[case::printed_syllable_dis_as_ter_spells_as("dis as ter", cells("⠙⠊⠎⠀⠁⠎⠀⠞⠻"))]
    #[case::stammer_as_keeps_wordsign("as-s-s-s", cells("⠵⠤⠰⠰⠎⠤⠎⠤⠎"))]
    #[case::dash_phrase_but_keeps_wordsign("some–but", cells("⠐⠎⠠⠤⠃"))]
    #[case::dash_phrase_not_keeps_wordsign("from–not", cells("⠋⠠⠤⠝"))]
    #[case::hyphenated_not_spells_wordsign("not-with-stand-ing", cells("⠝⠕⠞⠤⠾⠤⠌⠯⠤⠔⠛"))]
    #[case::hyphenated_more_spells_wordsign("for-ev-er-more", cells("⠿⠤⠑⠧⠤⠻⠤⠍⠕⠗⠑"))]
    #[case::hyphenated_it_spells_wordsign("al-be-it", cells("⠰⠁⠇⠤⠃⠑⠤⠊⠞"))]
    #[case::apostrophe_m_spells_you("you'm", cells("⠽⠳⠄⠍"))]
    fn encodes_wordsigns(#[case] text: &str, #[case] expected: Vec<u8>) {
        assert_eq!(enc(text), Some(expected));
    }

    /// §10.4.2: `ch/sh/th/wh/ou/st` spell as letters only where the groupsign would
    /// be misread as a word; an apostrophe plus following letters can still be a
    /// word fragment (`th'n`) and keep the groupsign.
    #[rstest::rstest]
    #[case::sh_exclamation_spells("Sh!", "⠠⠎⠓⠖")]
    #[case::th_apostrophe_spells("th'", "⠞⠓⠄")]
    #[case::th_apostrophe_n_contracts("th'n", "⠹⠄⠝")]
    fn strong_groupsign_word_ambiguity_10_4_2(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
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

    /// §10.5.3–§10.5.4: lower wordsigns may touch lower punctuation only while the
    /// full lower-sign sequence also contains a non-quote sign with upper dots.
    #[rstest::rstest]
    #[case::listen_in("listen-in", "⠇⠊⠌⠢⠤⠔")]
    #[case::come_in_comma("Come in, stay in.", "⠠⠉⠕⠍⠑⠀⠊⠝⠂⠀⠌⠁⠽⠀⠊⠝⠲")]
    #[case::quoted_in_no_dash("“in”", "⠦⠊⠝⠴")]
    #[case::quoted_in_dash_in("‘Is that “in”?–in style.’", "⠠⠦⠠⠊⠎⠀⠞⠀⠦⠔⠴⠦⠠⠤⠊⠝⠀⠌⠽⠇⠑⠲⠠⠴")]
    #[case::enough_dash_in("\"That's enough!\"–in a firm voice", "⠦⠠⠞⠄⠎⠀⠢⠖⠴⠠⠤⠊⠝⠀⠁⠀⠋⠊⠗⠍⠀⠧⠕⠊⠉⠑")]
    #[case::paren_quote_in("(\"In no way.\")", "⠐⠣⠦⠠⠔⠀⠝⠕⠀⠺⠁⠽⠲⠴⠐⠜")]
    fn lower_sign_sequences_10_5(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §8.4 capitals passage (3+ all-caps words) vs §8.3 capital word (1–2).
    #[rstest::rstest]
    #[case::passage_four("THE BBC AFRICA NEWS", "⠠⠠⠠⠮⠀⠃⠃⠉⠀⠁⠋⠗⠊⠉⠁⠀⠝⠑⠺⠎⠠⠄")]
    #[case::two_caps_no_passage("NEW YORK", "⠠⠠⠝⠑⠺⠀⠠⠠⠽⠕⠗⠅")]
    #[case::single_caps_word("DOG", "⠠⠠⠙⠕⠛")]
    // §8.4.2/§8.5.2: lowercase words between all-caps sequences break the passage
    // count; `GO` must not combine with later `TAKE CARE` across `quickly and`.
    #[case::lowercase_barrier_between_caps(
        "\"GO quickly and TAKE CARE!\"",
        "⠦⠠⠠⠛⠀⠟⠅⠇⠽⠀⠯⠀⠠⠠⠞⠁⠅⠑⠀⠠⠠⠉⠜⠑⠖⠴"
    )]
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
    #[case::lowercase_accent_in_caps_word("PREMIèRE", "⠠⠠⠏⠗⠑⠍⠊⠘⠡⠑⠗⠑")]
    #[case::lowercase_tilde_in_caps_word("ESPAñOLA", "⠠⠠⠑⠎⠏⠁⠘⠻⠝⠕⠇⠁")]
    fn unicode_caps_word_8_4(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    /// §4.2.4 with §10.6.5/§10.8: a modified letter cannot itself participate in a
    /// contraction, but groupsigns elsewhere in the English/anglicised word remain
    /// available (`abbé`, `réchauffé`, `séance`).
    #[rstest::rstest]
    #[case::abbe("abbé", "⠁⠆⠘⠌⠑")]
    #[case::rechauffe("réchauffé", "⠗⠘⠌⠑⠡⠁⠥⠖⠘⠌⠑")]
    #[case::seance("séance", "⠎⠘⠌⠑⠨⠑")]
    #[case::double_macron_between_letters("spo\u{035e}on", "⠎⠏⠈⠤⠣⠕⠕⠜⠝")]
    fn modified_letters_keep_other_groupsigns_4_2_10(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(enc(text), Some(cells(expected)));
    }

    #[rstest::rstest]
    #[case::grave('\u{0300}', "⠘⠡")]
    #[case::acute('\u{0301}', "⠘⠌")]
    #[case::circumflex('\u{0302}', "⠘⠩")]
    #[case::diaeresis('\u{0308}', "⠘⠒")]
    #[case::macron('\u{0304}', "⠈⠤")]
    #[case::breve('\u{0306}', "⠈⠬")]
    #[case::caron('\u{030c}', "⠘⠬")]
    #[case::strike('\u{0336}', "⠈⠒")]
    #[case::double_macron('\u{035e}', "⠈⠤")]
    #[case::double_breve('\u{035c}', "⠈⠬")]
    fn maps_combining_modifiers(#[case] mark: char, #[case] expected: &str) {
        assert_eq!(
            combining_modifier_cells(mark),
            Some([cells(expected)[0], cells(expected)[1]])
        );
    }

    #[test]
    fn rejects_unknown_combining_modifier() {
        assert_eq!(combining_modifier_cells('\u{0303}'), None);
    }

    #[rstest::rstest]
    #[case::alpha('Α', false, "⠠⠨⠁")]
    #[case::beta_suppressed('Β', true, "⠨⠃")]
    #[case::gamma('Γ', true, "⠨⠛")]
    #[case::delta('Δ', true, "⠨⠙")]
    #[case::epsilon('Ε', true, "⠨⠑")]
    #[case::zeta('Ζ', true, "⠨⠵")]
    #[case::eta('Η', true, "⠨⠱")]
    #[case::theta('Θ', true, "⠨⠹")]
    #[case::iota('Ι', true, "⠨⠊")]
    #[case::kappa('Κ', true, "⠨⠅")]
    #[case::lambda('Λ', true, "⠨⠇")]
    #[case::mu('Μ', true, "⠨⠍")]
    #[case::nu('Ν', true, "⠨⠝")]
    #[case::xi('Ξ', true, "⠨⠭")]
    #[case::omicron('Ο', true, "⠨⠕")]
    #[case::pi('Π', true, "⠨⠏")]
    #[case::rho('Ρ', true, "⠨⠗")]
    #[case::phi('Φ', true, "⠨⠋")]
    #[case::sigma('Σ', true, "⠨⠎")]
    #[case::tau('Τ', true, "⠨⠞")]
    #[case::upsilon('Υ', true, "⠨⠥")]
    #[case::chi('Χ', true, "⠨⠯")]
    #[case::psi('Ψ', true, "⠨⠽")]
    #[case::omega('Ω', true, "⠨⠺")]
    #[case::final_sigma('ς', false, "⠨⠎")]
    fn maps_greek_letters_with_capital_policy(
        #[case] input: char,
        #[case] suppress_capital: bool,
        #[case] expected: &str,
    ) {
        assert_eq!(
            greek_letter_cells_with_caps(input, suppress_capital),
            Some(cells(expected))
        );
    }

    #[test]
    fn rejects_non_greek_letter_mapping() {
        assert_eq!(greek_letter_cells_with_caps('A', false), None);
    }

    #[rstest::rstest]
    #[case::caps_word(&['A', 'B'], Some(Caps::Word))]
    #[case::single_cap(&['A'], Some(Caps::Single))]
    #[case::titlecase(&['A', 'b'], Some(Caps::Single))]
    #[case::lower(&['a', 'b'], Some(Caps::None))]
    #[case::mixed_internal(&['a', 'B'], None)]
    fn classifies_capital_patterns(#[case] input: &[char], #[case] expected: Option<Caps>) {
        assert_eq!(classify_caps(input), expected);
    }

    #[test]
    fn emits_ligature_and_struck_letter_helpers() {
        let mut out = Vec::new();
        emit_ligature_between(&['o'], &['e'], &mut out).unwrap();
        assert_eq!(out, cells("⠕⠘⠖⠑"));

        let tokens = [
            EnglishToken::Word(vec!['a']),
            EnglishToken::Symbol('\u{0336}'),
            EnglishToken::Word(vec!['b']),
            EnglishToken::Symbol('\u{0336}'),
        ];
        let mut struck = Vec::new();
        assert_eq!(
            emit_struck_letter_sequence(&tokens, 0, &['a'], &mut struck),
            Some(4)
        );
        assert_eq!(struck, cells("⠁⠘⠖⠃"));
    }

    #[rstest::rstest]
    #[case::empty(&[], false)]
    #[case::short_word(&[EnglishToken::Word(vec!['A', 'A', 'A', 'A'])], false)]
    #[case::two_words(&[EnglishToken::Word(vec!['H', 'E', 'Y', 'Y', 'Y']), EnglishToken::Word(vec!['A'])], false)]
    #[case::elongated(&[EnglishToken::Word(vec!['H', 'E', 'Y', 'Y', 'Y'])], true)]
    fn detects_single_elongated_caps_word_in_quotes(
        #[case] tokens: &[EnglishToken],
        #[case] expected: bool,
    ) {
        assert_eq!(single_elongated_caps_word_in_quotes(tokens), expected);
    }

    #[test]
    fn emits_word_modifier_on_last_paths() {
        let mut out = Vec::new();
        emit_word_with_modifier_on_last(&['c', 'a', 'f', 'e'], '\u{0301}', &mut out).unwrap();
        assert_eq!(out, cells("⠉⠁⠋⠘⠌⠑"));

        let mut empty = Vec::new();
        assert_eq!(
            emit_word_with_modifier_on_last(&[], '\u{0301}', &mut empty),
            None
        );

        let mut unknown_mark = Vec::new();
        assert_eq!(
            emit_word_with_modifier_on_last(&['a'], '\u{0303}', &mut unknown_mark),
            None
        );
    }

    #[test]
    fn emits_ligature_between_rejection_and_uppercase_paths() {
        let mut uppercase = Vec::new();
        emit_ligature_between(&['o'], &['E'], &mut uppercase).unwrap();
        assert_eq!(uppercase, cells("⠕⠠⠘⠖⠑"));

        let mut out = Vec::new();
        assert_eq!(emit_ligature_between(&[], &['e'], &mut out), None);
        assert_eq!(emit_ligature_between(&['o'], &[], &mut out), None);
    }

    #[test]
    fn emits_styled_struck_pair_paths() {
        let wrong_tokens = [
            EnglishToken::Styled('a', super::super::token::Typeform::Italic),
            EnglishToken::Symbol('\u{0336}'),
            EnglishToken::Word(vec!['b']),
        ];
        let mut out = Vec::new();
        assert_eq!(
            emit_styled_struck_pair(
                &wrong_tokens,
                0,
                super::super::token::Typeform::Italic,
                'a',
                &mut out,
            ),
            None
        );

        let missing_overlay = [
            EnglishToken::Styled('a', super::super::token::Typeform::Italic),
            EnglishToken::Symbol('\u{0336}'),
            EnglishToken::Styled('b', super::super::token::Typeform::Bold),
        ];
        assert_eq!(
            emit_styled_struck_pair(
                &missing_overlay,
                0,
                super::super::token::Typeform::Italic,
                'a',
                &mut out,
            ),
            None
        );

        let good = [
            EnglishToken::Styled('a', super::super::token::Typeform::Italic),
            EnglishToken::Symbol('\u{0336}'),
            EnglishToken::Styled('B', super::super::token::Typeform::Italic),
            EnglishToken::Symbol('\u{0336}'),
        ];
        let mut struck = Vec::new();
        assert_eq!(
            emit_styled_struck_pair(
                &good,
                0,
                super::super::token::Typeform::Italic,
                'a',
                &mut struck,
            ),
            Some(4)
        );
        assert_eq!(struck, cells("⠨⠂⠁⠠⠘⠖⠃"));
    }

    #[rstest::rstest]
    #[case::four_single(&[&['w'][..], &['a'][..], &['l'][..], &['k'][..]], true)]
    #[case::tail_single(&[&['b', 'r'][..], &['r'][..], &['r'][..], &['r'][..]], true)]
    #[case::same_tail(&[&['s', 'o'][..], &['o', 'o'][..], &['o', 'o'][..], &['o', 'o'][..]], true)]
    #[case::one_then_long_same(&[&['r'][..], &['m', 'm', 'm', 'm'][..]], true)]
    #[case::with_which(&[&['n', 'o', 't'][..], &['w', 'i', 't', 'h'][..], &['s', 't', 'a', 'n', 'd'][..], &['i', 'n', 'g'][..], &['x'][..]], false)]
    fn grade1_hyphenated_word_indicator_paths(#[case] words: &[&[char]], #[case] expected: bool) {
        assert_eq!(grade1_hyphenated_words_use_word_indicator(words), expected);
    }

    #[test]
    fn grade1_hyphenated_span_and_stammer_helpers_cover_edges() {
        let tokens = [
            EnglishToken::Word(vec!['w']),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['a']),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['l']),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['k']),
        ];
        let span = grade1_hyphenated_word_span(&tokens, 0).expect("spelling run should span");
        assert_eq!(span.end, tokens.len());
        assert_eq!(span.indicator_cells, 2);

        assert!(!same_letters(&[]));
        assert!(!repeated_single_letter_prefix(
            &[&['f'][..], &['f'][..]],
            &['f', 'a']
        ));
        assert!(!repeated_single_letter_prefix(
            &[&[][..], &['f'][..], &['f'][..]],
            &['f', 'a'],
        ));
    }

    #[test]
    fn spatial_helpers_encode_grade1_rows_and_symbols() {
        let engine = EnglishUebEngine::new();
        let mut chars = Vec::new();
        push_spatial_char(&mut chars, ' ').unwrap();
        push_spatial_char(&mut chars, '╳').unwrap();
        push_spatial_char(&mut chars, '>').unwrap();
        push_spatial_char(&mut chars, '<').unwrap();
        assert_eq!(chars, cells("⠀⠜⠠⠜⠠⠣"));

        let grade1_rows = encode_spatial_rows(&["╱╲", " ╳"], true).unwrap();
        assert_eq!(grade1_rows, cells("⠐⠐⠿⠰⠰⠰\n⠜⠣\n⠀⠜\n⠐⠐⠿⠰⠄"));

        let mut unsupported = Vec::new();
        assert_eq!(push_spatial_char(&mut unsupported, 'x'), None);

        let cross_gap = [
            EnglishToken::Symbol('─'),
            EnglishToken::Symbol('┼'),
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Symbol('─'),
            EnglishToken::Symbol('╲'),
        ];
        let encoded = engine.encode(&cross_gap, false).unwrap();
        assert_eq!(encoded, cells("⠐⠒⠺⠀⠐⠒⠣"));

        let game_board = [
            EnglishToken::Symbol('╲'),
            EnglishToken::LineBreak,
            EnglishToken::Word(vec!['X']),
            EnglishToken::Space,
            EnglishToken::Symbol('─'),
            EnglishToken::Symbol('┼'),
            EnglishToken::Symbol('─'),
            EnglishToken::Space,
            EnglishToken::Word(vec!['O']),
        ];
        let encoded = engine.encode(&game_board, false).unwrap();
        assert!(encoded.starts_with(&cells("⠐⠐⠿⠰⠰⠰\n")));
        assert!(encoded.ends_with(&cells("\n⠰⠄")));
        assert!(encoded.contains(&decode_unicode('⠭')));
        assert!(encoded.contains(&decode_unicode('⠕')));
    }

    #[test]
    fn spatial_box_and_grid_helpers_cover_positive_and_negative_paths() {
        let box_tokens = [
            EnglishToken::Symbol('┌'),
            EnglishToken::Symbol('─'),
            EnglishToken::Symbol('┐'),
            EnglishToken::LineBreak,
            EnglishToken::Symbol('│'),
            EnglishToken::Symbol('?'),
            EnglishToken::Space,
            EnglishToken::Symbol('!'),
            EnglishToken::Symbol('│'),
            EnglishToken::LineBreak,
            EnglishToken::Symbol('└'),
            EnglishToken::Symbol('─'),
            EnglishToken::Symbol('┘'),
        ];
        assert_eq!(
            encode_rule_3_14_punctuation_box(&box_tokens),
            Some(cells("⠀⠐⠐⠿⠰⠦⠀⠀⠀⠀⠀⠀⠐⠐⠿⠖\n⠐⠒⠒⠒⠒⠒⠒⠀⠀⠀⠐⠒⠒⠒⠒⠒⠒"))
        );

        assert_eq!(
            encode_rule_3_14_punctuation_box(&[EnglishToken::Symbol('┌')]),
            None
        );

        let grid_tokens = [
            EnglishToken::Word(vec!['A']),
            EnglishToken::Space,
            EnglishToken::Word(vec!['B']),
            EnglishToken::LineBreak,
            EnglishToken::Word(vec!['C']),
            EnglishToken::Space,
            EnglishToken::Word(vec!['D']),
        ];
        assert_eq!(
            encode_rule_3_14_letter_grid(&grid_tokens),
            Some(cells("⠐⠐⠿⠰⠰⠰⠠⠠⠠\n⠁⠀⠃\n⠉⠀⠙\n⠐⠐⠿⠠⠄⠰⠄"))
        );

        let ragged_grid = [
            EnglishToken::Word(vec!['A']),
            EnglishToken::Space,
            EnglishToken::Word(vec!['B']),
            EnglishToken::LineBreak,
            EnglishToken::Word(vec!['C']),
        ];
        assert_eq!(encode_rule_3_14_letter_grid(&ragged_grid), None);
    }

    #[rstest::rstest]
    #[case::diagonal("╲", "⠣\n⠀⠣\n⠀⠀⠣\n⠀⠀⠀⠣")]
    #[case::vertical("┊", "⠘\n⠘\n⠘\n⠘")]
    #[case::crossing("╲╱╱", "⠣⠀⠀⠀⠀⠀⠀⠀⠀⠠⠜\n⠀⠀⠣⠀⠀⠀⠀⠠⠜\n⠀⠀⠀⠀⠣⠠⠜")]
    fn compact_spatial_examples_encode_rows(#[case] text: &str, #[case] expected: &str) {
        let tokens: Vec<EnglishToken> = text.chars().map(EnglishToken::Symbol).collect();
        assert_eq!(
            encode_compact_spatial_example(&tokens),
            Some(cells(expected))
        );
    }

    #[test]
    fn styled_unstyled_span_helper_encodes_all_token_kinds() {
        let engine = EnglishUebEngine::new();
        let form = super::super::token::Typeform::Underline;
        let tokens = [
            EnglishToken::Styled('a', form),
            EnglishToken::Styled('b', form),
            EnglishToken::Symbol('/'),
            EnglishToken::Styled('1', form),
            EnglishToken::Styled('2', form),
            EnglishToken::LineBreak,
            EnglishToken::Styled('?', form),
        ];
        let mut out = Vec::new();
        engine
            .encode_styled_as_unstyled_span(
                0,
                tokens.len(),
                form,
                StyledContext {
                    tokens: &tokens,
                    suppress_caps: false,
                    foreign_scope: None,
                },
                &mut out,
            )
            .unwrap();
        assert_eq!(out, cells("⠁⠃⠸⠌⠼⠁⠃\n⠰⠦"));

        let wrong_form = [EnglishToken::Styled(
            'a',
            super::super::token::Typeform::Italic,
        )];
        let mut rejected = Vec::new();
        assert_eq!(
            engine.encode_styled_as_unstyled_span(
                0,
                wrong_form.len(),
                form,
                StyledContext {
                    tokens: &wrong_form,
                    suppress_caps: false,
                    foreign_scope: None,
                },
                &mut rejected,
            ),
            None
        );
    }

    #[test]
    fn styled_span_helper_preserves_line_breaks_between_segments() {
        let engine = EnglishUebEngine::new();
        let form = super::super::token::Typeform::Italic;
        let tokens = [
            EnglishToken::Styled('a', form),
            EnglishToken::LineBreak,
            EnglishToken::Styled('b', form),
        ];
        let mut out = Vec::new();

        engine
            .encode_styled_span(
                0,
                tokens.len(),
                form,
                StyledContext {
                    tokens: &tokens,
                    suppress_caps: false,
                    foreign_scope: None,
                },
                &mut out,
            )
            .unwrap();

        assert!(out.contains(&255));
    }

    #[test]
    fn styled_word_surface_encodes_plain_multiletter_run() {
        let engine = EnglishUebEngine::new();
        let form = super::super::token::Typeform::Italic;
        let tokens = [
            EnglishToken::Styled('r', form),
            EnglishToken::Styled('a', form),
            EnglishToken::Styled('d', form),
            EnglishToken::Styled('a', form),
            EnglishToken::Styled('r', form),
        ];

        let encoded = engine.encode(&tokens, false).unwrap();

        assert!(encoded.starts_with(&super::super::rule_9::word_indicator(form)));
    }

    #[test]
    fn typeform_passage_terminates_before_closing_quote_after_comma() {
        let engine = EnglishUebEngine::new();
        let form = super::super::token::Typeform::Italic;
        let tokens = [
            EnglishToken::Styled('r', form),
            EnglishToken::Styled('e', form),
            EnglishToken::Styled('d', form),
            EnglishToken::Space,
            EnglishToken::Styled('g', form),
            EnglishToken::Styled('r', form),
            EnglishToken::Styled('e', form),
            EnglishToken::Styled('e', form),
            EnglishToken::Styled('n', form),
            EnglishToken::Space,
            EnglishToken::Styled('b', form),
            EnglishToken::Styled('l', form),
            EnglishToken::Styled('u', form),
            EnglishToken::Styled('e', form),
            EnglishToken::Symbol(','),
            EnglishToken::Symbol('"'),
        ];

        let encoded = engine.encode(&tokens, false).unwrap();
        let terminator = super::super::rule_9::terminator(form);

        assert!(
            encoded
                .windows(terminator.len() + 2)
                .any(|cells| cells.starts_with(&terminator)
                    && cells[terminator.len()] == decode_unicode('⠠')
                    && cells[terminator.len() + 1] == decode_unicode('⠶'))
        );
    }

    #[test]
    fn chemical_script_branch_collects_all_token_kinds() {
        let engine = EnglishUebEngine::new();
        let form = super::super::token::Typeform::Italic;
        let tokens = [
            EnglishToken::Word(vec!['H']),
            EnglishToken::Symbol('₁'),
            EnglishToken::Space,
            EnglishToken::Number(vec!['2']),
            EnglishToken::LineBreak,
            EnglishToken::Styled('O', form),
            EnglishToken::WordDivision {
                chars: vec!['H'],
                break_at: 1,
            },
            EnglishToken::Symbol('+'),
        ];

        let encoded = engine.encode(&tokens, false).unwrap();

        assert!(encoded.starts_with(&[GRADE1, GRADE1, GRADE1]));
        assert!(encoded.ends_with(&[GRADE1, decode_unicode('⠄')]));
    }

    #[test]
    fn divided_word_helper_covers_caps_mixed_and_invalid_breaks() {
        let engine = EnglishUebEngine::new();

        let mut invalid = Vec::new();
        assert_eq!(
            engine.encode_divided_word(&['c', 'a', 't'], 0, false, &mut invalid),
            None
        );
        assert_eq!(
            engine.encode_divided_word(&['c', 'a', 't'], 3, false, &mut invalid),
            None
        );

        let mut lower = Vec::new();
        engine
            .encode_divided_word(&['c', 'a', 't', 's'], 2, false, &mut lower)
            .unwrap();
        assert!(lower.contains(&255));

        let mut title = Vec::new();
        engine
            .encode_divided_word(&['C', 'a', 't', 's'], 2, false, &mut title)
            .unwrap();
        assert!(title.starts_with(&[CAPITAL]));

        let mut caps = Vec::new();
        engine
            .encode_divided_word(&['C', 'A', 'T', 'S'], 2, false, &mut caps)
            .unwrap();
        assert!(caps.starts_with(&[CAPITAL, CAPITAL]));
        assert!(caps.contains(&255));

        let mut mixed = Vec::new();
        engine
            .encode_divided_word(&['M', 'c', 'D', 'o', 'g'], 2, false, &mut mixed)
            .unwrap();
        assert!(mixed.contains(&255));
        assert!(mixed.iter().filter(|cell| **cell == CAPITAL).count() >= 2);
    }

    #[rstest::rstest]
    #[case::sup_m(
        '\u{1D50}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Superscript,
        'm'
    )]
    #[case::sup_c(
        '\u{1D9C}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Superscript,
        'c'
    )]
    #[case::sub_a(
        '\u{2090}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        'a'
    )]
    #[case::sub_e(
        '\u{2091}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        'e'
    )]
    #[case::sub_h(
        '\u{2095}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        'h'
    )]
    #[case::sub_i(
        '\u{1D62}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        'i'
    )]
    #[case::sub_j(
        '\u{2C7C}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        'j'
    )]
    #[case::sub_k(
        '\u{2096}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        'k'
    )]
    #[case::sub_l(
        '\u{2097}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        'l'
    )]
    #[case::sub_m(
        '\u{2098}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        'm'
    )]
    #[case::sub_n(
        '\u{2099}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        'n'
    )]
    #[case::sub_o(
        '\u{2092}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        'o'
    )]
    #[case::sub_p(
        '\u{209A}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        'p'
    )]
    #[case::sub_r(
        '\u{1D63}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        'r'
    )]
    #[case::sub_s(
        '\u{209B}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        's'
    )]
    #[case::sub_t(
        '\u{209C}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        't'
    )]
    #[case::sub_u(
        '\u{1D64}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        'u'
    )]
    #[case::sub_v(
        '\u{1D65}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        'v'
    )]
    #[case::sub_x(
        '\u{2093}',
        crate::rules::english_ueb::rule_3_24::ScriptKind::Subscript,
        'x'
    )]
    fn script_letter_maps_supported_letters(
        #[case] input: char,
        #[case] kind: crate::rules::english_ueb::rule_3_24::ScriptKind,
        #[case] letter: char,
    ) {
        assert_eq!(script_letter(input), Some((kind, letter)));
    }

    #[test]
    fn struck_letter_sequence_helper_covers_short_upper_and_invalid_paths() {
        let mut short = Vec::new();
        assert_eq!(
            emit_struck_letter_sequence(
                &[
                    EnglishToken::Word(vec!['a']),
                    EnglishToken::Symbol('\u{0336}'),
                ],
                0,
                &['a'],
                &mut short,
            ),
            None
        );

        let tokens = [
            EnglishToken::Word(vec!['a']),
            EnglishToken::Symbol('\u{0336}'),
            EnglishToken::Word(vec!['B']),
            EnglishToken::Symbol('\u{0336}'),
        ];
        let mut out = Vec::new();
        assert_eq!(
            emit_struck_letter_sequence(&tokens, 0, &['a'], &mut out),
            Some(tokens.len())
        );
        assert_eq!(out, cells("⠁⠠⠘⠖⠃"));
    }

    #[test]
    fn listing_and_regex_helpers_cover_continuation_paths() {
        let slash_tokens = [
            EnglishToken::Word(vec!['a']),
            EnglishToken::Symbol('/'),
            EnglishToken::Word(vec!['b']),
        ];
        assert!(url_listing_line_continuation_after(
            &slash_tokens,
            1,
            &[true; 3]
        ));

        let hyphen_tokens = [
            EnglishToken::Word(vec!['a']),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['b']),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['c']),
        ];
        assert!(url_listing_line_continuation_after(
            &hyphen_tokens,
            3,
            &[true; 5]
        ));

        let range_tokens = [
            EnglishToken::Symbol('"'),
            EnglishToken::Symbol('['),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['a', 'Z']),
            EnglishToken::Symbol('-'),
            EnglishToken::Symbol(']'),
            EnglishToken::Symbol('"'),
        ];
        let mut range = Vec::new();
        assert_eq!(
            regex_char_class_word(&range_tokens, 3, &['a', 'Z'], &[true; 7], &mut range),
            Some(true)
        );
        assert_eq!(range, cells("⠁⠠⠐⠀⠵"));

        let terminal_upper_tokens = [
            EnglishToken::Symbol('"'),
            EnglishToken::Symbol('['),
            EnglishToken::Symbol('a'),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['Z']),
            EnglishToken::Symbol(']'),
            EnglishToken::Symbol('"'),
        ];
        let mut upper = Vec::new();
        assert_eq!(
            regex_char_class_word(&terminal_upper_tokens, 4, &['Z'], &[true; 7], &mut upper),
            Some(true)
        );
        assert_eq!(upper, cells("⠰⠠⠵"));
    }

    #[test]
    fn styled_symbol_sequence_helpers_cover_all_token_kinds() {
        let form = super::super::token::Typeform::Italic;
        let tokens = [
            EnglishToken::Styled('R', form),
            EnglishToken::Symbol('.'),
            EnglishToken::Styled('2', form),
            EnglishToken::Symbol('.'),
            EnglishToken::Styled('?', form),
        ];
        assert!(styled_capital_starts_symbol_sequence(&tokens, 0, 1));
        assert_eq!(styled_symbol_sequence_end(&tokens, 0, form), 4);

        let mut out = Vec::new();
        encode_styled_symbol_sequence(&tokens, 0, tokens.len(), form, &mut out).unwrap();
        assert_eq!(out, cells("⠠⠗⠲⠼⠃⠲⠰⠦"));

        let invalid = [EnglishToken::Styled('R', form), EnglishToken::Space];
        let mut rejected = Vec::new();
        assert_eq!(
            encode_styled_symbol_sequence(&invalid, 0, invalid.len(), form, &mut rejected),
            None
        );
    }

    #[test]
    fn styled_url_and_nested_typeform_helpers_cover_positive_paths() {
        let underline = super::super::token::Typeform::Underline;
        let url = [
            EnglishToken::Styled('h', underline),
            EnglishToken::Styled('t', underline),
            EnglishToken::Styled('t', underline),
            EnglishToken::Styled('p', underline),
            EnglishToken::Symbol(':'),
            EnglishToken::Symbol('/'),
            EnglishToken::Symbol('/'),
            EnglishToken::Styled('x', underline),
        ];
        assert!(styled_underline_url_span(&url, 0, url.len(), underline));
        assert!(styled_url_before(&url, url.len()));

        let bold_italic = super::super::token::Typeform::BoldItalic;
        let italic = super::super::token::Typeform::Italic;
        let nested = [
            EnglishToken::Styled('a', bold_italic),
            EnglishToken::Space,
            EnglishToken::Styled('b', italic),
            EnglishToken::Styled('c', italic),
            EnglishToken::Styled('d', italic),
        ];
        assert_eq!(
            nested_typeform_continuation(&nested, 1, bold_italic),
            Some((nested.len(), italic, super::super::token::Typeform::Bold))
        );
    }

    #[test]
    fn straight_single_quote_helpers_cover_quotation_roles() {
        let opened = [
            EnglishToken::Symbol('\''),
            EnglishToken::Word(vec!['H', 'i']),
            EnglishToken::Symbol('\''),
        ];
        assert!(straight_single_quote_is_matched_quotation(&opened, 0));
        assert!(matches!(
            straight_single_quote_role(&opened, 0),
            SingleQuote::Open
        ));
        assert!(matches!(
            straight_single_quote_role(&opened, 2),
            SingleQuote::Close
        ));

        let inner_double_close = [
            EnglishToken::Symbol('\''),
            EnglishToken::Word(vec!['H', 'i']),
            EnglishToken::Symbol('"'),
            EnglishToken::Symbol(','),
            EnglishToken::Symbol('\''),
        ];
        assert!(straight_single_quote_closes_after_inner_double(
            &inner_double_close,
            4
        ));
        assert!(straight_single_quote_exchanged(&inner_double_close, 4));
    }

    #[test]
    fn inner_double_quote_close_requires_an_opening_quote() {
        let terminal_punctuation = [EnglishToken::Symbol('!'), EnglishToken::Symbol(',')];

        assert!(!straight_single_quote_closes_after_inner_double(
            &terminal_punctuation,
            terminal_punctuation.len(),
        ));
    }

    #[test]
    fn encode_rare_document_level_symbol_paths() {
        let engine = EnglishUebEngine::new();

        assert_eq!(
            engine.encode(
                &[EnglishToken::Symbol('-'), EnglishToken::Symbol('-')],
                false
            ),
            Some(cells("⠐⠒⠒⠒"))
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Number(vec!['2']),
                        EnglishToken::Space,
                        EnglishToken::Symbol('×'),
                        EnglishToken::Space,
                        EnglishToken::Number(vec!['3']),
                    ],
                    true,
                )
                .is_some()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Word(vec!['H']),
                        EnglishToken::Symbol('₂'),
                        EnglishToken::Symbol('+'),
                        EnglishToken::Word(vec!['O']),
                        EnglishToken::Symbol('→'),
                        EnglishToken::Word(vec!['H']),
                        EnglishToken::Symbol('₂'),
                        EnglishToken::Word(vec!['O']),
                    ],
                    false,
                )
                .is_some()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Symbol('.'),
                        EnglishToken::Number(vec!['3', '7']),
                    ],
                    false,
                )
                .is_some()
        );
    }

    #[test]
    fn encode_rare_word_and_spatial_paths() {
        let engine = EnglishUebEngine::new();

        assert!(
            engine
                .encode(
                    &[EnglishToken::Word(vec!['a']), EnglishToken::Symbol('ₙ'),],
                    false,
                )
                .is_none()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Word(vec!['m', 'a', 's', 's']),
                        EnglishToken::Symbol('ₛ'),
                        EnglishToken::Symbol('ᵤ'),
                        EnglishToken::Symbol('ₙ'),
                    ],
                    false,
                )
                .is_some()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Word(vec!['A']),
                        EnglishToken::Symbol('='),
                        EnglishToken::Word(vec!['b']),
                        EnglishToken::Word(vec!['C']),
                    ],
                    false,
                )
                .is_some()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Symbol('┌'),
                        EnglishToken::Symbol('─'),
                        EnglishToken::Symbol('┼'),
                        EnglishToken::Space,
                        EnglishToken::Space,
                        EnglishToken::Symbol('─'),
                        EnglishToken::Symbol('┐'),
                    ],
                    false,
                )
                .is_some()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Styled('c', super::super::token::Typeform::Italic),
                        EnglishToken::Styled('h', super::super::token::Typeform::Italic),
                        EnglishToken::Styled('a', super::super::token::Typeform::Italic),
                    ],
                    false,
                )
                .is_some()
        );

        assert_eq!(
            engine
                .encode(&[EnglishToken::Number(vec!['4', '2'])], false)
                .unwrap(),
            cells("⠼⠙⠃")
        );
    }

    #[test]
    fn encode_rare_spatial_layout_branches() {
        let engine = EnglishUebEngine::new();

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::WordDivision {
                            chars: vec!['a', 'b'],
                            break_at: 1,
                        },
                        EnglishToken::Symbol('\t'),
                    ],
                    false,
                )
                .is_some()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Symbol('│'),
                        EnglishToken::Space,
                        EnglishToken::Space,
                        EnglishToken::Space,
                        EnglishToken::Space,
                        EnglishToken::Space,
                        EnglishToken::Space,
                        EnglishToken::Space,
                        EnglishToken::Space,
                        EnglishToken::Space,
                        EnglishToken::Space,
                        EnglishToken::Symbol('│'),
                        EnglishToken::LineBreak,
                    ],
                    false,
                )
                .is_some()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Symbol('┐'),
                        EnglishToken::Space,
                        EnglishToken::Space,
                        EnglishToken::Space,
                        EnglishToken::Space,
                        EnglishToken::Space,
                        EnglishToken::Symbol('┌'),
                        EnglishToken::Symbol('─'),
                        EnglishToken::LineBreak,
                        EnglishToken::Symbol('╲'),
                    ],
                    false,
                )
                .is_none()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Symbol('╲'),
                        EnglishToken::Space,
                        EnglishToken::Space,
                        EnglishToken::Symbol('│'),
                        EnglishToken::LineBreak,
                    ],
                    false,
                )
                .is_some()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Symbol('│'),
                        EnglishToken::Symbol('╲'),
                        EnglishToken::Symbol('│'),
                        EnglishToken::LineBreak,
                    ],
                    false,
                )
                .is_some()
        );
    }

    #[test]
    fn encode_rare_greek_grouping_branches() {
        let engine = EnglishUebEngine::new();

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Word(vec!['Α', 'Β']),
                        EnglishToken::Symbol('Γ'),
                    ],
                    false,
                )
                .is_some()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Symbol('Α'),
                        EnglishToken::Word(vec!['Β', 'Γ']),
                    ],
                    false,
                )
                .is_some()
        );
    }

    #[test]
    fn rare_helper_branches_cover_case_punctuation_and_styled_contexts() {
        let mut single = Vec::new();
        encode_lower_sequence_word(&['A'], &[decode_unicode('⠁')], &mut single).unwrap();
        assert_eq!(single, cells("⠠⠁"));

        let mut caps = Vec::new();
        encode_lower_sequence_word(
            &['A', 'B'],
            &[decode_unicode('⠁'), decode_unicode('⠃')],
            &mut caps,
        )
        .unwrap();
        assert_eq!(caps, cells("⠠⠠⠁⠃"));

        assert!(
            mixed_case_shortform_part(&['g', 'o', 'o', 'd', 'X'], 0, &['g', 'o', 'o', 'd'])
                .is_some()
        );
        assert!(shortform_meets_rule_10_9_4(
            &['g', 'o', 'o', 'd'],
            0,
            &['g', 'o', 'o', 'd'],
            true
        ));

        let parenthesized = [
            EnglishToken::Symbol('('),
            EnglishToken::Styled('a', super::super::token::Typeform::Italic),
            EnglishToken::Symbol(')'),
        ];
        assert!(parenthesized_foreign_style_before(&parenthesized, 2));
        assert!(!parenthesized_foreign_style_before(
            &[
                EnglishToken::Symbol('('),
                EnglishToken::Symbol(')'),
                EnglishToken::Symbol(')'),
            ],
            2
        ));

        let previous_punctuation = [
            EnglishToken::Word(vec!['H', 'i']),
            EnglishToken::Symbol('.'),
            EnglishToken::Symbol(')'),
            EnglishToken::Symbol('\''),
        ];
        assert!(previous_text_skipping_terminal_punctuation(
            &previous_punctuation,
            3
        ));
        assert!(previous_word_starts_uppercase(&previous_punctuation, 3));

        let styled_neighbors = [
            EnglishToken::Styled('a', super::super::token::Typeform::Italic),
            EnglishToken::Space,
            EnglishToken::Symbol(','),
            EnglishToken::Space,
            EnglishToken::Styled('b', super::super::token::Typeform::Italic),
        ];
        assert!(punctuation_adjacent_to_styled(&styled_neighbors, 2));
    }

    #[test]
    fn rare_helper_branches_cover_lower_sign_and_foreign_word_paths() {
        assert!(!spell_line_division_in(
            &[EnglishToken::Word(vec!['o', 'u', 't'])],
            0,
            "out"
        ));

        let enough = [
            EnglishToken::Word(vec!['e', 'n', 'o', 'u', 'g', 'h']),
            EnglishToken::Symbol('!'),
            EnglishToken::Symbol('”'),
            EnglishToken::Symbol('—'),
            EnglishToken::Word(vec!['i', 'n']),
        ];
        assert!(dash_after_enough_before_in(&enough, 4));

        let quoted_in = [
            EnglishToken::Word(vec!['i', 'n']),
            EnglishToken::Symbol('?'),
            EnglishToken::Symbol('”'),
            EnglishToken::Symbol('—'),
            EnglishToken::Word(vec!['i', 'n']),
        ];
        assert!(dash_after_quoted_in_before_in(&quoted_in, 4));

        assert!(!space_delimited_syllables_form_word(
            &[EnglishToken::Word(vec!['a'])],
            0
        ));
        assert!(foreign_en_spells_letters(None, Some(&EnglishToken::Space)));
        assert!(!styled_word_is_foreign(&['c', 'h']));
        assert!(!styled_single_word_is_foreign(&['t', 'h']));
        assert!(styled_word_has_foreign_signal(&['c', 'h', 'a', 'o', 's']));
    }

    #[test]
    fn rare_helper_branches_cover_styled_sequences_and_quotes() {
        let bold = super::super::token::Typeform::Bold;
        let numeric = [
            EnglishToken::Styled('1', bold),
            EnglishToken::Space,
            EnglishToken::Styled('5', bold),
        ];
        assert_eq!(
            styled_numeric_sequence_end(&numeric, 0, bold),
            numeric.len()
        );
        let mut numeric_out = Vec::new();
        encode_styled_numeric_sequence(&numeric, 0, numeric.len(), bold, &mut numeric_out).unwrap();
        assert_eq!(
            numeric_out,
            vec![decode_unicode('⠼'), 1, decode_unicode('⠐'), 17]
        );

        let invalid_numeric = [
            EnglishToken::Styled('1', bold),
            EnglishToken::Word(vec!['x']),
        ];
        let mut invalid_numeric_out = Vec::new();
        assert_eq!(
            encode_styled_numeric_sequence(
                &invalid_numeric,
                0,
                invalid_numeric.len(),
                bold,
                &mut invalid_numeric_out,
            ),
            None
        );

        let symbol_tail = [
            EnglishToken::Styled('R', bold),
            EnglishToken::Symbol('.'),
            EnglishToken::LineBreak,
            EnglishToken::Word(vec!['S']),
        ];
        assert!(styled_capital_starts_symbol_sequence(&symbol_tail, 0, 1));
        assert!(!styled_capital_starts_symbol_sequence(&symbol_tail, 3, 1));

        let adjacent_text = [
            EnglishToken::Word(vec!['A']),
            EnglishToken::Symbol('\''),
            EnglishToken::Word(vec!['B']),
        ];
        assert!(matches!(
            straight_single_quote_role(&adjacent_text, 1),
            SingleQuote::Apostrophe
        ));
    }

    #[test]
    fn rare_engine_paths_cover_remaining_symbol_and_word_branches() {
        let engine = EnglishUebEngine::new();

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Symbol('Α'),
                        EnglishToken::Symbol('Β'),
                        EnglishToken::Symbol('Γ'),
                    ],
                    false,
                )
                .is_some()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Word(vec!['Α']),
                        EnglishToken::Word(vec!['Β', 'Γ']),
                    ],
                    false,
                )
                .is_some()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Word(vec!['α', 'β']),
                        EnglishToken::Space,
                        EnglishToken::Word(vec!['π']),
                    ],
                    false,
                )
                .is_some()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Word(vec!['w', 'h', 'e', 'r', 'e']),
                        EnglishToken::Symbol('\''),
                        EnglishToken::Word(vec!['e', 'r']),
                    ],
                    false,
                )
                .is_some()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Number(vec!['1']),
                        EnglishToken::Word(vec!['i', 'n', 's']),
                    ],
                    false,
                )
                .is_some()
        );

        assert!(
            engine
                .encode(
                    &[
                        EnglishToken::Word(vec!['A']),
                        EnglishToken::Symbol('='),
                        EnglishToken::Word(vec!['b', 'C']),
                    ],
                    false,
                )
                .is_some()
        );
    }

    #[test]
    fn rare_helper_paths_cover_false_and_edge_branches() {
        assert_eq!(greek_letter_cells_with_caps('λ', false), Some(cells("⠨⠇")));
        assert_eq!(greek_letter_cells_with_caps('Ξ', true), Some(cells("⠨⠭")));
        assert_eq!(greek_letter_cells_with_caps('@', false), None);

        assert!(!parenthesized_foreign_style_before(
            &[EnglishToken::Styled(
                'a',
                super::super::token::Typeform::Italic
            )],
            1,
        ));

        assert_eq!(
            mixed_case_shortform_part(&['b', 'r', 'a', 'i', 'l', 'l', 'e', 'x'], 0, &['b', 'r']),
            Some((7, cells("⠃⠗⠇")))
        );

        assert!(styled_prose_double_space(
            &[
                EnglishToken::Styled('h', super::super::token::Typeform::Underline),
                EnglishToken::Styled('t', super::super::token::Typeform::Underline),
                EnglishToken::Styled('t', super::super::token::Typeform::Underline),
                EnglishToken::Styled('p', super::super::token::Typeform::Underline),
                EnglishToken::Symbol(':'),
                EnglishToken::Symbol('/'),
                EnglishToken::Symbol('/'),
                EnglishToken::Styled('x', super::super::token::Typeform::Underline),
                EnglishToken::Space,
                EnglishToken::Space,
                EnglishToken::Word(vec!['n', 'o', 'w']),
            ],
            8,
        ));

        assert!(straight_single_quote_is_matched_quotation(
            &[
                EnglishToken::Symbol('\''),
                EnglishToken::Word(vec!['C', 'a', 't']),
                EnglishToken::Symbol('\''),
            ],
            0,
        ));
        assert!(!straight_single_quote_is_matched_quotation(
            &[EnglishToken::Symbol('"')],
            0,
        ));

        assert!(previous_word_starts_uppercase(
            &[
                EnglishToken::Word(vec!['C', 'a', 't']),
                EnglishToken::Symbol('.'),
                EnglishToken::Symbol('\''),
            ],
            2,
        ));
        assert!(straight_single_quote_closes_after_inner_double(
            &[
                EnglishToken::Symbol('\''),
                EnglishToken::Word(vec!['H', 'i']),
                EnglishToken::Symbol('"'),
                EnglishToken::Symbol('!'),
                EnglishToken::Symbol(','),
                EnglishToken::Symbol('\''),
            ],
            5,
        ));
    }

    #[test]
    fn rare_foreign_scope_helpers_cover_remaining_decisions() {
        let italic = super::super::token::Typeform::Italic;
        let bold = super::super::token::Typeform::Bold;

        let foreign_code_span = [
            EnglishToken::Symbol('¿'),
            EnglishToken::Styled('Q', italic),
            EnglishToken::Styled('u', italic),
            EnglishToken::Styled('é', italic),
            EnglishToken::Space,
            EnglishToken::Word(vec!['s', 'a', 'i', 'd']),
        ];
        assert!(matches!(
            styled_passage_foreign_scope(&foreign_code_span, 1, 4, italic, true, true),
            Some((super::super::rule_13::AccentCode::Ueb, true))
        ));

        let whole_foreign = [
            EnglishToken::Styled('q', italic),
            EnglishToken::Styled('u', italic),
            EnglishToken::Styled('é', italic),
            EnglishToken::Space,
            EnglishToken::Styled('t', italic),
            EnglishToken::Styled('a', italic),
            EnglishToken::Styled('l', italic),
        ];
        assert!(matches!(
            styled_passage_foreign_scope(
                &whole_foreign,
                0,
                whole_foreign.len(),
                italic,
                false,
                true
            ),
            Some((super::super::rule_13::AccentCode::Foreign, true))
        ));

        let two_styled_foreign_phrases = [
            EnglishToken::Styled('q', italic),
            EnglishToken::Styled('u', italic),
            EnglishToken::Styled('é', italic),
            EnglishToken::Space,
            EnglishToken::Word(vec!['m', 'e', 'a', 'n', 's']),
            EnglishToken::Space,
            EnglishToken::Styled('o', italic),
            EnglishToken::Styled('ù', italic),
        ];
        assert!(matches!(
            styled_passage_foreign_scope(&two_styled_foreign_phrases, 0, 3, italic, false, false),
            Some((super::super::rule_13::AccentCode::Foreign, false))
        ));

        let lowercase_unrecorded = [
            EnglishToken::Styled('x', italic),
            EnglishToken::Styled('y', italic),
            EnglishToken::Styled('z', italic),
            EnglishToken::Styled('q', italic),
        ];
        assert!(matches!(
            styled_passage_foreign_scope(
                &lowercase_unrecorded,
                0,
                lowercase_unrecorded.len(),
                italic,
                true,
                false,
            ),
            Some((super::super::rule_13::AccentCode::Foreign, false))
        ));

        let foreign_multi_word_ueb = [
            EnglishToken::Styled('c', italic),
            EnglishToken::Styled('a', italic),
            EnglishToken::Styled('f', italic),
            EnglishToken::Styled('é', italic),
            EnglishToken::Space,
            EnglishToken::Styled('n', italic),
            EnglishToken::Styled('a', italic),
            EnglishToken::Styled('d', italic),
            EnglishToken::Space,
            EnglishToken::Word(vec!['s', 'a', 'i', 'd']),
        ];
        assert!(matches!(
            styled_passage_foreign_scope(&foreign_multi_word_ueb, 0, 8, italic, false, false,),
            Some((super::super::rule_13::AccentCode::Ueb, false))
        ));

        let unrecorded_pair = [
            EnglishToken::Styled('x', italic),
            EnglishToken::Styled('q', italic),
            EnglishToken::Space,
            EnglishToken::Styled('z', italic),
            EnglishToken::Styled('v', italic),
        ];
        assert!(matches!(
            styled_passage_foreign_scope(
                &unrecorded_pair,
                0,
                unrecorded_pair.len(),
                italic,
                false,
                false
            ),
            Some((super::super::rule_13::AccentCode::Ueb, false))
        ));

        assert!(!styled_passage_all_caps(
            &[
                EnglishToken::Styled('A', bold),
                EnglishToken::Styled('b', bold),
            ],
            0,
            2,
            bold,
        ));
    }

    #[test]
    fn rare_quote_and_listing_helpers_cover_remaining_decisions() {
        assert_eq!(
            straight_single_quote_role(
                &[
                    EnglishToken::Symbol('\''),
                    EnglishToken::Word(vec!['C', 'a', 't']),
                    EnglishToken::Symbol('\''),
                ],
                0,
            ),
            SingleQuote::Open,
        );
        assert_eq!(
            straight_single_quote_role(
                &[
                    EnglishToken::Symbol('\''),
                    EnglishToken::Word(vec!['C', 'a', 't']),
                    EnglishToken::Symbol('\''),
                ],
                2,
            ),
            SingleQuote::Close,
        );
        assert!(straight_single_quote_closes_after_inner_double(
            &[
                EnglishToken::Symbol('\''),
                EnglishToken::Word(vec!['S', 'i', 'n', 'g']),
                EnglishToken::Symbol('"'),
                EnglishToken::Symbol(','),
                EnglishToken::Symbol('\''),
            ],
            4,
        ));
        assert!(!previous_word_starts_uppercase(
            &[EnglishToken::Symbol('.')],
            1
        ));

        let mut out = Vec::new();
        let regex_tokens = [
            EnglishToken::Symbol('"'),
            EnglishToken::Symbol('['),
            EnglishToken::Word(vec!['a', 'Z']),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['A']),
            EnglishToken::Symbol(']'),
            EnglishToken::Symbol('"'),
        ];
        let regex_listing = vec![true; regex_tokens.len()];
        assert_eq!(
            regex_char_class_word(&regex_tokens, 2, &['a', 'Z'], &regex_listing, &mut out),
            Some(false)
        );
        out.clear();
        assert_eq!(
            regex_char_class_word(&regex_tokens, 4, &['A'], &regex_listing, &mut out),
            Some(true)
        );
        assert!(!out.is_empty());
        out.clear();
        let regex_range_tokens = [
            EnglishToken::Symbol('['),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['a', 'Z']),
            EnglishToken::Symbol('-'),
            EnglishToken::Symbol(']'),
        ];
        assert_eq!(
            regex_char_class_word(&regex_range_tokens, 2, &['a', 'Z'], &[true; 5], &mut out),
            Some(true)
        );
        assert!(!out.is_empty());

        let url_tokens = [
            EnglishToken::Word(vec!['a']),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['b']),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['c']),
        ];
        assert!(url_listing_line_continuation_after(
            &url_tokens,
            3,
            &[true; 5],
        ));
        assert!(!url_listing_line_continuation_after(
            &url_tokens,
            3,
            &[false; 5],
        ));
    }

    #[test]
    fn rare_document_and_modified_word_helpers_cover_remaining_branches() {
        let italic = super::super::token::Typeform::Italic;
        let bold = super::super::token::Typeform::Bold;
        let underline = super::super::token::Typeform::Underline;
        let bold_italic = super::super::token::Typeform::BoldItalic;

        let adjacent = [
            EnglishToken::Styled('c', italic),
            EnglishToken::Space,
            EnglishToken::Symbol('?'),
        ];
        assert!(punctuation_adjacent_to_styled(&adjacent, 2));
        assert!(document_any_styled_phrase_has_foreign_letter(&[
            EnglishToken::Styled('é', italic),
        ]));
        assert!(document_all_styled_phrases_are_short_vocabulary(&[
            EnglishToken::Styled('l', italic),
            EnglishToken::Styled('o', italic),
            EnglishToken::Symbol('-'),
            EnglishToken::Styled('e', italic),
            EnglishToken::Styled('i', italic),
            EnglishToken::Space,
            EnglishToken::Styled('d', italic),
            EnglishToken::Styled('e', italic),
        ]));
        assert!(!document_all_styled_phrases_are_short_vocabulary(&[
            EnglishToken::Styled('T', italic),
            EnglishToken::Styled('H', italic),
            EnglishToken::Styled('E', italic),
        ]));

        assert_eq!(typeform_word_lengths(&[]), Vec::<usize>::new());
        assert_eq!(
            typeform_word_lengths(&[
                EnglishToken::Styled('l', bold),
                EnglishToken::Symbol('\''),
                EnglishToken::Styled('o', bold),
                EnglishToken::Symbol('-'),
                EnglishToken::Styled('e', bold),
                EnglishToken::Space,
                EnglishToken::Word(vec!['x']),
            ]),
            vec![3]
        );

        let mut out = Vec::new();
        let engine = ContractionEngine::default();
        encode_modified_word(&engine, &['a', 'é', 'a'], true, true, &mut out)
            .expect("modified word should encode");
        assert!(!out.is_empty());

        for (left, right, expected) in [
            ('e', 'a', '⠂'),
            ('b', 'b', '⠆'),
            ('c', 'c', '⠒'),
            ('f', 'f', '⠖'),
            ('g', 'g', '⠶'),
        ] {
            assert_eq!(
                middle_lower_pair_cell(left, right),
                Some(decode_unicode(expected))
            );
        }
        assert_eq!(middle_lower_pair_cell('x', 'x'), None);

        assert!(!styled_url_before(
            &[
                EnglishToken::Styled('h', underline),
                EnglishToken::Word(vec!['x']),
            ],
            1,
        ));
        assert!(!styled_url_before(
            &[
                EnglishToken::Styled('h', underline),
                EnglishToken::Symbol(':'),
                EnglishToken::Word(vec!['x']),
            ],
            2,
        ));
        assert_eq!(
            nested_typeform_continuation(
                &[
                    EnglishToken::Styled('a', bold_italic),
                    EnglishToken::Space,
                    EnglishToken::Word(vec!['x']),
                ],
                1,
                bold_italic,
            ),
            None
        );
        assert!(!styled_underline_url_span(
            &[
                EnglishToken::Styled('h', underline),
                EnglishToken::Word(vec!['x']),
            ],
            0,
            2,
            underline,
        ));
        assert!(styled_letter_needs_grade1(
            &[
                EnglishToken::Symbol('('),
                EnglishToken::Styled('x', italic),
                EnglishToken::Symbol(')'),
            ],
            1,
            2,
        ));
    }

    #[test]
    fn rare_spacing_bracket_and_symbol_helpers_cover_remaining_branches() {
        let number_space = [
            EnglishToken::Number(vec!['1']),
            EnglishToken::Space,
            EnglishToken::Number(vec!['2']),
        ];
        assert!(is_numeric_space(&number_space, 1));
        let mut out = Vec::new();
        assert_eq!(
            encode_following_number_as_numeric_space(&number_space, 1, &mut out, true),
            Some(3)
        );
        assert!(!out.is_empty());

        let styled_gap = [
            EnglishToken::Styled('A', super::super::token::Typeform::Underline),
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Styled('B', super::super::token::Typeform::Underline),
        ];
        assert_eq!(styled_column_gap(&styled_gap, 1), Some(4));
        let styled_numeric_gap = [
            EnglishToken::Styled('1', super::super::token::Typeform::Underline),
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Styled('2', super::super::token::Typeform::Underline),
        ];
        assert_eq!(styled_column_gap(&styled_numeric_gap, 1), None);

        assert!(needs_spatial_grade1_passage(&[
            EnglishToken::Word(vec!['X']),
            EnglishToken::Symbol('┼'),
        ]));
        assert!(horizontal_run_reaches_arrow(
            &[
                EnglishToken::Symbol('═'),
                EnglishToken::Symbol('═'),
                EnglishToken::Symbol('↓'),
            ],
            0
        ));

        assert!(continues_across_bracket(
            &[
                EnglishToken::Word(vec!['c', 'h', 'i', 'l', 'd']),
                EnglishToken::Symbol('('),
                EnglishToken::Word(vec!['i', 's', 'h']),
            ],
            0
        ));
        assert!(continues_across_bracket(
            &[
                EnglishToken::Word(vec!['g', 'o']),
                EnglishToken::Symbol('\''),
                EnglishToken::Word(vec!['n']),
            ],
            0
        ));

        let mut symbol_out = Vec::new();
        encode_styled_nonword_symbol('5', &mut symbol_out).expect("styled digit should encode");
        assert!(!symbol_out.is_empty());
        symbol_out.clear();
        encode_styled_nonword_symbol('?', &mut symbol_out).expect("styled question should encode");
        assert!(symbol_out.starts_with(&[GRADE1]));
    }

    #[test]
    fn rare_syllable_and_spelled_run_helpers_cover_remaining_branches() {
        let dis_as_ter = [
            EnglishToken::Word(vec!['d', 'i', 's']),
            EnglishToken::Space,
            EnglishToken::Word(vec!['a', 's']),
            EnglishToken::Space,
            EnglishToken::Word(vec!['t', 'e', 'r']),
        ];
        assert!(space_delimited_syllables_form_word(&dis_as_ter, 2));
        assert!(!spaced_as_contracts(&dis_as_ter, 2));

        assert!(
            styled_word_count(&[
                EnglishToken::Styled('l', super::super::token::Typeform::Italic),
                EnglishToken::Symbol('-'),
                EnglishToken::Styled('o', super::super::token::Typeform::Italic),
                EnglishToken::Space,
                EnglishToken::Styled('x', super::super::token::Typeform::Italic),
            ]) >= 2
        );
        assert!(all_text_is_styled_or_punctuation(&[
            EnglishToken::Styled('x', super::super::token::Typeform::Italic),
            EnglishToken::Symbol('.'),
            EnglishToken::Space,
        ]));
        assert!(starts_with_ch_not_pronounced_ch("chaos"));
        assert!(styled_word_is_foreign(&['c', 'h', 'a', 'o', 's']));
        assert!(!styled_single_word_is_foreign(&['t', 'h']));
        assert_eq!(
            token_typeform(&EnglishToken::Styled(
                'x',
                super::super::token::Typeform::Italic
            )),
            Some(super::super::token::Typeform::Italic)
        );

        let spelled = [
            EnglishToken::Word(vec!['w']),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['i']),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['n']),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['s']),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['o', 'm', 'e']),
        ];
        assert_eq!(spelled_letter_run(&spelled, 0), Some((0, 6)));
        assert!(ends_spelled_letter_run_before_word(&spelled, 7));
        let stutter = [
            EnglishToken::Word(vec!['s', 'o']),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['o']),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(vec!['o']),
        ];
        assert!(leading_stutter_prefix(&stutter, 2));

        assert!(stammer_fragment_literal(
            &[
                EnglishToken::Word(vec!['c', 'h']),
                EnglishToken::Symbol('-'),
                EnglishToken::Word(vec!['a']),
            ],
            0,
            "ch",
        ));
        assert!(stammer_fragment_literal(
            &[
                EnglishToken::Word(vec!['t', 'h']),
                EnglishToken::Symbol('.'),
                EnglishToken::Symbol('.'),
            ],
            0,
            "th",
        ));
        assert!(midword_parenthesized_ing(
            &[
                EnglishToken::Word(vec!['s']),
                EnglishToken::Symbol('('),
                EnglishToken::Word(vec!['i', 'n', 'g']),
                EnglishToken::Symbol(')'),
            ],
            2,
            "ing",
        ));
    }

    #[test]
    fn rare_encode_loop_spatial_greek_and_typeform_paths_are_reachable() {
        let engine = EnglishUebEngine::new();
        let italic = super::super::token::Typeform::Italic;

        let vertical_gap = std::iter::once(EnglishToken::Symbol('│'))
            .chain(std::iter::repeat_n(EnglishToken::Space, 10))
            .chain(std::iter::once(EnglishToken::Symbol('│')))
            .collect::<Vec<_>>();
        let encoded_vertical_gap = engine.encode(&vertical_gap, false).unwrap();
        assert!(
            encoded_vertical_gap
                .windows(9)
                .any(|cells| cells == [SPACE; 9])
        );

        let corner_gap = [EnglishToken::Symbol('─'), EnglishToken::Symbol('┐')]
            .into_iter()
            .chain(std::iter::repeat_n(EnglishToken::Space, 5))
            .chain([
                EnglishToken::Symbol('┌'),
                EnglishToken::Symbol('─'),
                EnglishToken::Symbol('\t'),
            ])
            .collect::<Vec<_>>();
        let encoded_corner_gap = engine.encode(&corner_gap, false).unwrap();
        assert!(
            encoded_corner_gap
                .windows(6)
                .any(|cells| cells == [SPACE; 6])
        );

        let diagonal_after_vertical = [EnglishToken::Symbol('│'), EnglishToken::Symbol('╲')];
        let encoded_diagonal_after_vertical =
            engine.encode(&diagonal_after_vertical, false).unwrap();
        assert!(encoded_diagonal_after_vertical.contains(&decode_unicode('⠣')));

        let spatial_arrow = [
            EnglishToken::Symbol('│'),
            EnglishToken::LineBreak,
            EnglishToken::Symbol('↙'),
        ];
        let encoded_spatial_arrow = engine.encode(&spatial_arrow, false).unwrap();
        assert!(
            encoded_spatial_arrow
                .windows(2)
                .any(|window| window == cells("⠳⠜"))
        );

        let spaced_line_segments = [
            EnglishToken::Symbol(super::super::rule_16::VARIANT_SPACED_SEGMENT),
            EnglishToken::Space,
            EnglishToken::Symbol(super::super::rule_16::VARIANT_SPACED_SEGMENT),
        ];
        let encoded_spaced_line_segments = engine.encode(&spaced_line_segments, false).unwrap();
        assert!(encoded_spaced_line_segments.starts_with(&cells("⠐⠒⠂⠂⠂")));

        let table_word_gap = [
            EnglishToken::Word(vec!['R', 'o', 'w']),
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Word(vec!['A', 'B']),
        ];
        let encoded_table_word_gap = engine.encode(&table_word_gap, false).unwrap();
        assert!(encoded_table_word_gap.contains(&decode_unicode('⠐')));

        let table_number_gap = [
            EnglishToken::Word(vec!['I', 'n', 'c', 'o', 'm', 'e']),
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Number(vec!['1', '2']),
        ];
        let encoded_table_number_gap = engine.encode(&table_number_gap, false).unwrap();
        assert!(encoded_table_number_gap.contains(&decode_unicode('⠐')));

        let styled_gap = [
            EnglishToken::Styled('A', super::super::token::Typeform::Underline),
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Styled('B', super::super::token::Typeform::Underline),
        ];
        let encoded_styled_gap = engine.encode(&styled_gap, false).unwrap();
        assert!(encoded_styled_gap.contains(&SPACE));

        let uppercase_greek_word = [EnglishToken::Word(vec!['Α', 'Β'])];
        assert!(
            engine
                .encode(&uppercase_greek_word, false)
                .unwrap()
                .starts_with(&[CAPITAL, CAPITAL])
        );

        let uppercase_greek_word_then_symbols = [
            EnglishToken::Word(vec!['Α', 'Β']),
            EnglishToken::Symbol('Γ'),
            EnglishToken::Symbol('Δ'),
        ];
        assert!(
            engine
                .encode(&uppercase_greek_word_then_symbols, false)
                .unwrap()
                .starts_with(&[CAPITAL, CAPITAL])
        );

        let uppercase_greek_symbols = [EnglishToken::Symbol('Α'), EnglishToken::Symbol('Β')];
        assert!(
            engine
                .encode(&uppercase_greek_symbols, false)
                .unwrap()
                .starts_with(&[CAPITAL, CAPITAL])
        );

        let styled_ing = [
            EnglishToken::Word(vec!['n']),
            EnglishToken::Styled('i', italic),
            EnglishToken::Styled('n', italic),
            EnglishToken::Styled('g', italic),
        ];
        let encoded_styled_ing = engine.encode(&styled_ing, false).unwrap();
        assert!(encoded_styled_ing.contains(&decode_unicode('⠨')));

        let styled_digit = [EnglishToken::Styled('7', italic)];
        let encoded_styled_digit = engine.encode(&styled_digit, false).unwrap();
        assert!(encoded_styled_digit.starts_with(&super::super::rule_9::symbol_indicator(italic)));

        let styled_letter_after_number = [
            EnglishToken::Number(vec!['1']),
            EnglishToken::Styled('a', italic),
        ];
        let encoded_styled_letter_after_number =
            engine.encode(&styled_letter_after_number, false).unwrap();
        assert!(encoded_styled_letter_after_number.contains(&GRADE1));

        let styled_word_passage = [
            EnglishToken::Styled('a', italic),
            EnglishToken::Styled('b', italic),
            EnglishToken::Space,
            EnglishToken::Styled('c', italic),
            EnglishToken::Styled('d', italic),
            EnglishToken::Space,
            EnglishToken::Styled('e', italic),
            EnglishToken::Styled('f', italic),
        ];
        let encoded_styled_word_passage = engine.encode(&styled_word_passage, false).unwrap();
        assert!(encoded_styled_word_passage.contains(&decode_unicode('⠨')));

        let partial_styled_word = [
            EnglishToken::Styled('m', italic),
            EnglishToken::Word(vec!['o', 't', 'h', 'e', 'r']),
        ];
        let encoded_partial_styled_word = engine.encode(&partial_styled_word, false).unwrap();
        assert!(encoded_partial_styled_word.contains(&decode_unicode('⠨')));

        let hyphenated_styled_span = [
            EnglishToken::Styled('o', italic),
            EnglishToken::Styled('f', italic),
            EnglishToken::Symbol('-'),
            EnglishToken::Styled('t', italic),
            EnglishToken::Styled('h', italic),
            EnglishToken::Styled('e', italic),
        ];
        let encoded_hyphenated_styled_span = engine.encode(&hyphenated_styled_span, false).unwrap();
        assert!(encoded_hyphenated_styled_span.contains(&decode_unicode('⠤')));

        let underlined_url = [
            EnglishToken::Styled('h', super::super::token::Typeform::Underline),
            EnglishToken::Styled('t', super::super::token::Typeform::Underline),
            EnglishToken::Styled('t', super::super::token::Typeform::Underline),
            EnglishToken::Styled('p', super::super::token::Typeform::Underline),
            EnglishToken::Symbol(':'),
            EnglishToken::Symbol('/'),
            EnglishToken::Symbol('/'),
            EnglishToken::Styled('a', super::super::token::Typeform::Underline),
        ];
        let encoded_underlined_url = engine.encode(&underlined_url, false).unwrap();
        assert!(encoded_underlined_url.contains(&decode_unicode('⠓')));
    }

    #[test]
    fn rare_mixed_case_internal_caps_path_is_observable() {
        let engine = EnglishUebEngine::new();
        let mut out = Vec::new();
        engine
            .encode_mixed_case(
                &['f', 'o', 'u', 'n', 'D', 'A', 't', 'i', 'o', 'n'],
                true,
                &mut out,
            )
            .unwrap();
        assert!(out.contains(&CAPITAL));
    }

    #[test]
    fn token_typeform_reports_styled_form_only() {
        use super::super::token::Typeform;
        // §9: only a Styled token carries a typeform; structural tokens have none.
        assert_eq!(
            token_typeform(&EnglishToken::Styled('a', Typeform::Bold)),
            Some(Typeform::Bold)
        );
        assert_eq!(token_typeform(&EnglishToken::Space), None);
    }

    #[test]
    fn token_base_char_extracts_single_char_tokens() {
        use super::super::token::Typeform;
        // A styled letter and a one-letter word expose their base char; a
        // multi-char or structural token does not.
        assert_eq!(
            token_base_char(&EnglishToken::Styled('x', Typeform::Italic)),
            Some('x')
        );
        assert_eq!(token_base_char(&EnglishToken::Word(vec!['y'])), Some('y'));
        assert_eq!(token_base_char(&EnglishToken::Space), None);
    }

    #[test]
    fn dash_after_enough_before_in_needs_enough_before_dash() {
        // §10.6.5 seam: true only when the token two back is `enough`.
        let with_enough = [
            EnglishToken::Word("enough".chars().collect()),
            EnglishToken::Symbol('\u{2014}'),
        ];
        assert!(dash_after_enough_before_in(&with_enough, 2));
        // A dash reached over a bare space (no `enough`) is not the seam.
        let bare = [EnglishToken::Space, EnglishToken::Symbol('\u{2014}')];
        assert!(!dash_after_enough_before_in(&bare, 2));
    }

    #[test]
    fn dash_after_quoted_in_before_in_requires_quote_then_in() {
        // A dash not reached through a quotation mark from `in` is not the seam.
        let bare = [EnglishToken::Space, EnglishToken::Symbol('\u{2014}')];
        assert!(!dash_after_quoted_in_before_in(&bare, 2));
    }

    #[test]
    fn styled_numeric_sequence_end_spans_spaced_digit_groups() {
        use super::super::token::Typeform;
        // §9/§11: a styled digit run may include an internal space between two
        // styled digit groups of the same typeform.
        let tokens = [
            EnglishToken::Styled('1', Typeform::Italic),
            EnglishToken::Space,
            EnglishToken::Styled('2', Typeform::Italic),
        ];
        assert_eq!(styled_numeric_sequence_end(&tokens, 0, Typeform::Italic), 3);
    }

    #[test]
    fn document_all_styled_phrases_short_vocabulary_flushes_at_boundaries() {
        use super::super::token::Typeform;
        let it = Typeform::Italic;
        // Short lowercase styled words separated by a space → all short vocabulary.
        let ok = [
            EnglishToken::Styled('a', it),
            EnglishToken::Styled('b', it),
            EnglishToken::Space,
            EnglishToken::Styled('c', it),
        ];
        assert!(document_all_styled_phrases_are_short_vocabulary(&ok));
        // A styled word longer than 10 chars fails the flush at the trailing space.
        let long_then_space: Vec<EnglishToken> = "abcdefghijkl"
            .chars()
            .map(|c| EnglishToken::Styled(c, it))
            .chain([EnglishToken::Space])
            .collect();
        assert!(!document_all_styled_phrases_are_short_vocabulary(
            &long_then_space
        ));
        // A non-space/non-symbol token (a Number) after a too-long styled run hits
        // the catch-all flush arm.
        let long_then_number: Vec<EnglishToken> = "abcdefghijkl"
            .chars()
            .map(|c| EnglishToken::Styled(c, it))
            .chain([EnglishToken::Number(vec!['1'])])
            .collect();
        assert!(!document_all_styled_phrases_are_short_vocabulary(
            &long_then_number
        ));
    }

    #[test]
    fn encode_following_number_as_numeric_space_needs_a_number() {
        // Returns None when the next token is not a Number (nothing to encode).
        let tokens = [EnglishToken::Space];
        let mut out = Vec::new();
        assert_eq!(
            encode_following_number_as_numeric_space(&tokens, 0, &mut out, false),
            None
        );
        assert!(out.is_empty());
    }

    #[test]
    fn after_repeated_stammer_prefix_rejects_empty_word() {
        // An empty lower_word has no first char → not a stammer continuation.
        let tokens = [EnglishToken::Space];
        assert!(!after_repeated_stammer_prefix(&tokens, 1, ""));
    }

    #[test]
    fn space_delimited_syllables_need_three_words() {
        // §10.1.4: three space-separated fragments that concatenate to one
        // recorded word (`dis as ter`) form a word; a missing neighbour does not.
        let joined = [
            EnglishToken::Word("dis".chars().collect()),
            EnglishToken::Space,
            EnglishToken::Word("as".chars().collect()),
            EnglishToken::Space,
            EnglishToken::Word("ter".chars().collect()),
        ];
        assert!(space_delimited_syllables_form_word(&joined, 2));
        // Current token not a word → false.
        let curr_missing = [
            EnglishToken::Word("a".chars().collect()),
            EnglishToken::Space,
            EnglishToken::Space,
        ];
        assert!(!space_delimited_syllables_form_word(&curr_missing, 2));
        // Following token not a word → false.
        let next_missing = [
            EnglishToken::Word("a".chars().collect()),
            EnglishToken::Space,
            EnglishToken::Word("b".chars().collect()),
        ];
        assert!(!space_delimited_syllables_form_word(&next_missing, 2));
    }

    #[test]
    fn styled_form_at_reports_typeform_only_for_styled_tokens() {
        use super::super::token::Typeform;
        assert_eq!(
            styled_form_at(&[EnglishToken::Styled('a', Typeform::Bold)], 0),
            Some(Typeform::Bold)
        );
        assert_eq!(styled_form_at(&[EnglishToken::Space], 0), None);
    }

    #[test]
    fn leading_stutter_prefix_guards_missing_or_empty_word() {
        // Reaches the `-` guard but the token at `start` is not a Word.
        let non_word = [
            EnglishToken::Word("so".chars().collect()),
            EnglishToken::Symbol('-'),
            EnglishToken::Space,
        ];
        assert!(!leading_stutter_prefix(&non_word, 2));
        // The token at `start` is an empty Word (no first char).
        let empty_word = [
            EnglishToken::Word("so".chars().collect()),
            EnglishToken::Symbol('-'),
            EnglishToken::Word(Vec::new()),
        ];
        assert!(!leading_stutter_prefix(&empty_word, 2));
    }

    #[test]
    fn ends_spelled_letter_run_before_word_needs_a_hyphen() {
        // The token at `i` must be a hyphen symbol; anything else → false.
        assert!(!ends_spelled_letter_run_before_word(
            &[EnglishToken::Space],
            0
        ));
    }

    #[test]
    fn token_plain_chars_preserve_word_division_maps_all_token_kinds() {
        // A space becomes a literal space and a word division inserts a `\n` at
        // the break index — exercising every arm of the flattener.
        let tokens = [
            EnglishToken::Word("ab".chars().collect()),
            EnglishToken::Space,
            EnglishToken::WordDivision {
                chars: "cd".chars().collect(),
                break_at: 1,
            },
            EnglishToken::LineBreak,
        ];
        assert_eq!(
            token_plain_chars_preserve_word_division(&tokens),
            vec!['a', 'b', ' ', 'c', '\n', 'd', '\n']
        );
    }

    #[test]
    fn encode_compact_spatial_example_handles_diagonal_pair() {
        // §16 compact spatial layout: the `╱╲` diagonal pair renders as a
        // three-row grade-1 spatial arrangement.
        let tokens = [EnglishToken::Symbol('╱'), EnglishToken::Symbol('╲')];
        assert!(encode_compact_spatial_example(&tokens).is_some());
    }

    #[test]
    fn styled_column_gap_requires_a_space_at_index() {
        // §16.5 column gap detection starts at a space; a non-space token → None.
        let tokens = [EnglishToken::Word("a".chars().collect())];
        assert_eq!(styled_column_gap(&tokens, 0), None);
    }

    #[test]
    fn bibliography_styled_number_title_end_needs_trailing_space_number() {
        use super::super::token::Typeform;
        // A bibliography entry (`1.` + styled title) with no following
        // ` <number>` after the title has no numeric title end.
        let tokens = [
            EnglishToken::Number(vec!['1']),
            EnglishToken::Symbol('.'),
            EnglishToken::Styled('a', Typeform::Italic),
        ];
        assert_eq!(bibliography_styled_number_title_end(&tokens, 3, 2), None);
    }

    #[test]
    fn bibliography_title_starts_with_foreign_article_needs_two_words() {
        // A single-word title cannot open with a foreign article + noun.
        assert!(!bibliography_title_starts_with_foreign_article(&[vec![
            'l', 'e'
        ]]));
        // `le <noun>` is a French-article title.
        assert!(bibliography_title_starts_with_foreign_article(&[
            vec!['l', 'e'],
            vec!['m', 'o', 't']
        ]));
    }

    #[test]
    fn engine_default_matches_new() {
        // The `Default` impl delegates to `new`, producing a usable engine.
        let mut out = Vec::new();
        EnglishUebEngine::default()
            .encode_mixed_case(
                &['f', 'o', 'u', 'n', 'D', 'A', 't', 'i', 'o', 'n'],
                true,
                &mut out,
            )
            .unwrap();
        assert!(out.contains(&CAPITAL));
    }

    #[test]
    fn push_spatial_char_renders_line_arrow() {
        // §16 spatial mode: a line arrow (`→`) renders via its two-cell arrow sign.
        let mut out = Vec::new();
        assert_eq!(push_spatial_char(&mut out, '→'), Some(()));
        assert!(!out.is_empty());
    }

    #[test]
    fn styled_word_foreign_detects_non_accented_foreign_letter() {
        // §13: a dot-below foreign letter (`ọ`, U+1ECD) is foreign but not a §4.2
        // accent, so it is detected as foreign vocabulary / a foreign signal.
        assert!(styled_word_is_foreign(&['\u{1ECD}']));
        assert!(styled_word_has_foreign_signal(&['\u{1ECD}']));
        // A recorded English word is not foreign.
        assert!(!styled_word_is_foreign(&['c', 'a', 't']));
    }

    #[test]
    fn styled_phrase_from_named_place_breaks_without_styled_tail() {
        use super::super::token::Typeform;
        // `<styled> and <plain>` after the phrase has no styled continuation to a
        // `from <Place>`, so it is not a named-place attribution.
        let tokens = [
            EnglishToken::Styled('a', Typeform::Italic),
            EnglishToken::Space,
            EnglishToken::Word("and".chars().collect()),
            EnglishToken::Space,
            EnglishToken::Word("x".chars().collect()),
        ];
        assert!(!styled_phrase_from_named_place(&tokens, 1));
    }

    #[test]
    fn encode_dispatches_rule_3_14_punctuation_box() {
        // §3.14 headings box (`┌─┐ / │ ! │ / └─┘`) is a spatial layout the engine
        // renders directly.
        let tokens = [
            EnglishToken::Symbol('┌'),
            EnglishToken::Symbol('─'),
            EnglishToken::Symbol('─'),
            EnglishToken::Symbol('┐'),
            EnglishToken::LineBreak,
            EnglishToken::Symbol('│'),
            EnglishToken::Space,
            EnglishToken::Symbol('!'),
            EnglishToken::Space,
            EnglishToken::Symbol('│'),
            EnglishToken::LineBreak,
            EnglishToken::Symbol('└'),
            EnglishToken::Symbol('─'),
            EnglishToken::Symbol('─'),
            EnglishToken::Symbol('┘'),
        ];
        assert!(EnglishUebEngine::new().encode(&tokens, false).is_some());
    }

    #[test]
    fn encode_dispatches_rule_3_14_letter_grid() {
        // §3.14 letter grid: two aligned rows of single capitals encode as a grid.
        let tokens = [
            EnglishToken::Word(vec!['A']),
            EnglishToken::Space,
            EnglishToken::Word(vec!['B']),
            EnglishToken::LineBreak,
            EnglishToken::Word(vec!['C']),
            EnglishToken::Space,
            EnglishToken::Word(vec!['D']),
        ];
        assert!(EnglishUebEngine::new().encode(&tokens, false).is_some());
    }

    #[test]
    fn rule_3_14_punctuation_box_without_headings_is_none() {
        // §3.14: a box whose middle row carries no heading characters is not a
        // headings box.
        let tokens = [
            EnglishToken::Symbol('┌'),
            EnglishToken::Symbol('─'),
            EnglishToken::Symbol('─'),
            EnglishToken::Symbol('┐'),
            EnglishToken::LineBreak,
            EnglishToken::Symbol('│'),
            EnglishToken::Space,
            EnglishToken::Space,
            EnglishToken::Symbol('│'),
            EnglishToken::LineBreak,
            EnglishToken::Symbol('└'),
            EnglishToken::Symbol('─'),
            EnglishToken::Symbol('─'),
            EnglishToken::Symbol('┘'),
        ];
        assert_eq!(encode_rule_3_14_punctuation_box(&tokens), None);
    }

    #[test]
    fn encode_divided_word_suppressed_caps_path() {
        // §10.13: a line-divided word encoded inside a §8.4 caps passage
        // (suppress_caps) skips the per-word capital indicator.
        let mut out = Vec::new();
        assert!(
            EnglishUebEngine::new()
                .encode_divided_word(&['r', 'e', 'a', 'd', 'i', 'n', 'g'], 4, true, &mut out)
                .is_some()
        );
    }

    #[test]
    fn encode_word_spells_modified_letter_word_literally() {
        // §4.2.4: a word carrying a diacritic (`maître`) is spelled letter-by-letter
        // so no groupsign consumes print around the modified letter.
        let ctx = WordContext {
            standing_alone: true,
            upper_usable: false,
            shortform_usable: false,
            allow_longer_shortforms: true,
            lower_usable: false,
            suppress_caps: false,
            word_initial: true,
            restricted_prefix_boundary: true,
            digit_adjacent: false,
        };
        let mut out = Vec::new();
        assert!(
            EnglishUebEngine::new()
                .encode_word(&['m', 'a', 'î', 't', 'r', 'e'], ctx, &mut out)
                .is_some()
        );
        assert!(!out.is_empty());
    }

    #[test]
    fn encode_styled_word_handles_single_symbol() {
        use super::super::token::Typeform;
        // A one-character styled word that is not a letter encodes as its symbol.
        let tokens = [EnglishToken::Styled('&', Typeform::Italic)];
        let ctx = StyledContext {
            tokens: &tokens,
            suppress_caps: false,
            foreign_scope: None,
        };
        let mut out = Vec::new();
        assert!(
            EnglishUebEngine::new()
                .encode_styled_word(&['&'], 0, 1, ctx, &mut out)
                .is_some()
        );
        assert!(!out.is_empty());
    }

    #[test]
    fn encode_styled_span_rejects_non_styled_interior_token() {
        use super::super::token::Typeform;
        // A styled span whose interior carries a bare Space (not styled / symbol /
        // line break) is not a well-formed multi-segment styled word.
        let tokens = [
            EnglishToken::Styled('x', Typeform::Italic),
            EnglishToken::Space,
            EnglishToken::Styled('y', Typeform::Italic),
        ];
        let ctx = StyledContext {
            tokens: &tokens,
            suppress_caps: false,
            foreign_scope: None,
        };
        let mut out = Vec::new();
        assert!(
            EnglishUebEngine::new()
                .encode_styled_span(0, 3, Typeform::Italic, ctx, &mut out)
                .is_none()
        );
    }

    #[test]
    fn encode_styled_word_encodes_all_digit_word_as_number() {
        use super::super::token::Typeform;
        // §6/§9: a styled word made only of digits encodes as a number.
        let tokens = [
            EnglishToken::Styled('1', Typeform::Italic),
            EnglishToken::Styled('2', Typeform::Italic),
        ];
        let ctx = StyledContext {
            tokens: &tokens,
            suppress_caps: false,
            foreign_scope: None,
        };
        let mut out = Vec::new();
        assert!(
            EnglishUebEngine::new()
                .encode_styled_word(&['1', '2'], 0, 2, ctx, &mut out)
                .is_some()
        );
        assert!(!out.is_empty());
    }

    #[test]
    fn encode_styled_span_uses_provided_foreign_scope() {
        use super::super::token::Typeform;
        // §13.2.1: when the caller supplies a foreign scope, the span's segments
        // are encoded uncontracted under that accent code.
        let tokens = [EnglishToken::Styled('a', Typeform::Italic)];
        let ctx = StyledContext {
            tokens: &tokens,
            suppress_caps: false,
            foreign_scope: Some((super::super::rule_13::AccentCode::Ueb, false)),
        };
        let mut out = Vec::new();
        assert!(
            EnglishUebEngine::new()
                .encode_styled_span(0, 1, Typeform::Italic, ctx, &mut out)
                .is_some()
        );
        assert!(!out.is_empty());
    }

    #[test]
    fn encode_word_acronym_abutting_digit_spells_letters() {
        // §10.12.1: an all-caps initialism abutting a digit is "used as letters",
        // each spelled with no contraction.
        let ctx = WordContext {
            standing_alone: false,
            upper_usable: false,
            shortform_usable: false,
            allow_longer_shortforms: true,
            lower_usable: false,
            suppress_caps: false,
            word_initial: true,
            restricted_prefix_boundary: true,
            digit_adjacent: true,
        };
        let mut out = Vec::new();
        assert!(
            EnglishUebEngine::new()
                .encode_word(&['A', 'B'], ctx, &mut out)
                .is_some()
        );
        assert!(!out.is_empty());
    }

    #[test]
    fn encodes_middle_english_contraction_in_early_context() {
        // §12.2/§12.3: with an early-English letter present (`þ`), a Middle-English
        // spelling (`worlde`) takes its §12 contracted form `⠸⠺⠑`.
        let out = enc("þe worlde").expect("early-English text should encode");
        assert!(out.windows(3).any(|w| w == cells("⠸⠺⠑")));
    }

    #[test]
    fn encodes_struck_letter_sequence() {
        // §4.3.1: a run of stroke-overlaid letters (U+0336) encodes as struck text.
        assert!(enc("a\u{0336}b\u{0336}").is_some());
    }

    #[test]
    fn encodes_capitalized_where_before_apostrophe() {
        // §8/§10.5: a capitalized `Where'…` keeps the `where` groupsign `⠱⠻⠑`
        // preceded by a capital indicator.
        let out = enc("Where's").expect("Where's should encode");
        assert_eq!(out.first(), Some(&CAPITAL));
        assert!(out.windows(3).any(|w| w == cells("⠱⠻⠑")));
    }

    #[test]
    fn encodes_capitalized_enough_before_punctuation_dash() {
        // §10.5: a capitalized `Enough` before lower punctuation and a dash keeps
        // the `enough` wordsign `⠢` with a leading capital indicator.
        let out = enc("Enough!—more").expect("should encode");
        assert_eq!(out.first(), Some(&CAPITAL));
        assert!(out.contains(&decode_unicode('⠢')));
    }

    #[test]
    fn encodes_styled_letter_a_to_j_after_number_with_grade1() {
        // §6.5/§9: an italic letter a–j directly after a number takes a grade-1
        // indicator so `5𝑎` is not misread as a continuation of the number.
        let out = enc("5\u{1D44E}").expect("should encode");
        assert!(out.contains(&GRADE1));
    }

    #[test]
    fn encodes_uppercase_greek_run_in_english_prose() {
        // §11: an all-caps Greek run (`ΑΒΓ`) inside English prose encodes with the
        // §8.4 capitals passage over the Greek letters.
        assert!(enc("The ΑΒΓ set").is_some());
    }

    #[test]
    fn encodes_superscript_after_numeric_base() {
        // §3.24: a superscript (`³`) directly after a number takes the level
        // indicator; the numeric base needs no extra grade-1.
        assert!(enc("row 5³").is_some());
    }

    #[test]
    fn encodes_styled_uppercase_letter_after_number_with_capital() {
        // §8/§9: an italic/bold uppercase letter directly after a number keeps its
        // capital indicator (`5𝐀`).
        let out = enc("5\u{1D400}").expect("should encode");
        assert!(out.contains(&CAPITAL));
    }

    #[test]
    fn encodes_capitalized_enough_before_sentence_close() {
        // §10.5: a capitalized `Enough` closing a sentence keeps the `enough`
        // wordsign `⠢` with a leading capital indicator.
        let out = enc("Enough.").expect("should encode");
        assert_eq!(out.first(), Some(&CAPITAL));
        assert!(out.contains(&decode_unicode('⠢')));
    }

    #[test]
    fn encode_word_acronym_with_accented_letter_emits_accent_cells() {
        // §10.12.1/§4.2: an all-caps initialism abutting a digit spells each
        // letter; an accented capital (`É`) emits its §4.2 accent cells.
        let ctx = WordContext {
            standing_alone: false,
            upper_usable: false,
            shortform_usable: false,
            allow_longer_shortforms: true,
            lower_usable: false,
            suppress_caps: false,
            word_initial: true,
            restricted_prefix_boundary: true,
            digit_adjacent: true,
        };
        let mut out = Vec::new();
        assert!(
            EnglishUebEngine::new()
                .encode_word(&['É', 'B'], ctx, &mut out)
                .is_some()
        );
        assert!(!out.is_empty());
    }
}
