//! Shared context and state for rule execution.
//!
//! `RuleContext` provides the current encoding position and read access to input.
//! `EncoderState` tracks persistent state across characters/words (English mode, etc.).

use crate::char_struct::{CharType, KoreanChar};

/// The encoding context determines how ambiguous characters are interpreted.
/// For example, `·` is a tone mark in MiddleKorean mode but a middle dot in Korean mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingMode {
    /// Default Korean braille encoding
    Korean,
    /// English/Roman letter section (between ⠴ and ⠲)
    English,
    /// Math expression encoding
    Math,
    /// Middle Korean (중세국어) — archaic characters with special rules
    MiddleKorean,
}

/// Persistent state that survives across characters and words.
///
/// Tracks modal state like "are we currently in English mode?"
/// Rules can read and mutate this state.
#[derive(Debug, Clone)]
pub struct EncoderState {
    /// Stack of encoding modes. The top determines current context.
    pub mode_stack: Vec<EncodingMode>,
    /// Currently inside a Roman letter section (between ⠴ and ⠲)
    pub is_english: bool,
    /// Whether the input contains Korean (determines if Roman indicators are needed)
    pub english_indicator: bool,
    /// Currently in a triple-uppercase passage (⠠⠠⠠ ... ⠠⠄)
    pub triple_big_english: bool,
    /// Whether at least one word has been processed
    pub has_processed_word: bool,
    /// Need to emit English continuation marker (⠐) on next English char
    pub needs_english_continuation: bool,
    /// Rule 35 chain: English followed by digits may resume English without indicators
    pub roman_number_chain: bool,
    /// Stack tracking whether parentheses were opened in English context
    pub parenthesis_stack: Vec<bool>,
    /// Currently in a number sequence (수표 already emitted)
    pub is_number: bool,
    /// Currently in a consecutive uppercase run within a word
    pub is_big_english: bool,
}

impl EncoderState {
    pub fn new(english_indicator: bool) -> Self {
        Self {
            mode_stack: vec![EncodingMode::Korean],
            english_indicator,
            is_english: false,
            triple_big_english: false,
            has_processed_word: false,
            needs_english_continuation: false,
            roman_number_chain: false,
            parenthesis_stack: Vec::new(),
            is_number: false,
            is_big_english: false,
        }
    }

    /// Get the current encoding mode (top of stack, default Korean).
    pub fn current_mode(&self) -> EncodingMode {
        self.mode_stack
            .last()
            .copied()
            .unwrap_or(EncodingMode::Korean)
    }

    /// Push a new encoding mode onto the stack.
    pub fn push_mode(&mut self, mode: EncodingMode) {
        self.mode_stack.push(mode);
    }

    /// Pop the current encoding mode, returning to the previous one.
    pub fn pop_mode(&mut self) -> Option<EncodingMode> {
        if self.mode_stack.len() > 1 {
            self.mode_stack.pop()
        } else {
            None
        }
    }
}

/// Snapshot of the current encoding position within a word.
///
/// This is the "view" that each rule receives. Rules read this to decide
/// whether they match, then mutate `result` and `state` via `RuleContext`.
pub struct RuleContext<'a> {
    /// All characters in the current word
    pub word_chars: &'a [char],
    /// Current character index within the word
    pub index: usize,
    /// The classified type of the current character
    pub char_type: &'a CharType,
    /// Previous word (for cross-word context)
    pub prev_word: &'a str,
    /// Remaining words after this one
    pub remaining_words: &'a [&'a str],
    /// Whether this word contains any Korean syllable characters
    pub has_korean_char: bool,
    /// Whether the whole word is uppercase ASCII
    pub is_all_uppercase: bool,
    /// Whether ASCII letters start at index 0
    pub ascii_starts_at_beginning: bool,
    /// Skip count — rules can set this to skip subsequent characters
    pub skip_count: &'a mut usize,
    /// Shared mutable encoder state
    pub state: &'a mut EncoderState,
    /// Output buffer
    pub result: &'a mut Vec<u8>,
}

impl<'a> RuleContext<'a> {
    /// Current character.
    pub fn current_char(&self) -> char {
        self.word_chars[self.index]
    }

    /// Next character in the word, if any.
    pub fn next_char(&self) -> Option<char> {
        self.word_chars.get(self.index + 1).copied()
    }

    /// Previous character in the word, if any.
    pub fn prev_char(&self) -> Option<char> {
        if self.index > 0 {
            Some(self.word_chars[self.index - 1])
        } else {
            None
        }
    }

    /// Word length.
    pub fn word_len(&self) -> usize {
        self.word_chars.len()
    }

    /// Get the current KoreanChar if the char_type is Korean.
    pub fn as_korean(&self) -> Option<&KoreanChar> {
        if let CharType::Korean(k) = self.char_type {
            Some(k)
        } else {
            None
        }
    }

    /// Emit braille cell(s) to the output buffer.
    pub fn emit(&mut self, byte: u8) {
        self.result.push(byte);
    }

    /// Emit a slice of braille cells.
    pub fn emit_slice(&mut self, bytes: &[u8]) {
        self.result.extend_from_slice(bytes);
    }
}
