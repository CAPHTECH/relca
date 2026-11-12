//! Hand-derived review crossings. No production result supplies an expectation.
#![allow(dead_code)]
use serde_json::{Value, json};

pub const PLANS: &[&str] = &["AsIs", "ToBeP"];
const BASE: &str = include_str!("../../../../fixtures/eval-agency/model.relca");

pub fn cut_source(case: &str) -> String {
    let with_d = BASE.replace(
        "  cut C { include [Office] }",
        "  cut C { include [Office] }\n  cut D { include [Office] }",
    );
    match case {
        "cross" => with_d.replace(
            "configuration ToBeP { cut C;",
            "configuration ToBeP { cut D;",
        ),
        "no-declared-cut" => {
            cut_source("cross").replace("compare [AsIs, ToBeP] cut C", "compare [AsIs, ToBeP]")
        }
        "declared-only" => {
            with_d.replace("compare [AsIs, ToBeP] cut C", "compare [AsIs, ToBeP] cut D")
        }
        _ => panic!("unlisted Cut crossing {case}"),
    }
}

pub fn cut_hand(case: &str, out: &Value) {
    assert_eq!(out["verdict"], "invalid");
    assert!(out.get("evaluation").is_none());
    assert!(out["tables"].get("agency").is_none());
    for p in out["plans"].as_array().unwrap() {
        assert_eq!(p["report"]["verdict"], "satisfied", "unchanged substrate");
    }
    let diagnostics = out["diagnostics"].as_array().unwrap();
    assert!(diagnostics.iter().all(|d| d["code"] == "E-CMP-0"));
    let declared = "compare cut differs from plan cut";
    let between = "configurations AsIs and ToBeP have different Cuts";
    let messages: Vec<_> = diagnostics
        .iter()
        .map(|d| d["message"].as_str().unwrap())
        .collect();
    assert_eq!(
        messages,
        match case {
            "cross" => vec![declared, between],
            "no-declared-cut" => vec![between],
            "declared-only" => vec![declared],
            _ => unreachable!(),
        }
    );
    if case == "declared-only" {
        assert_eq!(out["mismatch"], Value::Null, "no fabricated plan mismatch");
    } else {
        let pairs = out["mismatch"]["pairs"].as_array().unwrap();
        assert_eq!(pairs.len(), 1);
        let p = &pairs[0];
        assert_eq!(p["left"], "AsIs");
        assert_eq!(p["right"], "ToBeP");
        assert_eq!(p["cut"]["left_cut"], "C");
        assert_eq!(p["cut"]["right_cut"], "D");
        assert_eq!(p["terminals"], Value::Null);
        // Name inequality lives in left_cut/right_cut, not a fabricated field row.
        assert_eq!(p["cut"]["differences"], json!([]));
    }
}

pub fn weighted_source(block: &str, word: &str) -> String {
    assert!(["hard", "objectives"].contains(&block));
    assert!(["weight", "score"].contains(&word));
    let source = include_str!("../../../../fixtures/eval-hard/model.relca");
    let row = if block == "hard" {
        "cohort.WrongPerArrival <= 0.05"
    } else {
        "minimize metrics.overall.work"
    };
    assert!(source.contains(row));
    source.replace(row, &format!("{row} {word} 1"))
}

pub fn pareto_hand(out: &Value) {
    assert_eq!(out["verdict"], "satisfied");
    assert!(out.get("evaluation").is_some());
    let a = out["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["field"] == "compare.pareto")
        .unwrap();
    let text = a["affects"].as_str().unwrap();
    assert!(text.contains("Objectives are not added."));
    assert!(text.contains("Simulation dominance is not computed."));
    assert!(
        text.ends_with("Shared observations: none among the declared objectives."),
        "explicit absence, not a blank suffix: {text}"
    );
}
