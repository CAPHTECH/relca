//! Explicit limits with CaseId sets and state invariants.
//! Received Held and Overflow cases are monitored by ADR 0037 state
//! invariants from their post-arrival state, at every reachable state,
//! including shared Actor state written later by another case. Every expected
//! value below is hand-derived from the model arithmetic (minutes, three
//! arrivals at 0, Task A.run of 2min on one slot), never copied from output.
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};

fn change(s: &str, a: &str, b: &str) -> String {
    let t = s.replacen(a, b, 1);
    assert_ne!(s, t, "replacement must occur: {a}");
    t
}

fn check(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}

fn property<'a>(v: &'a Value, suffix: &str) -> &'a Value {
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"].as_str().unwrap().ends_with(suffix))
        .unwrap_or_else(|| panic!("missing {suffix}: {v}"))
}

fn rows(v: &Value, suffix: &str) -> Vec<String> {
    property(v, suffix)["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["verdict"].as_str().unwrap().to_owned())
        .collect()
}

fn admitted(v: &Value) -> Vec<Value> {
    v["case_outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["admitted_at"].clone())
        .collect()
}

fn completed(v: &Value) -> Vec<Value> {
    v["case_outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            c["terminals"]
                .as_array()
                .unwrap()
                .first()
                .map_or(Value::Null, |t| {
                    json!([t["kind"], t["end"], t["completed_at"], t["work"]])
                })
        })
        .collect()
}

fn r(a: u64) -> Value {
    json!({"min": a, "max": a})
}

/// Exhaustive engine and the independent reference agree (lower diagnostics
/// are only in the report); the reference completes without exclusions.
fn both(s: &str) -> Value {
    let v = check(s);
    let m = lower::lower(&syntax::parse(s).unwrap()).unwrap();
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(
        r.stop,
        reference::StopReason::Complete,
        "no silent reference exclusion"
    );
    assert_eq!(e.properties, r.outcome.properties, "engine vs reference");
    assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
    v
}

const SAFETY: &str = "safety no_resource_overcommit";
const INV: &str = "#1 invariant";

// P17: concurrent1 wait_external fifo; flag restored by every start. The
// `writes [hit, flag]` is not in the language: `writes` lists the
// finish (result) writers only, and a start assignment needs no entry
// (E-TYPE-0 otherwise; precedent aobs-start-visible).
const P17: &str = r#"model Limits version "0.2" {
  type T = enum { ok }
  observed case { hit: bool = false; flag: bool = true }
  actor A { capacity 1; capability run() -> T { duration fixed 2min; result nondet { ok }; writes hit; start { flag = true }; finish { hit = true } } }
  process Flow { r = A.run(); end done }
  arrivals fixed [0min, 0min, 0min]
  policy fifo tie_by case_id
  concurrent cases 1 on_overflow wait_external fifo
  bounds { cases 3; live_tasks 1; clock 20min }
  verify safety no_resource_overcommit
  contract C { invariant flag == true }
}
"#;

#[test]
fn p17_held_cases_are_monitored_from_their_initial_state() {
    // A=(0,2,4), C=(2,4,6), work 2 each; flag true throughout.
    let v = both(P17);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(property(&v, SAFETY)["verdict"], "satisfied");
    assert_eq!(property(&v, INV)["verdict"], "satisfied");
    assert_eq!(rows(&v, INV), ["satisfied"; 3]);
    assert_eq!(admitted(&v), [r(0), r(2), r(4)]);
    assert_eq!(v["bounds"]["truncated"], json!(false));
    // Only the initial value changes. Arrival0 precedes every start, cases2,3
    // are observed false while Held; later starts restore true, the violation
    // persists. Times are unchanged.
    let b = both(&change(P17, "flag: bool = true", "flag: bool = false"));
    assert_eq!(b["verdict"], "violated", "{b}");
    assert_eq!(property(&b, INV)["verdict"], "violated");
    assert_eq!(rows(&b, INV), ["violated"; 3]);
    assert_eq!(admitted(&b), [r(0), r(2), r(4)]);
    let done = |c| json!(["end", "done", r(c), r(2)]);
    assert_eq!(completed(&b), [done(2), done(4), done(6)]);
}

