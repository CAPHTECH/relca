//! Single-process inputs preserve the original no-selector explanation coverage.
#[path = "authority_scope.rs"]
mod acceptance;
use serde_json::{Value, json};
fn fragment<'a>(source: &'a str, location: &Value) -> &'a str {
    let a = location["span"]["start"].as_u64().unwrap() as usize;
    let b = location["span"]["end"].as_u64().unwrap() as usize;
    &source[a..b]
}
#[test]
fn scope_expiry_arrivals_and_parent_counterexamples_are_independently_replayed() {
    for (name, time, case, deadline, restriction, observed) in [
        (
            "auth-scope.broken-false-branch",
            3,
            1,
            Value::Null,
            "scope approval == pass",
            Some(("approval", "fail")),
        ),
        (
            "auth-expiry.broken-minute-early",
            5,
            1,
            json!(4),
            "expires 4min",
            None,
        ),
        (
            "auth-expiry-absolute.broken-earlier-expiry",
            7,
            2,
            json!(6),
            "expires 6min",
            None,
        ),
        (
            "auth-inherited-scope.broken-parent-scope",
            2,
            1,
            json!(5),
            "scope approved == false",
            Some(("approved", "true")),
        ),
    ] {
        let src = acceptance::source(name);
        assert!(!src.contains("configuration "));
        let v = serde_json::to_value(relca_core::explain::explain(&src)).unwrap();
        assert_eq!(v["verdict"], "violated", "{name}: {v}");
        let cs = v["explanation"]["counterexamples"].as_array().unwrap();
        let c = cs
            .iter()
            .find(|c| c["id"] == "contract Controls #1 authority_basis")
            .expect("authority witness");
        assert_eq!(c["status"], "verified");
        assert_eq!(c["replay"]["status"], "verified");
        assert_eq!(c["case_id"], case);
        let steps = c["steps"].as_array().unwrap();
        let finish = steps.last().unwrap();
        assert_eq!(finish["kind"], "finish");
        assert_eq!(finish["time"], time);
        assert_eq!(finish["executor"], "q");
        assert_eq!(finish["actor"], "A");
        assert_eq!(finish["capability"], "run");
        assert_eq!(finish["violation"]["deadline"], deadline);
        let related = c["related_locations"].as_array().unwrap();
        assert!(
            related.iter().any(|l| fragment(&src, l) == restriction),
            "missing restriction {restriction}: {related:?}"
        );
        let authority = if name.starts_with("auth-inherited") {
            "authority Parent"
        } else {
            "authority G"
        };
        assert!(
            related
                .iter()
                .any(|l| fragment(&src, l).starts_with(authority)),
            "missing authority declaration {name}"
        );
        if let Some((name, value)) = observed {
            assert!(
                finish["values"]["observed"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v["name"] == name && v["value"] == value),
                "event snapshot"
            );
        }
        let mut locations = related
            .iter()
            .map(|l| {
                (
                    l["span"]["start"].as_u64().unwrap(),
                    l["span"]["end"].as_u64().unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let count = locations.len();
        locations.sort();
        locations.dedup();
        assert_eq!(count, locations.len(), "duplicate related locations");
        assert!(
            !steps
                .iter()
                .any(|s| s["kind"] == "revoke" || s["kind"] == "authority_expired")
        );
        let kinds: Vec<_> = finish["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["kind"].as_str().unwrap())
            .collect();
        assert_eq!(
            kinds,
            if name.starts_with("auth-expiry.broken") {
                vec!["finish", "commit"]
            } else {
                vec!["finish"]
            }
        );
    }
}
#[test]
fn cut_does_not_invent_expiry_witness() {
    let v = serde_json::to_value(relca_core::explain::explain(&acceptance::source(
        "auth-expiry-cut.broken-short-clock",
    )))
    .unwrap();
    assert_eq!(v["verdict"], "inconclusive");
    assert_eq!(v["explanation"]["counterexamples"], json!([]));
}
