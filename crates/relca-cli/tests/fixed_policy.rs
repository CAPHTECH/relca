//! Policy acceptance crosses the real CLI and the closed existing wire validator.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/policy_acceptance.rs"]
mod h;
use serde_json::{Value, json};
use std::process::{Command, Output};
fn run(args: &[&str]) -> (Output, Value) {
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .current_dir(h::root())
        .output()
        .unwrap();
    let v = serde_json::from_slice(&o.stdout).unwrap();
    (o, v)
}
fn path(n: &str) -> String {
    format!("fixtures/{n}/model.relca")
}
#[test]
fn all74_partial_expectations_validate_before_any_implementation_runs() {
    assert_eq!(h::names().len(), 74);
    let mut table = fixture_match::POLICY_FIXTURES.to_vec();
    table.sort();
    assert_eq!(
        table,
        h::names().iter().map(String::as_str).collect::<Vec<_>>()
    );
    for p in h::pairs() {
        let inv = fixture_match::Invocation::parse(if p["invocation"].is_null() {
            None
        } else {
            Some(&p["invocation"])
        })
        .unwrap();
        for n in [p["good"].as_str().unwrap(), p["bad"].as_str().unwrap()] {
            inv.validate_expected(&h::expected(n))
                .unwrap_or_else(|e| panic!("{n}: {e}"));
        }
    }
}
#[test]
fn all74_reports_have_closed_wire_hand_verdict_exit_and_numerics() {
    for p in h::pairs() {
        for n in [p["good"].as_str().unwrap(), p["bad"].as_str().unwrap()] {
            let inv = fixture_match::Invocation::parse(if p["invocation"].is_null() {
                None
            } else {
                Some(&p["invocation"])
            })
            .unwrap();
            let expected = h::expected(n);
            inv.validate_expected(&expected).unwrap();
            let o = Command::new(env!("CARGO_BIN_EXE_relca"))
                .args(inv.args(&h::root().join(path(n))))
                .output()
                .unwrap();
            let actual: Value = serde_json::from_slice(&o.stdout).unwrap();
            inv.compare(&expected, &actual)
                .unwrap_or_else(|e| panic!("{n}: {e}"));
            let verdict: relca_core::Verdict =
                serde_json::from_value(expected["verdict"].clone()).unwrap();
            assert_eq!(o.status.code(), Some(verdict.exit_code()), "{n}");
        }
    }
}
#[test]
fn configurations_compare_diff_preserve_exact_selected_reports_and_case_outcomes() {
    for (name, sim_exit) in [
        ("pol-configuration", 0),
        ("pol-configuration.broken-change", 4),
    ] {
        let p = path(name);
        for mode in ["exhaustive", "simulation"] {
            let (out, v) = run(&["compare", &p, "--plans", "CP,CQ", "--mode", mode]);
            assert_eq!(
                out.status.code(),
                Some(if mode == "simulation" { sim_exit } else { 0 })
            );
            assert_eq!(v["plans"].as_array().unwrap().len(), 2);
            let inv = fixture_match::Invocation::parse(Some(
                &json!({"command":"compare","plans":["CP","CQ"],"mode":mode}),
            ))
            .unwrap();
            inv.compare(&json!({"verdict":if mode=="simulation" && sim_exit==4{"unsupported"}else{"satisfied"}}),&v).unwrap();
            for (i, c, minutes) in [(0, "CP", 2), (1, "CQ", 3)] {
                let (_, single) = run(&[
                    if mode == "exhaustive" {
                        "check"
                    } else {
                        "simulate"
                    },
                    &p,
                    "--configuration",
                    c,
                ]);
                assert_eq!(
                    v["plans"][i]["report"], single,
                    "exact nested selection {c}/{mode}"
                );
                if mode == "exhaustive" {
                    assert_eq!(
                        single["case_outcomes"][0]["terminals"][0]["completed_at"],
                        json!({"min":minutes,"max":minutes})
                    );
                    assert_eq!(
                        h::row(&single, "policy.classes").unwrap()["value"],
                        if c == "CP" {
                            "policy[A]; environment[B]; party[]"
                        } else {
                            "policy[B]; environment[A]; party[]"
                        }
                    );
                }
            }
        }
        let (out, d) = run(&["diff", &p, "--from", "CP", "--to", "CQ"]);
        assert_eq!(out.status.code(), Some(0));
        let inv = fixture_match::Invocation::parse(Some(
            &json!({"command":"diff","from":"CP","to":"CQ"}),
        ))
        .unwrap();
        inv.compare(&json!({"verdict":"satisfied"}), &d).unwrap();
        for (side, c) in [("from", "CP"), ("to", "CQ")] {
            let (_, single) = run(&["check", &p, "--configuration", c]);
            assert_eq!(d[side]["report"], single);
        }
    }
}
#[test]
fn explain_uses_single_process_and_independent_verified_replay() {
    let (o, v) = run(&["explain", &path("pol-separation.broken-change")]);
    assert_eq!(o.status.code(), Some(1));
    let inv = fixture_match::Invocation::parse(Some(&json!({"command":"explain"}))).unwrap();
    inv.compare(&json!({"verdict":"violated"}), &v).unwrap();
    assert_eq!(v["explanation"]["status"], "complete");
    assert_eq!(
        v["explanation"]["counterexamples"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    for c in v["explanation"]["counterexamples"].as_array().unwrap() {
        assert_eq!(c["replay"]["status"], "verified");
        let mut finishes = Vec::new();
        for step in c["steps"].as_array().unwrap() {
            for e in step["events"].as_array().unwrap() {
                if e["kind"] == "finish" {
                    finishes.push(e["executor"].clone());
                }
            }
        }
        assert_eq!(finishes, vec![json!("c2"), json!("c2")]);
    }
}

/// Minimal input with one guard site and the same extension as pol-refuse.
/// No pol-* fixture has a guard site (an `if` whose discarded branch ends while
/// a call follows), so the mutant side is checked on this inline model.
const GUARDED_POLICY: &str = r#"model M version "0.2" {
  type V = enum { pass, fail }
  actor Clerk {
    capacity 1
    capability check() -> V { duration fixed 1min; result nondet { pass, fail } }
    capability post() -> V { duration fixed 1min; result nondet { pass } }
  }
  process P {
    a = Clerk.check()
    if a == pass { } else { end returned }
    b = Clerk.post()
    end done
  }
  cut C { include [Clerk] }
  bounds { cases 2; live_tasks 2; clock 10min }
  policy fifo tie_by case_id then call_order
  arrivals fixed [0min,0min]
  verify safety no_resource_overcommit
}
"#;
const GUARD_KINDS: [&str; 2] = ["hold_delayed", "refusal_path_cut"];
const READ_KINDS: [&str; 2] = ["evidence_missing", "original_access_removed"];

#[test]
fn mutation_original_and_every_guard_mutant_preserve_policy_declaration() {
    let file = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!(
            "p{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
        .join("fixed_policy_guard.relca");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, GUARDED_POLICY).unwrap();
    let (_, v) = run(&["mutate", file.to_str().unwrap()]);
    fixture_match::validate_mutation_wire(&v).unwrap();
    assert_eq!(v["original"]["verdict"], "satisfied");
    assert_eq!(
        h::row(&v["original"], "policy.queue").unwrap()["value"],
        "fifo tie_by case_id then call_order"
    );
    assert!(h::row(&v["original"], "policy.classes").is_some());
    // Hand count: one `if` (body/1) whose else ends and a call follows, so
    // keep_then is the only guard site; no reads, so no read site. Each of the
    // two guard kinds yields exactly that one mutant.
    let mut ids = Vec::new();
    for group in v["mutations"].as_array().unwrap() {
        let kind = group["kind"].as_str().unwrap();
        for mutant in group["mutants"].as_array().unwrap() {
            assert!(GUARD_KINDS.contains(&kind), "{kind}");
            ids.push(mutant["id"].as_str().unwrap().to_owned());
            assert!(!mutant["report"].is_null(), "{kind}: mutant must run");
            for field in ["policy.queue", "policy.classes", "schedule"] {
                assert_eq!(
                    h::row(&mutant["report"], field),
                    h::row(&v["original"], field),
                    "{kind}: {field}"
                );
            }
        }
    }
    assert_eq!(
        ids,
        GUARD_KINDS
            .iter()
            .map(|k| format!("{k}:P:body/1:keep_then"))
            .collect::<Vec<_>>()
    );
}

#[test]
fn pol_refuse_has_no_guard_or_read_site_so_no_mutant() {
    // Hand derivation: the only `if` keeps no call in either branch and no call
    // follows it (both arms `end`), and Ledger.post has no reads. So the guard
    // and read kinds have no matching site and no mutant at all.
    let (_, v) = run(&["mutate", &path("pol-refuse")]);
    fixture_match::validate_mutation_wire(&v).unwrap();
    assert_eq!(v["original"]["verdict"], "satisfied");
    assert_eq!(
        h::row(&v["original"], "policy.queue").unwrap()["value"],
        "fifo tie_by case_id then call_order"
    );
    assert!(h::row(&v["original"], "policy.classes").is_some());
    for group in v["mutations"].as_array().unwrap() {
        let kind = group["kind"].as_str().unwrap();
        assert_eq!(group["mutants"], json!([]), "{kind}");
        if GUARD_KINDS.contains(&kind) || READ_KINDS.contains(&kind) {
            assert_eq!(group["status"], "not_applicable", "{kind}");
            assert_eq!(group["reason"], "no_matching_site", "{kind}");
        }
    }
}

#[test]
fn old_fifo_controls_keep_exact_refusal_message_and_do_not_gain_assignment() {
    let inv = fixture_match::Invocation::parse(Some(&json!({"command":"simulate"}))).unwrap();
    for (name, exit) in [
        ("pol-assign.broken-change", 4),
        ("pol-observable-fifo.broken-change", 0),
    ] {
        let (o, v) = run(&["simulate", &path(name)]);
        inv.compare(&h::expected(name), &v).unwrap();
        assert_eq!(o.status.code(), Some(exit));
        assert!(h::row(&v, "policy.classes").is_none());
        assert!(h::row(&v, "policy.assign").is_none());
        if exit == 4 {
            assert_eq!(
                v["diagnostics"][0]["message"],
                "simulation encountered unresolved nondeterminism: replication 1, time 0, cases [1], sites [P.a]; at least two distinct successor actions remain"
            );
        }
    }
}
