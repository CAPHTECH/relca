//! ADR 0009 acceptance: hand oracles, only public report/check/diff entry points.
//! No dependency on the proposed agency module or its implementation types.

use relca_core::{diff, report};
use serde_json::{Value, json};
use std::path::Path;
#[path = "support/agency_viewpoints_legacy.rs"]
mod agency_legacy;

const ORIGIN: &str = include_str!("../../../fixtures/agency-origin/model.relca");
const ANY: &str = include_str!("../../../fixtures/agency-any/model.relca");
const REFUSAL: &str = include_str!("../../../fixtures/agency-refusal/model.relca");
const LINEAGE: &str = include_str!("../../../fixtures/agency-lineage/model.relca");

fn check(source: &str) -> Value {
    serde_json::to_value(report::check(source)).unwrap()
}

fn agency(report: &Value) -> &Value {
    let value = report
        .get("agency")
        .expect("agency must be explicit on wire");
    assert!(value.is_object(), "expected lowered agency: {report}");
    assert_eq!(value["rules"], "agency-p0-1");
    // ADR 0043 (plan C03): executors leaves unchecked only when the effects
    // viewpoint is derived; position and order of the rest are unchanged.
    // Plan C04: no source in this file declares people, so the lowered report
    // appends responsibility_people_undeclared last.
    let mut expected = vec![
        "W-RESP-1",
        "contest_path",
        "fallback_path",
        "executors",
        "via_model",
    ];
    if value["viewpoints"]["effects"]["status"] == "derived" {
        expected.retain(|x| *x != "executors");
    }
    expected.push("responsibility_people_undeclared");
    assert_eq!(value["unchecked"], json!(expected));
    value
}

fn has_code(report: &Value, code: &str) -> bool {
    report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == code)
}

fn refusal(call: &str, capability: &str, stops: &str, bypasses: u64) -> Value {
    json!({"call":call,"capability":capability,"stops":stops,"bypasses":bypasses})
}

fn static_failure(report: &Value, code: &str) {
    assert!(has_code(report, code), "missing {code}: {report}");
    assert_ne!(report["verdict"], "satisfied", "{report}");
    assert_eq!(report.get("agency"), Some(&Value::Null));
    assert_eq!(report["properties"], json!([]));
    assert_eq!(report["terminals"], json!([]));
    assert_eq!(report["bounds"]["truncated"], Value::Null);
}

fn fixture(name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name);
    let source = std::fs::read_to_string(dir.join("model.relca")).unwrap();
    let expected: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
    if name.starts_with("agency-refusal") {
        let actual = serde_json::to_value(diff::diff(&source, "AsIs", "ToBe")).unwrap();
        assert_eq!(actual["verdict"], "satisfied", "{actual}");
        let mut legacy_sides = actual["agency"].clone();
        // ADR 0043 (plan C07): every diff carries the new differences block;
        // the old hand oracle compares only from/to.
        assert!(
            legacy_sides
                .as_object_mut()
                .unwrap()
                .remove("differences")
                .is_some_and(|d| d.is_object())
        );
        for side in ["from", "to"] {
            legacy_sides[side] = agency_legacy::agency(&actual["agency"][side]);
        }
        assert_eq!(legacy_sides, expected["agency"], "{name}");
        for (side, configuration) in [("from", "AsIs"), ("to", "ToBe")] {
            let selected =
                serde_json::to_value(report::check_configuration(&source, configuration)).unwrap();
            assert_eq!(
                agency_legacy::agency(agency(&selected)),
                expected["agency"][side]
            );
            assert_eq!(actual["agency"][side], actual[side]["report"]["agency"]);
        }
        assert!(
            !actual["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("agency"))
        );
    } else {
        let actual = check(&source);
        assert_eq!(actual["verdict"], "satisfied", "{actual}");
        assert_eq!(
            agency_legacy::agency(agency(&actual)),
            expected["agency"],
            "{name}"
        );
    }
}

macro_rules! fixture_tests {
    ($($test:ident => $name:literal),+ $(,)?) => { $(
        #[test] fn $test() { fixture($name); }
    )+ };
}
fixture_tests! {
    f1_lineage => "agency-lineage",
    f1_missing_evidence => "agency-lineage.broken-missing-evidence",
    f1_reused_source => "agency-lineage.broken-reused-source",
    f2_refusal => "agency-refusal",
    f2_cut_guard => "agency-refusal.broken-cut-guard",
    f3_origin => "agency-origin",
    f3_no_direct_read => "agency-origin.broken-no-direct-read",
    f4_any => "agency-any",
    f4_singleton => "agency-any.broken-single-valued-a",
}

