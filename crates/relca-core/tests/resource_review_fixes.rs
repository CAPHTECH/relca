//! ADR 0033 review and mutation follow-up acceptance. Expectations are hand-derived.
use relca_core::{engine, explain, lower, reference, report, syntax};
use serde_json::{Value, json};

fn lowered(s: &str) -> relca_core::ir::Model {
    lower::lower(&syntax::parse(s).unwrap()).unwrap()
}
fn checked(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}
fn explained(s: &str) -> Value {
    serde_json::to_value(explain::explain(s)).unwrap()
}
fn edit(s: &str, from: &str, to: &str) -> String {
    assert!(s.contains(from), "edit absent: {from}");
    let t = s.replace(from, to);
    assert_ne!(s, t, "edit did not change the source");
    t
}
fn differential(s: &str) -> relca_core::outcome::Outcome {
    let m = lowered(s);
    let e = engine::explore(&m);
    let mut r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
    r.outcome.diagnostics.drain(..m.diagnostics.len());
    assert_eq!(e, r.outcome, "independent complete Outcome");
    e
}
fn elapsed_work(o: &relca_core::outcome::Outcome) -> ((u64, u64), (u64, u64)) {
    if let Some(cases) = &o.case_outcomes {
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].terminals.len(), 1);
        let t = &cases[0].terminals[0];
        let (e, w) = (t.elapsed.unwrap(), t.work.unwrap());
        return ((e.min, e.max), (w.min, w.max));
    }
    assert_eq!(o.terminals.len(), 1);
    let t = &o.terminals[0];
    ((t.elapsed.min, t.elapsed.max), (t.work.min, t.work.max))
}
fn counterexample<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["explanation"]["counterexamples"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no counterexample {id}: {v}"))
}
fn scalar(n: &str) -> Value {
    json!({"min":n,"max":n,"reason":null})
}
fn range(min: &str, max: &str) -> Value {
    json!({"min":min,"max":max,"reason":null})
}

const OLD_REFUSAL: &str = r#"model OldRefusal version "0.2" {
  type T = enum { ok,denied }
  actor A { capacity 2; capability x() -> T { duration fixed 3min; result nondet { ok } } capability y() -> T { duration fixed 1min; result nondet { ok }; requires flag == true else denied } }
  process P { fork f { a = A.x(); b = A.y() }; join all f; end done }
  cut K { question "old refusal"; include [A] }
  bounds { cases 1; live_tasks 3; clock 10min }
  metrics { window 10min; currency "JPY" }
  observed case { flag: bool = false }
  verify reachable end.done
}
"#;

// b is created by the fork at 0 and refused at 0 (flag stays false); the long
// branch x finishing at 3 is a later seam, not the refusal time. wait = 0 - 0.
#[test]
fn review1_old_model_refusal_wait_is_fixed_at_the_refusal_time() {
    for src in [
        OLD_REFUSAL.to_owned(),
        edit(OLD_REFUSAL, "  bounds", "  arrivals fixed [0min]\n  bounds"),
    ] {
        let v = checked(&src);
        assert_eq!(v["verdict"], "satisfied");
        let tasks = v["metrics"]["cases"][0]["tasks"].as_array().unwrap();
        let b = tasks.iter().find(|t| t["binding"] == "b").unwrap();
        assert_eq!(b["refused_count"], scalar("1"));
        assert_eq!(b["refusal_wait"], scalar("0"), "{b}");
        assert_eq!(b["work"], scalar("0"));
        let a = tasks.iter().find(|t| t["binding"] == "a").unwrap();
        assert_eq!(a["refusal_wait"]["reason"], "not_refused");
        assert_eq!(a["work"], scalar("3"));
        let o = differential(&src);
        assert_eq!(elapsed_work(&o), ((3, 3), (3, 3)));
    }
}

