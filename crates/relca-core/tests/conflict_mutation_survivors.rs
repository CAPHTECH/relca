//! ADR 0041 acceptance for mutation survivors (F21v, F30, NL3, NL4, NL7, REF4,
//! NL9, PC1, F18, AG3). Every expectation below was computed by hand from the
//! source before the test was run; the calc is the comment above each test.
//! The models are inline sources, not fixtures, so the closed `cf-*`
//! population is unchanged.
#[path = "support/actor_observed_acceptance.rs"]
mod h;
use relca_core::{engine, explain, ir, reference, report};
use serde_json::{Value, json};

fn property<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap_or_else(|| panic!("missing property {id}: {v}"))
}

fn loss_verdicts(o: &relca_core::outcome::Outcome, m: &ir::Model) -> Vec<(String, Value)> {
    o.properties
        .iter()
        .filter_map(|p| match p.property {
            ir::Property::NoLostUpdate(id) => Some((m.loss_property_id(id), json!(p.verdict))),
            _ => None,
        })
        .collect()
}

/// The engine and the independent reference agree on every property and the
/// reference finished its enumeration.
fn independent(s: &str) {
    let m = h::model(s);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e.properties, r.outcome.properties);
    assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
}

/// F21v (LWW). Ledger capacity 2, `holder: H = taken last_write_wins`,
/// claim 1min writes `holder = taken`. cases 2, arrivals [0,0].
/// Calc: both claims start at 0 (capacity 2) and finish at 1. The first finish
/// writes taken over taken: the value does not change, but it is a receipt,
/// so the other running claim is marked. The second finish overwrites while
/// marked -> loss. Expected: `no_lost_update Ledger.holder` violated, whole
/// report violated, `reachable end.done` satisfied, both cases end done at 1.
const SAME_VALUE_LWW: &str = r#"model C version "0.2" {
  type R = enum { committed, conflict }
  type H = enum { free, taken }
  actor Ledger {
    capacity 2
    state holder: H = taken last_write_wins
    capability claim() -> R {
      commits
      duration fixed 1min
      result nondet { committed }
      writes holder
      finish { holder = taken }
    }
  }
  process P {
    r = Ledger.claim()
    end done
  }
  cut C { include [Ledger] }
  bounds { cases 2; live_tasks 1; clock 10min }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  verify reachable end.done
}"#;

#[test]
fn f21v_unchanged_value_lww_receipt_still_marks_and_loses() {
    let v = json!(report::check(SAME_VALUE_LWW));
    assert_eq!(v["verdict"], "violated", "{v}");
    assert_eq!(
        property(&v, "no_lost_update Ledger.holder")["verdict"],
        "violated"
    );
    assert_eq!(property(&v, "reachable end.done")["verdict"], "satisfied");
    for case in [1, 2] {
        h::terminal(&v, case, "done", (1, 1), (1, 1), (1, 1));
    }
    independent(SAME_VALUE_LWW);
}

/// F21v (version). Picker (capacity 2, 1min) picks one|two. one = claim
/// (version_check [holder], uses slots:1 with slots capacity 1, 2min,
/// holder = taken); two = release (version_check [holder], 1min,
/// holder = free; free is also the initial value). cases 2, arrivals [0,0].
/// Calc: both picks finish at 1. Path (one,two): claim and release start at 1;
/// release finishes at 2 writing free over free (unchanged, still a receipt),
/// so the running claim is marked; claim finishes at 3 with the reject value
/// -> end.claim_conflict at 3 (elapsed 3, work 1+2=3). Path (one,one): the
/// claims serialize on slots (2nd starts at 3 after the 1st finished), no
/// mark. Release-only paths end released. Expected: `reachable
/// end.claim_conflict` satisfied, report satisfied, and in each case the
/// claim_conflict terminal is at exactly 3.
const SAME_VALUE_VERSION: &str = r#"model C version "0.2" {
  type R = enum { committed, conflict }
  type H = enum { free, taken }
  type Pick = enum { one, two }
  resource slots capacity 1
  actor Picker { capacity 2; capability choose() -> Pick { duration fixed 1min; result nondet { one, two } } }
  actor Ledger {
    capacity 2
    state holder: H = free
    capability claim() -> R {
      version_check [holder] else conflict
      uses slots: 1
      commits
      duration fixed 2min
      result nondet { committed }
      writes holder
      finish { holder = taken }
    }
    capability release() -> R {
      version_check [holder] else conflict
      duration fixed 1min
      result nondet { committed }
      writes holder
      finish { holder = free }
    }
  }
  process P {
    p = Picker.choose()
    if p == one {
      x = Ledger.claim()
      if x == conflict { end claim_conflict } else { end claimed }
    } else {
      y = Ledger.release()
      end released
    }
  }
  cut C { include [Picker, Ledger] }
  bounds { cases 2; live_tasks 1; clock 10min }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  verify reachable end.claim_conflict
}"#;

