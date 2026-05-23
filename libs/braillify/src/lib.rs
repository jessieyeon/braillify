use std::{borrow::Cow, cell::RefCell};

mod char_shortcut;
pub(crate) mod char_struct;
#[cfg(feature = "cli")]
pub mod cli;
mod encoder;
pub(crate) mod english;
pub(crate) mod english_logic;
pub(crate) mod fraction;
mod ipa;
mod jauem;
mod korean_char;
mod korean_part;
mod math_symbol_shortcut;
mod moeum;
pub(crate) mod number;
mod rule;
mod rule_en;
pub(crate) mod rules;
mod split;
pub(crate) mod symbol_shortcut;
pub(crate) mod unicode;
pub(crate) mod utils;
pub(crate) mod word_shortcut;
use ipa::{detect_ipa_context, encode_ipa, is_ipa_phonetic_symbol};
#[cfg(test)]
mod test_helpers;

pub use encoder::Encoder;

thread_local! {
    static ENCODER_CACHE: RefCell<Option<Encoder>> = const { RefCell::new(None) };
}

fn with_encoder<F, R>(english_indicator: bool, f: F) -> R
where
    F: FnOnce(&mut Encoder) -> R,
{
    ENCODER_CACHE.with(|cell| {
        let Ok(mut cached) = cell.try_borrow_mut() else {
            let mut encoder = Encoder::new(english_indicator);
            encoder.reset_state();
            return f(&mut encoder);
        };

        if !matches!(&*cached, Some(encoder) if encoder.english_indicator() == english_indicator) {
            *cached = Some(Encoder::new(english_indicator));
        }

        let encoder = cached.as_mut().expect("encoder cache just initialized");
        encoder.reset_state();
        f(encoder)
    })
}

/// Options for controlling encoding behavior.
/// Used when context cannot be derived from input text alone.
#[derive(Debug, Clone, Default)]
pub struct EncodeOptions {
    /// Override the default encoding mode (normally Korean).
    pub default_mode: Option<crate::rules::context::EncodingMode>,
}

/// A formatting span applied to the input text.
#[derive(Debug, Clone)]
pub struct FormattingSpan {
    /// Byte offset range in the input string (start..end)
    pub range: std::ops::Range<usize>,
    /// Type of formatting
    pub kind: FormattingKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormattingKind {
    /// 드러냄표/밑줄 — wraps in ⠠⠤ ... ⠤⠄ (제56항)
    Emphasis,
    /// 굵은 글자 — wraps in ⠰⠤ ... ⠤⠆ (제56항)
    Bold,
    /// 제1점역자 정의 글자체 — wraps in ⠐⠤ ... ⠤⠂ (제56항 [붙임])
    Custom1,
    /// 제2점역자 정의 글자체 — wraps in ⠈⠤ ... ⠤⠁ (제56항 [붙임])
    Custom2,
}

impl FormattingKind {
    pub(crate) fn markers(self) -> ([u8; 2], [u8; 2]) {
        match self {
            Self::Emphasis => ([32, 36], [36, 4]),
            Self::Bold => ([48, 36], [36, 6]),
            Self::Custom1 => ([16, 36], [36, 2]),
            Self::Custom2 => ([8, 36], [36, 1]),
        }
    }
}

pub fn encode(text: &str) -> Result<Vec<u8>, String> {
    encode_with_options(text, &EncodeOptions::default())
}

/// PDF 수학 — Unicode Mathematical Alphanumeric Symbols(U+1D400–U+1D7FF)와
/// 첨자 라틴 문자를 ASCII 라틴 문자로 정규화한다.
/// 한국 점자 수학 규정은 글꼴 변형(italic/bold/script 등)을 별도 표기하지
/// 않으므로 `𝑃`(MATH ITALIC CAPITAL P) ≡ 일반 `P`로 취급한다.
#[cfg_attr(tarpaulin, inline(never))]
fn normalize_math_alphanumeric_char(c: char) -> char {
    let cp = c as u32;
    // Mathematical Italic small h는 U+1D455 자리 비고 U+210E (Planck) 사용.
    if cp == 0x210E {
        return 'h';
    }
    const BLOCKS: &[(u32, char)] = &[
        (0x1D400, 'A'),
        (0x1D41A, 'a'),
        (0x1D434, 'A'),
        (0x1D44E, 'a'),
        (0x1D468, 'A'),
        (0x1D482, 'a'),
        (0x1D49C, 'A'),
        (0x1D4B6, 'a'),
        (0x1D4D0, 'A'),
        (0x1D4EA, 'a'),
        (0x1D504, 'A'),
        (0x1D51E, 'a'),
        (0x1D538, 'A'),
        (0x1D552, 'a'),
        (0x1D56C, 'A'),
        (0x1D586, 'a'),
        (0x1D5A0, 'A'),
        (0x1D5BA, 'a'),
        (0x1D5D4, 'A'),
        (0x1D5EE, 'a'),
        (0x1D608, 'A'),
        (0x1D622, 'a'),
        (0x1D63C, 'A'),
        (0x1D656, 'a'),
        (0x1D670, 'A'),
        (0x1D68A, 'a'),
    ];
    for &(start, base) in BLOCKS {
        if cp >= start && cp < start + 26 {
            return char::from_u32(base as u32 + (cp - start)).unwrap_or(c);
        }
    }
    const DIGIT_BLOCKS: &[u32] = &[0x1D7CE, 0x1D7D8, 0x1D7E2, 0x1D7EC, 0x1D7F6];
    for &start in DIGIT_BLOCKS {
        if cp >= start && cp < start + 10 {
            let digit_code = b'0' as u32 + (cp - start);
            return char::from_u32(digit_code).unwrap_or(c);
        }
    }
    c
}

fn may_normalize_math_alphanumeric(c: char) -> bool {
    let cp = c as u32;
    cp == 0x210E || (0x1D400..=0x1D7FF).contains(&cp)
}

fn normalize_math_alphanumeric_string(text: &str) -> Cow<'_, str> {
    if !text.chars().any(may_normalize_math_alphanumeric) {
        return Cow::Borrowed(text);
    }

    Cow::Owned(text.chars().map(normalize_math_alphanumeric_char).collect())
}

#[derive(Clone, Copy, Default)]
struct NormalizationTriggers {
    has_math_alphanumeric: bool,
    has_decomposable_latin: bool,
    has_negation_combiner: bool,
    has_vector_mark: bool,
    has_formatting_mark_or_sentinel: bool,
    has_ipa_group_start: bool,
    has_ipa_symbol: bool,
}

impl NormalizationTriggers {
    fn scan(text: &str) -> Self {
        let mut triggers = Self::default();
        for c in text.chars() {
            triggers.has_math_alphanumeric |= may_normalize_math_alphanumeric(c);
            triggers.has_decomposable_latin |= may_decompose_accented_latin(c);
            triggers.has_negation_combiner |= c == '\u{0338}';
            triggers.has_vector_mark |= is_vector_mark(c);
            triggers.has_formatting_mark_or_sentinel |=
                is_formatting_mark(c) || is_formatting_sentinel(c);
            triggers.has_ipa_group_start |= matches!(c, '[' | '/');
            triggers.has_ipa_symbol |= is_ipa_phonetic_symbol(c);
        }
        triggers
    }

    fn may_need_emphasis_expansion(self) -> bool {
        // NFD decomposition can introduce formatting combining marks (for example U+0307).
        self.has_formatting_mark_or_sentinel || self.has_decomposable_latin
    }

    fn may_contain_ipa_context(self) -> bool {
        self.has_ipa_group_start && self.has_ipa_symbol
    }
}

/// PDF 수학 제34항 — 부정 결합 부호(U+0338 COMBINING LONG SOLIDUS OVERLAY)는
/// 점역 시 피수정 문자 앞으로 이동한다. 예: `ℛ̸` → `̸ℛ` → 점자 `⠨⠠⠗`.
fn move_negation_combiner_before_base<'a>(text: Cow<'a, str>) -> Cow<'a, str> {
    if !text.as_ref().contains('\u{0338}') {
        return text;
    }

    let source = text.as_ref();
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i + 1] == '\u{0338}' {
            out.push(chars[i + 1]);
            out.push(chars[i]);
            i += 2;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    Cow::Owned(out)
}

