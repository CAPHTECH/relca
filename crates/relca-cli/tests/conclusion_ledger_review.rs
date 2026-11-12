//! ADR 0058 review acceptance. Hand expectations are in the adjacent calculations.
#[path = "support/ledger_harness.rs"]
mod h;
use h::{Spec, known, oracle};
use relca_core::{Verdict, report};
use serde_json::{Value, json};
use std::{process::Command, sync::OnceLock};

fn diff_control() -> Value {
    let mut v = serde_json::to_value(relca_core::diff::diff(h::PARALLEL, "AsIs", "ToBeP")).unwrap();
    assert_eq!(v["from"]["report"]["actors"], v["to"]["report"]["actors"]);
    for (side, configuration, process) in [("from", "AsIs", "Serial"), ("to", "ToBeP", "Parallel")]
    {
        let child = &mut v[side]["report"];
        child["ledger"] = oracle::expected(child, &h::selected(process, configuration)).unwrap();
    }
    v["ledger"] = oracle::expected(&v, &Spec::aggregate("diff")).unwrap();
    let typed: relca_core::diff::DiffReport =
        serde_json::from_value(v.clone()).expect("M1 positive control: independent ledgers decode");
    assert_eq!(serde_json::to_value(typed).unwrap(), v);
    v
}

#[test]
fn m1_diff_rejects_swapped_child_ledgers_with_identical_actor_tables() {
    let mut bad = diff_control();
    let from = bad["from"]["report"]["ledger"].take();
    bad["from"]["report"]["ledger"] = bad["to"]["report"]["ledger"].take();
    bad["to"]["report"]["ledger"] = from;
    assert!(
        serde_json::from_value::<relca_core::diff::DiffReport>(bad).is_err(),
        "M1_SWAPPED_LEDGER_ACCEPTED"
    );
}

#[test]
fn m1_diff_rejects_child_configuration_and_process_forgery() {
    let control = diff_control();
    let mut escaped = Vec::new();
    // Known limitation: effective.process has no independent ownership witness.
    // The root requested selection has no process, and the two sides can compare
    // different processes (Serial and Parallel here). Only those two process-forgery
    // pairs are excluded, without adding wire fields.
    for side in ["from", "to"] {
        for (path, value) in [
            ("requested/configuration", json!("Unrelated")),
            ("effective/configuration", known("Unrelated")),
        ] {
            let mut bad = control.clone();
            let pointer = format!("/{side}/report/ledger/execution/selection/{path}");
            *bad.pointer_mut(&pointer).unwrap() = value;
            if serde_json::from_value::<relca_core::diff::DiffReport>(bad).is_ok() {
                escaped.push(pointer);
            }
        }
    }
    assert!(
        escaped.is_empty(),
        "M1_SELECTION_FORGERY_ACCEPTED: {escaped:?}"
    );
}

fn prelower_control() -> Value {
    let mut v = json!({"uncertainty":{"rules":"uncertainty-p2-1",
        "aleatory":{"status":"unavailable","reason":"input_unavailable","items":[]},
        "estimation":{"status":"not_applicable","reason":"exhaustive_analysis","items":[]},
        "model":{"status":"unavailable","reason":"input_unavailable","items":[]}},
        "actors":null,"cut_context":{"notices":[],"reentry":[],"horizon":[]},
        "agency":null,"verdict":"invalid","mode":"exhaustive","unit":"min",
        "model":{"name":null,"sha256":null},"cut":"undeclared","assumptions":[],
        "diagnostics":[{"code":"E-SYN-0","severity":"error","message":"unreadable","span":null}],
        "bounds":{"cases":null,"live_tasks":null,"clock":null,"truncated":null},
        "properties":[],"terminals":[]});
    v["ledger"] = oracle::expected(&v, &Spec::prelower("exhaustive", true)).unwrap();
    serde_json::from_value::<report::Report>(v.clone()).expect("M2 positive prelower control");
    v
}

