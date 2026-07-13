use super::*;

/// §9.x: from the styled run at `i`, count consecutive same-`form` styled words
/// joined only by spaces/punctuation, and return that count with the passage end
/// (exclusive) — the last styled run plus any trailing punctuation (so `Cities.`
/// keeps its full stop inside the passage). A plain word or number ends the run.
///
/// A trailing *dash* is excluded: it separates the passage from following text
/// (e.g. an attribution `…𝑤𝑖𝑡.—Shakespeare`), so the terminator falls before the
/// dash (`…⠺⠊⠞⠲⠨⠄⠠⠤…`), not after it.
pub(super) fn styled_passage_extent(
    tokens: &[EnglishToken],
    i: usize,
    form: super::super::token::Typeform,
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
pub(super) fn styled_punctuation_bridge(
    tokens: &[EnglishToken],
    start: usize,
    form: super::super::token::Typeform,
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

pub(super) fn parenthesized_foreign_style_before(tokens: &[EnglishToken], close: usize) -> bool {
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
pub(super) fn trailing_comma_precedes_different_typeform(
    tokens: &[EnglishToken],
    end: usize,
    form: super::super::token::Typeform,
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
pub(super) fn styled_passage_all_caps(
    tokens: &[EnglishToken],
    start: usize,
    end: usize,
    form: super::super::token::Typeform,
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

pub(super) fn styled_passage_foreign_scope(
    tokens: &[EnglishToken],
    start: usize,
    end: usize,
    form: super::super::token::Typeform,
    foreign_code: bool,
    spanish_foreign: bool,
) -> Option<(super::super::rule_13::AccentCode, bool)> {
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
            return Some((super::super::rule_13::AccentCode::Ueb, spanish_foreign));
        }
        // §13.6.4: a whole typeform-marked foreign passage in instructional or
        // bilingual material keeps the typeform passage indicator, but its words
        // are uncontracted and accents use the relevant foreign-code cells.
        return Some((super::super::rule_13::AccentCode::Foreign, spanish_foreign));
    }

    // §13.6.4 whole-sentence bold-typeform trigger: a bold-italic passage
    // covering the whole sentence content and containing a foreign accented
    // letter (`𝐈𝐥 𝐲 𝐚 𝐝𝐞��𝐱 𝐜𝐫𝐞̀𝐜𝐡𝐞𝐬 𝐞𝐧 𝐯𝐢𝐥𝐥𝐞.`) uses foreign-code accents
    // (`⠮` for è, `⠿` for é) even though document-level `has_foreign_code_signal`
    // is false. The `no plain-Word tokens outside the passage` gate distinguishes
    // this from a §13.1.2 English narrative with an italic foreign phrase
    // (`Her pirouette was … fouetté en tournant …`) where UEB accents apply.
    let has_foreign_in_passage = words.iter().any(|w| {
        w.iter()
            .any(|c| super::super::rule_13::is_foreign_letter(*c))
    });
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
            word.len() >= 4 && !super::super::pronunciation::cmudict::is_recorded_word(&word)
        })
    {
        // §13.2.1: an entire quoted, typeform-marked foreign phrase is written
        // uncontracted even when only one word (`Prenons`) supplies dictionary
        // evidence; the surrounding quote/prose supplies the print context.
        return Some((super::super::rule_13::AccentCode::Ueb, spanish_foreign));
    }
    let has_plain_word_outside = tokens.iter().enumerate().any(|(i, t)| {
        matches!(t, EnglishToken::Word(chars) if chars.len() >= 2) && (i < start || i >= end)
    });
    if has_foreign_in_passage && !has_plain_word_outside && words.len() >= 2 {
        return Some((super::super::rule_13::AccentCode::Foreign, spanish_foreign));
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
        return Some((super::super::rule_13::AccentCode::Foreign, spanish_foreign));
    }

    let all_lowercase_words = words
        .iter()
        .all(|w| w.iter().all(|c| !c.is_alphabetic() || c.is_lowercase()));
    let has_long_unrecorded = words.iter().any(|w| {
        let word: String = w.iter().flat_map(|c| c.to_lowercase()).collect();
        word.len() >= 4 && !super::super::pronunciation::cmudict::is_recorded_word(&word)
    });
    if all_lowercase_words && has_long_unrecorded {
        return Some((super::super::rule_13::AccentCode::Ueb, spanish_foreign));
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
                super::super::rule_13::AccentCode::Foreign
            } else {
                super::super::rule_13::AccentCode::Ueb
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
            word.len() > 1 && !super::super::pronunciation::cmudict::is_recorded_word(&word)
        })
        .count();
    if unrecorded < 2 {
        return None;
    }

    Some((super::super::rule_13::AccentCode::Ueb, spanish_foreign))
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{cells, enc};
    use super::*;

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

    #[test]
    fn typeform_passage_terminates_before_closing_quote_after_comma() {
        let engine = EnglishUebEngine::new();
        let form = super::super::super::token::Typeform::Italic;
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
        let terminator = super::super::super::rule_9::terminator(form);

        assert!(
            encoded
                .windows(terminator.len() + 2)
                .any(|cells| cells.starts_with(&terminator)
                    && cells[terminator.len()] == decode_unicode('⠠')
                    && cells[terminator.len() + 1] == decode_unicode('⠶'))
        );
    }

    #[test]
    fn rare_foreign_scope_helpers_cover_remaining_decisions() {
        let italic = super::super::super::token::Typeform::Italic;
        let bold = super::super::super::token::Typeform::Bold;

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
            Some((super::super::super::rule_13::AccentCode::Ueb, true))
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
            Some((super::super::super::rule_13::AccentCode::Foreign, true))
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
            Some((super::super::super::rule_13::AccentCode::Foreign, false))
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
            Some((super::super::super::rule_13::AccentCode::Foreign, false))
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
            Some((super::super::super::rule_13::AccentCode::Ueb, false))
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
            Some((super::super::super::rule_13::AccentCode::Ueb, false))
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
    fn titlecase_foreign_phrase_in_prose_uses_ueb_accents() {
        let italic = super::super::super::token::Typeform::Italic;
        let tokens = [
            EnglishToken::Styled('C', italic),
            EnglishToken::Styled('a', italic),
            EnglishToken::Styled('f', italic),
            EnglishToken::Styled('é', italic),
            EnglishToken::Space,
            EnglishToken::Styled('N', italic),
            EnglishToken::Styled('a', italic),
            EnglishToken::Styled('d', italic),
            EnglishToken::Space,
            EnglishToken::Word(vec!['s', 'a', 'i', 'd']),
        ];

        assert_eq!(
            styled_passage_foreign_scope(&tokens, 0, 8, italic, false, false),
            Some((super::super::super::rule_13::AccentCode::Ueb, false))
        );
    }
}
