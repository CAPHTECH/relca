//! Rebase integration checks of ADR0035 with ADR 0034 (viewpoints), ADR 0036
//! (fixed policy) and ADR 0037 (state invariants). Expected values are
//! hand-derived in the comments, never copied from output.
use relca_core::{engine, ir, lower, reference, report, syntax};
use serde_json::{Value, json};
#[path = "support/responsibility_defaults_oracle.rs"]
mod responsibility_oracle;

fn change(s: &str, a: &str, b: &str) -> String {
    let t = s.replacen(a, b, 1);
    assert_ne!(s, t, "replacement must occur: {a}");
    t
}

fn model(s: &str) -> ir::Model {
    lower::lower(&syntax::parse(s).unwrap()).unwrap()
}

fn check(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}

fn codes(v: &Value) -> Vec<&str> {
    let mut c: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    c.sort();
    c
}

fn property<'a>(v: &'a Value, suffix: &str) -> &'a Value {
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"].as_str().unwrap().ends_with(suffix))
        .unwrap_or_else(|| panic!("missing {suffix}: {v}"))
}

/// Integration check (ADR0035 x ADR 0036 x ADR 0037), not part of the C3
/// Two cases arrive at 0min, the fixed policy starts case 1 first,
/// case 1 finishes at 1min (keys {1}), case 2 starts at 1min and finishes at
/// 2min (keys {1,2}). At 1min case 2 reads keys {1} with its own case.id 2, so
/// the antecedent is false for case 2; reading case 1's index would make case
/// 2's registered=false a false violation.
#[test]
fn membership_state_invariant_uses_each_case_context_under_the_fixed_policy() {
    let control = change(
        include_str!("../../../fixtures/aset-cases/model.relca"),
        "  contract C { at_most_once Ledger.post.commit per case.id }",
        "  contract C {\n    at_most_once Ledger.post.commit per case.id\n    invariant case.id in Ledger.keys -> registered\n  }",
    );
    let control = change(
        &control,
        "policy fifo tie_by case_id",
        "policy fifo tie_by case_id then call_order",
    );
    let broken = change(&control, "registered = true", "registered = false");
    for (s, invariant) in [(&control, "satisfied"), (&broken, "violated")] {
        let v = check(s);
        let m = model(s);
        responsibility_oracle::assert_report(&m, &v);
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(e.properties, r.outcome.properties);
        assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
        // ADR 0043 (plan C06): Ledger alone executes and atomically refuses
        // Ledger.post (W-AGY-2, one per effect); people [c1,c2] without
        // responsibility metadata warns for the atomic refuser Ledger (U12).
        assert_eq!(codes(&v), ["W-AGY-2", "W-RESP-0"], "{v}");
        assert_eq!(property(&v, "#1 at_most_once")["verdict"], "satisfied");
        assert_eq!(property(&v, "#2 invariant")["verdict"], invariant, "{v}");
        let cases = v["case_outcomes"].as_array().unwrap();
        for (case, elapsed) in [(1, 1), (2, 2)] {
            let t = &cases[case - 1]["terminals"][0];
            assert_eq!(cases[case - 1]["case_id"], case);
            assert_eq!(t["end"], "done");
            assert_eq!(t["elapsed"], json!({"min": elapsed, "max": elapsed}), "{v}");
            assert_eq!(t["work"], json!({"min": 1, "max": 1}));
        }
    }
}

/// One case, keys empty: both atomic posts commit, so the any-join always
/// adopts and end.lost is never reached (hand: a commits at 1min, b at 2min).
/// The adopted/none split reads atomic results, which depend on the set state,
/// exactly like an `if a == committed` on an atomic result (aset-dup and
/// aset-conditional-agency: framing ends not_derived, reason state_guards).
/// The viewpoints must not report `lost` as a derived reachable end.
const ANY_ATOMIC: &str = r#"model AnyAtomic version "0.2" {
  type Ack = enum { committed, rejected_duplicate }
  observed case { r1: bool = false; r2: bool = false }
  actor L1 { capacity 1; state keys: set<CaseId>; capability post(key: CaseId) -> Ack {
      commits
      writes [keys, r1]
      duration fixed 1min
      atomic { if key in keys { result rejected_duplicate } else { keys += key; r1 = true; result committed } }
    } }
  actor L2 { capacity 1; state keys: set<CaseId>; capability post(key: CaseId) -> Ack {
      commits
      writes [keys, r2]
      duration fixed 2min
      atomic { if key in keys { result rejected_duplicate } else { keys += key; r2 = true; result committed } }
    } }
  process Flow { fork f { a = L1.post(case.id); b = L2.post(case.id) }; join any f where result == committed tie by [a,b] remaining drain if none -> lost; end posted; end lost }
  cut K { include [L1, L2] }
  bounds { cases 1; live_tasks 3; clock 20min }
  verify reachable end.posted
}
"#;

#[test]
fn any_join_over_atomic_results_is_a_state_guard_in_the_viewpoints() {
    let v = check(ANY_ATOMIC);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    // ADR 0054 "範囲" / "意味": L1/L2 are both known E and atomic U
    // carriers even without people. Require the independent complete rows.
    let m = model(ANY_ATOMIC);
    responsibility_oracle::assert_report(&m, &v);
    let responsibility = responsibility_oracle::expected(&m);
    assert_eq!(
        responsibility
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>(),
        [
            "L1 has no declared responsibility",
            "L2 has no declared responsibility"
        ]
    );
    let mut expected_codes = vec!["W-AGY-2", "W-AGY-2", "W-UNREACH-2"];
    expected_codes.extend(std::iter::repeat_n("W-RESP-0", responsibility.len()));
    expected_codes.sort();
    assert_eq!(codes(&v), expected_codes, "end.lost is never reached");
    let ends = v["agency"]["viewpoints"]["framing"]["ends"]
        .as_array()
        .unwrap();
    assert_eq!(ends.len(), 2);
    for end in ends {
        assert_eq!(end["status"], "not_derived", "{end}");
        assert_eq!(end["reason"], "state_guards", "{end}");
    }
    // Control: the same shape with ordinary nondet results keeps the derived
    // selectors (posted: a or b in some contexts; lost: both, all contexts).
    let mut plain = ANY_ATOMIC.to_owned();
    for (n, d) in [("1", "1"), ("2", "2")] {
        plain = change(
            &plain,
            &format!(
                "writes [keys, r{n}]\n      duration fixed {d}min\n      atomic {{ if key in keys {{ result rejected_duplicate }} else {{ keys += key; r{n} = true; result committed }} }}"
            ),
            &format!(
                "writes r{n}\n      duration fixed {d}min\n      result nondet {{ committed, rejected_duplicate }}; finish {{ r{n} = true }}"
            ),
        );
    }
    let v = check(&plain);
    let m = model(&plain);
    responsibility_oracle::assert_report(&m, &v);
    assert_eq!(
        codes(&v),
        vec!["W-RESP-0"; responsibility_oracle::expected(&m).len()],
        "{v}"
    );
    let ends = v["agency"]["viewpoints"]["framing"]["ends"]
        .as_array()
        .unwrap();
    for (end, selects) in [("posted", "some_contexts"), ("lost", "all_contexts")] {
        let row = ends.iter().find(|e| e["end"] == end).unwrap();
        assert_eq!(row["status"], "derived");
        assert_eq!(row["reachable"], true);
        assert_eq!(
            row["selectors"],
            json!([
                {"call": "a", "capability": "L1.post", "selects": selects},
                {"call": "b", "capability": "L2.post", "selects": selects}
            ])
        );
    }
}
