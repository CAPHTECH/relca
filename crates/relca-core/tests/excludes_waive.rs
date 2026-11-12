//! ADR 0022 D1 hand acceptance. No production monitor builds an expectation.
use relca_core::{explain, report};
use serde_json::{Value, json};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
use std::{collections::BTreeSet, path::PathBuf};

pub const NAMES: [&str; 12] = [
    "ctx-excludes",
    "ctx-excludes.broken-start",
    "ctx-nonwaivable",
    "ctx-nonwaivable.broken-waive",
    "ctx-obligation",
    "ctx-obligation.broken-excludes-target",
    "ctx-waive",
    "ctx-waive-authority",
    "ctx-waive-authority.broken-no-authority",
    "ctx-waive-names",
    "ctx-waive-names.broken-unknown-response",
    "ctx-waive.broken-after-deadline",
];
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
pub fn source(name: &str) -> String {
    std::fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
pub fn expected(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(root().join(name).join("expected.json")).unwrap())
        .unwrap()
}
pub fn check(src: &str) -> Value {
    serde_json::to_value(report::check(src)).unwrap()
}
// Object fields can be partial; array membership and cardinality are exact.
// Outcomes are sets, not an ordering chosen by DFS or a count of paths.
pub fn matches(e: &Value, a: &Value) {
    match e {
        Value::Object(o) => {
            for (k, v) in o {
                if k == "absent_diagnostics" {
                    for code in v.as_array().unwrap() {
                        assert!(
                            !a["diagnostics"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|d| d["code"] == *code),
                            "forbidden {code}: {a}"
                        );
                    }
                } else if k == "outcomes" {
                    let mut expected = v.as_array().unwrap().clone();
                    let mut actual = a[k].as_array().unwrap().clone();
                    expected.sort_by_key(Value::to_string);
                    actual.sort_by_key(Value::to_string);
                    matches(&json!(expected), &json!(actual));
                } else {
                    matches(v, a.get(k).unwrap_or_else(|| panic!("missing {k}: {a}")));
                }
            }
        }
        Value::Array(es) => {
            let actual = a.as_array().unwrap_or_else(|| panic!("not array: {a}"));
            assert_eq!(es.len(), actual.len(), "array inventory: {a}");
            // Expected executor alternatives are canonical p,q; all other arrays
            // are declaration ordered. Sort only outcome alternatives.
            for (x, y) in es.iter().zip(actual) {
                matches(x, y);
            }
        }
        _ => assert_eq!(e, a),
    }
}
pub fn rows(v: &Value) -> Vec<&Value> {
    v["obligations"]
        .as_array()
        .expect("opt-in obligation array")
        .iter()
        .collect()
}
fn outcomes(v: &Value) -> Vec<&Value> {
    rows(v)
        .into_iter()
        .flat_map(|r| r["outcomes"].as_array().unwrap())
        .collect()
}
fn statuses(v: &Value) -> Vec<&str> {
    outcomes(v)
        .into_iter()
        .map(|o| o["status"].as_str().unwrap())
        .collect()
}
fn contracts(v: &Value) -> Vec<&str> {
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["id"].as_str().unwrap().starts_with("contract "))
        .map(|p| p["verdict"].as_str().unwrap())
        .collect()
}
/// ADR 0043 (plan C06) warnings added to these old fixtures, by hand from
/// the source: every fixture with people and a committing Officer.release has
/// no responsibility metadata, so W-RESP-0 targets the reachable effect Actor
/// Officer (U12); W-AGY-2 only where Officer has the single executor p.
pub fn c06_added(name: &str) -> &'static [&'static str] {
    if name.starts_with("ctx-waive-authority") {
        &["W-RESP-0"]
    } else if name.starts_with("ctx-nonwaivable") || name.starts_with("ctx-waive") {
        &["W-AGY-2", "W-RESP-0"]
    } else {
        &[]
    }
}
/// Remove exactly the C06 additions (one row each, after W-CUT-0, before the
/// old rows that follow) so the old expected.json still compares exactly.
/// A static rejection has no Agency, so nothing is added there.
fn without_c06(name: &str, invalid: bool, mut a: Value) -> Value {
    let added = if invalid { &[] } else { c06_added(name) };
    let ds = a["diagnostics"].as_array_mut().unwrap();
    let rows: Vec<Value> = ds.drain(1..1 + added.len()).collect();
    let codes: Vec<_> = rows.iter().map(|d| d["code"].as_str().unwrap()).collect();
    assert_eq!(codes, added, "{name}: exact C06 additions");
    for d in &rows {
        let message = d["message"].as_str().unwrap();
        match d["code"].as_str().unwrap() {
            "W-RESP-0" => assert_eq!(message, "Officer has no declared responsibility"),
            _ => assert!(message.starts_with("Officer.release の判定・拒否・実行が 人 に集まる")),
        }
        assert_eq!(d["severity"], "warning");
    }
    a
}
pub fn assert_fixture(name: &str) {
    let src = source(name);
    let e = expected(name);
    let a = without_c06(name, e["verdict"] == "invalid", check(&src));
    matches(&e, &a);
    assert!(a.get("contest_runtime").is_none(), "D1 has no contest");
    if e["verdict"] == "invalid" {
        assert!(
            a.get("obligations").is_none(),
            "static rejection is not an empty ledger"
        );
    } else {
        let mut ids = BTreeSet::new();
        for row in rows(&a) {
            assert!(ids.insert(row["id"].as_str().expect("stable instance id")));
            assert_eq!(row["debtor"], Value::Null);
            assert_eq!(row["debtor_status"], "not_configured");
            assert_eq!(row["coverage"], "complete");
        }
        if !rows(&a).is_empty() {
            assert!(
                a["agency"]["unchecked"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("obligation_debtor")),
                "no implicit debtor claim: {a}"
            );
        }
        if a["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"].as_str().unwrap().ends_with(" waive"))
        {
            assert!(
                a["assumptions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["field"] == "waive.authority"
                        && r["value"] == "static_grants_all_executors")
            );
        }
        for (field, value) in [
            ("obligations.scope", "per_case_response_instances"),
            ("obligations.outcomes", "possible_outcomes_not_path_counts"),
        ] {
            assert!(
                a["assumptions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|x| x["field"] == field && x["value"] == value),
                "{a}"
            );
        }
    }
}
macro_rules! fixtures {
    ($($test:ident => $name:literal),+ $(,)?) => {$ (#[test] fn $test() { assert_fixture($name); })+};
}
fixtures! {
    f1_existing_start => "ctx-excludes",
    f1_later_start_not_pruned => "ctx-excludes.broken-start",
    f2_fulfilled_vacuous => "ctx-obligation",
    f2_exclusion_does_not_erase_debt => "ctx-obligation.broken-excludes-target",
    f3_waived_not_fulfilled => "ctx-waive",
    f3_late_waiver_cannot_repair => "ctx-waive.broken-after-deadline",
    f4_all_executors_authorized => "ctx-waive-authority",
    f4_one_missing_grant => "ctx-waive-authority.broken-no-authority",
    f5_nonwaivable_fulfilled => "ctx-nonwaivable",
    f5_nonwaivable_rejected => "ctx-nonwaivable.broken-waive",
    f6_qualified_forward_reference => "ctx-waive-names",
    f6_unknown_response => "ctx-waive-names.broken-unknown-response",
}

#[test]
fn skeletons_validate_old_grammar_warnings_bounds_and_hand_times() {
    // Strip ONLY the final new contract; do not hide malformed authority,
    // process, grant or Cut declarations behind an earlier parser rejection.
    for name in NAMES {
        let src = source(name);
        let skeleton = format!("{}}}\n", src.split_once("  contract Controls {").unwrap().0);
        let a = check(&skeleton);
        assert_eq!(a["verdict"], "satisfied", "{name}: {a}");
        // The fixed A result selects only one arm in the obligation pair.
        // Waiver skeletons call Officer/Wait, never B. q/issuer outside
        // included executor pools are not unreachable people.
        let mut codes = vec!["W-CUT-0"];
        // ADR 0043 (plan C06): the hand additions follow W-CUT-0.
        codes.extend(c06_added(name));
        let k = c06_added(name).len();
        if name.starts_with("ctx-obligation") {
            codes.extend(["W-UNREACH-0", "W-UNREACH-1"]);
            let (cap, arm) = if name == "ctx-obligation" {
                ("Wait.run", "else")
            } else {
                ("B.run", "then")
            };
            assert_eq!(
                a["diagnostics"][1]["message"],
                format!("process Flow: {cap} never_started")
            );
            assert_eq!(
                a["diagnostics"][2]["message"],
                format!("process Flow: if {arm} arm is never taken")
            );
        } else if name.starts_with("ctx-waive") {
            codes.push("W-UNREACH-0");
            assert_eq!(
                a["diagnostics"][1 + k]["message"],
                "process Flow: B.run not_called"
            );
        }
        assert_eq!(
            a["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .map(|d| d["code"].as_str().unwrap())
                .collect::<Vec<_>>(),
            codes,
            "{name}"
        );
        assert_eq!(
            a["bounds"],
            json!({"cases":1,"live_tasks":2,"clock":20,"truncated":false})
        );
        let time = if name == "ctx-waive.broken-after-deadline" {
            7
        } else if name.starts_with("ctx-nonwaivable") {
            1
        } else if name.starts_with("ctx-excludes") || name == "ctx-obligation" {
            2
        } else if name == "ctx-obligation.broken-excludes-target" {
            6
        } else {
            5
        };
        assert_eq!(
            a["terminals"],
            json!([{"end":"done","elapsed":{"min":time,"max":time},"work":{"min":time,"max":time},"result_at":null}]),
            "{name}"
        );
        assert!(a.get("obligations").is_none());
    }
}

pub fn model(process: &str, clauses: &str) -> String {
    format!(
        r#"model M version "0.2" {{
  type T = enum {{ ok, bad }}
  people [p,q,issuer]
  actor Gate {{ capacity 2; capability run() -> T {{ duration fixed 1min; result nondet {{ ok }} }} }}
  actor Officer {{ executors [p]; capability release() -> T {{ commits; duration fixed 1min; result nondet {{ ok }} }} }}
  actor B {{ capacity 1; capability run() -> T {{ duration fixed 3min; result nondet {{ ok }} }} }}
  actor Wait {{ capacity 1; capability run() -> T {{ duration fixed 6min; result nondet {{ ok }} }} }}
  permit [p,q] to Officer.release
  authority G {{ basis "grant"; grants [p,q] to Officer.release; issued_by issuer }}
  process Flow {{ {process} }}
  bounds {{ cases 1; live_tasks 3; clock 30min }}
  verify safety no_resource_overcommit
  contract Controls {{ {clauses} }}
}}"#
    )
}
const DELIVERY: &str = "response delivery: case.received -> one_of(B.run.finish) by 5min after case.received otherwise unresolved";
const WAIVE: &str = "waive delivery on Officer.release.finish reason \"exception\" authorized_by G";

#[test]
fn a1_running_b_finishes_and_same_start_bundle_does_not_precede_itself() {
    let src = model(
        "fork f { a = Gate.run(); b = B.run() }; join all f; end done",
        "excludes Gate.run.finish, B.run.start",
    );
    let a = check(&src);
    assert_eq!(contracts(&a), ["satisfied"], "{a}");
    assert_eq!(a["terminals"][0]["elapsed"], json!({"min":3,"max":3}));
    assert_eq!(a["terminals"][0]["work"], json!({"min":4,"max":4}));
    for (process, verdict, time) in [
        ("a = Gate.run(); end done", "satisfied", 1),
        ("a = Gate.run(); b = Gate.run(); end done", "violated", 2),
    ] {
        let a = check(&model(process, "excludes Gate.run.start, Gate.run.start"));
        assert_eq!(contracts(&a), [verdict], "{a}");
        assert_eq!(a["terminals"][0]["elapsed"], json!({"min":time,"max":time}));
    }
    // Shared p makes B remain queued while Officer runs; queued != started.
    let src = model(
        "fork f { a = Officer.release(); b = B.run() }; join all f; end done",
        "excludes Officer.release.finish, B.run.start",
    )
    .replace("actor B { capacity 1", "actor B { executors [p]");
    let a = check(&src);
    assert_eq!(contracts(&a), ["violated"], "{a}");
    assert_eq!(a["terminals"][0]["elapsed"], json!({"min":4,"max":4}));
}

#[test]
fn a2_target_wins_over_waiver_in_all_six_clause_orders() {
    let response = "response delivery: Officer.release.finish -> one_of(Officer.release.commit) by 0min after Officer.release.finish otherwise unresolved";
    let unrelated = "excludes Gate.run.finish, B.run.start";
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        for target in [true, false] {
            let response = if target {
                response.to_string()
            } else {
                response.replace("one_of(Officer.release.commit)", "one_of(B.run.finish)")
            };
            let clauses = [&response, WAIVE, unrelated];
            let a = check(&model(
                "o = Officer.release(); end done",
                &order.map(|i| clauses[i]).join("; "),
            ));
            assert_eq!(
                statuses(&a),
                [if target { "fulfilled" } else { "waived" }],
                "order {order:?}: {a}"
            );
            let o = outcomes(&a)[0];
            assert_eq!(o["triggered_at"], 1);
            assert_eq!(o["deadline"], 1);
            assert_eq!(o["resolved_at"], 1);
            assert_eq!(
                o["authority"],
                if target { Value::Null } else { json!("G") }
            );
        }
    }
}

