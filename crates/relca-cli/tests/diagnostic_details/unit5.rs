use super::{blocker, diagnostic, invoke, location};
use serde_json::{Value, json};

pub(super) const SEE: &str = "docs/guide/dsl.md#解析が未接続のとき";
pub(super) const BASE: &str = r#"model M version "0.2" {
  type V = enum { ok }
  actor A { capacity 1; capability run() -> V { duration fixed 1min; result nondet { ok } } }
  cut C { include [A] }
  process P { r = A.run(); end done }
  bounds { cases 1; live_tasks 1; clock 20min }
  arrivals fixed [0min]
  simulation replications 2 seed 1 confidence 0.5
  verify safety no_resource_overcommit
}
"#;
pub(super) const DISTRIBUTION: &str = "arrivals distribution basis \"x\" { 1: fixed [0min] }";

fn row(role: &str, name: Option<&str>, line: usize, start: usize, end: usize) -> Value {
    blocker(role, name, location(line, start, line, end))
}

pub(super) fn rejected(
    (exit, report, stderr): (i32, Value, String),
    message: &str,
    reason: &str,
    cause: &str,
    loc: Value,
    blockers: Vec<Value>,
    complete_supplement: &str,
) {
    assert_eq!(exit, 4, "{stderr}");
    assert_eq!(report["verdict"], "unsupported");
    let d = diagnostic(&report, message);
    assert_eq!(d["location"], loc, "{stderr}");
    assert_eq!(
        d["detail"],
        json!({"reason":reason,"cause":cause,"kind":"analysis_not_connected","blockers":blockers,"see":SEE}),
        "{stderr}"
    );
    let header = format!("{}: {message}", d["code"].as_str().unwrap());
    let (_, after) = stderr.split_once(&format!("{header}\n")).unwrap();
    let actual: String = after
        .lines()
        .take_while(|line| line.starts_with("  "))
        .map(|line| format!("{line}\n"))
        .collect();
    assert_eq!(actual, complete_supplement);
}

pub(super) fn accepts(command: &str, source: &str, args: &[&str], reason: &str) {
    let (exit, _, stderr) = invoke(command, source, args);
    assert_eq!(exit, 0, "{stderr}");
    assert!(!stderr.contains(reason), "{stderr}");
}

