use super::{
    blocker, diagnostic, invoke, location,
    unit5::{BASE, DISTRIBUTION as DIST, SEE},
};
use serde_json::{Value, json};

const RECOVERY: &str = include_str!("../snapshots/inputs/r2-recovery.relca");

fn at(role: &str, name: Option<&str>, line: usize, start: usize, end: usize) -> Value {
    blocker(role, name, location(line, start, line, end))
}

// Supplement expectations are rendered from the hand-authored coordinates,
// never from the report. Include the header with the preexisting code.
fn refusal(
    (exit, report, stderr): (i32, Value, String),
    code: &str,
    message: &str,
    (reason, cause): (&str, &str),
    loc: Value,
    blockers: Vec<Value>,
) {
    assert_eq!(exit, 4, "{stderr}");
    assert_eq!(report["verdict"], "unsupported");
    let d = diagnostic(&report, message);
    assert_eq!(d["code"], code);
    assert_eq!(d["severity"], "error");
    assert_eq!(d["location"], loc);
    assert_eq!(
        d["detail"],
        json!({"reason":reason,"cause":cause,"kind":"analysis_not_connected","blockers":blockers,"see":SEE})
    );
    use std::fmt::Write as _;
    let mut expected = String::new();
    if !loc.is_null() {
        writeln!(expected, "  at {}:{}", loc["line"], loc["column"]).unwrap();
    }
    writeln!(expected, "  kind: analysis_not_connected\n  cause: {cause}").unwrap();
    for b in blockers {
        write!(expected, "  blocked by: {}", b["role"].as_str().unwrap()).unwrap();
        if let Some(name) = b["name"].as_str() {
            write!(expected, " {name}").unwrap();
        }
        let l = &b["location"];
        if !l.is_null() {
            write!(expected, " at {}:{}", l["line"], l["column"]).unwrap();
        }
        writeln!(expected).unwrap();
    }
    writeln!(expected, "  see: {SEE}").unwrap();
    let (_, tail) = stderr.split_once(&format!("{code}: {message}\n")).unwrap();
    let actual: String = tail
        .split_inclusive('\n')
        .take_while(|l| l.starts_with("  "))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn compare_rejects_distributed_arrivals_in_both_modes() {
    let source = BASE.replace("arrivals fixed [0min]", DIST).replace(
        "  process P { r = A.run(); end done }",
        "  process P { r = A.run(); end done }\n  configuration Old { cut C; process P }\n  configuration New { cut C; process P }",
    );
    let reason = "stochastic_arrivals_mode_unsupported";
    for (mode, cause) in [
        ("exhaustive", "compare_exhaustive_stochastic_arrivals"),
        ("simulation", "compare_simulation_stochastic_arrivals"),
    ] {
        refusal(
            invoke("compare", &source, &["--plans", "Old,New", "--mode", mode]),
            "U-SEM-0",
            reason,
            (reason, cause),
            Value::Null,
            vec![],
        );
    }
}

#[test]
fn stochastic_arrival_blockers_identify_quality_and_cut() {
    let source = BASE.replace("arrivals fixed [0min]", DIST);
    let reason = "stochastic_arrivals_feature_combination";
    for (input, cause, name) in [
        (
            source.replace(
                "  verify safety",
                "  metric mean.Cost = mean(cost)\n  verify safety",
            ),
            "quality",
            "Cost",
        ),
        (
            source.replace(
                "include [A]",
                "include [A]; defer Foo reason \"outside\" loses \"unknown effects\"",
            ),
            "cut_defer",
            "Foo",
        ),
    ] {
        refusal(
            invoke("simulate", &input, &[]),
            "U-SEM-0",
            reason,
            (reason, cause),
            location(5, 3, 5, 38),
            vec![
                at("feature", None, 7, 3, 54),
                blocker("feature", Some(name), Value::Null),
            ],
        );
    }
}

#[test]
fn mean_names_the_merge_binding_and_metric() {
    let source = BASE
        .replace(
            "r = A.run();",
            "r = A.run(); v = if r == ok { yield r } else { yield r };",
        )
        .replace(
            "  verify safety",
            "  metric mean.Cost = mean(cost)\n  verify safety",
        );
    refusal(
        invoke("check", &source, &[]),
        "U-SEM-0",
        "mean_feature_combination: merge and receipt/retry are unsupported",
        ("mean_feature_combination", "merge"),
        location(1, 1, 11, 2),
        vec![
            at("merge_binding", Some("v"), 5, 28, 29),
            at("feature", Some("Cost"), 9, 3, 32),
        ],
    );
}

#[test]
fn recovery_call_placement() {
    let reason = "recovery_feature_combination";
    for (process, cause, loc, rows) in [
        (
            "r = A.run(); q = A.run() for command r;",
            "query_outside_recovery",
            location(5, 28, 5, 53),
            vec![
                at("call", Some("q"), 5, 28, 53),
                at("reference", Some("r"), 5, 52, 53),
            ],
        ),
        (
            "fork F { q = A.run() for command r }; join all F;",
            "fork_recovery",
            location(5, 24, 5, 49),
            vec![
                at("feature", Some("F"), 5, 15, 63),
                at("call", Some("q"), 5, 24, 49),
            ],
        ),
        (
            "r = A.run() response_loss none on receipt_unknown { stop receipt_unknown };",
            "unmodeled_recovery_mechanism",
            location(5, 15, 5, 89),
            vec![
                at("feature", Some("A.run"), 3, 25, 92),
                at("call", Some("r"), 5, 15, 89),
            ],
        ),
    ] {
        let source = BASE.replace("r = A.run();", process);
        refusal(
            invoke("check", &source, &[]),
            "U-SEM-0",
            reason,
            (reason, cause),
            loc,
            rows,
        );
    }
}

#[test]
fn recovery_refuses_a_response_channel_in_the_handler() {
    let source = RECOVERY.replace(
        "decision = CS.decide(status)",
        "decision = CS.decide(status) response_loss none",
    );
    let reason = "recovery_feature_combination";
    refusal(
        invoke("check", &source, &["--scenario", "Full"]),
        "U-SEM-0",
        reason,
        (reason, "handler_response_channel"),
        location(29, 9, 29, 56),
        vec![
            blocker("call", Some("pay"), location(26, 5, 36, 6)),
            at("call", Some("decision"), 29, 9, 56),
        ],
    );
}

#[test]
fn policy_recovery_names_the_policy_and_call() {
    let source = RECOVERY.replace(
        "  metrics",
        "  case_priority fixed [0]\n  policy observable priority descending\n  metrics",
    );
    refusal(
        invoke("check", &source, &["--scenario", "Full"]),
        "U-SEM-0",
        "policy_feature_combination: policy ordering with a receipt query or recovery handler is unsupported",
        ("policy_feature_combination", "recovery"),
        location(1, 1, 50, 2),
        vec![
            blocker("call", Some("pay"), location(26, 5, 36, 6)),
            at("feature", None, 46, 3, 40),
        ],
    );
}

#[test]
fn unavailable_condition_and_yield_are_distinct() {
    let source = r#"model M version "0.2" {
  type V = enum { ok, bad }
  actor A { capacity 1; capability run() -> V { duration fixed 1min; result nondet { ok, bad } } }
  cut C { include [A] }
  process P {
    r = A.run()
    if r == ok { q = A.run() } else {}
    end done
  }
  bounds { cases 1; live_tasks 1; clock 20min }
  verify safety no_resource_overcommit
}
"#;
    let reason = "result_not_definitely_available";
    for (stmt, cause, from) in [
        ("if q == ok {} else {}", "condition_result", 8),
        (
            "v = if r == ok { yield q } else { yield r }",
            "merge_result",
            28,
        ),
    ] {
        let input = source.replace("    end done", &format!("    {stmt}\n    end done"));
        refusal(
            invoke("check", &input, &[]),
            "U-SEM-0",
            "result is not definitely available (not yet defined or possibly cancelled)",
            (reason, cause),
            location(8, from, 8, from + 1),
            vec![
                at("call", Some("q"), 7, 18, 29),
                at("reference", Some("q"), 8, from, from + 1),
            ],
        );
    }
}

