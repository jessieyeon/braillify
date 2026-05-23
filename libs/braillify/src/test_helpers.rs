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
    pub prev_word: String,
    pub remaining_words: Vec<String>,
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
            prev_word: String::new(),
            remaining_words: Vec::new(),
        }
    }

    /// Builder: set the `prev_word` field that the borrowed `RuleContext` exposes.
    pub(crate) fn with_prev_word(mut self, prev_word: impl Into<String>) -> Self {
        self.prev_word = prev_word.into();
        self
    }

    /// Builder: set the `remaining_words` field that the borrowed `RuleContext`
    /// exposes. Stores owned strings so the borrowed context can outlive call sites.
    pub(crate) fn with_remaining_words<I, S>(mut self, words: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.remaining_words = words.into_iter().map(Into::into).collect();
        self
    }

    /// Borrow a `RuleContext` at the given index. The borrow is exclusive
    /// against `self`, so call this once per rule invocation.
    pub(crate) fn ctx_at<'a>(&'a mut self, index: usize) -> RuleContext<'a> {
        // Build a transient Vec<&str> view over the owned strings. The view's
        // lifetime is tied to `self` because each &str borrows from an entry
        // in `self.remaining_words`. We can't store the Vec<&str> in `self`
        // (self-referential), so we leak the indirection through the caller's
        // borrow: the returned RuleContext borrows it via the slice below.
        let remaining: Vec<&str> = self.remaining_words.iter().map(String::as_str).collect();
        // SAFETY: We need to give `RuleContext` a `&[&str]` whose lifetime
        // matches `self`. Leaking the Vec lets us produce that slice while
        // keeping the owned strings alive for the duration of `self`.
        let leaked: &'a [&'a str] = Box::leak(remaining.into_boxed_slice());
        RuleContext {
            word_chars: &self.word_chars,
            index,
            char_type: &self.char_types[index],
            prev_word: &self.prev_word,
            remaining_words: leaked,
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
