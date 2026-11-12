//! Private acceptance adapters. Hand data remain in the frozen test inventory.
use crate::{comparison_projection, engine, ir, lower, quality, reference, syntax};
use serde_json::{Value, json};

// This entire module is cfg(test). Only the key immediately before each real
// lookup is replaced; inventory, lookup, abort and public wrappers stay real.
// Thread-local scope and Drop keep parallel tests and unwinding isolated.
struct OutcomeLookupProbe {
    site: &'static str,
    key: (&'static str, Option<&'static str>),
    calls: usize,
}
thread_local! {
    static OUTCOME_LOOKUP_PROBE: std::cell::RefCell<Option<OutcomeLookupProbe>> = const {
        std::cell::RefCell::new(None)
    };
}
pub(crate) fn outcome_lookup_key(
    site: &'static str,
    original: (&'static str, Option<String>),
) -> (&'static str, Option<String>) {
    OUTCOME_LOOKUP_PROBE.with_borrow_mut(|probe| {
        if let Some(p) = probe.as_mut().filter(|p| p.site == site) {
            p.calls += 1;
            (p.key.0, p.key.1.map(str::to_owned))
        } else {
            original
        }
    })
}
struct OutcomeLookupScope;
impl Drop for OutcomeLookupScope {
    fn drop(&mut self) {
        OUTCOME_LOOKUP_PROBE.with_borrow_mut(|p| *p = None);
    }
}
fn lookup_probe(site: &'static str, missing: bool) {
    // One deterministic one-minute task: end.done occurs once per replication,
    // Hit=1, N=D=1, Rate=1 for four replications. H2 does not cut this path.
    // The legacy inventory has no cut_off; the H2 inventory has no cut_off_missing.
    let scope = if site == "engine_legacy" {
        "bounds { cases 1; live_tasks 1; clock 5min } simulation replications 4 seed 1 confidence 0.5"
    } else {
        "scenario Day { arrivals fixed [0min]; horizon 2min; bounds { cases 1; live_tasks 1; clock 5min }; simulation replications 4 seed 1 confidence 0.5 }"
    };
    let src = format!(
        r#"model Lookup version "0.2" {{
type T = enum {{ ok }}
actor A {{ capacity 1; capability run() -> T {{ duration fixed 1min; result nondet {{ ok }} }} }}
process P {{ r = A.run(); end done }}
cut C {{ include [A] }}
property case.Hit = reached(end.done)
metric cohort.Rate = expected(count(case.Hit)) / arrivals
verify reachable end.done
{scope}
}}"#
    );
    let m = lower::lower_with_selection(
        &syntax::parse(&src).unwrap(),
        None,
        (site != "engine_legacy").then_some("Day"),
    )
    .unwrap();
    assert_eq!(m.legacy_route(), site == "engine_legacy");
    let key = if missing {
        (
            if site == "engine_legacy" {
                "cut_off"
            } else {
                "cut_off_missing"
            },
            None,
        )
    } else {
        ("end", Some("done"))
    };
    OUTCOME_LOOKUP_PROBE.with_borrow_mut(|p| {
        assert!(p.is_none());
        *p = Some(OutcomeLookupProbe {
            site,
            key,
            calls: 0,
        });
    });
    let _scope = OutcomeLookupScope;
    let out = if site == "reference" {
        let run = reference::simulate(&m);
        assert_eq!(
            run.stop,
            if missing {
                reference::StopReason::SemanticAbort
            } else {
                reference::StopReason::Complete
            }
        );
        run.outcome
    } else {
        engine::simulate(&m)
    };
    assert_eq!(
        OUTCOME_LOOKUP_PROBE.with_borrow(|p| p.as_ref().unwrap().calls),
        if missing { 1 } else { 4 },
        "the intended real lookup must have run"
    );
    let q = json!(out.quality.unwrap());
    if missing {
        assert!(
            out.diagnostics
                .iter()
                .any(|d| d.code == crate::diag::U_SEM_0)
        );
        assert!(out.replications.is_empty());
        assert_eq!(q["properties"][0]["expected_count"]["reason"], "aborted");
        assert_eq!(q["properties"][0]["expected_count"]["samples"], 0);
        assert_eq!(q["properties"][0]["cases"][0]["trials"], 0);
        assert_eq!(q["cohort"][0]["value"]["reason"], "aborted");
        assert_eq!(q["cohort"][0]["value"]["samples"], 0);
        assert!(q["cohort"][0]["value"]["point"].is_null());
        for row in q["outcomes_per_arrival"].as_array().unwrap() {
            assert_eq!(row["rate"]["reason"], "aborted");
            assert_eq!(row["rate"]["samples"], 0);
        }
    } else {
        assert!(
            !out.diagnostics
                .iter()
                .any(|d| d.severity == crate::diag::Severity::Error)
        );
        assert_eq!(out.replications.len(), 4);
        assert_eq!(q["properties"][0]["cases"][0]["hits"], 4);
        assert_eq!(q["properties"][0]["cases"][0]["unknown"], 0);
        assert_eq!(q["cohort"][0]["value"]["point"], "1");
        assert_eq!(q["cohort"][0]["value"]["samples"], 4);
        let done = &q["outcomes_per_arrival"][0];
        assert_eq!(done["kind"], "end");
        assert_eq!(done["end"], "done");
        assert_eq!(done["expected_count"]["mean"], "1");
        assert_eq!(done["rate"]["point"], "1");
    }
}
#[test]
fn outcome_lookup_engine_arrivals_known_key() {
    lookup_probe("engine_arrivals", false);
}
#[test]
fn outcome_lookup_engine_arrivals_unknown_key_aborts_without_panic() {
    lookup_probe("engine_arrivals", true);
}
#[test]
fn outcome_lookup_engine_legacy_known_key() {
    lookup_probe("engine_legacy", false);
}
#[test]
fn outcome_lookup_engine_legacy_unknown_key_aborts_without_panic() {
    lookup_probe("engine_legacy", true);
}
#[test]
fn outcome_lookup_reference_known_key() {
    lookup_probe("reference", false);
}
#[test]
fn outcome_lookup_reference_unknown_key_aborts_without_panic() {
    lookup_probe("reference", true);
}

