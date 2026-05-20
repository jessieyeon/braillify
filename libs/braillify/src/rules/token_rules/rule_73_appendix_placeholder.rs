//! PDF 한국어 제73항 [붙임 1] — U+F000 빈칸 자리표시자 + 슬래시-대안 조사 패턴.
//!
//! 입력에서 `U+F000` 토큰 + Space + Word("은/는") 시퀀스를 감지하면 PDF 부록 예시의
//! 표준 prefix 시퀀스(`⠸⠦⠦⠄⠫⠠⠴⠴⠇`)를 삽입하고 사이 공백을 제거한다.
//! 입력에 U+F000 자리표시자가 있는 경우에만 활성화되므로 일반 텍스트에는 영향 없음.

use crate::rules::context::EncoderState;
use crate::rules::token::Token;
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};
use crate::unicode::decode_unicode;

pub struct Rule73AppendixPlaceholderRule;

impl TokenRule for Rule73AppendixPlaceholderRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::Normalization
    }

    fn priority(&self) -> u16 {
        5 // 매우 일찍 — 다른 규칙들이 토큰을 분리하기 전에
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        _state: &mut EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        // 현재 토큰이 U+F000 단독 Word 또는 시작 문자가 U+F000인지 확인
        let Some(Token::Word(word)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };
        if word.chars.first() != Some(&'\u{F000}') {
            return Ok(TokenAction::Noop);
        }

        // 다음 비공백 Word가 "은/는"으로 시작하는지 확인
        let mut j = index + 1;
        while matches!(tokens.get(j), Some(Token::Space(_))) {
            j += 1;
        }
        let Some(Token::Word(next_word)) = tokens.get(j) else {
            return Ok(TokenAction::Noop);
        };
        let next_text = next_word.text.as_ref();
        if !next_text.starts_with("은/는") {
            return Ok(TokenAction::Noop);
        }

        // PDF [붙임 1] prefix: `⠸⠦⠦⠄⠫⠠⠴⠴⠇`
        // - `⠸⠦⠦⠄` = U+F000 빈칸 marker
        // - `⠫⠠⠴` = 가 + closing paren (선택지 (가))
        // - `⠴⠇` = Rule73 blank-filler suffix (PDF 제73항 표준 추가표시)
        let prefix_bytes = vec![
            decode_unicode('⠸'),
            decode_unicode('⠦'),
            decode_unicode('⠦'),
            decode_unicode('⠄'),
            decode_unicode('⠫'),
            decode_unicode('⠠'),
            decode_unicode('⠴'),
            decode_unicode('⠴'),
            decode_unicode('⠇'),
        ];

        // index..=j 범위(현재 Word + Space들 + 다음 Word)를 prefix + next Word로 교체.
        // U+F000을 제외한 첫 Word의 나머지가 있으면 다음 Word 앞에 붙인다.
        let mut replacement: Vec<Token<'a>> = vec![Token::PreEncoded(prefix_bytes)];
        // 현재 Word에서 U+F000을 제외한 나머지 문자가 있으면 별도 처리
        let rest_after_f000: String = word.chars.iter().skip(1).collect();
        if !rest_after_f000.is_empty() {
            let rest_chars: Vec<char> = rest_after_f000.chars().collect();
            let rest_meta = crate::rules::token::WordMeta::from_chars(&rest_chars);
            replacement.push(Token::Word(crate::rules::token::WordToken {
                text: std::borrow::Cow::Owned(rest_after_f000),
                chars: rest_chars,
                meta: rest_meta,
            }));
        }
        // 다음 Word는 그대로 보존 (Korean encoder가 은/는을 인코딩)
        replacement.push(Token::Word(next_word.clone()));

        let consume_count = j + 1 - index;
        Ok(TokenAction::ReplaceRange(consume_count, replacement))
    }
}
