//! Test-only child of agency. The implementation supplies super::adr0072_run.
//! Do not put this adapter or the frozen enumerator on a public production API.
//! See agency_viewpoint_limit_calculations.md for the exact adapter contract.
use serde_json::{Value, json};
#[path = "agency_viewpoint_limit_inputs.rs"]
mod input;

#[derive(Clone, Debug)]
pub(crate) struct Request {
    pub source: String,
    pub configuration: Option<String>,
    pub scenario: Option<String>,
    pub reference: bool,
    pub block: Option<String>,
    pub counter: String,
    pub limit: usize,
    pub scale: usize,
    pub corruption: Option<String>,
    pub revalidate: bool,
    pub domain_probe: bool,
    pub budget_profile: bool,
}
impl Request {
    fn new(source: String) -> Self {
        Self {
            source,
            configuration: None,
            scenario: None,
            reference: false,
            block: None,
            counter: "work".into(),
            limit: 1_000_000,
            scale: 1,
            corruption: None,
            revalidate: false,
            domain_probe: false,
            budget_profile: false,
        }
    }
}
fn run(r: &Request) -> Value {
    super::adr0072_run(r)
}
fn ok(r: &Request) -> Value {
    let out = run(r);
    assert_eq!(out["status"], "ok", "{r:?}: {out}");
    assert!(
        out["viewpoints_bytes"].as_str().is_some(),
        "must serialize the typed Viewpoints, not a projection"
    );
    assert_eq!(
        serde_json::from_str::<Value>(out["viewpoints_bytes"].as_str().unwrap()).unwrap(),
        out["viewpoints"]
    );
    out
}
fn whole_block_failure(out: &Value, block: &str, counter: &str, limit: usize) {
    let b = &out["viewpoints"][block];
    assert_eq!(b["status"], "not_derived", "{out}");
    assert_eq!(b["reason"], "derivation_limit");
    let record = &b["limit"];
    assert_eq!(record["counter"], counter, "{record}");
    assert_eq!(record["limit"], limit);
    assert!(record["requested"].as_u64().unwrap() > limit as u64);
    assert!(!record["phase"].as_str().unwrap().is_empty());
    for partial in [
        "ends",
        "actors",
        "capabilities",
        "sources",
        "people",
        "rows",
    ] {
        assert!(b.get(partial).is_none(), "no partial {partial}: {b}");
    }
    let warnings = out["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| {
            d["code"] == "W-AGY-5" && d["message"].as_str().is_some_and(|s| s.contains(block))
        })
        .collect::<Vec<_>>();
    assert_eq!(warnings.len(), 1, "one W-AGY-5 per limited block: {out}");
    assert_eq!(warnings[0]["severity"], "warning");
    let text = warnings[0]["message"].as_str().unwrap();
    for token in [
        counter.to_owned(),
        limit.to_string(),
        record["requested"].as_u64().unwrap().to_string(),
        record["phase"].as_str().unwrap().to_owned(),
    ] {
        assert!(text.contains(&token), "missing {token}: {text}");
    }
}

