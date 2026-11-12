//! Q6 configuration declarations used by the selected lexical call/route set.
//! This is not a workload/skill estimate and never changes legacy diff rows.
use super::{
    Change, DiffValue, Difference, Kind,
    static_projection::{Shape, capname},
};
use crate::{ir, report::work_profiles::WorkProfiles};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DegradationCondition {
    pub binding: String,
    pub value: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DegradationValue {
    pub condition: DegradationCondition,
    pub producer: String,
}
type Projection = BTreeMap<(Kind, String), DiffValue>;
#[cfg(test)]
#[path = "../../tests/support/p2_cleanup_diff_tests.rs"]
mod p2_cleanup_diff_tests;
fn project(m: &ir::Model, table: Option<&WorkProfiles>) -> Result<Projection, String> {
    #[cfg(test)]
    let table = if p2_cleanup_diff_tests::missing() {
        None
    } else {
        table
    };
    let mut out = BTreeMap::new();
    for id in Shape::new(m).active(m) {
        let c = &m.capabilities[id.0];
        let target = capname(m, id);
        if let Some(ids) = &c.qualified_executors {
            let mut people = ids
                .iter()
                .map(|id| m.people[id.0].name.clone())
                .collect::<Vec<_>>();
            people.sort();
            out.insert(
                (Kind::Qualification, target.clone()),
                DiffValue::Qualification(people),
            );
        }
        if let Some(source) = &c.performance {
            let profile = &m.performances[source.profile];
            let value = table
                .and_then(|t| t.profiles.iter().find(|p| p.name == profile.name))
                .ok_or("invalid_q6_record: q6 comparison missing recorded performance profile")?;
            out.insert(
                (Kind::Performance, target),
                DiffValue::Performance(value.clone()),
            );
        }
    }
    for route in &m.process.degradations {
        let binding = &m.process.bindings[route.condition.binding.0];
        let value = &m.types[binding.ty.0].members[route.condition.value.0];
        out.insert(
            (Kind::Degradation, route.name.clone()),
            DiffValue::Degradation(DegradationValue {
                condition: DegradationCondition {
                    binding: binding.name.clone(),
                    value: value.clone(),
                },
                producer: capname(m, route.producer),
            }),
        );
    }
    Ok(out)
}
pub(crate) fn differences(
    from: &ir::Model,
    from_table: Option<&WorkProfiles>,
    to: &ir::Model,
    to_table: Option<&WorkProfiles>,
) -> Result<Vec<Difference>, String> {
    let a = project(from, from_table)?;
    let z = project(to, to_table)?;
    let keys: BTreeSet<_> = a.keys().chain(z.keys()).cloned().collect();
    Ok(keys
        .into_iter()
        .filter_map(|(kind, target)| {
            let key = (kind, target.clone());
            let from = a.get(&key).cloned().unwrap_or(DiffValue::Inactive);
            let to = z.get(&key).cloned().unwrap_or(DiffValue::Inactive);
            (from != to).then(|| Difference {
                kind,
                target,
                change: if from == DiffValue::Inactive {
                    Change::Added
                } else if to == DiffValue::Inactive {
                    Change::Removed
                } else {
                    Change::Modified
                },
                reason: None,
                loses: None,
                from: Some(from),
                to: Some(to),
                annotations: vec![],
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use crate::ast::DiffKind;
    #[test]
    fn output_only_kinds_do_not_open_dsl_intent_syntax() {
        for kind in [
            DiffKind::Qualification,
            DiffKind::Performance,
            DiffKind::Degradation,
        ] {
            assert_eq!(DiffKind::parse(kind.name()), None);
        }
    }
}
