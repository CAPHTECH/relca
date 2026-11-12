//! Independent acceptance oracles for ADR 0008, obligations D1-D8.
//! Only the agreed public entry points are used; no AST/IR/lower internals.

use relca_core::{Verdict, diff, report};
use serde_json::{Value, json};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;

const PARALLEL: &str = include_str!("../../../fixtures/config-diff-parallel/model.relca");
const ADD: &str = include_str!("../../../fixtures/config-diff-add-actor/model.relca");
const RETIRE: &str = include_str!("../../../fixtures/config-diff-retire/model.relca");
const UNREACHABLE: &str = include_str!("../../../fixtures/config-diff-unreachable/model.relca");
const UNION: &str = include_str!("../../../fixtures/config-diff-terminal-union/model.relca");

fn check(src: &str, name: &str) -> Value {
    serde_json::to_value(report::check_configuration(src, name)).unwrap()
}

fn compare(src: &str, from: &str, to: &str) -> Value {
    serde_json::to_value(diff::diff(src, from, to)).unwrap()
}

fn has_code(value: &Value, code: &str) -> bool {
    value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == code)
}

fn diagnostic(value: &Value, code: &str, verdict: &str) {
    assert_eq!(value["verdict"], verdict, "{value}");
    assert!(has_code(value, code), "missing {code}: {value}");
    let severity = if code.starts_with("W-") {
        "warning"
    } else {
        "error"
    };
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == code && d["severity"] == severity),
        "{value}"
    );
}

fn no_diff_diagnostics(value: &Value) {
    // ADR 0031 adds warnings for new kinds and missing annotations. This old
    // suite still protects errors and retire-loses; diff2 protects new warnings.
    for code in ["E-DIFF-0", "E-CMP-0", "W-DIFF-2"] {
        assert!(!has_code(value, code), "unexpected {code}: {value}");
    }
}

fn metrics(elapsed: u64, work: u64, result_at: Option<u64>) -> Value {
    json!({"elapsed":{"min":elapsed,"max":elapsed}, "work":{"min":work,"max":work},
        "result_at":result_at.map(|n| json!({"min":n,"max":n}))})
}

/// ADR 0031 rows (every kind except actor/relation), reduced to the hand
/// fields kind, target, change, from, to.
fn new_rows(value: &Value) -> Value {
    json!(
        value["differences"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] != "actor" && r["kind"] != "relation")
            .map(|r| json!({"kind":r["kind"],"target":r["target"],"change":r["change"],"from":r["from"],"to":r["to"]}))
            .collect::<Vec<_>>()
    )
}

fn item<'a>(value: &'a Value, field: &str, key: &str, id: &str) -> &'a Value {
    let items: Vec<_> = value[field]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v[key] == id)
        .filter(|v| {
            field != "differences"
                || v.get("kind").is_none()
                || v["kind"] == "actor"
                || v["kind"] == "relation"
        })
        .collect();
    assert_eq!(items.len(), 1, "{field}/{id}: {value}");
    items[0]
}

#[test]
fn public_configuration_and_diff_entry_points_have_the_agreed_types() {
    let _: fn(&str, &str) -> report::Report = report::check_configuration;
    let _: fn(&str, &str, &str) -> diff::DiffReport = diff::diff;
    let result = diff::diff(PARALLEL, "AsIs", "ToBeP");
    assert_eq!(result.verdict, Verdict::Satisfied);
    assert!(serde_json::to_value(result).is_ok());
}

