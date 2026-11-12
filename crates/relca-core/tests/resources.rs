//! ADR 0033 Stage C independent hand acceptance; intentionally RED before implementation.
#[path = "support/resource_acceptance.rs"]
mod hand;
use relca_core::{engine, explain, lower, reference, report, syntax};
use serde_json::{Value, json};
fn checked(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}
fn lowered(s: &str) -> relca_core::ir::Model {
    lower::lower(&syntax::parse(s).unwrap()).unwrap()
}
fn differential(s: &str) -> relca_core::outcome::Outcome {
    lowered(&hand::old_skeleton(s));
    let m = lowered(s);
    let e = engine::explore(&m);
    let mut r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
    r.outcome.diagnostics.drain(..m.diagnostics.len());
    assert_eq!(e, r.outcome, "independent complete Outcome");
    e
}
#[test]
fn thirty_pairs_are_exact_nonempty_single_edits_and_closed_population() {
    let pairs = hand::pairs();
    assert_eq!(pairs.as_array().unwrap().len(), 30);
    for p in pairs.as_array().unwrap() {
        let s = hand::source(p["good"].as_str().unwrap());
        let t = hand::edit(&s, p["from"].as_str().unwrap(), p["to"].as_str().unwrap());
        assert_eq!(t, hand::source(p["bad"].as_str().unwrap()));
    }
    let current: std::collections::BTreeSet<_> = std::fs::read_dir(hand::root())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("rsrc-"))
        .collect();
    assert_eq!(current, hand::names().into_iter().collect());
    for name in hand::names() {
        let h = hand::expected(&name);
        assert_eq!(
            hand::static_case(&name),
            matches!(h["verdict"].as_str().unwrap(), "invalid" | "unsupported"),
            "closed static/runtime classification {name}"
        );
    }
}
#[test]
fn old_syntax_preflight_has_no_hidden_name_cancel_type_or_metrics_errors() {
    for p in hand::pairs().as_array().unwrap() {
        let good = p["good"].as_str().unwrap();
        for n in [good, p["bad"].as_str().unwrap()] {
            let s = hand::source(if hand::static_case(n) { good } else { n });
            let old = hand::old_skeleton(&s);
            let ast =
                syntax::parse(&old).unwrap_or_else(|d| panic!("old syntax {n}: {d:?}\n{old}"));
            lower::lower(&ast).unwrap_or_else(|d| panic!("old lower {n}: {d:?}\n{old}"));
        }
    }
}
#[test]
fn all60_public_reports_match_hand_time_work_occupancy_peak_utilization_and_diagnostics() {
    for n in hand::names() {
        let h = hand::expected(&n);
        let v = checked(&hand::source(&n));
        hand::subset(&h, &v);
    }
}
#[test]
fn runtime50_complete_zero_time1_excluded_static9_excluded_and_no_silent_reference_exclusions() {
    let mut complete = 0;
    let mut zero = vec![];
    let mut statics = vec![];
    for n in hand::names() {
        if hand::static_case(&n) {
            statics.push(n);
            continue;
        }
        if n == "rsrc-zero-time" {
            zero.push(n);
            continue;
        }
        let s = hand::source(&n);
        let m = lowered(&s);
        let h = hand::expected(&n);
        let e = engine::explore(&m);
        let mut r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete, "{n}");
        for o in [&e, &r.outcome] {
            let metrics =
                serde_json::to_value(o.metrics.as_ref().expect("hand resource metrics")).unwrap();
            hand::subset(&h["metrics"], &metrics);
            let ts:Vec<_>=o.terminals.iter().map(|t|json!({"end":m.ends[t.end.0],"elapsed":{"min":t.elapsed.min,"max":t.elapsed.max},"work":{"min":t.work.min,"max":t.work.max},"result_at":t.result_at.map(|v|json!({"min":v.min,"max":v.max}))})).collect();
            hand::subset(&h["terminals"], &json!(ts));
            if let Some(cs) = h.get("case_outcomes") {
                let actual = o.case_outcomes.as_ref().unwrap();
                assert_eq!(actual.len(), cs.as_array().unwrap().len());
                for (a, hc) in actual.iter().zip(cs.as_array().unwrap()) {
                    let t = &a.terminals[0];
                    assert_eq!(
                        t.elapsed.map(|v| json!({"min":v.min,"max":v.max})),
                        Some(hc["terminals"][0]["elapsed"].clone())
                    );
                    assert_eq!(
                        t.work.map(|v| json!({"min":v.min,"max":v.max})),
                        Some(hc["terminals"][0]["work"].clone())
                    );
                }
            }
        }
        assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
        r.outcome.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(e, r.outcome, "{n}: all Outcome fields");
        complete += 1;
    }
    assert_eq!(complete, 50);
    assert_eq!(zero, ["rsrc-zero-time"]);
    assert_eq!(statics.len(), 9);
}
#[test]
fn legacy_and_one_arrival_paths_agree_on_exact_atomic_cancel_refuse() {
    for n in [
        "rsrc-exact",
        "rsrc-atomic",
        "rsrc-cooperative",
        "rsrc-immediate",
        "rsrc-refusal",
    ] {
        let s = hand::source(n);
        let arrived = hand::edit(&s, "  bounds", "  arrivals fixed [0min]\n  bounds");
        let o = differential(&arrived);
        let h = hand::expected(n);
        let ts = o.case_outcomes.as_ref().unwrap();
        let t = &ts[0].terminals[0];
        assert_eq!(
            t.elapsed.map(|v| json!({"min":v.min,"max":v.max})),
            Some(h["terminals"][0]["elapsed"].clone())
        );
        hand::subset(
            &h["metrics"],
            &serde_json::to_value(o.metrics.unwrap()).unwrap(),
        );
    }
}
#[test]
fn weighted_distribution_and_same_tick_ack_finish_have_independent_interval_oracles() {
    let s = hand::source("rsrc-weighted");
    for (duration, occ, elapsed, work) in [
        ("duration { 3min:1 }", ("8", "8"), (3, 3), (5, 5)),
        ("duration { 1min:0.5,3min:0.5 }", ("4", "8"), (2, 3), (3, 5)),
    ] {
        let s = hand::edit(&s, "duration fixed 3min", duration);
        let o = differential(&s);
        let m = serde_json::to_value(o.metrics.unwrap()).unwrap();
        assert_eq!(
            m["resources"][0]["occupancy_time"],
            json!({"min":occ.0,"max":occ.1,"reason":null})
        );
        assert_eq!(
            (o.terminals[0].elapsed.min, o.terminals[0].elapsed.max),
            elapsed
        );
        assert_eq!((o.terminals[0].work.min, o.terminals[0].work.max), work);
    }
    let s = hand::source("rsrc-cooperative");
    let s = hand::edit(&s, "duration fixed 6min", "duration fixed 3min");
    let o = differential(&s);
    assert_eq!(o.terminals[0].elapsed.min, 4);
    hand::subset(
        &hand::expected("rsrc-cooperative")["metrics"],
        &serde_json::to_value(o.metrics.unwrap()).unwrap(),
    );
}
#[test]
fn names_grain_unused_resources_and_no_metrics_do_not_become_actor_or_agency_rows() {
    let s = hand::source("rsrc-exact");
    let base = checked(&s);
    {
        // Every name occupies the same nine bytes, so every location is
        // byte-identical; only the fields that carry the name are normalized.
        let named = |name: &str| {
            let padded = format!("{name:<9}");
            hand::edit(
                &hand::edit(&s, "resource R ", &format!("resource {padded} ")),
                "uses R:",
                &format!("uses {padded}:"),
            )
        };
        fn normalize(v: &mut Value, name: &str) {
            match v {
                Value::Object(o) => {
                    for (k, x) in o.iter_mut() {
                        if k == "resource" && x.as_str() == Some(name) {
                            *x = json!("R");
                        } else if k == "field"
                            && let Some(rest) = x
                                .as_str()
                                .and_then(|f| f.strip_prefix(&format!("resource.{name}")))
                                .filter(|rest| rest.is_empty() || rest.starts_with('.'))
                        {
                            *x = json!(format!("resource.R{rest}"));
                        } else {
                            normalize(x, name);
                        }
                    }
                }
                Value::Array(a) => a.iter_mut().for_each(|x| normalize(x, name)),
                _ => {}
            }
        }
        let witness = |src: &str| {
            hand::edit(
                src,
                "  bounds",
                "  observed case { flag: bool = false }\n  contract C { invariant A.y.finish -> flag == true }\n  bounds",
            )
        };
        let all = |src: &str| {
            let mut v = json!({
                "check": checked(src),
                "explain": serde_json::to_value(explain::explain(&witness(src))).unwrap(),
            });
            for surface in ["check", "explain"] {
                let hash = &mut v[surface]["model"]["sha256"];
                assert_eq!(hash.as_str().map(str::len), Some(64), "source hash");
                *hash = json!("source hash");
            }
            v
        };
        let reference = all(&named("R"));
        assert_eq!(reference["check"]["metrics"], base["metrics"]);
        assert_eq!(
            reference["explain"]["explanation"]["counterexamples"][0]["replay"]["status"],
            "verified"
        );
        for name in ["clerks", "erp_slots", "neutral"] {
            let raw = all(&named(name));
            assert_eq!(raw["check"]["metrics"]["resources"][0]["resource"], name);
            let mut v = raw.clone();
            normalize(&mut v, name);
            assert_ne!(raw, v, "the name appears in the output");
            assert_eq!(v, reference, "{name}: every non-name field");
        }
    }
    let s = hand::edit(&s, "actor A {", "actor A { grain document;");
    assert_eq!(checked(&s)["terminals"], base["terminals"]);
    let extra = hand::edit(
        &s,
        "resource R capacity 2",
        "resource R capacity 2; resource Unused capacity 3; resource Empty capacity 0",
    );
    let v = checked(&extra);
    assert_eq!(v["agency"], checked(&s)["agency"]);
    assert_eq!(v["metrics"]["resources"].as_array().unwrap().len(), 3);
    assert_eq!(v["metrics"]["resources"][1]["occupancy_time"]["min"], "0");
    assert_eq!(
        v["metrics"]["resources"][2]["utilization"]["reason"],
        "zero_capacity"
    );
    let s = hand::edit(&extra, "metrics { window 10min; currency \"JPY\" }", "");
    let v = checked(&s);
    assert!(v.get("metrics").is_none());
    assert!(v.get("resources").is_none());
    let assumptions = v["assumptions"].as_array().unwrap();
    let rows: Vec<_> = assumptions
        .iter()
        .filter(|a| a["field"].as_str().unwrap().starts_with("resource."))
        .collect();
    assert_eq!(rows.len(), 3);
    assert!(
        rows[1]["affects"]
            .as_str()
            .unwrap()
            .ends_with(" この資源を使うCapabilityの宣言はない。")
    );
}
#[test]
fn reserved_identifiers_keep_the_old_unsupported_classification() {
    let s = hand::old_skeleton(&hand::source("rsrc-over-demand"));
    for word in ["resource", "uses"] {
        for (from, to) in [
            ("actor A {", format!("actor {word} {{")),
            (
                "type T = enum",
                format!("observed case {{ {word}: bool = false }} type T = enum"),
            ),
        ] {
            let v = checked(&hand::edit(&s, from, &to));
            assert_eq!(v["verdict"], "unsupported");
            assert_eq!(v["diagnostics"][0]["code"], "U-SYN-0");
        }
    }
}
#[test]
fn static_boundaries_names_rates_cut_and_unselected_capabilities_fail_closed() {
    let s = hand::source("rsrc-over-demand");
    for (from, to, verdict, code) in [
        (
            "resource R capacity 1",
            "resource R capacity 1; resource R capacity 1",
            "unsupported",
            "U-SEM-0",
        ),
        (
            "currency \"JPY\"",
            "currency \"JPY\"; rate R per_minute 1",
            "unsupported",
            "U-SEM-0",
        ),
        ("include [A]", "include [A,R]", "invalid", "E-NAME-0"),
        ("actor A {", "actor A : human {", "invalid", "E-SYN-0"),
        (
            "R capacity 1",
            "R capacity 1 instance per_case",
            "unsupported",
            "U-SYN-0",
        ),
        (
            "R capacity 1",
            "R capacity 1 calendar always",
            "invalid",
            "E-NAME-0",
        ),
        ("uses R: 1", "uses R: 1 partial", "unsupported", "U-SYN-0"),
    ] {
        let v = checked(&hand::edit(&s, from, to));
        assert_eq!(v["verdict"], verdict, "{v}");
        assert_eq!(v["diagnostics"][0]["code"], code, "{v}");
    }
    let v = checked(&hand::edit(&s, "actor A {", "people [R]; actor A {"));
    assert_eq!(v["verdict"], "satisfied");
    let unused = hand::edit(
        &s,
        "actor A {",
        "actor A { capability dormant() -> T { uses R: 2; duration fixed 1min; result nondet { ok } }",
    );
    assert_eq!(checked(&unused)["verdict"], "satisfied");
}
#[test]
fn deterministic_fifo_simulation_keeps_whole_replication_units_and_no_random_scheduler() {
    let base = hand::edit(
        &hand::source("rsrc-global"),
        "verify safety no_resource_overcommit",
        "",
    );
    let source = hand::edit(
        &base,
        "  bounds",
        "  simulation replications 2 seed 1 confidence 0.95\n  bounds",
    );
    let m = lowered(&source);
    let e = engine::simulate(&m);
    let r = reference::simulate(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e, r.outcome);
    assert_eq!(e.completed_replications, 2);
    let v = serde_json::to_value(report::simulate(&source)).unwrap();
    for (key, value) in [
        ("occupancy_time", "4"),
        ("peak_units", "1"),
        ("utilization", "2/5"),
    ] {
        assert_eq!(
            v["metrics"]["resources"][0][key],
            json!({"mean":value,"min":value,"max":value,"samples":2,"reason":null})
        );
    }
    let source = hand::edit(&source, "policy fifo tie_by case_id", "");
    let e = engine::simulate(&lowered(&source));
    assert_eq!(e.completed_replications, 0);
    assert!(e.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
}
#[test]
fn explain_deadlock_has_exact_resource_shortage_and_independent_verified_replay() {
    let s = hand::source("rsrc-over-demand.broken-too-much");
    let v = serde_json::to_value(explain::explain(&s)).unwrap();
    assert_eq!(v["verdict"], "violated");
    let c = v["explanation"]["counterexamples"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "diagnostic:W-DEADLOCK-0")
        .unwrap();
    assert_eq!(c["replay"]["status"], "verified");
    let d = &c["steps"].as_array().unwrap().last().unwrap()["deadlock_detail"];
    assert_eq!(
        d["shared_resources"],
        json!([{"resource":"R","capacity":1,"occupied":0,"holders":[]}])
    );
    assert_eq!(
        d["waiting_tasks"][0]["resource_shortages"],
        json!([{"resource":"R","required":2,"available":1,"missing":1}])
    );
    assert_eq!(d["resources"][0]["actor"], "A");
}