#[test]
fn unavailable_result_at_start_of_its_own_call() {
    let source = BASE.replace(
        "  verify safety",
        "  contract Ck { invariant A.run.start -> r == ok }\n  verify safety",
    );
    refusal(
        invoke("check", &source, &[]),
        "U-SEM-0",
        "contract result is not definitely available at every call site",
        ("result_not_definitely_available", "contract_result"),
        location(9, 42, 9, 43),
        vec![
            at("call", Some("r"), 5, 15, 26),
            at("reference", Some("r"), 9, 42, 43),
        ],
    );
}

#[test]
fn anonymous_executor_reference_points_to_actor() {
    let source = r#"model M version "0.2" {
  type V = enum { ok }
  people [p]
  observed case { who: optional<PersonId> }
  actor A { capacity 1
    capability run() -> V {
      duration fixed 1min; result nondet { ok }
      writes who; finish { who = executor.id }
    }
  }
  cut C { include [A] }
  process P { r = A.run(); end done }
  bounds { cases 1; live_tasks 1; clock 20min }
  verify safety no_resource_overcommit
}
"#;
    refusal(
        invoke("check", source, &[]),
        "U-SEM-0",
        "executor.id requires declared executors",
        ("executor_identity_unavailable", "executor_reference"),
        location(8, 34, 8, 45),
        vec![blocker("feature", Some("A"), location(5, 3, 10, 4))],
    );
}

