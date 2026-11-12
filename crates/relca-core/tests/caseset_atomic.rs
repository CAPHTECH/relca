//! Independent ADR0035 acceptance; no dependency on a proposed new IR shape.
#[path = "support/actor_observed_acceptance.rs"]
mod h;
use relca_core::{Verdict, compare, engine, explain, ir, reference, report};
use serde_json::{Value, json};

fn names() -> Vec<String> {
    serde_json::from_str(include_str!("support/caseset_names.json")).unwrap()
}
fn pairs() -> Vec<Value> {
    serde_json::from_str(include_str!("support/caseset_pairs.json")).unwrap()
}
fn check(name: &str) -> Value {
    let source = h::source(name);
    if name.starts_with("aset-simulation") || name.starts_with("aset-atomic-sampling") {
        serde_json::to_value(report::simulate(&source)).unwrap()
    } else if name.starts_with("aset-compare-notes") {
        json!(compare::compare(
            &source,
            &["Left", "Right"],
            compare::Analysis::Exhaustive
        ))
    } else {
        json!(report::check(&source))
    }
}
fn property<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap_or_else(|| panic!("missing property {id}: {v}"))
}
fn once(v: &Value, note: bool, verdict: &str) {
    let p = property(v, "contract C #1 at_most_once");
    assert_eq!(p["verdict"], verdict);
    if note {
        assert_eq!(p["notes"], json!(["mechanism_unmodeled"]));
    } else {
        assert!(
            p.get("notes").is_none(),
            "empty notes key is not omission: {p}"
        );
    }
}
fn accept(base: &str) {
    let selected: Vec<_> = names()
        .into_iter()
        .filter(|n| n == base || n.starts_with(&format!("{base}.broken-")))
        .collect();
    assert_eq!(selected.len(), 2, "closed pair {base}");
    for name in selected {
        let want = h::expected(&name);
        let actual = check(&name);
        h::subset(&want, &actual);
        let source = h::source(&name);
        if ["invalid", "unsupported"].contains(&want["verdict"].as_str().unwrap())
            && !name.starts_with("aset-simulation")
        {
            h::reject(&source, want["diagnostics"][0]["code"].as_str().unwrap());
        } else if !name.starts_with("aset-simulation")
            && !name.starts_with("aset-atomic-sampling")
            && !name.starts_with("aset-compare-notes")
        {
            let model = h::model(&source);
            let e = engine::explore(&model);
            let r = reference::explore(&model);
            assert_eq!(
                r.stop,
                reference::StopReason::Complete,
                "no runtime reference exclusion: {name}"
            );
            assert_eq!(
                json!(r.verdict()),
                want["verdict"],
                "reference hand verdict {name}"
            );
            for out in [&e, &r.outcome] {
                // Only written keys are compared (fixtures/README); a written
                // bounds must still carry truncated.
                if let Some(bounds) = want.get("bounds") {
                    assert_eq!(out.truncated, bounds["truncated"].as_bool().unwrap());
                }
                let projection = h::projection(&model, out);
                for key in ["terminals", "case_outcomes"] {
                    if let Some(expected) = want.get(key) {
                        h::subset(expected, &projection[key]);
                    }
                }
            }
            // Only the reference prepends lowering diagnostics. Assert before slicing.
            assert!(r.outcome.diagnostics.starts_with(&model.diagnostics));
            assert_eq!(
                e.diagnostics,
                r.outcome.diagnostics[model.diagnostics.len()..]
            );
            assert_eq!(e.properties, r.outcome.properties, "{name}");
            assert_eq!(e.terminals, r.outcome.terminals, "{name}");
            assert_eq!(e.case_outcomes, r.outcome.case_outcomes, "{name}");
            assert_eq!(
                e.state_range_violations, r.outcome.state_range_violations,
                "{name}"
            );
            assert_eq!(
                e.capability_refusals, r.outcome.capability_refusals,
                "{name}"
            );
        }
    }
}

macro_rules! acceptance { ($($test:ident => $base:literal),+ $(,)?) => {$(
    #[test] fn $test() {accept($base);}
)+}; }
include!("support/caseset_pair_tests.rs");

