//! Effect Standing with sets, invariants and explicit limits (tests
//! P14 on the probability graph, P27 / F36, and three explicit-limit joins).
//! P14 itself (sov-atomic and sov-atomic.broken-held) is in the fixture group
//! and runs on both schedulers in effect_standing.rs; here it is repeated on
//! the probability graph. Every expected value below is hand-derived from the
//! model text (minutes) before any run, never copied from output.
#[path = "support/effect_standing.rs"]
mod hand;
use relca_core::{engine, explain, lower, reference, report, syntax};
use serde_json::{Value, json};

fn check(src: &str) -> Value {
    serde_json::to_value(report::check(src)).unwrap()
}

fn property(v: &Value, id: &str) -> Value {
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap_or_else(|| panic!("missing {id}: {v}"))
        .clone()
}

fn verdict(v: &Value, id: &str) -> String {
    property(v, id)["verdict"].as_str().unwrap().into()
}

fn r(a: u64) -> Value {
    json!({"min": a, "max": a})
}

/// Exhaustive engine and the independent reference agree on the whole runtime
/// Outcome (lower diagnostics are only in the reference and the report).
fn both(src: &str) -> Value {
    let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    let e = engine::explore(&m);
    let mut r = reference::explore(&m);
    assert_eq!(
        r.stop,
        reference::StopReason::Complete,
        "no silent reference exclusion"
    );
    assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
    r.outcome.diagnostics.drain(..m.diagnostics.len());
    assert_eq!(e, r.outcome, "engine vs reference");
    check(src)
}

fn arrivals(src: &str) -> String {
    hand::replace(src, "  bounds", "  arrivals fixed [0min]\n  bounds")
}

/// One (filed, target_at, deadline, response_at) tuple per alternative.
fn alternatives(row: &Value) -> Vec<Value> {
    let mut out: Vec<Value> = row["alternatives"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            json!([
                a["filed"],
                a["target_at"],
                a["deadline"],
                a["response_at"],
                a["status"]
            ])
        })
        .collect();
    out.sort_by_key(Value::to_string);
    out
}

fn sorted(mut v: Vec<Value>) -> Vec<Value> {
    v.sort_by_key(Value::to_string);
    v
}

const CONTEST: &str = "contract Controls #1 contest";
const RESPONSE: &str = "contract Controls #1 contest_response";

// ---- P14 on the legacy probability graph (the c4 plan, 3.5 item 5) ----

/// sov-atomic with a harmless finite probability input: approve's fixed 2min
/// is written as the one-point distribution {2min:1}. Times are unchanged
/// (approve0..2, post2..3), so the hand oracle of the fixture pair holds:
/// allow true -> commit3, three alternatives (declined closed3; filed
/// responses4 <= deadline8 with pass/fail), satisfied. allow false -> held
/// finish3 is not a commit, no alternatives, contest_response and root vacuous.
#[test]
fn p14_atomic_reject_finish_is_not_a_commit_on_the_probability_graph() {
    let base = hand::replace(
        &hand::source("sov-atomic"),
        "approve() -> Verdict { duration fixed 2min;",
        "approve() -> Verdict { duration {2min:1};",
    );
    for allow in [true, false] {
        let src = if allow {
            base.clone()
        } else {
            hand::replace(&base, "allow: bool = true", "allow: bool = false")
        };
        assert!(
            lower::lower(&syntax::parse(&src).unwrap())
                .unwrap()
                .uses_probability()
        );
        for arrivals_engine in [false, true] {
            let src = if arrivals_engine {
                arrivals(&src)
            } else {
                src.clone()
            };
            let v = both(&src);
            let at = format!("allow={allow} arrivals={arrivals_engine}");
            assert_eq!(verdict(&v, CONTEST), "satisfied", "{at}: {v}");
            assert_eq!(verdict(&v, "safety no_resource_overcommit"), "satisfied");
            let row = &v["contest_runtime"][0];
            assert_eq!(row["target"], "Ledger.post.commit", "{at}");
            if allow {
                assert_eq!(v["verdict"], "satisfied", "{at}: {v}");
                assert_eq!(verdict(&v, RESPONSE), "satisfied", "{at}");
                assert_eq!(
                    alternatives(row),
                    sorted(vec![
                        json!([false, 3, null, null, "not_filed"]),
                        json!([true, 3, 8, 4, "responded"]),
                        json!([true, 3, 8, 4, "responded"]),
                    ]),
                    "{at}"
                );
            } else {
                assert_eq!(v["verdict"], "vacuous", "{at}: {v}");
                assert_eq!(verdict(&v, RESPONSE), "vacuous", "{at}");
                assert_eq!(alternatives(row), Vec::<Value>::new(), "{at}");
            }
            let terminals = if arrivals_engine {
                v["case_outcomes"][0]["terminals"].clone()
            } else {
                v["terminals"].clone()
            };
            let posted = terminals
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["end"] == "posted")
                .unwrap_or_else(|| panic!("{at}: {terminals}"))
                .clone();
            assert_eq!(posted["work"], r(3), "{at}");
        }
    }
}

