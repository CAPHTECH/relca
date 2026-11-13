//! ADR 0066, stage 1: no ignored/xfail tests. Each interpreter faces the hand oracle.
//! BOUNDED_EXPANDED points at independently written OLD DSL witnesses for calibration.
use relca_core::{engine, ir, lower, outcome, reference, report, syntax};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/bounded_repetition")
}

fn statements(block: &ir::Block) -> usize {
    block
        .iter()
        .map(|s| {
            1 + match &s.kind {
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => statements(then_body) + statements(else_body),
                _ => 0,
            }
        })
        .sum()
}

fn terminals(m: &ir::Model, o: &outcome::Outcome) -> Value {
    json!(
        o.terminals
            .iter()
            .map(|t| json!({
                "end": m.ends[t.end.0], "elapsed": [t.elapsed.min,t.elapsed.max],
                "work": [t.work.min,t.work.max]
            }))
            .collect::<Vec<_>>()
    )
}

fn probabilities(o: &outcome::Outcome) -> Value {
    json!(
        o.properties
            .iter()
            .filter_map(|p| p.probability.as_ref())
            .collect::<Vec<_>>()
    )
}

// The public API exposes budget enforcement, not the private visited map.
// Find a certified high-water capacity (visited + frontier), an upper bound on
// visited. Preserve the carried assertion 0 < visited < 1_000_000, without
// inventing a private-state instrumentation hook or a fixed measured count.
fn state_capacity(m: &ir::Model, full: &outcome::Outcome) -> usize {
    if full.truncated {
        return 1;
    } // clock witness has no state-count assertion
    let (mut lo, mut hi) = (0, 999_999);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        let o = engine::explore_with_budget(m, mid);
        if o.truncated || o.diagnostics.iter().any(|d| d.code.starts_with("U-")) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let bounded = engine::explore_with_budget(m, hi);
    assert!(!bounded.truncated);
    assert_eq!(
        bounded, *full,
        "capacity certificate must preserve the whole outcome"
    );
    hi
}

fn backend(name: &str) {
    let models = std::env::var_os("BOUNDED_EXPANDED")
        .map(PathBuf::from)
        .unwrap_or_else(|| data().join("models"));
    let src = fs::read_to_string(models.join(format!("{name}.relca"))).unwrap();
    backend_source(name, &src);
}

fn resolve(name: &str, src: &str) -> ir::Model {
    let ast = syntax::parse(src).unwrap_or_else(|d| panic!("{name}: repeat parser RED: {d:?}"));
    lower::lower(&ast).unwrap_or_else(|d| panic!("{name}: lower RED: {d:?}"))
}

fn observe(name: &str, m: &ir::Model, rm: &ir::Model) -> Value {
    let e = engine::explore(m);
    // Explicit limits: repeat must work with the old default bounds, not a
    // silently enlarged response/retry branch-depth exception.
    let r = reference::explore_with_limits(rm, reference::Limits::default());
    assert_eq!(e.truncated, name == "L2-clock", "engine cut: {name}");
    assert_eq!(
        r.outcome.truncated,
        name == "L2-clock",
        "reference cut: {name}"
    );
    assert_eq!(r.stop, reference::StopReason::Complete);
    for o in [&e, &r.outcome] {
        assert!(
            !o.diagnostics
                .iter()
                .any(|d| d.code.starts_with("U-") || d.code.starts_with("E-")),
            "{name}: {:?}",
            o.diagnostics
        );
    }
    json!({
        "reference_stop": format!("{:?}",r.stop),
        "reference_verdict":r.verdict().as_str(),
        "reference_diagnostics":r.outcome.diagnostics,
        "visited":state_capacity(m,&e),
        "ir_statements": statements(&m.process.body),
        "engine_probabilities":probabilities(&e),
        "reference_probabilities":probabilities(&r.outcome),
        "engine_terminals":terminals(m,&e),
        "reference_terminals":terminals(rm,&r.outcome),
        "engine_metrics":e.metrics,
        "reference_metrics":r.outcome.metrics,
    })
}

fn hand_oracle(name: &str, value: &Value) -> Output {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.tmp")
        .join(format!(
            "bounded-core-{}-{}-{name}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(format!("{name}.json")),
        serde_json::to_vec_pretty(value).unwrap(),
    )
    .unwrap();
    Command::new("python3")
        .arg("-B")
        .arg(data().join("backends.py"))
        .arg(&root)
        .arg(name)
        .output()
        .unwrap()
}

fn backend_source(name: &str, src: &str) {
    let m = resolve(name, src);
    let out = hand_oracle(name, &observe(name, &m, &m));
    assert!(
        out.status.success(),
        "{name}: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // Typed JSON consumers must not lose repetition observations on roundtrip.
    let report = report::check(src);
    let wire = serde_json::to_value(&report).unwrap();
    let decoded: report::Report = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), wire);
}

fn witness(name: &str) -> String {
    fs::read_to_string(data().join("witnesses").join(format!("{name}.relca"))).unwrap()
}

#[test]
fn old_dsl_witnesses_reach_both_interpreters_and_hand_values() {
    for name in [
        "L1", "L2", "L3", "L4", "P1", "L1-FFF", "P1-r1", "P1-r2", "P1-r3", "P1-never", "L1-32",
        "L2-32", "L3-32", "L4-32", "P1-32", "L2-clock",
    ] {
        backend_source(name, &witness(name));
    }
}