#[test]
fn minor_a_diff_rejects_false_unlowered_child_selection() {
    let control = diff_control();
    let mut escaped = Vec::new();
    for status in ["ran", "not_run"] {
        let mut lowered = control.clone();
        for side in ["from", "to"] {
            assert!(lowered[side]["report"]["actors"].is_array());
            let child = &mut lowered[side]["report"]["ledger"];
            child["execution"]["status"] = json!(status);
            child["backends"]["builtin"]["status"] = json!(status);
            if status == "not_run" {
                child["execution"]["limits"]["state_budget"] =
                    oracle::unavailable("execution_settings_unavailable");
            }
        }
        serde_json::from_value::<relca_core::diff::DiffReport>(lowered.clone())
            .expect("lowered control, including refusal before exploration");
        for side in ["from", "to"] {
            let mut bad = lowered.clone();
            let child = &mut bad[side]["report"]["ledger"];
            // Only these three child-ledger fields change. The Report's Actor
            // table and the recorded execution status remain independent evidence.
            child["actor_versions"] =
                json!({"status":"unavailable","entries":[],"reason":"not_lowered"});
            child["execution"]["selection"]["requested"]["configuration"] = Value::Null;
            child["execution"]["selection"]["effective"]["configuration"] =
                oracle::unavailable("not_lowered");
            if serde_json::from_value::<relca_core::diff::DiffReport>(bad).is_ok() {
                escaped.push(format!("{side}/{status}"));
            }
        }
    }
    assert!(
        escaped.is_empty(),
        "MINOR_A_FALSE_UNLOWERED_CHILD_ACCEPTED: {escaped:?}"
    );
}

#[test]
fn minor_a_diff_keeps_genuinely_unlowered_children_with_unknown_selection() {
    let mut control = diff_control();
    for side in ["from", "to"] {
        control[side]["report"] = prelower_control();
        assert_eq!(control[side]["report"]["actors"], Value::Null);
        assert_eq!(
            control[side]["report"]["ledger"]["execution"]["status"],
            "not_run"
        );
    }
    control["ledger"] = oracle::expected(&control, &Spec::aggregate("diff")).unwrap();
    let typed: relca_core::diff::DiffReport = serde_json::from_value(control.clone())
        .expect("genuine prelower null selection is not a forged known owner");
    assert_eq!(serde_json::to_value(typed).unwrap(), control);
}

#[test]
fn m2_null_actor_table_rejects_known_empty_and_invented_actor_versions() {
    let control = prelower_control();
    let mut escaped = Vec::new();
    for (label, entries) in [
        ("empty", json!([])),
        (
            "invented",
            Spec::finite("P", &["Invented"]).actors["entries"].clone(),
        ),
    ] {
        let mut bad = control.clone();
        bad["ledger"]["actor_versions"] = json!({"status":"known","entries":entries,"reason":null});
        if serde_json::from_value::<report::Report>(bad).is_ok() {
            escaped.push(label);
        }
    }
    assert!(
        escaped.is_empty(),
        "M2_KNOWN_ACTORS_WITH_NULL_HOST_ACCEPTED: {escaped:?}"
    );
}

#[test]
fn minor1_preexploration_no_properties_does_not_claim_a_supplied_budget() {
    let rows = refusal_probes()["preexploration"].as_array().unwrap();
    let row = rows.iter().find(|r| r["refuse"] == true).unwrap();
    assert_eq!(row["explore_calls"], 0, "refusal must precede exploration");
    let v = &row["report"];
    assert_eq!(v["verdict"], "unsupported");
    assert_eq!(v["properties"], json!([]));
    assert_eq!(v["terminals"], json!([]));
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SEM-0" && d["severity"] == "error"),
        "pre-exploration diagnostics: {}",
        v["diagnostics"]
    );
    assert_eq!(v["ledger"]["execution"]["status"], "not_run");
    assert_eq!(v["ledger"]["backends"]["builtin"]["status"], "not_run");
    assert_eq!(v["ledger"]["actor_versions"]["status"], "known");
    assert_eq!(
        v["ledger"]["execution"]["selection"]["effective"]["process"],
        known("Check")
    );
    let mut spec = h::serial_spec();
    spec.status = "not_run";
    spec.limits["state_budget"] = oracle::unavailable("execution_settings_unavailable");
    h::ledger(v, &spec);
}

