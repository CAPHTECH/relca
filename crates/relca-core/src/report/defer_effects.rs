//! Declaration-only explanations of the existing deferred-metrics projection.
use crate::ir;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeferEffects {
    pub process: bool,
    pub entries: Vec<DeferEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "DeferEffectFields")]
pub struct DeferEffect {
    pub target: String,
    pub resolution: DeferResolution,
    pub capabilities: Vec<String>,
    pub withdraws: DeferWithdrawal,
    pub because: Vec<DeferBecause>,
    pub scope: Option<DeferScope>,
}

// `scope` is a required wire key with a nullable value, not an optional key.
// A field deserializer prevents serde's implicit missing-Option default.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeferEffectFields {
    target: String,
    resolution: DeferResolution,
    capabilities: Vec<String>,
    withdraws: DeferWithdrawal,
    because: Vec<DeferBecause>,
    #[serde(deserialize_with = "required_scope")]
    scope: Option<DeferScope>,
}

fn required_scope<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<DeferScope>, D::Error> {
    Option::<DeferScope>::deserialize(d)
}

impl TryFrom<DeferEffectFields> for DeferEffect {
    type Error = &'static str;
    fn try_from(fields: DeferEffectFields) -> Result<Self, Self::Error> {
        if (fields.withdraws == DeferWithdrawal::Scoped) != fields.scope.is_some() {
            return Err("defer scope must be an object for scoped and null for process/none");
        }
        Ok(Self {
            target: fields.target,
            resolution: fields.resolution,
            capabilities: fields.capabilities,
            withdraws: fields.withdraws,
            because: fields.because,
            scope: fields.scope,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeferResolution {
    Actor,
    Capability,
    Opaque,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeferWithdrawal {
    Process,
    Scoped,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeferBecause {
    Opaque,
    Call { capability: String },
    ContestRoute { capability: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeferScope {
    pub actors: Vec<String>,
    pub capabilities: Vec<String>,
    pub people: Vec<String>,
    pub resources: Vec<String>,
}

fn names(values: impl Iterator<Item = String>) -> Vec<String> {
    values.collect::<BTreeSet<_>>().into_iter().collect()
}

impl DeferEffects {
    pub(crate) fn from_model(m: &ir::Model) -> Option<Self> {
        let layers = m.deferred_entries()?;
        let cut = m.cut.as_ref()?;
        let cap = |id: ir::CapabilityId| {
            let c = &m.capabilities[id.0];
            format!("{}.{}", m.actors[c.actor.0].name, c.name)
        };
        let entries = cut
            .defer
            .iter()
            .zip(layers)
            .map(|(entry, d)| {
                let capabilities = names(d.capabilities.iter().copied().map(cap));
                let resolution = if d.opaque {
                    DeferResolution::Opaque
                } else if matches!(entry.target, ir::CutTarget::Actor(_)) {
                    DeferResolution::Actor
                } else {
                    DeferResolution::Capability
                };
                let withdraws = if d.process {
                    DeferWithdrawal::Process
                } else if capabilities.is_empty() {
                    DeferWithdrawal::None
                } else {
                    DeferWithdrawal::Scoped
                };
                let because = if d.opaque {
                    vec![DeferBecause::Opaque]
                } else {
                    names(d.calls.iter().copied().map(cap))
                        .into_iter()
                        .map(|capability| DeferBecause::Call { capability })
                        .chain(
                            names(d.contest_routes.iter().copied().map(cap))
                                .into_iter()
                                .map(|capability| DeferBecause::ContestRoute { capability }),
                        )
                        .collect()
                };
                let scope = (withdraws == DeferWithdrawal::Scoped).then(|| DeferScope {
                    actors: names(
                        m.actors
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| d.actor(m, ir::ActorId(*i)))
                            .map(|(_, a)| a.name.clone()),
                    ),
                    capabilities: capabilities.clone(),
                    people: names(
                        m.people
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| d.person(m, ir::PersonId(*i)))
                            .map(|(_, p)| p.name.clone()),
                    ),
                    resources: names(
                        m.resources
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| d.resource(m, ir::ResourceId(*i)))
                            .map(|(_, r)| r.name.clone()),
                    ),
                });
                DeferEffect {
                    target: match &entry.target {
                        ir::CutTarget::Actor(id) => m.actors[id.0].name.clone(),
                        ir::CutTarget::Label(label) => label.clone(),
                    },
                    resolution,
                    capabilities,
                    withdraws,
                    because,
                    scope,
                }
            })
            .collect::<Vec<_>>();
        Some(Self {
            process: entries
                .iter()
                .any(|e| e.withdraws == DeferWithdrawal::Process),
            entries,
        })
    }
}