#[test]
fn stochastic_arrivals_mode_unsupported() {
    let reason = "stochastic_arrivals_mode_unsupported";
    let source = BASE.replace("arrivals fixed [0min]", DISTRIBUTION);
    rejected(
        invoke("check", &source, &[]),
        reason,
        reason,
        "exhaustive_stochastic_arrivals",
        location(5, 3, 5, 38),
        vec![row("feature", None, 7, 3, 54)],
        "  at 5:3\n  kind: analysis_not_connected\n  cause: exhaustive_stochastic_arrivals\n  blocked by: feature at 7:3\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    accepts("check", BASE, &[], reason);
    let (exit, _, stderr) = invoke("simulate", &source, &[]);
    assert_eq!(exit, 2, "the safety question is not certified by sampling");
    assert!(!stderr.contains(reason));
}

#[test]
fn stochastic_arrivals_feature_combination() {
    let reason = "stochastic_arrivals_feature_combination";
    let source = BASE.replace("arrivals fixed [0min]", DISTRIBUTION);
    let refused = source.replace("r = A.run()", "r = A.run() response_loss none");
    rejected(
        invoke("simulate", &refused, &[]),
        reason,
        reason,
        "response_or_retry",
        location(5, 3, 5, 57),
        vec![
            row("call", Some("r"), 5, 15, 45),
            row("feature", None, 7, 3, 54),
        ],
        "  at 5:3\n  kind: analysis_not_connected\n  cause: response_or_retry\n  blocked by: call r at 5:15\n  blocked by: feature at 7:3\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    let (exit, _, stderr) = invoke("simulate", &source, &[]);
    assert_eq!(exit, 2, "the safety question is not certified by sampling");
    assert!(!stderr.contains(reason));
}

#[test]
fn recovery_feature_combination() {
    let reason = "recovery_feature_combination";
    let base = include_str!("../snapshots/inputs/r2-recovery.relca");
    let source = base.replace("for command pay\n", "for command pay response_loss none\n");
    rejected(
        invoke("check", &source, &["--scenario", "Full"]),
        reason,
        reason,
        "query_response_channel",
        location(27, 58, 27, 71),
        vec![
            row("reference", Some("pay"), 27, 54, 57),
            row("feature", None, 27, 58, 71),
        ],
        "  at 27:58\n  kind: analysis_not_connected\n  cause: query_response_channel\n  blocked by: reference pay at 27:54\n  blocked by: feature at 27:58\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    let result = invoke("mutate", base, &["--scenario", "Full"]);
    rejected(
        result,
        reason,
        reason,
        "mutate_recovery",
        Value::Null,
        vec![],
        "  kind: analysis_not_connected\n  cause: mutate_recovery\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    let (exit, _, stderr) = invoke("check", base, &["--scenario", "Full"]);
    assert_eq!(exit, 1, "{stderr}");
    assert!(!stderr.contains(reason));
    assert!(!invoke("mutate", BASE, &[]).2.contains(reason));
}

#[test]
fn mean_feature_combination() {
    let reason = "mean_feature_combination";
    let source = BASE.replace(
        "  verify safety",
        "  metric mean.Cost = mean(cost)\n  verify safety",
    );
    let refused = source.replace("r = A.run()", "r = A.run() response_loss none");
    rejected(
        invoke("check", &refused, &[]),
        "mean_feature_combination: merge and receipt/retry are unsupported",
        reason,
        "response_or_retry",
        location(1, 1, 11, 2),
        vec![
            row("call", Some("r"), 5, 15, 45),
            row("feature", Some("Cost"), 9, 3, 32),
        ],
        "  at 1:1\n  kind: analysis_not_connected\n  cause: response_or_retry\n  blocked by: call r at 5:15\n  blocked by: feature Cost at 9:3\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    accepts("check", &source, &[], reason);
}

#[test]
fn priority_feature_combination_verify() {
    let reason = "priority_feature_combination";
    rejected(
        invoke(
            "verify",
            include_str!("../snapshots/inputs/q5-external.relca"),
            &[],
        ),
        "priority_feature_combination: verify backend is not connected to priority disciplines",
        reason,
        "verify_priority",
        Value::Null,
        vec![],
        "  kind: analysis_not_connected\n  cause: verify_priority\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    // Without priority, verify still has no external backend, but it has no
    // priority refusal.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/serial-time/model.relca");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("verify")
        .arg(path)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(4));
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(report.get("diagnostics").is_none());
    assert!(!String::from_utf8(out.stderr).unwrap().contains(reason));
}

const RESOLUTION: &str = r#"model R version "0.2" {
  type V = enum { ok, conflict }
  actor A { control policy; capacity 1; state holder: bool = false
    capability run() -> V {
      duration fixed 1min; result nondet { ok }
      version_check [holder] else conflict resolve priority descending
      writes holder; finish { holder = true }
    }
  }
  cut C { include [A] }
  process P { r = A.run(); end done }
  case_priority fixed [0]
  bounds { cases 1; live_tasks 1; clock 20min }
  verify safety no_resource_overcommit
}
"#;

#[test]
fn priority_resolution_scope_and_retry() {
    let source = RESOLUTION.replace("control policy", "control environment");
    let reason = "priority_resolution_scope_unsupported";
    rejected(
        invoke("check", &source, &[]),
        reason,
        reason,
        "non_policy_actor",
        location(6, 7, 6, 71),
        vec![
            blocker("feature", Some("A"), location(3, 3, 9, 4)),
            row("feature", Some("A.run"), 6, 7, 71),
        ],
        "  at 6:7\n  kind: analysis_not_connected\n  cause: non_policy_actor\n  blocked by: feature A at 3:3\n  blocked by: feature A.run at 6:7\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    let reason = "retry_feature_combination";
    let source = RESOLUTION.replace("r = A.run();", "r = A.run() response_loss none retry max_attempts 2 when result [conflict] backoff fixed 0min exhausted end lost;");
    rejected(
        invoke("check", &source, &[]),
        "retry_feature_combination: version_check retry",
        reason,
        "version_check",
        location(11, 46, 11, 127),
        vec![
            row("feature", Some("A.run"), 6, 7, 71),
            row("call", Some("r"), 11, 15, 127),
        ],
        "  at 11:46\n  kind: analysis_not_connected\n  cause: version_check\n  blocked by: feature A.run at 6:7\n  blocked by: call r at 11:15\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    accepts("check", RESOLUTION, &[], reason);
}

#[test]
fn sensitivity_schema_unsupported() {
    // This suite has a single point using the checked BASE model. Its
    // relative path must be in the same directory as the manifest.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp");
    std::fs::create_dir_all(&dir).unwrap();
    let name = format!("d3-suite-point-{}.relca", std::process::id());
    let source = BASE.replace(
        "  verify safety",
        "  verify probability reached end.done >= 1\n  verify safety",
    );
    std::fs::write(dir.join(&name), source).unwrap();
    let mut manifest = json!({"schema":"relca-sensitivity/v2", "scope":{"kind":"enumerated_models","rationale":"test"},
        "confidence":null,"points":[{"id":"p","model":name,"scenario":null,"configuration":null}],
        "targets":[{"kind":"probability_query","id":"probability reached #1"}]});
    let reason = "sensitivity_schema_unsupported";
    rejected(
        invoke(
            "sensitivity",
            &manifest.to_string(),
            &["--mode", "exhaustive"],
        ),
        "sensitivity_schema_unsupported: only relca-sensitivity/v1 enumerated_models is supported",
        reason,
        "manifest_schema",
        Value::Null,
        vec![],
        "  kind: analysis_not_connected\n  cause: manifest_schema\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    manifest["schema"] = json!("relca-sensitivity/v1");
    accepts(
        "sensitivity",
        &manifest.to_string(),
        &["--mode", "exhaustive"],
        reason,
    );
    manifest["targets"][0]["kind"] = json!("cartesian");
    rejected(
        invoke(
            "sensitivity",
            &manifest.to_string(),
            &["--mode", "exhaustive"],
        ),
        "sensitivity_schema_unsupported: only probability_query targets are supported",
        reason,
        "manifest_target_kind",
        Value::Null,
        vec![],
        "  kind: analysis_not_connected\n  cause: manifest_target_kind\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    manifest["targets"][0]["kind"] = json!("probability_query");
    let point = dir.join(&name);
    let source = std::fs::read_to_string(&point).unwrap();
    std::fs::write(
        &point,
        source.replace("arrivals fixed [0min]", DISTRIBUTION),
    )
    .unwrap();
    rejected(
        invoke(
            "sensitivity",
            &manifest.to_string(),
            &["--mode", "exhaustive"],
        ),
        "sensitivity_feature_combination: point \"p\": sensitivity_feature_combination: point \"p\": finite arrival distributions are simulation only",
        "sensitivity_feature_combination",
        "sensitivity_exhaustive_stochastic_arrivals",
        Value::Null,
        vec![],
        "  kind: analysis_not_connected\n  cause: sensitivity_exhaustive_stochastic_arrivals\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
}

#[test]
fn retry_fork_response_channel() {
    let reason = "retry_feature_combination";
    let control = BASE.replace("r = A.run();", "fork F { r = A.run() }; join all F;");
    let refused = control.replace("A.run() }", "A.run() response_loss sequence [unknown] }");
    rejected(
        invoke("check", &refused, &[]),
        "retry_feature_combination: fork response channel",
        reason,
        "fork_response_channel",
        location(5, 24, 5, 68),
        vec![
            row("feature", Some("F"), 5, 15, 82),
            row("call", Some("r"), 5, 24, 68),
        ],
        "  at 5:24\n  kind: analysis_not_connected\n  cause: fork_response_channel\n  blocked by: feature F at 5:15\n  blocked by: call r at 5:24\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    accepts("check", &control, &[], reason);
}

#[test]
fn policy_merge() {
    let reason = "policy_feature_combination";
    let control = BASE.replace(
        "r = A.run();",
        "r = A.run(); v = if r == ok { yield r } else { yield r };",
    );
    let refused = control.replace(
        "  bounds",
        "  policy observable priority descending\n  case_priority fixed [0]\n  bounds",
    );
    rejected(
        invoke("check", &refused, &[]),
        "policy_feature_combination: policy ordering with merge is unsupported",
        reason,
        "merge",
        location(1, 1, 12, 2),
        vec![
            row("merge_binding", Some("v"), 5, 28, 29),
            row("feature", None, 6, 3, 40),
        ],
        "  at 1:1\n  kind: analysis_not_connected\n  cause: merge\n  blocked by: merge_binding v at 5:28\n  blocked by: feature at 6:3\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    accepts("check", &control, &[], reason);
}

#[test]
fn policy_merge_is_reported_once_across_scenarios() {
    let source = BASE
        .replace("r = A.run();", "r = A.run(); v = if r == ok { yield r } else { yield r };")
        .replace("  bounds", "  case_priority fixed [0]\n  scenario S1 { policy observable priority descending }\n  scenario S2 {   policy observable priority descending }\n  bounds");
    let message = "policy_feature_combination: policy ordering with merge is unsupported";
    for args in [&[][..], &["--scenario", "S1"][..]] {
        let result = invoke("check", &source, args);
        let matching: Vec<_> = result.1["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["message"] == message)
            .collect();
        assert_eq!(matching.len(), 1, "{}", result.2);
        assert_eq!(matching[0]["code"], "U-SEM-0");
        assert_eq!(matching[0]["severity"], "error");
        assert_eq!(matching[0]["span"], json!({"start":0,"end":550}));
        assert_eq!(
            result.2.matches(&format!("U-SEM-0: {message}\n")).count(),
            1
        );
        assert_eq!(matching[0]["location"], location(1, 1, 13, 2));
        assert_eq!(
            matching[0]["detail"],
            json!({
                "reason":"policy_feature_combination", "cause":"merge", "kind":"analysis_not_connected", "see":SEE,
                "blockers":[row("merge_binding", Some("v"), 5, 28, 29), row("feature", None, 7, 17, 54), row("feature", None, 8, 19, 56)]
            })
        );
        let prefix = if args.is_empty() {
            assert_eq!(result.0, 3);
            assert_eq!(result.1["verdict"], "invalid");
            assert_eq!(result.1["diagnostics"].as_array().unwrap().len(), 2);
            "E-CONF-0: a scenario selector is required\n"
        } else {
            assert_eq!(result.0, 4);
            assert_eq!(result.1["verdict"], "unsupported");
            assert_eq!(result.1["diagnostics"].as_array().unwrap().len(), 1);
            ""
        };
        assert_eq!(
            result.2,
            format!(
                "{prefix}U-SEM-0: {message}\n  at 1:1\n  kind: analysis_not_connected\n  cause: merge\n  blocked by: merge_binding v at 5:28\n  blocked by: feature at 7:17\n  blocked by: feature at 8:19\n  see: {SEE}\n"
            )
        );
    }
}

#[test]
fn contest_rates_effect_target() {
    let reason = "contest_rates_effect_target";
    let control = r#"model Contest version "0.2" {
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
    let source = control.replace("by 5min\n", "by 5min\n      rates { occurrence probability 1 basis \"x\"; service_time basis \"x\"; independence per_case_per_clause }\n");
    rejected(
        invoke("check", &source, &[]),
        reason,
        reason,
        "commit_target",
        location(12, 5, 13, 109),
        vec![
            row("reference", Some("P.post"), 12, 23, 36),
            row("feature", None, 13, 7, 109),
        ],
        "  at 12:5\n  kind: analysis_not_connected\n  cause: commit_target\n  blocked by: reference P.post at 12:23\n  blocked by: feature at 13:7\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    accepts("check", control, &[], reason);
}
