use super::context::EncoderState;
use super::token::Token;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TokenPhase {
    Normalization = 0,
    FractionDetection = 1,
    WordShortcut = 2,
    ModeEntry = 3,
    UppercasePassage = 4,
    PostWord = 5,
}

pub enum TokenAction<'a> {
    Noop,
    Replace(Token<'a>),
    #[cfg(test)]
    InsertBefore(Vec<Token<'a>>),
    ReplaceMany(Vec<Token<'a>>),
    #[cfg(test)]
    Remove,
}

pub trait TokenRule: Send + Sync {
    fn phase(&self) -> TokenPhase;
    fn priority(&self) -> u16 {
        100
    }
    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        state: &mut EncoderState,
    ) -> Result<TokenAction<'a>, String>;
}
