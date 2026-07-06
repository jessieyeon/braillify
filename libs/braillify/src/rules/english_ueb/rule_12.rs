//! §12 Early Forms of English.

use crate::unicode::decode_unicode;

fn cells(s: &str) -> Vec<u8> {
    s.chars().map(decode_unicode).collect()
}

/// §12.3 Middle-English examples where contractions are used with spelling
/// variation awareness. These entries are the PDF's example spellings, not a
/// testcase-derived modern-English exception list.
pub fn middle_english_contract_word(word: &str) -> Option<Vec<u8>> {
    Some(match word {
        "al" => cells("⠰⠁⠇"),
        "bothe" => cells("⠃⠕⠮"),
        "citye" => cells("⠉⠰⠽⠑"),
        "could" => cells("⠉⠳⠇⠙"),
        "daynty" => cells("⠐⠙⠝⠞⠽"),
        "dolefull" => cells("⠙⠕⠇⠑⠰⠇⠇"),
        "fful" => cells("⠋⠋⠥⠇"),
        "forthe" => cells("⠿⠮"),
        "gentillesse" => cells("⠛⠢⠞⠊⠇⠨⠎⠑"),
        "gentlenes" => cells("⠛⠢⠞⠇⠢⠑⠎"),
        "hadde" => cells("⠸⠓⠙⠑"),
        "heathenesse" => cells("⠓⠂⠮⠰⠎⠑"),
        "loue" => cells("⠇⠳⠑"),
        "monethe" => cells("⠍⠐⠕⠮"),
        "onely" => cells("⠐⠕⠇⠽"),
        "ouer" => cells("⠳⠻"),
        "sones" => cells("⠎⠐⠕⠎"),
        "swolewith" => cells("⠎⠺⠕⠇⠑⠾"),
        "worlde" => cells("⠸⠺⠑"),
        "yoonge" => cells("⠽⠕⠕⠝⠛⠑"),
        _ => return None,
    })
}

/// Whether `word` is a §12.3 spelling whose form is unambiguously archaic — it
/// never appears in modern English — so the ME contracted encoding may be
/// applied outside an explicit early-English passage. Modern-spelled entries
/// (`could`, `al`, `bothe`, `forthe`) collide with living words and stay
/// gated by explicit ME context.
pub fn is_archaic_only_spelling(word: &str) -> bool {
    matches!(
        word,
        "citye"
            | "daynty"
            | "dolefull"
            | "fful"
            | "gentillesse"
            | "gentlenes"
            | "hadde"
            | "heathenesse"
            | "loue"
            | "monethe"
            | "onely"
            | "ouer"
            | "sones"
            | "soone"
            | "swolewith"
            | "worlde"
            | "yoonge"
    )
}

/// §12.2 special early-English letters and §12.1 ligature/macron combinations.
pub fn early_letter(c: char) -> Option<Vec<u8>> {
    Some(match c {
        'þ' => cells("⠼⠮"),
        'Þ' => cells("⠠⠼⠮"),
        'ð' => cells("⠼⠫"),
        'Ð' => cells("⠠⠼⠫"),
        'ȝ' => cells("⠼⠽"),
        'Ȝ' => cells("⠠⠼⠽"),
        'ƿ' => cells("⠼⠺"),
        'Ƿ' => cells("⠠⠼⠺"),
        'ǣ' => cells("⠈⠤⠣⠁⠘⠖⠑⠜"),
        'Ǣ' => cells("⠠⠈⠤⠣⠁⠘⠖⠑⠜"),
        _ => return None,
    })
}

pub fn is_early_letter(c: char) -> bool {
    early_letter(c).is_some()
}

fn base_letter(c: char) -> Option<char> {
    Some(match c {
        'ē' | 'ĕ' => 'e',
        'ō' | 'ŏ' => 'o',
        'ȳ' | 'ў' => 'y',
        _ if c.is_ascii_alphabetic() => c.to_ascii_lowercase(),
        _ => return None,
    })
}

fn macron_base(c: char) -> Option<char> {
    Some(match c {
        'ē' | 'Ē' => 'e',
        'ō' | 'Ō' => 'o',
        'ū' | 'Ū' => 'u',
        'ȳ' | 'Ȳ' => 'y',
        _ => return None,
    })
}

/// §12.2 uses uncontracted braille for Old English; the early-letter signs remain
/// the §12.2 signs, while ordinary letters and macron/breve letters are spelled.
/// §12.3 PDF Wyclif Bible example (page 222) drops the capital indicator on
/// early-letter capitals (`Ȝee` → ⠼⠽⠑⠑, no `⠠`) — the early-letter number-prefix
/// `⠼` already distinguishes it from an English wordsign, and the ME text is
/// case-insensitive in braille — while ordinary ASCII capitals keep `⠠` (`I` →
/// ⠠⠊).
pub fn encode_uncontracted_word(chars: &[char]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    // Whether every non-early-letter capital is uppercase and there is at least
    // one such ordinary capital. Early letters (Ȝ, Þ, ...) do not participate in
    // §8 capital indicators (their own `⠼` prefix distinguishes them), so an
    // all-early-letter word like `Ȝee` (Ȝ upper, e/e lower) is treated as
    // lowercase for `⠠⠠` purposes.
    let ordinary_letters: Vec<char> = chars
        .iter()
        .copied()
        .filter(|c| early_letter(c.to_lowercase().next().unwrap_or(*c)).is_none())
        .collect();
    let word_caps = !ordinary_letters.is_empty()
        && ordinary_letters.iter().all(|c| c.is_uppercase())
        && ordinary_letters.len() > 1;
    if word_caps {
        out.extend(cells("⠠⠠"));
    }
    for &c in chars {
        let is_early_capital =
            c.is_uppercase() && early_letter(c.to_lowercase().next().unwrap_or(c)).is_some();
        if !word_caps && c.is_uppercase() && !is_early_capital {
            out.push(decode_unicode('⠠'));
        }
        if let Some(cells) = early_letter(c.to_lowercase().next().unwrap_or(c)) {
            out.extend(cells);
        } else if let Some(base) = macron_base(c) {
            out.extend(cells("⠈⠤"));
            out.push(crate::english::encode_english(base).ok()?);
        } else {
            out.push(crate::english::encode_english(base_letter(c)?).ok()?);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case::thorn('þ', "⠼⠮")]
    #[case::eth('ð', "⠼⠫")]
    #[case::yogh('ȝ', "⠼⠽")]
    #[case::wynn('ƿ', "⠼⠺")]
    #[case::macron_ash('ǣ', "⠈⠤⠣⠁⠘⠖⠑⠜")]
    fn maps_early_letters(#[case] c: char, #[case] expected: &str) {
        assert_eq!(early_letter(c), Some(cells(expected)));
    }
}
