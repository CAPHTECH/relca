//! Independent Stage C policy oracle helpers; no implementation output capture.
#![allow(dead_code)]
use relca_core::{engine, ir, lower, reference, report, syntax};
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::PathBuf};
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn pairs() -> Vec<Value> {
    serde_json::from_str(include_str!("policy_pairs.json")).unwrap()
}
pub fn names() -> Vec<String> {
    serde_json::from_str(include_str!("policy_names.json")).unwrap()
}
pub fn fixture(name: &str) -> bool {
    if !name.starts_with("pol-") {
        return false;
    }
    assert!(
        names().iter().any(|n| n == name),
        "unclassified policy fixture {name}"
    );
    true
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
pub fn edit(s: &str, a: &str, b: &str) -> String {
    assert!(s.contains(a), "missing control edit {a}");
    let t = s.replace(a, b);
    assert_ne!(s, t, "control edit must actually change input");
    t
}
pub fn old_skeleton(s: &str) -> String {
    let mut t = s.to_owned();
    for token in ["observable ", " then call_order", " assign first_idle"] {
        if t.contains(token) {
            t = edit(&t, token, "");
        }
    }
    t
}
pub fn permission_authority_source() -> String {
    let s = edit(
        &source("pol-permission"),
        "capability work() -> T {",
        "capability work() -> T { commits;",
    );
    edit(
        &s,
        "contract Ctl { permitted A.work }",
        "authority Grant { basis \"grant\"; grants q to A.work }\n  contract Ctl { permitted A.work; authority_basis A.work Grant }",
    )
}
pub fn selected(s: &str, selection: Option<&str>) -> ir::Model {
    let ast = syntax::parse(s).unwrap_or_else(|d| panic!("policy parse: {d:?}\n{s}"));
    match selection {
        Some(c) => lower::lower_configuration(&ast, c).map(|s| s.model),
        None => lower::lower(&ast),
    }
    .unwrap_or_else(|d| panic!("policy lower: {d:?}\n{s}"))
}
pub fn checked(s: &str, selection: Option<&str>) -> Value {
    serde_json::to_value(match selection {
        Some(c) => report::check_configuration(s, c),
        None => report::check(s),
    })
    .unwrap()
}
pub fn sampled(s: &str, selection: Option<&str>) -> Value {
    serde_json::to_value(match selection {
        Some(c) => report::simulate_configuration(s, c),
        None => report::simulate(s),
    })
    .unwrap()
}
pub fn subset(e: &Value, a: &Value) {
    match e {
        Value::Object(o) => {
            for (k, v) in o {
                subset(v, a.get(k).unwrap_or_else(|| panic!("missing {k}: {a}")));
            }
        }
        Value::Array(es) => {
            let actual = a
                .as_array()
                .unwrap_or_else(|| panic!("expected array: {a}"));
            assert_eq!(es.len(), actual.len(), "closed array {e} vs {a}");
            // Object subsets are matched bijectively, including duplicate diagnostic codes.
            let mut used = BTreeSet::new();
            for want in es {
                let index = actual
                    .iter()
                    .enumerate()
                    .find(|(i, v)| !used.contains(i) && is_subset(want, v))
                    .map(|(i, _)| i)
                    .unwrap_or_else(|| panic!("missing hand row {want} in {a}"));
                used.insert(index);
                subset(want, &actual[index]);
            }
        }
        _ => assert_eq!(e, a, "hand oracle"),
    }
}
fn is_subset(e: &Value, a: &Value) -> bool {
    match e {
        Value::Object(o) => o
            .iter()
            .all(|(k, v)| a.get(k).is_some_and(|x| is_subset(v, x))),
        Value::Array(es) => a.as_array().is_some_and(|xs| {
            es.len() == xs.len() && es.iter().zip(xs).all(|(x, y)| is_subset(x, y))
        }),
        _ => e == a,
    }
}
pub fn both(s: &str, selection: Option<&str>) -> relca_core::outcome::Outcome {
    let m = selected(s, selection);
    let e = engine::explore(&m);
    let mut r = reference::explore(&m);
    assert_eq!(
        r.stop,
        reference::StopReason::Complete,
        "independent reference must finish"
    );
    assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
    r.outcome.diagnostics.drain(..m.diagnostics.len());
    assert_eq!(
        e, r.outcome,
        "all Outcome fields, including scheduler classification"
    );
    e
}
pub fn row<'a>(v: &'a Value, field: &str) -> Option<&'a Value> {
    let rows: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["field"] == field)
        .collect();
    assert!(rows.len() <= 1, "duplicate assumption {field}");
    rows.first().copied()
}
pub fn probability(v: &Value, min: &str, max: &str) {
    let p = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "probability reached #1")
        .unwrap();
    assert_eq!(p["verdict"], "satisfied");
    assert_eq!(
        p["cases"].as_array().unwrap().len() as u64,
        v["bounds"]["cases"].as_u64().unwrap()
    );
    for c in p["cases"].as_array().unwrap() {
        assert_eq!(c["probability"]["min"], min);
        assert_eq!(c["probability"]["max"], max);
    }
}
pub const FIFO_AFFECTS: &str = "each start compares only the tasks queued within the Actor at that moment, by queue-entry time then case ID; tasks entering the queue later in the same instant are not compared; ties within one case remain nondeterministic";
pub const QUEUE_AFFECTS: &str = "方策の区分の待ちを、待ちに入った時刻・案件ID・呼出しの字句の順で比較し、開始可能な最小の1件を1遷移で開始または拒否する。Actorと資源の待ちの先頭を追い越さず、人は取り置かない。通常の呼出しはprocess本体の前順、contestの合成呼出しはその後のcontest宣言順とする。環境とpartyにはActorごとのFIFOだけが掛かる。同じ時刻に後から待ちに入るTaskは比べない。";
pub const ASSIGN_AFFECTS: &str = "方策の区分のActorには、executorsの宣言順の最初の空いている人を割り当てる。占有は全Actor・全案件で共有し、PermissionとAuthorityでは人を選ばない。";
pub const CLASSES_AFFECTS: &str = "方策が解くのはpolicy区分の宣言した開始順と割当てだけである。解かない選択は非決定のまま残る。方策はmodelの規則でActorとして切っておらず、Cutのinclude/excludeとSilent Cut検出の対象外である。安全性・契約・時間の結論は、この固定方策が生む経路と環境の全選択についてであり、別の方策の順序・割当てを保証しない。";
pub const FIXED_AFFECTS: &str = "方策の区分の選択は宣言した固定方策で決まる。min/maxは残りの環境の選択の全ての解き方での幅であり、環境は全状態を見られる。方策の最適値ではない。";
pub fn label(v: &Value, expected: Option<&str>) {
    match expected {
        Some(s) => assert_eq!(
            row(v, "probability.scheduler").unwrap(),
            &json!({"field":"probability.scheduler","value":s,"affects":if s=="fixed-policy" {FIXED_AFFECTS} else {"min/max は完全情報の緩和境界で、実現できる方策の値ではない"}})
        ),
        None => assert!(row(v, "probability.scheduler").is_none()),
    }
}