#[test]
fn green_review_injected_lowerer_control_really_explores_once() {
    let rows = refusal_probes()["preexploration"].as_array().unwrap();
    let row = rows.iter().find(|r| r["refuse"] == false).unwrap();
    assert_eq!(row["explore_calls"], 1);
    assert_eq!(row["report"]["verdict"], "satisfied");
    h::ledger(&row["report"], &h::serial_spec());
    h::leaf_numbers(&row["report"], 8, 8);
}

#[test]
fn green_review_running_control_keeps_budget_and_hand_eight_minutes() {
    let v = h::invoke("check", h::SERIAL, &[]);
    assert_eq!(v.exit, 0);
    h::ledger(&v.value, &h::serial_spec());
    h::leaf_numbers(&v.value, 8, 8);
    assert_eq!(
        v.value["ledger"]["execution"]["limits"]["state_budget"],
        known(1_000_000)
    );
}

fn ledger_diagnostics(v: &Value) -> Vec<&Value> {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "U-LEDGER-0")
        .collect()
}

fn one_ledger_error(v: &Value) {
    let ds = ledger_diagnostics(v);
    assert_eq!(ds.len(), 1, "MINOR3_DUPLICATE_LEDGER_DIAGNOSTIC");
    assert_eq!(ds[0]["severity"], "error");
    assert_eq!(ds[0]["span"], Value::Null);
    assert!(
        ds[0]["message"]
            .as_str()
            .unwrap()
            .starts_with("version_unavailable")
    );
}

#[test]
fn minor3_resealing_invalid_report_ledger_emits_one_diagnostic() {
    let mut r = report::check(h::SERIAL);
    r.ledger.producer["version"] = json!("");
    let r = report::seal(r);
    assert_eq!(r.verdict, Verdict::Unsupported);
    one_ledger_error(&serde_json::to_value(&r).unwrap());
    let r = report::seal(r);
    assert_eq!(r.verdict, Verdict::Unsupported);
    one_ledger_error(&serde_json::to_value(r).unwrap());
}

#[test]
fn minor3_resealing_invalid_simulation_ledger_emits_one_diagnostic() {
    let mut r = report::simulate(h::KEYS);
    r.ledger.producer["version"] = json!("");
    let r = report::seal_simulation(r);
    assert_eq!(r.verdict, Verdict::Unsupported);
    one_ledger_error(&serde_json::to_value(&r).unwrap());
    let r = report::seal_simulation(r);
    one_ledger_error(&serde_json::to_value(r).unwrap());
}

#[test]
fn minor3_resealing_invalid_conformance_ledger_emits_one_diagnostic() {
    let model = include_str!("snapshots/inputs/q3-conform-model.relca");
    let log = include_bytes!("snapshots/inputs/q3-conform-ok.jsonl");
    let mut r = relca_core::conformance::conform(model, log.as_slice(), None, None);
    assert_eq!(r.verdict, Verdict::Satisfied);
    r.ledger.producer["version"] = json!("");
    let r = r.seal();
    assert_eq!(r.verdict, Verdict::Unsupported);
    one_ledger_error(&serde_json::to_value(&r).unwrap());
    let r = r.seal();
    one_ledger_error(&serde_json::to_value(r).unwrap());
}

