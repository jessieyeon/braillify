use std::borrow::Cow;

use crate::rules::token::{Token, WordMeta, WordToken};
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};
use crate::utils::is_korean_char;

/// PDF 제39항 — 영어 어절 사이에 끼인 한글 어절은 한글표(⠸⠷) ... 한글 종료표(⠸⠾)로 감싼다.
///
/// 일반 알고리즘 (testcase 입력에 의존하지 않음):
/// 1. 현재 토큰 안에서 한글 char segment를 좌→우 스캔으로 추출한다.
/// 2. 각 한글 segment의 좌·우 컨텍스트가 영어인지 판정한다.
///    - 좌측: 같은 토큰 내 직전 알파벳/숫자가 ASCII 영문이거나,
///      segment 앞이 토큰 시작이고 이전 word token이 영어로 시작한다.
///    - 우측: 같은 토큰 내 직후 알파벳/숫자가 ASCII 영문이거나,
///      segment 뒤가 토큰 끝이고 다음 word token이 영어로 시작한다.
///    - 영문/한글이 아닌 문장부호는 컨텍스트 판단에서 투명하게 통과한다.
/// 3. 양쪽 모두 영어 컨텍스트이면 그 segment를 한글표/한글 종료표로 감싼다.
///
/// 이 알고리즘은:
///   - "What is 김치 in English?" — `김치`는 영어 토큰 `is`, `in` 사이 → wrap
///   - "Banchan (Korean: 반찬) are ..." — `반찬`은 영어 토큰 사이 → wrap
///   - "www.대통령.kr이다." — 한 토큰 내 `대통령`은 영어 segment `www.`와 `.kr` 사이 → wrap
///   - "원형 동사이다." (rule_37) — `원형` 양옆 영어 없음 → wrap X
///   - "[ㅅ떠디이]로," (rule_10) — 한글 segment 앞이 `[`(영어 컨텍스트 아님) → wrap X
const HANGUL_WRAP_START: [u8; 2] = [56, 55]; // ⠸⠷ — 한글표 (제39항)
const HANGUL_WRAP_END: [u8; 2] = [56, 62]; // ⠸⠾ — 한글 종료표 (제39항)

pub struct EnglishDominantKoreanWrapRule;

fn build_word_token<'a>(text: &str) -> Token<'a> {
    let chars: Vec<char> = text.chars().collect();

    Token::Word(WordToken {
        text: Cow::Owned(text.to_string()),
        chars: chars.clone(),
        meta: WordMeta::from_chars(&chars),
    })
}

/// 직전 Word 토큰을 거슬러 찾는다. Space/PreEncoded는 투명.
fn prev_word_token<'a, 'b>(tokens: &'b [Token<'a>], index: usize) -> Option<&'b WordToken<'a>> {
    for token in tokens[..index].iter().rev() {
        match token {
            Token::Word(w) => return Some(w),
            Token::Space(_) | Token::PreEncoded(_) => continue,
            _ => return None,
        }
    }
    None
}

/// 직후 Word 토큰을 찾는다. Space/PreEncoded는 투명.
fn next_word_token<'a, 'b>(tokens: &'b [Token<'a>], index: usize) -> Option<&'b WordToken<'a>> {
    for token in tokens.iter().skip(index + 1) {
        match token {
            Token::Word(w) => return Some(w),
            Token::Space(_) | Token::PreEncoded(_) => continue,
            _ => return None,
        }
    }
    None
}

/// 토큰이 한글 글자를 포함하지 않고 영어/숫자/기호로만 구성되는지 판정.
fn word_is_english_only(word: &WordToken<'_>) -> bool {
    !word.meta.has_korean && word.meta.has_ascii_alphabetic
}

/// 슬라이스가 영문/한글 없이 문장부호로만 구성되는지.
fn is_punct_only(chars: &[char]) -> bool {
    chars
        .iter()
        .all(|c| !c.is_ascii_alphabetic() && !is_korean_char(*c) && !c.is_ascii_digit())
}

/// 같은 토큰 내에서 좌측을 거슬러 처음 만나는 letter가 ASCII 영문인지.
/// 한글을 먼저 만나거나, 영문도 한글도 없으면 false.
fn same_token_left_is_english(left_chars: &[char]) -> bool {
    for ch in left_chars.iter().rev() {
        if ch.is_ascii_alphabetic() {
            return true;
        }
        if is_korean_char(*ch) {
            return false;
        }
    }
    false
}

/// 같은 토큰 내에서 우측으로 처음 만나는 letter가 ASCII 영문인지.
fn same_token_right_is_english(right_chars: &[char]) -> bool {
    for ch in right_chars.iter() {
        if ch.is_ascii_alphabetic() {
            return true;
        }
        if is_korean_char(*ch) {
            return false;
        }
    }
    false
}

