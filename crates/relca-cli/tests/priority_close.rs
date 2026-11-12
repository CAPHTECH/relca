//! Q5 closing acceptance; independent of the six frozen original artifacts.
use relca_core::{Verdict, report, syntax};
use serde_json::Value;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
#[path = "support/ledger_shape.rs"]
mod e9b_wire;

static NEXT: AtomicU64 = AtomicU64::new(0);
const MODES: &str = r#"model Modes version "0.2" {
type T = enum { ok }
actor A { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } }
process P { r = A.run(); end done }
cut C { include [A] }
bounds { cases 2; live_tasks 2; clock 10min }
arrivals fixed [0min,0min] case_priority fixed [0,9]
policy observable priority descending tie fifo tie_by case_id
simulation replications 2 seed 1 confidence 0.5
metrics { window 10min; currency "JPY" }
verify safety no_resource_overcommit
verify probability reached end.done >= 0
}"#;
fn append(source: &str, item: &str) -> String {
    let at = source.rfind('}').unwrap();
    format!("{}\n{item}\n}}", &source[..at])
}
fn plans(different: bool) -> String {
    let other = if different { "B" } else { "A" };
    let base = if different {
        MODES.replace("cut C { include [A] }", "actor B { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } } cut C { include [A,B] }")
    } else {
        MODES.into()
    };
    append(
        &base,
        &format!(
            "process Q {{ r = {other}.run(); end done }} configuration Left {{ cut C; process P }} configuration Right {{ cut C; process Q }}"
        ),
    )
}
fn errors(v: &Value) -> Vec<&Value> {
    v["diagnostics"]
        .as_array()
        .map(|ds| ds.iter().filter(|d| d["severity"] == "error").collect())
        .unwrap_or_default()
}
fn cli(source: &str, command: &str, args: &[&str]) -> (Value, i32, String) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = root.join(".tmp");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!(
        "q5-close-{}-{}.relca",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .current_dir(&root)
        .arg(command)
        .arg(&path)
        .args(args)
        .output()
        .unwrap();
    (
        serde_json::from_slice(&out.stdout).unwrap(),
        out.status.code().unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn m1_legacy_priority_forms_preserve_reason_span_and_real_cli_exit() {
    for direction in ["", " ascending"] {
        for (item, word, message) in [
            (
                format!("policy priority{direction}"),
                "priority",
                "unsupported syntax: priority",
            ),
            (
                format!("concurrent cases 1 on_overflow wait_external priority{direction}"),
                "priority",
                "external wait requires fifo order",
            ),
            (
                format!(
                    "actor A {{ capability c() -> R {{ version_check [x] else conflict resolve priority{direction} }} }}"
                ),
                "resolve",
                "unsupported syntax: resolve",
            ),
        ] {
            let source = format!("model M version \"0.2\" {{ {item} }}");
            let ds = syntax::parse(&source).unwrap_err();
            assert_eq!(ds[0].code, "U-SYN-0", "{source}: {ds:?}");
            assert_eq!(ds[0].message, message, "{source}");
            let span = ds[0].span.unwrap();
            assert_eq!(&source[span.start..span.end], word, "{source}");
            assert_eq!(report::check(&source).verdict, Verdict::Unsupported);
            let (wire, exit, stderr) = cli(&source, "check", &[]);
            assert_eq!(exit, 4, "{source}: {stderr}");
            assert!(
                wire["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "U-SYN-0"
                        && d["message"] == message
                        && d["span"]["start"] == span.start)
            );
        }
    }
}

#[test]
fn m3_refusal_empty_cases_require_a_parser_entered_priority_declaration() {
    for item in [
        "process policy priority descending {}",
        "observed case { case_priority fixed [9] }",
        "actor A { capability c() -> R { reads [policy priority descending] } }",
        "unknown policy priority descending",
    ] {
        let source = format!("model M version \"0.2\" {{ {item} }}");
        assert!(syntax::parse(&source).is_err());
        let wire = serde_json::to_value(report::check(&source)).unwrap();
        assert!(
            wire["case_outcomes"].is_null(),
            "lexical lookalike is not a declaration: {source}: {}",
            wire["case_outcomes"]
        );
    }
    for item in [
        "case_priority by secret",
        "case_priority fixed [9] unknown",
        "policy priority descending tie nondet tie fifo tie_by case_id",
        "concurrent cases 1 on_overflow wait_external priority descending then call_order",
        "actor A { capability c() -> R { version_check [x] else conflict resolve priority descending then call_order } }",
    ] {
        let source = format!("model M version \"0.2\" {{ {item} }}");
        let wire = serde_json::to_value(report::check(&source)).unwrap();
        assert_eq!(
            wire["case_outcomes"],
            serde_json::json!([]),
            "recognized declaration must retain the no-execution evidence: {source}"
        );
    }
}

#[test]
fn m4_different_priority_scopes_are_refused_in_both_comparison_modes() {
    let source = plans(true);
    for (command, args) in [
        (
            "compare",
            vec!["--plans", "Left,Right", "--mode", "exhaustive"],
        ),
        (
            "compare",
            vec!["--plans", "Left,Right", "--mode", "simulation"],
        ),
        ("diff", vec!["--from", "Left", "--to", "Right"]),
    ] {
        let (v, exit, _) = cli(&source, command, &args);
        assert_eq!(exit, 4, "{command}: {}", v["diagnostics"]);
        assert!(
            errors(&v).iter().any(|d| d["code"] == "U-SEM-0"
                && d["message"]
                    .as_str()
                    .unwrap_or("")
                    .contains("priority_feature_combination")),
            "{command}: {}",
            v["diagnostics"]
        );
    }
}

#[test]
fn m4_verify_retains_exit_but_names_the_unconnected_priority_backend() {
    let (v, exit, _) = cli(MODES, "verify", &[]);
    assert_eq!(exit, 4);
    assert!(errors(&v).iter().any(|d| {
        d["message"]
            .as_str()
            .unwrap_or("")
            .contains("priority_feature_combination")
    }));
}

#[test]
fn m4_connected_modes_preserve_priority_and_hand_case_order() {
    for command in ["check", "explain", "simulate"] {
        let (v, exit, _) = cli(MODES, command, &[]);
        assert!(matches!(exit, 0 | 2), "{command}: {}", v["diagnostics"]);
        assert!(errors(&v).is_empty());
        assert_eq!(
            v["priority_policy"]["case_ranks"],
            serde_json::json!([0, 9])
        );
        assert_eq!(v["case_outcomes"].as_array().unwrap().len(), 2);
    }
    let equal = plans(false);
    for mode in ["exhaustive", "simulation"] {
        let (v, exit, _) = cli(
            &equal,
            "compare",
            &["--plans", "Left,Right", "--mode", mode],
        );
        assert!(matches!(exit, 0 | 2), "{mode}: {}", v["diagnostics"]);
        assert!(errors(&v).is_empty());
        assert_eq!(
            v["plans"][0]["report"]["priority_policy"],
            v["plans"][1]["report"]["priority_policy"]
        );
        if mode == "exhaustive" {
            assert_eq!(
                v["plans"][0]["report"]["case_outcomes"][0]["terminals"][0]["completed_at"]["min"],
                2
            );
            assert_eq!(
                v["plans"][0]["report"]["case_outcomes"][1]["terminals"][0]["completed_at"]["max"],
                1
            );
        }
    }
    let (v, exit, _) = cli(&equal, "diff", &["--from", "Left", "--to", "Right"]);
    assert_eq!(exit, 0, "{}", v["diagnostics"]);
    assert!(errors(&v).is_empty());
}

#[test]
fn m4_contest_and_responsibility_keep_their_connected_narrow_guarantees() {
    let contest = append(
        include_str!("../../../fixtures/ctx-contest/model.relca"),
        "case_priority fixed [9] policy observable priority descending\nsimulation replications 2 seed 1 confidence 0.5",
    );
    let (v, exit, _) = cli(&contest, "check", &[]);
    assert_eq!(exit, 0);
    let filed = v["contest_runtime"][0]["alternatives"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["filed"] == true)
        .unwrap();
    assert_eq!(filed["response_at"], 3);
    assert_eq!(filed["status"], "responded");
    let (v, exit, _) = cli(&contest, "simulate", &[]);
    assert_eq!(exit, 4);
    assert!(errors(&v).iter().any(|d| {
        d["message"]
            .as_str()
            .unwrap_or("")
            .contains("nondeterminism")
    }));
    let duty = append(
        include_str!("../../../fixtures/resp2-within/model.relca"),
        "case_priority fixed [9] policy observable priority descending",
    );
    let (v, exit, _) = cli(&duty, "check", &[]);
    assert_eq!(exit, 0);
    assert_eq!(
        v["responsibilities"][0]["time_frame"]["basis"],
        "static_capability_path_upper"
    );
    assert_eq!(
        v["responsibilities"][0]["time_frame"]["required_duration"],
        5
    );
}

#[test]
fn m4_mutation_retains_priority_in_an_executed_mutant() {
    let source = append(
        &include_str!("../../../fixtures/mut-guard/model.relca").replace("cases 1;", "cases 2;"),
        "arrivals fixed [0min,0min] case_priority fixed [0,9] policy observable priority descending tie fifo tie_by case_id",
    );
    let (v, _, _) = cli(&source, "mutate", &[]);
    let mutant = v["mutations"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|r| r["mutants"].as_array().unwrap())
        .find(|m| m["operator"] == "bypass_guard" && m["report"]["verdict"] == "violated")
        .unwrap();
    assert_eq!(
        mutant["report"]["priority_policy"],
        v["original"]["priority_policy"]
    );
    let cases = mutant["report"]["case_outcomes"].as_array().unwrap();
    let earliest = |row: &Value| {
        row["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["completed_at"]["min"].as_u64())
            .min()
            .unwrap()
    };
    assert!(
        earliest(&cases[1]) < earliest(&cases[0]),
        "high case must still be scheduled first"
    );
}

#[test]
fn m4_conform_lists_each_priority_rule_outside_its_scoped_guarantee() {
    let source = append(
        include_str!("snapshots/inputs/q3-conform-model.relca"),
        "case_priority fixed [0,9] policy observable priority descending tie fifo tie_by case_id",
    );
    let (v, exit, _) = cli(
        &source,
        "conform",
        &[
            "--log",
            "crates/relca-cli/tests/snapshots/inputs/q3-conform-ok.jsonl",
            "--scope",
            "event-contracts",
        ],
    );
    assert_eq!(exit, 0);
    assert_eq!(v["scope"], "observable_event_contracts_v1");
    for item in [
        "priority_starts",
        "priority_external_readmission",
        "priority_version_resolution",
    ] {
        assert!(
            v["not_checked"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["item"] == item
                    && r["reason"] == "priority_order_not_in_event_contract_scope"),
            "missing {item}"
        );
    }
    assert!(
        v["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| !c["id"].as_str().unwrap().contains("priority"))
    );
    assert!(!v["summary"].to_string().contains("priority"));
    assert!(v.get("priority_conformance").is_none());
    let approval = append(
        include_str!("snapshots/inputs/q8-approval.relca"),
        "case_priority fixed [0,5,9] policy observable priority descending",
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let spec_path = root
        .join(".tmp")
        .join(format!("q5-close-approval-{}.json", std::process::id()));
    let mut spec: Value =
        serde_json::from_str(include_str!("snapshots/inputs/q8-approval-baseline.json")).unwrap();
    spec["model_sha256"] = serde_json::json!(report::check(&approval).model.sha256.unwrap());
    std::fs::write(&spec_path, serde_json::to_vec(&spec).unwrap()).unwrap();
    let (v, exit, _) = cli(
        &approval,
        "conform",
        &[
            "--log",
            "crates/relca-cli/tests/snapshots/inputs/q8-approval-baseline.jsonl",
            "--scope",
            "approval-burden",
            "--approval-spec",
            spec_path.to_str().unwrap(),
        ],
    );
    assert_eq!(exit, 0, "{}", v["diagnostics"]);
    assert_eq!(
        v["scope"],
        "observable_event_contracts_with_approval_burden_v1"
    );
    for item in [
        "priority_starts",
        "priority_external_readmission",
        "priority_version_resolution",
    ] {
        assert!(
            v["not_checked"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["item"] == item)
        );
    }
    assert!(!v["summary"].to_string().contains("priority"));
}

fn without_priority_declaration_metadata(mut value: Value) -> Value {
    e9b_wire::uncertainty_wire::validate_host(&value).unwrap();
    // E9b is later metadata, not one of M5's old runtime fields. Its priority
    // link and classifications are asserted separately on the unprojected data.
    value.as_object_mut().unwrap().remove("uncertainty");
    value.as_object_mut().unwrap().remove("priority_policy");
    value["model"].as_object_mut().unwrap().remove("sha256");
    value["assumptions"]
        .as_array_mut()
        .unwrap()
        .retain(|a| !a["field"].as_str().unwrap().starts_with("case_priority"));
    fn references(v: &mut Value) {
        match v {
            Value::Array(rows) => {
                rows.retain(|row| {
                    !["name", "at", "target", "value"].iter().any(|key| {
                        row[*key]
                            .as_str()
                            .is_some_and(|s| s.starts_with("/priority_policy"))
                    })
                });
                for row in rows {
                    references(row);
                }
            }
            Value::Object(fields) => {
                for item in fields.values_mut() {
                    references(item);
                }
            }
            _ => {}
        }
    }
    references(&mut value["ledger"]);
    value
}

#[test]
fn m5_unused_single_case_table_keeps_legacy_route_and_all_old_output_fields() {
    let plain = r#"model Single version "0.2" {
        type R = enum { ok } actor A { capacity 1; capability run() -> R { duration fixed 1min; result nondet { ok } } }
        process P { r = A.run(); end done } cut C { include [A] }
        bounds { cases 1; live_tasks 2; clock 10min }
        metrics { window 10min; currency "JPY" }
        verify safety no_resource_overcommit }"#;
    let declared = append(plain, "case_priority fixed [9]");
    let a = relca_core::lower::lower(&syntax::parse(plain).unwrap()).unwrap();
    let b = relca_core::lower::lower(&syntax::parse(&declared).unwrap()).unwrap();
    assert!(a.legacy_route());
    assert!(
        b.legacy_route(),
        "unused declaration is not an execution feature"
    );
    let plain_report = report::check(plain);
    let declared_report = report::check(&declared);
    assert_eq!(
        declared_report.verdict.exit_code(),
        plain_report.verdict.exit_code()
    );
    let declared_wire = serde_json::to_value(declared_report).unwrap();
    let plain_wire = serde_json::to_value(plain_report).unwrap();
    for category in ["aleatory", "estimation"] {
        assert_eq!(
            declared_wire["uncertainty"][category],
            plain_wire["uncertainty"][category]
        );
    }
    let linked = |v: &Value| {
        v["uncertainty"]["model"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["at"] == "/priority_policy")
    };
    assert!(linked(&declared_wire));
    assert!(!linked(&plain_wire));
    assert_eq!(
        without_priority_declaration_metadata(declared_wire),
        without_priority_declaration_metadata(plain_wire)
    );
    assert_eq!(
        relca_core::reference::explore(&a).stop,
        relca_core::reference::StopReason::Complete
    );
    assert_eq!(
        relca_core::reference::explore(&b).stop,
        relca_core::reference::StopReason::Complete
    );
}

#[test]
fn m6_duplicate_priority_tie_uses_a_semantic_reason_in_every_queue_scope() {
    for item in [
        "policy priority descending tie nondet tie fifo tie_by case_id",
        "scenario S { policy priority descending tie nondet tie fifo tie_by case_id }",
        "queue capacity 1 on_overflow wait_external priority descending tie nondet tie fifo tie_by case_id",
        "scenario S { concurrent cases 1 on_overflow wait_external priority descending tie nondet tie fifo tie_by case_id }",
    ] {
        let ds = syntax::parse(&format!("model M version \"0.2\" {{ {item} }}")).unwrap_err();
        assert_eq!(ds[0].code, "U-SEM-0");
        assert_eq!(ds[0].message, "duplicate priority tie", "{item}");
    }
}

fn nondeterminism_reason(source: &str, priority: bool) {
    let m = relca_core::lower::lower(&syntax::parse(source).unwrap()).unwrap();
    let e = relca_core::engine::simulate(&m);
    let r = relca_core::reference::simulate(&m);
    for (which, out) in [("engine", &e), ("reference", &r.outcome)] {
        assert_eq!(out.completed_replications, 0, "{which}");
        let d = out
            .diagnostics
            .iter()
            .find(|d| d.code == "U-SEM-0")
            .unwrap();
        if priority {
            assert!(
                d.message.contains("unresolved simulation nondeterminism")
                    && d.message.contains("priority_tie_remaining"),
                "{which}: {}",
                d.message
            );
        } else {
            assert!(
                d.message
                    .starts_with("simulation encountered unresolved nondeterminism"),
                "{which}: {}",
                d.message
            );
            assert!(!d.message.contains("priority_tie_remaining"));
        }
        assert!(
            out.replications
                .iter()
                .flat_map(|r| &r.draws)
                .all(|d| !d.key.contains("priority"))
        );
    }
    if !m.uses_priority() {
        assert_eq!(r.stop, relca_core::reference::StopReason::Complete);
    }
}

#[test]
fn m6_simulation_distinguishes_priority_ties_from_other_nondeterminism() {
    let start = MODES.replace("fixed [0,9]", "fixed [5,5]").replace(
        "priority descending tie fifo tie_by case_id",
        "priority descending",
    );
    nondeterminism_reason(&start, true);
    let external = append(
        &MODES
            .replace("cases 2;", "cases 3;")
            .replace("fixed [0min,0min]", "fixed [0min,1min,2min]")
            .replace("fixed [0,9]", "fixed [0,5,5]")
            .replace("duration fixed 1min", "duration fixed 4min"),
        "concurrent cases 1 on_overflow wait_external priority descending",
    );
    nondeterminism_reason(&external, true);
    let finish = append(
        &include_str!("snapshots/inputs/q5-conflict.relca").replace("fixed [0, 9]", "fixed [5, 5]"),
        "simulation replications 2 seed 1 confidence 0.5",
    );
    nondeterminism_reason(&finish, true);
    let other = MODES
        .replace("enum { ok }", "enum { ok, other }")
        .replace("result nondet { ok }", "result nondet { ok, other }")
        .replace(
            "r = A.run(); end done",
            "r = A.run(); if r == ok { end done } else { end other }",
        );
    nondeterminism_reason(&other, false);
    let unused = r#"model Unused version "0.2" { type R = enum { ok, other }
        actor A { capacity 1; capability run() -> R { duration fixed 1min; result nondet { ok, other } } }
        process P { r = A.run(); if r == ok { end done } else { end other } }
        bounds { cases 1; live_tasks 2; clock 10min } case_priority fixed [9]
        simulation replications 2 seed 1 confidence 0.5
        verify probability reached end.done >= 0 }"#;
    nondeterminism_reason(unused, false);
}