#[test]
fn green_review_healthy_reseal_and_independent_failures_remain_closed() {
    let r = report::check(h::SERIAL);
    let original = serde_json::to_value(&r).unwrap();
    assert_eq!(serde_json::to_value(report::seal(r)).unwrap(), original);
    for verdict in [Verdict::Invalid, Verdict::Violated] {
        let mut r = report::check(h::SERIAL);
        r.verdict = verdict;
        r.ledger.producer["version"] = json!("");
        let r = report::seal(r);
        assert_eq!(r.verdict, verdict);
        one_ledger_error(&serde_json::to_value(r).unwrap());
    }
}

fn refusal_probes() -> &'static Value {
    static PROBES: OnceLock<Value> = OnceLock::new();
    PROBES.get_or_init(|| {
        let output = Command::new("python3")
            .args([
                "-B",
                "crates/relca-cli/tests/support/ledger_refusal_probe.py",
            ])
            .current_dir(h::root())
            .output()
            .expect("run offline refusal probe");
        assert!(
            output.status.success(),
            "PROBE_HARNESS_FAILURE: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let probes: Value = serde_json::from_slice(&output.stdout).expect("probe observations");
        assert_eq!(probes["rows"].as_array().unwrap().len(), 8);
        assert_eq!(probes["preexploration"].as_array().unwrap().len(), 2);
        eprintln!(
            "Q3_REVIEW_PROBE_OBSERVATIONS: {}/observations.json",
            probes["sandbox"].as_str().unwrap()
        );
        probes
    })
}

#[test]
fn green_review_refusal_probe_controls_match_unmodified_cli_bytes() {
    for row in refusal_probes()["rows"].as_array().unwrap() {
        if row["corrupt"] != false || row["predecessor"] != false {
            continue;
        }
        let args: &[&str] = if row["mode"] == "verify" {
            &["verify"]
        } else {
            &[]
        };
        let control = h::cli(args);
        assert_eq!(row["stdout"].as_str().unwrap().as_bytes(), control.stdout);
        assert_eq!(row["stderr"], control.stderr);
        assert_eq!(row["exit"], control.exit);
    }
}

#[test]
fn minor2_verify_and_common_refusal_keep_injected_ledger_error_in_output() {
    let mut failures = Vec::new();
    for row in refusal_probes()["rows"].as_array().unwrap() {
        if row["corrupt"] != true || row["predecessor"] != false {
            continue;
        }
        assert_eq!(
            row["value"]["ledger"]["producer"]["version"], "",
            "fault reached seal"
        );
        let want_exit = if row["mode"] == "verify" { 4 } else { 3 };
        assert_eq!(row["exit"], want_exit, "independent refusal preserved");
        let ds: Vec<_> = row["value"]["diagnostics"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|d| d["code"] == "U-LEDGER-0")
            .collect();
        if ds.len() != 1
            || ds[0]["severity"] != "error"
            || !ds[0]["span"].is_null()
            || !ds[0]["message"]
                .as_str()
                .unwrap_or("")
                .starts_with("version_unavailable")
        {
            failures.push(format!("{}: U-LEDGER-0 missing or malformed", row["mode"]));
        }
    }
    assert!(
        failures.is_empty(),
        "MINOR2_REFUSAL_DIAGNOSTIC_LOST: {failures:?}"
    );
}

#[test]
fn minor2_refusal_exit_uses_sealed_not_predecessor_verdict() {
    let mut failures = Vec::new();
    for row in refusal_probes()["rows"].as_array().unwrap() {
        if row["predecessor"] != true {
            continue;
        }
        let bad = row["corrupt"] == true;
        let (want_verdict, want_exit) = if bad {
            ("unsupported", 4)
        } else {
            ("satisfied", 0)
        };
        assert_eq!(
            row["value"]["verdict"], want_verdict,
            "factory seals artificial predecessor"
        );
        if row["exit"] != want_exit {
            failures.push(format!(
                "{}: verdict={want_verdict}, exit={}",
                row["mode"], row["exit"]
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "MINOR2_PRESEAL_EXIT_USED: {failures:?}"
    );
}