#[test]
fn calibration_reference_only_one_fewer_iteration_is_rejected() {
    let src = witness("L2");
    let last =
        "r3 = Rev.review(); if r3 == pass { end done } else { f3 = Clerk.fix(); end escalated }";
    assert_eq!(src.matches(last).count(), 1);
    let faulty = src.replace(last, "end escalated");
    let m = resolve("engine-three", &src);
    let rm = resolve("reference-two", &faulty);
    let value = observe("L2", &m, &rm);
    let end = |key: &str| {
        value[key]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["end"] == "escalated")
            .unwrap()["elapsed"]
            .clone()
    };
    assert_eq!(end("engine_terminals"), json!([9, 9]));
    assert_eq!(end("reference_terminals"), json!([6, 6]));
    let out = hand_oracle("L2", &value);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success() && stderr.contains("AssertionError") && stderr.contains("reference"),
        "{stderr}"
    );
}

#[test]
fn calibration_clock_must_not_be_reported_as_business_limit() {
    let m = resolve("clock", &witness("L2-clock"));
    let mut value = observe("L2-clock", &m, &m);
    assert!(hand_oracle("L2-clock", &value).status.success());
    value["reference_verdict"] = json!("satisfied");
    let out = hand_oracle("L2-clock", &value);
    assert!(
        !out.status.success() && String::from_utf8_lossy(&out.stderr).contains("AssertionError")
    );
}

fn arrivals_two_cases(src: &str) {
    assert_eq!(src.matches("cases 1; live_tasks 1").count(), 1);
    let src = src.replace("cases 1; live_tasks 1", "cases 2; live_tasks 2");
    let i = src.rfind('}').unwrap();
    let src = format!("{} arrivals fixed [0min,0min]\n{}", &src[..i], &src[i..]);
    let m = resolve("two simultaneous L1 cases", &src);
    let e = engine::explore(&m);
    let r = reference::explore_with_limits(&m, reference::Limits::default());
    assert_eq!(r.stop, reference::StopReason::Complete);
    for (label, o) in [("engine", &e), ("reference", &r.outcome)] {
        assert!(!o.truncated, "{label}");
        assert_eq!(
            o.properties.len(),
            3,
            "safety and both reachability questions: {label}"
        );
        assert!(
            o.diagnostics
                .iter()
                .all(|d| d.severity != relca_core::diag::Severity::Error),
            "{label}: {:?}",
            o.diagnostics
        );
        assert!(
            o.properties
                .iter()
                .all(|p| p.verdict == relca_core::Verdict::Satisfied && p.probability.is_none()),
            "{label}: {:?}",
            o.properties
        );
        let cases = o
            .case_outcomes
            .as_ref()
            .expect("arrivals preserve case identity");
        assert_eq!(cases.len(), 2);
        let mut ids: Vec<_> = cases.iter().map(|c| c.case_id.0).collect();
        ids.sort_unstable();
        assert_eq!(ids, [0, 1], "neither case may disappear or be duplicated");
        for case in cases {
            assert_eq!(case.arrival, 0);
            assert_eq!(case.terminals.len(), 2);
            let mut names = Vec::new();
            for terminal in &case.terminals {
                let outcome::CaseTerminalKind::End(end) = terminal.kind else {
                    panic!("normal end only: {label}")
                };
                let (lo, hi, work_lo, work_hi) = match m.ends[end.0].as_str() {
                    "done" => (1, 10, 1, 7),
                    "escalated" => (9, 13, 9, 9),
                    name => panic!("unexpected end {name}: {label}"),
                };
                names.push(m.ends[end.0].as_str());
                let time = terminal.elapsed.unwrap();
                let work = terminal.work.unwrap();
                assert_eq!(
                    (time.min, time.max, work.min, work.max),
                    (lo, hi, work_lo, work_hi),
                    "{label}, case {:?}",
                    case.case_id
                );
            }
            names.sort_unstable();
            assert_eq!(names, ["done", "escalated"]);
        }
        let metrics = serde_json::to_value(o.metrics.as_ref().unwrap()).unwrap();
        for (cap, lo, hi) in [("Rev.review", "2", "6"), ("Clerk.fix", "0", "6")] {
            let row = metrics["capabilities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["capability"] == cap)
                .unwrap();
            assert_eq!(row["starts"]["min"], lo, "{label}: {cap}");
            assert_eq!(row["starts"]["max"], hi, "{label}: {cap}");
        }
    }
}

#[test]
fn arrivals_l1_two_cases_preserve_all_resource_orders() {
    let source = fs::read_to_string(data().join("models/L1.relca")).unwrap();
    arrivals_two_cases(&source);
}

#[test]
fn old_dsl_two_case_witness_reaches_arrivals_and_hand_envelopes() {
    arrivals_two_cases(&witness("L1"));
}

macro_rules! cases {
    ($($test:ident => $name:literal),+ $(,)?) => {$ (
        #[test] fn $test() { backend($name); }
    )+};
}
cases! {
    l1 => "L1", l2 => "L2", l3 => "L3", l4 => "L4", p1 => "P1",
    l1_fff => "L1-FFF", p1_r1 => "P1-r1", p1_r2 => "P1-r2",
    p1_r3 => "P1-r3", p1_never => "P1-never",
    l1_32 => "L1-32", l2_32 => "L2-32", l3_32 => "L3-32",
    l4_32 => "L4-32", p1_32 => "P1-32", l2_clock => "L2-clock",
}