// ---- P27 / F36: ADR 0037 state invariant observes the effect route ----

/// E (sov-basic) with an observed case safe = true, a state invariant
/// safe == true, and the route Clerk.recheck writing safe on finish.
fn p27(value: &str) -> String {
    let s = hand::replace(
        &hand::source("sov-basic"),
        "  actor Approver",
        "  observed case { safe: bool = true }\n  actor Approver",
    );
    let s = hand::replace(
        &s,
        "capability recheck() -> Verdict { duration fixed 1min; result nondet { pass, fail } }",
        &format!(
            "capability recheck() -> Verdict {{ duration fixed 1min; result nondet {{ pass, fail }}; writes safe; finish {{ safe = {value} }} }}"
        ),
    );
    hand::replace(
        &s,
        "contract Controls { ",
        "contract Controls { invariant safe == true; ",
    )
}

const P27_INVARIANT: &str = "contract Controls #1 invariant";
const P27_CONTEST: &str = "contract Controls #2 contest";
const P27_RESPONSE: &str = "contract Controls #2 contest_response";

/// Positive: approve0..2; fail -> End returned2/2. pass -> post2..3, commit3,
/// End posted3/work3. Filed recheck3..4 writes safe = true at its response4
/// (after the business End): the invariant holds everywhere; deadline8.
/// Broken (finish safe = false, the one edit): the filed branch writes false
/// at4, one minute after End posted3. The state invariant is violated (root
/// violated, exit1); contest_response stays satisfied (4 <= 8); End and work
/// are unchanged. F36 (route writes outside the invariant monitor) would make
/// the broken side satisfied.
#[test]
fn p27_state_invariant_observes_the_effect_route_after_the_business_end() {
    for arrivals_engine in [false, true] {
        for value in ["true", "false"] {
            let src = if arrivals_engine {
                arrivals(&p27(value))
            } else {
                p27(value)
            };
            let at = format!("safe={value} arrivals={arrivals_engine}");
            let v = both(&src);
            let good = value == "true";
            assert_eq!(
                v["verdict"],
                if good { "satisfied" } else { "violated" },
                "{at}: {v}"
            );
            assert_eq!(
                verdict(&v, P27_INVARIANT),
                if good { "satisfied" } else { "violated" },
                "{at}"
            );
            assert_eq!(verdict(&v, P27_CONTEST), "satisfied", "{at}");
            assert_eq!(verdict(&v, P27_RESPONSE), "satisfied", "{at}");
            assert_eq!(
                alternatives(&v["contest_runtime"][0]),
                sorted(vec![
                    json!([false, 3, null, null, "not_filed"]),
                    json!([true, 3, 8, 4, "responded"]),
                    json!([true, 3, 8, 4, "responded"]),
                ]),
                "{at}"
            );
            let terminals = if arrivals_engine {
                v["case_outcomes"][0]["terminals"].clone()
            } else {
                v["terminals"].clone()
            };
            let ends: Vec<Value> = terminals
                .as_array()
                .unwrap()
                .iter()
                .map(|t| json!([t["end"], t["work"]]))
                .collect();
            assert_eq!(
                sorted(ends),
                sorted(vec![json!(["returned", r(2)]), json!(["posted", r(3)])]),
                "{at}"
            );
        }
    }
}

/// The broken P27 counterexample must contain the route's response Finish at
/// 4 (the write of safe = false), after the business End at3, and replay as
/// verified on both schedulers.
#[test]
fn p27_explain_counterexample_includes_the_response_finish_and_replays() {
    for arrivals_engine in [false, true] {
        let src = if arrivals_engine {
            arrivals(&p27("false"))
        } else {
            p27("false")
        };
        let x = serde_json::to_value(explain::explain(&src)).unwrap();
        let c = x["explanation"]["counterexamples"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == P27_INVARIANT)
            .unwrap_or_else(|| panic!("arrivals={arrivals_engine}: {x}"))
            .clone();
        assert_eq!(c["status"], "verified", "arrivals={arrivals_engine}: {c}");
        assert_eq!(c["replay"]["status"], "verified");
        let steps = c["steps"].as_array().unwrap();
        let end = steps
            .iter()
            .position(|s| s["kind"] == "end" && s["time"] == 3)
            .unwrap_or_else(|| panic!("End3: {c}"));
        let response = steps
            .iter()
            .position(|s| s["kind"] == "contest_response" && s["time"] == 4)
            .unwrap_or_else(|| panic!("response4: {c}"));
        assert!(end < response, "{c}");
    }
}

