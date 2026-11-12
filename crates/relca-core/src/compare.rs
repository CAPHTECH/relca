//! ADR 0017 side-by-side analysis, with opt-in ADR 0047 hard filtering and sure
//! Pareto dominance (never ranking, subtraction or combined scores). Each
//! selection runs exactly one existing analysis; only validated report views are
//! projected. Comparison metadata never enters an engine state or random key.
use crate::{Verdict, comparison_projection as projection, diag, diff, ir, lower, report};
pub use projection::{
    Basis, ExhaustiveTerminal, Interval, PropertyValue, SimulationTerminal, TerminalValue, Witness,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Analysis {
    Exhaustive,
    Simulation,
}
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum SelectedReport {
    Exhaustive(report::Report),
    Simulation(report::SimulationReport),
}
impl SelectedReport {
    fn work_profiles(&self) -> Option<&report::work_profiles::WorkProfiles> {
        match self {
            Self::Exhaustive(r) => r.work_profiles.as_ref(),
            Self::Simulation(r) => r.work_profiles.as_ref(),
        }
    }
    fn agency_side(&self) -> crate::agency::ComparisonSide<'_> {
        match self {
            Self::Exhaustive(r) => crate::agency::ComparisonSide {
                agency: r.agency.as_ref(),
                responsibilities: r.responsibilities.as_deref(),
            },
            Self::Simulation(r) => crate::agency::ComparisonSide {
                agency: r.agency.as_ref(),
                responsibilities: r.responsibilities.as_deref(),
            },
        }
    }
    pub fn verdict(&self) -> Verdict {
        match self {
            Self::Exhaustive(r) => r.verdict,
            Self::Simulation(r) => r.verdict,
        }
    }
    pub fn diagnostics(&self) -> &[diag::Diagnostic] {
        match self {
            Self::Exhaustive(r) => &r.diagnostics,
            Self::Simulation(r) => &r.diagnostics,
        }
    }
    fn cut(&self) -> &report::Cut {
        match self {
            Self::Exhaustive(r) => &r.cut,
            Self::Simulation(r) => &r.cut,
        }
    }
    fn horizon(&self) -> &[ir::Horizon] {
        match self {
            Self::Exhaustive(r) => &r.cut_context.horizon,
            Self::Simulation(r) => &r.cut_context.horizon,
        }
    }
    fn view(&self, m: &ir::Model) -> projection::Result<projection::View> {
        match self {
            Self::Exhaustive(r) => projection::exhaustive(r, m),
            Self::Simulation(r) => projection::simulation(r, m),
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub configuration: String,
    pub report: SelectedReport,
}
#[derive(Debug, Clone, Serialize)]
pub struct CompareReport {
    pub ledger: crate::ledger::Ledger,
    pub uncertainty: crate::uncertainty::Uncertainty,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario: Option<String>,
    pub verdict: Verdict,
    pub mode: &'static str,
    pub analysis: Option<Analysis>,
    pub plans: Vec<Plan>,
    pub tables: Tables,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluation: Option<crate::evaluation::Evaluation>,
    pub mismatch: Option<Mismatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub common_random: Option<Option<CommonRandom>>,
    pub assumptions: Vec<ir::Assumption>,
    pub diagnostics: Vec<diag::Diagnostic>,
    pub checked: Vec<String>,
    pub unchecked: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize)]
pub struct Tables {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<Vec<ConfigurationPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agency: Option<Vec<AgencyPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "crate::quality::ordered_optional_rows")]
    pub quality: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Vec<MetricRow>>,
    pub properties: Vec<PropertyRow>,
    pub terminals: Vec<TerminalRow>,
}
#[derive(Debug, Clone, Serialize)]
pub struct AgencyPair {
    pub from: String,
    pub to: String,
    pub differences: crate::agency::Differences,
}
/// Q6 selected configuration inputs; no workload estimate is inferred here.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationPair {
    pub from: String,
    pub to: String,
    pub differences: Vec<diff::Difference>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricRow {
    pub key: projection::MetricKey,
    pub values: Vec<MetricCell>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricCell {
    pub configuration: String,
    pub status: projection::MetricStatus,
    pub value: Option<projection::MetricValue>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PropertyRow {
    pub id: String,
    pub case_id: Option<u64>,
    pub arrival: Option<u64>,
    pub values: Vec<PropertyCell>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PropertyCell {
    pub configuration: String,
    #[serde(flatten)]
    pub value: PropertyValue,
}
#[derive(Debug, Clone, Serialize)]
pub struct TerminalRow {
    pub kind: report::CaseTerminalKind,
    pub end: Option<String>,
    pub case_id: u64,
    pub arrival: u64,
    pub values: Vec<TerminalCell>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalStatus {
    Reported,
    NotReported,
    NotApplicable,
}
#[derive(Debug, Clone, Serialize)]
pub struct TerminalCell {
    pub configuration: String,
    pub status: TerminalStatus,
    pub value: Option<TerminalValue>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Mismatch {
    pub pairs: Vec<PairMismatch>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PairMismatch {
    pub left: String,
    pub right: String,
    pub cut: Option<CutMismatch>,
    pub terminals: Option<TerminalMismatch>,
}
#[derive(Debug, Clone, Serialize)]
pub struct CutMismatch {
    pub left_cut: String,
    pub right_cut: String,
    pub differences: Vec<CutDifference>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "field", rename_all = "snake_case")]
pub enum CutDifference {
    Horizon {
        left: Vec<crate::ir::Horizon>,
        right: Vec<crate::ir::Horizon>,
    },
    Question {
        left: Option<String>,
        right: Option<String>,
    },
    Include {
        left: Vec<String>,
        right: Vec<String>,
    },
    Exclude {
        left: Vec<report::CutEntry>,
        right: Vec<report::CutEntry>,
    },
    Defer {
        left: Vec<report::CutEntry>,
        right: Vec<report::CutEntry>,
    },
    Externality {
        left: Vec<String>,
        right: Vec<String>,
    },
    Reentry {
        left: Vec<String>,
        right: Vec<String>,
    },
}
impl From<diff::CutDifference> for CutDifference {
    fn from(d: diff::CutDifference) -> Self {
        match d {
            diff::CutDifference::Horizon { from, to } => Self::Horizon {
                left: from,
                right: to,
            },
            diff::CutDifference::Question { from, to } => Self::Question {
                left: from,
                right: to,
            },
            diff::CutDifference::Include { from, to } => Self::Include {
                left: from,
                right: to,
            },
            diff::CutDifference::Exclude { from, to } => Self::Exclude {
                left: from,
                right: to,
            },
            diff::CutDifference::Defer { from, to } => Self::Defer {
                left: from,
                right: to,
            },
            diff::CutDifference::Externality { from, to } => Self::Externality {
                left: from,
                right: to,
            },
            diff::CutDifference::Reentry { from, to } => Self::Reentry {
                left: from,
                right: to,
            },
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct TerminalMismatch {
    pub left_only: Vec<String>,
    pub right_only: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct CommonRandom {
    pub method: &'static str,
    pub scope: &'static str,
    pub shared: Vec<SharedKeys>,
    pub independent: Vec<IndependentKeys>,
    pub coupling: &'static str,
    pub independence: &'static str,
    pub confidence_scope: &'static str,
}
#[derive(Debug, Clone, Serialize)]
pub struct SharedKeys {
    pub kind: &'static str,
    pub case_ids: Vec<u64>,
    pub sites: Vec<String>,
    pub occurrence: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct IndependentKeys {
    pub configuration: String,
    pub process: String,
    pub kernel_sites: Vec<String>,
    pub kernel_occurrence: u64,
    pub duration_sites: Vec<String>,
    pub duration_occurrence: &'static str,
}

const CHECKED: [&str; 7] = [
    "configuration_selection",
    "cut_compatibility",
    "terminal_compatibility",
    "simulation_clause",
    "random_key_separation",
    "property_tables",
    "terminal_tables",
];
const UNCHECKED: [&str; 10] = [
    "ranking",
    "hard_filter",
    "pareto",
    "weighted_score",
    "differences",
    "difference_intervals",
    "cross_plan_confidence",
    "operation_coupling",
    "metrics",
    "terminal_meaning",
];
impl CompareReport {
    fn empty(analysis: Option<Analysis>) -> Self {
        Self {
            ledger: crate::ledger::Ledger::aggregate("compare"),
            uncertainty: crate::uncertainty::Uncertainty::not_lowered(),
            scenario: None,
            verdict: Verdict::Unsupported,
            mode: "compare",
            analysis,
            plans: Vec::new(),
            tables: Tables::default(),
            evaluation: None,
            mismatch: None,
            common_random: (analysis == Some(Analysis::Simulation)).then_some(None),
            assumptions: Vec::new(),
            diagnostics: Vec::new(),
            checked: Vec::new(),
            unchecked: Vec::new(),
        }
    }
    fn diagnostic(&mut self, code: &str, message: impl Into<String>) {
        self.diagnostics.push(diag::Diagnostic {
            code: code.into(),
            severity: diag::Severity::Error,
            message: message.into(),
            span: None,
        });
    }
    fn finish(mut self) -> Self {
        // Applicability is established from the selected IR, not from an
        // automatic property that might be absent after a backend abort.
        let receipt_not_applicable = self
            .unchecked
            .iter()
            .any(|x| x == projection::RECEIPT_NOT_APPLICABLE);
        let applies = |name: &&str| {
            !["simulation_clause", "random_key_separation"].contains(name)
                || self.analysis == Some(Analysis::Simulation)
        };
        let checked: BTreeSet<_> = self.checked.iter().cloned().collect();
        self.checked = CHECKED
            .iter()
            .filter(|n| checked.contains(**n))
            .map(|s| s.to_string())
            .collect();
        self.unchecked = UNCHECKED
            .iter()
            .chain(
                CHECKED
                    .iter()
                    .filter(|n| applies(n))
                    .filter(|n| !checked.contains(**n)),
            )
            .map(|s| s.to_string())
            .collect();
        if receipt_not_applicable {
            self.unchecked
                .push(projection::RECEIPT_NOT_APPLICABLE.into());
        }
        if checked.contains("metric_tables") {
            self.checked.push("metric_tables".into());
            self.unchecked.retain(|s| s != "metrics");
            self.unchecked.extend([
                "metric_differences".into(),
                "metric_ranking".into(),
                "metric_semantic_alignment".into(),
            ]);
        } else if self.tables.metrics.is_some() {
            self.unchecked.push("metric_tables".into());
        }
        if self.plans.iter().any(|p| match &p.report {
            SelectedReport::Exhaustive(r) => r.responsibilities.is_some(),
            SelectedReport::Simulation(r) => r.responsibilities.is_some(),
        }) {
            self.unchecked
                .extend(["responsibility".into(), "contest".into()]);
        }
        let has_obligations = self.plans.iter().any(|p| match &p.report {
            SelectedReport::Exhaustive(r) => r.obligations.is_some(),
            // Simulation explicitly omits these detailed tables in its own
            // report; there is no transported table to compare in this mode.
            SelectedReport::Simulation(_) => false,
        });
        let has_contests = self.plans.iter().any(|p| match &p.report {
            SelectedReport::Exhaustive(r) => r.contest_runtime.is_some(),
            SelectedReport::Simulation(_) => false,
        });
        if has_obligations {
            self.unchecked.push("obligation_tables".into());
        }
        if has_contests {
            self.unchecked.push("contest_runtime_tables".into());
        }
        if self.plans.iter().any(|p| {
            let metrics = match &p.report {
                SelectedReport::Exhaustive(r) => &r.metrics,
                SelectedReport::Simulation(r) => &r.metrics,
            };
            metrics
                .as_ref()
                .is_some_and(|m| m.overall.queue_wait_observed.is_some())
        }) {
            self.unchecked.push("waiting_tables".into());
        }
        self.verdict = fold(
            self.plans
                .iter()
                .map(|p| p.report.verdict())
                .chain(self.evaluation.iter().flat_map(|e| {
                    e.hard
                        .iter()
                        .flat_map(|h| h.values.iter().map(|v| v.verdict))
                })),
            &self.diagnostics,
        );
        if self.evaluation.is_some() {
            self.unchecked.retain(|s| s != "hard_filter");
            self.checked
                .extend(["hard_filter".into(), "admissibility".into()]);
            if self.analysis == Some(Analysis::Exhaustive) {
                self.unchecked.retain(|s| s != "pareto");
                self.checked.push("pareto_sure_dominance".into());
            } else {
                self.checked.push("hard_cross_plan_bonferroni".into());
            }
        }
        self.seal()
    }
    fn seal(mut self) -> Self {
        let host = serde_json::to_value(&self).expect("comparison uncertainty context");
        self.uncertainty = crate::uncertainty::Uncertainty::aggregate(&host, "compare");
        let host = serde_json::to_value(&self).expect("comparison wire");
        crate::ledger::complete(
            &mut self.ledger,
            &host,
            &mut self.verdict,
            &mut self.diagnostics,
        );
        self
    }
}
fn fold(verdicts: impl Iterator<Item = Verdict>, diagnostics: &[diag::Diagnostic]) -> Verdict {
    verdicts
        .chain(
            diagnostics
                .iter()
                .filter(|d| d.severity == diag::Severity::Error)
                .map(|d| {
                    if d.code.starts_with("U-") {
                        Verdict::Unsupported
                    } else {
                        Verdict::Invalid
                    }
                }),
        )
        .max_by_key(|v| report::precedence(*v))
        .unwrap_or(Verdict::Unsupported)
}
/// The API and CLI use the same strict ASCII identifier and order contract.
pub fn valid_plans(plans: &[&str]) -> bool {
    fn identifier(s: &str) -> bool {
        let mut b = s.bytes();
        b.next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
            && b.all(|b| b.is_ascii_alphanumeric() || b == b'_')
    }
    plans.len() >= 2
        && plans.iter().all(|p| identifier(p))
        && plans.iter().collect::<BTreeSet<_>>().len() == plans.len()
}
pub fn argument_failure(message: &str) -> CompareReport {
    let mut result = CompareReport::empty(None);
    result.diagnostic(diag::E_SYN_0, message);
    result.finish()
}
pub fn input_failure(message: &str, plans: &[&str], analysis: Analysis) -> CompareReport {
    input_failure_with_scenario(message, plans, analysis, None)
}
pub fn input_failure_with_scenario(
    message: &str,
    plans: &[&str],
    analysis: Analysis,
    scenario: Option<&str>,
) -> CompareReport {
    let mut result = CompareReport::empty(Some(analysis));
    result.plans = plans
        .iter()
        .map(|name| Plan {
            configuration: (*name).into(),
            report: match analysis {
                Analysis::Exhaustive => {
                    let mut r = report::comparison_input_failure(message);
                    r.ledger.requested(Some(name), scenario);
                    SelectedReport::Exhaustive(report::seal(r))
                }
                Analysis::Simulation => {
                    let mut r = report::simulation_input_failure(message);
                    r.ledger.requested(Some(name), scenario);
                    SelectedReport::Simulation(report::seal_simulation(r))
                }
            },
        })
        .collect();
    result.finish()
}

pub fn compare(src: &str, plans: &[&str], analysis: Analysis) -> CompareReport {
    compare_with_scenario(src, plans, analysis, None)
}

pub fn compare_with_scenario(
    src: &str,
    plans: &[&str],
    analysis: Analysis,
    scenario: Option<&str>,
) -> CompareReport {
    if let Ok(ast) = crate::syntax::parse(src) {
        let distribution = ast.items.iter().any(|i| match i {
            crate::ast::Item::ArrivalsDistribution(_) => true,
            crate::ast::Item::Scenario(s) => s
                .items
                .iter()
                .any(|i| matches!(i, crate::ast::ScenarioItem::ArrivalsDistribution(_))),
            _ => false,
        });
        if distribution
            && plans.iter().any(|name| {
                lower::lower_configuration_with_scenario(&ast, name, scenario)
                    .is_ok_and(|s| s.model.arrival_distribution.is_some())
            })
        {
            let mut result = CompareReport::empty(Some(analysis));
            result.diagnostic(diag::U_SEM_0, "stochastic_arrivals_mode_unsupported");
            return result.finish();
        }
    }
    if scenario.is_some_and(|s| s.contains(',')) {
        let mut result = CompareReport::empty(Some(analysis));
        result.diagnostic(
            diag::U_SEM_0,
            "compare under several scenarios is unsupported",
        );
        return result.finish();
    }
    let mut result = compare_using(plans, analysis, |name, analysis| match analysis {
        Analysis::Exhaustive => {
            let (r, m) =
                report::check_selected_scenario_using(src, name, scenario, crate::engine::explore);
            (
                SelectedReport::Exhaustive(report::finite_driver(
                    r,
                    crate::engine::DEFAULT_STATE_BUDGET,
                )),
                m,
            )
        }
        Analysis::Simulation => {
            let (r, m) = if scenario.is_none() {
                report::simulate_selected_metadata(src, name)
            } else {
                report::simulate_selected_scenario(src, name, scenario)
            };
            (SelectedReport::Simulation(r), m)
        }
    });
    if !result.plans.is_empty()
        && result.plans.iter().all(|p| match &p.report {
            SelectedReport::Exhaustive(r) => r.scenario.is_some(),
            SelectedReport::Simulation(r) => r.scenario.is_some(),
        })
    {
        result.scenario = scenario.map(String::from);
    }
    result.seal()
}
fn compare_using(
    plans: &[&str],
    analysis: Analysis,
    mut analyze: impl FnMut(&str, Analysis) -> (SelectedReport, Option<lower::SelectedConfiguration>),
) -> CompareReport {
    #[cfg(test)]
    crate::evaluation_spy::hit(3);
    if !valid_plans(plans) {
        return argument_failure("plans require at least two distinct ASCII identifiers");
    }
    let mut result = CompareReport::empty(Some(analysis));
    let mut metadata = Vec::new();
    for &configuration in plans {
        let (report, selected) = analyze(configuration, analysis);
        result.plans.push(Plan {
            configuration: configuration.into(),
            report,
        });
        metadata.push(selected);
    }
    result.assumptions.push(ir::Assumption{field:"compare.purpose".into(),value:"side-by-side-only".into(),affects:"plans are listed in requested order without ranking, a baseline, hard filtering, Pareto selection or a combined score".into()});
    if analysis == Analysis::Simulation {
        result.assumptions.extend([
            ir::Assumption{field:"compare.common_random".into(),value:"initial-only".into(),affects:"case initial keys and actor-local initial keys with case ID 0 are shared when the corresponding draws occur; rejected or unarrived cases do not draw; kernel and duration sites are separated by distinct process names; independence of distinct SHA-256 keys is a modeling assumption".into()},
            ir::Assumption{field:"compare.confidence".into(),value:"within-plan-only".into(),affects:"Bonferroni correction applies to probability-query case rows within each plan; no simultaneous confidence across plans or interval for a difference is claimed".into()},
        ]);
    }
    let Some(metadata) = metadata.into_iter().collect::<Option<Vec<_>>>() else {
        return result.finish();
    };
    if metadata
        .iter()
        .any(|m| !m.model.response_attempts().is_empty())
        && metadata
            .iter()
            .any(|m| m.model.response_attempts().is_empty())
    {
        result
            .unchecked
            .push(projection::RECEIPT_NOT_APPLICABLE.into());
    }
    if metadata
        .iter()
        .skip(1)
        .any(|m| report::priority::comparison_mismatch(&metadata[0].model, &m.model))
    {
        result.diagnostic(diag::U_SEM_0, "priority_feature_combination: compare requires identical priority inputs and disciplines");
        return result.finish();
    }
    if metadata
        .iter()
        .any(|s| s.model.resource_time.is_some() || s.model.has_condition_wait())
    {
        result.unchecked.push("resource_time_tables".into());
    }
    result.checked.push("configuration_selection".into());
    let declaration = metadata[0].model.comparison.as_ref();
    if metadata.iter().any(|m| m.model.comparison.is_some()) {
        for m in &metadata {
            if let Err(message) = crate::evaluation::validate_ir(&m.model) {
                result.diagnostic(diag::U_SEM_0, message);
                return result.finish();
            }
            if m.model.comparison.as_ref() != declaration {
                result.diagnostic(diag::U_SEM_0, "comparison specs differ");
                return result.finish();
            }
        }
    }
    if let Some(c) = declaration {
        if !c.plans.iter().map(String::as_str).eq(plans.iter().copied()) {
            result.diagnostic(diag::E_CONF_0, "plans differ from the compare declaration");
        }
        if let Some(under) = &c.under
            && !metadata[0]
                .model
                .scenario
                .as_ref()
                .is_some_and(|s| under.contains(&s.name))
        {
            result.diagnostic(
                diag::E_CONF_0,
                "scenario is not under the compare declaration",
            );
        }
        if let Some(cut) = &c.cut
            && metadata
                .iter()
                .any(|m| m.model.cut.as_ref().is_none_or(|c| &c.name != cut))
        {
            result.diagnostic(diag::E_CMP_0, "compare cut differs from plan cut");
        }
    }
    if metadata.iter().any(|m| m.model.quality.is_some()) {
        result.tables.quality = Some(Vec::new());
    }
    if analysis == Analysis::Simulation {
        result.checked.push("simulation_clause".into());
    }
    let mut pairs = Vec::new();
    for i in 0..plans.len() {
        for j in i + 1..plans.len() {
            let (report::Cut::Declared(a), report::Cut::Declared(b)) =
                (result.plans[i].report.cut(), result.plans[j].report.cut())
            else {
                result.diagnostic(diag::U_SEM_0, "selected report lacks a resolved Cut");
                return result.finish();
            };
            let (ah, bh) = (
                result.plans[i].report.horizon(),
                result.plans[j].report.horizon(),
            );
            let cut = (a.name != b.name || ah != bh).then(|| CutMismatch {
                left_cut: a.name.clone(),
                right_cut: b.name.clone(),
                differences: diff::cut_differences(a, b, ah, bh)
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            });
            let (a, b) = (&metadata[i].terminal_names, &metadata[j].terminal_names);
            let terminals = (a != b).then(|| TerminalMismatch {
                left_only: a.difference(b).cloned().collect(),
                right_only: b.difference(a).cloned().collect(),
            });
            if cut.is_some() || terminals.is_some() {
                let what = match (cut.is_some(), terminals.is_some()) {
                    (true, true) => "Cuts and terminal names",
                    (true, false) => "Cuts",
                    _ => "terminal names",
                };
                result.diagnostic(
                    diag::E_CMP_0,
                    format!(
                        "configurations {} and {} have different {what}",
                        plans[i], plans[j]
                    ),
                );
                pairs.push(PairMismatch {
                    left: plans[i].into(),
                    right: plans[j].into(),
                    cut,
                    terminals,
                });
            }
        }
    }
    result
        .checked
        .extend(["cut_compatibility".into(), "terminal_compatibility".into()]);
    if !pairs.is_empty() {
        result.mismatch = Some(Mismatch { pairs });
    }
    // Declaration failures do not invalidate the resolved pair metadata. Keep
    // every interplan mismatch before stopping, but never evaluate these plans.
    if !result.diagnostics.is_empty() {
        return result.finish();
    }
    if analysis == Analysis::Simulation {
        let mut groups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (p, m) in plans.iter().zip(&metadata) {
            groups.entry(&m.model.process.name).or_default().push(p);
        }
        for (process, names) in groups {
            if names.len() > 1 {
                result.diagnostic(diag::U_SEM_0,format!("configurations {} refer to the same process {process}; operation keys would be shared",names.join(", ")));
                result.diagnostics.last_mut().unwrap().span = metadata
                    .iter()
                    .find(|m| m.model.process.name == process)
                    .map(|m| m.process_reference_span);
            }
        }
        if !result.diagnostics.is_empty() {
            return result.finish();
        }
        result.checked.push("random_key_separation".into());
        result.common_random = Some(Some(common_random(plans, &metadata)));
    }
    match tables(&result.plans, &metadata, analysis) {
        Ok((tables, terminals_checked)) => {
            result.tables = tables;
            if let Err(message) = q6_tables(&mut result.tables, &result.plans, &metadata) {
                result.tables = Tables::default();
                result.diagnostic(diag::U_SEM_0, message);
                return result.finish();
            }
            if declaration.is_some() {
                match crate::evaluation::evaluate(&result.plans, &metadata, analysis) {
                    Ok(evaluation) => {
                        result.evaluation = Some(evaluation);
                        let side = |i: usize| result.plans[i].report.agency_side();
                        let mut pairs = Vec::new();
                        for i in 0..plans.len() {
                            for j in i + 1..plans.len() {
                                pairs.push(AgencyPair {
                                    from: plans[i].into(),
                                    to: plans[j].into(),
                                    differences: crate::agency::differences_for_models(
                                        side(i),
                                        &metadata[i].model,
                                        side(j),
                                        &metadata[j].model,
                                    ),
                                });
                            }
                        }
                        result.tables.agency = Some(pairs);
                        result.assumptions[0]=ir::Assumption {field:"compare.purpose".into(),value:"declared-evaluation".into(),affects:"hard constraints filter admissible plans before sure dominance; no ranking, baseline or combined score".into()};
                        result.assumptions.push(ir::Assumption {field:"compare.hard".into(),value:"conservative-endpoints".into(),affects:"Exhaustive intervals cover all schedules; violated means violation for every schedule and every resolution of unknown mass. Simulation uses per-case Clopper-Pearson sums with cross-plan Bonferroni and hits plus unknown at the upper endpoint, not point estimates or a proof. Truncated and undetermined mass stays unknown. The verify and hard families together do not have simultaneous confidence equal to confidence; neither mode guarantees reality.".into()});
                        let shared = crate::evaluation::shared_objectives(&metadata[0].model)
                            .into_iter()
                            .map(|(a, b)| format!("({a}, {b})"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        let shared = if shared.is_empty() {
                            "none among the declared objectives.".into()
                        } else {
                            shared
                        };
                        result.assumptions.push(ir::Assumption {field:"compare.pareto".into(),value:"sure-dominance-only".into(),affects:format!("Non-dominated means not certainly dominated, not proved Pareto optimal. Objectives are not added. Simulation dominance is not computed. Shared observations: {shared}")});
                    }
                    Err(message) => {
                        result.tables = Tables::default();
                        result.diagnostic(diag::U_SEM_0, message);
                        return result.finish();
                    }
                }
            }
            // Claim transported tables only after all fallible evaluation has
            // succeeded: its error path discards the entire table bundle.
            if result
                .tables
                .metrics
                .as_ref()
                .is_some_and(|rows| !rows.is_empty())
            {
                result.checked.push("metric_tables".into());
            }
            result.checked.push("property_tables".into());
            if terminals_checked {
                result.checked.push("terminal_tables".into());
            }
        }
        Err(message) => result.diagnostic(diag::U_SEM_0, message),
    }
    result.finish()
}

fn q6_tables(
    tables: &mut Tables,
    plans: &[Plan],
    metadata: &[lower::SelectedConfiguration],
) -> Result<(), String> {
    if !metadata
        .iter()
        .any(|m| m.model.uses_work_profiles() || !m.model.process.degradations.is_empty())
    {
        return Ok(());
    }
    let mut configuration = Vec::new();
    let mut agency = Vec::new();
    for i in 0..plans.len() {
        for j in i + 1..plans.len() {
            configuration.push(ConfigurationPair {
                from: plans[i].configuration.clone(),
                to: plans[j].configuration.clone(),
                differences: diff::q6_differences(
                    &metadata[i].model,
                    plans[i].report.work_profiles(),
                    &metadata[j].model,
                    plans[j].report.work_profiles(),
                )?,
            });
            agency.push(AgencyPair {
                from: plans[i].configuration.clone(),
                to: plans[j].configuration.clone(),
                differences: crate::agency::differences_for_models(
                    plans[i].report.agency_side(),
                    &metadata[i].model,
                    plans[j].report.agency_side(),
                    &metadata[j].model,
                ),
            });
        }
    }
    tables.configuration = Some(configuration);
    tables.agency = Some(agency);
    Ok(())
}

fn tables(
    plans: &[Plan],
    metadata: &[lower::SelectedConfiguration],
    analysis: Analysis,
) -> projection::Result<(Tables, bool)> {
    let views: Vec<_> = plans
        .iter()
        .zip(metadata)
        .map(|(p, m)| match (&p.report, analysis) {
            (SelectedReport::Exhaustive(_), Analysis::Exhaustive)
            | (SelectedReport::Simulation(_), Analysis::Simulation) => p.report.view(&m.model),
            _ => Err("nested report analysis differs from request"),
        })
        .collect::<projection::Result<_>>()?;
    let first = views.first().ok_or("empty report inventory")?;
    // CutOff rows use the first selected IR, so every plan must have the same
    // effective horizon, including its value rather than just its presence.
    if metadata
        .iter()
        .any(|m| m.model.horizon != metadata[0].model.horizon)
    {
        return Err("report inventories differ");
    }
    for v in &views {
        if v.arrivals != first.arrivals
            || !v
                .properties
                .iter()
                .map(|p| &p.id)
                .filter(|id| id.as_str() != "receipt_resolution")
                .eq(first
                    .properties
                    .iter()
                    .map(|p| &p.id)
                    .filter(|id| id.as_str() != "receipt_resolution"))
        {
            return Err("report inventories differ");
        }
    }
    let mut tables = Tables::default();
    let quality = plans
        .iter()
        .zip(metadata)
        .map(|(p, m)| {
            let q = match &p.report {
                SelectedReport::Exhaustive(r) => r.quality.as_ref(),
                SelectedReport::Simulation(r) => r.quality.as_ref(),
            };
            projection::quality_rows(q, &m.model, analysis == Analysis::Simulation)
        })
        .collect::<projection::Result<Vec<_>>>()?;
    if metadata[0].model.quality.is_some() {
        let mut rows = Vec::new();
        for (i, first) in quality[0].iter().enumerate() {
            let mut row = serde_json::json!({"key":first.key,"values":plans.iter().zip(&quality).map(|(p,q)|serde_json::json!({"configuration":p.configuration,"status":"reported","value":q[i].value})).collect::<Vec<_>>()});
            if let Some(d) = &first.denominator {
                row["denominator"] = d.clone();
            }
            rows.push(row);
        }
        tables.quality = Some(rows);
    }
    for p in &first.properties {
        // Automatic receipt resolution is applicable only to response plans.
        // Retain the side reports; do not manufacture a verdict for other plans.
        if p.id == "receipt_resolution"
            && views
                .iter()
                .any(|v| !v.properties.iter().any(|q| q.id == p.id))
        {
            continue;
        }
        for &case_id in p.values.keys() {
            let values = plans
                .iter()
                .zip(&views)
                .map(|(plan, v)| {
                    Ok(PropertyCell {
                        configuration: plan.configuration.clone(),
                        value: v
                            .properties
                            .iter()
                            .find(|q| q.id == p.id)
                            .ok_or("missing property")?
                            .values
                            .get(&case_id)
                            .ok_or("property case inventory differs")?
                            .clone(),
                    })
                })
                .collect::<projection::Result<_>>()?;
            tables.properties.push(PropertyRow {
                id: p.id.clone(),
                case_id,
                arrival: case_id.map(|id| first.arrivals[&id]),
                values,
            });
        }
    }
    // A reported end outside the lexical inventory would otherwise drop out of the table.
    let names = &metadata[0].terminal_names;
    if !views.iter().all(|v| {
        v.terminals.keys().all(|(_, k)| match k {
            projection::TerminalKey::End(e) | projection::TerminalKey::Overflow(e) => {
                names.contains(e)
            }
            projection::TerminalKey::Rejected
            | projection::TerminalKey::CutOff
            | projection::TerminalKey::ReceiptUnknown => true,
        })
    }) {
        return Err("reported terminal is outside the selected process's terminal names");
    }
    let complete = views.iter().all(|v| !v.aborted);
    if complete {
        let keys = metadata[0]
            .terminal_names
            .iter()
            .cloned()
            .map(|name| {
                if metadata[0]
                    .model
                    .overflow_end
                    .is_some_and(|e| metadata[0].model.ends[e.0] == name)
                {
                    projection::TerminalKey::Overflow(name)
                } else {
                    projection::TerminalKey::End(name)
                }
            })
            .chain(std::iter::once(projection::TerminalKey::Rejected))
            .chain(
                metadata
                    .iter()
                    .any(|m| !m.model.response_attempts().is_empty())
                    .then_some(projection::TerminalKey::ReceiptUnknown),
            )
            // Use the selected scenario's effective horizon, not observed cuts:
            // every plan needs the same lexical row even when all cases finish.
            .chain(
                metadata[0]
                    .model
                    .horizon
                    .is_some()
                    .then_some(projection::TerminalKey::CutOff),
            );
        for key in keys {
            for (&case_id, &arrival) in &first.arrivals {
                let (kind, end) = key.fields();
                let values = plans
                    .iter()
                    .zip(&views)
                    .zip(metadata)
                    .map(|((plan, v), m)| {
                        let applicable = key != projection::TerminalKey::ReceiptUnknown
                            || !m.model.response_attempts().is_empty();
                        let value =
                            v.terminals
                                .get(&(case_id, key.clone()))
                                .cloned()
                                .or_else(|| {
                                    (analysis == Analysis::Simulation && applicable).then_some(
                                        TerminalValue::Simulation(SimulationTerminal { count: 0 }),
                                    )
                                });
                        TerminalCell {
                            configuration: plan.configuration.clone(),
                            status: if !applicable {
                                TerminalStatus::NotApplicable
                            } else if value.is_some() {
                                TerminalStatus::Reported
                            } else {
                                TerminalStatus::NotReported
                            },
                            value,
                        }
                    })
                    .collect();
                tables.terminals.push(TerminalRow {
                    kind,
                    end,
                    case_id,
                    arrival,
                    values,
                });
            }
        }
    }
    if metadata[0].model.metrics.is_some() {
        tables.metrics = Some(Vec::new());
        let metric_reports: Vec<_> = plans
            .iter()
            .map(|p| match &p.report {
                SelectedReport::Exhaustive(r) => r.metrics.as_ref(),
                SelectedReport::Simulation(r) => r.metrics.as_ref(),
            })
            .collect::<Option<Vec<_>>>()
            .ok_or("missing metrics report")?;
        if complete
            && metric_reports
                .iter()
                .all(|m| m.status != crate::metrics::Status::Aborted)
        {
            let rows: Vec<_> = metric_reports
                .iter()
                .zip(metadata)
                .map(|(report, m)| projection::metric_rows(report, &m.model))
                .collect::<projection::Result<_>>()?;
            let keys: BTreeSet<_> = rows.iter().flat_map(|r| r.keys().cloned()).collect();
            tables.metrics = Some(
                keys.into_iter()
                    .map(|key| {
                        let values = plans
                            .iter()
                            .zip(&rows)
                            .map(|(p, rows)| {
                                let cell = projection::metric_cell(rows.get(&key));
                                MetricCell {
                                    configuration: p.configuration.clone(),
                                    status: cell.status,
                                    value: cell.value,
                                }
                            })
                            .collect();
                        MetricRow { key, values }
                    })
                    .collect(),
            );
        }
    }
    Ok((tables, complete))
}

fn common_random(plans: &[&str], metadata: &[lower::SelectedConfiguration]) -> CommonRandom {
    let m = &metadata[0].model;
    let mut case_sites = Vec::new();
    let mut actor_sites = Vec::new();
    for v in &m.state_variables {
        if matches!(v.initial, ir::InitialValue::Distribution(_)) {
            match v.kind {
                ir::StateKind::ActorLatent(a) => {
                    let sites = if m.actors[a.0].is_per_case() {
                        &mut case_sites
                    } else {
                        &mut actor_sites
                    };
                    sites.push(format!("init.actor.{}.{}", m.actors[a.0].name, v.name))
                }
                _ => case_sites.push(format!("init.case.{}", v.name)),
            }
        }
    }
    fn calls<'a>(block: &'a ir::Block, out: &mut Vec<&'a ir::Call>) {
        for s in block {
            match &s.kind {
                ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => out.push(c),
                ir::Stmt::Fork { branches, .. } => out.extend(branches),
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    calls(then_body, out);
                    calls(else_body, out)
                }
                _ => {}
            }
        }
    }
    let independent = plans
        .iter()
        .zip(metadata)
        .map(|(name, selected)| {
            let m = &selected.model;
            let mut sites = Vec::new();
            calls(&m.process.body, &mut sites);
            sites.sort_by_key(|c| c.binding);
            let sites_for = |kind: &str| {
                sites
                    .iter()
                    .filter(|c| {
                        let cap = &m.capabilities[c.capability.0];
                        if kind == "kernel" {
                            cap.kernel.is_some() || cap.performance.is_some()
                        } else {
                            cap.duration_distribution.is_some()
                        }
                    })
                    .map(|c| {
                        if let Some(r) = m.response_attempt(c.binding) {
                            format!(
                                "{kind}.{}.{}.attempt{}",
                                m.process.name,
                                m.site_name(r.command),
                                r.attempt
                            )
                        } else {
                            format!("{kind}.{}.{}", m.process.name, m.site_name(c.binding))
                        }
                    })
                    .collect()
            };
            IndependentKeys {
                configuration: (*name).into(),
                process: m.process.name.clone(),
                kernel_sites: sites_for("kernel"),
                kernel_occurrence: 1,
                duration_sites: sites_for("duration"),
                duration_occurrence: "positive-support-order",
            }
        })
        .collect();
    CommonRandom {
        method: "sha256-counter-v1",
        scope: "initial-only",
        shared: vec![
            SharedKeys {
                kind: "case_initial",
                case_ids: (1..=m.bounds.cases).collect(),
                sites: case_sites,
                occurrence: 1,
            },
            SharedKeys {
                kind: "actor_initial",
                case_ids: vec![0],
                sites: actor_sites,
                occurrence: 1,
            },
        ],
        independent,
        coupling: "potential-draws-common-when-admitted",
        independence: "domain-separated-sha256-assumption",
        confidence_scope: "within-plan-only",
    }
}

#[cfg(test)]
#[path = "../tests/support/evaluation_review_private_compare.rs"]
mod evaluation_review_private_compare;
#[cfg(test)]
#[path = "evaluation_tests.rs"]
mod evaluation_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{engine, syntax};
    use serde_json::{Value, json};
    const CASES: &str = include_str!("../../../fixtures/compare-case-diff/model.relca");
    const RANDOM: &str = include_str!("../../../fixtures/compare-random/model.relca");
    #[test]
    fn q6_performance_simulation_lists_the_actual_kernel_draw_sites() {
        // Implementation-side wiring check, not the independent hand oracle.
        let source = r#"model Q6Compare version "0.2" {
            type R = enum { ok }
            kernel K { * -> { 1:ok } }
            performance P basis "assumption" { load none; executor anonymous { kernel K } }
            actor A { capacity 1; capability c() -> R { duration fixed 1min; outcomes performance P } }
            process Before { r = A.c(); end done }
            process After { r = A.c(); end done }
            cut C { include [A] }
            configuration From { cut C; process Before }
            configuration To { cut C; process After }
            bounds { cases 1; live_tasks 4; clock 10min }
            simulation replications 1 seed 1 confidence 0.5
            verify reachable end.done
        }"#;
        let result = compare(source, &["From", "To"], Analysis::Simulation);
        assert_eq!(
            result.verdict,
            Verdict::Satisfied,
            "{:?}",
            result.diagnostics
        );
        let keys = result.common_random.unwrap().unwrap().independent;
        assert_eq!(keys[0].kernel_sites, ["kernel.Before.r"]);
        assert_eq!(keys[1].kernel_sites, ["kernel.After.r"]);
    }
    fn injected(
        mut alter: impl FnMut(&str, &mut report::Report, &mut Option<lower::SelectedConfiguration>),
    ) -> CompareReport {
        compare_using(&["Aplan", "Bplan"], Analysis::Exhaustive, |name, _| {
            let (mut r, mut m) = report::check_selected(CASES, name);
            alter(name, &mut r, &mut m);
            (SelectedReport::Exhaustive(r), m)
        })
    }
    #[test]
    fn c_internal_all_36_verdict_pairs_and_diagnostic_precedence() {
        let order = [
            Verdict::Satisfied,
            Verdict::Vacuous,
            Verdict::Inconclusive,
            Verdict::Unsupported,
            Verdict::Violated,
            Verdict::Invalid,
        ];
        for (i, &a) in order.iter().enumerate() {
            for (j, &b) in order.iter().enumerate() {
                for values in [[a, b], [b, a]] {
                    assert_eq!(fold(values.into_iter(), &[]), order[i.max(j)]);
                    for (code, index) in [(diag::E_CMP_0, 5), (diag::U_SEM_0, 3)] {
                        let ds = [diag::Diagnostic {
                            code: code.into(),
                            severity: diag::Severity::Error,
                            message: "injected".into(),
                            span: None,
                        }];
                        assert_eq!(fold(values.into_iter(), &ds), order[i.max(j).max(index)]);
                    }
                }
            }
        }
        assert_eq!(fold(std::iter::empty(), &[]), Verdict::Unsupported);
    }
    #[test]
    fn c_internal_case_alignment_uses_id_not_vector_position() {
        let normal = serde_json::to_value(injected(|_, _, _| {})).unwrap();
        let reverse = serde_json::to_value(injected(|name, r, _| {
            if name == "Bplan" {
                r.case_outcomes.as_mut().unwrap().reverse();
                for p in &mut r.properties {
                    if let Some(c) = &mut p.cases {
                        c.reverse();
                    }
                }
            }
        }))
        .unwrap();
        assert_eq!(normal["tables"], reverse["tables"]);
        assert_eq!(reverse["tables"]["terminals"][1]["case_id"], 2);
        assert_eq!(reverse["tables"]["terminals"][1]["arrival"], 3);
        // Equal arrival times must not mask a positional property join: the
        // slow plan's case probabilities are 1/2 and 1/4, not interchangeable.
        let source = include_str!("../../../fixtures/compare-arrivals/model.relca");
        let run = |reverse: bool| {
            compare_using(&["Aplan", "Bplan"], Analysis::Exhaustive, |name, _| {
                let (mut r, m) = report::check_selected(source, name);
                if reverse && name == "Aplan" {
                    r.case_outcomes.as_mut().unwrap().reverse();
                    for p in &mut r.properties {
                        if let Some(c) = &mut p.cases {
                            c.reverse();
                        }
                    }
                }
                (SelectedReport::Exhaustive(r), m)
            })
        };
        assert_eq!(
            serde_json::to_value(run(false)).unwrap()["tables"],
            serde_json::to_value(run(true)).unwrap()["tables"]
        );
    }
    #[test]
    fn c_internal_independent_keys_walk_fork_branches_and_if_bodies() {
        // Hand-derived: c has a kernel and a distribution, d has neither; bindings sort as a, b, x.
        let src = r#"model ForkKeys version "0.2" {
  type T = enum { first, second }
  kernel K { * -> { 0.5:first; 0.5:second } }
  actor A {
    capacity 2
    capability c() -> T { duration {1min:1}; outcomes kernel K }
    capability d() -> T { duration fixed 1min; result nondet { first } }
  }
  process P {
    fork f { a = A.c(); b = A.d() }
    join all f
    if a == first { x = A.c(); end yes } else { end no }
  }
  process Q { a = A.c(); if a == first { end yes } else { end no } }
  cut C { include [A] }
  configuration Aplan { cut C; process P }
  configuration Bplan { cut C; process Q }
  arrivals fixed [0min]
  bounds { cases 1; live_tasks 2; clock 5min }
  verify reachable end.yes
  simulation replications 2 seed 0 confidence 0.5
}"#;
        let out = compare(src, &["Aplan", "Bplan"], Analysis::Simulation);
        let keys = &out
            .common_random
            .as_ref()
            .and_then(|c| c.as_ref())
            .unwrap_or_else(|| panic!("{:?}", out.diagnostics))
            .independent;
        let sites = |i: usize| (keys[i].kernel_sites.clone(), keys[i].duration_sites.clone());
        assert_eq!(
            sites(0),
            (
                vec!["kernel.P.a".to_string(), "kernel.P.x".into()],
                vec!["duration.P.a".to_string(), "duration.P.x".into()]
            )
        );
        assert_eq!(
            sites(1),
            (
                vec!["kernel.Q.a".to_string()],
                vec!["duration.Q.a".to_string()]
            )
        );
    }
    #[test]
    fn c_internal_simulation_compare_legacy_and_two_case_routes() {
        // Legacy route: ToBeP leaves the fork start order open, so ADR 0016 refuses it.
        let legacy = include_str!("../../../fixtures/config-diff-parallel/model.relca").replace(
            "  verify safety no_resource_overcommit\n",
            "  verify safety no_resource_overcommit\n  simulation replications 3 seed 7 confidence 0.5\n",
        );
        let out = compare(&legacy, &["AsIs", "ToBeP"], Analysis::Simulation);
        assert_eq!(out.verdict, Verdict::Unsupported);
        let verdicts: Vec<Vec<Verdict>> = out
            .tables
            .properties
            .iter()
            .map(|r| r.values.iter().map(|c| c.value.verdict).collect())
            .collect();
        assert_eq!(
            verdicts,
            [
                [Verdict::Satisfied, Verdict::Unsupported],
                [Verdict::Inconclusive, Verdict::Unsupported]
            ]
        );
        assert!(out.tables.terminals.is_empty());
        assert!(!out.checked.iter().any(|c| c == "terminal_tables"));
        // Two cases. u < 1/2 (first hex digit below 8) draws first; P ends yes on first, Q on
        // second. kernel.P.a: case1 09e8.., 0a79.. (first, first); case2 9e64.., 78e6..
        // (second, first). kernel.Q.a: case1 d15b.., eed5.. (second, second); case2 4415..,
        // e226.. (first, second). So both plans count yes 2 / no 0 for case 1 and 1 / 1 for case 2.
        let two = r#"model MultiSim version "0.2" {
  type T = enum { first, second }
  latent case { z:T ~ {first:0.5,second:0.5} }
  kernel K { * -> { 0.5:first; 0.5:second } }
  actor A {
    capacity 1
    capability c() -> T { duration fixed 1min; outcomes kernel K }
  }
  process P { a = A.c(); if a == first { end yes } else { end no } }
  process Q { a = A.c(); if a == second { end yes } else { end no } }
  cut C { include [A] }
  configuration Aplan { cut C; process P }
  configuration Bplan { cut C; process Q }
  arrivals fixed [0min, 2min]
  bounds { cases 2; live_tasks 1; clock 5min }
  verify reachable end.yes
  verify probability reached z == first >= 0
  simulation replications 2 seed 3 confidence 0.5
}"#;
        let out = compare(two, &["Aplan", "Bplan"], Analysis::Simulation);
        assert_eq!(out.verdict, Verdict::Satisfied, "{:?}", out.diagnostics);
        let counts: Vec<(Option<String>, u64, Vec<Value>)> = out
            .tables
            .terminals
            .iter()
            .map(|r| {
                let values = r.values.iter().map(|c| json!(c.value)).collect();
                (r.end.clone(), r.case_id, values)
            })
            .collect();
        let c = |n: u64| json!({ "count": n });
        assert_eq!(
            counts,
            [
                (Some("no".into()), 1, vec![c(0), c(0)]),
                (Some("no".into()), 2, vec![c(1), c(1)]),
                (Some("yes".into()), 1, vec![c(2), c(2)]),
                (Some("yes".into()), 2, vec![c(1), c(1)]),
                (None, 1, vec![c(0), c(0)]),
                (None, 2, vec![c(0), c(0)]),
            ]
        );
    }
    #[test]
    fn c_internal_reported_end_outside_inventory_is_not_dropped() {
        let out = injected(|_, _, m| m.as_mut().unwrap().terminal_names.clear());
        assert_eq!(out.verdict, Verdict::Unsupported);
        assert!(out.tables.properties.is_empty() && out.tables.terminals.is_empty());
        assert!(out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
    }
    #[test]
    fn c_internal_malformed_report_inventory_never_publishes_partial_tables() {
        type Mutator = fn(&mut report::Report);
        let mutations: [Mutator; 12] = [
            |r| {
                r.case_outcomes.as_mut().unwrap().pop();
            },
            |r| {
                let c = r.case_outcomes.as_ref().unwrap()[0].clone();
                r.case_outcomes.as_mut().unwrap().push(c);
            },
            |r| r.case_outcomes.as_mut().unwrap()[0].arrival += 1,
            |r| {
                r.properties.pop();
            },
            |r| {
                r.properties.push(r.properties[0].clone());
            },
            |r| {
                r.properties[0].cases.as_mut().unwrap().pop();
            },
            |r| {
                let c = r.properties[0].cases.as_ref().unwrap()[0].clone();
                r.properties[0].cases.as_mut().unwrap().push(c);
            },
            |r| r.properties[0].cases.as_mut().unwrap()[0].arrival += 1,
            |r| r.case_outcomes.as_mut().unwrap()[0].terminals[0].end = None,
            |r| {
                r.case_outcomes.as_mut().unwrap()[0].terminals[0].kind =
                    report::CaseTerminalKind::Rejected
            },
            |r| r.bounds.truncated = None,
            |r| r.case_outcomes.as_mut().unwrap()[0].terminals[0].elapsed = None,
        ];
        for (i, mutation) in mutations.into_iter().enumerate() {
            let out = injected(|name, r, _| {
                if name == "Bplan" {
                    mutation(r)
                }
            });
            assert_eq!(out.verdict, Verdict::Unsupported, "mutation {i}");
            assert!(
                out.tables.properties.is_empty() && out.tables.terminals.is_empty(),
                "mutation {i}"
            );
            assert!(out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
        }
        let source = include_str!("../../../fixtures/compare-arrivals/model.relca");
        let out = compare_using(&["Aplan", "Bplan"], Analysis::Exhaustive, |name, _| {
            let (mut r, m) = report::check_selected(source, name);
            let c = r.properties.iter().find_map(|p| p.cases.clone()).unwrap();
            r.properties
                .iter_mut()
                .find(|p| p.id == projection::SAFETY)
                .unwrap()
                .cases = Some(c);
            (SelectedReport::Exhaustive(r), m)
        });
        assert_eq!(out.verdict, Verdict::Unsupported);
        assert!(out.tables.properties.is_empty());
    }
    #[test]
    fn c_internal_abort_with_known_violation_keeps_properties_retracts_terminals() {
        let out = injected(|name, r, _| {
            if name == "Bplan" {
                r.bounds.truncated = None;
                r.verdict = Verdict::Unsupported;
                for c in r.case_outcomes.as_mut().unwrap() {
                    c.terminals.clear();
                }
                for p in &mut r.properties {
                    p.verdict = Verdict::Unsupported;
                    for c in p.cases.as_mut().unwrap() {
                        c.verdict = Verdict::Unsupported;
                        c.probability = None;
                    }
                }
            } else {
                r.verdict = Verdict::Violated;
                r.properties[0].verdict = Verdict::Violated;
                r.properties[0].cases.as_mut().unwrap()[0].verdict = Verdict::Violated;
            }
        });
        assert_eq!(out.verdict, Verdict::Violated);
        assert!(out.tables.terminals.is_empty());
        assert_eq!(
            out.tables.properties[0].values[0].value.verdict,
            Verdict::Violated
        );
        assert_eq!(
            out.tables.properties[0].values[1].value.verdict,
            Verdict::Unsupported
        );
        assert!(out.unchecked.iter().any(|n| n == "terminal_tables"));
        assert!(out.checked.iter().any(|n| n == "property_tables"));
        let static_failure = injected(|name, r, m| {
            if name == "Bplan" {
                // Malformed producer: static failure with leftover numeric data.
                r.verdict = Verdict::Invalid;
                r.diagnostics.push(diag::Diagnostic {
                    code: diag::E_NAME_0.into(),
                    severity: diag::Severity::Error,
                    message: "injected static failure retaining numbers".into(),
                    span: None,
                });
                *m = None
            }
        });
        assert_eq!(static_failure.verdict, Verdict::Invalid);
        assert!(
            static_failure.tables.properties.is_empty()
                && static_failure.tables.terminals.is_empty()
        );
        assert!(
            static_failure
                .unchecked
                .iter()
                .any(|n| n == "configuration_selection")
        );
    }
    #[test]
    fn c_internal_analysis_engine_calls_once_per_valid_selection() {
        let source = RANDOM.replace(
            "  bounds",
            "  configuration Cplan { cut C; process P }\n  bounds",
        );
        for analysis in [Analysis::Exhaustive, Analysis::Simulation] {
            for plans in [
                vec!["Aplan", "Bplan"],
                vec!["Bplan", "Aplan", "Cplan"],
                vec!["Aplan", "Unknown"],
            ] {
                let mut analyzed = Vec::new();
                let mut executed = Vec::new();
                let out = compare_using(&plans, analysis, |name, mode| {
                    analyzed.push((name.to_owned(), mode));
                    match mode {
                        Analysis::Exhaustive => {
                            let (r, m) = report::check_selected_using(&source, name, |m| {
                                executed.push((name.to_owned(), mode));
                                engine::explore(m)
                            });
                            (SelectedReport::Exhaustive(r), m)
                        }
                        Analysis::Simulation => {
                            let (r, m) = report::simulate_selected_using(&source, name, |m| {
                                executed.push((name.to_owned(), mode));
                                engine::simulate(m)
                            });
                            (SelectedReport::Simulation(r), m)
                        }
                    }
                });
                assert_eq!(
                    analyzed,
                    plans
                        .iter()
                        .map(|s| (s.to_string(), analysis))
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    executed,
                    plans
                        .iter()
                        .filter(|n| **n != "Unknown")
                        .map(|s| (s.to_string(), analysis))
                        .collect::<Vec<_>>()
                );
                for p in out.plans {
                    let alone = match analysis {
                        Analysis::Exhaustive => serde_json::to_value(report::check_configuration(
                            &source,
                            &p.configuration,
                        ))
                        .unwrap(),
                        Analysis::Simulation => serde_json::to_value(
                            report::simulate_configuration(&source, &p.configuration),
                        )
                        .unwrap(),
                    };
                    assert_eq!(
                        crate::ledger_test_shape::original_report(
                            &serde_json::to_value(p.report).unwrap()
                        ),
                        crate::ledger_test_shape::original_report(&alone)
                    );
                }
            }
        }
    }
    #[test]
    fn c_internal_simulation_metadata_survives_with_lexical_end_inventory() {
        let source = RANDOM.replace(
            "end yes } else { end no }",
            "end yes; end suffix } else { end no }",
        );
        let (r, m) = report::simulate_selected_metadata(&source, "Aplan");
        let m = m.unwrap();
        assert_eq!(
            m.terminal_names,
            BTreeSet::from(["yes".into(), "no".into(), "suffix".into()])
        );
        assert_eq!(m.model.cut.as_ref().unwrap().name, "C");
        assert_eq!(
            serde_json::to_value(r).unwrap(),
            serde_json::to_value(report::simulate_configuration(&source, "Aplan")).unwrap()
        );
        let src = r#"model M version "0.2" {
          type T = enum { ok }
          actor A { capacity 2; capability c() -> T {duration fixed 1min;result nondet {ok}} }
          process P {fork f {a=A.c();b=A.c()} join any f where result == ok tie by [a,b] remaining drain if none -> ghost; end done; end suffix}
          process Q {end ghost}
          cut C {include [A]}
          configuration Aplan {cut C;process P}
          configuration Bplan {cut C;process Q}
          bounds {cases 1;live_tasks 2;clock 5min}
          verify safety no_resource_overcommit
          simulation replications 1 seed 0 confidence 0.5
        }"#;
        let (_, metadata) = report::simulate_selected_metadata(src, "Aplan");
        let m = metadata.unwrap();
        assert_eq!(
            m.terminal_names,
            BTreeSet::from(["done".into(), "ghost".into(), "suffix".into()])
        );
        assert_eq!(m.model.ends, vec!["done", "suffix", "ghost"]);
        assert!(syntax::parse(src).is_ok());
        let alias = RANDOM.replace("process Q }", "process P }");
        let out = compare(&alias, &["Aplan", "Bplan"], Analysis::Simulation);
        let span = out
            .diagnostics
            .iter()
            .find(|d| d.code == diag::U_SEM_0)
            .unwrap()
            .span
            .unwrap();
        assert_eq!(&alias[span.start..span.end], "P");
    }
    #[test]
    fn c_internal_simulation_anomalies_fail_closed_and_abort_withdraws_counts() {
        for mutation in 0..8 {
            let out = compare_using(&["Aplan", "Bplan"], Analysis::Simulation, |name, _| {
                let (mut r, m) = report::simulate_selected_metadata(RANDOM, name);
                if name == "Bplan" {
                    match mutation {
                        0 => {
                            r.case_outcomes.push(r.case_outcomes[0].clone());
                        }
                        1 => r.case_outcomes[0]["arrival"] = json!(9),
                        2 => r.case_outcomes[0]["terminals"][0]["end"] = Value::Null,
                        3 => r.properties[0]["cases"][0]["trials"] = Value::Null,
                        4 => r.properties[0]["cases"][0]["case_id"] = json!(2),
                        5 => {
                            r.simulation["status"] = json!("unsupported");
                            r.bounds.truncated = None;
                        }
                        7 => r.properties[0]["cases"][0]["unexpected"] = json!(1),
                        _ => r.properties.push(r.properties[0].clone()),
                    }
                }
                (SelectedReport::Simulation(r), m)
            });
            assert_eq!(out.verdict, Verdict::Unsupported, "mutation {mutation}");
            assert!(out.tables.properties.is_empty() && out.tables.terminals.is_empty());
        }
    }
    #[test]
    fn c_internal_u9_other_case_occupies_capacity_not_a_single_case_proof() {
        let one = r#"model CancelRead version "0.2" {
          type T = enum {pass,ok}
          actor F {capacity 2;capability run() -> T {duration fixed 1min;result nondet {pass};cancel forbidden_after_start}}
          actor S {capacity 1;capability run() -> T {duration fixed 3min;result nondet {ok};cancel forbidden_after_start}}
          actor H {capacity 2;capability use(x:T) -> T {duration fixed 1min;result nondet {ok}}}
          process P {
            fork f {fast=F.run();slow=S.run()}
            join any f where result == pass tie by [fast,slow] remaining cancel_then_drain if none -> rejected
            if slow == ok {h=H.use(slow);end done} else {end rejected}
          }
          cut C {include [F,S,H]}
          configuration Aplan {cut C;process P}
          configuration Bplan {cut C;process P}
          bounds {cases 1;live_tasks 2;clock 20min}
          verify safety no_resource_overcommit
        }"#;
        let mut forged = lower::lower_configuration(&syntax::parse(one).unwrap(), "Aplan")
            .unwrap()
            .model;
        assert_eq!(
            engine::explore(&forged).properties[0].verdict,
            Verdict::Satisfied
        );
        // Fault injection: resurrect the cases=1 capacity-only proof in public IR.
        // One S task starts, the other remains queued and is cancelled without a result.
        forged.bounds.cases = 2;
        forged.arrivals = Some(vec![0, 0]);
        let failed = engine::explore(&forged);
        assert!(failed.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
        assert_eq!(
            crate::reference::explore(&forged).stop,
            crate::reference::StopReason::SemanticAbort
        );
        let multi = one
            .replace("cases 1", "cases 2")
            .replace("bounds {", "arrivals fixed [0min,0min]\n bounds {");
        let arg = multi.replace(
            "if slow == ok {h=H.use(slow);end done} else {end rejected}",
            "h=H.use(slow);end done;end rejected",
        );
        let invariant = arg
            .replace("H.use(slow)", "H.use()")
            .replace("use(x:T)", "use()")
            .replace(
                "bounds {",
                "contract C {invariant H.use.finish -> slow == ok}\n bounds {",
            );
        for src in [&multi, &arg, &invariant] {
            let r = report::check_configuration(src, "Aplan");
            assert_eq!(r.verdict, Verdict::Unsupported);
            assert!(r.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
        }
        let observed = multi
            .replace("actor F {", "observed case {slow:T=ok}\n actor F {")
            .replace("duration fixed 3min;", "writes slow;duration fixed 3min;");
        let selected =
            lower::lower_configuration(&syntax::parse(&observed).unwrap(), "Aplan").unwrap();
        let actual = engine::explore(&selected.model);
        let reference = crate::reference::explore(&selected.model);
        assert_eq!(reference.stop, crate::reference::StopReason::Complete);
        assert_eq!(actual.properties, reference.outcome.properties);
        assert_eq!(actual.case_outcomes, reference.outcome.case_outcomes);
        assert!(
            actual
                .diagnostics
                .iter()
                .all(|d| d.severity != diag::Severity::Error)
        );
        assert_eq!(actual.case_outcomes.as_ref().unwrap().len(), 2);
    }
}
