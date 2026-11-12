//! Wire self-consistency, not authentication of a model or of real skills.
use super::*;
use std::collections::BTreeSet;

fn require(ok: bool, reason: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("invalid degradation: {reason}"))
    }
}
pub(crate) fn validate_cut(
    assumptions: &[ir::Assumption],
    truncated: Option<bool>,
    verdict: crate::Verdict,
) -> Result<(), String> {
    require(
        !(assumptions
            .iter()
            .any(|a| a.field == "agency.degradation.declarations")
            && truncated == Some(true)
            && matches!(verdict, crate::Verdict::Satisfied | crate::Verdict::Vacuous)),
        "truncated degradation cannot be satisfied or vacuous",
    )
}
fn unique(xs: &[String]) -> bool {
    xs.iter().all(|s| !s.is_empty()) && xs.iter().collect::<BTreeSet<_>>().len() == xs.len()
}
fn counts(rows: &[Count], names: &[String]) -> Result<(), String> {
    require(
        rows.iter().map(|r| &r.capability).eq(names.iter()),
        "count inventory",
    )?;
    require(rows.iter().all(|r| r.min <= r.max), "count interval")
}
fn reason(r: &Option<String>) -> bool {
    r.as_deref().is_some_and(known_reason)
}
pub(in crate::agency) fn known_reason(r: &str) -> bool {
    matches!(
        r,
        "derivation_limit"
            | "requires"
            | "shared_state_guard"
            | "calendar"
            | "arrival_coverage"
            | "priority"
            | "resource_impossible"
            | "qualification_unavailable"
            | "unknown_binding"
            | "unsupported_control"
            | "invalid_q6_ir"
            | "multiple_case_interference"
    )
}
impl Degradation {
    pub(super) fn validate_shape(&self) -> Result<(), String> {
        require(
            self.rules == "agency-degradation-v1"
                && self.scope == "structural_guard_paths"
                && self.basis == "unknown"
                && self.unchecked == ["retention_basis", "actual_skill"]
                && self.notice == "行使回数から不要とは判定しない",
            "rules/scope/basis/unchecked",
        )?;
        if self.status == "not_derived" {
            return require(
                reason(&self.reason) && self.routes.is_none(),
                "unknown table must be null",
            );
        }
        require(
            self.status == "derived" && self.reason.is_none(),
            "table status",
        )?;
        let routes = self
            .routes
            .as_ref()
            .ok_or("invalid degradation: missing routes")?;
        require(routes.len() == 1, "route capacity or duplicates")?;
        for r in routes {
            require(
                !r.name.is_empty()
                    && !r.producer.is_empty()
                    && !r.condition.binding.is_empty()
                    && !r.condition.value.is_empty(),
                "route identity",
            )?;
            let names: Vec<_> = r
                .common_prefix
                .iter()
                .map(|c| c.capability.clone())
                .collect();
            require(unique(&names), "duplicate count identity")?;
            counts(&r.common_prefix, &names)?;
            require(names.contains(&r.producer), "producer outside model")?;
            for side in [&r.normal, &r.degraded] {
                if side.status == "not_derived" {
                    require(
                        reason(&side.reason) && side.ends.is_none() && side.observation.is_none(),
                        "unknown side leaks values",
                    )?;
                    continue;
                }
                require(
                    side.status == "derived" && side.reason.is_none(),
                    "side status",
                )?;
                let ends = side
                    .ends
                    .as_ref()
                    .ok_or("invalid degradation: missing ends")?;
                let end_names: Vec<_> = ends.iter().map(|e| e.end.clone()).collect();
                require(
                    unique(&end_names) && end_names.windows(2).all(|w| w[0] < w[1]),
                    "End identities/order",
                )?;
                let mut used = BTreeSet::new();
                for e in ends {
                    counts(&e.counts, &names)?;
                    let mut may: Vec<_> = e
                        .counts
                        .iter()
                        .filter(|c| c.max > 0)
                        .map(|c| c.capability.clone())
                        .collect();
                    may.sort();
                    let mut must: Vec<_> = e
                        .counts
                        .iter()
                        .filter(|c| c.min > 0)
                        .map(|c| c.capability.clone())
                        .collect();
                    must.sort();
                    require(
                        e.may_require == may && e.must_require == must,
                        "may/must inconsistent with counts",
                    )?;
                    require(e.reachable || may.is_empty(), "vacuous dependencies")?;
                    used.extend(may);
                }
                let observation = side
                    .observation
                    .as_ref()
                    .ok_or("invalid degradation: missing observation")?;
                let mut seen = BTreeSet::new();
                for o in observation {
                    require(
                        used.contains(&o.capability)
                            && !o.source.is_empty()
                            && !o.actor.is_empty()
                            && seen.insert((&o.capability, &o.source)),
                        "observation identity",
                    )?;
                    for xs in [&o.executors, &o.qualified, &o.permitted]
                        .into_iter()
                        .flatten()
                    {
                        require(unique(xs), "person identities")?;
                    }
                    if let Some(q) = &o.qualified {
                        require(
                            o.executors
                                .as_ref()
                                .is_some_and(|e| q.iter().all(|p| e.contains(p))),
                            "qualification outside structural executors",
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn validate_host(
    agency: Option<&crate::agency::AgencyDerivation>,
    actors: Option<&[crate::actor_table::Actor]>,
    assumptions: &[ir::Assumption],
    work_profiles: Option<&crate::report::work_profiles::WorkProfiles>,
) -> Result<(), String> {
    let declared: Vec<_> = assumptions
        .iter()
        .filter(|a| a.field == "agency.degradation.declarations")
        .collect();
    let Some(t) = agency.and_then(crate::agency::AgencyDerivation::degradation) else {
        require(declared.is_empty(), "declaration without degradation table")?;
        return Ok(());
    };
    t.validate_shape()?;
    require(declared.len() == 1, "missing declaration provenance")?;
    let scope: Vec<_> = assumptions
        .iter()
        .filter(|a| a.field == "agency.retention.normal.scope")
        .collect();
    require(
        scope.len() == 1 && scope[0].value == "whole_process_paths",
        "missing whole_process_paths retention scope",
    )?;
    let actors = actors.ok_or("invalid degradation: no lowered Actors")?;
    let caps: Vec<_> = actors
        .iter()
        .flat_map(|a| {
            a.capability
                .value
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c.as_str().map(|c| format!("{}.{c}", a.name)))
        })
        .collect();
    #[derive(serde::Deserialize, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    struct Declaration {
        name: String,
        condition: Condition,
        producer: String,
    }
    let parsed: Vec<Declaration> =
        serde_json::from_str(&declared[0].value).map_err(|e| e.to_string())?;
    require(
        parsed.len() == 1
            && parsed.iter().all(|d| {
                !d.name.is_empty()
                    && !d.condition.binding.is_empty()
                    && !d.condition.value.is_empty()
                    && caps.contains(&d.producer)
            }),
        "invalid declaration provenance",
    )?;
    if let Some(routes) = &t.routes {
        let expected: Vec<_> = routes
            .iter()
            .map(|r| Declaration {
                name: r.name.clone(),
                condition: r.condition.clone(),
                producer: r.producer.clone(),
            })
            .collect();
        require(parsed == expected, "condition producer mismatch")?;
    }
    for r in t.routes.iter().flatten() {
        counts(&r.common_prefix, &caps)?;
        for s in [&r.normal, &r.degraded] {
            for o in s.observation.iter().flatten() {
                let a = actors
                    .iter()
                    .find(|a| a.name == o.actor)
                    .ok_or("invalid degradation: unknown Actor")?;
                require(
                    o.capability.starts_with(&format!("{}.", a.name))
                        && caps.contains(&o.capability),
                    "observation capability mismatch",
                )?;
                let expected: Option<Vec<String>> = a.executors.value["ids"].as_array().map(|xs| {
                    xs.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                });
                require(
                    o.executors == expected,
                    "structural executors differ from Actor",
                )?;
                let qualification = work_profiles.and_then(|p| {
                    p.qualifications
                        .iter()
                        .find(|q| q.capability == o.capability)
                });
                if let Some(q) = qualification {
                    let same = |a: &Option<Vec<String>>, b: &Option<Vec<String>>| {
                        a.as_ref().map(|xs| xs.iter().collect::<BTreeSet<_>>())
                            == b.as_ref().map(|xs| xs.iter().collect::<BTreeSet<_>>())
                    };
                    require(
                        q.actor == o.actor
                            && same(&q.executors, &o.executors)
                            && same(&q.qualified_executors, &o.qualified)
                            && same(&q.permitted, &o.permitted),
                        "observation qualification differs from work_profiles",
                    )?;
                } else {
                    require(
                        o.qualified.is_none(),
                        "qualified observation without work_profiles declaration",
                    )?;
                }
                require(
                    actors
                        .iter()
                        .any(|a| a.name == o.source && !a.source.value.is_null()),
                    "observation source outside model",
                )?;
            }
        }
    }
    Ok(())
}
