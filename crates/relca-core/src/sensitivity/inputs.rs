//! I04: the actual finite inputs of each selected IR, not instructions inferred
//! from point labels. Capture fixed values as well as stochastic distributions.
use super::{
    contract::names::Names,
    input::{FailureKind, InputError},
};
use crate::{diag::Span, ir, probability::Rational};
use serde::{Deserialize, Serialize};
use serde_json::Value;
type Result<T> = std::result::Result<T, InputError>;
fn bad(detail: &str) -> InputError {
    InputError::new(FailureKind::FeatureUnsupported, detail)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveInputs {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub response_channels: Vec<ResponseChannel>,
    pub rules: String,
    pub initials: Vec<Initial>,
    pub instances: Vec<Instance>,
    pub capabilities: Vec<Operation>,
    pub traffic: Traffic,
    pub bounds: Boundaries,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseChannel {
    pub command: String,
    pub capability: String,
    pub loss: crate::response_inputs::Loss,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Initial {
    pub variable: String,
    pub kind: InitialKind,
    pub entries: Vec<Mass>,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitialKind {
    Fixed,
    Distribution,
    Nondeterministic,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mass {
    pub value: Value,
    pub probability: Rational,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instance {
    pub actor: String,
    pub instance: Option<ir::ActorInstance>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "OperationWire")]
pub struct Operation {
    pub capability: String,
    pub span: Span,
    pub duration: Duration,
    pub kernel: Option<Kernel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance: Option<Performance>,
    pub kernel_reads_latent: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationWire {
    capability: String,
    span: Span,
    duration: Duration,
    kernel: Option<Kernel>,
    #[serde(default, deserialize_with = "declared_performance")]
    performance: Option<Performance>,
    kernel_reads_latent: Vec<String>,
}
// Missing means undeclared; a present key must contain the declared object.
// Option::deserialize would collapse explicit null into missing on re-encoding.
fn declared_performance<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<Performance>, D::Error> {
    Performance::deserialize(d).map(Some)
}
impl TryFrom<OperationWire> for Operation {
    type Error = String;
    fn try_from(w: OperationWire) -> std::result::Result<Self, String> {
        if let Some(p) = &w.performance {
            if w.kernel.is_some()
                || p.profile.is_empty()
                || p.basis.trim().is_empty()
                || p.snapshot != "start"
                || p.duration_effect != "not_supported"
                || !matches!(p.load.as_str(), "none" | "queued_same_actor_at_start")
                || p.rows.is_empty()
            {
                return Err("invalid effective performance source".into());
            }
            let mut ends = std::collections::BTreeMap::new();
            for row in &p.rows {
                if row.executor.is_empty() || row.kernel.name.is_empty() {
                    return Err("invalid effective performance row".into());
                }
                let valid = if p.load == "none" {
                    row.lower.is_none() && row.upper.is_none() && !ends.contains_key(&row.executor)
                } else {
                    row.lower.is_some()
                        && ends.get(&row.executor).copied().unwrap_or(Some(0)) == row.lower
                        && row
                            .upper
                            .is_none_or(|hi| row.lower.is_some_and(|lo| lo < hi))
                };
                if !valid {
                    return Err("invalid effective performance bands".into());
                }
                ends.insert(&row.executor, row.upper);
            }
            if ends.values().any(Option::is_some) {
                return Err("incomplete effective performance bands".into());
            }
        }
        Ok(Self {
            capability: w.capability,
            span: w.span,
            duration: w.duration,
            kernel: w.kernel,
            performance: w.performance,
            kernel_reads_latent: w.kernel_reads_latent,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Performance {
    pub profile: String,
    pub basis: String,
    pub load: String,
    pub snapshot: String,
    pub duration_effect: String,
    pub rows: Vec<PerformanceRow>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceRow {
    pub executor: String,
    #[serde(deserialize_with = "required_option")]
    pub lower: Option<u64>,
    #[serde(deserialize_with = "required_option")]
    pub upper: Option<u64>,
    pub kernel: Kernel,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Duration {
    Fixed { minutes: ir::Tick },
    Distribution { entries: Vec<DurationMass> },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DurationMass {
    pub minutes: ir::Tick,
    pub probability: Rational,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kernel {
    pub name: String,
    pub basis: Option<String>,
    pub given: Vec<String>,
    pub rows: Vec<KernelRow>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelRow {
    pub conditions: Vec<Assignment>,
    pub branches: Vec<Branch>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub variable: String,
    pub value: Value,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Branch {
    pub probability: Rational,
    pub result: String,
    pub writes: Vec<Assignment>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Traffic {
    #[serde(deserialize_with = "required_option")]
    pub arrivals: Option<Vec<ir::Tick>>,
    pub distribution: Option<ArrivalDistribution>,
    pub fixed_every: Option<FixedEvery>,
    pub horizon: Option<ir::Tick>,
    pub scenario: Option<String>,
}
fn required_option<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> std::result::Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArrivalDistribution {
    pub basis: String,
    pub entries: Vec<ArrivalMass>,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArrivalMass {
    pub times: Vec<ir::Tick>,
    pub probability: Rational,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixedEvery {
    pub every: ir::Tick,
    pub from: ir::Tick,
    pub until: ir::Tick,
    pub count: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boundaries {
    pub cases: u64,
    pub clock: ir::Tick,
    pub live_tasks: u64,
}

impl EffectiveInputs {
    pub fn from_model(m: &ir::Model) -> Result<Self> {
        let n = Names(m);
        let assignment = |id, v| -> Result<Assignment> {
            Ok(Assignment {
                variable: n.state(id)?,
                value: n.state_value(id, v)?,
            })
        };
        let initials = m
            .state_variables
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let id = ir::StateVarId(i);
                let (kind, entries) = match v.initial {
                    ir::InitialValue::Fixed(value) => (
                        InitialKind::Fixed,
                        vec![Mass {
                            value: n.state_value(id, value)?,
                            probability: Rational::one(),
                        }],
                    ),
                    ir::InitialValue::Nondet => (InitialKind::Nondeterministic, vec![]),
                    ir::InitialValue::Distribution(index) => {
                        let rows = m
                            .initial_distributions
                            .get(index)
                            .ok_or_else(|| bad("initial distribution reference"))?;
                        (
                            InitialKind::Distribution,
                            rows.iter()
                                .map(|(v, p)| {
                                    Ok(Mass {
                                        value: n.state_value(id, *v)?,
                                        probability: p.clone(),
                                    })
                                })
                                .collect::<Result<Vec<_>>>()?,
                        )
                    }
                };
                Ok(Initial {
                    variable: n.state(id)?,
                    kind,
                    entries,
                    span: v.span,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let instances = m
            .actors
            .iter()
            .map(|a| Instance {
                actor: a.name.clone(),
                instance: a.metadata.instance,
            })
            .collect();
        let capabilities = m
            .capabilities
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let project_kernel = |k: &ir::Kernel| -> Result<Kernel> {
                    let rows = k
                        .rows
                        .iter()
                        .map(|r| {
                            if r.values.len() != k.given.len() {
                                return Err(bad("kernel condition shape"));
                            }
                            Ok(KernelRow {
                                conditions: k
                                    .given
                                    .iter()
                                    .zip(&r.values)
                                    .map(|(id, v)| assignment(*id, *v))
                                    .collect::<Result<Vec<_>>>()?,
                                branches: r
                                    .branches
                                    .iter()
                                    .map(|b| {
                                        Ok(Branch {
                                            probability: b.probability.clone(),
                                            result: n.value(c.return_type, b.result)?,
                                            writes: b
                                                .overrides
                                                .iter()
                                                .map(|(id, v)| assignment(*id, *v))
                                                .collect::<Result<Vec<_>>>()?,
                                        })
                                    })
                                    .collect::<Result<Vec<_>>>()?,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    Ok(Kernel {
                        name: k.name.clone(),
                        basis: k.basis.clone(),
                        given: k
                            .given
                            .iter()
                            .map(|id| n.state(*id))
                            .collect::<Result<Vec<_>>>()?,
                        rows,
                    })
                };
                if c.kernel.is_some() && c.performance.is_some() {
                    return Err(bad(
                        "ordinary kernel and performance are mutually exclusive",
                    ));
                }
                let kernel = c.kernel.as_ref().map(project_kernel).transpose()?;
                let performance = c
                    .performance
                    .as_ref()
                    .map(|source| -> Result<Performance> {
                        let p = m
                            .performances
                            .get(source.profile)
                            .ok_or_else(|| bad("performance reference"))?;
                        if p.rows.len() != source.kernels.len() {
                            return Err(bad("performance effective kernel coverage"));
                        }
                        let rows = p
                            .rows
                            .iter()
                            .zip(&source.kernels)
                            .map(|(r, k)| {
                                Ok(PerformanceRow {
                                    executor: r
                                        .executor
                                        .map(|id| n.person(id))
                                        .transpose()?
                                        .unwrap_or_else(|| "anonymous".into()),
                                    lower: p.queued.then_some(r.from),
                                    upper: if p.queued { r.until } else { None },
                                    kernel: project_kernel(k)?,
                                })
                            })
                            .collect::<Result<Vec<_>>>()?;
                        Ok(Performance {
                            profile: p.name.clone(),
                            basis: p.basis.clone(),
                            load: if p.queued {
                                "queued_same_actor_at_start"
                            } else {
                                "none"
                            }
                            .into(),
                            snapshot: "start".into(),
                            duration_effect: "not_supported".into(),
                            rows,
                        })
                    })
                    .transpose()?;
                let duration = match &c.duration_distribution {
                    None => Duration::Fixed {
                        minutes: c.duration,
                    },
                    Some(d) => Duration::Distribution {
                        entries: d
                            .entries
                            .iter()
                            .map(|(t, p)| DurationMass {
                                minutes: *t,
                                probability: p.clone(),
                            })
                            .collect(),
                    },
                };
                Ok(Operation {
                    capability: n.capability(ir::CapabilityId(i))?,
                    span: c.span,
                    duration,
                    kernel,
                    performance,
                    kernel_reads_latent: c
                        .kernel_reads_latent
                        .iter()
                        .map(|id| n.state(*id))
                        .collect::<Result<Vec<_>>>()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            rules: "sensitivity-inputs-p2-1".into(),
            response_channels: m
                .response_attempts()
                .into_iter()
                .filter(|(_, r)| r.attempt == 1)
                .map(|(_, r)| {
                    let c = crate::response_inputs::control(m, r);
                    ResponseChannel {
                        command: m.site_name(r.command),
                        capability: c.capability,
                        loss: c.loss,
                    }
                })
                .collect(),
            initials,
            instances,
            capabilities,
            traffic: Traffic {
                arrivals: if m.arrival_distribution.is_some() {
                    None
                } else {
                    m.arrivals
                        .clone()
                        .or_else(|| m.arrival_every.is_none().then(|| vec![0]))
                },
                distribution: m
                    .arrival_distribution
                    .as_ref()
                    .map(|d| ArrivalDistribution {
                        basis: d.basis.clone(),
                        span: d.span,
                        entries: d
                            .entries
                            .iter()
                            .map(|(times, p)| ArrivalMass {
                                times: times.clone(),
                                probability: p.clone(),
                            })
                            .collect(),
                    }),
                fixed_every: m.arrival_every.as_ref().map(|e| FixedEvery {
                    every: e.every,
                    from: e.from,
                    until: e.until,
                    count: e.count,
                }),
                horizon: m.horizon,
                scenario: m.scenario.as_ref().map(|s| s.name.clone()),
            },
            bounds: Boundaries {
                cases: m.bounds.cases,
                clock: m.bounds.clock,
                live_tasks: m.bounds.live_tasks,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn capture(source: &str) -> EffectiveInputs {
        let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
        EffectiveInputs::from_model(&m).unwrap()
    }
    #[test]
    fn full_actual_inputs_keep_fixed_and_zero_mass_entries_and_names() {
        let b = capture(include_str!("../../tests/data/sensitivity/base.relca"));
        let h = capture(include_str!("../../tests/data/sensitivity/high.relca"));
        assert_eq!(b.initials[0].variable, "case.latent.wrong");
        assert_ne!(b.initials, h.initials);
        assert_eq!(b.capabilities, h.capabilities);
        assert_eq!(b.traffic.arrivals, Some(vec![0]));
        let zero = capture(include_str!("../../tests/data/sensitivity/zero.relca"));
        assert!(zero.initials.iter().any(|i| i.kind == InitialKind::Fixed));
        assert_eq!(
            zero.capabilities[0].duration,
            Duration::Fixed { minutes: 1 }
        );
        let p0 = capture(include_str!("../../tests/data/sensitivity/p0.relca"));
        assert!(
            p0.capabilities
                .iter()
                .filter_map(|c| c.kernel.as_ref())
                .flat_map(|k| &k.rows)
                .flat_map(|r| &r.branches)
                .any(|b| b.probability.is_zero())
        );
        let encoded = json!(b);
        assert_eq!(
            serde_json::from_value::<EffectiveInputs>(encoded.clone()).unwrap(),
            b
        );
        let mut extra = encoded;
        extra["traffic"]["implicit_cartesian"] = json!(true);
        assert!(serde_json::from_value::<EffectiveInputs>(extra).is_err());
    }
}
