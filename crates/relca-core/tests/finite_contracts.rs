//! ADR 0010 acceptance, fixed before implementation. Public report entry points
//! and existing budget API only; no proposed contract AST/IR types are required.
//! Arithmetic witnesses are in fixtures/contract-*/calc.md and comments below.

use relca_core::{Verdict, diff, engine, lower, report, syntax};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};
#[path = "support/diff_increment.rs"]
mod diff_increment;

const SAFETY: &str = "verify safety no_resource_overcommit";
const RESPONSE: &str = include_str!("../../../fixtures/contract-response/model.relca");
const CUT: &str = include_str!("../../../fixtures/contract-cut/model.relca");
const CANCEL: &str = include_str!("../../../fixtures/contract-cancel/model.relca");
const ORDER: &str = include_str!("../../../fixtures/contract-order.broken-parallel/model.relca");
const DIFF: &str = include_str!("../../../fixtures/contract-diff/model.relca");

fn check(source: &str) -> Value {
    serde_json::to_value(report::check(source)).unwrap()
}

fn has_code(value: &Value, code: &str) -> bool {
    value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == code)
}

fn contracts(value: &Value) -> Vec<&str> {
    value["properties"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["id"].as_str().unwrap().starts_with("contract "))
        .map(|p| p["verdict"].as_str().unwrap())
        .collect()
}

fn assert_contracts(value: &Value, expected: &[&str]) {
    assert_eq!(contracts(value), expected, "{value}");
}

fn failure(source: &str, code: &str) {
    let value = check(source);
    assert!(has_code(&value, code), "missing {code}: {value}");
    assert_eq!(
        value["verdict"],
        if code.starts_with("E-") {
            "invalid"
        } else {
            "unsupported"
        },
        "{value}"
    );
    assert_eq!(value["properties"], json!([]));
    assert_eq!(value["terminals"], json!([]));
    assert_eq!(value["bounds"]["truncated"], Value::Null);
    let diagnostic = value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == code)
        .unwrap();
    assert_eq!(diagnostic["severity"], "error");
    let start = diagnostic["span"]["start"]
        .as_u64()
        .expect("source diagnostic span") as usize;
    let end = diagnostic["span"]["end"].as_u64().unwrap() as usize;
    assert!(start <= end && end <= source.len());
    assert!(source.is_char_boundary(start) && source.is_char_boundary(end));
}

fn model(process: &str, clauses: &str) -> String {
    format!(
        r#"model M version "0.2" {{
      type T = enum {{ ok, bad }}
      actor A {{ capacity 2; capability a() -> T {{ commits; duration fixed 1min; result nondet {{ ok }} }} }}
      actor B {{ capacity 1; capability b() -> T {{ duration fixed 1min; result nondet {{ ok }} }} }}
      process P {{ {process} }}
      bounds {{ cases 1; live_tasks 2; clock 10min }}
      {SAFETY}
      contract Controls {{ {clauses} }}
    }}"#
    )
}

fn named_rows(value: &Value, key: &str, id: &str) -> BTreeMap<String, Value> {
    let rows = value[key].as_array().unwrap();
    let result: BTreeMap<_, _> = rows
        .iter()
        .map(|row| (row[id].as_str().unwrap().to_owned(), row.clone()))
        .collect();
    assert_eq!(result.len(), rows.len(), "duplicate {key}");
    result
}