#[test]
fn all_legacy_skeletons_are_source_bound_and_accepted_before_new_implementation() {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("support/caseset_legacy_controls.json")).unwrap();
    assert_eq!(rows.len(), 91);
    let mut failures = Vec::new();
    for row in rows {
        let name = row["name"].as_str().unwrap();
        assert_eq!(
            h::source(name),
            row["original"].as_str().unwrap(),
            "stale control {name}"
        );
        let source = row["legacy_control"].as_str().unwrap();
        let result = relca_core::syntax::parse(source).and_then(|ast| {
            if let Some(conf) = row["configuration"].as_str() {
                relca_core::lower::lower_configuration(&ast, conf).map(|_| ())
            } else {
                relca_core::lower::lower(&ast).map(|_| ())
            }
        });
        if let Err(d) = result {
            failures.push(format!("legacy skeleton {name}: {d:?}\n{source}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn closed_population_and_single_edits_are_real_and_comment_free() {
    let mut actual: Vec<_> = std::fs::read_dir(h::root().join("fixtures"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("aset-"))
        .collect();
    actual.sort();
    assert_eq!(actual, names());
    assert_eq!(actual.len(), 182);
    assert_eq!(pairs().len(), 91);
    let mut counts = [0usize; 4];
    for name in &actual {
        let category =
            if name.starts_with("aset-simulation") || name.starts_with("aset-atomic-sampling") {
                2
            } else if name.starts_with("aset-compare-notes") {
                3
            } else if ["invalid", "unsupported"]
                .contains(&h::expected(name)["verdict"].as_str().unwrap())
            {
                1
            } else {
                0
            };
        counts[category] += 1;
    }
    assert_eq!(
        counts,
        // ADR0041: aset-cases.broken-capacity moves static -> runtime (C).
        // aset-static-capacity-with-resource.broken-capacity-with-resource
        // moves static -> runtime (L). Total182 and old hand times unchanged.
        [106, 70, 4, 2],
        "ordinary reference-complete / static / sample / comparison inputs"
    );
    for pair in pairs() {
        let good = pair["good"].as_str().unwrap();
        let changed = h::change(
            &h::source(good),
            pair["from_"].as_str().unwrap(),
            pair["to"].as_str().unwrap(),
        );
        assert_eq!(changed, h::source(pair["bad"].as_str().unwrap()));
    }
    for name in actual {
        let source = h::source(&name);
        assert!(!source.contains("//") && !source.contains('#'));
    }
}

#[test]
fn duplicate_commit_count_and_mechanism_annotation_are_separate() {
    once(&check("aset-dup"), false, "satisfied");
    once(&check("aset-dup.broken-no-check"), true, "violated");
    for name in [
        "aset-cut-rg.broken-wrong-set",
        "aset-cut-resource",
        "aset-cut-resource.broken-two-slots",
    ] {
        once(&check(name), true, "satisfied");
    }
    once(&check("aset-cut-rg"), false, "satisfied");
    once(&check("aset-finish-snapshot"), true, "satisfied");
    once(
        &check("aset-finish-snapshot.broken-false-toggle"),
        true,
        "vacuous",
    );
}

#[test]
fn recognition_is_structural_and_keeps_parameter_identity() {
    let s = h::source("aset-dup");
    for (from, to, notes) in [
        ("if key in keys", "if (key in keys)", false),
        ("if key in keys", "if case.id in keys", true),
        ("if key in keys", "if key in keys && registered==true", true),
        ("if key in keys", "if key in keys || registered==true", true),
        ("key: CaseId", "key: CaseId, other: CaseId", true),
        ("committed", "posted_ok", false),
        ("actor Ledger {", "actor Ledger {grain document;", false),
        ("Ledger", "Book", false),
    ] {
        let mut t = h::change(&s, from, to);
        if from == "key: CaseId" {
            t = h::change(&t, "post(case.id)", "post(case.id,case.id)");
            t = h::change(&t, "keys += key", "keys += other");
        }
        let v = json!(report::check(&t));
        assert_eq!(v["verdict"], "satisfied");
        once(&v, notes, "satisfied");
    }
    let t = h::change(&s, "commits", "commits;start{keys+=key}");
    let t = h::change(&t, "writes [keys, registered]", "writes [registered]");
    let t = h::change(&t, "keys += key; ", "");
    let v = json!(report::check(&t));
    assert_eq!(v["verdict"], "vacuous");
    once(&v, true, "vacuous");
}

#[test]
fn requires_refusal_is_zero_work_but_atomic_rejection_is_not_refusal() {
    let v = check("aset-dup");
    assert!(v.get("capability_refusals").is_none());
    let v = check("aset-requires-in.broken-no-prime");
    let rows = v["capability_refusals"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["at"], json!({"min":1,"max":1}));
    assert_eq!(rows[0]["capability"], "Ledger.work");
    assert_eq!(v["terminals"][0]["work"], json!({"min":1,"max":1}));
}

#[test]
fn one_arrival_legacy_zero_time_and_state_budget_are_not_business_refusals() {
    let s = h::source("aset-dup");
    let arrival = h::change(&s, "cut K", "arrivals fixed[0min];cut K");
    let m = h::model(&arrival);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
    h::terminal(&h::projection(&m, &e), 1, "dup", (2, 2), (2, 2), (2, 2));
    once(&json!(report::check(&arrival)), false, "satisfied");
    let cut = engine::explore_with_budget(&m, 0);
    assert!(cut.truncated);
    assert!(
        cut.case_outcomes
            .as_ref()
            .unwrap()
            .iter()
            .all(|c| c.terminals.is_empty())
    );
    let zero = h::change(&s, "duration fixed 1min", "duration fixed 0min");
    let m = h::model(&zero);
    let out = engine::explore(&m);
    assert_eq!(out.terminals.len(), 1);
    assert_eq!(out.terminals[0].elapsed.min, 0);
    assert_eq!(out.terminals[0].work.max, 0);
    // Zero-duration reference exclusion is explicit, existing and exactly one.
    assert_eq!(
        out.properties
            .iter()
            .filter(|p| matches!(p.property, ir::Property::Contract(_)))
            .count(),
        1
    );
}

#[test]
fn atomic_simulation_and_exhaustive_probability_are_deterministic_without_draws() {
    for name in [
        "aset-atomic-sampling",
        "aset-atomic-sampling.broken-no-check",
        "aset-simulation",
        "aset-simulation.broken-two-people",
    ] {
        let m = h::model(&h::source(name));
        let e = engine::simulate(&m);
        let r = reference::simulate(&m);
        assert_eq!(e.replications, r.outcome.replications);
        let bad = name.ends_with("two-people");
        for o in [&e, &r.outcome] {
            assert_eq!(o.attempted_replications, if bad { 1 } else { 4 });
            assert_eq!(o.completed_replications, if bad { 0 } else { 4 });
            assert!(o.replications.iter().all(|r| r.draws.is_empty()));
            assert_eq!(o.diagnostics.iter().any(|d| d.code == "U-SEM-0"), bad);
        }
    }
    for (name, probability) in [
        ("aset-atomic-probability", "1"),
        ("aset-atomic-probability.broken-no-check", "0"),
    ] {
        let v = check(name);
        assert_eq!(
            property(&v, "probability reached #1")["probability"],
            json!({"min":probability,"max":probability})
        );
    }
    let s = h::source("aset-simulation.broken-two-people");
    let s = h::change(
        &s,
        "if input_by==c1 {end by_c1}else{end by_c2}",
        "end by_c1",
    );
    assert_eq!(report::simulate(&s).verdict, Verdict::Unsupported);
}

#[test]
fn old_syntax_notes_are_consumed_in_both_compare_modes_without_changing_tables() {
    let s = h::source("aset-compare-notes");
    let ast = relca_core::syntax::parse(&s).unwrap();
    for config in ["Left", "Right"] {
        let m = relca_core::lower::lower_configuration(&ast, config)
            .unwrap()
            .model;
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(e.properties, r.outcome.properties);
        assert_eq!(e.terminals, r.outcome.terminals);
        assert_eq!(e.terminals[0].elapsed.min, 1);
        assert_eq!(e.terminals[0].work.max, 1);
        let e = engine::simulate(&m);
        let r = reference::simulate(&m);
        assert_eq!(e.completed_replications, 4);
        assert_eq!(e.replications, r.outcome.replications);
    }
    for mode in [compare::Analysis::Exhaustive, compare::Analysis::Simulation] {
        let v = json!(compare::compare(&s, &["Left", "Right"], mode));
        assert_eq!(
            v["verdict"],
            if matches!(mode, compare::Analysis::Exhaustive) {
                "satisfied"
            } else {
                "inconclusive"
            }
        );
        assert_eq!(v["plans"].as_array().unwrap().len(), 2);
        for p in v["plans"].as_array().unwrap() {
            once(
                &p["report"],
                true,
                if matches!(mode, compare::Analysis::Exhaustive) {
                    "satisfied"
                } else {
                    "inconclusive"
                },
            );
        }
        assert!(!v["tables"].to_string().contains("notes"));
    }
}

#[test]
fn canonical_rejection_trace_has_finish_but_no_commit_and_replays() {
    let s = h::source("aset-dup");
    let s = h::change(
        &s,
        "contract C",
        "contract I{invariant Ledger.post.finish(rejected_duplicate) -> registered==false};contract C",
    );
    let v = json!(explain::explain(&s));
    assert_eq!(v["verdict"], "violated");
    assert_eq!(v["explanation"]["status"], "complete");
    let cs = v["explanation"]["counterexamples"].as_array().unwrap();
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0]["replay"]["status"], "verified");
    let last = cs[0]["steps"].as_array().unwrap().last().unwrap();
    assert_eq!(last["time"], 2);
    assert_eq!(last["kind"], "finish");
    let events = last["events"].as_array().unwrap();
    assert!(!events.iter().any(|e| e.to_string().contains("commit")));
    let values = last["values"]["observed"].as_array().unwrap();
    assert!(
        values
            .iter()
            .any(|v| v["name"] == "Ledger.keys" && v["value"] == json!([1]))
    );
    assert!(
        values
            .iter()
            .any(|v| v["name"] == "registered" && v["value"] == "true")
    );
}

#[test]
fn shared_set_display_is_sorted_one_based_at_the_earliest_counterexample() {
    let s = h::source("aset-replay-display");
    assert!(!s.contains("configuration"));
    let v = json!(explain::explain(&s));
    assert_eq!(v["verdict"], "violated");
    assert_eq!(v["explanation"]["status"], "complete");
    let cs = v["explanation"]["counterexamples"].as_array().unwrap();
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0]["replay"]["status"], "verified");
    let last = cs[0]["steps"].as_array().unwrap().last().unwrap();
    assert_eq!(last["time"], 5);
    assert_eq!(last["case_id"], 1);
    assert!(
        last["values"]["observed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["name"] == "Ledger.keys" && v["value"] == json!([1, 2]))
    );
}

#[test]
fn scalar_person_initialization_and_declared_person_order_are_not_integer_ids() {
    let s = h::source("aset-executor");
    for people in ["[c1,c2]", "[c2,c1]"] {
        let t = if people == "[c1,c2]" {
            s.clone()
        } else {
            h::change(&s, "people [c1,c2]", &format!("people {people}"))
        };
        let t = h::change(&t, "input_by:optional<PersonId>", "input_by:PersonId=c2");
        assert_eq!(report::check(&t).verdict, Verdict::Satisfied);
    }
    h::reject(
        &h::change(&s, "input_by:optional<PersonId>", "input_by:PersonId"),
        "U-SEM-0",
    );
    h::reject(
        &h::change(&s, "input_by==c1", "input_by==missing_person"),
        "E-TYPE-0",
    );
    let t = h::change(&s, "input_by:optional<PersonId>", "input_by:PersonId=c2");
    h::reject(&h::change(&t, "input_by==c1", "input_by==none"), "E-TYPE-0");
}

#[test]
fn contains_and_conditional_results_remain_symbolically_unknown_without_losing_observation() {
    let v = check("aset-dup");
    assert_eq!(v["agency"]["refusal"], json!([]));
    assert_eq!(
        v["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|x| **x == "state_guards")
            .count(),
        1
    );
    let good = check("aset-agency");
    let control = check("aset-agency.broken-scalar-requires");
    for actor in ["Ledger", "Poster"] {
        let origin = good["agency"]["origin"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["actor"] == actor)
            .unwrap();
        assert_eq!(origin["direct"], json!([]));
        assert_eq!(
            origin["indirect"],
            json!([]),
            "case.id is not a BindingId(0) evidence edge"
        );
    }
    assert!(
        good["agency"]["refusal"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["effect"] != "Poster.post")
    );
    let judgement = control["agency"]["refusal"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["effect"] == "Poster.post")
        .unwrap();
    assert!(
        judgement["judgements"]
            .as_array()
            .unwrap()
            .iter()
            .any(|j| j["stops"] == "all_paths")
    );
    for name in ["aset-dup", "aset-conditional-agency"] {
        let v = check(name);
        let vp = &v["agency"]["viewpoints"];
        // ADR 0043 (plan C01)
        assert_eq!(vp["rules"], "agency-viewpoints-p1-2");
        assert_eq!(vp["observation"]["status"], "derived");
        assert_eq!(
            vp["proposal"],
            json!({"status":"not_derived","reason":"no_proposal_contract"})
        );
        for end in vp["framing"]["ends"].as_array().unwrap() {
            assert_eq!(end["status"], "not_derived");
            assert_eq!(end["reason"], "state_guards");
        }
        let retention = vp["retention"]["capabilities"].as_array().unwrap();
        let post = retention
            .iter()
            .find(|c| c["capability"] == "Ledger.post")
            .unwrap();
        assert_eq!(post["status"], "derived");
        assert_eq!(
            post["normal"],
            if name == "aset-dup" {
                json!({"min":2,"max":2})
            } else {
                json!({"min":1,"max":1})
            }
        );
        if name == "aset-conditional-agency" {
            let next = retention
                .iter()
                .find(|c| c["capability"] == "Next.go")
                .unwrap();
            assert_eq!(next["status"], "not_derived");
            assert_eq!(next["reason"], "state_guards");
        }
    }
    let v = check("aset-conditional-agency.broken-nondet");
    let vp = &v["agency"]["viewpoints"];
    assert!(
        vp["framing"]["ends"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["status"] == "derived")
    );
    let next = vp["retention"]["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["capability"] == "Next.go")
        .unwrap();
    assert_eq!(next["normal"], json!({"min":0,"max":1}));
}

#[test]
fn public_ir_storage_bounds_owners_and_initial_values_fail_closed_without_panic() {
    let good = h::model(&h::source("aset-cases"));
    for fault in 0..8 {
        let mut m = good.clone();
        let set = m
            .state_variables
            .iter()
            .position(|v| v.name == "keys")
            .unwrap();
        match fault {
            0 => m.bounds.cases = 65,
            1 => m.bounds.cases = 1,
            2 => {
                m.state_variables[set].initial = ir::InitialValue::Fixed(ir::StateValue::Integer(1))
            }
            3 => m.state_variables[set].optional = true,
            4 => m.state_variables[set].kind = ir::StateKind::ActorLatent(ir::ActorId(0)),
            5 => m.actors[0].metadata.instance = Some(ir::ActorInstance::PerCase),
            6 => {
                m.state_variables[set].kind = ir::StateKind::ActorObserved(ir::ActorId(usize::MAX))
            }
            7 => {
                // ADR0041 now resolves InsertCase through C. Keep the eighth
                // malformed-IR obligation as an unresolved Assign writer;
                // only capacity is forged in this otherwise-valid control.
                m = h::model(&h::change(
                    &h::source("aobs-capacity"),
                    "posted += 1",
                    "posted = 1",
                ));
                m.actors[0].capacity = Some(2);
            }
            _ => unreachable!(),
        }
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(
            r.stop,
            reference::StopReason::SemanticAbort,
            "fault {fault}"
        );
        for out in [&e, &r.outcome] {
            assert!(
                out.diagnostics.iter().any(|d| d.code == "U-SEM-0"),
                "fault {fault}: {out:?}"
            );
            assert!(out.terminals.is_empty());
        }
    }
}

#[test]
fn rejection_finish_does_not_fulfil_a_commit_response_or_requires_event() {
    let base = h::source("aset-dup");
    let response = "contract R{response Ledger.post.finish(rejected_duplicate) -> one_of(Ledger.post.commit) by 0min after Ledger.post.finish(rejected_duplicate) otherwise unresolved};";
    let source = h::change(&base, "contract C", &format!("{response}contract C"));
    let v = json!(report::check(&source));
    assert_eq!(v["verdict"], "violated");
    let r = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| {
            let id = p["id"].as_str().unwrap_or("");
            id.starts_with("contract R #") && id.ends_with(" response")
        })
        .collect::<Vec<_>>();
    assert_eq!(r.len(), 1, "{v}");
    assert_eq!(r[0]["verdict"], "violated");
    let response = "contract R{response Ledger.post.finish(rejected_duplicate) -> one_of(Ledger.post.finish(rejected_duplicate)) by 0min after Ledger.post.finish(rejected_duplicate) otherwise unresolved};";
    let source = h::change(&base, "contract C", &format!("{response}contract C"));
    assert_eq!(report::check(&source).verdict, Verdict::Satisfied);
    let reject = h::change(&base, "if key in keys", "if registered==false");
    let reject = h::change(
        &reject,
        "actor Ledger",
        "actor Probe{capacity 1;capability go()->Ack{duration fixed 1min;result nondet{committed}}};actor Ledger",
    );
    let reject = h::change(
        &reject,
        "a = Ledger.post(case.id); b = Ledger.post(case.id); if b == rejected_duplicate { end dup } else { end twice }",
        "a=Ledger.post(case.id);q=Probe.go();end done",
    );
    let reject = h::change(&reject, "include [Ledger]", "include [Ledger,Probe]");
    let reject = h::change(
        &reject,
        "verify reachable end.dup",
        "contract R{requires_event Probe.go.start after Ledger.post.commit};verify reachable end.done",
    );
    let v = json!(report::check(&reject));
    assert_eq!(v["verdict"], "violated");
    assert_eq!(
        contract_property(&v, "R", "requires_event")["verdict"],
        "violated"
    );
    let control = h::change(
        &reject,
        "after Ledger.post.commit",
        "after Ledger.post.finish",
    );
    let v = json!(report::check(&control));
    assert_eq!(
        contract_property(&v, "R", "requires_event")["verdict"],
        "satisfied"
    );
    for s in [&reject, &control] {
        let m = h::model(s);
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(e.properties, r.outcome.properties);
    }
}

#[test]
fn original_migrated_inputs_are_preserved_and_gain_only_the_new_hand_meaning() {
    // DSL controls, not old executable output. No calls means zero elapsed/work;
    // passing case.id to a T parameter is a static type error (ADR 0035).
    for (job, source) in [
        (
            "unused-caseid",
            r#"model Classification version "0.2" {
          type T = enum { ok, no }
          actor A { capacity 1; capability unused(k: CaseId) -> T { duration fixed 1min; result nondet { ok } } }
          process P { end done }
          bounds { cases 1; live_tasks 1; clock 10min }
          verify reachable end.done
        }"#,
        ),
        (
            "unused-set",
            r#"model M version "0.2" {
          type Mode = enum { ok }
          actor A { state keys: set<CaseId> }
          process P { end done }
          cut C { include [A] }
          bounds { cases 1; live_tasks 2; clock 20min }
          verify reachable end.done
        }"#,
        ),
    ] {
        let v = json!(report::check(source));
        assert_eq!(v["verdict"], "satisfied", "{job}");
        assert_eq!(
            v["terminals"],
            json!([{"end":"done","elapsed":{"min":0,"max":0},"work":{"min":0,"max":0},"result_at":null}])
        );
        assert_eq!(v["bounds"]["truncated"], false);
        let m = h::model(source);
        let r = reference::explore(&m);
        let e = engine::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(e.terminals, r.outcome.terminals);
    }
    let source = r#"model Classification version "0.2" {
      type T = enum { ok, no }
      actor A { capacity 1; capability make() -> T { duration fixed 1min; result nondet { ok } }; capability use(k: T) -> T { duration fixed 1min; result nondet { ok } } }
      process P { a = A.make(); b = A.use(case.id); end done }
      bounds { cases 1; live_tasks 1; clock 10min }
      verify reachable end.done
    }"#;
    h::reject(source, "E-TYPE-0");
}

