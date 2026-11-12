//! ADR 0031 hand oracles. Static metadata is not an exploration transition.
use relca_core::{diff, lower, report, syntax};
use serde_json::{Value, json};
#[path = "support/configuration_diff_inputs.rs"]
mod inputs;
#[path = "support/configuration_diff_population.rs"]
pub mod population;
pub use inputs::{expected, source, stripped};
pub fn subset(e: &Value, a: &Value) {
    if let Some(obj) = e.as_object() {
        for (key, value) in obj {
            if key == "absent_diagnostics" {
                for code in value.as_array().unwrap() {
                    assert_eq!(warnings(a, code.as_str().unwrap()), 0, "{code}: {a}");
                }
            } else {
                assert!(a.get(key).is_some(), "missing {key}: {a}");
                subset(value, &a[key]);
            }
        }
    } else if let Some(es) = e.as_array() {
        let actual = a
            .as_array()
            .unwrap_or_else(|| panic!("expected array {e}, got {a}"));
        assert_eq!(es.len(), actual.len(), "array population: {e} != {a}");
        for (x, y) in es.iter().zip(actual) {
            subset(x, y);
        }
    } else {
        assert_eq!(e, a);
    }
}
fn compare(src: &str) -> Value {
    serde_json::to_value(diff::diff(src, "From", "To")).unwrap()
}
fn fixture(name: &str) -> Value {
    let src = source(name);
    let (from, to) = if name.starts_with("diff2-invoice") {
        ("AsIs", "ToBeA")
    } else {
        ("From", "To")
    };
    let actual = serde_json::to_value(diff::diff(&src, from, to)).unwrap();
    let mut e = expected(name);
    // ADR0051 hand: both invoice inputs read entry, whose sole producer moves
    // from Clerk.enter to Extractor.extract. Preserve suppresses W3, not W1.
    if matches!(
        name,
        "diff2-invoice" | "diff2-invoice.broken-missing-resource-claim"
    ) {
        e["differences"].as_array_mut().unwrap().push(json!({
            "kind":"relation","target":"entry","change":"modified",
            "reason":null,"loses":null,
            "from":{"producer":"Clerk.enter"},"to":{"producer":"Extractor.extract"}
        }));
    }
    // Diagnostic expectations are code-inclusion, not ordered array equality.
    if let Some(ds) = e.as_object_mut().unwrap().remove("diagnostics") {
        for d in ds.as_array().unwrap() {
            assert!(
                actual["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|x| x["code"] == d["code"] && x["severity"] == d["severity"]),
                "{name}: {actual}"
            );
        }
    }
    subset(&e, &actual);
    let (w1, w3) = population::hand_warnings(name);
    assert_eq!(warnings(&actual, "W-DIFF-1"), w1, "hand W1 {name}");
    assert_eq!(warnings(&actual, "W-DIFF-3"), w3, "hand W3 {name}");
    actual
}
fn warnings(v: &Value, code: &str) -> usize {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == code)
        .count()
}
fn row<'a>(v: &'a Value, kind: &str, target: &str) -> &'a Value {
    let found: Vec<_> = v["differences"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == kind && r["target"] == target)
        .collect();
    assert_eq!(found.len(), 1, "{kind}/{target}: {v}");
    found[0]
}
fn annotation<'a>(row: &'a Value, kind: &str) -> &'a Value {
    row["annotations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["kind"] == kind)
        .unwrap()
}

#[test]
fn all_38_old_syntax_skeletons_lower_76_selected_inputs_without_unrelated_errors() {
    let mut n = 0;
    for name in population::NAMES {
        let src = source(name);
        assert!(
            !src.contains("//") && !src.contains('\r'),
            "comment-free DSL {name}"
        );
        let old = stripped(&src);
        let ast = syntax::parse(&old).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        for selected in if name.starts_with("diff2-invoice") {
            ["AsIs", "ToBeA"]
        } else {
            ["From", "To"]
        } {
            lower::lower_configuration(&ast, selected)
                .unwrap_or_else(|e| panic!("{name}/{selected}: {e:?}"));
            n += 1;
        }
    }
    assert_eq!(n, 76);
}

