//! Independent ADR0037 public acceptance. Hand calculations are beside inputs.
#[path = "support/state_invariant_acceptance.rs"]
mod h;
use relca_core::{engine, ir, reference, report};
use serde_json::Value;

macro_rules! accept { ($($test:ident=>$base:literal),+ $(,)?)=>{$(#[test] fn $test(){
    for name in h::names().into_iter().filter(|n|n==$base||n.starts_with(&format!("{}.broken-",$base))) {
        let e=h::expected(&name);let a=h::report(&name); h::subset(&e,&a);
        if ["invalid","unsupported"].contains(&e["verdict"].as_str().unwrap()) {h::reject(&h::source(&name),e["diagnostics"][0]["code"].as_str().unwrap());}
    }
})+};}
accept! {
 f01_approval=>"sinv-approval",f02_event_is_not_state=>"sinv-rewrite",
 f03a_vacuous=>"sinv-vacuous",f03b_clock=>"sinv-clock",f04_intermediate_same_time=>"sinv-transient",
 f05_assertion_not_decision=>"sinv-latent",f06_cut_antecedent=>"sinv-cut",f07_shared=>"sinv-shared",
 f08_ended=>"sinv-ended-case",f08_clean=>"sinv-ended-clean",f09_no_range_poststate=>"sinv-range",
 f10_receipt=>"sinv-initial",f11_start=>"sinv-start-effect",f12_labels=>"sinv-label",
 f15_cancel=>"sinv-cancel",f17_sticky=>"sinv-sticky",f18_rejected=>"sinv-rejected",
 f19_bool=>"sinv-bool-type",f20_grouping=>"sinv-grouping",future=>"sinv-future",
 seed=>"sinv-seed",seed_mass=>"sinv-seed-mass",atomic_bundle=>"sinv-bundle",no_prune=>"sinv-no-prune",
 conjunction=>"sinv-conjunction",integer_collision=>"sinv-integer-collision"
}
#[test]
fn static_candidate13_complete_closed_population_and_exact_classifications() {
    let mut seen = 0;
    for n in h::names()
        .into_iter()
        .filter(|n| n.starts_with("sinv-static-"))
    {
        let e = h::expected(&n);
        h::subset(&e, &h::report(&n));
        if n.contains(".broken-") {
            h::reject(
                &h::source(&n),
                e["diagnostics"][0]["code"].as_str().unwrap(),
            );
            seen += 1;
        }
    }
    assert_eq!(seen, 30);
}
#[test]
fn population62_pairs124_and_all_replacements_are_real() {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    let mut actual: Vec<_> = std::fs::read_dir(h::root().join("fixtures"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("sinv-"))
        .collect();
    actual.sort();
    assert_eq!(actual, h::names());
    assert_eq!(actual.len(), 124);
    assert_eq!(h::pairs().len(), 62);
    let hashes: Value =
        serde_json::from_str(include_str!("support/state_invariant_fixture_hashes.json")).unwrap();
    assert_eq!(hashes.as_object().unwrap().len(), 124);
    for n in &actual {
        for (file, want) in hashes[n].as_object().unwrap() {
            let bytes = std::fs::read(h::root().join("fixtures").join(n).join(file)).unwrap();
            let mut got = String::with_capacity(64);
            for byte in Sha256::digest(&bytes) {
                write!(got, "{byte:02x}").unwrap();
            }
            assert_eq!(
                want,
                &Value::String(got),
                "frozen independent oracle {n}/{file}"
            );
        }
    }
    for p in h::pairs() {
        let n = p["name"].as_str().unwrap();
        let s = h::source(n);
        let t = h::change(&s, p["from"].as_str().unwrap(), p["to"].as_str().unwrap());
        assert_eq!(
            t,
            h::source(&format!("{n}.broken-{}", p["fault"].as_str().unwrap()))
        );
    }
    for n in actual {
        let s = h::source(&n);
        assert!(!s.contains("//") && !s.contains('#'));
    }
}
#[test]
fn old_syntax_substrates_are_accepted_before_the_new_clause_is_implemented() {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("support/state_invariant_legacy_controls.json")).unwrap();
    assert_eq!(rows.len(), 79);
    for r in rows {
        let n = r["name"].as_str().unwrap();
        assert_eq!(h::source(n), r["original"].as_str().unwrap());
        let t = r["legacy_control"].as_str().unwrap();
        assert_ne!(t, r["original"].as_str().unwrap());
        h::model(t);
    }
}
#[test]
fn shared_and_ended_case_numbers_hold_in_each_interpreter_not_only_marginal_verdict() {
    for n in [
        "sinv-shared",
        "sinv-shared.broken-no-guard",
        "sinv-ended-case",
        "sinv-ended-case.broken-increment",
        "sinv-ended-clean",
        "sinv-ended-clean.broken-increment",
        "sinv-label",
        "sinv-label.broken-person",
    ] {
        let m = h::model(&h::source(n));
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for o in [&e, &r.outcome] {
            let v = h::projection(&m, o);
            for case in 1..=2 {
                if n.starts_with("sinv-shared") || n.starts_with("sinv-label") {
                    h::case_terminal(
                        &v,
                        case,
                        "done",
                        if n == "sinv-shared.broken-no-guard" {
                            (1, 2)
                        } else {
                            (1, 1)
                        },
                        if n == "sinv-shared.broken-no-guard" {
                            (1, 1)
                        } else {
                            (0, 1)
                        },
                    );
                } else {
                    h::case_terminal(&v, case, "returned", (2, 2), (2, 2));
                    h::case_terminal(
                        &v,
                        case,
                        "posted",
                        if n == "sinv-ended-case.broken-increment" {
                            (3, 3)
                        } else {
                            (3, 4)
                        },
                        (3, 3),
                    );
                }
            }
        }
        let v = h::report(n);
        let p = h::property(&v, "contract Controls #1 invariant");
        let cs = p["cases"].as_array().unwrap();
        assert_eq!(cs.len(), 2);
        for (i, c) in cs.iter().enumerate() {
            assert_eq!(c["case_id"], i + 1);
            assert_eq!(
                c["verdict"],
                if n.contains("broken") && !n.starts_with("sinv-label") {
                    "violated"
                } else {
                    "satisfied"
                }
            );
        }
        if n == "sinv-shared.broken-no-guard" {
            assert!(v.get("capability_refusals").is_none());
        }
    }
}
#[test]
fn budget_zero_is_inconclusive_and_never_invents_receipt_witness() {
    let m = h::model(&h::source("sinv-initial"));
    let o = engine::explore_with_budget(&m, 0);
    assert!(o.truncated);
    assert!(o.terminals.is_empty());
    assert!(o.diagnostics.iter().any(|d| d.code == "W-BUDGET-0"));
    assert_eq!(o.properties[0].verdict, relca_core::Verdict::Inconclusive);
}

