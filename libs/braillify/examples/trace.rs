use braillify::encode_to_unicode;
use std::fs::File;
use std::io::Write;
fn main() {
    let mut f = File::create("trace_out.txt").unwrap();
    // Same string but in different contexts to find where the extra space comes from.
    let cases = [
        "반지름×반지름×3.14",
        "원의 면적은 반지름×반지름×3.14",
        "반지름×3.14",
        "반지름×반지름",
        "반지름×3",
        "둘레는 반지름×3.14이다.",
    ];
    for c in cases {
        match encode_to_unicode(c) {
            Ok(u) => writeln!(f, "{} -> {}", c, u).unwrap(),
            Err(e) => writeln!(f, "ERR {}: {}", c, e).unwrap(),
        }
    }
}