#[test]
fn adr0008_diff_wire_literal_round_trips_without_added_or_missing_keys() {
    // Deliberately independent of production output and of the CLI matcher.
    // This is a structural wire oracle, not a claim about a model execution.
    // Q3 prelower provenance requires an unknown Actor table (null), not a
    // known empty array: production empty_report keeps actors=None before lower.
    let side_report = ledger_shape::extend_value(
        json!({
            "verdict":"satisfied","mode":"exhaustive","unit":"min",
            "agency":null,
            "actors":null,
            "model":{"name":"Example","sha256":"abc"},"cut":"undeclared",
            "cut_context": {
                "notices": [
                    "Cut 外の存在(未知の被影響者、一度だけの拒否、登録されていない当事者)が作用しないとは主張しない",
                    "この結果を運用の範囲決定に使う場合は採用経路を別に追うこと"
                ],
                "reentry": [], "horizon": []
            },
            "assumptions":[],
            "diagnostics":[{"code":"W-CUT-0","severity":"warning","message":"undeclared","span":null}],
            "bounds":{"cases":1,"live_tasks":2,"clock":10,"truncated":false},
            "properties":[{"id":"reachable end.done","verdict":"satisfied"}],
            "terminals":[{"end":"done","elapsed":{"min":5,"max":5},"work":{"min":8,"max":8},"result_at":{"min":3,"max":3}}]
        }),
        &ledger_shape::Spec::prelower("exhaustive", false),
    );
    let literal = ledger_shape::extend_value(
        json!({
            "verdict":"satisfied",
            // ADR 0043 (plan C07/C08): both sides null, so the new protocol's
            // differences block is not_derived and top unchecked ends with
            // agency_differences.
            "agency":{"from":null,"to":null,"differences":{
                "rules":"agency-diff-p2-1","status":"not_derived","reason":"agency_unavailable",
                "unchecked":["fallback_path","workload_transfer","via_model_paths"]}},
            "from":{"configuration":"A","report":side_report},
            "to":{"configuration":"B","report":side_report},
            "mismatch":null,
            "differences":[{"kind":"actor","target":"Extra","change":"added","reason":"new step","loses":null}],
            "terminals":[{"end":"done","from":metrics(5,8,Some(3)),"to":null}],
            "properties":[{"id":"reachable end.done","from":"satisfied","to":"satisfied"}],
            "diagnostics":[],
            "checked":["comparison_mismatch","actor","relation","intent_claims","retire_loses"],
            "unchecked":["time_intent","resource","meaning","authority","visibility","contest","preserve","override","maintain","agency_differences"]
        }),
        &ledger_shape::Spec::aggregate("diff"),
    );
    let restored: diff::DiffReport = serde_json::from_value(literal.clone()).unwrap();
    assert_eq!(serde_json::to_value(restored).unwrap(), literal);
    let mut detailed = literal;
    detailed["mismatch"] = json!({"from_cut":"C","to_cut":"D","differences":[
        {"field":"question","from":null,"to":"new question"},
        {"field":"include","from":["Office"],"to":["Office","Extra"]},
        {"field":"exclude","from":[],"to":[{"target":"Payment","actor":false,"reason":"outside","loses":null}]},
        {"field":"defer","from":[],"to":[{"target":"Office.c.rates","actor":false,"reason":"unknown rate","loses":"load"}]},
        {"field":"externality","from":[],"to":["Vendor.effort"]},
        {"field":"reentry","from":[],"to":["review quarterly"]}
    ]});
    let restored: diff::DiffReport = serde_json::from_value(detailed.clone()).unwrap();
    assert_eq!(serde_json::to_value(restored).unwrap(), detailed);
}

#[test]
fn selection_keeps_process_local_state_and_full_source_hash() {
    let serial = check(PARALLEL, "AsIs");
    let parallel = check(PARALLEL, "ToBeP");
    for (value, elapsed) in [(&serial, 8), (&parallel, 5)] {
        assert_eq!(value["verdict"], "satisfied", "{value}");
        assert_eq!(value["bounds"]["truncated"], false);
        let terminal = item(value, "terminals", "end", "done");
        assert_eq!(terminal["elapsed"], json!({"min":elapsed,"max":elapsed}));
        assert_eq!(terminal["work"], json!({"min":8,"max":8}));
        assert_eq!(terminal["result_at"], Value::Null);
        assert_eq!(
            ledger_shape::original_field_count(value),
            13,
            "ADR 0023 adds only actors and cut_context to Report"
        );
    }
    assert_eq!(serial["model"]["sha256"], parallel["model"]["sha256"]);
    assert!(serial["model"]["sha256"].is_string());
    let changed = format!("{PARALLEL}\n// source bytes, not selected IR\n");
    assert_ne!(
        check(&changed, "AsIs")["model"]["sha256"],
        serial["model"]["sha256"]
    );
}