#[test]
fn membership_scope_and_responsibility_keep_cut_support_even_for_empty_false_sets() {
    let source = h::source("aobs-scope");
    let source = h::change(
        &source,
        "state allow: bool = true",
        "state keys:set<CaseId>",
    );
    let source = h::change(&source, "Doc.allow == true", "case.id in Doc.keys");
    let v = json!(report::check(&source));
    assert_eq!(
        v["verdict"], "violated",
        "empty set makes authority scope false, not a parse error"
    );
    h::reject(
        &h::change(
            &source,
            "include [Doc,A]",
            "include [A];exclude Doc reason \"outside\" loses \"keys\"",
        ),
        "E-CUT-1",
    );
    let source = h::source("aobs-responsibility");
    let source = h::change(&source, "state flag: bool = true", "state keys:set<CaseId>");
    let source = h::change(&source, "Doc.flag == true", "case.id in Doc.keys");
    let v = json!(report::check(&source));
    assert_eq!(v["verdict"], "satisfied");
    let row = &v["responsibilities"][0];
    assert_eq!(row["observation"]["required_sources"], json!(["Doc"]));
    assert_eq!(row["observation"]["direct"], json!(["Doc"]));
    assert_eq!(row["observation"]["status"], "present");
    let source = h::change(&source, "reads [Doc];", "");
    h::reject(
        &h::change(
            &source,
            "include [Doc,Reader]",
            "include [Reader];exclude Doc reason \"outside\" loses \"keys\"",
        ),
        "E-CUT-1",
    );
}

