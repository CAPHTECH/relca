//! ADR 0060 E9b: model randomness, sampling error and model assumptions are
//! different evidence. These projections never enter an interpreter or RNG key.
mod inputs;
pub mod wire;
use crate::ir;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wire::{Class, ContestQualification, Evidence, Item, ModelAssumption, Reason, Rules, Status};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UncertaintyWire")]
pub struct Uncertainty {
    pub rules: Rules,
    pub aleatory: Class,
    pub estimation: Class,
    pub model: Class,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UncertaintyWire {
    rules: Rules,
    aleatory: Class,
    estimation: Class,
    model: Class,
}

impl TryFrom<UncertaintyWire> for Uncertainty {
    type Error = String;
    fn try_from(w: UncertaintyWire) -> Result<Self, Self::Error> {
        let result = Self {
            rules: w.rules,
            aleatory: w.aleatory,
            estimation: w.estimation,
            model: w.model,
        };
        result.validate_shape()?;
        Ok(result)
    }
}

impl Uncertainty {
    pub fn not_lowered() -> Self {
        Self {
            rules: Rules::P2,
            aleatory: Class::absent(Status::Unavailable, Reason::NotLowered),
            estimation: Class::absent(Status::NotApplicable, Reason::ExhaustiveAnalysis),
            model: Class::absent(Status::Unavailable, Reason::NotLowered),
        }
    }

    pub(crate) fn unread() -> Self {
        Self {
            rules: Rules::P2,
            aleatory: Class::absent(Status::Unavailable, Reason::InputUnavailable),
            estimation: Class::absent(Status::NotApplicable, Reason::NoAnalysis),
            model: Class::absent(Status::Unavailable, Reason::InputUnavailable),
        }
    }

    pub(crate) fn from_model(m: &ir::Model) -> Self {
        let mut result = Self::not_lowered();
        result.aleatory = match inputs::collect(m) {
            Some(items) if items.is_empty() => {
                Class::absent(Status::NotConfigured, Reason::NoProbabilisticModel)
            }
            Some(items) => Class::available(items),
            None => Class::absent(Status::Unavailable, Reason::EffectiveInputsUnavailable),
        };
        // Preserve the IR property kind even when an aborted run withdraws all
        // statistical row fields. Display IDs are not semantic discriminants.
        let queries = m
            .verify
            .iter()
            .enumerate()
            .filter(|(_, v)| matches!(v.property, ir::Property::ProbabilityReached(_)))
            .map(|(i, _)| Evidence::StatisticalQuery {
                at: format!("/properties/{i}"),
            })
            .collect::<Vec<_>>();
        if !queries.is_empty() {
            result.estimation = Class::available(queries);
        }
        result
    }

    /// Refresh report-owned pointers after refusal/withdrawal and after sampling.
    /// Input distributions already came from the effective IR; no source I/O.
    pub(crate) fn refresh(&mut self, host: &Value, sampling: bool) {
        let input_missing = host["model"]["sha256"].is_null();
        let lowered =
            host["ledger"]["actor_versions"]["status"] == "known" || host["actors"].is_array();
        if input_missing {
            self.aleatory = Class::absent(Status::Unavailable, Reason::InputUnavailable);
        }
        self.model = if input_missing {
            Class::absent(Status::Unavailable, Reason::InputUnavailable)
        } else if lowered {
            Class::available(model_items(host, &self.aleatory))
        } else {
            Class::absent(Status::Unavailable, Reason::NotLowered)
        };
        self.estimation = if !sampling {
            Class::absent(Status::NotApplicable, Reason::ExhaustiveAnalysis)
        } else if host["simulation"]["confidence"].is_null() {
            Class::absent(
                Status::Unavailable,
                if input_missing {
                    Reason::InputUnavailable
                } else if lowered {
                    Reason::NoAnalysis
                } else {
                    Reason::NotLowered
                },
            )
        } else {
            Class::available(estimation_items(host, &self.estimation))
        };
    }

    pub(crate) fn conform(host: &Value) -> Self {
        let mut result = Self::not_lowered();
        result.refresh(host, false);
        result.aleatory = Class::absent(Status::NotApplicable, Reason::ObservableTraceOnly);
        result.estimation = Class::absent(Status::NotApplicable, Reason::ObservableTraceOnly);
        // Scope and log gaps are observations, never Bernoulli evidence.
        if result.model.status == Status::Available {
            result.model = Class::available(model_items(host, &result.aleatory));
        }
        result
    }