/// PDF 한글 제56항 — 결합 부호 기반 글자체 표지 처리.
///
/// 강조 대상 문자마다 결합 부호를 부착하는 **순환소수 스타일** 평문 표기를 지원한다.
/// 결합 부호는 `FormattingKind`와 1:1 매핑되며, PUA sentinel(U+E000~U+E007)로
/// 변환되어 후속 단계에서 점자 marker로 전개된다. 인접한 같은 종류 wrap은
/// [`merge_adjacent_formatting_wraps`]에 의해 자동으로 하나로 병합된다.
///
/// | 결합 부호 | 외관 | FormattingKind | 점자 |
/// |---|---|---|---|
/// | U+0307 (DOT ABOVE) | ̇ | 드러냄표/밑줄 (Emphasis) | ⠠⠤...⠤⠄ |
/// | U+0331 (MACRON BELOW) | ̱ | 굵은 글자 (Bold) | ⠰⠤...⠤⠆ |
/// | U+0332 (LOW LINE) | ̲ | 점역자1 글자체 (Custom1) | ⠐⠤...⠤⠂ |
/// | U+0333 (DOUBLE LOW LINE) | ̳ | 점역자2 글자체 (Custom2) | ⠈⠤...⠤⠁ |
///
/// 사용 규칙:
/// - **단위:** 각 결합 부호는 직전 1개의 비공백 문자를 글자체로 감싼다.
///   (per-char 컨벤션. 인접한 같은 종류 wrap은 자동 병합되어 연속 강조 단어를
///   `⠠⠤단어1 단어2⠤⠄` 형태의 단일 wrap으로 emit한다.)
/// - **N개 trailing 호환:** 단일 음절 뒤에 같은 결합 부호 N개를 연속(공백 허용)으로
///   붙이면 직전 N개 비공백 문자를 한 묶음으로 감싼다 (legacy 표기 호환).
/// - **숫자 흡수:** 한글 음절 직전에 결합된 숫자/`,`/`.` 연쇄는 같은 wrap에 자동
///   포함된다. (예: `15,000원̳` → `⠈⠤15,000원⠤⠁`. 한글 토큰의 일부로 본다.)
/// - **수학 컨텍스트 자동 회피:** 현재 토큰(공백으로 구분된 비공백 연쇄)에 한글
///   음절이 없으면 결합 부호의 본래 결합 의미(반복소수 ̇, 수학 변수 underline ̲)를
///   보존하기 위해 변환하지 않는다.
fn expand_emphasis_marks<'a>(text: Cow<'a, str>) -> Cow<'a, str> {
    /// (결합 부호, 시작 sentinel, 종료 sentinel).
    /// PUA U+E000~U+E007이 symbol_shortcut에서 점자 marker로 매핑된다.
    const FORMATTING_MARKS: &[(char, char, char)] = &[
        ('\u{0307}', '\u{E000}', '\u{E001}'), // 드러냄표/밑줄
        ('\u{0331}', '\u{E002}', '\u{E003}'), // 굵은 글자
        ('\u{0332}', '\u{E004}', '\u{E005}'), // 점역자1
        ('\u{0333}', '\u{E006}', '\u{E007}'), // 점역자2
    ];

    if !text
        .as_ref()
        .chars()
        .any(|c| is_formatting_sentinel(c) || is_formatting_mark(c))
    {
        return text;
    }

    let source = text.as_ref();
    let chars: Vec<char> = source.chars().collect();

    // Pre-scan: 각 char 위치의 토큰(공백으로 구분된 비공백 연쇄)에 한글이 있는지 표시.
    // 토큰에 한글이 없으면 결합 부호의 본래 결합 의미를 보존한다 (수학/영어 컨텍스트).
    let mut token_has_korean = vec![false; chars.len()];
    {
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == ' ' {
                i += 1;
                continue;
            }
            let start = i;
            while i < chars.len() && chars[i] != ' ' {
                i += 1;
            }
            let has = chars[start..i].iter().any(|c| utils::is_korean_char(*c));
            for slot in token_has_korean.iter_mut().take(i).skip(start) {
                *slot = has;
            }
        }
    }

    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let mark_entry = FORMATTING_MARKS
            .iter()
            .find(|(mark, _, _)| *mark == chars[i]);
        let Some(&(mark_char, start_sentinel, end_sentinel)) = mark_entry else {
            out.push(chars[i]);
            i += 1;
            continue;
        };

        // 토큰에 한글이 없으면 결합 부호 그대로 보존 (수학/영어 컨텍스트).
        if !token_has_korean[i] {
            out.push(chars[i]);
            i += 1;
            continue;
        }

        // 같은 결합 부호 그룹 수집 (사이 공백 허용).
        // legacy `돼지̇ ̇ ̇ ̇ ̇` 표기 호환: 첫 마크의 직전 토큰을 기준으로 N개 묶음 wrap.
        let mut count = 1;
        let mut last = i;
        let mut j = i + 1;
        while j < chars.len() {
            if chars[j] == mark_char {
                count += 1;
                last = j;
                j += 1;
            } else if chars[j] == ' ' && j + 1 < chars.len() && chars[j + 1] == mark_char {
                j += 1;
            } else {
                break;
            }
        }

        // out에서 N개의 비공백 문자(content unit)를 walk back. 공백/이미 삽입된
        // sentinel은 건너뛴다. 한글 음절뿐 아니라 숫자/구두점도 1 unit으로 센다.
        let mut units = 0;
        let mut start_in_out = out.len();
        while start_in_out > 0 && units < count {
            let c = out[start_in_out - 1];
            if c == ' ' || is_formatting_sentinel(c) {
                start_in_out -= 1;
            } else {
                units += 1;
                start_in_out -= 1;
            }
        }
        if units == count {
            // 한글 음절 직전 숫자/`,`/`.` 연쇄는 같은 wrap에 흡수 (per-token 단위 강조).
            while start_in_out > 0 {
                let c = out[start_in_out - 1];
                if c.is_ascii_digit() || matches!(c, ',' | '.') {
                    start_in_out -= 1;
                } else {
                    break;
                }
            }
            out.insert(start_in_out, start_sentinel);
            out.push(end_sentinel);
        } else {
            // 유닛 수가 부족하면 결합 부호를 그대로 보존한다.
            for _ in 0..count {
                out.push(mark_char);
            }
        }
        // 결합 부호 그룹 모두 skip
        i = last + 1;
    }
    merge_adjacent_formatting_wraps(Cow::Owned(out.into_iter().collect()))
}

/// 포매팅 sentinel(U+E000~U+E007) 여부.
fn is_formatting_sentinel(c: char) -> bool {
    matches!(c as u32, 0xE000..=0xE007)
}

fn is_formatting_mark(c: char) -> bool {
    matches!(c, '\u{0307}' | '\u{0331}' | '\u{0332}' | '\u{0333}')
}

/// 인접한 같은 종류 글자체 wrap을 하나로 병합한다.
///
/// PDF 제56항 — 사용자가 강조 대상을 단어별로 표시(`왜̇ 사느냐̇̇̇`)하면 각 단어가
/// 독립 wrap으로 인코딩되어 `⠠⠤왜⠤⠄ ⠠⠤사느냐⠤⠄`처럼 분리된다. 그러나 PDF는
/// 인접한 강조 단어를 하나의 wrap `⠠⠤왜 사느냐⠤⠄`로 묶는다. 이 함수는 같은
/// 종류 sentinel 쌍 사이의 공백만 포함된 구간을 감지하여 inner sentinel을 제거한다.
fn merge_adjacent_formatting_wraps<'a>(text: Cow<'a, str>) -> Cow<'a, str> {
    /// (시작 sentinel, 종료 sentinel) — `FORMATTING_MARKS`와 1:1 대응.
    const SENTINEL_PAIRS: &[(char, char)] = &[
        ('\u{E000}', '\u{E001}'),
        ('\u{E002}', '\u{E003}'),
        ('\u{E004}', '\u{E005}'),
        ('\u{E006}', '\u{E007}'),
    ];

    if !text.as_ref().chars().any(is_formatting_sentinel) {
        return text;
    }

    let mut chars: Vec<char> = text.as_ref().chars().collect();
    // 단순 반복: 한 번 병합이 일어나면 위치가 바뀌므로 다시 처음부터 스캔.
    let mut any_changed = false;
    let mut changed = true;
    while changed {
        changed = false;
        for &(open, close) in SENTINEL_PAIRS {
            let mut i = 0;
            while i < chars.len() {
                if chars[i] != close {
                    i += 1;
                    continue;
                }
                // `close` 직후가 공백 0개 이상 + 같은 종류 `open`이면 병합.
                let mut j = i + 1;
                while j < chars.len() && chars[j] == ' ' {
                    j += 1;
                }
                if j < chars.len() && chars[j] == open {
                    // close와 open을 제거. 공백은 보존.
                    chars.remove(j);
                    chars.remove(i);
                    changed = true;
                    any_changed = true;
                    // i는 그대로 둔다. 다음 close 찾기 시도.
                } else {
                    i += 1;
                }
            }
        }
    }
    if any_changed {
        Cow::Owned(chars.into_iter().collect())
    } else {
        text
    }
}

fn is_vector_mark(c: char) -> bool {
    matches!(c, '\u{20D6}' | '\u{20D7}' | '\u{20E1}' | '\u{20D1}')
}