/// ADR 0035 U12(意味の注記の条件と性質の行の形): atomic の check-and-insert を
/// 持たない旧いモデルでは、at_most_once の行にだけ notes ["mechanism_unmodeled"] が
/// 足される。他の行・verdict・cases は旧い手計算の期待のまま。at_most_once 以外の
/// 行に notes が付けば、この期待と一致しないので落ちる。
fn with_at_most_once_notes(source: &str, properties: &Value) -> Value {
    assert!(!source.contains("atomic"), "pre-A3 oracle only");
    let mut rows = properties.clone();
    for row in rows.as_array_mut().unwrap() {
        assert!(row.get("notes").is_none(), "hand oracle carries no notes");
        if row["id"].as_str().unwrap().ends_with(" at_most_once") {
            row["notes"] = json!(["mechanism_unmodeled"]);
        }
    }
    rows
}
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
fn assert_hand_report(source: &str, actual: &Value, expected: &Value) {
    let mut expected = expected.clone();
    expected["properties"] = with_at_most_once_notes(source, &expected["properties"]);
    let expected = &expected;
    let cut = expected["bounds"]["truncated"] == true;
    assert_eq!(
        ledger_shape::original_field_count(actual),
        if cut { 14 } else { 13 },
        "ADR 0025 adds unreachable unchecked only for incomplete exploration"
    );
    assert_eq!(
        actual.get("unchecked"),
        cut.then(|| json!(["unreachable"])).as_ref()
    );
    assert_eq!(actual["verdict"], expected["verdict"], "{actual}");
    assert_eq!(actual["bounds"], expected["bounds"]);
    for (key, id) in [("properties", "id"), ("terminals", "end")] {
        assert_eq!(
            named_rows(actual, key, id),
            named_rows(expected, key, id),
            "{key}: {actual}"
        );
    }
    for diagnostic in expected["diagnostics"].as_array().into_iter().flatten() {
        assert!(has_code(actual, diagnostic["code"].as_str().unwrap()));
    }
    for code in expected["absent_diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
    {
        assert!(!has_code(actual, code.as_str().unwrap()), "{actual}");
    }
}

fn fixture(name: &str) {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name);
    let source = std::fs::read_to_string(directory.join("model.relca")).unwrap();
    let expected: Value =
        serde_json::from_str(&std::fs::read_to_string(directory.join("expected.json")).unwrap())
            .unwrap();
    if name.starts_with("contract-diff") {
        let actual = serde_json::to_value(diff::diff(&source, "Before", "After")).unwrap();
        assert_eq!(actual["verdict"], expected["verdict"], "{actual}");
        for side in ["from", "to"] {
            assert_hand_report(&source, &actual[side]["report"], &expected[side]["report"]);
        }
        for (key, id) in [("properties", "id"), ("terminals", "end")] {
            assert_eq!(named_rows(&actual, key, id), named_rows(&expected, key, id));
        }
        assert_eq!(actual["differences"], expected["differences"]);
        // ADR 0031: the old diagnostics stay empty; only the enumerated
        // W-DIFF-1 / W-DIFF-3 increments for the rows are added.
        assert_eq!(
            diff_increment::old_diagnostics(&actual, &[]),
            Vec::<Value>::new()
        );
    } else {
        assert_hand_report(&source, &check(&source), &expected);
    }
}

macro_rules! fixtures {
    ($($test:ident => $name:literal),+ $(,)?) => {$ (
        #[test] fn $test() { fixture($name); }
    )+};
}
fixtures! {
    f1_invariant => "contract-invariant",
    f1_cut_guard => "contract-invariant.broken-cut-guard",
    f2_once => "contract-once",
    f2_repeat => "contract-once.broken-repeat-result",
    f3_response => "contract-response",
    f3_late => "contract-response.broken-late-finish",
    f3_clock => "contract-response.broken-short-clock",
    f4_requires => "contract-requires",
    f4_cut_guard => "contract-requires.broken-cut-guard",
    f5_vacuous => "contract-vacuous",
    f5_enabled => "contract-vacuous.broken-enable-branch",
    f6_cut => "contract-cut",
    f6_late_target => "contract-cut.broken-late-target",
    f7_diff => "contract-diff",
    f7_zero => "contract-diff.broken-zero-deadline",
    f8_order => "contract-order",
    f8_parallel => "contract-order.broken-parallel",
    f9_cancel => "contract-cancel",
    f9_ignore_cancel => "contract-cancel.broken-ignore-cancel",
}

