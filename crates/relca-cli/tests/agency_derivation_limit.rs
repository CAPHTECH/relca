//! CLI acceptance with hand-derived results. Full-output preservation is kept
//! separately in agency_limit_preservation.json; it is not a semantic oracle.
#[path = "../../relca-core/tests/support/agency_limit_inputs.rs"]
mod input;
use relca_core::{ast, syntax};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(command: &str, source: &str, options: &[&str]) -> Output {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = root().join(".tmp/agency-limit-inputs");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!(
        "{}-{}.relca",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(path)
        .args(options)
        .current_dir(root())
        .output()
        .unwrap()
}

fn wire(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&o.stderr)))
}

fn call_span(source: &str, binding: &str) -> Value {
    let parsed = syntax::parse(source).unwrap();
    parsed
        .items
        .iter()
        .filter_map(|item| match item {
            ast::Item::Process(p) if p.name.value == "Flow" => Some(p),
            _ => None,
        })
        .flat_map(|p| &p.body)
        .find_map(|s| match &s.value {
            ast::Stmt::Call(c) if c.binding.value == binding => Some(json!(c.span)),
            _ => None,
        })
        .unwrap()
}

fn unavailable(v: &Value, counter: &str) {
    let a = &v["agency"];
    assert_eq!(a["status"], "not_derived", "{a}");
    assert_eq!(a["reason"], "derivation_limit", "{a}");
    for partial in ["origin", "lineage", "lineage_groups", "refusal"] {
        assert!(
            a[partial].is_null(),
            "must not publish partial {partial}: {a}"
        );
    }
    let l = &a["limit"];
    assert_eq!(l["counter"], counter, "{l}");
    let limit = if counter == "paths" { 4096 } else { 1_000_000 };
    assert_eq!(l["limit"], limit, "{l}");
    assert!(l["requested"].as_u64().unwrap() > limit, "{l}");
    assert_eq!(l["process"], "Flow", "{l}");
    assert!(!l["phase"].as_str().unwrap().is_empty(), "{l}");
    assert!(
        v["unchecked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s.as_str().is_some_and(|s| s.contains("agency"))),
        "Agency must remain unchecked: {}",
        v["unchecked"]
    );
    let ds = v["diagnostics"].as_array().unwrap();
    let warnings = ds
        .iter()
        .filter(|d| d["code"] == "W-AGY-4")
        .collect::<Vec<_>>();
    assert_eq!(warnings.len(), 1, "{ds:?}");
    let d = warnings[0];
    assert_eq!(d["severity"], "warning");
    let text = d["message"].as_str().unwrap();
    for token in [counter, "Flow", l["phase"].as_str().unwrap()] {
        assert!(text.contains(token), "missing {token}: {text}");
    }
    for n in [
        l["limit"].as_u64().unwrap(),
        l["requested"].as_u64().unwrap(),
    ] {
        assert!(text.contains(&n.to_string()), "missing {n}: {text}");
    }
    assert!(ds.iter().all(|d| d["severity"] != "error"), "{ds:?}");
}

fn checked(source: &str, verdict: &str, exit: i32, count: usize) -> Value {
    let o = run("check", source, &[]);
    let v = wire(&o);
    assert_eq!(
        v["verdict"],
        verdict,
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(o.status.code(), Some(exit));
    assert_eq!(v["properties"].as_array().unwrap().len(), count);
    unavailable(&v, "work");
    assert!(String::from_utf8_lossy(&o.stderr).contains("W-AGY-4:"));
    v
}

#[test]
fn work_limit_keeps_complete_properties_and_bounded_execution() {
    let s = input::work();
    let v = checked(&s, "satisfied", 0, 2);
    assert!(
        v["properties"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["verdict"] == "satisfied")
    );
    assert_eq!(
        v["bounds"],
        json!({"cases":1,"live_tasks":2,"clock":92,"truncated":false})
    );
    let d = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "W-AGY-4")
        .unwrap();
    assert_eq!(d["span"], call_span(&s, "p"));
    let message = d["message"].as_str().unwrap();
    assert!(
        message.contains("Ledger.post") && message.contains("effect p"),
        "{message}"
    );
    let limit = &v["agency"]["limit"];
    assert!(
        !limit["effect"].is_null(),
        "effect must be recorded: {limit}"
    );
    let judgement = limit["judgement"]
        .as_str()
        .expect("the current judgement must be recorded");
    assert!((1..=11).any(|i| judgement == format!("j{i}")), "{limit}");
    assert!(message.contains(judgement), "{message}");
    assert!(
        v["agency"]["limit"]["phase"]
            .as_str()
            .unwrap()
            .starts_with("refusal")
    );
    assert!(
        v["via_model_paths"].is_null(),
        "without a via model declaration the independent output remains null even when Agency is unavailable"
    );
    assert!(
        [&v["unchecked"], &v["agency"]["unchecked"]]
            .into_iter()
            .filter_map(Value::as_array)
            .flatten()
            .any(|s| s
                .as_str()
                .is_some_and(|s| s.contains("responsibility_default_candidates")
                    && s.contains("not_derived")
                    && s.contains("derivation_limit")))
    );
}

#[test]
fn path_expansion_reports_the_boundary_call_not_a_null_span() {
    let s = input::requires(13);
    let o = run("check", &s, &[]);
    let v = wire(&o);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(v["properties"].as_array().unwrap().len(), 2);
    assert!(
        v["properties"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["verdict"] == "satisfied")
    );
    assert_eq!(
        v["bounds"],
        json!({"cases":1,"live_tasks":2,"clock":96,"truncated":false})
    );
    unavailable(&v, "paths");
    assert_eq!(v["agency"]["limit"]["requested"], 4097);
    let d = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "W-AGY-4")
        .unwrap();
    assert_eq!(d["span"], call_span(&s, "g13"));
    assert!(d["message"].as_str().unwrap().contains("g13"));
}

