//! Math symbol encoding with Korean spacing rules.
//!
//! Math symbols (＋, −, ×, ÷, etc.) need spacing around them when
//! adjacent to Korean text, unless the Korean is a grammatical particle (josa).

use crate::char_struct::CharType;
use crate::math_symbol_shortcut;
use crate::rules::RuleMeta;
use crate::rules::context::RuleContext;
use crate::rules::traits::{BrailleRule, Phase, RuleResult};
use crate::utils;

pub static META: RuleMeta = RuleMeta {
    section: "math",
    subsection: None,
    name: "math_symbol_encoding",
    standard_ref: "2024 Korean Braille Standard (math symbols)",
    description: "Math symbols with Korean spacing rules",
};

/// Korean particles (josa) that should NOT have spacing before them.
const JOSA: &[&str] = &["과", "와", "이다", "하고", "이랑", "와", "랑", "아니다"];

pub struct RuleMath;

impl BrailleRule for RuleMath {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        matches!(ctx.char_type, CharType::MathSymbol(_))
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        let CharType::MathSymbol(c) = ctx.char_type else {
            return Ok(RuleResult::Skip);
        };

        // Space before math symbol if preceded by Korean
        if ctx.index > 0
            && ctx.word_chars[..ctx.index]
                .iter()
                .any(|ch| utils::is_korean_char(*ch))
        {
            ctx.emit(0);
        }

        let encoded = math_symbol_shortcut::encode_char_math_symbol_shortcut(*c)?;
        ctx.emit_slice(encoded);

        // Space after math symbol if followed by non-josa Korean
        if ctx.index < ctx.word_chars.len() - 1 {
            let mut korean = Vec::new();
            for wc in &ctx.word_chars[ctx.index + 1..] {
                if utils::is_korean_char(*wc) {
                    korean.push(*wc);
                } else if !korean.is_empty() {
                    break;
                }
            }
            if !korean.is_empty() {
                let korean_str: String = korean.into_iter().collect();
                if !JOSA.contains(&korean_str.as_str()) {
                    ctx.emit(0);
                }
            }
        }

        Ok(RuleResult::Consumed)
    }
}