#[test]
fn c_fixture_precontract_execution_baselines_match_hand_arithmetic() {
    // A green control independent of the missing feature: strip only the final
    // contract block and validate the accepted Phase 0 execution skeletons.
    // This does NOT verify any clause or turn the full RED suite into success.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut checked = 0;
    for entry in std::fs::read_dir(root).unwrap() {
        let directory = entry.unwrap().path();
        if !directory.is_dir()
            || !directory
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("contract-")
        {
            continue;
        }
        let source = std::fs::read_to_string(directory.join("model.relca")).unwrap();
        let prefix = source
            .split_once("  contract Controls {")
            .expect("fixtures put contracts last")
            .0;
        let source = format!("{prefix}}}\n");
        let expected: Value = serde_json::from_str(
            &std::fs::read_to_string(directory.join("expected.json")).unwrap(),
        )
        .unwrap();
        let selections: &[Option<&str>] = if directory
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("contract-diff")
        {
            &[Some("Before"), Some("After")]
        } else {
            &[None]
        };
        for &selection in selections {
            let (actual, expected) = match selection {
                Some(name) => (
                    serde_json::to_value(report::check_configuration(&source, name)).unwrap(),
                    &expected[if name == "Before" { "from" } else { "to" }]["report"],
                ),
                None => (check(&source), &expected),
            };
            assert_eq!(
                actual["verdict"],
                expected["properties"][0]["verdict"],
                "{} {selection:?}: {actual}",
                directory.display()
            );
            assert_eq!(actual["bounds"], expected["bounds"]);
            assert_eq!(
                named_rows(&actual, "terminals", "end"),
                named_rows(expected, "terminals", "end")
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 21);
}

#[test]
fn c_syntax_context_and_contract_only() {
    let source = model(
        "x = A.a(); y = B.b(); end done",
        r#"
        invariant A.a.finish(ok) -> (x == ok && x == ok) || x == bad;
        at_most_once A.a.commit per case.id
        response A.a.finish -> one_of(A.a.commit, end.done) by 0min after A.a.finish otherwise unresolved;
        requires_event B.b.start after A.a.finish(ok)
    "#,
    );
    for source in [source.clone(), source.replace(SAFETY, "")] {
        let value = check(&source);
        assert_eq!(value["verdict"], "satisfied", "{value}");
        assert_contracts(&value, &["satisfied"; 4]);
    }
    for word in [
        "contract",
        "invariant",
        "at_most_once",
        "response",
        "requires_event",
        "one_of",
        "after",
        "otherwise",
        "unresolved",
        "start",
        "finish",
        "commit",
        "per",
        "case",
        "id",
        "received",
    ] {
        let value = check(
            &model("x = A.a(); end done", "at_most_once A.a.commit per case.id")
                .replace("actor A", &format!("actor {word}"))
                .replace("A.a", &format!("{word}.a")),
        );
        assert_eq!(
            value["verdict"], "satisfied",
            "contextual identifier {word}: {value}"
        );
    }
    // by was a reserved word before ADR 0010 (tie by); it stays reserved
    // outside contracts. Only contract leaves the reserved table.
    failure(
        &model("x = A.a(); end done", "at_most_once A.a.commit per case.id")
            .replace("actor A", "actor by")
            .replace("A.a", "by.a"),
        "E-SYN-0",
    );
    let empty = model("end done", "").replace(SAFETY, "");
    failure(&empty, "U-SEM-0");
    assert_eq!(check(&model("end done", ""))["verdict"], "satisfied");
}

#[test]
fn c_syntax_unknown_forms_and_known_wrong_positions() {
    // ADR 0011 migrates the valid separation syntax from unknown syntax to
    // unsupported anonymous identity. The two event-suffixed forms stay U-SYN.
    failure(
        &model("x = A.a(); end done", "separation A.a.finish, B.b.finish"),
        "U-SEM-0",
    );
    for clause in [
        "permitted A.a.start",
        "authority_basis A.a.commit Grant",
        "contest A.a.start",
        "excludes A.a.start",
        "waive A.a.start",
        "mystery A.a.start",
        "invariant always registered -> x == ok",
        "invariant A.a.unknown -> x == ok",
        "response case.received -> one_of(end.done) by 1min after case.received otherwise end.done",
    ] {
        failure(&model("x = A.a(); end done", clause), "U-SYN-0");
    }
    for clause in [
        "invariant A.a.finish x == ok",
        "at_most_once A.a.commit per case",
        "at_most_once A.a.commit per case.start",
        "invariant A.a.finish() -> x == ok",
        "response case.received -> one_of() by 1min after case.received otherwise unresolved",
        "response case.received -> one_of(end.done by 1min after case.received otherwise unresolved",
        "response case.received -> one_of(end.done) by after case.received otherwise unresolved",
        "invariant A.a.finish -> x == ok invariant A.a.commit -> x == ok",
    ] {
        failure(&model("x = A.a(); end done", clause), "E-SYN-0");
    }
}

#[test]
fn c_static_con_event_kinds_and_trigger_identity() {
    for clause in [
        "at_most_once B.b.commit per case.id",
        "at_most_once A.a.start per case.id",
        "at_most_once A.a.finish per case.id",
        "at_most_once A.a.finish(ok) per case.id",
        "requires_event A.a.finish after B.b.finish",
        "requires_event A.a.commit after B.b.finish",
        "response B.b.commit -> one_of(end.done) by 1min after B.b.commit otherwise unresolved",
        "response case.received -> one_of(B.b.commit) by 1min after case.received otherwise unresolved",
        "requires_event A.a.start after B.b.commit",
        "invariant B.b.commit -> x == ok",
        "response A.a.start -> one_of(end.done) by 1min after B.b.start otherwise unresolved",
        "response A.a.start -> one_of(end.done) by 1min after A.a.finish otherwise unresolved",
        "response A.a.finish(ok) -> one_of(end.done) by 1min after A.a.finish(bad) otherwise unresolved",
        "response A.a.finish -> one_of(end.done) by 1min after A.a.finish(ok) otherwise unresolved",
        "response case.received -> one_of(end.done) by 1min after A.a.start otherwise unresolved",
    ] {
        failure(&model("x = A.a(); end done", clause), "E-CON-0");
    }
}

#[test]
fn c_static_name_type_and_unreachable_events() {
    for clause in [
        "at_most_once Unknown.a.commit per case.id",
        "at_most_once A.unknown.commit per case.id",
        "response case.received -> one_of(end.unknown) by 1min after case.received otherwise unresolved",
        "invariant A.a.finish -> absent == ok",
    ] {
        failure(&model("x = A.a(); end done", clause), "E-NAME-0");
    }
    for clause in [
        "invariant A.a.finish(nope) -> x == ok",
        "invariant A.a.finish -> x == nope",
    ] {
        failure(&model("x = A.a(); end done", clause), "E-TYPE-0");
    }
    // Bad event types still fail when A has no lexical call in the selected process.
    failure(
        &model(
            "end done",
            "response A.a.finish(nope) -> one_of(end.done) by 1min after A.a.finish(nope) otherwise unresolved",
        ),
        "E-TYPE-0",
    );
    // A valid enum value absent from the result set is legal and vacuous.
    assert_contracts(
        &check(&model(
            "x = A.a(); end done",
            "invariant A.a.finish(bad) -> x == ok",
        )),
        &["vacuous"],
    );
    failure(
        &model("x = A.a(); end done", "at_most_once A.a.commit per case.id")
            .replace("by 1min", "by 1hour")
            .replace("clock 10min", "clock 10hour"),
        "U-SEM-0",
    );
    let duplicate = model("x = A.a(); end done", "at_most_once A.a.commit per case.id")
        .replace("  bounds", "contract Controls {}\n  bounds");
    failure(&duplicate, "U-SEM-0");
    failure(
        &model(
            "x = A.a(); end done",
            "response A.a.start -> one_of(end.done) by 1hour after A.a.start otherwise unresolved",
        ),
        "U-SEM-0",
    );
    // Value ordinals belong to the Capability's return enum, not the first enum
    // with a matching spelling: T={ok,bad}, Other={bad,ok}.
    let enums = model(
        "x = A.a(); y = B.b(); end done",
        "invariant B.b.finish(ok) -> y == ok; invariant B.b.finish(bad) -> x == ok",
    )
    .replace("actor B", "type Other = enum { bad, ok }; actor B")
    .replace("capability b() -> T", "capability b() -> Other");
    assert_contracts(&check(&enums), &["satisfied", "vacuous"]);
}

#[test]
fn c_static_available_all_callsites_and_own_finish() {
    for kind in ["finish", "finish(ok)", "commit"] {
        assert_contracts(
            &check(&model(
                "x = A.a(); end done",
                &format!("invariant A.a.{kind} -> x == ok"),
            )),
            &["satisfied"],
        );
    }
    for (process, clause) in [
        ("x = A.a(); end done", "invariant A.a.start -> x == ok"),
        (
            "early = A.a(); x = A.a(); end done",
            "invariant A.a.commit -> x == ok",
        ),
        (
            "fork f { x = A.a(); y = B.b() }; join all f; end done",
            "invariant A.a.finish -> y == ok",
        ),
        (
            "x = A.a(); if x == bad { y = B.b() } else {}; end done",
            "invariant A.a.finish -> y == ok",
        ),
        ("end done; x = A.a()", "invariant A.a.start -> x == ok"),
    ] {
        failure(&model(process, clause), "U-SEM-0");
    }
    // Both atoms are checked, even when an earlier atom can short-circuit.
    failure(
        &model(
            "x = A.a(); y = B.b(); end done",
            "invariant A.a.finish -> x == ok || y == ok",
        ),
        "U-SEM-0",
    );
    // A cancelled result is not made available to a later unrelated event.
    let cancelled = CANCEL.replace(
        "invariant Slow.work.finish -> slow == ok",
        "invariant Fast.win.finish -> slow == ok",
    );
    failure(&cancelled, "U-SEM-0");
}

#[test]
fn c_selected_binding_exception_and_global_resolution() {
    let source = DIFF.replace(
        "contract Controls {",
        "contract Controls { invariant Worker.run.finish -> x == ok;",
    );
    let before = serde_json::to_value(report::check_configuration(&source, "Before")).unwrap();
    let after = serde_json::to_value(report::check_configuration(&source, "After")).unwrap();
    assert_contracts(&before, &["satisfied", "satisfied"]);
    assert_contracts(&after, &["vacuous", "vacuous"]);
    let bad = source.replace(
        "process AfterFlow { end done }",
        "process AfterFlow { y = Worker.run(); end done }",
    );
    let after = serde_json::to_value(report::check_configuration(&bad, "After")).unwrap();
    assert!(has_code(&after, "U-SEM-0"), "{after}");
    assert_eq!(after["properties"], json!([]));
    // Nonselected-process binding existence is not permission to invent a global name.
    let unknown = source.replace("x == ok", "unknown == ok");
    let after = serde_json::to_value(report::check_configuration(&unknown, "After")).unwrap();
    assert!(has_code(&after, "E-NAME-0"));
    // End names resolve model-wide, although their reachability is configuration-local.
    let other_end = DIFF
        .replace(
            "process BeforeFlow {",
            "process BeforeFlow { end only_before;",
        )
        .replace("one_of(Worker.run.finish)", "one_of(end.only_before)");
    let after = serde_json::to_value(report::check_configuration(&other_end, "After")).unwrap();
    assert_eq!(after["verdict"], "vacuous", "{after}");
}

#[test]
fn c_requires_event_is_path_local_and_filters_finish_values() {
    let source = include_str!("../../../fixtures/contract-requires.broken-cut-guard/model.relca");
    // fail-only prevents a false finish(pass) match from being hidden by a
    // successful sibling path. Both declaration orders expose leaked A_seen
    // without assuming which result the implementation visits first.
    for results in ["fail", "pass, fail", "fail, pass"] {
        let value = check(&source.replace(
            "result nondet { pass, fail }",
            &format!("result nondet {{ {results} }}"),
        ));
        assert_contracts(&value, &["violated"]);
        assert_eq!(value["verdict"], "violated");
    }
}

#[test]
fn c_ids_and_wire_anonymous_empty_named_and_verify_order() {
    let source = model("x = A.a(); end done", "at_most_once A.a.commit per case.id")
        .replace("contract Controls {", "contract Empty {}\ncontract { invariant A.a.finish -> x == ok }\nverify reachable end.done\ncontract Controls {")
        .replace("per case.id }", "per case.id }\ncontract { response case.received -> one_of(end.done) by 1min after case.received otherwise unresolved }");
    let value = check(&source);
    assert_eq!(value["verdict"], "satisfied", "{value}");
    let ids: Vec<_> = value["properties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "safety no_resource_overcommit",
            "contract #1 invariant",
            "reachable end.done",
            "contract Controls #2 at_most_once",
            "contract #3 response"
        ]
    );
    let mut keys: Vec<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "actors",
            "agency",
            "assumptions",
            "bounds",
            "cut",
            "cut_context",
            "diagnostics",
            "ledger",
            "mode",
            "model",
            "properties",
            "terminals",
            "uncertainty",
            "unit",
            "verdict"
        ]
    );
    assert!(value["model"]["sha256"].is_string());
    assert_eq!(value["terminals"][0]["result_at"], Value::Null);
    assert_eq!(
        relca_core::diag::severity_of("E-CON-0"),
        Some(relca_core::diag::Severity::Error)
    );
    // The same mixed inventory survives a process with no A call/binding x.
    let configured = source.replace("  bounds", "process EmptyFlow { end done }\ncut C { include [A,B] }\nconfiguration From { cut C; process P }\nconfiguration To { cut C; process EmptyFlow }\n  bounds");
    let difference = serde_json::to_value(diff::diff(&configured, "From", "To")).unwrap();
    let rows = difference["properties"].as_array().unwrap();
    assert_eq!(rows.len(), ids.len());
    for (row, id) in rows.iter().zip(ids) {
        assert_eq!(row["id"], id);
        assert_eq!(row["from"], "satisfied");
        assert_eq!(
            row["to"],
            if id.starts_with("contract #1") || id.starts_with("contract Controls #2") {
                "vacuous"
            } else {
                "satisfied"
            }
        );
    }
    assert_eq!(difference["verdict"], "vacuous");
}

