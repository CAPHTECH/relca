//! ADR0046 intake, test-first. See mutation_integration_calculations.md.
#[path = "../../relca-cli/src/fixture_match.rs"]
mod fixture_match;
#[path = "support/mutation_integration_inputs.rs"]
mod i;
#[path = "support/mutation_default_population.rs"]
mod population;
use relca_core::{engine, lower, mutation, reference, report, syntax};
use serde_json::{Value, json};

fn checked(s: &str) -> Value {
    json!(report::check(s))
}
fn mutated(s: &str) -> Value {
    json!(mutation::mutate(s))
}
fn rows(v: &Value, k: usize) -> &[Value] {
    v["mutations"][k]["mutants"].as_array().unwrap()
}
fn only(v: &Value, k: usize) -> &Value {
    assert_eq!(rows(v, k).len(), 1, "kind {k}: {v}");
    &rows(v, k)[0]
}
fn time(v: &Value, end: &str, t: u64) {
    let r = v["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["end"] == end)
        .unwrap();
    assert_eq!(r["elapsed"], json!({"min":t,"max":t}));
    assert_eq!(r["work"], json!({"min":t,"max":t}));
}
fn both(s: &str, scenario: Option<&str>) -> Vec<Value> {
    let a = syntax::parse(s).unwrap_or_else(|d| panic!("substrate parse: {d:?}\n{s}"));
    let m = lower::lower_with_selection(&a, None, scenario)
        .unwrap_or_else(|d| panic!("substrate lower: {d:?}\n{s}"));
    let e = engine::explore(&m);
    let mut r = reference::explore(&m);
    assert_eq!(
        r.stop,
        reference::StopReason::Complete,
        "no silent exclusion"
    );
    assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
    r.outcome.diagnostics.drain(..m.diagnostics.len());
    assert_eq!(e, r.outcome, "whole independent runtime outcome");
    vec![json!(report::check_with_selection(s, None, scenario))]
}
fn held(m: &Value, subjects: Value, before: usize, after: usize) {
    assert_eq!(m["result"], "detected");
    assert!(m["reason"].is_null());
    let cs = m["assigned_changes"].as_array().unwrap();
    assert_eq!(cs.len(), 3);
    assert_eq!(cs[0]["field"], "subject_sources");
    assert_eq!(cs[0]["before"], subjects);
    assert_eq!(cs[0]["after"], subjects);
    assert_eq!(cs[1]["before"], json!([]));
    assert_eq!(cs[1]["after"], json!([]));
    assert_eq!(cs[2]["check"], "agency.lineage_groups");
    assert_eq!(cs[2]["field"], "judgements");
    assert_eq!(cs[2]["before"], before);
    assert_eq!(cs[2]["after"], after);
    assert!(cs.iter().all(|c| c["direction"] == "held"));
}

#[test]
fn held_helper_checks_input_specific_counts_even_when_direction_is_held() {
    // Helper calibration only, not evidence that Mutation generated this row.
    for (before, after) in [(3, 4), (2, 2)] {
        let m = json!({
            "result": "detected", "reason": null,
            "assigned_changes": [
                {"field":"subject_sources", "before":["Invoice"], "after":["Invoice"], "direction":"held"},
                {"before":[], "after":[], "direction":"held"},
                {"check":"agency.lineage_groups", "field":"judgements", "before":before, "after":after, "direction":"held"}
            ]
        });
        held(&m, json!(["Invoice"]), before, after);
        for field in ["before", "after"] {
            let mut broken = m.clone();
            broken["assigned_changes"][2][field] = json!(0);
            assert!(
                std::panic::catch_unwind(|| held(&broken, json!(["Invoice"]), before, after))
                    .is_err(),
                "wrong {field} must fail even with held direction"
            );
        }
    }
}

#[test]
fn t13d_shared_writer_version_check_does_not_become_a_summary_site() {
    for version in [false, true] {
        let s = i::version(version);
        for v in both(&s, None) {
            assert_eq!(v["verdict"], "satisfied");
            time(&v, "done", 4);
            time(&v, "rejected", 3);
        }
        let v = mutated(&s);
        assert_eq!(v["mutations"][4]["status"], "not_applicable");
        assert_eq!(v["mutations"][4]["reason"], "no_matching_site");
        assert!(rows(&v, 4).is_empty());
    }
}
#[test]
fn t13h_selected_scenario_is_used_for_site_context_and_mutant_execution() {
    let s = i::scenario(i::X);
    for v in both(&s, Some("Day")) {
        assert_eq!(v["verdict"], "satisfied");
    }
    let v = json!(mutation::mutate_set_with_scenario(
        &s,
        "bpr-default",
        Some("Day")
    ));
    assert_eq!(v["verdict"], "unsupported");
    let m = only(&v, 1);
    assert_eq!(m["result"], "detected");
    assert_eq!(m["report"]["scenario"], v["original"]["scenario"]);
    assert_eq!(m["assigned_changes"][0]["direction"], "worsened");
    assert_eq!(m["assigned_changes"][0]["before"], "satisfied");
    assert_eq!(m["assigned_changes"][0]["after"], "violated");
}
#[test]
fn t13h_missing_scenario_selector_is_a_located_gate_not_a_panic() {
    let s = i::scenario(i::X);
    let v = mutated(&s);
    assert_eq!(v["verdict"], "invalid");
    assert_eq!(v["original"]["diagnostics"][0]["code"], "E-CONF-0");
    assert!(v["original"]["diagnostics"][0]["span"].is_object());
    for k in [0, 1, 2, 4, 8, 9] {
        assert_eq!(v["mutations"][k]["status"], "blocked");
        assert!(rows(&v, k).is_empty());
    }
}
#[test]
fn t20_observed_read_sources_follow_writer_not_direct_reads_only() {
    let s = i::observed(true, false, false);
    for v in both(&s, None) {
        time(&v, "done", 5);
        time(&v, "rejected", 4);
    }
    for v in both(&i::manual(&s), None) {
        time(&v, "done", 6);
        time(&v, "rejected", 5);
    }
    let v = mutated(&s);
    held(only(&v, 4), json!(["Invoice"]), 3, 4);
    assert_eq!(only(&v, 4)["site"]["statement_path"], "body/1");
}
#[test]
fn t20_source_removed_is_unassigned_but_still_enumerated() {
    let v = mutated(&i::observed(false, false, false));
    let m = only(&v, 4);
    assert_eq!(m["result"], "undetected");
    assert_eq!(m["reason"], "no_assigned_check");
    assert_eq!(m["assigned_changes"], json!([]));
}
#[test]
fn t20_all_lexical_preceding_writers_are_included() {
    let s = i::observed(true, true, false);
    for v in both(&s, None) {
        time(&v, "done", 6);
        time(&v, "rejected", 5);
    }
    for v in both(&i::manual(&s), None) {
        time(&v, "done", 7);
        time(&v, "rejected", 6);
    }
    held(only(&mutated(&s), 4), json!(["Archive", "Invoice"]), 3, 4);
}
#[test]
fn t20_whole_record_observed_read_keeps_sources_and_field_guard() {
    let s = i::observed(true, true, true);
    let s = i::replace(
        &s,
        "if p == pass && a == pass",
        "if e.confidence == high && p == pass && a == pass",
    );
    for v in both(&s, None) {
        time(&v, "done", 6);
    }
    held(only(&mutated(&s), 4), json!(["Archive", "Invoice"]), 3, 4);
}
#[test]
fn t20_field_reads_stay_one_unsupported_error_with_exact_span() {
    let s = i::replace(
        &i::observed(true, false, true),
        "reads [snap]",
        "reads [snap.confidence]",
    );
    let v = mutated(&s);
    assert_eq!(v["verdict"], "unsupported");
    let errors: Vec<_> = v["original"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0]["code"], "U-SYN-0");
    assert_eq!(errors[0]["message"], "Actor state reads are unsupported");
    let at = s.find("snap.confidence").unwrap();
    assert_eq!(
        errors[0]["span"],
        json!({"start":at,"end":at+"snap.confidence".len()})
    );
    for k in [0, 1, 2, 4, 8, 9] {
        assert_eq!(v["mutations"][k]["status"], "blocked");
    }
}
#[test]
fn a4_record_repeated_arguments_and_guard_are_not_silently_rewritten() {
    let s = i::record();
    for v in both(&s, None) {
        time(&v, "done", 4);
        time(&v, "rejected", 3);
    }
    let manual = i::manual(&s);
    assert!(manual.contains("if e.confidence == high"));
    assert!(!manual.contains("if e_dup.confidence"));
    for v in both(&manual, None) {
        time(&v, "done", 5);
        time(&v, "rejected", 4);
    }
    let v = mutated(&s);
    let m = only(&v, 4);
    held(m, json!(["Invoice"]), 2, 2);
    assert_eq!(m["site"]["duplicate_binding"], "e_dup");
    let mut want = checked(&manual);
    let mut got = m["report"].clone();
    // Independently parsed hand edit has shifted spans/hash; only those are erased.
    fn erase(v: &mut Value) {
        match v {
            Value::Object(o) => {
                o.remove("span");
                if o.contains_key("sha256") {
                    o.remove("sha256");
                }
                for x in o.values_mut() {
                    erase(x);
                }
            }
            Value::Array(xs) => {
                for x in xs {
                    erase(x)
                }
            }
            _ => {}
        }
    }
    erase(&mut want);
    erase(&mut got);
    assert_eq!(got, want);
}
#[test]
fn d10_quality_delta_is_carried_without_becoming_an_assigned_or_other_check() {
    let s = i::quality();
    let v = mutated(&s);
    let m = only(&v, 4);
    assert_eq!(v["original"]["quality"]["cohort"][0]["value"]["lower"], "1");
    assert_eq!(m["report"]["quality"]["cohort"][0]["value"]["lower"], "0");
    assert_ne!(v["original"]["quality"], m["report"]["quality"]);
    held(m, json!(["Invoice"]), 2, 2);
    assert!(
        m["other_changes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["check"] != "quality")
    );
    assert!(
        v["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("quality_changes"))
    );
    // B7 closes C6's deferred kind7 obligation. Arrivals has the two lexical
    // ends rejected/done even though deterministic pass makes rejected unseen.
    assert_eq!(v["mutations"][7]["status"], "generated");
    assert!(v["mutations"][7]["reason"].is_null());
    let definitions = rows(&v, 7);
    assert_eq!(
        definitions
            .iter()
            .map(|m| m["site"]["removed_end"].clone())
            .collect::<Vec<_>>(),
        vec![json!("rejected"), json!("done")]
    );
    assert!(definitions.iter().all(|m| m["result"] == "detected"
        && m["report"].is_null()
        && m["other_changes"] == json!([])));
}
#[test]
fn a6_change_only_authority_is_not_an_expiry_site_and_rule_values_are_preserved() {
    let s = i::rule(false, false);
    let v = mutated(&s);
    assert_eq!(v["original_verdict"], "satisfied");
    assert_eq!(v["mutations"][1]["status"], "not_applicable");
    held(only(&v, 4), json!(["Invoice"]), 2, 2);
    let original = &v["original"]["actors"];
    assert_eq!(only(&v, 4)["report"]["actors"], *original);
    assert!(
        v["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("authority_expiry_unnamed"))
    );
    assert_eq!(v["mutations"][5]["reason"], "rule_version_unavailable");
}
#[test]
fn a6_normal_basis_and_change_grant_share_one_expiry_without_dropping_metadata() {
    let s = i::rule(true, false);
    let v = mutated(&s);
    assert_eq!(
        v["original_verdict"], "satisfied",
        "{}",
        v["original"]["diagnostics"]
    );
    let m = only(&v, 1);
    assert_eq!(m["site"]["authority"], "RuleChange");
    assert_eq!(m["result"], "invalid_mutant");
    assert_eq!(m["reason"], "static_unsupported");
    assert_eq!(m["assigned_changes"], json!([]));
    assert_eq!(v["run"]["completion"], "inconclusive");
    assert_eq!(
        rows(&v, 4).len(),
        1,
        "the invalid Authority mutant is not a summary candidate"
    );
    assert_eq!(
        rows(&v, 4)
            .iter()
            .filter(|r| r["result"] == "detected")
            .count(),
        1
    );
    assert!(
        rows(&v, 4)
            .iter()
            .all(|r| r["id"].as_str().unwrap().starts_with("summary_reused:"))
    );
    assert!(
        m["report"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["message"] == "restricted rule-change Authority is unsupported")
    );
}
#[test]
fn a6_quality_runs_normally_without_obsolete_rule_disclosure_in_every_mutant() {
    let v = mutated(&i::rule(false, true));
    assert_eq!(
        v["original"]["quality"]["cohort"][0]["value"],
        json!({"lower":"0","upper":"1","basis":"per_case_relaxation","reason":null})
    );
    let mut reports = vec![&v["original"]];
    for k in 0..10 {
        for m in rows(&v, k) {
            if !m["report"].is_null() {
                reports.push(&m["report"]);
            }
        }
    }
    assert!(reports.len() > 1);
    for r in reports {
        if !r["quality"].is_null() && r["verdict"] != "unsupported" && r["verdict"] != "invalid" {
            assert_ne!(r["quality"]["cohort"][0]["value"]["reason"], "aborted");
            assert!(r["quality"]["cohort"][0]["value"]["lower"].is_string());
            assert!(r["quality"]["cohort"][0]["value"]["upper"].is_string());
        }
        let assumptions = r["assumptions"].as_array().unwrap();
        assert_eq!(
            assumptions
                .iter()
                .filter(|a| a["field"] == "quality.rule")
                .count(),
            0
        );
    }
}
#[test]
fn a6_dynamic_amendment_is_blocked_not_a_mutation_of_rule_state() {
    let s = i::replace(
        &i::rule(false, false),
        "e = Extract.make();",
        "amend ApprovalRule from \"r1\" to \"r2\" by cfo under RuleChange { required = false }; e = Extract.make();",
    );
    let v = mutated(&s);
    assert_eq!(v["original_verdict"], "unsupported");
    for k in [0, 1, 2, 4, 8, 9] {
        assert_eq!(v["mutations"][k]["status"], "blocked");
        assert!(rows(&v, k).is_empty());
    }
    assert_eq!(v["mutations"][5]["reason"], "rule_version_unavailable");
}

#[test]
fn a6_fresh_names_include_decides_fields_and_keep_the_no_collision_control() {
    for occupied in [false, true] {
        let mut s = i::rule(false, false);
        if occupied {
            s = i::replace(
                &s,
                "decides required: bool = true",
                "decides required: bool = true; decides e_dup: bool = true",
            );
        }
        assert_eq!(checked(&s)["verdict"], "satisfied");
        let v = mutated(&s);
        let m = only(&v, 4);
        assert_eq!(
            m["site"]["duplicate_binding"],
            if occupied { "e_dup2" } else { "e_dup" }
        );
        held(m, json!(["Invoice"]), 2, 2);
    }
}
#[test]
fn a6_static_amend_configuration_without_selector_is_blocked() {
    let s = include_str!("../../../fixtures/rule-basic/model.relca");
    let v = mutated(s);
    assert_eq!(v["original_verdict"], "invalid");
    assert_eq!(v["original"]["diagnostics"][0]["code"], "E-CONF-0");
    for k in [0, 1, 2, 4, 8, 9] {
        assert_eq!(v["mutations"][k]["status"], "blocked");
        assert!(rows(&v, k).is_empty());
    }
}
#[test]
fn held_drain_original_and_independent_summary_edit_keep_case_arrivals_and_work() {
    for manual in [false, true] {
        let mut s = i::held_summary(None, 20);
        if manual {
            s = i::manual(&s);
        }
        let service = if manual { 5 } else { 4 };
        for v in both(&s, Some("Day")) {
            assert_eq!(v["verdict"], "satisfied");
            assert_eq!(v["bounds"]["truncated"], false);
            let cases = v["case_outcomes"].as_array().unwrap();
            assert_eq!(cases.len(), 2);
            for (index, row) in cases.iter().enumerate() {
                let arrival = index as u64;
                let finish = service * (index as u64 + 1);
                assert_eq!(row["arrival"], arrival);
                assert_eq!(row["terminals"][0]["kind"], "end");
                assert_eq!(
                    row["terminals"][0]["completed_at"],
                    json!({"min":finish,"max":finish})
                );
                assert_eq!(
                    row["terminals"][0]["elapsed"],
                    json!({"min":finish-arrival,"max":finish-arrival})
                );
                assert_eq!(
                    row["terminals"][0]["work"],
                    json!({"min":service,"max":service})
                );
            }
        }
    }
}
#[test]
fn held_selected_summary_reuses_same_scenario_without_converting_held_to_success() {
    let s = i::held_summary(None, 20);
    let v = json!(mutation::mutate_set_with_scenario(
        &s,
        "bpr-default",
        Some("Day")
    ));
    held(only(&v, 4), json!(["Invoice"]), 2, 2);
    assert_eq!(only(&v, 4)["report"]["scenario"]["name"], "Day");
    assert_eq!(
        only(&v, 4)["report"]["scenario"]["drain"],
        v["original"]["scenario"]["drain"]
    );
}
#[test]
fn business_horizon_keeps_cutoff_while_clock_is_computational_inconclusive() {
    for (horizon, clock) in [(Some(5), 20), (None, 5)] {
        let s = i::held_summary(horizon, clock);
        for v in both(&s, Some("Day")) {
            assert_eq!(v["bounds"]["truncated"], horizon.is_none());
            assert_eq!(v["case_outcomes"][0]["terminals"][0]["end"], "done");
            if horizon.is_some() {
                assert_eq!(v["case_outcomes"][1]["terminals"][0]["kind"], "cut_off");
            } else {
                assert_eq!(v["verdict"], "inconclusive");
            }
        }
    }
}
#[test]
fn overflow_is_a_business_end_not_an_implicit_completion_or_clock_cut() {
    let s = i::held_summary(None, 20);
    let s = i::replace(
        &s,
        "arrivals fixed [0min,1min]",
        "arrivals fixed [0min,0min]",
    );
    let s = i::replace(&s, "concurrent cases 1 on_overflow wait_external fifo;", "");
    let s = i::add(&s, "concurrent cases 1 on_overflow end spill");
    for v in both(&s, Some("Day")) {
        assert_eq!(v["bounds"]["truncated"], false);
        assert_eq!(v["case_outcomes"][1]["terminals"][0]["kind"], "overflow");
        assert_eq!(v["case_outcomes"][1]["terminals"][0]["end"], "spill");
        assert_eq!(
            v["case_outcomes"][1]["terminals"][0]["completed_at"],
            json!({"min":0,"max":0})
        );
    }
}
#[test]
fn effect_contest_expiry_preserves_restricted_authority_refusal() {
    let s = include_str!("../../../fixtures/sov-basic/model.relca");
    let s = i::replace(
        s,
        "contract Controls {",
        "contract Controls { authority_basis Approver.approve ApprovalGrant;",
    );
    assert_eq!(checked(&s)["verdict"], "satisfied");
    let v = mutated(&s);
    let m = only(&v, 1);
    assert_eq!(m["result"], "invalid_mutant");
    assert_eq!(m["reason"], "static_unsupported");
    assert!(
        m["report"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["message"] == "contest cannot reference restricted Authority ApprovalGrant")
    );
    assert_eq!(
        v["original"]["contest_paths"][0]["effect"],
        "Ledger.post.commit"
    );
}
#[test]
fn lww_and_no_lost_update_reports_are_not_silently_recomputed_with_old_semantics() {
    for (source, verdict) in [
        (
            include_str!("../../../fixtures/cf-lww.broken-overlap/model.relca"),
            "violated",
        ),
        (
            include_str!("../../../fixtures/cf-version/model.relca"),
            "satisfied",
        ),
    ] {
        let s = i::replace(
            source,
            "capability claim() -> R {",
            "capability claim() -> R { reads [Invoice];",
        );
        let s = i::replace(&s, "include [Ledger]", "include [Invoice,Ledger]");
        let s = i::add(&s, "actor Invoice { source document }");
        let original = checked(&s);
        assert_eq!(original["verdict"], verdict);
        let v = mutated(&s);
        assert_eq!(v["original"], original);
        assert_eq!(v["mutations"][4]["status"], "not_applicable");
        for k in [0, 9] {
            let m = only(&v, k);
            assert_eq!(m["report"]["properties"], original["properties"]);
            assert_eq!(m["report"]["case_outcomes"], original["case_outcomes"]);
            assert!(!m["report"]["properties"].as_array().unwrap().is_empty());
        }
    }
}

#[test]
fn t20_start_writer_is_not_missing_from_summary_source_ancestry() {
    let s = i::start_only_writer();
    let ast = syntax::parse(&s).unwrap();
    let context = lower::lower(&ast).expect("public start-only writer must lower");
    let snap = context
        .state_variables
        .iter()
        .position(|v| v.name == "snap")
        .unwrap();
    let writer = context
        .capabilities
        .iter()
        .find(|c| context.actors[c.actor.0].name == "Writer" && c.name == "make")
        .unwrap();
    assert!(writer.start_writes.iter().any(|w| w.variable.0 == snap));
    assert!(writer.finish_writes().all(|w| w.variable.0 != snap));
    assert!(!writer.kernel_writes.iter().any(|v| v.0 == snap));
    assert!(
        writer
            .kernel
            .iter()
            .flat_map(|k| &k.rows)
            .flat_map(|r| &r.branches)
            .flat_map(|b| &b.overrides)
            .all(|(v, _)| v.0 != snap)
    );
    for v in both(&s, None) {
        time(&v, "done", 5);
        time(&v, "rejected", 4);
    }
    for v in both(&i::manual(&s), None) {
        time(&v, "done", 6);
        time(&v, "rejected", 5);
    }
    let v = mutated(&s);
    held(only(&v, 4), json!(["Invoice"]), 3, 4);
    fixture_match::validate_mutation_wire(&v).unwrap();
}
#[test]
fn t20_atomic_writer_is_not_missing_from_summary_source_ancestry() {
    let s = i::observed(true, false, false);
    let s = i::replace(
        &s,
        "type Entry = enum { entry }",
        "type Entry = enum { entry, duplicate }",
    );
    let s = i::replace(
        &s,
        "writes snap; duration fixed 1min; result nondet { entry }",
        "writes snap; duration fixed 1min; atomic { if flag == true { result duplicate } else { snap = entry; result entry } }",
    );
    let s = i::replace(
        &s,
        "observed case { snap:optional<Entry> }",
        "observed case { snap:optional<Entry>; flag: bool = false }",
    );
    for v in both(&s, None) {
        time(&v, "done", 5);
    }
    held(only(&mutated(&s), 4), json!(["Invoice"]), 3, 4);
}
#[test]
fn scenario_local_overflow_terminal_is_a_located_static_refusal() {
    let s = i::replace(
        &i::held_summary(None, 20),
        "on_overflow wait_external fifo",
        "on_overflow end spill",
    );
    let r = json!(report::check_with_selection(&s, None, Some("Day")));
    assert_eq!(r["verdict"], "unsupported");
    let ds: Vec<_> = r["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert_eq!(ds.len(), 1);
    assert_eq!(ds[0]["code"], "U-SEM-0");
    assert_eq!(
        ds[0]["message"],
        "scenario overflow terminal is unsupported"
    );
    assert!(ds[0]["span"].is_object());
    let v = json!(mutation::mutate_set_with_scenario(
        &s,
        "bpr-default",
        Some("Day")
    ));
    for k in [0, 1, 2, 4, 8, 9] {
        assert_eq!(v["mutations"][k]["status"], "blocked");
        assert!(rows(&v, k).is_empty());
    }
}

#[test]
fn closed_c6_names_match_real_inputs_and_unknown_names_never_gain_an_exemption() {
    let names: Vec<String> =
        serde_json::from_str(include_str!("support/mutation_default_names.json")).unwrap();
    assert_eq!(names.len(), 79);
    for name in &names {
        assert!(population::fixture(name));
    }
    assert!(!population::fixture("old-not-a-c6-fixture"));
    assert!(
        std::panic::catch_unwind(|| population::fixture("mut2-unknown-future-fixture")).is_err()
    );
}

#[test]
fn t20_wlin2_of_the_observed_writer_is_carried_but_never_assigned() {
    let s = i::observed_wlin2();
    for r in both(&s, None) {
        time(&r, "done", 5);
    }
    let v = mutated(&s);
    let m = only(&v, 4);
    for r in [&v["original"], &m["report"]] {
        let warnings: Vec<_> = r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-LIN-2")
            .collect();
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0]["message"]
                .as_str()
                .unwrap()
                .contains("Approver.approve")
        );
    }
    assert!(
        v["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("W-LIN-2"))
    );
    assert!(
        m["assigned_changes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["check"] != "W-LIN-2")
    );
    held(m, json!(["Invoice"]), 3, 4);
}

#[test]
fn quality_horizon_hand_edit_preserves_cutoff_and_the_inclusive_deadline() {
    for horizon in [4, 6] {
        let source = i::quality_horizon(horizon);
        for manual in [false, true] {
            let source = if manual {
                i::manual(&source)
            } else {
                source.clone()
            };
            for report in both(&source, Some("Day")) {
                assert_eq!(report["bounds"]["truncated"], false);
                assert_eq!(
                    report["quality"]["cohort"][0]["value"]["lower"],
                    if manual { "0" } else { "1" }
                );
                assert_eq!(
                    report["quality"]["cohort"][0]["value"]["upper"],
                    if manual { "0" } else { "1" }
                );
                let row = &report["case_outcomes"][0]["terminals"][0];
                if manual && horizon == 4 {
                    assert_eq!(report["verdict"], "inconclusive");
                    assert_eq!(row["kind"], "cut_off");
                    assert!(row["end"].is_null());
                } else {
                    assert_eq!(report["verdict"], "satisfied");
                    assert_eq!(row["kind"], "end");
                    assert_eq!(row["end"], "done");
                }
            }
        }
    }
}

#[test]
fn quality_horizon_mutant_is_inconclusive_not_a_static_detection() {
    let source = i::quality_horizon(4);
    let v = json!(mutation::mutate_set_with_scenario(
        &source,
        "bpr-default",
        Some("Day")
    ));
    let m = only(&v, 4);
    assert_eq!(v["original"]["quality"]["cohort"][0]["value"]["lower"], "1");
    assert_eq!(m["report"]["quality"]["cohort"][0]["value"]["lower"], "0");
    assert_eq!(m["report"]["bounds"]["truncated"], false);
    assert_eq!(
        m["report"]["case_outcomes"][0]["terminals"][0]["kind"],
        "cut_off"
    );
    assert_eq!(m["report"]["verdict"], "inconclusive");
    assert_eq!(m["result"], "inconclusive");
    assert_eq!(m["reason"], "assigned_check_unavailable");
    assert_eq!(v["run"]["completion"], "inconclusive");
}

#[test]
fn guard_horizon_hand_edit_has_a_business_cutoff_not_a_computational_cut() {
    for horizon in [3, 5] {
        let source = i::guard_horizon(horizon);
        for r in both(&source, Some("Day")) {
            assert_eq!(r["verdict"], "satisfied");
            assert_eq!(r["bounds"]["truncated"], false);
            let t = &r["case_outcomes"][0]["terminals"][0];
            assert_eq!(t["end"], "rejected");
            assert_eq!(t["completed_at"], json!({"min":2,"max":2}));
            assert_eq!(t["work"], json!({"min":2,"max":2}));
        }
        let manual = i::replace(&source, "if p == pass {} else { end rejected };", "");
        for r in both(&manual, Some("Day")) {
            assert_eq!(r["bounds"]["truncated"], false);
            let t = &r["case_outcomes"][0]["terminals"][0];
            if horizon == 3 {
                assert_eq!(r["verdict"], "inconclusive");
                assert_eq!(t["kind"], "cut_off");
                assert!(t["end"].is_null());
            } else {
                assert_eq!(r["verdict"], "satisfied");
                assert_eq!(t["end"], "done");
                assert_eq!(t["completed_at"], json!({"min":4,"max":4}));
                assert_eq!(t["work"], json!({"min":4,"max":4}));
            }
        }
    }
}

#[test]
fn legacy_guard_horizon_rows_are_inconclusive_in_every_kind_and_valid_wire() {
    for horizon in [5, 3] {
        let v = json!(mutation::mutate_set_with_scenario(
            &i::guard_horizon(horizon),
            "bpr-default",
            Some("Day")
        ));
        // Invoke the full closed validator even on the intentionally RED
        // output; retain the result so classification assertions name the bug.
        let wire = fixture_match::validate_mutation_wire(&v);
        let mut cut_rows = 0;
        for (kind, row) in v["mutations"].as_array().unwrap().iter().enumerate() {
            for m in row["mutants"].as_array().unwrap() {
                let r = &m["report"];
                if r["verdict"] == "inconclusive" && r["bounds"]["truncated"] == false {
                    cut_rows += 1;
                    assert_eq!(m["result"], "inconclusive", "kind {kind}: {}", m["id"]);
                    assert_eq!(m["reason"], "assigned_check_unavailable");
                }
            }
        }
        for k in [2, 8] {
            let m = rows(&v, k)
                .iter()
                .find(|m| m["id"].as_str().unwrap().ends_with("body/2:keep_then"))
                .unwrap();
            assert_eq!(
                m["result"],
                if horizon == 3 {
                    "inconclusive"
                } else {
                    "undetected"
                }
            );
            assert_eq!(
                m["reason"],
                if horizon == 3 {
                    "assigned_check_unavailable"
                } else {
                    "no_assigned_check"
                }
            );
            assert_eq!(m["report"]["bounds"]["truncated"], false);
            let terminal = &m["report"]["case_outcomes"][0]["terminals"][0];
            assert_eq!(
                terminal["kind"],
                if horizon == 3 { "cut_off" } else { "end" }
            );
            assert_eq!(
                terminal["end"],
                if horizon == 3 {
                    Value::Null
                } else {
                    json!("done")
                }
            );
        }
        assert_eq!(cut_rows, if horizon == 3 { 2 } else { 0 });
        assert_eq!(
            v["run"]["completion"],
            if horizon == 3 {
                "inconclusive"
            } else {
                "complete"
            }
        );
        assert_eq!(
            v["run"]["reason"],
            if horizon == 3 {
                json!("mutant_incomplete")
            } else {
                Value::Null
            }
        );
        wire.unwrap();
    }
}
