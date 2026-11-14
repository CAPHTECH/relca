//! Small, deliberately unmerged path interpreter for ADR 0001/0002/0005/0006.
//!
//! Time advances one minute at a time. DFS replays even equivalent start orders;
//! there is no state key, visited set, event-time jump, or compressed ack domain.
//! IR invariants are checked independently before execution; malformed public
//! models fail closed rather than relying on lower having been called.

use crate::probability::{ProbabilityRange, Rational};
use crate::{Verdict, diag, ir::*, outcome::*};
use std::collections::{BTreeMap, BTreeSet};
mod arrivals;
#[cfg(test)]
pub(crate) use arrivals::q6_private as q6_observe;
mod arrivals_replay;
mod authority;
mod evidence;
mod evidence_scope;
mod metrics;

#[cfg(test)]
pub(crate) fn waits_review_envelope(points: Vec<crate::metrics::BusinessMetrics>) {
    let mut envelope = metrics::Envelope::default();
    for point in points {
        envelope.add(point);
    }
    envelope.finish("complete").expect("review envelope");
}
mod policy;
mod quality;
pub(crate) mod replay;
mod work_profiles;

// ADR 0029 U2, written here independently of the engine: abort, then a
// computational cut, then a branch stopped by an out-of-range write.
fn reference_coverage(
    aborted: bool,
    truncated: bool,
    range_stopped: bool,
) -> crate::obligation::Coverage {
    use crate::obligation::Coverage;
    if aborted {
        Coverage::Aborted
    } else if truncated {
        Coverage::Truncated
    } else if range_stopped {
        Coverage::RangeStopped
    } else {
        Coverage::Complete
    }
}

fn reference_reason(
    aborted: bool,
    truncated: bool,
    range_stopped: bool,
) -> Option<crate::metrics::Reason> {
    use crate::metrics::Reason;
    if aborted {
        Some(Reason::Aborted)
    } else if truncated {
        Some(Reason::Truncated)
    } else if range_stopped {
        Some(Reason::RangeStopped)
    } else {
        None
    }
}

fn reference_word(aborted: bool, truncated: bool, range_stopped: bool) -> &'static str {
    if aborted {
        "aborted"
    } else if truncated {
        "truncated"
    } else if range_stopped {
        "range_stopped"
    } else {
        "complete"
    }
}

// This interpreter independently withdraws exact cost claims for an incomplete
// enumeration. The shared canonicalizer only orders and deduplicates wire rows.
fn contest_table_alternatives(
    alternatives: Vec<crate::contest::Alternative>,
    model: &Model,
    id: usize,
    truncated: bool,
    range_stopped: bool,
    aborted: bool,
) -> Vec<crate::contest::Alternative> {
    let reason = reference_reason(aborted, truncated, range_stopped);
    let alternatives = alternatives
        .into_iter()
        .map(|mut a| {
            if let Some(reason) = reason
                && a.filed
            {
                a.route_cost.min = None;
                a.route_cost.max = None;
                a.route_cost.reason = Some(reason);
            }
            a
        })
        .collect();
    crate::contest::canonical_alternatives(alternatives, model, id)
}

#[cfg(test)]
mod probability_tests {
    use super::*;
    #[test]
    fn d1_review_numeric_cut_reason() {
        Interpreter::review_numeric_cut_reason();
    }
    const SUM: &str = include_str!("../../../fixtures/prob-sum/model.relca");
    fn model(source: &str) -> Model {
        crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap()
    }
    fn probability(source: &str) -> ProbabilityRange {
        let r = explore(&model(source));
        assert_eq!(r.stop, StopReason::Complete);
        r.outcome
            .properties
            .into_iter()
            .find_map(|p| p.probability)
            .unwrap()
    }
    #[test]
    fn p_reference_simultaneous_independent_product_and_initial_product() {
        let source = r#"model M version "0.2" { type T=enum { ok } observed case { a:bool=false;b:bool=false }
            kernel K { * -> { 0.5:ok with a=true;0.5:ok with a=false } }
            kernel L { * -> { 0.25:ok with b=true;0.75:ok with b=false } }
            actor A { capacity 2;capability first() -> T { duration fixed 1min;outcomes kernel K;writes a } capability second() -> T { duration fixed 1min;outcomes kernel L;writes b } }
            process P { fork f { x=A.first();y=A.second() };join all f;end done }
            bounds { cases 1;live_tasks 2;clock 2min } verify probability reached a == true && b == true <= 0.125 }"#;
        let p = probability(source);
        assert_eq!(p.min.to_string(), "1/8");
        assert_eq!(p.max.to_string(), "1/8");
        let initial = r#"model M version "0.2" { type T=enum { a,b }
            latent case { x:T ~ { a:0.25,b:0.75 } } actor D { latent y:bool ~ { true:0.4,false:0.6 } }
            process P { end done } bounds { cases 1;live_tasks 1;clock 1min }
            verify probability reached x == a && D.y == true <= 1 }"#;
        let r = explore(&model(initial));
        assert_eq!(r.statistics.initial_tuples.len(), 4);
        assert_eq!(
            r.outcome.properties[0]
                .probability
                .as_ref()
                .unwrap()
                .min
                .to_string(),
            "1/10"
        );
        for t in &r.statistics.terminal_traces {
            assert_eq!(
                t.iter()
                    .filter(|o| matches!(o.event, Occurrence::Receipt))
                    .count(),
                1
            );
        }
    }
    #[test]
    fn p_reference_initial_order_receipt_and_allocation() {
        let source = r#"model M version "0.2" {
            latent case { x:bool ~ { true:0.5,false:0.5 }; y:bool }
            process P { end done } bounds { cases 1;clock 1min;live_tasks 1 }
            verify probability reached (x == true && y == true) || (x == false && y == false) <= 1 }"#;
        let m = model(source);
        let r = explore(&m);
        assert_eq!(
            r.outcome.properties[0].probability,
            Some(ProbabilityRange {
                min: Rational::zero(),
                max: Rational::one()
            })
        );
        assert_eq!(r.statistics.initial_tuples.len(), 4);
        for trace in &r.statistics.terminal_traces {
            assert_eq!(
                trace
                    .iter()
                    .filter(|o| matches!(o.event, Occurrence::Receipt))
                    .count(),
                1
            );
        }
        let denied = explore_with_limits(
            &m,
            Limits {
                frames: 1,
                ..Limits::default()
            },
        );
        assert_eq!(denied.stop, StopReason::EnumerationLimit);
        assert_eq!(denied.statistics.successor_clones, 0);
        assert_eq!(denied.statistics.initial_copies, 0);
        let zeros = source.replace("true:0.5,false:0.5", "true:0,false:1");
        let r = explore(&model(&zeros));
        assert_eq!(r.statistics.initial_tuples.len(), 2);
    }
    #[test]
    fn p_reference_merge_transient_no_contract_and_cut_history() {
        let s = SUM.replace(
            "0.25: ok with hit = true; 0.75: ok with hit = false",
            "0.2: ok with hit = true; 0.3: ok with hit = true; 0.5: ok with hit = false",
        );
        assert_eq!(probability(&s).min.to_string(), "1/2");
        let reset=s.replace("process Flow","actor Reset { capacity 1; capability clear() -> T { duration fixed 1min; result nondet { ok }; writes hit; finish { hit = false } } }; process Flow").replace("if hit == true { end bad } else { end good }","r = Reset.clear(); end done");
        assert_eq!(probability(&reset).min.to_string(), "1/2");
        let cut = include_str!("../../../fixtures/prob-cut.broken-short-clock/model.relca")
            .replace("seen == true >= 0.75", "route == long <= 0.5");
        let p = probability(&cut);
        assert_eq!(p.min.to_string(), "1/2");
        assert_eq!(p.max.to_string(), "1/2");
    }
    #[test]
    fn p_reference_actions_cancel_draw_zero_and_finish_time_reads() {
        let cancel = include_str!("../../../fixtures/prob-cancel/model.relca");
        let r = explore(&model(cancel));
        assert_eq!(r.statistics.kernel_draws, 0);
        let forbidden = explore(&model(include_str!(
            "../../../fixtures/prob-cancel.broken-ignore-cancel/model.relca"
        )));
        assert!(forbidden.statistics.kernel_draws > 0);
        let tie = probability(&cancel.replace("fixed 5min", "fixed 2min"));
        assert_eq!(tie.min.to_string(), "0");
        assert_eq!(tie.max.to_string(), "1/2");
        let finish = include_str!("../../../fixtures/prob-finish-read/model.relca");
        assert_eq!(probability(finish).min.to_string(), "1/2");
        let conditional=finish.replace("fixed 1min","fixed 2min").replace("result nondet { ok }; writes flag; finish { flag = true }","outcomes kernel W; writes flag").replace("kernel K given","kernel W { * -> { 0.5: ok with flag = true; 0.5: ok with flag = false } }; kernel K given");
        let p = probability(&conditional);
        assert_eq!(p.min.to_string(), "0");
        assert_eq!(p.max.to_string(), "1/4");
        let zero = explore(&model(include_str!(
            "../../../fixtures/prob-zero/model.relca"
        )));
        assert_eq!(zero.statistics.normal_completions, 1);
        assert!(!zero.outcome.truncated);
        let no_room = explore_with_limits(
            &model(SUM),
            Limits {
                frames: 2,
                ..Limits::default()
            },
        );
        assert_eq!(no_room.stop, StopReason::EnumerationLimit);
        assert!(no_room.statistics.normal_completions == 0);
    }
    #[test]
    fn p_reference_denominator_19_exact_20_cut_and_already_hit() {
        for calls in [19, 20] {
            let tiny = format!("0.{}1", "0".repeat(62));
            let rest = format!("0.{}", "9".repeat(63));
            let mut sequence = String::new();
            for i in 0..calls {
                sequence.push_str(&format!(
                    "x{i} = A.flip(); if x{i} == fail {{ end no }} else {{ "
                ));
            }
            sequence.push_str("y = A.mark(); end yes");
            sequence.push_str(&" }".repeat(calls));
            let source = format!(
                r#"model M version "0.2" {{ type T = enum {{ pass,fail }}
                observed case {{ done:bool = false }} kernel K {{ * -> {{ {tiny}:pass;{rest}:fail }} }}
                actor A {{ capacity 1; capability flip() -> T {{ duration fixed 1min;outcomes kernel K }} capability mark() -> T {{ duration fixed 1min;result nondet {{ pass }};writes done;finish {{ done = true }} }} }}
                process P {{ {sequence} }} bounds {{ cases 1;live_tasks 1;clock 50min }}
                verify safety no_resource_overcommit verify probability reached done == true <= 1 }}"#
            );
            let r = explore(&model(&source));
            assert_eq!(r.stop, StopReason::Complete);
            let q = &r.outcome.properties[1];
            if calls == 19 {
                assert_eq!(q.verdict, Verdict::Satisfied);
                assert_eq!(
                    q.probability.as_ref().unwrap().min.to_string(),
                    format!("1/1{}", "0".repeat(1197))
                );
            } else {
                assert_eq!(q.verdict, Verdict::Inconclusive);
                assert_eq!(r.outcome.properties[0].verdict, Verdict::Inconclusive);
                assert!(r.outcome.truncated);
                assert_eq!(
                    r.outcome
                        .diagnostics
                        .iter()
                        .filter(|d| d.code == diag::W_BUDGET_0)
                        .count(),
                    1
                );
            }
            let already = explore(&model(
                &source.replace("done == true <= 1", "done == false <= 1"),
            ));
            assert_eq!(already.outcome.properties[1].verdict, Verdict::Satisfied);
            assert_eq!(
                already.outcome.properties[1]
                    .probability
                    .as_ref()
                    .unwrap()
                    .min,
                Rational::one()
            );
        }
    }
    #[test]
    fn p_reference_malformed_ir_rejected_independently() {
        let base = model(SUM);
        let faults: &[fn(&mut Model)] = &[
            |m| m.verify[1].property = Property::ProbabilityReached(999),
            |m| m.probability_queries[0].bound = Rational::decimal("1.01").unwrap(),
            |m| {
                m.probability_queries[0].condition = Cond::StateEq(StateMatch {
                    variable: StateVarId(999),
                    value: StateValue::None,
                })
                .into()
            },
            |m| m.capabilities[0].results.clear(),
            |m| m.capabilities[0].kernel.as_mut().unwrap().rows.clear(),
            |m| {
                let k = m.capabilities[0].kernel.as_mut().unwrap();
                k.rows.push(k.rows[0].clone());
            },
            |m| {
                m.capabilities[0]
                    .kernel
                    .as_mut()
                    .unwrap()
                    .given
                    .push(StateVarId(999))
            },
            |m| {
                m.capabilities[0].kernel.as_mut().unwrap().rows[0].branches[0].probability =
                    Rational::decimal("-0.5").unwrap()
            },
            |m| {
                m.capabilities[0].kernel.as_mut().unwrap().rows[0].branches[0].result = ValueId(999)
            },
            |m| {
                m.capabilities[0].kernel.as_mut().unwrap().rows[0].branches[0].overrides[0].0 =
                    StateVarId(999)
            },
            |m| {
                m.capabilities[0].kernel.as_mut().unwrap().rows[0].branches[0].overrides[0].1 =
                    StateValue::None
            },
        ];
        for (i, f) in faults.iter().enumerate() {
            let mut m = base.clone();
            f(&mut m);
            let r = explore(&m);
            assert_eq!(r.stop, StopReason::SemanticAbort, "{i}");
            assert!(r.outcome.terminals.is_empty());
            assert!(
                r.outcome
                    .diagnostics
                    .iter()
                    .any(|d| d.code == diag::U_SEM_0)
            );
        }
    }
    #[test]
    fn p_reference_initial_and_latent_ir_matrix_small_bit_reduction() {
        let base = model(include_str!("../../../fixtures/prob-rows/model.relca"));
        let defects: &[fn(&mut Model)] = &[
            |m| m.state_variables[1].initial = InitialValue::Distribution(999),
            |m| m.initial_distributions[0][0].0 = StateValue::None,
            |m| m.initial_distributions[0][0].1 = Rational::decimal("-0.1").unwrap(),
            |m| m.initial_distributions[0][0].1 = Rational::decimal("1.1").unwrap(),
            |m| {
                m.initial_distributions[0].pop();
            },
            |m| m.capabilities[0].kernel_reads_latent.clear(),
            |m| m.capabilities[0].kernel_reads_latent.push(StateVarId(0)),
            |m| m.capabilities[0].kernel.as_mut().unwrap().rows[0].values[0] = StateValue::None,
            |m| {
                m.capabilities[0].kernel.as_mut().unwrap().rows[0].branches[0].probability =
                    Rational::zero()
            },
        ];
        for defect in defects {
            let mut m = base.clone();
            defect(&mut m);
            assert_eq!(explore(&m).stop, StopReason::SemanticAbort);
        }
        let mut latent = model(
            &SUM.replace("observed case", "latent case")
                .replace("if hit == true { end bad } else { end good }", "end done"),
        );
        latent.capabilities[0].kernel.as_mut().unwrap().rows[0].branches[0]
            .overrides
            .clear();
        assert_eq!(explore(&latent).stop, StopReason::SemanticAbort);
        let m = model(SUM);
        for bits in [2, 3] {
            let mut i = Interpreter {
                case_index: 0,
                denominator_bits: bits,
                duration_numeric_cut: false,
                probability_stack: Vec::new(),
                run_work: None,
                probability_result: None,
                probability_scheduler: false,
                probability_chance: false,
                contract_facts: Vec::new(),
                model: &m,
                limits: Limits::default(),
                evidence: Evidence::default(),
                terminals: BTreeMap::new(),
                statistics: Statistics::default(),
            };
            let bound = |v: Rational| {
                vec![
                    ReferenceBound {
                        values: std::array::from_fn(|_| v.clone()),
                        numeric: false
                    };
                    m.probability_queries.len() + m.ends.len()
                ]
            };
            i.probability_stack.push(ReferenceChoices {
                owner: Default::default(),
                deferred_children: vec![],
                leaf: None,
                children: vec![bound(Rational::one()), bound(Rational::zero())],
            });
            i.probability_finish(Some(&[
                Rational::decimal("0.25").unwrap(),
                Rational::decimal("0.75").unwrap(),
            ]));
            let r = i.probability_result.take().unwrap();
            assert_eq!(r[0].numeric, bits == 2);
            if bits == 3 {
                assert_eq!(r[0].values[0].to_string(), "1/4");
            }
            // Reduction to one is exact; already witnessed targets need no tiny product.
            i.probability_stack.push(ReferenceChoices {
                owner: Default::default(),
                deferred_children: vec![],
                leaf: None,
                children: vec![bound(Rational::one())],
            });
            i.probability_finish(Some(&[Rational::one()]));
            assert!(!i.probability_result.as_ref().unwrap()[0].numeric);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// Enumeration, including all model-bound cuts, finished.
    Complete,
    SemanticAbort,
    EnumerationLimit,
}
#[cfg(test)]
mod integer_tests;
#[cfg(test)]
mod response_projection_tests;

/// Reference computation limits, NOT business/model bounds. All are checked
/// before allocating a successor or descending into another recursive choice.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub frames: usize,
    pub steps_per_path: usize,
    pub tasks_per_path: usize,
    pub control_depth: usize,
    pub branch_depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            frames: 1_000_000,
            steps_per_path: 10_000,
            tasks_per_path: 128,
            control_depth: 128,
            branch_depth: 128,
        }
    }
}

/// Monotone evidence, kept even when numeric terminal summaries are withdrawn.
/// Public to permit synthetic aggregation tests; synthetic overcommit evidence
/// is not a claim that a valid model can reach an overcommitted state.
#[derive(Debug, Default, Clone)]
pub struct Evidence {
    pub lost_updates: BTreeSet<StateVarId>,
    pub reached: BTreeSet<EndId>,
    pub overcommitted: bool,
    pub deadlocked: bool,
    pub truncated: bool,
}

impl Evidence {
    pub fn verdict(&self, property: Property, stop: StopReason) -> Verdict {
        match property {
            Property::NoLostUpdate(v) if self.lost_updates.contains(&v) => Verdict::Violated,
            Property::ReachableEnd(end) if self.reached.contains(&end) => Verdict::Satisfied,
            Property::NoResourceOvercommit if self.overcommitted => Verdict::Violated,
            _ if stop != StopReason::Complete => Verdict::Unsupported,
            _ if self.truncated => Verdict::Inconclusive,
            Property::ReachableEnd(_) => Verdict::Violated,
            Property::NoResourceOvercommit => Verdict::Satisfied,
            Property::NoLostUpdate(_) => Verdict::Satisfied,
            Property::StateInRange => {
                if self.truncated {
                    Verdict::Inconclusive
                } else {
                    Verdict::Satisfied
                }
            }
            // Contract evidence is owned by Interpreter, not the legacy public table.
            Property::Contest(_)
            | Property::Contract(_)
            | Property::ProbabilityReached(_)
            | Property::RecoverableEnd
            | Property::TerminatingEnd
            | Property::AlmostSureEnd => Verdict::Unsupported,
        }
    }
}

/// Bounded counters for otherwise unobservable reference choices, not a log of
/// every frame. They do not purport to observe the other implementation's paths.
#[derive(Debug, Default, Clone)]
pub struct Statistics {
    evidence_unsupported: BTreeSet<ClauseId>,
    // Output collection only. Never consulted by dispatch, reserve, or any
    // transition. The model (including metric-ready task history) stays intact.
    projection: Projection,
    scenario_observed_until: Option<Range>,
    horizon: bool,
    ranges: Vec<StateRangeViolation>,
    refusals: BTreeMap<BindingId, CapabilityRefusal>,
    observed: crate::completion::Observed,
    completion_bad: bool,
    completion_unknown: bool,
    contest_alternatives: BTreeMap<usize, Vec<crate::contest::Alternative>>,
    contest_open: Option<Range>,
    obligations: Vec<crate::obligation::Obligation>,
    metric_envelope: Option<metrics::Envelope>,
    #[cfg(test)]
    duration_ack_candidates: Vec<Tick>,
    #[cfg(test)]
    duration_probe: bool,
    #[cfg(test)]
    duration_copies: usize,
    #[cfg(test)]
    duration_leaves: Vec<(Tick, Vec<Option<Tick>>, Rational)>,
    #[cfg(test)]
    successor_clones: usize,
    #[cfg(test)]
    kernel_draws: usize,
    #[cfg(test)]
    state_seams: Vec<(&'static str, CapabilityId, Vec<StateValue>)>,
    #[cfg(test)]
    initial_copies: usize,
    #[cfg(test)]
    initial_tuples: Vec<Vec<StateValue>>,
    #[cfg(test)]
    terminal_traces: Vec<Vec<Observation>>,
    pub frames: usize,
    pub terminal_paths: usize,
    /// Peak live path-choice slots; output observation only, never a limit.
    pub path_choice_peak: usize,
    pub deadlock_paths: usize,
    pub adoptions: BTreeMap<BindingId, usize>,
    pub normal_completions: usize,
    pub cancellation_completions: usize,
    pub simultaneous_normal_choices: usize,
    pub simultaneous_ack_choices: usize,
}

#[derive(Debug, Clone)]
pub struct ReferenceRun {
    pub outcome: Outcome,
    pub stop: StopReason,
    pub statistics: Statistics,
}

impl ReferenceRun {
    /// ADR 0006: an aborted computation has no external boolean truncation claim.
    pub fn truncated(&self) -> Option<bool> {
        (self.stop == StopReason::Complete).then_some(self.outcome.truncated)
    }

    /// Independent ADR 0002 + 0006 fold; no report/engine aggregation is shared.
    pub fn verdict(&self) -> Verdict {
        let out = &self.outcome;
        if out.diagnostics.iter().any(|d| d.code.starts_with("E-")) {
            return Verdict::Invalid;
        }
        if out.properties.is_empty() {
            return Verdict::Unsupported;
        }
        let has = |v| out.properties.iter().any(|p| p.verdict == v);
        if has(Verdict::Invalid) {
            Verdict::Invalid
        } else if has(Verdict::Violated)
            || out.diagnostics.iter().any(|d| d.code == diag::W_DEADLOCK_0)
        {
            Verdict::Violated
        } else if has(Verdict::Unsupported)
            || out.diagnostics.iter().any(|d| d.code.starts_with("U-"))
        {
            Verdict::Unsupported
        } else if has(Verdict::Inconclusive) {
            Verdict::Inconclusive
        } else if has(Verdict::Vacuous) {
            Verdict::Vacuous
        } else {
            Verdict::Satisfied
        }
    }
}

pub fn explore(model: &Model) -> ReferenceRun {
    explore_projected(model, Projection::Full)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Projection {
    #[default]
    Full,
    ResponseExercise,
}

fn explore_projected(model: &Model, projection: Projection) -> ReferenceRun {
    let mut limits = Limits::default();
    // A 32-attempt declaration expands into bounded dispatch, clock and channel
    // choices. Keep the old corpus's default unchanged; an explicitly supplied
    // Limits remains an actual computational limit, never silently increased.
    if !model.response_attempts().is_empty() {
        limits.branch_depth = 512;
    }
    explore_with_projection(model, limits, projection)
}

/// Agency needs independently enumerated Start bounds, not a second business
/// metrics report. Keep all validation, trace/history, exploration and limits;
/// omit only the pure leaf metric table and its envelope reduction.
pub(crate) fn explore_response_exercise(model: &Model) -> ReferenceRun {
    explore_projected(model, Projection::ResponseExercise)
}

pub struct SimulationRun {
    pub stop: StopReason,
    pub outcome: crate::outcome::SimulationOutcome,
}

pub fn simulate(model: &Model) -> SimulationRun {
    #[cfg(test)]
    crate::evaluation_spy::hit(5);
    if let Some(v) = model.verify.iter().find(|v| v.property.completion()) {
        return SimulationRun {
            stop: StopReason::SemanticAbort,
            outcome: crate::outcome::SimulationOutcome {
                quality: model
                    .quality
                    .as_ref()
                    .filter(|_| validate(model, Limits::default()).is_ok())
                    .map(|_| quality::simulation(model, &[], true)),
                diagnostics: vec![diag::Diagnostic {
                    code: diag::U_SEM_0.into(),
                    severity: diag::Severity::Error,
                    message: "simulation cannot check structural completion questions".into(),
                    span: Some(v.span),
                }],
                ..Default::default()
            },
        };
    }
    let mut run = arrivals::simulation::simulate(model);
    if model.quality.is_some()
        && validate(model, Limits::default()).is_ok()
        && (run.outcome.quality.is_none()
            || run
                .outcome
                .diagnostics
                .iter()
                .any(|d| d.severity == diag::Severity::Error))
    {
        run.outcome.quality = Some(quality::simulation(model, &[], true));
    }
    for r in &mut run.outcome.replications {
        if let Some(metrics) = &mut r.metrics {
            metrics.apply_deferred(model);
        }
    }
    if let Some(metrics) = &mut run.outcome.metric_template {
        metrics.apply_deferred(model);
    }
    if let Some(quality) = &mut run.outcome.quality {
        quality.apply_deferred(model);
    }
    run
}
#[cfg(test)]
pub(crate) fn quality_empty(model: &Model, abort: bool) -> crate::quality::Quality {
    quality::simulation(model, &[], abort)
}
#[cfg(test)]
pub(crate) fn quantile_numeric_test() {
    let m = crate::quantile_test_support::model();
    let mut normal = explore(&m);
    crate::quantile_private_contract::numeric_isolation_contract(|bits| {
        quality::TEST_BITS.with(|b| b.set(Some(bits)));
        let mut limited = explore(&m);
        quality::TEST_BITS.with(|b| b.set(None));
        let reason = limited
            .outcome
            .quality
            .as_ref()
            .unwrap()
            .quantiles
            .as_ref()
            .unwrap()[0]["value"]["reason"]
            .clone();
        normal.outcome.quality.as_mut().unwrap().quantiles = None;
        limited.outcome.quality.as_mut().unwrap().quantiles = None;
        serde_json::json!({"quantile_reason":reason,"old_outcome_unchanged":normal.outcome==limited.outcome,"old_diagnostics_unchanged":normal.outcome.diagnostics==limited.outcome.diagnostics,"old_census_unchanged":format!("{:?}",normal.statistics)==format!("{:?}",limited.statistics)})
    });
}
#[cfg(test)]
pub(crate) fn quantile_sparse_test() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/qtl-static/model.relca"
    ))
    .unwrap();
    let mut m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
    crate::quantile_private_contract::sparse_graph_contract(|clock| {
        m.bounds.clock = clock;
        quality::GRAPH_BUILDS.with(|n| n.set(0));
        let out = explore(&m);
        serde_json::json!({"graph_builds":quality::GRAPH_BUILDS.with(|n|n.get()),"quantile":out.outcome.quality.unwrap().quantiles.unwrap()[0]["value"]["lower"]})
    });
}

pub fn explore_with_limits(model: &Model, limits: Limits) -> ReferenceRun {
    explore_with_projection(model, limits, Projection::Full)
}