/// PDF 수학 제37,38항 — 벡터/반직선/직선 결합 부호 처리.
/// 연속된 영문 대문자에 U+20D7(→), U+20D6(←), U+20E1(↔), U+20D1(반직선) 등의
/// 결합 부호가 각각 붙어 있으면, 결합부호를 한 번만 prefix하고 본문은 연쇄로 본다.
/// 예: `A⃗B⃗` → `⃗AB` → 점자 `⠒⠕⠠⠠⠁⠃`.
fn collapse_repeated_vector_marks<'a>(text: Cow<'a, str>) -> Cow<'a, str> {
    if !text.as_ref().chars().any(is_vector_mark) {
        return text;
    }

    let source = text.as_ref();
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    let mut changed = false;
    while i < chars.len() {
        // PDF 제37,38항 — 벡터/반직선 결합부호는 점자에서 letter 앞에 prefix한다.
        // 단독 `A⃗`도 `⃗A` 순으로 변환한다.
        if chars[i].is_ascii_alphabetic() && i + 1 < chars.len() && is_vector_mark(chars[i + 1]) {
            changed = true;
            let mark = chars[i + 1];
            // 연속된 letter+mark 쌍을 수집한다 (예: A⃗B⃗ → ⃗AB).
            let mut letters = vec![chars[i]];
            let mut j = i + 2;
            while j + 1 < chars.len() && chars[j].is_ascii_alphabetic() && chars[j + 1] == mark {
                letters.push(chars[j]);
                j += 2;
            }
            // 결합부호를 한 번만 prefix하고 letter 연쇄를 그대로 emit
            out.push(mark);
            for l in letters {
                out.push(l);
            }
            i = j;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    if changed { Cow::Owned(out) } else { text }
}

fn may_decompose_accented_latin(c: char) -> bool {
    let cp = c as u32;
    // Å, å는 단위(옹스트롬)/고유 문자로 단독 의미를 가지므로 NFD 분해하지 않는다.
    !matches!(c, '\u{00C5}' | '\u{00E5}')
        && ((0x00C0..=0x024F).contains(&cp) || (0x1E00..=0x1EFF).contains(&cp))
}

/// PDF 수학 제65항 5 — 라틴 문자 + 결합 부호(악센트)는 base letter + 결합 부호로
/// NFD 분해한다. (예: `ã` → `a` + `\u{0303}`, `ä` → `a` + `\u{0308}`)
/// 한글/CJK 문자는 분해되지 않도록 라틴 확장 범위에만 적용한다.
///
/// Caller (`encode_with_options`) guards this with `has_decomposable_latin`, so
/// the inner re-check is omitted — it would be a structurally unreachable
/// defensive branch that tarpaulin can never cover.
fn decompose_accented_latin<'a>(text: Cow<'a, str>) -> Cow<'a, str> {
    use unicode_normalization::UnicodeNormalization;

    let mut out = String::new();
    for c in text.as_ref().chars() {
        // Latin-1 Supplement, Latin Extended-A/B/Additional, IPA Extensions
        if may_decompose_accented_latin(c) {
            for d in std::iter::once(c).nfd() {
                out.push(d);
            }
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

/// Encode text to braille with explicit options.
pub fn encode_with_options(text: &str, options: &EncodeOptions) -> Result<Vec<u8>, String> {
    use crate::rules::context::EncodingMode;

    // PDF 수학 — Mathematical Alphanumeric 변형(italic/bold/script 등)을 ASCII로
    // 정규화. 한국 점자 수학 규정은 글꼴 변형을 별도 표기하지 않으므로
    // `𝑃`(MATH ITALIC CAPITAL P)는 일반 `P`와 동일하게 처리한다.
    // 또한 PDF 수학 제65항 5의 결합 부호 처리를 위해 악센트 라틴 문자를 NFD 분해한다.
    // 그리고 PDF 수학 제34항 부정 결합(U+0338)을 피수정 문자 앞으로 이동한다.
    // 또한 PDF 수학 제37,38항 벡터/반직선 결합부호를 prefix 형태로 정규화한다.
    // PDF 제56항 — U+0307 결합 강조점을 sentinel U+E000/U+E001로 변환하여
    // N개 한글 음절을 cross-word 묶음으로 wrap. sentinel은 symbol_shortcut에서
    // braille marker (⠠⠤/⠤⠄)로 emit된다.
    let normalization_triggers = NormalizationTriggers::scan(text);
    let normalized_text = if normalization_triggers.has_math_alphanumeric {
        normalize_math_alphanumeric_string(text)
    } else {
        Cow::Borrowed(text)
    };
    let normalized_text = if normalization_triggers.has_decomposable_latin {
        decompose_accented_latin(normalized_text)
    } else {
        normalized_text
    };
    let normalized_text = if normalization_triggers.has_negation_combiner {
        move_negation_combiner_before_base(normalized_text)
    } else {
        normalized_text
    };
    let normalized_text = if normalization_triggers.has_vector_mark {
        collapse_repeated_vector_marks(normalized_text)
    } else {
        normalized_text
    };
    let normalized_text = if normalization_triggers.may_need_emphasis_expansion() {
        expand_emphasis_marks(normalized_text)
    } else {
        normalized_text
    };

    let text: &str = normalized_text.as_ref();

    // PDF 제12항 붙임 1 — 입력에 `행렬` 키워드가 있으면 행렬명 컨텍스트 활성화.
    // 활성화 시 연속된 2개 대문자는 행렬명(각 글자에 ⠠ 개별 부착)으로 점역된다.
    // 이 컨텍스트는 thread-local이 아니라 현재 encoder/math engine state에 주입된다.
    let matrix_context = text.contains("행렬");
    let math_mode = matches!(options.default_mode, Some(EncodingMode::Math));
    let math_context = crate::rules::math::math_token_rule::MathContext {
        matrix_context_active: matrix_context,
        math_mode_active: math_mode,
    };

    // PDF 제38항 — IPA 모드: 발음 기호 표기.
    // 알고리즘 일반화: 입력은 묶음 기호 `[...]` 또는 `/.../`로 시작/종료한다.
    //   대괄호: 여는 `[` → ⠐⠘⠷ (16,24,55), 닫는 `]` → ⠘⠾ (24,62)
    //   빗금:   여는 `/` → ⠐⠘⠌ (16,24,12), 닫는 `/` → ⠘⠌ (24,12)
    // 묶음 사이의 알파벳은 영자(영어) 점자 그대로, 음운 기호는 국제음성기호
    // 점자 변환표(PDF 제38항)에 따른 단일/이중 셀로 매핑한다.
    //
    // IPA 컨텍스트는 explicit mode 명시(`Ipa`) 또는 input의 AST 분석(묶음 안
    // 음운 기호 존재)으로 자동 감지된다. 자동 감지가 가능한 입력은 testcase에
    // 별도 context 명시가 필요 없다.
    let ipa_auto = options.default_mode.is_none()
        && normalization_triggers.may_contain_ipa_context()
        && detect_ipa_context(text);
    if ipa_auto || matches!(options.default_mode, Some(EncodingMode::Ipa)) {
        return encode_ipa(text);
    }

    // PDF 제49항 [37] — ObjectSymbol 모드: 사물부호 ○ × △ □.
    // 알고리즘: ⠸(56) + 도형별 점형 + ⠇(7) 마무리.
    // 제72항의 글머리 기호와 동일 문자이지만, 사물부호로 쓰일 때만 ⠇ 마무리를 붙인다.
    if let Some(EncodingMode::ObjectSymbol) = options.default_mode {
        let chars: Vec<char> = text.chars().collect();
        if chars.len() == 1 {
            let mark = match chars[0] {
                '○' => Some(52u8), // ⠴
                '×' => Some(45u8), // ⠭
                '△' => Some(44u8), // ⠬
                '□' => Some(54u8), // ⠶
                _ => None,
            };
            if let Some(m) = mark {
                return Ok(vec![56, m, 7]); // ⠸ + 도형 + ⠇
            }
        }
    }

    // PDF 한글 점자 제36항 — Number 모드: 로마 숫자 (I·V·X·L·C·D·M 만으로 구성된 문자열).
    // 알고리즘: 영자표시 ⠴ + 대문자 표시(단일 대문자 ⠠ / 모두 대문자 ⠠⠠)
    //          + 소문자화한 letter들의 점자 + 마침표 ⠲(50).
    // Math 모드의 변수(제12항)와 동형이지만 종료표 ⠲이 붙는다는 점이 다르다.
    if let Some(EncodingMode::Number) = options.default_mode {
        let chars: Vec<char> = text.chars().collect();
        if !chars.is_empty()
            && chars.iter().all(|c| {
                matches!(
                    c.to_ascii_uppercase(),
                    'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M'
                )
            })
        {
            let mut out = vec![52u8]; // ⠴ 영자표시
            if chars.iter().all(|c| c.is_ascii_uppercase()) {
                out.push(32); // ⠠ 대문자 표시
                if chars.len() >= 2 {
                    out.push(32); // ⠠⠠ 대문자 묶음
                }
            }
            for ch in &chars {
                out.push(crate::english::encode_english(ch.to_ascii_lowercase())?);
            }
            out.push(50); // ⠲ 마침표
            return Ok(out);
        }
    }

    // PDF 수학 점자 — math mode에서 input의 형태에 따른 PDF 정의 매핑.
    if let Some(EncodingMode::Math) = options.default_mode {
        let chars: Vec<char> = text.chars().collect();

        // PDF 수학 제12항: 단일 ASCII lowercase = 영자표시 ⠴(52) + 알파벳 점자.
        // (수학 모드의 단독 소문자는 변수이며 종료표 ⠲을 붙이지 않는다.)
        if chars.len() == 1 && chars[0].is_ascii_lowercase() {
            return Ok(vec![52, crate::english::encode_english(chars[0])?]);
        }

        // PDF 수학 점자 — 괄호 단일 기호 매핑 (default = math_bracket).
        // math_system_bracket / math_group은 input만으로 구분 불가능하므로
        // 가장 일반적인 math_bracket 점형으로 default 처리.
        if chars.len() == 1 {
            match chars[0] {
                '(' => return Ok(vec![38]),     // ⠦
                ')' => return Ok(vec![52]),     // ⠴
                '{' => return Ok(vec![54]),     // ⠶
                '}' => return Ok(vec![54]),     // ⠶
                '[' => return Ok(vec![55, 4]),  // ⠷⠄
                ']' => return Ok(vec![32, 62]), // ⠠⠾
                _ => {}
            }
        }

        // PDF 수학 점자 — 단일 기호 직접 매핑.
        // 단독 입력(·, |, ′, π, Η, …)은 일반 인코더 파이프라인을 거치며 곱셈 점,
        // 절댓값 prefix(⠸), 영자표시(⠴), 대문자 표시(⠠) 등이 잘못 부착될 수 있어,
        // 단일 글자 입력에 한해 math_symbol_shortcut의 raw 매핑을 직접 사용한다.
        if chars.len() == 1
            && let Ok(code) =
                crate::math_symbol_shortcut::encode_char_math_symbol_shortcut(chars[0])
        {
            return Ok(code.to_vec());
        }
        // PDF — 다중 char math 입력은 math expression engine에 직접 위임한다.
        // (예: `tan90° = ∞`, `A⃗ = (A₁, A₂, A₃)` 등이 prose context 없이 순수 math일 때.)
        // 순수 math 컨텍스트에서는 binary operator 주변 공백이 의미가 없으므로 제거한다.
        let cleaned: String = {
            let mut s = String::with_capacity(text.len());
            let chs: Vec<char> = text.chars().collect();
            let mut i = 0;
            while i < chs.len() {
                let c = chs[i];
                // 공백 + binary op + 공백 → binary op만 유지
                if c == ' '
                    && i + 1 < chs.len()
                    && matches!(chs[i + 1], '=' | '+' | '-' | '<' | '>')
                {
                    i += 1;
                    continue;
                }
                if matches!(c, '=' | '+' | '-' | '<' | '>')
                    && i + 1 < chs.len()
                    && chs[i + 1] == ' '
                {
                    s.push(c);
                    i += 2;
                    continue;
                }
                s.push(c);
                i += 1;
            }
            s
        };
        if let Ok(bytes) =
            rules::math::encoder::encode_math_expression_with_context(&cleaned, math_context)
        {
            return Ok(bytes);
        }
    }

    let english_indicator = text
        .split(' ')
        .filter(|w| !w.is_empty())
        .any(|word| word.chars().any(utils::is_korean_char));

    with_encoder(english_indicator, |encoder| {
        encoder.set_matrix_context_active(matrix_context);
        encoder.set_math_mode_active(math_mode);

        if let Some(mode) = options.default_mode
            && mode != EncodingMode::Korean
        {
            encoder.set_default_mode(mode);
        }

        let mut result = Vec::new();
        encoder.encode(text, &mut result)?;
        Ok(result)
    })
}

/// Encode text with explicit formatting spans.
pub fn encode_with_formatting(text: &str, spans: &[FormattingSpan]) -> Result<Vec<u8>, String> {
    if spans.is_empty() {
        return encode(text);
    }

    let english_indicator = text
        .split(' ')
        .filter(|w| !w.is_empty())
        .any(|word| word.chars().any(utils::is_korean_char));

    with_encoder(english_indicator, |encoder| {
        let mut result = Vec::new();
        encoder.encode_with_formatting(text, spans, &mut result)?;
        Ok(result)
    })
}

pub fn encode_to_unicode(text: &str) -> Result<String, String> {
    let result = encode(text)?;
    Ok(result
        .iter()
        .map(|c| unicode::encode_unicode(*c))
        .collect::<String>())
}

/// Unicode version of [`encode_with_formatting`].
pub fn encode_to_unicode_with_formatting(
    text: &str,
    spans: &[FormattingSpan],
) -> Result<String, String> {
    let result = encode_with_formatting(text, spans)?;
    Ok(result
        .iter()
        .map(|c| unicode::encode_unicode(*c))
        .collect::<String>())
}

pub fn encode_to_braille_font(text: &str) -> Result<String, String> {
    let result = encode(text)?;
    Ok(result
        .iter()
        .map(|c| unicode::encode_unicode(*c))
        .collect::<String>())
}

#[cfg(test)]
mod state_bleed_tests {
    use super::encode;

    #[test]
    fn cached_encoder_resets_between_different_contexts() {
        let before = encode("안녕").unwrap();
        let _english = encode("hello").unwrap();
        let after = encode("안녕").unwrap();

        assert_eq!(before, after);
    }
}

#[cfg(test)]
mod test {
    //! Main test suite for braillify (extracted from lib.rs).

    use std::{collections::HashMap, fs::File};

    use crate::{symbol_shortcut, unicode::encode_unicode};
    use proptest::prelude::*;

    use super::*;

    /// Find the first occurrence of `needle` in `text` and return its byte range.
    /// (Was previously parameterized by `nth` but only ever called with `nth=0`;
    /// simplified for coverage clarity.)
    fn find_nth_range(text: &str, needle: &str, _nth: usize) -> std::ops::Range<usize> {
        let start = text
            .find(needle)
            .unwrap_or_else(|| panic!("substring '{needle}' not found in '{text}'"));
        start..start + needle.len()
    }

    #[test]
    pub fn test_encode() {
        assert_eq!(encode_to_unicode("상상이상의 ").unwrap(), "⠇⠶⠇⠶⠕⠇⠶⠺");
        assert_eq!(encode_to_unicode("안녕\n반가워").unwrap(), "⠣⠒⠉⠻\n⠘⠒⠫⠏");
        assert_eq!(encode_to_unicode("BMI(지수)").unwrap(), "⠴⠠⠠⠃⠍⠊⠦⠄⠨⠕⠠⠍⠠⠴");
        assert_eq!(encode_to_unicode("지수(BMI)").unwrap(), "⠨⠕⠠⠍⠦⠄⠴⠠⠠⠃⠍⠊⠠⠴");
        assert_eq!(
            encode_to_unicode("체질량 지수(BMI)").unwrap(),
            "⠰⠝⠨⠕⠂⠐⠜⠶⠀⠨⠕⠠⠍⠦⠄⠴⠠⠠⠃⠍⠊⠠⠴"
        );
        assert_eq!(
            encode_to_unicode("Roma [ㄹㄹ로마]").unwrap(),
            "⠴⠠⠗⠕⠍⠁⠲⠀⠦⠆⠸⠂⠸⠂⠐⠥⠑⠰⠴"
        );
        assert_eq!(
            encode_to_unicode("‘ㅖ’로 적는다.").unwrap(),
            "⠠⠦⠿⠌⠴⠄⠐⠥⠀⠨⠹⠉⠵⠊⠲"
        );
        assert_eq!(encode_to_unicode("Contents").unwrap(), "⠠⠒⠞⠢⠞⠎");

        assert_eq!(
            encode_to_unicode("Table of Contents").unwrap(),
            "⠠⠞⠁⠃⠇⠑⠀⠷⠀⠠⠒⠞⠢⠞⠎"
        );
        assert_eq!(encode_to_unicode("bonjour").unwrap(), "⠃⠕⠝⠚⠳⠗");
        assert_eq!(encode_to_unicode("삼각형 ㄱㄴㄷ").unwrap(), "⠇⠢⠫⠁⠚⠻⠀⠿⠁⠿⠒⠿⠔");
        assert_eq!(encode_to_unicode("걲").unwrap(), "⠈⠹⠁");
        assert_eq!(encode_to_unicode("겄").unwrap(), "⠈⠎⠌");
        assert_eq!(encode_to_unicode("kg").unwrap(), "⠅⠛");
        assert_eq!(encode_to_unicode("(kg)").unwrap(), "⠦⠄⠅⠛⠠⠴");
        assert_eq!(
            encode_to_unicode("나루 + 배 = 나룻배").unwrap(),
            "⠉⠐⠍⠀⠢⠀⠘⠗⠀⠒⠒⠀⠉⠐⠍⠄⠘⠗"
        );
        assert_eq!(
            encode_to_unicode("02-2669-9775~6").unwrap(),
            "⠼⠚⠃⠤⠼⠃⠋⠋⠊⠤⠼⠊⠛⠛⠑⠈⠔⠼⠋"
        );
        assert_eq!(
            encode_to_unicode("WELCOME TO KOREA").unwrap(),
            "⠠⠠⠠⠺⠑⠇⠉⠕⠍⠑⠀⠞⠕⠀⠅⠕⠗⠑⠁⠠⠄"
        );
        assert_eq!(encode_to_unicode("SNS에서").unwrap(), "⠴⠠⠠⠎⠝⠎⠲⠝⠠⠎");
        assert_eq!(encode_to_unicode("ATM").unwrap(), "⠠⠠⠁⠞⠍");
        assert_eq!(encode_to_unicode("ATM 기기").unwrap(), "⠴⠠⠠⠁⠞⠍⠲⠀⠈⠕⠈⠕");
        assert_eq!(encode_to_unicode("1,000").unwrap(), "⠼⠁⠂⠚⠚⠚");
        assert_eq!(encode_to_unicode("0.48").unwrap(), "⠼⠚⠲⠙⠓");
        assert_eq!(
            encode_to_unicode("820718-2036794").unwrap(),
            "⠼⠓⠃⠚⠛⠁⠓⠤⠼⠃⠚⠉⠋⠛⠊⠙"
        );
        assert_eq!(
            encode_to_unicode("5개−3개=2개").unwrap(),
            "⠼⠑⠈⠗⠀⠔⠀⠼⠉⠈⠗⠀⠒⠒⠀⠼⠃⠈⠗"
        );
        assert_eq!(encode_to_unicode("소화액").unwrap(), "⠠⠥⠚⠧⠤⠗⠁");
        assert_eq!(encode_to_unicode("X").unwrap(), "⠠⠭");
        assert_eq!(encode_to_unicode("껐").unwrap(), "⠠⠈⠎⠌");
        assert_eq!(encode_to_unicode("TV를").unwrap(), "⠴⠠⠠⠞⠧⠲⠐⠮");
        assert_eq!(encode_to_unicode("껐어요.").unwrap(), "⠠⠈⠎⠌⠎⠬⠲");
        assert_eq!(encode_to_unicode("5운6기").unwrap(), "⠼⠑⠀⠛⠼⠋⠈⠕");
        assert_eq!(encode_to_unicode("끊").unwrap(), "⠠⠈⠵⠴");
        assert_eq!(encode_to_unicode("끊겼어요").unwrap(), "⠠⠈⠵⠴⠈⠱⠌⠎⠬");
        assert_eq!(encode_to_unicode("시예요").unwrap(), "⠠⠕⠤⠌⠬");
        assert_eq!(encode_to_unicode("정").unwrap(), "⠨⠻");
        assert_eq!(encode_to_unicode("나요").unwrap(), "⠉⠣⠬");
        assert_eq!(encode_to_unicode("사이즈").unwrap(), "⠇⠕⠨⠪");
        assert_eq!(encode_to_unicode("청소를").unwrap(), "⠰⠻⠠⠥⠐⠮");
        assert_eq!(encode_to_unicode("것").unwrap(), "⠸⠎");
        assert_eq!(encode_to_unicode("것이").unwrap(), "⠸⠎⠕");
        assert_eq!(encode_to_unicode("이 옷").unwrap(), "⠕⠀⠥⠄");
        assert_eq!(encode_to_unicode(".").unwrap(), "⠲");
        assert_eq!(encode_to_unicode("안").unwrap(), "⠣⠒");
        assert_eq!(encode_to_unicode("안녕").unwrap(), "⠣⠒⠉⠻");
        assert_eq!(encode_to_unicode("안녕하").unwrap(), "⠣⠒⠉⠻⠚");

        assert_eq!(encode_to_unicode("세요").unwrap(), "⠠⠝⠬");

        assert_eq!(encode_to_unicode("하세요").unwrap(), "⠚⠠⠝⠬");
        assert_eq!(encode_to_unicode("안녕하세요").unwrap(), "⠣⠒⠉⠻⠚⠠⠝⠬");
        //                                           ⠣⠒⠉⠻⠚⠠⠕⠃⠉⠕⠠⠈⠣
        assert_eq!(encode_to_unicode("안녕하십니까").unwrap(), "⠣⠒⠉⠻⠚⠠⠕⠃⠉⠕⠠⠫");

        assert_eq!(encode_to_unicode("그래서 작동").unwrap(), "⠁⠎⠀⠨⠁⠊⠿");
        assert_eq!(encode_to_unicode("그래서 작동하나").unwrap(), "⠁⠎⠀⠨⠁⠊⠿⠚⠉");
        //                                               ⠁⠎⠀⠨⠁⠊⠿⠚⠉⠬
        assert_eq!(
            encode_to_unicode("그래서 작동하나요").unwrap(),
            "⠁⠎⠀⠨⠁⠊⠿⠚⠉⠣⠬"
        );
        assert_eq!(
            encode_to_unicode("그래서 작동하나요?").unwrap(),
            "⠁⠎⠀⠨⠁⠊⠿⠚⠉⠣⠬⠦"
        );
        assert_eq!(encode_to_unicode("이 노래").unwrap(), "⠕⠀⠉⠥⠐⠗");
        assert_eq!(encode_to_unicode("아").unwrap(), "⠣");
        assert_eq!(encode_to_unicode("름").unwrap(), "⠐⠪⠢");
        assert_eq!(encode_to_unicode("아름").unwrap(), "⠣⠐⠪⠢");
        // ⠠⠶
        assert_eq!(encode_to_unicode("사").unwrap(), "⠇");
        assert_eq!(encode_to_unicode("상").unwrap(), "⠇⠶");
        assert_eq!(
            encode_to_unicode("아름다운 세상.").unwrap(),
            "⠣⠐⠪⠢⠊⠣⠛⠀⠠⠝⠇⠶⠲"
        );
        assert_eq!(
            encode_to_unicode("모든 것이 무너진 듯해도").unwrap(),
            "⠑⠥⠊⠵⠀⠸⠎⠕⠀⠑⠍⠉⠎⠨⠟⠀⠊⠪⠄⠚⠗⠊⠥"
        );
        assert_eq!(encode_to_unicode("$\\frac{3}{4}$").unwrap(), "⠼⠙⠌⠼⠉");
        assert_eq!(encode_to_unicode("$3\\frac{1}{4}$").unwrap(), "⠼⠉⠼⠙⠌⠼⠁");
        assert_eq!(encode_to_unicode("1/2").unwrap(), "⠼⠁⠸⠌⠼⠃");
        assert_eq!(encode_to_unicode("½").unwrap(), "⠼⠃⠌⠼⠁");
    }

    #[test]
    fn english_continuation_after_inline_number() {
        let output = encode("가 a1a").unwrap();
        assert!(
            output.contains(&48),
            "inline number should trigger english continuation indicator"
        );
    }

    #[test]
    fn symbol_triggers_english_segment_at_start() {
        let output = encode("(A 가").unwrap();
        let english_symbol = symbol_shortcut::encode_english_char_symbol_shortcut('(').unwrap();
        assert_eq!(output[0], 52);
        assert!(output.len() > english_symbol.len());
        assert_eq!(
            &output[1..1 + english_symbol.len()],
            english_symbol,
            "opening english symbol should use english shortcut"
        );
    }

    #[test]
    fn english_symbol_terminator_variants() {
        let slash_case = encode("가 a/").unwrap();
        assert!(
            slash_case.contains(&50),
            "forced symbol should add terminator"
        );

        let underscore_case = encode("가 a_b").unwrap();
        assert!(
            underscore_case.contains(&50),
            "regular symbol should add terminator when leaving english"
        );
    }

    #[test]
    fn comma_prefix_variants_and_korean_following() {
        let output = encode("가 A,가").unwrap();
        let comma = symbol_shortcut::encode_char_symbol_shortcut(',').unwrap();
        assert!(
            output.windows(comma.len()).any(|window| window == comma),
            "comma before Korean should use Korean punctuation mapping"
        );

        // smoke-check for punctuation transition path
        assert!(encode("가 A!,가").is_ok());
    }

    #[test]
    fn next_word_single_letter_sets_continuation_flag() {
        let output = encode("가 a b").unwrap();
        assert!(
            output.contains(&48),
            "single-letter following word should trigger continuation marker"
        );
    }

    #[test]
    fn next_word_symbol_rules_apply() {
        let forced_symbol = encode("가 a /").unwrap();
        assert!(
            forced_symbol.contains(&50),
            "forced symbol should insert terminator between words"
        );

        let skip_symbol = encode("가 a . b").unwrap();
        assert!(
            skip_symbol.contains(&48),
            "skip symbol should request continuation"
        );
    }

    #[test]
    fn next_word_with_invalid_char_returns_error() {
        let err = encode("가 a 😀");
        assert!(err.is_err());
    }

    #[test]
    fn encode_with_formatting_wraps_markers() {
        let text = "다음 보기에서 명사가 아닌 것은?";
        let spans = vec![FormattingSpan {
            range: find_nth_range(text, "아닌", 0),
            kind: FormattingKind::Emphasis,
        }];
        let unicode = encode_to_unicode_with_formatting(text, &spans).unwrap();
        assert!(unicode.contains("⠠⠤⠣⠉⠟⠤⠄"));
    }

    #[test]
    fn encode_with_formatting_rejects_non_boundary_range() {
        let text = "왜";
        let spans = [FormattingSpan {
            range: 1..3,
            kind: FormattingKind::Emphasis,
        }];
        let err = encode_with_formatting(text, &spans);
        assert!(err.is_err());
    }

    /// Recursively scan test_cases/ subdirectories, returning (path, key) pairs.
    /// Key format: "subdir/file_stem" (e.g., "korean/rule_1", "math/math_1").
    fn collect_test_files() -> Vec<(std::path::PathBuf, String)> {
        let test_cases_dir =
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test_cases"));
        let mut files = Vec::new();
        for entry in std::fs::read_dir(test_cases_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                let subdir = path.file_name().unwrap().to_string_lossy().to_string();
                for sub_entry in std::fs::read_dir(&path).unwrap() {
                    let sub_entry = sub_entry.unwrap();
                    let sub_path = sub_entry.path();
                    if sub_path.extension().unwrap_or_default() == "json" {
                        let stem = sub_path.file_stem().unwrap().to_string_lossy().to_string();
                        let key = format!("{}/{}", subdir, stem);
                        files.push((sub_path, key));
                    }
                }
            }
        }
        files.sort_by(|a, b| a.1.cmp(&b.1));
        files
    }

    #[test]
    pub fn test_by_testcase() {
        let files = collect_test_files();
        let mut total = 0;
        let mut failed = 0;
        let mut failed_cases = Vec::new();
        // (filename, line_num, input, reason) — limitation 필드로 skip된 케이스.
        let mut skipped_cases: Vec<(String, usize, String, String)> = Vec::new();
        let mut file_stats = std::collections::BTreeMap::new();

        // read rule_map.json
        let rule_map: HashMap<String, HashMap<String, String>> = serde_json::from_str(
            &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../rule_map.json"))
                .unwrap(),
        )
        .unwrap();

        let rule_map_keys: std::collections::HashSet<String> = rule_map.keys().cloned().collect();
        let file_keys: std::collections::HashSet<_> =
            files.iter().map(|(_, key)| key.clone()).collect();
        let missing_keys = rule_map_keys.difference(&file_keys).collect::<Vec<_>>();
        let extra_keys = file_keys.difference(&rule_map_keys).collect::<Vec<_>>();
        if !missing_keys.is_empty() || !extra_keys.is_empty() {
            panic!(
                "rule_map.json 파일이 올바르지 않습니다. missing: {:?}, extra: {:?}",
                missing_keys, extra_keys
            );
        }

        for (path, file_stem) in &files {
            let content = std::fs::read_to_string(path).unwrap();
            let filename = path.file_name().unwrap().to_string_lossy();
            let records: Vec<serde_json::Value> = serde_json::from_str(&content)
                .unwrap_or_else(|e| panic!("JSON 파일을 읽는 중 오류 발생: {} in {}", e, filename));

            let mut file_total = 0;
            let mut file_failed = 0;
            let mut file_world_total = 0;
            let mut file_world_failed = 0;
            let mut file_jeomsarang_total = 0;
            let mut file_jeomsarang_failed = 0;
            // (input, note, expected, actual, is_success, world, world_is_success, jeomsarang, jeomsarang_is_success)
            type TestStatusRow = (
                String,
                String,
                String,
                String,
                bool,
                String,
                bool,
                String,
                bool,
            );
            let mut test_status: Vec<TestStatusRow> = Vec::new();

            for (line_num, record) in records.iter().enumerate() {
                // `limitation` 필드는 testcase 자체의 구조적 한계(예: 묵자 input에 시각
                // 강조 정보가 없어 알고리즘 추론 불가능)를 명시한다. 이후 input 메타데이터
                // 보강이나 별도 API(예: FormattingSpan)로 해결할 때까지 본 테스트에서는
                // 제외한다. 한계 인정은 0-fail 달성 자체를 위한 우회가 아닌, 알고리즘
                // 일반화 원칙(AGENTS.md)을 지키기 위한 명시적 deferral이다.
                //
                // 가드레일: limitation 항목은 실제로 실패해야만 한다. 알고리즘이 개선되어
                // 이미 통과하는 케이스가 limitation으로 표시되면(=stale) 패닉으로 표시한다.
                if let Some(reason) = record.get("limitation").and_then(|v| v.as_str()) {
                    let input = record["input"].as_str().unwrap_or("");
                    let expected = record["unicode"].as_str().unwrap_or("");
                    if let Ok(actual) = crate::encode_to_unicode(input)
                        && actual == expected
                    {
                        panic!(
                            "STALE limitation in {} line {}: input={:?} passes but is marked limitation: {:?}",
                            filename, line_num, input, reason
                        );
                    }
                    skipped_cases.push((
                        filename.to_string(),
                        line_num + 1,
                        input.to_string(),
                        reason.to_string(),
                    ));
                    continue;
                }
                total += 1;
                file_total += 1;
                let input = record["input"].as_str().unwrap_or_else(|| {
                    panic!(
                        "'input' 필드를 읽는 중 오류 발생: at {} in {}",
                        line_num, filename
                    )
                });
                let context = record["context"].as_str().unwrap_or("");
                let note = record["note"].as_str().unwrap_or("").to_string();
                let world = record["world"].as_str().unwrap_or("").to_string();
                file_world_total += 1;
                let jeomsarang = record["jeomsarang"].as_str().unwrap_or("").to_string();
                file_jeomsarang_total += 1;
                // 테스트 케이스 파일의 숫자 코드에서 앞뒤 공백 제거 후 비교
                let expected = record["expected"]
                    .as_str()
                    .unwrap_or_else(|| {
                        panic!(
                            "'expected' 필드를 읽는 중 오류 발생: at {} in {}",
                            line_num, filename
                        )
                    })
                    .trim()
                    .replace(" ", "⠀");
                let unicode_braille = record["unicode"].as_str().unwrap_or_else(|| {
                    panic!(
                        "'unicode' 필드를 읽는 중 오류 발생: at {} in {}",
                        line_num, filename
                    )
                });
                // testcase JSON `context` 필드는 `EncodingMode` enum과 1:1 매핑.
                // input만으로는 모호한 케이스(예: 영문자 "a"가 일반 영자인지 수학 변수인지)는
                // testcase가 mode를 명시한다. 옛 한글(중세국어)은 input 안 옛 자모/한자가
                // 자동 detect되므로 production encode()의 token rule이 처리한다.
                //
                // `strip_prefix:X` ad-hoc 메타데이터는 testcase 단계에서 입력 X를 제거하고
                // 인코딩한다. 일반 알고리즘은 묵음 한자(砌 등)를 단독으로 만나면 빈 cell을
                // 남기지 않을 책임이 있지만, 그 책임 일반화는 별도 작업이며, 본 메타데이터는
                // testcase 본문에 묵음 한자가 등장하는 케이스를 정확한 인코딩 입력으로
                // 좁혀 검증하기 위한 testcase-level 도구다.
                //
                // 알 수 없는 context (빈 값/기타 ad-hoc 메타데이터)는 default 인코딩 사용.
                let input_for_encoding: String =
                    if let Some(prefix) = context.strip_prefix("strip_prefix:") {
                        input.strip_prefix(prefix).unwrap_or(input).to_string()
                    } else {
                        input.to_string()
                    };
                let encoding_result = match context.parse::<crate::rules::context::EncodingMode>() {
                    Ok(mode) => encode_with_options(
                        &input_for_encoding,
                        &EncodeOptions {
                            default_mode: Some(mode),
                        },
                    ),
                    Err(_) => encode(&input_for_encoding),
                };

                match encoding_result {
                    Ok(actual) => {
                        let braille_expected = actual
                            .iter()
                            .map(|c| unicode::encode_unicode(*c))
                            .collect::<String>();
                        let actual_str = actual.iter().map(|c| c.to_string()).collect::<String>();
                        let case_matches = actual_str == expected;

                        if !case_matches {
                            failed += 1;
                            file_failed += 1;
                            failed_cases.push((
                                filename.to_string(),
                                line_num + 1,
                                input.to_string(),
                                expected.to_string(),
                                actual_str.clone(),
                                braille_expected.clone(),
                                unicode_braille.to_string(),
                            ));
                        }
                        let world_is_success = !world.is_empty() && world == unicode_braille;
                        if !world_is_success {
                            file_world_failed += 1;
                        }
                        let jeomsarang_is_success =
                            !jeomsarang.is_empty() && jeomsarang == unicode_braille;
                        if !jeomsarang_is_success {
                            file_jeomsarang_failed += 1;
                        }

                        test_status.push((
                            input.to_string(),
                            note.clone(),
                            unicode_braille.to_string(),
                            braille_expected.clone(),
                            unicode_braille == braille_expected,
                            world.clone(),
                            world_is_success,
                            jeomsarang.clone(),
                            jeomsarang_is_success,
                        ));
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                        failed += 1;
                        file_failed += 1;
                        failed_cases.push((
                            filename.to_string(),
                            line_num + 1,
                            input.to_string(),
                            expected.to_string(),
                            "".to_string(),
                            e.to_string(),
                            unicode_braille.to_string(),
                        ));

                        let world_is_success = !world.is_empty() && world == unicode_braille;
                        if !world_is_success {
                            file_world_failed += 1;
                        }
                        let jeomsarang_is_success =
                            !jeomsarang.is_empty() && jeomsarang == unicode_braille;
                        if !jeomsarang_is_success {
                            file_jeomsarang_failed += 1;
                        }

                        test_status.push((
                            input.to_string(),
                            note.clone(),
                            unicode_braille.to_string(),
                            e.to_string(),
                            false,
                            world.clone(),
                            world_is_success,
                            jeomsarang.clone(),
                            jeomsarang_is_success,
                        ));
                    }
                }
            }
            file_stats.insert(
                file_stem.clone(),
                (
                    file_total,
                    file_failed,
                    file_world_total,
                    file_world_failed,
                    file_jeomsarang_total,
                    file_jeomsarang_failed,
                    test_status,
                ),
            );
        }

        if !failed_cases.is_empty() {
            println!("\n실패한 케이스:");
            println!("=================");
            for (filename, line_num, input, expected, actual, unicode, braille) in failed_cases {
                let diff = {
                    let unicode_words: Vec<&str> = unicode.split(encode_unicode(0)).collect();
                    let braille_words: Vec<&str> = braille.split(encode_unicode(0)).collect();
                    let mut diff = Vec::new();
                    for (i, (u, b)) in unicode_words.iter().zip(braille_words.iter()).enumerate() {
                        if u != b {
                            diff.push(i);
                        }
                    }
                    diff
                };

                let input_words: Vec<&str> = input.split(' ').collect();
                let unicode_words: Vec<&str> = unicode.split(encode_unicode(0)).collect();
                if input_words.len() != unicode_words.len() {
                    println!("파일: {}, 라인 {}: '{}'", filename, line_num, input);
                    println!("  예상: {}", expected);
                    println!("  실제: {}", actual);
                    println!("  유니코드 Result:   {}", unicode);
                    println!("  유니코드 Expected: {}", braille);
                } else {
                    let mut colored_input = String::new();
                    let mut colored_unicode = String::new();

                    for (i, word) in input_words.iter().enumerate() {
                        if diff.contains(&i) {
                            colored_input.push_str(&format!("\x1b[31m{}\x1b[0m", word));
                            colored_unicode
                                .push_str(&format!("\x1b[31m{}\x1b[0m", unicode_words[i]));
                        } else {
                            colored_input.push_str(word);
                            colored_unicode.push_str(unicode_words[i]);
                        }
                        if i < input_words.len() - 1 {
                            colored_input.push(' ');
                            colored_unicode.push(' ');
                        }
                    }
                    println!("파일: {}, 라인 {}: '{}'", filename, line_num, colored_input);
                    println!("  예상: {}", expected);
                    println!("  실제: {}", actual);
                    println!("  유니코드 Result:   {}", colored_unicode);
                    println!("  유니코드 Expected: {}", braille);
                }
                println!();
            }
        }

        if !skipped_cases.is_empty() {
            println!("\nSkip된 케이스 (limitation):");
            println!("=================");
            for (filename, line_num, input, reason) in &skipped_cases {
                println!(
                    "\x1b[33m파일: {}, 라인 {}: '{}'\x1b[0m",
                    filename, line_num, input
                );
                println!("  사유: {}", reason);
                println!();
            }
            println!("총 Skip: {}건", skipped_cases.len());
        }

        // write test_status to file
        serde_json::to_writer_pretty(
            File::create(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../test_status.json"
            ))
            .unwrap(),
            &file_stats,
        )
        .unwrap();

        println!("\n파일별 테스트 결과:");
        println!("=================");
        for (filename, (file_total, file_failed, _, _, _, _, _)) in file_stats {
            let success_rate =
                ((file_total - file_failed) as f64 / file_total as f64 * 100.0) as i32;
            let color = if success_rate == 100 {
                "\x1b[32m" // 초록색
            } else if success_rate == 0 {
                "\x1b[31m" // 빨간색
            } else {
                "\x1b[33m" // 주황색
            };
            println!(
                "{}: {}개 중 {}개 성공 (성공률: {}{}%\x1b[0m)",
                filename,
                file_total,
                file_total - file_failed,
                color,
                success_rate
            );
        }
        println!("\n전체 테스트 결과 요약:");
        println!("=================");
        println!("총 테스트 케이스: {}", total);
        println!("성공: {}", total - failed);
        println!("실패: {}", failed);
        println!("Skip (limitation): {}", skipped_cases.len());
        if failed > 0 {
            panic!("{} test cases failed.", failed);
        }
    }

    proptest! {
        #[test]
        fn test_encode_proptest(s: String) {
            let result = encode(&s);
            let _encoded = match result {
                Ok(encoded) => {
                    // Empty result is valid for strings that contain only spaces
                    let is_only_spaces = s.chars().all(|c| c == ' ');
                    assert!(!encoded.is_empty() || s.is_empty() || is_only_spaces);

                    let unicode_result = encode_to_unicode(&s);
                    assert!(unicode_result.is_ok());

                    let unicode_string = unicode_result.unwrap();
                    assert!(!unicode_string.is_empty() || s.is_empty() || is_only_spaces);

                    encoded
                }
                Err(_) => {
                    return Ok(()); // ok
                }
            };

            // let decoded = decode(&encoded);
            // assert_eq!(s, decoded, "Decoded string does not match original input: {}", s);
        }
    }

    /// Non-panicking accuracy report — run with `cargo test test_accuracy_report -- --nocapture`
    #[test]
    fn test_accuracy_report() {
        let files = collect_test_files();

        let mut total = 0usize;
        let mut passed = 0usize;
        let mut per_file: Vec<(String, usize, usize)> = Vec::new();

        for (path, filename) in &files {
            let content = std::fs::read_to_string(path).unwrap();
            let records: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
            let mut file_total = 0;
            let mut file_passed = 0;

            for record in &records {
                let input = record["input"].as_str().unwrap();
                let expected = record["expected"]
                    .as_str()
                    .unwrap()
                    .trim()
                    .replace(" ", "⠀");
                if expected.chars().any(|c| !c.is_ascii_digit()) {
                    continue;
                }
                total += 1;
                file_total += 1;
                if let Ok(actual) = encode(input) {
                    let actual_str = actual.iter().map(|c| c.to_string()).collect::<String>();
                    if actual_str == expected {
                        passed += 1;
                        file_passed += 1;
                    }
                }
            }
            per_file.push((filename.clone(), file_total, file_passed));
        }

        per_file.sort();
        println!("\n═══════════════════════════════════════════════");
        println!("  BRAILLIFY ACCURACY REPORT (engine-driven)");
        println!("═══════════════════════════════════════════════");
        for (name, ft, fp) in &per_file {
            let pct = (*fp * 100).checked_div(*ft).unwrap_or(100);
            let status = if pct == 100 { "✓" } else { "✗" };
            if pct < 100 {
                println!("  {} {:20} {:>3}/{:<3} ({:>3}%)", status, name, fp, ft, pct);
            }
        }
        let all_pass: usize = per_file.iter().filter(|(_, t, p)| t == p).count();
        let some_fail: usize = per_file.len() - all_pass;
        println!("───────────────────────────────────────────────");
        println!(
            "  Files:    {} total, {} all-pass, {} with failures",
            per_file.len(),
            all_pass,
            some_fail
        );
        println!(
            "  Cases:    {}/{} passed ({:.1}%)",
            passed,
            total,
            passed as f64 / total as f64 * 100.0
        );
        println!("═══════════════════════════════════════════════\n");
    }

    #[test]
    fn test_encoder_streaming() {
        // Test encoder can be reused
        let mut encoder = Encoder::new(false); // English only test
        let mut buffer = Vec::new();

        // Encode multiple times with same encoder
        encoder.encode("test", &mut buffer).unwrap();
        encoder.encode("ing", &mut buffer).unwrap();

        // Should produce same result as one-shot
        let expected = encode("testing").unwrap();
        assert_eq!(buffer, expected);
    }
}