// P18: queue1 on_overflow end spill; flag never written.
const P18: &str = r#"model Limits version "0.2" {
  type T = enum { ok }
  observed case { hit: bool = false; flag: bool = true }
  actor A { capacity 1; capability run() -> T { duration fixed 2min; result nondet { ok }; writes hit; finish { hit = true } } }
  process Flow { r = A.run(); end done }
  arrivals fixed [0min, 0min, 0min]
  policy fifo tie_by case_id
  queue capacity 1 on_overflow end spill
  bounds { cases 3; live_tasks 1; clock 20min }
  verify safety no_resource_overcommit
  contract C { invariant flag == true }
}
"#;

#[test]
fn p18_overflow_cases_are_monitored_never_vacuous() {
    let v = both(P18);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(rows(&v, INV), ["satisfied"; 3]);
    // Case1 completed2/work2; cases2,3 overflow at0/work0, no admission.
    let spill = json!(["overflow", "spill", r(0), r(0)]);
    assert_eq!(
        completed(&v),
        [
            json!(["end", "done", r(2), r(2)]),
            spill.clone(),
            spill.clone()
        ]
    );
    let b = both(&change(P18, "flag: bool = true", "flag: bool = false"));
    assert_eq!(b["verdict"], "violated", "{b}");
    assert_eq!(rows(&b, INV), ["violated"; 3]);
    assert_eq!(
        completed(&b),
        [json!(["end", "done", r(2), r(2)]), spill.clone(), spill]
    );
}

// P17b: latent initial distribution, clock3 cut while case3 is Held.
const P17B: &str = r#"model Limits version "0.2" {
  type T = enum { ok }
  observed case { hit: bool = false }
  latent case { flag: bool ~ { true:0.5, false:0.5 } }
  actor A { capacity 1; capability run() -> T { duration fixed 2min; result nondet { ok }; writes hit; finish { hit = true } } }
  process Flow { r = A.run(); end done }
  arrivals fixed [0min, 0min, 0min]
  policy fifo tie_by case_id
  concurrent cases 1 on_overflow wait_external fifo
  bounds { cases 3; live_tasks 1; clock 3min }
  verify safety no_resource_overcommit
  contract C { invariant flag == true }
}
"#;

#[test]
fn p17b_initial_distribution_is_drawn_at_arrival_not_admission() {
    // At0 the full 2^3 product is initialized; each case has a false tuple.
    // Case3 is never admitted before clock3, yet its row is violated, not
    // inconclusive: its initial value exists from arrival.
    let v = both(P17B);
    assert_eq!(v["verdict"], "violated", "{v}");
    assert_eq!(v["bounds"]["truncated"], json!(true));
    assert_eq!(rows(&v, INV), ["violated"; 3]);
    assert_eq!(admitted(&v), [r(0), r(2), Value::Null]);
    // Only the clock changes: rows unchanged, case3 admitted at4.
    let long = both(&change(P17B, "clock 3min", "clock 20min"));
    assert_eq!(long["verdict"], "violated", "{long}");
    assert_eq!(long["bounds"]["truncated"], json!(false));
    assert_eq!(rows(&long, INV), ["violated"; 3]);
    assert_eq!(admitted(&long), [r(0), r(2), r(4)]);
}

// K32: ADR 0035 shared set<CaseId> with a positive-time atomic post.
const K32: &str = r#"model Limits version "0.2" {
  type Ack = enum { committed, rejected_duplicate }
  observed case { registered: bool = false }
  actor Ledger { capacity 1; state keys: set<CaseId>; capability post(key: CaseId) -> Ack {
      commits
      writes [keys, registered]
      duration fixed 2min
      atomic {
        if key in keys { result rejected_duplicate }
        else { keys += key; registered = true; result committed }
      }
      cancel forbidden_after_start
    } }
  process Flow { a = Ledger.post(case.id); if a == committed { end done } else { end duplicate } }
  cut K { include [Ledger] }
  arrivals fixed [0min, 0min, 0min]
  policy fifo tie_by case_id
  concurrent cases 1 on_overflow wait_external fifo
  bounds { cases 3; live_tasks 1; clock 20min }
  verify safety no_resource_overcommit
  contract C {
    invariant registered -> case.id in Ledger.keys
    at_most_once Ledger.post.commit per case.id
  }
}
"#;