#[test]
fn violated_at_most_once_is_not_hidden_by_agency_limit() {
    let v = checked(&input::violation(true), "violated", 1, 3);
    assert_eq!(v["bounds"]["truncated"], false);
    let once = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| {
            p["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("contract Once #1"))
        })
        .unwrap();
    assert_eq!(once["verdict"], "violated");
    assert!(
        v["properties"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["id"] != once["id"])
            .all(|p| p["verdict"] == "satisfied")
    );
}

#[test]
fn removing_only_the_contract_changes_violated_to_satisfied() {
    let v = checked(&input::violation(false), "satisfied", 0, 2);
    assert!(
        v["properties"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["verdict"] == "satisfied")
    );
    assert_eq!(v["bounds"]["truncated"], false);
}

#[test]
fn degradation_gate_survives_uncomputed_agency_and_a_done_witness() {
    let v = checked(&input::degradation(), "inconclusive", 2, 1);
    assert_eq!(v["bounds"]["truncated"], true);
    assert_eq!(
        v["properties"][0]["verdict"], "satisfied",
        "a witnessed done does not close the slow degradation arm"
    );
    assert_eq!(
        v["terminals"],
        json!([]),
        "per-case output is not duplicated at the root"
    );
    let done = v["case_outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["terminals"].as_array().unwrap())
        .find(|t| t["end"] == "done")
        .expect("reachable done witness");
    assert_eq!(done["completed_at"], json!({"min":14,"max":14}));
}

#[test]
fn standing_and_contest_validation_accept_matching_uncomputed_agency() {
    let v = checked(&input::contest(), "satisfied", 0, 4);
    let properties = v["properties"].as_array().unwrap();
    assert!(
        properties.iter().all(|p| p["verdict"] == "satisfied"),
        "{properties:?}"
    );
    assert!(
        properties
            .iter()
            .any(|p| p["id"].as_str().unwrap().contains("contest"))
    );
    assert_eq!(v["bounds"]["truncated"], false);
}

#[test]
fn explain_still_replays_the_contract_counterexample() {
    let o = run("explain", &input::violation(true), &[]);
    let v = wire(&o);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(v["verdict"], "violated");
    unavailable(&v, "work");
    assert_eq!(v["explanation"]["status"], "complete");
    let counterexamples = v["explanation"]["counterexamples"].as_array().unwrap();
    assert_eq!(counterexamples.len(), 1);
    assert_eq!(counterexamples[0]["status"], "verified");
    assert_eq!(counterexamples[0]["replay"]["status"], "verified");
    assert!(!counterexamples[0]["steps"].as_array().unwrap().is_empty());
    let commits = counterexamples[0]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["events"].as_array().unwrap().iter().map(move |e| (s, e)))
        .filter(|(_, e)| {
            e["kind"] == "commit" && e["actor"] == "Ledger" && e["capability"] == "post"
        })
        .map(|(s, e)| json!({"time":s["time"],"case_id":e["case_id"],"binding":e["binding"]}))
        .collect::<Vec<_>>();
    assert_eq!(
        commits,
        vec![
            json!({"time":13,"case_id":1,"binding":"p"}),
            json!({"time":14,"case_id":1,"binding":"p2"}),
        ]
    );
}