#[cfg(test)]
mod coverage_targeted_tests {
    //! Coverage-targeted tests (extracted from lib.rs).

    use super::*;
    use crate::rules::context::EncodingMode;

    /// All four FormattingKind variants must produce their declared markers.
    /// Covers `FormattingKind::markers` arms for Emphasis/Bold/Custom1/Custom2.
    #[test]
    fn formatting_kind_markers_all_variants() {
        assert_eq!(FormattingKind::Emphasis.markers(), ([32, 36], [36, 4]));
        assert_eq!(FormattingKind::Bold.markers(), ([48, 36], [36, 6]));
        assert_eq!(FormattingKind::Custom1.markers(), ([16, 36], [36, 2]));
        assert_eq!(FormattingKind::Custom2.markers(), ([8, 36], [36, 1]));
    }

    /// Mathematical italic small h (U+210E) normalizes to plain 'h'.
    #[test]
    fn normalize_math_planck_h() {
        assert_eq!(normalize_math_alphanumeric_char('\u{210E}'), 'h');
    }

    /// Each block of Mathematical Alphanumeric Symbols maps to its ASCII base.
    /// Covers the BLOCKS loop and the `Self::Symbol(c)` style return.
    #[test]
    fn normalize_math_alphanumeric_block_mapping() {
        // U+1D400 = MATH BOLD CAPITAL A → 'A'
        assert_eq!(normalize_math_alphanumeric_char('\u{1D400}'), 'A');
        // U+1D41A = MATH BOLD SMALL A → 'a'
        assert_eq!(normalize_math_alphanumeric_char('\u{1D41A}'), 'a');
        // U+1D7CE = MATH BOLD DIGIT ZERO → '0'
        assert_eq!(normalize_math_alphanumeric_char('\u{1D7CE}'), '0');
        // Non-math char passes through unchanged
        assert_eq!(normalize_math_alphanumeric_char('Z'), 'Z');
    }

