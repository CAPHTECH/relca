//! ADR 0069 stage 1: each interpreter independently faces the hand oracle.
//! MERGE_IF_WITNESSES selects handwritten old-DSL witnesses for calibration only.
use relca_core::{Verdict, engine, ir, lower, outcome, reference, syntax};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::PathBuf};

fn source(name: &str, witness: bool) -> String {
    let directory = if witness { "witnesses" } else { "models" };
    fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/merge_if")
            .join(directory)
            .join(format!("{name}.relca")),
    )
    .unwrap()
}

fn resolve(src: &str) -> ir::Model {
    let ast = syntax::parse(src).unwrap_or_else(|d| panic!("parser RED: {d:?}"));
    lower::lower(&ast).unwrap_or_else(|d| panic!("lower RED: {d:?}"))
}

fn range(lo: u64, hi: u64) -> Value {
    json!({"min":lo,"max":hi})
}

fn terminal(end: &str, lo: u64, hi: u64, p: &str, q: &str) -> Value {
    json!({"end":end,"elapsed":range(lo,hi),"work":range(lo,hi),
        "probability":{"min":p,"max":q},"result_at":null})
}

fn hand_terminals(name: &str) -> Vec<Value> {
    match name {
        "M1" | "M5" => vec![
            terminal("done_fixed", 5, 5, "1/4", "1/4"),
            terminal("done_as_is", 3, 3, "3/4", "3/4"),
        ],
        "M2" => vec![
            terminal("rejected", 2, 2, "1/4", "1/4"),
            terminal("done_fixed", 5, 5, "1/4", "1/4"),
            terminal("done_as_is", 3, 3, "1/2", "1/2"),
        ],
        "M3" => vec![
            terminal("done", 4, 6, "0", "1"),
            terminal("held", 3, 5, "0", "1"),
        ],
        "M4" => vec![
            terminal("done", 3, 5, "19/20", "19/20"),
            terminal("flagged", 3, 3, "1/20", "1/20"),
        ],
        _ => panic!("unknown hand case {name}"),
    }
}

fn by_end(rows: Vec<Value>) -> BTreeMap<String, Value> {
    let n = rows.len();
    let map: BTreeMap<_, _> = rows
        .into_iter()
        .map(|row| (row["end"].as_str().unwrap().to_owned(), row))
        .collect();
    assert_eq!(n, map.len(), "no duplicated terminal rows");
    map
}

fn metric(v: &Value, lo: &str, hi: &str) {
    assert_eq!(v, &json!({"min":lo,"max":hi,"reason":null}));
}

fn hand_metrics(name: &str, metrics: &Value, witness: bool) {
    assert_eq!(metrics["status"], "complete");
    let cases = metrics["cases"].as_array().unwrap();
    assert_eq!(cases.len(), if name == "M5" { 2 } else { 1 });
    for case in cases {
        let (work_lo, work_hi, cost_lo, cost_hi, fixes, sends) = match name {
            "M1" | "M5" => ("3", "5", "6", "9", "0", Some("1")),
            "M2" => ("2", "5", "4", "9", "0", Some("0")),
            "M3" => ("3", "6", "6", "11", "0", None),
            "M4" => ("3", "5", "6", "9", "0", None),
            _ => unreachable!(),
        };
        metric(&case["work"], work_lo, work_hi);
        metric(&case["observed_cost_total"], cost_lo, cost_hi);
        let tasks = case["tasks"].as_array().unwrap();
        let expected: &[&str] = match (name, witness) {
            ("M3", true) => &["d", "r", "f", "a1", "a2", "s1", "s2"],
            ("M3", false) => &["d", "r", "f", "a", "s"],
            ("M4", true) => &["d", "r", "f", "a1", "a2"],
            ("M4", false) => &["d", "r", "f", "a"],
            (_, true) => &["d", "r", "f", "s1", "s2"],
            (_, false) => &["d", "r", "f", "s"],
        };
        let mut found: Vec<_> = tasks
            .iter()
            .map(|t| t["binding"].as_str().unwrap())
            .collect();
        let mut expected = expected.to_vec();
        found.sort_unstable();
        expected.sort_unstable();
        assert_eq!(found, expected, "copy is not a Task; suffix is one site");
        let fix = tasks.iter().find(|t| t["binding"] == "f").unwrap();
        metric(&fix["starts"], fixes, "1");
        if let Some(sends) = sends {
            if witness {
                let s1 = tasks.iter().find(|t| t["binding"] == "s1").unwrap();
                let s2 = tasks.iter().find(|t| t["binding"] == "s2").unwrap();
                metric(&s1["starts"], "0", "1");
                metric(&s2["starts"], "0", "1");
            } else {
                let send = tasks.iter().find(|t| t["binding"] == "s").unwrap();
                metric(&send["starts"], sends, "1");
            }
        }
        for t in tasks {
            metric(&t["wait"], "0", "0");
        }
        // M3 deliberately has unresolved audit nondeterminism, not a fake mean.
        if name != "M3" {
            for row in case["terminals"].as_array().unwrap() {
                if row["count"]["stats"]["max"] == "0" {
                    continue;
                }
                let b = &row["breakdown"];
                assert_eq!(b["in_service"], row["lead_time"]);
                for field in ["external", "intentional", "unattributed", "contest_route"] {
                    metric(&b[field], "0", "0");
                }
                metric(&b["queue"]["combined"], "0", "0");
            }
        }
    }
}

