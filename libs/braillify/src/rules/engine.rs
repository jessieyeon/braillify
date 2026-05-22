//! `RuleEngine` — the plugin host.
//!
//! Collects rules, sorts by phase+priority, applies them in order.
//! Supports enabling/disabling rules by section ID.

use std::collections::HashSet;

use super::context::RuleContext;
use super::traits::{BrailleRule, Phase, RuleResult};

/// The rule engine — holds all registered rules and applies them.
///
/// # Usage
/// ```ignore
/// let mut engine = RuleEngine::new();
/// engine.register(Box::new(Rule11VowelYe));
/// engine.register(Box::new(Rule12VowelAe));
///
/// // Disable a specific rule:
/// engine.disable("12");
///
/// // Apply to a character context:
/// engine.apply(&mut ctx)?;
/// ```
pub struct RuleEngine {
    rules: Vec<Box<dyn BrailleRule>>,
    /// Rules disabled by section ID (e.g., "11", "14")
    disabled: HashSet<String>,
    sorted: bool,
}

impl RuleEngine {
    /// Create an empty engine.
    pub fn new() -> Self {
        Self { rules: Vec::new(), disabled: HashSet::new(), sorted: false }
    }

    /// Register a rule plugin.
    pub fn register(&mut self, rule: Box<dyn BrailleRule>) {
        self.rules.push(rule);
        self.sorted = false;
    }

    /// Disable a rule by its section ID (e.g., "11" to disable 제11항).
    #[cfg(test)]
    pub fn disable(&mut self, section: &str) {
        self.disabled.insert(section.to_string());
    }

    /// Enable a previously disabled rule.
    #[cfg(test)]
    pub fn enable(&mut self, section: &str) {
        self.disabled.remove(section);
    }

    /// Check if a rule is currently enabled.
    pub fn is_enabled(&self, section: &str) -> bool {
        !self.disabled.contains(section)
    }

    /// Get count of registered rules.
    #[cfg(test)]
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Get count of currently enabled rules.
    #[cfg(test)]
    pub fn enabled_count(&self) -> usize {
        self.rules.iter().filter(|r| self.is_enabled(r.meta().section)).count()
    }

    /// List all registered rule metadata (for introspection/debugging).
    #[cfg(test)]
    pub fn list_rules(&self) -> Vec<&super::RuleMeta> {
        self.rules.iter().map(|r| r.meta()).collect()
    }

    /// Sort rules by (phase, priority). Called automatically before first apply.
    fn ensure_sorted(&mut self) {
        if !self.sorted {
            self.rules.sort_by_key(|r| (r.phase() as u8, r.priority()));
            self.sorted = true;
        }
    }

    /// Apply all enabled rules to the current character context.
    ///
    /// Rules run in phase order, then by priority within a phase.
    /// If a rule returns `Consumed`, subsequent rules are skipped.
    /// If a rule returns `Continue`, the next rule runs.
    /// If a rule returns `Skip`, it didn't apply — next rule runs.
    #[cfg(test)]
    pub fn apply(&mut self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        self.ensure_sorted();

        for rule in &self.rules {
            let meta = rule.meta();
            if !self.is_enabled(meta.section) {
                continue;
            }
            if !rule.matches(ctx) {
                continue;
            }
            match rule.apply(ctx)? {
                RuleResult::Consumed => return Ok(RuleResult::Consumed),
                RuleResult::Continue => {}
                RuleResult::Skip => {}
            }
        }
        Ok(RuleResult::Skip)
    }

    /// Apply only rules in a specific phase.
    pub fn apply_phase(&mut self, phase: Phase, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        self.ensure_sorted();

        for rule in &self.rules {
            if rule.phase() != phase {
                continue;
            }
            let meta = rule.meta();
            if !self.is_enabled(meta.section) {
                continue;
            }
            if !rule.matches(ctx) {
                continue;
            }
            match rule.apply(ctx)? {
                RuleResult::Consumed => return Ok(RuleResult::Consumed),
                RuleResult::Continue => {}
                RuleResult::Skip => {}
            }
        }
        Ok(RuleResult::Skip)
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::RuleMeta;
    use crate::rules::context::EncoderState;

    static TEST_META: RuleMeta = RuleMeta { section: "test", subsection: None, name: "test_rule", standard_ref: "test", description: "test rule that emits byte 99" };

    struct TestRule;
    impl BrailleRule for TestRule {
        fn meta(&self) -> &'static RuleMeta {
            &TEST_META
        }
        fn phase(&self) -> Phase {
            Phase::CoreEncoding
        }
        fn matches(&self, _ctx: &RuleContext) -> bool {
            true
        }
        fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
            ctx.emit(99);
            Ok(RuleResult::Consumed)
        }
    }

    #[test]
    fn engine_registers_and_applies() {
        let mut engine = RuleEngine::new();
        engine.register(Box::new(TestRule));
        assert_eq!(engine.rule_count(), 1);

        let word_chars = vec!['가'];
        let char_type = crate::char_struct::CharType::new('가').unwrap();
        let mut state = EncoderState::new(false);
        let mut result = Vec::new();
        let mut skip = 0usize;
        let empty: Vec<&str> = vec![];
        let mut ctx = RuleContext { word_chars: &word_chars, index: 0, char_type: &char_type, prev_word: "", remaining_words: &empty, has_korean_char: true, is_all_uppercase: false, ascii_starts_at_beginning: false, skip_count: &mut skip, state: &mut state, result: &mut result };

        let outcome = engine.apply(&mut ctx).unwrap();
        assert_eq!(outcome, RuleResult::Consumed);
        assert_eq!(result, vec![99]);
    }

    #[test]
    fn engine_disables_rules() {
        let mut engine = RuleEngine::new();
        engine.register(Box::new(TestRule));
        engine.disable("test");

        assert_eq!(engine.enabled_count(), 0);
        assert!(!engine.is_enabled("test"));

        engine.enable("test");
        assert_eq!(engine.enabled_count(), 1);
    }

    #[test]
    fn engine_sorts_by_phase_and_priority() {
        static META_A: RuleMeta = RuleMeta { section: "a", subsection: None, name: "post", standard_ref: "", description: "" };
        static META_B: RuleMeta = RuleMeta { section: "b", subsection: None, name: "core", standard_ref: "", description: "" };

        struct PostRule;
        impl BrailleRule for PostRule {
            fn meta(&self) -> &'static RuleMeta {
                &META_A
            }
            fn phase(&self) -> Phase {
                Phase::PostProcessing
            }
            fn matches(&self, _: &RuleContext) -> bool {
                false
            }
            fn apply(&self, _: &mut RuleContext) -> Result<RuleResult, String> {
                Ok(RuleResult::Skip)
            }
        }
        struct CoreRule;
        impl BrailleRule for CoreRule {
            fn meta(&self) -> &'static RuleMeta {
                &META_B
            }
            fn phase(&self) -> Phase {
                Phase::CoreEncoding
            }
            fn matches(&self, _: &RuleContext) -> bool {
                false
            }
            fn apply(&self, _: &mut RuleContext) -> Result<RuleResult, String> {
                Ok(RuleResult::Skip)
            }
        }

        let mut engine = RuleEngine::new();
        engine.register(Box::new(PostRule));
        engine.register(Box::new(CoreRule));
        engine.ensure_sorted();

        let metas = engine.list_rules();
        assert_eq!(metas[0].name, "core"); // CoreEncoding before PostProcessing
        assert_eq!(metas[1].name, "post");
    }
}