fn source(n: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(n)
            .join("model.relca"),
    )
    .unwrap()
}
fn model(n: &str) -> ir::Model {
    lower::lower(&syntax::parse(&source(n)).unwrap()).unwrap()
}

#[test]
fn qp05_both_public_ir_entrances_refuse_all_invalid_quality_declarations() {
    let base = model("qual-base");
    for fault in 0..15 {
        let mut m = base.clone();
        let q = m.quality.as_mut().unwrap();
        match fault {
            0 => {
                q.properties.clear();
                q.metrics.clear();
            }
            1 => q.properties.push(q.properties[0].clone()),
            2 => q.properties[0].name = "received".into(),
            3 => q.properties[0].name = "admitted".into(),
            4 => q.properties[0].name = "id".into(),
            5 => q.metrics.push(q.metrics[0].clone()),
            6 => q.properties[0].target = ir::QualityTarget::Ends(vec![]),
            7 => q.properties[0].target = ir::QualityTarget::Ends(vec![ir::EndId(0), ir::EndId(0)]),
            8 => q.properties[0].target = ir::QualityTarget::Ends(vec![ir::EndId(m.ends.len())]),
            9 => q.metrics[0].numerator = q.properties.len(),
            10 => q.metrics[0].denominator = ir::Denominator::Property(q.properties.len()),
            11 => {
                q.properties[0].target =
                    ir::QualityTarget::State(ir::Cond::StateEq(ir::StateMatch {
                        variable: ir::StateVarId(m.state_variables.len()),
                        value: ir::StateValue::None,
                    }))
            }
            12 => {
                q.properties[0].target =
                    ir::QualityTarget::State(ir::Cond::StateEq(ir::StateMatch {
                        variable: ir::StateVarId(0),
                        value: ir::StateValue::Integer(1),
                    }))
            }
            13 => {
                q.properties[0].target = ir::QualityTarget::State(ir::Cond::Eq(ir::ResultMatch {
                    binding: ir::BindingId(0),
                    value: ir::ValueId(0),
                }))
            }
            14 => {
                q.metrics.clear();
                q.properties[1].target = ir::QualityTarget::State(ir::Cond::Or(vec![]));
            }
            _ => unreachable!(),
        }
        let e = engine::explore(&m);
        assert!(
            e.diagnostics.iter().any(|d| d.code == crate::diag::U_SEM_0),
            "engine fault {fault}: {e:?}"
        );
        let r = reference::explore(&m);
        assert_eq!(
            r.stop,
            reference::StopReason::SemanticAbort,
            "reference fault {fault}"
        );
        assert!(
            r.outcome
                .diagnostics
                .iter()
                .any(|d| d.code == crate::diag::U_SEM_0)
        );
    }
    let mut cut = model("qual-cut-state");
    let actor = cut
        .actors
        .iter()
        .position(|a| a.name == "Observer")
        .unwrap();
    cut.cut.as_mut().unwrap().include.retain(|a| a.0 != actor);
    assert!(
        engine::explore(&cut)
            .diagnostics
            .iter()
            .any(|d| d.code == crate::diag::U_SEM_0)
    );
    assert_eq!(
        reference::explore(&cut).stop,
        reference::StopReason::SemanticAbort
    );
}

