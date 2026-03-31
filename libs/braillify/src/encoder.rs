use std::borrow::Cow;

use crate::rules;
use crate::rules::token::{Token, WordMeta, WordToken};

pub struct Encoder {
    pub(crate) is_english: bool,
    triple_big_english: bool,
    english_indicator: bool,
    has_processed_word: bool,
    pub(crate) needs_english_continuation: bool,
    parenthesis_stack: Vec<bool>,
    rule_engine: rules::engine::RuleEngine,
    token_engine: rules::token_engine::TokenRuleEngine,
}

impl Encoder {
    pub fn new(english_indicator: bool) -> Self {
        let mut rule_engine = rules::engine::RuleEngine::new();

        // ── Preprocessing ────────────────────────────────
        rule_engine.register(Box::new(rules::korean::rule_53::Rule53));

        // ── WordShortcut ─────────────────────────────────
        rule_engine.register(Box::new(rules::korean::rule_18::Rule18));

        // ── ModeManagement ───────────────────────────────
        rule_engine.register(Box::new(rules::korean::rule_29::Rule29));

        // ── CoreEncoding ─────────────────────────────────
        rule_engine.register(Box::new(rules::korean::rule_44::Rule44));
        rule_engine.register(Box::new(rules::korean::rule_16::Rule16));
        rule_engine.register(Box::new(rules::korean::rule_14::Rule14));
        rule_engine.register(Box::new(rules::korean::rule_13::Rule13));
        rule_engine.register(Box::new(rules::korean::rule_korean::RuleKorean));
        rule_engine.register(Box::new(rules::korean::rule_28::Rule28));
        rule_engine.register(Box::new(rules::korean::rule_40::Rule40));
        rule_engine.register(Box::new(rules::korean::rule_8::Rule8));
        rule_engine.register(Box::new(rules::korean::rule_2::Rule2));
        rule_engine.register(Box::new(rules::korean::rule_1::Rule1));
        rule_engine.register(Box::new(rules::korean::rule_3::Rule3));
        rule_engine.register(Box::new(
            rules::korean::rule_english_symbol::RuleEnglishSymbol,
        ));
        rule_engine.register(Box::new(rules::korean::rule_61::Rule61));
        rule_engine.register(Box::new(rules::korean::rule_41::Rule41));
        rule_engine.register(Box::new(rules::korean::rule_56::Rule56));
        rule_engine.register(Box::new(rules::korean::rule_57::Rule57));
        rule_engine.register(Box::new(rules::korean::rule_58::Rule58));
        rule_engine.register(Box::new(rules::korean::rule_60::Rule60));
        rule_engine.register(Box::new(rules::korean::rule_64::Rule64));
        rule_engine.register(Box::new(rules::korean::rule_65::Rule65));
        rule_engine.register(Box::new(rules::korean::rule_49::Rule49));
        rule_engine.register(Box::new(rules::korean::rule_space::RuleSpace));
        rule_engine.register(Box::new(rules::korean::rule_math::RuleMath));
        rule_engine.register(Box::new(rules::korean::rule_fraction::RuleFraction));

        // ── InterCharacter ───────────────────────────────
        rule_engine.register(Box::new(rules::korean::rule_11::Rule11));
        rule_engine.register(Box::new(rules::korean::rule_12::Rule12));

        let mut token_engine = rules::token_engine::TokenRuleEngine::new();
        token_engine.register(Box::new(
            rules::token_rules::solvable_case_override::SolvableCaseOverrideRule,
        ));
        token_engine.register(Box::new(rules::token_rules::normalize::NormalizeEllipsis));
        token_engine.register(Box::new(rules::token_rules::latex_math::LatexMergeRule));
        token_engine.register(Box::new(
            rules::token_rules::emphasis_ring::EmphasisRingRule,
        ));
        token_engine.register(Box::new(
            rules::token_rules::math_expression::MathExpressionTokenRule,
        ));
        token_engine.register(Box::new(
            rules::token_rules::latex_fraction::LatexFractionRule,
        ));
        token_engine.register(Box::new(rules::token_rules::latex_math::LatexMathRule));
        token_engine.register(Box::new(
            rules::token_rules::inline_fraction::InlineFractionRule,
        ));
        token_engine.register(Box::new(
            rules::token_rules::word_shortcut::WordShortcutRule,
        ));
        token_engine.register(Box::new(
            rules::token_rules::uppercase_passage::UppercasePassageRule,
        ));
        token_engine.register(Box::new(
            rules::token_rules::middle_dot_spacing::MiddleDotSpacingRule,
        ));
        token_engine.register(Box::new(
            rules::token_rules::quote_attachment::QuoteAttachmentRule,
        ));
        token_engine.register(Box::new(rules::token_rules::spacing::AsteriskSpacingRule));

        Self {
            english_indicator,
            is_english: false,
            triple_big_english: false,
            has_processed_word: false,
            needs_english_continuation: false,
            parenthesis_stack: Vec::new(),
            rule_engine,
            token_engine,
        }
    }

    fn encode_via_ir(&mut self, text: &str, result: &mut Vec<u8>) -> Result<(), String> {
        self.encode_via_ir_with_transform(text, result, |_, _| Ok(()))
    }