const TWO_CANCELLED: &str = r#"model TwoCancelled version "0.2" {
  type T = enum { pass,fail }
  resource R capacity 2
  resource S capacity 1
  actor A { capacity 3; capability a() -> T { uses R: 1; duration fixed 2min; result nondet { pass }; cancel cooperative ack_within 1min } capability b() -> T { uses R: 1; duration fixed 6min; result nondet { fail }; cancel cooperative ack_within 1min } capability c() -> T { uses S: 1; duration fixed 6min; result nondet { fail }; cancel cooperative ack_within 1min } capability d() -> T { duration fixed 1min; result nondet { pass } } }
  process P { fork f { a = A.a(); b = A.b(); c = A.c() }; join any f where result == pass tie by [a,b,c] remaining cancel_then_drain if none -> done; d = A.d(); end done }
  cut K { question "two cancelled"; include [A] }
  bounds { cases 1; live_tasks 4; clock 10min }
  observed case { flag: bool = false }
  contract C { invariant A.d.finish -> flag == true }
  verify safety no_resource_overcommit
  verify reachable end.done
}
"#;

// a finishes pass at 2 and is adopted; b and c are both still running and get
// cancellation requests at 2 (ack due 2 + 1 = 3). A request is not a release:
// after the edge R holds b (1 of 2) and S holds c (1 of 1).
#[test]
fn review3_bundled_cancel_requests_show_every_cancelled_resource() {
    for src in [
        TWO_CANCELLED.to_owned(),
        edit(
            TWO_CANCELLED,
            "  bounds",
            "  arrivals fixed [0min]\n  bounds",
        ),
    ] {
        let v = explained(&src);
        let c = counterexample(&v, "contract C #1 invariant");
        assert_eq!(c["replay"]["status"], "verified", "{c}");
        let steps = c["steps"].as_array().unwrap();
        let stage = steps
            .iter()
            .find(|s| {
                s["events"]
                    .as_array()
                    .is_some_and(|es| es.iter().any(|e| e["kind"] == "cancel_request"))
            })
            .unwrap();
        assert_eq!(stage["time"], 2);
        assert_eq!(stage["kind"], "cancel_request");
        let requested: Vec<_> = stage["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "cancel_request")
            .map(|e| (e["binding"].clone(), e["ack_at"].clone()))
            .collect();
        assert_eq!(requested, [(json!("b"), json!(3)), (json!("c"), json!(3))]);
        assert_eq!(
            stage["shared_resources"],
            json!([
                {"resource":"R","capacity":2,"occupied":1,"holders":[{"case_id":1,"binding":"b","amount":1}]},
                {"resource":"S","capacity":1,"occupied":1,"holders":[{"case_id":1,"binding":"c","amount":1}]}
            ]),
            "{stage}"
        );
    }
}

const CONTEST_TWO_UNITS: &str = r#"model ContestResource version "0.2" {
type T = enum { ok }
resource R capacity 2
people [j1,j2,p]
actor Judge { executors [j1,j2]; capability work() -> T { uses R: 1; duration fixed 2min; result nondet { ok } } }
actor Route { executors [p]; capability respond() -> T { uses R: 2; duration fixed 1min; result nondet { ok } } }
actor Party { control party; standing over [end.done] basis "claim" }
permit j1 to Judge.work
permit j2 to Judge.work
permit p to Route.respond
authority G { basis "decision"; grants j1 to Judge.work; grants j2 to Judge.work }
process P { x = Judge.work(); end done }
cut K { include [Judge,Route,Party] }
arrivals fixed [0min,0min]
policy fifo tie_by case_id
bounds { cases 2; live_tasks 3; clock 10min }
metrics { window 10min; currency "JPY" }
verify safety no_resource_overcommit
verify reachable end.done
contract C { contest Party on end.done route Route.respond reversible_by [j1] respond_by 8min }
}
"#;

// Judge.work: 1 unit x 2min x 2 cases = 4. Each case's route response holds 2
// units for 1min: none contested -> 4, both contested -> 4 + 2 + 2 = 8.
// Utilization over capacity 2 x window 10: 4/20 = 1/5 .. 8/20 = 2/5. Peak 2
// (two Judge tasks, or one response alone).
#[test]
fn mutation_e09_contest_route_using_two_units_weights_occupancy_by_amount() {
    let v = checked(CONTEST_TWO_UNITS);
    let r = &v["metrics"]["resources"][0];
    assert_eq!(r["resource"], "R");
    assert_eq!(r["capacity"], 2);
    assert_eq!(r["occupancy_time"], range("4", "8"), "{r}");
    assert_eq!(r["utilization"], range("1/5", "2/5"), "{r}");
    assert_eq!(r["peak_units"], scalar("2"), "{r}");
    let overcommit = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "safety no_resource_overcommit")
        .unwrap();
    assert_eq!(overcommit["verdict"], "satisfied");
}

