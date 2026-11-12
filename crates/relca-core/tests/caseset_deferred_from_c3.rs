//! ADR 0037 integration with ADR0035 (D01-D04): leading
//! negation in state and event invariants, membership in a state invariant
//! across one atomic finish bundle, and PersonId equality in a state
//! invariant. Every expected value below is hand-derived from the model's
//! arithmetic (one case, fixed 1min durations), never copied from output.
use relca_core::{engine, explain, ir, lower, reference, report, syntax};
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

/// Exhaustive engine and the independent reference agree with the hand values.
fn both(s: &str, verdicts: &[(&str, &str)], end: &str, elapsed: u64, work: u64) -> Value {
    both_with(s, &[], verdicts, end, elapsed, work)
}

fn both_with(
    s: &str,
    warnings: &[&str],
    verdicts: &[(&str, &str)],
    end: &str,
    elapsed: u64,
    work: u64,
) -> Value {
    let v = check(s);
    let m = model(s);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(
        r.stop,
        reference::StopReason::Complete,
        "no silent reference exclusion"
    );
    assert_eq!(e.properties, r.outcome.properties);
    assert_eq!(e.terminals, r.outcome.terminals);
    assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
    // ADR 0054 "範囲" / "意味": only the selected IR's U/E carriers may
    // extend the closed diagnostic vector, and their full rows are required.
    responsibility_oracle::assert_report(&m, &v);
    let responsibility = responsibility_oracle::expected(&m);
    let mut expected = warnings.to_vec();
    expected.retain(|code| *code != "W-RESP-0");
    expected.extend(std::iter::repeat_n("W-RESP-0", responsibility.len()));
    expected.sort();
    assert_eq!(codes(&v), expected);
    assert_eq!(v["bounds"]["truncated"], json!(false));
    for (suffix, verdict) in verdicts {
        assert_eq!(property(&v, suffix)["verdict"], *verdict, "{suffix}: {v}");
    }
    let terminals = v["terminals"].as_array().unwrap();
    assert_eq!(terminals.len(), 1, "{v}");
    assert_eq!(terminals[0]["end"], end);
    assert_eq!(
        terminals[0]["elapsed"],
        json!({"min": elapsed, "max": elapsed})
    );
    assert_eq!(terminals[0]["work"], json!({"min": work, "max": work}));
    v
}

fn refused(s: &str) {
    let v = check(s);
    assert_eq!(v["verdict"], "unsupported", "{v}");
    assert_eq!(codes(&v), ["U-SYN-0", "W-CUT-0"]);
    assert_eq!(v["properties"], json!([]));
    assert!(
        v["terminals"].as_array().is_none_or(|t| t.is_empty()),
        "{v}"
    );
    assert_eq!(v["bounds"]["truncated"], Value::Null);
    let primary = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "U-SYN-0")
        .unwrap();
    let span = &primary["span"];
    let start = span["start"].as_u64().unwrap() as usize;
    assert_eq!(
        &s[start..start + 1],
        "!",
        "the refusal is placed on the bang"
    );
}

const D01: &str = include_str!("../../../fixtures/sinv-bool-type/model.relca");

const D02: &str = r#"model DeferredEvent version "0.2" {
  type R = enum { ok }
  observed case { ok: bool = true }
  actor Fix { capacity 1; capability fix() -> R { duration fixed 1min; result nondet { ok } } }
  process Flow { r = Fix.fix(); end done }
  cut C { include [Fix] }
  bounds { cases 1; live_tasks 1; clock 4min }
  contract Controls { invariant Fix.fix.start -> ok == true }
}
"#;

const D03: &str = r#"model DeferredAtomic version "0.2" {
  type Ack = enum { committed, rejected }
  observed case { registered: bool = false }
  actor Ledger {
    capacity 1
    state keys: set<CaseId>
    capability post(key: CaseId) -> Ack {
      commits
      duration fixed 1min
      writes [keys, registered]
      atomic { if key in keys { result rejected } else { keys += key; registered = true; result committed } }
    }
  }
  process Flow { a = Ledger.post(case.id); b = Ledger.post(case.id); end done }
  cut C { include [Ledger] }
  bounds { cases 1; live_tasks 1; clock 4min }
  contract Controls {
    invariant case.id in Ledger.keys -> registered
    at_most_once Ledger.post.commit per case.id
  }
}
"#;