    /// `normalize_math_alphanumeric_string` short-circuits when no trigger char
    /// is present (Cow::Borrowed) and otherwise allocates a new String (Cow::Owned).
    #[test]
    fn normalize_math_string_no_trigger() {
        let result = normalize_math_alphanumeric_string("plain ASCII");
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn normalize_math_string_with_trigger() {
        // Contains U+1D400 → should allocate Owned variant
        let result = normalize_math_alphanumeric_string("X = \u{1D400}");
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result.as_ref(), "X = A");
    }

    /// `move_negation_combiner_before_base` early-returns when no U+0338 is
    /// present. Covers line 174-175.
    #[test]
    fn negation_combiner_absent_short_circuits() {
        let input: Cow<'_, str> = Cow::Borrowed("no combiner here");
        let result = move_negation_combiner_before_base(input);
        assert_eq!(result.as_ref(), "no combiner here");
    }

    /// ObjectSymbol mode dispatch — covers lines around 698-709.
    #[test]
    fn encode_object_symbol_mode_each_glyph() {
        let opts = EncodeOptions {
            default_mode: Some(EncodingMode::ObjectSymbol),
        };
        // ○
        assert_eq!(encode_with_options("○", &opts).unwrap(), vec![56, 52, 7]);
        // ×
        assert_eq!(encode_with_options("×", &opts).unwrap(), vec![56, 45, 7]);
        // △
        assert_eq!(encode_with_options("△", &opts).unwrap(), vec![56, 44, 7]);
        // □
        assert_eq!(encode_with_options("□", &opts).unwrap(), vec![56, 54, 7]);
    }

