//! Rebase archived byte expectations by exactly the 2026-11-14 Cut decision.
//! No other field is normalized; callers still compare every remaining byte.
#![allow(dead_code)]
const WARNING: &str = r#",{"code":"W-CUT-0","severity":"warning","message":"Cut is undeclared; exclusions are unknown","span":null}"#;
const LINE: &str = "W-CUT-0: Cut is undeclared; exclusions are unknown\n";

pub fn current(raw: &str, stderr: &str) -> (String, String) {
    let v: serde_json::Value = serde_json::from_str(raw).unwrap();
    if !v["model"]["name"].is_null() {
        return (raw.into(), stderr.into());
    }
    assert_eq!(v["cut"], "undeclared");
    assert!(matches!(
        v["diagnostics"][0]["code"].as_str(),
        Some("E-SYN-0" | "U-SYN-0")
    ));
    assert_eq!(raw.matches(WARNING).count(), 1);
    assert_eq!(stderr.matches(LINE).count(), 1);
    (
        raw.replace(WARNING, "")
            .replace(r#""cut":"undeclared""#, r#""cut":"unresolved""#),
        stderr.replace(LINE, ""),
    )
}

pub fn archived_projection(raw: &[u8]) -> Vec<u8> {
    let text = std::str::from_utf8(raw).unwrap();
    let v: serde_json::Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(v["cut"], "unresolved");
    assert!(v["model"]["name"].is_null());
    assert_eq!(v["diagnostics"].as_array().unwrap().len(), 1);
    assert!(matches!(
        v["diagnostics"][0]["code"].as_str(),
        Some("E-SYN-0" | "U-SYN-0")
    ));
    // The archived digest still checks all unrelated report bytes. Reinsert
    // only the old Cut status and warning, after asserting the new contract.
    let text = text.replace(r#""cut":"unresolved""#, r#""cut":"undeclared""#);
    let end = text.find("],\"bounds\"").unwrap();
    let mut legacy = text[..end].to_owned();
    legacy.push_str(WARNING);
    legacy.push_str(&text[end..]);
    legacy.into_bytes()
}