#[test]
fn k32_readmitted_cases_post_their_own_case_id() {
    // A=(0,2,4); commits at (2,4,6) each add the case's own id: keys {1},
    // {1,2}, {1,2,3}. Every post is committed, so every case ends done; a
    // readmitted case posting another case's id would end duplicate.
    let v = both(K32);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(admitted(&v), [r(0), r(2), r(4)]);
    let done = |c| json!(["end", "done", r(c), r(2)]);
    assert_eq!(completed(&v), [done(2), done(4), done(6)]);
    assert_eq!(rows(&v, INV), ["satisfied"; 3]);
    // A fourth, repeated post of the case's own key (only the process gains
    // one call): the repeat is a duplicate for every case, the later two
    // cases still commit their first post. A=(0,4,8), C=(4,8,12), work4.
    let repeat = change(
        K32,
        "a = Ledger.post(case.id); if a == committed",
        "a = Ledger.post(case.id); b = Ledger.post(case.id); if b == committed",
    );
    let b = both(&repeat);
    assert_eq!(admitted(&b), [r(0), r(4), r(8)]);
    let dup = |c| json!(["end", "duplicate", r(c), r(4)]);
    assert_eq!(completed(&b), [dup(4), dup(8), dup(12)]);
    assert_eq!(rows(&b, INV), ["satisfied"; 3]);
}

// M1 / ADR 0039 m8 with shared Actor state: the Held and Overflow cases
// observe the shared state written later by case1's finish.
const SHARED: &str = r#"model Limits version "0.2" {
  type T = enum { ok }
  observed case { hit: bool = false }
  actor A { capacity 1; state open: bool = true; capability run() -> T { duration fixed 2min; result nondet { ok }; writes [hit, open]; finish { hit = true; open = false } } }
  process Flow { r = A.run(); end done }
  cut K { include [A] }
  arrivals fixed [0min, 0min, 0min]
  policy fifo tie_by case_id
  concurrent cases 1 on_overflow wait_external fifo
  bounds { cases 3; live_tasks 1; clock 3min }
  verify safety no_resource_overcommit
  contract I { invariant A.open == true }
}
"#;

#[test]
fn m8_held_case_observes_shared_state_written_after_its_arrival() {
    // t0: case1 admitted and starts; cases2,3 Held, open=true. t2: case1
    // finishes (open=false) and ends done; case2 admitted at2. clock3 cuts
    // with case3 still Held. Every received case observes open=false at2.
    let v = both(SHARED);
    assert_eq!(v["verdict"], "violated", "{v}");
    assert_eq!(v["bounds"]["truncated"], json!(true));
    assert_eq!(admitted(&v), [r(0), r(2), Value::Null]);
    assert_eq!(rows(&v, INV), ["violated"; 3]);
}

#[test]
fn m8_overflow_case_observes_shared_state_after_its_business_end() {
    // Queue1 reserves case1; cases2,3 end spill at0. Case1 finishes at2 with
    // open=false; the ended overflow cases still observe it, like ended
    // admitted cases do.
    let s = change(
        &change(
            SHARED,
            "concurrent cases 1 on_overflow wait_external fifo",
            "queue capacity 1 on_overflow end spill",
        ),
        "clock 3min",
        "clock 20min",
    );
    let v = both(&s);
    assert_eq!(v["verdict"], "violated", "{v}");
    assert_eq!(v["bounds"]["truncated"], json!(false));
    let spill = json!(["overflow", "spill", r(0), r(0)]);
    assert_eq!(
        completed(&v),
        [json!(["end", "done", r(2), r(2)]), spill.clone(), spill]
    );
    assert_eq!(rows(&v, INV), ["violated"; 3]);
}

// K15 proposal (mutlog-b4/proposals/K15.md): lim-clock.broken-short-clock
// with a state invariant whose initial value fails.
const K15: &str = r#"model Limits version "0.2" {
  type T = enum { ok }
  observed case { hit: bool = false; flag: bool = false }
  actor A { capacity 1; capability run() -> T { duration fixed 2min; result nondet { ok }; writes hit; finish { hit = true } } }
  process Flow { r = A.run(); end done }
  arrivals fixed [0min, 0min, 0min]
  policy fifo tie_by case_id
  concurrent cases 1 on_overflow wait_external fifo
  bounds { cases 3; live_tasks 1; clock 3min }
  verify safety no_resource_overcommit
  contract I { invariant flag == true }
}
"#;

#[test]
fn k15_held_case_cut_by_clock_keeps_its_violation() {
    // t0: case1 admitted, cases2,3 Held with flag=false. t2: case1 done,
    // case2 admitted at2. clock3: case2 unfinished, case3 Held. flag is never
    // written, so every case violates from its post-arrival state; violated
    // precedes the cut's inconclusive.
    let v = both(K15);
    assert_eq!(v["verdict"], "violated", "{v}");
    assert_eq!(v["bounds"]["truncated"], json!(true));
    assert_eq!(admitted(&v), [r(0), r(2), Value::Null]);
    assert_eq!(rows(&v, INV), ["violated"; 3]);
}

