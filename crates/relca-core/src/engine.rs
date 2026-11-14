//! Exhaustive interpreter for finite models.
//! 1. Inventory every loop/state producer, observer/completion edge and condition arm.
//! 2. Bound ack enumeration by clock plus one late representative before cloning.
//! 3. Keep late-ack states alive for other tasks' in-bound finishes; cut only on tick.
//! 4. Preserve checked arithmetic and the public IR/explore/Outcome contracts.
//! 5. Share the run loop with an initial-state injection seam for overcommit tests.
//! 6. Test finish/ack result vectors through repeated step, not only complete.
//! 7. Test And/Or truth tables and false nested operands through explore.
//! 8. Calibrate an observer fault, restore it, then run focused/clippy/check.sh.
//!
//! Loop/range/clone/Cond inventory.
//! Defect classes: eager expansion of time outside clock; helper-only tests of
//! run/step wiring; boolean tests lacking false inputs. Scope: this file only.
//!
//! | Occurrence | Disposition / evidence target |
//! | --- | --- |
//! | request_cancel ack range | Fixed: clamp before cloning, retain one late ack. |
//! | step start / next_time | No range defect: one fixed finish per task; tick checks clock before advancing. |
//! | adopt winners / 0..tasks.len | Not a time/value range: binding/task indices; delegates all ack expansion to request_cancel. |
//! | complete due / results | Not a numeric interval: explicit due indices / declared enum values at current time. |
//! | compile / task creation | Not a numeric interval: finite IR statements/calls. |
//! | run pending / visited | Not a numeric interval: transition frontier; tested observer edge via injected initial state. |
//! | occupancy / observe / validation | No State generation; actor/task/capability scans only. |
//! | step due -> complete | Test fixed: repeated step retains all nine finish/ack result vectors. |
//! | condition Eq | Existing explore tests cover equality and unavailable results. |
//! | condition And / Or | Test fixed: four truth rows each, plus nested false operands through explore. |
//! | cfg(test) ranges / state clones | Test setup only; finite task/end indices or explicit states, not production expansion. |
//!
//! The inventory covers the listed source sites only, not other components.
//! No public contract change or
//! conflict with ADR 0001 cancellation/bound semantics is intended.
//!
//! Calibration: temporarily removed exploration.observe from the shared run loop.
//! overcommit_observer_rejects_broken_acquisition_even_with_a_cut_elsewhere failed
//! (exit 101, Inconclusive instead of Violated); observation was then restored.

use crate::{
    Verdict, diag, ir, outcome,
    probability::{ProbabilityRange, Rational},
};
use std::collections::{BTreeMap, BTreeSet, HashSet};
mod arrivals;
#[cfg(test)]
pub(crate) use arrivals::q6_private as q6_observe;
mod authority;
#[cfg(test)]
#[path = "../tests/support/authority_population.rs"]
pub(crate) mod authority_population;
#[cfg(test)]
mod authority_tests;
#[cfg(test)]
mod calendar_frontier_tests;
#[cfg(test)]
mod completion_impl_tests;
pub(crate) mod evidence;
#[cfg(test)]
#[path = "../tests/support/limits_population.rs"]
pub(crate) mod limits_population;
mod metrics;

#[cfg(test)]
pub(crate) fn waits_review_merge(points: Vec<crate::metrics::BusinessMetrics>) {
    let mut accumulated = None;
    for point in points {
        metrics::merge(&mut accumulated, point);
    }
}
mod policy;
#[cfg(test)]
#[path = "../tests/support/policy_population.rs"]
pub(crate) mod policy_population;
#[cfg(test)]
mod req_impl_tests;
mod simulation;
#[cfg(test)]
#[path = "../tests/support/state_invariant_private_state.rs"]
mod state_invariant_private_state;
#[cfg(test)]
#[path = "engine/state_invariant_tests.rs"]
mod state_invariant_tests;
pub(crate) mod trace;
mod work_profiles;

#[cfg(test)]
pub(crate) fn requires_fixture(name: &str) -> bool {
    const NAMES: [&str; 32] = [
        "req-after-queue",
        "req-after-queue.broken-close",
        "req-agency",
        "req-agency.broken-remove-guard",
        "req-bypass",
        "req-bypass.broken-conjunction",
        "req-cancel-immediate",
        "req-cancel-immediate.broken-cooperative",
        "req-explicit-else",
        "req-explicit-else.broken-no-else",
        "req-kernel",
        "req-kernel.broken-enable",
        "req-no-capacity",
        "req-no-capacity.broken-zero",
        "req-observed",
        "req-observed.broken-latent",
        "req-same-time",
        "req-same-time.broken-no-close",
        "req-start",
        "req-start-kept",
        "req-start-kept.broken-forbidden",
        "req-start-siblings",
        "req-start-siblings.broken-same-target",
        "req-start.broken-false",
        "req-wait-admission",
        "req-wait-admission.broken-small-queue",
        "req-wait-arrivals",
        "req-wait-arrivals.broken-no-wait",
        "req-wait-clock",
        "req-wait-clock.broken-short-clock",
        "req-wait-gap",
        "req-wait-gap.broken-zero",
    ];
    assert!(
        !name.starts_with("req-") || NAMES.contains(&name),
        "unclassified requires fixture {name}"
    );
    NAMES.contains(&name)
}

/// ADR 0016 sampled execution, separate from exhaustive graph construction.
pub fn simulate(model: &ir::Model) -> outcome::SimulationOutcome {
    #[cfg(test)]
    crate::evaluation_spy::hit(4);
    let mut out = simulation::simulate(model);
    if model.quality.is_some()
        && validate(model).is_ok()
        && (!out
            .diagnostics
            .iter()
            .all(|d| d.severity != diag::Severity::Error)
            || out.quality.is_none())
    {
        out.quality = Some(crate::quality::simulation(model, &[], true));
    }
    for r in &mut out.replications {
        if let Some(metrics) = &mut r.metrics {
            metrics.apply_deferred(model);
        }
    }
    if let Some(metrics) = &mut out.metric_template {
        metrics.apply_deferred(model);
    }
    if let Some(quality) = &mut out.quality {
        quality.apply_deferred(model);
    }
    out
}

#[cfg(test)]
const SIMULATION_FIXTURES: [&str; 24] = [
    "sim-actor-initial",
    "sim-actor-initial.broken-missing-distribution",
    "sim-contract",
    "sim-contract.broken-short-response",
    "sim-cut",
    "sim-cut.broken-short-clock",
    "sim-deadlock",
    "sim-deadlock.broken-zero-capacity",
    "sim-fifo",
    "sim-fifo.broken-no-policy",
    "sim-hazard",
    "sim-hazard.broken-short-clock",
    "sim-keys",
    "sim-keys.broken-flipped-query",
    "sim-range",
    "sim-range.broken-zero-replications",
    "sim-reachable",
    "sim-reachable.broken-unseen-end",
    "sim-rejection",
    "sim-rejection.broken-positive-queue",
    "sim-required",
    "sim-required.broken-missing-simulation",
    "sim-statistical",
    "sim-statistical.broken-tight-threshold",
];

#[cfg(test)]
const ARRIVALS_FIXTURES: [&str; 24] = [
    "arrivals-simultaneous",
    "arrivals-simultaneous.broken-no-policy",
    "arrivals-staggered",
    "arrivals-staggered.broken-extra-capacity",
    "arrivals-reject-mass",
    "arrivals-reject-mass.broken-larger-queue",
    "arrivals-same-time-slot",
    "arrivals-same-time-slot.broken-later-arrival",
    "arrivals-initial-product",
    "arrivals-initial-product.broken-fixed-initial",
    "arrivals-relative-within",
    "arrivals-relative-within.broken-tight-deadline",
    "arrivals-count",
    "arrivals-count.broken-case-count",
    "arrivals-order",
    "arrivals-order.broken-descending",
    "arrivals-required",
    "arrivals-required.broken-missing-arrivals",
    "arrivals-shared-fixed",
    "arrivals-shared-fixed.broken-unset-shared",
    "arrivals-contracts",
    "arrivals-contracts.broken-short-response",
    "arrivals-zero-queue",
    "arrivals-zero-queue.broken-positive-queue",
];

#[cfg(test)]
mod arrivals_legacy_duration_census {
    use super::*;

    // Frozen before changing semantics. Debug is the complete, ordered Outcome
    // projection, including diagnostics and exact rational endpoints.
    fn projection(out: &outcome::Outcome) -> String {
        compatibility_outcome_debug(out)
    }

    #[test]
    fn arrivals_legacy_duration_census() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut dirs: Vec<_> = std::fs::read_dir(root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("duration-")
            })
            .collect();
        dirs.sort();
        assert_eq!(dirs.len(), 18);
        let mut rows = Vec::new();
        for dir in dirs {
            let name = dir.file_name().unwrap().to_str().unwrap();
            let source = std::fs::read_to_string(dir.join("model.relca")).unwrap();
            let ast = crate::syntax::parse(&source).unwrap();
            let Ok(model) = crate::lower::lower(&ast) else {
                continue;
            };
            let mut evidence = Exploration::default();
            let (nodes, _) =
                probability_graph(&model, DEFAULT_STATE_BUDGET, &mut evidence).unwrap();
            let (_, generated, held) = evidence.census;
            let budgets: Vec<_> = (0..=held + 1)
                .map(|b| format!("{b}: {}", projection(&explore_with_budget(&model, b))))
                .collect();
            let fingerprint = budgets
                .join("\n")
                .bytes()
                .fold(0xcbf29ce484222325u64, |h, b| {
                    (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
                });
            rows.push(format!(
                "{name} {} {generated} {held} {fingerprint:016x}",
                nodes.len()
            ));
        }
        assert_eq!(rows.len(), 16);
        let actual = rows.join("\n");
        println!("ARRIVALS-DURATION-CENSUS production a58041c, observer only\n{actual}");
        // Measured on a58041c with instrumentation only, not hand-computed.
        const BASELINE: &str = "duration-cancel 17 18 18 e267ead73172bca5
duration-cancel.broken-ignore-cancel 21 21 22 e6371e44c41b44ea
duration-clock 8 7 8 c2538014bd862d08
duration-clock.broken-short-clock 3 2 3 2c394db855fb0498
duration-contention 48 52 49 d74a683070703d17
duration-contention.broken-extra-capacity 28 31 29 a381e4b33f7c3e1e
duration-duplicate 12 11 12 82b174257647356b
duration-no-lookahead 38 42 40 bb3229f14c4eaf86
duration-no-lookahead.broken-late-choice 41 46 43 7430b2d44927449f
duration-parallel 28 31 29 284f1f71514c05cd
duration-parallel.broken-tight-upper 28 31 29 5e67f03d81c24807
duration-serial 32 32 32 5bc186719ea50259
duration-serial.broken-tight-upper 32 32 32 256ee2f1dc5730af
duration-sum 12 11 12 95d40b12badfbfab
duration-zero 7 6 7 1b60225d593b9c18
duration-zero.broken-late-positive 7 6 7 58eb7c179d2a40a6";
        assert_eq!(actual, BASELINE);
    }
}

#[cfg(test)]
fn compatibility_outcome_debug(out: &outcome::Outcome) -> String {
    assert!(out.case_outcomes.is_none() && out.admission.is_none());
    assert!(out.properties.iter().all(|p| p.cases.is_none()));
    format!("{out:?}")
        .replace("metrics: None, ", "")
        .replace("case_outcomes: None, ", "")
        .replace("admission: None, ", "")
        .replace("cases: None, ", "")
}

#[cfg(test)]
fn legacy_outcome_debug(out: outcome::Outcome) -> String {
    assert!(!out.probability_scheduler);
    assert!(out.properties.iter().all(|p| p.probability.is_none()));
    assert!(out.terminals.iter().all(|p| p.probability.is_none()));
    // Project only newly added fields. No prior field, byte, or value is changed.
    compatibility_outcome_debug(&out)
        .replace("probability_scheduler: false, ", "")
        .replace("probability: None, ", "")
}

#[cfg(test)]
mod state_census {
    use super::*;
    #[test]
    fn p_legacy_state_census() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut dirs: Vec<_> = std::fs::read_dir(root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("state-")
            })
            .collect();
        dirs.sort();
        let mut rows = Vec::new();
        let mut accepted = 0;
        for dir in dirs {
            let name = dir.file_name().unwrap().to_str().unwrap();
            let source = std::fs::read_to_string(dir.join("model.relca")).unwrap();
            let ast = crate::syntax::parse(&source).unwrap();
            let Ok(model) = crate::lower::lower(&ast) else {
                continue;
            };
            accepted += 1;
            let mut evidence = Exploration::default();
            explore_into(
                &model,
                true,
                DEFAULT_STATE_BUDGET,
                std::convert::identity,
                &mut evidence,
            )
            .unwrap();
            let (visited, generated, held) = evidence.census;
            let budgets: Vec<_> = (0..=held + 1)
                .map(|b| legacy_outcome_debug(explore_with_budget(&model, b)))
                .collect();
            let fingerprint = budgets
                .join("\n")
                .bytes()
                .fold(0xcbf29ce484222325u64, |h, b| {
                    (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
                });
            rows.push(format!(
                "{name} {visited} {generated} {held} {fingerprint:016x}"
            ));
        }
        assert_eq!(accepted, 13);
        let actual = rows.join("\n");
        println!("STATE-CENSUS measured on 5f17dcf before probability semantics\n{actual}");
        // Measured, not a hand oracle: production at 5f17dcf before ADR 0013.
        const BASELINE: &str = "state-agency 20 19 20 fca2e0405c4a920e
state-agency.broken-state-guard 20 19 20 fca2e0405c4a920e
state-cancel 16 16 16 4f272c0226dca538
state-cancel.broken-ignore-cancel 16 16 16 84ae499d67a91ed2
state-invoice-write 11 10 11 62c6b747cbefd083
state-invoice-write.broken-failed-approval 11 10 11 cf77b382cb9c0acd
state-latent-product 25 24 25 7e752ef0cb438a81
state-latent-product.broken-partial-assertion 25 24 25 a9f2832cf09ffac7
state-name-alias 7 6 7 2fa09d9146b2cfef
state-observed-range 20 19 20 7a5adb3c8cfd8193
state-observed-range.broken-fixed-route 20 19 20 48de95fd897cfdd3
state-policy 7 6 7 8424c1b336d8d282
state-writer-order 16 15 16 ac114f02b95e2ae7";
        assert_eq!(actual, BASELINE);
    }
}

#[cfg(test)]
mod probability_tests {
    use super::*;
    const SUM: &str = include_str!("../../../fixtures/prob-sum/model.relca");
    fn model(s: &str) -> ir::Model {
        crate::lower::lower(&crate::syntax::parse(s).unwrap()).unwrap()
    }
    fn range(m: &ir::Model) -> ProbabilityRange {
        explore(m)
            .properties
            .into_iter()
            .find_map(|p| p.probability)
            .unwrap()
    }

    #[test]
    fn p_private_initial_product_and_simultaneous_action_product() {
        let initial = model(
            r#"model M version "0.2" { type T=enum { a,b }
            latent case { x:T ~ { a:0.25,b:0.75 } } actor D { latent y:bool ~ { true:0.4,false:0.6 } }
            process P { end done } bounds { cases 1;live_tasks 1;clock 1min }
            verify probability reached x == a && D.y == true <= 1 }"#,
        );
        let mut program = vec![Instruction::Stop];
        let entry = compile(&initial.process.body, 0, &mut program);
        let mut evidence = Exploration::default();
        let mut room = Room(4);
        let choices = probability_initial(
            &State::initial(&initial, entry),
            &initial,
            &mut room,
            &mut evidence,
        )
        .unwrap();
        assert_eq!(room.0, 0);
        assert_eq!(evidence.initial_clones, 4);
        let mut weights: Vec<_> = choices.iter().map(|(p, _)| p.to_string()).collect();
        weights.sort();
        assert_eq!(weights, ["1/10", "3/10", "3/20", "9/20"]);
        assert_eq!(
            evidence
                .state_events
                .iter()
                .filter(|(_, b, _)| matches!(b, Bundle::Received))
                .count(),
            4
        );
        let m = model(
            r#"model M version "0.2" { type T=enum { ok } observed case { a:bool=false;b:bool=false }
            kernel K { * -> { 0.5:ok with a=true;0.5:ok with a=false } }
            kernel L { * -> { 0.25:ok with b=true;0.75:ok with b=false } }
            actor A { capacity 2;capability first() -> T { duration fixed 1min;outcomes kernel K;writes a } capability second() -> T { duration fixed 1min;outcomes kernel L;writes b } }
            process P { fork f { x=A.first();y=A.second() };join all f;end done }
            bounds { cases 1;live_tasks 2;clock 2min } verify probability reached a == true && b == true <= 0.125 }"#,
        );
        let mut e = Exploration::default();
        let (nodes, _) = probability_graph(&m, 10000, &mut e).unwrap();
        assert!(
            nodes
                .iter()
                .any(|n| n.actions.len() == 2 && n.actions.iter().all(|a| a.len() == 2))
        );
        assert_eq!(
            range(&m),
            ProbabilityRange {
                min: Rational::decimal("0.125").unwrap(),
                max: Rational::decimal("0.125").unwrap()
            }
        );
        let source = nodes
            .iter()
            .find(|n| {
                n.state.time == 1
                    && n.state
                        .tasks
                        .iter()
                        .filter(|t| matches!(t.lifecycle, Lifecycle::Running { .. }))
                        .count()
                        == 2
            })
            .unwrap();
        let mut evidence = Exploration::default();
        let result = step_observed(
            source.state.as_ref().clone(),
            &{
                let mut program = vec![Instruction::Stop];
                compile(&m.process.body, 0, &mut program);
                program
            },
            &m,
            &mut Room(2),
            &mut evidence,
        );
        assert!(matches!(result, Err(Halt::Budget)));
        assert_eq!(
            evidence.partial_successors.len(),
            2,
            "first completed action survives refusal of second action"
        );
        let xs = std::mem::take(&mut evidence.partial_successors);
        let actions = probability_actions(xs, &mut evidence).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].len(), 2);
        let mut partial = vec![source.clone()];
        partial[0].actions.clear();
        partial[0].unknown = true;
        partial[0].targets = vec![false];
        let mut edges = Vec::new();
        for (p, s) in actions.into_iter().next().unwrap() {
            let hit = s.state_values[0] == ir::StateValue::Value(ir::ValueId(0));
            let id = partial.len();
            edges.push((p, id));
            partial.push(ProbabilityNode {
                cut_reason: ProbabilityCut::Pending,
                numeric: false,
                state: std::rc::Rc::new(s),
                chance_pending: false,
                actions: Vec::new(),
                targets: vec![hit],
                end: None,
                unknown: true,
            });
        }
        partial[0].actions.push(edges);
        let order = probability_topology(&partial).unwrap();
        let b = solve_probability(&partial, &order, |n| n.targets[0], 4096);
        assert_eq!(b.0.to_string(), "0");
        assert_eq!(b.2.to_string(), "1/2");
        assert_eq!(b.3.to_string(), "1");
    }

    #[test]
    fn p_initial_order_receipt_and_reserve_before_clone() {
        let m = model(
            r#"model M version "0.2" {
            latent case { x:bool ~ { true:0.5,false:0.5 }; y:bool }
            process P { end done } bounds { cases 1;clock 1min;live_tasks 1 }
            verify probability reached (x == true && y == true) || (x == false && y == false) <= 1
        }"#,
        );
        let mut program = vec![Instruction::Stop];
        let entry = compile(&m.process.body, 0, &mut program);
        let seed = State::initial(&m, entry);
        let mut e = Exploration::default();
        assert!(matches!(
            probability_initial(&seed, &m, &mut Room(1), &mut e),
            Err(Halt::Budget)
        ));
        assert_eq!(e.initial_clones, 0);
        assert!(e.state_events.is_empty());
        let states = probability_initial(&seed, &m, &mut Room(2), &mut e).unwrap();
        assert_eq!(states.len(), 2);
        assert!(e.state_events.is_empty());
        for (_, s) in states {
            assert!(!s.initialized);
            let successors = initialize_latents(&s, &m, &mut Room(2), &mut e).unwrap();
            assert_eq!(successors.len(), 2);
        }
        assert_eq!(
            e.state_events
                .iter()
                .filter(|(_, b, _)| matches!(b, Bundle::Received))
                .count(),
            4
        );
        assert_eq!(
            range(&m),
            ProbabilityRange {
                min: Rational::zero(),
                max: Rational::one()
            }
        );
        for b in [0, 1] {
            let out = explore_with_budget(&m, b);
            assert!(out.truncated);
            let r = out.properties[0].probability.as_ref().unwrap();
            assert_eq!(r.min, Rational::zero());
            assert_eq!(r.max, Rational::one());
        }
        let zero = model(
            &SUM.replace("0.25: ok", "0: ok")
                .replace("0.75: ok", "1: ok"),
        );
        assert_eq!(range(&zero).max, Rational::zero());
    }

    #[test]
    fn p_actions_merge_and_transient_absorption() {
        let s = SUM.replace(
            "0.25: ok with hit = true; 0.75: ok with hit = false",
            "0.2: ok with hit = true; 0.3: ok with hit = true; 0.5: ok with hit = false",
        );
        let m = model(&s);
        let mut e = Exploration::default();
        let (nodes, _) = probability_graph(&m, 1000, &mut e).unwrap();
        let chance = nodes
            .iter()
            .flat_map(|n| &n.actions)
            .find(|a| a.len() == 2)
            .unwrap();
        assert!(chance.iter().all(|(p, _)| p.to_string() == "1/2"));
        assert_ne!(chance[0].1, chance[1].1);
        assert_eq!(e.kernel_clones, 3);
        assert_eq!(e.kernel_draws, 1);
        assert_eq!(range(&m).min.to_string(), "1/2");
        let reset = s.replace("process Flow","actor Reset { capacity 1; capability clear() -> T { duration fixed 1min; result nondet { ok }; writes hit; finish { hit = false } } }; process Flow").replace("if hit == true { end bad } else { end good }","r = Reset.clear(); end done");
        assert_eq!(range(&model(&reset)).min.to_string(), "1/2");
        // Two distinct scheduler choices to the same future are not summed.
        let mut synthetic = nodes;
        let branch = synthetic
            .iter_mut()
            .find(|n| n.actions.iter().any(|a| a.len() == 2))
            .unwrap();
        branch.actions.push(branch.actions[0].clone());
        let order = topological_for_test(&synthetic);
        let b = solve_probability(&synthetic, &order, |n| n.targets[0], 4096);
        assert_eq!(b.0.to_string(), "1/2");
        assert_eq!(b.3.to_string(), "1/2");
    }
    fn topological_for_test(nodes: &[ProbabilityNode]) -> Vec<usize> {
        fn visit(i: usize, n: &[ProbabilityNode], seen: &mut HashSet<usize>, out: &mut Vec<usize>) {
            if !seen.insert(i) {
                return;
            }
            for a in &n[i].actions {
                for (_, j) in a {
                    visit(*j, n, seen, out);
                }
            }
            out.push(i);
        }
        let mut out = Vec::new();
        visit(0, nodes, &mut HashSet::new(), &mut out);
        out.reverse();
        out
    }

    #[test]
    fn p_partial_room_zero_and_cancel_draw() {
        let m = model(&SUM.replace("0.25: ok with hit = true; 0.75: ok with hit = false","0.2: ok with hit = true; 0.3: ok with hit = true; 0.5: ok with hit = false; 0: ok with hit = false"));
        let mut e = Exploration::default();
        let (nodes, _) = probability_graph(&m, 1000, &mut e).unwrap();
        let source = nodes
            .iter()
            .find(|n| {
                n.state.time == 1
                    && n.state
                        .tasks
                        .iter()
                        .any(|t| matches!(t.lifecycle, Lifecycle::Running { finish: 1, .. }))
            })
            .unwrap();
        for (room, clones) in [(2, 0), (3, 3)] {
            let mut e = Exploration::default();
            let result = kernel_complete(
                &source.state,
                0,
                m.capabilities[0].kernel.as_ref().unwrap(),
                &m,
                &mut Room(room),
                &mut e,
            );
            assert_eq!(e.kernel_clones, clones);
            assert_eq!(result.is_ok(), room == 3);
        }
        let cancel = model(include_str!("../../../fixtures/prob-cancel/model.relca"));
        let mut e = Exploration::default();
        probability_graph(&cancel, 10000, &mut e).unwrap();
        assert_eq!(e.kernel_draws, 0);
        assert_eq!(e.kernel_clones, 0);
        let forbidden = model(include_str!(
            "../../../fixtures/prob-cancel.broken-ignore-cancel/model.relca"
        ));
        let mut e = Exploration::default();
        probability_graph(&forbidden, 10000, &mut e).unwrap();
        assert!(e.kernel_draws > 0);
        let tied = model(
            &include_str!("../../../fixtures/prob-cancel/model.relca")
                .replace("fixed 5min", "fixed 2min"),
        );
        assert_eq!(
            range(&tied),
            ProbabilityRange {
                min: Rational::zero(),
                max: Rational::decimal("0.5").unwrap()
            }
        );
    }

    pub(super) fn chain(n: usize) -> String {
        let p = format!("0.{}1", "0".repeat(62));
        let q = format!("0.{}", "9".repeat(63));
        let mut body = "z = A.mark(); end success".to_owned();
        for i in (0..n).rev() {
            body = format!("r{i} = A.toss(); if r{i} == pass {{ {body} }} else {{ end failed }}");
        }
        format!(
            r#"model M version "0.2" {{ type T = enum {{ pass,fail }}
            observed case {{ done:bool = false }}
            kernel K {{ * -> {{ {p}:pass; {q}:fail }} }}
            actor A {{ capacity 1;
                capability toss() -> T {{ duration fixed 1min; outcomes kernel K }}
                capability mark() -> T {{ duration fixed 1min; result nondet {{ pass }}; writes done; finish {{ done = true }} }} }}
            process P {{ {body} }} bounds {{ cases 1;clock 50min;live_tasks 1 }}
            verify safety no_resource_overcommit
            verify probability reached done == true <= 1 }}"#
        )
    }
    #[test]
    fn p_denominator_exact_19_cut_20_and_small_injected_limit() {
        for n in [19, 20] {
            let out = explore(&model(&chain(n)));
            let p = &out.properties[1];
            if n == 19 {
                assert_eq!(
                    p.probability.as_ref().unwrap().min.to_string(),
                    format!("1/1{}", "0".repeat(1197))
                );
                assert_eq!(p.verdict, Verdict::Satisfied);
                assert!(!out.truncated);
            } else {
                assert_eq!(p.verdict, Verdict::Inconclusive);
                assert!(out.truncated);
                assert_eq!(out.properties[0].verdict, Verdict::Inconclusive);
                assert_eq!(
                    out.diagnostics
                        .iter()
                        .filter(|d| d.code == diag::W_BUDGET_0)
                        .count(),
                    1
                );
            }
        }
        let m = model(SUM);
        let mut e = Exploration::default();
        let (nodes, _) = probability_graph(&m, 1000, &mut e).unwrap();
        let order = topological_for_test(&nodes);
        assert!(!solve_probability(&nodes, &order, |n| n.targets[0], 3).4);
        assert!(solve_probability(&nodes, &order, |n| n.targets[0], 2).4);
        let b = solve_probability(&nodes, &order, |_| true, 1);
        assert!(!b.4);
        assert_eq!(b.0, Rational::one());
    }

    #[test]
    fn p_zero_time_kernel_bundle_and_old_debug_projection() {
        let s = include_str!("../../../fixtures/prob-zero.broken-positive-write/model.relca")
            .replace("fixed 1min", "fixed 0min");
        let out = explore(&model(&s));
        assert_eq!(out.terminals[0].elapsed.min, 0);
        assert_eq!(
            out.properties
                .iter()
                .find_map(|p| p.probability.as_ref())
                .unwrap()
                .max,
            Rational::one()
        );
        assert!(
            out.properties
                .iter()
                .any(|p| matches!(p.property, ir::Property::Contract(_))
                    && p.verdict == Verdict::Violated)
        );
        assert_eq!(
            legacy_outcome_debug(outcome::Outcome {
                response_exercise: None,
                response_receipts: None,
                response_recovery: None,
                scenario_observed_until: None,
                quality: None,
                state_range_violations: Vec::new(),
                capability_refusals: None,
                unchecked: Vec::new(),
                postprocessing_diagnostics: Vec::new(),
                contest_runtime: None,
                contest_summary: None,
                obligations: None,
                metrics: None,
                case_outcomes: None,
                admission: None,
                probability_scheduler: false,
                probability_policy_unresolved: None,
                properties: Vec::new(),
                terminals: Vec::new(),
                truncated: false,
                diagnostics: Vec::new()
            }),
            "Outcome { properties: [], terminals: [], truncated: false, diagnostics: [] }"
        );
    }

    #[test]
    fn p_malformed_ir_and_acyclic_graph() {
        let base = model(SUM);
        let faults: &[fn(&mut ir::Model)] = &[
            |m| m.probability_queries[0].bound = Rational::decimal("1.1").unwrap(),
            |m| m.verify[1].property = ir::Property::ProbabilityReached(999),
            |m| {
                m.probability_queries[0].condition = ir::Cond::Eq(ir::ResultMatch {
                    binding: ir::BindingId(0),
                    value: ir::ValueId(0),
                })
                .into()
            },
            |m| {
                m.capabilities[0]
                    .kernel
                    .as_mut()
                    .unwrap()
                    .given
                    .push(ir::StateVarId(999))
            },
            |m| m.capabilities[0].kernel.as_mut().unwrap().rows.clear(),
            |m| {
                let k = m.capabilities[0].kernel.as_mut().unwrap();
                k.rows.push(k.rows[0].clone());
            },
            |m| {
                m.capabilities[0].kernel.as_mut().unwrap().rows[0].branches[0].probability =
                    Rational::decimal("-0.1").unwrap()
            },
            |m| {
                m.capabilities[0].kernel.as_mut().unwrap().rows[0].branches[0].result =
                    ir::ValueId(999)
            },
            |m| {
                m.capabilities[0].kernel.as_mut().unwrap().rows[0].branches[0].overrides[0].0 =
                    ir::StateVarId(999)
            },
            |m| {
                m.capabilities[0].kernel.as_mut().unwrap().rows[0].branches[0].overrides[0].1 =
                    ir::StateValue::None
            },
            |m| m.capabilities[0].results.clear(),
        ];
        for (i, fault) in faults.iter().enumerate() {
            let mut m = base.clone();
            fault(&mut m);
            let out = explore(&m);
            assert!(
                out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0),
                "{i}: {out:?}"
            );
            assert!(out.terminals.is_empty());
        }
        let mut e = Exploration::default();
        let (nodes, _) = probability_graph(
            &model(
                &include_str!("../../../fixtures/prob-zero/model.relca")
                    .replace("fixed 1min", "fixed 0min"),
            ),
            1000,
            &mut e,
        )
        .unwrap();
        let order = topological_for_test(&nodes);
        let positions: std::collections::HashMap<_, _> =
            order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        for (i, n) in nodes.iter().enumerate() {
            for a in &n.actions {
                for (_, j) in a {
                    assert!(positions[&i] < positions[j]);
                }
            }
        }
        let mut cycle = nodes;
        cycle[0].actions.push(vec![(Rational::one(), 0)]);
        assert!(matches!(
            probability_topology(&cycle),
            Err(Halt::Unsupported(_))
        ));
    }

    #[test]
    fn p_initial_distribution_and_latent_use_ir_matrix_and_empty_ast() {
        let rows = model(include_str!("../../../fixtures/prob-rows/model.relca"));
        let faults: &[fn(&mut ir::Model)] = &[
            |m| m.state_variables[1].initial = ir::InitialValue::Distribution(999),
            |m| m.initial_distributions[0][0].0 = ir::StateValue::None,
            |m| m.initial_distributions[0][0].1 = Rational::decimal("-0.1").unwrap(),
            |m| m.initial_distributions[0][0].1 = Rational::decimal("1.1").unwrap(),
            |m| {
                m.initial_distributions[0].pop();
            },
            |m| m.capabilities[0].kernel_reads_latent.clear(),
            |m| {
                m.capabilities[0]
                    .kernel_reads_latent
                    .push(ir::StateVarId(0))
            },
            |m| m.capabilities[0].kernel.as_mut().unwrap().rows[0].values[0] = ir::StateValue::None,
            |m| {
                m.capabilities[0].kernel.as_mut().unwrap().rows[0].branches[0].probability =
                    Rational::zero()
            },
        ];
        for f in faults {
            let mut m = rows.clone();
            f(&mut m);
            let o = explore(&m);
            assert!(
                o.diagnostics.iter().any(|d| d.code == diag::U_SEM_0),
                "{o:?}"
            );
        }
        let latent = model(
            &SUM.replace("observed case", "latent case")
                .replace("if hit == true { end bad } else { end good }", "end done"),
        );
        let mut missing = latent.clone();
        missing.capabilities[0].kernel.as_mut().unwrap().rows[0].branches[0]
            .overrides
            .clear();
        assert!(validate(&missing).is_err());
        let mut duplicate = latent;
        let b = &mut duplicate.capabilities[0].kernel.as_mut().unwrap().rows[0].branches[0];
        b.overrides.push(b.overrides[0]);
        assert!(validate(&duplicate).is_err());
        for fault in 0..3 {
            let mut ast =
                crate::syntax::parse(include_str!("../../../fixtures/prob-rows/model.relca"))
                    .unwrap();
            for item in &mut ast.items {
                match item {
                    crate::ast::Item::Kernel(k) if fault == 0 => k.rows.clear(),
                    crate::ast::Item::Kernel(k) if fault == 1 => k.rows[0].branches.clear(),
                    crate::ast::Item::Actor(a) if fault == 2 => {
                        for item in &mut a.items {
                            if let crate::ast::ActorItem::Latent(v) = item {
                                v.distribution.as_mut().unwrap().clear();
                            }
                        }
                    }
                    _ => {}
                }
            }
            let ds = crate::lower::lower(&ast).unwrap_err();
            assert!(ds.iter().any(|d| d.code == diag::E_PROB_0));
        }
    }
}

#[cfg(test)]
mod duration_legacy_probability_census {
    use super::*;

    #[test]
    fn duration_legacy_probability_census() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut dirs: Vec<_> = std::fs::read_dir(root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("prob-")
            })
            .collect();
        dirs.sort();
        assert_eq!(dirs.len(), 16);
        let mut rows = Vec::new();
        for dir in dirs {
            let name = dir.file_name().unwrap().to_str().unwrap();
            let source = std::fs::read_to_string(dir.join("model.relca")).unwrap();
            let ast = crate::syntax::parse(&source).unwrap();
            let selections = if name.starts_with("prob-invoice") {
                vec![Some("AsIs"), Some("ToBeP")]
            } else {
                vec![None]
            };
            for selection in selections {
                let model = match selection {
                    Some(s) => crate::lower::lower_configuration(&ast, s).map(|s| s.model),
                    None => crate::lower::lower(&ast),
                };
                let Ok(model) = model else { continue };
                let mut evidence = Exploration::default();
                let (nodes, _) =
                    probability_graph(&model, DEFAULT_STATE_BUDGET, &mut evidence).unwrap();
                let (_, generated, held) = evidence.census;
                // Full Outcome Debug is a deterministic canonical projection: it
                // includes every field, exact rationals and diagnostic spans.
                // Hash ALL budgets, not only complete executions.
                let budgets: Vec<_> = (0..=held + 1)
                    .map(|b| {
                        format!(
                            "{b}: {}",
                            compatibility_outcome_debug(&explore_with_budget(&model, b))
                        )
                    })
                    .collect();
                let fingerprint = budgets
                    .join("\n")
                    .bytes()
                    .fold(0xcbf29ce484222325u64, |h, b| {
                        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
                    });
                rows.push(format!(
                    "{name}/{} {} {generated} {held} {fingerprint:016x}",
                    selection.unwrap_or("-"),
                    nodes.len()
                ));
            }
        }
        assert_eq!(rows.len(), 16);
        let actual = rows.join("\n");
        println!("PROB-CENSUS production de1d5e5, observer only\n{actual}");
        // Measured on de1d5e5 before ANY ADR 0014 production changes.
        // This is a regression census, not a hand-computed semantic oracle.
        const BASELINE: &str = "prob-cancel/- 16 16 17 9d7c2c2f73b63906
prob-cancel.broken-ignore-cancel/- 23 23 24 bee4e776d922470a
prob-cut/- 15 14 15 e8169772c824473d
prob-cut.broken-short-clock/- 12 11 12 edaa9713690d5758
prob-finish-read/- 14 14 15 a9dd39b437f195ec
prob-finish-read.broken-late-writer/- 11 11 12 3b42676d2f76fae4
prob-invoice/AsIs 139 138 139 cf04161fd68e0ea8
prob-invoice/ToBeP 129 132 130 9ee3422f3096463d
prob-invoice.broken-ai-transfer/AsIs 139 138 139 cf04161fd68e0ea8
prob-invoice.broken-ai-transfer/ToBeP 129 132 130 63af5a5b47e14fb1
prob-mdp/- 26 25 26 ac59c7bb21ab27af
prob-mdp.broken-tight-upper/- 26 25 26 5f57b844a98d067b
prob-rows/- 21 20 21 5f75bc6dcda7dbe0
prob-sum/- 10 9 10 fa8602a0b94e2d21
prob-zero/- 7 6 7 8e3d49eff56aba53
prob-zero.broken-positive-write/- 7 6 7 3b9e1c8d05e1969d";
        assert_eq!(actual, BASELINE);
    }
}

#[cfg(test)]
mod duration_tests {
    use super::*;
    use std::hash::{Hash, Hasher};

    fn model(s: &str) -> ir::Model {
        crate::lower::lower(&crate::syntax::parse(s).unwrap()).unwrap()
    }
    fn parallel() -> ir::Model {
        model(
            r#"model M version "0.2" { type T=enum { ok }
        actor A { capacity 2;capability c() -> T { duration {1min:0,3min:0.5,5min:0.5};result nondet { ok } } }
        process P { fork f { x=A.c();y=A.c() };join all f;end done }
        bounds { cases 1;live_tasks 2;clock 10min } verify safety no_resource_overcommit }"#,
        )
    }
    fn at_three(m: &ir::Model) -> (State, Vec<Instruction>) {
        let mut program = vec![Instruction::Stop];
        let pc = compile(&m.process.body, 0, &mut program);
        let mut s = State::initial(m, pc);
        s.waiting = true;
        s.time = 3;
        s.work = 6;
        s.tasks = (0..2)
            .map(|i| Task {
                frozen_performance: None,
                stale: Vec::new(),
                executor: None,
                binding: ir::BindingId(i),
                capability: ir::CapabilityId(0),
                lifecycle: Lifecycle::DistributedRunning {
                    started: 0,
                    next: 0,
                    due: false,
                    ack: None,
                },
            })
            .collect();
        (s, program)
    }

    #[test]
    fn d_room_and_joint_before_choice() {
        let m = parallel();
        let (s, program) = at_three(&m);
        let mut e = Exploration::default();
        assert!(matches!(
            duration_gate(&s, &m, &mut Room(3), &mut e, 4096),
            Err(Halt::Budget)
        ));
        assert_eq!(e.duration_clones, 0);
        assert!(!e.duration_numeric);
        let mut room = Room(4);
        let leaves = duration_gate(&s, &m, &mut room, &mut e, 4096)
            .unwrap()
            .unwrap();
        assert_eq!(room.0, 0);
        assert_eq!(e.duration_clones, 4);
        assert_eq!(leaves.len(), 4);
        let mut flags = BTreeSet::new();
        for (p, leaf) in leaves {
            assert_eq!(p.to_string(), "1/4");
            let bits: Vec<_> = leaf
                .tasks
                .iter()
                .map(|t| match t.lifecycle {
                    Lifecycle::DistributedRunning {
                        due: true, next: 0, ..
                    } => true,
                    Lifecycle::DistributedRunning {
                        due: false,
                        next: 1,
                        ..
                    } => false,
                    _ => panic!("partially observed joint state"),
                })
                .collect();
            flags.insert(bits);
            assert!(
                duration_gate(&leaf, &m, &mut Room(0), &mut e, 4096)
                    .unwrap()
                    .is_none()
            );
            let step = step_observed(leaf, &program, &m, &mut Room(10), &mut e).unwrap();
            assert!(matches!(step, Step::Next(_)));
        }
        assert_eq!(flags.len(), 4);
        // The public probability graph must route the ENTIRE mandatory layer as
        // one action; no ordinary step is reachable between individual coins.
        let mut e = Exploration::default();
        let (nodes, _) = probability_graph(&m, 10000, &mut e).unwrap();
        let n = nodes
            .iter()
            .find(|n| {
                n.state.time == 3
                    && n.state.tasks.iter().all(|t| {
                        matches!(
                            t.lifecycle,
                            Lifecycle::DistributedRunning {
                                next: 0,
                                due: false,
                                ..
                            }
                        )
                    })
            })
            .unwrap();
        assert_eq!(n.actions.len(), 1);
        assert_eq!(n.actions[0].len(), 4);
        for (p, id) in &n.actions[0] {
            assert_eq!(p.to_string(), "1/4");
            assert!(nodes[*id].state.tasks.iter().all(|t| matches!(
                t.lifecycle,
                Lifecycle::DistributedRunning { due: true, .. }
                    | Lifecycle::DistributedRunning { next: 1, .. }
            )));
        }
    }

    #[test]
    fn d_resample_and_age_identity() {
        let m = parallel();
        let (mut s, _) = at_three(&m);
        s.tasks.truncate(1);
        let mut e = Exploration::default();
        let leaves = duration_gate(&s, &m, &mut Room(2), &mut e, 4096)
            .unwrap()
            .unwrap();
        let mut survivor = leaves
            .into_iter()
            .find(|(_, s)| {
                matches!(
                    s.tasks[0].lifecycle,
                    Lifecycle::DistributedRunning {
                        next: 1,
                        due: false,
                        ..
                    }
                )
            })
            .unwrap()
            .1;
        assert_eq!(e.duration_clones, 2);
        // An unrelated completion bundle at this same timestamp cannot redraw.
        emit(
            &mut survivor,
            Bundle::Finish(ir::CapabilityId(0), ir::ValueId(0), None),
            &m,
            &mut e,
        )
        .unwrap();
        assert!(
            duration_gate(&survivor, &m, &mut Room(0), &mut e, 4096)
                .unwrap()
                .is_none()
        );
        assert_eq!(e.duration_clones, 2);
        survivor.time = 5;
        let last = duration_gate(&survivor, &m, &mut Room(1), &mut e, 4096)
            .unwrap()
            .unwrap();
        assert_eq!(last.len(), 1);
        assert_eq!(last[0].0, Rational::one());
        assert!(matches!(
            last[0].1.tasks[0].lifecycle,
            Lifecycle::DistributedRunning { due: true, .. }
        ));
        let hash = |s: &State| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            s.hash(&mut h);
            h.finish()
        };
        for lifecycle in [
            Lifecycle::DistributedRunning {
                started: 1,
                next: 0,
                due: false,
                ack: None,
            },
            Lifecycle::DistributedRunning {
                started: 0,
                next: 1,
                due: false,
                ack: None,
            },
            Lifecycle::DistributedRunning {
                started: 0,
                next: 0,
                due: true,
                ack: None,
            },
        ] {
            let mut changed = s.clone();
            changed.tasks[0].lifecycle = lifecycle;
            assert_ne!(s, changed);
            assert_ne!(hash(&s), hash(&changed));
            assert!(
                duration_gate(&changed, &m, &mut Room(0), &mut e, 4096)
                    .unwrap()
                    .is_none()
            );
        }
        // A failed-to-advance cursor would create a graph cycle, not an exact
        // fixed point. The topology guard is mandatory even for hand graphs.
        let n = ProbabilityNode {
            state: std::rc::Rc::new(s),
            chance_pending: false,
            actions: vec![vec![(Rational::one(), 0)]],
            targets: vec![],
            end: None,
            unknown: false,
            cut_reason: ProbabilityCut::Pending,
            numeric: false,
        };
        assert!(matches!(
            probability_topology(&[n]),
            Err(Halt::Unsupported(_))
        ));
    }

    #[test]
    fn d_numeric_division_joint_and_dp() {
        let p = Rational::decimal("0.1").unwrap();
        let r = Rational::decimal("0.3").unwrap();
        assert!(duration_hazard(&p, &r, 1).is_none());
        assert_eq!(duration_hazard(&p, &r, 2).unwrap().to_string(), "1/3");
        let m = parallel();
        let (s, _) = at_three(&m);
        let mut e = Exploration::default();
        assert!(matches!(
            duration_gate(&s, &m, &mut Room(4), &mut e, 2),
            Err(Halt::Budget)
        ));
        assert!(e.duration_numeric);
        assert_eq!(e.duration_clones, 0);
        let exact = duration_gate(&s, &m, &mut Room(4), &mut e, 3)
            .unwrap()
            .unwrap();
        assert_eq!(exact.len(), 4);
        let node = |target, actions| ProbabilityNode {
            state: std::rc::Rc::new(s.clone()),
            chance_pending: false,
            actions,
            targets: vec![target],
            end: None,
            unknown: false,
            cut_reason: ProbabilityCut::Pending,
            numeric: false,
        };
        let half = Rational::decimal("0.5").unwrap();
        let third = duration_hazard(&p, &r, 2).unwrap();
        let nodes = vec![
            node(false, vec![vec![(half.clone(), 1), (half.clone(), 2)]]),
            node(
                false,
                vec![vec![
                    (third.clone(), 3),
                    (Rational(Rational::one().0 - &third.0), 4),
                ]],
            ),
            node(false, vec![vec![(half.clone(), 3), (half, 4)]]),
            node(true, vec![]),
            node(false, vec![]),
        ];
        let order = probability_topology(&nodes).unwrap();
        let limited = solve_probability(&nodes, &order, |n| n.targets[0], 3);
        assert!(
            limited.4,
            "DP sum 1/6+1/4 has denominator 12, while products fit 3 bits"
        );
        let exact = solve_probability(&nodes, &order, |n| n.targets[0], 4);
        assert!(!exact.4);
        assert_eq!(exact.0.to_string(), "5/12");
        let mut numeric = nodes[4].clone();
        numeric.numeric = true;
        numeric.unknown = true;
        numeric.cut_reason = ProbabilityCut::Clock;
        let b = solve_probability_within(&[numeric], &[0], |_| false, 4096, Some((3, 10)));
        assert!(b.4);
        assert_eq!(
            b.3,
            Rational::one(),
            "clock exemption must not hide numeric cut"
        );
        let mut m = model(include_str!("../../../fixtures/duration-clock/model.relca"));
        let tiny = Rational(num_rational::BigRational::new(
            1.into(),
            (num_bigint::BigInt::from(1u8) << 4096usize) + 1u8,
        ));
        m.capabilities[0]
            .duration_distribution
            .as_mut()
            .unwrap()
            .entries = vec![(1, tiny.clone()), (3, Rational(Rational::one().0 - tiny.0))];
        for q in &mut m.probability_queries {
            q.bound = Rational::one();
            q.within = Some(2);
        }
        let out = explore(&m);
        assert!(out.truncated);
        assert!(
            out.diagnostics
                .iter()
                .any(|d| d.code == diag::W_BUDGET_0 && d.message.contains("denominator"))
        );
        assert!(
            out.properties
                .iter()
                .all(|p| p.verdict == Verdict::Inconclusive),
            "numeric cut overrides even the trivial upper threshold 1"
        );
        m.probability_queries.clear();
        m.verify = vec![ir::Verify {
            property: ir::Property::NoResourceOvercommit,
            span: m.verify[0].span,
        }];
        m.ends.clear();
        m.process
            .body
            .retain(|s| !matches!(s.kind, ir::Stmt::End(_)));
        let out = explore(&m);
        assert!(out.truncated);
        assert_eq!(out.properties[0].verdict, Verdict::Inconclusive);
        assert!(
            out.diagnostics
                .iter()
                .any(|d| d.code == diag::W_BUDGET_0 && d.message.contains("denominator")),
            "duration-only numeric cut must not depend on query or end inventory"
        );
    }

    #[test]
    fn d1_review_kernel_ack_abort_preserves_generated() {
        let original = include_str!("../../../fixtures/duration-cancel/model.relca");
        let prefix = original.rsplit_once('}').unwrap().0;
        let mut m = model(&format!(
            "{prefix} contract C {{ response delivery: Slow.work.finish -> one_of(end.done) by 5min after Slow.work.finish otherwise unresolved }} }}"
        ));
        let mut exploration = Exploration::default();
        let (nodes, _) = probability_graph(&m, 10000, &mut exploration).unwrap();
        let mut state = nodes
            .iter()
            .find(|n| {
                n.state.time == 2
                    && n.state.tasks.iter().any(|t| {
                        matches!(
                            t.lifecycle,
                            Lifecycle::DistributedRunning {
                                due: true,
                                ack: Some(2),
                                ..
                            }
                        )
                    })
            })
            .unwrap()
            .state
            .as_ref()
            .clone();
        let ir::ContractKind::NamedResponse {
            trigger, within, ..
        } = &mut m.contracts[0].kind
        else {
            panic!()
        };
        *trigger = ir::Trigger::Event(ir::Event {
            capability: ir::CapabilityId(1),
            kind: ir::EventKind::CancelAck,
        });
        *within = u64::MAX;
        state.obligations = Some(Vec::new());
        state.monitors = vec![Monitor::default()];
        let mut program = vec![Instruction::Stop];
        compile(&m.process.body, 0, &mut program);
        let mut observed = Exploration::default();
        assert!(step_observed(state, &program, &m, &mut Room(100), &mut observed).is_err());
        assert!(
            !observed.partial_successors.is_empty(),
            "kernel finish successors preceding an aborted ack must survive"
        );
    }

    #[test]
    fn d_ack_kernel_and_maximum_ack_support() {
        let m = model(include_str!(
            "../../../fixtures/duration-cancel/model.relca"
        ));
        let mut e = Exploration::default();
        let (nodes, _) = probability_graph(&m, 10000, &mut e).unwrap();
        let source = nodes
            .iter()
            .find(|n| {
                n.state.time == 2
                    && n.state.tasks.iter().any(|t| {
                        matches!(
                            t.lifecycle,
                            Lifecycle::DistributedRunning {
                                due: false,
                                next: 1,
                                ack: Some(2),
                                ..
                            }
                        )
                    })
            })
            .unwrap();
        let mut program = vec![Instruction::Stop];
        compile(&m.process.body, 0, &mut program);
        let mut ack_e = Exploration::default();
        let Step::Next(acks) = step_observed(
            source.state.as_ref().clone(),
            &program,
            &m,
            &mut Room(10),
            &mut ack_e,
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(acks.len(), 1);
        assert_eq!(ack_e.kernel_draws, 0);
        assert!(ack_e.state_events.is_empty());
        assert_eq!(acks[0].state_values, source.state.state_values);
        let normal = nodes
            .iter()
            .find(|n| {
                n.state.time == 2
                    && n.state.tasks.iter().any(|t| {
                        matches!(
                            t.lifecycle,
                            Lifecycle::DistributedRunning {
                                due: true,
                                ack: Some(2),
                                ..
                            }
                        )
                    })
            })
            .unwrap();
        let mut normal_e = Exploration::default();
        let Step::Next(xs) = step_observed(
            normal.state.as_ref().clone(),
            &program,
            &m,
            &mut Room(10),
            &mut normal_e,
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(xs.len(), 3);
        assert_eq!(normal_e.kernel_draws, 1);
        assert_eq!(normal_e.kernel_clones, 2);
        assert_eq!(normal_e.state_events.len(), 2);
        let mut m = m.clone();
        let slow = m
            .capabilities
            .iter()
            .position(|c| c.duration_distribution.is_some())
            .unwrap();
        m.capabilities[slow]
            .duration_distribution
            .as_mut()
            .unwrap()
            .entries[1]
            .0 = 5;
        m.capabilities[slow].cancel = ir::Cancel::Cooperative { ack_within: 3 };
        let s = nodes
            .iter()
            .find(|n| {
                n.state.time == 1
                    && n.state.tasks.iter().any(|t| {
                        matches!(t.lifecycle, Lifecycle::DistributedRunning { ack: None, .. })
                    })
            })
            .unwrap()
            .state
            .as_ref()
            .clone();
        let i = s.tasks.iter().position(|t| t.capability.0 == slow).unwrap();
        let xs = request_cancel(vec![s], i, &m, &mut Room(2)).unwrap();
        let acks: Vec<_> = xs
            .iter()
            .map(|s| match s.tasks[i].lifecycle {
                Lifecycle::DistributedRunning { ack: Some(a), .. } => a,
                _ => panic!(),
            })
            .collect();
        assert_eq!(acks, [2, 3, 4]);
    }

    #[test]
    fn d_cut_reasons_are_not_interchangeable() {
        let mut m = model(include_str!("../../../fixtures/duration-clock/model.relca"));
        let (clock, _) = probability_graph(&m, 10000, &mut Exploration::default()).unwrap();
        assert!(
            clock
                .iter()
                .any(|n| n.unknown && n.cut_reason == ProbabilityCut::Clock)
        );
        let (budget, _) = probability_graph(&m, 1, &mut Exploration::default()).unwrap();
        assert_eq!(budget.len(), 1);
        assert!(budget[0].unknown);
        assert!(budget[0].cut_reason == ProbabilityCut::StateBudget);
        let (pending, _) = probability_graph(&m, 0, &mut Exploration::default()).unwrap();
        assert!(pending[0].cut_reason == ProbabilityCut::Pending);
        m.bounds.live_tasks = 0;
        let (live, _) = probability_graph(&m, 10000, &mut Exploration::default()).unwrap();
        assert!(
            live.iter()
                .any(|n| n.unknown && n.cut_reason == ProbabilityCut::LiveTasks)
        );
        assert!(!live.iter().any(|n| n.cut_reason == ProbabilityCut::Clock));
    }

    #[test]
    fn d_runtime_distribution_defense() {
        let m = parallel();
        for case in 0..10 {
            let mut bad = m.clone();
            let c = &mut bad.capabilities[0];
            let d = c.duration_distribution.as_mut().unwrap();
            match case {
                0 => d.entries.clear(),
                1 => d.entries.reverse(),
                2 => d.entries[1].0 = d.entries[0].0,
                3 => d.entries[1].1 = Rational::decimal("1.1").unwrap(),
                4 => d.entries[1].1 = Rational::decimal("0.4").unwrap(),
                5 => d.entries = vec![(0, Rational::one())],
                6 => c.duration = 3,
                7 => c.actor = ir::ActorId(999),
                8 => bad.probability_queries.push(ir::ProbabilityQuery {
                    condition: ir::Cond::StateEq(ir::StateMatch {
                        variable: ir::StateVarId(999),
                        value: ir::StateValue::None,
                    })
                    .into(),
                    within: Some(1),
                    comparison: ir::Comparison::Le,
                    bound: Rational::one(),
                }),
                9 => bad.actors[0].capacity = None,
                _ => unreachable!(),
            }
            let out = explore(&bad);
            assert!(
                out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0),
                "case {case}: {out:?}"
            );
        }
    }
}

#[cfg(test)]
mod legacy_census {
    use super::*;
    const EXPLAIN_FIXTURES: [&str; 12] = [
        "explain-invariant",
        "explain-invariant.broken-cut-guard",
        "explain-once",
        "explain-once.broken-repeat-result",
        "explain-response",
        "explain-response.broken-late-finish",
        "explain-requires",
        "explain-requires.broken-cut-guard",
        "explain-executors",
        "explain-executors.broken-shared-worker",
        "explain-deadlock",
        "explain-deadlock.broken-zero-capacity",
    ];
    pub(super) fn explain_fixture(name: &str) -> bool {
        let known = EXPLAIN_FIXTURES.contains(&name);
        assert!(
            !name.starts_with("explain-") || known,
            "unclassified fixture {name}"
        );
        known
    }
    #[test]
    #[should_panic(expected = "unclassified fixture")]
    fn unknown_explain_fixture_is_not_silently_excluded() {
        explain_fixture("explain-unknown");
    }
    const MUTATION_FIXTURES: [&str; 12] = [
        "mut-empty",
        "mut-empty.broken-invalid",
        "mut-evidence",
        "mut-evidence.broken-no-independent-read",
        "mut-gate",
        "mut-gate.broken-unsupported",
        "mut-guard",
        "mut-guard.broken-no-contract",
        "mut-limit",
        "mut-limit.broken-many-sites",
        "mut-origin",
        "mut-origin.broken-route-only",
    ];
    pub(super) fn mutation_fixture(name: &str) -> bool {
        let known = MUTATION_FIXTURES.contains(&name);
        assert!(
            !name.starts_with("mut-") || known,
            "unclassified fixture {name}"
        );
        known
    }
    #[test]
    #[should_panic(expected = "unclassified fixture")]
    fn unknown_mutation_fixture_is_not_silently_excluded() {
        mutation_fixture("mut-unknown");
    }
    const COMPARE_FIXTURES: [&str; 16] = [
        "compare-legacy",
        "compare-legacy.broken-longer-task",
        "compare-arrivals",
        "compare-arrivals.broken-slow-plan",
        "compare-random",
        "compare-random.broken-alias-process",
        "compare-cut",
        "compare-cut.broken-other-cut",
        "compare-terminal",
        "compare-terminal.broken-other-end",
        "compare-unlock",
        "compare-unlock.broken-excluded-call",
        "compare-case-diff",
        "compare-case-diff.broken-same-kernel",
        "compare-reject",
        "compare-reject.broken-zero-queue",
    ];
    pub(super) fn comparison_fixture(name: &str) -> bool {
        let known = COMPARE_FIXTURES.contains(&name);
        assert!(
            !name.starts_with("compare-") || known,
            "unclassified fixture {name}"
        );
        known
    }
    const RESPONSIBILITY_FIXTURES: [&str; 19] = [
        "resp-contest",
        "resp-contest.broken-no-authority",
        "resp-contest.broken-no-permission",
        "resp-contest.broken-zero-deadline",
        "resp-contest.broken-route-excluded",
        "resp-cut",
        "resp-cut.broken-no-explicit-cut",
        "resp-origin",
        "resp-origin.broken-no-direct-read",
        "resp-refusal",
        "resp-refusal.broken-cut-guard",
        "resp-standing",
        "resp-standing.broken-not-party",
        "resp-state",
        "resp-state.broken-no-direct-read",
        "resp-missing-duty",
        "resp-missing-duty.broken-no-duty",
        "resp-diff",
        "resp-diff.broken-no-authorized-decision",
    ];
    const METRICS_FIXTURES: [&str; 24] = [
        "met-serial",
        "met-serial.broken-longer-task",
        "met-parallel",
        "met-parallel.broken-one-slot",
        "met-fifo",
        "met-fifo.broken-shorter-second-step",
        "met-missing-rate",
        "met-missing-rate.broken-free-rate",
        "met-exclusions",
        "met-exclusions.broken-larger-queue",
        "met-positive-support",
        "met-positive-support.broken-positive-early-time",
        "met-sample",
        "met-sample.broken-short-clock",
        "met-compare",
        "met-compare.broken-longer-task",
        "met-diff",
        "met-diff.broken-free-start",
        "met-person",
        "met-person.broken-shared-person",
        "met-cancel",
        "met-cancel.broken-drain",
        "met-legacy",
        "met-legacy.broken-enabled",
    ];
    pub(super) fn metrics_fixture(name: &str) -> bool {
        let known = METRICS_FIXTURES.contains(&name);
        assert!(
            !name.starts_with("met-") || known,
            "unclassified fixture {name}"
        );
        known
    }
    #[test]
    #[should_panic(expected = "unclassified fixture")]
    fn unknown_metrics_fixture_is_not_silently_excluded() {
        metrics_fixture("met-unknown");
    }
    pub(super) fn responsibility_fixture(name: &str) -> bool {
        let known = RESPONSIBILITY_FIXTURES.contains(&name);
        assert!(
            !name.starts_with("resp-") || known,
            "unclassified fixture {name}"
        );
        known
    }
    #[test]
    #[should_panic(expected = "unclassified fixture")]
    fn unknown_responsibility_fixture_is_not_silently_excluded() {
        responsibility_fixture("resp-unknown");
    }
    #[test]
    #[should_panic(expected = "unclassified fixture")]
    fn unknown_comparison_fixture_is_not_silently_excluded() {
        comparison_fixture("compare-unknown");
    }
    #[test]
    fn c_configuration_selection_equivalent_census_20_arrivals() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut accepted = 0;
        for name in ARRIVALS_FIXTURES {
            let source = std::fs::read_to_string(root.join(name).join("model.relca")).unwrap();
            let ast = crate::syntax::parse(&source).unwrap();
            let Ok(before) = crate::lower::lower(&ast) else {
                continue;
            };
            accepted += 1;
            let actors = before
                .actors
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(",");
            let at = source.rfind('}').unwrap();
            let addition = format!(
                "\ncut SelectedCut {{include [{actors}]}}\nconfiguration Selected {{cut SelectedCut;process {}}}\n",
                before.process.name
            );
            let source = format!("{}{}{}", &source[..at], addition, &source[at..]);
            let selected = crate::lower::lower_configuration(
                &crate::syntax::parse(&source).unwrap(),
                "Selected",
            )
            .unwrap();
            let after = selected.model;
            let counts = arrivals::compatibility_counts(&before);
            assert_eq!(arrivals::compatibility_counts(&after), counts, "{name}");
            for budget in 0..=counts.2 + 1 {
                let a = explore_with_budget(&before, budget);
                let b = explore_with_budget(&after, budget);
                assert_eq!(a, b, "{name}/budget={budget}");
            }
        }
        assert_eq!(accepted, 20);
    }
    const DURATION_FIXTURES: [&str; 18] = [
        "duration-cancel",
        "duration-cancel.broken-ignore-cancel",
        "duration-clock",
        "duration-clock.broken-short-clock",
        "duration-contention",
        "duration-contention.broken-extra-capacity",
        "duration-duplicate",
        "duration-duplicate.broken-duplicate-value",
        "duration-no-lookahead",
        "duration-no-lookahead.broken-late-choice",
        "duration-parallel",
        "duration-parallel.broken-tight-upper",
        "duration-serial",
        "duration-serial.broken-tight-upper",
        "duration-sum",
        "duration-sum.broken-mass",
        "duration-zero",
        "duration-zero.broken-late-positive",
    ];
    #[test]
    fn e_legacy_state_identity() {
        const LIN: [&str; 24] = [
            "lin-basis",
            "lin-basis.broken-no-basis",
            "lin-case-latent",
            "lin-case-latent.broken-actor-local",
            "lin-correlation",
            "lin-correlation.broken-unrelated-given",
            "lin-five-kernels",
            "lin-five-kernels.broken-given-only-read",
            "lin-invoice",
            "lin-invoice.broken-po-evidence",
            "lin-no-error-writer",
            "lin-no-error-writer.broken-connect-error-writer",
            "lin-numeric",
            "lin-numeric.broken-zero-detection",
            "lin-same-actor",
            "lin-same-actor.broken-read-removed",
            "lin-same-source-different-target",
            "lin-same-source-different-target.broken-use-same-target",
            "lin-shared-kernel",
            "lin-shared-kernel.broken-drop-call",
            "lin-single",
            "lin-single.broken-second-call",
            "lin-tacit",
            "lin-tacit.broken-unused-tacit",
        ];
        // D1 has its own census; keep the old 124 identities and budget series.
        const D1: [&str; 24] = [
            "ctx-excludes",
            "ctx-excludes.broken-start",
            "ctx-nonwaivable",
            "ctx-nonwaivable.broken-waive",
            "ctx-obligation",
            "ctx-obligation.broken-excludes-target",
            "ctx-waive",
            "ctx-waive-authority",
            "ctx-waive-authority.broken-no-authority",
            "ctx-waive-names",
            "ctx-waive-names.broken-unknown-response",
            "ctx-waive.broken-after-deadline",
            "ctx-contest",
            "ctx-contest-branches",
            "ctx-contest-branches.broken-zero-capacity",
            "ctx-contest-clock",
            "ctx-contest-clock.broken-short-clock",
            "ctx-contest-metrics",
            "ctx-contest-metrics.broken-missing-route-rate",
            "ctx-contest-origin",
            "ctx-contest-origin.broken-arrival-clock",
            "ctx-contest-resource",
            "ctx-contest-resource.broken-slow-route",
            "ctx-contest.broken-late",
        ];
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let all_dirs: Vec<_> = std::fs::read_dir(root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_dir())
            .collect();
        assert_eq!(
            all_dirs
                .iter()
                .filter(|p| ARRIVALS_FIXTURES.contains(&p.file_name().unwrap().to_str().unwrap()))
                .count(),
            24
        );
        assert_eq!(
            all_dirs
                .iter()
                .filter(|p| comparison_fixture(p.file_name().unwrap().to_str().unwrap()))
                .count(),
            16
        );
        assert_eq!(
            all_dirs
                .iter()
                .filter(|p| responsibility_fixture(p.file_name().unwrap().to_str().unwrap()))
                .count(),
            19
        );
        assert_eq!(
            all_dirs
                .iter()
                .filter(|p| metrics_fixture(p.file_name().unwrap().to_str().unwrap()))
                .count(),
            24
        );
        let all_dirs: Vec<_> = all_dirs
            .into_iter()
            .filter(|p| {
                !relca_test_support::fixture_match::contract_scope_fixture(
                    p.file_name().unwrap().to_str().unwrap(),
                )
            })
            .filter(|p| !super::record_fixture(p.file_name().unwrap().to_str().unwrap()))
            .filter(|p| !ir::rule_fixture(p.file_name().unwrap().to_str().unwrap()))
            .filter(|p| {
                !crate::contest::population::fixture(p.file_name().unwrap().to_str().unwrap())
            })
            .filter(|p| {
                let name = p.file_name().unwrap().to_str().unwrap();
                !super::policy_population::fixture(name)
                    && !crate::scenario_population::fixture(name)
                    && !crate::quality_population::fixture(name)
                    && !crate::mutation::extension_fixture(name)
                    && !crate::mutation::review_fixture(name)
                    && !crate::evaluation_population::fixture(name)
                    && !crate::quantile_population::fixture(name)
                    && !include!("../tests/support/acceptance_gap_names.rs").contains(&name)
                    && !crate::state_invariant_population::fixture(name)
                    && !super::limits_population::fixture(name)
            })
            .filter(|p| !ir::resource_fixture(p.file_name().unwrap().to_str().unwrap()))
            .filter(|p| {
                !super::actor_observed_population::fixture(p.file_name().unwrap().to_str().unwrap())
            })
            .filter(|p| {
                !crate::agency::viewpoints_fixture(p.file_name().unwrap().to_str().unwrap())
                    && !crate::agency::second_half_fixture(p.file_name().unwrap().to_str().unwrap())
            })
            .filter(|p| !ir::per_case_fixture(p.file_name().unwrap().to_str().unwrap()))
            .filter(|p| {
                !super::conflict_population::fixture(p.file_name().unwrap().to_str().unwrap())
            })
            .filter(|p| {
                !super::caseset_population::fixture(p.file_name().unwrap().to_str().unwrap())
            })
            .filter(|p| !crate::integer_tests::fixture(p.file_name().unwrap().to_str().unwrap()))
            .filter(|p| {
                !crate::diff::extension_population::fixture(
                    p.file_name().unwrap().to_str().unwrap(),
                )
            })
            .filter(|p| !authority_population::fixture(p.file_name().unwrap().to_str().unwrap()))
            .filter(|p| {
                let name = p.file_name().unwrap().to_str().unwrap();
                assert!(
                    !name.starts_with("lin-") || LIN.contains(&name),
                    "unclassified {name}"
                );
                assert!(
                    !name.starts_with("done-") || crate::completion::FIXTURES.contains(&name),
                    "unclassified completion fixture {name}"
                );
                !LIN.contains(&name)
                    && !{
                        let names: &[&str] = include!("../tests/support/waits_names.rs");
                        names.contains(&name)
                    }
                    && !crate::completion::FIXTURES.contains(&name)
                    && !requires_fixture(name)
            })
            .filter(|p| !crate::actor_table::fixture(p.file_name().unwrap().to_str().unwrap()))
            // ADR 0028 has its own closed 26-runtime / 4-static population.
            .filter(|p| {
                !crate::responsibility::extension_population::fixture(
                    p.file_name().unwrap().to_str().unwrap(),
                )
            })
            .filter(|p| !D1.contains(&p.file_name().unwrap().to_str().unwrap()))
            .filter(|p| !explain_fixture(p.file_name().unwrap().to_str().unwrap()))
            .filter(|p| !mutation_fixture(p.file_name().unwrap().to_str().unwrap()))
            .filter(|p| !metrics_fixture(p.file_name().unwrap().to_str().unwrap()))
            .filter(|p| !responsibility_fixture(p.file_name().unwrap().to_str().unwrap()))
            .filter(|p| !comparison_fixture(p.file_name().unwrap().to_str().unwrap()))
            .filter(|p| !SIMULATION_FIXTURES.contains(&p.file_name().unwrap().to_str().unwrap()))
            .filter(|p| !ARRIVALS_FIXTURES.contains(&p.file_name().unwrap().to_str().unwrap()))
            .collect();
        assert_eq!(all_dirs.len(), 124);
        assert_eq!(
            all_dirs
                .iter()
                .filter(|p| DURATION_FIXTURES.contains(&p.file_name().unwrap().to_str().unwrap()))
                .count(),
            18
        );
        assert_eq!(
            all_dirs
                .iter()
                .filter(|p| !DURATION_FIXTURES.contains(&p.file_name().unwrap().to_str().unwrap()))
                .count(),
            106
        );
        let mut dirs: Vec<_> = all_dirs
            .into_iter()
            .filter(|p| {
                p.is_dir()
                    && !DURATION_FIXTURES.contains(&p.file_name().unwrap().to_str().unwrap())
                    && !p
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("exec-")
                    && !p
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("state-")
                    && !p
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("prob-")
            })
            .collect();
        dirs.sort();
        let mut rows = Vec::new();
        for dir in dirs {
            let name = dir.file_name().unwrap().to_str().unwrap();
            let source = std::fs::read_to_string(dir.join("model.relca")).unwrap();
            let ast = match crate::syntax::parse(&source) {
                Ok(a) => a,
                Err(_) => {
                    rows.push(format!("{name}/default rejected"));
                    continue;
                }
            };
            let inv = dir.join("invocation.json");
            let selections: Vec<Option<String>> = if inv.is_file() {
                let v: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(inv).unwrap()).unwrap();
                if v["command"] == "diff" {
                    vec![
                        Some(v["from"].as_str().unwrap().into()),
                        Some(v["to"].as_str().unwrap().into()),
                    ]
                } else {
                    vec![v["configuration"].as_str().map(str::to_owned)]
                }
            } else {
                vec![None]
            };
            for selection in selections {
                let label = format!("{name}/{}", selection.as_deref().unwrap_or("default"));
                let lowered = match selection.as_deref() {
                    Some(s) => crate::lower::lower_configuration(&ast, s).map(|c| c.model),
                    None => crate::lower::lower(&ast),
                };
                let Ok(model) = lowered else {
                    rows.push(format!("{label} rejected"));
                    continue;
                };
                let mut evidence = Exploration::default();
                explore_into(
                    &model,
                    true,
                    DEFAULT_STATE_BUDGET,
                    std::convert::identity,
                    &mut evidence,
                )
                .unwrap();
                let (visited, generated, held) = evidence.census;
                // Freeze the complete public budget series too, without exporting counters.
                let budgets: Vec<_> = (0..=held + 1)
                    .map(|b| legacy_outcome_debug(explore_with_budget(&model, b)))
                    .collect();
                // Stable byte fingerprint of Debug output (not std's randomized hasher).
                let fingerprint = budgets
                    .join("\n")
                    .bytes()
                    .fold(0xcbf29ce484222325u64, |h, b| {
                        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
                    });
                rows.push(format!(
                    "{label} {visited} {generated} {held} {fingerprint:016x}"
                ));
            }
        }
        let actual = rows.join("\n");
        println!("LEGACY-CENSUS\n{actual}");
        // Measured before ADR 0011 production changes, not a hand oracle.
        const BASELINE: &str = "agency-any/default 40 44 40 e307602ef1c97e70
agency-any.broken-single-valued-a/default 30 32 30 9081f05c05995165
agency-lineage/default 68 67 68 571bc73bb3208395
agency-lineage.broken-missing-evidence/default 68 67 68 571bc73bb3208395
agency-lineage.broken-reused-source/default 68 67 68 571bc73bb3208395
agency-origin/default 20 19 20 f76c5993589c6951
agency-origin.broken-no-direct-read/default 20 19 20 f76c5993589c6951
agency-refusal/AsIs 41 40 41 bf1f69c96fdadea1
agency-refusal/ToBe 48 47 48 98b5dfcf032a4c0d
agency-refusal.broken-cut-guard/AsIs 41 40 41 bf1f69c96fdadea1
agency-refusal.broken-cut-guard/ToBe 53 52 53 59f4da3fd4ec2eff
any-drain/default 12 12 12 756fb94b641246bb
any-drain.broken-missing-cancel/default rejected
config-diff-add-actor/AsIs 6 5 6 2dda198fc3b4b53f
config-diff-add-actor/ToBe 11 10 11 bd0bc543ced85c2d
config-diff-add-actor.broken-missing-intent/AsIs 6 5 6 2dda198fc3b4b53f
config-diff-add-actor.broken-missing-intent/ToBe 11 10 11 bd0bc543ced85c2d
config-diff-add-actor.broken-other-cut/AsIs 6 5 6 2dda198fc3b4b53f
config-diff-add-actor.broken-other-cut/ToBe 11 10 11 bd0bc543ced85c2d
config-diff-parallel/AsIs 11 10 11 798275ff3aa9048d
config-diff-parallel/ToBeP 10 10 10 05849da4ba155dc6
config-diff-parallel.broken-intent-only/AsIs 11 10 11 798275ff3aa9048d
config-diff-parallel.broken-intent-only/ToBeP 10 10 10 05849da4ba155dc6
config-diff-parallel.broken-no-configuration/default rejected
config-diff-retire/AsIs 11 10 11 bd0bc543ced85c2d
config-diff-retire/ToBe 6 5 6 ffcdd0891b603ba5
config-diff-retire.broken-missing-loses/AsIs 11 10 11 bd0bc543ced85c2d
config-diff-retire.broken-missing-loses/ToBe 6 5 6 ffcdd0891b603ba5
config-diff-terminal-union/AsIs 6 5 6 300c6a4cc9a5d07b
config-diff-terminal-union/ToBe 6 5 6 663958909f841c31
config-diff-terminal-union.broken-unknown-terminal/AsIs rejected
config-diff-terminal-union.broken-unknown-terminal/ToBe rejected
config-diff-unreachable/AsIs 6 5 6 d5efb47b76afd683
config-diff-unreachable/ToBe 7 6 7 94e8b2df8e12cad1
config-diff-unreachable.broken-intent-only/AsIs 6 5 6 d5efb47b76afd683
config-diff-unreachable.broken-intent-only/ToBe 7 6 7 45ef77f361f5bab1
contract-cancel/default 11 11 11 473606f0eba0324c
contract-cancel.broken-ignore-cancel/default 11 11 11 ca61b1fefedef92c
contract-cut/default 8 7 8 9987ad934aa5981d
contract-cut.broken-late-target/default 8 7 8 570935788aa86ce9
contract-diff/Before 6 5 6 9d578965436438ef
contract-diff/After 1 0 1 17956b372353aa7b
contract-diff.broken-zero-deadline/Before 6 5 6 f1e57212ffc85445
contract-diff.broken-zero-deadline/After 1 0 1 17956b372353aa7b
contract-invariant/default 15 14 15 e3b5113330d5fd1f
contract-invariant.broken-cut-guard/default 20 19 20 0cc2f68e8d4adf51
contract-once/default 7 6 7 ea31592e9fd4d597
contract-once.broken-repeat-result/default 17 16 17 f79b2b99b1ae01ef
contract-order/default 16 15 16 4169aedb2db6a1b7
contract-order.broken-parallel/default 22 22 22 244a2951795e0d5a
contract-requires/default 15 14 15 e3b5113330d5fd1f
contract-requires.broken-cut-guard/default 20 19 20 c9a77405061c4289
contract-response/default 6 5 6 8d257befbaa9dfbf
contract-response.broken-late-finish/default 6 5 6 a31a02d54496ed25
contract-response.broken-short-clock/default 3 2 3 a91776ae5d78a493
contract-vacuous/default 7 6 7 44ad00b9ef10cd04
contract-vacuous.broken-enable-branch/default 15 14 15 e7a1c8ffd9b71fa2
coop-cancel/default 12 12 12 6486ab85bc1fdd0b
coop-cancel.broken-short-clock/default 8 8 8 f539dc96b6ceb1db
cut-declared/default 11 10 11 4e8d89bf155d37aa
cut-declared.broken-excluded-call/default rejected
cut-declared.broken-missing-loses/default 11 10 11 4e8d89bf155d37aa
cut-declared.broken-missing-member/default rejected
invoice-phase0-ai/AsIs 68 67 68 f43da5bbad08f635
invoice-phase0-ai/ToBeA 64 68 64 0b3e1cd124201703
invoice-phase0-ai.broken-unguarded-approval/AsIs 68 67 68 f43da5bbad08f635
invoice-phase0-ai.broken-unguarded-approval/ToBeA 69 73 69 0fbf1a80c9292e56
invoice-phase0-parallel/AsIs 68 67 68 f43da5bbad08f635
invoice-phase0-parallel/ToBeP 64 68 64 f4cfb5fc320e1f2b
invoice-phase0-parallel.broken-one-clerk/AsIs 68 67 68 f43da5bbad08f635
invoice-phase0-parallel.broken-one-clerk/ToBeP 72 75 72 0debe29210eb142e
nondet-race/default 35 35 35 0211f677278fea1a
nondet-race.broken-quick-never-passes/default 14 14 14 3594dbe675b96b9a
parallel-time/default 10 10 10 ceea64e7669d10e8
parallel-time.broken-missing-join/default rejected
resource-contention/default 14 14 14 8a89957c737a8c6c
resource-contention.broken-zero-capacity/default 2 1 2 1bad81761a779f81
serial-time/default 11 10 11 4e8d89bf155d37aa
serial-time.broken-missing-capacity/default rejected";
        assert_eq!(actual, BASELINE);
    }
}

#[cfg(test)]
pub(crate) fn record_fixture(name: &str) -> bool {
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let names = NAMES.get_or_init(|| {
        serde_json::from_str(include_str!("../tests/support/record_names.json")).unwrap()
    });
    let known = names.iter().any(|n| n == name);
    assert!(
        !name.starts_with("rec-") || known,
        "unclassified record fixture {name}"
    );
    known
}

/// Explore all permitted schedules/results/acks, never randomly sample them.
/// The input obeys the resolved IR invariants. Metrics are part of the visited
/// key: two control-equivalent paths need not have equivalent terminal extrema.
pub fn explore(model: &ir::Model) -> outcome::Outcome {
    explore_with_budget(model, DEFAULT_STATE_BUDGET)
}

pub(crate) fn receipt_unknown_held(model: &ir::Model, command: &str) -> Option<(bool, bool)> {
    arrivals::receipt_unknown_held(model, command)
}

/// States held at once (visited plus frontier) before exploration stops. A
/// computational limit, not a model bound: reaching it is a cut (ADR 0005).
pub const DEFAULT_STATE_BUDGET: usize = 1_000_000;

/// As explore, but stops with W-BUDGET-0 and a cut before more than
/// max_states states would be held at once (visited plus frontier), checked
/// before successors are materialized, not only after they are queued.
pub fn explore_with_budget(model: &ir::Model, max_states: usize) -> outcome::Outcome {
    #[cfg(test)]
    crate::evaluation_spy::hit(1);
    if let Err(Halt::Unsupported(message)) = record::validate(model) {
        let mut out = Exploration::default().abort(message, model);
        out.properties.clear();
        return out;
    }
    let mut out = if !model.legacy_route() {
        arrivals::explore(model, max_states)
    } else {
        attach_legacy_quality(run(model, true, max_states), model, max_states)
    };
    if let Some(metrics) = &mut out.metrics {
        metrics.apply_deferred(model);
    }
    if let Some(quality) = &mut out.quality {
        quality.apply_deferred(model);
    }
    out.diagnostics
        .extend(evidence::diagnostics(model, &out.properties));
    out
}
fn attach_legacy_quality(
    mut out: outcome::Outcome,
    model: &ir::Model,
    max_states: usize,
) -> outcome::Outcome {
    if model.quality.is_some() && validate(model).is_ok() {
        out.quality = Some(quality_legacy(model, max_states));
    }
    out
}

fn quality_hit(
    p: &ir::QualityProperty,
    s: &State,
    end: Option<(ir::EndId, u64)>,
    arrival: u64,
) -> EngineResult<bool> {
    match &p.target {
        ir::QualityTarget::State(c) => Ok(s.initialized
            && s.time >= arrival
            && p.within.is_none_or(|d| s.time - arrival <= d)
            && condition_at(c, &s.results, &s.state_values, 0)?),
        ir::QualityTarget::Ends(es) => Ok(end.is_some_and(|(e, t)| {
            es.contains(&e) && t >= arrival && p.within.is_none_or(|d| t - arrival <= d)
        })),
    }
}
fn quality_legacy(m: &ir::Model, budget: usize) -> crate::quality::Quality {
    quality_legacy_with_bits(m, budget, 4096)
}
fn quality_legacy_with_bits(m: &ir::Model, budget: usize, bits: u64) -> crate::quality::Quality {
    use crate::quality::{Solution, reduce_with_bits};
    let spec = m.quality.as_ref().unwrap();
    let mut facts = Exploration::default();
    let result = probability_graph(m, budget, &mut facts)
        .and_then(|(n, _)| probability_topology(&n).map(|o| (n, o)));
    let (nodes, order) = match result {
        Ok(v) => v,
        Err(_) => return quality_aborted(m),
    };
    #[cfg(test)]
    if !spec.quantiles.is_empty() {
        crate::quality::GRAPH_BUILDS.with(|n| n.set(n.get() + 1));
    }
    let values: EngineResult<Vec<_>> = spec
        .properties
        .iter()
        .map(|p| {
            let hits = nodes
                .iter()
                .map(|n| {
                    let s = &n.state;
                    let end = n.end.map(|e| {
                        (
                            e,
                            s.contest
                                .as_ref()
                                .and_then(|c| c.business.as_ref())
                                .map_or(s.time, |b| b.at),
                        )
                    });
                    quality_hit(p, s, end, 0)
                })
                .collect::<EngineResult<Vec<_>>>()?;
            Ok(vec![reduce_with_bits(
                &order,
                |i| nodes[i].actions.clone(),
                |i| {
                    let n = &nodes[i];
                    let s = &n.state;
                    let end = n.end.map(|e| {
                        (
                            e,
                            s.contest
                                .as_ref()
                                .and_then(|c| c.business.as_ref())
                                .map_or(s.time, |b| b.at),
                        )
                    });
                    if hits[i] {
                        return (Some(true), false);
                    }
                    if (end.is_some() && matches!(p.target, ir::QualityTarget::Ends(_)))
                        || (p.within.is_some_and(|d| {
                            s.time > d
                                || (n.unknown
                                    && n.cut_reason == ProbabilityCut::Clock
                                    && d <= m.bounds.clock)
                        }))
                    {
                        return (Some(false), false);
                    }
                    (None, n.unknown || n.numeric)
                },
                bits,
            )])
        })
        .collect();
    let values = match values {
        Ok(v) => v,
        Err(_) => return quality_aborted(m),
    };
    let keys = crate::quality::outcome_keys(m);
    let outcomes = keys
        .iter()
        .map(|(kind, end)| {
            vec![reduce_with_bits(
                &order,
                |i| nodes[i].actions.clone(),
                |i| {
                    let n = &nodes[i];
                    if let Some(e) = n.end {
                        return (
                            Some(*kind == "end" && end.as_deref() == Some(&m.ends[e.0])),
                            false,
                        );
                    }
                    if n.unknown || n.numeric {
                        return (
                            if *kind == "rejected" {
                                Some(false)
                            } else {
                                None
                            },
                            true,
                        );
                    }
                    if n.actions.is_empty() {
                        return (Some(*kind == "deadlock"), false);
                    }
                    (None, false)
                },
                bits,
            )]
        })
        .collect::<Vec<Vec<Solution>>>();
    let mut q = crate::quality::exhaustive(m, &values, &outcomes, false);
    if !spec.quantiles.is_empty() {
        let rows: EngineResult<Vec<_>> = spec
            .quantiles
            .iter()
            .map(|c| {
                let p = &spec.properties[c.property];
                let views = nodes
                    .iter()
                    .map(|n| {
                        let s = &n.state;
                        let end = n.end.map(|e| {
                            (
                                e,
                                s.contest
                                    .as_ref()
                                    .and_then(|c| c.business.as_ref())
                                    .map_or(s.time, |b| b.at),
                            )
                        });
                        let hit = quality_hit(p, s, end, 0)?.then(|| match p.target {
                            ir::QualityTarget::Ends(_) => end.unwrap().1 as u128,
                            _ => s.time as u128,
                        });
                        let closed = end.is_some()
                            && matches!(p.target, ir::QualityTarget::Ends(_))
                            || p.within.is_some_and(|d| s.time > d);
                        let earliest = if n.cut_reason == ProbabilityCut::Clock {
                            m.bounds.clock as u128 + 1
                        } else {
                            s.time as u128
                        };
                        Ok(crate::quality::FirstView {
                            hit,
                            closed,
                            censor: (n.unknown || n.numeric).then(|| {
                                crate::quality::Arrival::censor(
                                    earliest,
                                    p.within.map(u128::from),
                                    true,
                                )
                            }),
                        })
                    })
                    .collect::<EngineResult<Vec<_>>>()?;
                let f = crate::quality::first_curves(
                    &order,
                    |i| nodes[i].actions.clone(),
                    &[views],
                    bits,
                );
                let (v, lb) = crate::quality::inverse_quantile(c, &f);
                Ok(crate::quality::quantile_row(m, c, v, lb))
            })
            .collect();
        match rows {
            Ok(rows) => q.quantiles = Some(rows),
            Err(_) => return quality_aborted(m),
        }
    }
    q
}
fn quality_aborted(m: &ir::Model) -> crate::quality::Quality {
    let s = crate::quality::Solution::new(
        [
            Rational::zero(),
            Rational::one(),
            Rational::zero(),
            Rational::one(),
        ],
        true,
    );
    crate::quality::exhaustive(
        m,
        &vec![
            vec![s.clone(); m.bounds.cases as usize];
            m.quality.as_ref().unwrap().properties.len()
        ],
        &vec![vec![s; m.bounds.cases as usize]; crate::quality::outcome_keys(m).len()],
        true,
    )
}

/// Why exploration stopped early. Budget is a cut; Unsupported aborts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Halt {
    Unsupported(&'static str),
    Budget,
}

impl From<&'static str> for Halt {
    fn from(message: &'static str) -> Self {
        Halt::Unsupported(message)
    }
}

type EngineResult<T> = Result<T, Halt>;
mod degradation;
#[cfg(test)]
mod effect_tests;
#[cfg(test)]
mod integer_tests;
mod recovery;
#[cfg(test)]
pub(crate) fn r2_history(source: &str) -> serde_json::Value {
    recovery::history_observe(source)
}
#[cfg(test)]
pub(crate) fn r2_project(scenario: &str) -> serde_json::Value {
    recovery::projection_probe(scenario)
}
#[cfg(test)]
pub(crate) fn r2_resolve(fault: &str) -> serde_json::Value {
    recovery::resolve_probe(fault)
}
mod response;

/// Budget left for the successor states one step creates. Every producer takes
/// from it BEFORE creating a successor, so no generation path can produce more
/// successors than the budget allows (review N1). Not counted: the one working
/// copy of the popped state that run hands to step (a constant per step, never
/// a successor); adopt borrows and request_cancel reuses its input, so they
/// make no further uncounted copies (review T1).
struct Room(usize);

/// Successor allocation and sampling are driver policies, not State identity.
/// Exhaustive callers retain Room's original before-clone accounting.
trait Generation {
    fn remaining(&self) -> usize {
        usize::MAX
    }
    fn take(&mut self, states: usize) -> EngineResult<()>;
    fn sampling(&mut self) -> Option<&mut simulation::Sample> {
        None
    }
}

impl Generation for Room {
    fn remaining(&self) -> usize {
        self.0
    }
    fn take(&mut self, states: usize) -> EngineResult<()> {
        Room::take(self, states)
    }
}

impl Room {
    fn take(&mut self, states: usize) -> EngineResult<()> {
        self.0 = self.0.checked_sub(states).ok_or(Halt::Budget)?;
        Ok(())
    }
}

#[cfg(test)]
mod exec_census {
    use super::*;
    #[test]
    fn s_legacy_exec_census() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut dirs: Vec<_> = std::fs::read_dir(root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("exec-")
            })
            .collect();
        dirs.sort();
        let mut rows = Vec::new();
        for dir in dirs {
            let name = dir.file_name().unwrap().to_str().unwrap();
            let ast =
                crate::syntax::parse(&std::fs::read_to_string(dir.join("model.relca")).unwrap())
                    .unwrap();
            let inv = dir.join("invocation.json");
            let selections: Vec<Option<String>> = if inv.is_file() {
                let v: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(inv).unwrap()).unwrap();
                if v["command"] == "diff" {
                    vec![
                        Some(v["from"].as_str().unwrap().into()),
                        Some(v["to"].as_str().unwrap().into()),
                    ]
                } else {
                    vec![v["configuration"].as_str().map(str::to_owned)]
                }
            } else {
                vec![None]
            };
            for selection in selections {
                let label = format!("{name}/{}", selection.as_deref().unwrap_or("default"));
                let lowered = match selection.as_deref() {
                    Some(s) => crate::lower::lower_configuration(&ast, s).map(|c| c.model),
                    None => crate::lower::lower(&ast),
                };
                let Ok(model) = lowered else {
                    rows.push(format!("{label} rejected"));
                    continue;
                };
                let mut evidence = Exploration::default();
                explore_into(
                    &model,
                    true,
                    DEFAULT_STATE_BUDGET,
                    std::convert::identity,
                    &mut evidence,
                )
                .unwrap();
                let (visited, generated, held) = evidence.census;
                let budgets: Vec<_> = (0..=held + 1)
                    .map(|b| legacy_outcome_debug(explore_with_budget(&model, b)))
                    .collect();
                let fingerprint = budgets
                    .join("\n")
                    .bytes()
                    .fold(0xcbf29ce484222325u64, |h, b| {
                        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
                    });
                rows.push(format!(
                    "{label} {visited} {generated} {held} {fingerprint:016x}"
                ));
            }
        }
        let actual = rows.join("\n");
        println!("EXEC-CENSUS\n{actual}");
        // Measured on 2f68ee9 before any ADR 0012 production semantic change.
        const BASELINE: &str = "exec-anonymous/default 11 10 11 27082ff12f414e42
exec-anonymous.broken-no-executor/default rejected
exec-authority/default 9 9 9 cc2828d06044acd0
exec-authority.broken-missing-grant/default 10 9 10 4a2100a2604a28b1
exec-cancel/default 16 16 16 bd1701907b532a6c
exec-cancel.broken-ignore-cancel/default 16 16 16 0247a3c9c2d2ab0a
exec-permitted/default 9 9 9 cc2828d06044acd0
exec-permitted.broken-narrow-permit/default 10 9 10 0dd7a5b5c5470549
exec-separation/default 11 10 11 881ba38682ad9a8d
exec-separation.broken-shared-approver/default 15 14 15 2bf301ccc1c54e39
exec-shared-person/default 10 10 10 bf15ecce1a5cb0e8
exec-shared-person.broken-reused-person/default 14 14 14 86609aad842ee9dc
exec-vacuous/AsIs 11 10 11 4add59b38f7a31b3
exec-vacuous/ToBeA 11 10 11 a75cfed7b327419b
exec-vacuous.broken-shared-approver/AsIs 15 14 15 e1869449b2422e1f
exec-vacuous.broken-shared-approver/ToBeA 15 14 15 12dcd671dd3f1bcb";
        assert_eq!(actual, BASELINE);
    }
}

#[cfg(test)]
mod responsibility_census {
    #[cfg(test)]
    mod extensions {
        include!("engine/responsibility_extension_tests.rs");
    }
    use super::*;
    fn lower(s: &str) -> ir::Model {
        crate::lower::lower(&crate::syntax::parse(s).unwrap()).unwrap()
    }
    fn counts(m: &ir::Model, budget: usize) -> ((usize, usize, usize), outcome::Outcome) {
        let mut e = Exploration::default();
        let result = explore_into(m, true, budget, std::convert::identity, &mut e);
        let census = e.census;
        let out = match result {
            Ok(()) => e.outcome(m),
            Err(Halt::Budget) => {
                e.truncated = true;
                e.budget_exhausted = true;
                e.outcome(m)
            }
            Err(Halt::Unsupported(msg)) => e.abort(msg, m),
        };
        (census, out)
    }
    fn snapshots(m: &ir::Model) -> BTreeSet<String> {
        let mut program = vec![Instruction::Stop];
        let entry = compile(&m.process.body, 0, &mut program);
        let mut pending = vec![State::initial(m, entry)];
        let mut visited = HashSet::new();
        let mut out = BTreeSet::new();
        while let Some(s) = pending.pop() {
            if !visited.insert(s.clone()) {
                continue;
            }
            out.insert(format!("{s:?}"));
            let mut e = Exploration::default();
            if let Step::Next(states) =
                step_observed(s, &program, m, &mut Room(DEFAULT_STATE_BUDGET), &mut e).unwrap()
            {
                pending.extend(states);
            }
        }
        out
    }
    fn business_prefix(m: &ir::Model) -> BTreeSet<String> {
        let mut program = vec![Instruction::Stop];
        let entry = compile(&m.process.body, 0, &mut program);
        let mut pending = vec![State::initial(m, entry)];
        let mut seen = HashSet::new();
        let mut out = BTreeSet::new();
        while let Some(s) = pending.pop() {
            if !seen.insert(s.clone()) || s.contest.as_ref().is_some_and(|c| c.business.is_some()) {
                continue;
            }
            let mut projected = s.clone();
            projected.monitors = projected
                .monitors
                .into_iter()
                .zip(&m.contracts)
                .filter_map(|(monitor, c)| {
                    (!matches!(c.kind, ir::ContractKind::ContestResponse { .. })).then_some(monitor)
                })
                .collect();
            projected.contest = None;
            projected.obligations = None;
            out.insert(format!("{projected:?}"));
            if let Step::Next(xs) = step_observed(
                s,
                &program,
                m,
                &mut Room(DEFAULT_STATE_BUDGET),
                &mut Exploration::default(),
            )
            .unwrap()
            {
                pending.extend(xs);
            }
        }
        out
    }
    #[test]
    fn r_static_metadata_census_all_budgets_state_debug_and_monitors() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let positive = std::fs::read_to_string(root.join("resp-contest/model.relca")).unwrap();
        let runtime = lower(
            &positive
                .lines()
                .filter(|line| !line.trim_start().starts_with("contest "))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let full = counts(&runtime, DEFAULT_STATE_BUDGET).0;
        for name in [
            "resp-contest",
            "resp-contest.broken-no-authority",
            "resp-contest.broken-no-permission",
            "resp-contest.broken-zero-deadline",
            "resp-contest.broken-route-excluded",
        ] {
            let m = lower(&std::fs::read_to_string(root.join(name).join("model.relca")).unwrap());
            assert_eq!(business_prefix(&m), snapshots(&runtime), "{name}");
            let expanded = counts(&m, DEFAULT_STATE_BUDGET).0;
            assert!(expanded.0 > full.0, "contest adds real execution states");
            assert!(format!("{:?}", State::initial(&m, 0)).contains("contest:"));
            for budget in 0..=expanded.2 + 1 {
                let (c, out) = counts(&m, budget);
                assert!(c.2 <= budget.max(1), "{name}/{budget}: {c:?}");
                let static_rows = out
                    .properties
                    .iter()
                    .filter(|p| matches!(p.property, ir::Property::Contest(_)))
                    .collect::<Vec<_>>();
                assert_eq!(static_rows.len(), 1);
                assert_eq!(static_rows[0].verdict, crate::contest::verdict(&m, 0));
                assert!(out.contest_runtime.is_some());
                assert_eq!(
                    out.properties
                        .iter()
                        .filter(|p| matches!(p.property, ir::Property::Contract(_)))
                        .count(),
                    1
                );
            }
        }
        let mixed = positive
            .replace(
                "contest Vendor",
                "permitted Approver.approve;\ncontest Vendor",
            )
            .replace(
                "respond_by 5min",
                "respond_by 5min;\nauthority_basis Approver.approve ApprovalGrant",
            );
        let m = lower(&mixed);
        assert_eq!(m.contracts.len(), 3);
        assert_eq!(
            m.contracts[0].property_id(ir::ClauseId(0)),
            "contract Controls #1 permitted"
        );
        assert_eq!(
            crate::contest::property_id(&m, 0),
            "contract Controls #2 contest"
        );
        assert_eq!(
            m.contracts[1].property_id(ir::ClauseId(1)),
            "contract Controls #2 contest_response"
        );
        assert_eq!(
            m.contracts[2].property_id(ir::ClauseId(2)),
            "contract Controls #3 authority_basis"
        );
        let mut program = vec![Instruction::Stop];
        let entry = compile(&m.process.body, 0, &mut program);
        let state = State::initial(&m, entry);
        assert_eq!(state.monitors.len(), 3);
        assert!(state.contest.is_some());
        // Responsible, standing and party declarations add no state either.
        let original = lower(include_str!("../../../fixtures/resp-origin/model.relca"));
        let mut stripped = original.clone();
        stripped.responsibility = None;
        assert_eq!(snapshots(&original), snapshots(&stripped));
        let full = counts(&stripped, DEFAULT_STATE_BUDGET).0;
        assert_eq!(counts(&original, DEFAULT_STATE_BUDGET).0, full);
        for budget in 0..=full.2 + 1 {
            let (census, out) = counts(&original, budget);
            let (expected, old) = counts(&stripped, budget);
            assert_eq!(census, expected, "non-contest metadata census/{budget}");
            assert_eq!(out, old, "non-contest metadata outcome/{budget}");
        }
    }
}

#[cfg(test)]
mod executor_tests {
    use super::*;
    const P: ir::PersonId = ir::PersonId(0);
    const Q: ir::PersonId = ir::PersonId(1);

    fn model() -> ir::Model {
        crate::lower::lower(&crate::syntax::parse(r#"model M version "0.2" {
            people [p,q]; type T = enum { ok }
            actor A { executors [p,q]; capability a() -> T { commits; duration fixed 5min; result nondet { ok }; cancel cooperative ack_within 1min } }
            actor B { executors [p]; capability b() -> T { commits; duration fixed 1min; result nondet { ok }; cancel cooperative ack_within 1min } }
            authority G { basis "test"; grants p to A.a; grants p to B.b }
            process F { fork w { x = A.a(); y = B.b() }; join all w; end done }
            bounds { cases 1; clock 20min; live_tasks 2 }
            verify safety no_resource_overcommit
            contract C { separation A.a.finish, B.b.finish; permitted A.a; authority_basis A.a G }
        }"#).unwrap()).unwrap()
    }

    fn queued(model: &ir::Model) -> (State, Vec<Instruction>) {
        let mut program = vec![Instruction::Stop];
        let entry = compile(&model.process.body, 0, &mut program);
        let Step::Next(mut states) =
            step(State::initial(model, entry), &program, model, &mut Room(1)).unwrap()
        else {
            panic!("create")
        };
        (states.remove(0), program)
    }

    fn next(state: State, program: &[Instruction], model: &ir::Model) -> Vec<State> {
        match step(state, program, model, &mut Room(100)).unwrap() {
            Step::Next(states) => states,
            _ => panic!("expected transition"),
        }
    }

    #[test]
    fn e_task_identity() {
        let m = model();
        let (mut a, _) = queued(&m);
        a.tasks[0].lifecycle = Lifecycle::Running {
            finish: 5,
            ack: None,
        };
        a.tasks[0].executor = Some(P);
        let mut b = a.clone();
        b.tasks[0].executor = Some(Q);
        assert_ne!(a, b);
        assert_eq!(HashSet::from([a, b]).len(), 2);
    }

    #[test]
    fn e_new_public_ir_invariants_fail_closed() {
        let base = model();
        let mutations: &[fn(&mut ir::Model)] = &[
            |m| m.actors[0].executors = Some(Vec::new()),
            |m| m.actors[0].executors = Some(vec![P, P]),
            |m| m.actors[0].executors = Some(vec![P, ir::PersonId(999)]),
            |m| m.actors[0].capacity = Some(1),
            |m| m.capabilities[0].permission = Some(vec![ir::PersonId(999)]),
            |m| m.capabilities[0].permission = Some(vec![P, P]),
            |m| {
                m.actors[0].executors = None;
                m.capabilities[0].permission = Some(vec![P]);
            },
            |m| m.authorities[0].grants[0].capability = ir::CapabilityId(999),
            |m| m.authorities[0].grants[0].persons = vec![ir::PersonId(999)],
            |m| m.authorities[0].grants[0].persons = vec![P, P],
            |m| m.authorities[0].issued_by = Some(ir::PersonId(999)),
            |m| m.authorities[0].not_by = vec![P],
            |m| m.authorities[0].not_by = vec![Q, Q],
            |m| m.authorities[0].not_by = vec![ir::PersonId(999)],
            |m| {
                m.authorities[0].not_by = vec![Q];
                m.authorities[0].issued_by = Some(Q);
            },
            |m| {
                m.contracts[2].kind = ir::ContractKind::AuthorityBasis {
                    capability: ir::CapabilityId(0),
                    authority: ir::AuthorityId(999),
                }
            },
            |m| {
                m.authorities[0].grants.remove(0);
            },
            |m| {
                m.contracts[1].kind = ir::ContractKind::Permitted {
                    capability: ir::CapabilityId(999),
                }
            },
            |m| {
                if let ir::ContractKind::Separation { left, right } = &mut m.contracts[0].kind {
                    *right = *left;
                }
            },
            |m| {
                m.actors[0].executors = None;
            },
        ];
        for (i, mutate) in mutations.iter().enumerate() {
            let mut broken = base.clone();
            mutate(&mut broken);
            let out = explore(&broken);
            assert!(
                out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0),
                "mutation {i}: {out:?}"
            );
            assert!(out.terminals.is_empty());
            assert!(
                out.properties
                    .iter()
                    .all(|p| p.verdict == Verdict::Unsupported)
            );
        }
    }

    #[test]
    fn e_monitor_identity_and_allpairs_prefix() {
        let mut m = model();
        let (base, _) = queued(&m);
        let mut states = vec![base.clone()];
        for (left, person) in [(true, P), (true, Q), (false, P), (false, Q)] {
            let mut s = base.clone();
            if left {
                s.monitors[0].left_people.insert(person);
            } else {
                s.monitors[0].right_people.insert(person);
            }
            states.push(s);
        }
        assert_eq!(states.iter().cloned().collect::<HashSet<_>>().len(), 5);
        let mut a = base.clone();
        a.monitors[0].left_people.extend([P, Q]);
        let mut b = base.clone();
        b.monitors[0].left_people.extend([Q, P]);
        assert_eq!(a, b);
        assert_eq!(HashSet::from([a, b]).len(), 1);
        for reverse in [false, true] {
            if reverse
                && let ir::ContractKind::Separation { left, right } = &mut m.contracts[0].kind
            {
                std::mem::swap(left, right);
            }
            let mut s = base.clone();
            let mut facts = Exploration::default();
            for (at, cap, person) in [(1, 0, P), (2, 0, Q), (3, 1, P)] {
                s.time = at;
                emit(
                    &mut s,
                    Bundle::Finish(ir::CapabilityId(cap), ir::ValueId(0), Some(person)),
                    &m,
                    &mut facts,
                )
                .unwrap();
                // Task storage can be discarded; history belongs to the path.
                s.tasks.clear();
            }
            let monitor = &s.monitors[0];
            assert!(monitor.activated && monitor.violated);
            assert_eq!(
                if reverse {
                    &monitor.right_people
                } else {
                    &monitor.left_people
                },
                &BTreeSet::from([P, Q])
            );
            assert_eq!(
                facts.contract_verdict(ir::ClauseId(0), false),
                Verdict::Violated
            );
        }
    }

    #[test]
    fn e_assignment_budget_room() {
        let mut m = model();
        m.capabilities[0].permission = Some(vec![Q]);
        let (mut s, program) = queued(&m);
        s.tasks.truncate(1);
        let mut room = Room(2);
        let mut facts = Exploration::default();
        let Step::Next(states) =
            step_observed(s.clone(), &program, &m, &mut room, &mut facts).unwrap()
        else {
            panic!("starts")
        };
        assert_eq!(room.0, 0);
        assert_eq!(states.len(), 2);
        assert_eq!(
            states
                .iter()
                .map(|s| s.tasks[0].executor)
                .collect::<Vec<_>>(),
            vec![Some(P), Some(Q)]
        );
        let mut facts = Exploration::default();
        assert!(matches!(
            step_observed(s, &program, &m, &mut Room(1), &mut facts),
            Err(Halt::Budget)
        ));
        assert_eq!(
            facts.contract_verdict(ir::ClauseId(1), false),
            Verdict::Violated
        );
    }

    #[test]
    fn e_ack_occupancy_and_all_release_edges() {
        let mut m = model();
        let (mut s, program) = queued(&m);
        s.time = 1;
        s.tasks[0].executor = Some(P);
        s.tasks[0].lifecycle = Lifecycle::Running {
            finish: 5,
            ack: None,
        };
        let requested = request_cancel(vec![s.clone()], 0, &m, &mut Room(100))
            .unwrap()
            .remove(0);
        assert_eq!(requested.tasks[0].executor, Some(P));
        assert_eq!(
            requested.tasks[0].lifecycle,
            Lifecycle::Running {
                finish: 5,
                ack: Some(2)
            }
        );
        let ticked = next(requested, &program, &m).remove(0);
        assert_eq!(ticked.time, 2);
        assert_eq!(ticked.tasks[1].lifecycle, Lifecycle::Queued);
        let acked = next(ticked, &program, &m).remove(0);
        assert_eq!(acked.tasks[0].lifecycle, Lifecycle::Done);
        let started = next(acked, &program, &m).remove(0);
        assert_eq!(started.tasks[1].executor, Some(P));
        assert!(matches!(
            started.tasks[1].lifecycle,
            Lifecycle::Running { .. }
        ));
        // Natural finish releases the same person as an acknowledgement.
        let mut natural = s.clone();
        natural.time = 5;
        let finished = complete(
            &natural,
            Due {
                i: 0,
                finish: 5,
                ack: None,
            },
            &m,
            &mut Room(100),
        )
        .unwrap()
        .remove(0);
        assert_eq!(next(finished, &program, &m)[0].tasks[1].executor, Some(P));
        m.capabilities[0].cancel = ir::Cancel::ForbiddenAfterStart;
        let forbidden = request_cancel(vec![s], 0, &m, &mut Room(100))
            .unwrap()
            .remove(0);
        assert_eq!(forbidden.tasks[0].executor, Some(P));
        let ticked = next(forbidden, &program, &m).remove(0);
        assert_eq!(ticked.time, 5);
        assert_eq!(ticked.tasks[1].lifecycle, Lifecycle::Queued);
        // Queued cancellation never acquires a person.
        let (s, _) = queued(&m);
        let cancelled = request_cancel(vec![s], 0, &m, &mut Room(100))
            .unwrap()
            .remove(0);
        assert_eq!(cancelled.tasks[0].executor, None);
        assert_eq!(cancelled.tasks[0].lifecycle, Lifecycle::Done);
        // Finish == ack yields both branches, retaining identity in each.
        let mut simultaneous = ticked;
        simultaneous.tasks[0].lifecycle = Lifecycle::Running {
            finish: 5,
            ack: Some(5),
        };
        let states = complete(
            &simultaneous,
            Due {
                i: 0,
                finish: 5,
                ack: Some(5),
            },
            &m,
            &mut Room(2),
        )
        .unwrap();
        assert_eq!(states.len(), 2);
        assert_eq!(
            states.iter().map(|s| s.results[0]).collect::<Vec<_>>(),
            vec![Some(ir::ValueId(0)), None]
        );
        for state in states {
            assert_eq!(state.tasks[0].executor, Some(P));
            assert_eq!(next(state, &program, &m)[0].tasks[1].executor, Some(P));
        }
    }

    #[test]
    fn e_overcommit_observer_retains_across_cut_and_abort() {
        let mut m = model();
        m.actors[0].executors = Some(vec![P]);
        m.actors[0].capacity = Some(1);
        let (mut s, _) = queued(&m);
        for task in &mut s.tasks {
            task.executor = Some(P);
            task.lifecycle = Lifecycle::Running {
                finish: 5,
                ack: None,
            };
        }
        assert_eq!(s.occupancy(&m), vec![1, 1]);
        let mut evidence = Exploration::default();
        evidence.observe(&s, &m).unwrap();
        assert!(evidence.overcommit);
        evidence.truncated = true;
        assert_eq!(
            evidence.outcome(&m).properties[0].verdict,
            Verdict::Violated
        );
        let mut evidence = Exploration::default();
        evidence.observe(&s, &m).unwrap();
        assert_eq!(
            evidence.abort("test abort", &m).properties[0].verdict,
            Verdict::Violated
        );
    }

    #[test]
    fn e_engine_zero_internal_identity_release_and_contracts() {
        let mut m = model();
        m.capabilities[0].duration = 0;
        m.capabilities[1].duration = 0;
        m.actors[0].executors = Some(vec![P]);
        m.actors[0].capacity = Some(1);
        m.contracts[0].kind = ir::ContractKind::Separation {
            left: ir::Event {
                capability: ir::CapabilityId(0),
                kind: ir::EventKind::Start,
            },
            right: ir::Event {
                capability: ir::CapabilityId(0),
                kind: ir::EventKind::Finish(None),
            },
        };
        let (s, program) = queued(&m);
        let start = next(s, &program, &m)
            .into_iter()
            .find(|s| s.tasks[0].executor == Some(P))
            .unwrap();
        assert_eq!(start.time, 0);
        assert_eq!(start.tasks[1].lifecycle, Lifecycle::Queued);
        assert!(start.monitors[1].activated);
        assert!(!start.monitors[2].activated);
        let finished = next(start, &program, &m).remove(0);
        assert_eq!(finished.monitors[0].left_people, BTreeSet::from([P]));
        assert_eq!(finished.monitors[0].right_people, BTreeSet::from([P]));
        assert!(finished.monitors[0].violated);
        assert!(finished.monitors[2].activated && !finished.monitors[2].violated);
        let started = next(finished, &program, &m).remove(0);
        assert_eq!(started.tasks[1].executor, Some(P));
        assert_eq!(
            started
                .tasks
                .iter()
                .filter(|t| matches!(t.lifecycle, Lifecycle::Running { .. }))
                .count(),
            1
        );
        assert_eq!((started.time, started.work), (0, 0));
    }
}

#[derive(Debug)]
enum Instruction {
    MergeCopy {
        binding: ir::BindingId,
        source: ir::BindingId,
        next: usize,
    },
    LoopControl {
        updates: Vec<(ir::StateVarId, ir::StateValue)>,
        next: usize,
    },
    Await {
        condition: ir::Cond,
        next: usize,
    },
    Receipt {
        binding: ir::BindingId,
        rule: Box<ir::ResponseAttempt>,
        retry: Option<usize>,
        failure: Option<usize>,
        recovery: Option<usize>,
        next: usize,
    },
    Sleep {
        duration: u64,
        next: usize,
    },
    Wait {
        calls: Vec<ir::Call>,
        join: ir::Join,
        next: usize,
    },
    Branch {
        span: diag::Span,
        site: Vec<usize>,
        condition: ir::Cond,
        yes: usize,
        no: usize,
    },
    ResolveEffect {
        command: ir::BindingId,
        evidence: ir::BindingId,
        site: diag::Span,
        next: usize,
    },
    ReceiptUnknown,
    End(ir::EndId),
    Stop,
}

// Continuations make nested if fallthrough explicit without storing a source
// tree (which is not Hash) in every state. No textual name lookup is involved.
fn compile(block: &ir::Block, next: usize, program: &mut Vec<Instruction>) -> usize {
    compile_at(block, next, program, &[])
}
fn compile_at(
    block: &ir::Block,
    mut next: usize,
    program: &mut Vec<Instruction>,
    prefix: &[usize],
) -> usize {
    let mut command_suffixes = BTreeMap::new();
    for (i, statement) in block.iter().enumerate().rev() {
        let mut path = prefix.to_vec();
        path.push(i);
        let instruction = match &statement.kind {
            ir::Stmt::MergeCopy { binding, source } => Instruction::MergeCopy {
                binding: *binding,
                source: *source,
                next,
            },
            ir::Stmt::LoopControl(control) => Instruction::LoopControl {
                updates: control.updates.clone(),
                next,
            },
            ir::Stmt::WaitUntil(condition) => Instruction::Await {
                condition: condition.clone(),
                next,
            },
            ir::Stmt::Wait(duration) => Instruction::Sleep {
                duration: *duration,
                next,
            },
            ir::Stmt::Response(call, rule) => {
                let suffix = *command_suffixes.entry(rule.command).or_insert(next);
                let retry = if rule.attempt < rule.max_attempts {
                    let sleep = program.len();
                    program.push(Instruction::Sleep {
                        duration: rule.backoff,
                        next,
                    });
                    Some(sleep)
                } else {
                    None
                };
                let failure = rule.exhausted.map(|e| {
                    let pc = program.len();
                    program.push(Instruction::End(e));
                    pc
                });
                let receipt = program.len();
                let recovery = rule.recovery.as_ref().map(|b| {
                    let mut child = path.clone();
                    child.push(2);
                    compile_at(b, 0, program, &child)
                });
                let receipt = if recovery.is_some() {
                    program.len()
                } else {
                    receipt
                };
                program.push(Instruction::Receipt {
                    binding: call.binding,
                    rule: rule.clone(),
                    retry,
                    failure,
                    recovery,
                    next: suffix,
                });
                Instruction::Wait {
                    calls: vec![call.clone()],
                    join: ir::Join::All,
                    next: receipt,
                }
            }
            ir::Stmt::Call(call) | ir::Stmt::Query(call, _) => Instruction::Wait {
                calls: vec![call.clone()],
                join: ir::Join::All,
                next,
            },
            ir::Stmt::Fork { branches, join, .. } => Instruction::Wait {
                calls: branches.clone(),
                join: join.clone(),
                next,
            },
            ir::Stmt::If {
                condition,
                then_body,
                else_body,
            } => Instruction::Branch {
                span: statement.span,
                site: path.clone(),
                condition: condition.clone(),
                yes: {
                    let mut child = path.clone();
                    child.push(0);
                    compile_at(then_body, next, program, &child)
                },
                no: {
                    let mut child = path;
                    child.push(1);
                    compile_at(else_body, next, program, &child)
                },
            },
            ir::Stmt::ResolveEffect { command, evidence } => Instruction::ResolveEffect {
                command: *command,
                evidence: *evidence,
                site: statement.span,
                next,
            },
            ir::Stmt::StopReceiptUnknown => Instruction::ReceiptUnknown,
            ir::Stmt::End(end) => Instruction::End(*end),
        };
        next = program.len();
        program.push(instruction);
    }
    next
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Lifecycle {
    Queued,
    Running {
        finish: u64,
        ack: Option<u64>,
    },
    DistributedRunning {
        started: u64,
        next: usize,
        due: bool,
        ack: Option<u64>,
    },
    Done,
}

impl Lifecycle {
    fn running(self) -> bool {
        matches!(self, Self::Running { .. } | Self::DistributedRunning { .. })
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Task {
    frozen_performance: Option<ir::FrozenPerformance>,
    stale: Vec<ir::StateVarId>,
    executor: Option<ir::PersonId>,
    binding: ir::BindingId,
    capability: ir::CapabilityId,
    lifecycle: Lifecycle,
}

impl std::hash::Hash for Task {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        // Empty history must preserve the pre-0041 stream, not even adding a
        // vector-length discriminant. Nonempty history remains semantic.
        self.executor.hash(h);
        self.binding.hash(h);
        self.capability.hash(h);
        self.lifecycle.hash(h);
        if let Some(selector) = &self.frozen_performance {
            selector.hash(h);
        }
        if !self.stale.is_empty() {
            self.stale.hash(h);
        }
    }
}

impl std::fmt::Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("Task");
        d.field("executor", &self.executor)
            .field("binding", &self.binding)
            .field("capability", &self.capability)
            .field("lifecycle", &self.lifecycle);
        if !self.stale.is_empty() {
            d.field("stale", &self.stale);
        }
        if let Some(selector) = &self.frozen_performance {
            d.field("frozen_performance", selector);
        }
        d.finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
struct State {
    evidence: Option<evidence::History>,
    response: Option<response::History>,
    lost_updates: Vec<ir::StateVarId>,
    sleep_until: Option<u64>,
    contest: Option<ContestState>,
    obligations: Option<Vec<Debt>>,
    metric_history: Option<metrics::History>,
    state_values: Vec<ir::StateValue>,
    initialized: bool,
    monitors: Vec<Monitor>,
    time: u64,
    pc: usize,
    results: Vec<Option<ir::ValueId>>,
    tasks: Vec<Task>,
    waiting: bool,
    adopted: Option<ir::BindingId>,
    work: u64,
    result_at: Option<u64>,
}

impl std::hash::Hash for State {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        if let Some(evidence) = &self.evidence {
            evidence.hash(h);
        }
        if let Some(response) = &self.response {
            response.hash(h);
        }
        if let Some(wake) = self.sleep_until {
            wake.hash(h);
        }
        self.state_values.hash(h);
        if !self.lost_updates.is_empty() {
            self.lost_updates.hash(h);
        }
        self.initialized.hash(h);
        self.monitors.hash(h);
        self.time.hash(h);
        self.pc.hash(h);
        self.results.hash(h);
        self.tasks.hash(h);
        self.waiting.hash(h);
        self.adopted.hash(h);
        self.work.hash(h);
        self.result_at.hash(h);
        if let Some(history) = &self.metric_history {
            history.hash(h);
        }
        if let Some(debts) = &self.obligations {
            debts.hash(h);
        }
        if let Some(contest) = &self.contest {
            contest.hash(h);
        }
    }
}

impl State {
    fn initial(model: &ir::Model, pc: usize) -> Self {
        Self {
            evidence: crate::evidence_basis::present(model).then(evidence::History::default),
            response: (!model.response_attempts().is_empty())
                .then(|| response::History::for_model(model)),
            lost_updates: Vec::new(),
            sleep_until: None,
            contest: (!model.contests().is_empty()).then(|| ContestState {
                end_open: None,
                business: None,
                routes: vec![RouteState::default(); model.contests().len()],
            }),
            obligations: model.tracks_obligations().then(Vec::new),
            metric_history: model.metrics.as_ref().map(|_| metrics::History::new(model)),
            state_values: model
                .state_variables
                .iter()
                .map(|v| match v.initial {
                    ir::InitialValue::Fixed(x) => x,
                    ir::InitialValue::Nondet | ir::InitialValue::Distribution(_) => {
                        if v.integer_max.is_some() {
                            ir::StateValue::Integer(0)
                        } else {
                            ir::StateValue::Value(ir::ValueId(0))
                        }
                    }
                })
                .collect(),
            initialized: !model
                .state_variables
                .iter()
                .any(|v| !matches!(v.initial, ir::InitialValue::Fixed(_))),
            monitors: model
                .contracts
                .iter()
                .map(|c| {
                    let mut monitor = Monitor::default();
                    if let ir::ContractKind::EvidenceBasis { capability, .. } = c.kind
                        && evidence::route(model, capability)
                    {
                        monitor.evidence_issue = Some(evidence::Issue::Contest);
                    }
                    monitor
                })
                .collect(),
            time: 0,
            pc,
            results: vec![None; model.process.bindings.len()],
            tasks: Vec::new(),
            waiting: false,
            adopted: None,
            work: 0,
            result_at: None,
        }
    }

    fn occupancy(&self, model: &ir::Model) -> Vec<u64> {
        let mut counts = vec![0; model.actors.len()];
        for task in &self.tasks {
            if task.lifecycle.running() {
                counts[model.capabilities[task.capability.0].actor.0] += 1;
            }
        }
        counts
    }

    fn resource_occupancy(&self, model: &ir::Model) -> EngineResult<Vec<u64>> {
        resource_occupancy(self.tasks.iter(), model)
    }

    fn exceeds_live_bound(&self, bound: u64) -> bool {
        self.tasks
            .iter()
            .filter(|task| task.lifecycle != Lifecycle::Done)
            .count() as u64
            > bound
    }
}

fn resource_occupancy<'a>(
    tasks: impl Iterator<Item = &'a Task>,
    model: &ir::Model,
) -> EngineResult<Vec<u64>> {
    let mut used = vec![0u64; model.resources.len()];
    for t in tasks.filter(|t| t.lifecycle.running()) {
        for u in &model.capabilities[t.capability.0].uses {
            used[u.resource.0] = used[u.resource.0]
                .checked_add(u.amount)
                .ok_or("resource occupancy overflow")?;
        }
    }
    Ok(used)
}

fn resources_fit(cap: &ir::Capability, used: &[u64], model: &ir::Model) -> bool {
    cap.uses.iter().all(|u| {
        let capacity = model.resources[u.resource.0].capacity;
        used[u.resource.0] <= capacity && u.amount <= capacity - used[u.resource.0]
    })
}

#[cfg(test)]
#[path = "engine/resource_tests.rs"]
mod resource_tests;

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("State");
        if let Some(response) = &self.response {
            d.field("response", response);
        }
        if !self.lost_updates.is_empty() {
            d.field("lost_updates", &self.lost_updates);
        }
        if let Some(wake) = self.sleep_until {
            d.field("sleep_until", &wake);
        }
        d.field("metric_history", &self.metric_history)
            .field("state_values", &self.state_values)
            .field("initialized", &self.initialized)
            .field("monitors", &self.monitors)
            .field("time", &self.time)
            .field("pc", &self.pc)
            .field("results", &self.results)
            .field("tasks", &self.tasks)
            .field("waiting", &self.waiting)
            .field("adopted", &self.adopted)
            .field("work", &self.work)
            .field("result_at", &self.result_at);
        if let Some(debts) = &self.obligations {
            d.field("obligations", debts);
        }
        if let Some(contest) = &self.contest {
            d.field("contest", contest);
        }
        d.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BusinessEnd {
    end: ir::EndId,
    at: u64,
    work: u64,
    result_at: Option<u64>,
}

#[derive(Clone, Default, PartialEq, Eq)]
struct RouteState {
    target_at: Option<u64>,
    filed: Option<bool>,
    queued: bool,
    started_at: Option<u64>,
    response_at: Option<u64>,
    executor: Option<ir::PersonId>,
    result: Option<ir::ValueId>,
    violation: Option<&'static str>,
    // A known non-response closure is a lifecycle observation, not a metric.
    failure_at: Option<u64>,
}

impl std::hash::Hash for RouteState {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.target_at.hash(h);
        self.filed.hash(h);
        self.queued.hash(h);
        self.started_at.hash(h);
        self.response_at.hash(h);
        self.executor.hash(h);
        self.result.hash(h);
        self.violation.hash(h);
        if let Some(at) = self.failure_at {
            at.hash(h);
        }
    }
}

impl std::fmt::Debug for RouteState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("RouteState");
        d.field("filed", &self.filed)
            .field("queued", &self.queued)
            .field("started_at", &self.started_at)
            .field("response_at", &self.response_at)
            .field("executor", &self.executor)
            .field("result", &self.result)
            .field("violation", &self.violation)
            .finish()
    }
}
#[derive(Clone, PartialEq, Eq, Hash)]
struct ContestState {
    end_open: Option<bool>,
    business: Option<BusinessEnd>,
    routes: Vec<RouteState>,
}
impl std::fmt::Debug for ContestState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContestState")
            .field("business", &self.business)
            .field("routes", &self.routes)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Debt {
    response: ir::ClauseId,
    site: Option<ir::BindingId>,
    triggered_at: u64,
    deadline: u64,
    resolution: Option<crate::obligation::Resolution>,
}

enum Step {
    Next(Vec<State>),
    Terminal(ir::EndId),
    Cut,
    Deadlock,
}

/// Sufficient path-local history; all fields participate in State's Eq/Hash.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Monitor {
    evidence_issue: Option<evidence::Issue>,
    left_people: BTreeSet<ir::PersonId>,
    right_people: BTreeSet<ir::PersonId>,
    activated: bool,
    violated: bool,
    count: u8,
    before_seen: bool,
    pending: Vec<ir::Tick>,
}

impl std::hash::Hash for Monitor {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.left_people.hash(h);
        self.right_people.hash(h);
        self.activated.hash(h);
        self.violated.hash(h);
        self.count.hash(h);
        self.before_seen.hash(h);
        self.pending.hash(h);
        if let Some(issue) = self.evidence_issue {
            issue.hash(h);
        }
    }
}

#[derive(Clone, Copy)]
enum Bundle {
    CancelAck(ir::CapabilityId, Option<ir::PersonId>),
    Received,
    Admitted,
    Start(ir::CapabilityId, Option<ir::PersonId>),
    Finish(ir::CapabilityId, ir::ValueId, Option<ir::PersonId>),
    End(ir::EndId),
}

impl Bundle {
    fn executor(self) -> EngineResult<ir::PersonId> {
        match self {
            Self::Start(_, Some(p)) | Self::Finish(_, _, Some(p)) => Ok(p),
            _ => Err("named event missing executor".into()),
        }
    }
    fn event(self, e: ir::Event, model: &ir::Model) -> bool {
        match (self, e.kind) {
            (Self::CancelAck(c, _), ir::EventKind::CancelAck) => c == e.capability,
            (Self::Start(c, _), ir::EventKind::Start) => c == e.capability,
            (Self::Finish(c, v, _), ir::EventKind::Finish(w)) => {
                c == e.capability && w.is_none_or(|w| w == v)
            }
            (Self::Finish(c, v, _), ir::EventKind::Commit) => {
                c == e.capability && model.capabilities[c.0].commits_result(v)
            }
            _ => false,
        }
    }
    fn trigger(self, t: ir::Trigger, model: &ir::Model) -> bool {
        match t {
            ir::Trigger::Received => matches!(self, Self::Received),
            ir::Trigger::Admitted => matches!(self, Self::Admitted),
            ir::Trigger::Event(e) => self.event(e, model),
        }
    }
    fn target(self, t: ir::Target, model: &ir::Model) -> bool {
        match t {
            ir::Target::End(e) => matches!(self, Self::End(actual) if e == actual),
            ir::Target::Event(e) => self.event(e, model),
        }
    }
}

fn emit(
    state: &mut State,
    bundle: Bundle,
    model: &ir::Model,
    exploration: &mut Exploration,
) -> EngineResult<()> {
    emit_at(state, bundle, None, model, exploration)
}

fn emit_at(
    state: &mut State,
    bundle: Bundle,
    binding: Option<ir::BindingId>,
    model: &ir::Model,
    exploration: &mut Exploration,
) -> EngineResult<()> {
    emit_in(state, bundle, binding, model, exploration, 0)
}

fn emit_in(
    state: &mut State,
    bundle: Bundle,
    binding: Option<ir::BindingId>,
    model: &ir::Model,
    exploration: &mut Exploration,
    case_index: usize,
) -> EngineResult<()> {
    recovery::task_event(state, bundle, binding, model);
    // ADR 0040: a commit of a contested effect fixes that route's target time.
    #[cfg(test)]
    match bundle {
        Bundle::Start(c, _) => crate::q6_acceptance::event(state.time, model, c, "start"),
        Bundle::Finish(c, _, _) => crate::q6_acceptance::event(state.time, model, c, "finish"),
        Bundle::CancelAck(c, _) => crate::q6_acceptance::event(state.time, model, c, "ack"),
        _ => {}
    }
    // The commit is the finish on the commit branch (ADR 0035 atomic), not
    // every normal finish of a commits Capability.
    if let Bundle::Finish(cap, result, _) = bundle
        && model.capabilities[cap.0].commits_result(result)
        && let Some(contest) = &mut state.contest
    {
        for (id, d) in model.contests().iter().enumerate() {
            if d.target == crate::contest::ContestTarget::Commit(cap)
                && contest.routes[id].target_at.replace(state.time).is_some()
            {
                return Err("repeated contest effect".into());
            }
        }
    }
    // This event was actually generated. Retain its proven facts even if a
    // later clause in the same bundle aborts (for example deadline overflow).
    match bundle {
        Bundle::Start(c, p) => {
            if let Some(site) = binding
                && let Some(h) = &mut state.response
            {
                h.started.insert(site, c);
            }
            exploration.observed.capabilities.insert(c);
            if let Some(p) = p {
                exploration.observed.people.insert(p);
            }
        }
        Bundle::End(e) => {
            exploration.observed.ends.insert(e);
        }
        _ => {}
    }
    #[cfg(test)]
    if !model.state_variables.is_empty() {
        exploration
            .state_events
            .push((state.time, bundle, state.state_values.clone()));
    }
    if let Some(id) = binding.and_then(|b| model.site_contest(b))
        && let Bundle::Start(capability, executor) = bundle
    {
        let route = &mut state.contest.as_mut().expect("contest site").routes[id];
        route.started_at = Some(state.time);
        route.executor = executor;
        let cap = &model.capabilities[capability.0];
        if let Some(allowed) = cap
            .permission
            .as_ref()
            .or(model.actors[cap.actor.0].executors.as_ref())
            && executor.is_none_or(|p| !allowed.contains(&p))
        {
            route.violation = Some("permission");
        }
    }
    refresh_contests(state, model, state.time, false);
    evidence::event(state, bundle, binding, model);
    update_state_invariants(state, model, case_index)?;
    let ledger = update_debts(state, bundle, binding, model);
    let result = update_monitors(state, bundle, binding, model, case_index).and(ledger);
    exploration.contracts(state);
    if result.is_err() {
        if model
            .contracts
            .iter()
            .any(|c| matches!(c.kind, ir::ContractKind::StateInvariant { .. }))
        {
            exploration.state_invariant_aborted_bundle = Some(state.clone());
        }
        exploration.bundle_aborted = true;
        exploration.collect_debts(state, model, "aborted");
    }
    if let Some(trace) = &mut exploration.trace {
        trace.emitted(state, bundle, binding, model);
    }
    result
}

/// Monitoring only: no business transition, draw, time or resource mutation.
/// `case_index` is the explicit case context for membership atoms (ADR0035);
/// the State carries no hidden case index.
fn update_state_invariants(
    state: &mut State,
    model: &ir::Model,
    case_index: usize,
) -> EngineResult<()> {
    if !state.initialized
        || !model
            .contracts
            .iter()
            .any(|c| matches!(c.kind, ir::ContractKind::StateInvariant { .. }))
    {
        return Ok(());
    }
    for (monitor, clause) in state.monitors.iter_mut().zip(&model.contracts) {
        if let ir::ContractKind::StateInvariant {
            antecedent,
            condition: consequent,
        } = &clause.kind
        {
            let active = match antecedent {
                Some(c) => condition_at(c, &state.results, &state.state_values, case_index)?,
                None => true,
            };
            if active {
                monitor.activated = true;
                monitor.violated |=
                    !condition_at(consequent, &state.results, &state.state_values, case_index)?;
            }
        }
    }
    Ok(())
}

fn update_monitors(
    state: &mut State,
    bundle: Bundle,
    binding: Option<ir::BindingId>,
    model: &ir::Model,
    case_index: usize,
) -> EngineResult<()> {
    // A clause error aborts exploration, not observation of this bundle. Keep
    // the first error while collecting every independently established fact.
    let mut error = None;
    for (m, clause) in state.monitors.iter_mut().zip(&model.contracts) {
        match &clause.kind {
            ir::ContractKind::Excludes { before, start } => {
                if bundle.event(*start, model) {
                    m.violated |= m.before_seen;
                }
                let fired = bundle.event(*before, model);
                m.activated |= fired;
                m.before_seen |= fired;
            }
            ir::ContractKind::Separation { left, right } => {
                let l = bundle.event(*left, model);
                let r = bundle.event(*right, model);
                if l || r {
                    match bundle.executor() {
                        Ok(p) => {
                            if l {
                                m.left_people.insert(p);
                            }
                            if r {
                                m.right_people.insert(p);
                            }
                            m.activated |= !m.left_people.is_empty() && !m.right_people.is_empty();
                            m.violated |= !m.left_people.is_disjoint(&m.right_people);
                        }
                        Err(cause) => {
                            error.get_or_insert(cause);
                        }
                    }
                }
            }
            ir::ContractKind::Permitted { capability } if matches!(bundle, Bundle::Start(c, _) if c == *capability) =>
            {
                m.activated = true;
                let cap = &model.capabilities[capability.0];
                let allowed = cap
                    .permission
                    .as_ref()
                    .or(model.actors[cap.actor.0].executors.as_ref());
                match (bundle.executor(), allowed) {
                    (Ok(p), Some(allowed)) => m.violated |= !allowed.contains(&p),
                    _ => {
                        error.get_or_insert(Halt::from("unresolved Permission event"));
                    }
                }
            }
            ir::ContractKind::EvidenceBasis {
                capability,
                sources,
                ..
            } if matches!(bundle, Bundle::Finish(c, _, _) if c == *capability) => {
                m.activated = true;
                if binding.is_some_and(|b| model.site_contest(b).is_some()) {
                    m.evidence_issue = Some(evidence::Issue::Contest);
                    continue;
                }
                match binding.and_then(|b| state.evidence.as_ref().and_then(|h| h.values.get(&b))) {
                    Some(value) => {
                        let direct = matches!(
                            clause.kind,
                            ir::ContractKind::EvidenceBasis { direct: true, .. }
                        );
                        let roots = if direct {
                            model.capabilities[capability.0]
                                .reads
                                .iter()
                                .copied()
                                .collect()
                        } else {
                            value.roots.clone()
                        };
                        let unknown = if direct {
                            BTreeSet::new()
                        } else {
                            value.unknown.clone()
                        };
                        m.violated |= sources
                            .iter()
                            .any(|s| !roots.contains(s) && !unknown.contains(s));
                        if sources.iter().any(|s| unknown.contains(s)) {
                            m.evidence_issue = Some(evidence::Issue::Snapshot);
                        }
                    }
                    None => {
                        error.get_or_insert(Halt::from("evidence_basis Finish missing call"));
                    }
                }
            }
            ir::ContractKind::AuthorityBasis {
                capability,
                authority,
            } if matches!(bundle, Bundle::Finish(c, _, _) if c == *capability) => {
                m.activated = true;
                match bundle.executor() {
                    Ok(p) => {
                        match authority::holds(
                            model,
                            *authority,
                            *capability,
                            p,
                            state.time,
                            &state.state_values,
                            case_index,
                        ) {
                            Ok(holds) => m.violated |= !holds,
                            Err(cause) => {
                                error.get_or_insert(cause);
                            }
                        }
                    }
                    Err(cause) => {
                        error.get_or_insert(cause);
                    }
                }
            }
            ir::ContractKind::Invariant {
                event,
                condition: c,
            } if bundle.event(*event, model) => {
                m.activated = true;
                let evaluated = c
                    .as_ref()
                    .ok_or_else(|| Halt::from("active invariant has no resolved condition"))
                    .and_then(|c| condition_at(c, &state.results, &state.state_values, case_index));
                match evaluated {
                    Ok(holds) => m.violated |= !holds,
                    Err(cause) => {
                        error.get_or_insert(cause);
                    }
                }
            }
            ir::ContractKind::AtMostOnce { event } if bundle.event(*event, model) => {
                m.activated = true;
                m.count = (m.count + 1).min(2);
                m.violated |= m.count == 2;
            }
            ir::ContractKind::RequiresEvent { before, start } => {
                if bundle.event(*start, model) {
                    m.activated = true;
                    m.violated |= !m.before_seen;
                }
                m.before_seen |= bundle.event(*before, model);
            }
            ir::ContractKind::Response {
                trigger,
                targets,
                within,
            } => {
                if state.obligations.is_some() {
                    continue;
                }
                m.violated |= m.pending.iter().any(|d| *d < state.time);
                if bundle.trigger(*trigger, model) {
                    m.activated = true;
                    if let Some(deadline) = state.time.checked_add(*within) {
                        match m.pending.binary_search(&deadline) {
                            Ok(_) => {}
                            Err(i) => m.pending.insert(i, deadline),
                        }
                    } else {
                        error.get_or_insert(Halt::from("response deadline overflow"));
                    }
                }
                if targets.iter().any(|t| bundle.target(*t, model)) {
                    m.pending.retain(|d| *d < state.time);
                }
            }
            _ => {}
        }
    }
    if let Some(debts) = &state.obligations {
        for d in debts {
            let m = &mut state.monitors[d.response.0];
            m.activated = true;
            m.violated |= d
                .resolution
                .as_ref()
                .is_some_and(|r| r.status == crate::obligation::Status::Violated);
        }
    }
    error.map_or(Ok(()), Err)
}

fn event_text(event: ir::Event, model: &ir::Model) -> String {
    let c = &model.capabilities[event.capability.0];
    let kind = match event.kind {
        ir::EventKind::Start => "start".into(),
        ir::EventKind::Commit => "commit".into(),
        ir::EventKind::CancelAck => "cancel_ack".into(),
        ir::EventKind::Finish(None) => "finish".into(),
        ir::EventKind::Finish(Some(v)) => {
            format!("finish({})", model.types[c.return_type.0].members[v.0])
        }
    };
    format!("{}.{}.{kind}", model.actors[c.actor.0].name, c.name)
}

fn debt_resolution(
    d: &Debt,
    status: crate::obligation::Status,
    at: Option<u64>,
    reason: Option<String>,
) -> crate::obligation::Resolution {
    crate::obligation::Resolution {
        status,
        triggered_at: d.triggered_at,
        deadline: d.deadline,
        resolved_at: at,
        reason,
        by_clause: None,
        executor: None,
        authority: None,
    }
}

fn update_debts(
    state: &mut State,
    bundle: Bundle,
    site: Option<ir::BindingId>,
    model: &ir::Model,
) -> EngineResult<()> {
    use crate::obligation::Status;
    let Some(debts) = &mut state.obligations else {
        return Ok(());
    };
    for d in debts.iter_mut() {
        if d.resolution.is_none() && d.deadline < state.time {
            d.resolution = Some(debt_resolution(
                d,
                Status::Violated,
                Some(d.deadline),
                Some("expiry".into()),
            ));
        }
    }
    let mut error = None;
    for (i, clause) in model.contracts.iter().enumerate() {
        let Some((trigger, _, within)) = clause.response() else {
            continue;
        };
        if bundle.trigger(trigger, model) {
            if matches!(trigger, ir::Trigger::Event(_)) && site.is_none() {
                error.get_or_insert(Halt::from("response trigger missing binding"));
                continue;
            }
            if let Some(deadline) = state.time.checked_add(within) {
                debts.push(Debt {
                    response: ir::ClauseId(i),
                    site: if !matches!(trigger, ir::Trigger::Event(_)) {
                        None
                    } else {
                        site
                    },
                    triggered_at: state.time,
                    deadline,
                    resolution: None,
                });
            } else {
                error.get_or_insert(Halt::from("response deadline overflow"));
            }
        }
    }
    for d in debts.iter_mut() {
        let (_, targets, _) = model.contracts[d.response.0]
            .response()
            .expect("response debt");
        if d.resolution.is_none() && targets.iter().any(|t| bundle.target(*t, model)) {
            d.resolution = Some(debt_resolution(
                d,
                Status::Fulfilled,
                Some(state.time),
                None,
            ));
        }
    }
    for (i, clause) in model.contracts.iter().enumerate() {
        let ir::ContractKind::Waive {
            response,
            event,
            reason,
            authority,
        } = &clause.kind
        else {
            continue;
        };
        if !bundle.event(*event, model) {
            continue;
        }
        state.monitors[i].activated = true;
        let person = match bundle.executor() {
            Ok(p) => p,
            Err(e) => {
                error.get_or_insert(e);
                continue;
            }
        };
        if !model.authorities[authority.0]
            .grants
            .iter()
            .any(|g| g.capability == event.capability && g.persons.contains(&person))
        {
            error.get_or_insert(Halt::from("waiver executor lacks Authority"));
            continue;
        }
        for d in debts.iter_mut().filter(|d| {
            d.response == *response && d.resolution.is_none() && state.time <= d.deadline
        }) {
            let mut r = debt_resolution(d, Status::Waived, Some(state.time), Some(reason.clone()));
            r.by_clause = Some(clause.property_id(ir::ClauseId(i)));
            r.executor = Some(model.people[person.0].name.clone());
            r.authority = Some(model.authorities[authority.0].name.clone());
            d.resolution = Some(r);
        }
    }
    error.map_or(Ok(()), Err)
}

fn close_responses(state: &mut State) {
    if let Some(debts) = &mut state.obligations {
        for d in debts {
            if d.resolution.is_none() {
                d.resolution = Some(debt_resolution(
                    d,
                    crate::obligation::Status::Violated,
                    Some(state.time),
                    Some("terminal_unresolved".into()),
                ));
                state.monitors[d.response.0].violated = true;
            }
        }
    }
    for m in &mut state.monitors {
        m.violated |= !m.pending.is_empty();
    }
}

fn expire_debts(state: &mut State, horizon: u64, inclusive: bool) {
    if let Some(debts) = &mut state.obligations {
        for d in debts {
            if d.resolution.is_none()
                && (d.deadline < horizon || (inclusive && d.deadline == horizon))
            {
                d.resolution = Some(debt_resolution(
                    d,
                    crate::obligation::Status::Violated,
                    Some(d.deadline),
                    Some("expiry".into()),
                ));
                state.monitors[d.response.0].violated = true;
            }
        }
    }
}

#[derive(Default)]
struct Exploration {
    evidence_issues: BTreeMap<ir::ClauseId, evidence::Issue>,
    lost_updates: BTreeSet<ir::StateVarId>,
    // A valid emitted bundle may be followed by an independent clause error.
    // Keep its valuation for all-case monitoring, never as a new graph state.
    state_invariant_aborted_bundle: Option<State>,
    range_attempts: usize,
    range_case: Option<u64>,
    ranges: Vec<outcome::StateRangeViolation>,
    refusals: BTreeMap<ir::BindingId, outcome::CapabilityRefusal>,
    observed: crate::completion::Observed,
    completion: Option<crate::completion::Graph>,
    contest_alternatives: BTreeMap<usize, Vec<crate::contest::Alternative>>,
    contest_open: Option<outcome::Range>,
    bundle_aborted: bool,
    obligations: Vec<crate::obligation::Obligation>,
    trace: Option<trace::Capture>,
    metrics: Option<crate::metrics::BusinessMetrics>,
    duration_numeric: bool,
    #[cfg(test)]
    duration_clones: usize,
    partial_successors: Vec<State>,
    // Charged leaves withdrawn from numerical reduction, observed only by trace.
    trace_discarded: Vec<State>,
    chance_seen: bool,
    chance_groups: Vec<(std::ops::Range<usize>, Vec<Rational>)>,
    #[cfg(test)]
    kernel_draws: usize,
    #[cfg(test)]
    kernel_clones: usize,
    #[cfg(test)]
    state_events: Vec<(u64, Bundle, Vec<ir::StateValue>)>,
    #[cfg(test)]
    initial_clones: usize,
    #[cfg(test)]
    contest_clones: usize,
    #[cfg(test)]
    census: (usize, usize, usize),
    activation: Vec<bool>,
    violation: Vec<bool>,
    terminals: BTreeMap<ir::EndId, outcome::TerminalOutcome>,
    truncated: bool,
    overcommit: bool,
    deadlock: bool,
    budget_exhausted: bool,
    // ADR 0013: only the exact-arithmetic limit stopped the computation.
    numeric_only: bool,
}

impl Exploration {
    fn collect_debts(&mut self, state: &State, model: &ir::Model, reason: &str) {
        if !(crate::contest::has_effects(model)
            && reason == "terminal_unresolved"
            && state.contest.as_ref().is_some_and(|c| c.business.is_none()))
        {
            self.collect_contests(state, model, reason);
        }
        use crate::obligation::{Coverage, Obligation, Status};
        let Some(debts) = &state.obligations else {
            return;
        };
        for d in debts {
            let c = &model.contracts[d.response.0];
            let (trigger, targets, _) = c.response().expect("response debt");
            let site = d
                .site
                .map(|b| model.site_name(b))
                .unwrap_or_else(|| "received".into());
            let ordinal = c.ordinal.unwrap_or(d.response.0 + 1);
            let id = format!("case1:response{ordinal}:{site}");
            let resolution = d.resolution.clone().unwrap_or_else(|| {
                if reason == "clock" && d.deadline <= model.bounds.clock
                    || reason == "horizon" && model.horizon.is_some_and(|h| d.deadline <= h)
                {
                    debt_resolution(d, Status::Violated, Some(d.deadline), Some("expiry".into()))
                } else if reason == "deadlock_unresolved" {
                    debt_resolution(d, Status::Violated, Some(state.time), Some(reason.into()))
                } else {
                    debt_resolution(d, Status::Unresolved, None, Some(reason.into()))
                }
            });
            if resolution.status == Status::Violated {
                self.violation.resize(model.contracts.len(), false);
                self.violation[d.response.0] = true;
            }
            if let Some(row) = self.obligations.iter_mut().find(|r| r.id == id) {
                if !row.outcomes.contains(&resolution) {
                    row.outcomes.push(resolution);
                    row.outcomes.sort();
                }
                continue;
            }
            let (label, non_waivable) = match &c.kind {
                ir::ContractKind::NamedResponse {
                    label,
                    non_waivable,
                    ..
                } => (label.clone(), *non_waivable),
                _ => (None, false),
            };
            self.obligations.push(Obligation {
                id,
                response_id: c.property_id(d.response),
                response_label: label,
                case_id: 1,
                trigger_site: site,
                trigger: match trigger {
                    ir::Trigger::Received => "case.received".into(),
                    ir::Trigger::Admitted => "case.admitted".into(),
                    ir::Trigger::Event(e) => event_text(e, model),
                },
                targets: targets
                    .iter()
                    .map(|t| match t {
                        ir::Target::End(e) => format!("end.{}", model.ends[e.0]),
                        ir::Target::Event(e) => event_text(*e, model),
                    })
                    .collect(),
                debtor: crate::obligation::debtor(c, model).0,
                debtor_status: crate::obligation::debtor(c, model).1,
                non_waivable,
                coverage: Coverage::Complete,
                outcomes: vec![resolution],
            });
        }
        self.obligations.sort_by_key(|r| {
            (
                r.case_id,
                model
                    .contracts
                    .iter()
                    .enumerate()
                    .position(|(i, c)| c.property_id(ir::ClauseId(i)) == r.response_id)
                    .unwrap_or(usize::MAX),
                model
                    .process
                    .bindings
                    .iter()
                    .position(|b| b.name == r.trigger_site)
                    .map_or(0, |i| i + 1),
            )
        });
    }
    fn collect_contests(&mut self, state: &State, model: &ir::Model, reason: &str) {
        use crate::contest::{Alternative, IntegerRange, RuntimeStatus};
        use crate::metrics::{Range, Reason};
        let Some(c) = &state.contest else {
            return;
        };
        if !crate::contest::has_effects(model) && c.business.is_none() {
            return;
        }
        let complete = reason == "terminal_unresolved";
        let execution_closed = complete
            && c.routes.iter().all(|r| {
                r.filed != Some(true)
                    || (r.response_at.is_some() && r.violation != Some("missing_route"))
            });
        let open = u64::from(if crate::contest::has_effects(model) {
            c.end_open.unwrap_or(false)
        } else {
            !execution_closed
        });
        if matches!(reason, "terminal_unresolved" | "deadlock_unresolved") {
            if let Some(range) = &mut self.contest_open {
                extend(range, open);
            } else {
                self.contest_open = Some(outcome::Range {
                    min: open,
                    max: open,
                });
            }
        }
        for (id, declaration) in model.contests().iter().enumerate() {
            let route = &c.routes[id];
            let Some(target_at) = route.target_at else {
                continue;
            };
            let Some(filed) = route.filed else {
                continue;
            };
            let deadline = target_at.checked_add(declaration.respond_by);
            let violation = route.violation.or_else(|| {
                if route
                    .response_at
                    .zip(deadline)
                    .is_some_and(|(at, d)| at > d)
                {
                    Some("late_response")
                } else if filed && reason == "deadlock_unresolved" && route.response_at.is_none() {
                    Some("deadlock")
                } else {
                    None
                }
            });
            if violation.is_some() {
                for (i, clause) in model.contracts.iter().enumerate() {
                    if matches!(clause.kind, ir::ContractKind::ContestResponse { contest } if contest == id)
                    {
                        self.violation.resize(model.contracts.len(), false);
                        self.violation[i] = true;
                    }
                }
            }
            let range = |n| Some(IntegerRange { min: n, max: n });
            let observed_at = if reason == "clock" {
                model.bounds.clock
            } else {
                state.time
            };
            let unresolved_cut = filed
                && route.response_at.is_none()
                && !matches!(reason, "terminal_unresolved" | "deadlock_unresolved");
            let work = if route.violation == Some("refused") {
                Some(0)
            } else {
                route
                    .started_at
                    .map(|start| route.response_at.unwrap_or(observed_at) - start)
            };
            let mut cost = Range {
                min: None,
                max: None,
                reason: Some(if route.started_at.is_none() {
                    Reason::NeverCreated
                } else {
                    Reason::NotConfigured
                }),
            };
            if route.started_at.is_some()
                && let Some(spec) = &model.metrics
            {
                let cap = &model.capabilities[declaration.route.0];
                if let (Some(rate), Some(start)) = (
                    spec.actor_rates[cap.actor.0],
                    spec.start_rates[declaration.route.0],
                ) {
                    // Exact arithmetic; fee overflow is not silently saturated.
                    let amount = num_bigint::BigInt::from(rate)
                        * num_bigint::BigInt::from(work.unwrap_or(0))
                        + num_bigint::BigInt::from(if route.started_at.is_some() {
                            start
                        } else {
                            0
                        });
                    let n = crate::metrics::MetricNumber(num_rational::BigRational::from_integer(
                        amount,
                    ));
                    cost = Range {
                        min: Some(n.clone()),
                        max: Some(n),
                        reason: None,
                    };
                }
            }
            let alternative = Alternative {
                filed,
                target_at,
                filed_at: filed.then_some(target_at),
                deadline: if filed { deadline } else { None },
                status: if !filed {
                    RuntimeStatus::NotFiled
                } else if violation.is_some() {
                    RuntimeStatus::Violated
                } else if route.response_at.is_some() {
                    RuntimeStatus::Responded
                } else {
                    RuntimeStatus::Unresolved
                },
                response_at: route.response_at,
                closed_at: if !filed {
                    Some(target_at)
                } else {
                    route.response_at
                },
                executor: route.executor.map(|p| model.people[p.0].name.clone()),
                result: route.result.map(|v| {
                    model.types[model.capabilities[declaration.route.0].return_type.0].members[v.0]
                        .clone()
                }),
                violation_reason: violation
                    .or_else(|| {
                        (unresolved_cut
                            && deadline.is_some_and(|d| {
                                observed_at > d
                                    || (matches!(reason, "clock" | "horizon") && observed_at == d)
                            }))
                        .then_some("deadline_passed_unresolved")
                    })
                    .map(String::from),
                route_work: if unresolved_cut {
                    None
                } else {
                    work.and_then(range)
                },
                route_wait: if !unresolved_cut
                    && filed
                    && route.queued
                    && route.violation != Some("missing_route")
                {
                    range(route.started_at.unwrap_or(observed_at) - target_at)
                } else {
                    None
                },
                route_cost: cost,
            };
            let alternatives = self.contest_alternatives.entry(id).or_default();
            if !alternatives.contains(&alternative) {
                alternatives.push(alternative);
            }
        }
    }

    fn contest_tables(
        &self,
        model: &ir::Model,
        aborted: bool,
    ) -> (
        Option<Vec<crate::contest::Runtime>>,
        Option<crate::contest::Summary>,
    ) {
        if model.contests().is_empty() || !crate::contest::runtime_declarations_valid(model) {
            return (None, None);
        }
        let coverage =
            crate::obligation::Coverage::of(aborted, self.truncated, !self.ranges.is_empty());
        let rows = model
            .contests()
            .iter()
            .enumerate()
            .map(|(id, d)| crate::contest::Runtime {
                id: format!("case1:contest{}", id + 1),
                static_id: d.property_id(),
                case_id: 1,
                party: model.actors[d.party.0].name.clone(),
                target: d.target.name(model),
                target_kind: d.target.effect().then(|| "effect".into()),
                route: crate::responsibility::cap_name(model, d.route),
                respond_by: d.respond_by,
                basis: "モデル内での期限内応答".into(),
                coverage,
                alternatives: contest_table_alternatives(
                    self.contest_alternatives
                        .get(&id)
                        .cloned()
                        .unwrap_or_default(),
                    coverage,
                    model,
                    id,
                ),
            })
            .collect();
        let reason = coverage.reason();
        let r = self
            .contest_open
            .unwrap_or(outcome::Range { min: 0, max: 0 });
        let summary = crate::contest::Summary {
            business_ended_open: crate::metrics::Range {
                min: reason.is_none().then(|| r.min.into()),
                max: reason.is_none().then(|| r.max.into()),
                reason,
            },
        };
        (Some(rows), Some(summary))
    }

    fn metric_leaf(
        &mut self,
        state: &State,
        end: Option<ir::EndId>,
        deadlocked: bool,
        model: &ir::Model,
    ) {
        let Some(history) = &state.metric_history else {
            return;
        };
        let point = metrics::snapshot(
            model,
            &[metrics::CaseObservation {
                admitted_at: Some(0),
                state: Some(state),
                arrival: 0,
                end,
                rejected: false,
                deadlocked,
                not_arrived: false,
            }],
            &history.peak,
            state.time,
        );
        metrics::merge(&mut self.metrics, point);
    }
    fn contracts(&mut self, state: &State) {
        self.activation.resize(state.monitors.len(), false);
        self.violation.resize(state.monitors.len(), false);
        for (i, m) in state.monitors.iter().enumerate() {
            self.activation[i] |= m.activated;
            self.violation[i] |= m.violated;
            if let Some(issue) = m.evidence_issue {
                self.evidence_issues.insert(ir::ClauseId(i), issue);
            }
        }
    }

    fn contract_verdict(&self, id: ir::ClauseId, aborted: bool) -> Verdict {
        if self.violation.get(id.0) == Some(&true) {
            Verdict::Violated
        } else if self.evidence_issues.contains_key(&id) || aborted {
            Verdict::Unsupported
        } else if self.truncated || !self.ranges.is_empty() {
            Verdict::Inconclusive
        } else if self.activation.get(id.0) != Some(&true) {
            Verdict::Vacuous
        } else {
            Verdict::Satisfied
        }
    }

    fn observe(&mut self, state: &State, model: &ir::Model) -> EngineResult<()> {
        self.lost_updates.extend(state.lost_updates.iter().copied());
        for (r, used) in model.resources.iter().zip(state.resource_occupancy(model)?) {
            self.overcommit |= used > r.capacity;
        }
        let mut busy = BTreeSet::new();
        for task in &state.tasks {
            if task.lifecycle.running() {
                let actor = &model.actors[model.capabilities[task.capability.0].actor.0];
                match (&actor.executors, task.executor) {
                    (Some(pool), Some(p)) if pool.contains(&p) => {
                        self.overcommit |= !busy.insert(p);
                    }
                    (None, None) => {}
                    _ => return Err("inconsistent running executor".into()),
                }
            }
        }
        for (actor, count) in model.actors.iter().zip(state.occupancy(model)) {
            if count > 0 {
                let capacity = actor.capacity.ok_or("capacity-less task execution")?;
                self.overcommit |= count > capacity;
            }
        }
        Ok(())
    }

    fn terminal(&mut self, state: &State, end: ir::EndId) -> EngineResult<()> {
        if state.contest.as_ref().is_some_and(|c| c.business.is_some()) {
            return Ok(()); // Snapshot was recorded at the unique business End.
        }
        let point = |value| outcome::Range {
            min: value,
            max: value,
        };
        if let Some(terminal) = self.terminals.get_mut(&end) {
            if terminal.result_at.is_some() != state.result_at.is_some() {
                return Err("mixed adopted/unadopted paths to one terminal".into());
            }
            extend(&mut terminal.elapsed, state.time);
            extend(&mut terminal.work, state.work);
            if let (Some(range), Some(value)) = (&mut terminal.result_at, state.result_at) {
                extend(range, value);
            }
        } else {
            self.terminals.insert(
                end,
                outcome::TerminalOutcome {
                    probability: None,
                    end,
                    elapsed: point(state.time),
                    work: point(state.work),
                    result_at: state.result_at.map(point),
                },
            );
        }
        Ok(())
    }

    fn outcome(mut self, model: &ir::Model) -> outcome::Outcome {
        let completion = self.completion.take();
        let incomplete = self.truncated || !self.ranges.is_empty();
        let warnings = if incomplete {
            Vec::new()
        } else {
            self.observed.warnings(model)
        };
        let (contest_runtime, contest_summary) = self.contest_tables(model, false);
        let coverage =
            crate::obligation::Coverage::of(false, self.truncated, !self.ranges.is_empty());
        for row in &mut self.obligations {
            row.coverage = coverage;
        }
        let properties = model
            .verify
            .iter()
            .map(|verify| outcome::PropertyOutcome {
                cases: None,
                probability: None,
                property: verify.property,
                verdict: match verify.property {
                    ir::Property::NoLostUpdate(v) if self.lost_updates.contains(&v) => {
                        Verdict::Violated
                    }
                    ir::Property::StateInRange if !self.ranges.is_empty() => Verdict::Violated,
                    ir::Property::Contest(id) => crate::contest::verdict(model, id),
                    ir::Property::Contract(id) => self.contract_verdict(id, false),
                    ir::Property::NoResourceOvercommit if self.overcommit => Verdict::Violated,
                    ir::Property::ReachableEnd(end) if self.terminals.contains_key(&end) => {
                        Verdict::Satisfied
                    }
                    _ if incomplete => Verdict::Inconclusive,
                    ir::Property::StateInRange => Verdict::Satisfied,
                    ir::Property::NoLostUpdate(_) => Verdict::Satisfied,
                    ir::Property::NoResourceOvercommit => Verdict::Satisfied,
                    ir::Property::ReachableEnd(_) => Verdict::Violated,
                    ir::Property::ProbabilityReached(_)
                    | ir::Property::RecoverableEnd
                    | ir::Property::TerminatingEnd
                    | ir::Property::AlmostSureEnd => Verdict::Unsupported,
                },
            })
            .collect();
        let mut out = outcome::Outcome {
            response_exercise: None,
            response_receipts: None,
            response_recovery: None,
            scenario_observed_until: None,
            quality: None,
            state_range_violations: self.ranges,
            capability_refusals: model.has_requires().then(|| {
                self.refusals
                    .into_values()
                    .map(|mut r| {
                        r.coverage = coverage.word().into();
                        r
                    })
                    .collect()
            }),
            unchecked: if incomplete {
                vec!["unreachable".into()]
            } else {
                Vec::new()
            },
            postprocessing_diagnostics: Vec::new(),
            contest_runtime,
            contest_summary,
            obligations: model.tracks_obligations().then_some(self.obligations),
            metrics: metrics::finish(self.metrics, model, coverage),
            case_outcomes: None,
            admission: None,
            probability_scheduler: false,
            probability_policy_unresolved: None,
            properties,
            terminals: self.terminals.into_values().collect(),
            truncated: self.truncated,
            diagnostics: [
                (
                    self.deadlock,
                    diag::W_DEADLOCK_0,
                    "a reachable nonterminal state has no enabled transition",
                ),
                (
                    self.budget_exhausted,
                    diag::W_BUDGET_0,
                    if self.numeric_only {
                        "probability denominator exceeded the exact arithmetic limit"
                    } else {
                        "exploration stopped at the engine state budget"
                    },
                ),
            ]
            .into_iter()
            .filter(|(raised, ..)| *raised)
            .map(|(_, code, message)| diag::Diagnostic {
                code: code.into(),
                severity: diag::Severity::Warning,
                message: message.into(),
                span: None,
            })
            .collect(),
        };
        out.diagnostics.extend(warnings);
        if let Some(g) = completion {
            crate::completion::apply(&mut out, model, &g, false);
        }
        out
    }
}

impl Exploration {
    // An engine error ends exploration without a verdict on the rest, but a
    // counterexample or witness already found stays found (ADR 0006): the
    // overcommit is violated, a reached end is satisfied, a deadlock stays.
    fn abort(mut self, message: &'static str, model: &ir::Model) -> outcome::Outcome {
        let completion = self.completion.take();
        let (contest_runtime, contest_summary) = self.contest_tables(model, true);
        for row in &mut self.obligations {
            row.coverage = crate::obligation::Coverage::Aborted;
        }
        let properties = model
            .verify
            .iter()
            .map(|verify| outcome::PropertyOutcome {
                cases: None,
                probability: None,
                property: verify.property,
                verdict: match verify.property {
                    ir::Property::NoLostUpdate(v) if self.lost_updates.contains(&v) => {
                        Verdict::Violated
                    }
                    ir::Property::StateInRange if !self.ranges.is_empty() => Verdict::Violated,
                    ir::Property::Contest(id) => crate::contest::verdict(model, id),
                    ir::Property::Contract(id) => self.contract_verdict(id, true),
                    ir::Property::NoResourceOvercommit if self.overcommit => Verdict::Violated,
                    ir::Property::ReachableEnd(end) if self.terminals.contains_key(&end) => {
                        Verdict::Satisfied
                    }
                    _ => Verdict::Unsupported,
                },
            })
            .collect();
        let mut diagnostics = vec![diag::Diagnostic {
            code: diag::U_SEM_0.into(),
            severity: diag::Severity::Error,
            message: message.into(),
            span: None,
        }];
        if self.deadlock {
            diagnostics.push(diag::Diagnostic {
                code: diag::W_DEADLOCK_0.into(),
                severity: diag::Severity::Warning,
                message: "a reachable nonterminal state has no enabled transition".into(),
                span: None,
            });
        }
        // Terminal extrema of an incomplete exploration are not reported.
        let mut out = outcome::Outcome {
            response_exercise: None,
            response_receipts: None,
            response_recovery: None,
            scenario_observed_until: None,
            quality: None,
            state_range_violations: self.ranges,
            capability_refusals: model.has_requires().then(|| {
                self.refusals
                    .into_values()
                    .map(|mut r| {
                        r.coverage = "aborted".into();
                        r
                    })
                    .collect()
            }),
            unchecked: vec!["unreachable".into()],
            postprocessing_diagnostics: Vec::new(),
            contest_runtime,
            contest_summary,
            obligations: model.tracks_obligations().then_some(self.obligations),
            metrics: metrics::finish(self.metrics, model, crate::obligation::Coverage::Aborted),
            case_outcomes: None,
            admission: None,
            probability_scheduler: false,
            probability_policy_unresolved: None,
            properties,
            terminals: Vec::new(),
            truncated: false,
            diagnostics,
        };
        if let Some(g) = completion {
            crate::completion::apply(&mut out, model, &g, true);
        }
        out
    }
}

fn extend(range: &mut outcome::Range, value: u64) {
    range.min = range.min.min(value);
    range.max = range.max.max(value);
}

fn run(model: &ir::Model, deduplicate: bool, max_states: usize) -> outcome::Outcome {
    if model.uses_probability() {
        return run_probability(model, max_states);
    }
    run_initialized(model, deduplicate, max_states, std::convert::identity)
}

// The normal entry uses the initial state unchanged. Tests can inject a broken
// acquisition here while exercising the same bounds/observe/step loop as run.
fn run_initialized(
    model: &ir::Model,
    deduplicate: bool,
    max_states: usize,
    initialize: impl FnOnce(State) -> State,
) -> outcome::Outcome {
    let mut exploration = Exploration::default();
    match explore_into(model, deduplicate, max_states, initialize, &mut exploration) {
        Ok(()) => exploration.outcome(model),
        Err(Halt::Budget) => {
            exploration.truncated = true;
            exploration.budget_exhausted = true;
            exploration.outcome(model)
        }
        Err(Halt::Unsupported(message)) => exploration.abort(message, model),
    }
}

fn explore_into(
    model: &ir::Model,
    deduplicate: bool,
    max_states: usize,
    initialize: impl FnOnce(State) -> State,
    exploration: &mut Exploration,
) -> EngineResult<()> {
    if model.bounds.cases != 1 || model.verify.is_empty() {
        return Err("exploration requires one case and at least one property".into());
    }
    validate(model)?;
    if model.completion_requested() {
        exploration.completion = Some(crate::completion::Graph::default());
    }
    for cap in &model.capabilities {
        if cap.results.is_empty() || matches!(cap.cancel, ir::Cancel::Cooperative { ack_within: 0 })
        {
            return Err("empty result set or empty cancellation acknowledgement interval".into());
        }
    }
    let mut program = vec![Instruction::Stop];
    let entry = compile(&model.process.body, 0, &mut program);
    if max_states == 0 {
        return Err(Halt::Budget);
    }
    let mut initial = initialize(State::initial(model, entry));
    if initial.initialized {
        emit(&mut initial, Bundle::Received, model, exploration)?;
    }
    if let Some(trace) = &mut exploration.trace {
        trace.initial(&initial, model);
    }
    let mut pending = vec![initial];
    let mut completion_pending = exploration.completion.as_mut().map(|g| vec![g.node()]);
    let mut trace_pending = exploration.trace.as_ref().map(|_| vec![0usize]);
    let mut visited = std::collections::HashMap::new();
    let mut traced_visited = exploration
        .trace
        .as_ref()
        .map(|_| std::collections::HashMap::new());
    while let Some(state) = pending.pop() {
        let completion_node = completion_pending
            .as_mut()
            .map(|p| p.pop().expect("completion frontier"));
        if let (Some(g), Some(i)) = (&mut exploration.completion, completion_node) {
            g.nodes[i].target = state.contest.as_ref().is_some_and(|c| c.business.is_some());
        }
        let node = trace_pending
            .as_mut()
            .map(|p| p.pop().expect("parallel frontier"));
        if let (Some(trace), Some(node)) = (&mut exploration.trace, node) {
            trace.begin(node);
        }
        // A cut is not a reached execution state from which to infer a witness.
        if state.time > model.bounds.clock || state.exceeds_live_bound(model.bounds.live_tasks) {
            exploration.truncated = true;
            exploration.collect_debts(
                &state,
                model,
                if state.exceeds_live_bound(model.bounds.live_tasks) {
                    "live_tasks"
                } else {
                    "clock"
                },
            );
            continue;
        }
        if deduplicate {
            if let Some(keys) = &mut traced_visited {
                if let Some(&(earlier, previous_completion)) = keys.get(&state) {
                    if let (Some(g), Some(i)) = (&mut exploration.completion, completion_node) {
                        g.nodes[i].unknown = false;
                        g.action(i, vec![(Rational::one(), previous_completion)]);
                    }
                    if let (Some(trace), Some(node)) = (&mut exploration.trace, node) {
                        trace.merge(node, earlier);
                    }
                    continue;
                }
                keys.insert(
                    state.clone(),
                    (
                        node.expect("trace frontier node"),
                        completion_node.unwrap_or(0),
                    ),
                );
            } else {
                if let Some(&earlier) = visited.get(&state) {
                    if let (Some(g), Some(i)) = (&mut exploration.completion, completion_node) {
                        g.nodes[i].unknown = false;
                        g.action(i, vec![(Rational::one(), earlier)]);
                    }
                    continue;
                }
                visited.insert(state.clone(), completion_node.unwrap_or(0));
            }
        }
        let visited_count = traced_visited
            .as_ref()
            .map_or_else(|| visited.len(), |keys| keys.len());
        // Held = visited (including this state) plus the frontier. Exceeding
        // the budget is a cut; the room bounds successor generation itself.
        let held = visited_count.max(1).saturating_add(pending.len());
        #[cfg(test)]
        {
            exploration.census.0 = visited_count;
            exploration.census.2 = exploration.census.2.max(held);
        }
        if held > max_states {
            return Err(Halt::Budget);
        }
        exploration.observe(&state, model)?;
        exploration.contracts(&state);
        exploration.bundle_aborted = false;
        let ranges_before = exploration.range_attempts;
        let generated = step_observed(
            state.clone(),
            &program,
            model,
            &mut Room(max_states - held),
            exploration,
        );
        if let Err(halt) = generated {
            let reason = if halt == Halt::Budget {
                "state_budget"
            } else {
                "aborted"
            };
            if !exploration.bundle_aborted {
                exploration.collect_debts(&state, model, reason);
            }
            for next in &pending {
                exploration.collect_debts(next, model, reason);
            }
            for next in std::mem::take(&mut exploration.partial_successors) {
                exploration.collect_debts(&next, model, reason);
                exploration.partial_successors.push(next);
            }
            if let Some(trace) = &mut exploration.trace {
                for next in std::mem::take(&mut exploration.partial_successors) {
                    let node = trace.transition(&state, &next, model);
                    if let Some(&(existing, _)) =
                        traced_visited.as_ref().and_then(|keys| keys.get(&next))
                    {
                        trace.merge(node, existing);
                    }
                }
            }
            return Err(halt);
        }
        match generated? {
            Step::Next(states) => {
                if let (Some(g), Some(i), Some(frontier)) = (
                    &mut exploration.completion,
                    completion_node,
                    &mut completion_pending,
                ) {
                    g.nodes[i].unknown = exploration.range_attempts != ranges_before;
                    for state in &states {
                        let j = g.node();
                        g.nodes[j].target =
                            state.contest.as_ref().is_some_and(|c| c.business.is_some());
                        g.action(i, vec![(Rational::one(), j)]);
                        frontier.push(j);
                    }
                }
                if let (Some(trace), Some(frontier)) = (&mut exploration.trace, &mut trace_pending)
                {
                    for next in &states {
                        frontier.push(trace.transition(&state, next, model));
                    }
                }
                #[cfg(test)]
                {
                    exploration.census.1 += states.len();
                }
                pending.extend(states);
            }
            Step::Terminal(end) => {
                if let (Some(g), Some(i)) = (&mut exploration.completion, completion_node) {
                    g.nodes[i].target = true;
                    g.nodes[i].unknown = false;
                }
                exploration.metric_leaf(&state, Some(end), false, model);
                let mut closed = state.clone();
                if closed.contest.as_ref().is_none_or(|c| c.business.is_none()) {
                    emit(&mut closed, Bundle::End(end), model, exploration)?;
                }
                close_responses(&mut closed);
                exploration.contracts(&closed);
                exploration.collect_debts(&closed, model, "terminal_unresolved");
                if let Some(trace) = &mut exploration.trace {
                    trace.leaf(&closed, model, crate::explain::Kind::End);
                }
                exploration.terminal(&state, end)?;
            }
            Step::Cut => {
                exploration.truncated = true;
                exploration.collect_debts(
                    &state,
                    model,
                    if state.exceeds_live_bound(model.bounds.live_tasks) {
                        "live_tasks"
                    } else {
                        "clock"
                    },
                );
            }
            Step::Deadlock => {
                if let (Some(g), Some(i)) = (&mut exploration.completion, completion_node) {
                    g.nodes[i].unknown = false;
                    g.known_failure |= !g.nodes[i].target;
                }
                exploration.metric_leaf(&state, None, true, model);
                exploration.deadlock = true;
                let mut closed = state.clone();
                contest_deadlock(&mut closed, model);
                close_responses(&mut closed);
                exploration.contracts(&closed);
                exploration.collect_debts(&state, model, "deadlock_unresolved");
                if let Some(trace) = &mut exploration.trace {
                    trace.leaf(&closed, model, crate::explain::Kind::Deadlock);
                }
            }
        }
    }
    Ok(())
}

// lower guarantees these IR invariants; a direct builder may not. Check every
// index the engine dereferences so a malformed model is unsupported, not a panic.
#[cfg(test)]
mod actor_metadata_tests {
    use super::*;
    pub(super) fn census(model: &ir::Model) -> (usize, usize, usize) {
        if !model.legacy_route() {
            return arrivals::compatibility_counts(model);
        }
        let mut e = Exploration::default();
        if model.uses_probability() {
            let (nodes, _) = probability_graph(model, DEFAULT_STATE_BUDGET, &mut e).unwrap();
            (nodes.len(), e.census.1, e.census.2)
        } else {
            explore_into(
                model,
                true,
                DEFAULT_STATE_BUDGET,
                std::convert::identity,
                &mut e,
            )
            .unwrap();
            e.census
        }
    }
    #[test]
    fn quantile_declarations_preserve_private_state_and_full_census() {
        use std::hash::{Hash, Hasher};
        let mut runtime = 0;
        for name in crate::quantile_population::NAMES {
            let source = std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../fixtures")
                    .join(name)
                    .join("model.relca"),
            )
            .unwrap();
            let Ok(ast) = crate::syntax::parse(&source) else {
                continue;
            };
            let Ok(m) = crate::lower::lower(&ast) else {
                continue;
            };
            runtime += 1;
            let mut old = m.clone();
            old.quality.as_mut().unwrap().quantiles.clear();
            assert_eq!(m.probability_queries, old.probability_queries);
            assert_eq!(m.uses_probability(), old.uses_probability());
            assert_eq!(m.legacy_route(), old.legacy_route());
            assert_eq!(census(&m), census(&old), "{name}");
            let mut program = vec![Instruction::Stop];
            let entry = compile(&m.process.body, 0, &mut program);
            let (a, b) = (State::initial(&m, entry), State::initial(&old, entry));
            assert_eq!(a, b);
            let hash = |s: &State| {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                s.hash(&mut h);
                h.finish()
            };
            assert_eq!(hash(&a), hash(&b));
            assert_eq!(format!("{a:?}"), format!("{b:?}"));
        }
        assert_eq!(runtime, 43);
    }
    #[test]
    fn qp06_quality_is_not_a_state_key_query_or_probability_mode() {
        use std::hash::{Hash, Hasher};
        for name in [
            "qual-legacy",
            "qual-legacy-route",
            "qual-base",
            "qual-late-abort",
        ] {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
            let src = std::fs::read_to_string(root.join(name).join("model.relca")).unwrap();
            let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
            let mut old = m.clone();
            old.quality = None;
            assert_eq!(m.uses_probability(), old.uses_probability());
            assert_eq!(m.probability_queries, old.probability_queries);
            assert_eq!(census(&m), census(&old));
            let mut program = vec![Instruction::Stop];
            let entry = compile(&m.process.body, 0, &mut program);
            let a = State::initial(&m, entry);
            let b = State::initial(&old, entry);
            assert_eq!(a, b);
            let hash = |s: &State| {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                s.hash(&mut h);
                h.finish()
            };
            assert_eq!(hash(&a), hash(&b));
            let mut out = explore(&m);
            out.quality = None;
            assert_eq!(format!("{out:?}"), format!("{:?}", explore(&old)));
            let mut sim = simulate(&m);
            sim.quality = None;
            assert_eq!(sim, simulate(&old));
        }
    }
    #[test]
    fn qp09_quality_only_numeric_does_not_change_existing_facts_or_verdict() {
        let src = include_str!("../../../fixtures/qual-invoice/model.relca");
        let m = crate::lower::lower_configuration(&crate::syntax::parse(src).unwrap(), "AsIs")
            .unwrap()
            .model;
        let before = run(&m, true, DEFAULT_STATE_BUDGET);
        let quality = quality_legacy_with_bits(&m, DEFAULT_STATE_BUDGET, 1);
        assert_eq!(quality.cohort[0]["value"]["reason"], "truncated");
        assert!(quality.cohort[0]["value"]["lower"].is_null());
        let after = run(&m, true, DEFAULT_STATE_BUDGET);
        assert_eq!(before, after);
        assert!(!after.truncated);
        assert!(after.diagnostics.iter().all(|d| d.code != diag::W_BUDGET_0));
    }
    #[test]
    fn metadata_never_changes_private_state_generated_or_held_census() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut accepted = 0;
        let mut rejected = 0;
        for name in crate::actor_table::FIXTURES {
            let src = std::fs::read_to_string(root.join(name).join("model.relca")).unwrap();
            let ast = crate::syntax::parse(&src).unwrap();
            let Ok(model) = crate::lower::lower(&ast) else {
                rejected += 1;
                continue;
            };
            accepted += 1;
            let mut stripped = model.clone();
            for a in &mut stripped.actors {
                a.metadata = ir::ActorMetadata::default();
            }
            if let Some(c) = &mut stripped.cut {
                c.horizon.clear();
            }
            for c in &mut stripped.contracts {
                if let ir::ContractKind::NamedResponse { debtor, .. } = &mut c.kind {
                    *debtor = None;
                }
            }
            assert_eq!(
                census(&model),
                census(&stripped),
                "{name}: full private census"
            );
        }
        assert_eq!((accepted, rejected), (24, 4));
    }
}

#[cfg(test)]
pub(crate) fn quantile_private_census(m: &ir::Model) -> (usize, usize, usize) {
    actor_metadata_tests::census(m)
}

#[cfg(test)]
#[path = "../tests/support/actor_observed_population.rs"]
pub(crate) mod actor_observed_population;
#[cfg(test)]
#[path = "../tests/support/caseset_population.rs"]
pub(crate) mod caseset_population;

fn validate_instances(model: &ir::Model) -> EngineResult<()> {
    for (id, actor) in model.actors.iter().enumerate() {
        if actor.is_per_case()
            && (actor.capacity.is_some()
                || actor.metadata.capacity_declared
                || actor.executors.is_some()
                || model.capabilities.iter().any(|c| c.actor.0 == id))
        {
            return Err("per_case Actor cannot declare capacity, executors or Capability".into());
        }
    }
    Ok(())
}

mod record;
/// Independent execution-entry validation; Rule metadata is not executable state.
fn validate_rules(m: &ir::Model) -> EngineResult<()> {
    let fail = || Halt::from("invalid rule metadata or in-run rule access");
    let rule = |id: ir::ActorId| m.actors.get(id.0).and_then(|a| a.metadata.rule.as_ref());
    let mut owned = BTreeSet::new();
    for (owner, actor) in m.actors.iter().enumerate() {
        let Some(info) = &actor.metadata.rule else {
            continue;
        };
        if actor.is_per_case()
            || info.values.is_empty()
            || info.declared_version.is_empty()
            || info.effective_version.is_empty()
        {
            return Err(fail());
        }
        for value in &info.values {
            let v = m.state_variables.get(value.variable.0).ok_or_else(fail)?;
            let ty = m.types.get(v.base.0).ok_or_else(fail)?;
            let scalar = |x| matches!(x, ir::StateValue::Value(id) if id.0 < ty.members.len());
            if v.kind != ir::StateKind::Rule(ir::ActorId(owner))
                || v.optional
                || v.integer_max.is_some()
                || v.case_set_size.is_some()
                || ty.name == "PersonId"
                || ty.name == "CaseId"
                || ty.record.is_some()
                || ty.members.is_empty()
                || !scalar(value.declared)
                || !matches!(v.initial, ir::InitialValue::Fixed(x) if scalar(x))
                || !owned.insert(value.variable)
            {
                return Err(fail());
            }
            if info.amendment.is_none() && v.initial != ir::InitialValue::Fixed(value.declared) {
                return Err(fail());
            }
        }
        match &info.amendment {
            None if info.declared_version != info.effective_version => return Err(fail()),
            Some(a) => {
                let auth = m.authorities.get(a.under.0).ok_or_else(fail)?;
                if a.from != info.declared_version
                    || a.to != info.effective_version
                    || a.from == a.to
                    || a.by.0 >= m.people.len()
                    || auth.not_by.contains(&a.by)
                    || !auth
                        .rule_grants
                        .iter()
                        .any(|g| g.rule.0 == owner && g.persons.contains(&a.by))
                    || auth.under.as_ref().is_some_and(|b| {
                        rule(b.rule).is_none_or(|r| b.version != r.declared_version)
                    })
                {
                    return Err(fail());
                }
            }
            _ => {}
        }
    }
    for (i, v) in m.state_variables.iter().enumerate() {
        if matches!(v.kind, ir::StateKind::Rule(_)) && !owned.contains(&ir::StateVarId(i)) {
            return Err(fail());
        }
    }
    for a in &m.authorities {
        if (!a.rule_grants.is_empty() && a.restrictions.restricted())
            || (a.under.is_some() && !a.grants.is_empty())
            || a.under
                .as_ref()
                .is_some_and(|b| rule(b.rule).is_none() || b.version.is_empty())
        {
            return Err(fail());
        }
        for g in &a.rule_grants {
            if rule(g.rule).is_none()
                || g.persons.is_empty()
                || g.persons.iter().any(|p| p.0 >= m.people.len())
                || g.persons.iter().collect::<BTreeSet<_>>().len() != g.persons.len()
            {
                return Err(fail());
            }
        }
    }
    let is_rule = |id: ir::StateVarId| {
        m.state_variables
            .get(id.0)
            .is_some_and(|v| matches!(v.kind, ir::StateKind::Rule(_)))
    };
    for c in &m.capabilities {
        if c.start_writes
            .iter()
            .chain(&c.writes)
            .chain(c.atomic.iter().flat_map(|a| &a.writes))
            .any(|w| is_rule(w.variable))
            || c.kernel_writes
                .iter()
                .chain(&c.kernel_reads_latent)
                .any(|id| is_rule(*id))
        {
            return Err(fail());
        }
        if c.result_kernels().any(|k| {
            k.given.iter().any(|id| is_rule(*id))
                || k.rows
                    .iter()
                    .flat_map(|r| &r.branches)
                    .flat_map(|b| &b.overrides)
                    .any(|(id, _)| is_rule(*id))
        }) {
            return Err(fail());
        }
    }
    for k in &m.kernels {
        if k.given.iter().any(|id| is_rule(*id))
            || k.rows
                .iter()
                .flat_map(|r| &r.branches)
                .flat_map(|b| &b.overrides)
                .any(|(id, _)| is_rule(*id))
        {
            return Err(fail());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "engine/rule_tests.rs"]
mod rule_tests;

#[cfg(test)]
#[path = "engine/rule_integration_tests.rs"]
mod rule_integration_tests;

fn validate(model: &ir::Model) -> EngineResult<()> {
    work_profiles::validate(model)?;
    if model.uses_work_profiles()
        && (model.arrival_distribution.is_some()
            || !model.contests().is_empty()
            || model.response_attempts().iter().any(|(_, r)| {
                !matches!(r.loss, ir::ResponseLoss::None)
                    || r.max_attempts != 1
                    || r.retry_span.is_some()
            }))
    {
        return Err("q6_feature_combination".into());
    }
    degradation::validate(model)?;
    policy::validate_priority(model)?;
    response::validate(model)?;
    model.validate_cut_targets()?;
    record::validate(model)?;
    validate_rules(model)?;
    let limits: Vec<_> = model
        .admission_queue
        .iter()
        .chain(model.concurrent_cases.iter())
        .collect();
    if limits.windows(2).any(|q| q[0].overflow != q[1].overflow) {
        return Err("queue and concurrent limits disagree on overflow".into());
    }
    let mut overflow = None;
    for q in limits {
        match q.overflow {
            ir::Overflow::WaitExternalFifo | ir::Overflow::WaitExternalPriority(_)
                if q.capacity == 0 =>
            {
                return Err("external wait requires a positive limit".into());
            }
            ir::Overflow::End(e) => {
                if e.0 >= model.ends.len() || Some(e) != model.overflow_end {
                    return Err("invalid overflow terminal".into());
                }
                overflow = Some(e);
            }
            _ => {}
        }
    }
    if overflow != model.overflow_end {
        return Err("missing or spurious overflow terminal".into());
    }
    fn contains(b: &ir::Block, e: ir::EndId) -> bool {
        b.iter().any(|s| match &s.kind {
            ir::Stmt::Response(_, r) => r.exhausted == Some(e),
            ir::Stmt::End(x) => *x == e,
            ir::Stmt::Fork {
                join: ir::Join::Any { if_none, .. },
                ..
            } => *if_none == e,
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => contains(then_body, e) || contains(else_body, e),
            _ => false,
        })
    }
    if let Some(e) = overflow
        && (contains(&model.process.body, e)
            || model.contests().iter().any(|c| c.target.end() == Some(e))
            || model
                .responsibility
                .as_ref()
                .is_some_and(|r| r.standings.iter().any(|s| s.target.end() == Some(e))))
    {
        return Err("overflow terminal used by process or Standing".into());
    }
    arrivals::validate_scenario(model)?;
    policy::validate(model)?;
    validate_instances(model)?;
    let mut resource_names = BTreeSet::new();
    for r in &model.resources {
        if r.name.is_empty()
            || !resource_names.insert(&r.name)
            || model.actors.iter().any(|a| a.name == r.name)
        {
            return Err("invalid resource declaration".into());
        }
    }
    for c in &model.capabilities {
        let mut seen = BTreeSet::new();
        for u in &c.uses {
            if u.resource.0 >= model.resources.len() || u.amount == 0 || !seen.insert(u.resource) {
                return Err("invalid resource use".into());
            }
        }
    }
    crate::actor_table::validate(model)
        .map_err(|_| Halt::from("invalid Actor metadata or debtor"))?;
    metrics::validate(model)?;
    let ok = |valid: bool, message: &'static str| {
        if valid {
            Ok(())
        } else {
            Err(Halt::from(message))
        }
    };
    let members = |ty: ir::TypeId| model.types.get(ty.0).map(|t| t.members.len());
    let persons = |ps: &[ir::PersonId]| {
        ps.iter().all(|p| p.0 < model.people.len())
            && ps.iter().collect::<BTreeSet<_>>().len() == ps.len()
    };
    for v in &model.state_variables {
        ok(
            v.case_set_size.is_some()
                || v.integer_max.is_some()
                || members(v.base).is_some_and(|n| {
                    n > 0 || (v.optional && model.types[v.base.0].name == "PersonId")
                }),
            "invalid state base",
        )?;
        ok(
            v.integer_max.is_none()
                || (!v.optional && !matches!(v.kind, ir::StateKind::ActorLatent(_))),
            "unsupported integer domain location",
        )?;
        ok(
            v.kind.actor().is_none_or(|a| a.0 < model.actors.len()),
            "invalid latent Actor",
        )?;
        ok(v.kind.is_observed() || !v.optional, "optional latent")?;
        if let Some(n) = v.case_set_size {
            ok(
                n > 0
                    && n <= 64
                    && u64::from(n) == model.bounds.cases
                    && v.base == ir::CASE_ID_TYPE
                    && v.integer_max.is_none()
                    && !v.optional
                    && model.is_shared_observed(v.kind)
                    && v.initial == ir::InitialValue::Fixed(ir::StateValue::Set(0)),
                "invalid caseset storage",
            )?;
        }
        if model
            .types
            .get(v.base.0)
            .is_some_and(|t| t.name == "PersonId")
        {
            ok(
                v.kind.is_observed()
                    && model.types[v.base.0].members
                        == model
                            .people
                            .iter()
                            .map(|p| p.name.clone())
                            .collect::<Vec<_>>(),
                "invalid PersonId state",
            )?;
        }
        match v.initial {
            ir::InitialValue::Distribution(id) => {
                let d = model
                    .initial_distributions
                    .get(id)
                    .ok_or("invalid distribution index")?;
                let mut sum = Rational::zero();
                let mut values = HashSet::new();
                for (x, p) in d {
                    ok(
                        valid_state_value(v, *x, model) && values.insert(*x) && p.valid(),
                        "invalid initial distribution",
                    )?;
                    sum.0 += &p.0;
                }
                ok(
                    v.kind.is_latent()
                        && !v.optional
                        && values.len() as u128 == v.cardinality(&model.types)
                        && sum == Rational::one(),
                    "invalid distribution domain/mass",
                )?;
            }
            ir::InitialValue::Fixed(x) => ok(
                valid_state_value(v, x, model),
                "invalid state initial value",
            )?,
            ir::InitialValue::Nondet => {
                ok(v.kind.is_latent() && !v.optional, "invalid nondet initial")?
            }
        }
    }
    ok(
        model
            .people
            .iter()
            .map(|p| &p.name)
            .collect::<BTreeSet<_>>()
            .len()
            == model.people.len(),
        "duplicate person",
    )?;
    for actor in &model.actors {
        if let Some(pool) = &actor.executors {
            ok(
                !pool.is_empty() && persons(pool) && actor.capacity == Some(pool.len() as u64),
                "invalid executor pool",
            )?;
        }
    }
    for a in &model.authorities {
        ok(
            (!a.grants.is_empty() || !a.rule_grants.is_empty())
                && persons(&a.not_by)
                && a.issued_by
                    .is_none_or(|p| p.0 < model.people.len() && !a.not_by.contains(&p)),
            "invalid Authority",
        )?;
        for g in &a.grants {
            ok(
                g.capability.0 < model.capabilities.len()
                    && !g.persons.is_empty()
                    && persons(&g.persons)
                    && g.persons.iter().all(|p| !a.not_by.contains(p)),
                "invalid grant",
            )?;
        }
    }
    authority::validate(model)?;
    ok(
        model
            .authorities
            .iter()
            .map(|a| &a.name)
            .collect::<BTreeSet<_>>()
            .len()
            == model.authorities.len(),
        "duplicate Authority",
    )?;
    for cap in &model.capabilities {
        fn precondition(
            c: &ir::Cond,
            m: &ir::Model,
            cap: &ir::Capability,
            depth: usize,
        ) -> EngineResult<()> {
            if depth >= 128 {
                return Err("requires nesting limit".into());
            }
            match c {
                ir::Cond::Contains { set, element } => {
                    if !m
                        .state_variables
                        .get(set.0)
                        .is_some_and(|v| v.case_set_size.is_some())
                        || matches!(element, ir::Element::Parameter(i) if cap.params.get(*i).is_none_or(|p| p.ty != ir::CASE_ID_TYPE))
                    {
                        return Err("invalid membership precondition".into());
                    }
                }
                ir::Cond::IntCompare { variable, .. } => {
                    if !m
                        .state_variables
                        .get(variable.0)
                        .is_some_and(|v| v.kind.is_observed() && v.integer_max.is_some())
                    {
                        return Err("invalid integer requires".into());
                    }
                }
                ir::Cond::StateEq(x) => {
                    let v = m
                        .state_variables
                        .get(x.variable.0)
                        .ok_or("invalid requires state")?;
                    if v.case_set_size.is_some()
                        || v.kind.is_latent()
                        || !valid_state_value(v, x.value, m)
                    {
                        return Err("invalid requires state/type".into());
                    }
                }
                ir::Cond::And(cs) | ir::Cond::Or(cs) => {
                    if cs.len() < 2 {
                        return Err("invalid requires chain".into());
                    }
                    for c in cs {
                        precondition(c, m, cap, depth + 1)?;
                    }
                }
                ir::Cond::Eq(_) => return Err("requires cannot capture result binding".into()),
            }
            Ok(())
        }
        if let Some(req) = &cap.requires {
            precondition(&req.condition, model, cap, 0)?;
            ok(
                members(cap.return_type).is_some_and(|n| req.otherwise.0 < n),
                "invalid requires else type",
            )?;
        }
        let mut start_targets = BTreeSet::new();
        let writable = |v: &ir::StateVariable| match v.kind {
            ir::StateKind::ObservedCase => true,
            ir::StateKind::ActorObserved(a) => {
                a == cap.actor && model.actors.get(a.0).is_some_and(|a| !a.is_per_case())
            }
            _ => false,
        };
        for w in &cap.start_writes {
            let v = model
                .state_variables
                .get(w.variable.0)
                .ok_or("invalid start target")?;
            ok(
                writable(v) && start_targets.insert(w.variable),
                "invalid or duplicate start target",
            )?;
            ok(
                matches!(w.source, ir::WriteSource::Constant(x) if v.case_set_size.is_none() && valid_state_value(v,x,model))
                    || new_write_valid(w, cap, model)
                    || matches!(w.source, ir::WriteSource::Integer { operand, .. } if v.integer_max.is_some() && operand.unsigned_abs() <= u128::from(u64::MAX)),
                "nonconstant or mistyped start",
            )?;
        }
        let mut targets = BTreeSet::new();
        for w in cap.finish_writes() {
            let v = model
                .state_variables
                .get(w.variable.0)
                .ok_or("unknown state write target")?;
            ok(
                writable(v) && targets.insert(w.variable),
                "invalid or duplicate state write",
            )?;
            ok(
                match w.source {
                    ir::WriteSource::InsertCase { .. } | ir::WriteSource::Executor => {
                        new_write_valid(w, cap, model)
                    }
                    ir::WriteSource::Integer { operand, .. } => {
                        v.integer_max.is_some() && operand.unsigned_abs() <= u128::from(u64::MAX)
                    }
                    ir::WriteSource::Result => {
                        v.case_set_size.is_none() && v.base == cap.return_type
                    }
                    ir::WriteSource::Constant(x) => {
                        v.case_set_size.is_none() && valid_state_value(v, x, model)
                    }
                },
                "invalid state write value/type",
            )?;
        }
        ok(
            cap.actor.0 < model.actors.len(),
            "capability actor out of range",
        )?;
        if let Some(d) = &cap.duration_distribution {
            ok(
                cap.duration == 0 && !d.entries.is_empty(),
                "invalid duration distribution representation",
            )?;
            ok(
                d.entries.windows(2).all(|w| w[0].0 < w[1].0),
                "unsorted or duplicate durations",
            )?;
            let mut sum = Rational::zero();
            for (t, p) in &d.entries {
                ok(
                    p.valid() && (p.is_zero() || *t > 0),
                    "invalid duration probability or zero time",
                )?;
                sum.0 += &p.0;
            }
            ok(
                sum == Rational::one() && model.actors[cap.actor.0].capacity.is_some(),
                "invalid duration sum or missing capacity",
            )?;
        }
        let count = members(cap.return_type).ok_or("capability type out of range")?;
        ok(!cap.results.is_empty(), "empty result set")?;
        if let Some(a) = &cap.atomic {
            precondition(&a.condition, model, cap, 0)?;
            ok(
                cap.kernel.is_none()
                    && cap.writes.is_empty()
                    && !a.writes.is_empty()
                    && a.commit != a.reject
                    && a.commit.0 < count
                    && a.reject.0 < count
                    && cap.results.len() == 2
                    && cap.results.contains(&a.commit)
                    && cap.results.contains(&a.reject),
                "invalid atomic shape",
            )?;
        }
        if let Some(permission) = &cap.permission {
            ok(
                !permission.is_empty()
                    && persons(permission)
                    && model.actors[cap.actor.0].executors.is_some(),
                "invalid Permission",
            )?;
        }
        ok(
            cap.results.iter().all(|value| value.0 < count),
            "capability result out of range",
        )?;
        let mut read_ids = BTreeSet::new();
        for id in &cap.kernel_reads_latent {
            ok(
                model
                    .state_variables
                    .get(id.0)
                    .is_some_and(|v| v.kind.is_latent())
                    && read_ids.insert(*id),
                "invalid kernel_reads",
            )?;
        }
        for id in &cap.kernel_writes {
            ok(
                (cap.kernel.is_some() || cap.performance.is_some())
                    && model
                        .state_variables
                        .get(id.0)
                        .is_some_and(|v| v.kind.is_latent())
                    && targets.insert(*id),
                "invalid latent kernel writes",
            )?;
        }
        if cap.kernel.is_some() || cap.performance.is_some() {
            let mut support = BTreeSet::new();
            for k in cap.result_kernels() {
                validate_kernel(k, model)?;
                ok(
                    k.given.iter().all(|id| {
                        model.state_variables[id.0].kind == ir::StateKind::ObservedCase
                            || read_ids.contains(id)
                    }),
                    "missing latent read",
                )?;
                for b in k.rows.iter().flat_map(|r| &r.branches) {
                    ok(b.result.0 < count, "invalid kernel result type")?;
                    if !b.probability.is_zero() {
                        support.insert(b.result);
                    }
                    let mut seen = BTreeSet::new();
                    for (id, x) in &b.overrides {
                        ok(
                            targets.contains(id)
                                && seen.insert(*id)
                                && !matches!(
                                    model.state_variables[id.0].kind,
                                    ir::StateKind::ActorObserved(_)
                                ),
                            "invalid override target",
                        )?;
                        ok(
                            valid_state_value(&model.state_variables[id.0], *x, model),
                            "invalid override value",
                        )?;
                    }
                    ok(
                        cap.kernel_writes.iter().all(|id| seen.contains(id)),
                        "missing latent override",
                    )?;
                }
            }
            ok(
                cap.results.iter().copied().collect::<BTreeSet<_>>() == support
                    && support.len() == cap.results.len(),
                "inconsistent positive result support",
            )?;
        }
    }
    for k in &model.kernels {
        validate_kernel(k, model)?;
    }
    for binding in &model.process.bindings {
        ok(members(binding.ty).is_some(), "binding type out of range")?;
    }
    let end = |end: ir::EndId| ok(end.0 < model.ends.len(), "end out of range");
    let test = |test: &ir::ResultMatch| {
        let binding = model
            .process
            .bindings
            .get(test.binding.0)
            .ok_or("condition binding out of range")?;
        ok(
            members(binding.ty).is_some_and(|count| test.value.0 < count),
            "condition value out of range",
        )
    };
    let mut bound: Vec<_> = model
        .process
        .bindings
        .iter()
        .map(|b| b.merge.is_some())
        .collect();
    let mut call = |call: &ir::Call| {
        let cap = model
            .capabilities
            .get(call.capability.0)
            .ok_or("call capability out of range")?;
        let binding = model
            .process
            .bindings
            .get(call.binding.0)
            .ok_or("call binding out of range")?;
        ok(binding.ty == cap.return_type, "call binding type mismatch")?;
        ok(
            call.args.len() == cap.params.len(),
            "call argument count mismatch",
        )?;
        for (arg, p) in call.args.iter().zip(&cap.params) {
            ok(
                if p.ty == ir::CASE_ID_TYPE {
                    *arg == ir::CURRENT_CASE
                } else {
                    *arg != ir::CURRENT_CASE
                        && model
                            .process
                            .bindings
                            .get(arg.0)
                            .is_some_and(|b| b.ty == p.ty)
                },
                "invalid call argument type",
            )?;
        }
        ok(
            !std::mem::replace(&mut bound[call.binding.0], true),
            "binding defined by two call sites",
        )
    };
    fn cond(
        c: &ir::Cond,
        test: &impl Fn(&ir::ResultMatch) -> EngineResult<()>,
        model: &ir::Model,
        policy: bool,
    ) -> EngineResult<()> {
        match c {
            ir::Cond::Contains { set, element } => {
                if *element == ir::Element::CurrentCase
                    && model
                        .state_variables
                        .get(set.0)
                        .is_some_and(|v| v.case_set_size.is_some())
                {
                    Ok(())
                } else {
                    Err("invalid membership condition".into())
                }
            }
            ir::Cond::IntCompare { variable, .. } => {
                if model
                    .state_variables
                    .get(variable.0)
                    .is_some_and(|v| v.integer_max.is_some() && (!policy || v.kind.is_observed()))
                {
                    Ok(())
                } else {
                    Err("invalid integer predicate".into())
                }
            }
            ir::Cond::StateEq(t) => {
                let v = model
                    .state_variables
                    .get(t.variable.0)
                    .ok_or("unknown state comparison")?;
                if v.case_set_size.is_some()
                    || !valid_state_value(v, t.value, model)
                    || (policy && v.kind.is_latent())
                {
                    return Err("invalid state comparison".into());
                }
                Ok(())
            }
            ir::Cond::Eq(t) => test(t),
            ir::Cond::And(operands) | ir::Cond::Or(operands) if operands.len() >= 2 => operands
                .iter()
                .try_for_each(|operand| cond(operand, test, model, policy)),
            _ => Err("and/or with fewer than two operands".into()),
        }
    }
    let mut blocks = vec![&model.process.body];
    if let Some(q) = &model.quality {
        ok(
            !q.properties.is_empty() || !q.metrics.is_empty(),
            "invalid quality declaration",
        )?;
        let mut names = std::collections::BTreeSet::new();
        for p in &q.properties {
            ok(
                !p.name.is_empty()
                    && !matches!(p.name.as_str(), "received" | "admitted" | "id")
                    && names.insert(&p.name),
                "invalid quality declaration",
            )?;
            match &p.target {
                ir::QualityTarget::Ends(es) => {
                    let mut seen = std::collections::BTreeSet::new();
                    ok(!es.is_empty(), "invalid quality declaration")?;
                    for e in es {
                        end(*e)?;
                        ok(seen.insert(e.0), "invalid quality declaration")?;
                    }
                }
                ir::QualityTarget::State(c) => {
                    let mut pending = vec![c];
                    while let Some(c) = pending.pop() {
                        match c {
                            ir::Cond::Contains { .. } | ir::Cond::Eq(_) => {
                                return Err("invalid quality declaration".into());
                            }
                            ir::Cond::And(cs) | ir::Cond::Or(cs) => pending.extend(cs),
                            ir::Cond::StateEq(ir::StateMatch { variable, .. })
                            | ir::Cond::IntCompare { variable, .. } => {
                                let v = model
                                    .state_variables
                                    .get(variable.0)
                                    .ok_or("invalid quality declaration")?;
                                if let (Some(a), Some(cut)) = (v.kind.actor(), &model.cut) {
                                    ok(cut.include.contains(&a), "invalid quality declaration")?;
                                }
                            }
                        }
                    }
                    cond(
                        c,
                        &|_| Err("invalid quality declaration".into()),
                        model,
                        false,
                    )?;
                }
            }
        }
        names.clear();
        for c in &q.quantiles {
            ok(
                !c.name.is_empty()
                    && names.insert(&c.name)
                    && c.property < q.properties.len()
                    && c.p.0.denom() > &num_bigint::BigInt::from(0)
                    && c.p.valid()
                    && c.p != Rational::zero(),
                "invalid quantile declaration",
            )?;
        }
        for c in &q.metrics {
            ok(
                names.insert(&c.name)
                    && !c.name.is_empty()
                    && c.numerator < q.properties.len()
                    && match c.denominator {
                        ir::Denominator::Arrivals => true,
                        ir::Denominator::Property(i) => i < q.properties.len(),
                    },
                "invalid quality declaration",
            )?;
        }
    }
    for q in &model.probability_queries {
        ok(q.bound.valid(), "invalid probability bound")?;
        match &q.condition {
            ir::ProbabilityCondition::End(e) => end(*e)?,
            ir::ProbabilityCondition::State(c) => {
                let mut pending = vec![c];
                while let Some(c) = pending.pop() {
                    match c {
                        ir::Cond::Contains { .. } => {
                            return Err("membership probability condition unsupported".into());
                        }
                        ir::Cond::And(cs) | ir::Cond::Or(cs) => pending.extend(cs),
                        _ => {}
                    }
                }
                cond(
                    c,
                    &|_| Err("probability query reads binding".into()),
                    model,
                    false,
                )?;
            }
        }
    }
    while let Some(block) = blocks.pop() {
        for statement in block {
            match &statement.kind {
                ir::Stmt::MergeCopy { binding, source } => {
                    let target = model
                        .process
                        .bindings
                        .get(binding.0)
                        .ok_or("invalid merge target")?;
                    let value = model
                        .process
                        .bindings
                        .get(source.0)
                        .ok_or("invalid merge source")?;
                    ok(
                        target.ty == value.ty
                            && target
                                .merge
                                .as_ref()
                                .is_some_and(|m| m.sources.contains(source))
                            && binding != source,
                        "invalid merge copy",
                    )?;
                }
                ir::Stmt::LoopControl(control) => {
                    for c in &control.aliases {
                        call(c)?;
                    }
                    for (id, value) in &control.updates {
                        ok(
                            model.state_variables.get(id.0).is_some_and(|v| {
                                v.name.starts_with("@loop:")
                                    && v.kind == ir::StateKind::ObservedCase
                                    && v.contains(&model.types, *value)
                            }),
                            "invalid loop control",
                        )?;
                    }
                }
                ir::Stmt::Wait(_) | ir::Stmt::WaitUntil(_) => {}
                ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => call(c)?,
                ir::Stmt::ResolveEffect { .. } | ir::Stmt::StopReceiptUnknown => {}
                ir::Stmt::Response(c, r) => {
                    call(c)?;
                    if r.attempt == 1 {
                        call(&r.command_call)?;
                        if let Some(body) = &r.recovery {
                            blocks.push(body);
                        }
                    }
                }
                ir::Stmt::Fork { id, branches, join } => {
                    ok(id.0 < model.process.forks.len(), "fork out of range")?;
                    ok(!branches.is_empty(), "fork without branches")?;
                    branches.iter().try_for_each(&mut call)?;
                    for (i, left) in branches.iter().enumerate() {
                        let a = &model.capabilities[left.capability.0];
                        for right in &branches[i + 1..] {
                            let b = &model.capabilities[right.capability.0];
                            if a.atomic.is_some()
                                || b.atomic.is_some()
                                || a.start_writes
                                    .iter()
                                    .chain(a.finish_writes())
                                    .chain(&b.start_writes)
                                    .chain(b.finish_writes())
                                    .any(|w| {
                                        matches!(
                                            w.source,
                                            ir::WriteSource::InsertCase { .. }
                                                | ir::WriteSource::Executor
                                        )
                                    })
                            {
                                ok(
                                    !a.finish_writes()
                                        .chain(&a.start_writes)
                                        .map(|w| w.variable)
                                        .chain(a.kernel_writes.iter().copied())
                                        .any(|x| {
                                            b.finish_writes()
                                                .chain(&b.start_writes)
                                                .map(|w| w.variable)
                                                .chain(b.kernel_writes.iter().copied())
                                                .any(|y| x == y)
                                        }),
                                    "concurrent identity/atomic writers",
                                )?;
                            }
                        }
                    }
                    if let ir::Join::Any {
                        matches,
                        tie,
                        if_none,
                        ..
                    } = join
                    {
                        end(*if_none)?;
                        matches.iter().try_for_each(test)?;
                        let in_fork = |b: &ir::BindingId| branches.iter().any(|c| c.binding == *b);
                        ok(
                            matches.iter().all(|m| in_fork(&m.binding)),
                            "any match outside its fork",
                        )?;
                        if let ir::Tie::By(order) = tie {
                            ok(order.iter().all(in_fork), "tie order outside its fork")?;
                        }
                    }
                }
                ir::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    cond(condition, &test, model, true)?;
                    blocks.extend([then_body, else_body]);
                }
                ir::Stmt::End(e) => end(*e)?,
            }
        }
    }
    let event = |e: ir::Event| -> EngineResult<()> {
        let cap = model
            .capabilities
            .get(e.capability.0)
            .ok_or("contract capability out of range")?;
        match e.kind {
            ir::EventKind::CancelAck => Err("cancel_ack outside excludes antecedent".into()),
            ir::EventKind::Commit => ok(cap.commits, "commit without commits"),
            ir::EventKind::Finish(Some(v)) => ok(
                v.0 < members(cap.return_type).ok_or("contract type out of range")?,
                "contract finish value out of range",
            ),
            _ => Ok(()),
        }
    };
    let mut d1_ordinals = BTreeSet::new();
    let mut d1_labels = BTreeSet::new();
    crate::evidence_basis::validate(model)?;
    for c in &model.contracts {
        if model.tracks_obligations() {
            if let Some(n) = c.ordinal {
                ok(
                    n > 0 && d1_ordinals.insert(n),
                    "duplicate or zero clause ordinal",
                )?;
            }
            if let ir::ContractKind::NamedResponse {
                label: Some(label), ..
            } = &c.kind
            {
                ok(
                    !label.is_empty()
                        && (c.name.is_none() || d1_labels.insert((c.name.clone(), label.clone()))),
                    "duplicate response label",
                )?;
            }
        }
        match &c.kind {
            ir::ContractKind::ContestResponse { contest } => {
                let declaration = model
                    .contests()
                    .get(*contest)
                    .ok_or("contest out of range")?;
                ok(
                    c.ordinal == Some(declaration.ordinal)
                        && c.name == declaration.name
                        && c.span == declaration.span,
                    "contest runtime clause does not match declaration",
                )?;
            }
            ir::ContractKind::Excludes { before, start } => {
                if before.kind == ir::EventKind::CancelAck {
                    ok(
                        before.capability.0 < model.capabilities.len(),
                        "excludes capability out of range",
                    )?;
                } else {
                    event(*before)?;
                }
                event(*start)?;
                ok(
                    start.kind == ir::EventKind::Start,
                    "excludes requires start",
                )?;
            }
            ir::ContractKind::Waive {
                response,
                event: e,
                authority,
                ..
            } => {
                event(*e)?;
                ok(
                    matches!(e.kind, ir::EventKind::Finish(_) | ir::EventKind::Commit),
                    "invalid waiver event",
                )?;
                let target = model
                    .contracts
                    .get(response.0)
                    .ok_or("waiver response out of range")?;
                ok(
                    target.response().is_some()
                        && !matches!(
                            target.kind,
                            ir::ContractKind::NamedResponse {
                                non_waivable: true,
                                ..
                            }
                        ),
                    "invalid waiver target",
                )?;
                let a = model
                    .authorities
                    .get(authority.0)
                    .ok_or("waiver Authority out of range")?;
                let cap = &model.capabilities[e.capability.0];
                let ps = model.actors[cap.actor.0]
                    .executors
                    .as_ref()
                    .ok_or("anonymous waiver")?;
                ok(
                    !ps.is_empty()
                        && ps.iter().all(|p| {
                            a.grants
                                .iter()
                                .any(|g| g.capability == e.capability && g.persons.contains(p))
                        }),
                    "incomplete waiver grant",
                )?;
            }
            ir::ContractKind::Separation { left, right } => {
                event(*left)?;
                event(*right)?;
                ok(left != right, "identical separation events")?;
                for e in [left, right] {
                    ok(
                        model.actors[model.capabilities[e.capability.0].actor.0]
                            .executors
                            .is_some(),
                        "anonymous separation",
                    )?;
                }
            }
            ir::ContractKind::EvidenceBasis { .. } => {} // Independently checked above.
            ir::ContractKind::Permitted { capability }
            | ir::ContractKind::AuthorityBasis { capability, .. } => {
                let cap = model
                    .capabilities
                    .get(capability.0)
                    .ok_or("contract capability out of range")?;
                ok(
                    model.actors[cap.actor.0].executors.is_some(),
                    "anonymous authority or permission",
                )?;
                if let ir::ContractKind::AuthorityBasis { authority, .. } = c.kind {
                    let a = model
                        .authorities
                        .get(authority.0)
                        .ok_or("Authority out of range")?;
                    ok(
                        a.grants.iter().any(|g| g.capability == *capability),
                        "missing target grant",
                    )?;
                }
            }
            ir::ContractKind::Invariant {
                event: e,
                condition,
            } => {
                event(*e)?;
                if let Some(c) = condition {
                    cond(c, &test, model, false)?;
                } else {
                    fn calls(block: &ir::Block, cap: ir::CapabilityId) -> bool {
                        block.iter().any(|s| match &s.kind {
                            ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => c.capability == cap,
                            ir::Stmt::Fork { branches, .. } => {
                                branches.iter().any(|c| c.capability == cap)
                            }
                            ir::Stmt::If {
                                then_body,
                                else_body,
                                ..
                            } => calls(then_body, cap) || calls(else_body, cap),
                            _ => false,
                        })
                    }
                    ok(
                        !calls(&model.process.body, e.capability),
                        "active invariant without condition",
                    )?;
                }
            }
            ir::ContractKind::StateInvariant {
                antecedent,
                condition,
            } => {
                for c in antecedent.iter().chain(std::iter::once(condition)) {
                    let mut pending = vec![(c, 0)];
                    while let Some((c, depth)) = pending.pop() {
                        ok(depth < 128, "state invariant depth limit")?;
                        match c {
                            ir::Cond::And(cs) | ir::Cond::Or(cs) => {
                                ok(
                                    cs.len() >= 2,
                                    "state invariant empty conjunction/disjunction",
                                )?;
                                pending.extend(cs.iter().map(|c| (c, depth + 1)));
                            }
                            _ => {
                                cond(
                                    c,
                                    &|_| Err("state invariant reads binding".into()),
                                    model,
                                    false,
                                )?;
                                let id = match c {
                                    ir::Cond::StateEq(t) => t.variable,
                                    // Membership of the case context (ADR0035);
                                    // `cond` above admits only CurrentCase.
                                    ir::Cond::IntCompare { variable, .. }
                                    | ir::Cond::Contains { set: variable, .. } => *variable,
                                    _ => return Err("state invariant reads binding".into()),
                                };
                                ok(
                                    model.state_variables[id.0]
                                        .kind
                                        .actor()
                                        .is_none_or(|a| crate::responsibility::included(model, a)),
                                    "state invariant reads excluded Actor",
                                )?;
                            }
                        }
                    }
                }
            }
            ir::ContractKind::AtMostOnce { event: e } => {
                event(*e)?;
                ok(e.kind == ir::EventKind::Commit, "once requires commit")?;
            }
            ir::ContractKind::RequiresEvent { before, start } => {
                event(*before)?;
                event(*start)?;
                ok(
                    start.kind == ir::EventKind::Start,
                    "precedence requires start",
                )?;
            }
            ir::ContractKind::Response {
                trigger, targets, ..
            }
            | ir::ContractKind::NamedResponse {
                trigger, targets, ..
            } => {
                ok(
                    *trigger != ir::Trigger::Admitted
                        || model.external_wait()
                        || model.overflow_end.is_some(),
                    "admitted trigger without supported overflow handling",
                )?;
                if let ir::Trigger::Event(e) = trigger {
                    event(*e)?;
                }
                ok(!targets.is_empty(), "response without targets")?;
                for t in targets {
                    match t {
                        ir::Target::End(e) => end(*e)?,
                        ir::Target::Event(e) => event(*e)?,
                    }
                }
            }
        }
    }
    if crate::contest::condition_wait_refusal(model).is_some() {
        return Err(Halt::Unsupported(
            crate::contest::CONDITION_WAIT_UNSUPPORTED,
        ));
    }
    crate::responsibility::validate(model)
        .map_err(|_| Halt::Unsupported("invalid static responsibility/contest IR"))?;
    if crate::contest::restricted_authority_reference(model).is_some() {
        return Err("restricted contest Authority".into());
    }
    let mut requests = HashSet::new();
    validate_concurrency(model)?;
    for v in &model.verify {
        ok(
            requests.insert(v.property),
            "duplicate verification request",
        )?;
    }
    ok(
        model
            .verify
            .iter()
            .filter(|v| v.property == ir::Property::StateInRange)
            .count()
            == usize::from(model.has_integers()),
        "missing or spurious integer range property",
    )?;
    model
        .verify
        .iter()
        .try_for_each(|verify| match verify.property {
            ir::Property::NoLostUpdate(id) => ok(
                model
                    .state_variables
                    .get(id.0)
                    .is_some_and(|v| v.last_write_wins && model.is_shared_observed(v.kind)),
                "invalid loss property",
            ),
            ir::Property::StateInRange => {
                ok(model.has_integers(), "range property without integer")
            }
            ir::Property::RecoverableEnd
            | ir::Property::TerminatingEnd
            | ir::Property::AlmostSureEnd => Ok(()),
            ir::Property::Contest(id) => ok(
                model
                    .responsibility
                    .as_ref()
                    .is_some_and(|m| id < m.contests.len()),
                "static property out of range",
            ),
            ir::Property::ProbabilityReached(id) => {
                let q = model
                    .probability_queries
                    .get(id)
                    .ok_or("invalid probability query index")?;
                ok(q.bound.valid(), "invalid query bound")?;
                match &q.condition {
                    ir::ProbabilityCondition::End(e) => end(*e),
                    ir::ProbabilityCondition::State(c) => {
                        cond(c, &|_| Err("query reads result".into()), model, false)
                    }
                }
            }
            ir::Property::ReachableEnd(e) => end(e),
            ir::Property::NoResourceOvercommit => Ok(()),
            ir::Property::Contract(id) => ok(
                id.0 < model.contracts.len(),
                "contract property out of range",
            ),
        })
}

fn validate_concurrency(m: &ir::Model) -> EngineResult<()> {
    let mut expected = Vec::new();
    for c in &m.capabilities {
        if let Some(v) = &c.version_check {
            if v.variables.is_empty()
                || c.kernel.is_some()
                || c.results.contains(&v.reject)
                || c.requires.as_ref().is_some_and(|r| r.otherwise == v.reject)
                || !m
                    .types
                    .get(c.return_type.0)
                    .is_some_and(|t| v.reject.0 < t.members.len())
            {
                return Err("invalid version_check result domain".into());
            }
            let mut previous = None;
            for &id in &v.variables {
                let variable = m
                    .state_variables
                    .get(id.0)
                    .ok_or("invalid version_check target")?;
                if previous.is_some_and(|p| p >= id)
                    || !m.is_shared_observed(variable.kind)
                    || variable.kind.actor() != Some(c.actor)
                    || !c.finish_writes().any(|w| w.variable == id)
                    || c.start_writes.iter().any(|w| w.variable == id)
                {
                    return Err("invalid version_check write receipt".into());
                }
                previous = Some(id);
            }
        }
    }
    for (i, variable) in m.state_variables.iter().enumerate() {
        let id = ir::StateVarId(i);
        let count = m
            .verify
            .iter()
            .filter(|v| v.property == ir::Property::NoLostUpdate(id))
            .count();
        if count != usize::from(variable.last_write_wins)
            || variable.last_write_wins && !m.is_shared_observed(variable.kind)
        {
            return Err("invalid implicit loss property or placement".into());
        }
        if m.bounds.cases < 2 || !m.is_shared_observed(variable.kind) {
            continue;
        }
        let writers: Vec<_> = m
            .capabilities
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                c.start_writes
                    .iter()
                    .chain(c.finish_writes())
                    .any(|w| w.variable == id)
            })
            .collect();
        for (j, (left, a)) in writers.iter().enumerate() {
            for (right, b) in &writers[j..] {
                use ir::{
                    AssignmentOp as O, CommutativeKind as K, ConcurrencyResolution as R,
                    WriteSource as W,
                };
                let mut resolution = None;
                if m.actors[a.actor.0].capacity == Some(1) {
                    resolution = Some(R::Serialized);
                }
                if resolution.is_none() {
                    for (r, resource) in m.resources.iter().enumerate() {
                        let x = a.uses.iter().find(|u| u.resource.0 == r);
                        let y = b.uses.iter().find(|u| u.resource.0 == r);
                        if let (Some(x), Some(y)) = (x, y)
                            && u128::from(x.amount) + u128::from(y.amount)
                                > u128::from(resource.capacity)
                        {
                            resolution = Some(R::Lock(ir::ResourceId(r)));
                            break;
                        }
                    }
                }
                if resolution.is_none() {
                    let ops: Vec<_> = [*a, *b]
                        .iter()
                        .flat_map(|c| c.start_writes.iter().chain(c.finish_writes()))
                        .filter(|w| w.variable == id)
                        .map(|w| match w.source {
                            W::InsertCase { .. } => Some(K::InsertCase),
                            W::Integer {
                                operator: O::Add, ..
                            } => Some(K::Add),
                            W::Integer {
                                operator: O::Subtract,
                                ..
                            } => Some(K::Subtract),
                            _ => None,
                        })
                        .collect();
                    if let Some(Some(k)) = ops.first()
                        && ops.iter().all(|op| *op == Some(*k))
                    {
                        resolution = Some(R::Commutative(*k));
                    }
                }
                if resolution.is_none()
                    && [*a, *b].iter().all(|c| {
                        c.version_check
                            .as_ref()
                            .is_some_and(|v| v.variables.contains(&id))
                    })
                {
                    resolution = Some(R::Versioned);
                }
                if resolution.is_none() && variable.last_write_wins {
                    resolution = Some(R::LastWriteWins);
                }
                let resolution = resolution.ok_or("unresolved concurrent writes (G5)")?;
                expected.push(ir::ResolvedPair {
                    variable: id,
                    left: ir::CapabilityId(*left),
                    right: ir::CapabilityId(*right),
                    resolution,
                });
            }
        }
    }
    if expected != m.concurrency_resolutions {
        return Err("invalid concurrency resolution evidence".into());
    }
    Ok(())
}

fn validate_kernel(k: &ir::Kernel, model: &ir::Model) -> EngineResult<()> {
    let mut given = BTreeSet::new();
    let mut count = num_bigint::BigUint::from(1u8);
    for id in &k.given {
        let v = model
            .state_variables
            .get(id.0)
            .ok_or("kernel given index")?;
        if v.case_set_size.is_some()
            || model
                .types
                .get(v.base.0)
                .is_some_and(|t| t.name == "PersonId")
            || (v.integer_max.is_some() && !v.is_loop_round())
            || matches!(v.kind, ir::StateKind::ActorObserved(_))
        {
            return Err("integer kernel given unsupported".into());
        }
        if !given.insert(*id) {
            return Err("duplicate kernel given".into());
        }
        let n = if v.is_loop_round() {
            v.integer_max.unwrap() as usize
        } else {
            model
                .types
                .get(v.base.0)
                .ok_or("kernel given type")?
                .members
                .len()
                + usize::from(v.optional)
        };
        count *= num_bigint::BigUint::from(n);
    }
    let mut rows = HashSet::new();
    for r in &k.rows {
        if r.values.len() != k.given.len()
            || !rows.insert(r.values.clone())
            || r.branches.is_empty()
        {
            return Err("invalid kernel row shape".into());
        }
        for (id, x) in k.given.iter().zip(&r.values) {
            if !valid_state_value(&model.state_variables[id.0], *x, model) {
                return Err("invalid kernel row value".into());
            }
        }
        let mut sum = Rational::zero();
        for b in &r.branches {
            if b.overrides.iter().any(|(id, x)| {
                if model.state_variables.get(id.0).is_some_and(|v| {
                    v.case_set_size.is_some()
                        || model
                            .types
                            .get(v.base.0)
                            .is_some_and(|t| t.name == "PersonId")
                }) {
                    return true;
                }
                matches!(x, ir::StateValue::Integer(_))
                    || model.state_variables.get(id.0).is_some_and(|v| {
                        v.integer_max.is_some() || matches!(v.kind, ir::StateKind::ActorObserved(_))
                    })
            }) {
                return Err("integer kernel override unsupported".into());
            }
            if !b.probability.valid() {
                return Err("invalid kernel branch probability".into());
            }
            sum.0 += &b.probability.0;
        }
        if sum != Rational::one() {
            return Err("invalid kernel branch sum".into());
        }
    }
    if count != num_bigint::BigUint::from(rows.len()) {
        return Err("kernel rows not exhaustive".into());
    }
    Ok(())
}

fn initialize_latents(
    seed: &State,
    model: &ir::Model,
    room: &mut dyn Generation,
    evidence: &mut Exploration,
) -> EngineResult<Vec<State>> {
    let variables: Vec<_> = model
        .state_variables
        .iter()
        .enumerate()
        .filter(|(_, v)| v.initial == ir::InitialValue::Nondet)
        .map(|(i, _)| i)
        .collect();
    if let Some(sample) = room.sampling()
        && variables
            .iter()
            .any(|i| model.state_variables[*i].cardinality(&model.types) > 1)
    {
        for i in &variables {
            if model.state_variables[*i].cardinality(&model.types) > 1 {
                sample.note_initial(model, *i, sample.case);
            }
        }
        return sample.reject();
    }
    let mut digits = vec![0u128; variables.len()];
    let mut successors = Vec::new();
    loop {
        room.take(1)?;
        #[cfg(test)]
        {
            evidence.initial_clones += 1;
        }
        let mut next = seed.clone();
        for (&i, &value) in variables.iter().zip(&digits) {
            next.state_values[i] = if model.state_variables[i].integer_max.is_some() {
                ir::StateValue::Integer(value as u64)
            } else {
                ir::StateValue::Value(ir::ValueId(value as usize))
            };
        }
        next.initialized = true;
        emit(&mut next, Bundle::Received, model, evidence)?;
        successors.push(next);
        let mut cursor = 0;
        loop {
            if cursor == digits.len() {
                return Ok(successors);
            }
            digits[cursor] += 1;
            if digits[cursor] < model.state_variables[variables[cursor]].cardinality(&model.types) {
                break;
            }
            digits[cursor] = 0;
            cursor += 1;
        }
    }
}

fn valid_state_value(v: &ir::StateVariable, x: ir::StateValue, model: &ir::Model) -> bool {
    v.contains(&model.types, x)
}

fn new_write_valid(w: &ir::StateWrite, c: &ir::Capability, m: &ir::Model) -> bool {
    let Some(v) = m.state_variables.get(w.variable.0) else {
        return false;
    };
    match w.source {
        ir::WriteSource::InsertCase { parameter } => {
            v.case_set_size.is_some()
                && c.params
                    .get(parameter)
                    .is_some_and(|p| p.ty == ir::CASE_ID_TYPE)
        }
        ir::WriteSource::Executor => {
            v.kind.is_observed()
                && m.types.get(v.base.0).is_some_and(|t| {
                    t.name == "PersonId"
                        && t.members == m.people.iter().map(|p| p.name.clone()).collect::<Vec<_>>()
                })
                && m.actors
                    .get(c.actor.0)
                    .is_some_and(|a| a.executors.is_some())
        }
        _ => false,
    }
}

fn integer_test(operator: ir::IntComparison, before: u64, value: i128) -> bool {
    let n = i128::from(before);
    match operator {
        ir::IntComparison::Eq => n == value,
        ir::IntComparison::Ne => n != value,
        ir::IntComparison::Lt => n < value,
        ir::IntComparison::Le => n <= value,
        ir::IntComparison::Gt => n > value,
        ir::IntComparison::Ge => n >= value,
    }
}

fn write_value(
    w: &ir::StateWrite,
    before: &[ir::StateValue],
    result: Option<ir::ValueId>,
    case_index: usize,
    executor: Option<ir::PersonId>,
) -> EngineResult<ir::StateValue> {
    Ok(match w.source {
        ir::WriteSource::Executor => {
            ir::StateValue::Value(ir::ValueId(executor.ok_or("anonymous executor write")?.0))
        }
        ir::WriteSource::InsertCase { .. } => {
            let ir::StateValue::Set(bits) = before[w.variable.0] else {
                return Err("nonset insertion".into());
            };
            ir::StateValue::Set(
                bits | 1u64
                    .checked_shl(case_index as u32)
                    .ok_or("invalid case insertion")?,
            )
        }
        ir::WriteSource::Result => ir::StateValue::Value(result.ok_or("start result write")?),
        ir::WriteSource::Constant(v) => v,
        ir::WriteSource::Integer {
            operator, operand, ..
        } => {
            let ir::StateValue::Integer(n) = before[w.variable.0] else {
                return Err("noninteger arithmetic state".into());
            };
            let n = i128::from(n);
            let attempted = match operator {
                ir::AssignmentOp::Assign => operand,
                ir::AssignmentOp::Add => n + operand,
                ir::AssignmentOp::Subtract => n - operand,
            };
            ir::StateValue::Integer(
                u64::try_from(attempted).map_err(|_| Halt::from("unchecked integer write"))?,
            )
        }
    })
}

fn range_bundle(
    s: &State,
    task: &Task,
    start: bool,
    m: &ir::Model,
    e: &mut Exploration,
) -> EngineResult<bool> {
    range_in(s, task, start, m, e, 0)
}

fn range_in(
    s: &State,
    task: &Task,
    start: bool,
    m: &ir::Model,
    e: &mut Exploration,
    case_index: usize,
) -> EngineResult<bool> {
    let cap = &m.capabilities[task.capability.0];
    if !start && version_rejected(task, cap) {
        return Ok(false);
    }
    let writes = if start {
        &cap.start_writes
    } else if let Some(a) = &cap.atomic {
        if condition_at(&a.condition, &s.results, &s.state_values, case_index)? == a.commit_when {
            &a.writes
        } else {
            return Ok(false);
        }
    } else {
        &cap.writes
    };
    let mut failed = false;
    for w in writes {
        if let ir::WriteSource::Integer {
            operator,
            operand,
            span,
        } = w.source
        {
            let v = &m.state_variables[w.variable.0];
            let ir::StateValue::Integer(before) = s.state_values[w.variable.0] else {
                return Err("integer write has noninteger state".into());
            };
            let attempted = match operator {
                ir::AssignmentOp::Assign => operand,
                ir::AssignmentOp::Add => i128::from(before) + operand,
                ir::AssignmentOp::Subtract => i128::from(before) - operand,
            };
            let upper = v.integer_max.ok_or("integer write has enum domain")?;
            if attempted < 0 || attempted > i128::from(upper) {
                failed = true;
                e.range_attempts += 1;
                let witness = outcome::StateRangeViolation {
                    case_id: e.range_case,
                    time: s.time,
                    binding: m.site_name(task.binding),
                    actor: m.actors[cap.actor.0].name.clone(),
                    capability: cap.name.clone(),
                    phase: if start { "start" } else { "finish" }.into(),
                    variable: match v.kind {
                        ir::StateKind::ActorObserved(a) => {
                            format!("{}.{}", m.actors[a.0].name, v.name)
                        }
                        _ => v.name.clone(),
                    },
                    operator: match operator {
                        ir::AssignmentOp::Assign => "=",
                        ir::AssignmentOp::Add => "+=",
                        ir::AssignmentOp::Subtract => "-=",
                    }
                    .into(),
                    before: before.to_string(),
                    operand: operand.to_string(),
                    attempted: attempted.to_string(),
                    lower: "0".into(),
                    upper: upper.to_string(),
                    write_span: span,
                    declaration_span: v.span,
                };
                if let Some(trace) = &mut e.trace {
                    trace.range(s, task, m, &witness);
                }
                if !e.ranges.iter().any(|r| {
                    r.case_id == witness.case_id
                        && r.binding == witness.binding
                        && r.phase == witness.phase
                        && r.variable == witness.variable
                }) {
                    e.ranges.push(witness);
                }
            }
        }
    }
    if failed {
        e.collect_debts(s, m, "state_range_violation");
    }
    Ok(failed)
}

fn condition_at(
    cond: &ir::Cond,
    results: &[Option<ir::ValueId>],
    values: &[ir::StateValue],
    case_index: usize,
) -> EngineResult<bool> {
    match cond {
        ir::Cond::Contains { set, .. } => match values.get(set.0) {
            Some(ir::StateValue::Set(bits)) => Ok(bits
                & 1u64
                    .checked_shl(case_index as u32)
                    .ok_or("invalid membership case")?
                != 0),
            _ => Err("invalid membership state".into()),
        },
        ir::Cond::IntCompare {
            variable,
            operator,
            value,
        } => match values.get(variable.0) {
            Some(ir::StateValue::Integer(n)) => Ok(integer_test(*operator, *n, *value)),
            _ => Err("invalid integer condition".into()),
        },
        ir::Cond::StateEq(t) => values
            .get(t.variable.0)
            .map(|x| *x == t.value)
            .ok_or("uninitialized state".into()),
        ir::Cond::Eq(test) => results[test.binding.0]
            .map(|value| value == test.value)
            .ok_or("condition reads an unavailable or cancelled result".into()),
        ir::Cond::And(operands) | ir::Cond::Or(operands) => {
            let and = matches!(cond, ir::Cond::And(_));
            let mut result = and;
            for operand in operands {
                // Do not short-circuit validation of unavailable results.
                let value = condition_at(operand, results, values, case_index)?;
                result = if and {
                    result && value
                } else {
                    result || value
                };
            }
            Ok(result)
        }
    }
}

#[cfg(test)]
fn step(
    state: State,
    program: &[Instruction],
    model: &ir::Model,
    room: &mut Room,
) -> EngineResult<Step> {
    step_observed(state, program, model, room, &mut Exploration::default())
}

fn query_hit(
    q: &ir::ProbabilityQuery,
    s: &State,
    end: Option<(ir::EndId, u64)>,
    arrival: u64,
) -> EngineResult<bool> {
    match &q.condition {
        ir::ProbabilityCondition::State(c) => Ok(s.time >= arrival
            && q.within.is_none_or(|d| s.time - arrival <= d)
            && condition_at(c, &s.results, &s.state_values, 0)?),
        ir::ProbabilityCondition::End(e) => Ok(end.is_some_and(|(actual, t)| {
            actual == *e && t >= arrival && q.within.is_none_or(|d| t - arrival <= d)
        })),
    }
}

fn finish_site(
    state: &mut State,
    site: ir::BindingId,
    result: Option<ir::ValueId>,
    model: &ir::Model,
) {
    if model.response_attempt(site).is_some() {
        if let Some(value) = result {
            state
                .response
                .as_mut()
                .expect("response history")
                .actuals
                .insert(site, value);
        }
        return;
    }
    if let Some(id) = model.site_contest(site) {
        let route = &mut state.contest.as_mut().expect("contest site").routes[id];
        route.result = result;
        if result.is_some() {
            route.response_at = Some(state.time);
        } else {
            route.violation = Some("cancelled");
        }
    } else {
        state.results[site.0] = result;
        if let Some(alias) = model.loop_alias(site) {
            state.results[alias.0] = result;
        }
    }
}

fn business_end(
    mut state: State,
    end: ir::EndId,
    model: &ir::Model,
    room: &mut dyn Generation,
    exploration: &mut Exploration,
) -> EngineResult<Step> {
    if state.contest.is_none() {
        return Ok(Step::Terminal(end));
    }
    room.take(1)?;
    emit(&mut state, Bundle::End(end), model, exploration)?;
    close_responses(&mut state);
    exploration.contracts(&state);
    exploration.terminal(&state, end)?;
    state.contest.as_mut().unwrap().business = Some(BusinessEnd {
        end,
        at: state.time,
        work: state.work,
        result_at: state.result_at,
    });
    for (id, declaration) in model.contests().iter().enumerate() {
        if let crate::contest::ContestTarget::End(e) = declaration.target {
            let route = &mut state.contest.as_mut().unwrap().routes[id];
            if e == end {
                route.target_at = Some(state.time);
            } else {
                route.filed = Some(false);
            }
        }
    }
    for (i, clause) in model.contracts.iter().enumerate() {
        if let ir::ContractKind::ContestResponse { contest } = clause.kind {
            state.monitors[i].activated |= model.contests()[contest].target.end() == Some(end);
        }
    }
    state
        .tasks
        .retain(|t| model.site_contest(t.binding).is_some());
    state.waiting = true;
    state.adopted = None;
    capture_end_open(&mut state, model);
    Ok(Step::Next(vec![state]))
}

fn capture_end_open(state: &mut State, model: &ir::Model) {
    if !crate::contest::has_effects(model) {
        return;
    }
    if let Some(c) = &mut state.contest
        && c.business.is_some()
        && c.end_open.is_none()
        && c.routes.iter().all(|r| {
            r.target_at.is_none() || (r.filed.is_some() && (r.filed != Some(true) || r.queued))
        })
    {
        c.end_open = Some(
            c.routes
                .iter()
                .any(|r| r.filed == Some(true) && r.response_at.is_none()),
        );
    }
}

// Cut semantics belong to this interpreter, not the shared wire canonicalizer.
fn contest_table_alternatives(
    mut alternatives: Vec<crate::contest::Alternative>,
    coverage: crate::obligation::Coverage,
    model: &ir::Model,
    id: usize,
) -> Vec<crate::contest::Alternative> {
    if let Some(reason) = coverage.reason() {
        // A branch that did not file never created route work: that is a fact
        // of the explored prefix, not a withdrawn extremum.
        for a in alternatives.iter_mut().filter(|a| a.filed) {
            a.route_cost = crate::metrics::Range {
                min: None,
                max: None,
                reason: Some(reason),
            };
        }
    }
    crate::contest::canonical_alternatives(alternatives, model, id)
}

fn contest_control(
    state: &State,
    model: &ir::Model,
    room: &mut dyn Generation,
    _exploration: &mut Exploration,
) -> EngineResult<Option<Step>> {
    let c = state.contest.as_ref().expect("contest execution");
    if let Some((id, declaration)) = model
        .contests()
        .iter()
        .enumerate()
        .find(|(id, _)| c.routes[*id].target_at.is_some() && c.routes[*id].filed.is_none())
    {
        if declaration.rates.is_none()
            && let Some(sample) = room.sampling()
        {
            return sample.reject();
        }
        c.routes[id]
            .target_at
            .unwrap()
            .checked_add(declaration.respond_by)
            .ok_or("contest deadline overflow")?;
        if let Some(sample) = room.sampling() {
            let rates = declaration.rates.as_ref().expect("sampled contest rates");
            let site = format!(
                "contest.{}.clause{}.occurrence",
                model.process.name, declaration.ordinal
            );
            let filed = sample.choose(
                sample.case,
                &site,
                1,
                &[
                    Rational(Rational::one().0 - &rates.probability.0),
                    rates.probability.clone(),
                ],
            )? == 1;
            let mut next = state.clone();
            next.contest.as_mut().unwrap().routes[id].filed = Some(filed);
            capture_end_open(&mut next, model);
            return Ok(Some(Step::Next(vec![next])));
        }
        let probabilities = declaration.rates.as_ref().map(|r| {
            [
                Rational(Rational::one().0 - &r.probability.0),
                r.probability.clone(),
            ]
        });
        let support: Vec<_> = [false, true]
            .into_iter()
            .enumerate()
            .filter(|(i, _)| probabilities.as_ref().is_none_or(|ps| !ps[*i].is_zero()))
            .collect();
        // Reserve positive support before cloning; do not build a product of clauses.
        room.take(support.len())?;
        #[cfg(test)]
        {
            _exploration.contest_clones += support.len();
        }
        let states = support
            .iter()
            .map(|(_, filed)| {
                let mut next = state.clone();
                next.contest.as_mut().unwrap().routes[id].filed = Some(*filed);
                capture_end_open(&mut next, model);
                next
            })
            .collect::<Vec<_>>();
        if let Some(ps) = probabilities {
            _exploration.chance_groups.push((
                0..states.len(),
                support.iter().map(|(i, _)| ps[*i].clone()).collect(),
            ));
        }
        return Ok(Some(Step::Next(states)));
    }
    if let Some((id, declaration)) = model
        .contests()
        .iter()
        .enumerate()
        .find(|(id, _)| c.routes[*id].filed == Some(true) && !c.routes[*id].queued)
    {
        let cap = &model.capabilities[declaration.route.0];
        if !cap.params.is_empty() {
            return Err("contest route requires arguments".into());
        }
        room.take(1)?;
        let mut queued = state.clone();
        let route = &mut queued.contest.as_mut().unwrap().routes[id];
        route.queued = true;
        if !crate::responsibility::included(model, cap.actor) {
            route.violation = Some("missing_route");
        } else {
            queued.tasks.push(Task {
                frozen_performance: None,
                stale: Vec::new(),
                executor: None,
                binding: model.contest_site(id),
                capability: declaration.route,
                lifecycle: Lifecycle::Queued,
            });
        }
        metrics::observe(&mut queued, model);
        let now = queued.time;
        refresh_contests(&mut queued, model, now, false);
        capture_end_open(&mut queued, model);
        return Ok(Some(Step::Next(vec![queued])));
    }
    Ok(None)
}

fn refresh_contests(state: &mut State, model: &ir::Model, horizon: u64, inclusive: bool) {
    let Some(c) = &state.contest else {
        return;
    };
    for (i, clause) in model.contracts.iter().enumerate() {
        if let ir::ContractKind::ContestResponse { contest } = clause.kind {
            let declaration = &model.contests()[contest];
            let route = &c.routes[contest];
            let monitor = &mut state.monitors[i];
            monitor.activated |= route.target_at.is_some();
            if route.filed == Some(true) {
                let late = route
                    .target_at
                    .unwrap()
                    .checked_add(declaration.respond_by)
                    .is_none_or(|deadline| {
                        route.response_at.map_or(
                            horizon > deadline || (inclusive && horizon == deadline),
                            |at| at > deadline,
                        )
                    });
                monitor.violated |= late || route.violation.is_some();
            }
        }
    }
}

fn contest_deadlock(state: &mut State, model: &ir::Model) {
    if let Some(c) = &mut state.contest {
        for r in &mut c.routes {
            if r.filed == Some(true) && r.response_at.is_none() && r.violation.is_none() {
                r.violation = Some("deadlock");
            }
        }
    }
    refresh_contests(state, model, state.time, false);
}

fn step_observed(
    state: State,
    program: &[Instruction],
    model: &ir::Model,
    room: &mut dyn Generation,
    exploration: &mut Exploration,
) -> EngineResult<Step> {
    let mut step = step_observed_inner(state, program, model, room, exploration)?;
    if let Step::Next(states) = &mut step {
        for state in states {
            if !state.exceeds_live_bound(model.bounds.live_tasks) {
                // The legacy single-case engine's case context is index 0.
                update_state_invariants(state, model, 0)?;
            }
        }
    }
    Ok(step)
}

fn step_observed_inner(
    mut state: State,
    program: &[Instruction],
    model: &ir::Model,
    room: &mut dyn Generation,
    exploration: &mut Exploration,
) -> EngineResult<Step> {
    if state.response.as_ref().is_some_and(|h| h.stopped) {
        return Err("receipt outcome collection pending".into());
    }
    let range_before = exploration.range_attempts;
    if !state.initialized {
        return initialize_latents(&state, model, room, exploration).map(Step::Next);
    }
    let after_end = state.contest.as_ref().is_some_and(|c| c.business.is_some());
    if state.contest.is_some()
        && (after_end || crate::contest::has_effects(model))
        && let Some(step) = contest_control(&state, model, room, exploration)?
    {
        return Ok(step);
    }
    let post_join = ir::Join::All;
    let post_next = 0;
    let (calls, join, next): (&[ir::Call], &ir::Join, &usize) = if after_end {
        (&[], &post_join, &post_next)
    } else {
        match &program[state.pc] {
            Instruction::MergeCopy {
                binding,
                source,
                next,
            } => {
                let value = state
                    .results
                    .get(source.0)
                    .copied()
                    .flatten()
                    .ok_or("merge source has no result")?;
                let target = state
                    .results
                    .get_mut(binding.0)
                    .ok_or("merge target out of range")?;
                if target.is_some() {
                    return Err("merge target already written".into());
                }
                *target = Some(value);
                state.pc = *next;
                room.take(1)?;
                return Ok(Step::Next(vec![state]));
            }
            Instruction::LoopControl { updates, next } => {
                for (id, value) in updates {
                    state.state_values[id.0] = *value;
                }
                state.pc = *next;
                room.take(1)?;
                return Ok(Step::Next(vec![state]));
            }
            Instruction::Await { .. } => {
                return Err("condition wait requires global execution".into());
            }
            Instruction::Receipt {
                binding,
                rule,
                retry,
                failure,
                recovery,
                next,
            } => {
                response::advance(&mut state, *binding, rule, *retry, *failure, *next)?;
                recovery::enter(&mut state, rule.command, *recovery)?;
                room.take(1)?;
                return Ok(Step::Next(vec![state]));
            }
            Instruction::ResolveEffect {
                command,
                evidence,
                site,
                next,
            } => {
                recovery::resolve(&mut state, *command, *evidence, *site, model, 0)?;
                state.pc = *next;
                room.take(1)?;
                return Ok(Step::Next(vec![state]));
            }
            Instruction::ReceiptUnknown => {
                recovery::stop(&mut state)?;
                room.take(1)?;
                return Ok(Step::Next(vec![state]));
            }
            Instruction::Sleep { duration, next, .. } => match state.sleep_until {
                None => {
                    let wake = state
                        .time
                        .checked_add(*duration)
                        .ok_or("wait time overflow")?;
                    room.take(1)?;
                    state.sleep_until = Some(wake);
                    if let Some(waits) =
                        state.metric_history.as_mut().and_then(|h| h.waits.as_mut())
                    {
                        waits.push((state.time, wake));
                    }
                    return Ok(Step::Next(vec![state]));
                }
                Some(wake) if wake <= state.time => {
                    room.take(1)?;
                    state.sleep_until = None;
                    state.pc = *next;
                    return Ok(Step::Next(vec![state]));
                }
                Some(wake) if !crate::contest::has_effects(model) => {
                    return advance_state(state, wake, model, room, exploration);
                }
                Some(_) => (&[], &post_join, &post_next),
            },
            Instruction::Wait { calls, join, next } => (calls, join, next),
            Instruction::Branch {
                condition: cond,
                yes,
                no,
                span,
                site,
            } => {
                let selected = condition_at(cond, &state.results, &state.state_values, 0)?;
                if !model.is_loop_guard(cond) {
                    exploration.observed.branch(*span, site, selected);
                }
                state.pc = if selected { *yes } else { *no };
                room.take(1)?;
                return Ok(Step::Next(vec![state]));
            }
            Instruction::End(end) => {
                if model.recovery().is_some()
                    && state
                        .response
                        .as_ref()
                        .is_some_and(|h| !h.unresolved.is_empty())
                {
                    return Err("invalid_recovery_ir".into());
                }
                return business_end(state, *end, model, room, exploration);
            }
            Instruction::Stop if crate::contest::has_effects(model) => {
                (&[], &post_join, &post_next)
            }
            Instruction::Stop => return Ok(Step::Deadlock),
        }
    };
    let ordinary_wait = !after_end && matches!(&program[state.pc], Instruction::Wait { .. });
    if !state.waiting && ordinary_wait {
        for call in calls {
            response::capture(&mut state, call, model, 0)?;
            recovery::queued(&mut state, call);
        }
        state.tasks.extend(calls.iter().map(|call| Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: call.binding,
            capability: call.capability,
            lifecycle: Lifecycle::Queued,
        }));
        state.waiting = true;
        metrics::observe(&mut state, model);
        room.take(1)?;
        return Ok(Step::Next(vec![state]));
    }

    // Do not adopt the first iterated finish: all currently due completions
    // participate in the simultaneous candidate set. Between those finishes,
    // a released slot may enable a start; those orders are not all commuting.
    let due: Vec<_> = state
        .tasks
        .iter()
        .enumerate()
        .filter_map(|(i, task)| match task.lifecycle {
            Lifecycle::Running { finish, ack }
                if finish == state.time || ack == Some(state.time) =>
            {
                Some(Due { i, finish, ack })
            }
            Lifecycle::DistributedRunning { due, ack, .. } if due || ack == Some(state.time) => {
                Some(Due {
                    i,
                    finish: if due {
                        state.time
                    } else {
                        state.time.wrapping_add(1)
                    },
                    ack,
                })
            }
            _ => None,
        })
        .collect();
    let mut successors = Vec::new();

    if let ir::Join::Any {
        matches,
        tie,
        remaining,
        ..
    } = join
        && state.adopted.is_none()
        && due.is_empty()
    {
        let candidates: Vec<_> = matches
            .iter()
            .filter(|test| state.results[test.binding.0] == Some(test.value))
            .map(|test| test.binding)
            .collect();
        if !candidates.is_empty() {
            let winners = match tie {
                ir::Tie::Nondet => candidates,
                ir::Tie::By(order) => vec![
                    *order
                        .iter()
                        .find(|b| candidates.contains(b))
                        .ok_or("tie order does not cover matching branches")?,
                ],
            };
            // Join is an immediate control transition, not an implicit lock on
            // dispatch. A newly eligible start can race the cancellation request
            // at this same timestamp, but time cannot advance before adoption.
            successors.extend(adopt(&state, &winners, *remaining, model, room)?);
        }
    }

    if state
        .tasks
        .iter()
        .filter(|t| after_end || model.site_contest(t.binding).is_none())
        .all(|task| task.lifecycle == Lifecycle::Done)
        && (after_end || ordinary_wait)
    {
        if after_end {
            let end = state
                .contest
                .as_ref()
                .unwrap()
                .business
                .as_ref()
                .unwrap()
                .end;
            return Ok(Step::Terminal(end));
        }
        if !successors.is_empty() {
            return Ok(Step::Next(successors));
        }
        if let ir::Join::Any { if_none, .. } = join
            && state.adopted.is_none()
        {
            return business_end(state, *if_none, model, room, exploration);
        }
        state
            .tasks
            .retain(|t| model.site_contest(t.binding).is_some());
        state.waiting = false;
        state.adopted = None;
        state.pc = *next;
        room.take(1)?;
        return Ok(Step::Next(vec![state]));
    }

    // One atomic completion per successor preserves starts newly enabled
    // between finishes. State deduplication collapses commuting finish orders.
    // finish == ack contributes BOTH possible published-result outcomes.
    for &due in &due {
        let task = &state.tasks[due.i];
        if due.finish == state.time && range_bundle(&state, task, false, model, exploration)? {
            if due.ack == Some(state.time) {
                let ack = complete_observed(
                    &state,
                    Due {
                        finish: state.time ^ 1,
                        ..due
                    },
                    model,
                    room,
                    exploration,
                );
                let ack = preserve_generated(ack, &mut successors, model, exploration)?;
                successors.extend(ack);
            }
            continue;
        }
        if let Some(sample) = room.sampling() {
            sample.note_task(model, task.binding, false);
        }
        if let Some(kernel) = &model.capabilities[task.capability.0].kernel {
            if due.finish == state.time {
                let start = successors.len();
                if let Some(sample) = room.sampling() {
                    // Keep a lazy completion, not every categorical successor.
                    // The binding/case identifies this factor before any draw.
                    sample.kernels.push((start, due.i));
                    sample.sites.push((
                        start,
                        simulation::Sample::task_site(model, task.binding, "kernel"),
                        sample.case,
                    ));
                    successors.push(state.clone());
                } else {
                    let result = kernel_complete(&state, due.i, kernel, model, room, exploration);
                    let branches = preserve_generated(result, &mut successors, model, exploration)?;
                    let weights = branches.iter().map(|(p, _)| p.clone()).collect();
                    successors.extend(branches.into_iter().map(|(_, s)| s));
                    exploration
                        .chance_groups
                        .push((start..successors.len(), weights));
                }
            }
            if due.ack == Some(state.time) {
                preserve_generated(room.take(1), &mut successors, model, exploration)?;
                let mut acked = state.clone();
                acked.tasks[due.i].lifecycle = Lifecycle::Done;
                if model.tracks_obligations() {
                    let result = emit_at(
                        &mut acked,
                        Bundle::CancelAck(task.capability, task.executor),
                        Some(task.binding),
                        model,
                        exploration,
                    );
                    preserve_generated(result, &mut successors, model, exploration)?;
                }
                metrics::observe(&mut acked, model);
                successors.push(acked);
            }
        } else {
            let group_start = exploration.chance_groups.len();
            let result = complete_observed(&state, due, model, room, exploration);
            let states = preserve_generated(result, &mut successors, model, exploration)?;
            for (range, _) in &mut exploration.chance_groups[group_start..] {
                range.start += successors.len();
                range.end += successors.len();
            }
            successors.extend(states);
        }
        if let Some(sample) = room.sampling() {
            sample.legacy_unique(&successors, exploration)?;
        }
    }

    // Starts commute unless they compete for the same capacity. Enumerating
    // each eligible start covers that competition as well as zero-time tasks
    // starting before/after an already due event. No intentional idle branch.
    let counts = state.occupancy(model);
    let resource_counts = state.resource_occupancy(model)?;
    for (i, task) in state.tasks.iter().enumerate() {
        if task.lifecycle != Lifecycle::Queued {
            continue;
        }
        let cap = &model.capabilities[task.capability.0];
        let capacity = model.actors[cap.actor.0]
            .capacity
            .ok_or("capacity-less task execution")?;
        if counts[cap.actor.0] < capacity && resources_fit(cap, &resource_counts, model) {
            let pool = model.actors[cap.actor.0].executors.as_ref();
            let candidates = pool
                .into_iter()
                .flatten()
                .copied()
                .filter(|p| {
                    !state
                        .tasks
                        .iter()
                        .any(|t| t.lifecycle.running() && t.executor == Some(*p))
                })
                .map(Some)
                .chain(pool.is_none().then_some(None));
            for executor in candidates {
                if let Some(req) = &cap.requires
                    && !condition_at(&req.condition, &state.results, &state.state_values, 0)?
                {
                    preserve_generated(room.take(1), &mut successors, model, exploration)?;
                    let mut refused = state.clone();
                    refused.tasks[i].lifecycle = Lifecycle::Done;
                    if !crate::contest::has_effects(model)
                        || model.site_contest(task.binding).is_none()
                    {
                        refused
                            .results
                            .resize(refused.results.len().max(task.binding.0 + 1), None);
                        refused.results[task.binding.0] = Some(req.otherwise);
                        if let Some(alias) = model.loop_alias(task.binding) {
                            refused.results[alias.0] = Some(req.otherwise);
                        }
                    }
                    record_refusal(
                        &mut refused,
                        task.binding,
                        task.capability,
                        req.otherwise,
                        model,
                        exploration,
                    );
                    metrics::observe(&mut refused, model);
                    successors.push(refused);
                    if let Some(sample) = room.sampling() {
                        // The refused call is a choice site too (boundary review m1).
                        sample.note_task(model, task.binding, false);
                        sample.legacy_unique(&successors, exploration)?;
                    }
                    break;
                }
                if let Some(sample) = room.sampling() {
                    sample.note_task(model, task.binding, false);
                }
                let lifecycle = if let Some(d) = &cap.duration_distribution {
                    for (t, p) in &d.entries {
                        if !p.is_zero() {
                            state.time.checked_add(*t).ok_or("finish time overflow")?;
                        }
                    }
                    Lifecycle::DistributedRunning {
                        started: state.time,
                        next: 0,
                        due: false,
                        ack: None,
                    }
                } else {
                    let finish = state
                        .time
                        .checked_add(cap.duration)
                        .ok_or("finish time overflow")?;
                    Lifecycle::Running { finish, ack: None }
                };
                if range_bundle(&state, task, true, model, exploration)? {
                    continue;
                }
                preserve_generated(room.take(1), &mut successors, model, exploration)?;
                let mut started = state.clone();
                started.tasks[i].executor = executor;
                started.tasks[i].lifecycle = lifecycle;
                started.tasks[i].stale.clear();
                for w in &cap.start_writes {
                    started.state_values[w.variable.0] =
                        write_value(w, &state.state_values, None, 0, executor)?;
                }
                notify_writes(&mut started, Some(task.binding), &cap.start_writes, model);
                metrics::observe(&mut started, model);
                let emitted = emit_at(
                    &mut started,
                    Bundle::Start(task.capability, executor),
                    Some(task.binding),
                    model,
                    exploration,
                );
                preserve_generated(emitted, &mut successors, model, exploration)?;
                successors.push(started);
                if let Some(sample) = room.sampling() {
                    sample.legacy_unique(&successors, exploration)?;
                }
            }
        }
    }
    if !successors.is_empty() || exploration.range_attempts > range_before {
        return Ok(Step::Next(successors));
    }

    let mut next_time = None;
    for task in &state.tasks {
        let at = match task.lifecycle {
            Lifecycle::Running { finish, ack } => ack.map_or(finish, |ack| ack.min(finish)),
            Lifecycle::DistributedRunning {
                started,
                next,
                due: false,
                ack,
            } => {
                // A running distributed task always has a next candidate:
                // the last positive one finishes with probability 1.
                let d = model.capabilities[task.capability.0]
                    .duration_distribution
                    .as_ref()
                    .ok_or("missing duration distribution")?;
                let t = d
                    .entries
                    .iter()
                    .filter(|(_, p)| !p.is_zero())
                    .nth(next)
                    .ok_or("duration candidate out of range")?
                    .0;
                let finish = started.checked_add(t).ok_or("finish time overflow")?;
                ack.map_or(finish, |a| a.min(finish))
            }
            _ => continue,
        };
        next_time = Some(next_time.map_or(at, |n: u64| n.min(at)));
    }
    if let Some(wake) = state.sleep_until {
        next_time = Some(next_time.map_or(wake, |n| n.min(wake)));
    }
    let Some(next_time) = next_time else {
        return Ok(Step::Deadlock);
    };
    advance_state(state, next_time, model, room, exploration)
}

fn advance_state(
    mut state: State,
    next_time: u64,
    model: &ir::Model,
    room: &mut dyn Generation,
    exploration: &mut Exploration,
) -> EngineResult<Step> {
    if next_time > model.bounds.clock {
        refresh_contests(&mut state, model, model.bounds.clock, true);
        expire_debts(&mut state, model.bounds.clock, true);
        for m in &mut state.monitors {
            m.violated |= m.pending.iter().any(|d| *d <= model.bounds.clock);
        }
        exploration.contracts(&state);
        if let Some(trace) = &mut exploration.trace {
            trace.leaf(&state, model, crate::explain::Kind::Deadline);
        }
        return Ok(Step::Cut);
    }
    let occupied = if crate::contest::has_effects(model) {
        state
            .tasks
            .iter()
            .filter(|t| t.lifecycle.running() && model.site_contest(t.binding).is_none())
            .count() as u64
    } else {
        state
            .occupancy(model)
            .iter()
            .try_fold(0u64, |sum, count| sum.checked_add(*count))
            .ok_or("occupancy overflow")?
    };
    let increment = (next_time - state.time)
        .checked_mul(occupied)
        .ok_or("work overflow")?;
    let work = state.work.checked_add(increment).ok_or("work overflow")?;
    room.take(1)?;
    for m in &mut state.monitors {
        m.violated |= m.pending.iter().any(|d| *d < next_time);
    }
    expire_debts(&mut state, next_time, false);
    exploration.contracts(&state);
    state.work = work;
    state.time = next_time;
    refresh_contests(&mut state, model, next_time, false);
    Ok(Step::Next(vec![state]))
}

fn preserve_generated<T>(
    result: EngineResult<T>,
    successors: &mut Vec<State>,
    model: &ir::Model,
    e: &mut Exploration,
) -> EngineResult<T> {
    if result.is_err() && e.trace.is_some() {
        // Move already charged successors, including those produced by a
        // nested completion before another result aborted. Trace consumes them
        // as edges only; they never enter the normal frontier or census.
        let nested = std::mem::take(&mut e.partial_successors);
        e.partial_successors = std::mem::take(successors);
        e.partial_successors.extend(nested);
    } else if result.is_err() && (model.uses_probability() || model.tracks_obligations()) {
        // Move, never clone: these states were already charged before generation.
        e.partial_successors = std::mem::take(successors);
    }
    result
}

fn record_refusal(
    s: &mut State,
    binding: ir::BindingId,
    capability: ir::CapabilityId,
    value: ir::ValueId,
    m: &ir::Model,
    e: &mut Exploration,
) {
    if let Some(t) = s
        .response
        .as_mut()
        .and_then(|h| h.recovery.as_mut())
        .and_then(|h| h.tasks.get_mut(&binding))
    {
        t.result = Some(value);
    }
    let cap = &m.capabilities[capability.0];
    e.refusals
        .entry(binding)
        .and_modify(|r| {
            r.at.min = r.at.min.min(s.time);
            r.at.max = r.at.max.max(s.time);
        })
        .or_insert_with(|| outcome::CapabilityRefusal {
            case_id: 1,
            call: m.site_name(binding),
            capability: format!("{}.{}", m.actors[cap.actor.0].name, cap.name),
            result: m.types[cap.return_type.0].members[value.0].clone(),
            at: outcome::RefusalRange {
                min: s.time,
                max: s.time,
            },
            coverage: "complete".into(),
            observed_count: None,
            samples: None,
        });
    if let Some(id) = m.site_contest(binding) {
        s.contest.as_mut().expect("contest refusal").routes[id].violation = Some("refused");
        for (i, clause) in m.contracts.iter().enumerate() {
            if matches!(clause.kind, ir::ContractKind::ContestResponse { contest } if contest == id)
            {
                s.monitors[i].violated = true;
            }
        }
        e.contracts(s);
    }
}

fn selected_refusals(old: &State, new: &mut State, m: &ir::Model, e: &mut Exploration) {
    let rows: Vec<_> = new
        .tasks
        .iter()
        .filter_map(|b| {
            old.tasks
                .iter()
                .find(|a| a.binding == b.binding)
                .map(|a| (a, b))
        })
        .filter_map(|(a, b)| {
            if a.binding == b.binding
                && a.lifecycle == Lifecycle::Queued
                && b.lifecycle == Lifecycle::Done
            {
                new.results
                    .get(b.binding.0)
                    .copied()
                    .flatten()
                    .or_else(|| {
                        (crate::contest::has_effects(m) && m.site_contest(b.binding).is_some())
                            .then(|| {
                                m.capabilities[b.capability.0]
                                    .requires
                                    .as_ref()
                                    .map(|r| r.otherwise)
                            })
                            .flatten()
                    })
                    .map(|v| (b.binding, b.capability, v))
            } else {
                None
            }
        })
        .collect();
    for (binding, capability, value) in rows {
        record_refusal(new, binding, capability, value, m, e);
    }
}

/// A running task with a finish or acknowledgement at the current time.
#[derive(Clone, Copy)]
struct Due {
    i: usize,
    finish: u64,
    ack: Option<u64>,
}

#[cfg(test)]
fn complete(
    state: &State,
    due: Due,
    model: &ir::Model,
    room: &mut Room,
) -> EngineResult<Vec<State>> {
    complete_observed(state, due, model, room, &mut Exploration::default())
}

fn complete_observed(
    state: &State,
    Due { i, finish, ack }: Due,
    model: &ir::Model,
    room: &mut dyn Generation,
    exploration: &mut Exploration,
) -> EngineResult<Vec<State>> {
    complete_in(state, Due { i, finish, ack }, model, room, exploration, 0)
}

fn complete_in(
    state: &State,
    Due { i, finish, ack }: Due,
    model: &ir::Model,
    room: &mut dyn Generation,
    exploration: &mut Exploration,
    case_index: usize,
) -> EngineResult<Vec<State>> {
    let task = &state.tasks[i];
    let mut results = Vec::new();
    if finish == state.time {
        let cap = &model.capabilities[task.capability.0];
        if version_rejected(task, cap) {
            results.push(Some(cap.version_check.as_ref().unwrap().reject));
        } else if cap.receipt_query.is_some() {
            results.extend(
                recovery::project(model, task.capability, &state.state_values, case_index)?
                    .into_iter()
                    .map(Some),
            );
        } else if let Some(a) = &cap.atomic {
            let commit = condition_at(
                &a.condition,
                &state.results,
                &state.state_values,
                case_index,
            )? == a.commit_when;
            results.push(Some(if commit { a.commit } else { a.reject }));
        } else {
            results.extend(
                model.capabilities[task.capability.0]
                    .results
                    .iter()
                    .copied()
                    .map(Some),
            );
        }
    }
    if ack == Some(state.time) {
        results.push(None);
    }
    if let Some(sample) = room.sampling()
        && results.iter().any(|x| *x != results[0])
    {
        return sample.reject();
    }
    room.take(results.len())?;
    let mut states = Vec::with_capacity(results.len());
    for result in results {
        let mut completed = state.clone();
        completed.tasks[i].lifecycle = Lifecycle::Done;
        finish_site(&mut completed, task.binding, result, model);
        metrics::observe(&mut completed, model);
        if let Some(value) = result {
            let cap = &model.capabilities[task.capability.0];
            if cap.receipt_query.is_some() {
                recovery::observe_query(
                    &mut completed,
                    task.binding,
                    task.capability,
                    value,
                    model,
                    case_index,
                )?;
            }
            let writes = if version_rejected(task, cap) {
                &[][..]
            } else {
                cap.atomic.as_ref().map_or(cap.writes.as_slice(), |a| {
                    if value == a.commit {
                        a.writes.as_slice()
                    } else {
                        &[]
                    }
                })
            };
            for w in writes {
                if model.state_variables[w.variable.0].last_write_wins
                    && task.stale.contains(&w.variable)
                    && overwrites(w.source)
                    && !completed.lost_updates.contains(&w.variable)
                {
                    completed.lost_updates.push(w.variable);
                    completed.lost_updates.sort();
                }
            }
            for w in writes {
                completed.state_values[w.variable.0] = write_value(
                    w,
                    &state.state_values,
                    Some(value),
                    case_index,
                    task.executor,
                )?;
            }
            notify_writes(&mut completed, Some(task.binding), writes, model);
            exploration
                .lost_updates
                .extend(completed.lost_updates.iter().copied());
            let emitted = emit_in(
                &mut completed,
                Bundle::Finish(task.capability, value, task.executor),
                Some(task.binding),
                model,
                exploration,
                case_index,
            );
            preserve_generated(emitted, &mut states, model, exploration)?;
        } else if model.tracks_obligations() {
            let emitted = emit_in(
                &mut completed,
                Bundle::CancelAck(task.capability, task.executor),
                Some(task.binding),
                model,
                exploration,
                case_index,
            );
            preserve_generated(emitted, &mut states, model, exploration)?;
        }
        if let Some(value) = result.filter(|_| model.response_attempt(task.binding).is_some()) {
            let start = states.len();
            let branches = response::deliver(&completed, task.binding, value, model, room)?;
            let probabilistic = branches.iter().all(|(p, _)| p.is_some());
            let weights = branches.iter().filter_map(|(p, _)| p.clone()).collect();
            states.extend(branches.into_iter().map(|(_, s)| s));
            if probabilistic {
                exploration
                    .chance_groups
                    .push((start..states.len(), weights));
            }
        } else {
            states.push(completed);
        }
    }
    Ok(states)
}

fn version_rejected(task: &Task, cap: &ir::Capability) -> bool {
    cap.version_check
        .as_ref()
        .is_some_and(|v| v.variables.iter().any(|x| task.stale.contains(x)))
}

fn overwrites(source: ir::WriteSource) -> bool {
    matches!(
        source,
        ir::WriteSource::Constant(_)
            | ir::WriteSource::Result
            | ir::WriteSource::Executor
            | ir::WriteSource::Integer {
                operator: ir::AssignmentOp::Assign,
                ..
            }
    )
}

fn notify_writes(
    state: &mut State,
    own: Option<ir::BindingId>,
    writes: &[ir::StateWrite],
    m: &ir::Model,
) {
    for task in &mut state.tasks {
        if Some(task.binding) == own || !task.lifecycle.running() {
            continue;
        }
        let cap = &m.capabilities[task.capability.0];
        for w in writes {
            let checked = cap
                .version_check
                .as_ref()
                .is_some_and(|v| v.variables.contains(&w.variable));
            let lww = m.state_variables[w.variable.0].last_write_wins
                && cap
                    .start_writes
                    .iter()
                    .chain(cap.finish_writes())
                    .any(|x| x.variable == w.variable);
            if (checked || lww) && !task.stale.contains(&w.variable) {
                task.stale.push(w.variable);
            }
        }
        task.stale.sort();
    }
}

// Receipts come from the executed branch, never from comparing state values.
fn transition_writes(before: &State, after: &State, m: &ir::Model) -> Vec<ir::StateWrite> {
    let mut writes = Vec::new();
    for old in &before.tasks {
        let Some(new) = after.tasks.iter().find(|t| t.binding == old.binding) else {
            continue;
        };
        let cap = &m.capabilities[old.capability.0];
        if old.lifecycle == Lifecycle::Queued && new.lifecycle.running() {
            writes.extend(&cap.start_writes);
        } else if old.lifecycle.running() && new.lifecycle == Lifecycle::Done {
            let result = m
                .site_contest(old.binding)
                .and_then(|id| after.contest.as_ref().and_then(|c| c.routes[id].result))
                .or_else(|| after.results.get(old.binding.0).copied().flatten())
                .or_else(|| {
                    after
                        .response
                        .as_ref()
                        .and_then(|h| h.actuals.get(&old.binding).copied())
                });
            if let Some(value) = result {
                if version_rejected(old, cap) {
                    continue;
                }
                if let Some(a) = &cap.atomic {
                    if value == a.commit {
                        writes.extend(&a.writes);
                    }
                } else {
                    writes.extend(&cap.writes);
                }
            }
        }
    }
    writes
}

fn kernel_complete(
    state: &State,
    i: usize,
    kernel: &ir::Kernel,
    model: &ir::Model,
    room: &mut dyn Generation,
    evidence: &mut Exploration,
) -> EngineResult<Vec<(Rational, State)>> {
    kernel_complete_in(state, i, kernel, model, room, evidence, 0)
}

fn kernel_complete_in(
    state: &State,
    i: usize,
    kernel: &ir::Kernel,
    model: &ir::Model,
    room: &mut dyn Generation,
    evidence: &mut Exploration,
    case_index: usize,
) -> EngineResult<Vec<(Rational, State)>> {
    evidence.chance_seen = true;
    #[cfg(test)]
    {
        evidence.kernel_draws += 1;
    }
    let row = kernel
        .rows
        .iter()
        .find(|row| {
            kernel
                .given
                .iter()
                .zip(&row.values)
                .all(|(id, x)| state.state_values[id.0] == *x)
        })
        .ok_or("missing kernel row")?;
    let branches: Vec<_> = row
        .branches
        .iter()
        .filter(|b| !b.probability.is_zero())
        .collect();
    // Atomic reservation: failure leaves the whole action unknown, with no clone.
    room.take(branches.len())?;
    let mut out = Vec::new();
    for b in branches {
        #[cfg(test)]
        {
            evidence.kernel_clones += 1;
        }
        if let Some(trace) = &mut evidence.trace {
            let row_index = kernel
                .rows
                .iter()
                .position(|r| std::ptr::eq(r, row))
                .expect("selected row");
            let branch_index = row
                .branches
                .iter()
                .position(|branch| std::ptr::eq(branch, b))
                .expect("selected branch");
            trace.kernel_choice = Some(crate::explain::KernelChoice {
                frozen_performance: state.tasks[i].frozen_performance.clone(),
                case_id: trace.case_id,
                binding: model.site_name(state.tasks[i].binding),
                row: row_index,
                branch: branch_index,
            });
        }
        let generated = apply_kernel_branch(state, i, b, model, evidence, case_index);
        if let Some(trace) = &mut evidence.trace {
            trace.kernel_choice = None;
        }
        let next = match generated {
            Ok(next) => next,
            Err(halt) => {
                if evidence.trace.is_some() {
                    evidence
                        .partial_successors
                        .extend(out.into_iter().map(|(_, s)| s));
                }
                return Err(halt);
            }
        };
        out.push((b.probability.clone(), next));
    }
    Ok(out)
}

fn apply_kernel_branch(
    state: &State,
    i: usize,
    branch: &ir::KernelBranch,
    model: &ir::Model,
    evidence: &mut Exploration,
    case_index: usize,
) -> EngineResult<State> {
    let task = &state.tasks[i];
    let cap = &model.capabilities[task.capability.0];
    let mut next = state.clone();
    next.tasks[i].lifecycle = Lifecycle::Done;
    finish_site(&mut next, task.binding, Some(branch.result), model);
    metrics::observe(&mut next, model);
    for w in &cap.writes {
        next.state_values[w.variable.0] = write_value(
            w,
            &state.state_values,
            Some(branch.result),
            case_index,
            state.tasks[i].executor,
        )?;
    }
    for &(id, x) in &branch.overrides {
        next.state_values[id.0] = x;
    }
    if let Some(h) = &mut next.evidence {
        h.overrides(
            task.binding,
            branch.overrides.iter().map(|(v, _)| *v).collect(),
        );
    }
    let mut receipts = cap.writes.clone();
    receipts.extend(
        branch
            .overrides
            .iter()
            .map(|(variable, value)| ir::StateWrite {
                variable: *variable,
                source: ir::WriteSource::Constant(*value),
            }),
    );
    for w in &receipts {
        if model.state_variables[w.variable.0].last_write_wins
            && task.stale.contains(&w.variable)
            && overwrites(w.source)
            && !next.lost_updates.contains(&w.variable)
        {
            next.lost_updates.push(w.variable);
        }
    }
    next.lost_updates.sort();
    notify_writes(&mut next, Some(task.binding), &receipts, model);
    evidence
        .lost_updates
        .extend(next.lost_updates.iter().copied());
    emit_in(
        &mut next,
        Bundle::Finish(task.capability, branch.result, task.executor),
        Some(task.binding),
        model,
        evidence,
        case_index,
    )?;
    Ok(next)
}

fn sample_kernel(
    state: &State,
    i: usize,
    model: &ir::Model,
    sample: &mut simulation::Sample,
    evidence: &mut Exploration,
) -> EngineResult<State> {
    sample_kernel_in(state, i, model, sample, evidence, 0)
}

fn sample_kernel_in(
    state: &State,
    i: usize,
    model: &ir::Model,
    sample: &mut simulation::Sample,
    evidence: &mut Exploration,
    case_index: usize,
) -> EngineResult<State> {
    let task = &state.tasks[i];
    let kernel = work_profiles::kernel(model, task)?.ok_or("missing sampled kernel")?;
    let row = kernel
        .rows
        .iter()
        .find(|row| {
            kernel
                .given
                .iter()
                .zip(&row.values)
                .all(|(id, x)| state.state_values[id.0] == *x)
        })
        .ok_or("missing kernel row")?;
    let site = simulation::Sample::task_site(model, task.binding, "kernel");
    #[cfg(test)]
    crate::q6_acceptance::draw(state.time, site.clone());
    let selected = sample.choose(
        sample.case,
        &site,
        1,
        &row.branches
            .iter()
            .map(|b| b.probability.clone())
            .collect::<Vec<_>>(),
    )?;
    let next = apply_kernel_branch(
        state,
        i,
        &row.branches[selected],
        model,
        evidence,
        case_index,
    )?;
    if model.response_attempt(task.binding).is_some() {
        return response::deliver(
            &next,
            task.binding,
            row.branches[selected].result,
            model,
            sample,
        )?
        .into_iter()
        .next()
        .map(|(_, s)| s)
        .ok_or_else(|| Halt::from("missing sampled receipt"));
    }
    Ok(next)
}

#[derive(Clone)]
struct ProbabilityNode {
    // Intern key and node share one state allocation; the index is not an
    // uncharged second copy of every generated state.
    state: std::rc::Rc<State>,
    chance_pending: bool,
    actions: Vec<Vec<(Rational, usize)>>,
    targets: Vec<bool>,
    end: Option<ir::EndId>,
    unknown: bool,
    // Applies to unknown nodes. Pending is deliberately NOT a clock cut.
    cut_reason: ProbabilityCut,
    numeric: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProbabilityCut {
    Horizon,
    Range,
    Pending,
    Clock,
    LiveTasks,
    StateBudget,
}

// A mandatory joint chance layer. It contains no completion bundles, kernels,
// or scheduler decisions. Reserve every positive leaf before cloning ANY leaf.
fn duration_gate(
    state: &State,
    model: &ir::Model,
    room: &mut dyn Generation,
    e: &mut Exploration,
    bits: u64,
) -> EngineResult<Option<Vec<(Rational, State)>>> {
    let mut coins = Vec::new();
    for (i, task) in state.tasks.iter().enumerate() {
        if let Lifecycle::DistributedRunning {
            started,
            next,
            due: false,
            ..
        } = task.lifecycle
        {
            let entries = &model.capabilities[task.capability.0]
                .duration_distribution
                .as_ref()
                .ok_or("missing duration distribution")?
                .entries;
            let support: Vec<_> = entries.iter().filter(|(_, p)| !p.is_zero()).collect();
            let (t, p) = support.get(next).ok_or("duration cursor out of range")?;
            if started.checked_add(*t).ok_or("finish time overflow")? != state.time {
                continue;
            }
            let mut residual = Rational::zero();
            for (_, p) in &support[next..] {
                residual.0 += &p.0;
                if !residual.within_bits(bits) {
                    e.duration_numeric = true;
                    return Err(Halt::Budget);
                }
            }
            let q = duration_hazard(p, &residual, bits).ok_or_else(|| {
                e.duration_numeric = true;
                Halt::Budget
            })?;
            coins.push((i, q));
        }
    }
    if coins.is_empty() {
        return Ok(None);
    }
    if let Some(sample) = room.sampling() {
        let mut next_state = state.clone();
        for (i, q) in &coins {
            let task = &state.tasks[*i];
            let Lifecycle::DistributedRunning { next, .. } = task.lifecycle else {
                unreachable!()
            };
            let site = simulation::Sample::task_site(model, task.binding, "duration");
            let finish = sample.choose(
                sample.case,
                &site,
                next + 1,
                &[q.clone(), Rational(Rational::one().0 - &q.0)],
            )? == 0;
            if let Lifecycle::DistributedRunning { next, due, .. } =
                &mut next_state.tasks[*i].lifecycle
            {
                *due = finish;
                if !finish {
                    *next += 1;
                }
            }
        }
        return Ok(Some(vec![(Rational::one(), next_state)]));
    }
    let count = coins
        .iter()
        .try_fold(1usize, |n, (_, q)| {
            n.checked_mul(if *q == Rational::one() { 1 } else { 2 })
        })
        .ok_or(Halt::Budget)?;
    room.take(count)?;
    let mut out = Vec::new();
    for code in 0..count {
        let mut digit = code;
        let mut p = Rational::one();
        // Compute weights before leaf allocation, and reject numerical excess.
        let mut decisions = Vec::new();
        for (i, q) in &coins {
            let finish = *q == Rational::one() || digit % 2 == 0;
            if *q != Rational::one() {
                digit /= 2;
            }
            let weight = if finish {
                q.clone()
            } else {
                Rational(Rational::one().0 - &q.0)
            };
            p.0 *= weight.0;
            if !p.within_bits(bits) {
                e.duration_numeric = true;
                if e.trace.is_some() {
                    e.trace_discarded
                        .extend(out.into_iter().map(|(_, state)| state));
                }
                return Err(Halt::Budget);
            }
            decisions.push((*i, finish));
        }
        #[cfg(test)]
        {
            e.duration_clones += 1;
        }
        let mut s = state.clone();
        for (i, finish) in decisions {
            if let Lifecycle::DistributedRunning { next, due, .. } = &mut s.tasks[i].lifecycle {
                *due = finish;
                if !finish {
                    *next += 1;
                }
            }
        }
        out.push((p, s));
    }
    e.chance_seen = true;
    Ok(Some(out))
}

fn duration_hazard(p: &Rational, residual: &Rational, bits: u64) -> Option<Rational> {
    if residual.is_zero() {
        return None;
    }
    let q = Rational(&p.0 / &residual.0);
    q.within_bits(bits).then_some(q)
}

fn probability_initial(
    seed: &State,
    model: &ir::Model,
    room: &mut dyn Generation,
    e: &mut Exploration,
) -> EngineResult<Vec<(Rational, State)>> {
    let vars: Vec<_> = model
        .state_variables
        .iter()
        .enumerate()
        .filter_map(|(i, v)| match v.initial {
            ir::InitialValue::Distribution(d) => Some((i, d)),
            _ => None,
        })
        .collect();
    let supports: Vec<Vec<_>> = vars
        .iter()
        .map(|(_, d)| {
            model.initial_distributions[*d]
                .iter()
                .filter(|(_, p)| !p.is_zero())
                .collect()
        })
        .collect();
    if let Some(sample) = room.sampling() {
        let mut next = seed.clone();
        for (j, (i, _)) in vars.iter().enumerate() {
            let (case, site) = simulation::Sample::initial_site(model, *i, sample.case);
            let ps = supports[j]
                .iter()
                .map(|(_, p)| p.clone())
                .collect::<Vec<_>>();
            let choice = sample.choose(case, &site, 1, &ps)?;
            next.state_values[*i] = supports[j][choice].0;
        }
        next.initialized = !model
            .state_variables
            .iter()
            .any(|v| v.initial == ir::InitialValue::Nondet);
        if next.initialized {
            emit(&mut next, Bundle::Received, model, e)?;
        }
        return Ok(vec![(Rational::one(), next)]);
    }
    let count = supports
        .iter()
        .try_fold(1usize, |n, s| n.checked_mul(s.len()))
        .ok_or(Halt::Budget)?;
    room.take(count)?;
    let mut digits = vec![0; vars.len()];
    let mut out = Vec::new();
    for _ in 0..count {
        #[cfg(test)]
        {
            e.initial_clones += 1;
        }
        let mut next = seed.clone();
        let mut p = Rational::one();
        for (j, (i, _)) in vars.iter().enumerate() {
            let (x, q) = supports[j][digits[j]];
            next.state_values[*i] = *x;
            p.0 *= &q.0;
        }
        next.initialized = !model
            .state_variables
            .iter()
            .any(|v| v.initial == ir::InitialValue::Nondet);
        if next.initialized {
            emit(&mut next, Bundle::Received, model, e)?;
        }
        out.push((p, next));
        for j in 0..digits.len() {
            digits[j] += 1;
            if digits[j] < supports[j].len() {
                break;
            }
            digits[j] = 0;
        }
    }
    Ok(out)
}

fn run_probability(model: &ir::Model, budget: usize) -> outcome::Outcome {
    run_probability_traced(model, budget, None).0
}

fn run_probability_traced(
    model: &ir::Model,
    budget: usize,
    trace: Option<trace::Capture>,
) -> (outcome::Outcome, Option<trace::Capture>) {
    let mut e = Exploration {
        trace,
        ..Exploration::default()
    };
    let abort = |mut e: Exploration, message| {
        let trace = e.trace.take();
        (e.abort(message, model), trace)
    };
    if model.bounds.cases != 1 || model.verify.is_empty() {
        return abort(e, "exploration requires one case and at least one property");
    }
    if budget == 0 {
        if let Err(Halt::Unsupported(message)) = validate(model) {
            return abort(e, message);
        }
        e.truncated = true;
        e.budget_exhausted = true;
        if model.completion_requested() {
            e.completion = Some(crate::completion::Graph::default());
        }
        let trace = e.trace.take();
        let mut out = e.outcome(model);
        for p in &mut out.properties {
            if let ir::Property::ProbabilityReached(id) = p.property {
                let q = &model.probability_queries[id];
                p.probability = Some(ProbabilityRange {
                    min: Rational::zero(),
                    max: Rational::one(),
                });
                p.verdict = match q.comparison {
                    ir::Comparison::Le if q.bound == Rational::one() => Verdict::Satisfied,
                    ir::Comparison::Ge if q.bound == Rational::zero() => Verdict::Satisfied,
                    _ => Verdict::Inconclusive,
                };
            }
        }
        return (out, trace);
    }
    let result = probability_graph(model, budget, &mut e);
    let (nodes, scheduler) = match result {
        Ok(x) => x,
        Err(Halt::Unsupported(m)) => return abort(e, m),
        Err(Halt::Budget) => unreachable!("graph represents budget as unknown"),
    };
    let order = match probability_topology(&nodes) {
        Ok(o) => o,
        Err(Halt::Unsupported(m)) => return abort(e, m),
        Err(Halt::Budget) => unreachable!(),
    };
    let mut queries = Vec::new();
    // A duration-only model need not have a query or any completed terminal.
    // Numeric failure is still global truncation and must carry W-BUDGET-0.
    let mut numeric = nodes.iter().any(|n| n.numeric);
    for (id, q) in model.probability_queries.iter().enumerate() {
        let bounds = solve_probability_within_target(
            &nodes,
            &order,
            |n| n.targets[id],
            4096,
            q.within.map(|d| (d, model.bounds.clock)),
            matches!(q.condition, ir::ProbabilityCondition::End(_)),
        );
        numeric |= bounds.4;
        let verdict = if bounds.4 {
            Verdict::Inconclusive
        } else {
            match q.comparison {
                ir::Comparison::Le if bounds.3 <= q.bound => Verdict::Satisfied,
                ir::Comparison::Le if bounds.2 > q.bound => Verdict::Violated,
                ir::Comparison::Ge if bounds.0 >= q.bound => Verdict::Satisfied,
                ir::Comparison::Ge if bounds.1 < q.bound => Verdict::Violated,
                _ => Verdict::Inconclusive,
            }
        };
        queries.push((
            verdict,
            ProbabilityRange {
                min: bounds.0,
                max: bounds.3,
            },
        ));
    }
    let mut terminals = BTreeMap::new();
    for &end in e.terminals.keys() {
        let b = solve_probability(&nodes, &order, |n| n.end == Some(end), 4096);
        numeric |= b.4;
        terminals.insert(end, ProbabilityRange { min: b.0, max: b.3 });
    }
    if numeric {
        e.numeric_only = !e.budget_exhausted;
        e.truncated = true;
        e.budget_exhausted = true;
    }
    let chance = e.chance_seen || !model.initial_distributions.is_empty();
    let trace = e.trace.take();
    let mut out = e.outcome(model);
    for p in &mut out.properties {
        if let ir::Property::ProbabilityReached(id) = p.property {
            p.verdict = queries[id].0;
            p.probability = Some(queries[id].1.clone());
        }
    }
    for t in &mut out.terminals {
        t.probability = terminals.remove(&t.end);
    }
    out.probability_scheduler = scheduler && chance;
    (out, trace)
}

fn probability_topology(nodes: &[ProbabilityNode]) -> EngineResult<Vec<usize>> {
    let mut indegree = vec![0; nodes.len()];
    for node in nodes {
        for a in &node.actions {
            for (_, id) in a {
                indegree[*id] += 1;
            }
        }
    }
    let mut ready: Vec<_> = indegree
        .iter()
        .enumerate()
        .filter_map(|(i, n)| (*n == 0).then_some(i))
        .collect();
    let mut order = Vec::new();
    while let Some(i) = ready.pop() {
        order.push(i);
        for a in &nodes[i].actions {
            for (_, id) in a {
                indegree[*id] -= 1;
                if indegree[*id] == 0 {
                    ready.push(*id);
                }
            }
        }
    }
    if order.len() != nodes.len() {
        return Err("probability graph contains a cycle".into());
    }
    Ok(order)
}

fn probability_graph(
    model: &ir::Model,
    budget: usize,
    e: &mut Exploration,
) -> EngineResult<(Vec<ProbabilityNode>, bool)> {
    validate(model)?;
    let mut program = vec![Instruction::Stop];
    let entry = compile(&model.process.body, 0, &mut program);
    let mut state = State::initial(model, entry);
    let chance_pending = !model.initial_distributions.is_empty();
    if state.initialized && budget > 0 {
        emit(&mut state, Bundle::Received, model, e)?;
    }
    if let Some(trace) = &mut e.trace {
        trace.initial(&state, model);
    }
    let mut trace_ids = e.trace.as_ref().map(|_| vec![0usize]);
    let state = std::rc::Rc::new(state);
    let mut nodes = vec![ProbabilityNode {
        state: state.clone(),
        chance_pending,
        actions: Vec::new(),
        targets: vec![false; model.probability_queries.len()],
        end: None,
        unknown: true,
        cut_reason: ProbabilityCut::Pending,
        numeric: false,
    }];
    let mut ids = std::collections::HashMap::from([((state, chance_pending), 0)]);
    let mut pending = vec![0];
    let mut scheduler = false;
    if budget == 0 {
        e.truncated = true;
        e.budget_exhausted = true;
        return Ok((nodes, false));
    }
    let result = (|| -> EngineResult<()> {
        while let Some(id) = pending.pop() {
            if let (Some(trace), Some(ids)) = (&mut e.trace, &trace_ids) {
                trace.begin(ids[id]);
            }
            let state = nodes[id].state.as_ref().clone();
            // Reaching a business end is an absorbing hit for the probability
            // question, not an instruction to stop exploring route work.
            if let Some(business) = state.contest.as_ref().and_then(|c| c.business.as_ref()) {
                nodes[id].end = Some(business.end);
                for (i, q) in model.probability_queries.iter().enumerate() {
                    if matches!(q.condition, ir::ProbabilityCondition::End(_)) {
                        nodes[id].targets[i] =
                            query_hit(q, &state, Some((business.end, business.at)), 0)?;
                    }
                }
            }
            if state.time > model.bounds.clock || state.exceeds_live_bound(model.bounds.live_tasks)
            {
                e.truncated = true;
                e.collect_debts(
                    &state,
                    model,
                    if state.exceeds_live_bound(model.bounds.live_tasks) {
                        "live_tasks"
                    } else {
                        "clock"
                    },
                );
                nodes[id].cut_reason = if state.exceeds_live_bound(model.bounds.live_tasks) {
                    ProbabilityCut::LiveTasks
                } else {
                    ProbabilityCut::Clock
                };
                continue;
            }
            if state.initialized {
                for (i, q) in model.probability_queries.iter().enumerate() {
                    nodes[id].targets[i] = query_hit(
                        q,
                        &state,
                        state
                            .contest
                            .as_ref()
                            .and_then(|c| c.business.as_ref())
                            .map(|b| (b.end, b.at)),
                        0,
                    )?;
                }
            }
            e.observe(&state, model)?;
            e.contracts(&state);
            let mut room = Room(budget.saturating_sub(nodes.len()));
            e.chance_groups.clear();
            e.partial_successors.clear();
            e.duration_numeric = false;
            let ranges_before = e.range_attempts;
            let gate = if state.initialized {
                duration_gate(&state, model, &mut room, e, 4096)
            } else {
                Ok(None)
            };
            let generated = if !matches!(gate, Ok(None)) {
                match gate {
                    Ok(Some(xs)) => Ok(vec![xs]),
                    Err(h) => Err(h),
                    _ => unreachable!(),
                }
            } else if nodes[id].chance_pending {
                probability_initial(&state, model, &mut room, e).map(|xs| vec![xs])
            } else if !state.initialized {
                // Reserve the whole nondeterministic initialization layer before
                // any Received bundle is materialized. Legacy seed Room is untouched.
                let count = model
                    .state_variables
                    .iter()
                    .filter(|v| v.initial == ir::InitialValue::Nondet)
                    .try_fold(1usize, |n, v| {
                        usize::try_from(v.cardinality(&model.types))
                            .ok()
                            .and_then(|size| n.checked_mul(size))
                    });
                match count {
                    Some(n) if n <= room.0 => {
                        room.take(n)?;
                        initialize_latents(&state, model, &mut Room(n), e)
                            .map(|xs| xs.into_iter().map(|s| vec![(Rational::one(), s)]).collect())
                    }
                    _ => Err(Halt::Budget),
                }
            } else {
                match step_observed(state.clone(), &program, model, &mut room, e) {
                    Ok(Step::Next(xs)) => probability_actions(xs, e),
                    Ok(Step::Terminal(end)) => {
                        e.metric_leaf(&state, Some(end), false, model);
                        let mut closed = state.clone();
                        if closed.contest.as_ref().is_none_or(|c| c.business.is_none()) {
                            emit(&mut closed, Bundle::End(end), model, e)?;
                        }
                        close_responses(&mut closed);
                        e.contracts(&closed);
                        e.collect_debts(&closed, model, "terminal_unresolved");
                        if let Some(trace) = &mut e.trace {
                            trace.leaf(&closed, model, crate::explain::Kind::End);
                        }
                        e.terminal(&state, end)?;
                        nodes[id].end = Some(end);
                        for (i, q) in model.probability_queries.iter().enumerate() {
                            if matches!(q.condition, ir::ProbabilityCondition::End(_)) {
                                nodes[id].targets[i] =
                                    query_hit(q, &state, Some((end, state.time)), 0)?;
                            }
                        }
                        nodes[id].unknown = false;
                        continue;
                    }
                    Ok(Step::Cut) => {
                        e.truncated = true;
                        e.collect_debts(&state, model, "clock");
                        nodes[id].cut_reason = ProbabilityCut::Clock;
                        continue;
                    }
                    Ok(Step::Deadlock) => {
                        e.metric_leaf(&state, None, true, model);
                        e.collect_debts(&state, model, "deadlock_unresolved");
                        e.deadlock = true;
                        let mut s = state;
                        contest_deadlock(&mut s, model);
                        close_responses(&mut s);
                        e.contracts(&s);
                        if let Some(trace) = &mut e.trace {
                            trace.leaf(&s, model, crate::explain::Kind::Deadlock);
                        }
                        nodes[id].unknown = false;
                        continue;
                    }
                    Err(h) => Err(h),
                }
            };
            if let Some(trace) = &mut e.trace {
                for next in std::mem::take(&mut e.trace_discarded) {
                    let node = trace.transition(&state, &next, model);
                    if let Some(&existing) = ids.get(&(std::rc::Rc::new(next), false)) {
                        trace.merge(node, trace_ids.as_ref().expect("trace node IDs")[existing]);
                    }
                }
            }
            let budget_cut = matches!(generated, Err(Halt::Budget));
            nodes[id].numeric = e.duration_numeric;
            #[cfg(test)]
            {
                e.census.2 = e.census.2.max(budget - room.0);
            }
            let actions = match generated {
                Ok(a) => a,
                Err(Halt::Budget) => {
                    e.truncated = true;
                    if !e.duration_numeric {
                        e.budget_exhausted = true;
                    }
                    let xs = std::mem::take(&mut e.partial_successors);
                    probability_actions(xs, e)?
                }
                Err(h) => {
                    if let Some(trace) = &mut e.trace {
                        for next in std::mem::take(&mut e.partial_successors) {
                            let node = trace.transition(&state, &next, model);
                            let key = (std::rc::Rc::new(next), false);
                            if let Some(&existing) = ids.get(&key) {
                                trace.merge(
                                    node,
                                    trace_ids.as_ref().expect("trace node IDs")[existing],
                                );
                            }
                        }
                    }
                    return Err(h);
                }
            };
            scheduler |= actions.len() + usize::from(budget_cut) > 1;
            nodes[id].unknown = budget_cut || e.range_attempts != ranges_before;
            if e.range_attempts != ranges_before {
                nodes[id].cut_reason = ProbabilityCut::Range;
            }
            if budget_cut {
                nodes[id].cut_reason = ProbabilityCut::StateBudget;
            }
            for action in actions {
                #[cfg(test)]
                {
                    e.census.1 += action.len();
                }
                let mut edges: BTreeMap<usize, Rational> = BTreeMap::new();
                for (p, s) in action {
                    let trace_node = e
                        .trace
                        .as_mut()
                        .map(|trace| trace.transition(&state, &s, model));
                    let s = std::rc::Rc::new(s);
                    let key = (s.clone(), false);
                    let next = if let Some(&i) = ids.get(&key) {
                        if let (Some(trace), Some(ids), Some(node)) =
                            (&mut e.trace, &trace_ids, trace_node)
                        {
                            trace.merge(node, ids[i]);
                        }
                        i
                    } else {
                        let i = nodes.len();
                        if let (Some(ids), Some(node)) = (&mut trace_ids, trace_node) {
                            ids.push(node);
                        }
                        let targets = if s.initialized
                            && s.time <= model.bounds.clock
                            && !s.exceeds_live_bound(model.bounds.live_tasks)
                        {
                            model
                                .probability_queries
                                .iter()
                                .map(|q| {
                                    query_hit(
                                        q,
                                        &s,
                                        s.contest
                                            .as_ref()
                                            .and_then(|c| c.business.as_ref())
                                            .map(|b| (b.end, b.at)),
                                        0,
                                    )
                                })
                                .collect::<EngineResult<Vec<_>>>()?
                        } else {
                            vec![false; model.probability_queries.len()]
                        };
                        ids.insert(key, i);
                        pending.push(i);
                        nodes.push(ProbabilityNode {
                            state: s,
                            chance_pending: false,
                            actions: Vec::new(),
                            targets,
                            end: None,
                            unknown: true,
                            cut_reason: ProbabilityCut::Pending,
                            numeric: false,
                        });
                        i
                    };
                    edges.entry(next).or_insert_with(Rational::zero).0 += p.0;
                }
                nodes[id]
                    .actions
                    .push(edges.into_iter().map(|(i, p)| (p, i)).collect());
            }
            if budget_cut {
                break;
            }
        }
        for n in &nodes {
            if n.unknown {
                e.collect_debts(
                    &n.state,
                    model,
                    match n.cut_reason {
                        ProbabilityCut::Horizon => "horizon",
                        ProbabilityCut::Range => "state_range_violation",
                        ProbabilityCut::Clock => "clock",
                        ProbabilityCut::LiveTasks => "live_tasks",
                        ProbabilityCut::Pending | ProbabilityCut::StateBudget => "state_budget",
                    },
                );
            }
        }
        Ok(())
    })();
    if model.completion_requested() {
        let mut g = crate::completion::Graph::default();
        for n in &nodes {
            g.nodes.push(crate::completion::Node {
                target: n.end.is_some()
                    || n.state
                        .contest
                        .as_ref()
                        .is_some_and(|c| c.business.is_some()),
                unknown: n.unknown || n.numeric,
                actions: Vec::new(),
            });
        }
        for (i, n) in nodes.iter().enumerate() {
            for a in &n.actions {
                g.action(i, a.clone());
            }
            g.known_failure |= n.end.is_none() && !n.unknown && n.actions.is_empty();
        }
        e.completion = Some(g);
    }
    result?;
    Ok((nodes, scheduler))
}

fn probability_actions(
    xs: Vec<State>,
    e: &mut Exploration,
) -> EngineResult<Vec<Vec<(Rational, State)>>> {
    let groups = std::mem::take(&mut e.chance_groups);
    let mut actions = Vec::new();
    let mut iter = xs.into_iter().enumerate();
    while let Some((i, s)) = iter.next() {
        if let Some((range, ps)) = groups.iter().find(|(r, _)| r.start == i) {
            let first = ps.first().ok_or("empty chance action")?;
            let mut a = vec![(first.clone(), s)];
            for p in &ps[1..] {
                let (_, s) = iter.next().ok_or("invalid chance grouping")?;
                a.push((p.clone(), s));
            }
            if a.len() != range.len() {
                return Err("invalid chance action length".into());
            }
            actions.push(a);
        } else {
            actions.push(vec![(Rational::one(), s)]);
        }
    }
    Ok(actions)
}

// Four endpoints: min lower, min upper, max lower, max upper. Targets absorb.
fn solve_probability(
    nodes: &[ProbabilityNode],
    order: &[usize],
    target: impl Fn(&ProbabilityNode) -> bool,
    bits: u64,
) -> (Rational, Rational, Rational, Rational, bool) {
    solve_probability_within(nodes, order, target, bits, None)
}

fn solve_probability_within(
    nodes: &[ProbabilityNode],
    order: &[usize],
    target: impl Fn(&ProbabilityNode) -> bool,
    bits: u64,
    within: Option<(u64, u64)>,
) -> (Rational, Rational, Rational, Rational, bool) {
    solve_probability_within_target(nodes, order, target, bits, within, false)
}
fn solve_probability_within_target(
    nodes: &[ProbabilityNode],
    order: &[usize],
    target: impl Fn(&ProbabilityNode) -> bool,
    bits: u64,
    within: Option<(u64, u64)>,
    end_condition: bool,
) -> (Rational, Rational, Rational, Rational, bool) {
    let mut table = vec![
        [
            Rational::zero(),
            Rational::zero(),
            Rational::zero(),
            Rational::zero()
        ];
        nodes.len()
    ];
    let mut cuts = vec![false; nodes.len()];
    for &i in order.iter().rev() {
        let n = &nodes[i];
        if target(n) {
            table[i] = std::array::from_fn(|_| Rational::one());
            continue;
        }
        if end_condition && n.end.is_some() {
            continue;
        }
        if n.numeric {
            cuts[i] = true;
            table[i] = [
                Rational::zero(),
                Rational::one(),
                Rational::zero(),
                Rational::one(),
            ];
            continue;
        }
        if within.is_some_and(|(d, clock)| {
            n.state.time > d || (n.cut_reason == ProbabilityCut::Clock && d <= clock)
        }) {
            continue;
        }
        if n.unknown && n.actions.is_empty() {
            table[i] = [
                Rational::zero(),
                Rational::one(),
                Rational::zero(),
                Rational::one(),
            ];
            continue;
        }
        if n.actions.is_empty() {
            continue;
        }
        let mut actions = Vec::new();
        if n.unknown {
            actions.push([
                Rational::zero(),
                Rational::one(),
                Rational::zero(),
                Rational::one(),
            ]);
        }
        for a in &n.actions {
            let mut values = std::array::from_fn::<_, 4, _>(|_| Rational::zero());
            for (p, j) in a {
                cuts[i] |= cuts[*j] || !p.within_bits(bits);
                for (k, v) in values.iter_mut().enumerate() {
                    let product = Rational(&p.0 * &table[*j][k].0);
                    cuts[i] |= !product.within_bits(bits);
                    v.0 += product.0;
                    cuts[i] |= !v.within_bits(bits);
                }
            }
            actions.push(values);
        }
        if cuts[i] {
            table[i] = [
                Rational::zero(),
                Rational::one(),
                Rational::zero(),
                Rational::one(),
            ];
        } else {
            for k in 0..4 {
                table[i][k] = if k < 2 {
                    actions.iter().map(|v| v[k].clone()).min().unwrap()
                } else {
                    actions.iter().map(|v| v[k].clone()).max().unwrap()
                };
            }
        }
    }
    let [a, b, c, d] = table[0].clone();
    (a, b, c, d, cuts[0])
}

fn adopt(
    state: &State,
    winners: &[ir::BindingId],
    remaining: ir::Remaining,
    model: &ir::Model,
    room: &mut dyn Generation,
) -> EngineResult<Vec<State>> {
    if state.result_at.is_some() {
        return Err("multiple any adoption times are not defined".into());
    }
    if let Some(sample) = room.sampling()
        && winners.iter().any(|w| *w != winners[0])
    {
        for winner in winners {
            sample.note_task(model, *winner, true);
        }
        return sample.reject();
    }
    let mut successors = Vec::new();
    for &winner in winners {
        if let Some(sample) = room.sampling() {
            sample.note_task(model, winner, true);
        }
        room.take(1)?;
        let mut adopted = state.clone();
        adopted.adopted = Some(winner);
        adopted.result_at = Some(state.time);
        let mut states = vec![adopted];
        if remaining == ir::Remaining::CancelThenDrain {
            for i in 0..state.tasks.len() {
                if state.tasks[i].binding != winner
                    && model.site_contest(state.tasks[i].binding).is_none()
                {
                    states = request_cancel(states, i, model, room)?;
                }
            }
        }
        successors.extend(states);
    }
    Ok(successors)
}

// The input states are already counted by the caller. Changing a lifecycle in
// place creates nothing; d acknowledgement candidates replace one state by d,
// so they take d - 1 BEFORE cloning (the multiplication across tasks is
// therefore bounded too).
fn request_cancel(
    states: Vec<State>,
    i: usize,
    model: &ir::Model,
    room: &mut dyn Generation,
) -> EngineResult<Vec<State>> {
    let mut successors = Vec::new();
    for mut state in states {
        let task = &state.tasks[i];
        let cancel = model.capabilities[task.capability.0].cancel;
        if let Some(sample) = room.sampling() {
            sample.note_task(model, task.binding, false);
        }
        match (task.lifecycle, cancel) {
            (Lifecycle::Running { finish, .. }, ir::Cancel::Immediate) => {
                state.tasks[i].lifecycle = Lifecycle::Running {
                    finish,
                    ack: Some(state.time),
                };
                successors.push(state);
            }
            (
                Lifecycle::DistributedRunning {
                    started, next, due, ..
                },
                ir::Cancel::Immediate,
            ) => {
                state.tasks[i].lifecycle = Lifecycle::DistributedRunning {
                    started,
                    next,
                    due,
                    ack: Some(state.time),
                };
                successors.push(state);
            }
            (Lifecycle::Done, _) => successors.push(state),
            (_, ir::Cancel::Unspecified) => {
                return Err(
                    "cancellation requested without a declared cancellation contract".into(),
                );
            }
            (Lifecycle::Queued, _) => {
                state.tasks[i].lifecycle = Lifecycle::Done;
                metrics::observe(&mut state, model);
                successors.push(state);
            }
            (Lifecycle::Running { finish, .. }, ir::Cancel::Cooperative { ack_within }) => {
                let latest = state
                    .time
                    .checked_add(ack_within)
                    .ok_or("ack time overflow")?;
                // Beyond either natural completion or clock, all later acks
                // are observationally identical. Clamp BEFORE cloning states.
                // Keep one late representative rather than cutting it now:
                // other tasks can still finish within the clock on this path.
                let last = latest
                    .min(finish.saturating_add(1))
                    .min(model.bounds.clock.saturating_add(1));
                // Refuse before cloning when the candidates cannot fit.
                let count = usize::try_from(last - state.time).unwrap_or(usize::MAX);
                if let Some(sample) = room.sampling()
                    && count > 1
                {
                    return sample.reject();
                }
                room.take(count - 1)?;
                // d - 1 clones; the last candidate reuses the input state, so
                // what is materialized matches what was taken.
                for ack in state.time + 1..last {
                    let mut requested = state.clone();
                    requested.tasks[i].lifecycle = Lifecycle::Running {
                        finish,
                        ack: Some(ack),
                    };
                    successors.push(requested);
                }
                state.tasks[i].lifecycle = Lifecycle::Running {
                    finish,
                    ack: Some(last),
                };
                successors.push(state);
            }
            (
                Lifecycle::DistributedRunning {
                    started, next, due, ..
                },
                ir::Cancel::Cooperative { ack_within },
            ) => {
                let cap = &model.capabilities[task.capability.0];
                let max = cap
                    .duration_distribution
                    .as_ref()
                    .ok_or("missing duration distribution")?
                    .entries
                    .iter()
                    .filter(|(_, p)| !p.is_zero())
                    .map(|(t, _)| *t)
                    .max()
                    .ok_or("empty duration support")?;
                let finish = started.checked_add(max).ok_or("finish time overflow")?;
                let latest = state
                    .time
                    .checked_add(ack_within)
                    .ok_or("ack time overflow")?;
                let last = latest
                    .min(finish.saturating_add(1))
                    .min(model.bounds.clock.saturating_add(1));
                let count = usize::try_from(last - state.time).unwrap_or(usize::MAX);
                if let Some(sample) = room.sampling()
                    && count > 1
                {
                    return sample.reject();
                }
                room.take(count - 1)?;
                for ack in state.time + 1..last {
                    let mut requested = state.clone();
                    requested.tasks[i].lifecycle = Lifecycle::DistributedRunning {
                        started,
                        next,
                        due,
                        ack: Some(ack),
                    };
                    successors.push(requested);
                }
                state.tasks[i].lifecycle = Lifecycle::DistributedRunning {
                    started,
                    next,
                    due,
                    ack: Some(last),
                };
                successors.push(state);
            }
            (
                Lifecycle::Running { .. } | Lifecycle::DistributedRunning { .. },
                ir::Cancel::ForbiddenAfterStart,
            ) => successors.push(state),
        }
    }
    Ok(successors)
}

#[cfg(test)]
mod d1_tests {
    use super::*;
    use crate::obligation::Status;
    fn model(src: &str) -> ir::Model {
        crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap()
    }
    fn waive() -> ir::Model {
        model(include_str!("../../../fixtures/ctx-waive/model.relca"))
    }
    fn digest(s: &State) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    }

    #[test]
    fn d1_a1_request_and_queued_cancel_are_not_ack_events() {
        let source = include_str!("../../../fixtures/contract-cancel/model.relca");
        let prefix = source.split_once("  contract Controls {").unwrap().0;
        let m = model(&format!(
            "{prefix}contract Controls {{ excludes Slow.work.cancel_ack, Fast.win.start }} }}"
        ));
        let mut s = State::initial(&m, 0);
        s.time = 1;
        s.tasks.push(Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: ir::BindingId(1),
            capability: ir::CapabilityId(1),
            lifecycle: Lifecycle::Queued,
        });
        let queued = request_cancel(vec![s.clone()], 0, &m, &mut Room(0))
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(queued.tasks[0].lifecycle, Lifecycle::Done);
        assert!(!queued.monitors[0].activated);
        s.tasks[0].lifecycle = Lifecycle::Running {
            finish: 3,
            ack: None,
        };
        let mut requested = request_cancel(vec![s], 0, &m, &mut Room(0))
            .unwrap()
            .pop()
            .unwrap();
        assert!(!requested.monitors[0].activated);
        assert_eq!(
            requested.tasks[0].lifecycle,
            Lifecycle::Running {
                finish: 3,
                ack: Some(2)
            }
        );
        requested.time = 2;
        let acked = complete_observed(
            &requested,
            Due {
                i: 0,
                finish: 3,
                ack: Some(2),
            },
            &m,
            &mut Room(1),
            &mut Exploration::default(),
        )
        .unwrap()
        .pop()
        .unwrap();
        assert!(acked.monitors[0].activated);
    }

    #[test]
    fn d1_k3_same_trace_exclusion_never_changes_debt() {
        let m = model(include_str!(
            "../../../fixtures/ctx-obligation.broken-excludes-target/model.relca"
        ));
        let mut other = m.clone();
        if let ir::ContractKind::Excludes { before, .. } = &mut other.contracts[1].kind {
            before.kind = ir::EventKind::Finish(Some(ir::ValueId(0)));
        }
        let mut a = State::initial(&m, 0);
        let mut b = State::initial(&other, 0);
        for bundle in [
            Bundle::Received,
            Bundle::Finish(ir::CapabilityId(0), ir::ValueId(1), None),
            Bundle::Start(ir::CapabilityId(1), None),
        ] {
            emit_at(
                &mut a,
                bundle,
                Some(ir::BindingId(0)),
                &m,
                &mut Exploration::default(),
            )
            .unwrap();
            emit_at(
                &mut b,
                bundle,
                Some(ir::BindingId(0)),
                &other,
                &mut Exploration::default(),
            )
            .unwrap();
            assert_eq!(a.obligations, b.obligations);
        }
        assert_ne!(a.monitors[1].violated, b.monitors[1].violated);
    }

    #[test]
    fn d1_k22_debt_fields_participate_in_hash_eq_and_outcome_debug() {
        let m = waive();
        let mut s = State::initial(&m, 0);
        emit(&mut s, Bundle::Received, &m, &mut Exploration::default()).unwrap();
        s.time = 1;
        emit_at(
            &mut s,
            Bundle::Finish(ir::CapabilityId(0), ir::ValueId(0), Some(ir::PersonId(0))),
            Some(ir::BindingId(0)),
            &m,
            &mut Exploration::default(),
        )
        .unwrap();
        assert_eq!(
            s.obligations.as_ref().unwrap()[0]
                .resolution
                .as_ref()
                .unwrap()
                .status,
            Status::Waived
        );
        for field in 0..7 {
            let mut t = s.clone();
            let d = &mut t.obligations.as_mut().unwrap()[0];
            match field {
                0 => d.deadline += 1,
                1 => d.triggered_at += 1,
                2 => d.site = Some(ir::BindingId(0)),
                3 => d.resolution = None,
                4 => d.resolution.as_mut().unwrap().status = Status::Fulfilled,
                5 => d.resolution.as_mut().unwrap().authority = None,
                _ => d.resolution.as_mut().unwrap().executor = None,
            }
            assert_ne!(s, t);
            assert_ne!(digest(&s), digest(&t));
        }
        let out = explore(&m);
        let mut changed = out.clone();
        changed.obligations.as_mut().unwrap()[0].outcomes[0].status = Status::Fulfilled;
        assert_ne!(out, changed);
        assert!(format!("{out:?}").contains("obligations"));
        assert_ne!(format!("{out:?}"), format!("{changed:?}"));
    }

    #[test]
    fn d1_k23_room_and_maximum_held_include_new_models() {
        let m = model(include_str!(
            "../../../fixtures/ctx-waive-authority/model.relca"
        ));
        for budget in 0..=40 {
            let mut e = Exploration::default();
            let result = explore_into(&m, true, budget, std::convert::identity, &mut e);
            assert!(e.census.2 <= budget, "budget={budget} held={}", e.census.2);
            if budget <= 1 {
                assert_eq!(result, Err(Halt::Budget));
                assert_eq!(e.census.1, 0);
            }
        }
    }

    #[test]
    fn d1_simulation_keeps_deterministic_and_nondeterministic_classification() {
        let src = include_str!("../../../fixtures/ctx-waive/model.relca").replace(
            "  bounds",
            "  simulation replications 2 seed 0 confidence 0.5\n  bounds",
        );
        let m = model(&src);
        let a = simulate(&m);
        let b = crate::reference::simulate(&m);
        assert_eq!(a.completed_replications, 2, "{a:?}");
        assert_eq!(a, b.outcome);
        let nondet = model(&src.replace("executors [p]", "executors [p,q]"));
        let a = simulate(&nondet);
        assert_eq!(a.completed_replications, 0);
        assert!(a.diagnostics.iter().any(|d| d.code == crate::diag::U_SEM_0));
    }

    #[test]
    fn d1_all_three_drivers_keep_probabilistic_and_cut_debt_outcomes() {
        let source = include_str!("../../../fixtures/ctx-waive/model.relca");
        for probability in [false, true] {
            for arrivals in [false, true] {
                for clock in [0, 2, 3, 20] {
                    let mut src = source.replace("clock 20min", &format!("clock {clock}min"));
                    if probability {
                        src = src.replace(
                            "commits; duration fixed 1min",
                            "commits; duration {1min:0.5,3min:0.5}",
                        );
                    }
                    if arrivals {
                        src = src.replace("  bounds", "  arrivals fixed [0min]\n  bounds");
                    }
                    let m = model(&src);
                    let e = explore(&m);
                    let r = crate::reference::explore(&m);
                    assert_eq!(r.stop, crate::reference::StopReason::Complete);
                    let mut reference = r.outcome;
                    reference.diagnostics.drain(..m.diagnostics.len());
                    assert_eq!(
                        e, reference,
                        "probability={probability} arrivals={arrivals} clock={clock}"
                    );
                    let rows = e.obligations.as_ref().unwrap();
                    assert_eq!(rows.len(), 1);
                    if clock == 20 {
                        assert!(
                            rows[0]
                                .outcomes
                                .iter()
                                .all(|o| o.status != Status::Unresolved)
                        );
                    }
                    if probability && clock == 20 {
                        assert_eq!(rows[0].outcomes.len(), 2);
                    }
                }
            }
        }
    }

    #[test]
    fn d1_untrusted_indices_nonresponse_targets_and_ordinals_fail_closed() {
        let valid = waive();
        for fault in 0..5 {
            let mut m = valid.clone();
            match fault {
                0 => {
                    if let ir::ContractKind::Waive { response, .. } = &mut m.contracts[1].kind {
                        *response = ir::ClauseId(usize::MAX);
                    }
                }
                1 => {
                    if let ir::ContractKind::Waive { response, .. } = &mut m.contracts[1].kind {
                        *response = ir::ClauseId(1);
                    }
                }
                2 => {
                    if let ir::ContractKind::Waive { authority, .. } = &mut m.contracts[1].kind {
                        *authority = ir::AuthorityId(usize::MAX);
                    }
                }
                3 => m.contracts[1].ordinal = m.contracts[0].ordinal,
                _ => {
                    if let ir::ContractKind::NamedResponse { non_waivable, .. } =
                        &mut m.contracts[0].kind
                    {
                        *non_waivable = true;
                    }
                }
            }
            assert!(
                explore(&m)
                    .diagnostics
                    .iter()
                    .any(|d| d.code == crate::diag::U_SEM_0)
            );
            assert_eq!(
                crate::reference::explore(&m).stop,
                crate::reference::StopReason::SemanticAbort
            );
        }
    }
    #[test]
    fn d1_abort_retains_same_bundle_resolutions_in_both_declaration_orders() {
        let base = include_str!("../../../fixtures/ctx-waive/model.relca");
        let boom = "response boom: Officer.release.finish -> one_of(end.done) by 18446744073709551615min after Officer.release.finish otherwise unresolved\n";
        for before in [false, true] {
            let src = if before {
                base.replace(
                    "    response delivery:",
                    &format!("    {boom}    response delivery:"),
                )
            } else {
                base.replace("    waive", &format!("    {boom}    waive"))
            };
            let src = src.replace("    waive", "    response audit: Officer.release.finish -> one_of(end.done) by 5min after Officer.release.finish otherwise unresolved\n    waive");
            for (probability, arrivals) in
                [(false, false), (true, false), (false, true), (true, true)]
            {
                let mut src = src.clone();
                if probability {
                    src = src.replace("commits; duration fixed 1min", "commits; duration {1min:1}");
                }
                if arrivals {
                    src = src.replace("  bounds", "  arrivals fixed [0min]\n  bounds");
                }
                let m = model(&src);
                let e = explore(&m);
                let r = crate::reference::explore(&m);
                assert_eq!(r.stop, crate::reference::StopReason::SemanticAbort);
                let rows = e.obligations.as_ref().unwrap();
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].outcomes.len(), 1);
                assert_eq!(rows[0].outcomes[0].status, Status::Waived);
                assert_eq!(rows[1].trigger_site, "o");
                assert_eq!(rows[1].outcomes.len(), 1);
                assert_eq!(rows[1].outcomes[0].status, Status::Unresolved);
                assert_eq!(
                    e.obligations, r.outcome.obligations,
                    "probability={probability} arrivals={arrivals}"
                );
            }
        }
    }
}

#[cfg(test)]
mod state_tests {
    use super::*;
    use ir::{BindingId as B, CapabilityId as C, StateValue as V, ValueId as E};

    fn parse(s: &str) -> ir::Model {
        crate::lower::lower(&crate::syntax::parse(s).unwrap()).unwrap()
    }
    fn product() -> ir::Model {
        parse(include_str!(
            "../../../fixtures/state-latent-product/model.relca"
        ))
    }
    fn writers() -> ir::Model {
        parse(
            r#"model M version "0.2" {
        type T = enum { ok }; observed case { flag: bool = false }
        actor A { capacity 1; capability set() -> T { writes flag; finish { flag = true }; duration fixed 1min; result nondet { ok } } }
        actor B { capacity 1; capability clear() -> T { writes flag; finish { flag = false }; duration fixed 1min; result nondet { ok } } }
        process P { a = A.set(); b = B.clear(); end done }
        bounds { cases 1; live_tasks 2; clock 10min }
        contract C { invariant A.set.finish -> flag == true; invariant B.clear.finish -> flag == false }
    }"#,
        )
    }

    #[test]
    fn s_state_key_all_namespaces_and_optional() {
        let m = parse(
            r#"model M version "0.2" { type T = enum { yes,no }
            observed case { flag: bool = false; choice: optional<T> }
            latent case { truth: bool }
            actor D { latent difficulty: T }
            process P { end done }; bounds { cases 1; live_tasks 1; clock 1min }
            verify safety no_resource_overcommit }"#,
        );
        let original = State::initial(&m, 0);
        for i in 0..4 {
            let mut other = original.clone();
            other.state_values[i] = if i == 1 {
                V::Value(E(0))
            } else {
                V::Value(E(1 - match original.state_values[i] {
                    V::Value(x) => x.0,
                    _ => unreachable!(),
                }))
            };
            assert_ne!(original, other);
            let set = HashSet::from([original.clone(), other.clone()]);
            assert_eq!(set.len(), 2);
            assert!(set.contains(&original));
            assert!(set.contains(&other));
        }
        let source = include_str!("../../../fixtures/state-policy/model.relca");
        let before = explore(&parse(source));
        let after = explore(&parse(&source.replace(
            "difficult_seen: bool = false",
            "difficult_seen: bool = true",
        )));
        assert_ne!(before.terminals, after.terminals);
    }

    #[test]
    fn s_latent_product_receipt_once_and_room_before_clone() {
        let m = product();
        let seed = State::initial(&m, 0);
        assert!(!seed.initialized);
        let mut evidence = Exploration::default();
        let mut room = Room(4);
        let initialized = initialize_latents(&seed, &m, &mut room, &mut evidence).unwrap();
        assert_eq!(room.0, 0);
        assert_eq!(evidence.initial_clones, 4);
        assert_eq!(1 + initialized.len(), 5); // held seed plus four successors
        let tuples: HashSet<_> = initialized.iter().map(|s| s.state_values.clone()).collect();
        let expected: HashSet<_> = [
            vec![V::Value(E(0)), V::Value(E(0))],
            vec![V::Value(E(0)), V::Value(E(1))],
            vec![V::Value(E(1)), V::Value(E(0))],
            vec![V::Value(E(1)), V::Value(E(1))],
        ]
        .into();
        assert_eq!(tuples, expected);
        assert_eq!(evidence.state_events.len(), 4);
        assert!(evidence.state_events.iter().all(|(at, b, values)| *at == 0
            && matches!(b, Bundle::Received)
            && tuples.contains(values)));
        let mut short = Exploration::default();
        assert_eq!(
            initialize_latents(&seed, &m, &mut Room(3), &mut short).unwrap_err(),
            Halt::Budget
        );
        assert_eq!(short.initial_clones, 3);
        assert_eq!(short.state_events.len(), 3);
        let mut all = Exploration::default();
        explore_into(
            &m,
            true,
            DEFAULT_STATE_BUDGET,
            std::convert::identity,
            &mut all,
        )
        .unwrap();
        assert_eq!(all.initial_clones, 4);
        for (_, _, values) in &all.state_events {
            assert!(tuples.contains(values));
        }
        assert_eq!(
            all.state_events
                .iter()
                .filter(|(_, b, _)| matches!(b, Bundle::Received))
                .count(),
            4
        );
        let mut singleton = m.clone();
        for t in &mut singleton.types {
            t.members.truncate(1);
        }
        let mut e = Exploration::default();
        assert_eq!(
            initialize_latents(
                &State::initial(&singleton, 0),
                &singleton,
                &mut Room(1),
                &mut e
            )
            .unwrap()
            .len(),
            1
        );
        assert_eq!(e.initial_clones, 1);
        let mut huge = m.clone();
        let v = huge.state_variables[0].clone();
        huge.state_variables = (0..128)
            .map(|i| {
                let mut v = v.clone();
                v.name = format!("v{i}");
                v
            })
            .collect();
        let mut e = Exploration::default();
        assert_eq!(
            initialize_latents(&State::initial(&huge, 0), &huge, &mut Room(2), &mut e).unwrap_err(),
            Halt::Budget
        );
        assert_eq!(e.initial_clones, 2);
        assert!(State::initial(&writers(), 0).initialized);
        let mut empty = writers();
        empty.state_variables.clear();
        assert!(State::initial(&empty, 0).initialized);
    }

    #[test]
    fn s_snapshot_and_zero_engine_postwrite() {
        let m = writers();
        let mut e = Exploration::default();
        explore_into(
            &m,
            true,
            DEFAULT_STATE_BUDGET,
            std::convert::identity,
            &mut e,
        )
        .unwrap();
        assert!(
            e.outcome(&m)
                .properties
                .iter()
                .all(|p| p.verdict == Verdict::Satisfied)
        );
        let mut zero = m;
        zero.capabilities[0].duration = 0;
        let mut e = Exploration::default();
        explore_into(
            &zero,
            true,
            DEFAULT_STATE_BUDGET,
            std::convert::identity,
            &mut e,
        )
        .unwrap();
        assert!(e.state_events.iter().any(|(at, b, v)| *at == 0
            && matches!(b, Bundle::Start(C(0), _))
            && v == &[V::Value(E(1))]));
        assert!(e.state_events.iter().any(|(at, b, v)| *at == 0
            && matches!(b, Bundle::Finish(C(0), _, _))
            && v == &[V::Value(E(0))]));
        assert!(
            e.outcome(&zero)
                .properties
                .iter()
                .all(|p| p.verdict == Verdict::Satisfied)
        );
    }

    #[test]
    fn s_cancel_request_ack_tie_queued_and_forbidden() {
        let source = include_str!("../../../fixtures/state-cancel/model.relca");
        for initial in [false, true] {
            let mut m = parse(source);
            // Slow writes the opposite value, so cancellation must preserve a nondefault too.
            let value = V::Value(E(usize::from(!initial)));
            m.state_variables[0].initial = ir::InitialValue::Fixed(value);
            m.capabilities[1].writes[0].source =
                ir::WriteSource::Constant(V::Value(E(usize::from(initial))));
            let mut state = State::initial(&m, 0);
            state.time = 1;
            state.tasks = vec![
                Task {
                    frozen_performance: None,
                    stale: Vec::new(),
                    executor: None,
                    binding: B(0),
                    capability: C(0),
                    lifecycle: Lifecycle::Done,
                },
                Task {
                    frozen_performance: None,
                    stale: Vec::new(),
                    executor: None,
                    binding: B(1),
                    capability: C(1),
                    lifecycle: Lifecycle::Running {
                        finish: 2,
                        ack: None,
                    },
                },
            ];
            let requested = request_cancel(vec![state.clone()], 1, &m, &mut Room(10))
                .unwrap()
                .remove(0);
            assert_eq!(requested.state_values, vec![value]);
            let mut due = requested;
            due.time = 2;
            let completed = complete(
                &due,
                Due {
                    i: 1,
                    finish: 2,
                    ack: Some(2),
                },
                &m,
                &mut Room(2),
            )
            .unwrap();
            assert_eq!(completed.len(), 2);
            assert_eq!(
                completed[0].state_values,
                vec![V::Value(E(usize::from(initial)))]
            );
            assert_eq!(completed[1].state_values, vec![value]);
            assert!(completed[1].results[1].is_none());
            // One shared executor: Fast has finished, Slow remains queued and has no person.
            m.people = vec![ir::Person {
                name: "p".into(),
                span: m.process.span,
            }];
            for a in &mut m.actors {
                a.executors = Some(vec![ir::PersonId(0)]);
                a.capacity = Some(1);
            }
            state.tasks[0].executor = Some(ir::PersonId(0));
            state.tasks[1].lifecycle = Lifecycle::Queued;
            let queued = request_cancel(vec![state.clone()], 1, &m, &mut Room(0))
                .unwrap()
                .remove(0);
            assert_eq!(queued.state_values, vec![value]);
            assert_eq!(queued.tasks[1].lifecycle, Lifecycle::Done);
            assert_eq!(queued.tasks[1].executor, None);
            m.capabilities[1].cancel = ir::Cancel::ForbiddenAfterStart;
            state.tasks[1].lifecycle = Lifecycle::Running {
                finish: 2,
                ack: None,
            };
            let mut ignored = request_cancel(vec![state], 1, &m, &mut Room(0))
                .unwrap()
                .remove(0);
            assert_eq!(ignored.state_values, vec![value]);
            ignored.time = 2;
            assert_eq!(
                complete(
                    &ignored,
                    Due {
                        i: 1,
                        finish: 2,
                        ack: None
                    },
                    &m,
                    &mut Room(1)
                )
                .unwrap()[0]
                    .state_values,
                vec![V::Value(E(usize::from(initial)))]
            );
        }
    }

    #[test]
    fn s_same_tick_reader_sees_both_event_orders() {
        let m = parse(
            r#"model M version "0.2" { type T = enum { ok }; observed case { flag: bool = false }
          actor A { capacity 1; capability run() -> T { writes flag; finish { flag = true }; duration fixed 1min; result nondet { ok } } }
          actor Probe { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } }
          process P { fork f { a=A.run(); b=Probe.run() }; join all f; end done }
          bounds { cases 1; live_tasks 2; clock 2min }
          contract C { invariant Probe.run.finish -> flag == true } }"#,
        );
        let mut e = Exploration::default();
        explore_into(
            &m,
            true,
            DEFAULT_STATE_BUDGET,
            std::convert::identity,
            &mut e,
        )
        .unwrap();
        for value in [V::Value(E(0)), V::Value(E(1))] {
            assert!(e.state_events.iter().any(|(at, b, v)| *at == 1
                && matches!(b, Bundle::Finish(C(1), _, _))
                && v == &[value]));
        }
        assert!(
            e.state_events
                .iter()
                .filter(|(_, b, _)| matches!(b, Bundle::Start(_, _)))
                .all(|(at, _, _)| *at == 0)
        );
    }

    #[test]
    fn s_invalid_state_ir_and_ast_fail_closed() {
        assert!(
            crate::syntax::parse(
                "model M version \"0.2\" { latent\ncase { truth: bool = false } }"
            )
            .is_ok()
        );
        let m = writers();
        let mutations: &[fn(&mut ir::Model)] = &[
            |m| m.state_variables[0].base = ir::TypeId(999),
            |m| m.state_variables[0].initial = ir::InitialValue::Fixed(V::Value(E(999))),
            |m| m.state_variables[0].initial = ir::InitialValue::Fixed(V::None),
            |m| m.state_variables[0].initial = ir::InitialValue::Nondet,
            |m| {
                m.state_variables[0].kind = ir::StateKind::LatentCase;
                m.state_variables[0].optional = true;
            },
            |m| m.capabilities[0].writes[0].variable = ir::StateVarId(999),
            |m| m.state_variables[0].kind = ir::StateKind::LatentCase,
            |m| {
                let w = m.capabilities[0].writes[0];
                m.capabilities[0].writes.push(w);
            },
            |m| m.capabilities[0].writes[0].source = ir::WriteSource::Result,
            |m| m.capabilities[0].writes[0].source = ir::WriteSource::Constant(V::None),
            |m| {
                if let ir::ContractKind::Invariant { condition, .. } = &mut m.contracts[0].kind {
                    *condition = Some(ir::Cond::StateEq(ir::StateMatch {
                        variable: ir::StateVarId(999),
                        value: V::None,
                    }));
                }
            },
            |m| {
                if let ir::ContractKind::Invariant { condition, .. } = &mut m.contracts[0].kind {
                    *condition = Some(ir::Cond::StateEq(ir::StateMatch {
                        variable: ir::StateVarId(0),
                        value: V::Value(E(999)),
                    }));
                }
            },
            |m| {
                m.capabilities.iter_mut().for_each(|c| c.writes.clear());
                m.state_variables[0].kind = ir::StateKind::LatentCase;
                m.process.body.insert(
                    0,
                    ir::Statement {
                        span: m.process.span,
                        kind: ir::Stmt::If {
                            condition: ir::Cond::StateEq(ir::StateMatch {
                                variable: ir::StateVarId(0),
                                value: V::Value(E(0)),
                            }),
                            then_body: vec![],
                            else_body: vec![],
                        },
                    },
                );
            },
        ];
        for mutate in mutations {
            let mut bad = m.clone();
            mutate(&mut bad);
            assert!(validate(&bad).is_err());
            let out = explore(&bad);
            assert!(out.terminals.is_empty());
            assert!(out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
        }
        let source = include_str!("../../../fixtures/state-policy/model.relca");
        for empty in [false, true] {
            let mut ast = crate::syntax::parse(source).unwrap();
            for item in &mut ast.items {
                if let crate::ast::Item::ObservedCase(ds) = item {
                    if empty {
                        ds[0].names.clear();
                    } else {
                        ds[0].optional = false;
                        ds[0].default = None;
                    }
                }
            }
            assert!(crate::lower::lower(&ast).is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ir::{BindingId as B, CapabilityId as C, EndId as E, ValueId as V};
    use serde_json::{Value, json};

    const SPAN: diag::Span = diag::Span { start: 0, end: 0 };

    #[test]
    fn contract_monitor_history_participates_in_state_identity() {
        // Equal control/results/time can have different future obligations.
        // This is a key-level check, independent of eager violation evidence.
        let mut state = State::initial(&base(1, None), 0);
        state.monitors = vec![Monitor::default()];
        let mut states = HashSet::from([state.clone()]);
        for monitor in [
            Monitor {
                activated: true,
                ..Monitor::default()
            },
            Monitor {
                violated: true,
                ..Monitor::default()
            },
            Monitor {
                count: 1,
                ..Monitor::default()
            },
            Monitor {
                before_seen: true,
                ..Monitor::default()
            },
            Monitor {
                pending: vec![2],
                ..Monitor::default()
            },
            Monitor {
                pending: vec![3],
                ..Monitor::default()
            },
            Monitor {
                pending: vec![2, 3],
                ..Monitor::default()
            },
        ] {
            let mut different = state.clone();
            different.monitors[0] = monitor;
            assert_ne!(state, different);
            assert!(states.insert(different));
        }
        assert_eq!(states.len(), 8);
    }

    #[test]
    fn budget_refusal_does_not_advance_response_deadlines() {
        let mut model = base(1, None);
        model.bounds.clock = 10;
        let mut program = vec![Instruction::Stop];
        let entry = compile(&model.process.body, 0, &mut program);
        let mut state = State::initial(&model, entry);
        state.waiting = true;
        state.tasks = vec![Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: B(0),
            capability: C(0),
            lifecycle: Lifecycle::Running {
                finish: 3,
                ack: None,
            },
        }];
        state.monitors = vec![Monitor {
            activated: true,
            pending: vec![1],
            ..Monitor::default()
        }];
        let mut exploration = Exploration::default();
        exploration.contracts(&state);
        assert!(matches!(
            step_observed(state, &program, &model, &mut Room(0), &mut exploration),
            Err(Halt::Budget)
        ));
        assert_eq!(exploration.violation, vec![false]);
    }

    #[test]
    fn clause_violation_in_one_bundle_survives_later_clause_overflow() {
        let source = r#"model M version "0.2" {
            type T = enum { ok, bad }
            actor A { capacity 1; capability a() -> T { duration fixed 1min; result nondet { ok } } }
            process P { x = A.a(); end done }
            contract {
                invariant A.a.finish -> x == bad
                response A.a.finish -> one_of(end.done) by 18446744073709551615min after A.a.finish otherwise unresolved
            }
            bounds { cases 1; live_tasks 1; clock 10min }
        }"#;
        let model = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
        let out = explore(&model);
        assert_eq!(out.properties[0].verdict, Verdict::Violated);
        assert_eq!(out.properties[1].verdict, Verdict::Unsupported);
        assert!(out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
    }

    fn stmt(kind: ir::Stmt) -> ir::Statement {
        ir::Statement { kind, span: SPAN }
    }

    fn call(binding: usize, capability: usize) -> ir::Call {
        ir::Call {
            args: Vec::new(),
            binding: B(binding),
            capability: C(capability),
            span: SPAN,
        }
    }

    fn eq(binding: usize, value: usize) -> ir::Cond {
        ir::Cond::Eq(ir::ResultMatch {
            binding: B(binding),
            value: V(value),
        })
    }

    fn choose(condition: ir::Cond, yes: usize, no: usize) -> ir::Statement {
        stmt(ir::Stmt::If {
            condition,
            then_body: vec![stmt(ir::Stmt::End(E(yes)))],
            else_body: vec![stmt(ir::Stmt::End(E(no)))],
        })
    }

    fn any(remaining: ir::Remaining) -> ir::Join {
        ir::Join::Any {
            matches: vec![
                ir::ResultMatch {
                    binding: B(0),
                    value: V(0),
                },
                ir::ResultMatch {
                    binding: B(1),
                    value: V(0),
                },
            ],
            tie: ir::Tie::By(vec![B(0), B(1)]),
            remaining,
            if_none: E(1),
        }
    }

    fn fork(join: ir::Join) -> ir::Statement {
        stmt(ir::Stmt::Fork {
            id: ir::ForkId(0),
            branches: vec![call(0, 0), call(1, 1)],
            join,
        })
    }

    fn base(capacity: u64, join: Option<ir::Join>) -> ir::Model {
        let body = if let Some(join) = join {
            vec![fork(join), stmt(ir::Stmt::End(E(0)))]
        } else {
            vec![
                stmt(ir::Stmt::Call(call(0, 0))),
                stmt(ir::Stmt::Call(call(1, 1))),
                stmt(ir::Stmt::End(E(0))),
            ]
        };
        ir::Model {
            contract_selection: Default::default(),
            case_priority: None,
            performances: Vec::new(),
            comparison: None,
            concurrency_resolutions: Vec::new(),
            resource_time: None,
            arrival_distribution: None,
            arrival_every: None,
            horizon: None,
            scenario: None,
            fallback_capabilities: Vec::new(),
            quality: None,
            resources: Vec::new(),
            responsibility: None,
            simulation: None,
            metrics: None,
            arrivals: None,
            start_policy: None,
            admission_queue: None,
            concurrent_cases: None,
            overflow_end: None,
            kernels: Vec::new(),
            initial_distributions: Vec::new(),
            probability_queries: Vec::new(),
            state_variables: Vec::new(),
            people: Vec::new(),
            authorities: Vec::new(),
            contracts: Vec::new(),
            agency: crate::agency::Agency::empty().into(),
            name: "hand-built".into(),
            version: "0.2".into(),
            types: vec![ir::EnumType {
                record: None,
                name: "Verdict".into(),
                members: vec!["pass".into()],
                span: SPAN,
            }],
            actors: vec![ir::Actor {
                metadata: ir::ActorMetadata::default(),
                executors: None,
                source: None,
                name: "Office".into(),
                capacity: Some(capacity),
                span: SPAN,
            }],
            capabilities: [3, 5]
                .into_iter()
                .enumerate()
                .map(|(i, duration)| ir::Capability {
                    receipt_query: None,
                    qualified_executors: None,
                    performance: None,
                    version_check: None,
                    read_variables: Vec::new(),
                    atomic: None,
                    uses: Vec::new(),
                    requires: None,
                    start_writes: Vec::new(),
                    duration_distribution: None,
                    kernel: None,
                    kernel_reads_latent: Vec::new(),
                    kernel_writes: Vec::new(),
                    writes: Vec::new(),
                    permission: None,
                    params: Vec::new(),
                    reads: Vec::new(),
                    commits: false,
                    name: ["short", "long"][i].into(),
                    actor: ir::ActorId(0),
                    return_type: ir::TypeId(0),
                    duration,
                    results: vec![V(0)],
                    cancel: ir::Cancel::Unspecified,
                    span: SPAN,
                })
                .collect(),
            process: ir::Process {
                degradations: Vec::new(),
                name: "Check".into(),
                bindings: ["a", "b"]
                    .into_iter()
                    .map(|name| ir::Binding {
                        merge: None,
                        repetition: None,
                        name: name.into(),
                        ty: ir::TypeId(0),
                        span: SPAN,
                    })
                    .collect(),
                forks: vec!["checks".into()],
                body,
                span: SPAN,
            },
            ends: vec!["done".into(), "no_match".into()],
            bounds: ir::Bounds {
                cases: 1,
                live_tasks: 2,
                clock: 8,
            },
            verify: vec![ir::Verify {
                property: ir::Property::ReachableEnd(E(0)),
                span: SPAN,
            }],
            assumptions: Vec::new(),
            diagnostics: Vec::new(),
            cut: None,
        }
    }

    fn with_safety(mut model: ir::Model) -> ir::Model {
        model.verify.insert(
            0,
            ir::Verify {
                property: ir::Property::NoResourceOvercommit,
                span: SPAN,
            },
        );
        model
    }

    fn cancel_model(cooperative: bool) -> ir::Model {
        let mut model = base(2, Some(any(ir::Remaining::CancelThenDrain)));
        for cap in &mut model.capabilities {
            cap.cancel = ir::Cancel::ForbiddenAfterStart;
        }
        if cooperative {
            model.capabilities[1].cancel = ir::Cancel::Cooperative { ack_within: 1 };
        }
        model.bounds.clock = if cooperative { 4 } else { 5 };
        model.process.body[1] = choose(eq(0, 0), 0, 1);
        model
    }

    fn range(range: outcome::Range) -> Value {
        json!({"min": range.min, "max": range.max})
    }

    // Compare precisely the engine-owned fields of the immutable expected JSON.
    // W-CUT-0, cut and the folded report verdict belong to lower/report, not engine.
    fn fixture(model: &ir::Model, expected: &str) -> outcome::Outcome {
        let expected: Value = serde_json::from_str(expected).unwrap();
        let out = explore(model);
        let properties: Vec<_> = out
            .properties
            .iter()
            .map(|property| {
                let id = match property.property {
                    ir::Property::NoLostUpdate(id) => model.loss_property_id(id),
                    ir::Property::StateInRange => "state_in_range".into(),
                    ir::Property::RecoverableEnd
                    | ir::Property::TerminatingEnd
                    | ir::Property::AlmostSureEnd => unreachable!("legacy test"),
                    ir::Property::Contest(_) => unreachable!(),
                    ir::Property::ProbabilityReached(_) => unreachable!("legacy test"),
                    ir::Property::Contract(_) => unreachable!("legacy fixture has no contracts"),
                    ir::Property::NoResourceOvercommit => "safety no_resource_overcommit".into(),
                    ir::Property::ReachableEnd(end) => {
                        format!("reachable end.{}", model.ends[end.0])
                    }
                };
                json!({"id": id, "verdict": property.verdict.as_str()})
            })
            .collect();
        let terminals: Vec<_> = out
            .terminals
            .iter()
            .map(|terminal| {
                json!({
                    "end": model.ends[terminal.end.0], "elapsed": range(terminal.elapsed),
                    "work": range(terminal.work), "result_at": terminal.result_at.map(range),
                })
            })
            .collect();
        assert_eq!(json!(properties), expected["properties"]);
        assert_eq!(json!(terminals), expected["terminals"]);
        let engine_codes: Vec<_> = expected["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|d| d["code"].as_str())
            .filter(|code| *code != diag::W_CUT_0)
            .collect();
        assert_eq!(
            out.diagnostics
                .iter()
                .map(|d| d.code.as_str())
                .collect::<Vec<_>>(),
            engine_codes
        );
        out
    }

    #[test]
    fn fixture_serial_time() {
        let out = fixture(
            &base(1, None),
            include_str!("../../../fixtures/serial-time/expected.json"),
        );
        assert!(!out.truncated); // exactly clock 8 is not a cut
    }

    #[test]
    fn fixture_parallel_time() {
        let mut model = base(2, Some(ir::Join::All));
        model.bounds.clock = 5;
        let out = fixture(
            &model,
            include_str!("../../../fixtures/parallel-time/expected.json"),
        );
        assert!(!out.truncated);
    }

    #[test]
    fn fixture_resource_contention() {
        let out = fixture(
            &with_safety(base(1, Some(ir::Join::All))),
            include_str!("../../../fixtures/resource-contention/expected.json"),
        );
        assert!(!out.truncated);
    }

    #[test]
    fn fixture_any_drain() {
        let out = fixture(
            &cancel_model(false),
            include_str!("../../../fixtures/any-drain/expected.json"),
        );
        assert!(!out.truncated);
    }

    #[test]
    fn fixture_coop_cancel() {
        let out = fixture(
            &cancel_model(true),
            include_str!("../../../fixtures/coop-cancel/expected.json"),
        );
        assert!(!out.truncated);
    }

    #[test]
    fn fixture_zero_capacity_is_deadlock_not_invalid_or_cut() {
        let out = fixture(
            &with_safety(base(0, Some(ir::Join::All))),
            include_str!(
                "../../../fixtures/resource-contention.broken-zero-capacity/expected.json"
            ),
        );
        assert!(!out.truncated);
    }

    #[test]
    fn fixture_short_clock_is_cut_not_terminal_or_deadlock() {
        let mut model = cancel_model(true);
        model.bounds.clock = 3;
        let out = fixture(
            &model,
            include_str!("../../../fixtures/coop-cancel.broken-short-clock/expected.json"),
        );
        assert!(out.truncated);
    }

    fn fixed(value: u64) -> outcome::Range {
        outcome::Range {
            min: value,
            max: value,
        }
    }

    // Hand-built models below use one fixed SPAN. Compare the whole warning
    // sequence, including messages/spans, instead of ignoring new diagnostics.
    fn unreachable(out: &outcome::Outcome, expected: &[(&str, &str)]) {
        assert_eq!(
            out.diagnostics,
            expected
                .iter()
                .map(|(code, message)| diag::Diagnostic {
                    code: (*code).into(),
                    severity: diag::Severity::Warning,
                    message: format!("process Check: {message}"),
                    span: Some(SPAN),
                })
                .collect::<Vec<_>>()
        );
        assert!(out.unchecked.is_empty());
    }

    #[test]
    fn all_result_combinations_are_explored_at_finish() {
        let mut model = base(2, Some(ir::Join::All));
        model.types[0].members.push("fail".into());
        for cap in &mut model.capabilities {
            cap.results.push(V(1));
        }
        model.ends = (0..4).map(|i| format!("end{i}")).collect();
        model.process.body[1] = stmt(ir::Stmt::If {
            condition: eq(0, 0),
            then_body: vec![choose(eq(1, 0), 0, 1)],
            else_body: vec![choose(eq(1, 0), 2, 3)],
        });
        model.verify = (0..4)
            .map(|i| ir::Verify {
                property: ir::Property::ReachableEnd(E(i)),
                span: SPAN,
            })
            .collect();
        let out = explore(&model);
        assert_eq!(out.terminals.len(), 4);
        assert!(
            out.properties
                .iter()
                .all(|p| p.verdict == Verdict::Satisfied)
        );
        assert!(
            out.terminals
                .iter()
                .all(|t| t.elapsed == fixed(5) && t.work == fixed(8))
        );
    }

    #[test]
    fn contending_starts_race_join_but_unstarted_cancellation_has_zero_work() {
        let mut model = cancel_model(false);
        model.actors[0].capacity = Some(1);
        model.bounds.clock = 8;
        // Either branch can win; do not read the possibly cancelled a afterward.
        model.process.body[1] = stmt(ir::Stmt::End(E(0)));
        let out = explore(&model);
        unreachable(
            &out,
            &[(diag::W_UNREACH_2, "end.no_match is never reached")],
        );
        let t = &out.terminals[0];
        // Either task can start first. If join cancels before redispatch, only
        // that task works (3 or 5). If the other starts before join, its
        // forbidden_after_start contract drains both tasks (8).
        assert_eq!(t.elapsed, outcome::Range { min: 3, max: 8 });
        assert_eq!(t.work, t.elapsed);
        assert_eq!(t.result_at, Some(outcome::Range { min: 3, max: 5 }));
        assert!(!out.truncated);

        let mut queued = State::initial(&model, 0);
        queued.tasks = vec![Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: B(1),
            capability: C(1),
            lifecycle: Lifecycle::Queued,
        }];
        let cancelled =
            request_cancel(vec![queued], 0, &model, &mut Room(DEFAULT_STATE_BUDGET)).unwrap();
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].tasks[0].lifecycle, Lifecycle::Done);
        assert_eq!(cancelled[0].work, 0);
        assert_eq!(cancelled[0].results, vec![None, None]);
    }

    #[test]
    fn drain_does_not_cancel_and_keeps_both_adoption_extrema_after_state_merge() {
        let model = base(1, Some(any(ir::Remaining::Drain)));
        let out = explore(&model);
        assert_eq!(out, run(&model, false, DEFAULT_STATE_BUDGET));
        let t = &out.terminals[0];
        assert_eq!(t.elapsed, fixed(8));
        assert_eq!(t.work, fixed(8));
        assert_eq!(t.result_at, Some(outcome::Range { min: 3, max: 5 }));
    }

    #[test]
    fn cooperative_ack_explores_every_delay_and_retains_work_until_ack() {
        let mut model = cancel_model(true);
        model.capabilities[1].duration = 10;
        model.capabilities[1].cancel = ir::Cancel::Cooperative { ack_within: 3 };
        model.bounds.clock = 6;
        let out = explore(&model);
        assert_eq!(out.terminals[0].elapsed, outcome::Range { min: 4, max: 6 });
        assert_eq!(out.terminals[0].work, outcome::Range { min: 7, max: 9 });
        assert_eq!(out.terminals[0].result_at, Some(fixed(3)));
        assert!(!out.truncated);
    }

    #[test]
    fn trillion_tick_ack_interval_generates_only_clock_bounded_candidates() {
        let mut model = with_safety(cancel_model(true));
        let huge = 1_000_000_000_000;
        model.capabilities[1].duration = huge;
        model.capabilities[1].cancel = ir::Cancel::Cooperative { ack_within: huge };
        let mut state = State::initial(&model, 0);
        state.time = 3;
        state.tasks = vec![Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: B(1),
            capability: C(1),
            lifecycle: Lifecycle::Running {
                finish: huge,
                ack: None,
            },
        }];
        let candidates =
            request_cancel(vec![state], 0, &model, &mut Room(DEFAULT_STATE_BUDGET)).unwrap();
        assert_eq!(candidates.len(), 2); // ack at clock 4, one late representative
        assert_eq!(
            candidates
                .iter()
                .map(|s| s.tasks[0].lifecycle)
                .collect::<Vec<_>>(),
            [4, 5].map(|ack| Lifecycle::Running {
                finish: huge,
                ack: Some(ack)
            })
        );

        let out = explore(&model);
        assert!(out.truncated);
        assert!(out.diagnostics.is_empty());
        assert_eq!(out.properties[0].verdict, Verdict::Inconclusive);
        assert_eq!(out.properties[1].verdict, Verdict::Satisfied);
        assert_eq!(out.terminals.len(), 1);
        assert_eq!(out.terminals[0].end, E(0));
        assert_eq!(out.terminals[0].elapsed, fixed(4));
        assert_eq!(out.terminals[0].work, fixed(7));
        assert_eq!(out.terminals[0].result_at, Some(fixed(3)));
    }

    #[test]
    fn late_ack_representative_keeps_other_tasks_in_bound_finish() {
        let mut model = base(2, Some(ir::Join::All));
        model.bounds.clock = 4;
        model.capabilities[1].cancel = ir::Cancel::Cooperative {
            ack_within: 1_000_000_000_000,
        };
        let mut program = vec![Instruction::Stop];
        let entry = compile(&model.process.body, 0, &mut program);
        let mut state = State::initial(&model, entry);
        state.waiting = true;
        state.time = 3;
        state.work = 6;
        state.tasks = [4, 1_000_000_000_000]
            .into_iter()
            .enumerate()
            .map(|(i, finish)| Task {
                frozen_performance: None,
                stale: Vec::new(),
                executor: None,
                binding: B(i),
                capability: C(i),
                lifecycle: Lifecycle::Running { finish, ack: None },
            })
            .collect();
        let mut candidates =
            request_cancel(vec![state], 1, &model, &mut Room(DEFAULT_STATE_BUDGET)).unwrap();
        assert_eq!(candidates.len(), 2);
        let late = candidates.pop().unwrap();
        let Step::Next(at_finish) =
            step(late, &program, &model, &mut Room(DEFAULT_STATE_BUDGET)).unwrap()
        else {
            panic!("late ack must not cut before the other task finishes");
        };
        assert_eq!(at_finish.len(), 1);
        assert_eq!(at_finish[0].time, 4);
        assert_eq!(at_finish[0].work, 8);
        let Step::Next(finished) = step(
            at_finish[0].clone(),
            &program,
            &model,
            &mut Room(DEFAULT_STATE_BUDGET),
        )
        .unwrap() else {
            panic!("clock-equal finish must be processed");
        };
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].results, vec![Some(V(0)), None]);
        assert_eq!(finished[0].tasks[0].lifecycle, Lifecycle::Done);
        assert_eq!(finished[0].occupancy(&model), vec![1]);
        assert!(matches!(
            step(
                finished[0].clone(),
                &program,
                &model,
                &mut Room(DEFAULT_STATE_BUDGET)
            )
            .unwrap(),
            Step::Cut
        ));
    }

    #[test]
    fn acknowledgement_candidates_include_interior_delays_and_only_uncut_metrics() {
        let mut model = with_safety(cancel_model(true));
        model.capabilities[1].duration = 10;
        model.capabilities[1].cancel = ir::Cancel::Cooperative { ack_within: 3 };
        model.bounds.clock = 6;
        let mut state = State::initial(&model, 0);
        state.time = 3;
        state.tasks = vec![Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: B(1),
            capability: C(1),
            lifecycle: Lifecycle::Running {
                finish: 10,
                ack: None,
            },
        }];
        let candidates =
            request_cancel(vec![state], 0, &model, &mut Room(DEFAULT_STATE_BUDGET)).unwrap();
        assert_eq!(
            candidates
                .iter()
                .map(|s| s.tasks[0].lifecycle)
                .collect::<Vec<_>>(),
            [4, 5, 6]
                .into_iter()
                .map(|ack| Lifecycle::Running {
                    finish: 10,
                    ack: Some(ack)
                })
                .collect::<Vec<_>>()
        );
        model.bounds.clock = 5; // ack 4 and 5 finish, ack 6 is cut
        let out = explore(&model);
        assert!(out.truncated);
        assert_eq!(out.properties[0].verdict, Verdict::Inconclusive);
        assert_eq!(out.properties[1].verdict, Verdict::Satisfied);
        assert_eq!(out.terminals[0].elapsed, outcome::Range { min: 4, max: 5 });
        assert_eq!(out.terminals[0].work, outcome::Range { min: 7, max: 8 });
    }

    #[test]
    fn any_ignores_earlier_nonmatching_result_and_accepts_later_match() {
        let mut model = base(2, Some(any(ir::Remaining::Drain)));
        model.types[0].members.push("fail".into());
        model.capabilities[0].results = vec![V(1)];
        let out = explore(&model);
        assert_eq!(out.terminals.len(), 1);
        assert_eq!(out.terminals[0].end, E(0));
        assert_eq!(out.terminals[0].elapsed, fixed(5));
        assert_eq!(out.terminals[0].work, fixed(8));
        assert_eq!(out.terminals[0].result_at, Some(fixed(5)));
    }

    #[test]
    fn separate_actors_do_not_contend_and_names_carry_no_scheduling_meaning() {
        let mut model = with_safety(base(1, Some(ir::Join::All)));
        model.actors.push(ir::Actor {
            metadata: ir::ActorMetadata::default(),
            executors: None,
            source: None,
            name: "Other".into(),
            capacity: Some(1),
            span: SPAN,
        });
        model.capabilities[1].actor = ir::ActorId(1);
        model.bounds.clock = 5;
        let out = explore(&model);
        assert_eq!(out.terminals[0].elapsed, fixed(5));
        assert_eq!(out.terminals[0].work, fixed(8));
        assert_eq!(out.properties[0].verdict, Verdict::Satisfied);
        model.actors[0].name = "party".into();
        model.actors[1].name = "human".into();
        model.process.name = "renamed".into();
        assert_eq!(out, explore(&model));
    }

    #[test]
    fn natural_completion_can_beat_ack_and_equal_time_has_both_outcomes() {
        let mut model = cancel_model(true);
        model.capabilities[1].cancel = ir::Cancel::Cooperative { ack_within: 3 };
        model.bounds.clock = 5;
        let out = explore(&model);
        assert_eq!(out.terminals[0].elapsed, outcome::Range { min: 4, max: 5 });
        assert_eq!(out.terminals[0].work, outcome::Range { min: 7, max: 8 });

        // Both tasks race normal finish (two enum values) against ack. There
        // are 3 x 3 distinguishable published-result vectors, not just 3 or 4.
        model.types[0].members.push("fail".into());
        for cap in &mut model.capabilities {
            cap.results.push(V(1));
        }
        let mut state = State::initial(&model, 0);
        state.time = 5;
        state.tasks = (0..2)
            .map(|i| Task {
                frozen_performance: None,
                stale: Vec::new(),
                executor: None,
                binding: B(i),
                capability: C(i),
                lifecycle: Lifecycle::Running {
                    finish: 5,
                    ack: Some(5),
                },
            })
            .collect();
        let due = |i| Due {
            i,
            finish: 5,
            ack: Some(5),
        };
        let done: Vec<_> = complete(&state, due(0), &model, &mut Room(DEFAULT_STATE_BUDGET))
            .unwrap()
            .iter()
            .flat_map(|partial| {
                complete(partial, due(1), &model, &mut Room(DEFAULT_STATE_BUDGET)).unwrap()
            })
            .collect();
        assert_eq!(done.len(), 9);
        assert_eq!(
            done.iter()
                .map(|s| s.results.clone())
                .collect::<HashSet<_>>()
                .len(),
            9
        );
        assert!(done.iter().all(|s| s.occupancy(&model) == vec![0]));

        // Exercise the production step -> complete edge, including both atomic
        // completion orders. The helper's 9 vectors alone cannot guard this edge.
        let mut program = vec![Instruction::Stop];
        state.pc = compile(&model.process.body, 0, &mut program);
        state.waiting = true;
        let mut pending = vec![state];
        let mut through_step = HashSet::new();
        while let Some(state) = pending.pop() {
            if state
                .tasks
                .iter()
                .all(|task| task.lifecycle == Lifecycle::Done)
            {
                assert_eq!(state.time, 5);
                assert_eq!(state.occupancy(&model), vec![0]);
                through_step.insert(state.results);
            } else {
                let Step::Next(states) =
                    step(state, &program, &model, &mut Room(DEFAULT_STATE_BUDGET)).unwrap()
                else {
                    panic!("both due finish/ack outcomes must remain enabled");
                };
                pending.extend(states);
            }
        }
        let values = [None, Some(V(0)), Some(V(1))];
        let expected: HashSet<_> = values
            .into_iter()
            .flat_map(|a| values.into_iter().map(move |b| vec![a, b]))
            .collect();
        assert_eq!(through_step, expected);
    }

    #[test]
    fn ties_use_the_complete_simultaneous_cohort_not_finish_iteration_order() {
        let mut model = cancel_model(false);
        model.capabilities[1].duration = 3;
        let mut program = vec![Instruction::Stop];
        let pc = compile(&model.process.body, 0, &mut program);
        let mut state = State::initial(&model, pc);
        state.time = 3;
        state.waiting = true;
        state.tasks = (0..2)
            .map(|i| Task {
                frozen_performance: None,
                stale: Vec::new(),
                executor: None,
                binding: B(i),
                capability: C(i),
                lifecycle: Lifecycle::Running {
                    finish: 3,
                    ack: None,
                },
            })
            .collect();
        let Step::Next(partial) =
            step(state, &program, &model, &mut Room(DEFAULT_STATE_BUDGET)).unwrap()
        else {
            panic!()
        };
        assert_eq!(partial.len(), 2); // both atomic finish orders
        assert!(partial.iter().all(|s| s.adopted.is_none()));
        let Step::Next(finished) = step(
            partial[0].clone(),
            &program,
            &model,
            &mut Room(DEFAULT_STATE_BUDGET),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].results, vec![Some(V(0)), Some(V(0))]);
        let Instruction::Wait {
            join: ir::Join::Any { tie, .. },
            ..
        } = &mut program[pc]
        else {
            panic!()
        };
        *tie = ir::Tie::By(vec![B(1), B(0)]);
        let Step::Next(adopted) = step(
            finished[0].clone(),
            &program,
            &model,
            &mut Room(DEFAULT_STATE_BUDGET),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(adopted.len(), 1);
        assert_eq!(adopted[0].adopted, Some(B(1)));
        let Instruction::Wait {
            join: ir::Join::Any { tie, .. },
            ..
        } = &mut program[pc]
        else {
            panic!()
        };
        *tie = ir::Tie::Nondet;
        let Step::Next(adopted) = step(
            finished[0].clone(),
            &program,
            &model,
            &mut Room(DEFAULT_STATE_BUDGET),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(
            adopted
                .iter()
                .map(|s| s.adopted.unwrap())
                .collect::<HashSet<_>>(),
            HashSet::from([B(0), B(1)])
        );
    }

    #[test]
    fn starts_enabled_between_simultaneous_finishes_are_not_silently_collapsed() {
        let mut model = cancel_model(true);
        model.actors[0].capacity = Some(1);
        model.actors.push(ir::Actor {
            metadata: ir::ActorMetadata::default(),
            executors: None,
            source: None,
            name: "Other".into(),
            capacity: Some(1),
            span: SPAN,
        });
        model.capabilities[1].duration = 3;
        model.capabilities[1].actor = ir::ActorId(1);
        let mut third = model.capabilities[0].clone();
        third.name = "third".into();
        third.duration = 10;
        third.cancel = ir::Cancel::Cooperative { ack_within: 1 };
        model.capabilities.push(third);
        model.process.bindings.push(ir::Binding {
            merge: None,
            repetition: None,
            name: "c".into(),
            ty: ir::TypeId(0),
            span: SPAN,
        });
        let ir::Stmt::Fork { branches, join, .. } = &mut model.process.body[0].kind else {
            panic!()
        };
        branches.push(call(2, 2));
        let ir::Join::Any { matches, tie, .. } = join else {
            panic!()
        };
        matches.push(ir::ResultMatch {
            binding: B(2),
            value: V(0),
        });
        *tie = ir::Tie::By(vec![B(0), B(1), B(2)]);
        model.bounds.live_tasks = 3;
        let mut program = vec![Instruction::Stop];
        let entry = compile(&model.process.body, 0, &mut program);
        let mut state = State::initial(&model, entry);
        state.waiting = true;
        state.time = 3;
        state.work = 6;
        state.tasks = (0..3)
            .map(|i| Task {
                frozen_performance: None,
                stale: Vec::new(),
                executor: None,
                binding: B(i),
                capability: C(i),
                lifecycle: if i == 2 {
                    Lifecycle::Queued
                } else {
                    Lifecycle::Running {
                        finish: 3,
                        ack: None,
                    }
                },
            })
            .collect();
        let mut pending = vec![state];
        let mut finished = Exploration::default();
        while let Some(state) = pending.pop() {
            match step(
                state.clone(),
                &program,
                &model,
                &mut Room(DEFAULT_STATE_BUDGET),
            )
            .unwrap()
            {
                Step::Next(states) => pending.extend(states),
                Step::Terminal(end) => finished.terminal(&state, end).unwrap(),
                _ => panic!("unexpected cut or deadlock"),
            }
        }
        let terminal = &finished.terminals[&E(0)];
        // A finish -> C start -> B finish -> cancel C gives elapsed 4/work 7.
        // A/B finish -> cancel queued C gives elapsed 3/work 6.
        assert_eq!(terminal.elapsed, outcome::Range { min: 3, max: 4 });
        assert_eq!(terminal.work, outcome::Range { min: 6, max: 7 });
        assert_eq!(terminal.result_at, Some(fixed(3)));
    }

    #[test]
    fn no_match_jumps_to_if_none_only_after_all_branches_finish() {
        let mut model = base(2, Some(any(ir::Remaining::Drain)));
        model.types[0].members.push("fail".into());
        for cap in &mut model.capabilities {
            cap.results = vec![V(1)];
        }
        let out = explore(&model);
        assert_eq!(out.terminals.len(), 1);
        assert_eq!(out.terminals[0].end, E(1));
        assert_eq!(out.terminals[0].elapsed, fixed(5));
        assert_eq!(out.terminals[0].work, fixed(8));
        assert_eq!(out.terminals[0].result_at, None);
        assert_eq!(out.properties[0].verdict, Verdict::Violated);
    }

    #[test]
    fn live_tasks_counts_queued_tasks_and_cuts_before_dispatch() {
        let mut model = with_safety(base(0, Some(ir::Join::All)));
        model.bounds.live_tasks = 1;
        let out = explore(&model);
        assert!(out.truncated);
        assert!(out.terminals.is_empty());
        assert!(out.diagnostics.is_empty());
        assert!(
            out.properties
                .iter()
                .all(|p| p.verdict == Verdict::Inconclusive)
        );
        let mut serial = base(1, None);
        serial.bounds.live_tasks = 1;
        assert!(!explore(&serial).truncated); // ended tasks do not count
    }

    #[test]
    fn witness_survives_other_cut_paths_but_absence_with_cut_is_inconclusive() {
        let mut model = base(1, None);
        model.bounds.clock = 3;
        model.types[0].members.push("fail".into());
        model.capabilities[0].results.push(V(1));
        model.process.body.insert(
            1,
            stmt(ir::Stmt::If {
                condition: eq(0, 0),
                then_body: vec![stmt(ir::Stmt::End(E(0)))],
                else_body: vec![],
            }),
        );
        model.verify.push(ir::Verify {
            property: ir::Property::ReachableEnd(E(1)),
            span: SPAN,
        });
        model = with_safety(model);
        let out = explore(&model);
        assert!(out.truncated);
        assert_eq!(
            out.properties.iter().map(|p| p.verdict).collect::<Vec<_>>(),
            vec![
                Verdict::Inconclusive,
                Verdict::Satisfied,
                Verdict::Inconclusive
            ]
        );
        assert_eq!(out.terminals.len(), 1);
        assert_eq!(out.terminals[0].elapsed, fixed(3));
        assert_eq!(out.terminals[0].work, fixed(3));
    }

    #[test]
    fn merging_keeps_different_work_at_identical_control_time_and_results() {
        let mut model = cancel_model(true);
        model.capabilities[1].duration = 10;
        model.capabilities[1].cancel = ir::Cancel::Cooperative { ack_within: 3 };
        model.actors[0].capacity = Some(3);
        model.bounds = ir::Bounds {
            cases: 1,
            live_tasks: 3,
            clock: 6,
        };
        model.process.bindings.push(ir::Binding {
            merge: None,
            repetition: None,
            name: "c".into(),
            ty: ir::TypeId(0),
            span: SPAN,
        });
        let ir::Stmt::Fork { branches, join, .. } = &mut model.process.body[0].kind else {
            panic!()
        };
        branches.push(call(2, 1));
        let ir::Join::Any { matches, tie, .. } = join else {
            panic!()
        };
        matches.push(ir::ResultMatch {
            binding: B(2),
            value: V(0),
        });
        *tie = ir::Tie::By(vec![B(0), B(1), B(2)]);
        let out = explore(&model);
        assert_eq!(out, run(&model, false, DEFAULT_STATE_BUDGET));
        assert_eq!(out.terminals[0].elapsed, outcome::Range { min: 4, max: 6 });
        assert_eq!(out.terminals[0].work, outcome::Range { min: 11, max: 15 });
        // Explicitly check the same-time extrema, not only the global extrema.
        let mut a = State::initial(&model, 0);
        a.time = 6;
        a.work = 13;
        a.result_at = Some(3);
        let mut b = a.clone();
        b.work = 15;
        assert_ne!(a, b);
        let mut exploration = Exploration::default();
        exploration.terminal(&a, E(0)).unwrap();
        exploration.terminal(&b, E(0)).unwrap();
        assert_eq!(
            exploration.terminals[&E(0)].work,
            outcome::Range { min: 13, max: 15 }
        );
    }

    #[test]
    fn overcommit_observer_rejects_broken_acquisition_even_with_a_cut_elsewhere() {
        let model = with_safety(base(1, Some(ir::Join::All)));
        let mut state = State::initial(&model, 0);
        // Fault injection: simulate an acquisition that skipped the atomic
        // capacity guard and started both tasks against one slot.
        state.tasks = (0..2)
            .map(|i| Task {
                frozen_performance: None,
                stale: Vec::new(),
                executor: None,
                binding: B(i),
                capability: C(i),
                lifecycle: Lifecycle::Running {
                    finish: 5,
                    ack: None,
                },
            })
            .collect();
        let mut exploration = Exploration {
            truncated: true,
            ..Exploration::default()
        };
        exploration.observe(&state, &model).unwrap();
        let out = exploration.outcome(&model);
        assert_eq!(out.properties[0].verdict, Verdict::Violated);
        assert_eq!(out.properties[1].verdict, Verdict::Inconclusive);

        let mut model = model;
        model.bounds.clock = 4; // injected tasks finish beyond clock
        let out = run_initialized(&model, true, DEFAULT_STATE_BUDGET, |mut initial| {
            initial.waiting = true;
            initial.tasks = state.tasks;
            initial
        });
        assert!(out.truncated);
        assert!(out.terminals.is_empty());
        assert!(out.diagnostics.is_empty());
        assert_eq!(out.properties[0].verdict, Verdict::Violated);
        assert_eq!(out.properties[1].verdict, Verdict::Inconclusive);
    }

    #[test]
    fn zero_duration_tasks_and_clock_zero_finish_without_time_or_work() {
        let mut model = with_safety(base(1, Some(ir::Join::All)));
        for cap in &mut model.capabilities {
            cap.duration = 0;
        }
        model.bounds.clock = 0;
        let out = explore(&model);
        assert!(!out.truncated);
        assert!(out.diagnostics.is_empty());
        assert_eq!(out.terminals[0].elapsed, fixed(0));
        assert_eq!(out.terminals[0].work, fixed(0));
        assert!(
            out.properties
                .iter()
                .all(|p| p.verdict == Verdict::Satisfied)
        );
    }

    #[test]
    fn nested_conditions_fall_through_and_end_skips_unreachable_suffix() {
        let mut model = base(1, None);
        model.process.body.insert(
            1,
            stmt(ir::Stmt::If {
                condition: ir::Cond::And(vec![eq(0, 0), ir::Cond::Or(vec![eq(0, 0), eq(0, 0)])]),
                then_body: vec![stmt(ir::Stmt::If {
                    condition: eq(0, 0),
                    then_body: vec![],
                    else_body: vec![],
                })],
                else_body: vec![stmt(ir::Stmt::End(E(1)))],
            }),
        );
        let skipped = model.process.bindings[1].clone();
        model.process.bindings.push(skipped);
        model.process.body.push(stmt(ir::Stmt::Call(call(2, 1)))); // skipped after end
        assert_eq!(explore(&model).terminals[0].elapsed, fixed(8));
        model.process.body = vec![];
        let out = explore(&model);
        assert_eq!(out.diagnostics[0].code, diag::W_DEADLOCK_0);
        assert_eq!(out.properties[0].verdict, Verdict::Violated);
    }

    #[test]
    fn and_or_truth_tables_and_nested_false_operands_through_explore() {
        for (a, b, and, or) in [
            (false, false, false, false),
            (false, true, false, true),
            (true, false, false, true),
            (true, true, true, true),
        ] {
            let mut model = base(2, Some(ir::Join::All));
            model.types[0].members.push("fail".into());
            let a_value = usize::from(!a);
            model.capabilities[0].results = vec![V(a_value)];
            model.capabilities[1].results = vec![V(usize::from(!b))];
            let always_false = eq(0, 1 - a_value);
            let always_true = eq(0, a_value);
            for (cond, expected) in [
                (ir::Cond::And(vec![eq(0, 0), eq(1, 0)]), and),
                (ir::Cond::Or(vec![eq(0, 0), eq(1, 0)]), or),
                (
                    ir::Cond::And(vec![
                        eq(0, 0),
                        ir::Cond::Or(vec![eq(1, 0), always_false.clone()]),
                    ]),
                    and,
                ),
                (
                    ir::Cond::Or(vec![
                        eq(0, 0),
                        ir::Cond::And(vec![eq(1, 0), always_false.clone()]),
                    ]),
                    a,
                ),
                (
                    ir::Cond::And(vec![ir::Cond::Or(vec![eq(0, 0), eq(1, 0)]), always_false]),
                    false,
                ),
                (
                    ir::Cond::Or(vec![ir::Cond::And(vec![eq(0, 0), eq(1, 0)]), always_true]),
                    true,
                ),
            ] {
                model.process.body[1] = choose(cond.clone(), 0, 1);
                let out = explore(&model);
                assert!(!out.truncated);
                unreachable(
                    &out,
                    &[
                        (
                            diag::W_UNREACH_1,
                            if expected {
                                "if else arm is never taken"
                            } else {
                                "if then arm is never taken"
                            },
                        ),
                        (
                            diag::W_UNREACH_2,
                            if expected {
                                "end.no_match is never reached"
                            } else {
                                "end.done is never reached"
                            },
                        ),
                    ],
                );
                assert_eq!(out.terminals.len(), 1, "{a}/{b}: {cond:?}");
                assert_eq!(
                    out.terminals[0].end,
                    E(usize::from(!expected)),
                    "{a}/{b}: {cond:?}"
                );
                assert_eq!(out.terminals[0].elapsed, fixed(5));
                assert_eq!(out.terminals[0].work, fixed(8));
                assert_eq!(
                    out.properties[0].verdict,
                    if expected {
                        Verdict::Satisfied
                    } else {
                        Verdict::Violated
                    }
                );
            }
        }
    }

    fn unsupported(model: &ir::Model) {
        let out = explore(model);
        assert!(out.terminals.is_empty());
        assert_eq!(out.diagnostics.len(), 1);
        assert_eq!(out.diagnostics[0].code, diag::U_SEM_0);
        assert!(
            out.properties
                .iter()
                .all(|p| p.verdict == Verdict::Unsupported)
        );
    }

    #[test]
    fn finish_and_work_arithmetic_overflow_fail_closed() {
        let mut model = base(1, None);
        model.bounds.clock = u64::MAX;
        model.capabilities[0].duration = u64::MAX;
        unsupported(&model); // second finish would exceed u64
        let mut model = base(2, Some(ir::Join::All));
        model.bounds.clock = u64::MAX;
        for cap in &mut model.capabilities {
            cap.duration = u64::MAX;
        }
        unsupported(&model); // elapsed fits, occupied-slot time does not
    }

    #[test]
    fn ack_overflow_and_undefined_capacity_or_ack_zero_fail_closed() {
        let mut model = cancel_model(true);
        model.bounds.clock = u64::MAX;
        model.capabilities[0].duration = u64::MAX - 1;
        model.capabilities[1].duration = u64::MAX;
        model.capabilities[1].cancel = ir::Cancel::Cooperative { ack_within: 2 };
        unsupported(&model);
        let mut model = base(1, None);
        model.actors[0].capacity = None;
        for cap in &mut model.capabilities {
            cap.duration = 0;
        }
        unsupported(&model);
        let mut model = cancel_model(true);
        model.capabilities[1].cancel = ir::Cancel::Cooperative { ack_within: 0 };
        unsupported(&model);
    }

    #[test]
    fn unavailable_cancelled_results_are_not_false_or_success() {
        let mut model = cancel_model(true);
        model.process.body[1] = choose(eq(1, 0), 0, 1);
        unsupported(&model);
    }

    #[test]
    fn unresolved_multiple_or_mixed_adoption_metrics_fail_closed() {
        let mut model = base(2, Some(any(ir::Remaining::Drain)));
        model
            .process
            .bindings
            .extend(model.process.bindings.clone());
        model.process.forks.push("second".into());
        let mut second = fork(any(ir::Remaining::Drain));
        let ir::Stmt::Fork { id, branches, join } = &mut second.kind else {
            panic!()
        };
        *id = ir::ForkId(1);
        for branch in branches {
            branch.binding.0 += 2;
        }
        let ir::Join::Any { matches, tie, .. } = join else {
            panic!()
        };
        for test in matches {
            test.binding.0 += 2;
        }
        *tie = ir::Tie::By(vec![B(2), B(3)]);
        model.process.body.insert(1, second);
        model.bounds.clock = 10;
        unsupported(&model);

        let model = base(1, None);
        let a = State::initial(&model, 0);
        let mut b = a.clone();
        b.result_at = Some(0);
        let mut exploration = Exploration::default();
        exploration.terminal(&a, E(0)).unwrap();
        assert!(exploration.terminal(&b, E(0)).is_err());
    }

    #[test]
    fn unconfigured_exploration_does_not_claim_empty_success() {
        let mut model = base(1, None);
        model.verify.clear();
        unsupported(&model);
        model.verify.push(ir::Verify {
            property: ir::Property::ReachableEnd(E(0)),
            span: SPAN,
        });
        model.bounds.cases = 2;
        unsupported(&model);
    }

    type Mutation = fn(&mut ir::Model);

    #[test]
    fn malformed_hand_built_ir_is_unsupported_not_a_panic() {
        let mutations: Vec<(&str, Mutation)> = vec![
            ("actor", |m| m.capabilities[0].actor = ir::ActorId(9)),
            ("cap type", |m| {
                m.capabilities[0].return_type = ir::TypeId(9)
            }),
            ("cap result", |m| m.capabilities[0].results = vec![V(9)]),
            ("binding type", |m| m.process.bindings[0].ty = ir::TypeId(9)),
            ("call cap", |m| {
                m.process.body[0] = stmt(ir::Stmt::Call(call(0, 9)))
            }),
            ("call binding", |m| {
                m.process.body[0] = stmt(ir::Stmt::Call(call(9, 0)))
            }),
            ("rebinding", |m| {
                m.process.body[1] = stmt(ir::Stmt::Call(call(0, 1)))
            }),
            ("end", |m| m.process.body[2] = stmt(ir::Stmt::End(E(9)))),
            ("verify end", |m| {
                m.verify[0].property = ir::Property::ReachableEnd(E(9))
            }),
            ("cond binding", |m| {
                m.process.body[2] = choose(eq(9, 0), 0, 1)
            }),
            ("cond value", |m| m.process.body[2] = choose(eq(0, 9), 0, 1)),
            ("unary and", |m| {
                m.process.body[2] = choose(ir::Cond::And(vec![eq(0, 0)]), 0, 1)
            }),
            ("uncalled binding type", |m| {
                let mut extra = m.process.bindings[0].clone();
                extra.ty = ir::TypeId(9);
                m.process.bindings.push(extra);
            }),
            ("call type mismatch in range", |m| {
                m.types.push(m.types[0].clone());
                m.process.bindings[0].ty = ir::TypeId(1);
            }),
            ("nested if end", |m| {
                m.process.body[2] = stmt(ir::Stmt::If {
                    condition: eq(0, 0),
                    then_body: vec![],
                    else_body: vec![stmt(ir::Stmt::End(E(9)))],
                })
            }),
        ];
        let any_mutations: Vec<(&str, Mutation)> = vec![
            ("fork id", |m| {
                if let ir::Stmt::Fork { id, .. } = &mut m.process.body[0].kind {
                    *id = ir::ForkId(9);
                }
            }),
            ("if_none", |m| {
                if let ir::Stmt::Fork {
                    join: ir::Join::Any { if_none, .. },
                    ..
                } = &mut m.process.body[0].kind
                {
                    *if_none = E(9);
                }
            }),
            ("match value", |m| {
                if let ir::Stmt::Fork {
                    join: ir::Join::Any { matches, .. },
                    ..
                } = &mut m.process.body[0].kind
                {
                    matches[0].value = V(9);
                }
            }),
            ("fork without branches", |m| {
                if let ir::Stmt::Fork { branches, join, .. } = &mut m.process.body[0].kind {
                    branches.clear();
                    *join = ir::Join::All;
                }
            }),
            ("match outside fork", |m| {
                m.process.bindings.push(m.process.bindings[0].clone());
                if let ir::Stmt::Fork {
                    join: ir::Join::Any { matches, .. },
                    ..
                } = &mut m.process.body[0].kind
                {
                    matches[0].binding = B(2);
                }
            }),
            ("tie outside fork", |m| {
                m.process.bindings.push(m.process.bindings[0].clone());
                if let ir::Stmt::Fork {
                    join: ir::Join::Any { tie, .. },
                    ..
                } = &mut m.process.body[0].kind
                {
                    *tie = ir::Tie::By(vec![B(2), B(0), B(1)]);
                }
            }),
        ];
        // Each mutation alone must be rejected; the unmutated models are valid.
        assert_eq!(
            explore(&base(1, None)).properties[0].verdict,
            Verdict::Satisfied
        );
        let any_model = base(2, Some(any(ir::Remaining::Drain)));
        assert_eq!(
            explore(&any_model).properties[0].verdict,
            Verdict::Satisfied
        );
        for (name, mutate) in mutations {
            let mut model = base(1, None);
            mutate(&mut model);
            let out = explore(&model);
            assert_eq!(out.diagnostics.len(), 1, "{name}");
            assert_eq!(out.diagnostics[0].code, diag::U_SEM_0, "{name}");
        }
        for (name, mutate) in any_mutations {
            let mut model = any_model.clone();
            mutate(&mut model);
            let out = explore(&model);
            assert_eq!(out.diagnostics.len(), 1, "{name}");
            assert_eq!(out.diagnostics[0].code, diag::U_SEM_0, "{name}");
        }
    }

    #[test]
    fn state_budget_is_a_cut_with_its_own_warning_and_keeps_found_witnesses() {
        let model = with_safety(base(2, Some(any(ir::Remaining::Drain))));
        let full = explore(&model);
        assert!(!full.truncated);
        unreachable(
            &full,
            &[(diag::W_UNREACH_2, "end.no_match is never reached")],
        );
        let budget_codes = |out: &outcome::Outcome| {
            out.diagnostics
                .iter()
                .filter(|d| d.code == diag::W_BUDGET_0)
                .count()
        };
        let verdicts =
            |out: &outcome::Outcome| out.properties.iter().map(|p| p.verdict).collect::<Vec<_>>();
        // Too small to reach any terminal: neither property may be decided.
        let starved = explore_with_budget(&model, 2);
        assert!(starved.truncated);
        assert!(starved.terminals.is_empty());
        assert_eq!(budget_codes(&starved), 1);
        assert_eq!(starved.diagnostics[0].severity, diag::Severity::Warning);
        assert_eq!(
            verdicts(&starved),
            vec![Verdict::Inconclusive, Verdict::Inconclusive]
        );
        // Some budget reaches the witness but not the whole space: reachability
        // stays satisfied, the universal safety property stays inconclusive.
        let partial = (3..100)
            .map(|budget| explore_with_budget(&model, budget))
            .find(|out| !out.terminals.is_empty() && out.truncated)
            .expect("some budget cuts after the first witness");
        assert_eq!(budget_codes(&partial), 1);
        assert_eq!(
            verdicts(&partial),
            vec![Verdict::Inconclusive, Verdict::Satisfied]
        );
        // A violation found before the budget is still a violation.
        let mut broken = with_safety(base(1, Some(ir::Join::All)));
        let out = run_initialized(&broken, true, 2, |mut initial| {
            initial.waiting = true;
            initial.tasks = (0..2)
                .map(|i| Task {
                    frozen_performance: None,
                    stale: Vec::new(),
                    executor: None,
                    binding: B(i),
                    capability: C(i),
                    lifecycle: Lifecycle::Running {
                        finish: 3 + 2 * i as u64,
                        ack: None,
                    },
                })
                .collect();
            initial
        });
        assert!(out.truncated);
        assert_eq!(out.properties[0].verdict, Verdict::Violated);
        broken.bounds.clock = 8;
        assert_eq!(budget_codes(&explore(&broken)), 0);
    }

    #[test]
    fn three_operand_and_or_fold_every_operand() {
        let mut model = base(1, None);
        model.types[0].members.push("fail".into());
        let t = || eq(0, 0);
        let f = || eq(0, 1);
        let mut cases = vec![
            (ir::Cond::And(vec![t(), t(), t()]), true),
            (ir::Cond::Or(vec![f(), f(), f()]), false),
        ];
        for odd in 0..3 {
            let mut and = vec![t(), t(), t()];
            and[odd] = f();
            let mut or = vec![f(), f(), f()];
            or[odd] = t();
            cases.push((ir::Cond::And(and), false));
            cases.push((ir::Cond::Or(or), true));
        }
        for (cond, expected) in cases {
            model.process.body[2] = choose(cond.clone(), 0, 1);
            let out = explore(&model);
            unreachable(
                &out,
                &[
                    (
                        diag::W_UNREACH_1,
                        if expected {
                            "if else arm is never taken"
                        } else {
                            "if then arm is never taken"
                        },
                    ),
                    (
                        diag::W_UNREACH_2,
                        if expected {
                            "end.no_match is never reached"
                        } else {
                            "end.done is never reached"
                        },
                    ),
                ],
            );
            assert_eq!(out.terminals.len(), 1, "{cond:?}");
            assert_eq!(out.terminals[0].end, E(usize::from(!expected)), "{cond:?}");
        }
    }

    #[test]
    fn state_budget_counts_held_states_and_exceeding_it_is_the_cut() {
        let mut model = base(1, None);
        model.process.body = vec![stmt(ir::Stmt::End(E(0)))];
        // One state held, budget one: not exceeded, the end is reached.
        let one = explore_with_budget(&model, 1);
        assert!(!one.truncated);
        unreachable(
            &one,
            &[
                (diag::W_UNREACH_0, "Office.short not_called"),
                (diag::W_UNREACH_0, "Office.long not_called"),
            ],
        );
        assert_eq!(one.properties[0].verdict, Verdict::Satisfied);
        // Budget zero: holding the initial state already exceeds it.
        let zero = explore_with_budget(&model, 0);
        assert!(zero.truncated);
        assert_eq!(zero.diagnostics[0].code, diag::W_BUDGET_0);
        assert_eq!(zero.properties[0].verdict, Verdict::Inconclusive);
    }

    #[test]
    fn ack_candidates_beyond_the_room_are_refused_before_cloning() {
        let mut model = cancel_model(true);
        model.bounds.clock = 100;
        model.capabilities[1].duration = 100;
        model.capabilities[1].cancel = ir::Cancel::Cooperative { ack_within: 100 };
        let mut state = State::initial(&model, 0);
        state.time = 3;
        state.tasks = vec![Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: B(1),
            capability: C(1),
            lifecycle: Lifecycle::Running {
                finish: 100,
                ack: None,
            },
        }];
        // 98 candidates: acks 4..=100 plus the one late representative 101
        // (clock + 1). They replace the input state (counted by the caller),
        // so 97 more must fit: exactly fitting is allowed, one less refused.
        assert_eq!(
            request_cancel(vec![state.clone()], 0, &model, &mut Room(97))
                .unwrap()
                .len(),
            98
        );
        assert_eq!(
            request_cancel(vec![state], 0, &model, &mut Room(96)).unwrap_err(),
            Halt::Budget
        );
        // A trillion-minute window end to end: a budget cut, not an allocation.
        let mut huge = cancel_model(true);
        huge.bounds.clock = 1_000_000_000_000;
        huge.capabilities[1].duration = 1_000_000_000_000;
        huge.capabilities[1].cancel = ir::Cancel::Cooperative {
            ack_within: 1_000_000_000_000,
        };
        let out = explore(&huge);
        assert!(out.truncated);
        assert_eq!(out.diagnostics[0].code, diag::W_BUDGET_0);
    }

    #[test]
    fn abort_keeps_found_counterexamples_and_witnesses() {
        let model = with_safety(base(1, None));
        let mut exploration = Exploration {
            overcommit: true,
            deadlock: true,
            ..Exploration::default()
        };
        exploration
            .terminal(&State::initial(&model, 0), E(0))
            .unwrap();
        let out = exploration.abort("overflow", &model);
        assert_eq!(
            out.properties.iter().map(|p| p.verdict).collect::<Vec<_>>(),
            vec![Verdict::Violated, Verdict::Satisfied]
        );
        let codes: Vec<_> = out.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, vec![diag::U_SEM_0, diag::W_DEADLOCK_0]);
        assert!(out.terminals.is_empty());
        let nothing = Exploration::default().abort("overflow", &model);
        assert!(
            nothing
                .properties
                .iter()
                .all(|p| p.verdict == Verdict::Unsupported)
        );
        assert_eq!(nothing.diagnostics.len(), 1);
    }

    // Review N1: each generation path is refused before it exceeds the room,
    // and exactly fitting is allowed. Counts are final successor states.
    #[test]
    fn every_successor_producer_is_bounded_by_the_room() {
        let at_wait = |model: &ir::Model, time: u64, tasks: Vec<Task>| {
            let mut program = vec![Instruction::Stop];
            let mut state = State::initial(model, 0);
            state.pc = compile(&model.process.body, 0, &mut program);
            state.waiting = true;
            state.time = time;
            state.tasks = tasks;
            (program, state)
        };
        let task = |i: usize, lifecycle| Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: B(i),
            capability: C(i),
            lifecycle,
        };
        let fits = |model: &ir::Model, program: &[Instruction], state: &State, n: usize| {
            let Step::Next(states) = step(state.clone(), program, model, &mut Room(n)).unwrap()
            else {
                panic!("expected successors")
            };
            assert_eq!(states.len(), n);
            assert_eq!(
                step(state.clone(), program, model, &mut Room(n - 1)).err(),
                Some(Halt::Budget)
            );
        };
        // Completion with two result values: 2 successors.
        let mut model = base(2, Some(ir::Join::All));
        model.types[0].members.push("fail".into());
        model.capabilities[0].results.push(V(1));
        let (program, state) = at_wait(
            &model,
            3,
            vec![
                task(
                    0,
                    Lifecycle::Running {
                        finish: 3,
                        ack: None,
                    },
                ),
                task(1, Lifecycle::Done),
            ],
        );
        fits(&model, &program, &state, 2);
        // Two queued tasks competing for capacity 1: 2 start successors.
        let model = base(1, Some(ir::Join::All));
        let (program, state) = at_wait(
            &model,
            0,
            vec![task(0, Lifecycle::Queued), task(1, Lifecycle::Queued)],
        );
        fits(&model, &program, &state, 2);
        // Two tied winners, each cancelling c with acks 4 and 5: 2 x 2 = 4.
        let mut model = base(3, Some(any(ir::Remaining::CancelThenDrain)));
        model.bounds.clock = 20;
        let mut long = model.capabilities[1].clone();
        long.duration = 10;
        long.cancel = ir::Cancel::Cooperative { ack_within: 2 };
        model.capabilities.push(long);
        model
            .process
            .bindings
            .push(model.process.bindings[0].clone());
        if let ir::Stmt::Fork { branches, join, .. } = &mut model.process.body[0].kind {
            branches.push(call(2, 2));
            if let ir::Join::Any { matches, tie, .. } = join {
                matches.push(ir::ResultMatch {
                    binding: B(2),
                    value: V(0),
                });
                *tie = ir::Tie::Nondet;
            }
        }
        let (program, mut state) = at_wait(
            &model,
            3,
            vec![
                task(0, Lifecycle::Done),
                task(1, Lifecycle::Done),
                task(
                    2,
                    Lifecycle::Running {
                        finish: 10,
                        ack: None,
                    },
                ),
            ],
        );
        state.results[0] = Some(V(0));
        state.results[1] = Some(V(0));
        fits(&model, &program, &state, 4);
        // A single successor (time advance) needs room for one.
        let model = base(1, None);
        let (program, state) = at_wait(
            &model,
            0,
            vec![task(
                0,
                Lifecycle::Running {
                    finish: 3,
                    ack: None,
                },
            )],
        );
        fits(&model, &program, &state, 1);
        // Creating the waited tasks is one successor.
        let (program, mut state) = at_wait(&model, 0, Vec::new());
        state.waiting = false;
        fits(&model, &program, &state, 1);
        // Every branch done under join all: continuing is one successor.
        let model = base(2, Some(ir::Join::All));
        let (program, state) = at_wait(
            &model,
            5,
            vec![task(0, Lifecycle::Done), task(1, Lifecycle::Done)],
        );
        fits(&model, &program, &state, 1);
        // A condition branch is one successor.
        let mut model = base(1, None);
        model.process.body[2] = choose(eq(0, 0), 0, 1);
        let (program, mut state) = at_wait(&model, 8, Vec::new());
        state.pc = program
            .iter()
            .position(|i| matches!(i, Instruction::Branch { .. }))
            .unwrap();
        state.results = vec![Some(V(0)), Some(V(0))];
        fits(&model, &program, &state, 1);
    }
}

#[cfg(test)]
mod d2_runtime_tests {
    use super::*;
    use std::hash::{Hash, Hasher};
    const SOURCE: &str = include_str!("../../../fixtures/ctx-contest/model.relca");
    const DECLARATION: &str =
        "contest Party on end.returned route Route.respond reversible_by [j] respond_by 2min";
    fn model(src: &str) -> ir::Model {
        crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap()
    }
    fn at_business(m: &ir::Model) -> State {
        let mut program = vec![Instruction::Stop];
        let pc = compile(&m.process.body, 0, &mut program);
        let mut pending = vec![State::initial(m, pc)];
        while let Some(s) = pending.pop() {
            if s.contest.as_ref().is_some_and(|c| c.business.is_some()) {
                return s;
            }
            if let Step::Next(xs) =
                step_observed(s, &program, m, &mut Room(1000), &mut Exploration::default()).unwrap()
            {
                pending.extend(xs);
            }
        }
        panic!("business end not reached")
    }
    fn hash(s: &State) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    }
    fn aligned(m: &ir::Model) -> outcome::Outcome {
        let e = explore(m);
        let mut r = crate::reference::explore(m);
        assert_eq!(r.stop, crate::reference::StopReason::Complete);
        assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
        r.outcome.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(e, r.outcome);
        e
    }
    #[test]
    fn d2_same_target_choices_are_joint_and_independent() {
        let source = SOURCE.replace(DECLARATION, &format!("{DECLARATION}; {DECLARATION}"));
        let m = model(&source);
        assert_eq!(m.contests().len(), 2);
        let mut program = vec![Instruction::Stop];
        let pc = compile(&m.process.body, 0, &mut program);
        let mut pending = vec![State::initial(&m, pc)];
        let mut seen = HashSet::new();
        let mut masks = BTreeSet::new();
        while let Some(s) = pending.pop() {
            if !seen.insert(s.clone()) {
                continue;
            }
            if let Some(c) = &s.contest
                && c.routes.iter().all(|r| r.filed.is_some())
            {
                masks.insert((c.routes[0].filed.unwrap(), c.routes[1].filed.unwrap()));
            }
            if let Step::Next(xs) = step_observed(
                s,
                &program,
                &m,
                &mut Room(10000),
                &mut Exploration::default(),
            )
            .unwrap()
            {
                pending.extend(xs);
            }
        }
        assert_eq!(
            masks,
            BTreeSet::from([(false, false), (false, true), (true, false), (true, true)])
        );
        let out = aligned(&m);
        assert_eq!(out.contest_runtime.unwrap().len(), 2);
    }
    #[test]
    fn d2_every_future_relevant_contest_field_is_in_state_eq_hash_debug() {
        let m = model(SOURCE);
        let original = at_business(&m);
        for field in 0..13 {
            let mut changed = original.clone();
            let c = changed.contest.as_mut().unwrap();
            match field {
                0 => c.business.as_mut().unwrap().end = ir::EndId(1),
                1 => c.business.as_mut().unwrap().at += 1,
                2 => c.business.as_mut().unwrap().work += 1,
                3 => c.business.as_mut().unwrap().result_at = Some(0),
                4 => c.routes[0].filed = Some(false),
                5 => c.routes[0].filed = Some(true),
                6 => c.routes[0].queued = true,
                7 => c.routes[0].started_at = Some(1),
                8 => c.routes[0].response_at = Some(3),
                9 => c.routes[0].executor = Some(ir::PersonId(1)),
                10 => c.routes[0].result = Some(ir::ValueId(0)),
                11 => c.routes[0].violation = Some("permission"),
                12 => changed.contest = None,
                _ => unreachable!(),
            }
            assert_ne!(original, changed, "field {field}");
            assert_ne!(hash(&original), hash(&changed), "field {field}");
            assert_ne!(
                format!("{original:?}"),
                format!("{changed:?}"),
                "field {field}"
            );
        }
        let out = explore(&m);
        let mut changed = out.clone();
        changed.contest_runtime.as_mut().unwrap()[0].alternatives[1].deadline = Some(99);
        assert_ne!(out, changed);
        assert_ne!(format!("{out:?}"), format!("{changed:?}"));
        let mut changed = out.clone();
        changed
            .contest_summary
            .as_mut()
            .unwrap()
            .business_ended_open
            .max = Some(1.into());
        assert_ne!(out, changed);
        assert_ne!(format!("{out:?}"), format!("{changed:?}"));
    }
    #[test]
    fn d2_room_reserves_binary_choices_before_any_product_and_honors_zero_one() {
        let source = SOURCE.replace(DECLARATION, &[DECLARATION; 12].join("; "));
        let m = model(&source);
        let seed = at_business(&m);
        assert_eq!(seed.contest.as_ref().unwrap().routes.len(), 12);
        for budget in [0, 1] {
            let mut room = Room(budget);
            assert!(matches!(
                contest_control(&seed, &m, &mut room, &mut Exploration::default()),
                Err(Halt::Budget)
            ));
            assert_eq!(room.0, budget);
            assert!(
                seed.contest
                    .as_ref()
                    .unwrap()
                    .routes
                    .iter()
                    .all(|r| r.filed.is_none())
            );
        }
        let mut room = Room(2);
        let Some(Step::Next(xs)) =
            contest_control(&seed, &m, &mut room, &mut Exploration::default()).unwrap()
        else {
            panic!()
        };
        assert_eq!(room.0, 0);
        assert_eq!(xs.len(), 2);
        for s in xs {
            assert_eq!(
                s.contest
                    .unwrap()
                    .routes
                    .iter()
                    .filter(|r| r.filed.is_some())
                    .count(),
                1
            );
            assert!(s.tasks.is_empty());
        }
        let m = model(SOURCE);
        for budget in 0..=40 {
            let mut e = Exploration::default();
            let _ = explore_into(&m, true, budget, std::convert::identity, &mut e);
            assert!(e.census.2 <= budget.max(1));
        }
    }
    #[test]
    fn d2_expired_response_before_clock_cut_is_not_fabricated_as_a_late_finish() {
        for clock in [2, 3, 4] {
            for arrivals in [false, true] {
                let mut src = SOURCE
                    .replace("duration fixed 2min", "duration fixed 3min")
                    .replace("clock 10min", &format!("clock {clock}min"));
                if arrivals {
                    src = src.replace("  bounds", "  arrivals fixed [0min]\n  bounds");
                }
                let m = model(&src);
                let out = aligned(&m);
                let runtime = out
                    .properties
                    .iter()
                    .find(|p| matches!(p.property, ir::Property::Contract(_)))
                    .unwrap();
                assert_eq!(
                    runtime.verdict,
                    if clock == 2 {
                        Verdict::Inconclusive
                    } else {
                        Verdict::Violated
                    }
                );
                let row = &out.contest_runtime.as_ref().unwrap()[0];
                let filed = row.alternatives.iter().find(|a| a.filed).unwrap();
                if clock < 4 {
                    assert_eq!(filed.status, crate::contest::RuntimeStatus::Unresolved);
                    assert!(filed.response_at.is_none());
                    assert_eq!(
                        filed.violation_reason.as_deref(),
                        (clock == 3).then_some("deadline_passed_unresolved")
                    );
                } else {
                    assert_eq!(filed.response_at, Some(4));
                    assert_eq!(filed.violation_reason.as_deref(), Some("late_response"));
                }
            }
        }
    }
    #[test]
    fn d2_post_end_cut_preserves_business_probability_hit_in_both_drivers() {
        for arrivals in [false, true] {
            let mut src = SOURCE.replace("clock 10min", "clock 2min").replace(
                "  verify safety",
                "  verify probability reached(true_flag == yes) >= 0\n  verify safety",
            );
            // A probability query selects the graph driver even with a fixed
            // initial state; the cut concerns route work, not the business end.
            src = src.replace(
                "  type T",
                "  type Flag = enum { yes }\n  latent case { true_flag: Flag = yes }\n  type T",
            );
            if arrivals {
                src = src.replace("  bounds", "  arrivals fixed [0min]\n  bounds");
            }
            let m = model(&src);
            let out = aligned(&m);
            let terminal = if arrivals {
                out.case_outcomes.as_ref().unwrap()[0].terminals[0]
                    .probability
                    .as_ref()
                    .unwrap()
            } else {
                out.terminals[0].probability.as_ref().unwrap()
            };
            assert_eq!(terminal.min, Rational::one());
            assert_eq!(terminal.max, Rational::one());
        }
    }
    #[test]
    fn d2_post_end_route_writes_remain_visible_to_reached_state_queries() {
        for arrivals in [false, true] {
            let mut src=SOURCE.replace("  people", "  observed case { route_done: bool = false }\n  people")
                .replace("duration fixed 2min; result nondet { pass }", "duration fixed 2min; result nondet { pass }; writes route_done; finish { route_done = true }")
                .replace("  verify safety", "  verify probability reached route_done == true >= 0\n  verify safety");
            if arrivals {
                src = src.replace("  bounds", "  arrivals fixed [0min]\n  bounds");
            }
            let m = model(&src);
            let out = aligned(&m);
            let q = out
                .properties
                .iter()
                .find(|p| matches!(p.property, ir::Property::ProbabilityReached(_)))
                .unwrap();
            let p = if arrivals {
                q.cases.as_ref().unwrap()[0].probability.as_ref().unwrap()
            } else {
                q.probability.as_ref().unwrap()
            };
            assert_eq!(p.min, Rational::zero());
            assert_eq!(p.max, Rational::one());
        }
    }
    #[test]
    fn d2_synthetic_site_cannot_alias_a_legal_process_binding() {
        let src=SOURCE.replace("process Flow {","process Flow { contest1 = Route.respond();")
            .replace("contract Controls {","contract Controls { response later: Route.respond.start -> one_of(Route.respond.finish) by 2min after Route.respond.start otherwise unresolved;");
        let m = model(&src);
        let out = aligned(&m);
        let rows = out.obligations.unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.trigger_site == "contest1"));
        assert!(rows.iter().any(|r| r.trigger_site == "contest:1"));
        assert_ne!(rows[0].id, rows[1].id);
        assert!(rows.iter().all(|r| {
            r.outcomes
                .iter()
                .all(|o| o.status == crate::obligation::Status::Fulfilled)
        }));
    }
}
#[cfg(test)]
#[path = "../tests/support/conflict_population.rs"]
pub(crate) mod conflict_population;
#[cfg(test)]
#[path = "../tests/support/conflict_private_state.rs"]
mod conflict_private_state;

#[cfg(test)]
mod conflict_extra_seams {
    use super::*;
    #[test]
    fn cf_existing_element_receipt_notifies_other_running_tasks_only() {
        let m = crate::syntax::parse(include_str!(
            "../../../fixtures/cf-version-atomic/model.relca"
        ))
        .and_then(|a| crate::lower::lower(&a))
        .unwrap();
        let write = *m.capabilities[0]
            .atomic
            .as_ref()
            .unwrap()
            .writes
            .iter()
            .find(|w| matches!(w.source, ir::WriteSource::InsertCase { .. }))
            .unwrap();
        let mut state = State::initial(&m, 0);
        state.state_values[write.variable.0] = ir::StateValue::Set(1);
        assert_eq!(
            write_value(
                &write,
                &state.state_values,
                Some(m.capabilities[0].results[0]),
                0,
                None
            )
            .unwrap(),
            ir::StateValue::Set(1)
        );
        for i in 0..3 {
            state.tasks.push(Task {
                frozen_performance: None,
                stale: vec![],
                executor: None,
                binding: ir::BindingId(i),
                capability: ir::CapabilityId(0),
                lifecycle: if i == 2 {
                    Lifecycle::Queued
                } else {
                    Lifecycle::Running {
                        finish: 1,
                        ack: None,
                    }
                },
            });
        }
        notify_writes(&mut state, Some(ir::BindingId(0)), &[write], &m);
        assert!(state.tasks[0].stale.is_empty());
        assert_eq!(state.tasks[1].stale, vec![write.variable]);
        assert!(state.tasks[2].stale.is_empty());
    }
    #[test]
    fn cf_receipt_marks_every_other_running_checked_task_not_only_the_first() {
        // F04b. One writer (binding 0) and three other tasks of the same checked
        // capability: 1 and 3 running, 2 queued. Calc: every running task other
        // than the writer receives the mark, independent of position, so 1 and 3
        // are marked and 0 and 2 are not. Task 1 is processed (and marked)
        // before task 3, so stopping at the first marked task would leave 3 bare.
        let m = crate::syntax::parse(include_str!(
            "../../../fixtures/cf-version-atomic/model.relca"
        ))
        .and_then(|a| crate::lower::lower(&a))
        .unwrap();
        let write = *m.capabilities[0]
            .atomic
            .as_ref()
            .unwrap()
            .writes
            .iter()
            .find(|w| matches!(w.source, ir::WriteSource::InsertCase { .. }))
            .unwrap();
        let mut state = State::initial(&m, 0);
        for i in 0..4 {
            state.tasks.push(Task {
                frozen_performance: None,
                stale: vec![],
                executor: None,
                binding: ir::BindingId(i),
                capability: ir::CapabilityId(0),
                lifecycle: if i == 2 {
                    Lifecycle::Queued
                } else {
                    Lifecycle::Running {
                        finish: 1,
                        ack: None,
                    }
                },
            });
        }
        notify_writes(&mut state, Some(ir::BindingId(0)), &[write], &m);
        assert!(state.tasks[0].stale.is_empty());
        assert_eq!(state.tasks[1].stale, vec![write.variable]);
        assert!(state.tasks[2].stale.is_empty());
        assert_eq!(state.tasks[3].stale, vec![write.variable]);
    }
    #[test]
    fn cf_prior_loss_survives_a_later_range_stop_in_both_interpreters() {
        // Two overlapping writes at1 consume claims0 ->1 ->2 and establish loss.
        // A later call would increment2 ->3 outside0..2. It stops without writing,
        // but cannot erase the loss already witnessed at1.
        let src = include_str!("../../../fixtures/cf-lww/model.relca")
            .replace("capacity 1", "capacity 2")
            .replace(
                "r = Ledger.claim()",
                "s = Ledger.claim()\n    r = Ledger.claim()",
            );
        let m = crate::syntax::parse(&src)
            .and_then(|a| crate::lower::lower(&a))
            .unwrap();
        let e = explore(&m);
        let r = crate::reference::explore(&m);
        assert_eq!(r.stop, crate::reference::StopReason::Complete);
        for o in [&e, &r.outcome] {
            assert!(!o.state_range_violations.is_empty());
            assert_eq!(
                o.properties
                    .iter()
                    .find(|p| matches!(p.property, ir::Property::NoLostUpdate(_)))
                    .unwrap()
                    .verdict,
                Verdict::Violated
            );
        }
    }
}