#[test]
fn simulation_runs_the_declared_probability_kernel_not_an_assumed_uniform_choice() {
    let o = run("simulate", &input::simulation(), &[]);
    let v = wire(&o);
    assert_eq!(
        o.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(v["verdict"], "inconclusive");
    let safety = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "safety no_resource_overcommit")
        .unwrap();
    assert_eq!(safety["verdict"], "inconclusive");
    assert_eq!(safety["basis"], "sample");
    unavailable(&v, "work");
    assert_ne!(v["simulation"]["status"], "unsupported");
    assert_eq!(v["simulation"]["attempted_replications"], 4);
    assert_eq!(v["simulation"]["completed_replications"], 4);
    assert_eq!(v["bounds"]["truncated"], false);
}

#[test]
fn selected_configurations_keep_their_own_derivation_status() {
    let s = input::configs();
    let small = run("check", &s, &["--configuration", "Small"]);
    assert_eq!(small.status.code(), Some(0));
    assert_ne!(wire(&small)["agency"]["status"], "not_derived");
    let big = run("check", &s, &["--configuration", "Big"]);
    assert_eq!(big.status.code(), Some(0));
    unavailable(&wire(&big), "work");
}

#[test]
fn diff_does_not_compare_uncomputed_agency_as_an_empty_derived_value() {
    for (from, to, missing) in [("Small", "Big", "to"), ("Big", "Small", "from")] {
        let o = run("diff", &input::configs(), &["--from", from, "--to", to]);
        let v = wire(&o);
        assert_eq!(o.status.code(), Some(0));
        assert_eq!(v["verdict"], "satisfied");
        unavailable(&v[missing]["report"], "work");
        assert_eq!(v["agency"]["differences"]["status"], "not_derived");
        assert!(
            v["agency"]["differences"]["rows"]
                .as_array()
                .is_none_or(Vec::is_empty)
        );
    }
}

#[test]
fn compare_continues_with_both_property_reports() {
    let o = run(
        "compare",
        &input::configs(),
        &["--plans", "Small,Big", "--mode", "exhaustive"],
    );
    let v = wire(&o);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(v["verdict"], "satisfied");
    // Both selected reports must remain present; merely folding a warning is
    // insufficient if Big's execution was silently skipped.
    let reports = v["plans"].as_array().unwrap();
    assert_eq!(reports.len(), 2);
    assert!(
        reports
            .iter()
            .all(|p| p["report"]["properties"].as_array().unwrap().len() == 2)
    );
    let big = reports
        .iter()
        .find(|p| p["configuration"] == "Big")
        .unwrap();
    unavailable(&big["report"], "work");
    let pairs = v["tables"]["agency"].as_array().unwrap();
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0]["differences"]["status"], "not_derived");
}

#[test]
fn mutation_agency_checks_are_unsupported_not_missed_or_detected() {
    let o = run("mutate", &input::work(), &[]);
    let v = wire(&o);
    assert_eq!(o.status.code(), Some(4));
    assert_eq!(v["original"]["verdict"], "satisfied");
    unavailable(&v["original"], "work");
    for kind in ["evidence_missing", "summary_reused", "refusal_path_cut"] {
        let row = v["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["kind"] == kind)
            .unwrap();
        assert!(
            !row["mutants"].as_array().unwrap().is_empty(),
            "must exercise {kind}"
        );
        for mutant in row["mutants"].as_array().unwrap() {
            assert_eq!(mutant["result"], "unsupported", "{kind}: {mutant}");
            assert_eq!(mutant["reason"], "agency_not_derived", "{kind}: {mutant}");
            assert_eq!(mutant["assigned_changes"], json!([]));
        }
    }
}

#[test]
fn small_models_preserve_entire_stdout_stderr_and_exit() {
    let path = root().join("crates/relca-cli/tests/agency_limit_preservation.json");
    let mut observed = serde_json::Map::new();
    for (name, s, options) in [
        ("gate-2", input::gate(2), vec![]),
        ("gate-3", input::gate(3), vec![]),
        ("and-3", input::joined(3, " && "), vec![]),
        ("or-3", input::joined(3, " || "), vec![]),
        ("or-10", input::joined(10, " || "), vec![]),
        ("continuing-3", input::continuing(3), vec![]),
        ("chain-3", input::chain(3), vec![]),
        (
            "configs-small",
            input::configs(),
            vec!["--configuration", "Small"],
        ),
    ] {
        let o = run("check", &s, &options);
        observed.insert(name.into(), json!({"stdout":String::from_utf8(o.stdout).unwrap(),"stderr":String::from_utf8(o.stderr).unwrap(),"exit":o.status.code()}));
    }
    if std::env::var_os("RELCA_RECORD_AGENCY_LIMIT_PRESERVATION").is_some() {
        std::fs::write(
            path,
            serde_json::to_string_pretty(&observed).unwrap() + "\n",
        )
        .unwrap();
    } else {
        let expected: Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(json!(observed), expected);
    }
}