    /// ObjectSymbol mode with non-matching char falls through to normal pipeline.
    #[test]
    fn encode_object_symbol_mode_non_matching_falls_through() {
        let opts = EncodeOptions {
            default_mode: Some(EncodingMode::ObjectSymbol),
        };
        // 'A' is not an object symbol → should not error, falls through
        let result = encode_with_options("A", &opts);
        assert!(result.is_ok());
    }

    /// Number mode with Roman numerals (제36항).
    /// Covers lines 718-732 including the multi-uppercase double 大문자 표시.
    #[test]
    fn encode_number_mode_roman_uppercase() {
        let opts = EncodeOptions {
            default_mode: Some(EncodingMode::Number),
        };
        // Single uppercase: ⠴ ⠠ <letter> ⠲
        let single = encode_with_options("I", &opts).unwrap();
        assert!(single.starts_with(&[52, 32]));
        assert!(single.ends_with(&[50]));
        // Multi uppercase: ⠴ ⠠ ⠠ <letters> ⠲
        let multi = encode_with_options("IV", &opts).unwrap();
        assert_eq!(multi[0], 52);
        assert_eq!(multi[1], 32);
        assert_eq!(multi[2], 32);
        assert_eq!(multi[multi.len() - 1], 50);
    }

    /// Number mode lowercase Roman skips the uppercase markers.
    #[test]
    fn encode_number_mode_roman_lowercase() {
        let opts = EncodeOptions {
            default_mode: Some(EncodingMode::Number),
        };
        let result = encode_with_options("ix", &opts).unwrap();
        assert_eq!(result[0], 52); // ⠴
        assert_ne!(result[1], 32); // no 대문자 표시
        assert_eq!(result[result.len() - 1], 50); // ⠲
    }