fn explore_with_projection(model: &Model, limits: Limits, projection: Projection) -> ReferenceRun {
    let timer = crate::perf::span("reference.total");
    #[cfg(test)]
    crate::evaluation_spy::hit(2);
    // The independent interpreter enumerates a bounded recursion tree. Expanded
    // attempts need an explicit bounded host stack, not the test harness's small
    // default stack. This does not change the semantic enumeration limits.
    let mut run = if model
        .response_attempts()
        .iter()
        .any(|(_, r)| r.max_attempts > 8)
    {
        std::thread::scope(|scope| {
            let unavailable = || {
                explore_unprojected(
                    model,
                    Limits {
                        frames: 0,
                        ..limits
                    },
                    projection,
                )
            };
            match std::thread::Builder::new()
                .stack_size(64 * 1024 * 1024)
                .spawn_scoped(scope, || explore_unprojected(model, limits, projection))
            {
                Ok(worker) => worker.join().unwrap_or_else(|_| unavailable()),
                Err(_) => unavailable(),
            }
        })
    } else {
        explore_unprojected(model, limits, projection)
    };
    if let Some(metrics) = &mut run.outcome.metrics {
        metrics.apply_deferred(model);
    }
    if let Some(quality) = &mut run.outcome.quality {
        quality.apply_deferred(model);
    }
    crate::perf::count("reference.total", "frames", run.statistics.frames);
    drop(timer);
    run
}

fn explore_unprojected(model: &Model, limits: Limits, projection: Projection) -> ReferenceRun {
    if !model.legacy_route() {
        return arrivals::explore_projected(model, limits, projection);
    }
    let mut interpreter = Interpreter {
        case_index: 0,
        probability_stack: Vec::new(),
        run_work: None,
        probability_result: None,
        probability_scheduler: false,
        probability_chance: !model.initial_distributions.is_empty(),
        denominator_bits: 4096,
        duration_numeric_cut: false,
        model,
        limits,
        evidence: Evidence::default(),
        terminals: BTreeMap::new(),
        statistics: Statistics {
            projection,
            ..Statistics::default()
        },
        contract_facts: vec![(false, false); model.contracts.len()],
    };
    let result = validate(model, limits)
        .and_then(|()| interpreter.reserve(0))
        .and_then(|()| {
            if limits.control_depth == 0 {
                return Err(StopReason::EnumerationLimit);
            }
            interpreter.initialize(
                Frame {
                    condition_waits: Vec::new(),
                    lost_updates: BTreeSet::new(),
                    state_snapshots: Vec::new(),
                    business: None,
                    state_values: model
                        .state_variables
                        .iter()
                        .map(|v| match v.initial {
                            InitialValue::Fixed(x) => x,
                            InitialValue::Nondet | InitialValue::Distribution(_) => {
                                if v.integer_max.is_some() {
                                    StateValue::Integer(0)
                                } else {
                                    StateValue::Value(ValueId(0))
                                }
                            }
                        })
                        .collect(),
                    now: 0,
                    steps: 0,
                    control: vec![model.process.body.as_slice()],
                    waiting: None,
                    tasks: Vec::new(),
                    adoption: None,
                    trace: Vec::new(),
                },
                0,
            )
        });
    let stop = result.err().unwrap_or(StopReason::Complete);
    let probability = interpreter.probability_result.take();
    let numeric = interpreter.duration_numeric_cut
        || probability
            .as_ref()
            .is_some_and(|v| v.iter().any(|v| v.numeric));
    if numeric {
        interpreter.evidence.truncated = true;
    }
    // Corrupt diagnostics are themselves an invalid IR invariant, not trusted
    // output vocabulary. Validation has already forced U-SEM-0 in that case.
    let mut diagnostics: Vec<_> = model
        .diagnostics
        .iter()
        .filter(|d| {
            d.severity == diag::Severity::Warning
                && diag::severity_of(&d.code) == Some(diag::Severity::Warning)
        })
        .cloned()
        .collect();
    if interpreter.evidence.deadlocked {
        diagnostics.push(diagnostic(
            diag::W_DEADLOCK_0,
            if model.contests().is_empty()
                && model.resources.is_empty()
                && !model.completion_requested()
                && !model.has_requires()
                && !model.has_metric_wait()
            {
                "reachable stopped nonterminal path"
            } else {
                "a reachable nonterminal state has no enabled transition"
            },
        ));
    }
    if numeric {
        diagnostics.push(diagnostic(
            diag::W_BUDGET_0,
            "probability denominator budget exceeded",
        ));
    }
    if stop != StopReason::Complete {
        interpreter.terminals.clear();
        diagnostics.push(diagnostic(
            diag::U_SEM_0,
            match stop {
                StopReason::EnumerationLimit => "reference enumeration limit exceeded",
                _ => "reference arithmetic or unresolved semantics",
            },
        ));
    }
    if stop == StopReason::Complete
        && !interpreter.evidence.truncated
        && interpreter.statistics.ranges.is_empty()
    {
        diagnostics.extend(interpreter.statistics.observed.warnings(model));
    }
    let quality = model
        .quality
        .as_ref()
        .filter(|_| validate(model, limits).is_ok())
        .map(|_| arrivals::explore(model, limits).outcome.quality.unwrap());
    ReferenceRun {
        outcome: Outcome {
            response_exercise: None,
            response_receipts: None,
            response_recovery: None,
            scenario_observed_until: None,
            quality,
            state_range_violations: interpreter.statistics.ranges.clone(),
            capability_refusals: model.has_requires().then(|| {
                interpreter
                    .statistics
                    .refusals
                    .values()
                    .cloned()
                    .map(|mut r| {
                        r.coverage = reference_word(
                            stop != StopReason::Complete,
                            interpreter.evidence.truncated,
                            !interpreter.statistics.ranges.is_empty(),
                        )
                        .into();
                        r
                    })
                    .collect()
            }),
            unchecked: if stop != StopReason::Complete
                || interpreter.evidence.truncated
                || !interpreter.statistics.ranges.is_empty()
            {
                vec!["unreachable".into()]
            } else {
                Vec::new()
            },
            postprocessing_diagnostics: Vec::new(),
            contest_runtime: (!model.contests().is_empty()
                && crate::contest::runtime_declarations_valid(model))
            .then(|| {
                model
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
                        coverage: reference_coverage(
                            stop != StopReason::Complete,
                            interpreter.evidence.truncated,
                            !interpreter.statistics.ranges.is_empty(),
                        ),
                        alternatives: contest_table_alternatives(
                            interpreter
                                .statistics
                                .contest_alternatives
                                .get(&id)
                                .cloned()
                                .unwrap_or_default(),
                            model,
                            id,
                            interpreter.evidence.truncated,
                            !interpreter.statistics.ranges.is_empty(),
                            stop != StopReason::Complete,
                        ),
                    })
                    .collect()
            }),
            contest_summary: (!model.contests().is_empty()).then(|| {
                let reason = reference_reason(
                    stop != StopReason::Complete,
                    interpreter.evidence.truncated,
                    !interpreter.statistics.ranges.is_empty(),
                );
                let range = interpreter.statistics.contest_open.unwrap_or(singleton(0));
                crate::contest::Summary {
                    business_ended_open: crate::metrics::Range {
                        min: reason.is_none().then(|| range.min.into()),
                        max: reason.is_none().then(|| range.max.into()),
                        reason,
                    },
                }
            }),
            obligations: model.tracks_obligations().then(|| {
                let mut rows = interpreter.statistics.obligations.clone();
                for row in &mut rows {
                    row.coverage = reference_coverage(
                        stop != StopReason::Complete,
                        interpreter.evidence.truncated,
                        !interpreter.statistics.ranges.is_empty(),
                    );
                }
                rows
            }),
            metrics: interpreter.statistics.metric_envelope.take().and_then(|e| {
                e.finish(reference_word(
                    stop != StopReason::Complete,
                    interpreter.evidence.truncated,
                    !interpreter.statistics.ranges.is_empty(),
                ))
            }),
            case_outcomes: None,
            admission: None,
            probability_scheduler: interpreter.probability_scheduler
                && interpreter.probability_chance,
            probability_policy_unresolved: None,
            properties: model
                .verify
                .iter()
                .map(|v| PropertyOutcome {
                    cases: None,
                    probability: match v.property {
                        Property::AlmostSureEnd if stop != StopReason::Complete => {
                            Some(ProbabilityRange {
                                min: Rational::zero(),
                                max: Rational::one(),
                            })
                        }
                        Property::AlmostSureEnd if stop == StopReason::Complete => {
                            probability.as_ref().map(|p| {
                                let b = &p[model.probability_queries.len() + model.ends.len()];
                                ProbabilityRange {
                                    min: b.values[0].clone(),
                                    max: b.values[3].clone(),
                                }
                            })
                        }
                        Property::ProbabilityReached(id) if stop == StopReason::Complete => {
                            probability.as_ref().map(|p| ProbabilityRange {
                                min: p[id].values[0].clone(),
                                max: p[id].values[3].clone(),
                            })
                        }
                        _ => None,
                    },
                    property: v.property,
                    verdict: if let Some(verdict) = interpreter.range_stop_verdict(v.property) {
                        verdict
                    } else if v.property.completion() {
                        if v.property == Property::AlmostSureEnd && stop == StopReason::Complete {
                            let b = &probability.as_ref().unwrap()
                                [model.probability_queries.len() + model.ends.len()];
                            if b.numeric {
                                Verdict::Inconclusive
                            } else if b.values[0] == Rational::one() {
                                Verdict::Satisfied
                            } else if b.values[1] < Rational::one() {
                                Verdict::Violated
                            } else {
                                Verdict::Inconclusive
                            }
                        } else if interpreter.statistics.completion_bad {
                            Verdict::Violated
                        } else if stop != StopReason::Complete {
                            Verdict::Unsupported
                        } else if interpreter.statistics.completion_unknown {
                            Verdict::Inconclusive
                        } else {
                            Verdict::Satisfied
                        }
                    } else if let Property::Contest(id) = v.property {
                        crate::contest::verdict(model, id)
                    } else if let Property::ProbabilityReached(id) = v.property {
                        if stop != StopReason::Complete {
                            Verdict::Unsupported
                        } else {
                            let b = &probability.as_ref().unwrap()[id];
                            let q = &model.probability_queries[id];
                            if b.numeric {
                                Verdict::Inconclusive
                            } else {
                                match q.comparison {
                                    Comparison::Le if b.values[3] <= q.bound => Verdict::Satisfied,
                                    Comparison::Le if b.values[2] > q.bound => Verdict::Violated,
                                    Comparison::Ge if b.values[0] >= q.bound => Verdict::Satisfied,
                                    Comparison::Ge if b.values[1] < q.bound => Verdict::Violated,
                                    _ => Verdict::Inconclusive,
                                }
                            }
                        }
                    } else if let Property::Contract(id) = v.property {
                        let (active, broken) = interpreter
                            .contract_facts
                            .get(id.0)
                            .copied()
                            .unwrap_or_default();
                        if broken {
                            Verdict::Violated
                        } else if interpreter.statistics.evidence_unsupported.contains(&id)
                            || stop != StopReason::Complete
                        {
                            Verdict::Unsupported
                        } else if interpreter.evidence.truncated {
                            Verdict::Inconclusive
                        } else if active {
                            Verdict::Satisfied
                        } else {
                            Verdict::Vacuous
                        }
                    } else {
                        interpreter.evidence.verdict(v.property, stop)
                    },
                })
                .collect(),
            terminals: interpreter
                .terminals
                .into_values()
                .map(|mut t| {
                    if let Some(p) = &probability
                        && model.uses_probability()
                    {
                        let b = &p[model.probability_queries.len() + t.end.0];
                        t.probability = Some(ProbabilityRange {
                            min: b.values[0].clone(),
                            max: b.values[3].clone(),
                        });
                    }
                    t
                })
                .collect(),
            truncated: interpreter.evidence.truncated,
            diagnostics,
        },
        stop,
        statistics: interpreter.statistics,
    }
}

fn diagnostic(code: &str, message: &str) -> diag::Diagnostic {
    diag::Diagnostic {
        code: code.into(),
        severity: if code.starts_with("W-") {
            diag::Severity::Warning
        } else {
            diag::Severity::Error
        },
        message: message.into(),
        span: None,
    }
}

type Execution = Result<(), StopReason>;

fn reference_resource_units<'a>(
    tasks: impl Iterator<Item = &'a Task>,
    m: &Model,
) -> Result<Vec<u64>, StopReason> {
    let mut totals = vec![0u64; m.resources.len()];
    for task in tasks {
        if task.phase != Phase::Running {
            continue;
        }
        for claim in &m.capabilities[task.call.capability.0].uses {
            totals[claim.resource.0] = totals[claim.resource.0]
                .checked_add(claim.amount)
                .ok_or(StopReason::SemanticAbort)?;
        }
    }
    Ok(totals)
}

#[cfg(test)]
#[path = "reference/resource_tests.rs"]
mod resource_tests;

fn require(valid: bool) -> Execution {
    valid.then_some(()).ok_or(StopReason::SemanticAbort)
}

/// ADR 0030: a per_case Actor holds no capacity, executors or Capability.
fn validate_instances(m: &Model) -> Execution {
    for (n, a) in m.actors.iter().enumerate() {
        if a.metadata.instance == Some(ActorInstance::PerCase) {
            require(
                a.capacity.is_none() && !a.metadata.capacity_declared && a.executors.is_none(),
            )?;
            require(m.capabilities.iter().all(|c| c.actor.0 != n))?;
        }
    }
    Ok(())
}

/// Validate the public IR contract, not source syntax. Zero-duration execution
/// remains a reached-path unsupported domain; an unreachable zero call is legal.
mod record;
// Deliberately independent of the engine's Rule validator.
#[cfg(test)]
#[path = "reference/rule_integration_tests.rs"]
mod rule_integration_tests;

fn validate_rules(m: &Model) -> Execution {
    let mut covered = BTreeSet::new();
    for (actor_id, actor) in m.actors.iter().enumerate() {
        if let Some(r) = &actor.metadata.rule {
            require(
                !actor.is_per_case()
                    && !r.declared_version.is_empty()
                    && !r.effective_version.is_empty()
                    && !r.values.is_empty(),
            )?;
            for rv in &r.values {
                require(covered.insert(rv.variable))?;
                let v = m
                    .state_variables
                    .get(rv.variable.0)
                    .ok_or(StopReason::SemanticAbort)?;
                let t = m.types.get(v.base.0).ok_or(StopReason::SemanticAbort)?;
                require(
                    v.kind == StateKind::Rule(ActorId(actor_id))
                        && !v.optional
                        && v.integer_max.is_none()
                        && v.case_set_size.is_none()
                        && !matches!(t.name.as_str(), "PersonId" | "CaseId")
                        && t.record.is_none()
                        && !t.members.is_empty(),
                )?;
                require(matches!(rv.declared, StateValue::Value(x) if x.0 < t.members.len()))?;
                require(
                    matches!(v.initial, InitialValue::Fixed(StateValue::Value(x)) if x.0 < t.members.len()),
                )?;
                if r.amendment.is_none() {
                    require(v.initial == InitialValue::Fixed(rv.declared))?;
                }
            }
            if let Some(a) = &r.amendment {
                require(
                    a.by.0 < m.people.len()
                        && a.from == r.declared_version
                        && a.to == r.effective_version
                        && a.from != a.to,
                )?;
                let auth = m
                    .authorities
                    .get(a.under.0)
                    .ok_or(StopReason::SemanticAbort)?;
                require(
                    !auth.not_by.contains(&a.by)
                        && auth
                            .rule_grants
                            .iter()
                            .any(|g| g.rule == ActorId(actor_id) && g.persons.contains(&a.by)),
                )?;
                if let Some(b) = &auth.under {
                    let prior = m
                        .actors
                        .get(b.rule.0)
                        .and_then(|x| x.metadata.rule.as_ref())
                        .ok_or(StopReason::SemanticAbort)?;
                    require(b.version == prior.declared_version)?;
                }
            } else {
                require(r.declared_version == r.effective_version)?;
            }
        }
    }
    for (i, v) in m.state_variables.iter().enumerate() {
        if matches!(v.kind, StateKind::Rule(_)) {
            require(covered.contains(&StateVarId(i)))?;
        }
    }
    for a in &m.authorities {
        require(a.rule_grants.is_empty() || !a.restrictions.restricted())?;
        if let Some(b) = &a.under {
            require(
                a.grants.is_empty()
                    && !b.version.is_empty()
                    && m.actors
                        .get(b.rule.0)
                        .is_some_and(|a| a.metadata.rule.is_some()),
            )?;
        }
        for g in &a.rule_grants {
            require(
                m.actors
                    .get(g.rule.0)
                    .is_some_and(|a| a.metadata.rule.is_some())
                    && !g.persons.is_empty(),
            )?;
            let mut people = BTreeSet::new();
            for p in &g.persons {
                require(p.0 < m.people.len() && people.insert(*p))?;
            }
        }
    }
    let forbidden = |id: &StateVarId| {
        m.state_variables
            .get(id.0)
            .is_some_and(|v| matches!(v.kind, StateKind::Rule(_)))
    };
    for c in &m.capabilities {
        for w in c
            .start_writes
            .iter()
            .chain(&c.writes)
            .chain(c.atomic.iter().flat_map(|a| &a.writes))
        {
            require(!forbidden(&w.variable))?;
        }
        require(
            !c.kernel_writes
                .iter()
                .chain(&c.kernel_reads_latent)
                .any(forbidden),
        )?;
    }
    for k in m
        .kernels
        .iter()
        .chain(m.capabilities.iter().flat_map(|c| c.result_kernels()))
    {
        require(!k.given.iter().any(forbidden))?;
        for row in &k.rows {
            for branch in &row.branches {
                for (id, _) in &branch.overrides {
                    require(!forbidden(id))?;
                }
            }
        }
    }
    Ok(())
}