#[test]
fn new_abstract_atomic_writes_degrade_only_refusal_not_required_observation() {
    let source = h::source("aset-dup");
    let source = h::change(
        &source,
        "capacity 1; state keys",
        "executors[c1];source record; state keys",
    );
    let source = h::change(&source, "commits", "commits;reads[Ledger]");
    let source = h::change(
        &source,
        "verify reachable end.dup",
        "responsible c1 for (case.id in Ledger.keys);verify reachable end.dup",
    );
    let v = json!(report::check(&source));
    assert_eq!(v["verdict"], "satisfied");
    let row = &v["responsibilities"][0];
    assert_eq!(row["observation"]["required_sources"], json!(["Ledger"]));
    assert_eq!(row["observation"]["status"], "present");
    assert_eq!(row["refusal"]["status"], "not_derived");
    assert_eq!(row["refusal"]["reason"], "state_guard");
}

#[test]
fn same_tick_contains_or_shared_scalar_guard_has_both_finish_before_branch_orders() {
    let s = h::source("aobs-read-write-order");
    let s = h::change(
        &s,
        "actor Desk",
        "actor Ledger{state keys:set<CaseId>};actor Desk",
    );
    let s = h::change(
        &s,
        "if Desk.ready == true",
        "if case.id in Ledger.keys || Desk.ready == true",
    );
    let s = h::change(&s, "include [Desk,Alt]", "include [Ledger,Desk,Alt]");
    let m = h::model(&s);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
    let v = h::projection(&m, &e);
    h::terminal(&v, 1, "done", (2, 2), (2, 2), (2, 2));
    h::terminal(&v, 2, "done", (3, 4), (1, 2), (1, 2));
    // This detects closing every shared guard early, NOT omission of just the
    // Contains arm: Desk.ready would rescue that omission in this OR fixture.
}

