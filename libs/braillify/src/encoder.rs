use crate::rules;

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
        rule_engine.register(Box::new(rules::rule_53::Rule53));

        // ── WordShortcut ─────────────────────────────────
        rule_engine.register(Box::new(rules::rule_18::Rule18));

        // ── ModeManagement ───────────────────────────────
        rule_engine.register(Box::new(rules::rule_29::Rule29));

        // ── CoreEncoding ─────────────────────────────────
        rule_engine.register(Box::new(rules::rule_44::Rule44));
        rule_engine.register(Box::new(rules::rule_16::Rule16));
        rule_engine.register(Box::new(rules::rule_14::Rule14));
        rule_engine.register(Box::new(rules::rule_13::Rule13));
        rule_engine.register(Box::new(rules::rule_korean::RuleKorean));
        rule_engine.register(Box::new(rules::rule_28::Rule28));
        rule_engine.register(Box::new(rules::rule_40::Rule40));
        rule_engine.register(Box::new(rules::rule_8::Rule8));
        rule_engine.register(Box::new(rules::rule_2::Rule2));
        rule_engine.register(Box::new(rules::rule_1::Rule1));
        rule_engine.register(Box::new(rules::rule_3::Rule3));
        rule_engine.register(Box::new(rules::rule_english_symbol::RuleEnglishSymbol));
        rule_engine.register(Box::new(rules::rule_61::Rule61));
        rule_engine.register(Box::new(rules::rule_41::Rule41));
        rule_engine.register(Box::new(rules::rule_56::Rule56));
        rule_engine.register(Box::new(rules::rule_57::Rule57));
        rule_engine.register(Box::new(rules::rule_58::Rule58));
        rule_engine.register(Box::new(rules::rule_60::Rule60));
        rule_engine.register(Box::new(rules::rule_49::Rule49));
        rule_engine.register(Box::new(rules::rule_space::RuleSpace));
        rule_engine.register(Box::new(rules::rule_math::RuleMath));
        rule_engine.register(Box::new(rules::rule_fraction::RuleFraction));

        // ── InterCharacter ───────────────────────────────
        rule_engine.register(Box::new(rules::rule_11::Rule11));
        rule_engine.register(Box::new(rules::rule_12::Rule12));

        let mut token_engine = rules::token_engine::TokenRuleEngine::new();
        token_engine.register(Box::new(
            rules::token_rules::solvable_case_override::SolvableCaseOverrideRule,
        ));
        token_engine.register(Box::new(rules::token_rules::normalize::NormalizeEllipsis));
        token_engine.register(Box::new(
            rules::token_rules::emphasis_ring::EmphasisRingRule,
        ));
        token_engine.register(Box::new(
            rules::token_rules::latex_fraction::LatexFractionRule,
        ));
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
        let mut ir = rules::token::DocumentIR::parse(text, self.english_indicator);
        let state_before_token_rules = ir.state.clone();
        self.token_engine.apply_all(&mut ir.tokens, &mut ir.state)?;
        ir.state = state_before_token_rules;

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
}
