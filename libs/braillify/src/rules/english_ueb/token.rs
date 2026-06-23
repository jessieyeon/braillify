//! Token model for the UEB Grade-2 encoder.
//!
//! A document is parsed into a flat token stream. The document engine walks
//! it, managing inter-word mode/indicators, while delegating intra-word
//! contraction to the `ContractionEngine`.

/// §9 typeform applied to a styled character (italic/bold/underline).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typeform {
    /// §9.x italic — symbol indicator `⠨⠆`.
    Italic,
    /// §9.x bold — symbol indicator `⠘⠆`.
    Bold,
    /// §9.x underline — symbol indicator `⠸⠆`.
    Underline,
}

/// A parsed unit of English source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnglishToken {
    /// A maximal run of ASCII letters (original case preserved).
    Word(Vec<char>),
    /// A maximal run of ASCII digits.
    Number(Vec<char>),
    /// A single non-letter, non-digit, non-space character.
    Symbol(char),
    /// One unit of inter-word whitespace.
    Space,
    /// §9: a single styled letter (its plain base char + typeform). It carries a
    /// symbol-level typeform indicator and acts as a contraction boundary, so the
    /// surrounding plain letters contract among themselves (`𝐛right` → bold-b then
    /// the `right` groupsign).
    Styled(char, Typeform),
}