#[test]
fn manual_parallel_add_retire_and_unreachable_oracles() {
    // New rows by hand (ADR 0031): all Actors here are anonymous without
    // permit, so no authority row; Office's two calls change sequential ->
    // fork; an Actor activated or deactivated changes its resource cell.
    let anon = |n: u64| json!({"capacity":n,"executors":null});
    for (src, to, before, after, work_before, work_after, expected, typed) in [
        (
            PARALLEL,
            "ToBeP",
            8,
            5,
            8,
            8,
            json!([]),
            json!([
                {"kind":"time","target":"Office.long","change":"modified","from":["sequential"],"to":["fork"]},
                {"kind":"time","target":"Office.short","change":"modified","from":["sequential"],"to":["fork"]}
            ]),
        ),
        (
            ADD,
            "ToBe",
            3,
            5,
            3,
            5,
            json!([
                {"kind":"actor","target":"Extractor","change":"added","reason":"automate","loses":null},
                {"kind":"relation","target":"Extractor.extract","change":"added","reason":"extract step","loses":null}
            ]),
            json!([{"kind":"resource","target":"Extractor","change":"added","from":null,"to":anon(1)}]),
        ),
        (
            RETIRE,
            "ToBe",
            5,
            3,
            5,
            3,
            json!([
                {"kind":"actor","target":"Extractor","change":"removed","reason":"remove automation","loses":"automation removed"},
                {"kind":"relation","target":"Extractor.extract","change":"removed","reason":"remove step","loses":"extraction removed"}
            ]),
            json!([{"kind":"resource","target":"Extractor","change":"removed","from":anon(1),"to":null}]),
        ),
        (
            UNREACHABLE,
            "ToBe",
            1,
            1,
            1,
            1,
            json!([
                {"kind":"actor","target":"Extra","change":"added","reason":"fallback actor","loses":null},
                {"kind":"relation","target":"Extra.run","change":"added","reason":"fallback step","loses":null}
            ]),
            json!([{"kind":"resource","target":"Extra","change":"added","from":null,"to":anon(1)}]),
        ),
    ] {
        let value = compare(src, "AsIs", to);
        assert_eq!(new_rows(&value), typed, "{to}");
        assert_eq!(value["verdict"], "satisfied", "{value}");
        assert_eq!(value["mismatch"], Value::Null);
        let mut actual: Vec<_> = value["differences"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "actor" || r["kind"] == "relation")
            .cloned()
            .collect();
        let mut expected = expected.as_array().unwrap().clone();
        actual.sort_by_key(Value::to_string);
        expected.sort_by_key(Value::to_string);
        assert_eq!(actual, expected);
        let terminal = item(&value, "terminals", "end", "done");
        assert_eq!(terminal["from"], metrics(before, work_before, None));
        assert_eq!(terminal["to"], metrics(after, work_after, None));
        for (side, elapsed, work) in [("from", before, work_before), ("to", after, work_after)] {
            let mut raw = item(&value[side]["report"], "terminals", "end", "done").clone();
            raw.as_object_mut().unwrap().remove("end");
            assert_eq!(raw, metrics(elapsed, work, None));
        }
        for id in ["reachable end.done", "safety no_resource_overcommit"] {
            assert_eq!(
                item(&value, "properties", "id", id),
                &json!({"id":id,"from":"satisfied","to":"satisfied"})
            );
        }
        no_diff_diagnostics(&value);
    }
}

#[test]
fn terminal_inventory_is_model_wide_and_union_has_null_not_zero() {
    let value = compare(UNION, "AsIs", "ToBe");
    assert_eq!(value["verdict"], "violated", "{value}");
    assert_eq!(value["terminals"].as_array().unwrap().len(), 2);
    assert_eq!(
        item(&value, "terminals", "end", "old"),
        &json!({"end":"old","from":metrics(3,3,None),"to":null})
    );
    assert_eq!(
        item(&value, "terminals", "end", "new"),
        &json!({"end":"new","from":null,"to":metrics(3,3,None)})
    );
    assert_eq!(
        item(&value, "properties", "id", "reachable end.old"),
        &json!({"id":"reachable end.old","from":"satisfied","to":"violated"})
    );
    assert_eq!(
        item(&value, "properties", "id", "reachable end.new"),
        &json!({"id":"reachable end.new","from":"violated","to":"satisfied"})
    );
    no_diff_diagnostics(&value);
}

#[test]
fn syntax_boundaries_fail_closed_with_decided_diagnostic_classes() {
    for (replacement, code, verdict) in [
        ("process Serial; cut C", "E-SYN-0", "invalid"),
        ("cut C", "E-SYN-0", "invalid"),
        ("process Serial", "E-SYN-0", "invalid"),
        (
            "cut C; process Serial; intent { add Office.short.extra reason \"too deep\" }",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "cut C; process Serial; intent { preserve_fallback Office }",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "cut C; process Serial; intent { override_rule Office }",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "cut C; process Serial; intent { maintain_fallback Office }",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "cut C; process Serial; intent { kernel Office }",
            "U-SYN-0",
            "unsupported",
        ),
    ] {
        let src = PARALLEL.replacen("cut C; process Serial", replacement, 1);
        diagnostic(&check(&src, "AsIs"), code, verdict);
    }
}

