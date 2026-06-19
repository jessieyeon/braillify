//! Token model for the UEB Grade-2 encoder.
//!
//! A document is parsed into a flat token stream. The document engine walks
//! it, managing inter-word mode/indicators, while delegating intra-word
//! contraction to the `ContractionEngine`.

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
}
