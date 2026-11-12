//! Hand-authored report refusal obligations, NOT connected production coverage.
//! Link this from report's cfg(test) child module.
//! Do not copy report.rs into a test or expose a new public analysis API.
#![allow(dead_code)]
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    SimulationLoweredError,
    SimulationAggregateOverflow,
    AnalyzeActorMetadataError,
    AnalyzeLoweredError,
    AnalyzeNoVerify,
    AnalyzeOutcomeWithoutMetrics,
    SimulationOutcomeWithoutMetrics,
}

pub struct Probe {
    pub route: Route,
    pub source: String,
    pub quality: bool,
    pub rule: bool,
    pub waits: bool,
    pub revocable: bool,
}

/// First lower source normally, then inject ONLY the named private defect:
/// - SimulationLoweredError: push E-CAP-0/error into model.diagnostics after lower
///   succeeds; enter simulate_pipeline's defense (940166ea report.rs:828).
/// - SimulationAggregateOverflow: set replications=u64::MAX and cases=2 AFTER
///   lower; enter the product guard (:847), without allocating cases or sampling.
/// - AnalyzeActorMetadataError: set A.metadata.control=Some("not-a-control");
///   enter analyze_model's actor_table::validate refusal (:1459).
/// - AnalyzeLoweredError: same single diagnostic as above, analyze_model (:1489).
/// - AnalyzeNoVerify: clear model.verify after lower; analyze_model (:1502).
/// - AnalyzeOutcomeWithoutMetrics: run engine::explore once, clear only metrics
///   on its Outcome, return it through the real check/analyze/apply_outcome path.
/// - SimulationOutcomeWithoutMetrics: run engine::simulate once, clear only
///   metric_template and each replication.metrics; real simulate_pipeline tail.
///
/// The last two inject producer absence, NOT a fabricated success/failure.
/// Return {report: actual serialized Report/SimulationReport, executions: n}.
/// Count real explore/simulate calls; no engine call is allowed for guard routes.
pub fn probes() -> Vec<Probe> {
    let routes = [
        Route::SimulationLoweredError,
        Route::SimulationAggregateOverflow,
        Route::AnalyzeActorMetadataError,
        Route::AnalyzeLoweredError,
        Route::AnalyzeNoVerify,
        Route::AnalyzeOutcomeWithoutMetrics,
        Route::SimulationOutcomeWithoutMetrics,
    ];
    let mut out = Vec::new();
    for route in routes {
        for quality in [false, true] {
            for rule in [false, true] {
                for waits in [false, true] {
                    let source = format!(
                        r#"model RefusalPrivate version "0.2" {{
type T = enum {{ ok }}
people [p, q, r]
actor A {{ executors [q]; capability run() -> T {{ duration fixed 2min; result nondet {{ ok }} }} }}
{}
authority G {{ basis "record only"; grants q to A.run; issued_by r; revocable_by p }}
process P {{ a = A.run(); end done }}
cut Scope {{ include [{}] }}
bounds {{ cases 1; live_tasks 1; clock 20min }}
metrics {{ window 20min; currency "JPY"; {} }}
{}
verify reachable end.done
simulation replications 4 seed 1 confidence 0.5
}}"#,
                        if rule {
                            "actor Rule { version \"r1\"; decides pause: bool = true }"
                        } else {
                            ""
                        },
                        if rule { "A, Rule" } else { "A" },
                        if waits {
                            "breakdown waits"
                        } else {
                            "               "
                        },
                        if quality {
                            "property case.Done = reached(end.done)\nmetric cohort.DoneRate = expected(count(case.Done)) / arrivals"
                        } else {
                            ""
                        }
                    );
                    for revocable in [false, true] {
                        out.push(Probe {
                            route,
                            source: if revocable {
                                source.clone()
                            } else {
                                source.replace("revocable_by p", "              ")
                            },
                            quality,
                            rule,
                            waits,
                            revocable,
                        });
                    }
                }
            }
        }
    }
    out
}

pub fn run(mut execute: impl FnMut(&Probe) -> Value) {
    for p in probes() {
        let observation = execute(&p);
        let v = &observation["report"];
        let sim = matches!(
            p.route,
            Route::SimulationLoweredError
                | Route::SimulationAggregateOverflow
                | Route::SimulationOutcomeWithoutMetrics
        );
        let calls = u64::from(matches!(
            p.route,
            Route::AnalyzeOutcomeWithoutMetrics | Route::SimulationOutcomeWithoutMetrics
        ));
        assert_eq!(
            observation["executions"], calls,
            "{:?}: execution guard",
            p.route
        );
        assert!(
            v.get("metrics").is_none(),
            "{:?}: no producer metrics",
            p.route
        );
        let fs: Vec<_> = v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["field"].as_str().unwrap())
            .collect();
        assert!(
            !fs.contains(&"metrics.waiting"),
            "{:?}: waiting without metrics: {fs:?}",
            p.route
        );
        if p.quality {
            assert!(
                v.get("quality").is_some(),
                "{:?}: retain refusal quality",
                p.route
            );
            let want = vec![
                "quality.definition",
                "quality.denominator",
                "quality.form",
                if sim {
                    "quality.estimation"
                } else {
                    "quality.scheduler"
                },
            ];
            let actual: Vec<_> = fs
                .iter()
                .copied()
                .filter(|f| f.starts_with("quality."))
                .collect();
            assert_eq!(
                actual, want,
                "{:?}: complete ordered quality refusal",
                p.route
            );
            let first = fs.iter().position(|f| *f == "quality.definition").unwrap();
            assert_eq!(
                &fs[first..first + want.len()],
                want.as_slice(),
                "{:?}: adjacent block",
                p.route
            );
        } else {
            assert!(v.get("quality").is_none());
            assert!(fs.iter().all(|f| !f.starts_with("quality.")));
        }
        if calls == 0 {
            let (verdict, code, message) = match p.route {
                Route::SimulationLoweredError | Route::AnalyzeLoweredError => {
                    ("invalid", "E-CAP-0", "a7fix injected lowered error")
                }
                Route::SimulationAggregateOverflow => {
                    ("unsupported", "U-SEM-0", "aggregate counts exceed")
                }
                Route::AnalyzeActorMetadataError => (
                    "unsupported",
                    "U-SEM-0",
                    "invalid Actor declaration metadata",
                ),
                Route::AnalyzeNoVerify => ("unsupported", "U-SEM-0", "No verification properties"),
                _ => unreachable!(),
            };
            assert_eq!(v["verdict"], verdict);
            assert!(
                v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == code && d["message"].as_str().unwrap().contains(message)),
                "{:?}: actual guard diagnostic",
                p.route
            );
        }
    }
}