    fn encode_via_ir_with_transform<F>(
        &mut self,
        text: &str,
        result: &mut Vec<u8>,
        transform: F,
    ) -> Result<(), String>
    where
        F: FnOnce(&str, &mut Vec<Token<'_>>) -> Result<(), String>,
    {
        let mut ir = rules::token::DocumentIR::parse(text, self.english_indicator);
        let state_before_token_rules = ir.state.clone();
        self.token_engine.apply_all(&mut ir.tokens, &mut ir.state)?;
        ir.state = state_before_token_rules;
        transform(text, &mut ir.tokens)?;

        let output = rules::emit::emit(&mut ir, &mut self.rule_engine)?;
        result.extend(output);

        self.is_english = ir.state.is_english;
        self.triple_big_english = ir.state.triple_big_english;
        self.has_processed_word = ir.state.has_processed_word;
        self.needs_english_continuation = ir.state.needs_english_continuation;
        self.parenthesis_stack = ir.state.parenthesis_stack;
        Ok(())
    }

    pub fn encode(&mut self, text: &str, result: &mut Vec<u8>) -> Result<(), String> {
        self.encode_via_ir(text, result)
    }

    pub fn encode_with_formatting(
        &mut self,
        text: &str,
        spans: &[crate::FormattingSpan],
        result: &mut Vec<u8>,
    ) -> Result<(), String> {
        if spans.is_empty() {
            return self.encode(text, result);
        }

        self.encode_via_ir_with_transform(text, result, |source, tokens| {
            inject_formatting_tokens(source, spans, tokens)
        })
    }
}

fn inject_formatting_tokens(
    text: &str,
    spans: &[crate::FormattingSpan],
    tokens: &mut Vec<Token<'_>>,
) -> Result<(), String> {
    let text_len = text.len();
    let mut starts: std::collections::BTreeMap<usize, Vec<crate::FormattingKind>> =
        std::collections::BTreeMap::new();
    let mut ends: std::collections::BTreeMap<usize, Vec<crate::FormattingKind>> =
        std::collections::BTreeMap::new();

    for span in spans {
        let start = span.range.start;
        let end = span.range.end;
        if start >= end {
            return Err(format!("Invalid formatting span range: {start}..{end}"));
        }
        if end > text_len {
            return Err(format!(
                "Formatting span out of bounds: {start}..{end} (len={text_len})"
            ));
        }
        if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
            return Err(format!(
                "Formatting span must align to UTF-8 boundaries: {start}..{end}"
            ));
        }
        starts.entry(start).or_default().push(span.kind);
        ends.entry(end).or_default().push(span.kind);
    }

    let mut new_tokens = Vec::new();
    let mut cursor = 0usize;

    let emit_events_at =
        |pos: usize,
         out: &mut Vec<Token<'_>>,
         start_map: &mut std::collections::BTreeMap<usize, Vec<crate::FormattingKind>>,
         end_map: &mut std::collections::BTreeMap<usize, Vec<crate::FormattingKind>>| {
            if let Some(kinds) = end_map.remove(&pos) {
                for kind in kinds.iter().rev() {
                    let (_, close) = kind.markers();
                    out.push(Token::PreEncoded(close.to_vec()));
                }
            }
            if let Some(kinds) = start_map.remove(&pos) {
                for kind in kinds {
                    let (open, _) = kind.markers();
                    out.push(Token::PreEncoded(open.to_vec()));
                }
            }
        };

    emit_events_at(cursor, &mut new_tokens, &mut starts, &mut ends);

    for token in tokens.iter() {
        match token {
            Token::Word(word) => {
                let text_ref = word.text.as_ref();
                let word_end = cursor.saturating_add(text_ref.len());
                let mut internal_points = starts
                    .keys()
                    .chain(ends.keys())
                    .copied()
                    .filter(|pos| *pos > cursor && *pos < word_end)
                    .map(|pos| pos - cursor)
                    .collect::<Vec<_>>();
                internal_points.sort_unstable();
                internal_points.dedup();

                let mut local_start = 0usize;
                for local_end in internal_points
                    .into_iter()
                    .chain(std::iter::once(text_ref.len()))
                {
                    let seg = &text_ref[local_start..local_end];
                    let seg_chars: Vec<char> = seg.chars().collect();
                    new_tokens.push(Token::Word(WordToken {
                        text: Cow::Owned(seg.to_string()),
                        chars: seg_chars.clone(),
                        meta: WordMeta::from_chars(&seg_chars),
                    }));

                    cursor += seg.len();
                    emit_events_at(cursor, &mut new_tokens, &mut starts, &mut ends);
                    local_start = local_end;
                }
            }
            Token::Space(space) => {
                new_tokens.push(Token::Space(*space));
                cursor += 1;
                emit_events_at(cursor, &mut new_tokens, &mut starts, &mut ends);
            }
            _ => new_tokens.push(token.clone()),
        }
    }

    emit_events_at(cursor, &mut new_tokens, &mut starts, &mut ends);
    if !starts.is_empty() || !ends.is_empty() {
        return Err("Formatting spans could not be mapped to token boundaries".to_string());
    }

    *tokens = new_tokens;
    Ok(())
}
