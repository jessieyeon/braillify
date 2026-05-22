pub struct HistoricalGlossEntry {
    pub symbol: char,
    pub reading_unicode: &'static str,
    pub symbol_unicode: &'static str,
}

pub const HISTORICAL_GLOSS_ENTRIES: &[HistoricalGlossEntry] = &[
    HistoricalGlossEntry {
        symbol: '刀',
        reading_unicode: "⠋⠂",
        symbol_unicode: "⠊⠥",
    },
    HistoricalGlossEntry {
        symbol: '舟',
        reading_unicode: "⠘⠗",
        symbol_unicode: "⠨⠍",
    },
    HistoricalGlossEntry {
        symbol: '石',
        reading_unicode: "⠊⠥⠂",
        symbol_unicode: "⠠⠹",
    },
    HistoricalGlossEntry {
        symbol: '雪',
        reading_unicode: "⠉⠛",
        symbol_unicode: "⠠⠞",
    },
];

pub fn encode_unicode_cells(unicode: &str) -> Vec<u8> {
    unicode
        .chars()
        .map(crate::unicode::decode_unicode)
        .collect()
}

pub fn gloss_entry(c: char) -> Option<&'static HistoricalGlossEntry> {
    HISTORICAL_GLOSS_ENTRIES
        .iter()
        .find(|entry| entry.symbol == c)
}
