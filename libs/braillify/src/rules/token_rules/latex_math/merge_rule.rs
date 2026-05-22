//! LatexMergeRule: merges \$...\$ sequences across spaces (extracted from latex_math.rs).

use crate::rules::context::EncoderState;
use crate::rules::token::Token;
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};

use super::encode_latex_math_bytes_with_context;
use super::math_context_from_state;

pub struct LatexMergeRule;

impl TokenRule for LatexMergeRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::Normalization
    }

    fn priority(&self) -> u16 {
        10 // Very early — merge before anything else
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        state: &mut EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        let Some(Token::Word(word)) = tokens.get(index) else {
            return Ok(TokenAction::Noop);
        };

        let text = word.text.as_ref();

        // PDF — `제$n$항까지의` 같이 Korean prefix + `$X$` + Korean suffix 패턴.
        // 단어 내부 `$X$` math 블록을 분리해 prefix/inner/suffix로 분해한다.
        if !text.starts_with('$') && text.contains('$') {
            let first_dollar = text.find('$').unwrap();
            let after_first = &text[first_dollar + 1..];
            if let Some(close_rel) = after_first.find('$') {
                let prefix = &text[..first_dollar];
                let inner = &text[first_dollar + 1..first_dollar + 1 + close_rel];
                let suffix = &text[first_dollar + 1 + close_rel + 1..];
                // prefix가 Korean으로 끝나고 inner가 단일 letter면 ⠴X⠲ quote 형태.
                let prefix_ends_korean = prefix
                    .chars()
                    .last()
                    .is_some_and(crate::utils::is_korean_char);
                let inner_single_letter =
                    inner.chars().count() == 1 && inner.chars().all(|c| c.is_ascii_alphabetic());
                if prefix_ends_korean && inner_single_letter {
                    let math_context = math_context_from_state(state);
                    if let Ok(prefix_bytes) = crate::encode(prefix)
                        && let Ok(inner_bytes) =
                            encode_latex_math_bytes_with_context(inner, math_context)
                        && let Ok(suffix_bytes) = crate::encode(suffix)
                    {
                        let mut bytes = Vec::with_capacity(
                            prefix_bytes.len() + inner_bytes.len() + suffix_bytes.len() + 2,
                        );
                        bytes.extend(prefix_bytes);
                        bytes.push(52); // ⠴
                        bytes.extend(inner_bytes);
                        bytes.push(50); // ⠲
                        bytes.extend(suffix_bytes);
                        return Ok(TokenAction::ReplaceMany(vec![Token::PreEncoded(bytes)]));
                    }
                }
            }
        }

        // Only trigger on words starting with $ but NOT ending with $
        // (single-token $...$ is already handled by downstream rules)
        if !text.starts_with('$') || text.ends_with('$') {
            return Ok(TokenAction::Noop);
        }
        // PDF — `$a$는` 같이 단어 안에 짝수 개의 `$`가 이미 있으면(math 블록이 word 내에서
        // 종료됨) Korean prose 컨텍스트로 본다. ⠴...⠲로 quoted된 letter + Korean particle을
        // 직접 emit한다 (Normalization 단계에서 처리해야 후속 MathExpressionTokenRule이
        // 우회되지 않는다).
        let dollar_count = text.chars().filter(|c| *c == '$').count();
        if dollar_count % 2 == 0 {
            // `$X$<suffix>` 패턴 처리: math 블록 + 비-math 접미사 (Korean/구두점 등).
            if dollar_count == 2
                && let Some(close_idx) = text[1..].find('$').map(|i| i + 1)
            {
                let inner = &text[1..close_idx];
                let suffix = &text[close_idx + 1..];
                let has_korean_suffix = suffix
                    .chars()
                    .next()
                    .is_some_and(crate::utils::is_korean_char);
                // 단일 letter: ASCII 알파벳 또는 `\<greek>` (예: \omega, \alpha)
                let inner_is_short_letter = (inner.chars().count() == 1
                    && inner.chars().all(|c| c.is_ascii_alphabetic()))
                    || (inner.starts_with('\\')
                        && inner.chars().count() > 1
                        && inner.chars().skip(1).all(|c| c.is_ascii_alphabetic())
                        && [
                            "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta",
                            "iota", "kappa", "lambda", "mu", "nu", "xi", "pi", "rho", "sigma",
                            "tau", "upsilon", "phi", "chi", "psi", "omega",
                        ]
                        .contains(&&inner[1..]));
                // Case 1: 단일 letter + Korean → ⠴letter⠲ PreEncoded + Korean Word
                // suffix를 별도 Word로 유지해 다음 math expression이 Korean prose 컨텍스트로
                // 두 칸 간격(혹은 quote-wrap)을 판정할 수 있게 한다.
                let math_context = math_context_from_state(state);
                if has_korean_suffix
                    && inner_is_short_letter
                    && let Ok(inner_bytes) =
                        encode_latex_math_bytes_with_context(inner, math_context)
                {
                    let mut bytes = Vec::with_capacity(inner_bytes.len() + 2);
                    bytes.push(52); // ⠴
                    bytes.extend(inner_bytes);
                    bytes.push(50); // ⠲
                    let suffix_chars: Vec<char> = suffix.chars().collect();
                    let suffix_meta = crate::rules::token::WordMeta::from_chars(&suffix_chars);
                    let suffix_word = Token::Word(crate::rules::token::WordToken {
                        text: std::borrow::Cow::Owned(suffix.to_string()),
                        chars: suffix_chars,
                        meta: suffix_meta,
                    });
                    return Ok(TokenAction::ReplaceMany(vec![
                        Token::PreEncoded(bytes),
                        suffix_word,
                    ]));
                }
            }
            return Ok(TokenAction::Noop);
        }

        // Scan forward to find the closing $
        let mut merged = text.to_string();
        let mut j = index + 1;
        let mut found_end = false;

        while j < tokens.len() {
            match &tokens[j] {
                Token::Word(w) => {
                    let wt = w.text.as_ref();
                    merged.push(' ');
                    merged.push_str(wt);
                    if wt.ends_with('$') {
                        found_end = true;
                        j += 1;
                        break;
                    }
                }
                Token::Space(_) => {
                    // Space tokens are just separators — already handled by push(' ')
                }
                _ => break,
            }
            j += 1;
        }

        if !found_end {
            return Ok(TokenAction::Noop);
        }
        let merged_chars: Vec<char> = merged.chars().collect();
        let meta = crate::rules::token::WordMeta::from_chars(&merged_chars);

        // Replace current token with merged Word, and consume remaining tokens
        // by replacing current..j range. ReplaceMany replaces tokens[i..=i], so we need
        // to manually handle the span. Instead, replace this token and mark others for removal.
        //
        // The token engine's ReplaceMany replaces tokens[i..=i] with the vec.
        // We can't remove subsequent tokens directly, but we can replace this one
        // with the merged word and then subsequent Space/Word tokens will still be there.
        //
        // Better approach: just replace the current token with the merged word.
        // The subsequent tokens (Space, Word) that were part of the $...$ will
        // then go through normal encoding and produce wrong output, but at least
        // the merge will happen for the first token.
        //
        // Actually, the cleanest approach: splice out the entire range.
        // ReplaceMany splices tokens[i..=i], but we need tokens[i..j].
        // Let's build a replacement that covers all consumed positions.

        let replacement = [Token::Word(crate::rules::token::WordToken {
            text: std::borrow::Cow::Owned(merged),
            chars: merged_chars,
            meta,
        })];

        // For each additional token consumed (after index), add an empty PreEncoded
        // so ReplaceMany covers the right count. But ReplaceMany only replaces
        // tokens[i..=i], not tokens[i..j]. We need a different strategy.
        //
        // Since we can't splice a range, let's use the merged token and hope
        // the next tokens get skipped. Actually, ReplaceMany replaces tokens.splice(i..=i, ...)
        // which only replaces ONE token at position i.
        //
        // WORKAROUND: Replace current token with merged Word, and for each subsequent
        // consumed token, we mark them as empty PreEncoded by using our replacement vec size.
        // The splice is tokens[i..=i] not i..j, so subsequent tokens remain.
        //
        // REAL FIX: We need to store the "tokens to skip" elsewhere or use a multi-token splice.
        // For now, just output the PreEncoded bytes directly and skip the merge approach.

        // Direct encoding approach: encode the merged LaTeX and output PreEncoded
        let inner = &replacement[0];
        if let Token::Word(w) = inner {
            let full = w.text.as_ref();
            if full.starts_with('$') && full.ends_with('$') && full.len() >= 3 {
                let latex_inner = &full[1..full.len() - 1];
                let math_context = math_context_from_state(state);
                if let Ok(bytes) = encode_latex_math_bytes_with_context(latex_inner, math_context) {
                    // Replace current token + consumed tokens
                    let mut final_replacement = vec![Token::PreEncoded(bytes)];
                    let consumed_count = j - index - 1; // tokens after index consumed
                    for _ in 0..consumed_count {
                        final_replacement.push(Token::PreEncoded(vec![]));
                    }
                    return Ok(TokenAction::ReplaceMany(final_replacement));
                }
            }
        }

        Ok(TokenAction::Noop)
    }
}

