//! ADR 0045 boundary-review regressions (M1, m1). A simulate refusal reached
//! after the quality declarations were lowered carries the aborted quality
//! wire, and its assumptions are exactly those of the same model with the
//! quality lines removed, plus the main path's four quality rows as one block
//! at the end (no probability.scheduler on these paths). Expectations are
//! written by hand; the quality-free pair is the byte oracle for everything
//! else. The inputs live here so the closed fixture population is unchanged.
use serde_json::{Value, json};
#[path = "support/ledger_shape.rs"]
mod ledger_shape;
use std::{fs, process::Command};

/// Copy of fixtures/qual-base/model.relca (no simulation clause).
const QUAL_BASE: &str = r#"model QualityBase version "0.2" {
  type V = enum { pass, fail }
  type Ack = enum { committed }
  observed case { registered: bool = false }
  latent case { correct: bool = false }
  kernel judge { * -> { 0.9: pass with correct = true; 0.02: pass with correct = false; 0.08: fail with correct = false } }
  actor Checker { capacity 2; capability check() -> V { duration fixed 3min; outcomes kernel judge; writes correct } }
  actor Ledger { capacity 2; capability post() -> Ack { commits; duration fixed 1min; result nondet { committed }; writes registered; finish { registered = true } } }
  process Flow { v = Checker.check(); if v == pass {} else { end returned }; a = Ledger.post(); end posted }
  cut C { include [Checker, Ledger] }
  arrivals fixed [0min, 10min]
  bounds { cases 2; live_tasks 2; clock 30min }
  verify reachable end.posted
  property case.Wrong = reached(registered == true && correct == false)
  property case.Registered = reached(registered == true)
  metric cohort.WrongPerArrival = expected(count(case.Wrong)) / arrivals
  metric cohort.WrongPerRegistered = expected(count(case.Wrong)) / expected(count(case.Registered))
}
"#;

/// fixtures/auth-revocation/model.relca plus one property and one metric: the
/// E-SIM-0 path that publishes the lowered rows for a revocation record.
const REVOCATION: &str = r#"model Authority version "0.2" {
  type T = enum { pass }
  people [r,p,q]
  actor A { executors [q]; capability run() -> T { duration fixed 2min; result nondet { pass } } }
  permit q to A.run
  authority G { basis "record only"; grants q to A.run; issued_by r; expires 3min; revocable_by p }
  process Flow { a = A.run(); end done }
  bounds { cases 1; live_tasks 2; clock 20min }
  verify safety no_resource_overcommit
  contract Controls { authority_basis A.run G }
  property case.Done = reached(end.done)
  metric cohort.DonePerArrival = expected(count(case.Done)) / arrivals
}
"#;

/// fixtures/resp-origin/model.relca with a simulation clause, a structural
/// completion question and quality: the sampled path's early return after
/// the main-path rows (control for m1).
const COMPLETION: &str = r#"model ResponsibilityOrigin version "0.2" {
  type Entry = enum { entry }
  type Verdict = enum { pass, fail }
  type Ack = enum { committed }
  people [a1]
  actor Invoice { source document }
  actor Reader { capacity 1; capability enter() -> Entry { reads [Invoice]; duration fixed 1min; result nondet { entry } } }
  actor Approver { executors [a1]; capability approve(e:Entry) -> Verdict { reads [Invoice]; duration fixed 1min; result nondet { pass,fail } } }
  actor Ledger { capacity 1; capability post(v:Verdict) -> Ack { commits; duration fixed 1min; result nondet { committed } } }
  responsible a1 for Approver.approve
  process Flow {
    entry = Reader.enter()
    approval = Approver.approve(entry)
    if approval == pass { } else { end returned }
    posted = Ledger.post(approval)
    end done
  }
  cut Scope { include [Invoice,Reader,Approver,Ledger] }
  bounds { cases 1; live_tasks 2; clock 20min }
  verify safety no_resource_overcommit
  verify recoverable end
  simulation replications 4 seed 0 confidence 0.95
  property case.Done = reached(end.done)
  metric cohort.DonePerArrival = expected(count(case.Done)) / arrivals
}
"#;

/// The quality lines replaced by spaces of the same length, so the model
/// span (and every other offset) is unchanged.
fn without_quality(src: &str, lines: usize) -> String {
    let mut blanked = 0;
    let out: Vec<String> = src
        .split('\n')
        .map(|line| {
            if line.starts_with("  property case.") || line.starts_with("  metric cohort.") {
                blanked += 1;
                " ".repeat(line.len())
            } else {
                line.to_owned()
            }
        })
        .collect();
    assert_eq!(blanked, lines, "quality lines in the input");
    out.join("\n")
}

