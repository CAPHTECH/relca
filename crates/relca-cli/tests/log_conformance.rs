//! ADR 0057 acceptance. Live tests intentionally RED until conform is implemented.
//! Calculations: log_conformance_calculations.md. No output snapshots/oracles here.
#[path = "support/log_conformance_inputs.rs"]
mod input;
#[path = "support/test_tmp.rs"]
mod test_tmp;
#[path = "support/log_conformance_wire.rs"]
mod wire;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn write_input(case: &input::Case) -> (PathBuf, PathBuf) {
    let tmp = test_tmp::root();
    let dir = tmp.join(format!(
        "q2-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let model = dir.join("model.relca");
    let log = dir.join("events.jsonl");
    std::fs::write(&model, &case.model).unwrap();
    std::fs::write(&log, case.log()).unwrap();
    (model, log)
}
fn digest(path: &std::path::Path) -> String {
    let out = Command::new("sha256sum").arg(path).output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}
fn invoke(case: &input::Case, reverse: bool) -> (Value, String, i32) {
    let mut c = case.clone();
    if reverse {
        c.records.as_array_mut().unwrap()[1..].reverse();
    }
    let (model, log) = write_input(&c);
    let before_model = digest(&model);
    let before_log = digest(&log);
    let mut command = Command::new(env!("CARGO_BIN_EXE_relca"));
    command
        .current_dir(root())
        .arg("conform")
        .arg(&model)
        .arg("--log")
        .arg(&log);
    if let Some(scope) = c.scope {
        command.args(["--scope", scope]);
    }
    let Output {
        stdout,
        stderr,
        status,
    } = command.args(&c.extra_args).output().unwrap();
    assert_eq!(digest(&model), before_model, "{} mutated model", c.name);
    assert_eq!(digest(&log), before_log, "{} mutated log", c.name);
    let err = String::from_utf8(stderr).unwrap();
    assert!(
        !err.starts_with("usage: relca <"),
        "ENTRYPOINT_MISSING {}: conform is an unknown subcommand, stdout={}, stderr={err}",
        c.name,
        String::from_utf8_lossy(&stdout)
    );
    assert!(stdout.ends_with(b"\n"), "{} stdout lacks LF", c.name);
    assert_eq!(
        stdout.iter().filter(|b| **b == b'\n').count(),
        1,
        "one JSON object and LF"
    );
    let v: Value = serde_json::from_slice(&stdout).expect("one raw JSON result");
    let exit = status.code().expect("normal process exit, not signal");
    wire::validate(&v, exit).unwrap_or_else(|e| panic!("{} WIRE: {e}", c.name));
    if v["model"]["sha256"].is_string() {
        assert_eq!(v["model"]["sha256"], before_model);
    }
    if v["log"]["sha256"].is_string() {
        assert_eq!(v["log"]["sha256"], before_log);
    }
    for d in v["diagnostics"].as_array().unwrap() {
        let prefix = format!("{}:", d["code"].as_str().unwrap());
        assert!(
            err.lines().any(|s| s.starts_with(&prefix)),
            "diagnostic missing on stderr: {prefix}"
        );
    }
    for row in v["checks"].as_array().unwrap() {
        for evidence in row["evidence"].as_array().unwrap() {
            let (index, event) = c
                .records
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .skip(1)
                .find(|(_, r)| r["event_id"] == evidence["event_id"])
                .expect("evidence must refer to a real event, not an invented missing phase");
            assert_eq!(
                evidence["line"],
                index + 1,
                "evidence must use actual JSONL line"
            );
            assert_eq!(evidence["case_id"], event["case_id"]);
            assert_eq!(evidence["task_id"], event["task_id"]);
        }
    }
    assert!(
        !err.contains("do-not-echo-private-sentinel"),
        "private input echoed"
    );
    (v, err, exit)
}
fn acceptance(name: &'static str) {
    let c = input::case(name);
    let (v, _, exit) = invoke(&c, false);
    let e = &c.expected;
    if matches!(e.exit, 0..=2) && e.limit.is_none() {
        assert!(
            v["model"]["sha256"].is_string(),
            "known model hash cannot be null"
        );
        assert!(
            v["log"]["sha256"].is_string(),
            "fully read log hash cannot be null"
        );
    }
    assert_eq!(
        (v["conformance"].as_str(), v["verdict"].as_str(), exit),
        (Some(e.conformance), Some(e.verdict), e.exit),
        "{name}: {v}"
    );
    if let Some(code) = e.code {
        assert!(wire::has_code(&v, code), "{name}: missing {code}");
    }
    if let Some(reason) = e.reason {
        assert!(
            wire::has_reason(&v, reason),
            "{name}: missing reason {reason}"
        );
    }
    if let Some(reason) = e.unmapped {
        let rows = v["unmapped_actor"].as_array().unwrap();
        assert!(
            rows.iter().any(|r| r["reason"] == reason),
            "unmapped_actor lost {reason}"
        );
        let ids: BTreeSet<_> = rows
            .iter()
            .flat_map(|r| r["event_ids"].as_array().unwrap())
            .map(|id| id.as_str().unwrap())
            .collect();
        for id in e.evidence {
            assert!(ids.contains(id), "unmapped evidence lost {id}");
        }
    } else {
        let ids = if e.exit == 1 {
            v["checks"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| {
                    row["conformance"] == "violating" && row["reason"].as_str() == e.reason
                })
                .flat_map(|row| row["evidence"].as_array().unwrap())
                .map(|ev| ev["event_id"].as_str().unwrap().to_owned())
                .collect()
        } else {
            wire::evidence_ids(&v)
        };
        for id in e.evidence {
            assert!(ids.contains(*id), "{name}: missing positive witness {id}");
        }
    }
    if let Some(limit) = e.limit {
        assert_eq!(v["limits"][limit], true);
        assert_eq!(v["limits"]["truncated"], true);
    } else {
        assert_eq!(
            v["limits"]["truncated"], false,
            "unexpected truncation: {name}"
        );
    }
    if e.exit == 0 {
        assert!(!v["checks"].as_array().unwrap().is_empty(), "empty success");
        assert!(!wire::has_code(&v, "E-LOG-2"));
    }
    if e.exit == 1 {
        assert!(wire::has_code(&v, "E-LOG-2"));
    }
    if e.conformance == "unknown" && e.exit == 2 {
        assert!(!wire::has_code(&v, "E-LOG-2"), "absence became a violation");
    }
    if name == "wire_cut_undeclared" {
        assert_eq!(v["cut"], "undeclared");
    }
    if name == "wire_unknown_operation" {
        assert!(!v["unmapped_operation"].as_array().unwrap().is_empty());
    }
    if name == "l21_independent_violation" {
        assert!(
            v["checks"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["conformance"] == "unknown")
        );
        assert!(v["not_checked"].to_string().contains("response"));
        fn count(v: &Value, label: &str) -> u64 {
            match v {
                Value::Object(o) => o
                    .iter()
                    .map(|(k, v)| {
                        if k == label {
                            v.as_u64().unwrap_or(0)
                        } else {
                            count(v, label)
                        }
                    })
                    .sum(),
                Value::Array(a) => a.iter().map(|v| count(v, label)).sum(),
                _ => 0,
            }
        }
        assert!(
            count(&v["summary"], "unknown") > 0,
            "unknown counts hidden behind the violation"
        );
        assert!(
            count(&v["summary"], "violating") > 0,
            "violation counts absent"
        );
    }
    if name == "l10_distinct_cases" || name == "l10_reused_task_id" {
        let cases = v["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 2);
        assert_eq!(
            cases
                .iter()
                .map(|c| c["case_id"].as_str().unwrap())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["c", "d"])
        );
        assert!(cases.iter().all(|c| c["conformance"] == "conformant"));
    }
}

macro_rules! live_tests {
    ($($name:ident),+ $(,)?) => {
        pub const NAMES: &[&str] = &[$(stringify!($name)),+];
        $(#[test] fn $name() {acceptance(stringify!($name));})+
    }
}
live_tests!(
    l01_serial_ok,
    l02_missing_start,
    l03_missing_finish,
    l04_time_precision,
    l05_partial_start,
    l05_unspecified_start,
    l06_partial_end,
    l07_missing_approval,
    l07_failed_approval,
    l08_separation_ok,
    l08_same_person,
    l08_distinct_person_control,
    l08_same_person_witness,
    l09_once_ok,
    l09_duplicate_commit,
    l10_distinct_cases,
    l10_reused_task_id,
    l11_duplicate_finish,
    l12_excludes_violation,
    l13_ambiguous,
    l14_forward,
    l14_reverse,
    l15_unmapped_actor,
    l16_unmapped_person,
    l17_cancel_ok,
    l17_request_only,
    l17_missing_request,
    l17_cancel_without_explicit_edge,
    l17_finish_approval_control,
    l17_ack_not_approval,
    l18_forbidden_ack,
    l19_bad_mapping,
    l20_future_schema,
    l20_unknown_flag,
    l21_response_unsupported,
    l21_independent_violation,
    l22_empty,
    l23_duration_violation,
    l23_event_boundary,
    l23_event_limit,
    l23_line_boundary,
    l23_line_limit,
    wire_missing_field,
    wire_unknown_field,
    wire_duplicate_key,
    wire_duplicate_event_id,
    wire_after_unknown,
    wire_after_cycle,
    wire_after_time_contradiction,
    wire_reversed_lifecycle,
    wire_outcome_domain,
    wire_result_type,
    wire_window,
    wire_unknown_operation,
    wire_cut_undeclared,
    wire_null_version,
    wire_equal_time_requires,
    wire_future_scope,
    wire_missing_scope,
    wire_future_unit,
    wire_no_requests,
    wire_explicit_task_edge,
    wire_source_cycle,
    wire_no_implicit_actor_match,
    wire_no_implicit_person_match,
    wire_operation_actor_mismatch,
);

#[test]
fn green_models_are_statically_valid_without_running_the_engine() {
    for name in NAMES {
        let c = input::case(name);
        let ast = relca_core::syntax::parse(&c.model).unwrap_or_else(|d| panic!("{name}: {d:?}"));
        let lowered = relca_core::lower::lower_with_selection(&ast, None, None);
        if *name == "wire_no_requests" {
            let ds =
                lowered.expect_err("the existing static gate must reject a model without requests");
            assert!(ds.iter().any(|d| d.code == "U-SEM-0"
                && d.message == "at least one verification request is required"));
            assert!(
                ds.iter()
                    .filter(|d| d.severity == relca_core::diag::Severity::Error)
                    .all(|d| d.code == "U-SEM-0")
            );
        } else {
            lowered.unwrap_or_else(|d| panic!("{name}: {d:?}"));
        }
    }
}
#[test]
fn green_hand_inputs_have_one_declared_edit_and_no_captured_expectations() {
    let mut names = BTreeSet::new();
    let mut pairs = 0;
    for name in NAMES {
        assert!(names.insert(name));
        let c = input::case(name);
        assert_eq!(c.name, *name);
        assert!(matches!(c.expected.exit, 0..=4));
        if let Some(parent) = c.parent {
            let mut p = input::case(parent);
            let before = (p.model.clone(), p.log(), p.extra_args.clone(), p.scope);
            p.apply(c.edit.as_ref().expect("one explicit edit per broken pair"));
            assert_ne!(
                before,
                (p.model.clone(), p.log(), p.extra_args.clone(), p.scope),
                "no-op {name}"
            );
            assert_eq!(
                (&p.model, p.log(), &p.extra_args, p.scope),
                (&c.model, c.log(), &c.extra_args, c.scope),
                "more than declared edit: {name}"
            );
            pairs += 1;
        }
    }
    assert!(pairs >= 40, "a separate broken pair for each property");
}
#[test]
fn green_full_wire_control_and_single_fault_rejections() {
    let valid = wire::control();
    wire::validate(&valid, 0).unwrap();
    for key in valid.as_object().unwrap().keys() {
        let mut bad = valid.clone();
        bad.as_object_mut().unwrap().remove(key);
        assert!(wire::validate(&bad, 0).is_err(), "missing {key} accepted");
    }
    for (path, wrong) in [
        ("/conformance", json!("satisfied")),
        ("/limits/truncated", json!(true)),
        ("/checks/0/evidence/0/line", json!(0)),
        ("/checks/0/model_span", json!({"start":3,"end":1})),
        ("/diagnostics/0/severity", json!("error")),
        ("/checks/0/conformance", json!("maybe")),
    ] {
        let mut bad = valid.clone();
        *bad.pointer_mut(path).unwrap() = wrong;
        assert!(
            wire::validate(&bad, 0).is_err(),
            "single wire fault escaped: {path}"
        );
    }
    assert!(
        wire::validate(&json!({"verdict":"satisfied"}), 0).is_err(),
        "partial expected JSON accepted"
    );
    assert!(
        wire::validate(&valid, 2).is_err(),
        "exit disagrees with wire"
    );
}
#[test]
fn green_unknown_command_still_fails_closed() {
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("not-a-relca-command")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap(),
        wire::ledger_shape::extend_value(
            json!({"verdict":"invalid"}),
            &wire::ledger_shape::Spec::minimal("cli_refusal")
        )
    );
}
#[test]
fn green_small_order_and_interval_hand_oracles_are_independent() {
    fn orders(prefix: Vec<usize>, rest: Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if rest.is_empty() {
            out.push(prefix);
            return;
        }
        for i in 0..rest.len() {
            let mut p = prefix.clone();
            p.push(rest[i]);
            let mut r = rest.clone();
            r.remove(i);
            orders(p, r, out);
        }
    }
    // Nodes: e.start=0,e.finish=1,a.start=1,a.finish=2. No event parser used.
    let times = [0, 1, 1, 2];
    let mut permutations = vec![];
    orders(vec![], vec![0, 1, 2, 3], &mut permutations);
    assert_eq!(permutations.len(), 24);
    let valid: Vec<_> = permutations
        .iter()
        .filter(|p| p.windows(2).all(|ab| times[ab[0]] <= times[ab[1]]))
        .filter(|p| {
            p.iter().position(|i| *i == 0) < p.iter().position(|i| *i == 1)
                && p.iter().position(|i| *i == 2) < p.iter().position(|i| *i == 3)
        })
        .collect();
    assert_eq!(valid.len(), 2);
    let violations = valid
        .iter()
        .filter(|p| p.iter().position(|i| *i == 1) < p.iter().position(|i| *i == 2))
        .count();
    assert_eq!(
        violations, 1,
        "L13 has both a violating and a conforming extension"
    );
    let forward: Vec<_> = valid
        .iter()
        .filter(|p| p.iter().position(|i| *i == 1) < p.iter().position(|i| *i == 2))
        .collect();
    let reverse: Vec<_> = valid
        .iter()
        .filter(|p| p.iter().position(|i| *i == 2) < p.iter().position(|i| *i == 1))
        .collect();
    assert_eq!(
        (forward.len(), reverse.len()),
        (1, 1),
        "L14 after picks exactly one"
    );
    assert_eq!(
        (0..=1).map(|start| 1 - start).collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 1])
    );
}
#[test]
fn live_partial_order_is_independent_of_jsonl_line_order() {
    for name in ["l13_ambiguous", "l14_forward", "l14_reverse"] {
        let c = input::case(name);
        let (a, _, ea) = invoke(&c, false);
        let (b, _, eb) = invoke(&c, true);
        assert_eq!(ea, eb);
        assert_eq!(a["conformance"], b["conformance"]);
        assert_eq!(wire::evidence_ids(&a), wire::evidence_ids(&b));
        assert_ne!(a["log"]["sha256"], b["log"]["sha256"]);
    }
}
