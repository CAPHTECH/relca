use super::*;

#[test]
fn priority_extensions_are_scope_and_order_checked_without_narrowing_legacy_fifo() {
    for item in [
        "policy observable priority descending tie fifo tie_by case_id assign first_idle then call_order",
        "policy priority descending tie nondet assign first_idle tie fifo tie_by case_id",
        "concurrent cases 1 on_overflow wait_external priority descending then call_order",
        "actor A { capability c() -> R { version_check [x] else conflict resolve priority descending then call_order } }",
    ] {
        let ds = parse(&format!("model M version \"0.2\" {{ {item} }}")).unwrap_err();
        assert_eq!(ds[0].code, diag::U_SEM_0, "{item}: {ds:?}");
        assert!(ds[0].message.contains("priority_tie_incompatible"));
    }
    for item in [
        "policy observable fifo tie_by case_id assign first_idle then call_order",
        "policy observable fifo tie_by case_id then call_order assign first_idle then call_order",
        "policy observable priority descending tie fifo tie_by case_id then call_order assign first_idle",
    ] {
        parse(&format!("model M version \"0.2\" {{ {item} }}")).unwrap();
    }
}

#[test]
fn priority_bare_legacy_unsupported_word_retains_its_original_diagnostic() {
    let source = "model M version \"0.2\" { policy priority\n simulation replications 4 seed 0 confidence 0.5 }";
    let ds = parse(source).unwrap_err();
    assert_eq!(ds[0].code, diag::U_SYN_0);
    assert_eq!(ds[0].message, "unsupported syntax: priority");
    let span = ds[0].span.unwrap();
    assert_eq!(&source[span.start..span.end], "priority");
    assert!(!has_priority_declaration(source));
    let ordinary_name = r#"model M version "0.2" {
        type case_priority = enum { ok }
        actor A { capacity 1; capability c() -> case_priority { duration fixed 1min; result nondet { ok } } }
        process Flow { case_priority = A.c(); end done }
        bounds { cases 1; live_tasks 1; clock 5min }
    }"#;
    parse(ordinary_name).unwrap();
    assert!(!has_priority_declaration(ordinary_name));
}