const D04: &str = r#"model DeferredPerson version "0.2" {
  type R = enum { done }
  people [p]
  observed case { who: optional<PersonId>; ok: bool = true }
  actor Worker { executors [p]; capability work() -> R { duration fixed 1min; result nondet { done }; writes who; finish { who = executor.id } } }
  process Flow { r = Worker.work(); end done }
  cut C { include [Worker] }
  bounds { cases 1; live_tasks 1; clock 4min }
  contract Controls { invariant who == p -> ok == true }
}
"#;

/// The first counterexample step that carries the state-invariant detail.
fn witness(s: &str) -> (Value, Value) {
    let x = serde_json::to_value(explain::explain(s)).unwrap();
    let c = x["explanation"]["counterexamples"][0].clone();
    assert_eq!(c["status"], "verified", "{x}");
    let step = c["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| !s["violation"].is_null())
        .unwrap()
        .clone();
    (c, step)
}

fn detail_values(step: &Value) -> Vec<(String, Value)> {
    step["violation"]["state_invariant"]["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (v["name"].as_str().unwrap().to_owned(), v["value"].clone()))
        .collect()
}

#[test]
fn d01_state_leading_negation_is_unsupported_on_the_bang() {
    both(D01, &[("#1 invariant", "satisfied")], "done", 1, 1);
    refused(&change(D01, "invariant ok", "invariant !ok"));
}

#[test]
fn d02_event_consequent_leading_negation_is_unsupported_on_the_bang() {
    both(D02, &[("#1 invariant", "satisfied")], "done", 1, 1);
    refused(&change(D02, "-> ok == true", "-> !ok == true"));
}

#[test]
fn d03_membership_state_invariant_reads_one_atomic_finish_bundle() {
    // finish1: keys {} -> {1} and registered=true together; finish2 rejected.
    // ADR 0043: Ledger alone executes Ledger.post and is its atomic refuser,
    // so one W-AGY-2. ADR 0054 adds Ledger's W-RESP-0 even without people;
    // both_with checks that complete row against the independent IR oracle.
    let v = both_with(
        D03,
        &["W-AGY-2"],
        &[
            ("#1 invariant", "satisfied"),
            ("#2 at_most_once", "satisfied"),
        ],
        "done",
        2,
        2,
    );
    assert!(
        property(&v, "#2 at_most_once").get("notes").is_none(),
        "the atomic membership mechanism is modeled: {v}"
    );
    let broken = change(D03, "registered = true", "registered = false");
    let v = both_with(
        &broken,
        &["W-AGY-2"],
        &[
            ("#1 invariant", "violated"),
            ("#2 at_most_once", "satisfied"),
        ],
        "done",
        2,
        2,
    );
    assert_eq!(v["verdict"], "violated");
    let (c, step) = witness(&broken);
    assert_eq!(c["case_id"], 1);
    assert_eq!(step["kind"], "finish");
    assert_eq!(step["time"], 1, "the first finish, no midpoint state");
    let d = &step["violation"]["state_invariant"];
    assert_eq!(
        (
            d["case_id"].clone(),
            d["antecedent"].clone(),
            d["condition"].clone()
        ),
        (json!(1), json!(true), json!(false))
    );
    assert_eq!(
        detail_values(&step),
        [
            ("registered".to_owned(), json!("false")),
            ("Ledger.keys".to_owned(), json!([1]))
        ]
    );
}

#[test]
fn d04_person_equality_state_invariant() {
    both(D04, &[("#1 invariant", "satisfied")], "done", 1, 1);
    let broken = change(D04, "-> ok == true", "-> ok == false");
    let v = both(&broken, &[("#1 invariant", "violated")], "done", 1, 1);
    assert_eq!(v["verdict"], "violated");
    let (c, step) = witness(&broken);
    assert_eq!(c["case_id"], 1);
    assert_eq!(
        (step["kind"].clone(), step["time"].clone()),
        (json!("finish"), json!(1))
    );
    assert_eq!(
        detail_values(&step),
        [
            ("who".to_owned(), json!("p")),
            ("ok".to_owned(), json!("true"))
        ]
    );
    // The same one case through the arrivals graph.
    for s in [D04.to_owned(), broken] {
        let a = change(
            &s,
            "  bounds { cases 1; live_tasks 1; clock 4min }\n",
            "  bounds { cases 1; live_tasks 1; clock 4min }\n  arrivals fixed [0min]\n",
        );
        let v = check(&a);
        let m = model(&a);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(engine::explore(&m).properties, r.outcome.properties);
        assert_eq!(codes(&v), Vec::<&str>::new());
        assert_eq!(
            property(&v, "#1 invariant")["verdict"],
            if s == D04 { "satisfied" } else { "violated" }
        );
    }
}
