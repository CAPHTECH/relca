//! Scenario runtime acceptance. Expected values are hand-derived interval/CP
//! oracles, never sampled from a new binary. Selection-API probes are separate
//! core tests/support modules.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn rows() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../relca-core/tests/support/scenario_cases.json"
    ))
    .unwrap()
}
fn expected(n: &str) -> Value {
    serde_json::from_slice(
        &fs::read(root().join("fixtures").join(n).join("expected.json")).unwrap(),
    )
    .unwrap()
}
fn invoke(n: &str) -> (Value, Option<i32>) {
    let path = root().join("fixtures").join(n);
    let inv: Value =
        serde_json::from_slice(&fs::read(path.join("invocation.json")).unwrap()).unwrap();
    let inv = fixture_match::Invocation::parse(Some(&inv)).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(inv.args(&path.join("model.relca")))
        .output()
        .unwrap();
    (serde_json::from_slice(&o.stdout).unwrap(), o.status.code())
}
fn population(pairs: &[&str]) {
    let mut failures = Vec::new();
    let mut count = 0;
    for r in rows() {
        if !pairs.contains(&r["pair"].as_str().unwrap()) {
            continue;
        }
        let n = r["name"].as_str().unwrap();
        let e = expected(n);
        let inv = fixture_match::Invocation::parse(Some(&r["invocation"])).unwrap();
        inv.validate_expected(&e)
            .unwrap_or_else(|err| panic!("{n}: malformed hand expectation: {err}"));
        let (actual, exit) = invoke(n);
        let verdict: relca_core::Verdict = serde_json::from_value(e["verdict"].clone()).unwrap();
        if let Err(err) = inv.compare(&e, &actual) {
            failures.push(format!("{n}: {err}"));
        }
        if exit != Some(verdict.exit_code()) {
            failures.push(format!("{n}: exit{exit:?} != {}", verdict.exit_code()));
        }
        count += 1;
    }
    assert!(count > 0, "empty acceptance selection");
    assert!(
        failures.is_empty(),
        "scenario acceptance failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn declarations_fixed_every_selection_and_whole_model_inheritance() {
    population(&["P01", "P02", "P08", "P09", "P12", "P21", "P28"]);
    let (direct, _) = invoke("scn-direct-every");
    assert!(
        direct.get("scenario").is_none(),
        "direct fixed_every is not a scenario declaration"
    );
    let (legacy, _) = invoke("scn-route.broken-no-scenario");
    assert!(legacy.get("scenario").is_none());
    assert!(legacy.get("case_outcomes").is_none());
}
#[test]
fn horizon_clock_cutoff_drain_counts_costs_and_full_window_denominator() {
    population(&[
        "P03", "P03/P14", "P04", "P05", "P14", "P15", "P16", "P17", "P19", "P23", "H34", "H-REJECT",
    ]);
}
#[test]
fn simulation_unknown_not_false_nondeterminism_and_scenario_name_keys() {
    population(&["P06", "P26", "P27"]);
}
#[test]
fn same_scenario_reaches_every_compare_plan_and_both_diff_sides() {
    population(&["P07", "P18", "H-PROJECTION"]);
}
#[test]
fn all_six_selector_entrypoints_and_every_closed_static_arm_fail_closed() {
    population(&["P13", "P13/U6", "P13/U7", "P13/U14"]);
}
#[test]
fn explain_deadline_at_horizon_and_mutate_original_keep_selection() {
    population(&["P24", "P25"]);
    let (v, _) = invoke("scn-explain");
    let c = &v["explanation"]["counterexamples"][0];
    let last = c["steps"].as_array().unwrap().last().unwrap();
    assert_eq!(last["kind"], "horizon");
    assert_eq!(last["time"], 12);
    assert_eq!(c["replay"]["status"], "verified");
}
#[test]
fn representation_and_label_transformations_keep_hand_times_probabilities_and_verdicts() {
    population(&["P10", "P11", "P22"]);
}

#[test]
fn only_source_derived_metadata_slots_differ_not_times_probabilities_or_state_counts() {
    let manifest: Value =
        serde_json::from_str(include_str!("support/scenario_metadata_expectations.json")).unwrap();
    assert_eq!(manifest["pairs"].as_array().unwrap().len(), 3);
    for row in manifest["pairs"].as_array().unwrap() {
        let a = row["left"].as_str().unwrap();
        let b = row["right"].as_str().unwrap();
        let (mut x, ex) = invoke(a);
        let (mut y, ey) = invoke(b);
        assert_eq!(ex, ey);
        assert!(
            x.get("scenario").is_some() && y.get("scenario").is_some(),
            "new syntax runtime prerequisite"
        );
        for (v, which, name) in [(&mut x, "left", a), (&mut y, "right", b)] {
            let hash = Command::new("sha256sum")
                .arg(root().join("fixtures").join(name).join("model.relca"))
                .output()
                .unwrap();
            assert!(hash.status.success());
            let hash = String::from_utf8(hash.stdout).unwrap();
            assert_eq!(
                v["model"]["sha256"],
                hash.split_whitespace().next().unwrap(),
                "source binding"
            );
            v["model"]["sha256"] = json!("source-derived-hash");
            for (p, e) in row["slots"].as_object().unwrap() {
                let ledger_slots: &[&str] = match p.as_str() {
                    "/actors/0/name" => &["/ledger/actor_versions/entries/0/actor"],
                    "/scenario/name" => &[
                        "/ledger/execution/selection/requested/scenario",
                        "/ledger/execution/selection/effective/scenario/value",
                    ],
                    _ => &[],
                };
                for path in ledger_slots {
                    let at = v.pointer_mut(path).unwrap();
                    assert_eq!(*at, e[which], "ledger declaration slot {path}");
                    *at = json!("permitted-metadata");
                }
                let at = v
                    .pointer_mut(p)
                    .unwrap_or_else(|| panic!("missing independent metadata slot {p}"));
                assert_eq!(
                    *at, e[which],
                    "{p}: exact name/metadata relation, not arbitrary deletion"
                );
                *at = json!("permitted-metadata");
            }
            for p in row["source_spans"].as_array().unwrap() {
                let at = v.pointer_mut(p.as_str().unwrap()).unwrap();
                assert!(at["start"].as_u64() <= at["end"].as_u64());
                *at = Value::Null;
            }
        }
        assert_eq!(x, y, "{}: nonmetadata byte-value relation", row["id"]);
        let mut bad = y.clone();
        bad["bounds"]["truncated"] = json!(true);
        assert_ne!(x, bad);
        let mut bad = y.clone();
        bad["other"] = json!({"scenario":{"name":"namesake"}});
        assert_ne!(x, bad);
        let mut bad = y.clone();
        bad["case_outcomes"][0]["arrival"] = json!(99);
        assert_ne!(x, bad);
    }
}

#[test]
fn scenario_name_only_transform_preserves_all_simulation_values_outside_two_exact_name_slots() {
    let (mut x, ex) = invoke("scn-random-name");
    let (mut y, ey) = invoke("scn-random-name.broken-renamed");
    assert_eq!(ex, Some(0));
    assert_eq!(ex, ey);
    for (v, name) in [(&mut x, "X"), (&mut y, "Y")] {
        for path in [
            "/ledger/execution/selection/requested/scenario",
            "/ledger/execution/selection/effective/scenario/value",
        ] {
            let at = v.pointer_mut(path).unwrap();
            assert_eq!(*at, name);
            *at = json!("selected");
        }
        assert_eq!(v["scenario"]["name"], name);
        v["scenario"]["name"] = json!("selected");
        let rows = v["assumptions"].as_array_mut().unwrap();
        let indices: Vec<_> = rows
            .iter()
            .enumerate()
            .filter_map(|(i, r)| (r["field"] == "scenario").then_some(i))
            .collect();
        assert_eq!(indices.len(), 1);
        let at = &mut rows[indices[0]]["value"];
        assert_eq!(*at, name);
        *at = json!("selected");
    }
    assert_eq!(
        x, y,
        "seed, query counts, intervals, terminal counts and source hash are not projected"
    );
}

#[test]
fn closed_names_invocations_layout_and_all_partial_expected_schemas_are_valid() {
    let rows = rows();
    let mut names: Vec<_> = rows.iter().map(|r| r["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, fixture_match::SCENARIO_FIXTURES);
    for n in names {
        let path = root().join("fixtures").join(n);
        let src = fs::read_to_string(path.join("model.relca")).unwrap();
        assert!(
            !src.contains("//") && !src.contains('#'),
            "{n}: Relca has no comment syntax"
        );
        assert!(path.join("calc.md").is_file());
        let inv: Value =
            serde_json::from_slice(&fs::read(path.join("invocation.json")).unwrap()).unwrap();
        fixture_match::Invocation::parse(Some(&inv))
            .unwrap()
            .validate_expected(&expected(n))
            .unwrap_or_else(|e| panic!("{n}: {e}"));
        if let Some((base, _)) = n.split_once(".broken-") {
            assert!(fixture_match::SCENARIO_FIXTURES.contains(&base));
        } else {
            assert!(
                fixture_match::SCENARIO_FIXTURES
                    .iter()
                    .any(|m| m.starts_with(&format!("{n}.broken-")))
            );
        }
    }
}

#[test]
fn cutoff_wire_guard_is_calibrated_without_a_new_implementation() {
    let mut v = expected("scn-horizon");
    // Isolated closed ADR0042 guard: other complete-wire obligations are
    // independently covered by the existing Invocation comparator.
    v.as_object_mut().unwrap().remove("metrics");
    v["assumptions"] = json!([
        {"field":"scenario","value":"Day","affects":"common conditions"},
        {"field":"scenario.horizon","value":"12min","affects":"suffix remains unknown"},
        {"field":"scenario.drain","value":"4..12","affects":"declared interval"},
        {"field":"scenario.aggregation","value":"0..stop","affects":"warmup0"}]);
    fixture_match::validate_scenario_wire(&v).unwrap();
    for (pointer, new) in [
        ("/case_outcomes/2/terminals/0/work", Value::Null),
        (
            "/case_outcomes/2/terminals/0/completed_at",
            json!({"min":12,"max":12}),
        ),
        ("/case_outcomes/2/terminals/0/end", json!("done")),
        ("/scenario/drain/from", json!(6)),
        ("/scenario/arrivals/count", json!(4)),
        ("/scenario/horizon", json!(0)),
        ("/scenario/warmup", json!(1)),
        ("/scenario/aggregation/observed_until/max", Value::Null),
        ("/scenario/aggregation/observed_until/min", json!("13")),
    ] {
        let mut bad = v.clone();
        let at = bad.pointer_mut(pointer).unwrap();
        assert_ne!(*at, new);
        *at = new;
        assert!(
            fixture_match::validate_scenario_wire(&bad).is_err(),
            "wire fault {pointer}"
        );
    }
    let mut bad = v.clone();
    bad["scenario"]["unknown"] = json!(1);
    assert!(fixture_match::validate_scenario_wire(&bad).is_err());
    let old: Value = serde_json::from_str(include_str!(
        "../../../fixtures/arrivals-staggered/expected.json"
    ))
    .unwrap();
    let mut bad = old.clone();
    bad["scenario"] = v["scenario"].clone();
    assert!(
        fixture_match::validate_scenario_wire(&bad).is_err(),
        "synthetic scenario on old report"
    );
    let mut bad = old;
    bad["case_outcomes"][0]["terminals"][0] = v["case_outcomes"][2]["terminals"][0].clone();
    assert!(
        fixture_match::validate_scenario_wire(&bad).is_err(),
        "synthetic cutoff without horizon"
    );
}

#[test]
fn actual_horizon_wire_rejects_unknown_keys_scope_changes_and_double_counting() {
    for n in ["scn-horizon", "scn-sim"] {
        let (v, _) = invoke(n);
        let inv = fixture_match::Invocation::parse(Some(
            &json!({"command":if n=="scn-sim" {"simulate"} else {"check"},"scenario":"Day"}),
        ))
        .unwrap();
        inv.compare(&json!({}), &v).unwrap();
        assert!(v.get("scenario").is_some(), "new runtime prerequisite");
        let mut bad = v.clone();
        bad["metrics"]["observation_scope"] = json!("until_execution_stop_no_extra_drain");
        assert!(inv.compare(&json!({}), &bad).is_err());
        let mut bad = v.clone();
        bad["scenario"]["unknown"] = json!(0);
        assert!(inv.compare(&json!({}), &bad).is_err());
        if n == "scn-sim" {
            let mut bad = v.clone();
            bad["case_outcomes"][2]["unfinished_count"] = json!(4);
            assert!(inv.compare(&json!({}), &bad).is_err());
            let mut bad = v.clone();
            bad["simulation"]["cut_off_replications"] = json!(5);
            assert!(inv.compare(&json!({}), &bad).is_err());
        }
    }
}

fn temporary_source(name: &str, source: &str) -> PathBuf {
    let dir = root()
        .join("target/scenario-acceptance")
        .join(std::process::id().to_string());
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.relca"));
    fs::write(&path, source).unwrap();
    path
}
fn raw(args: &[&str]) -> (Value, Option<i32>) {
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .output()
        .unwrap();
    (serde_json::from_slice(&o.stdout).unwrap(), o.status.code())
}

#[test]
fn renamed_small_exact_scenario_is_simulatable_not_a_magic_analysis_mode() {
    let source =
        fs::read_to_string(root().join("fixtures/scn-simulation-fifo/model.relca")).unwrap();
    let changed = source.replace("scenario Busy {", "scenario SmallExact {");
    assert_ne!(source, changed);
    let path = temporary_source("small-exact", &changed);
    let (v, exit) = raw(&[
        "simulate",
        path.to_str().unwrap(),
        "--scenario",
        "SmallExact",
    ]);
    assert_eq!(exit, Some(1));
    assert_eq!(v["scenario"]["name"], "SmallExact");
    assert_eq!(v["simulation"]["completed_replications"], 4);
    let cases = v["properties"][0]["cases"].as_array().unwrap();
    assert_eq!(
        cases
            .iter()
            .map(|c| c["hits"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [4, 0, 0]
    );
}

#[test]
fn mutation_selector_reaches_every_edited_ast_not_only_the_original() {
    let source = fs::read_to_string(root().join("fixtures/mut-guard/model.relca")).unwrap();
    let start = source.find("bounds {").unwrap();
    let end = source[start..].find('}').unwrap() + start + 1;
    let bounds = &source[start..end];
    let changed = source.replacen(bounds, &format!("scenario One {{ {bounds} }}"), 1);
    assert_ne!(source, changed);
    let path = temporary_source("mut-guard", &changed);
    let (v, exit) = raw(&["mutate", path.to_str().unwrap(), "--scenario", "One"]);
    assert_eq!(exit, Some(4));
    assert_eq!(v["original"]["scenario"]["name"], "One");
    fixture_match::compare_mutation_fixture_sites("mut-guard", &v).unwrap();
    for row in v["mutations"].as_array().unwrap() {
        for mutant in row["mutants"].as_array().unwrap() {
            if !mutant["report"].is_null() {
                assert_eq!(mutant["report"]["scenario"]["name"], "One");
            }
        }
    }
    assert!(
        v["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| !r["mutants"].as_array().unwrap().is_empty())
    );
}

#[test]
fn cli_scenario_flag_misuse_is_syntax_invalid_and_flag_order_is_independent() {
    let p = root().join("fixtures/scn-fixed-every/model.relca");
    let p = p.to_str().unwrap();
    for flags in [
        vec!["--scenario"],
        vec!["--scenario=Light"],
        vec!["--scenario", ""],
        vec!["--scenario", "Light", "--scenario", "Light"],
        vec!["--unknown", "Light"],
    ] {
        let mut args = vec!["check", p];
        args.extend(flags);
        let (v, exit) = raw(&args);
        assert_eq!(exit, Some(3));
        assert_eq!(v["diagnostics"][0]["code"], "E-SYN-0");
    }
    let p = root().join("fixtures/scn-compare/model.relca");
    let p = p.to_str().unwrap();
    let (a, ea) = raw(&["check", p, "--configuration", "Slow", "--scenario", "Light"]);
    let (b, eb) = raw(&["check", p, "--scenario", "Light", "--configuration", "Slow"]);
    assert_eq!(ea, Some(0));
    assert_eq!(eb, Some(0));
    assert_eq!(a, b);
}

#[test]
#[ignore = "run explicitly after implementation with a release binary; 120 cases x1000 replications, never a reduced workload"]
fn busyday_release_120_cases_1000_replications_complete_within_60_seconds() {
    let binary = std::env::var_os("RELCA_SCENARIO_PERF_BINARY").expect("set release binary path");
    let binary = Path::new(&binary);
    assert!(binary.is_file());
    let input = root().join("crates/relca-core/tests/support/scenario_busyday.relca");
    let started = Instant::now();
    let o = Command::new(binary)
        .args(["simulate", input.to_str().unwrap(), "--scenario", "BusyDay"])
        .output()
        .unwrap();
    let elapsed = started.elapsed();
    eprintln!(
        "BusyDay 120x1000 elapsed={elapsed:?}; binary={}",
        binary.display()
    );
    assert!(
        elapsed <= Duration::from_secs(60),
        "no workload reduction allowed"
    );
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    fixture_match::validate_simulation_wire(&v).unwrap();
    assert_eq!(v["scenario"]["arrivals"]["count"], 120);
    assert_eq!(v["scenario"]["drain"]["from"], 476);
    assert_eq!(v["simulation"]["completed_replications"], 1000);
    assert_eq!(v["simulation"]["truncated_replications"], 0);
    assert_eq!(v["simulation"]["cut_off_replications"], 0);
    assert_eq!(v["case_outcomes"].as_array().unwrap().len(), 120);
}

fn contest_horizon(horizon: u64) -> (Value, Option<i32>) {
    // ctx-contest-origin as a scenario: arrival 0, judge 4min, route 2min,
    // respond_by 2min (boundary review M2 input).
    let src = format!(
        r#"model ContestRuntime version "0.2" {{
  type T = enum {{ pass, fail }}
  people [j,p]
  actor Judge {{ executors [j]; capability judge() -> T {{ duration fixed 4min; result nondet {{ fail }} }} }}
  actor Route {{ executors [p]; capability respond() -> T {{ duration fixed 2min; result nondet {{ pass }} }} }}
  actor Party {{ control party; standing over [end.returned] basis "claim" }}
  permit j to Judge.judge
  permit p to Route.respond
  authority G {{ basis "decision"; grants j to Judge.judge }}
  process Flow {{ d = Judge.judge(); if d == fail {{ end returned }} else {{ end posted }} }}
  cut C {{ include [Judge,Route,Party] }}
  scenario Day {{
    arrivals fixed [0min]
    policy fifo tie_by case_id
    bounds {{ cases 1; live_tasks 2; clock 30min }}
    horizon {horizon}min
  }}
  verify safety no_resource_overcommit
  contract Controls {{ contest Party on end.returned route Route.respond reversible_by [j] respond_by 2min }}
}}
"#
    );
    let path = test_tmp::root().join(format!(
        "scn-review-contest-h{horizon}-{}.relca",
        std::process::id()
    ));
    fs::write(&path, src).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(["check", path.to_str().unwrap(), "--scenario", "Day"])
        .output()
        .unwrap();
    fs::remove_file(&path).unwrap();
    (serde_json::from_slice(&o.stdout).unwrap(), o.status.code())
}

#[test]
fn horizon_leaf_after_business_end_keeps_reachability_unchecked_on_the_wire() {
    // Boundary review M2. D=5: the filed branch stops at 5 with the route
    // running (4..6, deadline 6 > 5). No cutoff row (End at 4), yet the suffix
    // after 5 is not inspected: unchecked unreachable, no W-UNREACH.
    let (v, exit) = contest_horizon(5);
    assert_eq!(exit, Some(2));
    assert_eq!(v["verdict"], "inconclusive");
    assert_eq!(v["bounds"]["truncated"], false);
    assert_eq!(v["unchecked"], json!(["unreachable"]));
    assert!(
        !v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"].as_str().unwrap().starts_with("W-UNREACH-"))
    );
    let terminals = v["case_outcomes"][0]["terminals"].as_array().unwrap();
    assert_eq!(terminals.len(), 1);
    assert_eq!(terminals[0]["kind"], "end");
    assert_eq!(
        v["contest_runtime"][0]["alternatives"][1]["status"],
        "unresolved"
    );
    fixture_match::validate_completion_wire(&v).unwrap();
    let mut bad = v.clone();
    bad.as_object_mut().unwrap().remove("unchecked");
    assert!(
        fixture_match::validate_completion_wire(&bad).is_err(),
        "the horizon leaf must require the unchecked marker"
    );
    // Control D=6: the route finish at 6 closes in the layer at D, no leaf
    // stops at the horizon although observed_until.max equals D.
    let (w, exit) = contest_horizon(6);
    assert_eq!(exit, Some(0));
    assert_eq!(w["verdict"], "satisfied");
    assert!(w.get("unchecked").is_none());
    assert_eq!(w["scenario"]["aggregation"]["observed_until"]["max"], "6");
    fixture_match::validate_completion_wire(&w).unwrap();
    let mut bad = w.clone();
    bad["unchecked"] = json!(["unreachable"]);
    assert!(fixture_match::validate_completion_wire(&bad).is_err());
}
