//! Synthetic observations, not outcomes captured from either interpreter.
#![allow(dead_code)]
use super::core::{diag::Span, ir, metrics::contests::Table, outcome::*};
use serde_json::{Value, json};
pub fn absent(reason: &str) -> Value {
    json!({"mean":null,"min":null,"max":null,"samples":0,"reason":reason})
}
fn sample(n: u64, x: u64) -> Value {
    json!({"mean":x.to_string(),"min":x.to_string(),"max":x.to_string(),"samples":n,"reason":null})
}
pub fn wire(kind: &str) -> Value {
    let mut r = json!({"id":"contract Controls #1 contest","case_id":1,"party":"Party","target":"done","route":"Reply.reject",
        "configured_probability":"1","occurrence_basis":"assumed occurrence","service_time_basis":"assumed service","independence":"per_case_per_clause",
        "cohort":"normal_response_observed","trials":2,"target_observed":2,"target_not_reached":0,"target_unknown":0,
        "filed":2,"not_filed":0,"choice_unknown":0,"responded":2,"late_responded":0,"unresolved":0,"route_failed":0,
        "filing_fraction":{"value":"1","numerator":2,"denominator":2,"reason":null},
        "service_time":sample(2,2),"queue_wait":sample(2,0),"response_elapsed":sample(2,2)});
    match kind {
        "known" => (),
        "unresolved" => {
            r["responded"] = json!(1);
            r["unresolved"] = json!(1);
        }
        "failed" => {
            r["responded"] = json!(0);
            r["route_failed"] = json!(2);
        }
        "target_unknown" => {
            r["target_observed"] = json!(1);
            r["target_unknown"] = json!(1);
            r["filed"] = json!(1);
            r["responded"] = json!(1);
            r["filing_fraction"] =
                json!({"value":null,"numerator":1,"denominator":1,"reason":"unfinished"});
        }
        "choice_unknown" => {
            r["choice_unknown"] = json!(1);
            r["filed"] = json!(1);
            r["responded"] = json!(1);
            r["filing_fraction"] =
                json!({"value":null,"numerator":1,"denominator":2,"reason":"unfinished"});
        }
        "zero" => {
            for k in ["target_observed", "filed", "responded"] {
                r[k] = json!(0);
            }
            r["target_not_reached"] = json!(2);
            r["filing_fraction"] =
                json!({"value":null,"numerator":0,"denominator":0,"reason":"no_samples"});
        }
        "aborted" => {
            for k in [
                "target_observed",
                "target_not_reached",
                "target_unknown",
                "filed",
                "not_filed",
                "choice_unknown",
                "responded",
                "late_responded",
                "unresolved",
                "route_failed",
            ] {
                r[k] = Value::Null;
            }
            r["filing_fraction"] =
                json!({"value":null,"numerator":null,"denominator":null,"reason":"aborted"});
        }
        _ => panic!("unknown hand wire"),
    }
    if kind != "known" {
        let why = match kind {
            "zero" | "failed" => "no_samples",
            "aborted" => "aborted",
            _ => "unfinished",
        };
        for k in ["service_time", "queue_wait", "response_elapsed"] {
            r[k] = absent(why);
        }
    }
    json!({"rules":"contest-rates-sample-p2-1","basis":"sample","unit":"replication","scope":"declared-model-not-party-representation","rows":[r]})
}
pub fn decode(v: Value, ok: bool) {
    let r = serde_json::from_value::<Table>(v.clone());
    assert_eq!(r.is_ok(), ok, "{r:?}; {v}");
    if let Ok(r) = r {
        assert_eq!(json!(r), v);
    }
}
pub fn history(kind: &str, reverse: bool) -> SimulationOutcome {
    let obs = |contest, response| ContestObservation {
        contest,
        case_id: 1,
        target_known: true,
        target_at: Some(1),
        filed: Some(true),
        started_at: Some(1),
        response_at: response,
        closed_at: response,
        failed: false,
    };
    let mut out = SimulationOutcome {
        attempted_replications: 2,
        completed_replications: 2,
        truncated_replications: Some(1),
        ..SimulationOutcome::default()
    };
    for i in 0..2 {
        let mut a = obs(
            0,
            if i == 1 || kind == "known" {
                Some(3)
            } else {
                None
            },
        );
        let mut b = obs(1, None);
        if kind == "target" {
            for c in [&mut a, &mut b] {
                if c.response_at.is_none() {
                    c.target_known = false;
                    c.target_at = None;
                    c.filed = None;
                    c.started_at = None;
                }
            }
        }
        out.replications.push(ReplicationOutcome {
            response_recovery: None,
            contests: vec![a, b],
            lost_updates: vec![],
            admission: None,
            scenario_observed_until: None,
            metrics: None,
            replication: i + 1,
            draws: vec![],
            cases: vec![],
            cut: (i == 1).then_some(SimulationCut::Clock),
            overcommit: false,
        });
    }
    if reverse {
        out.replications.reverse();
        for (i, r) in out.replications.iter_mut().enumerate() {
            r.replication = i as u64 + 1;
        }
    }
    let range_rep = if reverse { 2 } else { 1 };
    out.range_stops.push((
        range_rep,
        vec![StateRangeViolation {
            case_id: Some(1),
            time: 2,
            binding: "a".into(),
            actor: "A".into(),
            capability: "run".into(),
            phase: "finish".into(),
            variable: "n".into(),
            operator: "+=".into(),
            before: "1".into(),
            operand: "1".into(),
            attempted: "2".into(),
            lower: "0".into(),
            upper: "1".into(),
            write_span: Span { start: 0, end: 1 },
            declaration_span: Span { start: 0, end: 1 },
        }],
    ));
    out
}
pub fn reducer(
    mut summarize: impl FnMut(&ir::Model, &SimulationOutcome) -> Value,
    m: &ir::Model,
    kind: &str,
) {
    for reverse in [false, true] {
        let t = summarize(m, &history(kind, reverse));
        decode(t.clone(), true);
        let rs = t["rows"].as_array().unwrap();
        assert_eq!(rs.len(), 2);
        for (i, route, reason) in [
            (
                0,
                "Reply.reject",
                if kind == "known" {
                    None
                } else {
                    Some("unfinished")
                },
            ),
            (1, "Reply.break_range", Some("truncated")),
        ] {
            let r = &rs[i];
            assert_eq!(r["route"], route);
            assert_eq!(r["trials"], 2);
            assert_eq!(
                r["target_unknown"],
                if kind == "target" {
                    json!(if i == 0 { 1 } else { 2 })
                } else {
                    json!(0)
                }
            );
            assert_eq!(
                r["unresolved"],
                if kind == "target" {
                    json!(0)
                } else {
                    json!(if i == 0 {
                        u64::from(kind != "known")
                    } else {
                        2
                    })
                }
            );
            assert_eq!(r["route_failed"], 0);
            for (key, x) in [
                ("service_time", 2),
                ("queue_wait", 0),
                ("response_elapsed", 2),
            ] {
                assert_eq!(
                    r[key],
                    match reason {
                        Some(why) => absent(why),
                        None => sample(2, x),
                    },
                    "{kind}/{reverse}/{route}/{key}"
                );
            }
            if kind == "target" {
                assert_eq!(
                    r["filing_fraction"],
                    json!({"value":null,"numerator":if i==0 {1}else{0},"denominator":if i==0 {1}else{0},"reason":reason})
                );
            } else {
                assert_eq!(
                    r["filing_fraction"],
                    json!({"value":"1","numerator":2,"denominator":2,"reason":null})
                );
            }
        }
    }
}