#[test]
fn f21v_unchanged_value_version_receipt_rejects_the_checked_claim() {
    let v = json!(report::check(SAME_VALUE_VERSION));
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(
        property(&v, "reachable end.claim_conflict")["verdict"],
        "satisfied"
    );
    for case in [1, 2] {
        h::terminal(&v, case, "claim_conflict", (3, 3), (3, 3), (3, 3));
    }
    independent(SAME_VALUE_VERSION);
}

/// F30 and NL3. cases 1. Judge.judge (1min, fail) -> end.returned at 1. Party
/// and Other both contest end.returned; their routes Ledger.respond and
/// Ledger.recheck (2min each, `holder = taken`, holder last_write_wins) run on
/// executors p, q with capacity 2. Calc: on the path where both are filed at 1
/// they run 1..3; the first finish at 3 writes holder and marks the other
/// running route; the second finish overwrites while marked -> loss inside a
/// single case. Expected: `no_lost_update Ledger.holder` violated and the
/// report violated, in the engine and in the reference alike.
const ROUTE_PAIR_LWW: &str = r#"model Route version "0.2" {
  type T = enum { pass, fail }
  type H = enum { free, taken }
  people [j, p, q]
  actor Judge { executors [j]; capability judge() -> T { duration fixed 1min; result nondet { fail } } }
  actor Ledger {
    executors [p, q]
    capacity 2
    state holder: H = free last_write_wins
    capability respond() -> T { duration fixed 2min; result nondet { pass }; writes holder; finish { holder = taken } }
    capability recheck() -> T { duration fixed 2min; result nondet { pass }; writes holder; finish { holder = taken } }
  }
  actor Party { control party; standing over [end.returned] basis "claim" }
  actor Other { control party; standing over [end.returned] basis "claim2" }
  permit j to Judge.judge
  permit p to Ledger.respond
  permit q to Ledger.respond
  permit p to Ledger.recheck
  permit q to Ledger.recheck
  authority G { basis "decision"; grants j to Judge.judge }
  process Flow { d = Judge.judge(); if d == fail { end returned } else { end posted } }
  cut C { include [Judge, Ledger, Party, Other] }
  bounds { cases 1; live_tasks 3; clock 10min }
  verify reachable end.returned
  contract Controls { contest Party on end.returned route Ledger.respond reversible_by [j] respond_by 5min }
  contract Controls2 { contest Other on end.returned route Ledger.recheck reversible_by [j] respond_by 5min }
}"#;

#[test]
fn f30_nl3_single_case_concurrent_routes_lose_an_update() {
    let v = json!(report::check(ROUTE_PAIR_LWW));
    assert_eq!(v["verdict"], "violated", "{v}");
    assert_eq!(
        property(&v, "no_lost_update Ledger.holder")["verdict"],
        "violated"
    );
    let m = h::model(ROUTE_PAIR_LWW);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    let want = vec![("no_lost_update Ledger.holder".to_owned(), json!("violated"))];
    assert_eq!(loss_verdicts(&e, &m), want, "engine");
    assert_eq!(loss_verdicts(&r.outcome, &m), want, "reference");
    assert_eq!(e.properties, r.outcome.properties);
}