#[test]
fn person_trace_records_the_selected_task_executor_not_the_first_declared_person() {
    let source = h::source("aset-executor");
    let source = h::change(
        &source,
        "verify reachable end.by_c1",
        "contract I{invariant Clerk.enter.finish -> input_by==none};verify reachable end.by_c1",
    );
    for reversed in [false, true] {
        let source = if reversed {
            h::change(&source, "people [c1,c2]", "people [c2,c1]")
        } else {
            source.clone()
        };
        let v = json!(explain::explain(&source));
        assert_eq!(v["verdict"], "violated");
        assert_eq!(v["explanation"]["status"], "complete");
        let path = &v["explanation"]["counterexamples"][0];
        assert_eq!(path["replay"]["status"], "verified");
        let stages = path["steps"].as_array().unwrap();
        assert!(
            stages[0]["values"]["observed"]
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x["name"] == "input_by" && x["value"] == "none")
        );
        let finish = stages.last().unwrap();
        assert_eq!(finish["time"], 3);
        let who = finish["values"]["observed"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["name"] == "input_by")
            .unwrap();
        assert_eq!(who["value"], finish["executor"]);
        assert!([json!("c1"), json!("c2")].contains(&who["value"]));
        let nonoptional = h::change(
            &source,
            "input_by:optional<PersonId>",
            "input_by:PersonId=c2",
        );
        let nonoptional = h::change(&nonoptional, "input_by==none", "input_by==c2");
        let v = json!(explain::explain(&nonoptional));
        assert_eq!(v["verdict"], "violated");
        let initial = &v["explanation"]["counterexamples"][0]["steps"][0];
        assert!(
            initial["values"]["observed"]
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x["name"] == "input_by" && x["value"] == "c2")
        );
    }
}

