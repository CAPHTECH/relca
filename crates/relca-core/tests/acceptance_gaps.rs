//! Hand oracles for only V01-10, V01-11 and V02-14's failure-free half.
use relca_core::{engine, ir, lower, outcome::Outcome, reference, report, syntax};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::PathBuf};

const NAMES: [&str; 9] = include!("support/acceptance_gap_names.rs");

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
fn source(name: &str) -> String {
    fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
fn expected(name: &str) -> Value {
    serde_json::from_slice(&fs::read(root().join(name).join("expected.json")).unwrap()).unwrap()
}
fn terminals(model: &ir::Model, out: &Outcome) -> BTreeMap<String, Value> {
    let rows: BTreeMap<_, _> = out.terminals.iter().map(|t| {
        let end = model.ends[t.end.0].clone();
        (end.clone(), json!({"end": end,
            "elapsed": {"min": t.elapsed.min, "max": t.elapsed.max},
            "work": {"min": t.work.min, "max": t.work.max},
            "result_at": t.result_at.map(|r| json!({"min":r.min,"max":r.max})),
            "probability": t.probability.as_ref().map(|p| json!({"min":p.min.to_string(),"max":p.max.to_string()}))}))
    }).collect();
    assert_eq!(rows.len(), out.terminals.len());
    rows
}
fn hand(name: &str, model: &ir::Model, out: &Outcome, h: &Value) {
    assert!(!out.truncated, "{name}");
    let expected_terminals: BTreeMap<_, _> = h["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t["end"].as_str().unwrap().to_owned(), t.clone()))
        .collect();
    assert_eq!(
        terminals(model, out),
        expected_terminals,
        "{name}: hand terminals"
    );
    assert_eq!(
        out.properties.len(),
        h["properties"].as_array().unwrap().len()
    );
    for p in &out.properties {
        let id = match p.property {
            ir::Property::ProbabilityReached(q) => format!("probability reached #{}", q + 1),
            ir::Property::NoResourceOvercommit => "safety no_resource_overcommit".to_owned(),
            ir::Property::Contract(_) => "contract C #1 at_most_once".to_owned(),
            other => panic!("unexpected property: {other:?}"),
        };
        let h = h["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|h| h["id"] == id)
            .unwrap();
        assert_eq!(json!(p.verdict), h["verdict"], "{name}: {}", h["id"]);
        assert_eq!(
            p.probability
                .as_ref()
                .map(|r| json!({"min":r.min.to_string(),"max":r.max.to_string()})),
            h.get("probability").cloned(),
            "{name}: {}",
            h["id"]
        );
    }
}

#[test]
fn closed_nine_inputs_have_five_exact_single_changes() {
    let mut actual: Vec<_> = fs::read_dir(root())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("k8-"))
        .collect();
    actual.sort();
    assert_eq!(actual, NAMES);
    for (good, bad, from, to) in [
        (
            "k8-conditional-error",
            "k8-conditional-error.broken-miss-rate",
            "true -> { 0.20: pass; 0.80: fail }",
            "true -> { 0.25: pass; 0.75: fail }",
        ),
        (
            "k8-conditional-error",
            "k8-conditional-error.broken-tight-upper",
            "<= 0.004",
            "<= 0.003",
        ),
        (
            "k8-common-cause",
            "k8-common-cause.broken-independent-upper",
            "a == fail && b == fail <= 0.1",
            "a == fail && b == fail <= 0.01",
        ),
        (
            "k8-cut-r",
            "k8-cut-r.broken-slow-post",
            "      duration fixed 1min",
            "      duration fixed 2min",
        ),
        (
            "k8-cut-g",
            "k8-cut-g.broken-slow-post",
            "      duration fixed 1min",
            "      duration fixed 2min",
        ),
    ] {
        let s = source(good);
        assert_eq!(s.matches(from).count(), 1);
        assert_ne!(from, to);
        assert_eq!(
            s.replacen(from, to, 1),
            source(bad),
            "one fault only: {bad}"
        );
        assert_eq!(expected(good)["verdict"], "satisfied");
        assert_eq!(expected(bad)["verdict"], "violated");
    }
}

#[test]
fn all_nine_engine_and_reference_results_match_the_hand_oracles_independently() {
    let mut count = 0;
    for name in NAMES {
        let s = source(name);
        let h = expected(name);
        let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(
            r.stop,
            reference::StopReason::Complete,
            "{name}: no reference exclusions"
        );
        hand(&format!("{name}/engine"), &m, &e, &h);
        hand(&format!("{name}/reference"), &m, &r.outcome, &h);
        assert_eq!(json!(r.verdict()), h["verdict"], "{name}");
        let v = json!(report::check(&s));
        assert_eq!(v["verdict"], h["verdict"], "{name}");
        let rows = v["properties"].as_array().unwrap();
        for h in h["properties"].as_array().unwrap() {
            let p = rows.iter().find(|p| p["id"] == h["id"]).unwrap();
            assert_eq!(p["verdict"], h["verdict"], "{name}: {}", h["id"]);
            if let Some(probability) = h.get("probability") {
                assert_eq!(&p["probability"], probability, "{name}: {}", h["id"]);
            }
        }
        count += 1;
    }
    assert_eq!(count, 9);
}

fn assert_same_terminals(r: &Value, g: &Value) {
    assert_eq!(r["terminals"], g["terminals"]);
}

#[test]
fn failure_free_cuts_have_equal_time_and_wrong_registration_not_equal_mechanisms() {
    let r = json!(report::check(&source("k8-cut-r")));
    let g = json!(report::check(&source("k8-cut-g")));
    for v in [&r, &g] {
        assert_eq!(v["verdict"], "satisfied");
        let p = v["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "probability reached #1")
            .unwrap();
        assert_eq!(p["probability"], json!({"min":"1/50","max":"1/50"}));
        assert_eq!(v["terminals"][0]["elapsed"], json!({"min":2,"max":2}));
    }
    assert_same_terminals(&r, &g);
    let mut shifted_g = g.clone();
    for edge in ["min", "max"] {
        let elapsed = shifted_g["terminals"][0]["elapsed"][edge].as_u64().unwrap();
        shifted_g["terminals"][0]["elapsed"][edge] = json!(elapsed + 1);
    }
    assert!(
        std::panic::catch_unwind(|| assert_same_terminals(&r, &shifted_g)).is_err(),
        "the same comparison must reject a synthetic one-minute terminal shift"
    );
    let once = |v: &Value| {
        v["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "contract C #1 at_most_once")
            .unwrap()["notes"]
            .clone()
    };
    assert_eq!(once(&r), json!(["mechanism_unmodeled"]));
    assert_eq!(once(&g), Value::Null);
}
