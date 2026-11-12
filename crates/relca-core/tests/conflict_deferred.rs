//! ADR 0041 integration onto revision 062c3d9: the deferred integration
//! items that depend on ADR 0039 (A25), ADR 0040 (A26, F03) and ADR 0038 (A4
//! type integration). Inputs are inline, so the closed cf-* population is
//! unchanged (precedent: record_deferred.rs, effect_standing_deferred.rs).
//! Every expected value below is hand-derived from the model text (minutes)
//! before the run and written in the comment above the test; nothing is copied
//! from output.
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};

fn check(src: &str) -> Value {
    serde_json::to_value(report::check(src)).unwrap()
}

fn change(s: &str, from: &str, to: &str) -> String {
    let t = s.replace(from, to);
    assert_ne!(s, t, "replacement must affect the control: {from}");
    t
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

fn errors(v: &Value) -> Vec<(String, String)> {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .map(|d| {
            (
                d["code"].as_str().unwrap().to_owned(),
                d["message"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// Exhaustive engine and the independent reference agree on the whole runtime
/// Outcome (lower diagnostics are only in the reference and the report), and
/// the reference finished its enumeration.
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

/// (end, completed_at, elapsed, work) of every terminal of one case.
fn case_terminals(v: &Value, case: u64) -> Vec<Value> {
    let c = v["case_outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["case_id"] == case)
        .unwrap_or_else(|| panic!("missing case {case}: {v}"));
    let mut out: Vec<Value> = c["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| json!([t["end"], t["completed_at"], t["elapsed"], t["work"]]))
        .collect();
    out.sort_by_key(Value::to_string);
    out
}

/// (filed, target_at, response_at, status, deadline, route_work, result) per
/// alternative of one contest runtime row.
fn alternatives(v: &Value, case: u64, target: &str) -> Vec<Value> {
    let rows: Vec<&Value> = v["contest_runtime"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["case_id"] == case && row["target"] == target)
        .collect();
    assert_eq!(rows.len(), 1, "one runtime row for {target}: {v}");
    let mut out: Vec<Value> = rows[0]["alternatives"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            json!([
                a["filed"],
                a["target_at"],
                a["response_at"],
                a["status"],
                a["deadline"],
                a["route_work"],
                a["result"]
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

// ---------------------------------------------------------------------------
// A25 (ADR 0039, deferred F01・F29): external admission is not Task start.
// cf-silent with Ledger capacity 2 and `concurrent cases 1 on_overflow
// wait_external fifo`, arrivals [0,0], claim 1min, version_check [holder].
// Hand table (minutes):
//   case | receipt | admitted | start | finish | work | result
//     1  |    0    |    0     |   0   |   1    |  1   | committed
//     2  |    0    |    1     |   1   |   2    |  1   | committed
// Case 2 is Held from 0 to 1 and starts only after case 1 committed at 1, so
// the version it reads at its actual start already includes case 1's write: no
// conflict, claims reaches 2 (its upper bound), root satisfied, exit 0.
// External wait counts in elapsed: case 2 elapsed 2, work 1.
// ---------------------------------------------------------------------------
const A25: &str = r#"model Conflict version "0.2" {
  type R = enum { committed, conflict }
  type H = enum { free, taken }
  actor Ledger {
    capacity 2
    state holder: H = free
    state claims: 0..2 = 0
    capability claim() -> R {
      version_check [holder] else conflict
      commits
      duration fixed 1min
      result nondet { committed }
      writes [holder, claims]
      finish { holder = taken; claims += 1 }
    }
  }
  process P {
    r = Ledger.claim()
    if r == committed { end claimed } else { end retry_needed }
  }
  cut C { include [Ledger] }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  concurrent cases 1 on_overflow wait_external fifo
  bounds { cases 2; live_tasks 1; clock 10min }
  verify reachable end.claimed
}
"#;

#[test]
fn a25_external_admission_is_not_the_version_origin() {
    let v = both(A25);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(errors(&v), Vec::<(String, String)>::new());
    assert_eq!(verdict(&v, "reachable end.claimed"), "satisfied");
    let rows: Vec<Value> = property(&v, "reachable end.claimed")["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| json!([c["case_id"], c["verdict"]]))
        .collect();
    assert_eq!(rows, [json!([1, "satisfied"]), json!([2, "satisfied"])]);
    // claims += 1 twice stays within 0..2: no range row, no refusal.
    assert_eq!(verdict(&v, "state_in_range"), "satisfied");
    assert!(v.get("state_range_violations").is_none(), "{v}");
    assert!(v.get("capability_refusals").is_none(), "{v}");
    let admitted: Vec<Value> = v["case_outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| json!([c["case_id"], c["arrival"], c["admitted_at"]]))
        .collect();
    assert_eq!(admitted, [json!([1, 0, r(0)]), json!([2, 0, r(1)])]);
    assert_eq!(
        case_terminals(&v, 1),
        [json!(["claimed", r(1), r(1), r(1)])]
    );
    assert_eq!(
        case_terminals(&v, 2),
        [json!(["claimed", r(2), r(2), r(1)])]
    );
    assert_eq!(
        v["admission"]["external_wait_max"],
        r(1),
        "case 2 waits externally while case 1 runs: {v}"
    );
}

/// Static concurrency is decided by the owner capacity (2), not by the
/// external limit (1): without version_check the holder self-pair is the
/// unresolved G5 pair (claims += is commutative), U-SEM-0, exit 4.
#[test]
fn a25_static_pairs_still_follow_owner_capacity_under_an_external_limit() {
    let s = change(A25, "      version_check [holder] else conflict\n", "");
    let v = check(&s);
    assert_eq!(v["verdict"], "unsupported", "{v}");
    assert_eq!(
        errors(&v),
        [(
            "U-SEM-0".to_owned(),
            "unresolved concurrent writes to Ledger.holder by Ledger.claim and Ledger.claim (G5)"
                .to_owned()
        )]
    );
}

// ---------------------------------------------------------------------------
// A26 (ADR 0040, deferred F03・F30, Q1): one-case normal/route interference.
// Authority/Standing base copied from sov-concurrent (people, permit, the
// approve decision that controls the target, ApprovalGrant). Ledger capacity 2
// owns both writers of the shared `holder`; each has version_check [holder]
// else conflict and no start effect. The prior effect Prior.post.commit opens
// the contest; the route is Ledger.route.
// Hand (minutes). The deferred note wrote commit1/start1/conflict4/work4; it
// omits the 2min approve that the sov-concurrent base puts first, so every
// time below is that note +2:
//   approve 0..2 (a1). fail -> end returned at 2 (work 2).
//   pass: Prior.post 2..3 commits at 3 -> filing is one nondeterministic choice.
//     not filed: normal 3..6 commits, holder = taken -> end claimed at 6
//                (work 2+1+3 = 6).
//     filed: route and normal both start at 3 (capacity 2). route 3..4
//            commits, holder = routed at 4, marks the running normal stale;
//            normal finishes at 6 with the reject value, no write and no
//            Commit -> end conflicted at 6. Terminal work = 2+1+3 = 6; the
//            route's 1min is excluded from the business terminal and is the
//            alternative's route_work. Response at 4, deadline 3+5 = 8.
// Both ends are reachable; the contest is structurally satisfied -> exit 0.
// ---------------------------------------------------------------------------
const A26: &str = r#"model StandingConflict version "0.2" {
  type Verdict = enum { pass, fail }
  type R = enum { committed, conflict }
  type H = enum { free, taken, routed }
  people [c1, a1, cfo]
  actor Approver { executors [a1]; capability approve() -> Verdict { duration fixed 2min; result nondet { pass, fail } } }
  actor Prior { capacity 1; capability post() -> R { commits; duration fixed 1min; result nondet { committed } } }
  actor Ledger {
    capacity 2
    state holder: H = free
    capability normal() -> R {
      version_check [holder] else conflict
      commits
      duration fixed 3min
      result nondet { committed }
      writes holder
      finish { holder = taken }
    }
    capability route() -> R {
      version_check [holder] else conflict
      commits
      duration fixed 1min
      result nondet { committed }
      writes holder
      finish { holder = routed }
    }
  }
  actor Vendor { control party; standing over [Prior.post.commit] basis "posting claim" }
  permit a1 to Approver.approve
  authority ApprovalGrant { basis "approval table"; grants a1 to Approver.approve; issued_by cfo }
  process Flow {
    approval = Approver.approve()
    if approval == fail { end returned } else { }
    p = Prior.post()
    n = Ledger.normal()
    if n == committed { end claimed } else { end conflicted }
  }
  cut Scope { include [Approver, Prior, Ledger, Vendor] }
  bounds { cases 1; live_tasks 2; clock 20min }
  verify safety no_resource_overcommit
  verify reachable end.claimed
  verify reachable end.conflicted
  contract Controls { contest Vendor on Prior.post.commit route Ledger.route reversible_by [a1] respond_by 5min }
}
"#;

const CONTEST: &str = "contract Controls #1 contest";

fn terminals(v: &Value) -> Vec<Value> {
    sorted(
        v["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| json!([t["end"], t["elapsed"], t["work"]]))
            .collect(),
    )
}

#[test]
fn a26_issued_route_write_rejects_the_same_case_normal_writer() {
    let v = both(A26);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(errors(&v), Vec::<(String, String)>::new());
    for id in [
        "safety no_resource_overcommit",
        "reachable end.claimed",
        "reachable end.conflicted",
        CONTEST,
    ] {
        assert_eq!(verdict(&v, id), "satisfied", "{id}: {v}");
    }
    assert_eq!(
        terminals(&v),
        sorted(vec![
            json!(["returned", r(2), r(2)]),
            json!(["claimed", r(6), r(6)]),
            json!(["conflicted", r(6), r(6)]),
        ])
    );
    assert_eq!(
        alternatives(&v, 1, "Prior.post.commit"),
        sorted(vec![
            json!([false, 3, null, "not_filed", null, null, null]),
            json!([true, 3, 4, "responded", 8, r(1), "committed"]),
        ])
    );
}

/// LWW counterpart: no version_check, `holder` marked last_write_wins.
/// Not filed: normal commits at 6, no other receipt -> claimed at 6.
/// Filed: the route's write at 4 is a receipt while the normal runs; the
/// normal's constant overwrite at 6 is a lost update. Both branches end
/// claimed at 6 (work 6); `no_lost_update Ledger.holder` is violated (exit 1)
/// although the model has one case.
#[test]
fn a26_lww_counterpart_loses_the_route_write_in_one_case() {
    let s = A26.replace("      version_check [holder] else conflict\n", "");
    assert_eq!(A26.matches("version_check").count(), 2);
    let s = change(
        &s,
        "state holder: H = free",
        "state holder: H = free last_write_wins",
    );
    let s = change(&s, "  verify reachable end.conflicted\n", "");
    let v = both(&s);
    assert_eq!(v["verdict"], "violated", "{v}");
    assert_eq!(errors(&v), Vec::<(String, String)>::new());
    assert_eq!(verdict(&v, "no_lost_update Ledger.holder"), "violated");
    assert_eq!(verdict(&v, "reachable end.claimed"), "satisfied");
    assert_eq!(verdict(&v, CONTEST), "satisfied");
    assert_eq!(
        terminals(&v),
        sorted(vec![
            json!(["returned", r(2), r(2)]),
            json!(["claimed", r(6), r(6)]),
        ])
    );
    // Without the filing there is no receipt: the loss belongs to the route.
    let quiet = change(
        &s,
        "  contract Controls { contest Vendor on Prior.post.commit route Ledger.route reversible_by [a1] respond_by 5min }\n",
        "",
    );
    let quiet = change(
        &quiet,
        "  actor Vendor { control party; standing over [Prior.post.commit] basis \"posting claim\" }\n",
        "",
    );
    let quiet = change(&quiet, ", Vendor]", "]");
    let q = both(&quiet);
    assert_eq!(
        verdict(&q, "no_lost_update Ledger.holder"),
        "satisfied",
        "{q}"
    );
}

// ---------------------------------------------------------------------------
// F03 (ADR 0040 x ADR 0041): a version-rejected Finish is not a Commit, so it
// never opens a standing-over effect target. A26 plus a second Standing over
// Ledger.normal.commit, contested with route Clerk.recheck (c1, 1min).
// Hand (minutes), approval pass:
//   contest 1 not filed: normal commits at 6 -> contest 2 target at 6;
//     not filed, or filed: recheck 6..7 responded at 7, deadline 11, with
//     result pass or fail (recheck has two results: two filed alternatives).
//   contest 1 filed: route 3..4 writes at 4, normal rejected at 6 -> no
//     Commit, no contest 2 target on that branch.
// Contest 2 alternatives: (false,6,-,not_filed) and (true,6,7,responded,11)
// once per recheck result.
// Whole-execution work differs by branch: the rejected branch has no recheck.
// Engine (Capability::commits_result) and the independent reference must
// agree on the whole Outcome, including the branch without a target.
// ---------------------------------------------------------------------------
#[test]
fn f03_version_rejected_finish_opens_no_standing_over_target() {
    let s = change(
        A26,
        "standing over [Prior.post.commit]",
        "standing over [Prior.post.commit, Ledger.normal.commit]",
    );
    let s = change(
        &s,
        "  permit a1",
        "  actor Clerk { executors [c1]; capability recheck() -> Verdict { duration fixed 1min; result nondet { pass, fail } } }\n  permit a1",
    );
    let s = change(&s, "Ledger, Vendor]", "Ledger, Clerk, Vendor]");
    let s = change(
        &s,
        " respond_by 5min }",
        " respond_by 5min; contest Vendor on Ledger.normal.commit route Clerk.recheck reversible_by [a1] respond_by 5min }",
    );
    let s = change(
        &s,
        "  bounds {",
        "  metrics { window 20min; currency \"JPY\" }\n  bounds {",
    );
    let v = both(&s);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(verdict(&v, "contract Controls #2 contest"), "satisfied");
    assert_eq!(
        alternatives(&v, 1, "Ledger.normal.commit"),
        sorted(vec![
            json!([false, 6, null, "not_filed", null, null, null]),
            json!([true, 6, 7, "responded", 11, r(1), "pass"]),
            json!([true, 6, 7, "responded", 11, r(1), "fail"]),
        ])
    );
    assert_eq!(
        alternatives(&v, 1, "Prior.post.commit"),
        sorted(vec![
            json!([false, 3, null, "not_filed", null, null, null]),
            json!([true, 3, 4, "responded", 8, r(1), "committed"]),
        ])
    );
}

// ---------------------------------------------------------------------------
// A4 type integration (ADR 0038 x ADR 0041). Expected classifications come
// from ADR 0038 (record literal positions are U-SYN-0, a name that is not a
// member of the result type is E-TYPE-0, record Actor state is U-SEM-0
// `Actor record state is unsupported`) and from ADR 0041 (the reject value of
// version_check is a result value of the Capability's own result type).
// ---------------------------------------------------------------------------
const CF_VERSION: &str = include_str!("../../../fixtures/cf-version/model.relca");

/// cf-version with the named result type replaced by the same inline enum.
/// The bare reject `conflict` is a member of the inline result enum, so the
/// model means the same: identical verdict, property rows and case outcomes
/// (the fixture's hand oracle: both cases start 0, one commits at 1, the other
/// finishes conflict at 1, both orders).
#[test]
fn a4_inline_enum_result_accepts_a_bare_conflict_reject() {
    let inline = change(
        CF_VERSION,
        "capability claim() -> R {",
        "capability claim() -> enum { committed, conflict } {",
    );
    let inline = change(&inline, "  type R = enum { committed, conflict }\n", "");
    let a = both(&inline);
    let b = both(CF_VERSION);
    assert_eq!(a["verdict"], "satisfied", "{a}");
    assert_eq!(errors(&a), Vec::<(String, String)>::new());
    assert_eq!(a["properties"], b["properties"]);
    assert_eq!(a["case_outcomes"], b["case_outcomes"]);
    for case in [1, 2] {
        assert_eq!(
            case_terminals(&a, case),
            sorted(vec![
                json!(["claimed", r(1), r(1), r(1)]),
                json!(["retry_needed", r(1), r(1), r(1)]),
            ])
        );
    }
}

const RECORD_RESULT: &str = r#"model Conflict version "0.2" {
  type H = enum { free, taken }
  type Entry = record { confidence: enum { high, low } }
  actor Ledger {
    capacity 2
    state holder: H = free
    capability claim() -> Entry {
      version_check [holder] REJECT
      commits
      duration fixed 1min
      result nondet { Entry { confidence = high } }
      writes holder
      finish { holder = taken }
    }
  }
  process P {
    r = Ledger.claim()
    if r.confidence == high { end claimed } else { end retry_needed }
  }
  cut C { include [Ledger] }
  bounds { cases 2; live_tasks 1; clock 10min }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  verify reachable end.claimed
}
"#;

/// A record literal after `else` is a record literal in a value position that
/// ADR 0038 does not open: U-SYN-0 at parse, nothing else, unsupported.
#[test]
fn a4_record_literal_reject_value_is_unsupported_syntax() {
    let s = change(RECORD_RESULT, "REJECT", "else Entry { confidence = low }");
    let v = check(&s);
    assert_eq!(v["verdict"], "unsupported", "{v}");
    let codes: Vec<String> = errors(&v).into_iter().map(|(c, _)| c).collect();
    assert_eq!(codes, ["U-SYN-0"], "{v}");
    assert_eq!(v["properties"], json!([]));
}

/// A naked enum member as the reject of a record-typed Capability is not a
/// value of the result type: E-TYPE-0, invalid (exit 3), never accepted.
#[test]
fn a4_naked_member_reject_of_a_record_result_is_a_type_error() {
    let s = change(RECORD_RESULT, "REJECT", "else conflict");
    let v = check(&s);
    assert_eq!(v["verdict"], "invalid", "{v}");
    let codes: Vec<String> = errors(&v).into_iter().map(|(c, _)| c).collect();
    assert!(codes.iter().any(|c| c == "E-TYPE-0"), "{v}");
    assert_eq!(v["properties"], json!([]));
}

/// Record Actor state stays unsupported from ADR 0038 even when a
/// version_check names it (the check does not open a new record path).
#[test]
fn a4_record_actor_state_under_version_check_stays_unsupported() {
    let s = r#"model Conflict version "0.2" {
  type R = enum { committed, conflict }
  type Entry = record { confidence: enum { high, low } }
  actor Ledger {
    capacity 2
    state s: Entry = Entry { confidence = high }
    capability claim() -> R {
      version_check [s] else conflict
      commits
      duration fixed 1min
      result nondet { committed }
    }
  }
  process P {
    r = Ledger.claim()
    if r == committed { end claimed } else { end retry_needed }
  }
  cut C { include [Ledger] }
  bounds { cases 2; live_tasks 1; clock 10min }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  verify reachable end.claimed
}
"#;
    let v = check(s);
    assert_eq!(v["verdict"], "unsupported", "{v}");
    let e = errors(&v);
    assert!(
        e.iter()
            .any(|(c, m)| c == "U-SEM-0" && m == "Actor record state is unsupported"),
        "{v}"
    );
    assert!(e.iter().all(|(c, _)| c.starts_with("U-")), "{v}");
    assert_eq!(v["properties"], json!([]));
}

/// Remove every "span" object so that reports of sources that differ only by
/// an inserted unused declaration can be compared (the caller also blanks the
/// source hash in `model.sha256`).
fn without_spans(v: &mut Value) {
    match v {
        Value::Object(o) => {
            o.remove("span");
            o.values_mut().for_each(without_spans);
        }
        Value::Array(a) => a.iter_mut().for_each(without_spans),
        _ => {}
    }
}

/// `last_write_wins` and `version_check` stay ordinary identifiers where a
/// declaration name is expected, across ADR 0038's default-value lookahead:
/// after `= false` and a newline, `last_write_wins: bool` starts the next
/// observed variable (no LWW marker, no no_lost_update row), and a state
/// named `version_check` is a state. An unused record declaration does not
/// change the reading.
#[test]
fn a4_marker_words_after_a_default_and_newline_remain_declaration_names() {
    let s = r#"model Words version "0.2" {
  type T = enum { ok }
  observed case {
    flag: bool = false
    last_write_wins: bool = true
  }
  actor A {
    capacity 1
    state version_check: bool = false
    capability run() -> T { duration fixed 1min; result nondet { ok }; writes [flag, version_check]; finish { flag = true; version_check = true } }
  }
  process P { r = A.run(); if last_write_wins == true { end done } else { end other } }
  cut C { include [A] }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify reachable end.done
}
"#;
    let mut a = check(s);
    assert_eq!(a["verdict"], "satisfied", "{a}");
    assert_eq!(errors(&a), Vec::<(String, String)>::new());
    assert!(
        a["properties"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| !p["id"].as_str().unwrap().starts_with("no_lost_update")),
        "{a}"
    );
    let with_record = change(
        s,
        "  type T = enum { ok }\n",
        "  type T = enum { ok }\n  type Unused = record { level: enum { high, low } }\n",
    );
    let mut b = check(&with_record);
    without_spans(&mut a);
    without_spans(&mut b);
    // The source hash differs by construction; the model name does not.
    assert_ne!(a["model"]["sha256"], b["model"]["sha256"]);
    a["model"]["sha256"] = Value::Null;
    b["model"]["sha256"] = Value::Null;
    assert_eq!(a, b);
}

// ---------------------------------------------------------------------------
// Integration gaps found by enumerating the ADR 0041 paths against ADR 0040
// (same type as the earlier integrations: one side keeps the old rule).
// ---------------------------------------------------------------------------

/// Every counterexample of an explained report replays as verified.
fn replays_verified(src: &str) -> Value {
    let v = serde_json::to_value(relca_core::explain::explain(src)).unwrap();
    assert_eq!(v["verdict"], "violated", "{v}");
    assert_eq!(v["explanation"]["status"], "complete", "{v}");
    let cs = v["explanation"]["counterexamples"].as_array().unwrap();
    assert!(!cs.is_empty(), "{v}");
    for c in cs {
        assert_eq!(c["replay"]["status"], "verified", "{c}");
    }
    v
}

/// G1 (reference `contest_target`): the replay must not treat a version-
/// rejected completion as the Commit of a standing-over effect. Two cases,
/// Ledger with two executors, post (1min) checks holder: both posts run 0..1,
/// the first finish commits (holder = taken), the second is marked and ends
/// `conflict` -> no Commit, no contest target for that case. Note (two
/// executors, 1min) then overwrites the LWW memo in both cases at 1..2: the
/// later finish is marked -> `no_lost_update Note.memo` violated. Expected:
/// violated, explanation complete, every counterexample replays verified
/// (the counterexample passes through `finish post conflict`).
const G1_REPLAY: &str = r#"model G1Replay version "0.2" {
  type Verdict = enum { pass, fail }
  type Ack = enum { committed, conflict }
  type H = enum { free, taken }
  people [c1, c2, l1, l2, n1, n2]
  actor Ledger { executors [l1, l2]; state holder: H = free; capability post() -> Ack { version_check [holder] else conflict; commits; duration fixed 1min; result nondet { committed }; writes [holder]; finish { holder = taken } } }
  actor Note { executors [n1, n2]; state memo: H = free last_write_wins; capability note() -> Verdict { duration fixed 1min; result nondet { pass }; writes [memo]; finish { memo = taken } } }
  actor Clerk { control policy; executors [c1, c2]; capability recheck() -> Verdict { duration fixed 1min; result nondet { pass } } }
  actor Vendor { control party; standing over [Ledger.post.commit] basis "posting claim" }
  permit l1 to Ledger.post
  permit l2 to Ledger.post
  permit n1 to Note.note
  permit n2 to Note.note
  permit c1 to Clerk.recheck
  permit c2 to Clerk.recheck
  process Flow { ack = Ledger.post(); n = Note.note(); end done }
  cut Scope { include [Ledger, Note, Clerk, Vendor] }
  arrivals fixed [0min, 0min]
  policy fifo tie_by case_id then call_order assign first_idle
  bounds { cases 2; live_tasks 3; clock 20min }
  verify safety no_resource_overcommit
  contract Controls { contest Vendor on Ledger.post.commit route Clerk.recheck reversible_by [l1] respond_by 5min }
}
"#;

#[test]
fn g1_replay_does_not_file_on_a_version_rejected_completion() {
    let v = replays_verified(G1_REPLAY);
    assert_eq!(verdict(&v, "no_lost_update Note.memo"), "violated", "{v}");
}

/// N1 (reference arrivals replay): a contest route Task's finish is relabelled
/// as the ContestResponse stage (ADR 0040); its write is still a receipt for
/// the other case's running Task. Two cases arrive at 0, Judge (two people,
/// 4min) fails both -> end returned at 4 in both; filing is nondeterministic;
/// with both filed the two routes Route.respond (p, q, 2min) run 4..6 and both
/// overwrite the LWW holder: the later finish is marked -> lost update.
/// Expected: violated (engine), explanation complete, every counterexample
/// replays verified. Control: the same shape with ordinary Tasks is verified
/// at a5 (probe c0, received from the enumeration).
const N1_ROUTE_LWW: &str = r#"model RouteLww version "0.2" {
  type T = enum { pass, fail }
  type H = enum { free, taken }
  people [j,k,p,q]
  actor Judge { executors [j,k]; capability judge() -> T { duration fixed 4min; result nondet { fail } } }
  actor Route { executors [p,q]; state holder: H = free last_write_wins; capability respond() -> T { duration fixed 2min; result nondet { pass }; writes [holder]; finish { holder = taken } } }
  actor Party { control party; standing over [end.returned] basis "claim" }
  permit j to Judge.judge
  permit k to Judge.judge
  permit p to Route.respond
  permit q to Route.respond
  authority G { basis "decision"; grants [j,k] to Judge.judge }
  process Flow { d = Judge.judge(); if d == fail { end returned } else { end posted } }
  cut C { include [Judge,Route,Party] }
  arrivals fixed [0min, 0min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 2; clock 30min }
  verify safety no_resource_overcommit
  contract Controls { contest Party on end.returned route Route.respond reversible_by [j] respond_by 4min }
}
"#;

#[test]
fn n1_route_write_in_one_case_marks_the_other_case_in_replay() {
    let v = replays_verified(N1_ROUTE_LWW);
    assert_eq!(
        verdict(&v, "no_lost_update Route.holder"),
        "violated",
        "{v}"
    );
    let e = both(N1_ROUTE_LWW);
    assert_eq!(verdict(&e, "no_lost_update Route.holder"), "violated");
}

/// N2 (contest.rs `candidates`, ADR 0040 U2 widened by the ADR 0041 M1
/// predicate): an effect Capability whose result is decided by the shared
/// state history (atomic or version_check, `Capability::history_dependent_result`)
/// decides whether the effect happens, so it is itself a decision candidate
/// even with one declared result value. Ledger.post has `version_check
/// [posted] else conflict` and `result nondet { committed }`: its possible
/// results are committed and conflict, decided by the history of posted.
/// Hand calc: Flow has no guard before the call, so the guard candidates are
/// empty and the only candidate is Ledger.post. lp is its executor
/// (executors [lp]), Ledger is in the Cut, `permit lp to Ledger.post` gives
/// Permission, and no Authority is declared -> one decision
/// {Ledger.post, executor, included, permission, authorities []}, reason
/// "Permission あり、Authority 未接続", violated, missing reversal_authority
/// (not `no_decision`). Control k1c: the same Ledger without version_check and
/// with two declared results { committed, conflict } is already a candidate
/// (U2 as written) and gives the identical row. With `authority PostGrant
/// grants lp to Ledger.post` both become satisfied with authorities
/// [PostGrant].
const N2_EFFECT_VERSION: &str = r#"model EffectVersion version "0.2" {
  type Verdict = enum { pass, fail }
  type Ack = enum { committed, conflict }
  type H = enum { free, taken }
  people [c1, lp]
  actor Ledger { executors [lp]; state posted: H = free; capability post() -> Ack { version_check [posted] else conflict; commits; duration fixed 1min; result nondet { committed }; writes [posted]; finish { posted = taken } } }
  actor Clerk { control policy; executors [c1]; capability recheck() -> Verdict { duration fixed 1min; result nondet { pass, fail } } }
  actor Vendor { control party; standing over [Ledger.post.commit] basis "posting claim" }
  permit lp to Ledger.post
  permit c1 to Clerk.recheck
  process Flow { ack = Ledger.post(); end posted }
  cut Scope { include [Ledger, Clerk, Vendor] }
  policy fifo tie_by case_id then call_order assign first_idle
  bounds { cases 1; live_tasks 2; clock 20min }
  verify safety no_resource_overcommit
  contract Controls { contest Vendor on Ledger.post.commit route Clerk.recheck reversible_by [lp] respond_by 5min }
}
"#;

fn n2_reversal(src: &str) -> Value {
    let v = check(src);
    let paths = v["contest_paths"]
        .as_array()
        .unwrap_or_else(|| panic!("{v}"));
    assert_eq!(paths.len(), 1, "{v}");
    let p = &paths[0];
    json!([
        p["checks"]["reversal_authority"],
        p["verdict"],
        p["missing"]
    ])
}

fn n2_expected(authorities: Value) -> Value {
    let satisfied = !authorities.as_array().unwrap().is_empty();
    json!([
        {
            "satisfied": satisfied,
            "quantifier": "exists_decision_per_person",
            "people": [{
                "person": "lp",
                "satisfied": satisfied,
                "decisions": [{
                    "capability": "Ledger.post",
                    "executor": true,
                    "included": true,
                    "permission": true,
                    "authorities": authorities
                }],
                "reason": if satisfied { Value::Null } else { json!("Permission あり、Authority 未接続") }
            }]
        },
        if satisfied { "satisfied" } else { "violated" },
        if satisfied { json!([]) } else { json!(["reversal_authority"]) }
    ])
}

#[test]
fn n2_single_result_version_check_effect_is_a_decision_candidate() {
    let control = change(
        N2_EFFECT_VERSION,
        "version_check [posted] else conflict; commits; duration fixed 1min; result nondet { committed }",
        "commits; duration fixed 1min; result nondet { committed, conflict }",
    );
    let grant = |s: &str| {
        change(
            s,
            "  permit c1",
            "  authority PostGrant { basis \"posting rule\"; grants lp to Ledger.post }\n  permit c1",
        )
    };
    assert_eq!(n2_reversal(N2_EFFECT_VERSION), n2_expected(json!([])));
    assert_eq!(n2_reversal(&control), n2_expected(json!([])));
    assert_eq!(
        n2_reversal(&grant(N2_EFFECT_VERSION)),
        n2_expected(json!(["PostGrant"]))
    );
    assert_eq!(
        n2_reversal(&grant(&control)),
        n2_expected(json!(["PostGrant"]))
    );
}

// ---------------------------------------------------------------------------
// m1 (boundary review after the integration): state invariant (ADR 0037) x
// version_check / last_write_wins (ADR 0041), engine against the reference.
// Positive: cf-version's Ledger (capacity 2, holder: H = free, claims: 0..2,
// claim = version_check [holder] else conflict, finish { holder = taken;
// claims += 1 }), arrivals [0,0], invariant holder == taken -> claims == 1.
// Hand calc (minutes): both cases start at 0 (capacity 2) and finish at 1;
// the finish order at 1 is nondeterministic. The first finisher commits
// (holder taken, claims 1); the other read holder = free at its start, so its
// version is stale and it finishes conflict without writing. Reachable states
// are (free, 0) and (taken, 1), so the invariant holds; each case ends claimed
// or retry_needed at 1 (elapsed 1, work 1) depending on the order; both
// reachable properties satisfied; root satisfied (exit 0).
// Broken: no version_check, holder last_write_wins, no reachable
// retry_needed. Both commit at 1: claims 2 (within 0..2), the state after the
// second finish is (taken, 2) -> invariant violated; the second write of
// holder overwrites the concurrent first one -> no_lost_update Ledger.holder
// violated; root violated (exit 1). Each case ends claimed at 1.
// ---------------------------------------------------------------------------
const M1_INVARIANT: &str = r#"model Conflict version "0.2" {
  type R = enum { committed, conflict }
  type H = enum { free, taken }
  actor Ledger {
    capacity 2
    state holder: H = free
    state claims: 0..2 = 0
    capability claim() -> R {
      version_check [holder] else conflict
      commits
      duration fixed 1min
      result nondet { committed }
      writes [holder, claims]
      finish { holder = taken; claims += 1 }
    }
  }
  process P {
    r = Ledger.claim()
    if r == committed { end claimed } else { end retry_needed }
  }
  cut C { include [Ledger] }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 1; clock 10min }
  contract C { invariant Ledger.holder == taken -> Ledger.claims == 1 }
  verify reachable end.claimed
  verify reachable end.retry_needed
}
"#;

#[test]
fn m1_state_invariant_under_version_check_engine_matches_reference() {
    let v = both(M1_INVARIANT);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(errors(&v), Vec::<(String, String)>::new());
    for id in [
        "contract C #1 invariant",
        "reachable end.claimed",
        "reachable end.retry_needed",
    ] {
        assert_eq!(verdict(&v, id), "satisfied", "{id}: {v}");
    }
    for case in [1, 2] {
        assert_eq!(
            case_terminals(&v, case),
            sorted(vec![
                json!(["claimed", r(1), r(1), r(1)]),
                json!(["retry_needed", r(1), r(1), r(1)]),
            ]),
            "case {case}"
        );
    }
}

#[test]
fn m1_state_invariant_under_last_write_wins_engine_matches_reference() {
    let s = change(
        M1_INVARIANT,
        "      version_check [holder] else conflict\n",
        "",
    );
    let s = change(
        &s,
        "holder: H = free\n",
        "holder: H = free last_write_wins\n",
    );
    let s = change(&s, "  verify reachable end.retry_needed\n", "");
    let v = both(&s);
    assert_eq!(v["verdict"], "violated", "{v}");
    assert_eq!(errors(&v), Vec::<(String, String)>::new());
    assert_eq!(verdict(&v, "contract C #1 invariant"), "violated", "{v}");
    assert_eq!(verdict(&v, "no_lost_update Ledger.holder"), "violated");
    assert_eq!(verdict(&v, "reachable end.claimed"), "satisfied");
    for case in [1, 2] {
        assert_eq!(
            case_terminals(&v, case),
            [json!(["claimed", r(1), r(1), r(1)])],
            "case {case}"
        );
    }
}
