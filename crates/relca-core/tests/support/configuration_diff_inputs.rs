//! Shared fixture loading only; no semantic oracle or production diff calls.
use serde_json::Value;
pub fn source(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}
pub fn expected(name: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures")
                .join(name)
                .join("expected.json"),
        )
        .unwrap(),
    )
    .unwrap()
}
/// Erase ONLY intent blocks, preserving byte offsets, for old-input controls.
pub fn stripped(src: &str) -> String {
    let mut bytes = src.as_bytes().to_vec();
    let mut at = 0;
    while let Some(offset) = src[at..].find("intent {") {
        let start = at + offset;
        let mut i = start + "intent ".len();
        let mut depth = 0;
        let mut quoted = false;
        loop {
            let b = bytes[i];
            if quoted && b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                quoted = !quoted;
            }
            if !quoted {
                if b == b'{' {
                    depth += 1;
                }
                if b == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
            }
            i += 1;
        }
        for b in &mut bytes[start..i] {
            if *b != b'\n' {
                *b = b' ';
            }
        }
        at = i;
    }
    String::from_utf8(bytes).unwrap()
}
