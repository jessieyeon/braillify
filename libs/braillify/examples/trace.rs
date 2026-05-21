use braillify::encode_to_unicode;
use std::fs::File;
use std::io::Write;
fn main() {
    let mut f = File::create("trace_out.txt").unwrap();
    for (label, c) in [("ㅄ U+3144", "\u{3144}"), ("ᄡ U+1121", "\u{1121}")] {
        match encode_to_unicode(c) {
            Ok(u) => writeln!(f, "{}: {}", label, u).unwrap(),
            Err(e) => writeln!(f, "{}: ERR {}", label, e).unwrap(),
        }
    }
}