#[test]
fn qp07_each_outcome_quality_is_first_checked_against_hand_json() {
    let inventory: Value =
        serde_json::from_str(include_str!("../tests/support/quality_inventory.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut checked = 0;
    for n in crate::quality_population::NAMES {
        let row = &inventory[n];
        if row["code"].is_string() {
            continue;
        }
        let ast = syntax::parse(&source(n)).unwrap();
        let selections = if n.starts_with("qual-compare") || n.starts_with("qual-diff") {
            vec![Some("Check"), Some("Fast")]
        } else if n.starts_with("qual-invoice") {
            vec![Some("AsIs")]
        } else {
            vec![None]
        };
        let expected: Value =
            serde_json::from_slice(&std::fs::read(root.join(n).join("expected.json")).unwrap())
                .unwrap();
        for cfg in selections {
            let m = match cfg {
                Some(c) => lower::lower_configuration(&ast, c).unwrap().model,
                None => lower::lower(&ast).unwrap(),
            };
            let want = cfg
                .and_then(|c| row["plans"].get(c))
                .or_else(|| expected.get("quality"));
            if let Some(want) = want {
                let sim = row["invocation"]["command"] == "simulate"
                    || row["invocation"]["mode"] == "simulation";
                let (e, r) = if sim {
                    (
                        engine::simulate(&m).quality,
                        reference::simulate(&m).outcome.quality,
                    )
                } else {
                    (
                        engine::explore(&m).quality,
                        reference::explore(&m).outcome.quality,
                    )
                };
                assert_eq!(
                    serde_json::to_value(&e).unwrap(),
                    *want,
                    "engine hand {n}/{cfg:?}"
                );
                assert_eq!(
                    serde_json::to_value(&r).unwrap(),
                    *want,
                    "reference hand {n}/{cfg:?}"
                );
                assert_eq!(e, r);
                checked += 1;
            }
        }
    }
    assert!(checked >= 70, "hand checks {checked}");
}

#[test]
fn qp08_wire_tampering_and_forged_comparison_inventory_fail_closed() {
    let m = model("qual-base");
    let base = engine::explore(&m).quality.unwrap();
    assert!(comparison_projection::quality_rows(Some(&base), &m, false).is_ok());
    for fault in 0..25 {
        let mut q = base.clone();
        match fault {
            0 => {
                q.properties.pop();
            }
            1 => q.properties.push(q.properties[0].clone()),
            2 => q.properties.swap(0, 1),
            3 => {
                q.properties[0]["cases"].as_array_mut().unwrap().pop();
            }
            4 => {
                let c = q.properties[0]["cases"][0].clone();
                q.properties[0]["cases"].as_array_mut().unwrap().push(c);
            }
            5 => q.properties[0]["cases"].as_array_mut().unwrap().swap(0, 1),
            6 => q.properties[0]["cases"][0]["case_id"] = json!(7),
            7 => q.properties[0]["cases"][0]["arrival"] = json!(7),
            8 => q.properties[0]["condition"] = json!("false"),
            9 => q.properties[0]["within"] = json!(7),
            10 => q.analysis = "simulation".into(),
            11 => q.properties[0]["expected_count"]["basis"] = json!("fabricated"),
            12 => q.properties[0]["expected_count"]["reason"] = json!("state_condition"),
            13 => q.properties[0]["expected_count"]["lower"] = json!("2/50"),
            14 => {
                q.properties[0]["expected_count"]
                    .as_object_mut()
                    .unwrap()
                    .remove("reason");
            }
            15 => {
                q.cohort.pop();
            }
            16 => q.cohort.push(q.cohort[0].clone()),
            17 => q.cohort.swap(0, 1),
            18 => q.cohort[0]["denominator"]["count"] = json!(1),
            19 => {
                q.outcomes_per_arrival.pop();
            }
            20 => q
                .outcomes_per_arrival
                .push(q.outcomes_per_arrival[0].clone()),
            21 => q.outcomes_per_arrival.swap(0, 1),
            22 => q.outcomes_per_arrival[0]["extra"] = json!(0),
            23 => q.cohort[0]["value"]["lower"] = Value::Null,
            24 => q.checked.push("fake".into()),
            _ => unreachable!(),
        }
        assert!(
            comparison_projection::quality_rows(Some(&q), &m, false).is_err(),
            "accepted corruption {fault}"
        );
    }
    let mut none = m.clone();
    none.quality = None;
    assert!(comparison_projection::quality_rows(Some(&base), &none, false).is_err());
    assert!(comparison_projection::quality_rows(None, &m, false).is_err());
    let mut raw = serde_json::to_value(&base).unwrap();
    raw["cohort"][0]
        .as_object_mut()
        .unwrap()
        .remove("bounds_with_undetermined");
    assert!(serde_json::from_value::<quality::Quality>(raw).is_err());
    let wire = serde_json::to_string(&base).unwrap();
    assert!(wire.contains("\"case_id\":1"));
    let duplicate = wire.replacen("\"case_id\":1", "\"case_id\":1,\"case_id\":1", 1);
    assert!(serde_json::from_str::<quality::Quality>(&duplicate).is_err());
    assert_eq!(
        serde_json::from_str::<quality::Quality>(&wire).unwrap(),
        base
    );
}

#[test]
fn qp10_empty_and_refused_samples_are_withdrawn_in_both_aggregators() {
    let m = model("qual-base");
    for abort in [false, true] {
        let e = quality::simulation(&m, &[], abort);
        let r = reference::quality_empty(&m, abort);
        assert_eq!(e, r);
        assert!(e.checked.is_empty());
        let reason = if abort { "aborted" } else { "no_samples" };
        for p in &e.properties {
            assert_eq!(p["expected_count"]["reason"], reason);
            assert_eq!(p["unknown_count"]["samples"], 0);
        }
        assert_eq!(e.cohort[0]["value"]["reason"], reason);
        assert_eq!(e.cohort[0]["value"]["point"], Value::Null);
    }
    let s = source("qual-sim").replace("verify reachable end.posted", "verify terminating end");
    let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
    assert_eq!(
        engine::simulate(&m).quality.as_ref().unwrap().cohort[0]["value"]["reason"],
        "aborted"
    );
    // Reference's structural refusal is also observable even before a trial.
    let r = reference::simulate(&m);
    assert_eq!(
        r.outcome.quality.as_ref().unwrap().cohort[0]["value"]["reason"],
        "aborted"
    );
}

#[test]
fn closed_quality_deadlines_survive_range_stops_without_changing_old_verdicts() {
    let src=source("int-upper.broken-overflow").replace("  verify reachable end.done", "  simulation replications 2 seed 1 confidence 0.5\n  property case.Early = reached(n == 0) within 0min\n  property case.Later = reached(n == 0)\n  metric cohort.EarlyRate = expected(count(case.Early)) / arrivals\n  metric cohort.LaterRate = expected(count(case.Later)) / arrivals\n  verify reachable end.done");
    for multi in [false, true] {
        let src = if multi {
            src.replace("  bounds", "  arrivals fixed [0min]\n  bounds")
        } else {
            src.clone()
        };
        let m = lower::lower(&syntax::parse(&src).unwrap()).unwrap();
        let e = engine::simulate(&m);
        let r = reference::simulate(&m);
        assert_eq!(e.quality, r.outcome.quality);
        let q = e.quality.unwrap();
        assert_eq!(q.cohort[0]["value"]["point"], "0");
        assert_eq!(q.cohort[1]["value"]["reason"], "truncated");
        assert!(!e.range_stops.is_empty());
        let checked = crate::report::check(&src);
        assert_eq!(checked.verdict, crate::Verdict::Violated);
    }
}

#[test]
fn alternate_captured_report_entrances_preserve_quality_serialization_bytes() {
    for name in [
        "qual-above-one",
        "qual-legacy-route",
        "qual-base",
        "qual-post-end-state",
    ] {
        let src = source(name);
        let normal = crate::report::check(&src);
        let captured = crate::explain::explain(&src).report;
        assert!(normal.quality.is_some(), "{name}");
        assert_eq!(
            serde_json::to_vec(&normal.quality).unwrap(),
            serde_json::to_vec(&captured.quality).unwrap(),
            "quality byte identity {name}"
        );
    }
}

/// Mutation survivor O03: the exhaustive aggregator under a computation abort
/// withdraws every value as aborted, the arrivals denominator included (an
/// aborted case is not removed from the denominator, so no zero_denominator).
/// No public input aborts an exhaustive quality run, so the aggregator is
/// called as the engine's abort path calls it. Hand JSON for qual-base.
#[test]
fn o03_exhaustive_abort_withdraws_every_value_including_the_arrivals_ratio() {
    use crate::probability::Rational;
    let m = model("qual-base");
    let s = quality::Solution::new(
        [
            Rational::zero(),
            Rational::one(),
            Rational::zero(),
            Rational::one(),
        ],
        true,
    );
    let q = quality::exhaustive(
        &m,
        &vec![vec![s.clone(); 2]; 2],
        &vec![vec![s; 2]; quality::outcome_keys(&m).len()],
        true,
    );
    let aborted = json!({"lower":null,"upper":null,"basis":null,"reason":"aborted"});
    let cases = json!([
        {"case_id":1,"arrival":0,"min":null,"max":null,"reason":"aborted"},
        {"case_id":2,"arrival":10,"min":null,"max":null,"reason":"aborted"}
    ]);
    let outcome = |kind: &str, end: Value| json!({"kind":kind,"end":end,"expected_count":aborted,"rate":aborted});
    let want = json!({
        "rules":"quality-cohort-p1-1",
        "analysis":"exhaustive",
        "properties":[
            {"name":"Wrong","condition":"registered == true && correct == false","within":null,
             "cases":cases,"expected_count":aborted},
            {"name":"Registered","condition":"registered == true","within":null,
             "cases":cases,"expected_count":aborted}
        ],
        "cohort":[
            {"name":"WrongPerArrival","form":"ratio_of_expected_counts","numerator":"Wrong",
             "denominator":{"kind":"arrivals","count":2},"value":aborted,
             "bounds_with_undetermined":null},
            {"name":"WrongPerRegistered","form":"ratio_of_expected_counts","numerator":"Wrong",
             "denominator":{"kind":"property","property":"Registered","covers":null,"reason":"state_condition"},
             "value":aborted,"bounds_with_undetermined":null}
        ],
        "outcomes_per_arrival":[
            outcome("end", json!("returned")),
            outcome("end", json!("posted")),
            outcome("rejected", Value::Null),
            outcome("unfinished", Value::Null),
            outcome("deadlock", Value::Null)
        ],
        "checked":[],
        "unchecked":["expectation_of_ratio","cohort_any_probability","metric_intervals",
            "metric_constraints","numerator_subset_of_denominator","business_value","rework_loss",
            "deadline_attainment","case_reachability","expected_counts","ratio_of_expected_counts",
            "denominator_disclosure","outcomes_per_arrival"]
    });
    assert_eq!(serde_json::to_value(&q).unwrap(), want);
}