fn validate(m: &Model, limits: Limits) -> Execution {
    work_profiles::validate(m)?;
    if m.uses_work_profiles() {
        require(
            m.arrival_distribution.is_none()
                && m.contests().is_empty()
                && m.response_attempts().iter().all(|(_, r)| {
                    matches!(r.loss, ResponseLoss::None)
                        && r.max_attempts == 1
                        && r.retry_span.is_none()
                }),
        )?;
    }
    degradation::validate(m)?;
    policy::validate_priority(m)?;
    response::validate(m)?;
    require(m.validate_cut_targets().is_ok())?;
    record::validate(m)?;
    validate_rules(m)?;
    if let (Some(a), Some(b)) = (m.admission_queue, m.concurrent_cases) {
        require(a.overflow == b.overflow)?;
    }
    let mut terminal = None;
    for q in m.admission_queue.iter().chain(m.concurrent_cases.iter()) {
        match q.overflow {
            Overflow::WaitExternalFifo | Overflow::WaitExternalPriority(_) => {
                require(q.capacity > 0)?
            }
            Overflow::End(id) => {
                require(id.0 < m.ends.len() && Some(id) == m.overflow_end)?;
                terminal = Some(id);
            }
            Overflow::Reject => {}
        }
    }
    require(terminal == m.overflow_end)?;
    fn excludes(b: &Block, e: EndId) -> bool {
        b.iter().all(|s| match &s.kind {
            Stmt::Response(_, r) => r.exhausted != Some(e),
            Stmt::End(id) => *id != e,
            Stmt::Fork {
                join: Join::Any { if_none, .. },
                ..
            } => *if_none != e,
            Stmt::If {
                then_body,
                else_body,
                ..
            } => excludes(then_body, e) && excludes(else_body, e),
            _ => true,
        })
    }
    if let Some(e) = terminal {
        require(excludes(&m.process.body, e))?;
        require(!m.contests().iter().any(|c| c.target.end() == Some(e)))?;
        require(
            !m.responsibility
                .as_ref()
                .is_some_and(|r| r.standings.iter().any(|s| s.target.end() == Some(e))),
        )?;
    }
    for c in &m.contracts {
        if let ContractKind::Response {
            trigger: Trigger::Admitted,
            ..
        }
        | ContractKind::NamedResponse {
            trigger: Trigger::Admitted,
            ..
        } = c.kind
        {
            require(m.external_wait() || terminal.is_some())?;
        }
    }
    arrivals::validate_scenario(m, m.arrivals.as_deref().unwrap_or(&[0]))?;
    policy::validate(m)?;
    validate_instances(m)?;
    let mut resource_names = BTreeSet::new();
    for r in &m.resources {
        require(
            !r.name.is_empty()
                && resource_names.insert(&r.name)
                && !m.actors.iter().any(|a| a.name == r.name),
        )?;
    }
    for c in &m.capabilities {
        let mut ids = BTreeSet::new();
        for u in &c.uses {
            require(u.resource.0 < m.resources.len() && u.amount > 0 && ids.insert(u.resource))?;
        }
    }
    require(crate::actor_table::validate(m).is_ok())?;
    require(metrics::validate(m))?;
    fn unique<'a>(names: impl Iterator<Item = &'a str>) -> bool {
        let mut seen = BTreeSet::new();
        names.into_iter().all(|n| !n.is_empty() && seen.insert(n))
    }
    // Legacy route only has cases == 1; arrivals validates its real count (set size == cases below).
    require(m.bounds.cases >= 1 && m.version == "0.2")?;
    require(unique(m.types.iter().map(|t| t.name.as_str())))?;
    require(unique(m.actors.iter().map(|a| a.name.as_str())))?;
    require(unique(m.ends.iter().map(String::as_str)))?;
    require(unique(m.process.bindings.iter().map(|b| b.name.as_str())))?;
    require(unique(m.process.forks.iter().map(String::as_str)))?;
    require(unique(m.people.iter().map(|p| p.name.as_str())))?;
    require(unique(m.authorities.iter().map(|a| a.name.as_str())))?;
    let valid_people = |people: &[PersonId]| {
        let set: BTreeSet<_> = people.iter().copied().collect();
        set.len() == people.len() && set.iter().all(|p| p.0 < m.people.len())
    };
    for a in &m.actors {
        if let Some(pool) = &a.executors {
            require(
                !pool.is_empty() && valid_people(pool) && a.capacity == Some(pool.len() as u64),
            )?;
        }
    }
    for a in &m.authorities {
        require((!a.grants.is_empty() || !a.rule_grants.is_empty()) && valid_people(&a.not_by))?;
        if let Some(p) = a.issued_by {
            require(p.0 < m.people.len() && !a.not_by.contains(&p))?;
        }
        for grant in &a.grants {
            require(
                grant.capability.0 < m.capabilities.len()
                    && !grant.persons.is_empty()
                    && valid_people(&grant.persons),
            )?;
            require(grant.persons.iter().all(|p| !a.not_by.contains(p)))?;
        }
    }
    authority::validate(m)?;
    for t in &m.types {
        require(
            (!t.members.is_empty() || t.name == "PersonId")
                && unique(t.members.iter().map(String::as_str)),
        )?;
    }
    for v in &m.state_variables {
        require(
            v.case_set_size.is_some()
                || v.integer_max.is_some()
                || m.types
                    .get(v.base.0)
                    .is_some_and(|t| !t.members.is_empty() || (v.optional && t.name == "PersonId")),
        )?;
        require(
            v.integer_max.is_none()
                || (!v.optional && !matches!(v.kind, StateKind::ActorLatent(_))),
        )?;
        require(v.kind.actor().is_none_or(|a| a.0 < m.actors.len()))?;
        require(v.kind.is_observed() || !v.optional)?;
        if let Some(n) = v.case_set_size {
            require(
                (1..=64).contains(&n)
                    && u64::from(n) == m.bounds.cases
                    && v.integer_max.is_none()
                    && v.base == CASE_ID_TYPE
                    && !v.optional
                    && m.is_shared_observed(v.kind)
                    && v.initial == InitialValue::Fixed(StateValue::Set(0)),
            )?;
        }
        if m.types.get(v.base.0).is_some_and(|t| t.name == "PersonId") {
            require(
                v.kind.is_observed()
                    && m.types[v.base.0]
                        .members
                        .iter()
                        .map(String::as_str)
                        .eq(m.people.iter().map(|p| p.name.as_str())),
            )?;
        }
        match v.initial {
            InitialValue::Distribution(id) => {
                let entries = m
                    .initial_distributions
                    .get(id)
                    .ok_or(StopReason::SemanticAbort)?;
                let mut total = Rational::zero();
                let mut seen = std::collections::HashSet::new();
                for (x, p) in entries {
                    require(reference_value_valid(m, v, *x) && p.valid() && seen.insert(*x))?;
                    total.0 += &p.0;
                }
                require(
                    v.kind.is_latent()
                        && !v.optional
                        && seen.len() as u128 == reference_cardinality(m, v)
                        && total == Rational::one(),
                )?;
            }
            InitialValue::Fixed(x) => require(reference_value_valid(m, v, x))?,
            InitialValue::Nondet => require(v.kind.is_latent() && !v.optional)?,
        }
    }
    let mut capabilities = BTreeSet::new();
    for c in &m.capabilities {
        fn requires_valid(c: &Cond, m: &Model, cap: &Capability, depth: usize) -> Execution {
            require(depth < 128)?;
            match c {
                Cond::Contains { set, element } => require(
                    m.state_variables
                        .get(set.0)
                        .is_some_and(|v| v.case_set_size.is_some())
                        && match element {
                            Element::CurrentCase => true,
                            Element::Parameter(i) => {
                                cap.params.get(*i).is_some_and(|p| p.ty == CASE_ID_TYPE)
                            }
                        },
                )?,
                Cond::IntCompare { variable, .. } => require(
                    m.state_variables
                        .get(variable.0)
                        .is_some_and(|v| v.integer_max.is_some() && v.kind.is_observed()),
                )?,
                Cond::StateEq(x) => {
                    let v = m
                        .state_variables
                        .get(x.variable.0)
                        .ok_or(StopReason::SemanticAbort)?;
                    require(
                        v.case_set_size.is_none()
                            && v.kind.is_observed()
                            && reference_value_valid(m, v, x.value),
                    )?;
                }
                Cond::And(cs) | Cond::Or(cs) => {
                    require(cs.len() >= 2)?;
                    for c in cs {
                        requires_valid(c, m, cap, depth + 1)?;
                    }
                }
                Cond::Eq(_) => return Err(StopReason::SemanticAbort),
            }
            Ok(())
        }
        if let Some(req) = &c.requires {
            requires_valid(&req.condition, m, c, 0)?;
            require(
                m.types
                    .get(c.return_type.0)
                    .is_some_and(|t| req.otherwise.0 < t.members.len()),
            )?;
        }
        let mut start_targets = BTreeSet::new();
        let own_observed = |v: &StateVariable| match v.kind {
            StateKind::ObservedCase => true,
            StateKind::ActorObserved(owner) => {
                owner == c.actor && m.actors.get(owner.0).is_some_and(|a| !a.is_per_case())
            }
            _ => false,
        };
        for w in &c.start_writes {
            let v = m
                .state_variables
                .get(w.variable.0)
                .ok_or(StopReason::SemanticAbort)?;
            require(own_observed(v) && start_targets.insert(w.variable))?;
            require(
                matches!(w.source, WriteSource::Constant(x) if v.case_set_size.is_none() && reference_value_valid(m,v,x))
                    || reference_identity_write(m, c, w)
                    || matches!(w.source, WriteSource::Integer { operand, .. } if v.integer_max.is_some() && operand.unsigned_abs() <= u128::from(u64::MAX)),
            )?;
        }
        let mut targets = BTreeSet::new();
        for w in c.finish_writes() {
            let v = m
                .state_variables
                .get(w.variable.0)
                .ok_or(StopReason::SemanticAbort)?;
            require(own_observed(v) && targets.insert(w.variable))?;
            match w.source {
                WriteSource::InsertCase { .. } | WriteSource::Executor => {
                    require(reference_identity_write(m, c, w))?
                }
                WriteSource::Integer { operand, .. } => require(
                    v.integer_max.is_some() && operand.unsigned_abs() <= u128::from(u64::MAX),
                )?,
                WriteSource::Result => {
                    require(v.case_set_size.is_none() && v.base == c.return_type)?
                }
                WriteSource::Constant(x) => {
                    require(v.case_set_size.is_none() && reference_value_valid(m, v, x))?
                }
            }
        }
        let actor = m.actors.get(c.actor.0).ok_or(StopReason::SemanticAbort)?;
        let ty = m
            .types
            .get(c.return_type.0)
            .ok_or(StopReason::SemanticAbort)?;
        if let Some(a) = &c.atomic {
            requires_valid(&a.condition, m, c, 0)?;
            require(
                c.kernel.is_none()
                    && c.writes.is_empty()
                    && !a.writes.is_empty()
                    && a.commit != a.reject
                    && a.commit.0 < ty.members.len()
                    && a.reject.0 < ty.members.len()
                    && c.results.len() == 2
                    && c.results.contains(&a.commit)
                    && c.results.contains(&a.reject),
            )?;
        }
        require(capabilities.insert((c.actor, &c.name)) && !c.name.is_empty())?;
        require(c.duration == 0 || actor.capacity.is_some())?;
        if let Some(distribution) = &c.duration_distribution {
            require(
                c.duration == 0 && !distribution.entries.is_empty() && actor.capacity.is_some(),
            )?;
            let mut previous = None;
            let mut total = Rational::zero();
            for (minute, weight) in &distribution.entries {
                require(previous.is_none_or(|p| p < *minute))?;
                require(weight.valid() && (weight.is_zero() || *minute != 0))?;
                previous = Some(*minute);
                total.0 += &weight.0;
            }
            require(total == Rational::one())?;
        }
        if let Some(ps) = &c.permission {
            require(actor.executors.is_some() && !ps.is_empty() && valid_people(ps))?;
        }
        require(!c.results.is_empty())?;
        let mut values = BTreeSet::new();
        for v in &c.results {
            require(v.0 < ty.members.len() && values.insert(*v))?;
        }
        require(!matches!(c.cancel, Cancel::Cooperative { ack_within: 0 }))?;
        let read_set: BTreeSet<_> = c.kernel_reads_latent.iter().copied().collect();
        require(read_set.len() == c.kernel_reads_latent.len())?;
        for id in &read_set {
            require(
                m.state_variables
                    .get(id.0)
                    .is_some_and(|v| v.kind.is_latent()),
            )?;
        }
        for id in &c.kernel_writes {
            require(
                (c.kernel.is_some() || c.performance.is_some())
                    && targets.insert(*id)
                    && m.state_variables
                        .get(id.0)
                        .is_some_and(|v| v.kind.is_latent()),
            )?;
        }
        if c.kernel.is_some() || c.performance.is_some() {
            let mut possible = BTreeSet::new();
            for k in c.result_kernels() {
                reference_kernel(k, m)?;
                for id in &k.given {
                    require(
                        m.state_variables[id.0].kind == StateKind::ObservedCase
                            || read_set.contains(id),
                    )?;
                }
                for row in &k.rows {
                    for b in &row.branches {
                        require(b.result.0 < ty.members.len())?;
                        if b.probability > Rational::zero() {
                            possible.insert(b.result);
                        }
                        let mut assigned = BTreeSet::new();
                        for (id, x) in &b.overrides {
                            require(assigned.insert(*id) && targets.contains(id))?;
                            let variable = m
                                .state_variables
                                .get(id.0)
                                .ok_or(StopReason::SemanticAbort)?;
                            require(
                                reference_value_valid(m, variable, *x)
                                    && !matches!(variable.kind, StateKind::ActorObserved(_)),
                            )?;
                        }
                        require(c.kernel_writes.iter().all(|id| assigned.contains(id)))?;
                    }
                }
            }
            require(possible == values)?;
        }
    }
    for declaration in &m.kernels {
        reference_kernel(declaration, m)?;
    }
    for b in &m.process.bindings {
        // The malformed-IR test covers this index. Isolated removal is masked
        // by call/type agreement plus full slot coverage: every binding must
        // be used by a capability whose return type was already validated.
        // Keep this early guard to protect result_match's direct type indexing.
        require(b.ty.0 < m.types.len())?;
    }
    fn state_condition(c: &Cond, m: &Model, depth: usize, limit: usize) -> Execution {
        if depth >= limit {
            return Err(StopReason::EnumerationLimit);
        }
        match c {
            Cond::Contains { .. } => Err(StopReason::SemanticAbort),
            Cond::IntCompare { variable, .. } => require(
                m.state_variables
                    .get(variable.0)
                    .is_some_and(|v| v.integer_max.is_some()),
            ),
            Cond::StateEq(t) => require(m.state_variables.get(t.variable.0).is_some_and(|v| {
                v.case_set_size.is_none() && reference_value_valid(m, v, t.value)
            })),
            Cond::And(cs) | Cond::Or(cs) => {
                require(cs.len() >= 2)?;
                for c in cs {
                    state_condition(c, m, depth + 1, limit)?;
                }
                Ok(())
            }
            _ => Err(StopReason::SemanticAbort),
        }
    }
    if let Some(q) = &m.quality {
        require(!q.properties.is_empty() || !q.metrics.is_empty())?;
        let mut pnames = BTreeSet::new();
        for p in &q.properties {
            require(
                !p.name.is_empty()
                    && !matches!(p.name.as_str(), "received" | "admitted" | "id")
                    && pnames.insert(&p.name),
            )?;
            match &p.target {
                QualityTarget::Ends(es) => {
                    require(!es.is_empty())?;
                    let mut seen = BTreeSet::new();
                    for e in es {
                        require(e.0 < m.ends.len() && seen.insert(e.0))?;
                    }
                }
                QualityTarget::State(c) => {
                    state_condition(c, m, 0, limits.control_depth)?;
                    let mut todo = vec![c];
                    while let Some(c) = todo.pop() {
                        match c {
                            Cond::And(cs) | Cond::Or(cs) => todo.extend(cs),
                            Cond::StateEq(StateMatch { variable, .. })
                            | Cond::IntCompare { variable, .. } => {
                                let v = &m.state_variables[variable.0];
                                if let (Some(a), Some(cut)) = (v.kind.actor(), &m.cut) {
                                    require(cut.include.contains(&a))?;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        let mut cnames = BTreeSet::new();
        for c in &q.quantiles {
            require(
                !c.name.is_empty()
                    && cnames.insert(&c.name)
                    && c.property < q.properties.len()
                    && c.p.0.denom() > &num_bigint::BigInt::from(0)
                    && c.p.valid()
                    && c.p != Rational::zero(),
            )?;
        }
        for c in &q.metrics {
            require(
                !c.name.is_empty()
                    && cnames.insert(&c.name)
                    && c.numerator < q.properties.len()
                    && match c.denominator {
                        Denominator::Arrivals => true,
                        Denominator::Property(i) => i < q.properties.len(),
                    },
            )?;
        }
    }
    for q in &m.probability_queries {
        require(q.bound.valid())?;
        match &q.condition {
            ProbabilityCondition::State(c) => state_condition(c, m, 0, limits.control_depth)?,
            ProbabilityCondition::End(e) => require(e.0 < m.ends.len())?,
        }
    }
    let mut properties = BTreeSet::new();
    arrivals::validate_observed_writers(m)?;
    require(
        m.verify
            .iter()
            .filter(|v| v.property == Property::StateInRange)
            .count()
            == usize::from(m.has_integers()),
    )?;
    for v in &m.verify {
        let key = match v.property {
            Property::NoLostUpdate(id) => {
                require(
                    m.state_variables
                        .get(id.0)
                        .is_some_and(|v| v.last_write_wins && m.is_shared_observed(v.kind)),
                )?;
                (10, id.0)
            }
            Property::StateInRange => {
                require(m.has_integers())?;
                (9, 0)
            }
            Property::RecoverableEnd => (5, 0),
            Property::TerminatingEnd => (6, 0),
            Property::AlmostSureEnd => (7, 0),
            Property::Contest(id) => {
                require(
                    m.responsibility
                        .as_ref()
                        .is_some_and(|meta| id < meta.contests.len()),
                )?;
                (4, id)
            }
            Property::ProbabilityReached(id) => {
                require(id < m.probability_queries.len())?;
                (3, id)
            }
            Property::NoResourceOvercommit => (0, 0),
            Property::ReachableEnd(e) => {
                require(e.0 < m.ends.len())?;
                (1, e.0)
            }
            Property::Contract(id) => {
                require(id.0 < m.contracts.len())?;
                (2, id.0)
            }
        };
        require(properties.insert(key))?;
    }
    for d in &m.diagnostics {
        require(
            d.severity == diag::Severity::Warning
                && diag::severity_of(&d.code) == Some(diag::Severity::Warning),
        )?;
    }
    if let Some(cut) = &m.cut {
        for a in &cut.include {
            require(a.0 < m.actors.len())?;
        }
        for e in cut.exclude.iter().chain(&cut.defer) {
            if let CutTarget::Actor(a) = e.target {
                require(a.0 < m.actors.len())?;
            }
        }
    }
    let mut validator = Validator {
        model: m,
        limits,
        calls: BTreeSet::new(),
        forks: BTreeSet::new(),
    };
    validator.block(&m.process.body, BTreeSet::new(), 0)?;
    let mut d1_ordinals = BTreeSet::new();
    let mut d1_labels = BTreeSet::new();
    for clause in &m.contracts {
        if m.tracks_obligations() {
            if let Some(n) = clause.ordinal {
                require(n > 0 && d1_ordinals.insert(n))?;
            }
            if let ContractKind::NamedResponse {
                label: Some(label), ..
            } = &clause.kind
            {
                require(
                    !label.is_empty()
                        && (clause.name.is_none()
                            || d1_labels.insert((clause.name.clone(), label.clone()))),
                )?;
            }
        }
        let event = |e: Event| -> Execution {
            let cap = m
                .capabilities
                .get(e.capability.0)
                .ok_or(StopReason::SemanticAbort)?;
            match e.kind {
                EventKind::CancelAck => Err(StopReason::SemanticAbort),
                EventKind::Commit => require(cap.commits),
                EventKind::Finish(Some(v)) => {
                    require(v.0 < m.types[cap.return_type.0].members.len())
                }
                _ => Ok(()),
            }
        };
        match &clause.kind {
            ContractKind::ContestResponse { contest } => {
                let declaration = m
                    .contests()
                    .get(*contest)
                    .ok_or(StopReason::SemanticAbort)?;
                require(
                    clause.ordinal == Some(declaration.ordinal)
                        && clause.name == declaration.name
                        && clause.span == declaration.span,
                )?;
            }
            ContractKind::Excludes { before, start } => {
                if before.kind == EventKind::CancelAck {
                    require(before.capability.0 < m.capabilities.len())?;
                } else {
                    event(*before)?;
                }
                event(*start)?;
                require(start.kind == EventKind::Start)?;
            }
            ContractKind::Waive {
                response,
                event: e,
                authority,
                ..
            } => {
                event(*e)?;
                require(matches!(e.kind, EventKind::Finish(_) | EventKind::Commit))?;
                let c = m
                    .contracts
                    .get(response.0)
                    .ok_or(StopReason::SemanticAbort)?;
                require(
                    c.response().is_some()
                        && !matches!(
                            c.kind,
                            ContractKind::NamedResponse {
                                non_waivable: true,
                                ..
                            }
                        ),
                )?;
                let a = m
                    .authorities
                    .get(authority.0)
                    .ok_or(StopReason::SemanticAbort)?;
                let cap = &m.capabilities[e.capability.0];
                let ps = m.actors[cap.actor.0]
                    .executors
                    .as_ref()
                    .ok_or(StopReason::SemanticAbort)?;
                require(
                    !ps.is_empty()
                        && ps.iter().all(|p| {
                            a.grants
                                .iter()
                                .any(|g| g.capability == e.capability && g.persons.contains(p))
                        }),
                )?;
            }
            ContractKind::Separation { left, right } => {
                event(*left)?;
                event(*right)?;
                require(left != right)?;
                require([left, right].iter().all(|e| {
                    m.actors[m.capabilities[e.capability.0].actor.0]
                        .executors
                        .is_some()
                }))?;
            }
            ContractKind::EvidenceBasis {
                capability,
                sources,
                ..
            } => {
                evidence::validate(m, *capability, sources)?;
            }
            ContractKind::Permitted { capability }
            | ContractKind::AuthorityBasis { capability, .. } => {
                let c = m
                    .capabilities
                    .get(capability.0)
                    .ok_or(StopReason::SemanticAbort)?;
                require(m.actors[c.actor.0].executors.is_some())?;
                if let ContractKind::AuthorityBasis { authority, .. } = clause.kind {
                    let a = m
                        .authorities
                        .get(authority.0)
                        .ok_or(StopReason::SemanticAbort)?;
                    require(a.grants.iter().any(|g| g.capability == *capability))?;
                }
            }
            ContractKind::Invariant {
                event: e,
                condition,
            } => {
                event(*e)?;
                if let Some(c) = condition {
                    validator.condition(c, &validator.calls, 0, false)?;
                } else {
                    fn invokes(b: &Block, cap: CapabilityId) -> bool {
                        b.iter().any(|s| match &s.kind {
                            Stmt::Call(c) | Stmt::Query(c, _) | Stmt::Response(c, _) => {
                                c.capability == cap
                            }
                            Stmt::Fork { branches, .. } => {
                                branches.iter().any(|c| c.capability == cap)
                            }
                            Stmt::If {
                                then_body,
                                else_body,
                                ..
                            } => invokes(then_body, cap) || invokes(else_body, cap),
                            Stmt::End(_)
                            | Stmt::Wait(_)
                            | Stmt::WaitUntil(_)
                            | Stmt::ResolveEffect { .. }
                            | Stmt::StopReceiptUnknown
                            | Stmt::LoopControl(_)
                            | Stmt::MergeCopy { .. } => false,
                        })
                    }
                    require(!invokes(&m.process.body, e.capability))?;
                }
            }
            ContractKind::StateInvariant {
                antecedent,
                condition,
            } => {
                for c in antecedent.iter().chain(std::iter::once(condition)) {
                    // Membership is a state atom of the case context here
                    // (ADR0035 x ADR0037); probability conditions still refuse it.
                    fn invariant_condition(
                        c: &Cond,
                        m: &Model,
                        depth: usize,
                        limit: usize,
                    ) -> Execution {
                        if depth >= limit {
                            return Err(StopReason::EnumerationLimit);
                        }
                        match c {
                            Cond::Contains { set, element } => require(
                                *element == Element::CurrentCase
                                    && m.state_variables
                                        .get(set.0)
                                        .is_some_and(|v| v.case_set_size.is_some()),
                            ),
                            Cond::And(cs) | Cond::Or(cs) => {
                                require(cs.len() >= 2)?;
                                for c in cs {
                                    invariant_condition(c, m, depth + 1, limit)?;
                                }
                                Ok(())
                            }
                            c => state_condition(c, m, depth, limit),
                        }
                    }
                    invariant_condition(c, m, 0, limits.control_depth)?;
                    let mut reads = vec![c];
                    while let Some(c) = reads.pop() {
                        match c {
                            Cond::And(cs) | Cond::Or(cs) => reads.extend(cs),
                            Cond::Contains { set: variable, .. } => {
                                require(m.state_variables[variable.0].kind.actor().is_none_or(
                                    |a| m.cut.as_ref().is_none_or(|cut| cut.include.contains(&a)),
                                ))?
                            }
                            Cond::StateEq(t) => {
                                require(m.state_variables[t.variable.0].kind.actor().is_none_or(
                                    |a| m.cut.as_ref().is_none_or(|cut| cut.include.contains(&a)),
                                ))?
                            }
                            Cond::IntCompare { variable, .. } => {
                                require(m.state_variables[variable.0].kind.actor().is_none_or(
                                    |a| m.cut.as_ref().is_none_or(|cut| cut.include.contains(&a)),
                                ))?
                            }
                            Cond::Eq(_) => return Err(StopReason::SemanticAbort),
                        }
                    }
                }
            }
            ContractKind::AtMostOnce { event: e } => {
                event(*e)?;
                require(e.kind == EventKind::Commit)?;
            }
            ContractKind::RequiresEvent { before, start } => {
                event(*before)?;
                event(*start)?;
                require(start.kind == EventKind::Start)?;
            }
            ContractKind::Response {
                trigger, targets, ..
            }
            | ContractKind::NamedResponse {
                trigger, targets, ..
            } => {
                if let Trigger::Event(e) = trigger {
                    event(*e)?;
                }
                require(!targets.is_empty())?;
                for t in targets {
                    match t {
                        Target::End(e) => require(e.0 < m.ends.len())?,
                        Target::Event(e) => event(*e)?,
                    }
                }
            }
        }
    }
    require(
        validator.calls.len()
            + m.process
                .bindings
                .iter()
                .filter(|b| b.merge.is_some())
                .count()
            == m.process.bindings.len()
            && validator.forks.len() == m.process.forks.len(),
    )?;
    // Independent runtime-target guard. Do not trust a serialized static
    // certificate or the production scheduler's target/site validation.
    if let Some(meta) = &m.responsibility {
        fn count(b: &Block, cap: CapabilityId) -> usize {
            b.iter()
                .map(|s| match &s.kind {
                    Stmt::Call(c) | Stmt::Response(c, _) => usize::from(c.capability == cap),
                    Stmt::Fork { branches, .. } => {
                        branches.iter().filter(|c| c.capability == cap).count()
                    }
                    Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } => count(then_body, cap) + count(else_body, cap),
                    _ => 0,
                })
                .sum()
        }
        for s in &meta.standings {
            if let crate::contest::ContestTarget::Commit(c) = s.target {
                require(m.capabilities.get(c.0).is_some_and(|c| c.commits))?;
                require(count(&m.process.body, c) <= 1)?;
                require(!meta.contests.iter().any(|d| d.route == c))?;
            }
        }
        for d in &meta.contests {
            match d.target {
                crate::contest::ContestTarget::End(e) => require(e.0 < m.ends.len())?,
                crate::contest::ContestTarget::Commit(c) => {
                    require(m.capabilities.get(c.0).is_some_and(|c| c.commits))?
                }
            }
        }
    }
    crate::responsibility::validate(m).map_err(|_| StopReason::SemanticAbort)?;
    require(crate::contest::restricted_authority_reference(m).is_none())
}

struct Validator<'a> {
    model: &'a Model,
    limits: Limits,
    calls: BTreeSet<BindingId>,
    forks: BTreeSet<ForkId>,
}

impl Validator<'_> {
    fn result_match(&self, r: &ResultMatch) -> Execution {
        // Index existence is also implied by known/branch membership after call
        // validation; removing this local lookup alone cannot admit bad IR.
        // Keep it before indexing the binding's type (defensive fail closed).
        let b = self
            .model
            .process
            .bindings
            .get(r.binding.0)
            .ok_or(StopReason::SemanticAbort)?;
        require(r.value.0 < self.model.types[b.ty.0].members.len())
    }

    fn call(&mut self, c: &Call) -> Execution {
        let cap = self
            .model
            .capabilities
            .get(c.capability.0)
            .ok_or(StopReason::SemanticAbort)?;
        let b = self
            .model
            .process
            .bindings
            .get(c.binding.0)
            .ok_or(StopReason::SemanticAbort)?;
        require(c.args.len() == cap.params.len())?;
        for (a, p) in c.args.iter().zip(&cap.params) {
            require(if p.ty == CASE_ID_TYPE {
                *a == CURRENT_CASE
            } else {
                *a != CURRENT_CASE
                    && self
                        .model
                        .process
                        .bindings
                        .get(a.0)
                        .is_some_and(|b| b.ty == p.ty)
            })?;
        }
        require(b.merge.is_none() && b.ty == cap.return_type && self.calls.insert(c.binding))?;
        if let Some(alias) = self.model.loop_alias(c.binding) {
            require(self.model.process.bindings[alias.0].ty == b.ty)?;
            self.calls.insert(alias);
        }
        Ok(())
    }

    fn condition(
        &self,
        c: &Cond,
        known: &BTreeSet<BindingId>,
        depth: usize,
        policy: bool,
    ) -> Execution {
        // Depth limits are computational refusal, not malformed IR. The
        // reference_validation_depth_limits test checks them independently.
        if depth >= self.limits.control_depth {
            return Err(StopReason::EnumerationLimit);
        }
        match c {
            Cond::Contains { set, element } => require(
                *element == Element::CurrentCase
                    && self
                        .model
                        .state_variables
                        .get(set.0)
                        .is_some_and(|v| v.case_set_size.is_some()),
            ),
            Cond::IntCompare { variable, .. } => require(
                self.model
                    .state_variables
                    .get(variable.0)
                    .is_some_and(|v| v.integer_max.is_some() && (!policy || v.kind.is_observed())),
            ),
            Cond::StateEq(t) => {
                let v = self
                    .model
                    .state_variables
                    .get(t.variable.0)
                    .ok_or(StopReason::SemanticAbort)?;
                require(
                    v.case_set_size.is_none()
                        && reference_value_valid(self.model, v, t.value)
                        && (!policy || v.kind.is_observed()),
                )
            }
            Cond::Eq(r) => {
                self.result_match(r)?;
                require(known.contains(&r.binding))
            }
            Cond::And(cs) | Cond::Or(cs) => {
                require(cs.len() >= 2)?;
                for c in cs {
                    self.condition(c, known, depth + 1, policy)?;
                }
                Ok(())
            }
        }
    }

    fn block(
        &mut self,
        block: &Block,
        mut known: BTreeSet<BindingId>,
        depth: usize,
    ) -> Result<Option<BTreeSet<BindingId>>, StopReason> {
        if depth >= self.limits.control_depth {
            return Err(StopReason::EnumerationLimit);
        }
        let mut flows = true;
        if depth == 0 && self.model.has_loops() {
            // Lexical alias inventory; actual presence is independently checked
            // on every executed condition, like cancellation-sensitive joins.
            known.extend((0..self.model.process.bindings.len()).map(BindingId));
        }
        for s in block {
            match &s.kind {
                Stmt::MergeCopy { binding, source } => {
                    let target = self
                        .model
                        .process
                        .bindings
                        .get(binding.0)
                        .ok_or(StopReason::SemanticAbort)?;
                    let value = self
                        .model
                        .process
                        .bindings
                        .get(source.0)
                        .ok_or(StopReason::SemanticAbort)?;
                    require(
                        binding != source
                            && target.ty == value.ty
                            && target
                                .merge
                                .as_ref()
                                .is_some_and(|m| m.sources.contains(source)),
                    )?;
                    require(known.contains(source))?;
                    known.insert(*binding);
                }
                Stmt::LoopControl(control) => {
                    for c in &control.aliases {
                        self.call(c)?;
                    }
                    for (id, x) in &control.updates {
                        require(self.model.state_variables.get(id.0).is_some_and(|v| {
                            v.name.starts_with("@loop:")
                                && v.kind == StateKind::ObservedCase
                                && reference_value_valid(self.model, v, *x)
                        }))?;
                    }
                }
                Stmt::Wait(_) | Stmt::WaitUntil(_) => {}
                Stmt::End(e) => {
                    require(e.0 < self.model.ends.len())?;
                    flows = false;
                }
                Stmt::Response(c, r) => {
                    self.call(c)?;
                    known.insert(c.binding);
                    if r.attempt == 1 {
                        self.call(&r.command_call)?;
                        if let Some(body) = &r.recovery {
                            self.block(body, known.clone(), depth + 1)?;
                        }
                    }
                    known.insert(r.command);
                }
                Stmt::Call(c) | Stmt::Query(c, _) => {
                    self.call(c)?;
                    known.insert(c.binding);
                }
                Stmt::ResolveEffect { .. } => {}
                Stmt::StopReceiptUnknown => flows = false,
                Stmt::Fork { id, branches, join } => {
                    require(
                        id.0 < self.model.process.forks.len()
                            && self.forks.insert(*id)
                            && !branches.is_empty(),
                    )?;
                    for c in branches {
                        self.call(c)?;
                    }
                    for (i, call) in branches.iter().enumerate() {
                        let a = &self.model.capabilities[call.capability.0];
                        for call in &branches[i + 1..] {
                            let b = &self.model.capabilities[call.capability.0];
                            let identity = |c: &Capability| {
                                c.atomic.is_some()
                                    || c.start_writes.iter().chain(c.finish_writes()).any(|w| {
                                        matches!(
                                            w.source,
                                            WriteSource::Executor | WriteSource::InsertCase { .. }
                                        )
                                    })
                            };
                            if identity(a) || identity(b) {
                                let targets: BTreeSet<_> = a
                                    .start_writes
                                    .iter()
                                    .chain(a.finish_writes())
                                    .map(|w| w.variable)
                                    .chain(a.kernel_writes.iter().copied())
                                    .collect();
                                require(
                                    !b.start_writes
                                        .iter()
                                        .chain(b.finish_writes())
                                        .map(|w| w.variable)
                                        .chain(b.kernel_writes.iter().copied())
                                        .any(|v| targets.contains(&v)),
                                )?;
                            }
                        }
                    }
                    match join {
                        Join::All => known.extend(branches.iter().map(|c| c.binding)),
                        Join::Any {
                            matches,
                            tie,
                            if_none,
                            ..
                        } => {
                            require(if_none.0 < self.model.ends.len())?;
                            let ids: BTreeSet<_> = branches.iter().map(|c| c.binding).collect();
                            let mut matched = BTreeSet::new();
                            for r in matches {
                                self.result_match(r)?;
                                // Foreign membership is also rejected by the
                                // exact matched==ids check below. Its isolated
                                // removal therefore has no distinguishing input;
                                // keep the local guard for an immediate failure.
                                require(ids.contains(&r.binding) && matched.insert(r.binding))?;
                            }
                            require(matched == ids)?;
                            if let Tie::By(order) = tie {
                                require(
                                    order.len() == ids.len()
                                        && order.iter().copied().collect::<BTreeSet<_>>() == ids,
                                )?;
                            }
                            // Unspecified cancellation is checked when a request
                            // is possible on an enumerated path, not blanket-
                            // rejected when all branches may simply finish.
                            // Check lexical availability here. Actual result
                            // presence is checked for EVERY operand at execution:
                            // cancel_then_drain can leave a slot without a value.
                            for c in branches {
                                known.insert(c.binding);
                            }
                        }
                    }
                }
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    self.condition(condition, &known, depth, true)?;
                    let a = self.block(then_body, known.clone(), depth + 1)?;
                    let b = self.block(else_body, known.clone(), depth + 1)?;
                    match (a, b) {
                        (Some(a), Some(b)) => known = a.intersection(&b).copied().collect(),
                        (Some(a), None) | (None, Some(a)) => known = a,
                        (None, None) => flows = false,
                    }
                }
            }
        }
        Ok(flows.then_some(known))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Refused(ValueId),
    Queued,
    Running,
    Finished(ValueId),
    Cancelled,
}

#[derive(Clone, PartialEq, Eq)]
struct Task {
    frozen_performance: Option<FrozenPerformance>,
    stale: BTreeSet<StateVarId>,
    metric_ready: Option<Tick>,
    // Last age whose finish/survival was observed; not a sampled future time.
    checked_through: Option<Tick>,
    person: Option<PersonId>,
    call: Call,
    phase: Phase,
    start: Option<Tick>,
    stop: Option<Tick>,
    finish: Option<Tick>,
    ack: Option<Tick>,
}

impl Task {
    fn checked_write(&self, id: StateVarId, m: &Model) -> bool {
        let c = &m.capabilities[self.call.capability.0];
        c.version_check
            .as_ref()
            .is_some_and(|v| v.variables.contains(&id))
            || m.state_variables[id.0].last_write_wins
                && c.start_writes
                    .iter()
                    .chain(c.finish_writes())
                    .any(|w| w.variable == id)
    }
    fn terminal(&self) -> bool {
        matches!(
            self.phase,
            Phase::Finished(_) | Phase::Refused(_) | Phase::Cancelled
        )
    }
}

fn reference_stale(task: &Task, cap: &Capability) -> bool {
    cap.version_check
        .as_ref()
        .is_some_and(|v| v.variables.iter().any(|id| task.stale.contains(id)))
}

fn reference_notify(frame: &mut Frame<'_>, own: Option<usize>, writes: &[StateWrite], m: &Model) {
    for (index, task) in frame.tasks.iter_mut().enumerate() {
        if Some(index) == own || task.phase != Phase::Running {
            continue;
        }
        for write in writes {
            if task.checked_write(write.variable, m) {
                task.stale.insert(write.variable);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Occurrence {
    Merged(BindingId, BindingId, ValueId, crate::diag::Span),
    RecoveryEnter(BindingId),
    ReceiptQuery(recovery::QueryEvidence),
    ResolveEffect(recovery::Resolution),
    ResponseDestination(EndId),
    ResponseStopped(BindingId),
    ResponseInput(BindingId, Vec<ValueId>),
    Channel(response::Receipt),
    AdoptResponse(BindingId, ValueId),
    Admission,
    Refusal(CapabilityId, ValueId),
    Wait(Tick),
    Wake,
    ContestChoice(usize, bool),
    ContestQueue(usize),
    ExecutionClose,
    CancelAck(CapabilityId),
    Receipt,
    Dispatch(CapabilityId),
    Completion(CapabilityId, ValueId),
    Terminal(EndId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Observation {
    binding: Option<BindingId>,
    state_values: Vec<StateValue>,
    person: Option<PersonId>,
    at: Tick,
    event: Occurrence,
    values: Vec<Option<ValueId>>,
}

impl Observation {
    fn matches_event(&self, pattern: Event, m: &Model) -> bool {
        match &self.event {
            Occurrence::CancelAck(c) => {
                *c == pattern.capability && pattern.kind == EventKind::CancelAck
            }
            Occurrence::Dispatch(c) => *c == pattern.capability && pattern.kind == EventKind::Start,
            Occurrence::Completion(c, v) if *c == pattern.capability => match pattern.kind {
                EventKind::Finish(None) => true,
                EventKind::Finish(Some(wanted)) => *v == wanted,
                EventKind::Commit => {
                    let cap = &m.capabilities[c.0];
                    cap.commits_result(*v)
                }
                EventKind::Start | EventKind::CancelAck => false,
            },
            _ => false,
        }
    }
    fn matches_trigger(&self, t: Trigger, m: &Model) -> bool {
        match t {
            Trigger::Received => matches!(self.event, Occurrence::Receipt),
            Trigger::Admitted => matches!(self.event, Occurrence::Admission),
            Trigger::Event(e) => self.matches_event(e, m),
        }
    }
    fn matches_target(&self, t: Target, m: &Model) -> bool {
        match t {
            Target::End(e) => matches!(self.event, Occurrence::Terminal(actual) if actual == e),
            Target::Event(e) => self.matches_event(e, m),
        }
    }
}

fn reference_value_valid(m: &Model, v: &StateVariable, x: StateValue) -> bool {
    if let Some(n) = v.case_set_size {
        return (1..=64).contains(&n)
            && matches!(x, StateValue::Set(bits) if n == 64 || bits < (1u64 << n));
    }
    match (v.integer_max, x) {
        (Some(max), StateValue::Integer(n)) => n <= max,
        (None, StateValue::None) => v.optional,
        (None, StateValue::Value(x)) => {
            m.types.get(v.base.0).is_some_and(|t| x.0 < t.members.len())
        }
        _ => false,
    }
}

fn reference_membership(
    values: &[StateValue],
    set: StateVarId,
    index: usize,
) -> Result<bool, StopReason> {
    let Some(StateValue::Set(bits)) = values.get(set.0) else {
        return Err(StopReason::SemanticAbort);
    };
    let mask = 2u64
        .checked_pow(u32::try_from(index).map_err(|_| StopReason::SemanticAbort)?)
        .ok_or(StopReason::SemanticAbort)?;
    Ok(bits & mask == mask)
}

fn reference_display(m: &Model, v: &StateVariable, x: StateValue) -> crate::explain::DisplayValue {
    match x {
        StateValue::Set(bits) => crate::explain::DisplayValue::CaseSet(
            (1u64..=64)
                .filter(|i| bits & (1u64 << (i - 1)) != 0)
                .collect(),
        ),
        StateValue::None => "none".into(),
        StateValue::Integer(n) => n.to_string().into(),
        StateValue::Value(n) => m.types[v.base.0].members[n.0].clone().into(),
    }
}

fn reference_identity_write(m: &Model, c: &Capability, w: &StateWrite) -> bool {
    let Some(v) = m.state_variables.get(w.variable.0) else {
        return false;
    };
    match w.source {
        WriteSource::Executor => {
            v.kind.is_observed()
                && m.types.get(v.base.0).is_some_and(|t| {
                    t.name == "PersonId"
                        && t.members
                            .iter()
                            .map(String::as_str)
                            .eq(m.people.iter().map(|p| p.name.as_str()))
                })
                && m.actors
                    .get(c.actor.0)
                    .is_some_and(|a| a.executors.is_some())
        }
        WriteSource::InsertCase { parameter } => {
            v.case_set_size.is_some()
                && c.params
                    .get(parameter)
                    .is_some_and(|p| p.ty == CASE_ID_TYPE)
        }
        _ => false,
    }
}

fn reference_cardinality(m: &Model, v: &StateVariable) -> u128 {
    if let Some(n) = v.case_set_size {
        return 2u128.checked_pow(u32::from(n)).unwrap_or(0);
    }
    match v.integer_max {
        Some(n) => u128::from(n) + 1,
        None => m.types.get(v.base.0).map_or(0, |t| t.members.len() as u128),
    }
}
fn reference_domain<'a>(
    m: &'a Model,
    v: &'a StateVariable,
) -> impl Iterator<Item = StateValue> + 'a {
    (0..reference_cardinality(m, v)).map(move |n| {
        if v.integer_max.is_some() {
            StateValue::Integer(n as u64)
        } else {
            StateValue::Value(ValueId(n as usize))
        }
    })
}
fn reference_integer_test(n: StateValue, op: IntComparison, rhs: i128) -> Result<bool, StopReason> {
    let StateValue::Integer(n) = n else {
        return Err(StopReason::SemanticAbort);
    };
    let ordering = i128::from(n).cmp(&rhs);
    Ok(match op {
        IntComparison::Eq => ordering.is_eq(),
        IntComparison::Ne => !ordering.is_eq(),
        IntComparison::Lt => ordering.is_lt(),
        IntComparison::Le => !ordering.is_gt(),
        IntComparison::Gt => ordering.is_gt(),
        IntComparison::Ge => !ordering.is_lt(),
    })
}
fn reference_effect(
    w: &StateWrite,
    values: &[StateValue],
    result: Option<ValueId>,
) -> Result<StateValue, StopReason> {
    reference_effect_at(w, values, result, 0, None)
}

fn reference_effect_at(
    w: &StateWrite,
    values: &[StateValue],
    result: Option<ValueId>,
    case_index: usize,
    person: Option<PersonId>,
) -> Result<StateValue, StopReason> {
    Ok(match w.source {
        WriteSource::Executor => {
            StateValue::Value(ValueId(person.ok_or(StopReason::SemanticAbort)?.0))
        }
        WriteSource::InsertCase { .. } => {
            let StateValue::Set(old) = values[w.variable.0] else {
                return Err(StopReason::SemanticAbort);
            };
            let bit = 2u64
                .checked_pow(u32::try_from(case_index).map_err(|_| StopReason::SemanticAbort)?)
                .ok_or(StopReason::SemanticAbort)?;
            StateValue::Set(old | bit)
        }
        WriteSource::Constant(v) => v,
        WriteSource::Result => StateValue::Value(result.ok_or(StopReason::SemanticAbort)?),
        WriteSource::Integer {
            operator, operand, ..
        } => {
            let StateValue::Integer(n) = values[w.variable.0] else {
                return Err(StopReason::SemanticAbort);
            };
            let a = match operator {
                AssignmentOp::Assign => operand,
                AssignmentOp::Add => i128::from(n) + operand,
                AssignmentOp::Subtract => i128::from(n) - operand,
            };
            StateValue::Integer(u64::try_from(a).map_err(|_| StopReason::SemanticAbort)?)
        }
    })
}

/// Whether a successful write stays in its declared range. A replayed trace
/// that claims a successful Start or Finish with a write outside the range is
/// an illegal transition (the interpreter itself stops such a branch first).
fn reference_write_fits(m: &Model, w: &StateWrite, values: &[StateValue]) -> bool {
    match w.source {
        WriteSource::Integer { .. } => matches!(
            (
                reference_effect(w, values, None),
                m.state_variables.get(w.variable.0).and_then(|v| v.integer_max),
            ),
            (Ok(StateValue::Integer(x)), Some(upper)) if x <= upper
        ),
        _ => true,
    }
}

fn reference_kernel(k: &Kernel, m: &Model) -> Execution {
    let mut size = num_bigint::BigUint::from(1u8);
    let refs: BTreeSet<_> = k.given.iter().collect();
    require(refs.len() == k.given.len())?;
    for id in &k.given {
        let v = m
            .state_variables
            .get(id.0)
            .ok_or(StopReason::SemanticAbort)?;
        if v.name.starts_with("@loop:") && v.name.ends_with(":round") {
            require(
                v.kind == StateKind::ObservedCase
                    && !v.optional
                    && v.integer_max.is_some_and(|n| (1..=32).contains(&n)),
            )?;
            size *= num_bigint::BigUint::from(v.integer_max.unwrap());
            continue;
        }
        let domain = m.types.get(v.base.0).ok_or(StopReason::SemanticAbort)?;
        require(
            v.case_set_size.is_none()
                && domain.name != "PersonId"
                && !matches!(v.kind, StateKind::ActorObserved(_)),
        )?;
        size *= num_bigint::BigUint::from(domain.members.len() + usize::from(v.optional));
    }
    require(size == num_bigint::BigUint::from(k.rows.len()))?;
    for (i, row) in k.rows.iter().enumerate() {
        require(row.values.len() == k.given.len() && !row.branches.is_empty())?;
        require(!k.rows[..i].iter().any(|r| r.values == row.values))?;
        for (x, id) in row.values.iter().zip(&k.given) {
            require(reference_value_valid(m, &m.state_variables[id.0], *x))?;
        }
        let mut mass = Rational::zero();
        for branch in &row.branches {
            require(branch.overrides.iter().all(|(id, x)| {
                !matches!(x, StateValue::Integer(_))
                    && m.state_variables.get(id.0).is_none_or(|v| {
                        v.case_set_size.is_none()
                            && m.types.get(v.base.0).is_none_or(|t| t.name != "PersonId")
                            && v.integer_max.is_none()
                            && !matches!(v.kind, StateKind::ActorObserved(_))
                    })
            }))?;
            require(branch.probability.valid())?;
            mass.0 += &branch.probability.0;
        }
        require(mass == Rational::one())?;
    }
    Ok(())
}

fn observation_hit(
    q: &ProbabilityQuery,
    o: &Observation,
    arrival: Tick,
) -> Result<bool, StopReason> {
    if o.at < arrival || q.within.is_some_and(|d| o.at - arrival > d) {
        return Ok(false);
    }
    match &q.condition {
        ProbabilityCondition::State(c) => snapshot_condition(c, &o.values, &o.state_values),
        ProbabilityCondition::End(e) => {
            Ok(matches!(o.event,Occurrence::Terminal(actual) if actual==*e))
        }
    }
}

fn snapshot_condition(
    c: &Cond,
    values: &[Option<ValueId>],
    state_values: &[StateValue],
) -> Result<bool, StopReason> {
    snapshot_condition_at(c, values, state_values, 0)
}

fn snapshot_condition_at(
    c: &Cond,
    values: &[Option<ValueId>],
    state_values: &[StateValue],
    case_index: usize,
) -> Result<bool, StopReason> {
    match c {
        Cond::Contains { set, .. } => reference_membership(state_values, *set, case_index),
        Cond::IntCompare {
            variable,
            operator,
            value,
        } => reference_integer_test(
            *state_values
                .get(variable.0)
                .ok_or(StopReason::SemanticAbort)?,
            *operator,
            *value,
        ),
        Cond::StateEq(t) => state_values
            .get(t.variable.0)
            .map(|x| *x == t.value)
            .ok_or(StopReason::SemanticAbort),
        Cond::Eq(r) => values
            .get(r.binding.0)
            .copied()
            .flatten()
            .map(|v| v == r.value)
            .ok_or(StopReason::SemanticAbort),
        Cond::And(cs) | Cond::Or(cs) => {
            let all: Result<Vec<_>, _> = cs
                .iter()
                .map(|c| snapshot_condition_at(c, values, state_values, case_index))
                .collect();
            let all = all?;
            Ok(if matches!(c, Cond::And(_)) {
                all.into_iter().all(|v| v)
            } else {
                all.into_iter().any(|v| v)
            })
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Waiting<'a> {
    tasks: std::ops::Range<usize>,
    // None is the implicit all-join of a serial call.
    join: Option<&'a Join>,
    winner: Option<BindingId>,
}

#[derive(Clone, PartialEq, Eq)]
struct Frame<'a> {
    condition_waits: Vec<(Tick, Option<Tick>)>,
    lost_updates: BTreeSet<StateVarId>,
    state_snapshots: Vec<(Tick, Vec<StateValue>)>,
    business: Option<(EndId, Tick)>,
    state_values: Vec<StateValue>,
    trace: Vec<Observation>,
    now: Tick,
    steps: usize,
    control: Vec<&'a [Statement]>,
    waiting: Option<Waiting<'a>>,
    tasks: Vec<Task>,
    adoption: Option<Tick>,
}

impl Frame<'_> {
    fn merge_value(&self, model: &Model, binding: BindingId) -> Result<ValueId, StopReason> {
        if let Some(value) = self.trace.iter().rev().find_map(|o| match o.event {
            Occurrence::Merged(id, _, value, _) | Occurrence::AdoptResponse(id, value)
                if id == binding =>
            {
                Some(value)
            }
            _ => None,
        }) {
            return Ok(value);
        }
        let task = self
            .tasks
            .iter()
            .find(|t| t.call.binding == binding)
            .or_else(|| {
                self.tasks.iter().rev().find(|t| {
                    model.loop_alias(t.call.binding) == Some(binding)
                        && matches!(t.phase, Phase::Finished(_) | Phase::Refused(_))
                })
            })
            .ok_or(StopReason::SemanticAbort)?;
        match task.phase {
            Phase::Finished(value) | Phase::Refused(value) => Ok(value),
            _ => Err(StopReason::SemanticAbort),
        }
    }
    fn condition_enter(&mut self) {
        if self.condition_waits.last().is_none_or(|w| w.1.is_some()) {
            self.condition_waits.push((self.now, None));
        }
    }
    fn condition_pass(&mut self) -> Execution {
        if let Some(w) = self.condition_waits.last_mut()
            && w.1.is_none()
        {
            w.1 = Some(self.now);
        }
        while self.control.last().is_some_and(|b| b.is_empty()) {
            self.control.pop();
        }
        let b = self.control.last_mut().ok_or(StopReason::SemanticAbort)?;
        *b = &b[1..];
        Ok(())
    }
    fn snapshot(&mut self, model: &Model) {
        if model
            .contracts
            .iter()
            .any(|c| matches!(c.kind, ContractKind::StateInvariant { .. }))
            && self
                .trace
                .iter()
                .any(|o| matches!(o.event, Occurrence::Receipt))
            && self
                .state_snapshots
                .last()
                .is_none_or(|(_, v)| *v != self.state_values)
        {
            self.state_snapshots
                .push((self.now, self.state_values.clone()));
        }
    }
    // Reconstruct targets from this interpreter's event history, never engine state.
    fn contest_pending(&self, m: &Model) -> Option<usize> {
        m.contests().iter().enumerate().find_map(|(id, _)| {
            (self.contest_target(m, id).is_some()
                && !self
                    .trace
                    .iter()
                    .any(|o| matches!(o.event, Occurrence::ContestChoice(n, _) if n == id)))
            .then_some(id)
        })
    }

    fn contest_target(&self, m: &Model, id: usize) -> Option<&Observation> {
        self.trace.iter().find(|o| match m.contests()[id].target {
            crate::contest::ContestTarget::End(e) => matches!(o.event, Occurrence::Terminal(x) if e == x),
            // A commit is a completion on the commit branch (ADR 0035 atomic),
            // never the version_check reject value (ADR 0041: no write, no Commit).
            crate::contest::ContestTarget::Commit(c) => matches!(o.event, Occurrence::Completion(x, v) if x == c && m.capabilities[x.0].commits && m.capabilities[x.0].atomic.as_ref().is_none_or(|a| a.commit == v) && m.capabilities[x.0].version_check.as_ref().is_none_or(|k| k.reject != v)),
        })
    }
    fn ended_open(&self, m: &Model) -> bool {
        let Some(end_index) = self
            .trace
            .iter()
            .position(|o| matches!(o.event, Occurrence::Terminal(_)))
        else {
            return false;
        };
        // Include same-tick filing management following End, but not later work.
        let stable = self
            .trace
            .iter()
            .enumerate()
            .skip(end_index + 1)
            .find_map(|(i, o)| {
                (!matches!(
                    o.event,
                    Occurrence::ContestChoice(_, _) | Occurrence::ContestQueue(_)
                ))
                .then_some(i)
            })
            .unwrap_or(self.trace.len());
        m.contests().iter().enumerate().any(|(id, _)| {
            self.trace[..stable]
                .iter()
                .any(|o| matches!(o.event, Occurrence::ContestChoice(n,true) if n == id))
                && !self.trace[..stable].iter().any(|o| {
                    m.site_contest(o.binding.unwrap_or(BindingId(usize::MAX))) == Some(id)
                        && matches!(o.event, Occurrence::Completion(_, _))
                })
        })
    }
    fn intentional_wait(&self, until: u64) -> u64 {
        self.trace
            .iter()
            .filter_map(|o| match o.event {
                Occurrence::Wait(end) => Some(until.min(end).saturating_sub(o.at)),
                _ => None,
            })
            .sum()
    }
    fn sleep(&self) -> Option<Tick> {
        self.trace
            .iter()
            .rev()
            .find_map(|o| match o.event {
                Occurrence::Wait(wake) => Some(Some(wake)),
                Occurrence::Wake => Some(None),
                _ => None,
            })
            .flatten()
    }
}

struct Interpreter<'a> {
    case_index: usize,
    probability_stack: Vec<ReferenceChoices>,
    run_work: Option<path_stack::Work<'a>>,
    probability_result: Option<Vec<ReferenceBound>>,
    probability_scheduler: bool,
    probability_chance: bool,
    denominator_bits: u64,
    duration_numeric_cut: bool,
    contract_facts: Vec<(bool, bool)>,
    model: &'a Model,
    limits: Limits,
    evidence: Evidence,
    terminals: BTreeMap<EndId, TerminalOutcome>,
    statistics: Statistics,
}

mod degradation;
mod recovery;
#[cfg(test)]
pub(crate) fn r2_replay(fault: &str) -> serde_json::Value {
    recovery::replay_probe(fault)
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

#[derive(Clone)]
struct ReferenceBound {
    values: [Rational; 4],
    numeric: bool,
}
#[derive(Default)]
struct ReferenceChoices {
    owner: std::rc::Rc<std::cell::Cell<Option<usize>>>,
    children: Vec<Vec<ReferenceBound>>,
    deferred_children: Vec<usize>,
    leaf: Option<Vec<ReferenceBound>>,
}

mod path_stack;
#[cfg(test)]
#[path = "reference/round1_conditions.rs"]
mod round1_conditions;
#[cfg(test)]
#[path = "reference/round2_conditions.rs"]
mod round2_conditions;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReferenceCut {
    Horizon,
    Range,
    None,
    Clock,
    LiveTasks,
    Numeric,
}

impl<'a> Interpreter<'a> {
    // Recompute survival from declaration and observed age, without an engine
    // cursor or graph. None means the division exceeded the numeric limit.
    fn duration_coins(&self, f: &Frame<'a>) -> Result<Option<Vec<(usize, Rational)>>, StopReason> {
        let mut coins = Vec::new();
        for (i, task) in f.tasks.iter().enumerate() {
            if task.phase != Phase::Running || task.finish.is_some() {
                continue;
            }
            let Some(distribution) =
                &self.model.capabilities[task.call.capability.0].duration_distribution
            else {
                continue;
            };
            let age = f
                .now
                .checked_sub(task.start.ok_or(StopReason::SemanticAbort)?)
                .ok_or(StopReason::SemanticAbort)?;
            if task.checked_through.is_some_and(|a| a >= age) {
                continue;
            }
            let mut at_this_age = Rational::zero();
            let mut alive = Rational::zero();
            for (minute, weight) in &distribution.entries {
                if *minute >= age {
                    alive.0 += &weight.0;
                    if !alive.within_bits(self.denominator_bits) {
                        return Ok(None);
                    }
                }
                if *minute == age {
                    at_this_age = weight.clone();
                }
            }
            if at_this_age.is_zero() {
                continue;
            }
            require(!alive.is_zero())?;
            let Some(q) = reference_duration_divide(&at_this_age, &alive, self.denominator_bits)
            else {
                return Ok(None);
            };
            coins.push((i, q));
        }
        Ok(Some(coins))
    }

    fn duration_tree(
        &mut self,
        mut f: Frame<'a>,
        coins: &[(usize, Rational)],
        depth: usize,
    ) -> Execution {
        let mut leaves = 1usize;
        for (_, q) in coins {
            if *q != Rational::one() {
                leaves = leaves.checked_mul(2).ok_or(StopReason::EnumerationLimit)?;
            }
        }
        // Reserve the entire layer, independently of the engine's Room. No
        // intermediate Frame is cloned: recurse over mutations of one scratch
        // frame, and clone only fully observed leaves.
        if depth + 1 > self.limits.branch_depth
            || self
                .statistics
                .frames
                .checked_add(leaves)
                .is_none_or(|n| n > self.limits.frames)
        {
            return Err(StopReason::EnumerationLimit);
        }
        self.statistics.frames += leaves;
        self.probability_chance = true;
        self.duration_bernoulli(&mut f, coins, 0, depth, Rational::one())
    }

    fn duration_bernoulli(
        &mut self,
        f: &mut Frame<'a>,
        coins: &[(usize, Rational)],
        at: usize,
        depth: usize,
        weight: Rational,
    ) -> Execution {
        if at == coins.len() {
            #[cfg(test)]
            {
                self.statistics.duration_copies += 1;
                self.statistics.duration_leaves.push((
                    f.now,
                    f.tasks.iter().map(|t| t.finish).collect(),
                    weight,
                ));
            }
            #[cfg(not(test))]
            let _ = weight;
            #[cfg(test)]
            if self.statistics.duration_probe {
                self.probability_stack.push(ReferenceChoices::default());
                self.probability_leaf(f, None, ReferenceCut::None)?;
                self.probability_finish(None);
                return Ok(());
            }
            return self.run(f.clone(), depth + 1);
        }
        let (i, q) = &coins[at];
        let old_checked = f.tasks[*i].checked_through;
        let age = f.now - f.tasks[*i].start.ok_or(StopReason::SemanticAbort)?;
        f.tasks[*i].checked_through = Some(age);
        self.probability_stack.push(ReferenceChoices::default());
        let mut weights = Vec::new();
        // A nested Bernoulli path tree, not a joint-state graph enumeration.
        for (finish, p) in [
            (true, q.clone()),
            (false, Rational(Rational::one().0 - &q.0)),
        ] {
            if p.is_zero() {
                continue;
            }
            let product = Rational(&weight.0 * &p.0);
            f.tasks[*i].finish = finish.then_some(f.now);
            if !product.within_bits(self.denominator_bits) {
                // Reduce this numeric leaf as a separate child.
                self.probability_stack.push(ReferenceChoices::default());
                self.probability_leaf(f, None, ReferenceCut::Numeric)?;
                self.probability_finish(None);
            } else {
                self.duration_bernoulli(f, coins, at + 1, depth, product)?;
            }
            weights.push(p);
        }
        f.tasks[*i].finish = None;
        f.tasks[*i].checked_through = old_checked;
        self.probability_finish(Some(&weights));
        Ok(())
    }

    fn probability_initial(
        &mut self,
        mut frame: Frame<'a>,
        vars: &[usize],
        at: usize,
    ) -> Execution {
        if at == vars.len() {
            frame.trace.push(Observation {
                binding: None,
                state_values: frame.state_values.clone(),
                person: None,
                at: 0,
                event: Occurrence::Receipt,
                values: vec![None; self.model.process.bindings.len()],
            });
            #[cfg(test)]
            {
                self.statistics
                    .initial_tuples
                    .push(frame.state_values.clone());
            }
            frame.snapshot(self.model);
            return self.run(frame, 0);
        }
        let i = vars[at];
        let v = &self.model.state_variables[i];
        self.probability_stack.push(ReferenceChoices::default());
        let weights = if let InitialValue::Distribution(id) = v.initial {
            let mut weights = Vec::new();
            for (x, p) in &self.model.initial_distributions[id] {
                if p.is_zero() {
                    continue;
                }
                let mut next = self.successor(&frame, 0)?;
                #[cfg(test)]
                {
                    self.statistics.initial_copies += 1;
                }
                next.state_values[i] = *x;
                self.probability_initial(next, vars, at + 1)?;
                weights.push(p.clone());
            }
            Some(weights)
        } else {
            for value in reference_domain(self.model, v) {
                let mut next = self.successor(&frame, 0)?;
                #[cfg(test)]
                {
                    self.statistics.initial_copies += 1;
                }
                next.state_values[i] = value;
                self.probability_initial(next, vars, at + 1)?;
            }
            None
        };
        self.probability_finish(weights.as_deref());
        Ok(())
    }

    #[cfg(test)]
    fn review_numeric_cut_reason() {
        let source = r#"model M version "0.2" {
          type T = enum { ok }
          actor A { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } }
          process P { a = A.run(); end done }
          bounds { cases 1; live_tasks 2; clock 10min }
          contract C { response delivery: case.received -> one_of(end.done) by 5min after case.received otherwise unresolved }
        }"#;
        let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
        let mut i = replay::interpreter(&m);
        let mut f = replay::initial(&m);
        i.record(&mut f, Occurrence::Receipt, None).unwrap();
        i.probability_stack.push(ReferenceChoices::default());
        i.probability_leaf(&f, None, ReferenceCut::Numeric).unwrap();
        let result = &i.statistics.obligations[0].outcomes[0];
        assert_eq!(result.status, crate::obligation::Status::Unresolved);
        assert_eq!(result.reason.as_deref(), Some("state_budget"));
    }

    fn collect_contests(&mut self, f: &Frame<'a>, cut: ReferenceCut, ended: bool) -> Execution {
        use crate::contest::{Alternative, IntegerRange, RuntimeStatus};
        let mut open = false;
        for (id, declaration) in self.model.contests().iter().enumerate() {
            let Some(target) = f.contest_target(self.model, id) else {
                continue;
            };
            let at = target.at;
            let filed = f.trace.iter().find_map(|o| match o.event {
                Occurrence::ContestChoice(n, filed) if n == id => Some(filed),
                _ => None,
            });
            let Some(filed) = filed else {
                continue;
            };
            let task = f
                .tasks
                .iter()
                .find(|t| self.model.site_contest(t.call.binding) == Some(id));
            let response = task.and_then(|t| match t.phase {
                Phase::Finished(v) => t.stop.map(|stop| (stop, v)),
                _ => None,
            });
            let deadline = at
                .checked_add(declaration.respond_by)
                .ok_or(StopReason::SemanticAbort)?;
            let cap = &self.model.capabilities[declaration.route.0];
            let allowed = cap
                .permission
                .as_ref()
                .or(self.model.actors[cap.actor.0].executors.as_ref());
            let permission = task.is_some_and(|t| {
                t.start.is_some()
                    && allowed.is_some_and(|ps| t.person.is_none_or(|p| !ps.contains(&p)))
            });
            let violation = if !filed {
                None
            } else if !crate::responsibility::included(self.model, cap.actor) {
                Some("missing_route")
            } else if permission {
                Some("permission")
            } else if response.is_some_and(|(t, _)| t > deadline) {
                Some("late_response")
            } else if task.is_some_and(|t| t.phase == Phase::Cancelled) {
                Some("cancelled")
            } else if task.is_some_and(|t| matches!(t.phase, Phase::Refused(_))) {
                Some("refused")
            } else if response.is_none() && cut == ReferenceCut::None {
                Some("deadlock")
            } else {
                None
            };
            let closed = ended && response.is_some() || !filed;
            open |= !closed;
            let range = |n| Some(IntegerRange { min: n, max: n });
            let observed_at = if cut == ReferenceCut::Clock {
                self.model.bounds.clock
            } else {
                f.now
            };
            let unresolved_cut = filed && response.is_none() && cut != ReferenceCut::None;
            let work = if task.is_some_and(|t| matches!(t.phase, Phase::Refused(_))) {
                Some(0)
            } else {
                task.and_then(|t| t.start.map(|start| t.stop.unwrap_or(observed_at) - start))
            };
            let mut cost = crate::metrics::Range {
                min: None,
                max: None,
                reason: Some(if task.is_none_or(|t| t.start.is_none()) {
                    crate::metrics::Reason::NeverCreated
                } else {
                    crate::metrics::Reason::NotConfigured
                }),
            };
            if task.is_some_and(|t| t.start.is_some())
                && let Some(spec) = &self.model.metrics
                && let (Some(rate), Some(start)) = (
                    spec.actor_rates[cap.actor.0],
                    spec.start_rates[declaration.route.0],
                )
            {
                let total = num_bigint::BigInt::from(rate)
                    * num_bigint::BigInt::from(work.unwrap_or(0))
                    + num_bigint::BigInt::from(if task.is_some_and(|t| t.start.is_some()) {
                        start
                    } else {
                        0
                    });
                let n =
                    crate::metrics::MetricNumber(num_rational::BigRational::from_integer(total));
                cost = crate::metrics::Range {
                    min: Some(n.clone()),
                    max: Some(n),
                    reason: None,
                };
            }
            let alternative = Alternative {
                filed,
                target_at: at,
                filed_at: filed.then_some(at),
                deadline: filed.then_some(deadline),
                status: if !filed {
                    RuntimeStatus::NotFiled
                } else if violation.is_some() {
                    RuntimeStatus::Violated
                } else if response.is_some() {
                    RuntimeStatus::Responded
                } else {
                    RuntimeStatus::Unresolved
                },
                response_at: response.map(|(t, _)| t),
                closed_at: if filed {
                    response.map(|(t, _)| t)
                } else {
                    Some(at)
                },
                executor: task
                    .and_then(|t| t.person)
                    .map(|p| self.model.people[p.0].name.clone()),
                result: response
                    .map(|(_, v)| self.model.types[cap.return_type.0].members[v.0].clone()),
                violation_reason: violation
                    .or_else(|| {
                        (unresolved_cut
                            && (deadline < observed_at
                                || (matches!(cut, ReferenceCut::Clock | ReferenceCut::Horizon)
                                    && deadline == observed_at)))
                            .then_some("deadline_passed_unresolved")
                    })
                    .map(String::from),
                route_work: if unresolved_cut {
                    None
                } else {
                    work.and_then(range)
                },
                route_wait: if unresolved_cut {
                    None
                } else {
                    task.and_then(|t| range(t.start.unwrap_or(observed_at) - at))
                },
                route_cost: cost,
            };
            let alternatives = self.statistics.contest_alternatives.entry(id).or_default();
            if !alternatives.contains(&alternative) {
                alternatives.push(alternative);
            }
        }
        if crate::contest::has_effects(self.model) {
            open = f.ended_open(self.model);
        }
        if cut == ReferenceCut::None {
            let n = u64::from(open);
            if let Some(range) = &mut self.statistics.contest_open {
                extend(range, n);
            } else {
                self.statistics.contest_open = Some(singleton(n));
            }
        }
        Ok(())
    }

    fn probability_leaf(
        &mut self,
        f: &Frame<'a>,
        end: Option<EndId>,
        cut: ReferenceCut,
    ) -> Execution {
        if self.run_work.is_some() {
            return self.defer_in_place(f, move |this, f| this.probability_leaf_ready(f, end, cut));
        }
        self.probability_leaf_ready(f, end, cut)
    }

    fn probability_leaf_ready(
        &mut self,
        f: &Frame<'a>,
        end: Option<EndId>,
        cut: ReferenceCut,
    ) -> Execution {
        self.collect_contests(f, cut, end.is_some())?;
        if end.is_none() {
            self.collect_obligations(
                f,
                match cut {
                    ReferenceCut::Range => "state_range_violation",
                    ReferenceCut::Clock => "clock",
                    ReferenceCut::Horizon => "horizon",
                    ReferenceCut::LiveTasks => "live_tasks",
                    ReferenceCut::Numeric => "state_budget",
                    ReferenceCut::None => "deadlock_unresolved",
                },
            )?;
        }
        if self.model.metrics.is_some() && self.statistics.projection == Projection::Full {
            let point = metrics::project(
                self.model,
                &[metrics::Observation {
                    condition_intervals: f.condition_waits.clone(),
                    admitted_at: Some(0),
                    wait_intervals: f
                        .trace
                        .iter()
                        .filter_map(|o| match o.event {
                            Occurrence::Wait(end) => Some((o.at, end)),
                            _ => None,
                        })
                        .collect(),
                    intentional_wait: f.intentional_wait(f.now.min(self.model.bounds.clock)),
                    tasks: &f.tasks,
                    arrival: 0,
                    terminal: f.business.or_else(|| end.map(|e| (e, f.now))),
                    rejected: false,
                    deadlock: end.is_none() && cut == ReferenceCut::None,
                    future: false,
                }],
                f.now,
            );
            self.statistics
                .metric_envelope
                .get_or_insert_with(metrics::Envelope::default)
                .add(point);
        }
        if cut == ReferenceCut::Numeric {
            self.duration_numeric_cut = true;
            self.evidence.truncated = true;
        }
        let reached_end = f.business.map(|(e, _)| e).or(end);
        self.statistics.completion_bad |= reached_end.is_none() && cut == ReferenceCut::None;
        self.statistics.completion_unknown |= reached_end.is_none() && cut != ReferenceCut::None;
        if !self.model.uses_probability() && !self.model.completion_requested() {
            return Ok(());
        }
        let mut result = Vec::new();
        for q in &self.model.probability_queries {
            let mut hit = matches!(q.condition,ProbabilityCondition::End(e) if end==Some(e) && q.within.is_none_or(|d|f.now<=d));
            // A compiler control update can immediately precede End without
            // creating a Task/event. Observe that final valid snapshot too;
            // a clock cut beyond the bound is not an observation at that time.
            if self.model.has_loops()
                && f.now <= self.model.bounds.clock
                && q.within.is_none_or(|d| f.now <= d)
                && let ProbabilityCondition::State(c) = &q.condition
            {
                hit |= snapshot_condition(
                    c,
                    &response::visible_values(f, self.model, None),
                    &f.state_values,
                )?;
            }
            for o in &f.trace {
                hit |= observation_hit(q, o, 0)?;
            }
            // A cut after the deadline cannot change a within query: every
            // observation at or before d is already in the trace.
            let uncertain = !(matches!(q.condition, ProbabilityCondition::End(_))
                && reached_end.is_some())
                && cut != ReferenceCut::None
                && !q.within.is_some_and(|d| {
                    (cut != ReferenceCut::Numeric && f.now > d)
                        || (cut == ReferenceCut::Clock && d <= self.model.bounds.clock)
                });
            result.push(ReferenceBound {
                values: std::array::from_fn(|k| {
                    if hit || (uncertain && k % 2 == 1) {
                        Rational::one()
                    } else {
                        Rational::zero()
                    }
                }),
                numeric: cut == ReferenceCut::Numeric && !hit,
            });
        }
        for i in 0..self.model.ends.len() {
            let reached_end = f.business.map(|(e, _)| e).or(end);
            result.push(ReferenceBound {
                values: std::array::from_fn(|k| {
                    if reached_end == Some(EndId(i))
                        || (reached_end.is_none() && cut != ReferenceCut::None && k % 2 == 1)
                    {
                        Rational::one()
                    } else {
                        Rational::zero()
                    }
                }),
                numeric: cut == ReferenceCut::Numeric,
            });
        }
        if self.model.completion_requested() {
            result.push(ReferenceBound {
                values: std::array::from_fn(|k| {
                    if reached_end.is_some() || (cut != ReferenceCut::None && k % 2 == 1) {
                        Rational::one()
                    } else {
                        Rational::zero()
                    }
                }),
                numeric: reached_end.is_none() && cut == ReferenceCut::Numeric,
            });
        }
        self.probability_stack
            .last_mut()
            .ok_or(StopReason::SemanticAbort)?
            .leaf = Some(result);
        Ok(())
    }

    // Independent path-tree reduction. No graph, interned state, or engine DP.
    fn probability_finish(&mut self, weights: Option<&[Rational]>) {
        let choices = self.probability_stack.pop().unwrap();
        if let Some(work) = &mut self.run_work {
            let id = work.choice(choices, weights.map(<[Rational]>::to_vec));
            self.probability_stack
                .last_mut()
                .expect("path reduction parent")
                .deferred_children
                .push(id);
            return;
        }
        let result = self.probability_reduce(choices, weights);
        self.probability_publish(result);
    }

    fn probability_reduce(
        &mut self,
        choices: ReferenceChoices,
        weights: Option<&[Rational]>,
    ) -> Vec<ReferenceBound> {
        debug_assert!(choices.deferred_children.is_empty());
        if let Some(leaf) = choices.leaf {
            leaf
        } else {
            let count = self.model.probability_queries.len()
                + self.model.ends.len()
                + usize::from(self.model.completion_requested());
            let mut result = Vec::new();
            self.probability_scheduler |= weights.is_none() && choices.children.len() > 1;
            for question in 0..count {
                let mut numeric = choices.children.iter().any(|c| c[question].numeric);
                let values = if let Some(ps) = weights {
                    std::array::from_fn(|k| {
                        let mut sum = Rational::zero();
                        for (p, c) in ps.iter().zip(&choices.children) {
                            let product = Rational(&p.0 * &c[question].values[k].0);
                            numeric |= !product.within_bits(self.denominator_bits);
                            sum.0 += product.0;
                            numeric |= !sum.within_bits(self.denominator_bits);
                        }
                        sum
                    })
                } else {
                    std::array::from_fn(|k| {
                        let values = choices
                            .children
                            .iter()
                            .map(|c| c[question].values[k].clone());
                        if k < 2 {
                            values.min().unwrap_or_else(Rational::zero)
                        } else {
                            values.max().unwrap_or_else(Rational::zero)
                        }
                    })
                };
                result.push(ReferenceBound {
                    values: if numeric {
                        [
                            Rational::zero(),
                            Rational::one(),
                            Rational::zero(),
                            Rational::one(),
                        ]
                    } else {
                        values
                    },
                    numeric,
                });
            }
            result
        }
    }

    fn probability_publish(&mut self, result: Vec<ReferenceBound>) {
        if let Some(parent) = self.probability_stack.last_mut() {
            parent.children.push(result);
        } else {
            self.probability_result = Some(result);
        }
    }
    fn initialize(&mut self, mut frame: Frame<'a>, index: usize) -> Execution {
        if self.model.uses_probability() {
            let vars: Vec<_> = self
                .model
                .state_variables
                .iter()
                .enumerate()
                .filter_map(|(i, v)| {
                    matches!(v.initial, InitialValue::Distribution(_)).then_some(i)
                })
                .chain(
                    self.model
                        .state_variables
                        .iter()
                        .enumerate()
                        .filter_map(|(i, v)| (v.initial == InitialValue::Nondet).then_some(i)),
                )
                .collect();
            return self.probability_initial(frame, &vars, 0);
        }
        if !self
            .model
            .state_variables
            .iter()
            .any(|v| v.initial == InitialValue::Nondet)
        {
            frame.trace.push(Observation {
                binding: None,
                state_values: frame.state_values.clone(),
                person: None,
                at: 0,
                event: Occurrence::Receipt,
                values: vec![None; self.model.process.bindings.len()],
            });
            frame.snapshot(self.model);
            return self.run(frame, 0);
        }
        self.initial_choices(&frame, &mut frame.state_values.clone(), index)
    }

    // Reference-only recursive traversal, no engine seed/cursor/Room is reused.
    fn initial_choices(
        &mut self,
        seed: &Frame<'a>,
        values: &mut [StateValue],
        index: usize,
    ) -> Execution {
        if index > self.limits.control_depth {
            return Err(StopReason::EnumerationLimit);
        }
        if index == self.model.state_variables.len() {
            self.reserve(0)?;
            #[cfg(test)]
            {
                self.statistics.initial_copies += 1;
                self.statistics.initial_tuples.push(values.to_vec());
            }
            let mut frame = seed.clone();
            frame.state_values = values.to_vec();
            frame.trace.push(Observation {
                binding: None,
                state_values: values.to_vec(),
                person: None,
                at: 0,
                event: Occurrence::Receipt,
                values: vec![None; self.model.process.bindings.len()],
            });
            frame.snapshot(self.model);
            return self.run(frame, 0);
        }
        let v = &self.model.state_variables[index];
        if v.initial == InitialValue::Nondet {
            for value in reference_domain(self.model, v) {
                values[index] = value;
                self.initial_choices(seed, values, index + 1)?;
            }
            Ok(())
        } else {
            self.initial_choices(seed, values, index + 1)
        }
    }

    fn finish_state(&self, frame: &mut Frame<'a>, task: usize, result: ValueId) {
        let cap = &self.model.capabilities[frame.tasks[task].call.capability.0];
        if reference_stale(&frame.tasks[task], cap) {
            return;
        }
        let writes = cap.atomic.as_ref().map_or(cap.writes.as_slice(), |a| {
            if result == a.commit {
                a.writes.as_slice()
            } else {
                &[]
            }
        });
        let before = frame.state_values.clone();
        for assignment in writes {
            if self.model.state_variables[assignment.variable.0].last_write_wins
                && frame.tasks[task].stale.contains(&assignment.variable)
                && matches!(
                    assignment.source,
                    WriteSource::Constant(_)
                        | WriteSource::Result
                        | WriteSource::Executor
                        | WriteSource::Integer {
                            operator: AssignmentOp::Assign,
                            ..
                        }
                )
            {
                frame.lost_updates.insert(assignment.variable);
            }
            frame.state_values[assignment.variable.0] = reference_effect_at(
                assignment,
                &before,
                Some(result),
                self.case_index,
                frame.tasks[task].person,
            )
            .expect("range checked atomic finish");
        }
        reference_notify(frame, Some(task), writes, self.model);
    }

    fn range_attempt(
        &mut self,
        f: &Frame<'a>,
        task: usize,
        start: bool,
        case_id: Option<u64>,
    ) -> Result<bool, StopReason> {
        let call = &f.tasks[task].call;
        let cap = &self.model.capabilities[call.capability.0];
        if !start && reference_stale(&f.tasks[task], cap) {
            return Ok(false);
        }
        let mut failed = false;
        for w in if start {
            &cap.start_writes
        } else if let Some(a) = &cap.atomic {
            if self.condition(f, &a.condition, 0)? == a.commit_when {
                &a.writes
            } else {
                return Ok(false);
            }
        } else {
            &cap.writes
        } {
            if let WriteSource::Integer {
                operator,
                operand,
                span,
            } = w.source
            {
                let v = &self.model.state_variables[w.variable.0];
                let StateValue::Integer(before) = f.state_values[w.variable.0] else {
                    return Err(StopReason::SemanticAbort);
                };
                let attempted = match operator {
                    AssignmentOp::Assign => operand,
                    AssignmentOp::Add => operand + i128::from(before),
                    AssignmentOp::Subtract => i128::from(before) - operand,
                };
                let upper = v.integer_max.ok_or(StopReason::SemanticAbort)?;
                if !(0..=i128::from(upper)).contains(&attempted) {
                    failed = true;
                    let witness = StateRangeViolation {
                        case_id,
                        time: f.now,
                        binding: self.model.site_name(call.binding),
                        actor: self.model.actors[cap.actor.0].name.clone(),
                        capability: cap.name.clone(),
                        phase: if start { "start" } else { "finish" }.into(),
                        variable: match v.kind {
                            StateKind::ActorObserved(a) => {
                                format!("{}.{}", self.model.actors[a.0].name, v.name)
                            }
                            _ => v.name.clone(),
                        },
                        operator: match operator {
                            AssignmentOp::Assign => "=",
                            AssignmentOp::Add => "+=",
                            AssignmentOp::Subtract => "-=",
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
                    if !self.statistics.ranges.iter().any(|r| {
                        r.case_id == witness.case_id
                            && r.binding == witness.binding
                            && r.phase == witness.phase
                            && r.variable == witness.variable
                    }) {
                        self.statistics.ranges.push(witness);
                    }
                }
            }
        }
        Ok(failed)
    }

    fn record(
        &mut self,
        f: &mut Frame<'a>,
        event: Occurrence,
        person: Option<PersonId>,
    ) -> Execution {
        self.record_with_site(f, event, person, None)
    }

    fn record_with_site(
        &mut self,
        f: &mut Frame<'a>,
        event: Occurrence,
        person: Option<PersonId>,
        binding: Option<BindingId>,
    ) -> Execution {
        #[cfg(test)]
        work_profiles::observe(f.now, self.model, &event);
        match event {
            Occurrence::Dispatch(c) => {
                self.statistics.observed.capabilities.insert(c);
                if let Some(p) = person {
                    self.statistics.observed.people.insert(p);
                }
            }
            Occurrence::Terminal(e) => {
                self.statistics.observed.ends.insert(e);
            }
            _ => {}
        }
        if self.model.contracts.is_empty()
            && self.model.probability_queries.is_empty()
            && !self.model.has_metric_wait()
            && !self.model.has_requires()
            && self.model.response_attempts().is_empty()
            && !matches!(event, Occurrence::Merged(..))
        {
            return Ok(());
        }
        let values = response::visible_values(f, self.model, Some(&event));
        f.trace.push(Observation {
            binding,
            state_values: f.state_values.clone(),
            person,
            at: f.now,
            event,
            values,
        });
        f.snapshot(self.model);
        let result = self.inspect_trace(f, f.now, false, false, false);
        if result.is_err() {
            let _ = self.collect_obligations(f, "aborted");
        }
        result
    }

    fn record_site(
        &mut self,
        f: &mut Frame<'a>,
        event: Occurrence,
        person: Option<PersonId>,
        binding: BindingId,
    ) -> Execution {
        self.record_with_site(f, event, person, Some(binding))
    }

    // Re-evaluate each trigger against its suffix, never maintain the engine's
    // pending-deadline automaton. Trace indices preserve same-tick bundle order.
    fn collect_obligations(&mut self, f: &Frame<'a>, reason: &str) -> Execution {
        use crate::obligation::{Coverage, Obligation, Resolution, Status};
        if !self.model.tracks_obligations() {
            return Ok(());
        }
        let text = |e: Event| {
            let cap = &self.model.capabilities[e.capability.0];
            let kind = match e.kind {
                EventKind::Start => "start".into(),
                EventKind::Commit => "commit".into(),
                EventKind::CancelAck => "cancel_ack".into(),
                EventKind::Finish(None) => "finish".into(),
                EventKind::Finish(Some(v)) => format!(
                    "finish({})",
                    self.model.types[cap.return_type.0].members[v.0]
                ),
            };
            format!(
                "{}.{}.{kind}",
                self.model.actors[cap.actor.0].name, cap.name
            )
        };
        for (i, clause) in self.model.contracts.iter().enumerate() {
            let Some((trigger, targets, within)) = clause.response() else {
                continue;
            };
            for (index, o) in f
                .trace
                .iter()
                .enumerate()
                .filter(|(_, o)| o.matches_trigger(trigger, self.model))
            {
                let Some(deadline) = o.at.checked_add(within) else {
                    continue;
                };
                let mut result = None;
                for t in &f.trace[index..] {
                    if t.at > deadline {
                        break;
                    }
                    if targets.iter().any(|p| t.matches_target(*p, self.model)) {
                        result = Some((Status::Fulfilled, Some(t.at), None, None, None, None));
                        break;
                    }
                    if let Some((n, w, authority, why)) =
                        self.model.contracts.iter().enumerate().find_map(|(n, w)| {
                            if let ContractKind::Waive {
                                response,
                                event,
                                reason,
                                authority,
                            } = &w.kind
                                && response.0 == i
                                && t.matches_event(*event, self.model)
                            {
                                return Some((n, w, *authority, reason.clone()));
                            }
                            None
                        })
                    {
                        let p = t.person.ok_or(StopReason::SemanticAbort)?;
                        let ContractKind::Waive { event, .. } = w.kind else {
                            unreachable!()
                        };
                        require(
                            self.model.authorities[authority.0].grants.iter().any(|g| {
                                g.capability == event.capability && g.persons.contains(&p)
                            }),
                        )?;
                        result = Some((
                            Status::Waived,
                            Some(t.at),
                            Some(why),
                            Some(w.property_id(ClauseId(n))),
                            Some(self.model.people[p.0].name.clone()),
                            Some(self.model.authorities[authority.0].name.clone()),
                        ));
                        break;
                    }
                    if matches!(t.event, Occurrence::Terminal(_)) {
                        result = Some((
                            Status::Violated,
                            Some(t.at),
                            Some("terminal_unresolved".into()),
                            None,
                            None,
                            None,
                        ));
                        break;
                    }
                }
                let (status, resolved_at, why, by_clause, executor, authority) = result
                    .unwrap_or_else(|| {
                        if deadline < f.now
                            || (reason == "clock" && deadline <= self.model.bounds.clock)
                            || (reason == "horizon"
                                && self.model.horizon.is_some_and(|d| deadline <= d))
                        {
                            (
                                Status::Violated,
                                Some(deadline),
                                Some("expiry".into()),
                                None,
                                None,
                                None,
                            )
                        } else if reason.ends_with("_unresolved") {
                            (
                                Status::Violated,
                                Some(f.now),
                                Some(reason.into()),
                                None,
                                None,
                                None,
                            )
                        } else {
                            (
                                Status::Unresolved,
                                None,
                                Some(reason.into()),
                                None,
                                None,
                                None,
                            )
                        }
                    });
                let outcome = Resolution {
                    status,
                    triggered_at: o.at,
                    deadline,
                    resolved_at,
                    reason: why,
                    by_clause,
                    executor,
                    authority,
                };
                let site = match trigger {
                    Trigger::Received => "received".into(),
                    Trigger::Admitted => "admitted".into(),
                    Trigger::Event(_) => self
                        .model
                        .site_name(o.binding.ok_or(StopReason::SemanticAbort)?),
                };
                let id = format!("case1:response{}:{site}", clause.ordinal.unwrap_or(i + 1));
                if let Some(row) = self.statistics.obligations.iter_mut().find(|r| r.id == id) {
                    if !row.outcomes.contains(&outcome) {
                        row.outcomes.push(outcome);
                        row.outcomes.sort();
                    }
                } else {
                    let (label, non_waivable) = match &clause.kind {
                        ContractKind::NamedResponse {
                            label,
                            non_waivable,
                            ..
                        } => (label.clone(), *non_waivable),
                        _ => (None, false),
                    };
                    self.statistics.obligations.push(Obligation {
                        id,
                        response_id: clause.property_id(ClauseId(i)),
                        response_label: label,
                        case_id: 1,
                        trigger_site: site,
                        trigger: match trigger {
                            Trigger::Received => "case.received".into(),
                            Trigger::Admitted => "case.admitted".into(),
                            Trigger::Event(e) => text(e),
                        },
                        targets: targets
                            .iter()
                            .map(|t| match t {
                                Target::End(e) => format!("end.{}", self.model.ends[e.0]),
                                Target::Event(e) => text(*e),
                            })
                            .collect(),
                        debtor: crate::obligation::debtor(clause, self.model).0,
                        debtor_status: crate::obligation::debtor(clause, self.model).1,
                        non_waivable,
                        coverage: Coverage::Complete,
                        outcomes: vec![outcome],
                    });
                }
            }
        }
        self.statistics.obligations.sort_by_key(|r| {
            (
                self.model
                    .contracts
                    .iter()
                    .enumerate()
                    .position(|(i, c)| c.property_id(ClauseId(i)) == r.response_id)
                    .unwrap_or(usize::MAX),
                self.model
                    .process
                    .bindings
                    .iter()
                    .position(|b| b.name == r.trigger_site)
                    .map_or(0, |i| i + 1),
            )
        });
        Ok(())
    }

    /// `deadlocked`: this closed leaf has no enabled transition (not a normal
    /// close). A filed route without a response is then violated, as in the
    /// engine's `contest_deadlock`, even when the route is still queued.
    fn inspect_trace(
        &mut self,
        f: &Frame<'a>,
        horizon: Tick,
        inclusive: bool,
        closed: bool,
        deadlocked: bool,
    ) -> Execution {
        // Finish inspecting the recorded trace for every clause even when one
        for o in &f.trace {
            if let Occurrence::Refusal(capability, value) = o.event {
                let binding = o.binding.ok_or(StopReason::SemanticAbort)?;
                let cap = &self.model.capabilities[capability.0];
                self.statistics
                    .refusals
                    .entry(binding)
                    .and_modify(|r| {
                        r.at.min = r.at.min.min(o.at);
                        r.at.max = r.at.max.max(o.at);
                    })
                    .or_insert_with(|| CapabilityRefusal {
                        case_id: 1,
                        call: self.model.site_name(binding),
                        capability: crate::responsibility::cap_name(self.model, capability),
                        result: self.model.types[cap.return_type.0].members[value.0].clone(),
                        at: RefusalRange {
                            min: o.at,
                            max: o.at,
                        },
                        coverage: "complete".into(),
                        observed_count: None,
                        samples: None,
                    });
            }
        }
        // cannot be evaluated. An abort must not hide another clause's witness.
        let mut aborted = false;
        for (i, clause) in self.model.contracts.iter().enumerate() {
            let facts = &mut self.contract_facts[i];
            match &clause.kind {
                ContractKind::ContestResponse { contest } => {
                    let declaration = &self.model.contests()[*contest];
                    if let Some(target) = f.contest_target(self.model, *contest) {
                        facts.0 = true;
                        let filed = f.trace.iter().any(|o| matches!(o.event, Occurrence::ContestChoice(id, true) if id == *contest));
                        if filed {
                            let deadline = target.at.checked_add(declaration.respond_by);
                            let response = f.trace.iter().find(|o| {
                                self.model
                                    .site_contest(o.binding.unwrap_or(BindingId(usize::MAX)))
                                    == Some(*contest)
                                    && matches!(o.event, Occurrence::Completion(_, _))
                            });
                            let cap = &self.model.capabilities[declaration.route.0];
                            let allowed = cap
                                .permission
                                .as_ref()
                                .or(self.model.actors[cap.actor.0].executors.as_ref());
                            let denied = f.trace.iter().any(|o| {
                                self.model
                                    .site_contest(o.binding.unwrap_or(BindingId(usize::MAX)))
                                    == Some(*contest)
                                    && matches!(o.event, Occurrence::Dispatch(_))
                                    && allowed
                                        .is_some_and(|ps| o.person.is_none_or(|p| !ps.contains(&p)))
                            });
                            facts.1 |= denied
                                || !crate::responsibility::included(self.model, cap.actor)
                                || deadline.is_none_or(|d| {
                                    response.map_or(
                                        (closed
                                            && (deadlocked
                                                || !crate::contest::has_effects(self.model)
                                                || (f.business.is_some()
                                                    && f.tasks.iter().all(Task::terminal))))
                                            || d < horizon
                                            || (inclusive && d == horizon),
                                        |o| o.at > d,
                                    )
                                });
                        }
                    }
                }
                ContractKind::Excludes { before, start } => {
                    facts.0 |= f.trace.iter().any(|o| o.matches_event(*before, self.model));
                    for (index, o) in f.trace.iter().enumerate() {
                        if o.matches_event(*start, self.model) {
                            facts.1 |= f.trace[..index]
                                .iter()
                                .any(|a| a.matches_event(*before, self.model));
                        }
                    }
                }
                ContractKind::Waive { event, .. } => {
                    facts.0 |= f.trace.iter().any(|o| o.matches_event(*event, self.model));
                }
                ContractKind::Separation { left, right } => {
                    let left: Vec<_> = f
                        .trace
                        .iter()
                        .filter(|o| o.matches_event(*left, self.model))
                        .collect();
                    let right: Vec<_> = f
                        .trace
                        .iter()
                        .filter(|o| o.matches_event(*right, self.model))
                        .collect();
                    facts.0 |= !left.is_empty() && !right.is_empty();
                    for l in &left {
                        for r in &right {
                            match (l.person, r.person) {
                                (Some(p), Some(q)) => facts.1 |= p == q,
                                _ => aborted = true,
                            }
                        }
                    }
                }
                ContractKind::EvidenceBasis {
                    capability,
                    direct,
                    sources,
                    ..
                } => match evidence::facts(self.model, &f.trace, *capability, *direct, sources) {
                    Ok((active, broken, issue)) => {
                        facts.0 |= active;
                        facts.1 |= broken;
                        if issue {
                            self.statistics.evidence_unsupported.insert(ClauseId(i));
                        }
                    }
                    Err(_) => aborted = true,
                },
                ContractKind::Permitted { capability } => {
                    let c = &self.model.capabilities[capability.0];
                    let allowed = c.permission.as_ref().unwrap_or_else(|| {
                        self.model.actors[c.actor.0].executors.as_ref().unwrap()
                    });
                    for o in &f.trace {
                        if matches!(o.event, Occurrence::Dispatch(c) if c == *capability) {
                            facts.0 = true;
                            match o.person {
                                Some(p) => facts.1 |= !allowed.contains(&p),
                                None => aborted = true,
                            }
                        }
                    }
                }
                ContractKind::AuthorityBasis {
                    capability,
                    authority,
                } => {
                    for o in &f.trace {
                        if matches!(o.event, Occurrence::Completion(c, _) if c == *capability) {
                            facts.0 = true;
                            match o.person {
                                Some(p) => {
                                    match authority::event_holds(
                                        self.model,
                                        *authority,
                                        *capability,
                                        p,
                                        o,
                                        self.case_index,
                                    ) {
                                        Ok(holds) => facts.1 |= !holds,
                                        Err(_) => aborted = true,
                                    }
                                }
                                None => aborted = true,
                            }
                        }
                    }
                }
                ContractKind::Invariant { event, condition } => {
                    for o in f
                        .trace
                        .iter()
                        .filter(|o| o.matches_event(*event, self.model))
                    {
                        facts.0 = true;
                        match condition
                            .as_ref()
                            .ok_or(StopReason::SemanticAbort)
                            .and_then(|c| {
                                snapshot_condition_at(
                                    c,
                                    &o.values,
                                    &o.state_values,
                                    self.case_index,
                                )
                            }) {
                            Ok(holds) => facts.1 |= !holds,
                            Err(_) => aborted = true,
                        }
                    }
                }
                ContractKind::StateInvariant {
                    antecedent,
                    condition,
                } => {
                    for (_, values) in &f.state_snapshots {
                        let active = match antecedent {
                            Some(c) => snapshot_condition_at(c, &[], values, self.case_index),
                            None => Ok(true),
                        };
                        match active {
                            Ok(true) => {
                                facts.0 = true;
                                match snapshot_condition_at(condition, &[], values, self.case_index)
                                {
                                    Ok(holds) => facts.1 |= !holds,
                                    Err(_) => aborted = true,
                                }
                            }
                            Ok(false) => {}
                            Err(_) => aborted = true,
                        }
                    }
                }
                ContractKind::AtMostOnce { event } => {
                    let count = f
                        .trace
                        .iter()
                        .filter(|o| o.matches_event(*event, self.model))
                        .count();
                    facts.0 |= count > 0;
                    facts.1 |= count > 1;
                }
                ContractKind::RequiresEvent { before, start } => {
                    for (index, o) in f.trace.iter().enumerate() {
                        if o.matches_event(*start, self.model) {
                            facts.0 = true;
                            facts.1 |= !f.trace[..index]
                                .iter()
                                .any(|a| a.matches_event(*before, self.model));
                        }
                    }
                }
                ContractKind::Response {
                    trigger,
                    targets,
                    within,
                }
                | ContractKind::NamedResponse {
                    trigger,
                    targets,
                    within,
                    ..
                } => {
                    for (index, o) in f.trace.iter().enumerate() {
                        if !o.matches_trigger(*trigger, self.model) {
                            continue;
                        }
                        facts.0 = true;
                        let Some(deadline) = o.at.checked_add(*within) else {
                            aborted = true;
                            continue;
                        };
                        let fulfilled = f.trace[index..].iter().any(|t| {
                            t.at <= deadline
                                && (targets
                                    .iter()
                                    .any(|pattern| t.matches_target(*pattern, self.model))
                                    || self.model.contracts.iter().any(|w| matches!(&w.kind, ContractKind::Waive { response, event, .. } if response.0 == i && t.matches_event(*event, self.model))))
                        });
                        facts.1 |= !fulfilled
                            && (closed || deadline < horizon || (inclusive && deadline == horizon));
                    }
                }
            }
        }
        if aborted {
            Err(StopReason::SemanticAbort)
        } else {
            Ok(())
        }
    }

    fn reserve(&mut self, depth: usize) -> Execution {
        if depth > self.limits.branch_depth || self.statistics.frames >= self.limits.frames {
            return Err(StopReason::EnumerationLimit);
        }
        self.statistics.frames += 1;
        Ok(())
    }

    fn successor(&mut self, frame: &Frame<'a>, depth: usize) -> Result<Frame<'a>, StopReason> {
        self.reserve(depth)?;
        #[cfg(test)]
        {
            self.statistics.successor_clones += 1;
        }
        Ok(frame.clone())
    }

    fn run(&mut self, f: Frame<'a>, depth: usize) -> Execution {
        if let Some(work) = &mut self.run_work {
            let id = work.enqueue(f, depth);
            self.probability_stack
                .last_mut()
                .expect("path continuation parent")
                .deferred_children
                .push(id);
            return Ok(());
        }
        self.run_iterative(f, depth)
    }

    fn run_inner(&mut self, mut f: Frame<'a>, depth: usize) -> Execution {
        loop {
            if f.steps >= self.limits.steps_per_path {
                return Err(StopReason::EnumerationLimit);
            }
            f.steps += 1;
            self.observe_capacity(&f)?;
            if f.now > self.model.bounds.clock
                || f.tasks.iter().filter(|t| !t.terminal()).count() as u64
                    > self.model.bounds.live_tasks
            {
                self.evidence.truncated = true;
                if f.now > self.model.bounds.clock {
                    self.inspect_trace(&f, self.model.bounds.clock, true, false, false)?;
                } else {
                    self.inspect_trace(&f, f.now, false, false, false)?;
                }
                let reason = if f.tasks.iter().filter(|t| !t.terminal()).count() as u64
                    > self.model.bounds.live_tasks
                {
                    ReferenceCut::LiveTasks
                } else {
                    ReferenceCut::Clock
                };
                self.probability_leaf(&f, None, reason)?;
                return Ok(());
            }
            let Some(coins) = self.duration_coins(&f)? else {
                return self.probability_leaf(&f, None, ReferenceCut::Numeric);
            };
            if !coins.is_empty() {
                return self.duration_tree(f, &coins, depth);
            }
            self.inspect_trace(&f, f.now, false, false, false)?;
            if let Some(wake) = f.sleep() {
                if f.now >= wake {
                    self.record(&mut f, Occurrence::Wake, None)?;
                } else if !crate::contest::has_effects(self.model) {
                    f.now = f.now.checked_add(1).ok_or(StopReason::SemanticAbort)?;
                    continue;
                }
            }
            if f.business.is_some() || crate::contest::has_effects(self.model) {
                if let Some(id) = self
                    .model
                    .contests()
                    .iter()
                    .enumerate()
                    .find_map(|(id, _d)| {
                        (f.contest_target(self.model, id).is_some()
                            && !f.trace.iter().any(
                                |o| matches!(o.event, Occurrence::ContestChoice(n, _) if n == id),
                            ))
                        .then_some(id)
                    })
                {
                    for filed in [false, true] {
                        self.defer_successor(&f, depth + 1, move |this, _, mut next| {
                            this.record(&mut next, Occurrence::ContestChoice(id, filed), None)?;
                            this.run(next, depth + 1)
                        })?;
                    }
                    return Ok(());
                }
                if let Some(id) = self
                    .model
                    .contests()
                    .iter()
                    .enumerate()
                    .find_map(|(id, _)| {
                        (f.trace.iter().any(
                            |o| matches!(o.event, Occurrence::ContestChoice(n, true) if n == id),
                        ) && !f
                            .trace
                            .iter()
                            .any(|o| matches!(o.event, Occurrence::ContestQueue(n) if n == id)))
                        .then_some(id)
                    })
                {
                    let declaration = &self.model.contests()[id];
                    let cap = &self.model.capabilities[declaration.route.0];
                    require(cap.params.is_empty())?;
                    f.contest_target(self.model, id)
                        .unwrap()
                        .at
                        .checked_add(declaration.respond_by)
                        .ok_or(StopReason::SemanticAbort)?;
                    self.record(&mut f, Occurrence::ContestQueue(id), None)?;
                    if crate::responsibility::included(self.model, cap.actor) {
                        let start = f
                            .tasks
                            .iter()
                            .position(|t| self.model.site_contest(t.call.binding).is_some())
                            .unwrap_or(f.tasks.len());
                        let waiting = f.waiting.clone();
                        self.create(
                            &mut f,
                            &[Call {
                                binding: self.model.contest_site(id),
                                capability: declaration.route,
                                args: Vec::new(),
                                span: declaration.span,
                            }],
                            None,
                        )?;
                        if crate::contest::has_effects(self.model) {
                            f.waiting = waiting;
                        } else {
                            f.waiting.as_mut().unwrap().tasks.start = start;
                        }
                    }
                    continue;
                }
            }

            if crate::contest::has_effects(self.model) && f.sleep().is_none() {
                if let Some(wait) = f.waiting.clone()
                    && wait.tasks.clone().all(|i| f.tasks[i].terminal())
                    && self.winners(&f, &wait).is_empty()
                {
                    if let Some(Join::Any { if_none, .. }) = wait.join
                        && wait.winner.is_none()
                    {
                        return self.terminal(&f, *if_none, depth);
                    }
                    f.waiting = None;
                }
                if f.waiting.is_none() && f.business.is_none() {
                    while f.control.last().is_some_and(|b| b.is_empty()) {
                        f.control.pop();
                    }
                    if let Some(block) = f.control.last_mut() {
                        let statement = &block[0];
                        *block = &block[1..];
                        match &statement.kind {
                            Stmt::MergeCopy { binding, source } => {
                                let value = f.merge_value(self.model, *source)?;
                                require(!f.trace.iter().any(|o| matches!(o.event, Occurrence::Merged(id, _, _, _) if id == *binding)))?;
                                self.record(
                                    &mut f,
                                    Occurrence::Merged(*binding, *source, value, statement.span),
                                    None,
                                )?;
                            }
                            Stmt::LoopControl(control) => {
                                for (id, x) in &control.updates {
                                    f.state_values[id.0] = *x;
                                }
                            }
                            Stmt::Query(call, _) => {
                                self.create(&mut f, std::slice::from_ref(call), None)?
                            }
                            Stmt::ResolveEffect { command, evidence } => {
                                let event = recovery::resolve(
                                    &f,
                                    self.model,
                                    *command,
                                    *evidence,
                                    statement.span,
                                    self.case_index,
                                )?;
                                self.record(&mut f, event, None)?;
                            }
                            Stmt::StopReceiptUnknown => {
                                let event = recovery::stop(&f, self.model)?;
                                self.record(&mut f, event, None)?;
                                return self.probability_leaf(&f, None, ReferenceCut::None);
                            }
                            Stmt::WaitUntil(_) => return Err(StopReason::SemanticAbort),
                            Stmt::Wait(duration) => {
                                let wake = f
                                    .now
                                    .checked_add(*duration)
                                    .ok_or(StopReason::SemanticAbort)?;
                                self.record(&mut f, Occurrence::Wait(wake), None)?;
                            }
                            Stmt::End(e) => return self.terminal(&f, *e, depth),
                            Stmt::If {
                                condition,
                                then_body,
                                else_body,
                            } => {
                                let yes = self.condition(&f, condition, 0)?;
                                require(
                                    self.statistics
                                        .observed
                                        .branch_statement(self.model, statement, yes),
                                )?;
                                if f.control.len() >= self.limits.control_depth {
                                    return Err(StopReason::EnumerationLimit);
                                }
                                f.control.push(if yes { then_body } else { else_body });
                            }
                            Stmt::Call(call) | Stmt::Response(call, _) => {
                                self.create(&mut f, std::slice::from_ref(call), None)?
                            }
                            Stmt::Fork { branches, join, .. } => {
                                self.create(&mut f, branches, Some(join))?
                            }
                        }
                        continue;
                    }
                }
            }
            // Resolve the entire due completion set before testing a tie. Each
            // product is enumerated lazily, one result/ack successor at a time.
            let due: Vec<_> = f
                .tasks
                .iter()
                .enumerate()
                .filter_map(|(i, t)| {
                    (t.phase == Phase::Running && (t.finish == Some(f.now) || t.ack == Some(f.now)))
                        .then_some(i)
                })
                .collect();
            for &i in &due {
                if f.tasks[i].finish == Some(f.now) {
                    self.defer_in_place(&f, move |this, f| {
                        if this.range_attempt(f, i, false, None)? {
                            this.probability_leaf(f, None, ReferenceCut::Range)?;
                        } else {
                            let cap = &this.model.capabilities[f.tasks[i].call.capability.0];
                            if let Some(kernel) = &cap.kernel {
                                #[cfg(test)]
                                {
                                    this.statistics.kernel_draws += 1;
                                }
                                let row = kernel
                                    .rows
                                    .iter()
                                    .find(|r| {
                                        r.values
                                            .iter()
                                            .enumerate()
                                            .all(|(j, x)| f.state_values[kernel.given[j].0] == *x)
                                    })
                                    .ok_or(StopReason::SemanticAbort)?;
                                let mut weights = Vec::new();
                                this.probability_chance = true;
                                this.probability_stack.push(ReferenceChoices::default());
                                for b in &row.branches {
                                    if b.probability.is_zero() {
                                        continue;
                                    }
                                    let weight = b.probability.clone();
                                    let b = b.clone();
                                    this.defer_successor(
                                        f,
                                        depth + 1,
                                        move |this, base, mut next| {
                                            next.tasks[i].phase = Phase::Finished(b.result);
                                            next.tasks[i].stop = Some(base.now);
                                            let cap_id = next.tasks[i].call.capability;
                                            let person = next.tasks[i].person;
                                            this.finish_state(&mut next, i, b.result);
                                            for &(id, x) in &b.overrides {
                                                next.state_values[id.0] = x;
                                            }
                                            this.record_site(
                                                &mut next,
                                                Occurrence::Completion(cap_id, b.result),
                                                person,
                                                base.tasks[i].call.binding,
                                            )?;
                                            this.statistics.normal_completions += 1;
                                            this.response_finish(next, i, depth + 1)
                                        },
                                    )?;
                                    weights.push(weight);
                                }
                                this.probability_finish(Some(&weights));
                            } else {
                                let selected = if reference_stale(&f.tasks[i], cap) {
                                    vec![cap.version_check.as_ref().unwrap().reject]
                                } else if cap.receipt_query.is_some() {
                                    recovery::project(
                                        this.model,
                                        f.tasks[i].call.capability,
                                        &f.state_values,
                                        this.case_index,
                                    )?
                                } else if let Some(a) = &cap.atomic {
                                    vec![if this.condition(f, &a.condition, 0)? == a.commit_when {
                                        a.commit
                                    } else {
                                        a.reject
                                    }]
                                } else {
                                    cap.results.clone()
                                };
                                for value in &selected {
                                    let value = *value;
                                    this.defer_successor(
                                        f,
                                        depth + 1,
                                        move |this, base, mut next| {
                                            next.tasks[i].phase = Phase::Finished(value);
                                            next.tasks[i].stop = Some(base.now);
                                            let cap_id = next.tasks[i].call.capability;
                                            let person = next.tasks[i].person;
                                            this.finish_state(&mut next, i, value);
                                            this.record_site(
                                                &mut next,
                                                Occurrence::Completion(cap_id, value),
                                                person,
                                                base.tasks[i].call.binding,
                                            )?;
                                            this.statistics.normal_completions += 1;
                                            if base.tasks[i].ack == Some(base.now) {
                                                this.statistics.simultaneous_normal_choices += 1;
                                            }
                                            this.response_finish(next, i, depth + 1)
                                        },
                                    )?;
                                }
                            }
                        }
                        Ok(())
                    })?;
                }
                if f.tasks[i].ack == Some(f.now) {
                    self.defer_successor(&f, depth + 1, move |this, base, mut next| {
                        next.tasks[i].phase = Phase::Cancelled;
                        next.tasks[i].stop = Some(base.now);
                        if this.model.tracks_obligations() {
                            this.record_site(
                                &mut next,
                                Occurrence::CancelAck(base.tasks[i].call.capability),
                                base.tasks[i].person,
                                base.tasks[i].call.binding,
                            )?;
                        }
                        #[cfg(test)]
                        if !this.model.state_variables.is_empty() {
                            this.statistics.state_seams.push((
                                "ack",
                                next.tasks[i].call.capability,
                                next.state_values.clone(),
                            ));
                        }
                        this.statistics.cancellation_completions += 1;
                        if base.tasks[i].finish == Some(base.now) {
                            this.statistics.simultaneous_ack_choices += 1;
                        }
                        this.run(next, depth + 1)
                    })?;
                }
            }

            if let Some(wait) = f.waiting.clone().or_else(|| {
                crate::contest::has_effects(self.model).then_some(Waiting {
                    tasks: 0..0,
                    join: None,
                    winner: None,
                })
            }) {
                let candidates = if due.is_empty() {
                    self.winners(&f, &wait)
                } else {
                    Vec::new()
                };
                // Adoption and each eligible start are alternative NEXT instant
                // events. Recursing repeats this choice, allowing adoption before,
                // between, or after dispatches at the same tick (ADR 0001:73).
                let mut instant = !due.is_empty();
                for winner in candidates {
                    instant = true;
                    let cancel = matches!(
                        wait.join,
                        Some(Join::Any {
                            remaining: Remaining::CancelThenDrain,
                            ..
                        })
                    );
                    let tasks = wait.tasks.clone();
                    self.defer_successor(&f, depth + 1, move |this, base, mut next| {
                        if next.adoption.replace(base.now).is_some() {
                            return Err(StopReason::SemanticAbort);
                        }
                        next.waiting.as_mut().unwrap().winner = Some(winner);
                        *this.statistics.adoptions.entry(winner).or_default() += 1;
                        if cancel {
                            this.cancel(next, tasks.start, tasks.end, winner, depth + 1)
                        } else {
                            this.run(next, depth + 1)
                        }
                    })?;
                }
                for i in if crate::contest::has_effects(self.model) {
                    0..f.tasks.len()
                } else {
                    wait.tasks.clone()
                } {
                    if self.can_start(&f, i)? {
                        let actor = self.model.capabilities[f.tasks[i].call.capability.0].actor;
                        let pool = self.model.actors[actor.0].executors.as_ref();
                        let mut people = pool
                            .into_iter()
                            .flatten()
                            .copied()
                            .filter(|p| Self::person_available(&f, *p))
                            .map(Some)
                            .chain(pool.is_none().then_some(None))
                            .peekable();
                        let refuses = people.peek().is_some()
                            && self.model.capabilities[f.tasks[i].call.capability.0]
                                .requires
                                .as_ref()
                                .map(|r| self.condition(&f, &r.condition, 0))
                                .transpose()?
                                .is_some_and(|v| !v);
                        for person in people.take(if refuses { 1 } else { usize::MAX }) {
                            instant = true;
                            self.defer_in_place(&f, move |this, base| {
                                let mut next = this.successor(base, depth + 1)?;
                                let cap = &this.model.capabilities[next.tasks[i].call.capability.0];
                                if let Some(req) = &cap.requires
                                    && !this.condition(base, &req.condition, 0)?
                                {
                                    next.tasks[i].phase = Phase::Refused(req.otherwise);
                                    next.tasks[i].stop = Some(base.now);
                                    this.record_site(
                                        &mut next,
                                        Occurrence::Refusal(
                                            base.tasks[i].call.capability,
                                            req.otherwise,
                                        ),
                                        None,
                                        base.tasks[i].call.binding,
                                    )?;
                                    return this.run(next, depth + 1);
                                }
                                next.tasks[i].phase = Phase::Running;
                                next.tasks[i].stale.clear();
                                next.tasks[i].person = person;
                                next.tasks[i].start = Some(base.now);
                                if let Some(distribution) = &cap.duration_distribution {
                                    for (duration, weight) in &distribution.entries {
                                        if !weight.is_zero() {
                                            base.now
                                                .checked_add(*duration)
                                                .ok_or(StopReason::SemanticAbort)?;
                                        }
                                    }
                                    next.tasks[i].finish = None;
                                } else {
                                    next.tasks[i].finish = Some(
                                        base.now
                                            .checked_add(cap.duration)
                                            .ok_or(StopReason::SemanticAbort)?,
                                    );
                                }
                                let cap_id = next.tasks[i].call.capability;
                                if this.range_attempt(base, i, true, None)? {
                                    return this.probability_leaf(base, None, ReferenceCut::Range);
                                }
                                for w in &cap.start_writes {
                                    next.state_values[w.variable.0] = reference_effect_at(
                                        w,
                                        &base.state_values,
                                        None,
                                        this.case_index,
                                        person,
                                    )?;
                                }
                                reference_notify(&mut next, Some(i), &cap.start_writes, this.model);
                                this.record_site(
                                    &mut next,
                                    Occurrence::Dispatch(cap_id),
                                    person,
                                    base.tasks[i].call.binding,
                                )?;
                                this.run(next, depth + 1)
                            })?;
                        }
                    }
                }
                if instant {
                    return Ok(());
                }
                if f.waiting.is_some() && wait.tasks.clone().all(|i| f.tasks[i].terminal()) {
                    if wait.join.is_none()
                        && self
                            .model
                            .response_attempt(f.tasks[wait.tasks.start].call.binding)
                            .is_some()
                    {
                        let now = f.now;
                        let (decision, events) = response::progress(&mut f, self.model, now)?;
                        for event in events {
                            self.record(&mut f, event, None)?;
                        }
                        match decision {
                            response::Progress::Unknown => {
                                return self.probability_leaf(&f, None, ReferenceCut::None);
                            }
                            response::Progress::End(e) => return self.terminal(&f, e, depth),
                            response::Progress::Next => continue,
                        }
                    }
                    if let Some(Join::Any { if_none, .. }) = wait.join
                        && wait.winner.is_none()
                    {
                        return self.terminal(&f, *if_none, depth);
                    }
                    f.waiting = None;
                    continue;
                }
                if f.tasks.iter().any(|t| t.phase == Phase::Running) || f.sleep().is_some() {
                    f.now = f.now.checked_add(1).ok_or(StopReason::SemanticAbort)?;
                    continue;
                }
                if let Some((end, _)) = f.business
                    && f.tasks.iter().all(Task::terminal)
                {
                    self.inspect_trace(&f, f.now, false, true, false)?;
                    self.collect_obligations(&f, "terminal_unresolved")?;
                    return self.probability_leaf(&f, Some(end), ReferenceCut::None);
                }
                return self.deadlock(&f);
            }

            while f.control.last().is_some_and(|b| b.is_empty()) {
                f.control.pop();
            }
            if let Some((end, _)) = f.business {
                self.inspect_trace(&f, f.now, false, true, false)?;
                self.collect_obligations(&f, "terminal_unresolved")?;
                return self.probability_leaf(&f, Some(end), ReferenceCut::None);
            }
            let Some(block) = f.control.last_mut() else {
                return self.deadlock(&f);
            };
            let statement = &block[0];
            *block = &block[1..];
            match &statement.kind {
                Stmt::MergeCopy { binding, source } => {
                    let value = f.merge_value(self.model, *source)?;
                    require(!f.trace.iter().any(
                        |o| matches!(o.event, Occurrence::Merged(id, _, _, _) if id == *binding),
                    ))?;
                    self.record(
                        &mut f,
                        Occurrence::Merged(*binding, *source, value, statement.span),
                        None,
                    )?;
                }
                Stmt::LoopControl(control) => {
                    for (id, x) in &control.updates {
                        f.state_values[id.0] = *x;
                    }
                }
                Stmt::Query(call, _) => self.create(&mut f, std::slice::from_ref(call), None)?,
                Stmt::ResolveEffect { command, evidence } => {
                    let event = recovery::resolve(
                        &f,
                        self.model,
                        *command,
                        *evidence,
                        statement.span,
                        self.case_index,
                    )?;
                    self.record(&mut f, event, None)?;
                }
                Stmt::StopReceiptUnknown => {
                    let event = recovery::stop(&f, self.model)?;
                    self.record(&mut f, event, None)?;
                    return self.probability_leaf(&f, None, ReferenceCut::None);
                }
                Stmt::WaitUntil(_) => return Err(StopReason::SemanticAbort),
                Stmt::Wait(duration) => {
                    let wake = f
                        .now
                        .checked_add(*duration)
                        .ok_or(StopReason::SemanticAbort)?;
                    self.record(&mut f, Occurrence::Wait(wake), None)?;
                }
                Stmt::End(end) => return self.terminal(&f, *end, depth),
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    let yes = self.condition(&f, condition, 0)?;
                    require(
                        self.statistics
                            .observed
                            .branch_statement(self.model, statement, yes),
                    )?;
                    let selected = if yes { then_body } else { else_body };
                    if f.control.len() >= self.limits.control_depth {
                        return Err(StopReason::EnumerationLimit);
                    }
                    f.control.push(selected.as_slice());
                }
                Stmt::Response(call, r) => {
                    let event = response::input(&f, self.model, call, r, self.case_index)?;
                    if let Some(event) = event {
                        self.record(&mut f, event, None)?;
                    }
                    self.create(&mut f, std::slice::from_ref(call), None)?
                }
                Stmt::Call(call) => self.create(&mut f, std::slice::from_ref(call), None)?,
                Stmt::Fork { branches, join, .. } => self.create(&mut f, branches, Some(join))?,
            }
        }
    }

    fn create(&self, f: &mut Frame<'a>, calls: &[Call], join: Option<&'a Join>) -> Execution {
        let end = f
            .tasks
            .len()
            .checked_add(calls.len())
            .ok_or(StopReason::EnumerationLimit)?;
        if end > self.limits.tasks_per_path {
            return Err(StopReason::EnumerationLimit);
        }
        for call in calls {
            let cap = &self.model.capabilities[call.capability.0];
            if (cap.duration == 0 && cap.duration_distribution.is_none())
                || self.model.actors[cap.actor.0].capacity.is_none()
            {
                return Err(StopReason::SemanticAbort);
            }
        }
        let start = f.tasks.len();
        f.tasks.extend(calls.iter().map(|call| Task {
            frozen_performance: None,
            stale: BTreeSet::new(),
            metric_ready: self.model.metrics.as_ref().map(|_| f.now),
            checked_through: None,
            person: None,
            call: call.clone(),
            phase: Phase::Queued,
            start: None,
            stop: None,
            finish: None,
            ack: None,
        }));
        f.waiting = Some(Waiting {
            tasks: start..end,
            join,
            winner: None,
        });
        // The next loop checks live_tasks BEFORE any of the new tasks dispatch.
        Ok(())
    }

    /// A state range stop ends the single-case search early. Evidence found
    /// before the stop keeps its verdict; every other property is inconclusive.
    fn range_stop_verdict(&self, property: Property) -> Option<Verdict> {
        if self.statistics.ranges.is_empty() {
            return None;
        }
        match property {
            Property::StateInRange => Some(Verdict::Violated),
            // ADR 0041: a loss found before the stop stays violated, as in the engine.
            Property::NoLostUpdate(v) if self.evidence.lost_updates.contains(&v) => {
                Some(Verdict::Violated)
            }
            Property::ProbabilityReached(_) | Property::Contest(_) => None,
            Property::ReachableEnd(e) if self.evidence.reached.contains(&e) => None,
            Property::Contract(id) if self.contract_facts.get(id.0).is_some_and(|x| x.1) => None,
            Property::NoResourceOvercommit if self.evidence.overcommitted => None,
            _ => Some(Verdict::Inconclusive),
        }
    }

    fn observe_capacity(&mut self, f: &Frame<'a>) -> Execution {
        self.evidence
            .lost_updates
            .extend(f.lost_updates.iter().copied());
        for (r, quantity) in self
            .model
            .resources
            .iter()
            .zip(reference_resource_units(f.tasks.iter(), self.model)?)
        {
            self.evidence.overcommitted |= quantity > r.capacity;
        }
        for (index, task) in f
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.phase == Phase::Running)
        {
            let a = &self.model.actors[self.model.capabilities[task.call.capability.0].actor.0];
            match (&a.executors, task.person) {
                (Some(ps), Some(p)) => {
                    require(ps.contains(&p))?;
                    self.evidence.overcommitted |= f.tasks[..index]
                        .iter()
                        .any(|t| t.phase == Phase::Running && t.person == Some(p));
                }
                (None, None) => {}
                _ => return Err(StopReason::SemanticAbort),
            }
        }
        for (a, actor) in self.model.actors.iter().enumerate() {
            let occupied = f
                .tasks
                .iter()
                .filter(|t| {
                    t.phase == Phase::Running
                        && self.model.capabilities[t.call.capability.0].actor == ActorId(a)
                })
                .count() as u64;
            match actor.capacity {
                Some(capacity) if occupied > capacity => self.evidence.overcommitted = true,
                None if occupied > 0 => return Err(StopReason::SemanticAbort),
                _ => {}
            }
        }
        Ok(())
    }

    fn can_start(&self, f: &Frame<'a>, i: usize) -> Result<bool, StopReason> {
        if f.tasks[i].phase != Phase::Queued {
            return Ok(false);
        }
        let actor = self.model.capabilities[f.tasks[i].call.capability.0].actor;
        let quantities = reference_resource_units(f.tasks.iter(), self.model)?;
        for u in &self.model.capabilities[f.tasks[i].call.capability.0].uses {
            let limit = self.model.resources[u.resource.0].capacity;
            if quantities[u.resource.0] > limit || u.amount > limit - quantities[u.resource.0] {
                return Ok(false);
            }
        }
        let capacity = self.model.actors[actor.0]
            .capacity
            .ok_or(StopReason::SemanticAbort)?;
        let used = f
            .tasks
            .iter()
            .filter(|t| {
                t.phase == Phase::Running
                    && self.model.capabilities[t.call.capability.0].actor == actor
            })
            .count() as u64;
        if !policy::calendar_slots(
            self.model,
            &self.model.capabilities[f.tasks[i].call.capability.0],
            f.now,
            used,
            &quantities,
        ) {
            return Ok(false);
        }
        Ok((f
            .tasks
            .iter()
            .filter(|t| {
                t.phase == Phase::Running
                    && self.model.capabilities[t.call.capability.0].actor == actor
            })
            .count() as u64)
            < capacity)
    }

    fn person_available(f: &Frame<'a>, person: PersonId) -> bool {
        !f.tasks
            .iter()
            .any(|task| task.phase == Phase::Running && task.person == Some(person))
    }

    fn winners(&self, f: &Frame<'a>, wait: &Waiting<'a>) -> Vec<BindingId> {
        let Some(Join::Any { matches, tie, .. }) = wait.join else {
            return Vec::new();
        };
        if wait.winner.is_some() {
            return Vec::new();
        }
        let mut candidates = Vec::new();
        let mut earliest = None;
        for i in wait.tasks.clone() {
            let t = &f.tasks[i];
            if let Phase::Finished(value) | Phase::Refused(value) = t.phase
                && matches
                    .iter()
                    .any(|m| m.binding == t.call.binding && m.value == value)
            {
                if earliest.is_none() || t.stop < earliest {
                    candidates.clear();
                    earliest = t.stop;
                }
                if t.stop == earliest {
                    candidates.push(t.call.binding);
                }
            }
        }
        match tie {
            Tie::Nondet => candidates,
            Tie::By(order) => order
                .iter()
                .find(|b| candidates.contains(b))
                .copied()
                .into_iter()
                .collect(),
        }
    }

    fn cancel(
        &mut self,
        f: Frame<'a>,
        i: usize,
        end: usize,
        winner: BindingId,
        depth: usize,
    ) -> Execution {
        if let Some(work) = &mut self.run_work {
            let id = work.enqueue_cancel(f, i, end, winner, depth);
            self.probability_stack
                .last_mut()
                .expect("cancellation parent")
                .deferred_children
                .push(id);
            return Ok(());
        }
        match self.cancel_step(f, i, end, winner, depth)? {
            path_stack::CancelStep::Run(f, depth) => self.run(f, depth),
            path_stack::CancelStep::Acks {
                frame,
                index,
                end,
                winner,
                depth,
                max_delay,
            } => {
                for delay in 1..=max_delay {
                    let mut next = self.successor(&frame, depth + 1)?;
                    next.tasks[index].ack = Some(
                        frame
                            .now
                            .checked_add(delay)
                            .ok_or(StopReason::SemanticAbort)?,
                    );
                    #[cfg(test)]
                    if self.model.capabilities[frame.tasks[index].call.capability.0]
                        .duration_distribution
                        .is_some()
                    {
                        self.statistics
                            .duration_ack_candidates
                            .push(next.tasks[index].ack.unwrap());
                    }
                    self.cancel(next, index + 1, end, winner, depth + 1)?;
                }
                Ok(())
            }
        }
    }

    fn cancel_step(
        &mut self,
        mut f: Frame<'a>,
        mut i: usize,
        end: usize,
        winner: BindingId,
        depth: usize,
    ) -> Result<path_stack::CancelStep<'a>, StopReason> {
        while i < end {
            let t = &f.tasks[i];
            if t.call.binding != winner && !t.terminal() {
                #[cfg(test)]
                if !self.model.state_variables.is_empty() {
                    self.statistics.state_seams.push((
                        "request",
                        t.call.capability,
                        f.state_values.clone(),
                    ));
                }
                let contract = self.model.capabilities[t.call.capability.0].cancel;
                if contract == Cancel::Unspecified {
                    return Err(StopReason::SemanticAbort);
                }
                if t.phase == Phase::Queued {
                    f.tasks[i].phase = Phase::Cancelled;
                    f.tasks[i].stop = Some(f.now);
                    #[cfg(test)]
                    if !self.model.state_variables.is_empty() {
                        self.statistics.state_seams.push((
                            "queued",
                            f.tasks[i].call.capability,
                            f.state_values.clone(),
                        ));
                    }
                } else if contract == Cancel::Immediate {
                    f.tasks[i].ack = Some(f.now);
                } else if let Cancel::Cooperative { ack_within } = contract {
                    if ack_within == 0 {
                        return Err(StopReason::SemanticAbort);
                    }
                    let cap = &self.model.capabilities[t.call.capability.0];
                    let max_delay = if let Some(distribution) = &cap.duration_distribution {
                        let longest = distribution
                            .entries
                            .iter()
                            .filter(|(_, p)| !p.is_zero())
                            .map(|(d, _)| *d)
                            .max()
                            .ok_or(StopReason::SemanticAbort)?;
                        let natural = t
                            .start
                            .ok_or(StopReason::SemanticAbort)?
                            .checked_add(longest)
                            .ok_or(StopReason::SemanticAbort)?;
                        let horizon = natural
                            .saturating_add(1)
                            .min(self.model.bounds.clock.saturating_add(1));
                        ack_within.min(horizon - f.now)
                    } else {
                        ack_within
                    };
                    return Ok(path_stack::CancelStep::Acks {
                        frame: f,
                        index: i,
                        end,
                        winner,
                        depth,
                        max_delay,
                    });
                }
            }
            i += 1;
        }
        Ok(path_stack::CancelStep::Run(f, depth))
    }

    fn condition(&self, f: &Frame<'a>, c: &Cond, depth: usize) -> Result<bool, StopReason> {
        if depth >= self.limits.control_depth {
            return Err(StopReason::EnumerationLimit);
        }
        match c {
            Cond::Contains { set, .. } => {
                reference_membership(&f.state_values, *set, self.case_index)
            }
            Cond::IntCompare {
                variable,
                operator,
                value,
            } => reference_integer_test(
                *f.state_values
                    .get(variable.0)
                    .ok_or(StopReason::SemanticAbort)?,
                *operator,
                *value,
            ),
            Cond::StateEq(t) => f
                .state_values
                .get(t.variable.0)
                .map(|x| *x == t.value)
                .ok_or(StopReason::SemanticAbort),
            Cond::Eq(m) => {
                if let Some(value) = f.trace.iter().rev().find_map(|o| match o.event {
                    Occurrence::Merged(id, _, value, _) | Occurrence::AdoptResponse(id, value)
                        if id == m.binding =>
                    {
                        Some(value)
                    }
                    _ => None,
                }) {
                    return Ok(value == m.value);
                }
                let task = f
                    .tasks
                    .iter()
                    .find(|t| t.call.binding == m.binding)
                    .or_else(|| {
                        f.tasks.iter().rev().find(|t| {
                            self.model.loop_alias(t.call.binding) == Some(m.binding)
                                && matches!(t.phase, Phase::Finished(_) | Phase::Refused(_))
                        })
                    })
                    .ok_or(StopReason::SemanticAbort)?;
                let cap = &self.model.capabilities[task.call.capability.0];
                if cap.return_type != self.model.process.bindings[m.binding.0].ty {
                    return Err(StopReason::SemanticAbort);
                }
                match task.phase {
                    Phase::Finished(value) | Phase::Refused(value) => Ok(value == m.value),
                    _ => Err(StopReason::SemanticAbort),
                }
            }
            Cond::And(cs) => {
                let mut result = true;
                for c in cs {
                    result &= self.condition(f, c, depth + 1)?;
                }
                Ok(result)
            }
            Cond::Or(cs) => {
                let mut result = false;
                for c in cs {
                    result |= self.condition(f, c, depth + 1)?;
                }
                Ok(result)
            }
        }
    }

    fn deadlock(&mut self, f: &Frame<'a>) -> Execution {
        self.inspect_trace(f, f.now, false, true, true)?;
        self.evidence.deadlocked = true;
        self.statistics.deadlock_paths += 1;
        self.probability_leaf(f, None, ReferenceCut::None)?;
        Ok(())
    }

    fn terminal(&mut self, f: &Frame<'a>, end: EndId, depth: usize) -> Execution {
        if self.model.recovery().is_some() {
            require(recovery::unresolved(f).is_empty())?;
        }
        if self.model.contests().is_empty() {
            self.probability_leaf(f, Some(end), ReferenceCut::None)?;
        }
        #[cfg(test)]
        if !self.model.state_variables.is_empty() {
            self.statistics.terminal_traces.push(f.trace.clone());
        }
        let mut closed = f.clone();
        self.record(&mut closed, Occurrence::Terminal(end), None)?;
        self.inspect_trace(&closed, f.now, false, true, false)?;
        self.collect_obligations(&closed, "terminal_unresolved")?;
        self.evidence.reached.insert(end);
        self.statistics.terminal_paths += 1;
        let mut work = 0_u64;
        for t in &f.tasks {
            if crate::contest::has_effects(self.model)
                && self.model.site_contest(t.call.binding).is_some()
            {
                continue;
            }
            if let Some(start) = t.start {
                let occupied = t
                    .stop
                    .and_then(|stop| stop.checked_sub(start))
                    .ok_or(StopReason::SemanticAbort)?;
                work = work
                    .checked_add(occupied)
                    .ok_or(StopReason::SemanticAbort)?;
            }
        }
        if let Some(existing) = self.terminals.get_mut(&end) {
            if existing.result_at.is_some() != f.adoption.is_some() {
                return Err(StopReason::SemanticAbort);
            }
            extend(&mut existing.elapsed, f.now);
            extend(&mut existing.work, work);
            if let (Some(range), Some(at)) = (&mut existing.result_at, f.adoption) {
                extend(range, at);
            }
        } else {
            self.terminals.insert(
                end,
                TerminalOutcome {
                    probability: None,
                    end,
                    elapsed: singleton(f.now),
                    work: singleton(work),
                    result_at: f.adoption.map(singleton),
                },
            );
        }
        if !self.model.contests().is_empty() {
            closed.business = Some((end, f.now));
            closed.control.clear();
            closed.waiting = None;
            return self.run(closed, depth + 1);
        }
        Ok(())
    }
}

fn singleton(value: u64) -> Range {
    Range {
        min: value,
        max: value,
    }
}

fn reference_duration_divide(at_age: &Rational, alive: &Rational, bits: u64) -> Option<Rational> {
    if alive.is_zero() {
        return None;
    }
    let conditional = Rational(at_age.0.clone() / alive.0.clone());
    conditional.within_bits(bits).then_some(conditional)
}

#[cfg(test)]
mod duration_tests {
    use super::*;
    fn model(s: &str) -> Model {
        crate::lower::lower(&crate::syntax::parse(s).unwrap()).unwrap()
    }

    #[test]
    fn d_no_lookahead_and_clock_hand_oracle() {
        let m = model(include_str!(
            "../../../fixtures/duration-no-lookahead/model.relca"
        ));
        let r = explore(&m);
        assert_eq!(r.stop, StopReason::Complete);
        let p = r
            .outcome
            .properties
            .iter()
            .find_map(|p| p.probability.as_ref())
            .unwrap();
        assert_eq!(
            (p.min.to_string(), p.max.to_string()),
            ("1/2".into(), "1/2".into())
        );
        let m = model(include_str!("../../../fixtures/duration-clock/model.relca"));
        let r = explore(&m);
        assert_eq!(r.stop, StopReason::Complete);
        let p = r
            .outcome
            .properties
            .iter()
            .find_map(|p| p.probability.as_ref())
            .unwrap();
        assert_eq!(
            (p.min.to_string(), p.max.to_string()),
            ("1/2".into(), "1/2".into())
        );
        assert!(r.outcome.truncated);
    }
    fn parallel() -> Model {
        model(
            r#"model M version "0.2" { type T=enum { ok }
        actor A { capacity 2; capability c() -> T { duration {1min:0,3min:0.5,5min:0.5};result nondet { ok } } }
        process P { fork f { x=A.c();y=A.c() };join all f;end done }
        bounds { cases 1;live_tasks 2;clock 10min } verify safety no_resource_overcommit }"#,
        )
    }
    fn interpreter(m: &Model) -> Interpreter<'_> {
        Interpreter {
            case_index: 0,
            model: m,
            limits: Limits::default(),
            evidence: Evidence::default(),
            terminals: BTreeMap::new(),
            statistics: Statistics::default(),
            contract_facts: vec![(false, false); m.contracts.len()],
            probability_stack: vec![],
            run_work: None,
            probability_result: None,
            probability_scheduler: false,
            probability_chance: false,
            denominator_bits: 4096,
            duration_numeric_cut: false,
        }
    }
    fn running(m: &Model, now: Tick) -> Frame<'_> {
        let Stmt::Fork { branches, join, .. } = &m.process.body[0].kind else {
            panic!()
        };
        let values: Vec<_> = m
            .state_variables
            .iter()
            .map(|v| match v.initial {
                InitialValue::Fixed(x) => x,
                _ => panic!(),
            })
            .collect();
        Frame {
            condition_waits: Vec::new(),
            lost_updates: BTreeSet::new(),
            state_snapshots: Vec::new(),
            business: None,
            now,
            steps: 0,
            control: vec![&m.process.body[1..]],
            adoption: None,
            state_values: values.clone(),
            trace: vec![Observation {
                binding: None,
                at: 0,
                event: Occurrence::Receipt,
                person: None,
                values: vec![None; m.process.bindings.len()],
                state_values: values,
            }],
            waiting: Some(Waiting {
                tasks: 0..branches.len(),
                join: Some(join),
                winner: None,
            }),
            tasks: branches
                .iter()
                .map(|call| Task {
                    frozen_performance: None,
                    stale: BTreeSet::new(),
                    metric_ready: None,
                    call: call.clone(),
                    person: None,
                    phase: Phase::Running,
                    start: Some(0),
                    stop: None,
                    finish: None,
                    ack: None,
                    checked_through: None,
                })
                .collect(),
        }
    }

    #[test]
    fn d_room_and_joint_before_choice() {
        let m = parallel();
        let f = running(&m, 3);
        let mut i = interpreter(&m);
        let coins = i.duration_coins(&f).unwrap().unwrap();
        assert_eq!(coins.len(), 2);
        i.limits.frames = 3;
        i.statistics.duration_probe = true;
        assert_eq!(
            i.duration_tree(f.clone(), &coins, 0),
            Err(StopReason::EnumerationLimit)
        );
        assert_eq!(i.statistics.duration_copies, 0);
        assert_eq!(i.statistics.frames, 0);
        i.limits.frames = 4;
        i.duration_tree(f, &coins, 0).unwrap();
        assert_eq!(i.statistics.frames, 4);
        assert_eq!(i.statistics.duration_copies, 4);
        let flags: BTreeSet<_> = i
            .statistics
            .duration_leaves
            .iter()
            .map(|(at, finishes, p)| {
                assert_eq!(*at, 3);
                assert_eq!(p.to_string(), "1/4");
                finishes.iter().map(|t| *t == Some(3)).collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(
            flags,
            BTreeSet::from([
                vec![true, true],
                vec![true, false],
                vec![false, true],
                vec![false, false]
            ])
        );
        // The probe above observes fully formed leaves without invoking run.
        // Check the real seam too: ordinary dispatch/completion sees only leaves
        // with BOTH checked-through markers, never a partial Bernoulli prefix.
        let actual = explore(&m);
        assert_eq!(actual.stop, StopReason::Complete);
        let at_three: Vec<_> = actual
            .statistics
            .duration_leaves
            .iter()
            .filter(|(at, _, _)| *at == 3)
            .collect();
        assert_eq!(
            at_three.len(),
            8,
            "two start orders, each with four complete chance leaves"
        );
        assert!(at_three.iter().all(|(_, _, p)| p.to_string() == "1/4"));
    }

    #[test]
    fn d_resample_and_observed_age() {
        let m = parallel();
        let mut f = running(&m, 3);
        f.tasks.truncate(1);
        let i = interpreter(&m);
        assert_eq!(
            i.duration_coins(&f).unwrap().unwrap()[0].1.to_string(),
            "1/2"
        );
        f.tasks[0].checked_through = Some(3);
        assert!(i.duration_coins(&f).unwrap().unwrap().is_empty());
        f.trace.push(Observation {
            binding: None,
            at: 3,
            event: Occurrence::Completion(CapabilityId(0), ValueId(0)),
            person: None,
            values: vec![None; 2],
            state_values: vec![],
        });
        assert!(
            i.duration_coins(&f).unwrap().unwrap().is_empty(),
            "same-time bundle cannot redraw survival"
        );
        f.now = 5;
        assert_eq!(i.duration_coins(&f).unwrap().unwrap()[0].1, Rational::one());
        f.now = 3;
        f.tasks[0].checked_through = None;
        f.tasks[0].start = Some(1);
        assert!(
            i.duration_coins(&f).unwrap().unwrap().is_empty(),
            "different age means different future"
        );
        f.tasks[0].start = Some(0);
        f.tasks[0].finish = Some(3);
        assert!(
            i.duration_coins(&f).unwrap().unwrap().is_empty(),
            "observed finish cannot be redrawn"
        );
    }

    #[test]
    fn d_numeric_division_joint_and_tree_reduction() {
        let p = Rational::decimal("0.1").unwrap();
        let alive = Rational::decimal("0.3").unwrap();
        assert!(reference_duration_divide(&p, &alive, 1).is_none());
        assert_eq!(
            reference_duration_divide(&p, &alive, 2)
                .unwrap()
                .to_string(),
            "1/3"
        );
        let m = parallel();
        let f = running(&m, 3);
        let mut i = interpreter(&m);
        i.denominator_bits = 2;
        i.statistics.duration_probe = true;
        let coins = i.duration_coins(&f).unwrap().unwrap();
        i.duration_tree(f, &coins, 0).unwrap();
        assert_eq!(i.statistics.duration_copies, 0);
        assert!(i.probability_result.unwrap().iter().all(|b| b.numeric));
        let half = Rational::decimal("0.5").unwrap();
        let third = reference_duration_divide(&p, &alive, 2).unwrap();
        for (bits, numeric) in [(3, true), (4, false)] {
            let mut i = interpreter(&m);
            i.denominator_bits = bits;
            i.probability_stack.push(ReferenceChoices {
                owner: Default::default(),
                deferred_children: vec![],
                leaf: None,
                children: vec![
                    vec![ReferenceBound {
                        values: std::array::from_fn(|_| third.clone()),
                        numeric: false,
                    }],
                    vec![ReferenceBound {
                        values: std::array::from_fn(|_| half.clone()),
                        numeric: false,
                    }],
                ],
            });
            i.probability_finish(Some(&[half.clone(), half.clone()]));
            let b = &i.probability_result.as_ref().unwrap()[0];
            assert_eq!(b.numeric, numeric);
            if !numeric {
                assert_eq!(b.values[0].to_string(), "5/12");
            }
        }
        let q = model(include_str!("../../../fixtures/duration-clock/model.relca"));
        let mut i = interpreter(&q);
        let f = Frame {
            condition_waits: Vec::new(),
            lost_updates: BTreeSet::new(),
            state_snapshots: Vec::new(),
            business: None,
            now: 0,
            steps: 0,
            control: vec![],
            waiting: None,
            tasks: vec![],
            adoption: None,
            trace: vec![],
            state_values: vec![],
        };
        i.probability_stack.push(ReferenceChoices::default());
        i.probability_leaf(&f, None, ReferenceCut::Numeric).unwrap();
        let b = &i.probability_stack.last().unwrap().leaf.as_ref().unwrap()[0];
        assert!(b.numeric);
        assert_eq!(b.values[3], Rational::one());
        let mut m = model(include_str!("../../../fixtures/duration-clock/model.relca"));
        let small = Rational(num_rational::BigRational::new(
            1.into(),
            (num_bigint::BigInt::from(1u8) << 4096usize) + 1u8,
        ));
        m.capabilities[0]
            .duration_distribution
            .as_mut()
            .unwrap()
            .entries = vec![
            (1, small.clone()),
            (3, Rational(Rational::one().0 - small.0)),
        ];
        for query in &mut m.probability_queries {
            query.bound = Rational::one();
            query.within = Some(2);
        }
        let run = explore(&m);
        assert_eq!(run.stop, StopReason::Complete);
        assert!(run.outcome.truncated);
        assert!(
            run.outcome
                .properties
                .iter()
                .all(|p| p.verdict == Verdict::Inconclusive)
        );
        assert!(
            run.outcome
                .diagnostics
                .iter()
                .any(|d| d.code == diag::W_BUDGET_0 && d.message.contains("denominator"))
        );
    }

    #[test]
    fn d_duration_only_numeric_cut_without_queries_or_ends() {
        let mut m = model(include_str!("../../../fixtures/duration-clock/model.relca"));
        let small = Rational(num_rational::BigRational::new(
            1.into(),
            (num_bigint::BigInt::from(1u8) << 4096usize) + 1u8,
        ));
        m.capabilities[0]
            .duration_distribution
            .as_mut()
            .unwrap()
            .entries = vec![
            (1, small.clone()),
            (3, Rational(Rational::one().0 - small.0)),
        ];
        m.probability_queries.clear();
        m.verify = vec![Verify {
            property: Property::NoResourceOvercommit,
            span: m.verify[0].span,
        }];
        m.ends.clear();
        m.process.body.retain(|s| !matches!(s.kind, Stmt::End(_)));
        let run = explore(&m);
        assert_eq!(run.stop, StopReason::Complete);
        assert!(run.outcome.truncated);
        assert_eq!(run.outcome.properties[0].verdict, Verdict::Inconclusive);
        assert!(
            run.outcome
                .diagnostics
                .iter()
                .any(|d| d.code == diag::W_BUDGET_0 && d.message.contains("denominator"))
        );
    }

    #[test]
    fn d_ack_only_has_no_kernel_or_completion_and_long_ack_remains() {
        let m = model(include_str!(
            "../../../fixtures/duration-cancel/model.relca"
        ));
        let mut f = running(&m, 2);
        f.tasks[0].phase = Phase::Finished(ValueId(0));
        f.tasks[0].stop = Some(1);
        f.tasks[1].checked_through = Some(2);
        f.tasks[1].ack = Some(2);
        f.waiting.as_mut().unwrap().winner = Some(BindingId(0));
        f.adoption = Some(1);
        let mut i = interpreter(&m);
        i.run(f.clone(), 0).unwrap();
        assert_eq!(i.statistics.kernel_draws, 0);
        assert_eq!(i.statistics.normal_completions, 0);
        assert_eq!(i.statistics.cancellation_completions, 1);
        assert!(
            i.statistics
                .terminal_traces
                .iter()
                .flatten()
                .all(|o| !matches!(o.event, Occurrence::Completion(CapabilityId(1), _)))
        );
        assert!(
            i.statistics
                .state_seams
                .iter()
                .filter(|(tag, _, _)| *tag == "ack")
                .all(|(_, _, v)| *v == f.state_values)
        );
        let mut i = interpreter(&m);
        f.tasks[1].finish = Some(2);
        i.run(f, 0).unwrap();
        assert_eq!(i.statistics.kernel_draws, 1);
        assert_eq!(i.statistics.normal_completions, 2);
        assert_eq!(i.statistics.cancellation_completions, 1);
        let source = include_str!("../../../fixtures/duration-cancel/model.relca")
            .replace("4min:0.5", "5min:0.5")
            .replace("ack_within 1min", "ack_within 3min");
        let m = model(&source);
        let mut f = running(&m, 1);
        f.tasks[0].phase = Phase::Finished(ValueId(0));
        f.tasks[0].stop = Some(1);
        f.adoption = Some(1);
        f.waiting.as_mut().unwrap().winner = Some(BindingId(0));
        let mut i = interpreter(&m);
        i.probability_stack.push(ReferenceChoices::default());
        i.cancel(f, 0, 2, BindingId(0), 0).unwrap();
        assert_eq!(
            i.statistics
                .duration_ack_candidates
                .iter()
                .copied()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([2, 3, 4])
        );
    }

    #[test]
    fn d_runtime_distribution_defense_independent() {
        let m = parallel();
        for case in 0..10 {
            let mut bad = m.clone();
            let c = &mut bad.capabilities[0];
            let rows = &mut c.duration_distribution.as_mut().unwrap().entries;
            match case {
                0 => rows.clear(),
                1 => rows.swap(0, 2),
                2 => rows[2].0 = 3,
                3 => rows[1].1 = Rational::decimal("-0.1").unwrap(),
                4 => rows[2].1 = Rational::decimal("0.4").unwrap(),
                5 => *rows = vec![(0, Rational::one())],
                6 => c.duration = 1,
                7 => c.actor = ActorId(999),
                8 => bad.probability_queries.push(ProbabilityQuery {
                    condition: Cond::StateEq(StateMatch {
                        variable: StateVarId(999),
                        value: StateValue::None,
                    })
                    .into(),
                    within: Some(0),
                    comparison: Comparison::Le,
                    bound: Rational::one(),
                }),
                9 => bad.actors[0].capacity = None,
                _ => unreachable!(),
            }
            let run = explore(&bad);
            assert_eq!(run.stop, StopReason::SemanticAbort, "case {case}");
            assert!(
                run.outcome
                    .diagnostics
                    .iter()
                    .any(|d| d.code == diag::U_SEM_0)
            );
        }
    }
}

fn extend(range: &mut Range, value: u64) {
    range.min = range.min.min(value);
    range.max = range.max.max(value);
}

#[cfg(test)]
mod state_tests {
    use super::*;
    fn parse(s: &str) -> Model {
        crate::lower::lower(&crate::syntax::parse(s).unwrap()).unwrap()
    }
    fn interpreter(m: &Model) -> Interpreter<'_> {
        Interpreter {
            case_index: 0,
            probability_stack: Vec::new(),
            run_work: None,
            probability_result: None,
            probability_scheduler: false,
            probability_chance: false,
            denominator_bits: 4096,
            duration_numeric_cut: false,
            model: m,
            limits: Limits::default(),
            evidence: Evidence::default(),
            terminals: BTreeMap::new(),
            statistics: Statistics::default(),
            contract_facts: vec![(false, false); m.contracts.len()],
        }
    }
    fn seed(m: &Model) -> Frame<'_> {
        Frame {
            condition_waits: Vec::new(),
            lost_updates: BTreeSet::new(),
            state_snapshots: Vec::new(),
            business: None,
            state_values: m
                .state_variables
                .iter()
                .map(|v| match v.initial {
                    InitialValue::Fixed(x) => x,
                    InitialValue::Nondet | InitialValue::Distribution(_) => {
                        StateValue::Value(ValueId(0))
                    }
                })
                .collect(),
            trace: vec![],
            now: 0,
            steps: 0,
            control: vec![m.process.body.as_slice()],
            waiting: None,
            tasks: vec![],
            adoption: None,
        }
    }

    #[test]
    fn s_latent_product_receipt_snapshot_and_reserve_before_copy() {
        let m = parse(include_str!(
            "../../../fixtures/state-latent-product/model.relca"
        ));
        let r = explore(&m);
        assert_eq!(r.stop, StopReason::Complete);
        let expected = vec![
            vec![StateValue::Value(ValueId(0)), StateValue::Value(ValueId(0))],
            vec![StateValue::Value(ValueId(0)), StateValue::Value(ValueId(1))],
            vec![StateValue::Value(ValueId(1)), StateValue::Value(ValueId(0))],
            vec![StateValue::Value(ValueId(1)), StateValue::Value(ValueId(1))],
        ];
        assert_eq!(r.statistics.initial_tuples, expected);
        assert_eq!(r.statistics.initial_copies, 4);
        assert_eq!(r.statistics.terminal_traces.len(), 4);
        for trace in &r.statistics.terminal_traces {
            assert_eq!(
                trace
                    .iter()
                    .filter(|o| matches!(o.event, Occurrence::Receipt))
                    .count(),
                1
            );
            assert_eq!(trace[0].at, 0);
            assert!(matches!(trace[0].event, Occurrence::Receipt));
            assert!(expected.contains(&trace[0].state_values));
            assert!(
                trace
                    .iter()
                    .all(|o| o.state_values == trace[0].state_values)
            );
        }
        // Isolate the initialization seam: end-only successors need no runtime copies.
        let mut m = m;
        m.process.body = vec![Statement {
            span: m.process.span,
            kind: Stmt::End(EndId(0)),
        }];
        m.process.bindings.clear();
        m.contracts.clear();
        m.verify
            .retain(|v| !matches!(v.property, Property::Contract(_)));
        for (limit, copies, stop) in [
            (5, 4, StopReason::Complete),
            (4, 3, StopReason::EnumerationLimit),
            (1, 0, StopReason::EnumerationLimit),
        ] {
            let r = explore_with_limits(
                &m,
                Limits {
                    frames: limit,
                    ..Limits::default()
                },
            );
            assert_eq!(r.stop, stop);
            assert_eq!(r.statistics.initial_copies, copies);
            assert_eq!(r.statistics.frames, 1 + copies);
        }
        for t in &mut m.types {
            t.members.truncate(1);
        }
        let r = explore_with_limits(
            &m,
            Limits {
                frames: 2,
                ..Limits::default()
            },
        );
        assert_eq!(r.stop, StopReason::Complete);
        assert_eq!(r.statistics.initial_copies, 1);
        assert_eq!(r.statistics.frames, 2);
        let mut empty = m.clone();
        empty.state_variables.clear();
        let r = explore_with_limits(
            &empty,
            Limits {
                frames: 1,
                ..Limits::default()
            },
        );
        assert_eq!(r.stop, StopReason::Complete);
        assert_eq!(r.statistics.frames, 1);
        assert_eq!(r.statistics.initial_copies, 0);
        for v in &mut m.state_variables {
            v.initial = InitialValue::Fixed(StateValue::Value(ValueId(0)));
        }
        let r = explore_with_limits(
            &m,
            Limits {
                frames: 1,
                ..Limits::default()
            },
        );
        assert_eq!(r.stop, StopReason::Complete);
        assert_eq!(r.statistics.initial_copies, 0);
        let mut huge = empty;
        huge.types[0].members = vec!["true".into(), "false".into()];
        huge.state_variables = (0..100)
            .map(|i| StateVariable {
                last_write_wins: false,
                case_set_size: None,
                integer_max: None,
                name: format!("v{i}"),
                kind: StateKind::LatentCase,
                base: TypeId(0),
                optional: false,
                initial: InitialValue::Nondet,
                span: huge.process.span,
            })
            .collect();
        let r = explore_with_limits(
            &huge,
            Limits {
                frames: 3,
                ..Limits::default()
            },
        );
        assert_eq!(r.stop, StopReason::EnumerationLimit);
        assert_eq!(r.statistics.initial_copies, 2);
    }

    #[test]
    fn s_snapshot_history_not_final_and_reverse() {
        let original = r#"model M version "0.2" { type T = enum { ok }; observed case { flag: bool = false }
          actor A { capacity 1; capability set() -> T { writes flag; finish { flag = true }; duration fixed 1min; result nondet { ok } } }
          actor B { capacity 1; capability clear() -> T { writes flag; finish { flag = false }; duration fixed 1min; result nondet { ok } } }
          process P { a=A.set(); b=B.clear(); end done }
          bounds { cases 1;live_tasks 2;clock 3min }
          contract C { invariant A.set.finish -> flag == true; invariant B.clear.finish -> flag == false } }"#;
        for reverse in [false, true] {
            let source = if reverse {
                original.replace("a=A.set(); b=B.clear()", "b=B.clear(); a=A.set()")
            } else {
                original.into()
            };
            let m = parse(&source);
            let r = explore(&m);
            assert_eq!(r.stop, StopReason::Complete);
            assert!(
                r.outcome
                    .properties
                    .iter()
                    .all(|p| p.verdict == Verdict::Satisfied)
            );
            assert_eq!(r.outcome.terminals[0].elapsed, Range { min: 2, max: 2 });
            assert_eq!(r.outcome.terminals[0].work, Range { min: 2, max: 2 });
            let trace = &r.statistics.terminal_traces[0];
            let finishes: Vec<_> = trace
                .iter()
                .filter(|o| matches!(o.event, Occurrence::Completion(_, _)))
                .collect();
            assert_eq!(finishes.len(), 2);
            for o in &finishes {
                let Occurrence::Completion(c, _) = o.event else {
                    unreachable!()
                };
                assert_eq!(o.state_values, vec![StateValue::Value(ValueId(c.0))]);
                assert_eq!(o.at, if c.0 == usize::from(reverse) { 1 } else { 2 });
            }
            let mut f = seed(&m);
            f.now = 2;
            f.trace = trace.clone();
            f.state_values = finishes[1].state_values.clone();
            let i = interpreter(&m);
            let current = Cond::StateEq(StateMatch {
                variable: StateVarId(0),
                value: finishes[1].state_values[0],
            });
            assert!(i.condition(&f, &current, 0).unwrap());
            // Reinspect the whole historical trace after final state is present.
            let mut i = i;
            i.inspect_trace(&f, 2, true, true, false).unwrap();
            assert_eq!(i.contract_facts, vec![(true, false); 2]);
        }
    }

    #[test]
    fn s_same_tick_readwrite_trace_orders() {
        let m = parse(
            r#"model M version "0.2" { type T = enum { ok }; observed case { flag: bool = false }
          actor A { capacity 1; capability run() -> T { writes flag; finish { flag = true }; duration fixed 1min; result nondet { ok } } }
          actor Probe { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } }
          process P { fork f { a=A.run(); b=Probe.run() }; join all f; end done }
          bounds { cases 1;live_tasks 2;clock 2min }; contract C { invariant Probe.run.finish -> flag == true } }"#,
        );
        let r = explore(&m);
        assert_eq!(r.stop, StopReason::Complete);
        for writer_first in [false, true] {
            assert!(r.statistics.terminal_traces.iter().any(|trace| {
                let finish:Vec<_>=trace.iter().filter(|o|matches!(o.event,Occurrence::Completion(_,_))).collect();
                finish.len()==2 && matches!(finish[0].event,Occurrence::Completion(CapabilityId(c),_) if (c==0)==writer_first)
                    && finish.iter().all(|o|o.at==1) && finish.iter().find(|o|matches!(o.event,Occurrence::Completion(CapabilityId(1),_))).unwrap().state_values==vec![StateValue::Value(ValueId(usize::from(!writer_first)))]
                    && trace.iter().filter(|o|matches!(o.event,Occurrence::Dispatch(_))).all(|o|o.at==0)
            }));
        }
    }

    #[test]
    fn s_cancel_snapshots_request_ack_queue_and_forbidden_normal() {
        let source = include_str!("../../../fixtures/state-cancel/model.relca");
        for initial in [false, true] {
            let mut m = parse(&source.replace("fixed 5min", "fixed 2min"));
            let old = StateValue::Value(ValueId(usize::from(!initial)));
            let new = StateValue::Value(ValueId(usize::from(initial)));
            m.state_variables[0].initial = InitialValue::Fixed(old);
            m.capabilities[1].writes[0].source = WriteSource::Constant(new);
            let r = explore(&m);
            assert_eq!(r.stop, StopReason::Complete);
            assert!(
                r.statistics
                    .state_seams
                    .iter()
                    .any(|(kind, c, v)| *kind == "request" && *c == CapabilityId(1) && v == &[old])
            );
            assert!(
                r.statistics
                    .state_seams
                    .iter()
                    .any(|(kind, c, v)| *kind == "ack" && *c == CapabilityId(1) && v == &[old])
            );
            assert!(
                r.statistics
                    .terminal_traces
                    .iter()
                    .any(|t| t.iter().any(|o| o.at == 2
                        && matches!(o.event, Occurrence::Completion(CapabilityId(1), _))
                        && o.state_values == vec![new]))
            );
            m.people = vec![Person {
                name: "p".into(),
                span: m.process.span,
            }];
            for a in &mut m.actors {
                a.executors = Some(vec![PersonId(0)]);
                a.capacity = Some(1);
            }
            let r = explore(&m);
            assert_eq!(r.stop, StopReason::Complete);
            assert!(
                r.statistics
                    .state_seams
                    .iter()
                    .any(|(kind, c, v)| *kind == "queued" && *c == CapabilityId(1) && v == &[old])
            );
            // Explicit Fast-first queue schedule, not inferred from aggregate verdict.
            assert!(r.statistics.terminal_traces.iter().any(|t| {
                t.iter()
                    .any(|o| matches!(o.event, Occurrence::Completion(CapabilityId(0), _)))
                    && !t
                        .iter()
                        .any(|o| matches!(o.event, Occurrence::Dispatch(CapabilityId(1))))
                    && t.iter().all(|o| o.state_values == vec![old])
            }));
            let mut m = parse(source);
            m.state_variables[0].initial = InitialValue::Fixed(old);
            m.capabilities[1].writes[0].source = WriteSource::Constant(new);
            m.capabilities[1].cancel = Cancel::ForbiddenAfterStart;
            let r = explore(&m);
            assert_eq!(r.stop, StopReason::Complete);
            assert!(
                r.statistics
                    .terminal_traces
                    .iter()
                    .all(|t| t.iter().any(|o| matches!(
                        o.event,
                        Occurrence::Completion(CapabilityId(1), _)
                    ) && o.state_values == vec![new]))
            );
        }
    }

    #[test]
    fn s_invalid_state_ir_independently_rejected() {
        let m = parse(include_str!(
            "../../../fixtures/state-invoice-write/model.relca"
        ));
        let mutations: &[fn(&mut Model)] = &[
            |m| m.state_variables[0].base = TypeId(999),
            |m| m.state_variables[0].initial = InitialValue::Fixed(StateValue::Value(ValueId(999))),
            |m| m.state_variables[1].initial = InitialValue::Fixed(StateValue::None),
            |m| m.state_variables[0].initial = InitialValue::Nondet,
            |m| m.state_variables[0].kind = StateKind::LatentCase,
            |m| m.capabilities[0].writes[0].variable = StateVarId(999),
            |m| {
                m.state_variables[0].optional = false;
                m.state_variables[0].initial = InitialValue::Fixed(StateValue::Value(ValueId(0)));
                m.state_variables[0].kind = StateKind::LatentCase;
            },
            |m| {
                let w = m.capabilities[0].writes[0];
                m.capabilities[0].writes.push(w);
            },
            |m| m.capabilities[1].writes[0].source = WriteSource::Result,
            |m| m.capabilities[1].writes[0].source = WriteSource::Constant(StateValue::None),
            |m| {
                if let ContractKind::Invariant { condition, .. } = &mut m.contracts[0].kind {
                    *condition = Some(Cond::StateEq(StateMatch {
                        variable: StateVarId(999),
                        value: StateValue::None,
                    }));
                }
            },
            |m| {
                if let ContractKind::Invariant { condition, .. } = &mut m.contracts[0].kind {
                    *condition = Some(Cond::StateEq(StateMatch {
                        variable: StateVarId(0),
                        value: StateValue::Value(ValueId(999)),
                    }));
                }
            },
            |m| {
                m.capabilities.iter_mut().for_each(|c| c.writes.clear());
                m.state_variables[0].optional = false;
                m.state_variables[0].kind = StateKind::LatentCase;
                m.state_variables[0].initial = InitialValue::Fixed(StateValue::Value(ValueId(0)));
                m.process.body.insert(
                    0,
                    Statement {
                        span: m.process.span,
                        kind: Stmt::If {
                            condition: Cond::StateEq(StateMatch {
                                variable: StateVarId(0),
                                value: StateValue::Value(ValueId(0)),
                            }),
                            then_body: vec![],
                            else_body: vec![],
                        },
                    },
                );
            },
        ];
        for (index, mutate) in mutations.iter().enumerate() {
            let mut bad = m.clone();
            mutate(&mut bad);
            let r = explore(&bad);
            assert_eq!(r.stop, StopReason::SemanticAbort, "case {index}");
            assert!(r.outcome.terminals.is_empty());
        }
    }
}

