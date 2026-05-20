use braillify::encode_to_unicode;
use std::fs::File;
use std::io::Write;
fn main() {
    let mut f = File::create("trace_out.txt").unwrap();
    let cases = vec!["abc\u{C758}", "ab\u{C758}", "AB\u{C758}", "\u{D589}\u{B82C} AB\u{C758}"];
    for test in cases {
        match encode_to_unicode(test) {
            Ok(r) => {
                writeln!(f, "INPUT: {:?}", test).unwrap();
                write!(f, "HEX: ").unwrap();
                for c in r.chars() { write!(f, "U+{:04X} ", c as u32).unwrap(); }
                writeln!(f).unwrap();
            }, Err(e) => writeln!(f, "err: {}", e).unwrap(),
        }
    }
}