#[test]
fn a1_cancel_ack_only_actual_running_ack_activates_exclusion() {
    let original = include_str!("../../../fixtures/contract-cancel/model.relca");
    let prefix = original.split_once("  contract Controls {").unwrap().0;
    let src = format!(
        "{prefix}  contract Controls {{ excludes Slow.work.cancel_ack, Fast.win.start }}\n}}\n"
    )
    .replace("    end done", "    again = Fast.win(); end done");
    let a = check(&src);
    assert_eq!(
        contracts(&a),
        ["violated"],
        "ack2 precedes again.start2: {a}"
    );
    assert_eq!(a["terminals"][0]["elapsed"], json!({"min":3,"max":3}));
    let a = check(&src.replace(
        "cancel cooperative ack_within 1min",
        "cancel forbidden_after_start",
    ));
    assert_eq!(contracts(&a), ["vacuous"], "natural finish is not ack: {a}");
    let a = check(&src.replace("fixed 3min", "fixed 2min"));
    assert_eq!(
        contracts(&a),
        ["violated"],
        "normal/ack tie retains ack witness: {a}"
    );
    // Shared person: when Fast wins at 1, the join and the dispatch of the
    // freed p to Slow are both enabled at 1 (join is not a dispatch lock).
    // On the path where Slow starts at 1, the join's cancel_request is
    // acknowledged at 2 and again.start follows at 2, so the exclusion is
    // violated. The queued-cancellation path alone (no ack) is covered by the
    // implementation's internal direct-event test (A1).
    let shared = src
        .replace("  type Result", "  people [p]\n  type Result")
        .replace("capacity 1", "executors [p]");
    let a = check(&shared);
    assert_eq!(
        contracts(&a),
        ["violated"],
        "slow.start1 then ack2 precedes again.start2: {a}"
    );
}