#[test]
fn d_each_viewpoint_limit_is_recorded_warned_and_block_local() {
    let base = Request::new(input::gate(4));
    let full = ok(&base);
    for block in ["observation", "framing", "retention", "effects"] {
        let mut request = base.clone();
        request.block = Some(block.into());
        request.limit = 0;
        let out = ok(&request);
        whole_block_failure(&out, block, "work", 0);
        for other in ["observation", "framing", "retention", "effects", "proposal"] {
            if other != block {
                assert_eq!(
                    out["viewpoints"][other], full["viewpoints"][other],
                    "{block} must not spend {other} budget"
                );
            }
        }
        assert_eq!(
            out["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["code"] == "W-AGY-5")
                .count(),
            1
        );
    }
}

#[test]
fn d_legacy_gate_twelve_boundary_has_two_work_records_and_warnings() {
    let mut r = Request::new(input::gate(12));
    r.reference = true;
    let out = ok(&r);
    whole_block_failure(&out, "framing", "work", 1_000_000);
    whole_block_failure(&out, "retention", "work", 1_000_000);
    assert_eq!(out["viewpoints"]["observation"]["status"], "derived");
    assert_eq!(out["viewpoints"]["effects"]["status"], "derived");
    assert_eq!(
        out["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-AGY-5")
            .count(),
        2
    );
    r.source = input::gate(11);
    let out = ok(&r);
    whole_block_failure(&out, "retention", "work", 1_000_000);
    assert_eq!(out["viewpoints"]["framing"]["status"], "derived");
    assert_eq!(
        out["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-AGY-5")
            .count(),
        1
    );
}

#[test]
fn g8_invalid_ir_is_not_a_budget_exhaustion() {
    for corruption in ["call_capability", "origin_actor"] {
        let mut r = Request::new(input::gate(2));
        r.corruption = Some(corruption.into());
        let out = ok(&r);
        let blocks: &[&str] = if corruption == "call_capability" {
            &["framing", "retention", "effects"]
        } else {
            &["observation"]
        };
        for block in blocks {
            assert_eq!(out["viewpoints"][block]["status"], "not_derived", "{out}");
            assert_eq!(out["viewpoints"][block]["reason"], "invalid_ir", "{out}");
            assert!(
                out["viewpoints"][block].get("limit").is_none(),
                "invalid IR must not invent a budget record"
            );
        }
        assert!(
            out["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .all(|d| d["code"] != "W-AGY-5")
        );
    }
}

#[test]
fn g3_viewpoint_domain_is_not_the_main_possible_results_domain() {
    // A direct domain seam, not a claim that a DSL version_check has no state
    // guard. The synthetic recorded sink has only j1==fail and no state guard.
    // Viewpoint domain={pass}; main possible_results={pass,fail}.
    let mut r = Request::new(input::version_domain());
    r.domain_probe = true;
    let out = run(&r);
    assert_eq!(out["status"], "ok", "{out}");
    assert_eq!(out["main_reachable"], true);
    assert_eq!(
        out["viewpoint_end"],
        json!({"status":"derived","end":"rejected","reachable":false,"selectors":[],"read_only":[]})
    );
}

#[test]
fn g6_scale_never_multiplies_contexts_paths_or_depth() {
    let full = ok(&Request::new(input::joined(3, " || ")));
    for (counter, upper) in [("contexts", 65536), ("paths", 4096), ("depth", 128)] {
        let mut r = Request::new(input::joined(3, " || "));
        r.block = Some("framing".into());
        r.counter = counter.into();
        let mut lo = 0;
        let mut hi = upper;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            r.limit = mid;
            if ok(&r)["viewpoints"]["framing"]["status"] == "derived" {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        assert!(lo > 0, "counter must be used: {counter}");
        for scale in [1, 100] {
            r.scale = scale;
            r.limit = lo;
            assert_eq!(ok(&r)["viewpoints"], full["viewpoints"]);
            r.limit = lo - 1;
            whole_block_failure(&ok(&r), "framing", counter, lo - 1);
        }
    }
    for block in ["observation", "framing", "retention", "effects"] {
        for counter in ["work", "cells"] {
            for scale in [1, 2] {
                let mut r = Request::new(input::gate(4));
                r.block = Some(block.into());
                r.counter = counter.into();
                r.limit = 1;
                r.scale = scale;
                // With one/two units each block still fails before completion.
                whole_block_failure(&ok(&r), block, counter, scale);
            }
        }
    }
}

#[test]
fn cli_all_budget_families_scale_only_work_and_cells() {
    for scale in [1, 100, 1000] {
        let mut r = Request::new(input::gate(2));
        r.scale = scale;
        r.budget_profile = true;
        let out = run(&r);
        assert_eq!(out["status"], "ok");
        let budgets = &out["budgets"];
        for block in [
            "main",
            "observation",
            "framing",
            "retention",
            "effects",
            "common_cause",
        ] {
            assert_eq!(budgets[block]["work"], scale * 1_000_000, "{block}");
            assert_eq!(budgets[block]["cells"], scale * 1_000_000, "{block}");
            assert_eq!(budgets[block]["depth"], 128);
            assert_eq!(
                budgets[block]["paths"],
                if block == "common_cause" { 0 } else { 4096 }
            );
            assert_eq!(
                budgets[block]["contexts"],
                if block == "common_cause" { 0 } else { 65536 }
            );
        }
        assert_eq!(out["degradation_work"], 1_000_000);
    }
}

fn compare(r: &Request, label: &str) {
    let actual = run(r);
    let mut old = r.clone();
    old.reference = true;
    let reference = run(&old);
    assert_eq!(
        actual["status"], reference["status"],
        "{label}: lowering/rejection must agree"
    );
    if reference["status"] == "rejected" {
        assert_eq!(actual["diagnostics"], reference["diagnostics"], "{label}");
        eprintln!("ADR0072 rejected before viewpoints: {label}");
        return;
    }
    assert_eq!(actual["status"], "ok", "{label}: {actual}");
    let blocks = ["observation", "framing", "retention", "effects"];
    for out in [&actual, &reference] {
        assert!(out["viewpoints"].is_object(), "{label}: missing viewpoints");
        let bytes = out["viewpoints_bytes"]
            .as_str()
            .expect("typed serialized viewpoints bytes");
        assert_eq!(
            serde_json::from_str::<Value>(bytes).unwrap(),
            out["viewpoints"],
            "{label}: bytes must cover the entire viewpoints value"
        );
        for block in blocks {
            assert!(
                matches!(
                    out["viewpoints"][block]["status"].as_str(),
                    Some("derived" | "not_derived")
                ),
                "{label}: missing block {block}"
            );
        }
    }
    let has_old_limit = blocks
        .iter()
        .any(|b| reference["viewpoints"][b]["reason"] == "derivation_limit");
    if has_old_limit {
        for block in blocks {
            if reference["viewpoints"][block]["reason"] != "derivation_limit" {
                assert_eq!(
                    actual["viewpoints"][block], reference["viewpoints"][block],
                    "{label}: {block} reference success cannot regress"
                );
            }
        }
        eprintln!("ADR0072 old budget limit; compared all other blocks: {label}");
    } else {
        assert_eq!(
            actual["viewpoints_bytes"], reference["viewpoints_bytes"],
            "{label}: entire typed Viewpoints bytes"
        );
    }
}

#[test]
fn s1_s2_generated_matrix_matches_frozen_enumerator_bytes_without_regression() {
    for (name, source) in input::differential_models() {
        let r = Request::new(source);
        compare(&r, &name);
        let mut generous = r;
        generous.scale = 100;
        compare(&generous, &format!("{name}/generous"));
    }
}

#[test]
fn s1_s2_all_fixture_invocations_match_reference_without_regression() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    // Separately hand-checked population; old inputs and assertions stay intact.
    const SEPARATE: [&str; 10] = [
        "agy-vp-limit-control",
        "agy-vp-limit-control.broken-refusal",
        "agy-vp-scale",
        "agy-vp-scale.broken-default-budget",
        "agy-vp-merge-scale",
        "agy-vp-merge-scale.broken-default-budget",
        "agy-vp-repeat-control",
        "agy-vp-repeat-control.broken-no-leave",
        "agy-vp-response-control",
        "agy-vp-response-control.broken-delivered",
    ];
    for name in SEPARATE {
        assert!(
            root.join(name).is_dir(),
            "registered fixture is missing: {name}"
        );
    }
    let mut directories = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect::<Vec<_>>();
    directories.sort();
    assert!(!directories.is_empty());
    for directory in directories {
        let name = directory.file_name().unwrap().to_str().unwrap();
        if SEPARATE.contains(&name) {
            continue;
        }
        let source = std::fs::read_to_string(directory.join("model.relca")).unwrap();
        let invocation = directory.join("invocation.json");
        let inv: Value = if invocation.is_file() {
            serde_json::from_str(&std::fs::read_to_string(invocation).unwrap()).unwrap()
        } else {
            json!({})
        };
        let mut configs = Vec::new();
        for field in ["configuration", "from", "to"] {
            if let Some(c) = inv[field].as_str() {
                configs.push(Some(c.to_owned()));
            }
        }
        if let Some(plans) = inv["plans"].as_array() {
            configs.extend(plans.iter().map(|c| Some(c.as_str().unwrap().to_owned())));
        }
        if configs.is_empty() {
            configs.push(None);
        }
        configs.sort();
        configs.dedup();
        for configuration in configs {
            let label = format!("{name}/{configuration:?}");
            let mut r = Request::new(source.clone());
            r.configuration = configuration;
            r.scenario = inv["scenario"].as_str().map(str::to_owned);
            compare(&r, &label);
        }
    }
}

#[test]
fn s2_gate_twenty_matches_sufficient_budget_reference_bytes() {
    let request = Request::new(input::gate(20));
    let actual = ok(&request);
    input::assert_gate(&actual["viewpoints"], 20);
    let mut old = request;
    old.reference = true;
    // Test-only scale, NOT an accepted CLI scale. All other limits stay fixed.
    old.scale = 1_000_000;
    let reference = ok(&old);
    input::assert_gate(&reference["viewpoints"], 20);
    assert_eq!(actual["viewpoints_bytes"], reference["viewpoints_bytes"]);
}

#[test]
fn g7_responsibility_and_party_rederive_with_the_ir_scale() {
    for scale in [1, 100] {
        let mut r = Request::new(input::rederivation());
        r.scale = scale;
        r.revalidate = true;
        let out = run(&r);
        assert_eq!(out["status"], "ok", "{out}");
        assert_eq!(out["responsibility_valid"], true, "{out}");
        assert_eq!(out["model_scale"], scale);
        assert_eq!(
            out["agency_derived"],
            scale == 100,
            "the low/high-budget pair must actually straddle the main derivation boundary"
        );
    }
}