#[cfg(test)]
mod executor_tests {
    use super::*;

    #[test]
    fn e_reference_new_ir_validation_is_independent_and_closed() {
        let source = r#"model M version "0.2" {
            people [p,q]; type T = enum { ok }
            actor A { executors [p]; capability a() -> T { commits; duration fixed 1min; result nondet { ok } } }
            authority G { basis "test"; grants p to A.a }
            process P { x = A.a(); end done }
            bounds { cases 1; clock 2min; live_tasks 1 }
            contract C { permitted A.a; authority_basis A.a G; separation A.a.start, A.a.finish }
        }"#;
        let base = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
        for case in 0..16 {
            let mut m = base.clone();
            match case {
                0 => m.actors[0].executors = Some(Vec::new()),
                1 => m.actors[0].executors = Some(vec![PersonId(0), PersonId(0)]),
                2 => m.actors[0].executors = Some(vec![PersonId(2)]),
                3 => m.actors[0].capacity = Some(2),
                4 => m.capabilities[0].permission = Some(vec![PersonId(3)]),
                5 => m.authorities[0].grants[0].capability = CapabilityId(3),
                6 => m.authorities[0].grants[0].persons = vec![PersonId(3)],
                7 => m.authorities[0].not_by = vec![PersonId(0)],
                8 => m.authorities[0].issued_by = Some(PersonId(3)),
                9 => m.authorities[0].not_by = vec![PersonId(1), PersonId(1)],
                10 => m.authorities[0].grants.clear(),
                11 => {
                    m.contracts[0].kind = ContractKind::Permitted {
                        capability: CapabilityId(3),
                    }
                }
                12 => {
                    m.contracts[1].kind = ContractKind::AuthorityBasis {
                        capability: CapabilityId(0),
                        authority: AuthorityId(3),
                    }
                }
                13 => m.actors[0].executors = None,
                14 => {
                    if let ContractKind::Separation { left, right } = &mut m.contracts[2].kind {
                        *left = *right;
                    }
                }
                _ => {
                    m.authorities[0].issued_by = Some(PersonId(1));
                    m.authorities[0].not_by = vec![PersonId(1)];
                }
            }
            let result = explore(&m);
            assert_eq!(result.stop, StopReason::SemanticAbort, "case {case}");
            assert!(result.outcome.terminals.is_empty());
            assert!(
                result
                    .outcome
                    .properties
                    .iter()
                    .all(|p| p.verdict == Verdict::Unsupported)
            );
        }
    }

    #[test]
    fn e_reference_separation_checks_every_pair_not_the_latest_left() {
        // Review major-1: path (p,q,p) violates only through the earlier left
        // finish by p. Public paths cannot isolate it (other assignments also
        // violate), so the reference trace inspection is driven directly.
        let model = crate::lower::lower(&crate::syntax::parse(r#"model M version "0.2" {
            people [p,q]; type T = enum { ok }
            actor A { executors [p,q]; capability a() -> T { duration fixed 1min; result nondet { ok } } }
            actor B { executors [p]; capability b() -> T { duration fixed 1min; result nondet { ok } } }
            process P { x = A.a(); y = A.a(); z = B.b(); end done }
            bounds { cases 1; clock 20min; live_tasks 1 }
            contract C { separation A.a.finish, B.b.finish }
            contract D { separation B.b.finish, A.a.finish }
        }"#).unwrap()).unwrap();
        let (a, b) = (CapabilityId(0), CapabilityId(1));
        assert_eq!(model.capabilities[a.0].name, "a");
        assert_eq!(model.capabilities[b.0].name, "b");
        let (p, q) = (PersonId(0), PersonId(1));
        let finish = |c, person, at| Observation {
            binding: None,
            state_values: Vec::new(),
            person: Some(person),
            at,
            event: Occurrence::Completion(c, ValueId(0)),
            values: vec![None; model.process.bindings.len()],
        };
        let inspect = |trace: Vec<Observation>| {
            let mut interpreter = Interpreter {
                case_index: 0,
                probability_stack: Vec::new(),
                run_work: None,
                probability_result: None,
                probability_scheduler: false,
                probability_chance: false,
                denominator_bits: 4096,
                duration_numeric_cut: false,
                model: &model,
                limits: Limits::default(),
                evidence: Evidence::default(),
                terminals: BTreeMap::new(),
                statistics: Statistics::default(),
                contract_facts: vec![(false, false); 2],
            };
            let f = Frame {
                condition_waits: Vec::new(),
                lost_updates: BTreeSet::new(),
                state_snapshots: Vec::new(),
                business: None,
                state_values: Vec::new(),
                now: 3,
                steps: 0,
                control: Vec::new(),
                waiting: None,
                tasks: Vec::new(),
                adoption: None,
                trace,
            };
            let _ = interpreter.inspect_trace(&f, 3, true, true, false);
            interpreter.contract_facts
        };
        let broken = inspect(vec![finish(a, p, 1), finish(a, q, 2), finish(b, p, 3)]);
        assert_eq!(
            broken,
            vec![(true, true); 2],
            "earlier left p meets right p"
        );
        let kept = inspect(vec![finish(a, q, 1), finish(a, q, 2), finish(b, p, 3)]);
        assert_eq!(kept, vec![(true, false); 2]);
    }

    #[test]
    fn e_reference_overcommit_observer_and_release_edges() {
        let model = crate::lower::lower(&crate::syntax::parse(r#"model M version "0.2" {
            people [p]; type T = enum { ok }
            actor A { executors [p]; capability a() -> T { duration fixed 5min; result nondet { ok }; cancel cooperative ack_within 1min } }
            actor B { executors [p]; capability b() -> T { duration fixed 1min; result nondet { ok } } }
            process P { fork f { x = A.a(); y = B.b() }; join all f; end done }
            bounds { cases 1; clock 20min; live_tasks 2 }
            verify safety no_resource_overcommit
        }"#).unwrap()).unwrap();
        let mut interpreter = Interpreter {
            case_index: 0,
            probability_stack: Vec::new(),
            run_work: None,
            probability_result: None,
            probability_scheduler: false,
            probability_chance: false,
            denominator_bits: 4096,
            duration_numeric_cut: false,
            model: &model,
            limits: Limits::default(),
            evidence: Evidence::default(),
            terminals: BTreeMap::new(),
            statistics: Statistics::default(),
            contract_facts: Vec::new(),
        };
        let mut f = Frame {
            condition_waits: Vec::new(),
            lost_updates: BTreeSet::new(),
            state_snapshots: Vec::new(),
            business: None,
            state_values: Vec::new(),
            now: 1,
            steps: 0,
            control: Vec::new(),
            waiting: None,
            tasks: Vec::new(),
            adoption: None,
            trace: Vec::new(),
        };
        let Stmt::Fork { branches, .. } = &model.process.body[0].kind else {
            panic!("fork")
        };
        interpreter.create(&mut f, branches, None).unwrap();
        let p = PersonId(0);
        f.tasks[0].person = Some(p);
        f.tasks[0].phase = Phase::Running;
        f.tasks[0].start = Some(0);
        f.tasks[0].finish = Some(5);
        // A cancellation request schedules ack; it does not stop the running phase.
        f.tasks[0].ack = Some(2);
        assert!(interpreter.can_start(&f, 1).unwrap()); // B has its own capacity.
        assert!(!Interpreter::person_available(&f, p));
        assert_eq!(f.tasks[1].person, None);
        for terminal in [Phase::Cancelled, Phase::Finished(ValueId(0))] {
            let mut released = f.clone();
            released.tasks[0].phase = terminal;
            released.tasks[0].stop = Some(2);
            assert!(Interpreter::person_available(&released, p));
            assert_eq!(released.tasks[0].person, Some(p));
        }
        // No acknowledgement (forbidden cancellation) still occupies until finish.
        f.tasks[0].ack = None;
        assert!(!Interpreter::person_available(&f, p));
        // Broken acquisition: one slot in each Actor, but the same human twice.
        f.tasks[1].phase = Phase::Running;
        f.tasks[1].person = Some(p);
        interpreter.observe_capacity(&f).unwrap();
        assert!(interpreter.evidence.overcommitted);
        interpreter.evidence.truncated = true;
        for stop in [StopReason::Complete, StopReason::SemanticAbort] {
            assert_eq!(
                interpreter
                    .evidence
                    .verdict(Property::NoResourceOvercommit, stop),
                Verdict::Violated
            );
        }
    }
}

#[cfg(test)]
mod range_stop_loss_tests {
    use super::*;

    /// ADR 0041 boundary review m1. The engine keeps a found loss violated
    /// when a range stop ends the search; the single-case reference does too.
    #[test]
    fn range_stop_keeps_found_lost_update_violated() {
        let source = r#"model L version "0.2" {
          type T = enum { ok }
          actor Ledger { capacity 1; state n: 0..2 = 0 last_write_wins; capability add() -> T { writes n; finish { n += 1 }; duration fixed 1min; result nondet { ok } } }
          process P { r = Ledger.add(); end done }
          cut C { include [Ledger] }
          bounds { cases 1; live_tasks 1; clock 5min }
          verify reachable end.done
        }"#;
        let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
        let loss = m
            .verify
            .iter()
            .find_map(|v| match v.property {
                Property::NoLostUpdate(id) => Some(id),
                _ => None,
            })
            .expect("last_write_wins adds no_lost_update");
        let mut i = replay::interpreter(&m);
        let mut f = replay::initial(&m);
        f.lost_updates.insert(loss);
        i.observe_capacity(&f).unwrap();
        let span = crate::diag::Span { start: 0, end: 1 };
        i.statistics.ranges.push(StateRangeViolation {
            case_id: None,
            time: 1,
            binding: "r".into(),
            actor: "Ledger".into(),
            capability: "add".into(),
            phase: "finish".into(),
            variable: "Ledger.n".into(),
            operator: "+=".into(),
            before: "2".into(),
            operand: "1".into(),
            attempted: "3".into(),
            lower: "0".into(),
            upper: "2".into(),
            write_span: span,
            declaration_span: span,
        });
        assert_eq!(
            i.range_stop_verdict(Property::NoLostUpdate(loss)),
            Some(Verdict::Violated)
        );
        // Without the found loss the range stop stays inconclusive.
        let mut j = replay::interpreter(&m);
        j.statistics.ranges = i.statistics.ranges.clone();
        assert_eq!(
            j.range_stop_verdict(Property::NoLostUpdate(loss)),
            Some(Verdict::Inconclusive)
        );
        assert_eq!(
            i.range_stop_verdict(Property::StateInRange),
            Some(Verdict::Violated)
        );
    }
}
