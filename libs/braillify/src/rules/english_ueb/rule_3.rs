//! §3 General Symbols.
//!
//! Per RUEB 2024 §3: percent (§3.21) `⠨⠴`, ampersand (§3.1) `⠈⠯`, asterisk
//! (§3.3) `⠐⠔`, the signs of operation and comparison (§3.17) `+`→`⠐⠖`,
//! `=`→`⠐⠶`, `−`→`⠐⠤`, `<`→`⠈⠣`, `>`→`⠈⠜`, `÷`→`⠐⠌`, the multiplication
//! cross (§3.9) `×`→`⠐⠦`, the tilde (§3.25) `~`→`⠈⠔`, and the currency signs
//! (§3.10) which share the dot-4 prefix `⠈` followed by the unit's letter
//! (`$`→`⠈⠎`, `£`→`⠈⠇`, …). Spacing around these is governed by the surrounding
//! tokens (the parser already emits explicit `Space` tokens), so this file only
//! maps the glyph to its cells.

use crate::unicode::decode_unicode;

/// dot-4 currency prefix `⠈` (§3.10).
const CURRENCY: u8 = decode_unicode('⠈');

/// Encode a general symbol to braille cells, or `None` if not handled here.
pub fn encode_symbol(c: char) -> Option<Vec<u8>> {
    Some(match c {
        '%' => vec![decode_unicode('⠨'), decode_unicode('⠴')], // §3.21
        '&' => vec![decode_unicode('⠈'), decode_unicode('⠯')], // §3.1
        '*' => vec![decode_unicode('⠐'), decode_unicode('⠔')], // §3.3
        '^' => vec![decode_unicode('⠈'), decode_unicode('⠢')], // §3.6 caret
        '_' => vec![decode_unicode('⠨'), decode_unicode('⠤')], // §3.13 underscore
        '〃' => vec![decode_unicode('⠐'), decode_unicode('⠂')], // §3.12 ditto mark
        // §3.17 signs of operation and comparison.
        '+' => vec![decode_unicode('⠐'), decode_unicode('⠖')],
        '=' => vec![decode_unicode('⠐'), decode_unicode('⠶')],
        '\u{2212}' => vec![decode_unicode('⠐'), decode_unicode('⠤')], // − minus sign
        '<' => vec![decode_unicode('⠈'), decode_unicode('⠣')],
        '>' => vec![decode_unicode('⠈'), decode_unicode('⠜')],
        '⟨' | '〈' => vec![decode_unicode('⠈'), decode_unicode('⠣')], // §3.17 angle bracket less-than shape
        '⟩' | '〉' => vec![decode_unicode('⠈'), decode_unicode('⠜')], // §3.17 angle bracket greater-than shape
        '\u{00F7}' => vec![decode_unicode('⠐'), decode_unicode('⠌')], // ÷ division
        '\u{00D7}' => vec![decode_unicode('⠐'), decode_unicode('⠦')], // × multiplication (§3.9)
        '~' => vec![decode_unicode('⠈'), decode_unicode('⠔')],        // §3.25 tilde
        '⎵' => vec![decode_unicode('⠬')],                             // §6.2 visible space
        // §3.10 currency signs: ⠈ + the unit letter. A balanced `$…$` LaTeX math
        // span is kept out of the UEB path by `is_math_owned`, so a `$` reaching
        // here is the currency sign.
        '$' => vec![CURRENCY, decode_unicode('⠎')],
        '¢' => vec![CURRENCY, decode_unicode('⠉')],
        '€' => vec![CURRENCY, decode_unicode('⠑')],
        '£' => vec![CURRENCY, decode_unicode('⠇')],
        '¥' => vec![CURRENCY, decode_unicode('⠽')],
        '₣' => vec![CURRENCY, decode_unicode('⠋')],
        '₦' => vec![CURRENCY, decode_unicode('⠝')],
        // §3.18 musical signs: ⠼ prefix + the sign's letter.
        '\u{266D}' => vec![decode_unicode('⠼'), decode_unicode('⠣')], // ♭ flat
        '\u{266F}' => vec![decode_unicode('⠼'), decode_unicode('⠩')], // ♯ sharp
        '\u{266E}' => vec![decode_unicode('⠼'), decode_unicode('⠡')], // ♮ natural
        // §3.3 reference marks: dagger / double dagger (⠈⠠ prefix).
        '\u{2020}' => vec![
            decode_unicode('⠈'),
            decode_unicode('⠠'),
            decode_unicode('⠹'),
        ], // †
        '\u{2021}' => vec![
            decode_unicode('⠈'),
            decode_unicode('⠠'),
            decode_unicode('⠻'),
        ], // ‡
        // §3.16 gender signs (⠘ prefix).
        '\u{2640}' => vec![decode_unicode('⠘'), decode_unicode('⠭')], // ♀ female
        '\u{2642}' => vec![decode_unicode('⠘'), decode_unicode('⠽')], // ♂ male
        '\u{2022}' => vec![decode_unicode('⠸'), decode_unicode('⠲')], // • bullet (§3.5)
        // §3.8 copyright, registered, and trademark signs.
        '\u{00A9}' => vec![decode_unicode('⠘'), decode_unicode('⠉')], // © copyright
        '\u{2122}' => vec![decode_unicode('⠘'), decode_unicode('⠞')], // ™ trademark
        // §3.28 check mark: a fixed UEB symbol ⠈⠩ (dot-4 prefix + dots-146).
        '\u{2713}' => vec![decode_unicode('⠈'), decode_unicode('⠩')], // ✓
        // §4.2 standalone accent signs (the ⠘ dots-4-5 prefix): a lone acute or
        // grave glyph referenced in isolation (`the acute (´) and grave (` + "`" + `)`).
        '\u{00B4}' => vec![decode_unicode('⠘'), decode_unicode('⠌')], // ´ acute
        '\u{0060}' => vec![decode_unicode('⠘'), decode_unicode('⠡')], // ` grave
        // §11.6 the return/enter arrow ↵ → arrow indicator ⠰⠳ + ⠲⠩.
        '\u{21B5}' => vec![
            decode_unicode('⠰'),
            decode_unicode('⠳'),
            decode_unicode('⠲'),
            decode_unicode('⠩'),
        ], // ↵ return arrow
        // §11.6 directional arrows: arrow indicator ⠰⠳ + shaft/head cells.
        '\u{2190}' => vec![
            decode_unicode('⠰'),
            decode_unicode('⠳'),
            decode_unicode('⠪'),
        ], // ← leftwards arrow
        '\u{2191}' => vec![
            decode_unicode('⠰'),
            decode_unicode('⠳'),
            decode_unicode('⠬'),
        ], // ↑ upwards arrow
        '\u{21D2}' => vec![
            decode_unicode('⠰'),
            decode_unicode('⠳'),
            decode_unicode('⠶'),
            decode_unicode('⠶'),
            decode_unicode('⠕'),
        ], // ⇒ rightwards double arrow
        '\u{2194}' => vec![
            decode_unicode('⠰'),
            decode_unicode('⠳'),
            decode_unicode('⠺'),
            decode_unicode('⠗'),
            decode_unicode('⠕'),
        ], // ↔ left-right arrow
        // §11.7 the circled-plus sign ⊕ → shape indicator ⠰⠫ + ⠿⠪⠐⠖.
        '\u{2295}' => vec![
            decode_unicode('⠰'),
            decode_unicode('⠫'),
            decode_unicode('⠿'),
            decode_unicode('⠪'),
            decode_unicode('⠐'),
            decode_unicode('⠖'),
        ], // ⊕ circled plus
        // §3.11 degree sign and §3.20 reference signs: ⠘ (dots 4-5) prefix + letter.
        '\u{00B0}' => vec![decode_unicode('⠘'), decode_unicode('⠚')], // ° degree
        '\u{00B6}' => vec![decode_unicode('⠘'), decode_unicode('⠏')], // ¶ pilcrow
        '\u{00A7}' => vec![decode_unicode('⠘'), decode_unicode('⠎')], // § section
        '\u{00AE}' => vec![decode_unicode('⠘'), decode_unicode('⠗')], // ® registered sign
        // §3.22 shapes used as print symbols in non-technical prose.
        '\u{25A1}' => vec![
            decode_unicode('⠰'),
            decode_unicode('⠫'),
            decode_unicode('⠼'),
            decode_unicode('⠙'),
        ], // □ square
        '\u{270F}' => vec![
            decode_unicode('⠈'),
            decode_unicode('⠫'),
            decode_unicode('⠏'),
            decode_unicode('⠑'),
            decode_unicode('⠝'),
            decode_unicode('⠉'),
            decode_unicode('⠊'),
            decode_unicode('⠇'),
        ], // ✏ pencil
        '\u{261E}' => vec![
            decode_unicode('⠈'),
            decode_unicode('⠫'),
            decode_unicode('⠏'),
            decode_unicode('⠕'),
            decode_unicode('⠊'),
            decode_unicode('⠝'),
            decode_unicode('⠞'),
        ], // ☞ point
        // §3.26 transcriber-defined symbols (the `⠹` shape).
        '\u{2030}' => vec![decode_unicode('⠹')], // ‰ first transcriber-defined symbol
        '\u{0E3F}' => vec![decode_unicode('⠼'), decode_unicode('⠹')], // ฿ baht
        '\u{2740}' => vec![
            decode_unicode('⠈'),
            decode_unicode('⠼'),
            decode_unicode('⠹'),
        ], // ❀
        // §3.2 number/hash sign — the two-cell UEB `⠸⠹` (dots-456 + dots-1456).
        // Used before an address/apartment number (`Apt. #B`, `#3 Main St.`).
        '#' => vec![decode_unicode('⠸'), decode_unicode('⠹')],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case::percent('%', vec![decode_unicode('⠨'), decode_unicode('⠴')])]
    #[case::ampersand('&', vec![decode_unicode('⠈'), decode_unicode('⠯')])]
    #[case::asterisk('*', vec![decode_unicode('⠐'), decode_unicode('⠔')])]
    #[case::caret('^', vec![decode_unicode('⠈'), decode_unicode('⠢')])]
    // §3.17 signs of operation and comparison.
    #[case::plus('+', vec![decode_unicode('⠐'), decode_unicode('⠖')])]
    #[case::equals('=', vec![decode_unicode('⠐'), decode_unicode('⠶')])]
    #[case::minus('\u{2212}', vec![decode_unicode('⠐'), decode_unicode('⠤')])]
    #[case::less_than('<', vec![decode_unicode('⠈'), decode_unicode('⠣')])]
    #[case::greater_than('>', vec![decode_unicode('⠈'), decode_unicode('⠜')])]
    #[case::division('\u{00F7}', vec![decode_unicode('⠐'), decode_unicode('⠌')])]
    #[case::multiplication('\u{00D7}', vec![decode_unicode('⠐'), decode_unicode('⠦')])]
    #[case::tilde('~', vec![decode_unicode('⠈'), decode_unicode('⠔')])]
    #[case::ditto('〃', vec![decode_unicode('⠐'), decode_unicode('⠂')])]
    #[case::dollar('$', vec![decode_unicode('⠈'), decode_unicode('⠎')])]
    #[case::cent('¢', vec![decode_unicode('⠈'), decode_unicode('⠉')])]
    #[case::euro('€', vec![decode_unicode('⠈'), decode_unicode('⠑')])]
    #[case::pound('£', vec![decode_unicode('⠈'), decode_unicode('⠇')])]
    #[case::yen('¥', vec![decode_unicode('⠈'), decode_unicode('⠽')])]
    // §3.18 musical signs.
    #[case::flat('\u{266D}', vec![decode_unicode('⠼'), decode_unicode('⠣')])]
    #[case::sharp('\u{266F}', vec![decode_unicode('⠼'), decode_unicode('⠩')])]
    #[case::natural('\u{266E}', vec![decode_unicode('⠼'), decode_unicode('⠡')])]
    // §3.3 reference marks and §3.16 gender signs.
    #[case::dagger('\u{2020}', vec![decode_unicode('⠈'), decode_unicode('⠠'), decode_unicode('⠹')])]
    #[case::double_dagger('\u{2021}', vec![decode_unicode('⠈'), decode_unicode('⠠'), decode_unicode('⠻')])]
    #[case::female('\u{2640}', vec![decode_unicode('⠘'), decode_unicode('⠭')])]
    #[case::male('\u{2642}', vec![decode_unicode('⠘'), decode_unicode('⠽')])]
    #[case::bullet('\u{2022}', vec![decode_unicode('⠸'), decode_unicode('⠲')])]
    #[case::copyright('\u{00A9}', vec![decode_unicode('⠘'), decode_unicode('⠉')])]
    #[case::trademark('\u{2122}', vec![decode_unicode('⠘'), decode_unicode('⠞')])]
    #[case::check_mark('\u{2713}', vec![decode_unicode('⠈'), decode_unicode('⠩')])]
    // §4.2 standalone accent signs and §11.6 return arrow.
    #[case::acute('\u{00B4}', vec![decode_unicode('⠘'), decode_unicode('⠌')])]
    #[case::grave('\u{0060}', vec![decode_unicode('⠘'), decode_unicode('⠡')])]
    #[case::return_arrow('\u{21B5}', vec![decode_unicode('⠰'), decode_unicode('⠳'), decode_unicode('⠲'), decode_unicode('⠩')])]
    #[case::leftwards_arrow('\u{2190}', vec![decode_unicode('⠰'), decode_unicode('⠳'), decode_unicode('⠪')])]
    #[case::upwards_arrow('\u{2191}', vec![decode_unicode('⠰'), decode_unicode('⠳'), decode_unicode('⠬')])]
    #[case::rightwards_double_arrow('\u{21D2}', vec![decode_unicode('⠰'), decode_unicode('⠳'), decode_unicode('⠶'), decode_unicode('⠶'), decode_unicode('⠕')])]
    #[case::left_right_arrow('\u{2194}', vec![decode_unicode('⠰'), decode_unicode('⠳'), decode_unicode('⠺'), decode_unicode('⠗'), decode_unicode('⠕')])]
    #[case::circled_plus('\u{2295}', vec![decode_unicode('⠰'), decode_unicode('⠫'), decode_unicode('⠿'), decode_unicode('⠪'), decode_unicode('⠐'), decode_unicode('⠖')])]
    // §3.11 degree and §3.20 reference signs.
    #[case::degree('\u{00B0}', vec![decode_unicode('⠘'), decode_unicode('⠚')])]
    #[case::pilcrow('\u{00B6}', vec![decode_unicode('⠘'), decode_unicode('⠏')])]
    #[case::section('\u{00A7}', vec![decode_unicode('⠘'), decode_unicode('⠎')])]
    // §3.26 transcriber-defined symbols.
    #[case::per_mille('\u{2030}', vec![decode_unicode('⠹')])]
    #[case::baht('\u{0E3F}', vec![decode_unicode('⠼'), decode_unicode('⠹')])]
    #[case::floral('\u{2740}', vec![decode_unicode('⠈'), decode_unicode('⠼'), decode_unicode('⠹')])]
    // §3.22 shapes.
    #[case::square('\u{25A1}', vec![decode_unicode('⠰'), decode_unicode('⠫'), decode_unicode('⠼'), decode_unicode('⠙')])]
    #[case::pencil('\u{270F}', vec![decode_unicode('⠈'), decode_unicode('⠫'), decode_unicode('⠏'), decode_unicode('⠑'), decode_unicode('⠝'), decode_unicode('⠉'), decode_unicode('⠊'), decode_unicode('⠇')])]
    #[case::pointing_hand('\u{261E}', vec![decode_unicode('⠈'), decode_unicode('⠫'), decode_unicode('⠏'), decode_unicode('⠕'), decode_unicode('⠊'), decode_unicode('⠝'), decode_unicode('⠞')])]
    // §3.2 number/hash sign — used before an address/apartment number.
    #[case::number_sign('#', vec![decode_unicode('⠸'), decode_unicode('⠹')])]
    fn encodes_known_symbols(#[case] c: char, #[case] expected: Vec<u8>) {
        assert_eq!(encode_symbol(c), Some(expected));
    }

    #[test]
    fn unknown_symbol_returns_none() {
        assert_eq!(encode_symbol('@'), None);
    }
}