#[test]
fn stochastic_arrivals_name_the_wait() {
    let source = BASE
        .replace("arrivals fixed [0min]", DIST)
        .replace(
            "  actor A",
            "  observed case { ready: bool = true }\n  actor A",
        )
        .replace("r = A.run();", "r = A.run(); wait until ready == true;");
    let reason = "stochastic_arrivals_feature_combination";
    refusal(
        invoke("simulate", &source, &[]),
        "U-SEM-0",
        reason,
        (reason, "condition_wait"),
        location(6, 3, 6, 64),
        vec![
            at("feature", None, 6, 28, 52),
            at("feature", None, 8, 3, 54),
        ],
    );
}

const CONTEST: &str = r#"model Contest version "0.2" {
  type V = enum { pass, fail }
  people [p, cfo]
  actor A { executors [p]; capability judge() -> V { duration fixed 1min; result nondet { pass } } }
  actor P { capacity 1; capability post() -> V { commits; duration fixed 1min; result nondet { pass } } }
  actor Clerk { capacity 1; capability recheck() -> V { duration fixed 1min; result nondet { pass } } }
  actor Vendor { control party; standing over [P.post.commit] basis "claim" }
  authority Grant { basis "grant"; grants p to A.judge; issued_by cfo }
  process Flow { v0 = A.judge(); if v0 == pass { posted = P.post(); end done } else { end rejected } }
  cut C { include [A, P, Clerk, Vendor] }
  contract Controls {
    contest Vendor on P.post.commit route Clerk.recheck reversible_by [p] respond_by 5min
  }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify safety no_resource_overcommit
}
"#;

#[test]
fn contest_blocks_stochastic_arrivals_and_response_analysis() {
    let reason = "stochastic_arrivals_feature_combination";
    let source = CONTEST.replace(
        "  verify safety",
        &format!("  {DIST}\n  simulation replications 2 seed 1 confidence 0.5\n  verify safety"),
    );
    refusal(
        invoke("simulate", &source, &[]),
        "U-SEM-0",
        reason,
        (reason, "contest"),
        location(9, 3, 9, 103),
        vec![
            at("contest", None, 12, 5, 90),
            at("feature", None, 15, 3, 54),
        ],
    );

    let reason = "retry_feature_combination";
    let source = CONTEST.replace("v0 = A.judge()", "v0 = A.judge() response_loss none");
    refusal(
        invoke("check", &source, &[]),
        "U-SEM-0",
        "retry_feature_combination: receipt/retry with contest or quantiles is unsupported",
        (reason, "contest"),
        location(1, 1, 16, 2),
        vec![
            at("call", Some("v0"), 9, 18, 51),
            at("contest", None, 12, 5, 90),
        ],
    );
}

