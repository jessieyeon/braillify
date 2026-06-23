//! §9 Typeforms (italic, bold, underline, script).
//!
//! §9.x: emphasised text carries a typeform indicator. This module covers the
//! *symbol* level — a single styled letter takes a symbol indicator (`⠨⠆`
//! italic, `⠘⠆` bold, `⠸⠆` underline, `⠈⠆` script) before its base cell. The
//! style signal comes from Unicode: the Mathematical Alphanumeric Symbols block
//! (U+1D400–1D7FF) encodes bold/italic/… letters, and a combining low line
//! (U+0332) underlines its preceding letter. The parser turns each styled letter
//! into an [`EnglishToken::Styled`] that acts as a contraction boundary, so the
//! plain neighbours still contract (`𝐛right` → bold-`b` then the `right`
//! groupsign).
//!
//! NFC (not NFKC/NFKD) is used upstream so these styled code points survive — a
//! compatibility fold would collapse them back to plain ASCII and erase the
//! typeform.

use crate::unicode::decode_unicode;

use super::token::Typeform;

/// Decode a styled Latin letter to its plain base char and typeform, or `None`.
///
/// Handles the contiguous bold (U+1D400) and italic (U+1D434) letter blocks
/// (each laid out as `A`–`Z` then `a`–`z`) plus the one Letterlike-block gap
/// (`ℎ` U+210E = italic h, whose math-alphanumeric slot U+1D455 is unassigned).
pub fn decode_styled(c: char) -> Option<(char, Typeform)> {
    let cp = c as u32;
    if cp == 0x210E {
        return Some(('h', Typeform::Italic)); // ℎ fills the italic-h gap
    }
    for (base, form) in [(0x1D400u32, Typeform::Bold), (0x1D434u32, Typeform::Italic)] {
        if (base..base + 52).contains(&cp) {
            let off = (cp - base) as u8;
            let letter = if off < 26 {
                (b'A' + off) as char
            } else {
                (b'a' + off - 26) as char
            };
            return Some((letter, form));
        }
    }
    // Bold digits 𝟎–𝟗 (U+1D7CE–1D7D7) — the styled-digit block in the corpus.
    if (0x1D7CE..=0x1D7D7).contains(&cp) {
        return Some(((b'0' + (cp - 0x1D7CE) as u8) as char, Typeform::Bold));
    }
    None
}

/// The dot-4/5/6 prefix that selects a typeform (`⠨` italic, `⠘` bold, `⠸`
/// underline). The §9 indicators are this prefix plus a level cell.
fn prefix(form: Typeform) -> char {
    match form {
        Typeform::Italic => '⠨',
        Typeform::Bold => '⠘',
        Typeform::Underline => '⠸',
    }
}

/// The §9 *symbol* typeform indicator cells (prefix + `⠆`) for `form` — used
/// before a single styled letter (`𝑝neumonia` → `⠨⠆⠏…`).
pub fn symbol_indicator(form: Typeform) -> [u8; 2] {
    [decode_unicode(prefix(form)), decode_unicode('⠆')]
}

/// The §9.x *word* typeform indicator cells (prefix + `⠂`) for `form` — used
/// before a run of two or more styled letters (`𝑅𝑎𝑑𝑎𝑟` → `⠨⠂…`).
pub fn word_indicator(form: Typeform) -> [u8; 2] {
    [decode_unicode(prefix(form)), decode_unicode('⠂')]
}

/// The §9.x *passage* typeform indicator cells (prefix + `⠶`) for `form` — opens
/// a run of three or more styled words (`𝑂𝑙𝑖𝑣𝑒𝑟 𝑇𝑤𝑖𝑠𝑡, 𝐺𝑟𝑒𝑎𝑡 …` → `⠨⠶…⠨⠄`).
pub fn passage_indicator(form: Typeform) -> [u8; 2] {
    [decode_unicode(prefix(form)), decode_unicode('⠶')]
}

/// The §9.x typeform terminator cells (prefix + `⠄`) for `form` — closes a word
/// indicator when the emphasis ends mid-word (`𝐭𝐞𝐱𝐭book` → `⠘⠂⠞⠑⠭⠞⠘⠄⠃…`).
pub fn terminator(form: Typeform) -> [u8; 2] {
    [decode_unicode(prefix(form)), decode_unicode('⠄')]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case::bold_upper_a('\u{1D400}', 'A', Typeform::Bold)]
    #[case::bold_lower_b('\u{1D41B}', 'b', Typeform::Bold)]
    #[case::bold_lower_z('\u{1D433}', 'z', Typeform::Bold)]
    #[case::italic_upper_a('\u{1D434}', 'A', Typeform::Italic)]
    #[case::italic_lower_p('\u{1D45D}', 'p', Typeform::Italic)]
    #[case::italic_h_gap('\u{210E}', 'h', Typeform::Italic)]
    #[case::italic_lower_z('\u{1D467}', 'z', Typeform::Italic)]
    fn decodes_styled_letters(#[case] c: char, #[case] base: char, #[case] form: Typeform) {
        assert_eq!(decode_styled(c), Some((base, form)));
    }

    #[rstest::rstest]
    #[case::plain_ascii('a')]
    #[case::digit('5')]
    #[case::combining_low_line('\u{0332}')]
    fn plain_chars_are_not_styled(#[case] c: char) {
        assert_eq!(decode_styled(c), None);
    }

    #[rstest::rstest]
    #[case::italic(Typeform::Italic, '⠨')]
    #[case::bold(Typeform::Bold, '⠘')]
    #[case::underline(Typeform::Underline, '⠸')]
    fn symbol_indicator_uses_the_right_prefix(#[case] form: Typeform, #[case] prefix: char) {
        assert_eq!(
            symbol_indicator(form),
            [decode_unicode(prefix), decode_unicode('⠆')]
        );
    }
}