#[test]
fn v4_origin_is_scoped_to_the_selected_configuration_process() {
    let source = REFUSAL.replace("Reader", "Input")
        .replace("actor Ledger {", "actor ZOnly { source record } actor Reader { capacity 1; capability enter() -> Entry { reads [ZOnly]; duration fixed 1min; result nondet { entry } } } actor Ledger {")
        .replace("process After {", "process After { extra = Reader.enter();")
        .replace("include [Invoice,", "include [Invoice,ZOnly,Reader,");
    let before = serde_json::to_value(report::check_configuration(&source, "AsIs")).unwrap();
    let after = serde_json::to_value(report::check_configuration(&source, "ToBe")).unwrap();
    for actual in [&before, &after] {
        assert_eq!(actual["verdict"], "satisfied", "{actual}");
    }
    let origins = |actual: &Value| agency(actual)["origin"].as_array().unwrap().clone();
    let before_origins = origins(&before);
    let after_origins = origins(&after);
    assert_eq!(
        before_origins
            .iter()
            .find(|o| o["actor"] == "Reader")
            .unwrap(),
        &json!({"actor":"Reader","direct":[],"indirect":[]})
    );
    assert_eq!(
        after_origins
            .iter()
            .find(|o| o["actor"] == "Reader")
            .unwrap(),
        &json!({"actor":"Reader","direct":["ZOnly"],"indirect":[]})
    );
    for origin in &before_origins {
        for field in ["direct", "indirect"] {
            assert!(
                !origin[field].as_array().unwrap().contains(&json!("ZOnly")),
                "{before}"
            );
        }
    }
    for (from, to, from_report, to_report) in [
        ("AsIs", "ToBe", &before, &after),
        ("ToBe", "AsIs", &after, &before),
    ] {
        let actual = serde_json::to_value(diff::diff(&source, from, to)).unwrap();
        assert_eq!(actual["verdict"], "satisfied", "{actual}");
        assert_eq!(
            actual["agency"]["from"]["origin"],
            agency(from_report)["origin"]
        );
        assert_eq!(
            actual["agency"]["to"]["origin"],
            agency(to_report)["origin"]
        );
    }
}

