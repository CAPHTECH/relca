//! Validated report views, not another analyzer. IDs, never vector positions,
//! identify cases. The executable IR supplies the expected inventory.
use crate::{
    Verdict, ir,
    probability::{ProbabilityRange, Rational},
    report,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) type Result<T> = std::result::Result<T, &'static str>;
pub(crate) const SAFETY: &str = "safety no_resource_overcommit";
pub(crate) const RECEIPT_NOT_APPLICABLE: &str = "receipt_resolution_not_applicable";

fn loss_id(m: &ir::Model, id: &str) -> bool {
    m.verify
        .iter()
        .any(|v| matches!(v.property, ir::Property::NoLostUpdate(x) if m.loss_property_id(x) == id))
}

// Sampling reports range safety globally, unlike the per-case exhaustive row.
// Keep both sample shape branches on this closed, IR-backed inventory; a sample
// without a witness remains inconclusive, not a proof of safety.
fn sample_global_safety_id(m: &ir::Model, id: &str) -> bool {
    m.verify.iter().any(|v| match v.property {
        ir::Property::NoResourceOvercommit => id == SAFETY,
        ir::Property::NoLostUpdate(x) => m.loss_property_id(x) == id,
        ir::Property::StateInRange => id == "state_in_range",
        _ => false,
    })
}

#[cfg(test)]
mod sample_safety_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn range_samples_keep_global_shape_and_nonproof_verdicts() {
        for overflow in [false, true] {
            let mut source = include_str!("../../../fixtures/int-arrivals/model.relca").replace(
                "\n}",
                "\n simulation replications 2 seed 7 confidence 0.5\n}",
            );
            if overflow {
                source = source.replace("n += 1", "n += 2");
            }
            let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
            let report = report::simulate(&source);
            let view = simulation(&report, &m).unwrap();
            let row = view
                .properties
                .iter()
                .find(|p| p.id == "state_in_range")
                .unwrap();
            let expected = if overflow {
                Verdict::Violated
            } else {
                Verdict::Inconclusive
            };
            assert_eq!(row.aggregate, expected);
            assert_eq!(row.values.len(), 1);
            let value = &row.values[&None];
            assert_eq!(value.verdict, expected);
            assert_eq!(value.witness.is_some(), overflow);

            let index = report
                .properties
                .iter()
                .position(|p| p["id"] == "state_in_range")
                .unwrap();
            let mut altered = report.clone();
            altered.properties[index] = json!({"id":"state_in_range","verdict":expected,"cases":[
                {"case_id":1,"arrival":0,"verdict":"inconclusive","basis":"sample","witness":null},
                {"case_id":2,"arrival":0,"verdict":"inconclusive","basis":"sample","witness":null}
            ]});
            assert_eq!(
                simulation(&altered, &m).err(),
                Some("global property has cases")
            );

            let mut altered = report.clone();
            altered.properties[index]["id"] = json!("unknown_safety");
            assert!(simulation(&altered, &m).is_err());

            for fault in 0..4 {
                let mut altered = report.clone();
                match fault {
                    0 => {
                        altered.case_outcomes[0]
                            .as_object_mut()
                            .unwrap()
                            .remove("range_stopped_count");
                    }
                    1 => altered.case_outcomes[0]["range_stopped_count"] = json!(null),
                    2 => altered.case_outcomes[0]["range_stopped_count"] = json!(3),
                    3 => altered.case_outcomes[0]["unknown_count"] = json!(0),
                    _ => unreachable!(),
                }
                assert!(
                    simulation(&altered, &m).is_err(),
                    "range count fault {fault}"
                );
            }

            let mut altered = report.clone();
            let index = altered
                .properties
                .iter()
                .position(|p| p["id"] == "reachable end.done")
                .unwrap();
            altered.properties[index] = json!({"id":"reachable end.done","verdict":"inconclusive","basis":"sample","witness":null});
            assert_eq!(
                simulation(&altered, &m).err(),
                Some("nonglobal property lacks cases")
            );
        }
    }
}

