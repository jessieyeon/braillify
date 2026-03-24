//! Korean Braille rules (한글 점자 규정).
//!
//! Each module implements one or more articles from the
//! 2024 Korean Braille Standard (개정 한국 점자 규정).

// ── Chapter 1: 자모 (Jamo) ──────────────────────────────
pub mod rule_1; // 제1항: basic choseong (initial consonants)
pub mod rule_2; // 제2항: double choseong (된소리)
pub mod rule_3; // 제3항–제5항: jongseong (final consonants)
pub mod rule_8; // 제8항–제10항: standalone jamo
pub mod rule_11; // 제11항: vowel + 예 separator
pub mod rule_12; // 제12항: ㅑ/ㅘ/ㅜ/ㅝ + 애 separator
pub mod rule_korean; // General Korean syllable encoding (composite fallback)

// ── Chapter 2: 약자와 약어 (Abbreviations) ──────────────
pub mod rule_13; // 제13항, 제15항: syllable abbreviations
pub mod rule_14; // 제14항: no abbreviation before vowel
pub mod rule_16; // 제16항, 제17항: exception decomposition (팠,껐,셩,쎵,졍,쪙,쳥,겄)
pub mod rule_18; // 제18항: word abbreviations

// ── Chapter 4: 로마자 (Roman letters) ───────────────────
pub mod rule_28; // 제28항: English encoding + uppercase
pub mod rule_29; // 제29항, 제31항, 제33항, 제35항: Roman indicators

// ── Chapter 5: 숫자 (Numbers) ───────────────────────────
pub mod rule_40; // 제40항, 제43항: number prefix indicator
pub mod rule_41; // 제41항: numeric comma (⠂)
pub mod rule_44; // 제44항 [다만]: number + confusable Korean spacing

// ── Chapter 6: 문장 부호 (Punctuation) ──────────────────
pub mod rule_49; // 제49항: symbol/punctuation encoding
pub mod rule_53; // 제53항: ellipsis normalization
pub mod rule_56; // 제56항: combining emphasis marks
pub mod rule_57; // 제57항: placeholder symbol grouping (○×△☆◇◆)
pub mod rule_58; // 제58항: blank marks (□)
pub mod rule_60; // 제60항: asterisk (*) spacing
pub mod rule_61; // 제61항: apostrophe (') before numbers
pub mod rule_english_symbol; // English-context punctuation rendering

// ── Other ───────────────────────────────────────────────
pub mod rule_fraction; // Unicode fraction (½, ⅓, etc.)
pub mod rule_math; // Math symbols with Korean spacing
pub mod rule_space; // Space/newline encoding