#[test]
fn f1_through_f7_six_typed_projections_and_single_mutations() {
    for name in &population::NAMES[..14] {
        fixture(name);
    }
}
#[test]
fn f8_f9_preserve_six_buckets_and_selector_union_are_not_attachment_counts() {
    for name in &population::NAMES[14..18] {
        fixture(name);
    }
    for (name, id) in [
        ("diff2-preserve-union", "contract Ctl #2 at_most_once"),
        (
            "diff2-preserve-union.broken-other-selector",
            "contract Ctl #1 at_most_once",
        ),
    ] {
        let v = fixture(name);
        let as_ = row(&v, "relation", "A.b")["annotations"]
            .as_array()
            .unwrap();
        assert_eq!(as_.len(), 2);
        let local = as_.iter().find(|a| a["global"] == false).unwrap();
        assert_eq!(local["selector"], id);
        assert_eq!(local["properties"], json!([id]));
        assert_eq!(warnings(&v, "W-DIFF-1"), 1);
        assert_eq!(warnings(&v, "W-DIFF-3"), 0);
    }
}
#[test]
fn f10_f11_override_all_paths_person_qualification_and_root_fold() {
    for name in &population::NAMES[18..22] {
        fixture(name);
    }
    let src = source("diff2-override");
    for text in [
        src.replace("by p }", "by A }"),
        src.replace("override relation P.post by p", "override P.post by A"),
    ] {
        let v = compare(&text);
        assert_eq!(v["verdict"], "satisfied");
        assert_eq!(
            annotation(row(&v, "relation", "P.post"), "override")["subject_kind"],
            "actor"
        );
    }
    let no_permission = src.replace("permit p to A.g", "permit q to A.g");
    assert_eq!(
        annotation(
            row(&compare(&no_permission), "relation", "P.post"),
            "override"
        )["verdict"],
        "violated"
    );
    let mixed = no_permission.replace("by p }", "by q }");
    assert_eq!(
        annotation(row(&compare(&mixed), "relation", "P.post"), "override")["verdict"],
        "violated",
        "p executor plus q Permission is not one qualified person"
    );
}
#[test]
fn f12_maintain_links_conflicts_unlinked_and_no_fabricated_duties() {
    fixture("diff2-maintain");
    fixture("diff2-maintain.broken-conflict");
    let src = source("diff2-maintain").replace("  responsible s for A.maintenance\n", "");
    let v = compare(&src);
    assert_eq!(v["verdict"], "satisfied");
    let a = annotation(row(&v, "actor", "A"), "maintain");
    assert_eq!(
        a["links"],
        json!([{"actor":"A","from_duties":[],"to_duties":[],"status":"unlinked"}])
    );
    for s in ["from", "to"] {
        assert!(v[s]["report"]["responsibilities"].is_null());
    }
    // Without the duty, A.a still has Permission [p,q] (A's executors), so the
    // authority row remains, but no duty is fabricated for it.
    assert_eq!(
        row(&v, "authority", "A.a")["to"],
        json!({"permission":["p","q"],"grants":[],"authority_basis":[],"responsibilities":[]})
    );
}
#[test]
fn f13_f14_f15_surplus_claims_w1_and_annotation_warning_are_distinct() {
    for name in &population::NAMES[24..30] {
        fixture(name);
    }
    for (name, w1, w3) in [
        ("diff2-warning-intent", 0, 0),
        ("diff2-warning-intent.broken-no-change", 1, 0),
        ("diff2-warning-annotation", 0, 0),
        ("diff2-warning-annotation.broken-no-preserve", 0, 1),
    ] {
        let v = fixture(name);
        assert_eq!(warnings(&v, "W-DIFF-1"), w1);
        assert_eq!(warnings(&v, "W-DIFF-3"), w3);
    }
    let v = fixture("diff2-surplus.broken-intent-only");
    for side in ["from", "to"] {
        assert_eq!(warnings(&v[side]["report"], "E-DIFF-0"), 0);
    }
    let absent = source("diff2-warning-annotation")
        .replace("on meaning end.rejected", "on meaning end.done");
    let v = compare(&absent);
    assert_eq!(v["verdict"], "invalid");
    assert_eq!(warnings(&v, "E-DIFF-0"), 1);
    assert_eq!(warnings(&v, "W-DIFF-3"), 1);
    assert_eq!(
        warnings(&v, "W-DIFF-1"),
        0,
        "wrong annotation cannot hide the other row's warning"
    );
    let from_only = source("diff2-warning-intent")
        .replace("; preserve Ctl", "")
        .replace(
            "configuration From { cut C; process Before }",
            "configuration From { cut C; process Before; intent { preserve Ctl } }",
        );
    let v = compare(&from_only);
    assert_eq!(v["verdict"], "satisfied");
    assert!(v.get("preservation").is_none());
    assert_eq!(
        warnings(&v, "W-DIFF-3"),
        1,
        "only To intent annotates differences"
    );
}
#[test]
fn f16_f17_f18_capacity_cut_and_dead_end_do_not_redefine_static_activity() {
    for name in &population::NAMES[30..36] {
        fixture(name);
    }
    let v = compare(&source("diff2-dead-end").replace("clock 20min", "clock 0min"));
    assert_eq!(v["verdict"], "inconclusive");
    assert_eq!(v["differences"].as_array().unwrap().len(), 1);
    assert_eq!(v["terminals"], json!([]));
    let v = fixture("diff2-mismatch.broken-other-cut");
    for word in [
        "time_intent",
        "resource",
        "meaning",
        "authority",
        "visibility",
        "contest",
        "preserve",
        "override",
        "maintain",
    ] {
        assert!(v["unchecked"].as_array().unwrap().contains(&json!(word)));
    }
}
#[test]
fn f19_invoice_nine_rows_five_claims_and_seven_satisfied_one_vacuous() {
    for (name, claims, w1) in [
        ("diff2-invoice", 5, 4),
        ("diff2-invoice.broken-missing-resource-claim", 4, 5),
    ] {
        let v = fixture(name);
        assert_eq!(
            v["differences"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| !r["reason"].is_null())
                .count(),
            claims
        );
        assert_eq!(warnings(&v, "W-DIFF-1"), w1);
        assert_eq!(warnings(&v, "W-DIFF-3"), 0);
        assert_eq!(
            annotation(row(&v, "relation", "entry"), "preserve")["global"],
            true
        );
        assert_eq!(v["preservation"]["properties"].as_array().unwrap().len(), 8);
    }
    let src = include_str!("../../../examples/invoice/configurations.relca");
    for (a, b, n) in [
        ("AsIs", "ToBeP", 2),
        ("AsIs", "ToBeA", 9),
        ("ToBeP", "ToBeA", 7),
        ("ToBeA", "AsIs", 9),
        ("ToBeA", "ToBeA", 0),
    ] {
        let v = serde_json::to_value(diff::diff(src, a, b)).unwrap();
        assert_eq!(v["differences"].as_array().unwrap().len(), n);
        assert_eq!(warnings(&v, "W-DIFF-1"), n);
        assert_eq!(warnings(&v, "W-DIFF-3"), n);
    }
}

