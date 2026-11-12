//! Public CLI cross-mode gates and old-If positive controls, not private calls.
#[path = "../../relca-core/tests/support/q6b_inputs.rs"]
mod input;
#[path = "../../relca-core/tests/support/q6b_review_inputs.rs"]
mod review;
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

fn dir() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.tmp")
        .join(format!(
            "q6bt2-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir_all(&p).unwrap();
    p
}
fn diagnostics(v: &Value, prefix: &str) -> String {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            format!(
                "{prefix}{}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            )
        })
        .collect()
}
fn run(mode: &str, degrade: bool) -> (Value, i32) {
    // A fresh directory for every invocation, including each control.
    let d = dir();
    let model = d.join("model.relca");
    let src = review::modes(degrade);
    // Conform does not claim probabilistic verification (ADR 0057). Its pair
    // uses only the same observable requires_event contract; both sides drop
    // the probability query, never a runtime or degradation declaration.
    let src = if mode == "conform" {
        input::edit(&src, "verify probability reached end.done >= 1", "")
    } else {
        src
    };
    fs::write(&model, src).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_relca"));
    cmd.arg(mode);
    if mode == "sensitivity" {
        let manifest = json!({"schema":"relca-sensitivity/v1","scope":{"kind":"enumerated_models","rationale":"Two identical explicit points test only the degrade gate"},
            "points":[{"id":"a","model":"model.relca","configuration":null,"scenario":null},{"id":"b","model":"model.relca","configuration":null,"scenario":null}],
            "targets":[{"kind":"probability_query","id":"probability reached #1"}],"confidence":null});
        let path = d.join("manifest.json");
        fs::write(&path, manifest.to_string()).unwrap();
        cmd.arg(path).args(["--mode", "exhaustive"]);
    } else {
        cmd.arg(model);
        if mode == "conform" {
            let log = d.join("events.jsonl");
            let mut rows = vec![
                json!({"kind":"header","schema":"relca-log/v1","clock":"fixture-clock","time_unit":"min","window":{"from":0,"to":10},
                "cases":[{"case_id":"c","from_case_start":true,"through_case_end":true}],
                "actor_map":[{"source_id":"probe","actor":"Probe"},{"source_id":"fast","actor":"Fast"}],
                "executor_map":[{"source_id":"p","person":"p"},{"source_id":"f","person":"f"}]}),
            ];
            for (task, op, actor, person, start) in [
                ("status", "Probe.check", "probe", "p", 0),
                ("f", "Fast.run", "fast", "f", 1),
            ] {
                for (life, at) in [("start", start), ("finish", start + 1)] {
                    rows.push(json!({"kind":"event","event_id":format!("{task}-{life}"),"case_id":"c","task_id":task,"operation":op,"lifecycle":life,"cancel_phase":null,
                        "time":{"earliest":at,"latest":at},"after":if task=="f" && life=="start" {json!(["status-finish"])} else {json!([])},"actor_id":actor,"executor_id":person,"input_version":"input-v1",
                        "output_version":if life=="finish" {json!("output-v1")} else {Value::Null},"result":if life=="finish" {json!("ok")} else {Value::Null}}));
                }
            }
            fs::write(
                &log,
                rows.iter().map(|r| format!("{r}\n")).collect::<String>(),
            )
            .unwrap();
            cmd.arg("--log")
                .arg(log)
                .args(["--scope", "event-contracts"]);
        }
    }
    let out = cmd
        .env("CARGO_NET_OFFLINE", "true")
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    assert_eq!(out.stdout.iter().filter(|b| **b == b'\n').count(), 1);
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut stderr = String::new();
    if mode == "mutate" {
        if !v["original"].is_null() {
            stderr.push_str(&diagnostics(&v["original"], "original: "));
        }
        for row in v["mutations"].as_array().unwrap() {
            for m in row["mutants"].as_array().unwrap() {
                if !m["report"].is_null() {
                    stderr.push_str(&diagnostics(
                        &m["report"],
                        &format!("{}: ", m["id"].as_str().unwrap()),
                    ));
                }
            }
        }
        stderr.push_str(&diagnostics(&v, "mutate: "));
    } else {
        if mode == "sensitivity" {
            for p in v["points"].as_array().unwrap() {
                if p["status"] == "evaluated" && !p["report"].is_null() {
                    stderr.push_str(&diagnostics(
                        &p["report"],
                        &format!("point {}: ", p["id"].as_str().unwrap()),
                    ));
                }
            }
        }
        stderr.push_str(&diagnostics(
            &v,
            if mode == "sensitivity" {
                "sensitivity: "
            } else {
                ""
            },
        ));
    }
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        stderr,
        "stderr must mirror diagnostics with public prefixes"
    );
    (v, out.status.code().unwrap())
}
fn refused(mode: &str) {
    let (v, exit) = run(mode, true);
    assert_eq!(exit, 4, "{v}");
    assert_eq!(v["verdict"], "unsupported");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SEM-0"
                && (d["reason"] == "q6_feature_combination"
                    || d["message"]
                        .as_str()
                        .unwrap()
                        .contains("q6_feature_combination"))),
        "specific Q6 reason, not incidental unsupported: {v}"
    );
}
#[test]
fn m5_mutate_degrade_is_specifically_unsupported() {
    refused("mutate");
}
#[test]
fn m5_conform_degrade_is_specifically_unsupported() {
    refused("conform");
}
#[test]
fn m5_sensitivity_degrade_is_specifically_unsupported() {
    refused("sensitivity");
}
#[test]
fn m5_old_if_mutate_keeps_original_and_enumerates_sites() {
    let (v, exit) = run("mutate", false);
    assert_eq!(
        exit, 4,
        "legacy incomplete default set is still unsupported"
    );
    assert_eq!(v["original_verdict"], "satisfied");
    assert!(!v.to_string().contains("q6_feature_combination"));
    assert!(
        v["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["status"] == "generated"),
        "old If guard is a mutation site: {v}"
    );
}
#[test]
fn m5_old_if_conform_accepts_complete_observed_tasks() {
    let (v, exit) = run("conform", false);
    assert_eq!(exit, 0, "{}", v["diagnostics"]);
    assert_eq!(v["conformance"], "conformant");
    assert_eq!(v["verdict"], "satisfied");
    assert!(!v.to_string().contains("q6_feature_combination"));
}
#[test]
fn m5_old_if_sensitivity_keeps_exact_probability_one() {
    let (v, exit) = run("sensitivity", false);
    assert_eq!(exit, 0, "{v}");
    assert_eq!(v["verdict"], "satisfied");
    assert!(!v.to_string().contains("q6_feature_combination"));
    let points = v["points"].as_array().unwrap();
    assert_eq!(points.len(), 2);
    for p in points {
        let query = p["report"]["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|q| q["id"] == "probability reached #1")
            .unwrap();
        assert_eq!(query["probability"], json!({"min":"1","max":"1"}));
    }
}