/// NL4. The route pair above plus a third contest (Third) whose route
/// Judge.slow takes 20min, with clock 10min; the process ends returned
/// unconditionally. Calc: the loss happens at 3 exactly as above; slow starts
/// at 1 and would finish at 21 > clock 10, so the search is cut (truncated).
/// A loss already found is a witness and stays violated under the cut (the
/// m1 rule of ADR 0041). Expected in both interpreters: truncated = true and
/// `no_lost_update Ledger.holder` violated (not inconclusive).
const ROUTE_PAIR_TRUNCATED: &str = r#"model Route version "0.2" {
  type T = enum { pass, fail }
  type H = enum { free, taken }
  people [j, p, q]
  actor Judge { executors [j]; capability judge() -> T { duration fixed 1min; result nondet { fail } }
    capability slow() -> T { duration fixed 20min; result nondet { pass } } }
  actor Ledger {
    executors [p, q]
    capacity 2
    state holder: H = free last_write_wins
    capability respond() -> T { duration fixed 2min; result nondet { pass }; writes holder; finish { holder = taken } }
    capability recheck() -> T { duration fixed 2min; result nondet { pass }; writes holder; finish { holder = taken } }
  }
  actor Party { control party; standing over [end.returned] basis "claim" }
  actor Third { control party; standing over [end.returned] basis "claim3" }
  actor Other { control party; standing over [end.returned] basis "claim2" }
  permit j to Judge.judge
  permit j to Judge.slow
  permit p to Ledger.respond
  permit q to Ledger.respond
  permit p to Ledger.recheck
  permit q to Ledger.recheck
  authority G { basis "decision"; grants j to Judge.judge }
  process Flow { d = Judge.judge(); end returned }
  cut C { include [Judge, Ledger, Party, Other, Third] }
  bounds { cases 1; live_tasks 3; clock 10min }
  verify reachable end.returned
  contract Controls { contest Party on end.returned route Ledger.respond reversible_by [j] respond_by 5min }
  contract Controls3 { contest Third on end.returned route Judge.slow reversible_by [j] respond_by 30min }
  contract Controls2 { contest Other on end.returned route Ledger.recheck reversible_by [j] respond_by 5min }
}"#;

#[test]
fn nl4_found_loss_beats_the_clock_cut_in_both_interpreters() {
    let m = h::model(ROUTE_PAIR_TRUNCATED);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert!(e.truncated, "engine is cut by clock");
    assert!(r.outcome.truncated, "reference is cut by clock");
    let want = vec![("no_lost_update Ledger.holder".to_owned(), json!("violated"))];
    assert_eq!(loss_verdicts(&e, &m), want, "engine");
    assert_eq!(loss_verdicts(&r.outcome, &m), want, "reference");
    let v = json!(report::check(ROUTE_PAIR_TRUNCATED));
    assert_eq!(v["bounds"]["truncated"], true);
    assert_eq!(
        property(&v, "no_lost_update Ledger.holder")["verdict"],
        "violated"
    );
}

/// NL7 and REF4. `last: R = conflict last_write_wins`; claim writes `last`
/// with its result (no finish block = result-valued write), result committed,
/// capacity 2, cases 2, arrivals [0,0]. Calc: both claims run 0..1; the first
/// finish writes last = committed and marks the other; the second finish
/// assigns its result while marked -> loss (a result write replaces the value
/// like a constant). Expected: `no_lost_update Ledger.last` violated, report
/// violated; explain's counterexample for it is replayed by the reference and
/// verified (the reference must also count the result write as an overwrite).
const RESULT_LWW: &str = r#"model C version "0.2" {
  type R = enum { committed, conflict }
  actor Ledger {
    capacity 2
    state last: R = conflict last_write_wins
    capability claim() -> R {
      commits
      duration fixed 1min
      result nondet { committed }
      writes last
    }
  }
  process P {
    r = Ledger.claim()
    end done
  }
  cut C { include [Ledger] }
  bounds { cases 2; live_tasks 1; clock 10min }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  verify reachable end.done
}"#;