#[test]
fn sixty_four_end_only_cases_have_zero_work_and_no_phantom_seed_enumeration() {
    let m = h::model(&h::source("aset-limit"));
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
    let v = h::projection(&m, &e);
    assert_eq!(v["case_outcomes"].as_array().unwrap().len(), 64);
    for case in 1..=64 {
        h::terminal(&v, case, "done", (case - 1, case - 1), (0, 0), (0, 0));
    }
    assert!(!e.truncated);
}

#[test]
fn atomic_start_and_requires_coexist_without_confusing_prestart_refusal_with_finish() {
    let source = h::source("aset-dup");
    let source = h::change(&source, "commits", "commits;start{registered=true}");
    assert_eq!(report::check(&source).verdict, Verdict::Satisfied);
    let source = h::change(
        &source,
        "commits",
        "commits;requires registered==false else rejected_duplicate",
    );
    let v = json!(report::check(&source));
    assert_eq!(v["verdict"], "satisfied");
    once(&v, false, "satisfied");
    assert_eq!(v["terminals"][0]["elapsed"], json!({"min":1,"max":1}));
    assert_eq!(v["terminals"][0]["work"], json!({"min":1,"max":1}));
    assert_eq!(v["capability_refusals"].as_array().unwrap().len(), 1);
}