#[test]
fn c_response_many_each_trigger_and_shared_target() {
    // A.start0 -> B.finish2 resolves first; A.start2 has no later B target before done3.
    let clause =
        "response A.a.start -> one_of(B.b.finish) by 2min after A.a.start otherwise unresolved";
    assert_contracts(
        &check(&model("x = A.a(); y = B.b(); z = A.a(); end done", clause)),
        &["violated"],
    );
    // Two A starts0, both finish1, one B.finish2 resolves both obligations with deadline2.
    assert_contracts(
        &check(&model(
            "fork f { x = A.a(); z = A.a() }; join all f; y = B.b(); end done",
            clause,
        )),
        &["satisfied"],
    );
}

#[test]
fn c_response_one_of_end_before_deadline_and_if_none() {
    // Only second target occurs at1; empty target sets are covered by syntax tests.
    let source = model(
        "x = A.a(); end done; end other",
        "response case.received -> one_of(end.other, end.done) by 1min after case.received otherwise unresolved",
    );
    assert_contracts(&check(&source), &["satisfied"]);
    assert_contracts(
        &check(
            &source
                .replace("one_of(end.other, end.done)", "one_of(end.other)")
                .replace("by 1min", "by 5min"),
        ),
        &["violated"],
    );
    let none = CANCEL.replace("result nondet { pass }", "result nondet { ok }")
        .replace("contract Controls {", "contract Controls { response case.received -> one_of(end.rejected) by 3min after case.received otherwise unresolved;");
    assert_contracts(
        &check(&none),
        &["satisfied", "satisfied", "satisfied", "violated"],
    );
}