#[test]
fn known_configuration_and_intent_words_in_wrong_positions_are_structural_errors() {
    for replacement in [
        "cut C; cut C; process Serial",
        "cut C; process Serial; process Serial",
        "cut C; process Serial; cut C",
        "cut C; process Serial; intent {} intent {}",
        "cut C; process Serial; intent { reason \"orphan\" }",
        "cut C; process Serial; intent { loses \"orphan\" }",
        "cut C; process Serial; intent { intent {} }",
        "cut C; process Serial; intent { add Office reason \"a\" reason \"b\" }",
        "cut C; process Serial; intent { retire Office reason \"a\" loses \"b\" loses \"c\" }",
        "cut C; process Serial; intent { retire Office loses \"b\" reason \"a\" }",
        "cut C; process Serial; intent { add Office; reason \"a\" }",
        "cut C; process Serial; add Office reason \"a\"",
    ] {
        for text in [replacement.to_owned(), replacement.replace(' ', "\n")] {
            let src = PARALLEL.replacen("cut C; process Serial", &text, 1);
            diagnostic(&check(&src, "AsIs"), "E-SYN-0", "invalid");
        }
    }
}

#[test]
fn unknown_configuration_and_intent_clauses_stay_unsupported() {
    for replacement in [
        "unknown C; process Serial",
        "cut C; unknown Serial",
        "cut C; process Serial; unknown Office",
        "cut C; process Serial; intent { unknown Office }",
        "cut C; process Serial; intent { add Office unknown \"a\" }",
        "cut C; process Serial; intent { add Office reason \"a\" unknown \"b\" }",
    ] {
        let src = PARALLEL.replacen("cut C; process Serial", replacement, 1);
        diagnostic(&check(&src, "AsIs"), "U-SYN-0", "unsupported");
    }
}

#[test]
fn add_followed_by_loses_is_structural_error() {
    let src = ADD.replace(
        "add Extractor reason \"automate\"",
        "add Extractor reason \"automate\" loses \"not a retirement\"",
    );
    assert_ne!(src, ADD);
    diagnostic(&check(&src, "ToBe"), "E-SYN-0", "invalid");
}

#[test]
fn contextual_intent_words_do_not_become_global_reserved_identifiers() {
    let src = PARALLEL
        .replace("Office", "add")
        .replace("short", "retire")
        .replace("Serial", "intent");
    assert_eq!(check(&src, "AsIs")["verdict"], "satisfied");
    let newline = PARALLEL.replace("cut C; process Serial", "cut C\nprocess Serial");
    assert_eq!(check(&newline, "AsIs")["verdict"], "satisfied");
}

#[test]
fn names_and_intent_targets_are_global_declarations_not_selected_flow() {
    for (old, new) in [
        ("cut C; process Parallel", "cut Absent; process Parallel"),
        ("cut C; process Parallel", "cut C; process Absent"),
        (
            "cut C; process Parallel",
            "cut C; process Parallel; intent { add Absent reason \"bad actor\" }",
        ),
        (
            "cut C; process Parallel",
            "cut C; process Parallel; intent { add Office.absent reason \"bad capability\" }",
        ),
    ] {
        let src = PARALLEL.replace(old, new);
        diagnostic(&check(&src, "AsIs"), "E-NAME-0", "invalid");
    }
    diagnostic(&check(PARALLEL, "Unknown"), "E-NAME-0", "invalid");
}

#[test]
fn duplicate_configuration_process_cut_and_intent_are_unsupported() {
    for addition in [
        "configuration AsIs { cut C; process Serial }",
        "process Serial { end done }",
        "cut C { include [Office] }",
    ] {
        let src = PARALLEL.replace("  bounds", &format!("  {addition}\n  bounds"));
        diagnostic(&check(&src, "AsIs"), "U-SEM-0", "unsupported");
    }
    for claims in [
        "add Office reason \"first\"; add Office reason \"second\"",
        "add Office reason \"first\"; retire Office reason \"second\"",
    ] {
        let src = PARALLEL.replace(
            "cut C; process Parallel",
            &format!("cut C; process Parallel; intent {{ {claims} }}"),
        );
        diagnostic(&check(&src, "AsIs"), "U-SEM-0", "unsupported");
    }
}

#[test]
fn nonselected_flow_is_not_analysed_but_unused_capability_is_checked() {
    let src = PARALLEL.replace(
        "fork f { a = Office.short(); b = Office.long() }",
        "fork f { a = Missing.short(); b = Office.long() }",
    );
    assert_eq!(check(&src, "AsIs")["verdict"], "satisfied");
    diagnostic(&check(&src, "ToBeP"), "E-NAME-0", "invalid");
    let orphan = PARALLEL.replace("  bounds", "  process Orphan { bad = Missing.call(); end orphan }\n  cut OrphanCut { include [Office] }\n  bounds");
    diagnostic(&check(&orphan, "AsIs"), "W-CONF-0", "satisfied");
    let capacity = UNREACHABLE.replace("actor Extra { capacity 1;", "actor Extra {");
    diagnostic(&check(&capacity, "AsIs"), "E-CAP-0", "invalid");
}

