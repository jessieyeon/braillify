use crate::char_struct::CharType;
use crate::rules::RuleMeta;
use crate::rules::context::{EncodingMode, RuleContext};
use crate::rules::traits::{BrailleRule, Phase, RuleResult};

pub static META: RuleMeta = RuleMeta { section: "71", subsection: None, name: "information_symbols", standard_ref: "2024 Korean Braille Standard, Ch.6 Art.71", description: "Keyboard, copyright, and information symbols" };

const MAPPINGS: &[(char, &str)] = &[('@', "⠈⠁"), ('^', "⠈⠢"), ('#', "⠸⠹"), ('|', "⠸⠳"), ('\\', "⠸⠡"), ('&', "⠈⠯"), ('§', "⠘⠎"), ('¶', "⠘⠏"), ('©', "⠘⠉"), ('®', "⠘⠗"), ('™', "⠘⠞")];

fn encode_unicode_cells(unicode: &str) -> Vec<u8> {
    unicode.chars().map(crate::unicode::decode_unicode).collect()
}

fn should_wrap_information_symbol(ctx: &RuleContext) -> bool {
    if ctx.word_len() > 1 {
        return true;
    }

    let prev_has_korean = !ctx.prev_word.is_empty() && ctx.prev_word.chars().any(crate::utils::is_korean_char);
    let next_has_korean = ctx.remaining_words.first().is_some_and(|word| !word.is_empty() && word.chars().any(crate::utils::is_korean_char));

    prev_has_korean || next_has_korean
}

pub fn is_rule_71_symbol(c: char) -> bool {
    MAPPINGS.iter().any(|(candidate, _)| *candidate == c)
}

pub struct Rule71;

impl BrailleRule for Rule71 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn phase(&self) -> Phase {
        Phase::CoreEncoding
    }

    fn priority(&self) -> u16 {
        175
    }

    fn matches(&self, ctx: &RuleContext) -> bool {
        ctx.state.current_mode() != EncodingMode::Math && matches!(ctx.char_type, CharType::Symbol(c) if is_rule_71_symbol(*c))
    }

    fn apply(&self, ctx: &mut RuleContext) -> Result<RuleResult, String> {
        if ctx.current_char() == '§' {
            if should_wrap_information_symbol(ctx) {
                // 제71항: 정보 기호는 한국어/숫자 컨텍스트에서 ⠴...⠲ wrap을 두른다.
                // 직후가 숫자면 종료표 ⠲ 생략(숫자 자체가 영자 컨텍스트로 이어짐).
                // 어절 내부에서 §을 만났을 때(ctx.index > 0)도 추가 공백을 emit하지 않는다.
                // 어절 간 공백은 Token::Space가 책임지며, 어절 내 음절/기호 사이는
                // 묵자 입력 그대로 결합한다(한국어 띄어쓰기 일반 원칙).
                let mut encoded = vec![crate::unicode::decode_unicode('⠴')];
                encoded.extend(encode_unicode_cells("⠘⠎"));
                if !ctx.next_char().is_some_and(|ch| ch.is_ascii_digit()) {
                    encoded.push(crate::unicode::decode_unicode('⠲'));
                }
                ctx.emit_slice(&encoded);
                return Ok(RuleResult::Consumed);
            }

            let encoded = encode_unicode_cells("⠘⠎");
            ctx.emit_slice(&encoded);
            return Ok(RuleResult::Consumed);
        }

        let Some((_, unicode)) = MAPPINGS.iter().find(|(candidate, _)| *candidate == ctx.current_char()) else {
            return Ok(RuleResult::Skip);
        };

        let mut encoded = Vec::new();
        if should_wrap_information_symbol(ctx) && matches!(ctx.current_char(), '&' | '¶' | '©' | '®' | '™') {
            encoded.push(crate::unicode::decode_unicode('⠴'));
            encoded.extend(encode_unicode_cells(unicode));
            encoded.push(crate::unicode::decode_unicode('⠲'));
        } else {
            encoded = encode_unicode_cells(unicode);
        }
        ctx.emit_slice(&encoded);
        Ok(RuleResult::Consumed)
    }
}
