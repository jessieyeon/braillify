//! MathTokenRule trait — plugin interface for math token encoding.
//!
//! Each rule handles specific math token patterns. The MathTokenEngine
//! runs rules in priority order, dispatching to the first matching rule.

use super::parser::MathToken;

/// Shared mutable state across math token encoding.
pub struct MathEncodeState {
    pub prev_was_number: bool,
    pub logic_context: bool,
}

impl MathEncodeState {
    pub fn new(logic_context: bool) -> Self {
        Self {
            prev_was_number: false,
            logic_context,
        }
    }
}

/// Result of applying a math token rule.
pub enum MathTokenResult {
    /// Rule consumed N tokens (advance index by N).
    Consumed(usize),
    /// Rule did not apply. Try next rule.
    Skip,
}

/// Plugin interface for math token encoding rules.
pub trait MathTokenRule: Send + Sync {
    /// Rule name for debugging.
    fn name(&self) -> &'static str;

    /// Priority (lower runs first). Default: 100.
    fn priority(&self) -> u16 {
        100
    }

    /// Fast check: does this rule handle the token at the given index?
    fn matches(&self, tokens: &[MathToken], index: usize, state: &MathEncodeState) -> bool;

    /// Encode the matched tokens. Returns how many tokens were consumed.
    fn apply(
        &self,
        tokens: &[MathToken],
        index: usize,
        result: &mut Vec<u8>,
        state: &mut MathEncodeState,
        engine: &MathTokenEngine,
    ) -> Result<MathTokenResult, String>;
}

/// Engine that dispatches math tokens to registered rules.
pub struct MathTokenEngine {
    rules: Vec<Box<dyn MathTokenRule>>,
}

impl MathTokenEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register(&mut self, rule: Box<dyn MathTokenRule>) {
        self.rules.push(rule);
    }

    /// Sort rules by priority (call once after all rules registered).
    pub fn finalize(&mut self) {
        self.rules.sort_by_key(|r| r.priority());
    }

    /// Encode a sequence of math tokens into braille bytes.
    pub fn encode_tokens(&self, tokens: &[MathToken], result: &mut Vec<u8>) -> Result<(), String> {
        let logic_context = Self::has_logic_symbol(tokens);
        let mut state = MathEncodeState::new(logic_context);
        let mut i = 0usize;

        while i < tokens.len() {
            let mut handled = false;
            for rule in &self.rules {
                let _ = rule.name();
                if rule.matches(tokens, i, &state) {
                    match rule.apply(tokens, i, result, &mut state, self)? {
                        MathTokenResult::Consumed(n) => {
                            i += n;
                            handled = true;
                            break;
                        }
                        MathTokenResult::Skip => continue,
                    }
                }
            }
            if !handled {
                return Err(format!(
                    "No rule matched token at index {}: {:?}",
                    i, tokens[i]
                ));
            }
        }
        Ok(())
    }

    fn has_logic_symbol(tokens: &[MathToken]) -> bool {
        tokens.iter().any(|token| {
            matches!(
                token,
                MathToken::MathSymbol(
                    '\u{00AC}'
                        | '\u{21D2}'
                        | '\u{2194}'
                        | '\u{21D4}'
                        | '\u{21C4}'
                        | '\u{2227}'
                        | '\u{2228}'
                        | '\u{22BB}'
                        | '\u{2193}'
                        | '\u{2191}'
                        | '\u{2200}'
                        | '\u{2203}'
                        | '\u{2204}'
                )
            )
        })
    }
}
