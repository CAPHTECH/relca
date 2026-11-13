//! Stage-1 repetition projections. A result alias is never an extra execution.
use super::*;
use crate::agency::{AgencyDerivation, effects, viewpoints};
use std::collections::BTreeSet;

#[cfg(test)]
#[path = "round1_repetition_queries.rs"]
mod round1_repetition_queries;

pub(super) fn metrics(m: &ir::Model, metrics: &mut crate::metrics::BusinessMetrics) {
    for case in &mut metrics.cases {
        for task in &mut case.tasks {
            task.repetition = m.loop_site_named(&task.binding).cloned();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    pub rules: String,
    pub coverage: String,
    pub basis: String,
    pub sites: Vec<Site>,
    pub cases: Vec<Case>,
    pub unchecked: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Site {
    pub instance: String,
    pub origin: ir::LoopSite,
    pub source_span: diag::Span,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub case_id: u64,
    pub loop_name: String,
    pub rounds: Option<outcome::Range>,
    pub distribution: Vec<Frequency>,
    pub on_limit: Option<outcome::Range>,
    pub on_limit_probability: Option<crate::probability::ProbabilityRange>,
    pub samples: Option<u64>,
    pub excluded_samples: Option<u64>,
    pub observed_on_limit: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frequency {
    pub rounds: u64,
    pub probability: Option<crate::probability::ProbabilityRange>,
    pub observed_count: Option<u64>,
}

fn table(m: &ir::Model) -> Option<Table> {
    m.has_loops().then(|| Table {
        rules: "bounded-repetition-1".into(), coverage: "not_derived".into(),
        basis: "started body rounds; on_limit only after final again; no probability without a declared probabilistic model".into(),
        sites: m.process.bindings.iter().filter_map(|b| b.repetition.clone().map(|origin| Site { instance: b.name.clone(), origin, source_span: b.span })).collect(),
        cases: vec![], unchecked: vec![],
    })
}

struct QueryLoop {
    name: String,
    n: u64,
    first: usize,
    limit: usize,
}

// These are read-only observations, not a new execution interpretation. Keep
// them off the public model and the user's verification/CI family of queries.
fn queries(m: &ir::Model) -> (ir::Model, Vec<QueryLoop>) {
    let mut q = m.clone();
    let mut loops = Vec::new();
    for (i, v) in m
        .state_variables
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_loop_round())
    {
        let n = v.integer_max.expect("validated loop domain");
        let name = v.name.strip_suffix(":round").expect("loop round variable");
        let named = |role: &str| {
            ir::StateVarId(
                m.state_variables
                    .iter()
                    .position(|v| v.name == format!("{name}:{role}"))
                    .expect("validated loop observation variable"),
            )
        };
        let started = named("started");
        let limit_variable = named("limit");
        let first = q.probability_queries.len();
        for k in 1..=n {
            add_query(
                &mut q,
                ir::Cond::And(vec![
                    ir::Cond::IntCompare {
                        variable: started,
                        operator: ir::IntComparison::Eq,
                        value: 1,
                    },
                    ir::Cond::IntCompare {
                        variable: ir::StateVarId(i),
                        operator: ir::IntComparison::Ge,
                        value: i128::from(k),
                    },
                ]),
                v.span,
            );
        }
        let limit = q.probability_queries.len();
        add_query(
            &mut q,
            ir::Cond::IntCompare {
                variable: limit_variable,
                operator: ir::IntComparison::Eq,
                value: 1,
            },
            v.span,
        );
        loops.push(QueryLoop {
            name: v
                .name
                .trim_start_matches("@loop:")
                .trim_end_matches(":round")
                .into(),
            n,
            first,
            limit,
        });
    }
    (q, loops)
}
fn add_query(m: &mut ir::Model, c: ir::Cond, span: diag::Span) {
    let id = m.probability_queries.len();
    m.probability_queries.push(ir::ProbabilityQuery {
        condition: ir::ProbabilityCondition::State(c),
        within: None,
        comparison: ir::Comparison::Ge,
        bound: crate::probability::Rational::zero(),
    });
    m.verify.push(ir::Verify {
        property: ir::Property::ProbabilityReached(id),
        span,
    });
}
fn probability(
    o: &outcome::Outcome,
    id: usize,
    case: usize,
) -> Option<crate::probability::ProbabilityRange> {
    let p = o
        .properties
        .iter()
        .find(|p| p.property == ir::Property::ProbabilityReached(id))?;
    if let Some(cs) = &p.cases {
        cs.iter().find(|c| c.case_id.0 == case)?.probability.clone()
    } else {
        p.probability.clone()
    }
}

pub(super) fn exhaustive(m: &ir::Model, original: &outcome::Outcome) -> Option<Table> {
    use crate::probability::{ProbabilityRange as P, Rational as R};
    let mut table = table(m)?;
    if original.truncated
        || original
            .diagnostics
            .iter()
            .any(|d| d.severity == diag::Severity::Error)
    {
        table.unchecked.push(
            "iteration_distribution(analysis_incomplete); no cut is an on_limit occurrence".into(),
        );
        return Some(table);
    }
    let (q, loops) = queries(m);
    let e = crate::engine::explore(&q);
    let r = crate::reference::explore(&q);
    if e.truncated
        || r.outcome.truncated
        || r.stop != crate::reference::StopReason::Complete
        || [&e, &r.outcome].iter().any(|o| {
            o.diagnostics
                .iter()
                .any(|d| d.severity == diag::Severity::Error)
        })
    {
        table
            .unchecked
            .push("iteration_distribution(observation_analysis_incomplete)".into());
        return Some(table);
    }
    let mut rows = Vec::new();
    for c in 0..m.bounds.cases as usize {
        for l in &loops {
            let mut tails = vec![P {
                min: R::one(),
                max: R::one(),
            }];
            for id in l.first..=l.limit {
                let (Some(a), Some(b)) = (probability(&e, id, c), probability(&r.outcome, id, c))
                else {
                    table
                        .unchecked
                        .push("iteration_distribution(missing_observation)".into());
                    return Some(table);
                };
                if a != b {
                    table.unchecked.push(format!("iteration_distribution(independent_observation_disagreement): query={id}; engine={a:?}; reference={b:?}"));
                    return Some(table);
                }
                tails.push(a);
            }
            let on_limit = tails.pop().unwrap();
            let min = (1..=l.n)
                .filter(|k| tails[*k as usize].min == R::one())
                .max()
                .unwrap_or(0);
            let max = (1..=l.n)
                .filter(|k| !tails[*k as usize].max.is_zero())
                .max()
                .unwrap_or(0);
            tails.push(P {
                min: R::zero(),
                max: R::zero(),
            });
            let distribution = (0..=l.n)
                .map(|k| Frequency {
                    rounds: k,
                    probability: m.uses_probability().then(|| P {
                        min: R((&tails[k as usize].min.0 - &tails[k as usize + 1].max.0)
                            .max(R::zero().0)),
                        max: R((&tails[k as usize].max.0 - &tails[k as usize + 1].min.0)
                            .min(R::one().0)),
                    }),
                    observed_count: None,
                })
                .collect();
            rows.push(Case {
                case_id: c as u64 + 1,
                loop_name: l.name.clone(),
                rounds: Some(outcome::Range { min, max }),
                distribution,
                on_limit: Some(outcome::Range {
                    min: u64::from(on_limit.min == R::one()),
                    max: u64::from(!on_limit.max.is_zero()),
                }),
                on_limit_probability: m.uses_probability().then_some(on_limit),
                samples: None,
                excluded_samples: None,
                observed_on_limit: None,
            });
        }
    }
    table.coverage = "complete".into();
    table.basis = "engine/reference independently cross-checked event queries; probability intervals retain the model's scheduler relaxation; interval differences are conservative".into();
    table.cases = rows;
    Some(table)
}

pub(super) fn sampled(m: &ir::Model, original: &outcome::SimulationOutcome) -> Option<Table> {
    use outcome::QueryTruth;
    let mut table = table(m)?;
    if original
        .diagnostics
        .iter()
        .any(|d| d.severity == diag::Severity::Error)
        || original.completed_replications == 0
    {
        table
            .unchecked
            .push("iteration_samples(no_completed_run)".into());
        return Some(table);
    }
    let (q, loops) = queries(m);
    let sampled = crate::engine::simulate(&q);
    let same_paths = sampled.replications.len() == original.replications.len()
        && sampled
            .replications
            .iter()
            .zip(&original.replications)
            .all(|(a, b)| {
                a.replication == b.replication
                    && a.draws == b.draws
                    && a.cut == b.cut
                    && a.metrics == b.metrics
                    && a.cases.len() == b.cases.len()
                    && a.cases.iter().zip(&b.cases).all(|(a, b)| {
                        a.case_id == b.case_id && a.arrival == b.arrival && a.terminal == b.terminal
                    })
            });
    if !same_paths {
        table
            .unchecked
            .push("iteration_samples(observation_changed_path)".into());
        return Some(table);
    }
    if sampled
        .diagnostics
        .iter()
        .any(|d| d.severity == diag::Severity::Error)
    {
        table
            .unchecked
            .push("iteration_samples(observation_run_refused)".into());
        return Some(table);
    }
    for case in 1..=m.bounds.cases {
        for l in &loops {
            let mut counts = vec![0u64; l.n as usize + 1];
            let mut limits = 0u64;
            let mut excluded = 0u64;
            for r in &sampled.replications {
                let c = r.cases.iter().find(|c| c.case_id == case);
                let Some(c) = c.filter(|c| {
                    r.cut.is_none()
                        && c.terminal.is_some()
                        && (l.first..=l.limit).all(|id| {
                            matches!(c.queries.get(id), Some(QueryTruth::Hit | QueryTruth::Miss))
                        })
                }) else {
                    excluded += 1;
                    continue;
                };
                let k = (0..l.n as usize)
                    .filter(|k| c.queries[l.first + k] == QueryTruth::Hit)
                    .count();
                counts[k] += 1;
                limits += u64::from(c.queries[l.limit] == QueryTruth::Hit);
            }
            let n: u64 = counts.iter().sum();
            let present: Vec<_> = counts
                .iter()
                .enumerate()
                .filter_map(|(k, n)| (*n > 0).then_some(k as u64))
                .collect();
            table.cases.push(Case {
                case_id: case,
                loop_name: l.name.clone(),
                rounds: present
                    .first()
                    .zip(present.last())
                    .map(|(min, max)| outcome::Range {
                        min: *min,
                        max: *max,
                    }),
                distribution: counts
                    .into_iter()
                    .enumerate()
                    .map(|(k, n)| Frequency {
                        rounds: k as u64,
                        probability: None,
                        observed_count: Some(n),
                    })
                    .collect(),
                on_limit: (n > 0).then_some(outcome::Range {
                    min: u64::from(limits == n),
                    max: u64::from(limits > 0),
                }),
                on_limit_probability: None,
                samples: Some(n),
                excluded_samples: Some(excluded),
                observed_on_limit: Some(limits),
            });
        }
    }
    table.coverage = "sampled".into();
    table.basis = "same seed and per-site random keys; read-only observational rerun; counts describe completed uncut samples, not exhaustive bounds or probability guarantees".into();
    Some(table)
}

pub(super) fn agency(report: &mut Report, m: &ir::Model) {
    if !m.has_loops() {
        return;
    }
    let aliases: BTreeSet<_> = (0..m.process.bindings.len())
        .filter_map(|i| m.loop_alias(ir::BindingId(i)))
        .map(|i| m.site_name(i))
        .collect();
    report.via_model_paths = None;
    report.assumptions.push(Assumption {
        field: "via_model_paths".into(),
        value: "not_derived(loop_control)".into(),
        affects: "反復制御を含む読取り経路は未導出。結果の別名は追加のTaskでも独立根拠でもない。"
            .into(),
    });
    for code in ["W-AGY-0", "W-AGY-2", "W-LIN-2", "W-RESP-2"] {
        report.assumptions.push(Assumption {
            field: format!("agency.diagnostics.{code}"),
            value: "unchecked(loop_control)".into(),
            affects: "反復制御はこの警告の導出対象外。警告の不在は検査成功を意味しない。".into(),
        });
    }
    report.diagnostics.retain(|d| {
        !matches!(
            d.code.as_str(),
            "W-AGY-0" | "W-AGY-2" | "W-LIN-2" | "W-RESP-2"
        )
    });
    for duty in report.responsibilities.as_mut().into_iter().flatten() {
        duty.refusal.status = "not_derived".into();
        duty.refusal.reason = Some("loop_control".into());
        duty.refusal.effects.clear();
        duty.warnings.retain(|w| w != "W-RESP-2");
    }
    let Some(a) = report
        .agency
        .as_mut()
        .and_then(AgencyDerivation::as_derived_mut)
    else {
        return;
    };
    a.refusal.clear();
    a.unchecked.push("loop_control".into());
    a.lineage.retain(|r| !aliases.contains(&r.call));
    if let Some(causes) = a
        .common_causes
        .as_mut()
        .and_then(crate::agency::CommonCauseDerivation::as_derived_mut)
    {
        for c in &mut causes.causes {
            c.targets.retain(|t| !aliases.contains(&t.call));
        }
        causes.causes.retain(|c| c.targets.len() >= 2);
    }
    if let Some(u) = &mut a.model_uncertainties {
        u.items.retain(|r| !aliases.contains(&r.call));
    }
    if let Some(v) = &mut a.viewpoints {
        v.framing = viewpoints::Framing::NotDerived {
            reason: viewpoints::LimitReason::LoopControl,
            complete: false,
            unchecked: vec!["loop_control".into()],
        };
        v.retention = viewpoints::Retention::NotDerived {
            reason: viewpoints::LimitReason::LoopControl,
            notice: "反復の実行回数を静的なcall集合から導かない".into(),
            unchecked: vec!["loop_control".into()],
        };
        if let Some(effects::Effects::Derived { rows, unchecked }) = &mut v.effects {
            rows.retain(|r| !aliases.contains(&r.call));
            unchecked.push("loop_control".into());
            for r in rows {
                r.u = None;
                r.state_refusers.clear();
                let underived = effects::Underived {
                    status: effects::NotDerived::NotDerived,
                    reason: viewpoints::StateReason::LoopControl,
                };
                r.process_judgements = effects::ProcessJudgements::NotDerived(underived.clone());
                r.concentration.generation_roots = effects::Roots::NotDerived(underived);
                r.concentration.single_actor = effects::Truth::Unknown(effects::Unknown::Unknown);
                r.concentration.single_person = effects::Truth::Unknown(effects::Unknown::Unknown);
                r.concentration.single_generation_root =
                    effects::Truth::Unknown(effects::Unknown::Unknown);
            }
        }
    }
}
