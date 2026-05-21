use braillify::encode_to_unicode;
use std::fs::File;
use std::io::Write;
fn main() {
    let mut f = File::create("trace_out.txt").unwrap();
    let cases = [
        (
            "per-char on every char (15,000원 모두 표시)",
            "금액 할인: 1\u{0333}5\u{0333},\u{0333}0\u{0333}0\u{0333}0\u{0333}원\u{0333} 14,500원",
            "⠈⠪⠢⠗⠁⠀⠚⠂⠟⠐⠂⠀⠈⠤⠼⠁⠑⠂⠚⠚⠚⠏⠒⠤⠁⠀⠼⠁⠙⠂⠑⠚⠚⠏⠒",
        ),
        (
            "legacy trailing-1 (digit-attach)",
            "금액 할인: 15,000원\u{0333} 14,500원",
            "⠈⠪⠢⠗⠁⠀⠚⠂⠟⠐⠂⠀⠈⠤⠼⠁⠑⠂⠚⠚⠚⠏⠒⠤⠁⠀⠼⠁⠙⠂⠑⠚⠚⠏⠒",
        ),
        (
            "legacy trailing-5 (돼지̇ ̇ ̇ ̇ ̇)",
            "배부른 돼지\u{0307} \u{0307} \u{0307} \u{0307} \u{0307}보다는 배고픈 소크라테스가 되겠다.",
            "",
        ),
        ("math 0.6 overdot", "0.6\u{0307}", "⠼⠚⠲⠈⠉"),
        ("math X underline", "거리공간 X\u{0332}", "⠈⠎⠐⠕⠈⠿⠫⠒⠀⠀⠠⠭⠠⠤"),
    ];
    for (label, input, expected) in cases {
        match encode_to_unicode(input) {
            Ok(u) => {
                if expected.is_empty() {
                    writeln!(f, "{}: {}", label, u).unwrap();
                } else {
                    let ok = u == expected;
                    writeln!(f, "{} {}", label, if ok { "OK" } else { "FAIL" }).unwrap();
                    if !ok {
                        writeln!(f, "  exp: {}", expected).unwrap();
                        writeln!(f, "  got: {}", u).unwrap();
                    }
                }
            }
            Err(e) => writeln!(f, "{}: ERR {}", label, e).unwrap(),
        }
    }
}