#[test]
fn m8_simulation_monitors_held_and_overflow_cases_in_engine_and_reference() {
    // The selected path is deterministic (one Task at a time, FIFO): same
    // times as the exhaustive runs. clock20 lets case3 be admitted at4 under
    // the concurrent limit; the queue limit spills cases2,3 at0. Either way
    // all three received cases observe open=false from t2.
    for limit in [
        "concurrent cases 1 on_overflow wait_external fifo",
        "queue capacity 1 on_overflow end spill",
    ] {
        let s = SHARED.replacen(
            "concurrent cases 1 on_overflow wait_external fifo",
            limit,
            1,
        );
        let s = change(
            &change(&s, "clock 3min", "clock 20min"),
            "  verify safety",
            "  simulation replications 2 seed 1 confidence 0.95\n  verify safety",
        );
        let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
        let e = engine::simulate(&m);
        let r = reference::simulate(&m);
        for out in [&e, &r.outcome] {
            assert_eq!(out.completed_replications, 2, "{limit}");
            assert_eq!(out.replications.len(), 2);
            for rep in &out.replications {
                let v: Vec<_> = rep.cases.iter().map(|c| c.violations.clone()).collect();
                assert_eq!(v, vec![vec![true]; 3], "{limit}");
            }
        }
        assert_eq!(e.replications, r.outcome.replications, "{limit}");
    }
}

#[test]
fn m8_transient_shared_value_is_observed_by_held_and_overflow_cases() {
    // open=false only while a Task runs ([0,2), [2,4), [4,6) or [0,2)), true
    // again at each finish. Every received case observes a false state:
    // overflow cases from t0 (case1's start at0 follows their receipt), Held
    // cases while the earlier cases run. A transient value is not skipped.
    let flip = change(
        SHARED,
        "writes [hit, open]; finish { hit = true; open = false }",
        "writes [hit, open]; start { open = false }; finish { hit = true; open = true }",
    );
    for limit in [
        "concurrent cases 1 on_overflow wait_external fifo",
        "queue capacity 1 on_overflow end spill",
    ] {
        let s = flip.replacen(
            "concurrent cases 1 on_overflow wait_external fifo",
            limit,
            1,
        );
        let s = change(&s, "clock 3min", "clock 20min");
        let v = both(&s);
        assert_eq!(rows(&v, INV), ["violated"; 3], "{limit}");
        let s = change(
            &s,
            "  verify safety",
            "  simulation replications 2 seed 1 confidence 0.95\n  verify safety",
        );
        let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
        let e = engine::simulate(&m);
        let r = reference::simulate(&m);
        for out in [&e, &r.outcome] {
            assert_eq!(out.completed_replications, 2, "{limit}");
            for rep in &out.replications {
                let v: Vec<_> = rep.cases.iter().map(|c| c.violations.clone()).collect();
                assert_eq!(v, vec![vec![true]; 3], "{limit}");
            }
        }
    }
}

#[test]
fn m8_explain_still_replays_with_held_case_state_details() {
    // The trace now captures state details of the Held cases too; the
    // independent checker still replays the emitted counterexample (case1's,
    // the first violation). Case3's own counterexample is not emitted here,
    // so this does not check its replay.
    let x = serde_json::to_value(relca_core::explain::explain(SHARED)).unwrap();
    let c = x["explanation"]["counterexamples"].as_array().unwrap();
    assert_eq!(c.len(), 1, "{x}");
    assert_eq!(c[0]["case_id"], 1);
    assert_eq!(c[0]["status"], "verified", "{x}");
}

// Explicit-limit boundary: the explicit-limit
// Admitted (Activate and readmission) and overflow End bundles must evaluate
// ADR 0035 membership with the case's own index, like every other arrivals
// bundle, and like the reference. The antecedent is the membership, so a
// membership read for case1 instead of case k activates the monitor against
// case k's own registered=false and the violation is sticky.
fn member_first(s: &str) -> String {
    change(
        s,
        "invariant registered -> case.id in Ledger.keys",
        "invariant case.id in Ledger.keys -> registered",
    )
}