#[cfg(test)]
mod metrics_tests {
    #[test]
    fn corrupt_public_metric_inventories_fail_closed_in_projection() {
        let source = include_str!("../../../fixtures/met-compare/model.relca");
        let model =
            crate::lower::lower_configuration(&crate::syntax::parse(source).unwrap(), "AsIs")
                .unwrap()
                .model;
        let report = crate::report::check_configuration(source, "AsIs");
        let original = report.metrics.unwrap();
        assert!(super::validate_metrics(Some(&original), &model, false).is_ok());
        for fault in 0..9 {
            let mut altered = original.clone();
            match fault {
                0 => altered.window += 1,
                1 => altered.currency = "USD".into(),
                2 => altered.cases[0].terminals[0].end = Some("unknown".into()),
                3 => altered.terminals.clear(),
                4 => altered.cases[0].tasks.pop().map(|_| ()).unwrap(),
                5 => altered.cases[0].tasks[0].capability = "unknown".into(),
                6 => altered.rates.capabilities[0].required = false,
                7 => altered.actors[0].capacity = Some(99),
                8 => {
                    let row = altered.people.first().cloned();
                    if let Some(row) = row {
                        altered.people.push(row);
                    } else {
                        altered.capabilities.push(altered.capabilities[0].clone());
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                super::validate_metrics(Some(&altered), &model, false).is_err(),
                "fault {fault}"
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricScope {
    Overall,
    Case,
    CaseTerminal,
    Task,
    Terminal,
    Actor,
    Person,
    Capability,
    Resource,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricKey {
    pub scope: MetricScope,
    pub case_id: Option<u64>,
    pub end: Option<String>,
    pub subject: Option<String>,
    pub metric: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetricValue {
    Count(crate::metrics::Count),
    Scalar(crate::metrics::Scalar),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricStatus {
    Reported,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricCell {
    pub status: MetricStatus,
    pub value: Option<MetricValue>,
}

pub(crate) fn metric_rows(
    m: &crate::metrics::BusinessMetrics,
    model: &ir::Model,
) -> Result<BTreeMap<MetricKey, MetricValue>> {
    use serde_json::Value;
    fn leaves(
        value: &Value,
        key: &MetricKey,
        out: &mut BTreeMap<MetricKey, MetricValue>,
    ) -> Result<()> {
        match value {
            Value::Object(fields) if fields.contains_key("stats") || fields.contains_key("min") => {
                let cell =
                    serde_json::from_value(value.clone()).map_err(|_| "invalid metric scalar")?;
                if out.insert(key.clone(), cell).is_some() {
                    return Err("duplicate metric key");
                }
            }
            Value::Object(fields) => {
                for (name, value) in fields {
                    if name == "business_value" {
                        continue;
                    }
                    // ADR 0059: these columns have no comparison-table or
                    // objective contract. Do not discover them by recursion.
                    if key.metric.is_empty()
                        && matches!(
                            name.as_str(),
                            "scheduled_capacity_time"
                                | "scheduled_occupancy_time"
                                | "overtime_occupancy_time"
                                | "scheduled_utilization"
                                | "overtime_work"
                                | "tariff_regular_work"
                                | "tariff_overtime_work"
                                | "tariff_regular_cost"
                                | "tariff_overtime_cost"
                                | "arrival_time"
                        )
                    {
                        continue;
                    }
                    if key.metric.is_empty()
                        && matches!(
                            (key.scope, name.as_str()),
                            (MetricScope::Overall, "queue_wait_observed")
                                | (MetricScope::Case, "dwell" | "joins")
                                | (MetricScope::CaseTerminal, "breakdown")
                                | (MetricScope::Task, "wait_causes" | "predecessor_wait")
                                | (MetricScope::Actor, "queue")
                        )
                    {
                        continue;
                    }
                    let mut child = key.clone();
                    child.metric = if key.metric.is_empty() {
                        name.clone()
                    } else {
                        format!("{}.{name}", key.metric)
                    };
                    leaves(value, &child, out)?;
                }
            }
            Value::Array(rows) if key.metric.ends_with("end_counts") => {
                for row in rows {
                    let mut child = key.clone();
                    child.metric = format!(
                        "{}.{}",
                        key.metric,
                        row["end"].as_str().ok_or("missing metric end")?
                    );
                    leaves(&row["count"], &child, out)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn key(
        scope: MetricScope,
        case_id: Option<u64>,
        end: Option<String>,
        subject: Option<String>,
    ) -> MetricKey {
        MetricKey {
            scope,
            case_id,
            end,
            subject,
            metric: String::new(),
        }
    }
    let mut out = BTreeMap::new();
    let raw = serde_json::to_value(m).map_err(|_| "invalid metrics wire")?;
    leaves(
        &raw["overall"],
        &key(MetricScope::Overall, None, None, None),
        &mut out,
    )?;
    for case in &m.cases {
        let mut row = serde_json::to_value(case).map_err(|_| "invalid case metrics")?;
        row.as_object_mut()
            .ok_or("invalid case metrics")?
            .remove("terminals");
        row.as_object_mut().unwrap().remove("tasks");
        leaves(
            &row,
            &key(MetricScope::Case, Some(case.case_id), None, None),
            &mut out,
        )?;
        for terminal in &case.terminals {
            leaves(
                &serde_json::to_value(terminal).map_err(|_| "invalid terminal metrics")?,
                &key(
                    MetricScope::CaseTerminal,
                    Some(case.case_id),
                    terminal.end.clone(),
                    None,
                ),
                &mut out,
            )?;
        }
        for task in &case.tasks {
            let binding = model
                .process
                .bindings
                .iter()
                .position(|b| b.name == task.binding)
                .ok_or("metric task binding is outside selected model")?;
            leaves(
                &serde_json::to_value(task).map_err(|_| "invalid task metrics")?,
                &key(
                    MetricScope::Task,
                    Some(case.case_id),
                    None,
                    Some(model.comparison_site_name(ir::BindingId(binding))),
                ),
                &mut out,
            )?;
        }
    }
    for terminal in &m.terminals {
        leaves(
            &serde_json::to_value(terminal).map_err(|_| "invalid terminal metrics")?,
            &key(
                MetricScope::Terminal,
                None,
                Some(terminal.end.clone()),
                None,
            ),
            &mut out,
        )?;
    }
    for actor in &m.actors {
        leaves(
            &serde_json::to_value(actor).map_err(|_| "invalid Actor metrics")?,
            &key(MetricScope::Actor, None, None, Some(actor.actor.clone())),
            &mut out,
        )?;
    }
    for resource in m.resources.iter().flatten() {
        leaves(
            &serde_json::to_value(resource).map_err(|_| "invalid resource metrics")?,
            &key(
                MetricScope::Resource,
                None,
                None,
                Some(resource.resource.clone()),
            ),
            &mut out,
        )?;
    }
    for person in &m.people {
        leaves(
            &serde_json::to_value(person).map_err(|_| "invalid person metrics")?,
            &key(MetricScope::Person, None, None, Some(person.person.clone())),
            &mut out,
        )?;
    }
    for capability in &m.capabilities {
        leaves(
            &serde_json::to_value(capability).map_err(|_| "invalid Capability metrics")?,
            &key(
                MetricScope::Capability,
                None,
                None,
                Some(capability.capability.clone()),
            ),
            &mut out,
        )?;
    }
    Ok(out)
}

pub(crate) fn metric_cell(value: Option<&MetricValue>) -> MetricCell {
    MetricCell {
        status: if value.is_some() {
            MetricStatus::Reported
        } else {
            MetricStatus::NotApplicable
        },
        value: value.cloned(),
    }
}

pub(crate) fn validate_metrics(
    value: Option<&crate::metrics::BusinessMetrics>,
    m: &ir::Model,
    sample: bool,
) -> Result<()> {
    let (Some(spec), Some(value)) = (&m.metrics, value) else {
        return if m.metrics.is_none() && value.is_none() {
            Ok(())
        } else {
            Err("metrics presence differs from declaration")
        };
    };
    if value.rules
        != if m.resources.is_empty() {
            "business-metrics-p1-1"
        } else {
            "business-metrics-p1-resource-1"
        }
        || value.window != spec.window
        || value.currency != spec.currency
        || value.time_basis != "arrival_to_declared_end"
        || value.lead_time_scope != "terminal_cases_only"
        || value.observation_scope
            != if m.horizon.is_some() {
                "until_horizon_or_execution_stop"
            } else {
                "until_execution_stop_no_extra_drain"
            }
        || value.statistical_unit.as_deref() != sample.then_some("replication")
    {
        return Err("metrics metadata differs from declaration or mode");
    }
    require(
        value.overall.counts.cut_off.is_some() == m.horizon.is_some()
            && value.cases.iter().all(|c| {
                c.counts.cut_off.is_some() == m.horizon.is_some()
                    && c.terminals
                        .iter()
                        .all(|t| t.excluded.cut_off.is_some() == m.horizon.is_some())
            }),
        "metric cutoff inventory differs from horizon declaration",
    )?;
    fn unique<T: Ord>(values: impl Iterator<Item = T>) -> bool {
        let mut seen = BTreeSet::new();
        values.into_iter().all(|v| seen.insert(v))
    }
    match &value.resources {
        None if m.resources.is_empty() => {}
        Some(rows)
            if !m.resources.is_empty()
                && rows.len() == m.resources.len()
                && rows
                    .iter()
                    .zip(&m.resources)
                    .all(|(row, r)| row.resource == r.name && row.capacity == r.capacity)
                && unique(rows.iter().map(|r| &r.resource)) => {}
        _ => return Err("metric shared resource inventory differs from declaration"),
    }
    let resource_words = [
        "resource_occupancy_time",
        "resource_peak_units",
        "resource_capacity_window_utilization",
    ];
    let new_words = [
        ("calendar_start_gates", m.resource_time.is_some()),
        ("calendar_capacity_integrals", m.resource_time.is_some()),
        (
            "condition_wait_partition",
            m.has_condition_wait() && spec.breakdown_waits,
        ),
    ];
    for (word, present) in new_words {
        if value.checked.iter().filter(|v| v.as_str() == word).count() != usize::from(present) {
            return Err("resource time checked vocabulary differs");
        }
    }
    let legacy_checked: Vec<_> = value
        .checked
        .iter()
        .filter(|v| !new_words.iter().any(|(w, _)| v.as_str() == *w))
        .cloned()
        .collect();
    let presence = |fields: &[&Option<crate::metrics::Scalar>], required: bool| -> Result<()> {
        for field in fields {
            if field.is_some() != required {
                return Err("resource time quantity presence differs");
            }
            if let Some(scalar) = field
                && matches!(scalar, crate::metrics::Scalar::Sample(_)) != sample
            {
                return Err("resource time quantity mode differs");
            }
        }
        Ok(())
    };
    for actor in &value.actors {
        let id = m
            .actors
            .iter()
            .position(|a| a.name == actor.actor)
            .ok_or("unknown resource time Actor")?;
        let scheduled = m
            .resource_time
            .as_ref()
            .is_some_and(|t| t.actors[id].is_some());
        presence(
            &[
                &actor.scheduled_capacity_time,
                &actor.scheduled_occupancy_time,
                &actor.overtime_occupancy_time,
                &actor.scheduled_utilization,
            ],
            scheduled,
        )?;
        presence(
            &[
                &actor.tariff_regular_work,
                &actor.tariff_overtime_work,
                &actor.tariff_regular_cost,
                &actor.tariff_overtime_cost,
            ],
            m.resource_time.is_some(),
        )?;
    }
    for resource in value.resources.iter().flatten() {
        let id = m
            .resources
            .iter()
            .position(|r| r.name == resource.resource)
            .ok_or("unknown resource time resource")?;
        presence(
            &[
                &resource.scheduled_capacity_time,
                &resource.scheduled_occupancy_time,
                &resource.overtime_occupancy_time,
                &resource.scheduled_utilization,
            ],
            m.resource_time
                .as_ref()
                .is_some_and(|t| t.resources[id].is_some()),
        )?;
    }
    for person in &value.people {
        let id = m
            .people
            .iter()
            .position(|p| p.name == person.person)
            .ok_or("unknown resource time person")?;
        presence(
            &[&person.overtime_work],
            m.resource_time
                .as_ref()
                .is_some_and(|t| t.people[id].is_some()),
        )?;
    }
    let extended = m.resource_time.is_some() || m.has_condition_wait();
    for case in &value.cases {
        for task in &case.tasks {
            presence(
                &[
                    &task.tariff_regular_work,
                    &task.tariff_overtime_work,
                    &task.tariff_regular_cost,
                    &task.tariff_overtime_cost,
                ],
                m.resource_time.is_some(),
            )?;
            if let Some(cause) = &task.wait_causes {
                presence(&[&cause.calendar, &cause.condition], extended)?;
            }
        }
        for breakdown in case
            .terminals
            .iter()
            .filter_map(|t| t.breakdown.as_ref())
            .chain(case.dwell.iter().map(|d| &d.breakdown))
        {
            presence(&[&breakdown.condition, &breakdown.queue.calendar], extended)?;
        }
    }
    let checked = if spec.breakdown_waits {
        let suffix = [
            "waiting_decomposition".to_owned(),
            "queue_length_max_and_average".to_owned(),
        ];
        if !legacy_checked.ends_with(&suffix) {
            return Err("metric waiting checked vocabulary differs");
        }
        &legacy_checked[..legacy_checked.len() - 2]
    } else {
        &legacy_checked[..]
    };
    if !m.resources.is_empty() && !checked.ends_with(&resource_words.map(String::from))
        || m.resources.is_empty()
            && value
                .checked
                .iter()
                .any(|w| resource_words.contains(&w.as_str()))
    {
        return Err("metric shared resource checked vocabulary differs");
    }
    if let Some(rows) = &value.resources {
        let expected_checked: Vec<_> = [
            "lead_time_terminal_only",
            "first_start_wait",
            "task_ready_wait",
            "actor_work",
            "named_people_work",
            "peak_tasks",
            "capacity_window_utilization",
            "declared_rates",
            "observed_processing_cost",
            "outcome_counts",
        ]
        .into_iter()
        .chain(resource_words)
        .map(String::from)
        .collect();
        if checked != expected_checked {
            return Err("metric shared resource checked vocabulary differs");
        }
        for row in rows {
            for (scalar, zero) in [
                (&row.occupancy_time, false),
                (&row.peak_units, false),
                (&row.utilization, row.capacity == 0),
            ] {
                use crate::metrics::{Reason, Scalar, Status};
                let (is_sample, reason) = match scalar {
                    Scalar::Range(x) => (false, x.reason),
                    Scalar::Sample(x) => (true, x.reason),
                };
                let expected = match value.status {
                    Status::Complete if zero => Some(Reason::ZeroCapacity),
                    Status::Complete => m
                        .deferred_metrics()
                        .filter(|d| {
                            d.process
                                || m.resources.iter().enumerate().any(|(i, r)| {
                                    r.name == row.resource && d.resource(m, ir::ResourceId(i))
                                })
                        })
                        .map(|_| Reason::NotConfigured),
                    Status::Truncated => Some(Reason::Truncated),
                    Status::Aborted => Some(Reason::Aborted),
                    Status::RangeStopped => Some(Reason::RangeStopped),
                };
                if is_sample != sample || reason != expected {
                    return Err("metric shared resource mode or null reason differs");
                }
            }
        }
    }
    if value.cases.len() as u64 != m.bounds.cases
        || value.overall.counts.admitted.is_some() != m.explicit_limits()
        || value.overall.counts.overflowed.is_some() != m.overflow_end.is_some()
        || value.rates.actors.len() != m.actors.len()
        || value.rates.capabilities.len() != m.capabilities.len()
        || !unique(value.cases.iter().map(|c| c.case_id))
        || !unique(value.actors.iter().map(|a| &a.actor))
        || !unique(value.people.iter().map(|p| &p.person))
        || !unique(value.capabilities.iter().map(|c| &c.capability))
    {
        return Err("invalid metrics inventory");
    }
    fn inventory(
        block: &ir::Block,
        ends: &mut BTreeSet<usize>,
        calls: &mut BTreeMap<usize, usize>,
    ) {
        for statement in block {
            match &statement.kind {
                ir::Stmt::LoopControl(_) => {}
                ir::Stmt::Wait(_)
                | ir::Stmt::WaitUntil(_)
                | ir::Stmt::ResolveEffect { .. }
                | ir::Stmt::StopReceiptUnknown => {}
                ir::Stmt::End(e) => {
                    ends.insert(e.0);
                }
                ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => {
                    calls.insert(c.binding.0, c.capability.0);
                }
                ir::Stmt::Response(c, r) => {
                    calls.insert(c.binding.0, c.capability.0);
                    if r.attempt == 1
                        && let Some(body) = &r.recovery
                    {
                        inventory(body, ends, calls);
                    }
                }
                ir::Stmt::Fork { branches, join, .. } => {
                    for c in branches {
                        calls.insert(c.binding.0, c.capability.0);
                    }
                    if let ir::Join::Any { if_none, .. } = join {
                        ends.insert(if_none.0);
                    }
                }
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    inventory(then_body, ends, calls);
                    inventory(else_body, ends, calls);
                }
            }
        }
    }
    let (mut ends, mut calls) = (BTreeSet::new(), BTreeMap::new());
    inventory(&m.process.body, &mut ends, &mut calls);
    if let Some(e) = m.overflow_end {
        ends.insert(e.0);
    }
    let ends: BTreeSet<_> = ends.into_iter().map(|i| m.ends[i].as_str()).collect();
    let end_names = |rows: &[crate::metrics::EndCount]| {
        rows.iter().map(|r| r.end.as_str()).collect::<BTreeSet<_>>() == ends
            && unique(rows.iter().map(|r| &r.end))
    };
    if !unique(value.terminals.iter().map(|t| &t.end))
        || value.terminals.iter().any(|t| {
            t.kind
                != m.overflow_end.map(|e| {
                    if t.end == m.ends[e.0] {
                        crate::metrics::TerminalKind::Overflow
                    } else {
                        crate::metrics::TerminalKind::End
                    }
                })
        })
        || value
            .terminals
            .iter()
            .map(|t| t.end.as_str())
            .collect::<BTreeSet<_>>()
            != ends
        || !end_names(&value.overall.counts.end_counts)
    {
        return Err("metric terminal inventory differs from selected process");
    }
    for (i, c) in value.cases.iter().enumerate() {
        if c.case_id != i as u64 + 1
            || c.counts.admitted.is_some() != m.explicit_limits()
            || c.counts.overflowed.is_some() != m.overflow_end.is_some()
            || c.external_wait.is_some() != m.external_wait()
            || c.arrival != Some(m.arrivals.as_ref().map_or(0, |a| a[i]))
            || !unique(c.tasks.iter().map(|t| &t.binding))
            || !unique(c.terminals.iter().map(|t| &t.end))
        {
            return Err("invalid case metrics inventory");
        }
        if !c
            .tasks
            .iter()
            .map(|t| t.binding.as_str())
            .eq(calls.keys().map(|id| m.process.bindings[*id].name.as_str()))
        {
            return Err("metric task inventory differs from selected process");
        }
        if !end_names(&c.counts.end_counts)
            || c.terminals.len() != ends.len() + 1
            || c.terminals
                .iter()
                .filter(|t| t.kind == crate::metrics::TerminalKind::Rejected && t.end.is_none())
                .count()
                != 1
            || c.terminals.iter().any(|t| match t.kind {
                crate::metrics::TerminalKind::End => t.end.as_deref().is_none_or(|e| {
                    !ends.contains(e) || m.overflow_end.is_some_and(|id| m.ends[id.0] == e)
                }),
                crate::metrics::TerminalKind::Overflow => t
                    .end
                    .as_deref()
                    .is_none_or(|e| !m.overflow_end.is_some_and(|id| m.ends[id.0] == e)),
                crate::metrics::TerminalKind::Rejected => t.end.is_some(),
            })
        {
            return Err("metric case terminal inventory differs from selected process");
        }
        for (binding, capability) in &calls {
            let cap = &m.capabilities[*capability];
            if c.tasks
                .iter()
                .find(|t| t.binding == m.process.bindings[*binding].name)
                .ok_or("missing metric task")?
                .capability
                != format!("{}.{}", m.actors[cap.actor.0].name, cap.name)
            {
                return Err("metric task Capability differs from selected call");
            }
        }
    }
    if !value.actors.iter().map(|a| a.actor.as_str()).eq(spec
        .required_actors
        .iter()
        .map(|a| m.actors[a.0].name.as_str()))
        || !value
            .people
            .iter()
            .map(|p| p.person.as_str())
            .eq(m.people.iter().map(|p| p.name.as_str()))
    {
        return Err("metric resource inventory differs from declaration");
    }
    if value
        .actors
        .iter()
        .zip(&spec.required_actors)
        .any(|(a, id)| a.capacity != m.actors[id.0].capacity)
        || !value
            .capabilities
            .iter()
            .map(|c| c.capability.clone())
            .eq(spec.required_capabilities.iter().map(|id| {
                let cap = &m.capabilities[id.0];
                format!("{}.{}", m.actors[cap.actor.0].name, cap.name)
            }))
    {
        return Err("metric capacity or Capability inventory differs from declaration");
    }
    for (i, rate) in value.rates.actors.iter().enumerate() {
        if rate.actor != m.actors[i].name
            || rate.per_minute != spec.actor_rates[i]
            || rate.required != spec.required_actors.contains(&ir::ActorId(i))
            || rate.reason
                != rate
                    .per_minute
                    .is_none()
                    .then_some(crate::metrics::Reason::NotConfigured)
        {
            return Err("invalid metrics Actor rate");
        }
    }
    for (i, rate) in value.rates.capabilities.iter().enumerate() {
        let cap = &m.capabilities[i];
        if rate.capability != format!("{}.{}", m.actors[cap.actor.0].name, cap.name)
            || rate.per_start != spec.start_rates[i]
            || rate.required != spec.required_capabilities.contains(&ir::CapabilityId(i))
            || rate.reason
                != rate
                    .per_start
                    .is_none()
                    .then_some(crate::metrics::Reason::NotConfigured)
        {
            return Err("invalid metrics start rate");
        }
    }
    for scalar in metric_rows(value, m)?.values() {
        let scalar = match scalar {
            MetricValue::Scalar(s) => s,
            MetricValue::Count(c) => &c.stats,
        };
        if matches!(scalar, crate::metrics::Scalar::Sample(_)) != sample {
            return Err("metric statistic mode differs");
        }
    }
    validate_waits(value, m, sample)?;
    Ok(())
}

fn validate_waits(
    value: &crate::metrics::BusinessMetrics,
    m: &ir::Model,
    sample: bool,
) -> Result<()> {
    use crate::metrics as w;
    let enabled = m.metrics.as_ref().is_some_and(|s| s.breakdown_waits);
    if value.overall.queue_wait_observed.is_some() != enabled
        || value.actors.iter().any(|a| a.queue.is_some() != enabled)
        || value.cases.iter().any(|c| {
            c.joins.is_some() != enabled
                || c.tasks.iter().any(|t| {
                    t.wait_causes.is_some() != enabled || t.predecessor_wait.is_some() != enabled
                })
                || c.terminals.iter().any(|t| t.breakdown.is_some() != enabled)
                || c.dwell.is_some() && (!enabled || m.horizon.is_none())
        })
    {
        return Err("metric waiting presence differs from declaration");
    }
    if !enabled {
        return Ok(());
    }
    let invalid = match value.status {
        w::Status::Complete => None,
        w::Status::Truncated if sample => None,
        w::Status::Truncated => Some("truncated"),
        w::Status::Aborted => Some("aborted"),
        w::Status::RangeStopped => Some("range_stopped"),
    };
    let actual_cutoff = |count: &Option<w::Count>| {
        count.as_ref().is_some_and(|c| {
            let max = match &c.stats {
                w::Scalar::Range(r) => &r.max,
                w::Scalar::Sample(s) => &s.max,
            };
            max.as_ref().is_some_and(|n| n > &0u64.into())
        })
    };
    fn null_reason(s: &w::Scalar) -> Option<w::Reason> {
        match s {
            w::Scalar::Range(r) if r.min.is_none() && r.max.is_none() => r.reason,
            w::Scalar::Sample(s)
                if s.min.is_none() && s.max.is_none() && s.mean.is_none() && s.samples == 0 =>
            {
                s.reason
            }
            _ => None,
        }
    }
    let bounds: Vec<_> = value
        .actors
        .iter()
        .filter_map(|a| a.queue.as_ref())
        .map(|q| (q.wait_total.bound, q.length_time_average.bound))
        .collect();
    if bounds
        .iter()
        .any(|(a, b)| a != b || a.is_some() && m.horizon.is_none())
        || bounds.windows(2).any(|pair| pair[0] != pair[1])
        || invalid.is_none()
            && (actual_cutoff(&value.overall.counts.cut_off)
                || value.cases.iter().any(|c| actual_cutoff(&c.counts.cut_off)))
            && bounds.iter().any(|(a, _)| *a != Some(w::Bound::Lower))
    {
        return Err("metric waiting bounds differ from horizon observation");
    }
    let mut expected_unchecked = vec![
        "quality",
        "business_value",
        "queue_length_quantiles",
        "quantiles",
        "exact_expectations",
        "metric_constraints",
        "metric_intervals",
        "anonymous_human_work",
        "calendar",
        "rework_loss",
        "introduction_cost",
        "terminal_meaning",
    ];
    if m.resource_time.is_some() {
        expected_unchecked.retain(|s| *s != "calendar");
        expected_unchecked.extend(["calendar_preemption", "person_overtime_pricing"]);
    }
    if value
        .unchecked
        .iter()
        .map(String::as_str)
        .ne(expected_unchecked)
    {
        return Err("metric waiting unchecked vocabulary differs");
    }
    fn forks<'a>(body: &'a ir::Block, out: &mut Vec<(ir::ForkId, &'a [ir::Call], w::JoinMode)>) {
        for s in body {
            match &s.kind {
                ir::Stmt::Response(_, _) => {} // Sequential attempts are not forks.
                ir::Stmt::Fork { id, branches, join } => out.push((
                    *id,
                    branches,
                    if matches!(join, ir::Join::All) {
                        w::JoinMode::All
                    } else {
                        w::JoinMode::Any
                    },
                )),
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    forks(then_body, out);
                    forks(else_body, out);
                }
                _ => {}
            }
        }
    }
    let mut inventory = Vec::new();
    forks(&m.process.body, &mut inventory);
    let mut fields = Vec::new();
    fields.push(
        serde_json::to_value(&value.overall.queue_wait_observed).map_err(|_| "waiting wire")?,
    );
    for c in &value.cases {
        if invalid.is_none() && c.dwell.is_some() != actual_cutoff(&c.counts.cut_off) {
            return Err("metric dwell differs from actual cutoff observation");
        }
        let rows = c.joins.as_ref().unwrap();
        if rows.len() != inventory.len()
            || rows
                .iter()
                .zip(&inventory)
                .any(|(r, (id, branches, mode))| {
                    r.fork != m.process.forks[id.0]
                        || r.mode != *mode
                        || r.branches.iter().map(|b| b.binding.as_str()).ne(branches
                            .iter()
                            .map(|b| m.process.bindings[b.binding.0].name.as_str()))
                })
        {
            return Err("metric join inventory differs from process");
        }
        if invalid.is_none()
            && rows.iter().any(|r| {
                r.branches.iter().any(|b| {
                    (r.mode == w::JoinMode::Any)
                        != (null_reason(&b.join_wait) == Some(w::Reason::JoinAny))
                })
            })
        {
            return Err("metric join any requires null join_any");
        }
        fields.push(serde_json::to_value(rows).map_err(|_| "waiting wire")?);
        fields.push(serde_json::to_value(&c.dwell).map_err(|_| "waiting wire")?);
        for t in &c.tasks {
            fields.push(serde_json::to_value(&t.wait_causes).map_err(|_| "waiting wire")?);
            fields.push(serde_json::to_value(&t.predecessor_wait).map_err(|_| "waiting wire")?);
        }
        for t in &c.terminals {
            fields.push(serde_json::to_value(&t.breakdown).map_err(|_| "waiting wire")?);
        }
    }
    for a in &value.actors {
        fields.push(serde_json::to_value(&a.queue).map_err(|_| "waiting wire")?);
    }
    fn modes(v: &serde_json::Value, sample: bool, invalid: Option<&str>) -> bool {
        match v {
            serde_json::Value::Object(o) if o.contains_key("min") => {
                o.contains_key("mean") == sample
                    && invalid.is_none_or(|r| o["min"].is_null() && o["reason"] == r)
            }
            serde_json::Value::Object(o) => o.values().all(|v| modes(v, sample, invalid)),
            serde_json::Value::Array(a) => a.iter().all(|v| modes(v, sample, invalid)),
            _ => true,
        }
    }
    if fields.iter().any(|v| !modes(v, sample, invalid)) {
        return Err("metric waiting mode or invalidation differs");
    }
    if invalid.is_some()
        && (value.cases.iter().any(|c| c.dwell.is_some())
            || value
                .actors
                .iter()
                .filter_map(|a| a.queue.as_ref())
                .any(|q| q.wait_total.bound.is_some() || q.length_time_average.bound.is_some()))
    {
        return Err("invalidated waiting bounds remain");
    }
    Ok(())
}

#[cfg(test)]
mod waiting_projection_tests {
    use super::*;
    #[test]
    fn review_cutoff_fields_are_individually_required_in_both_modes() {
        for sample in [false, true] {
            let src = if sample {
                include_str!("../../../fixtures/wait-sample.broken-short-horizon/model.relca")
            } else {
                include_str!("../../../fixtures/wait-horizon-queue/model.relca")
            };
            let m = crate::lower::lower_with_selection(
                &crate::syntax::parse(src).unwrap(),
                None,
                Some("Day"),
            )
            .unwrap();
            let point = if sample {
                crate::report::simulate_with_selection(src, None, Some("Day"))
                    .metrics
                    .unwrap()
            } else {
                crate::report::check_with_selection(src, None, Some("Day"))
                    .metrics
                    .unwrap()
            };
            validate_metrics(Some(&point), &m, sample).unwrap();
            // The frozen R1 pair removes both fields together. Keep their
            // obligations independent too, so neither guard masks the other.
            for case in 0..point.cases.len() {
                let mut bad = point.clone();
                bad.cases[case].dwell = None;
                assert!(
                    validate_metrics(Some(&bad), &m, sample).is_err(),
                    "missing dwell case{case}, sample={sample}"
                );
            }
            let mut bad = point.clone();
            for actor in &mut bad.actors {
                let q = actor.queue.as_mut().unwrap();
                q.wait_total.bound = None;
                q.length_time_average.bound = None;
            }
            assert!(
                validate_metrics(Some(&bad), &m, sample).is_err(),
                "missing lower, sample={sample}"
            );
        }
    }

    #[test]
    fn waiting_tampering_is_rejected_at_private_projection() {
        let src = include_str!("../../../fixtures/wait-person/model.relca");
        let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let point = crate::report::check(src).metrics.unwrap();
        assert!(validate_metrics(Some(&point), &m, false).is_ok());
        for fault in 0..12 {
            let mut bad = point.clone();
            match fault {
                0 => bad.cases[0].terminals[0].breakdown = None,
                1 => bad.cases[0].tasks[0].wait_causes = None,
                2 => bad.cases[0].tasks[0].predecessor_wait = None,
                3 => bad.cases[0].joins = None,
                4 => bad.actors[0].queue = None,
                5 => bad.overall.queue_wait_observed = None,
                6 => bad.cases[0].joins.as_mut().unwrap()[0].mode = crate::metrics::JoinMode::Any,
                7 => bad.cases[0].joins.as_mut().unwrap()[0].fork = "wrong".into(),
                8 => bad.cases[0].joins.as_mut().unwrap()[0].branches.reverse(),
                9 => {
                    bad.actors[0].queue.as_mut().unwrap().wait_total.bound =
                        Some(crate::metrics::Bound::Lower)
                }
                10 => {
                    bad.cases[0].tasks[0].predecessor_wait =
                        Some(crate::metrics::Scalar::Sample(crate::metrics::Sample {
                            mean: Some(0.into()),
                            min: Some(0.into()),
                            max: Some(0.into()),
                            samples: 1,
                            reason: None,
                        }))
                }
                11 => bad.unchecked.push("waiting_decomposition".into()),
                _ => unreachable!(),
            }
            assert!(
                validate_metrics(Some(&bad), &m, false).is_err(),
                "fault{fault}"
            );
        }
        let mut legacy = m.clone();
        legacy.metrics.as_mut().unwrap().breakdown_waits = false;
        assert!(validate_metrics(Some(&point), &legacy, false).is_err());
    }
}

#[cfg(test)]
mod resource_projection_tests {
    use super::*;
    #[test]
    fn resource_h12_forged_inventory_unknown_fields_modes_and_reasons_are_rejected() {
        let src = include_str!("../../../fixtures/rsrc-global/model.relca").replace("  bounds", "  configuration Left { cut K; process P }\n  configuration Right { cut K; process P }\n  bounds");
        let m = crate::lower::lower_configuration(&crate::syntax::parse(&src).unwrap(), "Left")
            .unwrap()
            .model;
        let report = crate::report::check_configuration(&src, "Left");
        let metrics = report.metrics.clone().unwrap();
        validate_metrics(Some(&metrics), &m, false).unwrap();
        let raw = serde_json::to_value(&metrics).unwrap();
        for fault in 0..9 {
            let mut v = raw.clone();
            match fault {
                0 => {
                    v.as_object_mut().unwrap().remove("resources");
                }
                1 => {
                    let row = v["resources"][0].clone();
                    v["resources"].as_array_mut().unwrap().push(row);
                }
                2 => v["resources"][0]["resource"] = "Wrong".into(),
                3 => v["resources"][0]["capacity"] = 2.into(),
                4 => v["resources"] = serde_json::json!([]),
                5 => v["resources"][0]["cost"] = 0.into(),
                6 => {
                    v["resources"][0]["peak_units"] = serde_json::json!({"mean":"1","min":"1","max":"1","samples":2,"reason":null})
                }
                7 => {
                    v["resources"][0]["occupancy_time"] =
                        serde_json::json!({"min":null,"max":null,"reason":"not_configured"})
                }
                _ => v["checked"]
                    .as_array_mut()
                    .unwrap()
                    .push("resource_price".into()),
            }
            match serde_json::from_value::<crate::metrics::BusinessMetrics>(v) {
                Ok(forged) => {
                    assert!(
                        validate_metrics(Some(&forged), &m, false).is_err(),
                        "fault {fault}"
                    );
                    let mut forged_report = report.clone();
                    forged_report.metrics = Some(forged);
                    assert!(
                        exhaustive(&forged_report, &m).is_err(),
                        "projection fault {fault}"
                    );
                }
                Err(_) => assert_eq!(fault, 5),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interval {
    pub lower: Rational,
    pub upper: Rational,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Witness {
    pub replication: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    Structural,
    Statistical,
    Witness,
    Sample,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropertyValue {
    pub verdict: Verdict,
    pub probability: Option<ProbabilityRange>,
    pub basis: Option<Basis>,
    pub hits: Option<u64>,
    pub unknown: Option<u64>,
    pub trials: Option<u64>,
    pub interval: Option<Interval>,
    pub witness: Option<Witness>,
}
impl PropertyValue {
    fn exhaustive(verdict: Verdict, probability: Option<ProbabilityRange>) -> Self {
        Self {
            verdict,
            probability,
            basis: None,
            hits: None,
            unknown: None,
            trials: None,
            interval: None,
            witness: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExhaustiveTerminal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_until: Option<report::Range>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_elapsed: Option<report::Range>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_work: Option<report::Range>,
    pub probability: Option<ProbabilityRange>,
    pub completed_at: Option<report::Range>,
    pub elapsed: Option<report::Range>,
    pub work: Option<report::Range>,
    pub result_at: Option<report::Range>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationTerminal {
    pub count: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TerminalValue {
    Exhaustive(Box<ExhaustiveTerminal>),
    Simulation(SimulationTerminal),
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TerminalKey {
    ReceiptUnknown,
    End(String),
    Rejected,
    Overflow(String),
    CutOff,
}
impl TerminalKey {
    pub(crate) fn fields(&self) -> (report::CaseTerminalKind, Option<String>) {
        match self {
            Self::ReceiptUnknown => (report::CaseTerminalKind::ReceiptUnknown, None),
            Self::End(e) => (report::CaseTerminalKind::End, Some(e.clone())),
            Self::Rejected => (report::CaseTerminalKind::Rejected, None),
            Self::Overflow(e) => (report::CaseTerminalKind::Overflow, Some(e.clone())),
            Self::CutOff => (report::CaseTerminalKind::CutOff, None),
        }
    }
}
pub(crate) struct Property {
    pub id: String,
    pub aggregate: Verdict,
    pub values: BTreeMap<Option<u64>, PropertyValue>,
}
pub(crate) struct View {
    pub arrivals: BTreeMap<u64, u64>,
    pub properties: Vec<Property>,
    pub terminals: BTreeMap<(u64, TerminalKey), TerminalValue>,
    pub aborted: bool,
}

fn require(ok: bool, message: &'static str) -> Result<()> {
    if ok { Ok(()) } else { Err(message) }
}
fn arrivals(m: &ir::Model) -> BTreeMap<u64, u64> {
    m.arrivals
        .clone()
        .unwrap_or_else(|| vec![0])
        .into_iter()
        .enumerate()
        .map(|(i, a)| (i as u64 + 1, a))
        .collect()
}
fn expected_properties(m: &ir::Model) -> Vec<String> {
    m.verify
        .iter()
        .map(|v| match v.property {
            ir::Property::NoLostUpdate(id) => m.loss_property_id(id),
            ir::Property::StateInRange => "state_in_range".into(),
            ir::Property::RecoverableEnd => "recoverable end".into(),
            ir::Property::TerminatingEnd => "terminating end".into(),
            ir::Property::AlmostSureEnd => "almost_sure end".into(),
            ir::Property::Contest(c) => crate::contest::property_id(m, c),
            ir::Property::NoResourceOvercommit => SAFETY.into(),
            ir::Property::ReachableEnd(e) => format!("reachable end.{}", m.ends[e.0]),
            ir::Property::ProbabilityReached(q) => format!("probability reached #{}", q + 1),
            ir::Property::Contract(c) => m.contracts[c.0].property_id(c),
        })
        .chain((!m.response_attempts().is_empty()).then(|| "receipt_resolution".into()))
        .collect()
}
fn inventory(ids: impl Iterator<Item = String>, m: &ir::Model, automatic: bool) -> Result<()> {
    let ids: Vec<_> = ids.collect();
    require(
        !ids.is_empty()
            && ids
                == expected_properties(m)
                    .into_iter()
                    .filter(|id| automatic || id != "receipt_resolution")
                    .collect::<Vec<_>>()
            && ids.iter().collect::<BTreeSet<_>>().len() == ids.len(),
        "property inventory is missing, duplicated or inconsistent",
    )
}
fn structural(m: &ir::Model, id: &str) -> Option<Verdict> {
    m.verify.iter().find_map(|v| {
        if let ir::Property::Contest(c) = v.property {
            (crate::contest::property_id(m, c) == id).then(|| crate::contest::verdict(m, c))
        } else {
            None
        }
    })
}
fn case_ids<T>(rows: &BTreeMap<u64, T>, expected: &BTreeMap<u64, u64>) -> Result<()> {
    require(
        rows.keys().eq(expected.keys()),
        "case IDs are missing, duplicated or inconsistent",
    )
}
fn insert_case<T>(
    rows: &mut BTreeMap<u64, T>,
    expected: &BTreeMap<u64, u64>,
    id: u64,
    arrival: u64,
    value: T,
) -> Result<()> {
    require(
        expected.get(&id) == Some(&arrival),
        "case arrival differs from model",
    )?;
    require(rows.insert(id, value).is_none(), "duplicate case ID")
}
fn key(
    kind: &report::CaseTerminalKind,
    end: &Option<String>,
    m: &ir::Model,
) -> Result<TerminalKey> {
    match (kind, end) {
        (report::CaseTerminalKind::End, Some(e))
            if m.ends.contains(e) && !m.overflow_end.is_some_and(|id| m.ends[id.0] == *e) =>
        {
            Ok(TerminalKey::End(e.clone()))
        }
        (report::CaseTerminalKind::Overflow, Some(e))
            if m.overflow_end.is_some_and(|id| m.ends[id.0] == *e) =>
        {
            Ok(TerminalKey::Overflow(e.clone()))
        }
        (report::CaseTerminalKind::Rejected, None) => Ok(TerminalKey::Rejected),
        (report::CaseTerminalKind::CutOff, None) if m.horizon.is_some() => Ok(TerminalKey::CutOff),
        (report::CaseTerminalKind::ReceiptUnknown, None) if !m.response_attempts().is_empty() => {
            Ok(TerminalKey::ReceiptUnknown)
        }
        _ => Err("terminal kind/end is inconsistent"),
    }
}
fn ranges(t: &ExhaustiveTerminal, rejected: bool) -> Result<()> {
    require(
        [t.observed_until, t.observed_elapsed, t.observed_work]
            .iter()
            .all(Option::is_none),
        "observation on a business terminal",
    )?;
    let all = [t.completed_at, t.elapsed, t.work, t.result_at];
    require(
        all.iter().flatten().all(|r| r.min <= r.max),
        "invalid terminal range",
    )?;
    require(
        if rejected {
            all.iter().all(Option::is_none)
        } else {
            all[..3].iter().all(Option::is_some)
        },
        "terminal time nullability is inconsistent",
    )
}

// Validate provenance against this selected IR, not another configuration's
// numeric binding IDs. Aggregated receipts are observations, never a path.
fn validate_receipts(r: &report::Report, m: &ir::Model) -> Result<()> {
    let attempts = m.response_attempts();
    require(
        r.response_receipts.is_some() == !attempts.is_empty(),
        "response receipt inventory differs",
    )?;
    let mut identities = BTreeSet::new();
    let arrivals = arrivals(m);
    for row in r.response_receipts.iter().flatten() {
        let (call, rule) = attempts
            .iter()
            .find(|(_, a)| m.site_name(a.command) == row.command && a.attempt == row.attempt)
            .ok_or("receipt command or attempt differs from model")?;
        let cap = &m.capabilities[call.capability.0];
        require(
            row.capability == format!("{}.{}", m.actors[cap.actor.0].name, cap.name)
                && arrivals.get(&row.case_id).is_some_and(|a| *a <= row.at.min)
                && row.at.min <= row.at.max
                && row.at.max <= m.bounds.clock
                && (row.receipt == crate::ast::Receipt::Unknown) == row.observed_result.is_none()
                && !(row.receipt == crate::ast::Receipt::Unknown && row.resolved_effect),
            "invalid response observation",
        )?;
        if let Some(value) = &row.observed_result {
            require(
                m.types[cap.return_type.0]
                    .members
                    .iter()
                    .any(|v| v == value),
                "unknown receipt result",
            )?;
        }
        require(
            match &rule.loss {
                ir::ResponseLoss::None => row.receipt == crate::ast::Receipt::Delivered,
                ir::ResponseLoss::Sequence(xs) => {
                    xs.get(row.attempt as usize - 1) == Some(&row.receipt)
                }
                ir::ResponseLoss::Nondet => true,
                ir::ResponseLoss::Probability { probability: p, .. } => {
                    if p.is_zero() {
                        row.receipt == crate::ast::Receipt::Delivered
                    } else if p == &Rational::one() {
                        row.receipt == crate::ast::Receipt::Unknown
                    } else {
                        true
                    }
                }
            },
            "receipt is outside channel support",
        )?;
        require(
            identities.insert((
                row.case_id,
                &row.command,
                row.attempt,
                row.receipt == crate::ast::Receipt::Unknown,
                &row.observed_result,
                row.resolved_effect,
            )),
            "duplicate response observation",
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/support/p3a_projection_tests.rs"]
mod p3a_projection_tests;

#[cfg(test)]
#[path = "../tests/support/p3a_review_projection_tests.rs"]
mod p3a_review_projection_tests;

pub(crate) fn exhaustive(r: &report::Report, m: &ir::Model) -> Result<View> {
    validate_receipts(r, m)?;
    validate_scenario(r.scenario.as_ref(), m, false)?;
    if let Some(s) = &r.scenario {
        let observation: crate::metrics::Range =
            serde_json::from_value(s["aggregation"]["observed_until"].clone())
                .map_err(|_| "invalid scenario range")?;
        let expected = if r.bounds.truncated.is_none() {
            Some(crate::metrics::Reason::Aborted)
        } else if r.bounds.truncated == Some(true) {
            Some(crate::metrics::Reason::Truncated)
        } else if r.bounds.range_violation_stops == Some(true) {
            Some(crate::metrics::Reason::RangeStopped)
        } else {
            None
        };
        require(
            observation.reason == expected,
            "scenario range coverage differs from report",
        )?;
    }
    validate_metrics(r.metrics.as_ref(), m, false)?;
    require(
        r.bounds.cases == Some(m.bounds.cases),
        "report bounds differ from model",
    )?;
    inventory(r.properties.iter().map(|p| p.id.clone()), m, true)?;
    let mut view = View {
        arrivals: arrivals(m),
        properties: Vec::new(),
        terminals: BTreeMap::new(),
        aborted: r.bounds.truncated.is_none(),
    };
    for p in &r.properties {
        require(
            p.notes == report::notes_for_id(m, &p.id),
            "invalid property notes",
        )?;
        let mut values = BTreeMap::new();
        if let Some(verdict) = structural(m, &p.id) {
            require(
                p.cases.is_none() && p.probability.is_none() && p.verdict == verdict,
                "invalid static global row",
            )?;
            values.insert(None, PropertyValue::exhaustive(verdict, None));
        } else if p.id == SAFETY || loss_id(m, &p.id) {
            require(
                p.cases.is_none() && p.probability.is_none(),
                "global safety has case or probability rows",
            )?;
            values.insert(None, PropertyValue::exhaustive(p.verdict, None));
        } else if m.legacy_route() {
            require(p.cases.is_none(), "legacy property has case rows")?;
            values.insert(
                Some(1),
                PropertyValue::exhaustive(p.verdict, p.probability.clone()),
            );
        } else {
            let mut cases = BTreeMap::new();
            for c in p.cases.as_ref().ok_or("missing property case inventory")? {
                insert_case(
                    &mut cases,
                    &view.arrivals,
                    c.case_id,
                    c.arrival,
                    PropertyValue::exhaustive(c.verdict, c.probability.clone()),
                )?;
            }
            case_ids(&cases, &view.arrivals)?;
            values.extend(cases.into_iter().map(|(k, v)| (Some(k), v)));
        }
        view.properties.push(Property {
            id: p.id.clone(),
            aggregate: p.verdict,
            values,
        });
    }
    if m.legacy_route() {
        require(r.case_outcomes.is_none(), "legacy report has case outcomes")?;
        for t in &r.terminals {
            require(m.ends.contains(&t.end), "unknown terminal name")?;
            let value = ExhaustiveTerminal {
                observed_until: None,
                observed_elapsed: None,
                observed_work: None,
                probability: t.probability.clone(),
                completed_at: Some(t.elapsed),
                elapsed: Some(t.elapsed),
                work: Some(t.work),
                result_at: t.result_at,
            };
            ranges(&value, false)?;
            require(
                view.terminals
                    .insert(
                        (1, TerminalKey::End(t.end.clone())),
                        TerminalValue::Exhaustive(Box::new(value)),
                    )
                    .is_none(),
                "duplicate terminal",
            )?;
        }
    } else {
        if !m.response_attempts().is_empty() && view.arrivals.len() == 1 {
            let expected: Vec<_> = r
                .case_outcomes
                .as_ref()
                .into_iter()
                .flatten()
                .flat_map(|c| &c.terminals)
                .filter(|t| {
                    matches!(
                        t.kind,
                        report::CaseTerminalKind::End | report::CaseTerminalKind::Overflow
                    )
                })
                .map(|t| {
                    Ok(report::Terminal {
                        end: t.end.clone().ok_or("missing end name")?,
                        probability: t.probability.clone(),
                        elapsed: t.elapsed.ok_or("missing elapsed")?,
                        work: t.work.ok_or("missing work")?,
                        result_at: t.result_at,
                    })
                })
                .collect::<Result<_>>()?;
            require(
                r.terminals == expected,
                "single-response terminal summary differs",
            )?;
        } else {
            require(
                r.terminals.is_empty(),
                "per-case report has top-level terminals",
            )?;
        }
        let mut cases = BTreeMap::new();
        for c in r.case_outcomes.as_ref().ok_or("missing case outcomes")? {
            require(
                c.admitted_at.is_some() == m.external_wait(),
                "admission time presence differs from declaration",
            )?;
            insert_case(&mut cases, &view.arrivals, c.case_id, c.arrival, ())?;
            for t in &c.terminals {
                let key = key(&t.kind, &t.end, m)?;
                let value = ExhaustiveTerminal {
                    observed_until: t.observed_until,
                    observed_elapsed: t.observed_elapsed,
                    observed_work: t.observed_work,
                    probability: t.probability.clone(),
                    completed_at: t.completed_at,
                    elapsed: t.elapsed,
                    work: t.work,
                    result_at: t.result_at,
                };
                if key == TerminalKey::ReceiptUnknown {
                    require(
                        [
                            value.completed_at,
                            value.elapsed,
                            value.work,
                            value.result_at,
                        ]
                        .iter()
                        .all(Option::is_none)
                            && [
                                value.observed_until,
                                value.observed_elapsed,
                                value.observed_work,
                            ]
                            .iter()
                            .all(|r| r.is_some_and(|r| r.min <= r.max))
                            && value
                                .observed_until
                                .zip(value.observed_elapsed)
                                .is_some_and(|(a, e)| {
                                    e.min.checked_add(c.arrival) == Some(a.min)
                                        && e.max.checked_add(c.arrival) == Some(a.max)
                                }),
                        "invalid receipt_unknown observation",
                    )?;
                } else if key == TerminalKey::CutOff {
                    require(
                        value.completed_at.is_none()
                            && value.elapsed.is_none()
                            && value.result_at.is_none()
                            && value.observed_until.is_none()
                            && value.observed_elapsed.is_none()
                            && value.observed_work.is_none()
                            && value.work.is_some_and(|w| w.min <= w.max)
                            && value.probability.is_some(),
                        "invalid cutoff row",
                    )?;
                } else {
                    ranges(&value, key == TerminalKey::Rejected)?;
                }
                require(
                    view.terminals
                        .insert((c.case_id, key), TerminalValue::Exhaustive(Box::new(value)))
                        .is_none(),
                    "duplicate terminal",
                )?;
            }
        }
        case_ids(&cases, &view.arrivals)?;
    }
    require(
        !view.aborted || view.terminals.is_empty(),
        "aborted report contains terminal numbers",
    )?;
    Ok(view)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SampleFields {
    verdict: Verdict,
    basis: Basis,
    witness: Option<Witness>,
    hits: Option<u64>,
    unknown: Option<u64>,
    trials: Option<u64>,
    interval: Option<Interval>,
}
impl SampleFields {
    fn value(self, n: u64, aborted: bool, query: bool) -> Result<PropertyValue> {
        match self.basis {
            Basis::Structural => return Err("structural basis requires a static global property"),
            Basis::Statistical => {
                require(
                    query && !aborted && self.witness.is_none(),
                    "invalid statistical property",
                )?;
                let (h, u, t, i) = (
                    self.hits.ok_or("missing hits")?,
                    self.unknown.ok_or("missing unknown")?,
                    self.trials.ok_or("missing trials")?,
                    self.interval.as_ref().ok_or("missing interval")?,
                );
                require(
                    t == n
                        && h.checked_add(u).is_some_and(|x| x <= t)
                        && i.lower.valid()
                        && i.upper.valid()
                        && i.lower <= i.upper,
                    "invalid statistical counts or interval",
                )?;
            }
            Basis::Witness | Basis::Sample => {
                require(
                    self.hits.is_none()
                        && self.unknown.is_none()
                        && self.trials.is_none()
                        && self.interval.is_none(),
                    "sample row contains statistics",
                )?;
                require(
                    (self.basis == Basis::Witness) == self.witness.is_some(),
                    "witness basis differs from witness",
                )?;
                if let Some(w) = &self.witness {
                    require(
                        (1..=n).contains(&w.replication),
                        "invalid witness replication",
                    )?;
                }
                require(
                    !aborted || (self.verdict == Verdict::Unsupported && self.witness.is_none()),
                    "aborted sample retains a conclusion",
                )?;
                require(!query || aborted, "probability query lacks statistics")?;
            }
        }
        Ok(PropertyValue {
            verdict: self.verdict,
            probability: None,
            basis: Some(self.basis),
            hits: self.hits,
            unknown: self.unknown,
            trials: self.trials,
            interval: self.interval,
            witness: self.witness,
        })
    }
}
// Spelled out: serde ignores deny_unknown_fields across a flatten.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SampleCase {
    case_id: u64,
    arrival: u64,
    verdict: Verdict,
    basis: Basis,
    witness: Option<Witness>,
    hits: Option<u64>,
    unknown: Option<u64>,
    trials: Option<u64>,
    interval: Option<Interval>,
}
impl SampleCase {
    fn split(self) -> (u64, u64, SampleFields) {
        let fields = SampleFields {
            verdict: self.verdict,
            basis: self.basis,
            witness: self.witness,
            hits: self.hits,
            unknown: self.unknown,
            trials: self.trials,
            interval: self.interval,
        };
        (self.case_id, self.arrival, fields)
    }
}
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum SampleProperty {
    Automatic {
        id: String,
        verdict: Verdict,
    },
    Cases {
        #[serde(default, deserialize_with = "report::deserialize_notes")]
        notes: Option<Vec<String>>,
        id: String,
        verdict: Verdict,
        cases: Vec<SampleCase>,
    },
    Global {
        id: String,
        #[serde(flatten)]
        fields: SampleFields,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SampleTerminal {
    kind: report::CaseTerminalKind,
    end: Option<String>,
    count: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SampleOutcome {
    cut_off_count: Option<u64>,
    range_stopped_count: Option<u64>,
    case_id: u64,
    arrival: u64,
    terminals: Vec<SampleTerminal>,
    unfinished_count: Option<u64>,
    deadlocked_count: Option<u64>,
}
pub(crate) fn simulation(r: &report::SimulationReport, m: &ir::Model) -> Result<View> {
    validate_scenario(r.scenario.as_ref(), m, true)?;
    validate_metrics(r.metrics.as_ref(), m, true)?;
    require(
        r.mode == "simulation" && r.bounds.cases == Some(m.bounds.cases) && r.terminals.is_empty(),
        "simulation report mode or bounds differs",
    )?;
    let n = m
        .simulation
        .as_ref()
        .ok_or("missing simulation metadata")?
        .replications;
    let aborted = match r.simulation["status"].as_str() {
        Some("completed") => false,
        Some("unsupported") => true,
        _ => return Err("invalid simulation report status"),
    };
    if let Some(s) = &r.scenario {
        let observation: crate::metrics::Sample =
            serde_json::from_value(s["aggregation"]["observed_until"].clone())
                .map_err(|_| "invalid scenario sample")?;
        require(
            if aborted {
                observation.reason == Some(crate::metrics::Reason::Aborted)
                    && observation.samples == 0
            } else {
                observation.reason.is_none()
                    && Some(observation.samples) == r.simulation["completed_replications"].as_u64()
            },
            "scenario sample coverage differs from report",
        )?;
    }
    require(
        r.simulation.get("cut_off_replications").is_some() == m.horizon.is_some(),
        "cutoff replication presence differs",
    )?;
    if m.horizon.is_some() {
        require(
            if aborted {
                r.simulation["cut_off_replications"].is_null()
            } else {
                r.simulation["cut_off_replications"]
                    .as_u64()
                    .is_some_and(|c| c <= n)
            },
            "invalid cutoff replication count",
        )?;
    }
    require(
        r.bounds.truncated.is_none() == aborted,
        "simulation abort differs from bounds",
    )?;
    let mut view = View {
        arrivals: arrivals(m),
        properties: Vec::new(),
        terminals: BTreeMap::new(),
        aborted,
    };
    let ps: Vec<SampleProperty> = r
        .properties
        .iter()
        .cloned()
        .map(serde_json::from_value)
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| "invalid simulation property shape")?;
    inventory(
        ps.iter().map(|p| match p {
            SampleProperty::Automatic { id, .. } => id.clone(),
            SampleProperty::Cases { id, .. } | SampleProperty::Global { id, .. } => id.clone(),
        }),
        m,
        !aborted,
    )?;
    for p in ps {
        let (id, aggregate, values) = match p {
            SampleProperty::Automatic { id, verdict } => {
                require(
                    id == "receipt_resolution" && verdict == Verdict::Inconclusive && !aborted,
                    "invalid automatic sample property",
                )?;
                (
                    id,
                    verdict,
                    BTreeMap::from([(None, PropertyValue::exhaustive(verdict, None))]),
                )
            }
            SampleProperty::Global { id, fields } => {
                let v = if let Some(verdict) = structural(m, &id) {
                    require(
                        fields.basis == Basis::Structural
                            && fields.verdict == verdict
                            && fields.witness.is_none()
                            && fields.hits.is_none()
                            && fields.unknown.is_none()
                            && fields.trials.is_none()
                            && fields.interval.is_none(),
                        "invalid structural simulation row",
                    )?;
                    let mut value = PropertyValue::exhaustive(verdict, None);
                    value.basis = Some(Basis::Structural);
                    value
                } else {
                    require(
                        sample_global_safety_id(m, &id),
                        "nonglobal property lacks cases",
                    )?;
                    fields.value(n, aborted, false)?
                };
                (id, v.verdict, BTreeMap::from([(None, v)]))
            }
            SampleProperty::Cases {
                id,
                verdict,
                cases,
                notes,
            } => {
                require(
                    notes == report::notes_for_id(m, &id),
                    "invalid sample property notes",
                )?;
                require(
                    !sample_global_safety_id(m, &id) && structural(m, &id).is_none(),
                    "global property has cases",
                )?;
                let mut rows = BTreeMap::new();
                for c in cases {
                    let (case_id, arrival, fields) = c.split();
                    let value =
                        fields.value(n, aborted, id.starts_with("probability reached #"))?;
                    insert_case(&mut rows, &view.arrivals, case_id, arrival, value)?;
                }
                case_ids(&rows, &view.arrivals)?;
                (
                    id,
                    verdict,
                    rows.into_iter().map(|(k, v)| (Some(k), v)).collect(),
                )
            }
        };
        view.properties.push(Property {
            id,
            aggregate,
            values,
        });
    }
    let mut cases = BTreeMap::new();
    for c in &r.case_outcomes {
        require(
            c.get("range_stopped_count").is_some() == (m.has_integers() && !aborted),
            "range stop count presence differs",
        )?;
        require(
            c.get("cut_off_count").is_some() == m.horizon.is_some(),
            "cutoff count presence differs",
        )?;
        let c: SampleOutcome =
            serde_json::from_value(c.clone()).map_err(|_| "invalid simulation case shape")?;
        require(
            c.range_stopped_count.is_some() == (m.has_integers() && !aborted),
            "range stop count presence differs",
        )?;
        insert_case(&mut cases, &view.arrivals, c.case_id, c.arrival, ())?;
        require(
            if aborted {
                c.cut_off_count.is_none()
            } else {
                c.cut_off_count.is_some() == m.horizon.is_some()
            },
            "cutoff count presence differs",
        )?;
        if aborted {
            require(
                c.terminals.is_empty()
                    && c.unfinished_count.is_none()
                    && c.deadlocked_count.is_none(),
                "aborted simulation contains counts",
            )?;
        }
        let mut total = 0u64;
        let mut unknown = 0u64;
        for t in c.terminals {
            let key = key(&t.kind, &t.end, m)?;
            require(
                key != TerminalKey::CutOff,
                "sample cutoff cannot be a business terminal",
            )?;
            require(t.count > 0, "reported sample terminal has zero count")?;
            if key == TerminalKey::ReceiptUnknown {
                unknown = t.count;
            } else {
                total = total
                    .checked_add(t.count)
                    .ok_or("terminal count overflow")?;
            }
            require(
                view.terminals
                    .insert(
                        (c.case_id, key),
                        TerminalValue::Simulation(SimulationTerminal { count: t.count }),
                    )
                    .is_none(),
                "duplicate simulation terminal",
            )?;
        }
        if !aborted {
            require(
                unknown <= c.unfinished_count.unwrap_or(0),
                "unknown is not a subset of unfinished",
            )?;
            if !m.response_attempts().is_empty() {
                require(
                    unknown == c.unfinished_count.unwrap_or(0) || r.bounds.truncated == Some(true),
                    "missing receipt_unknown outcome",
                )?;
            }
            if let Some(count) = c.cut_off_count.filter(|n| *n > 0) {
                view.terminals.insert(
                    (c.case_id, TerminalKey::CutOff),
                    TerminalValue::Simulation(SimulationTerminal { count }),
                );
            }
            let total = total
                .checked_add(c.unfinished_count.ok_or("missing unfinished count")?)
                .and_then(|x| x.checked_add(c.deadlocked_count?))
                .and_then(|x| x.checked_add(c.cut_off_count.unwrap_or(0)))
                .and_then(|x| x.checked_add(c.range_stopped_count.unwrap_or(0)));
            require(
                total == Some(n),
                "simulation case counts do not sum to trials",
            )?;
        }
    }
    case_ids(&cases, &view.arrivals)?;
    Ok(view)
}

fn validate_scenario(value: Option<&serde_json::Value>, m: &ir::Model, sample: bool) -> Result<()> {
    let Some(s) = value else {
        return require(m.scenario.is_none(), "missing scenario metadata");
    };
    require(m.scenario.is_some(), "unexpected scenario metadata")?;
    let observed = s["aggregation"]["observed_until"].clone();
    let scalar: crate::metrics::Scalar =
        serde_json::from_value(observed.clone()).map_err(|_| "invalid scenario observation")?;
    require(
        matches!(
            (&scalar, sample),
            (crate::metrics::Scalar::Sample(_), true) | (crate::metrics::Scalar::Range(_), false)
        ),
        "scenario observation mode differs",
    )?;
    require(
        report::scenario_record(m, observed).as_ref() == Some(s),
        "scenario metadata differs from IR",
    )
}

#[cfg(test)]
mod responsibility_tests {
    use super::*;
    #[test]
    fn r_dynamic_contract_without_cases_is_not_a_global_structural_row() {
        let source = include_str!("../../../fixtures/resp-contest/model.relca")
            .replace(
                "contest Vendor",
                "permitted Approver.approve;\ncontest Vendor",
            )
            .replace("cases 1", "cases 2")
            .replace("bounds {", "arrivals fixed [0min, 1min]\nbounds {");
        let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
        let report = report::check(&source);
        assert!(exhaustive(&report, &m).is_ok());
        let mut broken = report.clone();
        let dynamic = broken
            .properties
            .iter_mut()
            .find(|p| p.id.ends_with(" permitted"))
            .unwrap();
        dynamic.cases = None;
        assert!(exhaustive(&broken, &m).is_err());
        let mut broken = report.clone();
        let static_row = broken
            .properties
            .iter_mut()
            .find(|p| p.id.ends_with(" contest"))
            .unwrap();
        static_row.cases = Some(vec![]);
        assert!(exhaustive(&broken, &m).is_err());
        let sim = source.replace("result nondet { pass,fail }", "result nondet { fail }");
        let at = sim.rfind('}').unwrap();
        let sim = format!(
            "{} simulation replications 4 seed 17 confidence 0.95\n{}",
            &sim[..at],
            &sim[at..]
        );
        let m = crate::lower::lower(&crate::syntax::parse(&sim).unwrap()).unwrap();
        let mut r = report::simulate(&sim);
        assert!(simulation(&r, &m).is_ok());
        let row = r
            .properties
            .iter_mut()
            .find(|p| p["id"].as_str().unwrap().ends_with(" permitted"))
            .unwrap();
        row.as_object_mut().unwrap().remove("cases");
        row["basis"] = serde_json::json!("structural");
        assert!(simulation(&r, &m).is_err());
    }
}
/// Rows are kept in declaration order, not sorted by their display names.
pub(crate) struct QualityProjection {
    pub key: serde_json::Value,
    pub denominator: Option<serde_json::Value>,
    pub value: serde_json::Value,
}
pub(crate) fn quality_rows(
    q: Option<&crate::quality::Quality>,
    m: &crate::ir::Model,
    simulation: bool,
) -> Result<Vec<QualityProjection>> {
    use serde_json::json;
    let Some(spec) = &m.quality else {
        return if q.is_none() {
            Ok(vec![])
        } else {
            Err("undeclared quality report")
        };
    };
    let q = q.ok_or("missing quality report")?;
    crate::quality::validate_wire(q).map_err(|_| "invalid quality wire")?;
    if q.analysis
        != if simulation {
            "simulation"
        } else {
            "exhaustive"
        }
        || q.properties.len() != spec.properties.len()
        || q.cohort.len() != spec.metrics.len()
    {
        return Err("quality inventory differs");
    }
    let arrivals = m.arrivals.clone().unwrap_or_else(|| vec![0]);
    let mut rows = Vec::new();
    for (v, p) in q.properties.iter().zip(&spec.properties) {
        if v["name"] != p.name
            || v["condition"] != crate::quality::definition(m, p)
            || v["within"] != json!(p.within)
        {
            return Err("quality definition differs");
        }
        let cases = v["cases"].as_array().ok_or("quality cases missing")?;
        if cases.len() != arrivals.len()
            || !cases
                .iter()
                .zip(&arrivals)
                .enumerate()
                .all(|(i, (c, a))| c["case_id"] == json!(i + 1) && c["arrival"] == *a)
        {
            return Err("quality case inventory differs");
        }
        for field in if simulation {
            vec!["expected_count", "unknown_count"]
        } else {
            vec!["expected_count"]
        } {
            rows.push(QualityProjection {
                key: json!({"scope":"property","subject":p.name,"end":null,"field":field}),
                denominator: None,
                value: v[field].clone(),
            });
        }
    }
    for (v, c) in q.cohort.iter().zip(&spec.metrics) {
        let d = crate::quality::denominator(m, &c.denominator);
        if v["name"] != c.name
            || v["numerator"] != spec.properties[c.numerator].name
            || v["denominator"] != d
        {
            return Err("quality denominator or metric differs");
        }
        for field in ["value", "bounds_with_undetermined"] {
            rows.push(QualityProjection {
                key: json!({"scope":"cohort","subject":c.name,"end":null,"field":field}),
                denominator: Some(d.clone()),
                value: v[field].clone(),
            });
        }
    }
    let keys = crate::quality::outcome_keys(m);
    if keys.len() != q.outcomes_per_arrival.len() {
        return Err("quality outcome inventory differs");
    }
    for (v, (kind, end)) in q.outcomes_per_arrival.iter().zip(keys) {
        if v["kind"] != kind || v["end"] != json!(end) {
            return Err("quality outcome definition differs");
        }
        for field in ["expected_count", "rate"] {
            rows.push(QualityProjection {
                key: json!({"scope":"outcome","subject":kind,"end":end,"field":field}),
                denominator: None,
                value: v[field].clone(),
            });
        }
    }
    let aborted = q.properties.iter().any(|v| {
        v["expected_count"]["reason"] == "aborted" || v["expected_count"]["reason"] == "no_samples"
    });
    let inventory = crate::quality::inventory(m, &q.analysis, aborted);
    if q.checked != inventory.checked || q.unchecked != inventory.unchecked {
        return Err("quality calculation inventory differs");
    }
    Ok(rows)
}