#[test]
fn additional_structure_modes_fork_permutation_and_if_arms_are_sets() {
    let src = source("diff2-capacity");
    let mixed = src.replace(
        "fork f { x = A.a(); y = A.b() }; join all f; end done",
        "z = A.a(); fork f { x = A.a(); y = A.b() }; join all f; end done",
    );
    let v = compare(&mixed);
    assert_eq!(row(&v, "time", "A.a")["to"], json!(["sequential", "fork"]));
    let perm = src.replace(
        "x = A.a(); y = A.b() }; join",
        "y = A.b(); x = A.a() }; join",
    );
    assert_eq!(compare(&src)["differences"], compare(&perm)["differences"]);
    let branches = source("diff2-time").replace(
        "fork f { x = A.a(); z = A.c() }; join all f",
        "x = A.a(); if x == pass { z = A.c() } else {} ",
    );
    assert!(
        !compare(&branches)["differences"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "time")
    );
}

#[test]
fn override_every_call_conditional_removed_and_unknown_are_fail_closed() {
    let src = source("diff2-override");
    // y follows the if whose else arm no longer ends, so the fail path reaches
    // y without passing g: a real bypass of the override subject.
    let two = src.replace(
        "else { end done }; end done }",
        "else {}; y = P.post(g); end done }",
    );
    assert_ne!(two, src);
    let v = compare(&two);
    assert_eq!(v["verdict"], "violated");
    let effects = annotation(row(&v, "relation", "P.post"), "override")["effects"]
        .as_array()
        .unwrap();
    assert_eq!(effects.len(), 2);
    assert!(
        effects
            .iter()
            .any(|e| e["call"] == "y" && e["stops"] != "all_paths")
    );
    let conditional=src.replace("capability g(v:V)","capability h(v:V) -> V { duration fixed 1min; result nondet { pass,fail } }; capability g(v:V)")
        .replace("if g == pass", "h = A.h(seed); if g == pass || h == pass");
    assert_eq!(compare(&conditional)["verdict"], "violated");
    let state = src
        .replace(
            "  type V",
            "  observed case { flag: bool = false }\n  type V",
        )
        .replace(
            "commits; duration",
            "writes flag; finish { flag = true }; duration",
        );
    let v = compare(&state);
    assert_eq!(v["verdict"], "unsupported");
    let guard = state.replace("if g == pass", "if flag == false");
    assert_eq!(compare(&guard)["verdict"], "unsupported");
    let no_intent = stripped(&src);
    let removed = no_intent
        .replace(
            "configuration From { cut C; process Before }",
            "configuration From { cut C; process After }",
        )
        .replace(
            "configuration To { cut C; process After;",
            "configuration To { cut C; process Before;",
        );
    let removed = removed.replace(
        "process Before;",
        "process Before; intent { override relation P.post by p };",
    );
    let v = compare(&removed);
    assert_eq!(
        v["verdict"], "vacuous",
        "empty removed effect is folded, not success"
    );
    assert_eq!(
        annotation(row(&v, "relation", "P.post"), "override")["verdict"],
        "vacuous"
    );
}