#[test]
fn nl7_ref4_result_valued_lww_write_is_an_overwrite() {
    let v = json!(report::check(RESULT_LWW));
    assert_eq!(v["verdict"], "violated", "{v}");
    assert_eq!(
        property(&v, "no_lost_update Ledger.last")["verdict"],
        "violated"
    );
    independent(RESULT_LWW);
    let x = json!(explain::explain(RESULT_LWW));
    assert_eq!(x["verdict"], "violated", "{x}");
    let c = x["explanation"]["counterexamples"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "no_lost_update Ledger.last")
        .unwrap_or_else(|| panic!("no loss counterexample: {x}"));
    assert_eq!(c["status"], "verified", "{c}");
    assert_eq!(c["replay"]["status"], "verified", "{c}");
    let last = c["steps"].as_array().unwrap().last().unwrap();
    assert_eq!(last["kind"], "finish");
    assert_eq!(last["time"], 1);
}

/// NL9. LWW a and b. ab (uses slots:1 of a capacity-1 resource, 2min) writes
/// a and b; aonly (1min) writes a. Picker (1min) chooses one -> ab, two ->
/// aonly. cases 2, arrivals [0,0]. Calc: only ab writes b and two ab calls
/// never overlap (slots), so b is never overwritten while marked -> b holds.
/// Path (one,two): both start at 1, aonly finishes at 2 and marks ab on {a}
/// only; ab finishes at 3 writing a and b -> loss on a, not on b. Expected:
/// `no_lost_update Ledger.a` violated, `no_lost_update Ledger.b` satisfied,
/// report violated, engine and reference equal.
const TWO_LWW: &str = r#"model C version "0.2" {
  type R = enum { committed, conflict }
  type H = enum { free, taken }
  type Pick = enum { one, two }
  resource slots capacity 1
  actor Picker { capacity 2; capability choose() -> Pick { duration fixed 1min; result nondet { one, two } } }
  actor Ledger {
    capacity 2
    state a: H = free last_write_wins
    state b: H = free last_write_wins
    capability ab() -> R {
      uses slots: 1
      commits
      duration fixed 2min
      result nondet { committed }
      writes [a, b]
      finish { a = taken; b = taken }
    }
    capability aonly() -> R {
      duration fixed 1min
      result nondet { committed }
      writes a
      finish { a = taken }
    }
  }
  process P {
    p = Picker.choose()
    if p == one { x = Ledger.ab(); end d1 } else { y = Ledger.aonly(); end d2 }
  }
  cut C { include [Picker, Ledger] }
  bounds { cases 2; live_tasks 1; clock 10min }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  verify reachable end.d1
}"#;

#[test]
fn nl9_loss_is_charged_only_to_the_marked_variable() {
    let v = json!(report::check(TWO_LWW));
    assert_eq!(v["verdict"], "violated", "{v}");
    assert_eq!(
        property(&v, "no_lost_update Ledger.a")["verdict"],
        "violated"
    );
    assert_eq!(
        property(&v, "no_lost_update Ledger.b")["verdict"],
        "satisfied"
    );
    independent(TWO_LWW);
}

/// PC1. Ledger capacity 2, `holder: H = free` with no resolution; claim writes
/// holder only in `start { holder = taken }`. cases 2, arrivals [0,0]. Calc:
/// the start write makes claim a writer of shared holder; claim/claim is an
/// unresolved pair (no lock, version, commutative op or LWW), so lowering
/// refuses it with U-SEM-0 naming both writers and spanning the later
/// capability's declaration (here the only one). Expected: report
/// unsupported; lower fails with exactly one U-SEM-0
/// `unresolved concurrent writes to Ledger.holder by Ledger.claim and
/// Ledger.claim (G5)` whose span is `capability claim(` .. its closing brace.
const START_ONLY_WRITER: &str = r#"model C version "0.2" {
  type R = enum { committed, conflict }
  type H = enum { free, taken }
  actor Ledger {
    capacity 2
    state holder: H = free
    capability claim() -> R {
      start { holder = taken }
      commits
      duration fixed 1min
      result nondet { committed }
    }
  }
  process P {
    r = Ledger.claim()
    end done
  }
  cut C { include [Ledger] }
  bounds { cases 2; live_tasks 1; clock 10min }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  verify reachable end.done
}"#;

