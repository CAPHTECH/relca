//! Cross-mode guards, not independent semantic acceptance.
#[path = "../../relca-core/tests/support/q9_inputs.rs"]
mod contest_input;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn base() -> String {
    r#"model Guard version "0.2" {
      type R = enum { ok }
      people [p]
      actor Desk { control policy; executors [p]; capability run() -> R {
        duration fixed 2min; result nondet { ok }
      } }
      process Flow { r = Desk.run(); end done }
      cut C { include [Desk] }
      permit p to Desk.run
      contract Control { permitted Desk.run }
      arrivals fixed [0min]
      policy observable fifo tie_by case_id then call_order assign first_idle
      bounds { cases 1; live_tasks 4; clock 20min }
      simulation replications 16 seed 1 confidence 0.5
      verify reachable end.done
    }"#
    .into()
}
fn feature(source: &str, performance: bool, person: &str, marker: &str, result: &str) -> String {
    if performance {
        let insert = format!(
            "kernel K {{ * -> {{ 1: {result} }} }}\nperformance P basis \"test assumption\" {{ load none; executor {person} {{ kernel K }} }}\n"
        );
        let at = source.rfind('}').unwrap();
        let with_profile = format!("{}\n{insert}}}", &source[..at]);
        with_profile.replacen(marker, "outcomes performance P", 1)
    } else {
        source.replacen(
            marker,
            &format!("qualified_executors [{person}]; {marker}"),
            1,
        )
    }
}
fn invoke(name: &str, source: &str, mode: &str) -> (Value, i32) {
    let dir = root()
        .join(".tmp/q6-unconnected")
        .join(format!("{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.relca");
    std::fs::write(&path, source).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_relca"));
    cmd.current_dir(root()).arg(mode).arg(path);
    if mode == "conform" {
        let log = dir.join("events.jsonl");
        let rows = [
            json!({"kind":"header","schema":"relca-log/v1","clock":"shared","time_unit":"min",
                "window":{"from":0,"to":20},"cases":[{"case_id":"c","from_case_start":true,"through_case_end":true}],
                "actor_map":[{"source_id":"desk","actor":"Desk"}],"executor_map":[{"source_id":"person","person":"p"}]}),
            json!({"kind":"event","event_id":"start","case_id":"c","task_id":"r","operation":"Desk.run","lifecycle":"start",
                "cancel_phase":null,"time":{"earliest":0,"latest":0},"after":[],"actor_id":"desk","executor_id":"person",
                "input_version":"v1","output_version":null,"result":null}),
            json!({"kind":"event","event_id":"finish","case_id":"c","task_id":"r","operation":"Desk.run","lifecycle":"finish",
                "cancel_phase":null,"time":{"earliest":2,"latest":2},"after":["start"],"actor_id":"desk","executor_id":"person",
                "input_version":"v1","output_version":"v1","result":"ok"}),
        ];
        std::fs::write(
            &log,
            rows.iter().map(|v| format!("{v}\n")).collect::<String>(),
        )
        .unwrap();
        cmd.arg("--log")
            .arg(log)
            .args(["--scope", "event-contracts"]);
    }
    let output = cmd.output().unwrap();
    let v: Value = serde_json::from_slice(&output.stdout).unwrap();
    let stderr: String = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            format!(
                "{}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(String::from_utf8(output.stderr).unwrap(), stderr);
    (v, output.status.code().unwrap())
}
fn rejected(name: &str, source: &str, mode: &str) {
    let (v, exit) = invoke(name, source, mode);
    assert_eq!(exit, 4, "{name}: {v}");
    assert_eq!(v["verdict"], "unsupported");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SEM-0"
                && d["message"]
                    .as_str()
                    .unwrap()
                    .contains("q6_feature_combination")),
        "{name}: {v}"
    );
}
#[test]
fn conform_refuses_qualification_and_performance_not_old_model() {
    let source = base()
        .replace("verify reachable end.done", "")
        .replace(
            "contract Control { permitted Desk.run }",
            "contract Control { at_most_once Desk.run.commit per case.id }",
        )
        .replace("duration fixed 2min;", "commits; duration fixed 2min;");
    let (v, exit) = invoke("old-conform", &source, "conform");
    assert_eq!((exit, &v["verdict"]), (0, &json!("satisfied")), "{v}");
    for perf in [false, true] {
        rejected(
            &format!("conform-{perf}"),
            &feature(&source, perf, "p", "result nondet { ok }", "ok"),
            "conform",
        );
    }
}
#[test]
fn contest_refuses_qualification_and_performance_not_old_model() {
    let source = contest_input::source("fixed");
    let (v, exit) = invoke("old-contest", &source, "check");
    assert_eq!((exit, &v["verdict"]), (0, &json!("satisfied")), "{v}");
    for perf in [false, true] {
        rejected(
            &format!("contest-{perf}"),
            &feature(&source, perf, "r", "result nondet { pass }", "pass"),
            "check",
        );
    }
}
#[test]
fn stochastic_arrivals_refuse_qualification_and_performance_not_old_model() {
    let source = base().replace(
        "arrivals fixed [0min]",
        "arrivals distribution basis \"test assumption\" { 1: fixed [0min] }",
    );
    let (v, exit) = invoke("old-arrivals", &source, "simulate");
    // The permitted contract is safety: a sample cannot prove it.
    assert_eq!((exit, &v["verdict"]), (2, &json!("inconclusive")), "{v}");
    for perf in [false, true] {
        rejected(
            &format!("arrivals-{perf}"),
            &feature(&source, perf, "p", "result nondet { ok }", "ok"),
            "simulate",
        );
    }
}
#[test]
fn retry_refuses_qualification_and_performance_not_old_model() {
    let source = base().replace("r = Desk.run();", "r = Desk.run() response_loss sequence [delivered, delivered] retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed;")
        .replace("policy observable fifo tie_by case_id then call_order assign first_idle", "");
    let (v, exit) = invoke("old-retry", &source, "check");
    assert_eq!((exit, &v["verdict"]), (0, &json!("satisfied")), "{v}");
    for perf in [false, true] {
        rejected(
            &format!("retry-{perf}"),
            &feature(&source, perf, "p", "result nondet { ok }", "ok"),
            "check",
        );
    }
}