    pub(crate) fn aggregate(host: &Value, mode: &str) -> Self {
        let children = crate::ledger::source_reports(host, mode);
        let class = |name: &str| {
            if children.is_empty() {
                Class::absent(Status::Unavailable, Reason::NoSourceReports)
            } else {
                Class {
                    status: Status::References,
                    reason: Some(Reason::AggregatePerSource),
                    items: children
                        .iter()
                        .map(|p| Item::Reference(format!("{p}/uncertainty/{name}")))
                        .collect(),
                }
            }
        };
        Self {
            rules: Rules::P2,
            aleatory: class("aleatory"),
            estimation: class("estimation"),
            model: class("model"),
        }
    }

    fn validate_shape(&self) -> Result<(), String> {
        for (name, class) in [
            ("aleatory", &self.aleatory),
            ("estimation", &self.estimation),
            ("model", &self.model),
        ] {
            class.validate()?;
            for item in &class.items {
                if let Item::Evidence(e) = item {
                    let valid = match name {
                        "aleatory" => matches!(
                            e,
                            Evidence::InitialDistribution { .. }
                                | Evidence::Kernel { .. }
                                | Evidence::DurationDistribution { .. }
                                | Evidence::ResponseChannel { .. }
                                | Evidence::ArrivalDistribution { .. }
                                | Evidence::ContestOccurrence { .. }
                        ),
                        "estimation" => matches!(
                            e,
                            Evidence::StatisticalQuery { .. }
                                | Evidence::PointEstimate { .. }
                                | Evidence::SamplingMethod { .. }
                        ),
                        _ => matches!(
                            e,
                            Evidence::Assumption { .. }
                                | Evidence::ModelContext { .. }
                                | Evidence::ModelAssumption { .. }
                                | Evidence::ContestEvidenceLimit { .. }
                        ),
                    };
                    if !valid {
                        return Err(format!(
                            "context_mismatch: uncertainty {name} evidence kind"
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

fn model_items(host: &Value, aleatory: &Class) -> Vec<Evidence> {
    let mut items = vec![Evidence::ModelAssumption {
        name: ModelAssumption::AnalystAssumptionsNotExhaustive,
        subjects: Vec::new(),
    }];
    if let Some(rows) = host["assumptions"].as_array() {
        items.extend((0..rows.len()).map(|i| Evidence::Assumption {
            at: format!("/assumptions/{i}"),
        }));
    }
    // A null agency/local table remains null at its pointer. Do not replace it
    // by an invented empty suppression/lineage table or recompute removed aliases.
    for field in [
        "cut",
        "cut_context",
        "bounds",
        "agency",
        "via_model_paths",
        "unchecked",
        "not_checked",
        "scope",
        "limits",
        "resource_time",
        "priority_policy",
        "work_profiles",
        "scenario",
        "diagnostics",
        "response_analysis",
        "approval_burden",
    ] {
        if host.get(field).is_some() {
            items.push(Evidence::ModelContext {
                at: format!("/{field}"),
            });
        }
    }
    if host.pointer("/ledger/execution/limits").is_some() {
        items.push(Evidence::ModelContext {
            at: "/ledger/execution/limits".into(),
        });
    }
    if host.pointer("/agency/degradation").is_some() {
        items.push(Evidence::ModelContext {
            at: "/agency/degradation".into(),
        });
    }
    let mut initials = Vec::new();
    let mut durations = Vec::new();
    let mut channels = Vec::new();
    let mut contests = Vec::new();
    for item in &aleatory.items {
        if let Item::Evidence(e) = item {
            match e {
                Evidence::InitialDistribution { variable, .. } => initials.push(variable.clone()),
                Evidence::DurationDistribution { capability, .. } => {
                    durations.push(capability.clone())
                }
                Evidence::ResponseChannel { binding, .. } => channels.push(binding.clone()),
                Evidence::ContestOccurrence { clause, .. } => contests.push(clause.clone()),
                _ => {}
            }
        }
    }
    for (name, subjects) in [
        (ModelAssumption::InitialProductIndependence, initials),
        (
            ModelAssumption::DurationResultConditionalIndependence,
            durations,
        ),
        (ModelAssumption::ResponseChannelIndependence, channels),
        (
            ModelAssumption::ContestOccurrenceIndependence,
            contests.clone(),
        ),
    ] {
        if !subjects.is_empty() {
            items.push(Evidence::ModelAssumption { name, subjects });
        }
    }
    for clause in contests {
        items.push(Evidence::ContestEvidenceLimit {
            clause,
            qualification: ContestQualification::AnalystDeclaredNotValidated,
        });
    }
    // Q4 time quantities are not a CI. Retain exact pointers including reasoned
    // nulls on refusal and the calendar/wait causes alongside the elapsed values.
    visit_quantities(host, "", &mut |at, _| {
        items.push(Evidence::ModelContext { at })
    });
    items
}

fn estimation_items(host: &Value, prior: &Class) -> Vec<Evidence> {
    let mut items = vec![Evidence::SamplingMethod {
        at: "/simulation".into(),
    }];
    if let Some(properties) = host["properties"].as_array() {
        for (i, property) in properties.iter().enumerate() {
            let at = format!("/properties/{i}");
            let statistical = property["cases"]
                .as_array()
                .is_some_and(|cases| cases.iter().any(|c| c["basis"] == "statistical"));
            let aborted_typed_query=host["simulation"]["status"]=="unsupported"
                && property["cases"].as_array().is_some_and(|cases| !cases.is_empty() && cases.iter().all(|c|c["verdict"]=="unsupported" && c["basis"]=="sample"))
                && prior.items.iter().any(|item| matches!(item,Item::Evidence(Evidence::StatisticalQuery {at:old}) if old==&at));
            if property
                .get("interval_method")
                .is_some_and(|v| !v.is_null())
                || statistical
                || aborted_typed_query
            {
                items.push(Evidence::StatisticalQuery { at });
            }
        }
    }
    for field in ["metrics", "quality", "admission"] {
        if host.get(field).is_some_and(|v| !v.is_null()) {
            items.push(Evidence::PointEstimate {
                at: format!("/{field}"),
                reason: wire::EstimateReason::IntervalNotConfigured,
            });
        }
    }
    visit_quantities(host, "", &mut |at, name| {
        if name == "arrival_time" {
            items.push(Evidence::PointEstimate {
                at,
                reason: wire::EstimateReason::IntervalNotConfigured,
            });
        }
    });
    items
}

fn visit_quantities(v: &Value, path: &str, emit: &mut impl FnMut(String, &str)) {
    match v {
        Value::Object(fields) => {
            for (key, v) in fields {
                if path.is_empty() && matches!(key.as_str(), "ledger" | "uncertainty") {
                    continue;
                }
                let at = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                if matches!(
                    key.as_str(),
                    "calendar" | "calendars" | "wait_causes" | "arrival_time"
                ) {
                    emit(at.clone(), key);
                }
                visit_quantities(v, &at, emit);
            }
        }
        Value::Array(rows) => {
            for (i, v) in rows.iter().enumerate() {
                visit_quantities(v, &format!("{path}/{i}"), emit);
            }
        }
        _ => {}
    }
}

/// Every published ledger owns one mandatory uncertainty block. Decoding checks
/// its closed payloads, actual pointer targets and exact aggregate child owners.
pub(crate) fn validate_host(host: &Value) -> Result<(), String> {
    let uncertainty: Uncertainty = serde_json::from_value(
        host.get("uncertainty")
            .cloned()
            .ok_or("context_mismatch: missing uncertainty")?,
    )
    .map_err(|e| format!("context_mismatch: uncertainty: {e}"))?;
    let mode = host["ledger"]["execution"]["mode"].as_str().unwrap_or("");
    if matches!(
        mode,
        "diff" | "compare" | "mutation" | "sensitivity_exhaustive" | "sensitivity_simulation"
    ) {
        if uncertainty != Uncertainty::aggregate(host, mode) {
            return Err("context_mismatch: uncertainty child ownership".into());
        }
        for class in [
            &uncertainty.aleatory,
            &uncertainty.estimation,
            &uncertainty.model,
        ] {
            for item in &class.items {
                if let Item::Reference(at) = item
                    && host.pointer(at).is_none()
                {
                    return Err(format!("context_mismatch: missing child uncertainty {at}"));
                }
            }
        }
    } else {
        let expected = match mode {
            "verify" | "cli_refusal" => Uncertainty::unread(),
            "log_conformance" => Uncertainty::conform(host),
            _ => {
                let mut expected = uncertainty.clone();
                expected.refresh(host, mode == "simulation");
                expected
            }
        };
        if uncertainty != expected {
            return Err("context_mismatch: uncertainty report context".into());
        }
        if !matches!(mode, "log_conformance" | "verify" | "cli_refusal")
            && !matches!(
                uncertainty.aleatory.status,
                Status::Available | Status::NotConfigured | Status::Unavailable
            )
        {
            return Err("context_mismatch: analysis aleatory classification".into());
        }
        if [
            &uncertainty.aleatory,
            &uncertainty.estimation,
            &uncertainty.model,
        ]
        .iter()
        .any(|c| c.status == Status::References)
        {
            return Err("context_mismatch: leaf uncertainty references children".into());
        }
        for class in [
            &uncertainty.aleatory,
            &uncertainty.estimation,
            &uncertainty.model,
        ] {
            for item in &class.items {
                if let Item::Evidence(e) = item
                    && let Some(at) = e.at()
                    && host.pointer(at).is_none()
                {
                    return Err(format!(
                        "context_mismatch: unresolved uncertainty evidence {at}"
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn simulation_method_is_estimation_evidence_not_a_model_context_duplicate() {
        let report = crate::report::simulate(include_str!("../tests/data/sensitivity/zero.relca"));
        assert!(report.uncertainty.estimation.items.iter().any(
            |item| matches!(item,Item::Evidence(Evidence::SamplingMethod {at}) if at=="/simulation")
        ));
        assert!(!report.uncertainty.model.items.iter().any(
            |item| matches!(item,Item::Evidence(Evidence::ModelContext {at}) if at=="/simulation")
        ));
        for kept in ["/agency", "/diagnostics"] {
            assert!(report.uncertainty.model.items.iter().any(
                |item| matches!(item,Item::Evidence(Evidence::ModelContext {at}) if at==kept)
            ));
        }
    }

    #[test]
    fn statistical_properties_are_structural_not_display_id_prefixes() {
        let host = json!({"simulation":{},"properties":[
            {"id":"renamed query","cases":[{"basis":"statistical","hits":0,"unknown":0,"trials":4,"interval":{"lower":"0","upper":"1/2"}}]},
            {"id":"probability misleading witness label","cases":[{"basis":"sample","witness":null}]},
            {"id":"another name","interval_method":"clopper-pearson","cases":[]}
        ]});
        let prior = Class::absent(Status::NotApplicable, Reason::ExhaustiveAnalysis);
        let items = estimation_items(&host, &prior);
        let paths = items
            .iter()
            .filter_map(|e| match e {
                Evidence::StatisticalQuery { at } => Some(at.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["/properties/0", "/properties/2"]);
    }

    #[test]
    fn aborted_queries_keep_typed_references_after_display_ids_change() {
        let mut host = json!(crate::report::simulate(include_str!(
            "../tests/data/sensitivity/abort.relca"
        )));
        assert_eq!(host["simulation"]["status"], "unsupported");
        let mut uncertainty: Uncertainty =
            serde_json::from_value(host["uncertainty"].clone()).unwrap();
        for (i, p) in host["properties"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            p["id"] = json!(format!("display-{i}"));
        }
        uncertainty.refresh(&host, true);
        let paths = uncertainty
            .estimation
            .items
            .iter()
            .filter_map(|e| match e {
                Item::Evidence(Evidence::StatisticalQuery { at }) => Some(at.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["/properties/0", "/properties/1"]);
        host["uncertainty"] = json!(uncertainty);
        validate_host(&host).unwrap();
    }

    #[test]
    fn q5_fixed_priority_metadata_is_model_context_not_randomness_or_estimation() {
        // Hand-built ADR 0062 metadata: this checks projection only, not the Q5
        // scheduler. Real Q5 source/engine acceptance requires the main rebase.
        let mut host = json!({
            "model":{"sha256":"hand-priority-input"}, "actors":[],
            "ledger":{"actor_versions":{"status":"known"},
                "execution":{"mode":"exhaustive","limits":{}}},
            "priority_policy":{"rules":"priority-queues-v1","case_ranks":[0,10,20],
                "known_at":"receipt","immutable":true,
                "starts":{"direction":"descending","tie":"nondet","actors":["Desk"],
                    "assign_first_idle":false,"scope":"policy_queued_only"},
                "external":null,"version_resolution":null}
        });
        let mut uncertainty = Uncertainty::not_lowered();
        uncertainty.aleatory = Class::absent(Status::NotConfigured, Reason::NoProbabilisticModel);
        let prior_aleatory = uncertainty.aleatory.clone();
        uncertainty.refresh(&host, false);
        assert_eq!(uncertainty.aleatory, prior_aleatory);
        assert_eq!(
            uncertainty.estimation,
            Class::absent(Status::NotApplicable, Reason::ExhaustiveAnalysis)
        );
        assert!(
            uncertainty
                .model
                .items
                .contains(&Item::Evidence(Evidence::ModelContext {
                    at: "/priority_policy".into(),
                }))
        );
        host["uncertainty"] = json!(uncertainty);
        validate_host(&host).unwrap();
        host.as_object_mut().unwrap().remove("priority_policy");
        assert!(
            validate_host(&host).is_err(),
            "stale priority evidence escaped"
        );
    }
}