#[test]
fn c_self_finish_commit_bundle_publishes_result_before_conditions() {
    let source = model(
        "x = A.a(); end done",
        r#"
        invariant A.a.finish -> x == ok
        invariant A.a.commit -> x == ok
        response A.a.finish -> one_of(A.a.commit) by 0min after A.a.finish otherwise unresolved
        response A.a.commit -> one_of(A.a.finish) by 0min after A.a.commit otherwise unresolved
        response A.a.finish -> one_of(A.a.finish) by 0min after A.a.finish otherwise unresolved
    "#,
    );
    assert_contracts(&check(&source), &["satisfied"; 5]);
    // Same-bundle matching is response-only; self start is NOT a preceding event.
    assert_contracts(
        &check(&model(
            "x = A.a(); end done",
            "requires_event A.a.start after A.a.start",
        )),
        &["violated"],
    );
}

#[test]
fn c_order_both_directions_and_zero_deadline() {
    let source = ORDER.replace("contract Controls {", "contract Controls { response B.b.finish -> one_of(A.a.finish) by 0min after B.b.finish otherwise unresolved;")
        .replace("by 1min", "by 0min");
    // AB violates B->A, BA violates A->B, both starts orders are also explored.
    assert_contracts(&check(&source), &["violated", "violated"]);
    let starts = model(
        "fork f { x = A.a(); y = B.b() }; join all f; end done",
        "requires_event A.a.start after B.b.start; requires_event B.b.start after A.a.start",
    );
    assert_contracts(&check(&starts), &["violated", "violated"]);
    let mirror = ORDER
        .replace(
            "response A.a.finish -> one_of(B.b.finish)",
            "response B.b.finish -> one_of(A.a.finish)",
        )
        .replace("after A.a.finish", "after B.b.finish");
    assert_contracts(&check(&mirror), &["violated"]);
}