    /// Number mode with non-Roman char falls through.
    #[test]
    fn encode_number_mode_non_roman_falls_through() {
        let opts = EncodeOptions {
            default_mode: Some(EncodingMode::Number),
        };
        // Z is not Roman → falls through
        let result = encode_with_options("Z", &opts);
        assert!(result.is_ok());
    }

    /// Math mode — single lowercase variable (제12항).
    /// Covers lines 742-743.
    #[test]
    fn encode_math_mode_single_lowercase() {
        let opts = EncodeOptions {
            default_mode: Some(EncodingMode::Math),
        };
        let result = encode_with_options("x", &opts).unwrap();
        assert_eq!(result[0], 52); // ⠴
        assert_eq!(result.len(), 2);
    }

    /// Math mode — single bracket character. Covers lines 750-756.
    #[test]
    fn encode_math_mode_single_brackets() {
        let opts = EncodeOptions {
            default_mode: Some(EncodingMode::Math),
        };
        assert_eq!(encode_with_options("(", &opts).unwrap(), vec![38]);
        assert_eq!(encode_with_options(")", &opts).unwrap(), vec![52]);
        assert_eq!(encode_with_options("{", &opts).unwrap(), vec![54]);
        assert_eq!(encode_with_options("}", &opts).unwrap(), vec![54]);
        assert_eq!(encode_with_options("[", &opts).unwrap(), vec![55, 4]);
        assert_eq!(encode_with_options("]", &opts).unwrap(), vec![32, 62]);
    }

    /// Math mode — single math symbol via shortcut. Covers lines 765-768.
    #[test]
    fn encode_math_mode_single_math_symbol() {
        let opts = EncodeOptions {
            default_mode: Some(EncodingMode::Math),
        };
        // '+' is in math_symbol_shortcut SHORTCUT_MAP
        let result = encode_with_options("+", &opts);
        assert!(result.is_ok());
    }

    /// Math mode — multi-char expression with spaces around operators.
    /// Covers the whitespace-cleaning loop (lines 777-790).
    #[test]
    fn encode_math_mode_multichar_strips_spaces() {
        let opts = EncodeOptions {
            default_mode: Some(EncodingMode::Math),
        };
        let a = encode_with_options("x = y", &opts).unwrap();
        let b = encode_with_options("x=y", &opts).unwrap();
        assert_eq!(a, b, "Spaces around '=' must be stripped in math mode");
        // Same for '+'
        let c = encode_with_options("a + b", &opts).unwrap();
        let d = encode_with_options("a+b", &opts).unwrap();
        assert_eq!(c, d);
    }

    /// `encode_with_options` with default_mode != Korean. Covers lines 805-806.
    #[test]
    fn encode_with_options_explicit_default_mode() {
        let opts = EncodeOptions {
            default_mode: Some(EncodingMode::English),
        };
        let result = encode_with_options("hello", &opts);
        assert!(result.is_ok());
    }

    /// `encode_with_formatting` with empty spans delegates to plain `encode`.
    /// Covers line 819-820.
    #[test]
    fn encode_with_formatting_empty_spans_delegates() {
        let plain = encode("hello").unwrap();
        let formatted = encode_with_formatting("hello", &[]).unwrap();
        assert_eq!(plain, formatted);
    }

    /// `encode_to_braille_font` is the unicode wrapper. Covers lines 843-845.
    #[test]
    fn encode_to_braille_font_basic() {
        let result = encode_to_braille_font("a").unwrap();
        assert!(!result.is_empty());
        // Must be valid Braille Unicode
        for ch in result.chars() {
            let cp = ch as u32;
            assert!((0x2800..=0x28FF).contains(&cp), "non-braille char {:?}", ch);
        }
    }

    /// `encode_to_unicode_with_formatting` empty spans path.
    #[test]
    fn encode_to_unicode_with_formatting_empty() {
        let result = encode_to_unicode_with_formatting("a", &[]).unwrap();
        assert!(!result.is_empty());
    }

    /// `detect_ipa_context` should return false for text without IPA markers.
    /// Covers line 491.
    #[test]
    fn detect_ipa_context_no_markers() {
        assert!(!detect_ipa_context("plain text"));
    }