#[test]
fn contest_blocks_recovery_analysis() {
    let source = RECOVERY
        .replace(
            "include [Psp,CS,Shop]",
            "include [Psp,CS,Shop,Clerk,Vendor]",
        )
        .strip_suffix("}\n")
        .unwrap()
        .to_owned()
        + r#"  people [p]
  actor Clerk { executors [p]; capability recheck() -> Ack { duration fixed 1min; result nondet { recorded } } }
  actor Vendor { control party; standing over [end.shipped] basis "claim" }
  contract Review {
    contest Vendor on end.shipped route Clerk.recheck reversible_by [p] respond_by 5min
  }
}
"#;
    refusal(
        invoke("check", &source, &["--scenario", "Full"]),
        "U-SEM-0",
        "recovery_feature_combination: contest or quantiles are unsupported",
        ("recovery_feature_combination", "contest"),
        location(1, 1, 54, 2),
        vec![
            blocker("call", Some("pay"), location(26, 5, 36, 6)),
            at("contest", None, 52, 5, 88),
        ],
    );
}

#[test]
fn quantile_blocks_response_and_recovery_analysis() {
    const METRICS: &str = "  property case.Done = reached(end.done)\n  metric quantile.Median = quantile(0.5, case.Done)\n";
    let source = BASE
        .replace("r = A.run()", "r = A.run() response_loss none")
        .replace("  verify", &format!("{METRICS}  verify"));
    refusal(
        invoke("check", &source, &[]),
        "U-SEM-0",
        "retry_feature_combination: receipt/retry with contest or quantiles is unsupported",
        ("retry_feature_combination", "quantile"),
        location(1, 1, 12, 2),
        vec![
            at("call", Some("r"), 5, 15, 45),
            at("quantile", Some("Median"), 10, 3, 52),
        ],
    );

    let source = RECOVERY.replace(
        "  verify probability",
        &format!(
            "{}  verify probability",
            METRICS.replace("end.done", "end.shipped")
        ),
    );
    refusal(
        invoke("check", &source, &["--scenario", "Full"]),
        "U-SEM-0",
        "recovery_feature_combination: contest or quantiles are unsupported",
        ("recovery_feature_combination", "quantile"),
        location(1, 1, 50, 2),
        vec![
            blocker("call", Some("pay"), location(26, 5, 36, 6)),
            at("quantile", Some("Median"), 48, 3, 52),
        ],
    );
}
fn side_input(name: &str, text: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("diagnostic-{name}-{}", std::process::id()));
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn sensitivity_projection_and_completion_refusals_have_no_location() {
    let base = BASE.replace(
        "  verify safety",
        "  verify probability reached end.done >= 1\n  verify safety",
    );
    let reason = "sensitivity_feature_combination";
    let shared = include_str!("../../../../fixtures/cf-lww/model.relca").replace(
        "  verify reachable",
        "  verify probability reached end.claimed >= 1\n  verify reachable",
    );
    let fallback = include_str!("../../../../fixtures/agy2-keep-fallback/model.relca").replace(
        "  verify safety",
        "  verify probability reached end.posted >= 1\n  verify safety",
    );
    let contest = CONTEST.replace(
        "  verify safety",
        "  verify probability reached end.done >= 1\n  verify safety",
    );
    const FAMILY: &str =
        "contract projection: calendars/shared-resolution families not yet projected";
    for (source, configuration, cause, mode, message) in [
        (base.replace("  actor A", "  calendar Day through 21min on_close continue { [0min, 21min) capacity 1 }\n  actor A").replace("capacity 1; capability", "capacity 1; calendar Day; capability"), None, "sensitivity_calendar_projection", "exhaustive", FAMILY),
        (shared, None, "sensitivity_shared_resolution_projection", "exhaustive", FAMILY),
        (fallback, Some("To"), "sensitivity_fallback_projection", "exhaustive", FAMILY),
        (contest, None, "sensitivity_contest_projection", "exhaustive", "contract projection: contest sensitivity not projected"),
        (base.replace("  verify safety", "  verify terminating end\n  verify safety"), None, "sensitivity_simulation_completion", "simulation", "structural completion is not available to simulation"),
    ] {
        let point = side_input(cause, &source);
        let confidence = if mode == "simulation" { json!("1/2") } else { Value::Null };
        let manifest = json!({"schema":"relca-sensitivity/v1", "scope":{"kind":"enumerated_models","rationale":"test"},
            "confidence":confidence,"points":[{"id":"p","model":point.file_name().unwrap().to_str().unwrap(),"scenario":null,"configuration":configuration}],
            "targets":[{"kind":"probability_query","id":"probability reached #1"}]});
        let message = format!("{reason}: point \"p\": {reason}: point \"p\": {message}");
        refusal(invoke("sensitivity", &manifest.to_string(), &["--mode", mode]), "U-SEM-0", &message, (reason, cause), Value::Null, vec![]);
    }
}