#[test]
fn new_name_namespace_duplicate_lane_and_unselected_configuration_boundaries() {
    let base = source("diff2-override");
    for (old, new, code) in [
        ("by p", "by absent", "E-NAME-0"),
        ("relation P.post", "resource P.post", "E-CON-0"),
        (
            "override relation P.post by p",
            "override relation P.post by p; override relation P.post by p",
            "U-SEM-0",
        ),
        (
            "override relation P.post by p",
            "override relation P.post by p; change relation P.post reason \"claim\"; change relation P.post reason \"again\"",
            "U-SEM-0",
        ),
    ] {
        let v = serde_json::to_value(report::check_configuration(&base.replace(old, new), "From"))
            .unwrap();
        assert!(warnings(&v, code) > 0, "{code}: {v}");
    }
    let src = source("diff2-preserve");
    for selector in ["Missing", "\"contract Ctl #99 at_most_once\"", "\"\""] {
        let v = compare(&src.replace("preserve Ctl", &format!("preserve {selector}")));
        assert_eq!(v["verdict"], "invalid");
        assert!(warnings(&v["to"]["report"], "E-NAME-0") > 0);
    }
    let coexisting=base.replace("override relation P.post by p", "change relation P.post reason \"主張\"; override relation P.post by p; maintain relation P.post by q");
    let v = compare(&coexisting);
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(row(&v, "relation", "P.post")["reason"], "主張");
    assert_eq!(
        row(&v, "relation", "P.post")["annotations"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn preservation_multi_case_cut_and_abort_keep_existing_verdicts() {
    let src = source("diff2-preserve")
        .replace("cases 1", "cases 2")
        .replace("  bounds", "  arrivals fixed [0min,0min]\n  bounds");
    let v = compare(&src);
    assert_eq!(v["preservation"]["summary"]["vacuous"], 1);
    assert_eq!(v["preservation"]["properties"].as_array().unwrap().len(), 1);
    let cut = source("diff2-preserve").replace("clock 20min", "clock 0min");
    let v = compare(&cut);
    assert_eq!(v["verdict"], "inconclusive");
    assert_eq!(v["preservation"]["summary"]["inconclusive"], 1);
    let abort = source("diff2-preserve")
        .replace(
            "capability post() -> V { commits; duration fixed 1min",
            "capability post() -> V { commits; duration fixed 18446744073709551615min",
        )
        .replace("clock 20min", "clock 18446744073709551615min");
    let v = compare(&abort);
    assert_eq!(v["verdict"], "unsupported");
    assert!(
        v["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("terminals"))
    );
    assert!(
        v["checked"]
            .as_array()
            .unwrap()
            .contains(&json!("preserve"))
    );
}

#[test]
fn auxiliary_old_syntax_controls_validate_judgements_and_single_mutation_shapes() {
    for (name, status) in [
        ("diff2-override", "all_paths"),
        ("diff2-override.broken-bypass", "none"),
    ] {
        let src = stripped(&source(name));
        let ast = syntax::parse(&src).unwrap();
        let m = lower::lower_configuration(&ast, "To").unwrap().model;
        let v = serde_json::to_value(&m.agency).unwrap();
        assert_eq!(v["refusal"][0]["judgements"][0]["call"], "g");
        assert_eq!(
            v["refusal"][0]["judgements"][0]["stops"], status,
            "existing U hand control {name}"
        );
    }
    let conditional=source("diff2-override")
        .replace("capability g(v:V)","capability h(v:V) -> V { duration fixed 1min; result nondet { pass,fail } }; capability g(v:V)")
        .replace("if g == pass", "h = A.h(seed); if g == pass || h == pass");
    let m = lower::lower_configuration(&syntax::parse(&stripped(&conditional)).unwrap(), "To")
        .unwrap()
        .model;
    let v = serde_json::to_value(&m.agency).unwrap();
    let js = v["refusal"][0]["judgements"].as_array().unwrap();
    assert_eq!(js.len(), 2);
    assert!(
        js.iter()
            .all(|j| j["stops"] == "conditional" && j["bypasses"].as_u64().unwrap() > 0)
    );
}

#[test]
fn unreferenced_grants_deduplicated_readers_multiple_contests_and_maintenance_owners() {
    let src = source("diff2-authority").replace("  contract Ctl { authority_basis A.b G }\n", "");
    let v = compare(&src);
    let a = &row(&v, "authority", "A.b")["to"];
    assert_eq!(a["grants"].as_array().unwrap().len(), 1);
    assert_eq!(a["authority_basis"], json!([]));
    let src = source("diff2-visibility.broken-same-reader").replace(
        "process After { y = A.b();",
        "process After { y = A.b(); again = A.b();",
    );
    let v = compare(&src);
    assert!(
        !v["differences"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "visibility")
    );
    let src=source("diff2-contest").replace("respond_by 5min }", "respond_by 5min; contest V on end.rejected route Reader.r reversible_by [p] respond_by 6min }");
    let v = compare(&src);
    assert_eq!(
        row(&v, "contest", "end.rejected")["to"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let src = source("diff2-contest").replace(
        "configuration To { cut C; process After }",
        "configuration To { cut C; process After; intent { maintain contest end.rejected by p } }",
    );
    let v = compare(&src);
    let a = annotation(row(&v, "contest", "end.rejected"), "maintain");
    assert_eq!(
        a["links"],
        json!([{"actor":"Gate","from_duties":[],"to_duties":[],"status":"unlinked"},{"actor":"Reader","from_duties":[],"to_duties":[],"status":"unlinked"}])
    );
}

#[test]
fn authority_restriction_records_and_condition_writer_duties_are_not_reinterpreted() {
    let scoped = source("diff2-authority")
        .replace(
            "  type V",
            "  observed case { flag: bool = false }\n  type V",
        )
        .replace(
            "issued_by p }",
            "issued_by p; scope flag == false; expires 10min; revocable_by p }",
        );
    lower::lower_configuration(&syntax::parse(&scoped).unwrap(), "To")
        .expect("old restriction skeleton valid");
    let duty = source("diff2-maintain")
        .replace("  type V", "  observed case { r: bool = false }\n  type V")
        .replace(
            "responsible s for A.maintenance",
            "responsible s for (r == true)",
        )
        .replace(
            "capability a() -> V {",
            "capability a() -> V { writes r; finish { r = true };",
        );
    lower::lower_configuration(&syntax::parse(&stripped(&duty)).unwrap(), "To")
        .expect("old condition writer skeleton valid");
    let v = compare(&scoped);
    let g = &row(&v, "authority", "A.b")["to"]["grants"][0];
    assert_eq!(g["scope"], "flag == false");
    assert_eq!(g["expires"], 10);
    assert_eq!(g["effective_expires"], 10);
    assert_eq!(g["revocable_by"], json!({"kind":"person","name":"p"}));
    let v = compare(&duty);
    let a = &row(&v, "authority", "A.a")["to"];
    assert_eq!(a["responsibilities"][0]["id"], 1);
    assert_eq!(
        a["responsibilities"][0]["target"]["kind"],
        "state_condition"
    );
    assert_eq!(
        annotation(row(&v, "actor", "A"), "maintain")["links"][0]["status"],
        "unlinked",
        "condition duty is NOT maintenance"
    );
}

#[test]
fn contextual_new_grammar_is_closed_and_utf8_error_spans_are_bytes() {
    let base = source("diff2-override");
    for (clause, code) in [
        ("preserve", "E-SYN-0"),
        ("override relation P.post by", "E-SYN-0"),
        ("maintain actor P by", "E-SYN-0"),
        ("change time A.g", "E-SYN-0"),
        ("change unknown P reason \"x\"", "U-SYN-0"),
        // ADR 0043: `keep fallback` after an old-form retire is a known word
        // in the wrong position (not the new form, E-SYN-0).
        ("retire P.post reason \"x\" keep fallback", "E-SYN-0"),
        ("override relation P.post by [p,q]", "U-SYN-0"),
        ("override relation P.post by A.g", "U-SYN-0"),
    ] {
        let src = base.replace("override relation P.post by p", clause);
        let v = serde_json::to_value(report::check_configuration(&src, "From")).unwrap();
        assert!(warnings(&v, code) > 0, "{clause} -> {code}: {v}");
    }
    let src = base
        .replace("actor A ", "actor time ")
        .replace("A.", "time.")
        .replace("[A,P]", "[time,P]")
        .replace("by p }", "by time }");
    assert_eq!(
        compare(&src)["verdict"],
        "satisfied",
        "time remains a contextual word"
    );
    let src = source("diff2-surplus.broken-intent-only").replace(
        "cut C { include [A] }",
        "cut C { question \"多バイト\"; include [A] }",
    );
    let v = compare(&src);
    let d = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "E-DIFF-0")
        .unwrap();
    assert_eq!(d["span"]["start"], src.find("change time").unwrap());
}
