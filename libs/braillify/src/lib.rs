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
            return char::from_u32(b'0' as u32 + (cp - start)).unwrap_or(c);
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
fn decompose_accented_latin<'a>(text: Cow<'a, str>) -> Cow<'a, str> {
    use unicode_normalization::UnicodeNormalization;

    if !text.as_ref().chars().any(may_decompose_accented_latin) {
        return text;
    }

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
#[path = "lib_main_tests.rs"]
mod test;

#[cfg(test)]
#[path = "lib_coverage_tests.rs"]
mod coverage_targeted_tests;