#[test]
fn all_cut_memberships_and_all_referencing_process_excluded_calls_are_checked() {
    // D2=B: even checking AsIs checks the calls of ToBe against ToBe's Cut.
    let excluded = ADD
        .replace(
            "cut D { include [Office, Extractor] }",
            "cut D { include [Office]; exclude Extractor reason \"outside\" loses \"extraction\" }",
        )
        .replace("configuration ToBe { cut C", "configuration ToBe { cut D");
    diagnostic(&check(&excluded, "AsIs"), "E-CUT-1", "invalid");
    // The classes are
    // ADR 0007's, applied to every Cut. `include []` is outside the grammar
    // (E-SYN-0), a duplicate include is U-SEM-0 / unsupported, an undeclared
    // include is E-NAME-0, and a membership conflict is E-CUT-1. Defer of an
    // excluded Actor needs a second Actor and stays covered by lower's tests.
    for (bad_cut, code, verdict) in [
        ("cut OrphanCut { include [] }", "E-SYN-0", "invalid"),
        (
            "cut OrphanCut { include [Office, Office] }",
            "U-SEM-0",
            "unsupported",
        ),
        (
            "cut OrphanCut { include [Office, Missing] }",
            "E-NAME-0",
            "invalid",
        ),
        (
            "cut OrphanCut { include [Office]; exclude Office reason \"duplicate\" loses \"x\" }",
            "E-CUT-1",
            "invalid",
        ),
    ] {
        let src = PARALLEL.replace("  bounds", &format!("  {bad_cut}\n  bounds"));
        diagnostic(&check(&src, "AsIs"), code, verdict);
    }
}

#[test]
fn excluded_call_check_does_not_cross_cut_reference_scope() {
    let src = ADD.replace(
        "cut D { include [Office, Extractor] }",
        "cut D { include [Office]; exclude Extractor reason \"outside\" loses \"extraction\" }",
    );
    assert_ne!(src, ADD);
    // Only After calls Extractor; it uses C. Hold uses D with Before.
    for configuration in ["AsIs", "ToBe", "Hold"] {
        let value = check(&src, configuration);
        assert_eq!(value["verdict"], "satisfied", "{configuration}: {value}");
        assert!(!has_code(&value, "E-CUT-1"), "{value}");
    }
}

#[test]
fn unused_cut_loses_warnings_and_nonselected_flow_availability_have_separate_scopes() {
    let loses = PARALLEL.replace(
        "  bounds",
        "  cut Unused { include [Office]; exclude Payment reason \"outside\" }\n  bounds",
    );
    let value = check(&loses, "AsIs");
    diagnostic(&value, "W-CONF-0", "satisfied");
    diagnostic(&value, "W-CUT-1", "satisfied");
    // The syntax makes
    // `join` follow its `fork` syntactically, so an `if` between them is a
    // whole-model E-SYN-0. Read the result before the fork instead; lower
    // reports an unavailable result as U-SEM-0 / unsupported.
    let unavailable = PARALLEL.replace(
        "    fork f {",
        "    if a == pass { end done } else { end done }\n    fork f {",
    );
    assert_eq!(check(&unavailable, "AsIs")["verdict"], "satisfied");
    diagnostic(&check(&unavailable, "ToBeP"), "U-SEM-0", "unsupported");
}

#[test]
fn only_to_intent_counts_and_wrong_direction_does_not_justify_a_difference() {
    let from_only = ADD.replace("configuration AsIs { cut C; process Before }", "configuration AsIs { cut C; process Before; intent { add Extractor reason \"automate\"; add Extractor.extract reason \"extract step\" } }")
        .replace("configuration ToBe { cut C; process After; intent {\n    add Extractor reason \"automate\"\n    add Extractor.extract reason \"extract step\"\n  } }", "configuration ToBe { cut C; process After }");
    let value = compare(&from_only, "AsIs", "ToBe");
    diagnostic(&value, "W-DIFF-1", "satisfied");
    assert!(
        value["differences"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["reason"].is_null())
    );
    let wrong = ADD.replace("add Extractor reason", "retire Extractor reason");
    let value = compare(&wrong, "AsIs", "ToBe");
    diagnostic(&value, "E-DIFF-0", "invalid");
    assert!(has_code(&value, "W-DIFF-1"));
    assert!(
        has_code(&value, "W-DIFF-2"),
        "D6: all to retire claims need loses, including invalid claims"
    );
    assert!(
        !has_code(&check(&wrong, "ToBe"), "W-DIFF-2"),
        "check is not diff"
    );
}

#[test]
fn capability_only_intent_does_not_explain_actor_change() {
    let src = ADD.replace("    add Extractor reason \"automate\"\n", "");
    assert_ne!(src, ADD);
    let value = compare(&src, "AsIs", "ToBe");
    diagnostic(&value, "W-DIFF-1", "satisfied");
    assert_eq!(
        item(&value, "differences", "target", "Extractor")["reason"],
        Value::Null
    );
    assert_eq!(
        item(&value, "differences", "target", "Extractor.extract")["reason"],
        "extract step"
    );
    assert!(!has_code(&value, "E-DIFF-0"));
}