#[derive(Debug, Clone, Copy)]
struct KoreanSegment {
    char_start: usize,
    char_end: usize, // exclusive
}

/// 토큰 텍스트에서 연속된 한글 char segment의 (시작, 끝) 인덱스를 모두 추출.
fn find_korean_segments(chars: &[char]) -> Vec<KoreanSegment> {
    let mut segments = Vec::new();
    let mut current_start: Option<usize> = None;
    for (idx, ch) in chars.iter().enumerate() {
        if is_korean_char(*ch) {
            if current_start.is_none() {
                current_start = Some(idx);
            }
        } else if let Some(start) = current_start.take() {
            segments.push(KoreanSegment {
                char_start: start,
                char_end: idx,
            });
        }
    }
    if let Some(start) = current_start {
        segments.push(KoreanSegment {
            char_start: start,
            char_end: chars.len(),
        });
    }
    segments
}

/// 문서의 영어/한글 어절 분포로 영어가 다수파인지 판정.
/// case B(토큰 경계 wrap)는 영어가 다수인 문장에서만 활성화한다.
/// 한국어 주도 문장에 우연히 영어 어절 사이에 한국어가 끼어 있는 경우
/// (예: "평창 ... SNS 계정은 pyeongchang ...")는 wrap 대상이 아니다.
fn document_is_english_majority<'a>(tokens: &[Token<'a>]) -> bool {
    let mut english_words = 0usize;
    let mut korean_words = 0usize;
    for token in tokens.iter() {
        let Token::Word(word) = token else { continue };
        let first = word
            .chars
            .iter()
            .copied()
            .find(|ch| ch.is_ascii_alphabetic() || is_korean_char(*ch));
        match first {
            Some(c) if c.is_ascii_alphabetic() => english_words += 1,
            Some(c) if is_korean_char(c) => korean_words += 1,
            _ => {}
        }
    }
    english_words >= korean_words.max(1)
}

/// 한글 segment의 좌·우 컨텍스트가 영어인지 판정.
///
/// 두 케이스로 나누어 처리한다:
/// 1. **양쪽 토큰 경계** — segment의 양쪽이 모두 토큰 끝이거나 문장부호만 있다.
///    예: "김치", "반찬)". 인접 word token이 모두 영어이면서 _문서 전체가 영어 다수_
///    일 때만 wrap. (한글 주도 문장에 영어가 끼인 경우는 wrap 대상 아님.)
/// 2. **양쪽 토큰 내부** — segment의 양쪽이 같은 토큰 내 영어 letter로 둘러싸였다.
///    예: "www.대통령.kr"의 "대통령". 양쪽이 영어 letter이면 wrap.
///    (단일 단어 내부 패턴은 문서 비율과 무관하게 항상 적용한다.)
///
/// 두 케이스가 _혼합_된 경우(한쪽은 token boundary, 다른 쪽은 same-token letter)는
/// 영어 어절 + 한국어 조사/어미 결합(예: "be는")일 가능성이 높으므로 wrap하지 않는다.
fn segment_in_english_context<'a>(
    chars: &[char],
    seg: KoreanSegment,
    tokens: &[Token<'a>],
    token_index: usize,
) -> bool {
    let left_slice = &chars[..seg.char_start];
    let right_slice = &chars[seg.char_end..];

    let left_at_boundary = is_punct_only(left_slice);
    let right_at_boundary = is_punct_only(right_slice);

    match (left_at_boundary, right_at_boundary) {
        (true, true) => {
            let prev_eng = prev_word_token(tokens, token_index).is_some_and(word_is_english_only);
            let next_eng = next_word_token(tokens, token_index).is_some_and(word_is_english_only);
            prev_eng && next_eng && document_is_english_majority(tokens)
        }
        (false, false) => {
            same_token_left_is_english(left_slice) && same_token_right_is_english(right_slice)
        }
        _ => false,
    }
}

/// 문서 전체에서 어떤 한글 segment 하나라도 영어로 둘러싸였는지 확인.
/// (영어 주도성 판정의 보조 신호로 사용.)
fn document_has_english_context_for_korean<'a>(tokens: &[Token<'a>]) -> bool {
    for (idx, token) in tokens.iter().enumerate() {
        let Token::Word(word) = token else { continue };
        if !word.meta.has_korean {
            continue;
        }
        for seg in find_korean_segments(&word.chars) {
            if segment_in_english_context(&word.chars, seg, tokens, idx) {
                return true;
            }
        }
    }
    false
}