#[test]
fn v3_argument_types_must_match_their_parameter_positions() {
    let source = ORIGIN.replace("post(v:Verdict)", "post(p:Verdict,e:Entry)");
    let correct = check(&source.replace("Ledger.post(approval)", "Ledger.post(approval, entry)"));
    assert_eq!(correct["verdict"], "satisfied", "{correct}");
    let swapped = check(&source.replace("Ledger.post(approval)", "Ledger.post(entry, approval)"));
    static_failure(&swapped, "E-TYPE-0");
    assert_eq!(swapped["verdict"], "invalid");
    let errors: Vec<_> = swapped["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "E-TYPE-0")
        .collect();
    assert_eq!(errors.len(), 2, "{swapped}");
}

#[test]
fn v2_transitive_argument_ancestors_remain_judgements_without_guards() {
    let source = ORIGIN
        .replace("actor Ledger {", "actor Summ { capacity 1; capability make(v:Verdict) -> Verdict { duration fixed 1min; result nondet { pass, fail } } } actor Ledger {")
        .replace("approval = Reviewer.review(entry)", "approval = Reviewer.review(entry); k = Summ.make(approval)")
        .replace("if approval == pass { } else { end rejected }", "")
        .replace("Ledger.post(approval)", "Ledger.post(k)");
    let actual = check(&source);
    assert_eq!(actual["verdict"], "satisfied", "{actual}");
    let a = agency(&actual);
    assert_eq!(
        a["refusal"][0]["judgements"],
        json!([
            refusal("approval", "Reviewer.review", "none", 1),
            refusal("k", "Summ.make", "none", 1)
        ])
    );
    assert_eq!(
        a["lineage"],
        json!([
            {"call":"approval","capability":"Reviewer.review","subject_sources":["Invoice"],"independent_sources":[]},
            {"call":"k","capability":"Summ.make","subject_sources":["Invoice"],"independent_sources":[]}
        ])
    );
}

#[test]
fn v1_lineage_groups_are_keyed_by_subject_sources() {
    let source = LINEAGE
        .replace("capability match_po", "capability recheck(p:Verdict) -> Verdict { reads [Invoice]; duration fixed 1min; result nondet { pass, fail } } capability match_po")
        .replace("post(p:Verdict,r:Verdict,a:Verdict)", "post(p:Verdict,r:Verdict,a:Verdict,k:Verdict)")
        .replace("approval = Approver.approve(entry)", "approval = Approver.approve(entry); k = Clerk.recheck(po)")
        .replace("Ledger.post(po, receipt, approval)", "Ledger.post(po, receipt, approval, k)");
    let actual = check(&source);
    assert_eq!(actual["verdict"], "satisfied", "{actual}");
    let a = agency(&actual);
    assert_eq!(
        a["lineage_groups"],
        json!([
            {"subject_sources":["Invoice"],"judgements":3,"actors":2,"independent_sources":["PurchaseOrder","ReceiptRecord"]},
            {"subject_sources":["Invoice","PurchaseOrder"],"judgements":1,"actors":1,"independent_sources":[]}
        ])
    );
    assert_eq!(
        a["lineage"][3],
        json!({"call":"k","capability":"Clerk.recheck","subject_sources":["Invoice","PurchaseOrder"],"independent_sources":[]})
    );
}

#[test]
fn u2_unsatisfiable_path_does_not_supply_judgements_or_context_variables() {
    let source = ORIGIN
        .replace(
            "approval = Reviewer.review(entry)",
            "approval = Reviewer.review(entry); k = Reviewer.review(entry)",
        )
        .replace(
            "if approval == pass { } else { end rejected }",
            "if approval == pass { } else { if k == pass && k == fail { } else { end rejected } }",
        );
    let actual = check(&source);
    assert_eq!(actual["verdict"], "satisfied", "{actual}");
    let a = agency(&actual);
    assert_eq!(
        a["refusal"][0]["judgements"],
        json!([refusal("approval", "Reviewer.review", "all_paths", 0)])
    );
    assert_eq!(a["lineage"].as_array().unwrap().len(), 1);
    assert_eq!(a["lineage"][0]["call"], "approval");
    assert!(!has_code(&actual, "W-AGY-0"), "{actual}");
    assert!(!has_code(&actual, "W-AGY-1"), "{actual}");

    // k remains J through argument ancestry, but its impossible guard must
    // not create extra contexts for either judgement.
    let actual = check(
        &source
            .replace("post(v:Verdict)", "post(v:Verdict,k:Verdict)")
            .replace("Ledger.post(approval)", "Ledger.post(approval,k)"),
    );
    assert_eq!(
        agency(&actual)["refusal"][0]["judgements"],
        json!([
            refusal("approval", "Reviewer.review", "all_paths", 0),
            refusal("k", "Reviewer.review", "none", 1)
        ])
    );

    // An unguarded third judgement m: its contexts are the support variables
    // other than m on the satisfiable path, {approval = pass} only. Taking k
    // from the unsatisfiable path as a context variable would give none / 2.
    let actual = check(
        &source
            .replace(
                "k = Reviewer.review(entry)",
                "k = Reviewer.review(entry); m = Reviewer.review(entry)",
            )
            .replace("post(v:Verdict)", "post(v:Verdict,k:Verdict,m:Verdict)")
            .replace("Ledger.post(approval)", "Ledger.post(approval,k,m)"),
    );
    assert_eq!(
        agency(&actual)["refusal"][0]["judgements"],
        json!([
            refusal("approval", "Reviewer.review", "all_paths", 0),
            refusal("k", "Reviewer.review", "none", 1),
            refusal("m", "Reviewer.review", "none", 1)
        ])
    );
}

#[test]
fn u2_guard_only_judgements_require_a_satisfiable_effect_path() {
    let source = ORIGIN
        .replace("post(v:Verdict)", "post()")
        .replace("Ledger.post(approval)", "Ledger.post()");
    let reached = check(&source);
    assert_eq!(
        agency(&reached)["refusal"][0]["judgements"],
        json!([refusal("approval", "Reviewer.review", "all_paths", 0)])
    );
    let unreachable = check(&source.replace(
        "if approval == pass",
        "if approval == pass && approval == fail",
    ));
    let a = agency(&unreachable);
    assert_eq!(
        a["refusal"][0],
        json!({"effect":"Ledger.post","call":"posted","reachable":false,"judgements":[]})
    );
    assert_eq!(a["lineage"], json!([]));
    assert_eq!(a["lineage_groups"], json!([]));
    assert!(!has_code(&unreachable, "W-AGY-0"));
    assert!(!has_code(&unreachable, "W-AGY-1"));

    // Argument ancestry is static: with post(approval) and the same impossible
    // guard, approval stays in lineage while refusal has no judgements.
    let unreachable = check(&ORIGIN.replace(
        "if approval == pass",
        "if approval == pass && approval == fail",
    ));
    let a = agency(&unreachable);
    assert_eq!(
        a["refusal"][0],
        json!({"effect":"Ledger.post","call":"posted","reachable":false,"judgements":[]})
    );
    assert_eq!(a["lineage"].as_array().unwrap().len(), 1);
    assert_eq!(a["lineage"][0]["call"], "approval");
    assert!(!has_code(&unreachable, "W-AGY-0"));
}

#[test]
fn m1_source_description_is_unsupported_not_malformed() {
    let actual = check(&ORIGIN.replace("source document", "source document \"説明\""));
    // ADR 0024 accepts a description only on source tacit; document stays U-SYN-0.
    static_failure(&actual, "U-SYN-0");
    assert_eq!(actual["verdict"], "unsupported");
    assert!(!has_code(&actual, "E-SYN-0"));
}

#[test]
fn m1_unbracketed_reads_is_unsupported_not_malformed() {
    let actual = check(&ORIGIN.replace("reads [Invoice]", "reads Invoice"));
    static_failure(&actual, "U-SYN-0");
    assert_eq!(actual["verdict"], "unsupported");
    assert!(!has_code(&actual, "E-SYN-0"));
}

#[test]
fn m1_known_words_in_the_wrong_reads_position_stay_malformed() {
    for word in [
        "duration", "result", "commits", "source", "reads", "if", "include",
    ] {
        let actual = check(&ORIGIN.replace("reads [Invoice]", &format!("reads {word}")));
        static_failure(&actual, "E-SYN-0");
        assert!(!has_code(&actual, "U-SYN-0"), "{word}: {actual}");
    }
}

#[test]
fn m3_unknown_parameter_types_keep_arity_and_positions_without_cascades() {
    for (params, args) in [
        ("v:Unknown", "approval"),
        ("u:Unknown,v:Verdict", "entry, approval"),
        ("v:Verdict,u:Unknown,e:Entry", "approval, entry, entry"),
        ("v:Verdict,u:Unknown", "approval, entry"),
    ] {
        let source = ORIGIN
            .replace("post(v:Verdict)", &format!("post({params})"))
            .replace("Ledger.post(approval)", &format!("Ledger.post({args})"));
        let actual = check(&source);
        static_failure(&actual, "E-NAME-0");
        assert!(!has_code(&actual, "E-TYPE-0"), "{params}: {actual}");
    }
    // An unknown slot must not mask definite errors at other slots or arity.
    for (params, args) in [
        ("u:Unknown,v:Verdict", "entry, entry"),
        ("u:Unknown,v:Verdict", "entry"),
        ("u:Unknown", "entry, approval"),
    ] {
        let actual = check(
            &ORIGIN
                .replace("post(v:Verdict)", &format!("post({params})"))
                .replace("Ledger.post(approval)", &format!("Ledger.post({args})")),
        );
        static_failure(&actual, "E-NAME-0");
        assert!(has_code(&actual, "E-TYPE-0"), "{params}: {actual}");
    }
}

#[test]
fn no_guard_and_reconverging_guard_are_none_in_one_empty_context() {
    for replacement in ["", "if approval == pass { } else { }"] {
        let source = ORIGIN.replace("if approval == pass { } else { end rejected }", replacement);
        let actual = check(&source);
        assert_eq!(actual["verdict"], "satisfied");
        assert_eq!(
            agency(&actual)["refusal"][0]["judgements"],
            json!([refusal("approval", "Reviewer.review", "none", 1)])
        );
        assert!(has_code(&actual, "W-AGY-0"));
        assert!(!has_code(&actual, "W-AGY-1"));
    }
}

#[test]
fn three_results_not_just_a_boolean_and_tautological_support_is_retained() {
    let source = ORIGIN.replace("pass, fail", "pass, fail, unknown").replace(
        "if approval == pass",
        "if approval == pass || approval == unknown",
    );
    let actual = check(&source);
    assert_eq!(
        agency(&actual)["refusal"][0]["judgements"],
        json!([refusal("approval", "Reviewer.review", "all_paths", 0)])
    );
    let tautology = source.replace(
        "approval == pass || approval == unknown",
        "approval == pass || approval == fail || approval == unknown",
    );
    assert_eq!(
        agency(&check(&tautology))["refusal"][0]["judgements"],
        json!([refusal("approval", "Reviewer.review", "none", 1)])
    );
}

#[test]
fn contradiction_keeps_static_lineage_but_not_vacuous_refusal() {
    let source = ORIGIN.replace(
        "if approval == pass { } else { end rejected }\n    posted = Ledger.post(approval)\n    end done",
        "if approval == pass && approval == fail { posted = Ledger.post(approval); end rejected } else { end done }",
    );
    let actual = check(&source);
    let a = agency(&actual);
    assert_eq!(
        a["lineage"],
        json!([{"call":"approval","capability":"Reviewer.review","subject_sources":["Invoice"],"independent_sources":[]}])
    );
    assert_eq!(
        a["origin"][3],
        json!({"actor":"Ledger","direct":[],"indirect":["Invoice"]})
    );
    assert_eq!(
        a["refusal"],
        json!([{"effect":"Ledger.post","call":"posted","reachable":false,"judgements":[]}])
    );
    assert!(!has_code(&actual, "W-AGY-0"));
    assert!(!has_code(&actual, "W-AGY-1"));
}

#[test]
fn singleton_unreachable_effect_becomes_none_when_only_comparison_changes() {
    let source = ORIGIN.replace("result nondet { pass, fail }", "result nondet { pass }")
        .replace("if approval == pass { } else { end rejected }\n    posted = Ledger.post(approval)\n    end done",
            "if approval == fail { posted = Ledger.post(approval); end rejected } else { end done }");
    let actual = check(&source);
    assert_eq!(
        agency(&actual)["refusal"][0],
        json!({"effect":"Ledger.post","call":"posted","reachable":false,"judgements":[]})
    );
    assert_eq!(agency(&actual)["lineage"].as_array().unwrap().len(), 1);
    assert!(!has_code(&actual, "W-AGY-0"));
    let reached = check(&source.replace("if approval == fail", "if approval == pass"));
    assert_eq!(agency(&reached)["refusal"][0]["reachable"], true);
    assert_eq!(
        agency(&reached)["refusal"][0]["judgements"],
        json!([refusal("approval", "Reviewer.review", "none", 1)])
    );
}

#[test]
fn unused_capability_does_not_give_origin_but_unreachable_call_does() {
    let source = ORIGIN.replace("actor Invoice { source document }", "actor Invoice { source document } actor ZUnused { source tacit }")
        .replace("capability review(e:Entry)", "capability unused() -> Entry { reads [ZUnused]; duration fixed 1min; result nondet { entry } } capability review(e:Entry)");
    let actual = check(&source);
    assert_eq!(
        agency(&actual)["origin"][3],
        json!({"actor":"Reviewer","direct":["Invoice"],"indirect":["Invoice"]})
    );
    let unreachable = source
        .replace("result nondet { pass, fail }", "result nondet { pass }")
        .replace(
            "else { end rejected }",
            "else { extra = Reviewer.unused(); end rejected }",
        );
    assert_eq!(
        agency(&check(&unreachable))["origin"][3],
        json!({"actor":"Reviewer","direct":["Invoice","ZUnused"],"indirect":["Invoice"]})
    );
}

#[test]
fn declaration_order_forward_reads_and_repeated_source_inputs_are_preserved() {
    let source = ORIGIN
        .replace("actor Invoice { source document }", "")
        .replace(
            "actor Ledger {",
            "actor ZLast { source tacit } actor Invoice { source record } actor Ledger {",
        )
        .replace(
            "capability review(e:Entry)",
            "capability review(e:Entry,copy:Entry)",
        )
        .replace(
            "approval = Reviewer.review(entry)",
            "entry2 = Reader.enter(); approval = Reviewer.review(entry, entry2)",
        );
    let actual = check(&source);
    let a = agency(&actual);
    assert_eq!(
        a["origin"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["actor"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Reader", "Reviewer", "ZLast", "Invoice", "Ledger"]
    );
    assert_eq!(
        a["lineage_groups"],
        json!([{"subject_sources":["Invoice"],"judgements":1,"actors":1,"independent_sources":[]}])
    );
    assert_eq!(a["origin"][1]["indirect"], json!(["Invoice"]));
}

#[test]
fn no_argument_producer_is_not_a_judgement_even_when_guard_reads_it() {
    let source = ORIGIN
        .replace("review(e:Entry)", "review()")
        .replace("Reviewer.review(entry)", "Reviewer.review()");
    let actual = check(&source);
    assert_eq!(agency(&actual)["lineage"], json!([]));
    assert_eq!(agency(&actual)["refusal"][0]["judgements"], json!([]));
    assert!(!has_code(&actual, "W-AGY-0"));
}

#[test]
fn partial_contexts_have_no_ghost_values_and_new_bindings_are_universally_quantified() {
    // Contexts for j are exactly {x=pass} and {y=pass}. Switching j opens
    // a new binding with a pass completion: no j value stops ALL new values.
    let source = REFUSAL.replace("if x == pass {\n      if j == pass { } else { end rejected }\n    } else { }", "if j == pass { xx = Flag.check(entry); if xx == pass { } else { end rejected } } else { yy = Flag.check(entry); if yy == pass { } else { end rejected } }");
    let actual = serde_json::to_value(report::check_configuration(&source, "ToBe")).unwrap();
    assert_eq!(
        agency(&actual)["refusal"][0]["judgements"][0],
        refusal("j", "Judge.check", "none", 2)
    );
    assert!(has_code(&actual, "W-AGY-0"));
}

#[test]
fn separate_effect_calls_do_not_share_their_reach_formula() {
    let source = ORIGIN.replace(
        "posted = Ledger.post(approval)",
        "posted = Ledger.post(approval); posted_again = Ledger.post(approval)",
    );
    let actual = check(&source);
    assert_eq!(
        agency(&actual)["refusal"],
        json!([
            {"effect":"Ledger.post","call":"posted","reachable":true,"judgements":[refusal("approval","Reviewer.review","all_paths",0)]},
            {"effect":"Ledger.post","call":"posted_again","reachable":true,"judgements":[refusal("approval","Reviewer.review","all_paths",0)]}
        ])
    );
}

#[test]
fn any_tie_order_and_different_enum_ordinals_do_not_change_boolean_contexts() {
    let baseline = check(ANY);
    let reordered = check(&ANY.replace("tie by [a,b]", "tie by [b,a]"));
    assert_eq!(agency(&baseline), agency(&reordered));
    let source = ANY
        .replace("type Ack", "type Other = enum { fail, pass } type Ack")
        .replace(
            "actor JB {\n    capacity 1\n    capability check(e:Entry) -> Verdict",
            "actor JB {\n    capacity 1\n    capability check(e:Entry) -> Other",
        );
    assert_eq!(agency(&baseline), agency(&check(&source)));
}

#[test]
fn where_does_not_guard_effects_inside_the_fork() {
    let source = ANY.replacen("-> Verdict { duration", "-> Verdict { commits; duration", 1);
    let actual = check(&source);
    let rows = agency(&actual)["refusal"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0],
        json!({"effect":"JA.check","call":"a","reachable":true,"judgements":[]})
    );
    assert_eq!(
        rows[1]["judgements"],
        json!([
            refusal("a", "JA.check", "conditional", 1),
            refusal("b", "JB.check", "conditional", 1)
        ])
    );
}

#[test]
fn static_name_type_availability_and_named_argument_fail_closed() {
    for (source, code) in [
        (
            ORIGIN.replace("reads [Invoice]", "reads [Unknown]"),
            "E-NAME-0",
        ),
        (ORIGIN.replace("source document", ""), "E-NAME-0"),
        (
            ORIGIN.replace("review(e:Entry)", "review(e:Unknown)"),
            "E-NAME-0",
        ),
        (
            ORIGIN.replace("Reviewer.review(entry)", "Reviewer.review()"),
            "E-TYPE-0",
        ),
        (
            ORIGIN.replace("Ledger.post(approval)", "Ledger.post(entry)"),
            "E-TYPE-0",
        ),
        (
            ORIGIN.replace("Reviewer.review(entry)", "Reviewer.review(unknown)"),
            "E-NAME-0",
        ),
        (
            ORIGIN
                .replace("review(e:Entry)", "review(e:Verdict)")
                .replace("Reviewer.review(entry)", "Reviewer.review(approval)"),
            "U-SEM-0",
        ),
        (
            ORIGIN.replace(
                "entry = Reader.enter()\n    approval = Reviewer.review(entry)",
                "approval = Reviewer.review(entry); entry = Reader.enter()",
            ),
            "U-SEM-0",
        ),
        (
            ORIGIN.replace(
                "entry = Reader.enter()\n    approval = Reviewer.review(entry)",
                "fork f { entry = Reader.enter(); approval = Reviewer.review(entry) } join all f",
            ),
            "U-SEM-0",
        ),
        (
            ORIGIN.replace(
                "approval = Reviewer.review(entry)",
                "if entry == entry { approval = Reviewer.review(entry) } else { }",
            ),
            "U-SEM-0",
        ),
        (
            ORIGIN.replace("Reviewer.review(entry)", "Reviewer.review(e: entry)"),
            "U-SYN-0",
        ),
        (
            ORIGIN.replace("Reviewer.review(entry)", "Reviewer.review(e = entry)"),
            "U-SYN-0",
        ),
        (ORIGIN.replace("reads [Invoice]", "reads []"), "E-SYN-0"),
    ] {
        static_failure(&check(&source), code);
    }
}

#[test]
fn all_duplicate_declarations_are_unsupported_without_hiding_other_name_errors() {
    for source in [
        ORIGIN.replace("source document", "source document; source record"),
        ORIGIN.replace("reads [Invoice]", "reads [Invoice]; reads [Invoice]"),
        ORIGIN.replace("reads [Invoice]", "reads [Invoice,Invoice]"),
        ORIGIN.replace("commits;", "commits; commits;"),
        ORIGIN.replace("review(e:Entry)", "review(e:Entry,e:Entry)"),
    ] {
        static_failure(&check(&source), "U-SEM-0");
    }
    let source = ORIGIN.replace("reads [Invoice]", "reads [Invoice,Unknown,Invoice]");
    let actual = check(&source);
    static_failure(&actual, "U-SEM-0");
    assert!(has_code(&actual, "E-NAME-0"), "{actual}");
}

#[test]
fn cancelled_result_is_not_available_as_an_argument() {
    // With equal 1min
    // branches both finish before any ack, so b is definitely available
    // (lower's available_after_cancel). Make b longer so a's adoption can
    // cancel it before it finishes.
    let source = ANY
        .replace(
            "actor JB {\n    capacity 1\n    capability check(e:Entry) -> Verdict { duration fixed 1min",
            "actor JB {\n    capacity 1\n    capability check(e:Entry) -> Verdict { duration fixed 3min",
        )
        .replace(
            "cancel forbidden_after_start",
            "cancel cooperative ack_within 1min",
        )
        .replace("remaining drain", "remaining cancel_then_drain")
        .replace("capability post()", "capability post(v:Verdict)")
        .replace("Ledger.post()", "Ledger.post(b)");
    static_failure(&check(&source), "U-SEM-0");
}

#[test]
fn selected_process_only_but_all_capability_declarations_are_checked() {
    let bad_after = REFUSAL.replacen(
        "x = Flag.check(entry)\n    if x",
        "x = Flag.check(j)\n    if x",
        1,
    );
    let before = serde_json::to_value(report::check_configuration(&bad_after, "AsIs")).unwrap();
    assert_eq!(before["verdict"], "satisfied");
    agency(&before);
    let after = serde_json::to_value(report::check_configuration(&bad_after, "ToBe")).unwrap();
    static_failure(&after, "E-TYPE-0");
    let bad_declaration = REFUSAL.replace("capability post(v:Verdict)", "capability unused() -> Entry { reads [Unknown]; duration fixed 1min; result nondet { entry } } capability post(v:Verdict)");
    static_failure(
        &serde_json::to_value(report::check_configuration(&bad_declaration, "AsIs")).unwrap(),
        "E-NAME-0",
    );
}

#[test]
fn static_failures_null_and_runtime_clock_cut_keeps_agency() {
    static_failure(&check("not a model"), "E-SYN-0");
    static_failure(
        &serde_json::to_value(report::check_configuration(REFUSAL, "Missing")).unwrap(),
        "E-NAME-0",
    );
    let baseline = check(ORIGIN);
    let cut = check(&ORIGIN.replace("clock 32min", "clock 1min"));
    assert_eq!(cut["verdict"], "inconclusive");
    assert_eq!(cut["bounds"]["truncated"], true);
    assert_eq!(agency(&cut), agency(&baseline));
}

#[test]
fn runtime_arithmetic_abort_keeps_lowered_agency_not_a_partial_runtime_success() {
    let source = ORIGIN
        .replace(
            "commits; duration fixed 1min",
            "commits; duration fixed 18446744073709551615min",
        )
        .replace("clock 32min", "clock 18446744073709551615min");
    let actual = check(&source);
    assert!(has_code(&actual, "U-SEM-0"), "{actual}");
    assert_ne!(actual["verdict"], "satisfied");
    assert_eq!(actual["bounds"]["truncated"], Value::Null);
    assert_eq!(actual["terminals"], json!([]));
    assert_eq!(agency(&actual), agency(&check(ORIGIN)));
}

#[test]
fn diff_copies_agency_on_success_static_failure_and_cut_mismatch() {
    let cases = [
        (REFUSAL.to_owned(), "AsIs", "ToBe"),
        (REFUSAL.to_owned(), "Missing", "ToBe"),
        (REFUSAL.to_owned(), "Missing", "AlsoMissing"),
        (
            REFUSAL.replace(
                "configuration ToBe { cut C",
                "cut D { include [Invoice,Reader,Judge,Flag,Ledger] } configuration ToBe { cut D",
            ),
            "AsIs",
            "ToBe",
        ),
    ];
    for (source, from, to) in cases {
        let actual = serde_json::to_value(diff::diff(&source, from, to)).unwrap();
        let sides = actual
            .get("agency")
            .expect("diff agency is required even on early return");
        for side in ["from", "to"] {
            assert_eq!(
                sides.get(side),
                actual[side]["report"].get("agency"),
                "{actual}"
            );
            assert!(sides.get(side).is_some());
        }
        assert!(
            !actual["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("agency"))
        );
    }
}

#[test]
fn public_default_derivation_budget_keeps_properties_without_a_partial_agency() {
    // 24 binary condition bindings: 2^24 combinations, above the agreed
    // 65,536-context / 1,000,000-work domain. No injected internal Limits API.
    let calls = (0..24)
        .map(|i| format!("v{i} = Reviewer.review(entry); "))
        .collect::<String>();
    let condition = (0..24)
        .map(|i| format!("v{i} == pass"))
        .collect::<Vec<_>>()
        .join(" || ");
    let source = ORIGIN
        .replace("approval = Reviewer.review(entry)", &calls)
        .replace("approval == pass", &condition)
        .replace("Ledger.post(approval)", "Ledger.post(v0)");
    // ADR 0067 B1: the one Or remains inseparable, but its Agency limit no
    // longer stops execution. The separate engine's 1M-state budget cannot
    // cover 2^24 result tuples. All paths need at most 1+24+1=26 < clock32,
    // so this is a computation cut, not a business horizon or a violation.
    let actual = check(&source);
    assert_eq!(actual["agency"]["status"], "not_derived");
    assert_eq!(actual["agency"]["reason"], "derivation_limit");
    for field in ["origin", "lineage", "lineage_groups", "refusal"] {
        assert!(actual["agency"][field].is_null());
    }
    assert!(
        actual["agency"]["limit"]["requested"].as_u64().unwrap()
            > actual["agency"]["limit"]["limit"].as_u64().unwrap()
    );
    assert!(has_code(&actual, "W-AGY-4"));
    assert!(has_code(&actual, "W-BUDGET-0"));
    assert!(!has_code(&actual, "U-SEM-0"));
    assert_eq!(actual["ledger"]["execution"]["status"], "ran");
    assert_eq!(actual["properties"].as_array().unwrap().len(), 2);
    assert!(
        actual["properties"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| matches!(p["verdict"].as_str(), Some("satisfied" | "inconclusive")))
    );
    assert_eq!(
        actual["bounds"],
        json!({"cases":1,"live_tasks":4,"clock":32,"truncated":true})
    );
    assert_eq!(actual["verdict"], "inconclusive");
}