#[test]
fn static_walker_covers_call_fork_then_else_nested_if_and_statements_after_end() {
    // New rows by hand (ADR 0031 U1: lexical candidates, dead arms and the
    // suffix after an end included): Extra (capacity 1, anonymous) becomes
    // active in every body -> resource Extra added. Only the fork body also
    // calls Gate.pick as a fork branch -> time Gate.pick [sequential] ->
    // [sequential, fork]. Anonymous without permit: no authority row.
    let extra = json!({"kind":"resource","target":"Extra","change":"added","from":null,"to":{"capacity":1,"executors":null}});
    let gate = json!({"kind":"time","target":"Gate.pick","change":"modified","from":["sequential"],"to":["sequential","fork"]});
    for (body, elapsed, work, typed) in [
        ("y = Extra.run(); end done", 3, 3, json!([extra])),
        (
            "fork f { y = Extra.run(); z = Gate.pick() }; join all f; end done",
            3,
            4,
            json!([extra, gate]),
        ),
        (
            "if x == pass { y = Extra.run() } else { }; end done",
            3,
            3,
            json!([extra]),
        ),
        (
            "if x == pass { end done } else { y = Extra.run(); end done }",
            1,
            1,
            json!([extra]),
        ),
        (
            "if x == pass { end done } else { if x == pass { end done } else { y = Extra.run(); end done } }",
            1,
            1,
            json!([extra]),
        ),
        ("end done; y = Extra.run()", 1, 1, json!([extra])),
    ] {
        let src = UNREACHABLE.replace(
            "if x == pass { end done } else { y = Extra.run(); end done }",
            body,
        );
        let value = compare(&src, "AsIs", "ToBe");
        assert_eq!(value["verdict"], "satisfied", "{body}: {value}");
        assert_eq!(new_rows(&value), typed, "{body}");
        assert_eq!(
            value["differences"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["kind"] == "actor" || r["kind"] == "relation")
                .count(),
            2,
            "{body}"
        );
        assert_eq!(
            item(&value, "terminals", "end", "done")["to"],
            metrics(elapsed, work, None),
            "{body}"
        );
        no_diff_diagnostics(&value);
    }
}

#[test]
fn static_differences_are_sets_not_call_order_multiplicity_or_binding_names() {
    // long 0→5, short 5→8, short 8→11; work=5+3+3=11.
    let src = PARALLEL
        .replace(
            "fork f { a = Office.short(); b = Office.long() }\n    join all f",
            "renamed = Office.long(); other = Office.short(); again = Office.short()",
        )
        .replace("clock 10min", "clock 12min");
    let value = compare(&src, "AsIs", "ToBeP");
    assert_eq!(value["verdict"], "satisfied");
    assert_eq!(value["differences"], json!([]));
    assert_eq!(
        item(&value, "terminals", "end", "done")["to"],
        metrics(11, 11, None)
    );
    no_diff_diagnostics(&value);
}

#[test]
fn same_configuration_still_checks_surplus_to_claims() {
    let value = compare(PARALLEL, "AsIs", "AsIs");
    assert_eq!(value["verdict"], "satisfied");
    assert_eq!(value["differences"], json!([]));
    let terminal = item(&value, "terminals", "end", "done");
    assert_eq!(terminal["from"], terminal["to"]);
    let value = compare(ADD, "ToBe", "ToBe");
    diagnostic(&value, "E-DIFF-0", "invalid");
    assert_eq!(value["differences"], json!([]));
}

