//! §16.2 Horizontal line mode.
//!
//! RUEB 2024 §16.2: a horizontal "line" drawn with box-drawing characters opens
//! with the horizontal line mode indicator `⠐⠒` and is then built from segment,
//! corner and crossing cells. The indicator's `⠒` is itself a simple segment, so
//! a line that *starts* with a solid segment (`─`) folds that first segment into
//! the indicator (§16.2.1, page 230 examples). Each subsequent box character maps
//! to one cell:
//!
//! | print            | cell | meaning (§16.2)                          |
//! |------------------|------|------------------------------------------|
//! | `─` U+2500       | `⠒`  | simple (solid single) segment            |
//! | `═` U+2550       | `⠶`  | double segment                           |
//! | `≡` U+2261       | `⠿`  | triple segment                           |
//! | `┴` U+2534       | `⠚`  | corner with upward vertical              |
//! | `┐` U+2510       | `⠲`  | corner with downward vertical            |
//! | `┼` U+253C       | `⠺`  | crossing with vertical line              |
//! | `╲` U+2572       | `⠣`  | crossing with left-leaning diagonal      |
//! | `╱` U+2571       | `⠜`  | crossing with right-leaning diagonal     |
//!
//! Only a *run* of two or more adjacent box characters is line mode; an isolated
//! one (a lone `─` rule, a mathematical `≡`) is left to the symbol/legacy path so
//! the math meaning is preserved.

use crate::unicode::decode_unicode;

/// The §16.2 horizontal-line cell for a box-drawing character, or `None`.
pub fn line_segment(c: char) -> Option<u8> {
    Some(match c {
        '\u{2500}' => decode_unicode('⠒'), // ─ simple
        '\u{2550}' => decode_unicode('⠶'), // ═ double
        '\u{2261}' => decode_unicode('⠿'), // ≡ triple
        '\u{2534}' => decode_unicode('⠚'), // ┴ corner up
        '\u{2510}' => decode_unicode('⠲'), // ┐ corner down
        '\u{253C}' => decode_unicode('⠺'), // ┼ crossing vertical
        '\u{2572}' => decode_unicode('⠣'), // ╲ crossing left diagonal
        '\u{2571}' => decode_unicode('⠜'), // ╱ crossing right diagonal
        _ => return None,
    })
}

/// Whether `c` is a §16.2 horizontal box-drawing character.
pub fn is_line_char(c: char) -> bool {
    line_segment(c).is_some()
}

/// The solid single horizontal segment `─`, folded into the `⠐⠒` indicator.
pub const SIMPLE_SEGMENT: char = '\u{2500}';

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case::simple('\u{2500}', '⠒')]
    #[case::double('\u{2550}', '⠶')]
    #[case::triple('\u{2261}', '⠿')]
    #[case::corner_up('\u{2534}', '⠚')]
    #[case::corner_down('\u{2510}', '⠲')]
    #[case::crossing_vertical('\u{253C}', '⠺')]
    #[case::crossing_left('\u{2572}', '⠣')]
    #[case::crossing_right('\u{2571}', '⠜')]
    fn line_segments_map(#[case] c: char, #[case] cell: char) {
        assert_eq!(line_segment(c), Some(decode_unicode(cell)));
        assert!(is_line_char(c));
    }

    #[rstest::rstest]
    #[case::letter('a')]
    #[case::digit('5')]
    #[case::space(' ')]
    #[case::equals('=')]
    fn non_line_chars(#[case] c: char) {
        assert_eq!(line_segment(c), None);
        assert!(!is_line_char(c));
    }
}
