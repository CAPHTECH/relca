//! Test first: these are intended RED until ADR 0065 is implemented.
#[path = "support/r2_hand.rs"]
mod h;
#[path = "support/r2_inputs.rs"]
mod i;
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::json;
fn model(s: &str) -> relca_core::ir::Model {
    lower::lower(&syntax::parse(s).expect("R2 grammar")).expect("R2 independent static contract")
}
macro_rules! runtime {
    ($name:ident,$id:literal) => {
        #[test]
        fn $name() {
            let v = json!(report::check(&i::source($id)));
            h::check(&v, $id);
            h::waiting(&v, $id);
            h::strict_roundtrip(&v);
        }
    };
}
runtime!(r01_proceed_evidence3_decision6_end7, "R01");
runtime!(r02_unavailable_unknown_is_unfinished, "R02");
runtime!(r03_without_handler_preserves_old_stop, "R03");
runtime!(r04_refund_keeps_charge_and_separate_refund, "R04");
runtime!(r05_final_record_does_not_rewrite_decision, "R05");
runtime!(r06_take_over_is_not_financial_evidence, "R06");
runtime!(r07_intentional_wait_is_not_work, "R07");
runtime!(r08_one_handler_after_exhaustion_not_per_unknown, "R08");
runtime!(r09_denied_approval_is_not_failed_payment, "R09");
#[test]
fn old_position_words_remain_bindings_and_identifiers() {
    for word in i::WORDS {
        let s = i::old_binding(word);
        syntax::parse(&s).expect(word);
        let r = report::check(&s);
        assert_eq!(
            r.verdict,
            relca_core::Verdict::Satisfied,
            "{word}: {:?}",
            r.diagnostics
        );
        h::strict_roundtrip(&json!(r));
    }
    let bad = i::edit(
        &i::old_binding("on"),
        "on = A.run()",
        "on = A.run(); frobnicate extension",
    );
    let ds = syntax::parse(&bad).unwrap_err();
    assert_eq!(ds.len(), 1);
    assert_eq!(ds[0].code, "U-SYN-0");
    assert_eq!(ds[0].message, "unsupported syntax: frobnicate");
    let start = bad.find("frobnicate").unwrap();
    assert_eq!(
        ds[0].span,
        Some(relca_core::diag::Span {
            start,
            end: start + "frobnicate".len()
        })
    );
}
#[test]
fn old_substrate_has_valid_capabilities_and_no_recovery() {
    let s = i::source("R03");
    let m = model(&s);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert!(!e.truncated);
    assert!(!r.outcome.truncated);
    for o in [&e, &r.outcome] {
        assert!(
            o.diagnostics
                .iter()
                .all(|d| d.severity != relca_core::diag::Severity::Error)
        );
        assert!(o.terminals.is_empty());
        assert_eq!(o.response_receipts.as_ref().unwrap().len(), 1);
    }
}
#[test]
fn both_interpreters_are_each_checked_against_hand_not_only_each_other() {
    for id in [
        "R01", "R02", "R03", "R04", "R05", "R06", "R07", "R08", "R09",
    ] {
        let m = model(&i::source(id));
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete, "{id}");
        for (system, o) in [("engine", &e), ("reference", &r.outcome)] {
            assert!(!o.truncated, "{system}/{id}");
            assert!(
                o.diagnostics
                    .iter()
                    .all(|d| d.severity != relca_core::diag::Severity::Error),
                "{system}/{id}"
            );
            let (t, w, end, _, _, _) = h::timeline(id);
            if let Some(end) = end {
                let ts: Vec<_> = o
                    .terminals
                    .iter()
                    .filter(|t| m.ends[t.end.0] == end)
                    .collect();
                assert_eq!(ts.len(), 1);
                assert_eq!(ts[0].elapsed, relca_core::outcome::Range { min: t, max: t });
                assert_eq!(ts[0].work, relca_core::outcome::Range { min: w, max: w });
            } else {
                assert!(o.terminals.is_empty());
                let term = &o.case_outcomes.as_ref().unwrap()[0].terminals[0];
                assert_eq!(
                    term.kind,
                    relca_core::outcome::CaseTerminalKind::ReceiptUnknown
                );
                assert_eq!(
                    term.observed_work,
                    Some(relca_core::outcome::Range { min: w, max: w })
                );
            }
        }
        assert_eq!(
            e.case_outcomes, r.outcome.case_outcomes,
            "after independent hand checks: {id}"
        );
    }
}
#[test]
fn missing_new_form_is_invalid_while_old_extension_remains_unsupported() {
    for (s, code, reason) in [
        (
            i::edit(
                &i::source("R01"),
                "on receipt_unknown {",
                "on receipt_unknown",
            ),
            "E-SYN-0",
            "invalid_recovery_syntax",
        ),
        (
            i::edit(&i::source("R01"), "for command pay", "for command"),
            "E-SYN-0",
            "invalid_recovery_syntax",
        ),
        (
            i::edit(
                &i::source("R01"),
                "end shipped\n   }",
                "sh2 = Shop.instruct_ship(decision)\n   }",
            ),
            "E-SYN-0",
            "invalid_recovery_syntax",
        ),
        (
            i::edit(
                &i::source("R01"),
                "status = Psp.query_status(case.id) for command pay",
                "status = Psp.query_status(case.id) for command missing",
            ),
            "E-NAME-0",
            "unknown_recovery_reference",
        ),
        (
            i::edit(
                &i::old_binding("on"),
                "on = A.run()",
                "on = A.run(); frobnicate extension",
            ),
            "U-SYN-0",
            "",
        ),
    ] {
        h::rejection(&json!(report::check(&s)), code, reason);
    }
}
#[test]
fn suffix_order_newline_and_semicolon_boundary_are_distinct() {
    let s = i::source("R08");
    model(&s);
    model(&i::edit(
        &s,
        "exhausted end failed on receipt_unknown",
        "exhausted end failed\non receipt_unknown",
    ));
    for old in [
        "exhausted end failed; on receipt_unknown",
        "exhausted end failed on receipt_unknown",
    ] {
        if old.contains(';') {
            let bad = i::edit(&s, "exhausted end failed on receipt_unknown", old);
            h::rejection(&json!(report::check(&bad)), "U-SYN-0", "");
        }
    }
    let s = i::source("R01");
    let bad = i::edit(
        &s,
        "query_status(case.id) for command pay",
        "query_status(case.id); for command pay",
    );
    h::rejection(&json!(report::check(&bad)), "U-SYN-0", "");
}
#[test]
fn handler_binding_resolve_and_stop_are_not_stolen() {
    for word in ["on", "for", "resolve", "stop"] {
        let b = format!("{word} = CS.take_over(status)\nstop receipt_unknown");
        let handler = format!("status = Psp.query_status(case.id) for command pay\n{b}");
        model(&i::body(Some(&handler), i::LOSS));
    }
}
#[test]
fn all_allowed_handler_statements_parse_and_all_disallowed_forms_refuse() {
    for id in ["R01", "R04", "R05", "R06", "R07"] {
        model(&i::source(id));
    }
    for stmt in [
        "fork xs { z = CS.take_over(status) }; join all xs",
        "wait until status == committed_once",
        "degrade if status == committed_once {} else {}",
        "retry 3",
        "on receipt_unknown {}",
    ] {
        let s = i::edit(
            &i::source("R02"),
            "if status == committed_once {",
            &format!("{stmt}\nif status == committed_once {{"),
        );
        h::rejection(&json!(report::check(&s)), "U-SYN-0", "");
    }
    for suffix in [
        "response_loss none",
        "response_loss sequence [unknown]",
        "response_loss none retry max_attempts 2 when result [recorded] backoff fixed 0min exhausted end failed",
    ] {
        let s = i::edit(
            &i::source("R04"),
            "r = Psp.refund(v)",
            &format!("r = Psp.refund(v) {suffix}"),
        );
        h::rejection(
            &json!(report::check(&s)),
            "U-SEM-0",
            "recovery_feature_combination",
        );
    }
}
#[test]
fn resolve_requires_direct_known_dominating_matching_evidence() {
    for (old, new) in [
        (
            "resolve effect pay using status",
            "resolve effect pay using decision",
        ),
        (
            "resolve effect pay using status",
            "resolve effect pay using absent",
        ),
        (
            "resolve effect pay using status",
            "resolve effect normal using status",
        ),
        (
            "if status == committed_once {",
            "if status == still_unknown {",
        ),
        (
            "if status == committed_once {",
            "if status == committed_once || status == still_unknown {",
        ),
    ] {
        let s = i::edit(&i::source("R01"), old, new);
        h::rejection(
            &json!(report::check(&s)),
            if new.contains("absent") {
                "E-NAME-0"
            } else {
                "E-TYPE-0"
            },
            if new.contains("absent") {
                "unknown_recovery_reference"
            } else {
                "recovery_evidence_mismatch"
            },
        );
    }
    let handler = "resolve effect pay using status\nstatus = Psp.query_status(case.id) for command pay\nstop receipt_unknown";
    h::rejection(
        &json!(report::check(&i::body(Some(handler), i::LOSS))),
        "E-TYPE-0",
        "recovery_evidence_mismatch",
    );
}
#[test]
fn end_stop_and_double_resolve_preserve_unresolved_contract() {
    for (old, new) in [
        (
            "resolve effect pay using status",
            "resolve effect pay using status\nresolve effect pay using status",
        ),
        ("end shipped\n   }", "stop receipt_unknown\n   }"),
        (
            "on receipt_unknown {",
            "on receipt_unknown { stop receipt_unknown } on receipt_unknown {",
        ),
    ] {
        h::rejection(
            &json!(report::check(&i::edit(&i::source("R01"), old, new))),
            "E-CON-0",
            "invalid_recovery_contract",
        );
    }
    let s = i::edit(&i::source("R01"), "resolve effect pay using status", "");
    h::rejection(
        &json!(report::check(&s)),
        "E-CON-0",
        "invalid_recovery_contract",
    );
}
#[test]
fn unavailable_is_not_known_fail_and_nondet_is_not_half_probability() {
    let s = i::edit(
        &i::source("R01"),
        "availability available",
        "availability nondet",
    );
    let v = json!(report::check(&s));
    h::verdict(&v, "inconclusive");
    let ts = &v["case_outcomes"][0]["terminals"];
    assert_eq!(
        h::row(ts, "kind", "receipt_unknown")["observed_until"],
        h::point(3)
    );
    assert_eq!(h::row(ts, "end", "shipped")["elapsed"], h::point(7));
    for path in h::rows(&v["response_recovery"]["paths"]) {
        let q = &path["commands"][0]["queries"][0];
        assert!(["committed_once", "still_unknown"].contains(&q["status"].as_str().unwrap()));
        assert!(q.get("probability").is_none());
    }
    let v = json!(report::simulate(&s));
    h::verdict(&v, "unsupported");
    assert!(
        h::rows(&v["diagnostics"])
            .iter()
            .any(|d| d["code"] == "U-SEM-0")
    );
}
#[test]
fn query_source_is_read_only_and_corresponds_to_one_g() {
    for extra in [
        "commits;",
        "writes refunded;",
        "finish { refunded = true };",
        "start { refunded = true };",
        "result nondet {committed_once};",
        "requires refunded == true else still_unknown;",
        "idempotency key key;",
        "effect_scope modeled;",
    ] {
        let s = i::edit(
            &i::source("R01"),
            "duration fixed 2min; receipt_query",
            &format!("{extra} duration fixed 2min; receipt_query"),
        );
        h::rejection(
            &json!(report::check(&s)),
            "U-SEM-0",
            "recovery_query_unmodeled",
        );
    }
    let s = i::edit(
        &i::source("R01"),
        "receipt_query charge_keyed key key",
        "receipt_query refund key key",
    );
    h::rejection(
        &json!(report::check(&s)),
        "U-SEM-0",
        "recovery_query_unmodeled",
    );
}
#[test]
fn latent_and_protected_observed_provider_state_cannot_drive_business() {
    for condition in [
        "hidden_truth == true",
        "charged_truth == true",
        "case.id in Psp.keys",
        "charges == 1",
    ] {
        let s = i::edit(
            &i::source("R01"),
            "if status == committed_once {",
            &format!("if {condition} {{"),
        );
        h::rejection(
            &json!(report::check(&s)),
            if condition.starts_with("hidden_truth") {
                "E-POL-LATENT"
            } else {
                "E-TYPE-0"
            },
            if condition.starts_with("hidden_truth") {
                ""
            } else {
                "recovery_provider_state_leak"
            },
        );
    }
    let s = i::edit(
        &i::source("R01"),
        "capability decide(s: PaymentStatus) -> Decision {",
        "capability decide(s: PaymentStatus) -> Decision { requires charges == 1 else withdraw;",
    );
    h::rejection(
        &json!(report::check(&s)),
        "E-TYPE-0",
        "recovery_provider_state_leak",
    );
}
#[test]
fn query_limit_counts_unreachable_lexical_calls_32_not_33() {
    for n in [32, 33] {
        let queries = (0..n)
            .map(|j| format!("q{j} = Psp.query_status(case.id) for command pay\n"))
            .collect::<String>();
        let s = i::body(
            Some(&format!(
                "if charged_truth == true {{{queries}stop receipt_unknown}} else {{stop receipt_unknown}}"
            )),
            i::LOSS,
        );
        // The unprotected false refund flag makes the entire query arm dead.
        let s = i::edit(&s, "if charged_truth == true", "if refunded == true");
        if n == 32 {
            model(&s);
        } else {
            h::rejection(
                &json!(report::check(&s)),
                "U-SEM-0",
                "recovery_expansion_limit",
            );
        }
    }
}
#[test]
fn arrival_deadlines_are_inclusive_and_resolution_cannot_reset_them() {
    for (id, late, by) in [("T01", 6, 7), ("T02", 5, 6), ("T07", 5, 6)] {
        for (s, expected) in [
            (i::source(id), "satisfied"),
            (
                i::edit(
                    &i::source(id),
                    &format!("by {by}min"),
                    &format!("by {late}min"),
                ),
                "violated",
            ),
        ] {
            let v = json!(report::check(&s));
            h::admitted(&v);
            let ps: Vec<_> = h::rows(&v["properties"])
                .iter()
                .filter(|p| p["id"].as_str().unwrap().contains("Deadline"))
                .collect();
            assert_eq!(ps.len(), 1);
            assert_eq!(ps[0]["verdict"], expected);
        }
    }
    let v = json!(report::check(&i::source("T03")));
    assert!(
        h::rows(&v["properties"])
            .iter()
            .any(|p| p["id"].as_str().unwrap().contains("Deadline") && p["verdict"] == "violated")
    );
}
#[test]
fn clock_and_business_horizon_do_not_become_receipt_stops() {
    for id in ["T04", "T05", "T06"] {
        let s = i::source(id);
        let r = if id == "T05" {
            report::check_with_selection(&s, None, Some("Day"))
        } else {
            report::check(&s)
        };
        let v = json!(r);
        h::verdict(&v, "inconclusive");
        assert_eq!(v["bounds"]["truncated"], id != "T05");
        assert_eq!(
            h::row(&v["properties"], "id", "receipt_resolution")["verdict"],
            "inconclusive"
        );
        if id == "T05" {
            assert_eq!(v["case_outcomes"][0]["terminals"][0]["kind"], "cut_off");
        } else {
            assert!(!v["case_outcomes"].to_string().contains("receipt_unknown"));
            assert!(v["metrics"]["overall"]["work"]["min"].is_null());
            assert_eq!(v["metrics"]["overall"]["work"]["reason"], "truncated");
        }
    }
}
#[test]
fn b_resolution_is_refused_but_old_b_double_commit_is_preserved() {
    let g = "idempotency key key; commits; writes [keys,charged_truth,charges]; duration fixed 1min\n     atomic { if key in keys { result rejected_duplicate } else { keys += key; charged_truth = true; charges += 1; result charged } }";
    let b = "effect_scope modeled; commits; writes [charged_truth,charges]; duration fixed 1min; result nondet {charged}; finish {charged_truth = true; charges += 1}";
    h::rejection(
        &json!(report::check(&i::edit(&i::source("R08"), g, b))),
        "U-SEM-0",
        "recovery_feature_combination",
    );
    let old = i::edit(&i::body(None, i::RETRY), g, b);
    let v = json!(report::check(&old));
    h::verdict(&v, "violated");
    assert!(
        h::rows(&v["properties"]).iter().any(|p| p["id"]
            .as_str()
            .unwrap()
            .contains("at_most_once")
            && p["verdict"] == "violated")
    );
}

