//! Korean Braille rules (한글 점자 규정).
//!
//! Each module implements one or more articles from the
//! 2024 Korean Braille Standard (개정 한국 점자 규정).

// ── Chapter 1: 자모 (Jamo) ──────────────────────────────
pub mod rule_1; // 제1항: basic choseong (initial consonants)
pub mod rule_11; // 제11항: vowel + 예 separator
pub mod rule_12; // 제12항: ㅑ/ㅘ/ㅜ/ㅝ + 애 separator
pub mod rule_2; // 제2항: double choseong (된소리)
pub mod rule_3; // 제3항–제5항: jongseong (final consonants)
pub mod rule_8; // 제8항–제10항: standalone jamo
pub mod rule_korean; // General Korean syllable encoding (composite fallback)

// ── Chapter 2: 약자와 약어 (Abbreviations) ──────────────
pub mod rule_13; // 제13항, 제15항: syllable abbreviations
pub mod rule_14; // 제14항: no abbreviation before vowel
pub mod rule_16; // 제16항, 제17항: exception decomposition (팠,껐,셩,쎵,졍,쪙,쳥,겄)
pub mod rule_18; // 제18항: word abbreviations
pub mod rule_19; // 제19항: old consonants with old-letter marker
pub mod rule_20; // 제20항: ㅸ-series Middle Korean glyphs
pub mod rule_21; // 제21항: aspirated old-consonant composites
pub mod rule_22; // 제22항 및 붙임: fortis/cluster legacy glyphs
pub mod rule_23; // 제23항: historical letter symbols (ㅿ, ㅸ, ㆆ, 字, etc.)
pub mod rule_24; // 제24항: additional Middle Korean legacy syllables
pub mod rule_25; // 제25항: Middle Korean standalone vowels
pub mod rule_26; // 제26항: legacy glyphs after Hanja readings
pub mod rule_27; // 제27항: Middle Korean tone marks (거성/상성)

// ── Chapter 4: 로마자 (Roman letters) ───────────────────
pub mod rule_28; // 제28항: English encoding + uppercase
pub mod rule_29; // 제29항, 제31항, 제33항, 제35항: Roman indicators
pub mod rule_31; // 제31항: Greek letters in Korean context

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
pub mod rule_64; // 제64항: enclosed/circled symbols
pub mod rule_65; // 제65항: currency symbols
pub mod rule_66; // 제66항: literal braille cell rendering
pub mod rule_67; // 제67항: braille-cell mention marker in explanatory prose
pub mod rule_68; // 제68항: superscripts, subscripts, and selected unit symbols
pub mod rule_69; // 제69항: common measurement/unit symbols
pub mod rule_70; // 제70항: arrows
pub mod rule_71; // 제71항: information/keyboard symbols
pub mod rule_72; // 제72항: list and placeholder markers
pub mod rule_74; // 제74항: digital notation symbols reuse
pub mod rule_english_symbol; // English-context punctuation rendering

// ── Other ───────────────────────────────────────────────
pub mod rule_fraction; // Unicode fraction (½, ⅓, etc.)
pub mod rule_math; // Math symbols with Korean spacing
pub mod rule_space; // Space/newline encoding