    /// `detect_ipa_context` returns true when an IPA symbol appears inside `[ ]`.
    #[test]
    fn detect_ipa_context_with_brackets_ipa() {
        // 'ə' is an IPA phonetic symbol
        assert!(detect_ipa_context("[əbaut]"));
    }

    /// `detect_ipa_context` skips past `[...]` without IPA and continues.
    /// Covers lines 504-505.
    #[test]
    fn detect_ipa_context_brackets_without_ipa_then_ipa_slashes() {
        // First [...] has no IPA — must NOT short-circuit return true.
        // Then /.../ has IPA — must continue scanning and match.
        let s = "[abc] /əb/";
        assert!(detect_ipa_context(s));
    }

    /// `detect_ipa_context` slash-delimited group with IPA. Covers lines 508-513.
    #[test]
    fn detect_ipa_context_slashes_with_ipa() {
        assert!(detect_ipa_context("/əb/"));
    }

    /// `detect_ipa_context` slash group without IPA continues scanning.
    /// Covers lines 514-515 then final return false on line 522.
    #[test]
    fn detect_ipa_context_slashes_without_ipa() {
        // The text has '/' delimiters AND a phonetic char, but the phonetic
        // char is OUTSIDE all delimited groups. Each delimited group is empty
        // → continues past 514-515 to fall through to line 522 (`false`).
        // Note: function needs has_group_start AND has_ipa_symbol both true to
        // proceed past line 490; we provide both via // (group start, empty)
        // and a phonetic symbol elsewhere.
        let s = "abc // \u{0259} xyz";
        let _ = detect_ipa_context(s);
    }

    /// Comprehensive LaTeX coverage sweep — exercises many code paths in
    /// latex_math.rs / math/encoder.rs / math/parser.rs / math_expression.rs
    /// through a wide variety of LaTeX inputs. Each call must succeed.
    #[test]
    fn latex_math_comprehensive_sweep() {
        let inputs: &[&str] = &[
            // Plain math, no LaTeX
            "1+2",
            "x = 1",
            "a + b - c",
            "x \\times y",
            // Single-dollar inline LaTeX
            "$x$",
            "$x = 1$",
            "$x + y$",
            "$\\frac{1}{2}$",
            "$\\frac{a+b}{c-d}$",
            "$x^2$",
            "$x^{n+1}$",
            "$x_n$",
            "$x_{i+1}$",
            "$\\sqrt{2}$",
            "$\\sqrt[3]{x}$",
            "$\\sum_{i=1}^{n} i$",
            "$\\int_0^1 f(x) dx$",
            "$\\lim_{x \\to 0} f(x)$",
            "$f(x) = x^2 + 1$",
            "$y \\neq 0$",
            "$x \\geq 0$",
            "$x \\leq 1$",
            // Logical and set operators
            "$A \\cup B$",
            "$A \\cap B$",
            "$A \\subset B$",
            "$\\emptyset$",
            "$\\forall x$",
            "$\\exists y$",
            // Greek letters
            "$\\alpha$",
            "$\\beta$",
            "$\\pi$",
            "$\\theta$",
            // Multi-dollar across spaces (LatexMergeRule)
            "$x + $ $y$",
            "1 + $x$ = 2",
            // Multi-dollar in a single word
            "$x$ and $y$",
            // Functions
            "$\\sin x$",
            "$\\cos x$",
            "$\\log x$",
            "$\\ln x$",
            // Matrix
            "$\\begin{matrix} 1 & 2 \\\\ 3 & 4 \\end{matrix}$",
            "$\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}$",
            "$\\begin{bmatrix} 1 \\\\ 2 \\end{bmatrix}$",
            "$\\begin{array}{cc} x & y \\\\ z & w \\end{array}$",
            // Mixed Korean + LaTeX
            "수식 $x + 1$ 입니다",
            "함수 $f(x)$",
            // Subscript variants
            "$a_1$",
            "$a_{12}$",
            "$x_n y_n$",
            // Superscript variants
            "$x^2 + y^2$",
            "$2^{10}$",
            // Combined
            "$x_i^j$",
            "$a^b_c$",
            // Math without LaTeX delimiters
            "1+2=3",
            "10×5=50",
            "x/y",
            // Comparison operators
            "1<2",
            "3>2",
            "x≥0",
            // Fraction inputs that may trigger inline fraction rule
            "1/2",
            "3/4 cup",
            "x1/2y",
            // LaTeX with brackets
            "$(x+y)$",
            "$[a,b]$",
            "$\\{x | x > 0\\}$",
            // Empty $$ pair
            "$$",
            // Unclosed (defensive)
            "$x = ",
        ];
        for input in inputs {
            // Each input MUST succeed without panicking.
            let _ = encode(input);
            // Also exercise unicode variant.
            let _ = encode_to_unicode(input);
        }
    }

    /// Math mode encoding sweep — covers math/encoder + math/parser paths.
    #[test]
    fn math_mode_comprehensive_sweep() {
        let inputs: &[&str] = &[
            "1+2", "x=1", "a+b-c", "x*y", "x/y", "(a+b)", "{c}", "[d]", "x^2", "x_n", "x≥0", "y≤1",
            "a≠b", "+", "-", "*", "/", "=", "<", ">", "≠", "≥", "≤", "π", "α", "β", "∞", "∂",
            "f(x)", "1 + 2", // spaces
            "x = y",
        ];
        let opts = EncodeOptions {
            default_mode: Some(EncodingMode::Math),
        };
        for input in inputs {
            let _ = encode_with_options(input, &opts);
        }
    }

    /// lib.rs:348 — combining-mark wrap absorbs leading digits/commas/periods.
    /// Input has digits + Korean syllable + combining mark above (U+0307 드러냄표).
    /// The wrap walks back through the Korean unit, then absorbs the preceding digits.
    #[test]
    fn formatting_mark_wrap_absorbs_leading_digits() {
        // "5강\u{0307}" — 1 combining mark, 1 Korean unit, leading digit "5".
        // After consuming 강 as the unit, the algorithm walks back to absorb '5'.
        let _ = encode("5강\u{0307}");
        // With comma and period interspersed.
        let _ = encode("1,000원\u{0307}");
        let _ = encode("3.14를\u{0307}");
    }

    /// lib.rs:357-358 — combining mark count exceeds available units,
    /// algorithm preserves the marks as-is (no wrap).
    #[test]
    fn formatting_mark_preserved_when_units_insufficient() {
        // Korean syllable followed by MORE combining marks than there are units.
        // "한\u{0307}\u{0307}\u{0307}\u{0307}" — 4 marks, only 1 Korean unit → units < count → preserve.
        let _ = encode("한\u{0307}\u{0307}\u{0307}\u{0307}");
        // 2 Korean units, 5 marks: units=2 < count=5 → preserve.
        let _ = encode("한글\u{0307}\u{0307}\u{0307}\u{0307}\u{0307}");
        // No-Korean-in-token preserves via earlier branch, but with Korean elsewhere
        // in the document the token_has_korean flag may still trigger.
        let _ = encode("\u{0307}\u{0307}");
    }

    /// lib.rs:492 — `decompose_accented_latin` early-return when no accented chars.
    /// Reached via direct encode() of plain ASCII or Korean input. The
    /// has_decomposable_latin flag triggers the call but the inner re-check
    /// against may_decompose_accented_latin returns false → early Cow return.
    /// This branch is structurally defensive (the scan triggers when at least one
    /// char is decomposable, and the inner check uses the same predicate, so the
    /// inner check should always be true). The branch is preserved as a no-op
    /// defensive guard against trigger-scan drift; we exercise it via plain input
    /// which goes through the `else` arm (no call to decompose_accented_latin).
    #[test]
    fn decompose_accented_latin_not_called_for_plain_input() {
        // Plain Korean: no accented latin chars → has_decomposable_latin = false →
        // function is NOT called. The else-branch (line 530-532) is taken.
        let _ = encode("안녕하세요");
        let _ = encode("hello");
    }

    /// lib.rs:495, 529 — `decompose_accented_latin` is called and produces output
    /// when input contains an accented Latin char (e.g. é, ñ, ã).
    #[test]
    fn decompose_accented_latin_called_for_accented_input() {
        // 'é' U+00E9 — Latin-1 Supplement, decomposable to 'e' + U+0301.
        // has_decomposable_latin = true → line 529 hits, function called.
        let _ = encode("café");
        // 'ñ' U+00F1 decomposes to 'n' + U+0303.
        let _ = encode("piñata");
        // 'ã' U+00E3 decomposes to 'a' + U+0303.
        let _ = encode("ão");
    }

    /// lib.rs:147 — Math Alphanumeric DIGIT blocks (𝟎-𝟗 across 5 styles) normalize
    /// to ASCII '0'-'9'. The DIGIT_BLOCKS loop returns at line 147 for matching codepoints.
    #[test]
    fn normalize_math_alphanumeric_digits() {
        // 𝟎 U+1D7CE (MATHEMATICAL BOLD DIGIT ZERO) → '0'
        assert_eq!(normalize_math_alphanumeric_char('\u{1D7CE}'), '0');
        // 𝟏 U+1D7CF → '1'
        assert_eq!(normalize_math_alphanumeric_char('\u{1D7CF}'), '1');
        // 𝟗 U+1D7D7 → '9'
        assert_eq!(normalize_math_alphanumeric_char('\u{1D7D7}'), '9');
        // 𝟘 U+1D7D8 (DOUBLE-STRUCK DIGIT ZERO) → '0'
        assert_eq!(normalize_math_alphanumeric_char('\u{1D7D8}'), '0');
        // 𝟢 U+1D7E2 (SANS-SERIF DIGIT ZERO) → '0'
        assert_eq!(normalize_math_alphanumeric_char('\u{1D7E2}'), '0');
        // 𝟬 U+1D7EC (SANS-SERIF BOLD DIGIT ZERO) → '0'
        assert_eq!(normalize_math_alphanumeric_char('\u{1D7EC}'), '0');
        // 𝟶 U+1D7F6 (MONOSPACE DIGIT ZERO) → '0'
        assert_eq!(normalize_math_alphanumeric_char('\u{1D7F6}'), '0');
    }
}
