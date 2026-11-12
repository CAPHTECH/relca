#![allow(dead_code)]
use serde_json::Value;
use std::path::PathBuf;
pub const PAIRS: [(&str, &str, &str, &str); 28] = [
    ("two-add", "assignment", "n += 1", "n = 1"),
    ("upper", "overflow", "0..2 = 1", "0..2 = 2"),
    ("underflow", "underflow", "0..2 = 1", "0..2 = 0"),
    ("default", "default", "0..2 = 2", "0..2 = 3"),
    ("type", "enum", "n == 1", "n == ok"),
    ("latent-policy", "latent", "observed case", "latent case"),
    ("lt", "boundary", "0..3 = 1", "0..3 = 2"),
    ("le", "boundary", "0..3 = 2", "0..3 = 3"),
    ("gt", "boundary", "0..3 = 2", "0..3 = 1"),
    ("ge", "boundary", "0..3 = 2", "0..3 = 1"),
    ("ne", "boundary", "0..3 = 1", "0..3 = 2"),
    (
        "fork",
        "same-target",
        "start { m += 1 }",
        "start { n += 1 }",
    ),
    (
        "start-finish",
        "zero-start",
        "start { n += 1 }",
        "start { n += 0 }",
    ),
    ("set-stop", "outward", "n = 2 }", "n = 3 }"),
    ("dist", "threshold", ">= 0.75", ">= 0.76"),
    ("nondet", "threshold", ">= 0\n", ">= 1\n"),
    ("class-int", "unbounded", "n: T", "n: int"),
    ("class-param", "caseid", "k: T", "k: PersonId"),
    ("class-caseid", "reference", "A.use(a)", "A.use(case.other)"),
    ("class-reads", "observed", "reads [S]", "reads [flag]"),
    ("class-stateid", "caseid", "k: T", "k: CaseId"),
    (
        "kernel-given",
        "integer",
        "latent case { h: H }\n  kernel K given [h] { zero -> { 1: ok }; one -> { 1: ok } }",
        "latent case { h: 0..1 }\n  kernel K given [h] { 0 -> { 1: ok }; 1 -> { 1: ok } }",
    ),
    ("requires", "refused", "0..2 = 1", "0..2 = 0"),
    ("kernel-finish", "overflow", "0..1 = 0", "0..1 = 1"),
    ("atomic", "overflow", "0..1 = 0", "0..1 = 1"),
    (
        "cancel",
        "drain",
        "remaining cancel_then_drain",
        "remaining drain",
    ),
    ("range-vs-clock", "clock", "clock 1min", "clock 0min"),
    ("arrivals", "overflow", "0..1 = 0", "0..1 = 1"),
];
pub fn names() -> Vec<String> {
    PAIRS
        .iter()
        .flat_map(|(base, fault, _, _)| {
            [format!("int-{base}"), format!("int-{base}.broken-{fault}")]
        })
        .collect()
}
pub fn fixture(name: &str) -> bool {
    if !name.starts_with("int-") {
        return false;
    }
    assert!(
        names().iter().any(|n| n == name),
        "unclassified integer fixture {name}"
    );
    true
}
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn source(name: &str) -> String {
    std::fs::read_to_string(root().join("fixtures").join(name).join("model.relca")).unwrap()
}
pub fn expected(name: &str) -> Value {
    serde_json::from_slice(
        &std::fs::read(root().join("fixtures").join(name).join("expected.json")).unwrap(),
    )
    .unwrap()
}
pub fn subset(e: &Value, a: &Value) {
    match e {
        Value::Object(o) => {
            for (k, v) in o {
                if k == "absent_diagnostics" {
                    for code in v.as_array().unwrap() {
                        assert!(
                            !a["diagnostics"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|d| d["code"] == *code),
                            "forbidden {code}: {a}"
                        );
                    }
                } else if k == "diagnostics" {
                    for d in v.as_array().unwrap() {
                        assert!(
                            a[k].as_array()
                                .unwrap()
                                .iter()
                                .any(|actual| actual["code"] == d["code"]
                                    && actual["severity"] == d["severity"]),
                            "diagnostic {d}: {a}"
                        );
                    }
                } else {
                    assert!(a.get(k).is_some(), "missing {k}: {a}");
                    subset(v, &a[k]);
                }
            }
        }
        Value::Array(es) => {
            let actual = a
                .as_array()
                .unwrap_or_else(|| panic!("expected array {e}: {a}"));
            assert_eq!(es.len(), actual.len(), "closed row inventory: {e} vs {a}");
            for (e, a) in es.iter().zip(actual) {
                subset(e, a);
            }
        }
        _ => assert_eq!(e, a),
    }
}
pub fn property<'a>(v: &'a Value, id: &str) -> &'a Value {
    let ps = v["properties"].as_array().unwrap();
    assert_eq!(
        ps.iter().filter(|p| p["id"] == id).count(),
        1,
        "property exactly once {id}: {v}"
    );
    ps.iter().find(|p| p["id"] == id).unwrap()
}
pub fn rejection(s: &str, code: &str) {
    let v = serde_json::to_value(relca_core::report::check(s)).unwrap();
    let errors: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert!(!errors.is_empty(), "missing {code}: {s}\n{v}");
    assert!(
        errors.iter().all(|d| d["code"] == code),
        "single intended fault {code}: {s}\n{v}"
    );
    assert_eq!(
        v["verdict"],
        if code.starts_with('E') {
            "invalid"
        } else {
            "unsupported"
        },
        "{s}\n{v}"
    );
    assert_eq!(v["properties"], serde_json::json!([]));
    assert_eq!(v["terminals"], serde_json::json!([]));
    for d in errors {
        let a = d["span"]["start"].as_u64().unwrap() as usize;
        let b = d["span"]["end"].as_u64().unwrap() as usize;
        assert!(a < b && b <= s.len() && s.is_char_boundary(a) && s.is_char_boundary(b));
    }
}
/// Deliberately NOT an integer oracle: removes the integer layer and its
/// classification faults to preflight authored legacy calls/resources/joins.
pub fn old_skeleton(name: &str) -> String {
    let mut s = source(name);
    if name == "int-kernel-given.broken-integer" {
        s = source("int-kernel-given");
    }
    for (from, to) in [
        ("n: int", "n: T"),
        ("k: PersonId", "k: T"),
        ("k: CaseId", "k: T"),
        ("A.use(case.other)", "A.use(a)"),
        ("reads [flag]", "reads [S]"),
        ("latent case { n:", "observed case { n:"),
        (
            "h: 0..1 ~ { 0:0.25, 1:0.75 }",
            "h: bool ~ { false:0.25, true:0.75 }",
        ),
        ("h: 0..1", "h: bool"),
        ("0..3 = 0", "bool = false"),
        ("0..3 = 1", "bool = true"),
        ("0..3 = 2", "bool = true"),
        ("0..3 = 3", "bool = true"),
        ("0..2 = 0", "bool = false"),
        ("0..2 = 1", "bool = true"),
        ("0..2 = 2", "bool = true"),
        ("0..2 = 3", "bool = true"),
        ("0..1 = 0", "bool = false"),
        ("0..1 = 1", "bool = true"),
        ("n += 1", "n = true"),
        ("n += 0", "n = false"),
        ("m += 1", "m = true"),
        ("n -= 1", "n = false"),
        ("n = 1", "n = true"),
        ("n = 2", "n = true"),
        ("n = 3", "n = true"),
        ("n == ok", "n == true"),
        ("n == 0", "n == false"),
        ("n == 1", "n == true"),
        ("n == 2", "n == true"),
        ("m == 1", "m == true"),
        ("h == 1", "h == true"),
        ("n<=2", "n == true"),
        ("n>=2", "n == true"),
        ("n>=1", "n == true"),
        ("n<2", "n == true"),
        ("n>1", "n == true"),
        ("n!=2", "n == true"),
        ("start { n = true }", "start { m = true }"),
    ] {
        s = s.replace(from, to);
    }
    // The target correction is only for F12's deliberately overlapping pair.
    if !name.starts_with("int-fork") {
        s = s.replace("start { m = true }", "start { n = true }");
    }
    s
}
