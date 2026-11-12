//! ADR 0064 structural guard paths. No runtime explorer or probability reducer.
use crate::ir;
use serde::{Deserialize, Serialize};

mod derive;
mod indices;
mod validate;
pub(crate) use derive::derive;
pub(super) use indices::validate as validate_indices;
pub(super) use validate::known_reason;
pub(crate) use validate::{validate_cut, validate_host};

macro_rules! declare_table {
    ($($(#[$attr:meta])* pub $field:ident: $ty:ty,)*) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct Degradation { $(pub $field: $ty,)* }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire { $($(#[$attr])* $field: $ty,)* }
        impl<'de> Deserialize<'de> for Degradation {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self,D::Error> {
                let wire = Wire::deserialize(d)?;
                let table = Self { $($field:wire.$field,)* };
                table.validate_shape().map_err(serde::de::Error::custom)?;
                Ok(table)
            }
        }
    }
}
declare_table! {
    pub rules: String,
    pub status: String,
    pub scope: String,
    #[serde(deserialize_with = "required")]
    pub reason: Option<String>,
    pub basis: String,
    pub unchecked: Vec<String>,
    pub notice: String,
    #[serde(deserialize_with = "required")]
    pub routes: Option<Vec<Route>>,
}
fn required<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub name: String,
    pub condition: Condition,
    pub producer: String,
    pub common_prefix: Vec<Count>,
    pub normal: Side,
    pub degraded: Side,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub binding: String,
    pub value: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Side {
    pub status: String,
    #[serde(deserialize_with = "required")]
    pub reason: Option<String>,
    #[serde(deserialize_with = "required")]
    pub ends: Option<Vec<End>>,
    #[serde(deserialize_with = "required")]
    pub observation: Option<Vec<Observation>>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct End {
    pub end: String,
    pub reachable: bool,
    pub counts: Vec<Count>,
    pub may_require: Vec<String>,
    pub must_require: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Count {
    pub capability: String,
    pub min: u64,
    pub max: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub actor: String,
    pub capability: String,
    pub source: String,
    #[serde(deserialize_with = "required")]
    pub executors: Option<Vec<String>>,
    #[serde(deserialize_with = "required")]
    pub qualified: Option<Vec<String>>,
    #[serde(deserialize_with = "required")]
    pub permitted: Option<Vec<String>>,
}

fn cap_name(m: &ir::Model, i: usize) -> String {
    let c = &m.capabilities[i];
    format!("{}.{}", m.actors[c.actor.0].name, c.name)
}
fn table(routes: Option<Vec<Route>>, reason: Option<&str>) -> Degradation {
    Degradation {
        rules: "agency-degradation-v1".into(),
        status: if reason.is_some() {
            "not_derived"
        } else {
            "derived"
        }
        .into(),
        scope: "structural_guard_paths".into(),
        reason: reason.map(Into::into),
        basis: "unknown".into(),
        unchecked: vec!["retention_basis".into(), "actual_skill".into()],
        notice: "行使回数から不要とは判定しない".into(),
        routes,
    }
}
fn absent(reason: &str) -> Side {
    Side {
        status: "not_derived".into(),
        reason: Some(reason.into()),
        ends: None,
        observation: None,
    }
}