fn run(name: &str, src: &str, command: &str) -> (Vec<u8>, Vec<u8>, i32) {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!(
            "p{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
        .join("quality-review-fixes");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.relca"));
    fs::write(&path, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(&path)
        .output()
        .unwrap();
    (out.stdout, out.stderr, out.status.code().unwrap())
}

/// The hand-written (field, value) of the four quality rows, in order.
const QUALITY_ROWS: [(&str, &str); 4] = [
    ("quality.definition", "model-declarations"),
    ("quality.denominator", "all_arrivals_or_declared_property"),
    ("quality.form", "ratio_of_expected_counts"),
    ("quality.estimation", "ratio_of_sums_per_replication"),
];

/// Removes the leading `"quality":{..},` member and every assumption object
/// whose field starts with `quality.` from the raw stdout, and swaps the
/// source hash, leaving bytes comparable with the quality-free run.
fn strip_quality(raw: &str, own_hash: &str, other_hash: &str) -> String {
    let mut s = raw.to_owned();
    if let Some(rest) = s.strip_prefix("{\"quality\":") {
        let end = rest.find(",\"actors\":").expect("member after quality");
        s = format!("{{{}", &rest[end + 1..]);
    }
    while let Some(start) = s.find("{\"field\":\"quality.") {
        let end = start + s[start..].find("\"}").unwrap() + 2;
        let (from, to) = if s[..start].ends_with(',') {
            (start - 1, end)
        } else if s[end..].starts_with(',') {
            (start, end + 1)
        } else {
            (start, end)
        };
        s.replace_range(from..to, "");
    }
    s.replace(own_hash, other_hash)
}

/// The quality block is the main path's rows: the same four objects, byte for
/// byte, as a simulate run of the same model that reaches sampling.
fn main_path_rows(name: &str, src: &str) -> Vec<Value> {
    let sampled = src.replacen(
        "  verify ",
        "  simulation replications 4 seed 0 confidence 0.95\n  verify ",
        1,
    );
    let (stdout, _, _) = run(&format!("{name}-main-path"), &sampled, "simulate");
    let v: Value = serde_json::from_slice(&stdout).unwrap();
    assert!(
        !v["simulation"]["replications"].is_null(),
        "{name}: main path reached sampling"
    );
    v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["field"].as_str().unwrap().starts_with("quality."))
        .cloned()
        .collect()
}

/// Shared pair check: exit/stderr equal, assumptions = quality-free rows
/// followed by the four quality rows, all other bytes equal.
fn assert_pair(name: &str, q: &str, nq: &str, exit: i32, main_rows: Option<&[Value]>) -> Value {
    let (q_out, q_err, q_exit) = run(&format!("{name}-quality"), q, "simulate");
    let (n_out, n_err, n_exit) = run(&format!("{name}-plain"), nq, "simulate");
    assert_eq!((q_exit, n_exit), (exit, exit), "{name}: exit");
    assert_eq!(q_err, n_err, "{name}: stderr");
    let qv: Value = serde_json::from_slice(&q_out).unwrap();
    let nv: Value = serde_json::from_slice(&n_out).unwrap();
    assert!(
        nv.get("quality").is_none(),
        "{name}: plain model has no quality"
    );
    let rows = qv["assumptions"].as_array().unwrap();
    let plain = nv["assumptions"].as_array().unwrap();
    assert_eq!(rows.len(), plain.len() + 4, "{name}: plain rows plus four");
    assert_eq!(
        &rows[..plain.len()],
        &plain[..],
        "{name}: plain rows first, unchanged"
    );
    let block = &rows[plain.len()..];
    let got: Vec<(&str, &str)> = block
        .iter()
        .map(|a| (a["field"].as_str().unwrap(), a["value"].as_str().unwrap()))
        .collect();
    assert_eq!(
        got, QUALITY_ROWS,
        "{name}: one block of four quality rows at the end"
    );
    if let Some(main) = main_rows {
        assert_eq!(block, main, "{name}: same rows as the sampled main path");
    }
    let (qh, nh) = (
        qv["model"]["sha256"].as_str().unwrap(),
        nv["model"]["sha256"].as_str().unwrap(),
    );
    let spec = if name == "completion" {
        ledger_shape::Spec::simulation(
            "Flow",
            &["Invoice", "Reader", "Approver", "Ledger"],
            0,
            4,
            "19/20",
        )
    } else {
        ledger_shape::Spec::prelower("simulation", false)
    };
    let q_raw = ledger_shape::remove_expected_prefix(&q_out, &spec);
    let n_raw = ledger_shape::remove_expected_prefix(&n_out, &spec);
    assert_eq!(
        strip_quality(std::str::from_utf8(&q_raw).unwrap(), qh, nh),
        std::str::from_utf8(&n_raw).unwrap(),
        "{name}: every byte except quality, the four rows and the source hash"
    );
    qv
}

fn aborted_sample() -> Value {
    json!({"mean":null,"min":null,"max":null,"samples":0,"reason":"aborted"})
}

/// Aborted shape for qual-base, written by hand.
fn qual_base_aborted() -> Value {
    let s = aborted_sample();
    let cases = json!([
        {"case_id":1,"arrival":0,"hits":0,"unknown":0,"trials":0},
        {"case_id":2,"arrival":10,"hits":0,"unknown":0,"trials":0}
    ]);
    let value = json!({"point":null,"numerator":s,"denominator":s,"samples":0,"reason":"aborted"});
    let rate = json!({"point":null,"samples":0,"reason":"aborted"});
    let outcome =
        |kind: &str, end: Value| json!({"kind":kind,"end":end,"expected_count":s,"rate":rate});
    json!({
        "rules":"quality-cohort-p1-1",
        "analysis":"simulation",
        "properties":[
            {"name":"Wrong","condition":"registered == true && correct == false","within":null,
             "cases":cases,"expected_count":s,"unknown_count":s},
            {"name":"Registered","condition":"registered == true","within":null,
             "cases":cases,"expected_count":s,"unknown_count":s}
        ],
        "cohort":[
            {"name":"WrongPerArrival","form":"ratio_of_expected_counts","numerator":"Wrong",
             "denominator":{"kind":"arrivals","count":2},"value":value,"bounds_with_undetermined":null},
            {"name":"WrongPerRegistered","form":"ratio_of_expected_counts","numerator":"Wrong",
             "denominator":{"kind":"property","property":"Registered","covers":null,"reason":"state_condition"},
             "value":value,"bounds_with_undetermined":null}
        ],
        "outcomes_per_arrival":[
            outcome("end", json!("returned")),
            outcome("end", json!("posted")),
            outcome("rejected", Value::Null),
            outcome("unfinished", Value::Null),
            outcome("deadlock", Value::Null)
        ],
        "checked":[],
        "unchecked":["expectation_of_ratio","cohort_any_probability","metric_intervals",
            "metric_constraints","numerator_subset_of_denominator","business_value","rework_loss",
            "deadline_attainment","case_reachability","expected_counts","ratio_of_expected_counts",
            "denominator_disclosure","outcomes_per_arrival"]
    })
}

#[test]
fn m1_e_sim_0_quality_assumptions_are_plain_rows_plus_four_quality_rows() {
    let main = main_path_rows("qual-base", QUAL_BASE);
    let v = assert_pair(
        "qual-base",
        QUAL_BASE,
        &without_quality(QUAL_BASE, 4),
        3,
        Some(&main),
    );
    assert_eq!(v["verdict"], "invalid");
    let codes: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, ["E-SIM-0"]);
    // Without a revocation record the E-SIM-0 path publishes no lowered rows.
    let rows: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["field"].as_str().unwrap())
        .collect();
    assert_eq!(rows, QUALITY_ROWS.map(|(f, _)| f));
    assert_eq!(v["quality"], qual_base_aborted());
}

#[test]
fn m1_e_sim_0_with_revocation_record_keeps_lowered_rows_then_quality_block() {
    let main = main_path_rows("revocation", REVOCATION);
    let v = assert_pair(
        "revocation",
        REVOCATION,
        &without_quality(REVOCATION, 2),
        3,
        Some(&main),
    );
    assert!(v["assumptions"].as_array().unwrap().len() > 4);
    assert_eq!(v["quality"]["analysis"], "simulation");
    assert_eq!(v["quality"]["cohort"][0]["value"]["reason"], "aborted");
    assert_eq!(v["quality"]["checked"], json!([]));
}

#[test]
fn m1_completion_refusal_after_sampling_keeps_the_same_block_control() {
    let v = assert_pair(
        "completion",
        COMPLETION,
        &without_quality(COMPLETION, 2),
        4,
        None,
    );
    assert_eq!(v["verdict"], "unsupported");
}
