//! Intra-word contraction dispatch (UEB §10).
//!
//! Each [`ContractionRule`] offers a match at a position; the engine picks the
//! **longest** match, breaking ties by **lowest priority number** — this is the
//! §10.10 preference rule encoded structurally. Unmatched positions fall back to
//! single letters (§4.1 alphabet), realised via [`crate::english::encode_english`].

use crate::english::encode_english;

/// A contraction match starting at a word position.
pub struct ContractionMatch {
    /// Braille cells produced for the matched span.
    pub cells: Vec<u8>,
    /// Number of source characters consumed.
    pub consumed: usize,
    /// Tie-breaker among equal-length matches (lower wins).
    pub priority: u16,
}

/// One UEB contraction rule (§10.x). `word` is the lowercased letter slice.
pub trait ContractionRule: Send + Sync {
    /// Offer a match starting at `pos`, or `None`.
    fn try_match(&self, word: &[char], pos: usize) -> Option<ContractionMatch>;
}

/// Longest-prefix match of `word[pos..]` against a PHF map of `&str -> cell`.
pub fn match_longest(
    word: &[char],
    pos: usize,
    map: &phf::Map<&'static str, u8>,
    priority: u16,
) -> Option<ContractionMatch> {
    let mut best: Option<(usize, u8)> = None;
    for (key, &cell) in map.entries() {
        let klen = key.chars().count();
        if pos + klen <= word.len()
            && key
                .chars()
                .zip(&word[pos..pos + klen])
                .all(|(k, w)| k == *w)
            && best.is_none_or(|(bl, _)| klen > bl)
        {
            best = Some((klen, cell));
        }
    }
    best.map(|(klen, cell)| ContractionMatch {
        cells: vec![cell],
        consumed: klen,
        priority,
    })
}

/// Dispatches contraction rules over a single (lowercased) word.
#[derive(Default)]
pub struct ContractionEngine {
    rules: Vec<Box<dyn ContractionRule>>,
}

impl ContractionEngine {
    /// Register a contraction rule.
    pub fn register(&mut self, rule: Box<dyn ContractionRule>) {
        self.rules.push(rule);
    }

    /// Encode the best contraction at `pos`, or the single fallback letter.
    pub fn encode_at(&self, word: &[char], pos: usize) -> Option<(Vec<u8>, usize)> {
        self.best_match(word, pos)
            .map(|matched| (matched.cells, matched.consumed))
            .or_else(|| super::rule_4::accent_cells(word[pos]).map(|cells| (cells, 1)))
            .or_else(|| encode_english(word[pos]).ok().map(|cell| (vec![cell], 1)))
    }

    /// Encode a lowercased letter slice to braille cells.
    /// Returns `None` if a character cannot be encoded as an English letter.
    #[cfg(test)]
    pub fn encode_word(&self, word: &[char]) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(word.len());
        let mut pos = 0;
        while pos < word.len() {
            match self.best_match(word, pos) {
                Some(m) => {
                    out.extend(m.cells);
                    pos += m.consumed;
                }
                None => {
                    // §4.2: an accented letter becomes its accent indicator plus
                    // the base letter; otherwise a plain alphabet letter (§4.1).
                    if let Some(cells) = super::rule_4::accent_cells(word[pos]) {
                        out.extend(cells);
                    } else {
                        out.push(encode_english(word[pos]).ok()?);
                    }
                    pos += 1;
                }
            }
        }
        Some(out)
    }

    fn best_match(&self, word: &[char], pos: usize) -> Option<ContractionMatch> {
        let mut best: Option<ContractionMatch> = None;
        for rule in &self.rules {
            if let Some(m) = rule.try_match(word, pos) {
                let better = best.as_ref().is_none_or(|b| {
                    m.consumed > b.consumed || (m.consumed == b.consumed && m.priority < b.priority)
                });
                if better {
                    best = Some(m);
                }
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unicode::decode_unicode;

    #[test]
    fn plain_letters_fall_back_to_alphabet() {
        let eng = ContractionEngine::default();
        let cells = eng.encode_word(&['c', 'a', 't']).unwrap();
        assert_eq!(
            cells,
            vec![
                decode_unicode('⠉'),
                decode_unicode('⠁'),
                decode_unicode('⠞')
            ]
        );
    }
}
