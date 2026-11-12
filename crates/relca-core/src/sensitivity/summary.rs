//! I12: extrema over the declared, complete finite points only. A scheduler
//! range is not a model interval; simulation CIs are never scalar extrema.
use super::input::{Mode, Target};
use crate::probability::{ProbabilityRange, Rational};
use serde::{Deserialize, Serialize};
fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub id: String,
    pub point_ids: Vec<String>,
    pub points: Vec<PointReference>,
    pub kind: SummaryKind,
    pub status: SummaryStatus,
    #[serde(deserialize_with = "nullable")]
    pub reason: Option<SummaryReason>,
    #[serde(deserialize_with = "nullable")]
    pub value: Option<ProbabilityRange>,
    pub min_point_ids: Vec<String>,
    pub max_point_ids: Vec<String>,
    pub limitation: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointReference {
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub at: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryKind {
    FinitePointExtrema,
    SimulationPointwise,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryStatus {
    Available,
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryReason {
    IncompletePoints,
    SimulationPointwise,
}

pub(super) struct Input {
    pub id: String,
    pub evaluated: bool,
    pub report: Option<serde_json::Value>,
}
pub(super) fn collect(mode: Mode, targets: &[Target], points: &[Input]) -> Vec<Summary> {
    targets.iter().map(|target| {
        let mut complete = !points.is_empty();
        let mut ranges = Vec::new();
        let references = points.iter().enumerate().map(|(i,p)| {
            let host = p.report.as_ref();
            let property = host.as_ref().and_then(|h| h["properties"].as_array()).and_then(|rows| rows.iter().enumerate().find(|(_,q)| q["id"]==target.id));
            if !p.evaluated || host.as_ref().is_none_or(|h| h["bounds"]["truncated"]!=false) {
                complete = false;
            }
            if let Some((_,q)) = property {
                match serde_json::from_value::<ProbabilityRange>(q["probability"].clone()) {
                    Ok(range) if range.min.valid() && range.max.valid() && range.min<=range.max => ranges.push((p.id.clone(),range)),
                    _ => complete = false,
                }
            } else { complete = false; }
            PointReference {id:p.id.clone(),at:property.map(|(j,_)|format!("/points/{i}/report/properties/{j}"))}
        }).collect();
        let (value,min_point_ids,max_point_ids) = if mode==Mode::Exhaustive && complete {
            let min = ranges.iter().map(|(_,p)|&p.min).min().cloned().unwrap_or_else(Rational::zero);
            let max = ranges.iter().map(|(_,p)|&p.max).max().cloned().unwrap_or_else(Rational::zero);
            let mins = ranges.iter().filter(|(_,p)|p.min==min).map(|(id,_)|id.clone()).collect();
            let maxs = ranges.iter().filter(|(_,p)|p.max==max).map(|(id,_)|id.clone()).collect();
            (Some(ProbabilityRange {min,max}),mins,maxs)
        } else { (None,vec![],vec![]) };
        let available = value.is_some();
        Summary {id:target.id.clone(),point_ids:points.iter().map(|p|p.id.clone()).collect(),points:references,
            kind:if mode==Mode::Exhaustive {SummaryKind::FinitePointExtrema} else {SummaryKind::SimulationPointwise},
            status:if available {SummaryStatus::Available} else {SummaryStatus::Unavailable},
            reason:if available {None} else if mode==Mode::Simulation && points.iter().all(|p| p.evaluated && p.report.as_ref().is_some_and(|r|r["bounds"]["truncated"]==false && r["simulation"]["replications"].as_u64().is_some_and(|n|n>0 && r["simulation"]["completed_replications"]==n))) {Some(SummaryReason::SimulationPointwise)} else {Some(SummaryReason::IncompletePoints)},
            value,min_point_ids,max_point_ids,
            limitation:if mode==Mode::Exhaustive {"enumerated_points_only; scheduler_endpoints_may_come_from_different_schedules; no_continuous_region_or_robust_guarantee"} else {"pointwise_suite_adjusted_sampling_error_only; not_model_or_aleatory_width; no_continuous_region_or_robust_guarantee"}.into()}
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const BASE: &str = include_str!("../../tests/data/sensitivity/base.relca");
    fn point(id: &str, source: &str) -> Input {
        Input {
            id: id.into(),
            evaluated: true,
            report: Some(json!(crate::report::check(source))),
        }
    }
    fn targets() -> Vec<Target> {
        vec![Target {
            kind: "probability_query".into(),
            id: "probability reached #1".into(),
        }]
    }
    #[test]
    fn finite_extrema_keep_all_sources_and_never_impute_an_unexecuted_point() {
        let mut points = vec![
            point("base", BASE),
            point(
                "high",
                include_str!("../../tests/data/sensitivity/high.relca"),
            ),
            point(
                "miss",
                include_str!("../../tests/data/sensitivity/miss.relca"),
            ),
        ];
        let s = collect(Mode::Exhaustive, &targets(), &points);
        assert_eq!(json!(s[0].value), json!({"min":"1/250","max":"1/125"}));
        assert_eq!(s[0].max_point_ids, vec!["high"]);
        assert_eq!(s[0].points.len(), 3);
        points[2].evaluated = false;
        points[2].report = None;
        let s = collect(Mode::Exhaustive, &targets(), &points);
        assert_eq!(s[0].reason, Some(SummaryReason::IncompletePoints));
        assert!(s[0].value.is_none());
        points[2] = point("cut", &BASE.replace("clock 4min", "clock 0min"));
        assert!(
            collect(Mode::Exhaustive, &targets(), &points)[0]
                .value
                .is_none()
        );
    }
}