#[test]
fn future_receipt_and_rejected_case_time_fields_are_not_fabricated() {
    use serde_json::json;
    for n in [
        "sinv-future",
        "sinv-future.broken-received",
        "sinv-rejected",
        "sinv-rejected.broken-zero-queue",
    ] {
        let m = h::model(&h::source(n));
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for o in [&e, &r.outcome] {
            let v = h::projection(&m, o);
            let cs = v["case_outcomes"].as_array().unwrap();
            if n.starts_with("sinv-future") {
                assert_eq!(cs.len(), 1);
                assert_eq!(cs[0]["arrival"], 5);
                if n.contains("broken") {
                    h::subset(
                        &json!([{"kind":"end","end":"done","completed_at":{"min":5,"max":5},"elapsed":{"min":0,"max":0},"work":{"min":0,"max":0},"result_at":null}]),
                        &cs[0]["terminals"],
                    );
                } else {
                    assert_eq!(cs[0]["terminals"], json!([]));
                }
            } else {
                assert_eq!(cs.len(), 2);
                if n == "sinv-rejected" {
                    h::case_terminal(&v, 1, "done", (1, 1), (1, 1));
                }
                for (i, c) in cs.iter().enumerate() {
                    if n.contains("broken") || i == 1 {
                        assert_eq!(
                            c["terminals"],
                            json!([{"kind":"rejected","end":null,"completed_at":null,"elapsed":null,"work":null,"result_at":null}])
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn grain_document_person_ledger_only_changes_declared_metadata_and_source_coordinates() {
    fn normalize(v: &mut Value) {
        match v {
            Value::Object(o) => {
                for k in ["span", "line", "column", "end_line", "end_column"] {
                    o.remove(k);
                }
                for x in o.values_mut() {
                    normalize(x);
                }
            }
            Value::Array(a) => {
                for x in a {
                    normalize(x)
                }
            }
            _ => {}
        }
    }
    let source = h::source("sinv-label");
    let mut reports = Vec::new();
    for grain in ["document", "person", "ledger"] {
        let s = if grain == "document" {
            source.clone()
        } else {
            h::change(&source, "grain document", &format!("grain {grain}"))
        };
        let mut v = serde_json::to_value(report::check(&s)).unwrap();
        assert_eq!(v["verdict"], "satisfied");
        for case in 1..=2 {
            h::case_terminal(&v, case, "done", (1, 1), (0, 1));
        }
        v["model"]["sha256"] = Value::Null;
        let a = v["actors"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|a| a["name"] == "Ledger")
            .unwrap();
        assert_eq!(a["grain"]["value"], grain);
        a["grain"]["value"] = Value::Null;
        normalize(&mut v);
        reports.push(v);
    }
    assert_eq!(reports[0], reports[1]);
    assert_eq!(reports[0], reports[2]);
}
#[test]
fn bool_and_grouping_apply_only_to_state_invariants_and_preserve_weak_outer_arrow() {
    let s = h::source("sinv-approval");
    for clause in [
        "invariant registered == true -> approval == pass",
        "invariant (registered) -> (approval == pass)",
        "invariant registered -> approval == pass && registered",
        "invariant registered -> (approval == pass || registered == false)",
    ] {
        let t = h::change(&s, "invariant registered -> approval == pass", clause);
        assert_eq!(report::check(&t).verdict, relca_core::Verdict::Satisfied);
    }
    let optional = h::source("sinv-bool-type.broken-optional");
    let t = h::change(&optional, "invariant ok", "invariant ok == true");
    assert_eq!(report::check(&t).verdict, relca_core::Verdict::Satisfied);
    // Legacy process and responsibility grammar do not gain bare latent decisions.
    let t = h::change(&s, "if approval == fail", "if registered");
    h::reject(&t, "E-SYN-0");
}
#[test]
fn cut_checks_observed_or_latent_locals_on_both_sides_and_under_false_groups() {
    let base = h::source("sinv-cut");
    for observed in [false, true] {
        for right in [false, true] {
            let mut s = base.clone();
            if observed {
                s = h::change(
                    &s,
                    "latent difficulty: Difficulty = routine",
                    "state difficulty: Difficulty = routine",
                );
            }
            if right {
                s = h::change(
                    &s,
                    "Invoice.difficulty == routine && registered -> approval == pass",
                    "registered -> Invoice.difficulty == routine && approval == pass",
                );
            }
            assert_eq!(report::check(&s).verdict, relca_core::Verdict::Satisfied);
            let t = h::change(
                &s,
                "include [Approver, Ledger, Invoice]",
                "include [Approver, Ledger]; exclude Invoice reason \"out\" loses \"state\"",
            );
            h::reject(&t, "E-CUT-1");
        }
    }
}
#[test]
fn model_wide_alias_check_is_not_selected_process_local() {
    let src = r#"model M version "0.2" {
      type V=enum{pass,fail}
      observed case { approval: optional<V> }
      actor A {capacity 1;capability good()->V{duration fixed 1min;result nondet{pass};writes approval} capability other()->V{duration fixed 1min;result nondet{pass};writes approval}}
      process Good {approval=A.good();end done}
      process Other {approval=A.other();end done}
      cut C{include[A]}
      configuration X{cut C;process Good} configuration Y{cut C;process Other}
      bounds{cases 1;live_tasks 1;clock 3min}
      contract Controls{invariant approval == pass -> approval == pass}
    }"#;
    for plan in ["X", "Y"] {
        assert_eq!(
            report::check_configuration(src, plan).verdict,
            relca_core::Verdict::Satisfied
        );
        let t = h::change(
            src,
            "result nondet{pass};writes approval}}",
            "result nondet{pass}}}",
        );
        let r = serde_json::to_value(report::check_configuration(&t, plan)).unwrap();
        assert_eq!(r["verdict"], "unsupported");
        assert_eq!(r["diagnostics"][0]["code"], "U-SEM-0");
        assert!(r["terminals"].as_array().unwrap().is_empty());
    }
}
#[test]
fn alias_fixture_is_checked_in_both_configurations() {
    for n in ["sinv-alias", "sinv-alias.broken-other-writer"] {
        let s = h::source(n);
        for plan in ["X", "Y"] {
            let v = serde_json::to_value(report::check_configuration(&s, plan)).unwrap();
            h::subset(&h::expected(n), &v);
        }
    }
}

#[test]
fn latent_collision_in_nonselected_process_is_not_hidden_by_name_resolution() {
    let s = h::source("sinv-alias");
    let s = h::change(
        &s,
        "  actor A",
        "  latent case { j: bool = false }\n  actor A",
    );
    let s = h::change(
        &s,
        "invariant approval == pass -> approval == pass",
        "invariant j == false",
    );
    let bad = h::change(
        &s,
        "process Other { approval = A.other(); end done }",
        "process Other { j = A.other(); end done }",
    );
    for plan in ["X", "Y"] {
        assert_eq!(
            report::check_configuration(&s, plan).verdict,
            relca_core::Verdict::Satisfied
        );
        let v = serde_json::to_value(report::check_configuration(&bad, plan)).unwrap();
        assert_eq!(v["verdict"], "unsupported");
        assert_eq!(v["diagnostics"][0]["code"], "U-SEM-0");
        assert!(
            v["diagnostics"][0]["message"]
                .as_str()
                .unwrap()
                .contains("both a binding and a latent")
        );
    }
}
#[test]
fn state_read_checks_are_not_short_circuited_by_false_antecedents() {
    let s = h::source("sinv-approval");
    for clause in [
        "invariant registered == false -> j == pass",
        "invariant registered -> (approval == pass || j == pass)",
        "invariant (registered && j == pass) -> registered",
    ] {
        h::reject(
            &h::change(&s, "invariant registered -> approval == pass", clause),
            "U-SEM-0",
        );
    }
}
#[test]
fn public_property_identity_retains_invariant_suffix() {
    let m = h::model(&h::source("sinv-approval"));
    // Typed malformed-new-IR construction is an explicit implementation-side
    // obligation until the new ContractKind exists; this test checks identity.
    assert_eq!(
        m.contracts[0].property_id(ir::ClauseId(0)),
        "contract Controls #1 invariant"
    );
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
}