#[test]
fn pc1_start_only_writer_is_an_unresolved_pair_at_lowering() {
    let s = START_ONLY_WRITER;
    let v = json!(report::check(s));
    assert_eq!(v["verdict"], "unsupported", "{v}");
    let ds = relca_core::syntax::parse(s)
        .and_then(|a| relca_core::lower::lower(&a))
        .expect_err("refused while lowering, not deferred to the engine");
    let errors: Vec<_> = ds
        .iter()
        .filter(|d| d.severity == relca_core::diag::Severity::Error)
        .collect();
    assert_eq!(errors.len(), 1, "{ds:?}");
    assert_eq!(errors[0].code, "U-SEM-0");
    assert_eq!(
        errors[0].message,
        "unresolved concurrent writes to Ledger.holder by Ledger.claim and Ledger.claim (G5)"
    );
    let start = s.find("capability claim(").unwrap();
    let end = s.find("\n    }\n  }\n  process").unwrap() + "\n    }".len();
    let span = errors[0].span.expect("span of the later capability");
    assert_eq!((span.start, span.end), (start, end));
    let d = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "U-SEM-0")
        .unwrap();
    assert_eq!(d["message"], errors[0].message.as_str());
    assert_eq!(d["span"], json!({"start":start,"end":end}));
}

/// F18 and AG3 on the existing fixtures cf-version and cf-agency.
/// Calc (ADR 0041: a version_check result is decided by the state history at
/// finish, so it is Unknown to Agency like an atomic result; the rule-set word
/// follows ADR 0026/0034 for new syntax):
/// - Neither model has requires, wait, start writes or cancel immediate, and
///   `if r == committed` reads a result, not state. The only route to
///   `agency-p1-requires-1` is the history-dependent result of Ledger.claim.
///   Expected rules = `agency-p1-requires-1`.
/// - `r == committed` cannot be decided symbolically, so neither end is
///   derived: framing.ends = [claimed, retry_needed] both
///   {status not_derived, reason state_guards}, framing.complete = false.
///
/// Control cf-agency.broken-ordinary-result (no version_check, capacity 1,
/// result nondet {committed}): r == committed is the constant true, the rules
/// word stays `agency-p0-1`, claimed is derived reachable, retry_needed is
/// derived unreachable, complete = true.
#[test]
fn f18_ag3_version_result_is_history_dependent_for_agency() {
    for name in ["cf-version", "cf-agency"] {
        let v = json!(report::check(&h::source(name)));
        let a = &v["agency"];
        assert_eq!(a["rules"], "agency-p1-requires-1", "{name}");
        let framing = &a["viewpoints"]["framing"];
        assert_eq!(
            framing["ends"],
            json!([
                {"status":"not_derived","end":"claimed","reason":"state_guards"},
                {"status":"not_derived","end":"retry_needed","reason":"state_guards"}
            ]),
            "{name}"
        );
        assert_eq!(framing["complete"], false, "{name}");
    }
    let v = json!(report::check(&h::source(
        "cf-agency.broken-ordinary-result"
    )));
    let a = &v["agency"];
    assert_eq!(a["rules"], "agency-p0-1");
    let framing = &a["viewpoints"]["framing"];
    assert_eq!(framing["complete"], true);
    let ends = framing["ends"].as_array().unwrap();
    assert_eq!(ends.len(), 2);
    assert_eq!(
        (&ends[0]["end"], &ends[0]["status"], &ends[0]["reachable"]),
        (&json!("claimed"), &json!("derived"), &json!(true))
    );
    assert_eq!(
        (&ends[1]["end"], &ends[1]["status"], &ends[1]["reachable"]),
        (&json!("retry_needed"), &json!("derived"), &json!(false))
    );
}