fn hand(name: &str, model: &ir::Model, out: &outcome::Outcome, witness: bool) {
    assert!(!out.truncated, "no computational cut in {name}");
    assert!(
        out.diagnostics
            .iter()
            .all(|d| !d.code.starts_with(['U', 'E'])),
        "{:?}",
        out.diagnostics
    );
    let expected = by_end(hand_terminals(name));
    if name == "M5" {
        let cases = out.case_outcomes.as_ref().unwrap();
        assert_eq!(cases.len(), 2);
        for (i, case) in cases.iter().enumerate() {
            assert_eq!(case.case_id.0, i);
            assert_eq!(case.arrival, i as u64 * 10);
            let rows = case
                .terminals
                .iter()
                .map(|t| {
                    let outcome::CaseTerminalKind::End(end) = t.kind else {
                        panic!("unexpected non-end: {t:?}")
                    };
                    let elapsed = t.elapsed.unwrap();
                    let at = t.completed_at.unwrap();
                    assert_eq!(at.min, elapsed.min + case.arrival);
                    assert_eq!(at.max, elapsed.max + case.arrival);
                    json!({"end":model.ends[end.0],"elapsed":range(elapsed.min,elapsed.max),
                    "work":range(t.work.unwrap().min,t.work.unwrap().max),"result_at":null,
                    "probability":t.probability})
                })
                .collect();
            assert_eq!(by_end(rows), expected, "per-case oracle {name}/{i}");
        }
    } else {
        let rows = out
            .terminals
            .iter()
            .map(|t| {
                json!({
                    "end":model.ends[t.end.0],"elapsed":range(t.elapsed.min,t.elapsed.max),
                    "work":range(t.work.min,t.work.max),"result_at":t.result_at,
                    "probability":t.probability
                })
            })
            .collect();
        assert_eq!(by_end(rows), expected, "hand terminal oracle {name}");
    }
    let probabilities: Vec<_> = out
        .properties
        .iter()
        .flat_map(|p| {
            p.probability.iter().chain(
                p.cases
                    .iter()
                    .flatten()
                    .filter_map(|c| c.probability.as_ref()),
            )
        })
        .collect();
    let p: Vec<_> = probabilities
        .iter()
        .map(|r| (r.min.to_string(), r.max.to_string()))
        .collect();
    let pairs: &[(&str, &str)] = match name {
        "M1" => &[("1/4", "1/4")],
        "M5" => &[("1/4", "1/4"), ("1/4", "1/4")],
        "M2" => &[("1/4", "1/4"), ("1/2", "1/2"), ("1/4", "1/4")],
        "M3" => &[("0", "1"), ("0", "1")],
        "M4" => &[("1/20", "1/20")],
        _ => unreachable!(),
    };
    assert_eq!(
        p,
        pairs
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect::<Vec<_>>()
    );
    let violations: Vec<_> = out
        .properties
        .iter()
        .filter(|p| p.verdict != Verdict::Satisfied)
        .collect();
    assert_eq!(
        violations.len(),
        usize::from(name == "M1"),
        "{violations:?}"
    );
    if name == "M1" {
        assert_eq!(violations[0].verdict, Verdict::Violated);
    }
    hand_metrics(name, &json!(out.metrics.as_ref().unwrap()), witness);
}