// ---- B4 joins (ADR 0039) ----

/// B4 1 (P13 for effects): with a declared overflow terminal, Standing on it
/// beside an effect target is U-SEM-0 (ADR 0039 / ADR 0040 scope), not
/// E-NAME-0 for an unknown End, and not accepted.
#[test]
fn b4_standing_on_the_overflow_terminal_beside_an_effect_is_unsupported() {
    let base = hand::replace(
        &hand::source("sov-basic"),
        "  bounds { cases 1;",
        "  arrivals fixed [0min, 0min]\n  policy fifo tie_by case_id\n  concurrent cases 1 on_overflow end spill\n  bounds { cases 2;",
    );
    let standing = hand::replace(
        &base,
        "standing over [Ledger.post.commit]",
        "standing over [Ledger.post.commit, end.spill]",
    );
    let contested = hand::replace(
        &standing,
        "respond_by 5min }",
        "respond_by 5min; contest Vendor on end.spill route Clerk.recheck reversible_by [a1] respond_by 5min }",
    );
    for src in [standing, contested] {
        let v = check(&src);
        assert_eq!(v["verdict"], "unsupported", "{v}");
        let diagnostics = v["diagnostics"].as_array().unwrap();
        assert!(
            diagnostics.iter().any(|d| d["code"] == "U-SEM-0"
                && d["message"] == "Standing on an overflow terminal is unsupported"),
            "{v}"
        );
        assert!(!diagnostics.iter().any(|d| d["code"] == "E-NAME-0"), "{v}");
    }
    // The control: without the overflow terminal in the Standing, the same
    // two-case model is accepted.
    assert_ne!(check(&base)["verdict"], "unsupported");
}

/// Two cases, concurrent1 wait_external fifo. Ledger.post has two values
/// (self candidate, l1 via PostGrant). Clerk c1 runs note (1min) and the route
/// recheck (3min); Desk.work is 2min on its own Actor.
const TWO_CASES: &str = r#"model StandingEffects version "0.2" {
  type Verdict = enum { pass, fail }
  type Ack = enum { committed, held }
  type Done = enum { ok }
  people [c1, cfo, l1]
  actor Ledger { executors [l1]; capacity 1; capability post() -> Ack { commits; duration fixed 1min; result nondet { committed, held } } }
  actor Desk { capacity 1; capability work() -> Done { duration fixed 2min; result nondet { ok } } }
  actor Clerk { executors [c1]; capability note() -> Done { duration fixed 1min; result nondet { ok } }; capability recheck() -> Verdict { duration fixed 3min; result nondet { pass, fail } } }
  actor Vendor { control party; standing over [Ledger.post.commit] basis "posting claim" }
  authority PostGrant { basis "posting rule"; grants l1 to Ledger.post; issued_by cfo }
  process Flow { n = Clerk.note(); ack = Ledger.post(); w = Desk.work(); end posted }
  cut Scope { include [Ledger, Desk, Clerk, Vendor] }
  arrivals fixed [0min, 0min]
  policy fifo tie_by case_id
  concurrent cases 1 on_overflow wait_external fifo
  bounds { cases 2; live_tasks 3; clock 30min }
  verify safety no_resource_overcommit
  contract Controls { contest Vendor on Ledger.post.commit route Clerk.recheck reversible_by [l1] respond_by 5min }
}
"#;

fn completed(v: &Value, case: usize) -> Value {
    let t = &v["case_outcomes"][case]["terminals"][0];
    json!([t["end"], t["completed_at"], t["work"]])
}