#[test]
fn m1_readmitted_case_reads_its_own_membership() {
    // K32 times: A=(0,2,4), commits at (2,4,6) add the case's own id. Case2 is
    // readmitted at2 with keys={1}: 2 is not in keys, inactive (case1's id
    // would be, with registered=false). Case2 becomes active only after its
    // own commit at4, when registered=true; case3 likewise at4 (keys={1,2})
    // and at6. Every row is active and holds.
    let v = both(&member_first(K32));
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(property(&v, INV)["verdict"], "satisfied");
    assert_eq!(rows(&v, INV), ["satisfied"; 3]);
    assert_eq!(admitted(&v), [r(0), r(2), r(4)]);
    let done = |c| json!(["end", "done", r(c), r(2)]);
    assert_eq!(completed(&v), [done(2), done(4), done(6)]);
}

#[test]
fn m1_case_admitted_at_a_later_arrival_reads_its_own_membership() {
    // Arrivals 0,3,6 under concurrent1: no case waits. Case1 posts 0..2
    // (keys={1}) and ends at2. Case2 arrives at3 into a free slot and is
    // admitted in the same instant with keys={1}: inactive. It commits at5
    // (keys={1,2}); case3 is admitted at6 and commits at8.
    let s = change(
        &member_first(K32),
        "arrivals fixed [0min, 0min, 0min]",
        "arrivals fixed [0min, 3min, 6min]",
    );
    let v = both(&s);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(rows(&v, INV), ["satisfied"; 3]);
    assert_eq!(admitted(&v), [r(0), r(3), r(6)]);
    let done = |c| json!(["end", "done", r(c), r(2)]);
    assert_eq!(completed(&v), [done(2), done(5), done(8)]);
}

#[test]
fn m1_overflow_case_after_a_commit_reads_its_own_membership() {
    // Case1 posts 0..2 (keys={1}, registered) and then runs A 2..4, so it is
    // still in the system at3. Case2 arrives at3, concurrent1 is full, and it
    // ends spill at once with keys={1}: 2 is not in keys. It never posts, so
    // its row never activates: vacuous, not violated (and not satisfied).
    let s = change(
        &change(
            &change(
                &change(
                    &member_first(K32),
                    "observed case { registered: bool = false }",
                    "type T = enum { ok }\n  observed case { registered: bool = false; hit: bool = false }",
                ),
                "  process Flow { a = Ledger.post(case.id);",
                "  actor A { capacity 1; capability run() -> T { duration fixed 2min; result nondet { ok }; writes hit; finish { hit = true } } }\n  process Flow { a = Ledger.post(case.id); r = A.run();",
            ),
            "arrivals fixed [0min, 0min, 0min]",
            "arrivals fixed [0min, 3min]",
        ),
        "cut K { include [Ledger] }",
        "cut K { include [Ledger, A] }",
    );
    let s = change(
        &s,
        "concurrent cases 1 on_overflow wait_external fifo\n  bounds { cases 3;",
        "concurrent cases 1 on_overflow end spill\n  bounds { cases 2;",
    );
    let v = both(&s);
    assert_eq!(rows(&v, INV), ["satisfied", "vacuous"], "{v}");
    assert_eq!(property(&v, INV)["verdict"], "vacuous");
    assert_eq!(v["verdict"], "vacuous", "{v}");
    // No wait_external is declared, so no case row carries admitted_at (ADR
    // 0039, output: only models declaring external waiting add the key).
    let outcomes = v["case_outcomes"].as_array().unwrap();
    assert!(
        outcomes.iter().all(|c| c.get("admitted_at").is_none()),
        "{v}"
    );
    assert_eq!(
        completed(&v),
        [
            json!(["end", "done", r(4), r(4)]),
            json!(["overflow", "spill", r(3), r(0)])
        ]
    );
}

#[test]
fn m1_simulation_reads_each_case_membership_in_engine_and_reference() {
    // The readmission input as one deterministic path (one Task at a time,
    // FIFO): the same times as the exhaustive run, so no case violates either
    // clause (the invariant or at_most_once) in any replication.
    let s = change(
        &member_first(K32),
        "  verify safety",
        "  simulation replications 2 seed 1 confidence 0.95\n  verify safety",
    );
    let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
    let e = engine::simulate(&m);
    let r = reference::simulate(&m);
    for out in [&e, &r.outcome] {
        assert_eq!(out.completed_replications, 2);
        for rep in &out.replications {
            let v: Vec<_> = rep.cases.iter().map(|c| c.violations.clone()).collect();
            assert_eq!(v, vec![vec![false, false]; 3]);
        }
    }
    assert_eq!(e.replications, r.outcome.replications);
}
