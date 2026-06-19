//! §4.2 Accented and modified letters.
//!
//! A letter bearing a diacritic is written as a two-cell accent indicator
//! (`⠘…` or `⠈…`) followed by the base letter (RUEB 2024 §4.2): `è` → `⠘⠡⠑`,
//! `é` → `⠘⠌⠑`, `û` → `⠘⠩⠥`. The base letter still belongs to the surrounding
//! word, so contractions elsewhere are unaffected (`Frühling` keeps its `ing`
//! groupsign).

use crate::english::encode_english;
use crate::unicode::decode_unicode;

const GRAVE: [u8; 2] = [decode_unicode('⠘'), decode_unicode('⠡')];
const ACUTE: [u8; 2] = [decode_unicode('⠘'), decode_unicode('⠌')];
const CIRCUMFLEX: [u8; 2] = [decode_unicode('⠘'), decode_unicode('⠩')];
const DIAERESIS: [u8; 2] = [decode_unicode('⠘'), decode_unicode('⠒')];
const CARON: [u8; 2] = [decode_unicode('⠘'), decode_unicode('⠬')];
const RING: [u8; 2] = [decode_unicode('⠘'), decode_unicode('⠫')];
const CEDILLA: [u8; 2] = [decode_unicode('⠘'), decode_unicode('⠯')];
const TILDE: [u8; 2] = [decode_unicode('⠘'), decode_unicode('⠻')];
const STROKE: [u8; 2] = [decode_unicode('⠈'), decode_unicode('⠡')];
const MACRON: [u8; 2] = [decode_unicode('⠈'), decode_unicode('⠤')];
const BREVE: [u8; 2] = [decode_unicode('⠈'), decode_unicode('⠬')];

/// Map an accented letter to (accent indicator cells, base ASCII letter).
fn accent_of(c: char) -> Option<([u8; 2], char)> {
    let m = match c {
        'à' => (GRAVE, 'a'),
        'è' => (GRAVE, 'e'),
        'ì' => (GRAVE, 'i'),
        'ò' => (GRAVE, 'o'),
        'ù' => (GRAVE, 'u'),
        'á' => (ACUTE, 'a'),
        'é' => (ACUTE, 'e'),
        'í' => (ACUTE, 'i'),
        'ó' => (ACUTE, 'o'),
        'ú' => (ACUTE, 'u'),
        'ý' => (ACUTE, 'y'),
        'â' => (CIRCUMFLEX, 'a'),
        'ê' => (CIRCUMFLEX, 'e'),
        'î' => (CIRCUMFLEX, 'i'),
        'ô' => (CIRCUMFLEX, 'o'),
        'û' => (CIRCUMFLEX, 'u'),
        'ä' => (DIAERESIS, 'a'),
        'ë' => (DIAERESIS, 'e'),
        'ï' => (DIAERESIS, 'i'),
        'ö' => (DIAERESIS, 'o'),
        'ü' => (DIAERESIS, 'u'),
        'ÿ' => (DIAERESIS, 'y'),
        'č' => (CARON, 'c'),
        'š' => (CARON, 's'),
        'ž' => (CARON, 'z'),
        'ě' => (CARON, 'e'),
        'ř' => (CARON, 'r'),
        'å' => (RING, 'a'),
        'ç' => (CEDILLA, 'c'),
        'ã' => (TILDE, 'a'),
        'ñ' => (TILDE, 'n'),
        'õ' => (TILDE, 'o'),
        'ø' => (STROKE, 'o'),
        'ā' => (MACRON, 'a'),
        'ē' => (MACRON, 'e'),
        'ī' => (MACRON, 'i'),
        'ō' => (MACRON, 'o'),
        'ū' => (MACRON, 'u'),
        'ă' => (BREVE, 'a'),
        'ĕ' => (BREVE, 'e'),
        'ĭ' => (BREVE, 'i'),
        'ŏ' => (BREVE, 'o'),
        'ŭ' => (BREVE, 'u'),
        _ => return None,
    };
    Some(m)
}

/// Whether `c` is a supported accented letter (so the parser keeps it in a word).
pub fn is_accented(c: char) -> bool {
    accent_of(c).is_some()
}

/// Braille cells for an accented letter — indicator + base. `None` if `c` is not
/// a supported accented letter.
pub fn accent_cells(c: char) -> Option<Vec<u8>> {
    let (indicator, base) = accent_of(c)?;
    let mut cells = indicator.to_vec();
    cells.push(encode_english(base).ok()?);
    Some(cells)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case::e_grave('è', "⠘⠡⠑")]
    #[case::e_acute('é', "⠘⠌⠑")]
    #[case::u_circumflex('û', "⠘⠩⠥")]
    #[case::u_diaeresis('ü', "⠘⠒⠥")]
    #[case::a_ring('å', "⠘⠫⠁")]
    #[case::c_cedilla('ç', "⠘⠯⠉")]
    #[case::a_tilde('ã', "⠘⠻⠁")]
    #[case::o_stroke('ø', "⠈⠡⠕")]
    fn accent_cells_match_indicator_plus_base(#[case] c: char, #[case] expected: &str) {
        let want: Vec<u8> = expected.chars().map(decode_unicode).collect();
        assert_eq!(accent_cells(c), Some(want));
    }

    #[test]
    fn plain_letter_is_not_accented() {
        assert!(!is_accented('e'));
        assert!(accent_cells('e').is_none());
    }
}