/// B4 2: case1 note0..1, post1..2 (commit2), work2..4, End posted4 (work4).
/// Filed: recheck2..5 on c1, response5 <= deadline7. The business End at4
/// releases the concurrent slot, so case2 is admitted at4 on every path, not
/// at case1's execution close5. If case1 filed, c1 is held by the route until
/// 5: case2 note5..6, post6..7 (commit7), work7..9, End9; its own route7..10
/// <= 12. If case1 declined: note4..5, post5..6 (commit6), work6..8, End8;
/// route6..9 <= 11. Case2 completed_at 8..9, work4. Both cases are open at
/// their End on a filed branch: business_ended_open 0..2.
#[test]
fn b4_business_end_releases_the_slot_while_the_route_keeps_its_person() {
    let v = both(TWO_CASES);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(verdict(&v, CONTEST), "satisfied");
    assert_eq!(verdict(&v, RESPONSE), "satisfied");
    let admitted: Vec<Value> = v["case_outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["admitted_at"].clone())
        .collect();
    assert_eq!(admitted, [r(0), r(4)], "{v}");
    assert_eq!(completed(&v, 0), json!(["posted", r(4), r(4)]));
    assert_eq!(
        completed(&v, 1),
        json!(["posted", {"min": 8, "max": 9}, r(4)])
    );
    let rows = v["contest_runtime"].as_array().unwrap();
    let case = |k: u64| {
        rows.iter()
            .find(|row| row["case_id"] == k)
            .unwrap_or_else(|| panic!("case{k}: {v}"))
    };
    let mut one = vec![json!([false, 2, null, null, "not_filed"])];
    one.extend(vec![json!([true, 2, 7, 5, "responded"]); 2]);
    assert_eq!(alternatives(case(1)), sorted(one));
    let mut two = vec![
        json!([false, 6, null, null, "not_filed"]),
        json!([false, 7, null, null, "not_filed"]),
    ];
    two.extend(vec![json!([true, 6, 11, 9, "responded"]); 2]);
    two.extend(vec![json!([true, 7, 12, 10, "responded"]); 2]);
    assert_eq!(alternatives(case(2)), sorted(two));
    assert_eq!(
        v["contest_summary"]["business_ended_open"],
        json!({"min": "0", "max": "2", "reason": null})
    );
}

/// B4 3: the commit at the same instant as the End. Process note, work, post:
/// case1 note0..1, work1..3, post3..4 (commit4), End posted4/work4. Filing is
/// decided at the commit (same tick) before the End releases the slot and
/// case2 is admitted at4. If filed, the route is queued at4 before case2's
/// note (queue order and tie_by case_id both put case1 first): recheck4..7,
/// response7 <= 9; case2 note7..8, work8..10, post10..11 (commit11), End11;
/// route11..14 <= 16. If declined: case2 note4..5, work5..7, post7..8
/// (commit8), End8; route8..11 <= 13.
#[test]
fn b4_same_tick_filing_is_stable_before_readmission() {
    let src = hand::replace(
        TWO_CASES,
        "n = Clerk.note(); ack = Ledger.post(); w = Desk.work();",
        "n = Clerk.note(); w = Desk.work(); ack = Ledger.post();",
    );
    let v = both(&src);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    let admitted: Vec<Value> = v["case_outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["admitted_at"].clone())
        .collect();
    assert_eq!(admitted, [r(0), r(4)], "{v}");
    assert_eq!(completed(&v, 0), json!(["posted", r(4), r(4)]));
    assert_eq!(
        completed(&v, 1),
        json!(["posted", {"min": 8, "max": 11}, r(4)])
    );
    let rows = v["contest_runtime"].as_array().unwrap();
    let case = |k: u64| rows.iter().find(|row| row["case_id"] == k).unwrap();
    let mut one = vec![json!([false, 4, null, null, "not_filed"])];
    one.extend(vec![json!([true, 4, 9, 7, "responded"]); 2]);
    assert_eq!(alternatives(case(1)), sorted(one));
    let mut two = vec![
        json!([false, 8, null, null, "not_filed"]),
        json!([false, 11, null, null, "not_filed"]),
    ];
    two.extend(vec![json!([true, 8, 13, 11, "responded"]); 2]);
    two.extend(vec![json!([true, 11, 16, 14, "responded"]); 2]);
    assert_eq!(alternatives(case(2)), sorted(two));
}

/// P27 on two cases (the M1 shape of ADR 0039: a case kept out of the state
/// monitor). TWO_CASES with an observed case safe = true, a state invariant
/// safe == true, and the route writing safe = false on finish. Case1's route
/// responds at5, after its business End4; case2's at9 or 10, after its End8
/// or 9. Each case's row is violated on its filed branch, and both cases
/// have one: both rows violated, root violated. With safe = true both rows
/// are satisfied.
#[test]
fn p27_each_business_ended_case_stays_in_the_state_monitor() {
    for value in ["true", "false"] {
        let s = hand::replace(
            TWO_CASES,
            "  people",
            "  observed case { safe: bool = true }\n  people",
        );
        let s = hand::replace(
            &s,
            "capability recheck() -> Verdict { duration fixed 3min; result nondet { pass, fail } }",
            &format!(
                "capability recheck() -> Verdict {{ duration fixed 3min; result nondet {{ pass, fail }}; writes safe; finish {{ safe = {value} }} }}"
            ),
        );
        let s = hand::replace(
            &s,
            "contract Controls { ",
            "contract Controls { invariant safe == true; ",
        );
        let v = both(&s);
        let want = if value == "true" {
            "satisfied"
        } else {
            "violated"
        };
        assert_eq!(v["verdict"], want, "safe={value}: {v}");
        let p = property(&v, P27_INVARIANT);
        assert_eq!(p["verdict"], want);
        let rows: Vec<Value> = p["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["verdict"].clone())
            .collect();
        assert_eq!(rows, [json!(want), json!(want)], "safe={value}: {p}");
        assert_eq!(verdict(&v, P27_RESPONSE), "satisfied");
    }
}