fn backend(name: &str, witness: bool) {
    let model = resolve(&source(name, witness));
    let e = engine::explore(&model);
    hand(name, &model, &e, witness);
    let r = reference::explore_with_limits(&model, reference::Limits::default());
    assert_eq!(r.stop, reference::StopReason::Complete);
    hand(name, &model, &r.outcome, witness);
    // Differential comparison follows, rather than replacing, both hand checks.
    assert_eq!(e.terminals, r.outcome.terminals);
    assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
    assert_eq!(e.properties, r.outcome.properties);
}

macro_rules! accept {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            backend($name, std::env::var_os("MERGE_IF_WITNESSES").is_some());
        }
    };
}
accept!(m1_selected_source_is_zero_time_non_task_copy, "M1");
accept!(m2_end_branch_and_nested_merge_keep_probability_mass, "M2");
accept!(m3_nondeterminism_is_not_uniform_probability, "M3");
accept!(m4_branch_writes_survive_the_copy, "M4");
accept!(m5_each_arrival_gets_its_own_result, "M5");

#[test]
fn old_dsl_expansions_each_face_both_hand_oracles() {
    for name in ["M1", "M2", "M3", "M4", "M5"] {
        backend(name, true);
    }
}

#[test]
fn calibration_reference_only_state_reset_is_rejected_by_hand_oracle() {
    let good = resolve(&source("M4", true));
    let bad_source = source("M4", true).replace("with bad = false", "with bad = true");
    let bad = resolve(&bad_source);
    hand("M4", &good, &engine::explore(&good), true);
    let r = reference::explore(&bad);
    assert_eq!(r.stop, reference::StopReason::Complete);
    let flagged = r
        .outcome
        .terminals
        .iter()
        .find(|t| bad.ends[t.end.0] == "flagged")
        .unwrap();
    assert_eq!(flagged.probability.as_ref().unwrap().min.to_string(), "1/5");
    assert!(std::panic::catch_unwind(|| hand("M4", &bad, &r.outcome, true)).is_err());
}

#[test]
fn copy_has_no_random_site_and_does_not_shift_later_samples() {
    let raw = source("M1", false)
        .replace(
            "s = Ledger.send(d2)",
            "s = Ledger.send(d2); after = Rev.review(d)",
        )
        .replace("if d2 == repaired", "if after == pass");
    let raw = raw
        .lines()
        .filter(|line| {
            !line.trim_start().starts_with("contract Deadline")
                && !line.trim_start().starts_with("verify probability")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let i = raw.rfind('}').unwrap();
    let raw = format!(
        "{} simulation replications 32 seed 7 confidence 0.5 }}",
        &raw[..i]
    );
    // Handwritten non-merge control retains exactly the same execution and draw
    // sites. Only the unused merged value is removed; it is not an adapter.
    let plain = raw
        .replace(
            "d2 = if r == fail { f = Clerk.fix(d); yield f } else { yield d }",
            "if r == fail { f = Clerk.fix(d) } else { }",
        )
        .replace("Ledger.send(d2)", "Ledger.send(d)");
    let a = engine::simulate(&resolve(&raw));
    let b = engine::simulate(&resolve(&plain));
    assert_eq!(a.completed_replications, 32, "{:?}", a.diagnostics);
    assert_eq!(b.completed_replications, 32, "{:?}", b.diagnostics);
    for (a, b) in a.replications.iter().zip(&b.replications) {
        assert_eq!(a.cases[0].terminal, b.cases[0].terminal);
        let suffix = |r: &outcome::ReplicationOutcome| {
            r.draws
                .iter()
                .filter(|d| d.key.contains("after"))
                .map(|d| d.key.clone())
                .collect::<Vec<_>>()
        };
        assert!(!suffix(a).is_empty());
        assert_eq!(suffix(a), suffix(b));
        assert!(a.draws.iter().all(|d| !d.key.contains("d2")));
    }
}