#[test]
fn static_failure_keeps_both_reports_and_does_not_claim_comparison_checks() {
    let value = compare(PARALLEL, "Unknown", "ToBeP");
    assert_eq!(value["verdict"], "invalid");
    diagnostic(&value["from"]["report"], "E-NAME-0", "invalid");
    assert_eq!(value["from"]["report"]["bounds"]["truncated"], Value::Null);
    assert_eq!(value["to"]["report"]["verdict"], "satisfied");
    for field in ["differences", "terminals", "properties", "checked"] {
        assert_eq!(value[field], json!([]));
    }
    assert_eq!(value["mismatch"], Value::Null);
    assert!(
        !has_code(&value, "E-NAME-0"),
        "root diagnostics are diff-only (D5)"
    );
    for unperformed in [
        "comparison_mismatch",
        "actor",
        "relation",
        "intent_claims",
        "retire_loses",
    ] {
        assert!(
            value["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!(unperformed))
        );
    }
    let duplicate = PARALLEL.replace(
        "  bounds",
        "  configuration AsIs { cut C; process Serial }\n  bounds",
    );
    let value = compare(&duplicate, "AsIs", "ToBeP");
    assert_eq!(
        value["verdict"], "unsupported",
        "D8 preserves U-* classification"
    );
    assert_eq!(value["differences"], json!([]));
}

#[test]
fn mismatch_is_cut_identity_and_details_include_metadata_not_description_order() {
    let src = ADD.replace("configuration ToBe { cut C", "configuration ToBe { cut D");
    let value = compare(&src, "AsIs", "ToBe");
    diagnostic(&value, "E-CMP-0", "invalid");
    assert_eq!(
        value["mismatch"],
        json!({"from_cut":"C","to_cut":"D","differences":[]})
    );
    assert_eq!(value["checked"], json!(["comparison_mismatch"]));
    let reordered = src.replace(
        "cut D { include [Office, Extractor] }",
        "cut D { include [Extractor, Office] }",
    );
    assert_eq!(
        compare(&reordered, "AsIs", "ToBe")["mismatch"]["differences"],
        json!([])
    );
    let metadata = src.replace("cut C { include [Office, Extractor] }", "cut C { question \"old question\"; include [Office, Extractor]; exclude Payment reason \"old reason\" loses \"old loss\"; defer Office.c.rates reason \"old rates\" loses \"old uncertainty\"; externality [Vendor.effort]; reentry [review quarterly] }")
        .replace("cut D { include [Office, Extractor] }", "cut D { question \"new question\"; include [Office, Extractor]; exclude Payment reason \"new reason\" loses \"new loss\"; defer Office.c.rates reason \"new rates\" loses \"new uncertainty\"; externality [Vendor.cost]; reentry [review monthly] }");
    let value = compare(&metadata, "AsIs", "ToBe");
    diagnostic(&value, "E-CMP-0", "invalid");
    let details = &value["mismatch"];
    assert_eq!(
        item(details, "differences", "field", "question"),
        &json!({"field":"question","from":"old question","to":"new question"})
    );
    assert_eq!(
        item(details, "differences", "field", "exclude"),
        &json!({"field":"exclude","from":[{"target":"Payment","actor":false,"reason":"old reason","loses":"old loss"}],"to":[{"target":"Payment","actor":false,"reason":"new reason","loses":"new loss"}]})
    );
    assert_eq!(
        item(details, "differences", "field", "defer"),
        &json!({"field":"defer","from":[{"target":"Office.c.rates","actor":false,"reason":"old rates","loses":"old uncertainty"}],"to":[{"target":"Office.c.rates","actor":false,"reason":"new rates","loses":"new uncertainty"}]})
    );
    assert_eq!(
        item(details, "differences", "field", "externality")["to"],
        json!(["Vendor.cost"])
    );
    assert_eq!(
        item(details, "differences", "field", "reentry")["to"],
        json!(["review monthly"])
    );
    assert_eq!(details["differences"].as_array().unwrap().len(), 5);
}

#[test]
fn mismatch_exclude_and_defer_are_independent_of_description_order() {
    for clause in ["exclude", "defer"] {
        let first = format!("{clause} Payment reason \"outside\" loses \"payment\"");
        let second = format!("{clause} Resubmission reason \"later\" loses \"retry\"");
        let src = ADD
            .replace(
                "cut C { include [Office, Extractor] }",
                &format!("cut C {{ include [Office, Extractor]; {first}; {second} }}"),
            )
            .replace(
                "cut D { include [Office, Extractor] }",
                &format!("cut D {{ include [Office, Extractor]; {second}; {first} }}"),
            )
            .replace("configuration ToBe { cut C", "configuration ToBe { cut D");
        let value = compare(&src, "AsIs", "ToBe");
        diagnostic(&value, "E-CMP-0", "invalid");
        assert_eq!(value["from"]["report"]["verdict"], "satisfied");
        assert_eq!(value["to"]["report"]["verdict"], "satisfied");
        assert_eq!(
            value["mismatch"],
            json!({"from_cut":"C","to_cut":"D","differences":[]}),
            "{clause}: {value}"
        );
    }
}

#[test]
fn cut_mismatch_membership_details_use_declared_actor_flags() {
    let src=ADD.replace("  process Before", "  actor Archive { }\n  process Before")
        .replace("cut C { include [Office, Extractor] }", "cut C { include [Office, Extractor, Archive] }")
        .replace("cut D { include [Office, Extractor] }", "cut D { include [Office, Extractor]; exclude Archive reason \"archive outside\" loses \"storage coverage\" }")
        .replace("configuration ToBe { cut C", "configuration ToBe { cut D");
    let value = compare(&src, "AsIs", "ToBe");
    diagnostic(&value, "E-CMP-0", "invalid");
    let details = &value["mismatch"];
    for (side, expected) in [
        ("from", vec!["Archive", "Extractor", "Office"]),
        ("to", vec!["Extractor", "Office"]),
    ] {
        let mut names: Vec<_> = item(details, "differences", "field", "include")[side]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, expected);
    }
    assert_eq!(
        item(details, "differences", "field", "exclude"),
        &json!({"field":"exclude","from":[],"to":[{"target":"Archive","actor":true,"reason":"archive outside","loses":"storage coverage"}]})
    );
    assert_eq!(details["differences"].as_array().unwrap().len(), 2);
    for field in ["differences", "terminals", "properties"] {
        assert_eq!(value[field], json!([]));
    }
}