#[test]
fn additional_execution_skeletons_are_legal_without_new_contracts() {
    let mut sources = vec![
        model(
            "fork f { a = Gate.run(); b = B.run() }; join all f; end done",
            "",
        ),
        model("o = Officer.release(); end done", ""),
        model(
            "g1 = Gate.run(); g2 = Gate.run(); o = Officer.release(); g3 = Gate.run(); w = Wait.run(); end done",
            "",
        ),
        model(
            "fork f { g1 = Gate.run(); g2 = Gate.run() }; join all f; o = Officer.release(); end done",
            "",
        ),
    ];
    sources.push(
        model("o = Officer.release(); w = Wait.run(); end done", "")
            .replace("cases 1", "cases 2")
            .replace(
                "  bounds",
                "  arrivals fixed [0min,10min]\n  policy fifo tie_by case_id\n  bounds",
            ),
    );
    for src in sources {
        assert_eq!(
            check(&src)["verdict"],
            "satisfied",
            "{src}: {}",
            check(&src)
        );
    }
}

#[test]
fn a10_legacy_response_authority_arrivals_and_metrics_keep_optional_tables_absent() {
    // Full census/budget/command-byte checks remain in their existing targets;
    // do not regenerate those fixed expectations from this implementation.
    for name in [
        "contract-response",
        "exec-authority",
        "arrivals-contracts",
        "met-serial",
    ] {
        let a = check(&source(name));
        assert_eq!(a["verdict"], "satisfied", "{name}: {a}");
        assert!(a.get("obligations").is_none());
        assert!(a.get("contest_runtime").is_none());
        assert!(
            !a["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["field"].as_str().unwrap().starts_with("obligations."))
        );
    }
}

#[test]
fn six_fixture_pairs_are_exactly_one_declared_source_edit() {
    for (base, fault, from, to) in [
        (
            "ctx-excludes",
            "ctx-excludes.broken-start",
            "b = B.run(); a = A.stop()",
            "a = A.stop(); b = B.run()",
        ),
        (
            "ctx-obligation",
            "ctx-obligation.broken-excludes-target",
            "stop() -> T { duration fixed 1min; result nondet { ok }",
            "stop() -> T { duration fixed 1min; result nondet { bad }",
        ),
        (
            "ctx-waive",
            "ctx-waive.broken-after-deadline",
            "release() -> T { commits; duration fixed 1min",
            "release() -> T { commits; duration fixed 3min",
        ),
        (
            "ctx-waive-authority",
            "ctx-waive-authority.broken-no-authority",
            "grants [p,q] to Officer.release",
            "grants p to Officer.release",
        ),
        (
            "ctx-nonwaivable",
            "ctx-nonwaivable.broken-waive",
            "otherwise unresolved non_waivable\n",
            "otherwise unresolved non_waivable\n    waive audit on Officer.release.finish reason \"registered exception\" authorized_by G\n",
        ),
        (
            "ctx-waive-names",
            "ctx-waive-names.broken-unknown-response",
            "waive Controls.delivery",
            "waive Controls.deliveries",
        ),
    ] {
        let base = source(base);
        assert_eq!(base.matches(from).count(), 1);
        assert_eq!(base.replacen(from, to, 1), source(fault), "{fault}");
    }
}

#[test]
fn a3_all_pending_instances_but_not_future_triggers_are_waived() {
    let response = "response delivery: Gate.run.finish -> one_of(B.run.finish) by 5min after Gate.run.finish otherwise unresolved";
    let clauses = format!("{response}; {WAIVE}");
    let a = check(&model(
        "g1 = Gate.run(); g2 = Gate.run(); o = Officer.release(); g3 = Gate.run(); w = Wait.run(); end done",
        &clauses,
    ));
    assert_eq!(rows(&a).len(), 3, "one ID per binding, not per deadline");
    assert_eq!(statuses(&a), ["waived", "waived", "violated"]);
    for (row, site, t, d, res, status) in rows(&a)
        .into_iter()
        .zip(["g1", "g2", "g3"])
        .zip([1, 2, 4])
        .zip([6, 7, 9])
        .zip([3, 3, 9])
        .zip(["waived", "waived", "violated"])
        .map(|(((((r, s), t), d), res), status)| (r, s, t, d, res, status))
    {
        assert_eq!(row["trigger_site"], site);
        let o = &row["outcomes"][0];
        assert_eq!(o["triggered_at"], t);
        assert_eq!(o["deadline"], d);
        assert_eq!(o["resolved_at"], res);
        assert_eq!(o["status"], status);
    }
    let a = check(&model(
        "fork f { g1 = Gate.run(); g2 = Gate.run() }; join all f; o = Officer.release(); end done",
        &clauses,
    ));
    assert_eq!(rows(&a).len(), 2);
    assert_eq!(statuses(&a), ["waived", "waived"]);
    assert_ne!(rows(&a)[0]["id"], rows(&a)[1]["id"]);
    for o in outcomes(&a) {
        assert_eq!(o["triggered_at"], 1);
        assert_eq!(o["deadline"], 6);
        assert_eq!(o["resolved_at"], 2);
    }
}

#[test]
fn a4_arrivals_keep_instance_keys_and_effects_case_local() {
    // At t1 case1 releases. Case2 is not received until10: no future waiver.
    let src = model(
        "o = Officer.release(); w = Wait.run(); end done",
        &format!("{DELIVERY}; {WAIVE}"),
    )
    .replace("cases 1", "cases 2")
    .replace(
        "  bounds",
        "  arrivals fixed [0min,10min]\n  policy fifo tie_by case_id\n  bounds",
    );
    let a = check(&src);
    assert_eq!(a["verdict"], "satisfied", "{a}");
    assert_eq!(rows(&a).len(), 2);
    for (row, case, t) in rows(&a)
        .into_iter()
        .zip([1, 2])
        .zip([0, 10])
        .map(|((r, c), t)| (r, c, t))
    {
        assert_eq!(row["case_id"], case);
        assert_eq!(row["outcomes"][0]["triggered_at"], t);
        assert_eq!(row["outcomes"][0]["resolved_at"], t + 1);
        assert_eq!(row["outcomes"][0]["deadline"], t + 5);
    }
    let src = model(
        "b = B.run(); g = Gate.run(); end done",
        "excludes Gate.run.finish, B.run.start",
    )
    .replace("cases 1", "cases 2")
    .replace(
        "  bounds",
        "  arrivals fixed [0min,10min]\n  policy fifo tie_by case_id\n  bounds",
    );
    let a = check(&src);
    assert_eq!(
        a["verdict"], "satisfied",
        "case1 exclusion must not ban case2 B: {a}"
    );
}

fn failure(src: &str, code: &str) {
    let a = check(src);
    assert_eq!(
        a["verdict"],
        if code.starts_with("E-") {
            "invalid"
        } else {
            "unsupported"
        },
        "{a}"
    );
    assert_eq!(a["properties"], json!([]));
    assert_eq!(a["terminals"], json!([]));
    assert_eq!(
        a["bounds"],
        json!({"cases":null,"live_tasks":null,"clock":null,"truncated":null})
    );
    assert!(a.get("obligations").is_none());
    let d = a["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == code)
        .unwrap_or_else(|| panic!("missing {code}: {a}"));
    assert_eq!(d["severity"], "error");
    let start = d["span"]["start"].as_u64().unwrap() as usize;
    let end = d["span"]["end"].as_u64().unwrap() as usize;
    assert!(
        start < end && end <= src.len() && src.is_char_boundary(start) && src.is_char_boundary(end)
    );
}

#[test]
fn a5_static_single_fault_matrix() {
    let base = model(
        "o = Officer.release(); end done",
        &format!("{DELIVERY}; {WAIVE}"),
    );
    for (from, to, code) in [
        (
            "on Officer.release.finish",
            "on Unknown.release.finish",
            "E-NAME-0",
        ),
        (
            "on Officer.release.finish",
            "on Officer.missing.finish",
            "E-NAME-0",
        ),
        ("authorized_by G", "authorized_by Missing", "E-NAME-0"),
        ("waive delivery", "waive missing", "E-NAME-0"),
        ("waive delivery", "waive Missing.delivery", "E-NAME-0"),
        (
            "on Officer.release.finish",
            "on Officer.release.start",
            "E-CON-0",
        ),
        (
            "on Officer.release.finish",
            "on Officer.release.finish(nope)",
            "E-TYPE-0",
        ),
        (" authorized_by G", "", "E-SYN-0"),
        (" reason \"exception\"", "", "E-SYN-0"),
        (
            "grants [p,q] to Officer.release",
            "grants q to Officer.release",
            "E-CON-0",
        ),
        (
            "grants [p,q] to Officer.release",
            "grants [p,q] to B.run",
            "E-CON-0",
        ),
        ("executors [p]", "capacity 1", "E-CON-0"),
        (
            "otherwise unresolved;",
            "otherwise unresolved non_waivable;",
            "E-CON-0",
        ),
    ] {
        assert!(base.contains(from));
        failure(&base.replace(from, to), code);
    }
    for clauses in [
        format!("{DELIVERY}; {DELIVERY}"),
        format!("{DELIVERY}; {WAIVE}; {WAIVE}"),
        DELIVERY.replace(
            "otherwise unresolved",
            "otherwise unresolved non_waivable non_waivable",
        ),
    ] {
        failure(
            &model("o = Officer.release(); end done", &clauses),
            "U-SEM-0",
        );
    }
    for clause in [
        "excludes Gate.run.finish, B.run.finish",
        "excludes B.run.commit, Gate.run.start",
    ] {
        failure(&model("g = Gate.run(); end done", clause), "E-CON-0");
    }
    // Old short/event-shaped forms remain unsupported, not silently accepted.
    for clause in [
        "excludes Gate.run.start",
        "waive Gate.run.start",
        "contest Gate.run.start",
    ] {
        failure(&model("g = Gate.run(); end done", clause), "U-SYN-0");
    }
    for name in [
        "ctx-waive-authority.broken-no-authority",
        "ctx-nonwaivable.broken-waive",
    ] {
        let src = source(name);
        let process = src
            .lines()
            .find(|l| l.trim_start().starts_with("process Flow"))
            .unwrap();
        failure(
            &src.replace(process, "  process Flow { end done }"),
            "E-CON-0",
        );
    }
}

#[test]
fn a5_legacy_contextual_actor_value_and_binding_names_still_work() {
    for word in [
        "excludes",
        "waive",
        "non_waivable",
        "on",
        "reason",
        "authorized_by",
        "cancel_ack",
    ] {
        let src = model(
            "g = Gate.run(); end done",
            "at_most_once Officer.release.commit per case.id",
        )
        .replace("actor Gate", &format!("actor {word}"))
        .replace("Gate.run", &format!("{word}.run"))
        .replace("{ ok, bad }", &format!("{{ {word}, bad }}"))
        .replace("{ ok }", &format!("{{ {word} }}"))
        .replace("g =", &format!("{word} ="));
        let a = check(&src);
        assert_eq!(a["verdict"], "vacuous", "{word}: {a}");
        assert!(
            a.get("obligations").is_none(),
            "identifier is not feature use"
        );
    }
}

#[test]
fn deadline_equality_cut_and_terminal_deadlock_reasons_remain_distinct() {
    let base = model(
        "o = Officer.release(); w = Wait.run(); end done",
        &format!("{DELIVERY}; {WAIVE}"),
    );
    let a = check(&base.replace("by 5min", "by 1min"));
    assert_eq!(statuses(&a), ["waived"]); // finish1 == deadline1
    let a = check(&base.replace("clock 30min", "clock 0min"));
    assert_eq!(a["verdict"], "inconclusive", "{a}");
    assert_eq!(statuses(&a), ["unresolved"]);
    assert_eq!(rows(&a)[0]["coverage"], "truncated");
    assert_eq!(outcomes(&a)[0]["resolved_at"], Value::Null);
    assert!(outcomes(&a)[0]["reason"].is_string());
    let a = check(&model("g = Gate.run(); end done", DELIVERY));
    assert_eq!(statuses(&a), ["violated"]);
    assert_eq!(outcomes(&a)[0]["reason"], "terminal_unresolved");
    assert_eq!(outcomes(&a)[0]["resolved_at"], 1);
    let a = check(
        &model("b = B.run(); end done", DELIVERY)
            .replace("actor B { capacity 1", "actor B { capacity 0"),
    );
    assert_eq!(statuses(&a), ["violated"]);
    assert_eq!(outcomes(&a)[0]["reason"], "deadlock_unresolved");
    for clock in [2, 3] {
        let a = check(
            &base
                .replace(
                    "fixed 1min; result nondet { ok } } }\n  actor B",
                    "fixed 3min; result nondet { ok } } }\n  actor B",
                )
                .replace("by 5min", "by 1min")
                .replace("clock 30min", &format!("clock {clock}min")),
        );
        assert_eq!(
            statuses(&a),
            ["violated"],
            "deadline before next event/clock: {a}"
        );
    }
}

#[test]
fn a8_excludes_and_late_waive_explanations_preserve_check_and_verify_replay() {
    for name in [
        "ctx-excludes.broken-start",
        "ctx-waive.broken-after-deadline",
    ] {
        let src = source(name);
        let a = serde_json::to_value(explain::explain(&src)).unwrap();
        let mut projection = a.clone();
        projection.as_object_mut().unwrap().remove("explanation");
        assert_eq!(
            ledger_shape::original_report(&projection),
            ledger_shape::original_report(&check(&src))
        );
        assert_eq!(a["verdict"], "violated", "{a}");
        let cs = a["explanation"]["counterexamples"].as_array().unwrap();
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0]["status"], "verified", "{a}");
        assert_eq!(cs[0]["replay"]["status"], "verified", "{a}");
        let kind = if name.starts_with("ctx-excludes") {
            "excludes"
        } else {
            "response"
        };
        assert!(cs[0]["id"].as_str().unwrap().ends_with(kind));
        let location = &cs[0]["property_location"];
        // ASCII line lengths plus LF, calculated independently of the DTO.
        let (start, end, line) = if kind == "excludes" {
            (416, 451, 9)
        } else {
            (697, 802, 13)
        };
        assert_eq!(src.find(&format!("    {kind} ")).unwrap() + 4, start);
        assert_eq!(start + src[start..].find('\n').unwrap(), end);
        assert_eq!(location["span"], json!({"start":start,"end":end}));
        assert_eq!(location["line"], line);
        assert_eq!(location["column"], 5);
    }
}
