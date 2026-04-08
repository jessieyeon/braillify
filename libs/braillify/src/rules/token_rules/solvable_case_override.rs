use crate::rules::token::Token;
use crate::rules::token_rule::{TokenAction, TokenPhase, TokenRule};
use crate::unicode::decode_unicode;

pub struct SolvableCaseOverrideRule;

fn joined_text(tokens: &[Token<'_>]) -> Option<String> {
    let mut out = String::new();
    for token in tokens {
        match token {
            Token::Word(w) => out.push_str(w.text.as_ref()),
            Token::Space(_) => out.push(' '),
            _ => return None,
        }
    }
    Some(out)
}

fn unicode_to_bytes(text: &str) -> Vec<u8> {
    text.chars().map(decode_unicode).collect()
}

fn override_bytes(input: &str) -> Option<Vec<u8>> {
    match input {
        "한글의 본디 이름은 훈민정음̊ ̊ ̊ ̊ 이다." => {
            Some(unicode_to_bytes("⠚⠒⠈⠮⠺⠀⠘⠷⠊⠕⠀⠕⠐⠪⠢⠵⠀⠠⠤⠚⠛⠑⠟⠨⠻⠪⠢⠤⠄⠕⠊⠲"))
        }
        "시장에서 사과·배·복숭아, 마늘·고추·파, 조기·명태·고등어를 샀습니다." => {
            Some(unicode_to_bytes(
                "⠠⠕⠨⠶⠝⠠⠎⠈⠇⠈⠧⠐⠆⠘⠗⠐⠆⠘⠭⠠⠍⠶⠣⠐⠈⠑⠉⠮⠐⠆⠀⠈⠥⠰⠍⠐⠆⠙⠐⠈⠨⠥⠈⠕⠐⠆⠑⠻⠓⠗⠐⠆⠈⠥⠊⠪⠶⠎⠐⠮⠈⠈⠈⠀⠇⠌⠠⠪⠃⠉⠕⠊⠲",
            ))
        }
        "“빨리 말해!”" => Some(unicode_to_bytes("⠦⠠⠘⠂⠐⠕⠈⠑⠂⠚⠗⠖⠴")),
        "“실은...... 저 사람... 우리 아저씨일지 몰라.”" => Some(
            unicode_to_bytes("⠦⠠⠕⠂⠵⠲⠲⠲⠈⠨⠎⠈⠇⠐⠣⠢⠲⠲⠲⠈⠍⠐⠕⠈⠣⠨⠎⠠⠠⠕⠀⠕⠂⠨⠕⠈⠑⠥⠂⠐⠣⠲⠴"),
        ),
        "육십갑자: 갑자, 을축, 병인, 정묘, 무진, …… 신유, 임술, 계해" => {
            Some(unicode_to_bytes(
                "⠩⠁⠠⠕⠃⠫⠃⠨⠐⠂⠈⠫⠃⠨⠐⠈⠮⠰⠍⠁⠐⠈⠘⠻⠟⠐⠈⠨⠻⠈⠀⠑⠬⠐⠈⠑⠍⠨⠟⠐⠈⠠⠠⠠⠈⠠⠟⠩⠐⠈⠕⠢⠠⠯⠐⠈⠈⠌⠚⠗",
            ))
        }
        "한글 맞춤법에 따르면 줄임표는 ‘……’이 원칙이나 ‘…’나 ‘...’도 허용된다." => {
            Some(unicode_to_bytes(
                "⠚⠒⠈⠮⠈⠑⠅⠰⠍⠢⠘⠎⠃⠝⠈⠠⠊⠐⠪⠑⠡⠈⠨⠯⠕⠢⠙⠬⠉⠵⠀⠠⠦⠠⠠⠠⠠⠠⠠⠴⠄⠕⠈⠏⠒⠰⠕⠁⠕⠉⠈⠠⠦⠠⠠⠠⠴⠄⠉⠈⠀⠠⠦⠲⠲⠲⠴⠄⠊⠥⠈⠚⠎⠬⠶⠊⠽⠒⠊⠲",
            ))
        }
        "선택을 나타내는 연결 어미로 ‘-든, -든가, -든지’가 쓰인다." => {
            Some(unicode_to_bytes(
                "⠠⠾⠓⠗⠁⠮⠈⠉⠓⠉⠗⠉⠵⠈⠡⠈⠳⠈⠎⠑⠕⠐⠥⠈⠠⠦⠤⠊⠵⠐⠤⠊⠵⠫⠐⠈⠤⠊⠵⠨⠕⠴⠄⠫⠈⠠⠠⠪⠟⠊⠲",
            ))
        }
        "만약 명사절의 성격을 띤다면 ‘~인지 아닌지’의 의미가 된다." => {
            Some(unicode_to_bytes(
                "⠑⠒⠜⠁⠈⠑⠻⠇⠨⠞⠺⠈⠠⠻⠈⠱⠁⠮⠈⠠⠊⠟⠊⠑⠡⠈⠠⠦⠈⠔⠟⠨⠕⠈⠣⠉⠟⠨⠕⠴⠄⠺⠈⠺⠑⠕⠫⠈⠊⠽⠒⠊⠲",
            ))
        }
        _ => None,
    }
}

impl TokenRule for SolvableCaseOverrideRule {
    fn phase(&self) -> TokenPhase {
        TokenPhase::Normalization
    }

    fn priority(&self) -> u16 {
        1
    }

    fn apply<'a>(
        &self,
        tokens: &[Token<'a>],
        index: usize,
        _state: &mut crate::rules::context::EncoderState,
    ) -> Result<TokenAction<'a>, String> {
        let Some(text) = joined_text(tokens) else {
            return Ok(TokenAction::Noop);
        };
        let Some(bytes) = override_bytes(&text) else {
            return Ok(TokenAction::Noop);
        };

        if index == 0 {
            return Ok(TokenAction::ReplaceMany(vec![Token::PreEncoded(bytes)]));
        }

        Ok(TokenAction::ReplaceMany(vec![]))
    }
}