const EXACT_FIT_DEADLOCK: &str = r#"model ExactFitDeadlock version "0.2" {
  type T = enum { ok }
  resource R capacity 1
  actor A { capacity 0; capability x() -> T { uses R: 1; duration fixed 1min; result nondet { ok } } }
  process P { a = A.x(); end done }
  cut K { question "exact fit"; include [A] }
  bounds { cases 1; live_tasks 2; clock 10min }
  verify safety no_resource_overcommit
  verify reachable end.done
}
"#;

// A has no slot, so a never starts. R is free: available 1 - 0 = 1 >= required 1,
// missing 0, so the waiting task has no resource shortage row.
#[test]
fn mutation_e14_actor_blocked_deadlock_with_enough_resource_has_no_shortage() {
    let v = explained(EXACT_FIT_DEADLOCK);
    assert_eq!(v["explanation"]["status"], "complete", "{v}");
    let c = counterexample(&v, "diagnostic:W-DEADLOCK-0");
    assert_eq!(c["replay"]["status"], "verified", "{c}");
    let d = &c["steps"].as_array().unwrap().last().unwrap()["deadlock_detail"];
    assert_eq!(
        d["shared_resources"],
        json!([{"resource":"R","capacity":1,"occupied":0,"holders":[]}])
    );
    assert_eq!(
        d["waiting_tasks"],
        json!([{"case_id":1,"binding":"a","actor":"A"}]),
        "{d}"
    );
    assert_eq!(d["resources"][0]["actor"], "A");
    assert_eq!(d["resources"][0]["capacity"], 0);
}

const SECOND_RESOURCE_BLOCKS: &str = r#"model SecondResourceBlocks version "0.2" {
  type T = enum { ok }
  resource R capacity 1
  resource S capacity 1
  actor A { capacity 2; capability x() -> T { uses S: 1; duration fixed 3min; result nondet { ok } } capability y() -> T { uses R: 1, S: 1; duration fixed 1min; result nondet { ok } } }
  process P { fork f { a = A.x(); b = A.y() }; join all f; end done }
  cut K { question "second resource"; include [A] }
  bounds { cases 1; live_tasks 3; clock 10min }
  metrics { window 10min; currency "JPY" }
  verify safety no_resource_overcommit
  verify reachable end.done
}
"#;

// y needs {R,S}; R is always free but S is held by x, so x and y never overlap.
// x first: x[0,3) y[3,4); y first: y[0,1) x[1,4). Elapsed 4 and work 4 in both.
// R occupancy 1 (1/10), S occupancy 3 + 1 = 4 (2/5), each peak 1.
#[test]
fn mutation_e01_second_resource_of_a_use_set_blocks_the_start() {
    let reversed = edit(
        SECOND_RESOURCE_BLOCKS,
        "uses R: 1, S: 1;",
        "uses S: 1, R: 1;",
    );
    for src in [
        SECOND_RESOURCE_BLOCKS.to_owned(),
        reversed.clone(),
        edit(
            SECOND_RESOURCE_BLOCKS,
            "  bounds",
            "  arrivals fixed [0min]\n  bounds",
        ),
        edit(&reversed, "  bounds", "  arrivals fixed [0min]\n  bounds"),
    ] {
        let o = differential(&src);
        assert_eq!(elapsed_work(&o), ((4, 4), (4, 4)));
        let v = checked(&src);
        assert_eq!(v["verdict"], "satisfied", "{}", v["properties"]);
        let rs = &v["metrics"]["resources"];
        assert_eq!(rs[0]["resource"], "R");
        assert_eq!(rs[0]["occupancy_time"], scalar("1"));
        assert_eq!(rs[0]["utilization"], scalar("1/10"));
        assert_eq!(rs[0]["peak_units"], scalar("1"));
        assert_eq!(rs[1]["resource"], "S");
        assert_eq!(rs[1]["occupancy_time"], scalar("4"));
        assert_eq!(rs[1]["utilization"], scalar("2/5"));
        assert_eq!(rs[1]["peak_units"], scalar("1"), "{rs}");
    }
}