#[test]
fn old_b_without_handler_retains_double_commit_and_unknown_counterexample() {
    let source = i::edit(
        &i::body(None, i::RETRY),
        "idempotency key key; commits; writes [keys,charged_truth,charges]; duration fixed 1min\n     atomic { if key in keys { result rejected_duplicate } else { keys += key; charged_truth = true; charges += 1; result charged } }",
        "effect_scope modeled; commits; writes [charged_truth,charges]; duration fixed 1min; result nondet {charged}; finish {charged_truth = true; charges += 1}",
    );
    let v = json!(report::check(&source));
    h::verdict(&v, "violated");
    let once: Vec<_> = h::rows(&v["properties"])
        .iter()
        .filter(|p| p["id"].as_str().unwrap().contains("at_most_once"))
        .collect();
    assert_eq!(once.len(), 1);
    assert_eq!(once[0]["verdict"], "violated");
    let u = h::row(
        &v["case_outcomes"][0]["terminals"],
        "kind",
        "receipt_unknown",
    );
    assert_eq!(u["observed_work"], h::point(2));
    assert_eq!(u["work"], serde_json::Value::Null);
    assert_eq!(h::rows(&v["response_receipts"]).len(), 2);
}
#[test]
fn original_unknown_ack_cannot_be_read_or_implicitly_resumed() {
    let s = i::edit(
        &i::source("R01"),
        "if status == committed_once {",
        "if pay == charged {",
    );
    h::rejection(
        &json!(report::check(&s)),
        "E-TYPE-0",
        "recovery_evidence_mismatch",
    );
    let s = i::edit(
        &i::source("R04"),
        "r = Psp.refund(v)",
        "cancel pay\nr = Psp.refund(v)",
    );
    h::rejection(&json!(report::check(&s)), "U-SYN-0", "");
}