#[test]
fn optional_person_without_people_is_none_and_actor_person_state_is_owned() {
    let source = "model M version \"0.2\" {observed case{who:optional<PersonId>};actor A{};process P{if who==none{end unset}else{end set}};cut C{include[A]};bounds{cases 1;live_tasks 1;clock 2min};verify reachable end.unset}";
    assert_eq!(report::check(source).verdict, Verdict::Satisfied);
    let source = h::source("aset-executor");
    let source = h::change(&source, "observed case { input_by:optional<PersonId> }", "");
    let source = h::change(
        &source,
        "actor Clerk {executors",
        "actor Clerk {state input_by:optional<PersonId>;executors",
    );
    let source = h::change(&source, "if input_by==c1", "if Clerk.input_by==c1");
    assert_eq!(report::check(&source).verdict, Verdict::Satisfied);
    let m = h::model(&source);
    let r = reference::explore(&m);
    let e = engine::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e.terminals, r.outcome.terminals);
}

#[test]
fn executor_write_is_unknown_to_later_requires_not_a_symbolic_c1_constant() {
    let source = h::source("aset-executor");
    let source = h::change(
        &source,
        "actor Clerk",
        "actor Gate{capacity 1;capability go()->Ack{commits;requires input_by==c1 else rejected_duplicate;duration fixed 1min;result nondet{committed}}};actor Clerk",
    );
    let source = h::change(
        &source,
        "a=Clerk.enter();if",
        "a=Clerk.enter();b=Gate.go();if",
    );
    let source = h::change(&source, "include [Clerk]", "include [Clerk,Gate]");
    let v = json!(report::check(&source));
    assert_eq!(v["verdict"], "satisfied");
    assert!(
        v["agency"]["refusal"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["effect"] != "Gate.go")
    );
    assert!(
        v["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("state_guards"))
    );
    let rows = v["terminals"].as_array().unwrap();
    assert_eq!(
        rows.iter().find(|r| r["end"] == "by_c1").unwrap()["elapsed"],
        json!({"min":4,"max":4})
    );
    assert_eq!(
        rows.iter().find(|r| r["end"] == "by_c2").unwrap()["elapsed"],
        json!({"min":3,"max":3})
    );
}

#[test]
fn atomic_business_rejection_consumes_task_work_and_is_not_a_metric_refusal() {
    let s = h::source("aset-dup");
    let s = h::change(
        &s,
        "verify reachable end.dup",
        "metrics{window 20min;currency \"JPY\"};verify reachable end.dup",
    );
    let v = json!(report::check(&s));
    assert_eq!(v["verdict"], "satisfied");
    let tasks = v["metrics"]["cases"][0]["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 2);
    for row in tasks {
        assert_eq!(row["starts"], json!({"min":"1","max":"1","reason":null}));
        assert_eq!(row["work"], json!({"min":"1","max":"1","reason":null}));
        assert!(row.get("refused_count").is_none());
    }
    assert!(v.get("capability_refusals").is_none());
}

#[test]
fn authority_checks_rejected_normal_finish_even_without_a_commit_event() {
    let s = h::source("aset-dup");
    let s = h::change(&s, "if key in keys", "if registered==false");
    let s = h::change(&s, "capacity 1; state keys", "executors[c1]; state keys");
    let s = h::change(
        &s,
        "contract C",
        "authority G{basis \"atomic rejection\";grants c1 to Ledger.post;scope registered==true};contract I{authority_basis Ledger.post G};contract C",
    );
    let v = json!(report::check(&s));
    assert_eq!(v["verdict"], "violated");
    assert_eq!(
        property(&v, "contract I #1 authority_basis")["verdict"],
        "violated"
    );
    let m = h::model(&s);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e.properties, r.outcome.properties);
}

#[test]
fn finish_snapshot_is_read_independently_on_legacy_arrivals_engine_and_reference_routes() {
    for name in [
        "aset-finish-snapshot",
        "aset-finish-snapshot.broken-false-toggle",
    ] {
        let source = h::source(name);
        let broken = name.contains("broken");
        for arrivals in [false, true] {
            let s = if arrivals {
                h::change(&source, "cut K", "arrivals fixed[0min];cut K")
            } else {
                source.clone()
            };
            let m = h::model(&s);
            let e = engine::explore(&m);
            let r = reference::explore(&m);
            assert_eq!(r.stop, reference::StopReason::Complete);
            assert_eq!(e.properties, r.outcome.properties);
            assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
            for out in [&e, &r.outcome] {
                let v = h::projection(&m, out);
                if arrivals {
                    h::terminal(
                        &v,
                        1,
                        if broken { "rejected" } else { "committed_end" },
                        (3, 3),
                        (3, 3),
                        (4, 4),
                    );
                } else {
                    assert_eq!(out.terminals.len(), 1);
                    assert_eq!(out.terminals[0].elapsed.min, 3);
                    assert_eq!(out.terminals[0].work.max, 4);
                    assert_eq!(
                        m.ends[out.terminals[0].end.0],
                        if broken { "rejected" } else { "committed_end" }
                    );
                }
            }
        }
    }
}

#[test]
fn atomic_in_and_set_remain_contextual_names_in_old_positions() {
    let source = "model Contextual version \"0.2\" {type T=enum{atomic,in,set};actor atomic{capacity 1;capability in()->T{duration fixed 1min;result nondet{set}}};process set{in=atomic.in();if in==set{end atomic}else{end other}};cut C{include[atomic]};bounds{cases 1;live_tasks 1;clock 3min};verify reachable end.atomic}";
    let m = h::model(source);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e.terminals, r.outcome.terminals);
    assert_eq!(report::check(source).verdict, Verdict::Satisfied);
    assert_eq!(e.terminals[0].elapsed.min, 1);
}

