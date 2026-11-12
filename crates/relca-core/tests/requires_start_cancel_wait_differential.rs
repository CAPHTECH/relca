//! Each interpreter must meet the hand arithmetic before equality is considered.
#[path = "support/requires_acceptance.rs"]
mod hand;
use relca_core::{engine, lower, reference, syntax};
use serde_json::json;
#[test]
fn runtime29_complete_static3_excluded_and_no_silent_reference_exclusions() {
    let mut complete = 0;
    let mut excluded = vec![];
    for name in hand::NAMES {
        if hand::static_case(name) {
            excluded.push(name);
            continue;
        }
        let m = lower::lower(&syntax::parse(&hand::source(name)).unwrap()).unwrap();
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete, "{name}");
        let e = engine::explore(&m);
        let h = hand::expected(name);
        assert_eq!(
            serde_json::to_value(r.verdict()).unwrap(),
            h["verdict"],
            "reference hand root {name}"
        );
        for (which, o) in [("reference", &r.outcome), ("engine", &e)] {
            assert_eq!(
                o.truncated,
                h["bounds"]["truncated"].as_bool().unwrap(),
                "{which} hand cut {name}"
            );
            let ts:Vec<_>=o.terminals.iter().map(|t|json!({"end":m.ends[t.end.0],"elapsed":{"min":t.elapsed.min,"max":t.elapsed.max},"work":{"min":t.work.min,"max":t.work.max},"result_at":t.result_at.map(|r|json!({"min":r.min,"max":r.max}))})).collect();
            hand::subset(&h["terminals"], &json!(ts));
            if let Some(metrics) = h.get("metrics") {
                hand::subset(
                    metrics,
                    &serde_json::to_value(o.metrics.as_ref().expect("hand metrics")).unwrap(),
                );
            }
            if let Some(hcs) = h.get("case_outcomes") {
                let cs = o.case_outcomes.as_ref().expect("hand arrivals cases");
                assert_eq!(cs.len(), 2);
                for (c, hc) in cs.iter().zip(hcs.as_array().unwrap()) {
                    assert_eq!(c.arrival, hc["arrival"].as_u64().unwrap());
                    assert_eq!(c.terminals.len(), 1);
                    let t = &c.terminals[0];
                    for (key, value) in [
                        ("elapsed", t.elapsed),
                        ("work", t.work),
                        ("completed_at", t.completed_at),
                    ] {
                        assert_eq!(
                            value.map(|v| json!({"min":v.min,"max":v.max})),
                            (!hc["terminals"][0][key].is_null())
                                .then(|| hc["terminals"][0][key].clone()),
                            "{which} {name} case {key}"
                        );
                    }
                }
            }
        }
        let mut ro = r.outcome;
        assert!(ro.diagnostics.starts_with(&m.diagnostics));
        ro.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(e, ro, "all runtime fields {name}");
        complete += 1;
    }
    assert_eq!(complete, 29);
    assert_eq!(
        excluded,
        [
            "req-explicit-else.broken-no-else",
            "req-observed.broken-latent",
            "req-start-siblings.broken-same-target"
        ]
    );
}

#[test]
fn deterministic_legacy_arrivals_and_refused_kernel_have_four_actual_paths_and_no_draws() {
    let mut complete = 0;
    for name in [
        "req-start.broken-false",
        "req-wait-gap",
        "req-wait-arrivals",
        "req-kernel",
    ] {
        let s = hand::source(name).replacen(
            "  bounds",
            "  simulation replications 4 seed 0 confidence 0.5\n  bounds",
            1,
        );
        let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
        let e = engine::simulate(&m);
        let r = reference::simulate(&m);
        assert_eq!(r.stop, reference::StopReason::Complete, "{name}");
        for o in [&e, &r.outcome] {
            assert_eq!(o.completed_replications, 4, "{name}");
            assert_eq!(o.replications.len(), 4);
            for rep in &o.replications {
                assert!(
                    rep.draws.is_empty(),
                    "no duration/kernel/implicit scheduling draws on {name}"
                );
                assert_eq!(rep.cut, None);
                for c in &rep.cases {
                    assert_eq!(
                        c.terminal.as_ref().unwrap().end.as_deref(),
                        Some(if name == "req-start.broken-false" {
                            "refused"
                        } else if name == "req-kernel" {
                            "denied"
                        } else {
                            "done"
                        })
                    );
                }
            }
        }
        assert_eq!(e, r.outcome, "simulation differential {name}");
        complete += 1;
    }
    assert_eq!(complete, 4);
}

#[test]
fn contest_route_requires_refusal_is_not_normal_response_in_either_interpreter() {
    let base=hand::source("ctx-contest").replace("  people", "  observed case { flag: bool = false; begun: bool = false }\n  people")
        .replace("duration fixed 2min; result nondet { pass }", "duration fixed 2min; result nondet { pass }; requires flag == true else fail; start { begun = true }; cancel immediate");
    for (source, refused) in [
        (base.clone(), true),
        (
            base.replace("flag: bool = false", "flag: bool = true"),
            false,
        ),
    ] {
        let legacy = hand::old_skeleton(&source);
        lower::lower(&syntax::parse(&legacy).unwrap()).unwrap();
        let m = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        let e = engine::explore(&m);
        for o in [&r.outcome, &e] {
            assert_eq!(o.terminals.len(), 1);
            assert_eq!(
                (o.terminals[0].elapsed.min, o.terminals[0].elapsed.max),
                (1, 1),
                "business end frozen"
            );
            let rows = serde_json::to_value(o.contest_runtime.as_ref().unwrap()).unwrap();
            let alts = rows[0]["alternatives"].as_array().unwrap();
            assert!(alts.iter().any(|a| a["filed"] == false));
            for a in alts.iter().filter(|a| a["filed"] == true) {
                if refused {
                    assert_eq!(a["status"], "violated");
                    assert_eq!(a["violation_reason"], "refused");
                    assert!(a["response_at"].is_null());
                    assert!(a["result"].is_null());
                    assert_eq!(a["route_work"], json!({"min":0,"max":0}));
                } else {
                    assert_eq!(a["status"], "responded");
                    assert_eq!(a["response_at"], 3);
                    assert_eq!(a["route_work"], json!({"min":2,"max":2}));
                }
            }
        }
        let report = serde_json::to_value(relca_core::report::check(&source)).unwrap();
        assert_eq!(
            report["verdict"],
            if refused { "violated" } else { "satisfied" }
        );
        if refused {
            hand::subset(
                &json!([{"case_id":1,"call":"contest:1","capability":"Route.respond","result":"fail","at":{"min":1,"max":1},"coverage":"complete"}]),
                &report["capability_refusals"],
            );
        } else {
            assert_eq!(report["capability_refusals"], json!([]));
        }
        let mut ro = r.outcome;
        ro.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(e, ro);
    }
}