#[test]
fn c_normal_ack_tie_and_cancel_is_not_fulfillment() {
    // Fast finish1, Slow normal2 vs cancellation ack2. Normal activates invariant;
    // ack does NOT fulfill response and produces a violating end.done2 path.
    let tied = CANCEL.replace("fixed 3min", "fixed 2min")
        .replace("contract Controls {", "contract Controls { response Fast.win.finish -> one_of(Slow.work.finish) by 1min after Fast.win.finish otherwise unresolved;");
    assert_contracts(
        &check(&tied),
        &["violated", "satisfied", "satisfied", "satisfied"],
    );
    let response = CANCEL.replace("contract Controls {", "contract Controls { response Fast.win.finish -> one_of(Slow.work.finish) by 2min after Fast.win.finish otherwise unresolved;");
    assert_contracts(
        &check(&response),
        &["violated", "vacuous", "vacuous", "vacuous"],
    );
    let natural = response.replace(
        "cancel cooperative ack_within 1min",
        "cancel forbidden_after_start",
    );
    assert_contracts(&check(&natural), &["satisfied"; 4]);
}

#[test]
fn c_cut_four_stages_and_clock_equality() {
    let pending = RESPONSE.replace("clock 10min", "clock 4min");
    assert_contracts(&check(&pending), &["inconclusive"]);
    assert_contracts(&check(CUT), &["inconclusive"]); // already fulfilled, still cut
    assert_contracts(
        &check(&CUT.replace("one_of(Worker.run.finish)", "one_of(Slow.run.finish)")),
        &["violated"],
    );
    // deadline == clock3 and next event6: already certainly expired.
    assert_contracts(
        &check(
            &CUT.replace("one_of(Worker.run.finish)", "one_of(Slow.run.finish)")
                .replace("by 2min", "by 3min"),
        ),
        &["violated"],
    );
    // Worker.finish3 is processed at clock3 before cutting the subsequent Slow call.
    assert_contracts(
        &check(
            &CUT.replace("fixed 1min", "fixed 3min")
                .replace("by 2min", "by 3min"),
        ),
        &["inconclusive"],
    );
    let inactive = model("x = B.b(); end done", "at_most_once A.a.commit per case.id");
    assert_contracts(&check(&inactive), &["vacuous"]);
    assert_contracts(
        &check(&inactive.replace("clock 10min", "clock 0min")),
        &["inconclusive"],
    );
    let mixed = model(
        "g = B.b(); if g == ok { x = A.a(); end done } else { y = B.long(); end other }",
        "at_most_once A.a.commit per case.id",
    )
    .replace(
        "capability b()",
        "capability long() -> T { duration fixed 5min; result nondet { ok } }; capability b()",
    )
    .replace("result nondet { ok }", "result nondet { ok, bad }")
    .replace("clock 10min", "clock 2min");
    // pass completes activated at2; bad cuts at2 with no A event. Any cut wins
    // over another path's fulfilled/activated clause, but never over violation.
    let mixed = check(&mixed);
    assert_contracts(&mixed, &["inconclusive"]);
    assert_eq!(mixed["bounds"]["truncated"], true);
    assert_eq!(mixed["terminals"][0]["end"], "done");
    assert_eq!(mixed["terminals"][0]["elapsed"], json!({"min":2,"max":2}));
    // Atomic fork exceeds live_tasks before starts. Do not advance received deadline1 into future.
    let live = model("fork f { x = A.a(); y = B.b() }; join all f; end done", "response case.received -> one_of(end.done) by 1min after case.received otherwise unresolved").replace("live_tasks 2", "live_tasks 1");
    assert_contracts(&check(&live), &["inconclusive"]);
}