#[test]
fn unused_atomic_capability_cannot_hide_a_latent_business_guard() {
    let source = h::source("aset-policy.broken-latent");
    let marker = "process Flow {";
    let begin = source.find(marker).unwrap();
    let mut depth = 1;
    let mut end = None;
    for (i, byte) in source
        .as_bytes()
        .iter()
        .enumerate()
        .skip(begin + marker.len())
    {
        if *byte == b'{' {
            depth += 1;
        }
        if *byte == b'}' {
            depth -= 1;
        }
        if depth == 0 {
            end = Some(i + 1);
            break;
        }
    }
    let changed = h::change(
        &source,
        &source[begin..end.unwrap()],
        "process Flow {end done}",
    );
    let changed = h::change(
        &changed,
        "verify reachable end.posted",
        "verify reachable end.done",
    );
    h::reject(&changed, "E-POL-LATENT");
}

// The ordinal after `#` follows the existing whole-model contract numbering;
// these tests name the contract and the property kind instead.
fn contract_property<'a>(v: &'a Value, contract: &str, kind: &str) -> &'a Value {
    let prefix = format!("contract {contract} #");
    let suffix = format!(" {kind}");
    let found = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| {
            let id = p["id"].as_str().unwrap_or("");
            id.starts_with(&prefix) && id.ends_with(&suffix)
        })
        .collect::<Vec<_>>();
    assert_eq!(found.len(), 1, "{contract} {kind}: {v}");
    found[0]
}