/// 문서가 _완전히 영어 주도_인지 판정. 즉 한글이 극히 소수만 등장.
///
/// 이 경우 wrap(⠸⠷⠸⠾)에 더해 영어 어절 사이의 영자표시(⠴), 단일 대문자
/// 표시(⠠), 종료표(⠲)도 모두 생략한다 (PDF 제39항 영어 주도 문장).
///
/// 기준: 영어 어절 수가 한글 어절 수의 5배 이상 _그리고_ 영어 어절이 10개 이상.
/// (단순 비율은 짧은 문장에서 과적용되고, 단순 절대 수는 한글 비중을 무시한다.)
pub(crate) fn document_is_english_dominant<'a>(tokens: &[Token<'a>]) -> bool {
    let mut english_words = 0usize;
    let mut korean_words = 0usize;
    for token in tokens.iter() {
        let Token::Word(word) = token else { continue };
        let first = word
            .chars
            .iter()
            .copied()
            .find(|ch| ch.is_ascii_alphabetic() || is_korean_char(*ch));
        match first {
            Some(c) if c.is_ascii_alphabetic() => english_words += 1,
            Some(c) if is_korean_char(c) => korean_words += 1,
            _ => {}
        }
    }
    english_words >= 10 && english_words >= korean_words.saturating_mul(5)
}

/// 토큰을 한글 segment 기준으로 분할하여 각각 wrap된 토큰 시퀀스를 만든다.
/// segment의 좌우 컨텍스트가 영어가 아닌 경우엔 그대로 둔다.
fn build_wrapped_replacement<'a>(
    word: &WordToken<'a>,
    tokens: &[Token<'a>],
    token_index: usize,
) -> Option<Vec<Token<'a>>> {
    let segments = find_korean_segments(&word.chars);
    if segments.is_empty() {
        return None;
    }

    let chars = &word.chars;

    let mut wrap_segments = Vec::new();
    for seg in segments {
        if segment_in_english_context(chars, seg, tokens, token_index) {
            wrap_segments.push(seg);
        }
    }

    if wrap_segments.is_empty() {
        return None;
    }

    let mut result: Vec<Token<'a>> = Vec::new();
    let mut cursor = 0usize;

    for seg in wrap_segments {
        if seg.char_start > cursor {
            let prefix: String = chars[cursor..seg.char_start].iter().collect();
            if !prefix.is_empty() {
                result.push(build_word_token(&prefix));
            }
        }
        let korean: String = chars[seg.char_start..seg.char_end].iter().collect();
        result.push(Token::PreEncoded(HANGUL_WRAP_START.to_vec()));
        result.push(build_word_token(&korean));
        result.push(Token::PreEncoded(HANGUL_WRAP_END.to_vec()));
        cursor = seg.char_end;
    }

    if cursor < chars.len() {
        let suffix: String = chars[cursor..].iter().collect();
        if !suffix.is_empty() {
            result.push(build_word_token(&suffix));
        }
    }

    Some(result)
}

impl TokenRule for EnglishDominantKoreanWrapRule {
    fn phase(&self) -> TokenPhase {
        // PostWord 단계는 fall-through(Noop이면 다음 룰 시도) 지원이라
        // 다른 PostWord 룰들과 협력 가능하며, 다른 ModeEntry 변환(digital_notation
        // 등)이 끝난 후 wrap을 적용하기 적합하다.
        TokenPhase::PostWord
    }

    fn priority(&self) -> u16 {
        // PostWord의 spacing(400) / middle_dot/quote(126) 룰보다 먼저 실행되어야
        // 단어 분할이 일어나기 전에 wrap을 적용할 수 있다.
        50
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        state: &mut crate::rules::context::EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        let Some(Token::Word(word)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };

        if !word.meta.has_korean {
            return Ok(TokenAction::Noop);
        }

        // 문서 전체에서 영어로 둘러싸인 한글 segment가 하나라도 있어야
        // 제39항 wrap을 활성화한다. (영어 주도성 보조 신호)
        if !document_has_english_context_for_korean(tokens) {
            return Ok(TokenAction::Noop);
        }

        // 영-한 wrap 컨텍스트가 활성화됨을 state에 기록한다. 이 플래그는
        // 영어 char 인코더가 단독 단어 약자(예: "in" → ⠔)를 적용할지 결정한다.
        state.english_dominant_wrap_active = true;

        // 추가: 문서가 영어 주도(영어 어절 ≫ 한글 어절)인 경우, 영자표시·
        // 단일 대문자 표시·종료표 모두 생략한다.
        if document_is_english_dominant(tokens) {
            state.english_dominant_no_indicator = true;
        }

        match build_wrapped_replacement(word, tokens, index) {
            Some(replacement) => Ok(TokenAction::ReplaceMany(replacement)),
            None => Ok(TokenAction::Noop),
        }
    }
}
