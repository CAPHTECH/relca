//! ADR0036 independent acceptance, intentionally RED until policy is implemented.
#[path = "support/policy_acceptance.rs"]
mod h;
use relca_core::{engine, explain, ir, reference, syntax};
use serde_json::json;

#[test]
fn closed74_names37_exact_single_edits_without_language_comments() {
    let mut actual: Vec<_> = std::fs::read_dir(h::root().join("fixtures"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("pol-"))
        .collect();
    actual.sort();
    assert_eq!(actual, h::names());
    assert_eq!(actual.len(), 74);
    assert_eq!(h::pairs().len(), 37);
    assert_eq!(
        h::pairs()
            .iter()
            .filter(|p| p["static_bad"] == true)
            .count(),
        11
    );
    assert_eq!(
        h::pairs()
            .iter()
            .filter(|p| p["invocation"]["command"] == "simulate")
            .count()
            * 2,
        12
    );
    for p in h::pairs() {
        let s = h::source(p["good"].as_str().unwrap());
        let t = h::edit(&s, p["from"].as_str().unwrap(), p["to"].as_str().unwrap());
        assert_eq!(t, h::source(p["bad"].as_str().unwrap()));
        assert!(!s.contains("//") && !t.contains("//") && !s.contains('#') && !t.contains('#'));
    }
}
#[test]
#[should_panic(expected = "unclassified policy fixture")]
fn unknown_policy_name_is_not_silently_skipped() {
    h::fixture("pol-unknown-fault");
}

#[test]
fn old_skeleton_preflight_excludes_unimplemented_policy_not_other_errors() {
    for p in h::pairs() {
        for n in [p["good"].as_str().unwrap(), p["bad"].as_str().unwrap()] {
            let source_name = if p["static_bad"] == true && n == p["bad"] {
                p["good"].as_str().unwrap()
            } else {
                n
            };
            let s = h::old_skeleton(&h::source(source_name));
            h::selected(&s, p["invocation"]["configuration"].as_str());
        }
    }
    h::selected(&h::old_skeleton(&h::permission_authority_source()), None);
}
fn accept(base: &str) {
    let p = h::pairs().into_iter().find(|p| p["good"] == base).unwrap();
    for n in [p["good"].as_str().unwrap(), p["bad"].as_str().unwrap()] {
        let s = h::source(n);
        let selection = p["invocation"]["configuration"].as_str();
        let actual = if p["invocation"]["command"] == "simulate" {
            h::sampled(&s, selection)
        } else {
            h::checked(&s, selection)
        };
        h::subset(&h::expected(n), &actual);
        if p["static_bad"] == true && n == p["bad"] {
            continue;
        }
        h::both(&s, selection);
    }
}
macro_rules! accepts { ($($test:ident => $name:literal),+ $(,)?) => { $(#[test] fn $test() { accept($name); })+ }; }
accepts! {
 p01_assign => "pol-assign", p02_separation => "pol-separation",
 p03_tie_actor => "pol-tie-actor", p04_tie_person => "pol-tie-person",
 p05_resource_key_and_clock => "pol-resource", p06_control_empty_policy => "pol-control",
 p07_label_cofinish => "pol-label-cofinish", p08_label_dtmc => "pol-label-dtmc",
 p09_label_env => "pol-label-env", p10_latent => "pol-latent", p11_refuse => "pol-refuse",
 p12_names => "pol-names", p13_round_robin => "pol-static-round-robin",
 p13_random => "pol-static-random", p13_skill => "pol-static-skill",
 p13_priority => "pol-static-priority", p13_optimize => "pol-static-optimize",
 p13_synthesize => "pol-static-synthesize", p13_duplicate_assign => "pol-static-duplicate-assign",
 p13_duplicate_then => "pol-static-duplicate-then", p13_duplicate_policy => "pol-static-duplicate-policy",
 p13_block => "pol-static-block", p14_observable_assign => "pol-observable-assign",
 p15_observable_fifo => "pol-observable-fifo", p16_resource_hol => "pol-resource-hol",
 p17_no_person_reservation => "pol-no-person-reservation", p18_cross_class_person => "pol-cross-class-person",
 p18r_cross_class_resource => "pol-cross-class-resource", p19_tie_nondet => "pol-tie-nondet",
 p20_permission => "pol-permission", p21_late_enqueue => "pol-late-enqueue",
 p22_no_effect => "pol-no-effect", p23_deadlock_not_cut => "pol-deadlock",
 p24_configuration => "pol-configuration", label_cut => "pol-label-cut",
 single_winner => "pol-single-winner", tie_ack_environment => "pol-tie-ack"
}

#[test]
fn probability_scheduler_labels_are_separate_from_exact_marginals_and_completion() {
    for (base, positive, negative, min, max) in [
        (
            "pol-label-cofinish",
            Some("fixed-policy"),
            Some("full-information"),
            "1/10",
            "1/10",
        ),
        ("pol-label-dtmc", None, Some("fixed-policy"), "1/10", "1/10"),
        (
            "pol-label-env",
            Some("fixed-policy"),
            Some("full-information"),
            "0",
            "1/10",
        ),
        ("pol-latent", None, None, "1/2", "1/2"),
        (
            "pol-tie-nondet",
            Some("fixed-policy"),
            Some("full-information"),
            "1/2",
            "1/2",
        ),
        (
            "pol-single-winner",
            Some("fixed-policy"),
            Some("fixed-policy"),
            "1/2",
            "1/2",
        ),
        (
            "pol-tie-ack",
            Some("fixed-policy"),
            Some("fixed-policy"),
            "1/2",
            "1/2",
        ),
    ] {
        for (n, label) in [
            (base.to_string(), positive),
            (format!("{base}.broken-change"), negative),
        ] {
            if n == "pol-latent.broken-change" {
                continue;
            }
            let s = h::source(&n);
            let v = h::checked(&s, None);
            h::probability(&v, min, max);
            h::label(&v, label);
            h::both(&s, None);
        }
    }
    for (n, label) in [
        ("pol-label-cut", "fixed-policy"),
        ("pol-label-cut.broken-change", "full-information"),
    ] {
        h::label(&h::checked(&h::source(n), None), Some(label));
    }
}

#[test]
fn assumptions_have_closed_shape_exact_text_and_declared_order() {
    let v = h::checked(&h::source("pol-label-cofinish"), None);
    assert_eq!(
        h::row(&v, "policy.queue").unwrap(),
        &json!({"field":"policy.queue","value":"fifo tie_by case_id then call_order","affects":h::QUEUE_AFFECTS})
    );
    assert_eq!(
        h::row(&v, "policy.assign").unwrap(),
        &json!({"field":"policy.assign","value":"first_idle(executors order)","affects":h::ASSIGN_AFFECTS})
    );
    assert_eq!(
        h::row(&v, "policy.classes").unwrap(),
        &json!({"field":"policy.classes","value":"policy[Clerk]; environment[]; party[]","affects":h::CLASSES_AFFECTS})
    );
    let fields: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["field"].as_str().unwrap())
        .collect();
    let i = fields.iter().position(|s| *s == "policy.queue").unwrap();
    assert_eq!(
        &fields[i..i + 3],
        &["policy.queue", "policy.assign", "policy.classes"]
    );
    assert_eq!(
        h::row(&v, "schedule").unwrap()["affects"],
        "宣言した固定方策で解けない開始・割当て、区分をまたぐ競合、終了・採用・開始の同時刻の順は非決定のまま全て展開する。即時遷移が尽きるまで時刻を進めない。"
    );
    for n in ["pol-observable-fifo", "pol-observable-fifo.broken-change"] {
        let v = h::checked(&h::source(n), None);
        assert!(h::row(&v, "policy.assign").is_none() && h::row(&v, "policy.classes").is_none());
        assert_eq!(
            h::row(&v, "policy.queue").unwrap()["affects"],
            h::FIFO_AFFECTS
        );
        assert_eq!(
            h::row(&v, "schedule").unwrap()["affects"],
            "competing starts unresolved by the declared FIFO rule remain nondeterministic"
        );
        assert_eq!(
            h::row(&v, "policy.queue").unwrap()["value"],
            if n.ends_with("broken-change") {
                "fifo tie_by case_id"
            } else {
                "observable fifo tie_by case_id"
            }
        );
    }
    let v = h::checked(&h::source("pol-assign"), None);
    assert_eq!(
        h::row(&v, "policy.queue").unwrap()["affects"],
        h::FIFO_AFFECTS
    );
    let v = h::checked(&h::source("pol-tie-actor"), None);
    assert!(h::row(&v, "policy.assign").is_none());
    let v = h::checked(&h::source("pol-no-effect"), None);
    assert_eq!(
        h::row(&v, "policy.assign").unwrap()["affects"],
        format!(
            "{} 方策の区分にexecutorsを持つActorが無いため、この割当規則の適用先は無い。",
            h::ASSIGN_AFFECTS
        )
    );
    let v = h::checked(&h::source("pol-control.broken-change"), None);
    assert_eq!(
        h::row(&v, "policy.classes").unwrap()["value"],
        "policy[]; environment[Clerk]; party[]"
    );
    assert_eq!(
        h::row(&v, "policy.classes").unwrap()["affects"],
        format!(
            "{} policy区分のActorが無いため、延長の適用先は無い。",
            h::CLASSES_AFFECTS
        )
    );
}

#[test]
fn metadata_and_agency_never_become_execution_inputs() {
    for n in ["pol-separation", "pol-cross-class-person", "pol-permission"] {
        let s = h::source(n);
        let m = h::selected(&s, None);
        let mut erased = m.clone();
        for a in &mut erased.actors {
            a.metadata = Default::default();
        }
        assert_eq!(engine::explore(&m), engine::explore(&erased));
        assert_eq!(
            reference::explore(&m).outcome,
            reference::explore(&erased).outcome
        );
        let old = h::checked(&h::old_skeleton(&s), None);
        let new = h::checked(&s, None);
        assert_eq!(
            old["agency"], new["agency"],
            "static Agency independent of policy {n}"
        );
        assert_eq!(old["actors"], new["actors"], "Actor table stays static {n}");
    }
}

#[test]
fn resource_utilization_refusal_and_range_stop_have_hand_oracles() {
    for (n, finish, s_time) in [
        ("pol-resource-hol", 6, "5"),
        ("pol-resource-hol.broken-change", 3, "3"),
    ] {
        let v = h::checked(&h::source(n), None);
        assert_eq!(
            v["case_outcomes"][0]["terminals"][0]["completed_at"],
            json!({"min":finish,"max":finish})
        );
        let resources = v["metrics"]["resources"].as_array().unwrap();
        for (name, time, util) in [
            ("R", "3", "3/10"),
            ("S", s_time, if s_time == "5" { "1/2" } else { "3/10" }),
        ] {
            let r = resources.iter().find(|r| r["resource"] == name).unwrap();
            assert_eq!(r["occupancy_time"]["min"], time);
            assert_eq!(r["occupancy_time"]["max"], time);
            assert_eq!(r["utilization"]["min"], util);
            assert_eq!(r["utilization"]["max"], util);
        }
    }
    let v = h::checked(&h::source("pol-refuse"), None);
    let rs = v["capability_refusals"].as_array().unwrap();
    assert_eq!(rs.len(), 1);
    assert_eq!(rs[0]["case_id"], 2);
    assert_eq!(rs[0]["at"], json!({"min":1,"max":1}));
    let v = h::checked(&h::source("pol-refuse.broken-change"), None);
    let r = &v["state_range_violations"][0];
    assert_eq!(r["attempted"], "2");
    assert_eq!(r["upper"], "1");
    assert_eq!(r["time"], 2);
    assert_eq!(r["case_id"], 2);
}

#[test]
fn simulation_preserves_runtime_refusal_and_environment_choices() {
    for (n, time, cases, sites) in [
        ("pol-assign.broken-change", 0, "[1]", "[P.a]"),
        ("pol-control.broken-change", 0, "[1]", "[P.a]"),
        ("pol-label-cofinish", 3, "[1,2]", "[P.a]"),
        ("pol-label-cofinish.broken-change", 0, "[1]", "[P.a]"),
        ("pol-cross-class-person", 0, "[1]", "[P.a,P.b]"),
    ] {
        let mut s = h::source(n);
        if !s.contains("simulation replications") {
            s = h::edit(
                &s,
                "  bounds",
                "  simulation replications 4 seed 0 confidence 0.5\n  bounds",
            );
        }
        let m = h::selected(&s, None); // all-environment must lower, not static reject
        let e = engine::simulate(&m);
        let r = reference::simulate(&m);
        assert_eq!(e, r.outcome, "independent simulation {n}");
        let v = h::sampled(&s, None);
        assert_eq!(v["simulation"]["status"], "unsupported");
        assert_eq!(v["simulation"]["attempted_replications"], 1);
        assert_eq!(v["simulation"]["completed_replications"], 0);
        assert!(v["simulation"]["truncated_replications"].is_null());
        assert!(v["bounds"]["truncated"].is_null());
        assert!(v["admission"].is_null());
        for p in v["properties"].as_array().unwrap() {
            assert_eq!(p["verdict"], "unsupported");
        }
        for c in v["case_outcomes"].as_array().unwrap() {
            assert_eq!(c["terminals"], json!([]));
            assert!(c["unfinished_count"].is_null() && c["deadlocked_count"].is_null());
        }
        let message = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["code"] == "U-SEM-0")
            .unwrap()["message"]
            .as_str()
            .unwrap();
        assert_eq!(
            message,
            format!(
                "simulation encountered unresolved nondeterminism: replication 1, time {time}, cases {cases}, sites {sites}; at least two distinct successor actions remain"
            ),
            "{n}"
        );
        assert!(h::row(&v, "probability.scheduler").is_none());
        assert!(h::row(&v, "simulation.guarantee").is_some());
    }
    {
        let n = "pol-label-env";
        let s = h::edit(
            &h::source(n),
            "  bounds",
            "  simulation replications 4 seed 0 confidence 0.5\n  bounds",
        );
        assert_eq!(
            h::sampled(&s, None)["simulation"]["status"],
            "unsupported",
            "remaining cofinish or nondet is not resolved: {n}"
        );
    }
    let s = h::edit(
        &h::source("pol-no-person-reservation"),
        "  bounds",
        "  simulation replications 4 seed 0 confidence 0.5\n  bounds",
    );
    let v = h::sampled(&s, None);
    assert_eq!(v["simulation"]["status"], "completed");
    assert_eq!(v["simulation"]["completed_replications"], 4);
    let s = h::edit(
        &h::source("pol-assign"),
        "  process P",
        "  latent case { coin: bool }\n  process P",
    );
    assert_eq!(
        h::sampled(&s, None)["simulation"]["status"],
        "unsupported",
        "missing initial probability is not uniform"
    );
    let m = h::selected(&h::source("pol-assign"), None);
    let e = engine::simulate(&m);
    let r = reference::simulate(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e, r.outcome);
    assert_eq!(e.completed_replications, 4);
    assert!(
        e.replications.iter().all(|r| r.draws.is_empty()),
        "first_idle adds no random scheduler keys"
    );
}

#[test]
fn call_order_uses_body_not_binding_id_or_capability_declaration_order() {
    let s = h::source("pol-tie-actor");
    let m = h::selected(&s, None);
    let expected = engine::explore(&m);
    let mut renumbered = m.clone();
    assert_eq!(renumbered.process.bindings.len(), 2);
    renumbered.process.bindings.swap(0, 1);
    let ir::Stmt::Fork { branches, join, .. } = &mut renumbered.process.body[0].kind else {
        panic!("hand fork");
    };
    for c in branches.iter_mut() {
        c.binding = ir::BindingId(1 - c.binding.0);
    }
    let ir::Join::Any { matches, tie, .. } = join else {
        panic!("hand any");
    };
    for m in matches {
        m.binding = ir::BindingId(1 - m.binding.0);
    }
    let ir::Tie::By(ids) = tie else {
        panic!("hand fixed tie");
    };
    for id in ids {
        *id = ir::BindingId(1 - id.0);
    }
    assert_eq!(
        engine::explore(&renumbered),
        expected,
        "body order survives consistent public IR renumbering"
    );
    let mut r = reference::explore(&renumbered);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert!(r.outcome.diagnostics.starts_with(&renumbered.diagnostics));
    r.outcome.diagnostics.drain(..renumbered.diagnostics.len());
    assert_eq!(r.outcome, expected);
    let mut capabilities = m.clone();
    assert_eq!(capabilities.capabilities.len(), 2);
    capabilities.capabilities.swap(0, 1);
    let ir::Stmt::Fork { branches, .. } = &mut capabilities.process.body[0].kind else {
        unreachable!()
    };
    for c in branches {
        c.capability = ir::CapabilityId(1 - c.capability.0);
    }
    assert_eq!(
        engine::explore(&capabilities),
        expected,
        "Capability declaration indices are not call rank"
    );
    let mut r = reference::explore(&capabilities);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert!(r.outcome.diagnostics.starts_with(&capabilities.diagnostics));
    r.outcome
        .diagnostics
        .drain(..capabilities.diagnostics.len());
    assert_eq!(r.outcome, expected);
    let mut reversed = m;
    let ir::Stmt::Fork { branches, .. } = &mut reversed.process.body[0].kind else {
        unreachable!()
    };
    branches.reverse();
    let out = engine::explore(&reversed);
    let t = &out.case_outcomes.as_ref().unwrap()[0].terminals[0];
    assert_eq!(t.completed_at.map(|r| (r.min, r.max)), Some((5, 6)));
    assert_eq!(t.result_at.map(|r| (r.min, r.max)), Some((5, 5)));
}

#[test]
fn observable_is_not_assignment_or_a_new_random_key() {
    let s = h::source("pol-observable-fifo");
    let t = h::edit(&s, "observable ", "");
    assert_eq!(h::both(&s, None), h::both(&t, None));
    let named = h::edit(
        &s,
        "actor A { capacity 1;",
        "people [p,q]; actor A { executors [p,q];",
    );
    assert_eq!(
        h::sampled(&named, None)["simulation"]["status"],
        "unsupported"
    );
    let s = h::source("pol-observable-assign");
    let t = h::edit(&s, "observable ", "");
    let a = h::selected(&s, None);
    let b = h::selected(&t, None);
    assert_eq!(engine::simulate(&a), engine::simulate(&b));
}

#[test]
fn zero_budget_does_not_invent_finished_work_or_fixed_policy_proof() {
    let s = h::source("pol-label-env");
    let m = h::selected(&s, None);
    let e = engine::explore_with_budget(&m, 0);
    assert!(e.truncated);
    assert!(e.terminals.is_empty());
    assert!(
        e.case_outcomes
            .as_ref()
            .unwrap()
            .iter()
            .all(|c| c.terminals.is_empty())
    );
    assert!(e.diagnostics.iter().any(|d| d.code == "W-BUDGET-0"));
}

#[test]
fn explain_negative_separation_has_verified_c2_c2_witness_not_a_selector() {
    let s = h::source("pol-separation.broken-change");
    let v = serde_json::to_value(explain::explain(&s)).unwrap();
    assert_eq!(v["verdict"], "violated");
    assert_eq!(v["explanation"]["status"], "complete");
    let paths = v["explanation"]["counterexamples"].as_array().unwrap();
    assert!(!paths.is_empty());
    for p in paths {
        assert_eq!(p["replay"]["status"], "verified");
        let text = serde_json::to_string(p).unwrap();
        assert!(text.contains("c2"));
    }
}

#[test]
fn grammar_orders_newlines_semicolons_and_contextual_identifiers_are_explicit_controls() {
    let s = h::source("pol-label-cofinish");
    let a = h::selected(&s, None);
    for spelling in [
        "assign first_idle then call_order",
        "then\n call_order\n assign first_idle",
        "assign\nfirst_idle\nthen call_order",
        "then call_order assign first_idle;",
    ] {
        let t = h::edit(&s, "then call_order assign first_idle", spelling);
        let b = h::selected(&t, None);
        assert_eq!(engine::explore(&a), engine::explore(&b));
    }
    for word in ["observable", "then", "call_order", "assign", "first_idle"] {
        let old = h::old_skeleton(&h::source("pol-assign"));
        let t = h::edit(&old, "Clerk", word);
        assert!(syntax::parse(&t).is_ok());
        h::selected(&t, None);
        let t = h::edit(&old, "a = Clerk.work()", &format!("{word} = Clerk.work()"));
        h::selected(&t, None);
    }
}

#[test]
fn declared_executor_order_not_people_table_or_permission_controls_first_idle() {
    let s = h::source("pol-permission");
    let old = h::checked(&s, None);
    let t = h::edit(&s, "people [p,q]", "people [q,p]");
    assert_eq!(h::checked(&t, None)["verdict"], "violated");
    assert_eq!(old["case_outcomes"], h::checked(&t, None)["case_outcomes"]);
    let s = h::source("pol-separation");
    let old = h::checked(&s, None);
    let t = h::edit(&s, "people [c1,c2,a1]", "people [a1,c2,c1]");
    assert_eq!(h::checked(&t, None)["verdict"], "satisfied");
    assert_eq!(old["case_outcomes"], h::checked(&t, None)["case_outcomes"]);
    let s = h::permission_authority_source();
    for (source, want) in [
        (s.clone(), "violated"),
        (
            h::edit(&s, "executors [p,q]", "executors [q,p]"),
            "satisfied",
        ),
    ] {
        let v = h::checked(&source, None);
        assert_eq!(v["verdict"], want);
        for id in [
            "contract Ctl #1 permitted",
            "contract Ctl #2 authority_basis",
        ] {
            assert_eq!(
                v["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|p| p["id"] == id)
                    .unwrap()["verdict"],
                want
            );
        }
        assert_eq!(
            v["case_outcomes"][0]["terminals"][0]["completed_at"],
            json!({"min":1,"max":1})
        );
        h::both(&source, None);
    }
}

#[test]
fn old_fifo_extension_copies_cover_every_named_pair_and_contest_route() {
    const OLD: &[&str] = &[
        "arrivals-simultaneous",
        "arrivals-simultaneous.broken-no-policy",
        "arrivals-same-time-slot",
        "arrivals-same-time-slot.broken-later-arrival",
        "met-fifo",
        "met-fifo.broken-shorter-second-step",
        "sim-fifo",
        "sim-fifo.broken-no-policy",
        "rsrc-competition",
        "rsrc-competition.broken-same-actor",
        "ctx-contest-resource",
        "ctx-contest-resource.broken-slow-route",
        "ctx-contest-origin",
        "ctx-contest-origin.broken-arrival-clock",
        "aobs-shared-order",
        "aobs-shared-order.broken-no-requires",
    ];
    assert_eq!(OLD.len(), 16);
    for n in OLD {
        let s = h::source(n);
        let t = if s.contains("policy fifo tie_by case_id") {
            h::edit(
                &s,
                "policy fifo tie_by case_id",
                "policy fifo tie_by case_id then call_order assign first_idle",
            )
        } else {
            h::edit(
                &s,
                "bounds {",
                "policy fifo tie_by case_id then call_order assign first_idle\n bounds {",
            )
        };
        h::both(&t, None);
        if n.starts_with("ctx-contest-") {
            let v = h::checked(&t, None);
            assert!(
                h::row(&v, "policy.classes").unwrap()["value"]
                    .as_str()
                    .unwrap()
                    .contains("environment[Route]")
            );
            let u = h::edit(&t, "actor Route {", "actor Route { control policy;");
            h::both(&u, None);
        }
    }
}

#[test]
fn sampled_fixtures_also_have_independent_exhaustive_hand_times() {
    for (n, complete, elapsed, work) in [
        ("pol-assign", vec![3, 4], vec![3, 3], 3),
        ("pol-control", vec![3, 4], vec![3, 3], 3),
        ("pol-observable-assign", vec![3, 4], vec![3, 3], 3),
        ("pol-observable-fifo", vec![1, 2], vec![1, 2], 1),
        ("pol-no-effect", vec![3, 4], vec![3, 3], 3),
    ] {
        for name in [n.to_string(), format!("{n}.broken-change")] {
            let s = h::source(&name);
            let o = h::both(&s, None);
            let cs = o.case_outcomes.unwrap();
            assert_eq!(cs.len(), complete.len());
            for (i, c) in cs.iter().enumerate() {
                assert_eq!(c.terminals.len(), 1);
                let t = &c.terminals[0];
                assert_eq!(
                    t.completed_at.map(|r| (r.min, r.max)),
                    Some((complete[i], complete[i]))
                );
                assert_eq!(
                    t.elapsed.map(|r| (r.min, r.max)),
                    Some((elapsed[i], elapsed[i]))
                );
                assert_eq!(t.work.map(|r| (r.min, r.max)), Some((work, work)));
            }
        }
    }
}

#[test]
fn static_diagnostic_spans_point_at_bad_choice_or_second_extension() {
    for (name, needle, code, message, last) in [
        (
            "pol-static-round-robin.broken-change",
            "round_robin",
            "U-SYN-0",
            "unsupported syntax: round_robin",
            false,
        ),
        (
            "pol-static-random.broken-change",
            "random",
            "U-SYN-0",
            "unsupported syntax: random",
            false,
        ),
        (
            "pol-static-skill.broken-change",
            "by",
            "U-SYN-0",
            "unsupported syntax: by",
            true,
        ),
        (
            "pol-static-priority.broken-change",
            "priority",
            "U-SYN-0",
            "unsupported syntax: priority",
            false,
        ),
        (
            "pol-static-optimize.broken-change",
            "optimize",
            "U-SYN-0",
            "unsupported syntax: optimize",
            false,
        ),
        (
            "pol-static-synthesize.broken-change",
            "synthesize",
            "U-SYN-0",
            "unsupported syntax: synthesize",
            false,
        ),
        (
            "pol-latent.broken-change",
            "hard",
            "U-SYN-0",
            "unsupported syntax: hard",
            true,
        ),
        (
            "pol-static-duplicate-assign.broken-change",
            "assign first_idle",
            "U-SEM-0",
            "duplicate assign first_idle",
            true,
        ),
        (
            "pol-static-duplicate-then.broken-change",
            "then call_order",
            "U-SEM-0",
            "duplicate then call_order",
            true,
        ),
    ] {
        let s = h::source(name);
        let v = h::checked(&s, None);
        let start = if last {
            s.rfind(needle)
        } else {
            s.find(needle)
        }
        .unwrap();
        let ds = v["diagnostics"].as_array().unwrap();
        // Parse failures cannot establish Cut presence. Lower errors retain
        // the written Cut without adding a missing-Cut warning.
        if code == "U-SYN-0" {
            assert_eq!(v["cut"], "unresolved", "{name}");
        }
        assert_eq!(ds.len(), 1, "{name}");
        assert_eq!(ds[0]["code"], code);
        assert_eq!(ds[0]["severity"], "error");
        assert_eq!(ds[0]["message"], message);
        assert_eq!(
            ds[0]["span"],
            json!({"start":start,"end":start+needle.len()}),
            "{name}"
        );
    }
    for suffix in ["then", "assign"] {
        let s = format!("model M version \"0.2\" {{ policy fifo tie_by case_id {suffix} }}");
        let v = h::checked(&s, None);
        assert_eq!(v["verdict"], "invalid");
        assert_eq!(v["diagnostics"][0]["code"], "E-SYN-0");
    }
}

#[test]
fn contract_properties_and_case_cut_verdicts_are_not_only_root_classifications() {
    for (base, id, positive, negative) in [
        (
            "pol-separation",
            "contract Ctl #1 separation",
            "satisfied",
            "violated",
        ),
        (
            "pol-names",
            "contract Ctl #1 separation",
            "satisfied",
            "violated",
        ),
        (
            "pol-permission",
            "contract Ctl #1 permitted",
            "violated",
            "satisfied",
        ),
        (
            "pol-tie-actor",
            "contract Due #1 response",
            "satisfied",
            "violated",
        ),
        (
            "pol-tie-person",
            "contract Due #1 response",
            "satisfied",
            "violated",
        ),
        (
            "pol-cross-class-person",
            "contract Due #1 response",
            "violated",
            "satisfied",
        ),
        (
            "pol-cross-class-resource",
            "contract Due #1 response",
            "violated",
            "satisfied",
        ),
    ] {
        for (name, want) in [
            (base.to_string(), positive),
            (format!("{base}.broken-change"), negative),
        ] {
            let v = h::checked(&h::source(&name), None);
            let properties = v["properties"].as_array().unwrap();
            assert_eq!(properties.len(), 3, "two queries and one contract: {name}");
            let p = properties.iter().find(|p| p["id"] == id).unwrap();
            assert_eq!(p["verdict"], want);
            assert_eq!(
                properties
                    .iter()
                    .find(|p| p["id"] == "safety no_resource_overcommit")
                    .unwrap()["verdict"],
                "satisfied"
            );
        }
    }
    let v = h::checked(&h::source("pol-resource.broken-change"), None);
    let p = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "reachable end.done")
        .unwrap();
    assert_eq!(p["verdict"], "inconclusive");
    assert_eq!(p["cases"][0]["verdict"], "satisfied");
    assert_eq!(p["cases"][1]["verdict"], "inconclusive");
    let v = h::checked(&h::source("pol-refuse.broken-change"), None);
    assert_eq!(
        v["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "state_in_range")
            .unwrap()["verdict"],
        "violated"
    );
}

#[test]
fn new_words_are_not_global_reserved_words_on_old_models() {
    let old = h::old_skeleton(&h::source("pol-assign"));
    for word in ["observable", "then", "call_order", "assign", "first_idle"] {
        let actor = h::edit(&old, "Clerk", word);
        h::selected(&actor, None);
        let binding = h::edit(&old, "a = Clerk.work()", &format!("{word} = Clerk.work()"));
        h::selected(&binding, None);
        let variable = h::edit(
            &old,
            "  process P",
            &format!("  observed case {{ {word}: bool = false }}\n  process P"),
        );
        h::selected(&variable, None);
    }
}

#[test]
fn bijective_same_width_names_and_grain_preserve_the_entire_report_except_source_hash() {
    let s = h::edit(
        &h::source("pol-separation"),
        "actor Clerk {",
        "actor Clerk { grain human;",
    );
    let mut t = s.clone();
    let renames = [
        ("Clerk", "Zebra"),
        ("Approver", "Minister"),
        ("c1", "z1"),
        ("c2", "y2"),
        ("a1", "x1"),
        ("human", "table"),
    ];
    for (old, new) in renames {
        assert_eq!(old.len(), new.len());
        t = h::edit(&t, old, new);
    }
    assert_eq!(s.len(), t.len());
    let mut expected = h::checked(&s, None);
    let mut actual = h::checked(&t, None);
    assert_ne!(expected["model"]["sha256"], actual["model"]["sha256"]);
    expected["model"].as_object_mut().unwrap().remove("sha256");
    actual["model"].as_object_mut().unwrap().remove("sha256");
    fn normalize(v: &mut serde_json::Value, renames: &[(&str, &str)]) {
        match v {
            serde_json::Value::String(s) => {
                for (old, new) in renames {
                    *s = s.replace(new, old);
                }
            }
            serde_json::Value::Array(xs) => {
                for x in xs {
                    normalize(x, renames);
                }
            }
            serde_json::Value::Object(o) => {
                for x in o.values_mut() {
                    normalize(x, renames);
                }
            }
            _ => {}
        }
    }
    for actor in actual["actors"].as_array_mut().unwrap() {
        if actor["name"] == "Zebra" {
            assert_eq!(actor["grain"]["value"], "table");
            actor["grain"]["value"] = json!("human");
        }
    }
    normalize(&mut actual, &renames[..5]);
    assert_eq!(
        expected, actual,
        "no diagnostics/assumptions/Agency/locations dropped"
    );
}

#[test]
fn all_three_classes_and_unused_declared_policy_have_explicit_noop_text() {
    let s = h::source("pol-separation");
    let s = h::edit(
        &s,
        "  cut C { include [Clerk,Approver] }",
        "  actor Vendor { control party }\n  cut C { include [Clerk,Approver,Vendor] }",
    );
    let v = h::checked(&s, None);
    assert_eq!(
        h::row(&v, "policy.classes").unwrap()["value"],
        "policy[Clerk, Approver]; environment[]; party[Vendor]"
    );
    let s = h::source("pol-control.broken-change");
    let s = h::edit(&s, "people [c1,c2]", "people [c1,c2,q]");
    let s = h::edit(
        &s,
        "  cut C { include [Clerk] }",
        "  actor Idle { executors [q]; control policy }\n  cut C { include [Clerk,Idle] }",
    );
    let v = h::checked(&s, None);
    assert_eq!(
        h::row(&v, "policy.classes").unwrap()["value"],
        "policy[Idle]; environment[Clerk]; party[]"
    );
    assert_eq!(
        h::row(&v, "policy.classes").unwrap()["affects"],
        format!(
            "{} 選択processと宣言されたcontest routeにpolicy区分の呼出しは無い。",
            h::CLASSES_AFFECTS
        )
    );
    assert_eq!(
        h::row(&v, "policy.assign").unwrap()["affects"],
        format!(
            "{} 選択processと宣言されたcontest routeに、方策の区分の実行者つき呼出しは無い。",
            h::ASSIGN_AFFECTS
        )
    );
    // A single lexical site / one case call-order-only declaration is a separate no-op.
    let s = h::edit(
        &h::source("pol-permission"),
        "assign first_idle",
        "then call_order",
    );
    let v = h::checked(&s, None);
    assert_eq!(
        h::row(&v, "policy.classes").unwrap()["affects"],
        format!(
            "{} 方策の区分で同時に待つ呼出しは高々1件であり、開始順の取り合いは無い。",
            h::CLASSES_AFFECTS
        )
    );
}

#[test]
fn default_classification_includes_dead_lexical_calls_but_queue_comparison_does_not() {
    let s = h::source("pol-assign");
    let s = h::edit(&s, "assign first_idle", "then call_order assign first_idle");
    let s = h::edit(
        &s,
        "  process P",
        "  observed case { flag: bool = false }\n  actor Dormant { capacity 1; capability work() -> T { duration fixed 1min; result nondet { ok } } }\n  process P",
    );
    let s = h::edit(
        &s,
        "process P { a = Clerk.work()",
        "process P { if flag == true { b = Dormant.work() } else {}; a = Clerk.work()",
    );
    let s = h::edit(&s, "include [Clerk]", "include [Clerk,Dormant]");
    let v = h::checked(&s, None);
    assert_eq!(
        h::row(&v, "policy.classes").unwrap()["value"],
        "policy[Clerk, Dormant]; environment[]; party[]"
    );
    let v = h::sampled(&s, None);
    assert_eq!(v["simulation"]["status"], "completed");
    assert_eq!(v["simulation"]["completed_replications"], 4);
}

fn refused_post(first: bool) -> String {
    let s = h::edit(&h::source("pol-label-env"), " assign first_idle", "");
    let s = h::edit(&s, "  kernel K", "  people [p,q]\n  kernel K");
    let s = h::edit(
        &s,
        "  process P {",
        "  actor R { executors [p,q]; state posted: 0..1 = 1; capability post() -> T { requires posted == 0 else bad; duration fixed 1min; result nondet { ok } } }\n  process P {",
    );
    let s = if first {
        h::edit(
            &s,
            "fork f { a = A.draw();",
            "fork f { r = R.post(); a = A.draw();",
        )
    } else {
        h::edit(&s, "b = B.choose() }", "b = B.choose(); r = R.post() }")
    };
    h::edit(&s, "include [A,B]", "include [A,B,R]")
}

fn refused_post_oracle(s: &str, refused: bool, label: &str, work: u64) {
    let v = h::checked(s, None);
    assert_eq!(v["verdict"], "satisfied");
    h::probability(&v, "0", "1/10");
    h::label(&v, Some(label));
    h::subset(
        &json!([{"kind":"end","end":"done","completed_at":{"min":2,"max":2},"elapsed":{"min":2,"max":2},"work":{"min":work,"max":work}}]),
        &v["case_outcomes"][0]["terminals"],
    );
    let rs = v["capability_refusals"].as_array().unwrap();
    if refused {
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0]["call"], "r");
        assert_eq!(rs[0]["result"], "bad");
        assert_eq!(rs[0]["at"], json!({"min":0,"max":0}));
    } else {
        assert!(rs.is_empty());
    }
    let e = h::both(s, None);
    assert_eq!(
        e.probability_policy_unresolved,
        Some(label != "fixed-policy"),
        "engine and reference classify the same policy choices"
    );
}

#[test]
fn refused_policy_task_is_one_policy_choice_even_with_two_idle_people() {
    let s = refused_post(false);
    refused_post_oracle(&s, true, "fixed-policy", 3);
    let started = h::edit(&s, "posted: 0..1 = 1", "posted: 0..1 = 0");
    refused_post_oracle(&started, false, "full-information", 4);
}

#[test]
fn refusal_and_next_start_are_not_alternatives_of_one_state() {
    let s = refused_post(true);
    refused_post_oracle(&s, true, "fixed-policy", 3);
}

#[test]
fn declared_control_metadata_disagreeing_with_classes_is_not_read_at_runtime() {
    for n in ["pol-separation", "pol-cross-class-person", "pol-permission"] {
        let m = h::selected(&h::source(n), None);
        let classes = m.start_policy.as_ref().unwrap().classes.clone().unwrap();
        assert!(!classes.policy.is_empty(), "{n} has a policy Actor");
        let mut flipped = m.clone();
        for (i, a) in flipped.actors.iter_mut().enumerate() {
            let policy = classes.policy.contains(&ir::ActorId(i));
            a.metadata.control = Some(if policy { "environment" } else { "policy" }.into());
        }
        assert_ne!(flipped.actors, m.actors);
        assert_eq!(engine::explore(&flipped), engine::explore(&m), "{n}");
        assert_eq!(
            reference::explore(&flipped).outcome,
            reference::explore(&m).outcome,
            "{n}"
        );
    }
}

#[test]
fn unknown_control_in_hand_built_ast_is_unsupported_not_environment() {
    use relca_core::{ast, diag, lower};
    let s = h::edit(
        &h::source("pol-label-env"),
        "actor B { capacity 1;",
        "actor B { capacity 1; control environment;",
    );
    let mut tree = syntax::parse(&s).unwrap();
    let m = lower::lower(&tree).unwrap();
    let b = ir::ActorId(m.actors.iter().position(|a| a.name == "B").unwrap());
    let classes = m.start_policy.unwrap().classes.unwrap();
    assert!(classes.environment.contains(&b) && !classes.policy.contains(&b));
    let mut edited = 0;
    for item in &mut tree.items {
        if let ast::Item::Actor(a) = item {
            for field in &mut a.items {
                if let ast::ActorItem::Control(v) = field {
                    assert_eq!(v.value, "environment");
                    v.value = "bystander".into();
                    edited += 1;
                }
            }
        }
    }
    assert_eq!(edited, 1);
    let ds = lower::lower(&tree).expect_err("unknown control must fail closed");
    assert!(
        ds.iter()
            .any(|d| d.code == diag::U_SEM_0 && d.severity == diag::Severity::Error),
        "{ds:?}"
    );
}

#[test]
fn reference_evaluates_requires_only_for_start_candidates_like_engine() {
    let base = "model M version \"0.2\" {\n  type T = enum { ok, bad }\n  actor R { capacity 1; state posted: 0..1 = 0; capability hold() -> T { duration fixed 5min; result nondet { ok }; cancel cooperative ack_within 1min } capability post() -> T { requires posted == 0 else bad; duration fixed 1min; result nondet { ok }; cancel cooperative ack_within 1min } }\n  actor G { capacity 1; capability quick() -> T { duration fixed 1min; result nondet { ok }; cancel cooperative ack_within 1min } }\n  process P {\n    fork f { h = R.hold(); r = R.post(); g = G.quick() }\n    join any f where result == ok tie by [g, h, r] remaining cancel_then_drain if none -> done\n    end done\n  }\n  cut C { include [R,G] }\n  bounds { cases 1; live_tasks 4; clock 10min }\n  policy fifo tie_by case_id then call_order\n  verify reachable end.done\n}\n";
    let nested = h::edit(
        base,
        "requires posted == 0 else",
        "requires posted == 0 && (posted == 0 || posted == 1) else",
    );
    let limits = reference::Limits {
        control_depth: 2,
        ..Default::default()
    };
    for s in [base.to_string(), nested.clone()] {
        let v = h::checked(&s, None);
        assert_eq!(v["verdict"], "satisfied");
        h::subset(
            &json!([{"kind":"end","end":"done","completed_at":{"min":2,"max":2},"work":{"min":3,"max":3},"result_at":{"min":1,"max":1}}]),
            &v["case_outcomes"][0]["terminals"],
        );
        let m = h::selected(&s, None);
        let e = engine::explore(&m);
        let mut r = reference::explore_with_limits(&m, limits);
        assert_eq!(
            r.stop,
            reference::StopReason::Complete,
            "post stays queued behind hold and is cancelled at the join before it is a start candidate: {s}"
        );
        assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
        r.outcome.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(e, r.outcome);
    }
    let alone = h::edit(
        &nested,
        "fork f { h = R.hold(); r = R.post(); g = G.quick() }\n    join any f where result == ok tie by [g, h, r] remaining cancel_then_drain if none -> done\n",
        "r = R.post()\n",
    );
    let m = h::selected(&alone, None);
    assert_eq!(
        reference::explore_with_limits(&m, limits).stop,
        reference::StopReason::EnumerationLimit,
        "a start candidate still evaluates the nested requires under the limit"
    );
    assert_eq!(reference::explore(&m).stop, reference::StopReason::Complete);
}