#[test]
fn c_budget0_budget1_and_retained_violation() {
    let source = model(
        "end done",
        "response case.received -> one_of(end.done) by 0min after case.received otherwise unresolved",
    );
    let ir = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
    let zero = engine::explore_with_budget(&ir, 0);
    assert!(zero.truncated);
    assert!(
        zero.properties
            .iter()
            .all(|p| p.verdict == Verdict::Inconclusive)
    );
    assert!(zero.diagnostics.iter().any(|d| d.code == "W-BUDGET-0"));
    let one = engine::explore_with_budget(&ir, 1);
    assert!(!one.truncated);
    assert!(
        one.properties
            .iter()
            .all(|p| p.verdict == Verdict::Satisfied)
    );
    assert_eq!(one.terminals.len(), 1);
    // A self-precedence violation is certain at first start0; many later states
    // force budget exhaustion. Search budgets, not a brittle state-count literal.
    let source = model(
        "x = A.a(); y = B.b(); z = A.a(); end done",
        "requires_event A.a.start after A.a.start; at_most_once A.a.commit per case.id",
    );
    let ir = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
    let retained = (1..128)
        .map(|n| engine::explore_with_budget(&ir, n))
        .find(|out| out.truncated && out.properties[1].verdict == Verdict::Violated)
        .expect("a budget after first start retains the violation");
    assert_eq!(retained.properties[2].verdict, Verdict::Inconclusive);
    assert!(retained.diagnostics.iter().any(|d| d.code == "W-BUDGET-0"));
    // Preserve an existing reachable witness as well as new clause evidence.
    let witness = model(
        "x = A.a(); if x == ok { end done } else { y = B.b(); z = A.a(); end other }",
        "requires_event A.a.start after A.a.start",
    )
    .replace("result nondet { ok }", "result nondet { ok, bad }")
    .replace(SAFETY, "verify reachable end.done");
    // Either result declaration order is allowed; avoid assuming DFS stack order.
    let retained = [
        witness.clone(),
        witness.replace("{ ok, bad }", "{ bad, ok }"),
    ]
    .iter()
    .any(|source| {
        let ir = lower::lower(&syntax::parse(source).unwrap()).unwrap();
        (1..128)
            .map(|n| engine::explore_with_budget(&ir, n))
            .any(|out| {
                out.truncated
                    && out.properties[0].verdict == Verdict::Satisfied
                    && out.properties[1].verdict == Verdict::Violated
            })
    });
    assert!(
        retained,
        "a visited reachable witness and clause violation survive a later budget cut"
    );
}

