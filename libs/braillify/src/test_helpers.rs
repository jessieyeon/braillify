//! Shared test helpers (cfg(test) only).
//!
//! Provides reusable builders for `RuleContext` and friends so individual
//! rule tests don't have to repeat 10+ lines of field initialization.

#![cfg(test)]

use crate::char_struct::CharType;
use crate::rules::context::{EncoderState, RuleContext};

/// Borrowed snapshot used by `make_ctx`. Owns everything `RuleContext` needs
/// references to (chars, char_type, state, etc.) so the caller can hand out
/// a single mutable view.
pub(crate) struct CtxOwned {
    pub word_chars: Vec<char>,
    pub char_types: Vec<CharType>,
    pub skip_count: usize,
    pub state: EncoderState,
    pub result: Vec<u8>,
}

impl CtxOwned {
    /// Build a fresh owned context for `text`. Each char is classified via
    /// `CharType::new`. The `index` parameter is used by callers when
    /// constructing the actual `RuleContext` borrow.
    pub(crate) fn for_text(text: &str, english_indicator: bool) -> Self {
        let word_chars: Vec<char> = text.chars().collect();
        let char_types: Vec<CharType> = word_chars
            .iter()
            .map(|c| CharType::new(*c).expect("CharType::new should not fail in tests"))
            .collect();
        Self {
            word_chars,
            char_types,
            skip_count: 0,
            state: EncoderState::new(english_indicator),
            result: Vec::new(),
        }
    }

    /// Borrow a `RuleContext` at the given index. The borrow is exclusive
    /// against `self`, so call this once per rule invocation.
    pub(crate) fn ctx_at<'a>(&'a mut self, index: usize) -> RuleContext<'a> {
        RuleContext {
            word_chars: &self.word_chars,
            index,
            char_type: &self.char_types[index],
            prev_word: "",
            remaining_words: &[],
            has_korean_char: self.word_chars.iter().any(|c| {
                let cp = *c as u32;
                (0xAC00..=0xD7A3).contains(&cp)
            }),
            is_all_uppercase: false,
            ascii_starts_at_beginning: false,
            skip_count: &mut self.skip_count,
            state: &mut self.state,
            result: &mut self.result,
        }
    }
}