#[test]
fn conformance_schema_and_time_unit_are_spanless() {
    let header = r#"{"kind":"header","schema":"relca-log/v1","clock":"c","time_unit":"min","window":{"from":0,"to":10},"cases":[],"actor_map":[],"executor_map":[]}"#;
    for (log, reason, cause) in [
        (
            header.replace("relca-log/v1", "relca-log/v9"),
            "schema",
            "log_schema",
        ),
        (
            header.replace("\"min\"", "\"sec\""),
            "time_unit",
            "log_time_unit",
        ),
    ] {
        let log = side_input(cause, &log);
        refusal(
            invoke(
                "conform",
                BASE,
                &["--log", log.to_str().unwrap(), "--scope", "event-contracts"],
            ),
            "U-LOG-0",
            reason,
            (reason, cause),
            Value::Null,
            vec![],
        );
    }
}

#[test]
fn conformance_approval_schema_and_profiles_are_spanless() {
    let header = r#"{"kind":"header","schema":"relca-log/v1","clock":"c","time_unit":"min","window":{"from":0,"to":10},"cases":[],"actor_map":[],"executor_map":[]}"#;
    let log = side_input("approval-log", header);
    for (spec, code, reason, cause) in [
        (
            json!({"schema":"relca-approval-spec/v9"}),
            "U-LOG-0",
            "approval_spec_schema",
            "log_approval_schema",
        ),
        (
            json!({"schema":"relca-approval-spec/v1", "profile":[{},{}]}),
            "U-LOG-1",
            "unsupported_approval_profiles",
            "log_approval_profiles",
        ),
    ] {
        let spec = side_input(cause, &spec.to_string());
        refusal(
            invoke(
                "conform",
                BASE,
                &[
                    "--log",
                    log.to_str().unwrap(),
                    "--scope",
                    "approval-burden",
                    "--approval-spec",
                    spec.to_str().unwrap(),
                ],
            ),
            code,
            reason,
            (reason, cause),
            Value::Null,
            vec![],
        );
    }
}

const APPROVAL_RECORD: &str = r#"model Approval version "0.2" {
  type V = enum { ok }
  type Evidence = record { flag: bool }
  actor A { capacity 1
    capability propose() -> Evidence { duration fixed 1min; result nondet { Evidence { flag = true } } }
    capability decide() -> V { duration fixed 1min; result nondet { ok } }
    capability review() -> V { duration fixed 1min; result nondet { ok } }
  }
  process P { p = A.propose(); d = A.decide(); r = A.review(); end done }
  cut C { include [A] }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify safety no_resource_overcommit
}
"#;
const APPROVAL_LOG: &str = r#"{"kind":"header","schema":"relca-log/v1","clock":"c","time_unit":"min","window":{"from":0,"to":10},"cases":[],"actor_map":[],"executor_map":[]}
"#;

