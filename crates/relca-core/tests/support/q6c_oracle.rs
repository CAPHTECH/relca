//! Hand-only projections and exact inventories; no production collector calls.
#![allow(dead_code)]
use serde_json::{Value, json};
use std::collections::BTreeSet;
pub fn keys(v: &Value, names: &[&str]) {
    let actual: BTreeSet<_> = v
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(actual, names.iter().copied().collect(), "closed keys: {v}");
}
pub fn profile(name: &str, boundary: u64) -> Value {
    json!({"name":name,"basis":"selector assumption","evidence_status":"analyst_declared_not_validated",
        "load":"queued_same_actor_at_start","snapshot":"start","duration_effect":"not_supported",
        "rows":[{"executor":"q","lower":0,"upper":boundary,"kernel":"Low"},
                {"executor":"q","lower":boundary,"upper":null,"kernel":"Half"}]})
}
fn difference(kind: &str, target: &str, from: Value, to: Value) -> Value {
    let change = if from.is_null() {
        "added"
    } else if to.is_null() {
        "removed"
    } else {
        "modified"
    };
    json!({"kind":kind,"target":target,"change":change,"reason":null,"loses":null,"from":from,"to":to})
}
pub fn changes(kind: &str, reverse: bool) -> Vec<Value> {
    let mut rows = match kind {
        "work" => vec![
            difference("qualification", "Desk.run", Value::Null, json!(["q"])),
            difference("performance", "Desk.run", Value::Null, profile("P", 1)),
        ],
        "bands" => vec![
            difference("qualification", "Desk.run", json!(["q"]), Value::Null),
            difference("qualification", "Desk.alternate", Value::Null, json!(["q"])),
            difference("performance", "Desk.run", profile("P", 1), Value::Null),
            difference(
                "performance",
                "Desk.alternate",
                Value::Null,
                profile("Q", 2),
            ),
        ],
        "degrade" => vec![difference(
            "degradation",
            "Manual",
            Value::Null,
            json!({"condition":{"binding":"status","value":"fail"},"producer":"Probe.check"}),
        )],
        _ => panic!("unknown comparison {kind}"),
    };
    if reverse {
        for r in &mut rows {
            let a = r["from"].take();
            r["from"] = r["to"].take();
            r["to"] = a;
            r["change"] = json!(if r["from"].is_null() {
                "added"
            } else if r["to"].is_null() {
                "removed"
            } else {
                "modified"
            });
        }
    }
    rows.sort_by_key(Value::to_string);
    rows
}
pub fn assert_changes(v: &Value, kind: &str, reverse: bool) {
    let mut rows: Vec<_> = v
        .as_array()
        .expect("configuration differences")
        .iter()
        .filter(|r| {
            matches!(
                r["kind"].as_str(),
                Some("qualification" | "performance" | "degradation")
            )
        })
        .cloned()
        .collect();
    rows.sort_by_key(Value::to_string);
    assert_eq!(
        rows,
        changes(kind, reverse),
        "explicit Q6 configuration changes"
    );
}
pub fn no_errors(v: &Value) {
    let errors: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert!(errors.is_empty(), "unrelated error: {errors:?}");
}
pub fn probability(v: &Value, want: &str) {
    let rows: Vec<_> = v["properties"]
        .as_array()
        .expect("properties")
        .iter()
        .filter(|p| p["id"] == "probability reached #1")
        .collect();
    assert_eq!(rows.len(), 1);
    if let Some(cases) = rows[0]["cases"].as_array() {
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0]["probability"], json!({"min":want,"max":want}));
    } else {
        assert_eq!(rows[0]["probability"], json!({"min":want,"max":want}));
    }
}
pub fn two_minutes(v: &Value) {
    let terminals = v["terminals"].as_array().expect("terminals");
    if !terminals.is_empty() {
        for t in terminals {
            for field in ["work", "elapsed"] {
                assert_eq!(
                    t[field],
                    json!({"min":2,"max":2}),
                    "fixed duration, not inferred performance"
                );
            }
        }
    } else {
        let cases = v["case_outcomes"].as_array().expect("case outcomes");
        assert_eq!(cases.len(), 1);
        let terminals = cases[0]["terminals"].as_array().expect("case terminals");
        assert_eq!(terminals.len(), 1);
        for field in ["work", "elapsed"] {
            assert_eq!(
                terminals[0][field],
                json!({"min":2,"max":2}),
                "fixed duration: {cases:?}"
            );
        }
    }
}
pub fn diff(v: &Value, kind: &str, reverse: bool) {
    no_errors(v);
    assert_eq!(v["verdict"], "satisfied", "{}", v["diagnostics"]);
    assert_changes(&v["differences"], kind, reverse);
    if kind == "degrade" {
        missing_side(&v["agency"]["differences"], reverse);
        scope(&v[if reverse { "from" } else { "to" }]["report"]);
    } else {
        for side in ["from", "to"] {
            probability(&v[side]["report"], "1/10");
            two_minutes(&v[side]["report"]);
        }
    }
}
pub fn compare(v: &Value, kind: &str, reverse: bool) {
    no_errors(v);
    assert_eq!(v["verdict"], "satisfied", "{}", v["diagnostics"]);
    let pairs = v["tables"]["configuration"]
        .as_array()
        .expect("configuration pairs");
    assert_eq!(pairs.len(), 1);
    keys(&pairs[0], &["from", "to", "differences"]);
    assert_eq!(pairs[0]["from"], if reverse { "To" } else { "From" });
    assert_eq!(pairs[0]["to"], if reverse { "From" } else { "To" });
    assert_changes(&pairs[0]["differences"], kind, reverse);
    if kind == "degrade" {
        missing_side(&v["tables"]["agency"][0]["differences"], reverse);
        scope(&v["plans"][if reverse { 0 } else { 1 }]["report"]);
    } else {
        for p in v["plans"].as_array().unwrap() {
            probability(&p["report"], "1/10");
            two_minutes(&p["report"]);
        }
    }
}
pub fn scope(v: &Value) {
    assert_eq!(
        v["agency"]["degradation"]["scope"],
        "structural_guard_paths"
    );
    let rows: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["field"] == "agency.retention.normal.scope")
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["value"], "whole_process_paths");
    let ret = &v["agency"]["viewpoints"]["retention"];
    assert_eq!(ret["status"], "derived");
    for (cap, min, max) in [
        ("Probe.check", 1, 1),
        ("Clerk.enter", 0, 1),
        ("Fast.run", 0, 1),
    ] {
        let rows: Vec<_> = ret["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["capability"] == cap)
            .collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["normal"], json!({"min":min,"max":max}));
    }
}
pub fn missing_side(v: &Value, reverse: bool) {
    let rows: Vec<_> = v["rows"]
        .as_array()
        .expect("agency rows")
        .iter()
        .filter(|r| r["viewpoint"] == "degradation")
        .collect();
    assert_eq!(rows.len(), 3, "absent table is not an empty equality");
    for side in ["normal", "degraded"] {
        let r = rows.iter().find(|r| r["item"] == side).unwrap();
        let absent = if reverse { "to" } else { "from" };
        let known = if reverse { "from" } else { "to" };
        assert_eq!(r["call"], "Manual");
        assert_eq!(r["status"], "not_comparable");
        assert_eq!(
            r[absent],
            json!({"status":"not_derived","reason":format!("{absent}.agency.degradation:null")})
        );
        assert_eq!(r[known], json!({"status":"derived","reason":null}));
    }
}
pub fn kernel(name: &str, kind: &str) -> Value {
    let (yes, no) = if name == "Half" {
        ("1/2", "1/2")
    } else if kind == "probability" {
        ("3/4", "1/4")
    } else {
        ("9/10", "1/10")
    };
    let bad = if name == "Low" && kind == "value" {
        "false"
    } else {
        "true"
    };
    let basis = if name == "Half" {
        "high assumption"
    } else if kind == "basis" {
        "changed low assumption"
    } else {
        "low assumption"
    };
    json!({"name":name,"basis":basis,"given":[],"rows":[{"conditions":[],"branches":[
        {"probability":yes,"result":"ok","writes":[{"variable":"bad","value":"false"}]},
        {"probability":no,"result":"bad","writes":[{"variable":"bad","value":bad}]}
    ]}]})
}
pub fn effective(v: &Value, kind: &str) {
    let op = v["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["capability"] == "Desk.run")
        .unwrap();
    assert!(
        op["kernel"].is_null(),
        "ordinary and selector result sources are exclusive"
    );
    assert_eq!(op["duration"], json!({"kind":"fixed","minutes":2}));
    let mut expected = json!({"profile":"P","basis":if kind=="basis" {"changed selector assumption"}else{"selector assumption"},
        "load":"queued_same_actor_at_start","snapshot":"start","duration_effect":"not_supported",
        "rows":[{"executor":"q","lower":0,"upper":1,"kernel":kernel("Low",kind)},
                {"executor":"q","lower":1,"upper":null,"kernel":kernel("Half",kind)}]});
    for row in expected["rows"].as_array_mut().unwrap() {
        for branch in row["kernel"]["rows"][0]["branches"].as_array_mut().unwrap() {
            branch["writes"][0]["variable"] = json!("case.latent.bad");
        }
    }
    if kind == "band" {
        expected["rows"][0]["upper"] = json!(2);
        expected["rows"][1]["lower"] = json!(2);
    }
    if kind == "qualification" {
        for row in expected["rows"].as_array_mut().unwrap() {
            row["executor"] = json!("p");
        }
    }
    assert_eq!(
        op["performance"], expected,
        "complete effective selector, not a null kernel placeholder"
    );
}
pub fn uncertainty(v: &Value, kind: &str) {
    let items = v["uncertainty"]["aleatory"]["items"].as_array().unwrap();
    let rows: Vec<_> = items
        .iter()
        .filter(|r| r["kind"] == "kernel" && r["capability"] == "Desk.run")
        .collect();
    assert_eq!(rows.len(), 2);
    for name in ["Low", "Half"] {
        let row = rows.iter().find(|r| r["name"] == name).unwrap();
        let mut actual = (*row).clone();
        assert!(actual.as_object_mut().unwrap().remove("span").is_some());
        let mut expected = kernel(name, kind);
        expected["kind"] = json!("kernel");
        expected["capability"] = json!("Desk.run");
        assert_eq!(actual, expected, "all exact distributions, not an average");
    }
    let context: Vec<_> = v["uncertainty"]["model"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == "model_context" && r["at"] == "/work_profiles")
        .collect();
    assert_eq!(context.len(), 1);
}
pub fn suite(v: &Value, kinds: &[&str]) {
    no_errors(v);
    assert_eq!(v["verdict"], "satisfied", "{}", v["diagnostics"]);
    let points = v["points"].as_array().unwrap();
    assert_eq!(points.len(), kinds.len());
    for (p, kind) in points.iter().zip(kinds) {
        assert_eq!(p["id"], *kind);
        assert_eq!(p["status"], "evaluated");
        probability(
            &p["report"],
            match *kind {
                "probability" => "1/4",
                "value" => "0",
                _ => "1/10",
            },
        );
        two_minutes(&p["report"]);
        effective(&p["effective_inputs"], kind);
        uncertainty(&p["report"], kind);
    }
}
/// Inspect the COMPLETE diagnostic collection, not one matching member.
pub fn refusal(v: &Value, mutate: bool, conform: bool) {
    assert_eq!(v["verdict"], "unsupported");
    let owner = if mutate { &v["original"] } else { v };
    let ds = owner["diagnostics"].as_array().unwrap();
    assert_eq!(ds.len(), 1, "no incidental errors or warnings: {ds:?}");
    assert_eq!(ds[0]["code"], "U-SEM-0");
    assert_eq!(ds[0]["severity"], "error");
    if conform {
        assert_eq!(ds[0]["reason"], "q6_feature_combination");
    } else {
        assert_eq!(
            ds[0]["message"]
                .as_str()
                .unwrap()
                .split(':')
                .next()
                .unwrap(),
            "q6_feature_combination"
        );
    }
    if mutate {
        assert!(v["diagnostics"].as_array().unwrap().is_empty());
    }
}
pub fn sensitivity_refusal(p: &Value) {
    let v = &p["report"];
    assert_eq!(v["verdict"], "unsupported");
    let id = p["id"].as_str().unwrap();
    assert_eq!(
        v["diagnostics"],
        json!([{"code":"U-SEM-0","severity":"error","span":null,
        "message":format!("sensitivity_feature_combination: point {id:?}: contract projection: q6_feature_combination: degradation sensitivity not connected")}])
    )
}