#[test]
fn c_abort_retains_known_violation_and_null_truncation() {
    for (process, clauses, rewrite_duration) in [
        (
            "x = A.a(); y = B.b(); end done",
            "requires_event A.a.start after A.a.start; response B.b.finish -> one_of(end.done) by 18446744073709551615min after B.b.finish otherwise unresolved",
            false,
        ),
        (
            "x = A.a(); y = B.b(); end done",
            "requires_event A.a.start after A.a.start; at_most_once A.a.commit per case.id",
            true,
        ),
    ] {
        let mut source =
            model(process, clauses).replace("clock 10min", "clock 18446744073709551615min");
        if rewrite_duration {
            source = source.replace(
                "capability b() -> T { duration fixed 1min",
                "capability b() -> T { duration fixed 18446744073709551615min",
            );
        }
        let value = check(&source);
        assert!(has_code(&value, "U-SEM-0"), "{value}");
        assert_eq!(value["bounds"]["truncated"], Value::Null);
        assert_eq!(value["terminals"], json!([]));
        assert_contracts(&value, &["violated", "unsupported"]);
        assert_eq!(value["verdict"], "violated");
    }
}

#[test]
fn c_deadlock_pending_is_violation_without_synthetic_end() {
    let source = RESPONSE
        .replace("capacity 1", "capacity 0")
        .replace("by 5min", "by 1min");
    let value = check(&source);
    assert_contracts(&value, &["violated"]);
    assert_eq!(value["verdict"], "violated");
    assert_eq!(value["bounds"]["truncated"], false);
    assert_eq!(value["terminals"], json!([]));
    assert!(has_code(&value, "W-DEADLOCK-0"));
    assert!(!has_code(&value, "E-CON-0"));
}

#[test]
fn c_same_bundle_violation_survives_abort_in_either_clause_order() {
    // A.a finishes at 1 (with its commit). The response deadline 1 + max
    // overflows and aborts; the invariant reads x == ok and is violated in
    // the same bundle. The violation is kept whichever clause comes first.
    let response = "response A.a.finish -> one_of(end.done) by 18446744073709551615min after A.a.finish otherwise unresolved";
    let invariant = "invariant A.a.finish -> x == bad";
    for (clauses, expected) in [
        (
            format!("{response}; {invariant}"),
            ["unsupported", "violated"],
        ),
        (
            format!("{invariant}; {response}"),
            ["violated", "unsupported"],
        ),
    ] {
        let value = check(&model("x = A.a(); end done", &clauses));
        assert!(has_code(&value, "U-SEM-0"), "{value}");
        assert_eq!(value["bounds"]["truncated"], Value::Null);
        assert_eq!(value["terminals"], json!([]));
        assert_contracts(&value, &expected);
        assert_eq!(value["verdict"], "violated");
    }
}

#[test]
fn c_deadline_passed_by_silent_ack_before_live_tasks_cut_is_violation() {
    // Hand calculation: Fast and Slow start at 0 (two orders). Fast finishes
    // pass at 1 and is adopted; Slow is asked to cancel and acks at 2 (ack
    // within 1min), before its natural finish at 3. The ack emits no contract
    // event. At 2 the join drains and fork g creates 3 live tasks > 2: cut.
    // The obligation from case.received has deadline 1 < 2 with no end.done,
    // so the response is violated although the path is truncated.
    let source = r#"model M version "0.2" {
  type R = enum { pass, ok }
  actor Fast { capacity 1; capability win() -> R { duration fixed 1min; result nondet { pass }; cancel forbidden_after_start } }
  actor Slow { capacity 1; capability work() -> R { duration fixed 3min; result nondet { ok }; cancel cooperative ack_within 1min } }
  actor W { capacity 3; capability w() -> R { duration fixed 1min; result nondet { ok } } }
  process P {
    fork f { fast = Fast.win(); slow = Slow.work() }
    join any f where result == pass tie by [fast,slow] remaining cancel_then_drain if none -> rejected
    fork g { a = W.w(); b = W.w(); c = W.w() }
    join all g
    end done
    end rejected
  }
  bounds { cases 1; live_tasks 2; clock 10min }
  contract { response case.received -> one_of(end.done) by 1min after case.received otherwise unresolved }
}"#;
    let value = check(source);
    assert_contracts(&value, &["violated"]);
    assert_eq!(value["verdict"], "violated");
    assert_eq!(value["bounds"]["truncated"], true);
    assert_eq!(value["terminals"], json!([]));
    // Same deadline 2: the obligation is not yet late at the cut, so the
    // truncated run stays inconclusive (no future is invented).
    let value = check(&source.replace("by 1min after", "by 2min after"));
    assert_contracts(&value, &["inconclusive"]);
    assert_eq!(value["verdict"], "inconclusive");
}