#[test]
fn conformance_refuses_record_return_in_approval_profile() {
    let log = side_input("approval-record-log", APPROVAL_LOG);
    let spec = json!({"schema":"relca-approval-spec/v1",
        "model_sha256":"99a757919ac3ea387a9c14ff23162f9f9d325b72789463f5bcbb1fa1d24320b1",
        "log_sha256":"d90cb4ca742bb2e4f4b4fbee1ad3a2bc4f95b1133313d2e6b09279c6bd8e4dcd",
        "profile":{"approval_operation":"A.decide","proposal_operation":"A.propose",
            "proposal_approve_results":["yes"],"approval_decision_results":["ok"],
            "approval_approve_results":["ok"],"reinvestigation_operations":["A.review"]},"episodes":[]});
    let spec = side_input("approval-record-spec", &spec.to_string());
    let reason = "unsupported_approval_semantics";
    refusal(
        invoke(
            "conform",
            APPROVAL_RECORD,
            &[
                "--log",
                log.to_str().unwrap(),
                "--scope",
                "approval-burden",
                "--approval-spec",
                spec.to_str().unwrap(),
            ],
        ),
        "U-LOG-1",
        reason,
        (reason, "log_approval_semantics"),
        Value::Null,
        vec![],
    );
}

#[test]
fn conformance_names_unsupported_contract_and_verify_kinds() {
    let log = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots/inputs/r2-conform.jsonl");
    for (message, reason, cause, loc) in [
        (
            "contract:1: unsupported_contract",
            "unsupported_contract",
            "log_contract_kind",
            location(41, 22, 41, 116),
        ),
        (
            "verify:2: unsupported_verify",
            "unsupported_verify",
            "log_verify_kind",
            location(47, 3, 47, 56),
        ),
    ] {
        refusal(
            invoke(
                "conform",
                RECOVERY,
                &[
                    "--log",
                    log.to_str().unwrap(),
                    "--scope",
                    "event-contracts",
                    "--scenario",
                    "Full",
                ],
            ),
            "U-LOG-1",
            message,
            (reason, cause),
            loc.clone(),
            vec![blocker("feature", None, loc)],
        );
    }
}

#[test]
fn conformance_cancel_and_capability_name_the_declaration() {
    let header = json!({"kind":"header","schema":"relca-log/v1","clock":"c","time_unit":"min","window":{"from":0,"to":10},
        "cases":[{"case_id":"c","from_case_start":true,"through_case_end":true}],
        "actor_map":[{"source_id":"a","actor":"A"}],"executor_map":[{"source_id":"ep","person":"p"}]});
    let event = |id, lifecycle, phase, tick, after| {
        json!({"kind":"event","event_id":id,"case_id":"c","task_id":"t","operation":"A.run","lifecycle":lifecycle,
        "cancel_phase":phase,"time":{"earliest":tick,"latest":tick},"after":after,"actor_id":"a","executor_id":"ep","input_version":null,"output_version":null,"result":null})
    };
    let start = event("s", "start", None, 0, Vec::<&str>::new());
    let request = event("r", "cancel", Some("request"), 1, vec!["s"]);
    let ack = event("a", "cancel", Some("ack"), 2, vec!["r"]);
    let log = side_input(
        "cancel-log",
        &format!("{header}\n{start}\n{request}\n{ack}\n"),
    );
    let args = ["--log", log.to_str().unwrap(), "--scope", "event-contracts"];
    let src = BASE.replace(
        "  actor A { capacity 1",
        "  people [p]\n  actor A { executors [p]",
    );
    let loc = location(4, 28, 4, 95);
    refusal(
        invoke("conform", &src, &args),
        "U-LOG-1",
        "task:t:cancel: unsupported_cancel_semantics",
        ("unsupported_cancel_semantics", "log_cancel_semantics"),
        loc.clone(),
        vec![blocker("feature", None, loc)],
    );

    let source = src.replace("duration fixed 1min", "duration { 1min: 1 }");
    let loc = location(4, 28, 4, 96);
    refusal(
        invoke("conform", &source, &args),
        "U-LOG-1",
        "task:t:capability:A.run:duration_distribution: unsupported_capability_duration_distribution",
        ("unsupported_capability_lifecycle", "log_capability_field"),
        loc.clone(),
        vec![blocker("feature", None, loc)],
    );
}