#[test]
fn clock_cut_is_inconclusive_not_success_and_side_deadlock_is_not_erased() {
    let cut = PARALLEL.replace("clock 10min", "clock 4min");
    let value = compare(&cut, "AsIs", "ToBeP");
    assert_eq!(value["verdict"], "inconclusive", "{value}");
    for side in ["from", "to"] {
        assert_eq!(value[side]["report"]["bounds"]["truncated"], true);
    }
    let partial = PARALLEL.replace("clock 10min", "clock 6min");
    let value = compare(&partial, "AsIs", "ToBeP");
    assert_eq!(value["verdict"], "inconclusive");
    assert_eq!(value["from"]["report"]["bounds"]["truncated"], true);
    assert_eq!(value["to"]["report"]["bounds"]["truncated"], false);
    assert_eq!(
        item(&value, "terminals", "end", "done"),
        &json!({"end":"done","from":null,"to":metrics(5,8,None)})
    );
    assert!(
        !value["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("terminals"))
    );
    let deadlock = PARALLEL
        .replace("capacity 2", "capacity 0")
        .replace("  verify reachable end.done\n", "");
    let value = compare(&deadlock, "AsIs", "ToBeP");
    assert_eq!(
        value["verdict"], "violated",
        "side report verdict, not just property verdicts"
    );
    for side in ["from", "to"] {
        diagnostic(&value[side]["report"], "W-DEADLOCK-0", "violated");
        assert!(
            value[side]["report"]["properties"]
                .as_array()
                .unwrap()
                .iter()
                .all(|p| p["verdict"] == "satisfied")
        );
    }
}

#[test]
fn any_drain_projects_nonnull_result_at_without_recomputing_elapsed_or_work() {
    let src = PARALLEL.replace("result nondet { pass }", "result nondet { pass }; cancel forbidden_after_start")
        .replace("join all f", "join any f where result == pass tie by [a, b] remaining cancel_then_drain if none -> no_match")
        .replace("  cut C", "  process NamesOnly { end no_match }\n  configuration Names { cut C; process NamesOnly }\n  cut C");
    let value = compare(&src, "ToBeP", "ToBeP");
    assert_eq!(value["verdict"], "satisfied", "{value}");
    let terminal = item(&value, "terminals", "end", "done");
    assert_eq!(terminal["from"], metrics(5, 8, Some(3)));
    assert_eq!(terminal["to"], metrics(5, 8, Some(3)));
}

#[test]
fn engine_abort_after_lower_preserves_static_diff_and_unanalysed_truncation() {
    let src = ADD
        .replace(
            "duration fixed 2min",
            "duration fixed 18446744073709551615min",
        )
        .replace("clock 10min", "clock 18446744073709551615min");
    let value = compare(&src, "AsIs", "ToBe");
    assert_eq!(value["from"]["report"]["verdict"], "satisfied");
    diagnostic(&value["to"]["report"], "U-SEM-0", "unsupported");
    assert_eq!(value["to"]["report"]["bounds"]["truncated"], Value::Null);
    assert_eq!(value["verdict"], "unsupported");
    assert_eq!(
        value["differences"].as_array().unwrap().len(),
        3,
        "D8=A1 still computes static changes"
    );
    assert_eq!(value["terminals"], json!([]));
    assert!(
        value["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("terminals"))
    );
    assert_eq!(value["checked"].as_array().unwrap().len(), 15);
    assert_eq!(
        item(&value, "properties", "id", "reachable end.done"),
        &json!({"id":"reachable end.done","from":"satisfied","to":"unsupported"})
    );
    let reversed = compare(&src, "ToBe", "AsIs");
    assert_eq!(reversed["terminals"], json!([]));
    assert!(
        reversed["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("terminals"))
    );
    assert_eq!(
        item(&reversed, "properties", "id", "reachable end.done"),
        &json!({"id":"reachable end.done","from":"unsupported","to":"satisfied"})
    );
}
