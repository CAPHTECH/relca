//! ADR 0020: independent single-fault AST edits, exhaustive checks only.
use crate::{Verdict, ast, diag, engine, ir, ir::Assumption, lower, report, syntax};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

mod response;
#[cfg(test)]
mod response_boundary_oracle {
    use crate as relca_core;
    include!("../tests/support/response_boundary_oracle.rs");
}

pub const DEFAULT_MUTANT_LIMIT: usize = 128;
pub const MAX_MUTANT_LIMIT: usize = 4096;

/// Validated request scope. Omitted kinds retain the complete default set.
#[derive(Debug, Clone)]
pub struct Options {
    limit: usize,
    selected: [bool; 10],
    sites: Option<BTreeSet<String>>,
    configuration: Option<String>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            limit: DEFAULT_MUTANT_LIMIT,
            selected: [true; 10],
            sites: None,
            configuration: None,
        }
    }
}
impl Options {
    pub fn with_configuration(mut self, configuration: Option<&str>) -> Self {
        self.configuration = configuration.map(str::to_owned);
        self
    }
    pub fn with_sites(mut self, sites: Option<&str>) -> Result<Self, &'static str> {
        if let Some(sites) = sites {
            let mut chosen = BTreeSet::new();
            for id in sites.split(',') {
                if id.is_empty() || id.trim() != id || !chosen.insert(id.to_owned()) {
                    return Err("sites must be a nonempty list of distinct mutant IDs");
                }
            }
            self.sites = Some(chosen);
        }
        Ok(self)
    }
    pub fn parse(limit: Option<&str>, kinds: Option<&str>) -> Result<Self, &'static str> {
        let mut options = Self::default();
        if let Some(value) = limit {
            if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err("mutant-limit must be an integer in 1..=4096");
            }
            options.limit = value
                .parse()
                .map_err(|_| "mutant-limit must be in 1..=4096")?;
            if !(1..=MAX_MUTANT_LIMIT).contains(&options.limit) {
                return Err("mutant-limit must be in 1..=4096");
            }
        }
        if let Some(value) = kinds {
            options.selected = [false; 10];
            for kind in value.split(',') {
                let i = KINDS.iter().position(|k| k.0 == kind).ok_or(
                    "kinds must be a nonempty comma-separated list of mutation kind names",
                )?;
                if options.selected[i] {
                    return Err("duplicate mutation kind");
                }
                options.selected[i] = true;
            }
        }
        Ok(options)
    }
}

#[cfg(test)]
#[path = "../tests/support/mutation_default_population.rs"]
mod extension_population;

#[cfg(test)]
pub(crate) fn extension_fixture(name: &str) -> bool {
    extension_population::fixture(name)
}

#[cfg(test)]
pub(crate) fn review_fixture(name: &str) -> bool {
    extension_population::review_fixture(name)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MutationReport {
    pub ledger: crate::ledger::Ledger,
    pub uncertainty: crate::uncertainty::Uncertainty,
    pub verdict: Verdict,
    pub mode: &'static str,
    pub analysis: &'static str,
    pub set: &'static str,
    pub original: Option<report::Report>,
    pub original_verdict: Option<Verdict>,
    pub run: Run,
    pub mutations: Vec<KindRow>,
    pub assumptions: Vec<Assumption>,
    pub diagnostics: Vec<diag::Diagnostic>,
    pub checked: Vec<String>,
    pub unchecked: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Run {
    pub completion: &'static str,
    pub reason: Option<&'static str>,
    pub mutant_limit: MutantLimit,
    pub limit_reached: bool,
    pub candidates_total: Option<usize>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum MutantLimit {
    Used(usize),
    Unused {
        status: &'static str,
        value: (),
        reason: &'static str,
    },
}
impl MutantLimit {
    fn unused() -> Self {
        Self::Unused {
            status: "not_applicable",
            value: (),
            reason: "no_analysis",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KindRow {
    pub candidates: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped_counts: Option<BTreeMap<String, usize>>,
    pub kind: &'static str,
    pub name: &'static str,
    pub status: &'static str,
    pub reason: Option<&'static str>,
    pub assigned_checks: Vec<&'static str>,
    pub mutants: Vec<MutantRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped_sites: Option<Vec<SkippedSite>>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkippedSite {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cohort: Option<String>,
    pub status: &'static str,
    pub reason: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<Site>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MutantRow {
    pub id: String,
    pub site: Site,
    pub operator: &'static str,
    pub result: &'static str,
    pub reason: Option<&'static str>,
    pub report: Option<report::Report>,
    pub assigned_changes: Vec<Change>,
    pub other_changes: Vec<Change>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Site {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cohort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed_end: Option<String>,
    pub process: Option<String>,
    pub actor: Option<String>,
    pub capability: Option<String>,
    pub source: Option<String>,
    pub affected_bindings: Vec<String>,
    pub statement_path: Option<String>,
    pub retained_branch: Option<&'static str>,
    pub span: diag::Span,
    pub line: usize,
    pub column: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_binding: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    pub check: String,
    pub key: String,
    pub field: String,
    pub before: Payload,
    pub after: Payload,
    pub direction: &'static str,
}
/// Only the closed scalar/string-set payload family is emitted; no opaque JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Payload {
    Null,
    Text(String),
    Bool(bool),
    Integer(u64),
    Strings(Vec<String>),
}

const KINDS: [(&str, &str, Option<&str>); 10] = [
    ("evidence_missing", "Evidence の欠落", None),
    ("authority_expired", "Authority の失効", None),
    ("hold_delayed", "Hold の遅延", None),
    ("command_resent", "同じ Command の再送", None),
    ("summary_reused", "同じ要約の多重利用", None),
    (
        "stale_definition_write",
        "定義更新中の旧版書込み",
        Some("rule_version_unavailable"),
    ),
    ("receipt_unknown", "外部 Receipt の unknown", None),
    (
        "old_promise_denominator_removed",
        "旧約束を分母から消す",
        Some("quality_denominator_unavailable"),
    ),
    ("refusal_path_cut", "拒否経路の切断", None),
    ("original_access_removed", "原本到達の削除", None),
];

fn checks(i: usize) -> Vec<&'static str> {
    match i {
        0 => vec!["agency.lineage", "contract"],
        1 => vec!["authority_basis"],
        2 => vec!["requires_event", "requires", "invariant"],
        3 => vec!["at_most_once"],
        4 => vec!["agency.lineage", "independent_verification"],
        5 => vec!["rule_version"],
        6 => vec!["receipt_unknown"],
        7 => vec!["quality_denominator", "comparison_mismatch"],
        8 => vec!["agency.refusal", "invariant"],
        9 => vec!["responsibilities", "contest_paths"],
        _ => unreachable!(),
    }
}
fn uncomputed(reason: &'static str, limit: usize) -> MutationReport {
    let assumptions = [
        (
            "purpose",
            "weakness_candidates_not_quality_score",
            "Not coverage or a quality score",
        ),
        (
            "detection",
            "assigned_checks_only",
            "Only assigned worsening counts as detection",
        ),
        (
            "original",
            "not_established_correct",
            "The original model is not established correct",
        ),
        (
            "analysis",
            "exhaustive_only",
            "No simulation or compare detection",
        ),
        (
            "limit",
            "computational_128",
            "Computational truncation, not a business bound",
        ),
        (
            "provenance",
            "original_source_plus_ast_edit",
            "Nested hashes identify the original source, together with site and operator",
        ),
        (
            "guard_scope",
            "terminal_guard_bypass",
            "Syntactic terminal guard bypass only",
        ),
        (
            "authority_scope",
            "expires_zero_on_basis_authorities",
            "authority_basis が名指す Authority だけの明示の期限を0minにする。時刻0の事象は期限内。動的失効と名指されない Authority は未検査。",
        ),
        (
            "summary_scope",
            "duplicate_producer_one_consumer",
            "結果を2つ以上の呼出しに渡す作る側を直後に複製し、2番目以降の消費者を1つだけ付け替える。caseの変数と潜在は共有され再び書かれる。summary_reused は既存の悪化検出の例外で、根拠集合の一致とgroup判定数の非減少を検出とする。この種は Relca の系譜の規則を確かめるもので、モデルの強さを示さない。fork枝の作る側、共有副作用、後のprocess guardが読む観測変数の書き手を除く。guardとreadsは付け替えない。other_changesはquality・via_model_paths等の閉じた欄の外を記録しない。",
        ),
    ]
    .into_iter()
    .map(|(field, value, affects)| Assumption {
        field: format!("mutation.{field}"),
        value: value.into(),
        affects: affects.into(),
    })
    .collect();
    MutationReport {
        ledger: crate::ledger::Ledger::aggregate("mutation"),
        uncertainty: crate::uncertainty::Uncertainty::not_lowered(),
        verdict: Verdict::Invalid,
        mode: "mutation",
        analysis: "exhaustive",
        set: "bpr-default",
        original: None,
        original_verdict: None,
        run: Run {
            completion: "blocked",
            reason: Some(reason),
            mutant_limit: MutantLimit::Used(limit),
            limit_reached: false,
            candidates_total: None,
        },
        mutations: KINDS
            .iter()
            .enumerate()
            .map(|(i, &(kind, name, unsupported))| KindRow {
                candidates: None,
                skipped_counts: None,
                kind,
                name,
                status: if unsupported.is_some() {
                    "unsupported"
                } else {
                    "blocked"
                },
                reason: unsupported.or(Some(reason)),
                assigned_checks: checks(i),
                mutants: vec![],
                skipped_sites: None,
            })
            .collect(),
        assumptions,
        diagnostics: vec![],
        checked: vec![],
        unchecked: [
            "original_check",
            "site_enumeration",
            "single_fault_generation",
            "assigned_check_comparison",
            "coverage_quality",
            "real_world_model_correctness",
            "simulation_detection",
            "via_model",
            "authority_expiry",
            "command_resend",
            "rule_version",
            "receipt_unknown",
            "quality_denominator",
            "W-LIN-2",
            "requires",
            "guard_operator_scope",
            "contest_route_handoff",
            "contest_occurrence",
            "contest_response_deadline",
            "responsibility_time_frame",
            "authority_dynamic_expiry",
            "authority_expiry_unnamed",
            "summary_reuse_scope",
            "quality_changes",
        ]
        .map(String::from)
        .to_vec(),
    }
}
pub fn argument_failure(message: &str) -> MutationReport {
    failure("invalid_arguments", message)
}
pub fn input_failure(message: &str) -> MutationReport {
    failure("input_failure", message)
}
pub fn input_failure_with_scenario(message: &str, scenario: Option<&str>) -> MutationReport {
    let mut r = input_failure(message);
    r.ledger.requested(None, scenario);
    seal(r)
}
pub fn input_failure_with_options(
    message: &str,
    scenario: Option<&str>,
    options: &Options,
) -> MutationReport {
    let mut r = input_failure(message);
    r.run.mutant_limit = MutantLimit::Used(options.limit);
    apply_options(&mut r, options);
    r.ledger
        .requested(options.configuration.as_deref(), scenario);
    seal(r)
}

fn apply_options(r: &mut MutationReport, options: &Options) {
    r.assumptions
        .iter_mut()
        .find(|a| a.field == "mutation.limit")
        .unwrap()
        .value = format!("computational_{}", options.limit);
    for (i, row) in r.mutations.iter_mut().enumerate() {
        if !options.selected[i] {
            row.status = "not_selected";
            row.reason = Some("not_selected");
            r.unchecked.push(format!("mutation.kind.{}", row.kind));
        }
    }
}
fn failure(reason: &'static str, message: &str) -> MutationReport {
    let mut r = uncomputed(reason, DEFAULT_MUTANT_LIMIT);
    if reason == "invalid_arguments" {
        r.run.mutant_limit = MutantLimit::unused();
        r.assumptions
            .iter_mut()
            .find(|a| a.field == "mutation.limit")
            .unwrap()
            .value = "not_applicable".into();
    }
    r.diagnostics.push(diag::Diagnostic {
        code: diag::E_SYN_0.into(),
        severity: diag::Severity::Error,
        message: message.into(),
        span: None,
    });
    seal(r)
}
pub fn mutate(src: &str) -> MutationReport {
    mutate_set(src, "bpr-default")
}
pub fn mutate_set(src: &str, set: &str) -> MutationReport {
    mutate_set_with_scenario(src, set, None)
}

pub fn mutate_set_with_scenario(src: &str, set: &str, scenario: Option<&str>) -> MutationReport {
    mutate_with_options(src, set, scenario, &Options::default())
}

pub fn mutate_with_options(
    src: &str,
    set: &str,
    scenario: Option<&str>,
    options: &Options,
) -> MutationReport {
    if set != "bpr-default" {
        return argument_failure("Unknown mutation set; expected bpr-default");
    }
    let configuration = options.configuration.as_deref();
    let original = if let Some(sites) = &options.sites {
        // Census all kinds without edits/checks, then validate identifiers before
        // executing any mutant. Reuse the original check for the selected run.
        let census = mutate_using_raw(
            src,
            &Options {
                limit: 0,
                configuration: options.configuration.clone(),
                ..Options::default()
            },
            scenario,
            |s| report::check_with_selection(s, configuration, scenario),
            |_, _| unreachable!(),
        );
        if census.run.candidates_total.is_none() {
            let mut blocked = census;
            blocked.run.mutant_limit = MutantLimit::Used(options.limit);
            apply_options(&mut blocked, options);
            return seal(blocked);
        }
        let known: BTreeSet<_> = census
            .mutations
            .iter()
            .flat_map(|r| &r.mutants)
            .map(|m| m.id.as_str())
            .collect();
        if sites.iter().any(|s| !known.contains(s.as_str())) {
            return argument_failure("Unknown mutation site ID");
        }
        census.original
    } else {
        None
    };
    seal(mutate_using_raw(
        src,
        options,
        scenario,
        |s| original.unwrap_or_else(|| report::check_with_selection(s, configuration, scenario)),
        |src, ast| {
            report::finite_driver(
                report::check_ast_with_selection_using(
                    src,
                    ast,
                    configuration,
                    scenario,
                    engine::explore,
                ),
                engine::DEFAULT_STATE_BUDGET,
            )
        },
    ))
}

fn stage(r: &mut MutationReport, name: &str) {
    r.unchecked.retain(|x| x != name);
    r.checked.push(name.into());
}

// Any explicit configuration selects its effective process, even a sole configuration.
fn scoped_process(options: &Options) -> bool {
    options.configuration.is_some()
}

fn select_site(m: &mut MutantRow, options: &Options) -> bool {
    if options
        .sites
        .as_ref()
        .is_some_and(|sites| !sites.contains(&m.id))
    {
        m.result = "not_selected";
        m.reason = Some("not_selected");
        false
    } else {
        true
    }
}

fn all_sites_ran(row: &KindRow) -> bool {
    !row.mutants.is_empty()
        && row
            .mutants
            .iter()
            .all(|m| !matches!(m.result, "not_run" | "not_selected"))
}
#[cfg(test)]
fn mutate_using(
    src: &str,
    limit: usize,
    scenario: Option<&str>,
    baseline: impl FnOnce(&str) -> report::Report,
    execute: impl FnMut(&str, &ast::Model) -> report::Report,
) -> MutationReport {
    seal(mutate_using_raw(
        src,
        &Options {
            limit,
            ..Options::default()
        },
        scenario,
        baseline,
        execute,
    ))
}
fn seal(mut r: MutationReport) -> MutationReport {
    r.ledger.execution["limits"]["mutant_limit"] = match r.run.mutant_limit {
        MutantLimit::Used(n) => crate::ledger::known(n),
        MutantLimit::Unused { .. } => crate::ledger::na("no_analysis"),
    };
    let host = serde_json::to_value(&r).expect("mutation uncertainty context");
    r.uncertainty = crate::uncertainty::Uncertainty::aggregate(&host, "mutation");
    let host = serde_json::to_value(&r).expect("mutation wire");
    crate::ledger::complete(&mut r.ledger, &host, &mut r.verdict, &mut r.diagnostics);
    r
}
fn mutate_using_raw(
    src: &str,
    options: &Options,
    scenario: Option<&str>,
    baseline: impl FnOnce(&str) -> report::Report,
    mut execute: impl FnMut(&str, &ast::Model) -> report::Report,
) -> MutationReport {
    let limit = options.limit;
    let configuration = options.configuration.as_deref();
    let original = report::work_profiles::refusal(src, configuration, scenario)
        .map_or_else(|| baseline(src), |(r, _)| r);
    let mut r = uncomputed("original_not_decidable", limit);
    apply_options(&mut r, options);
    if configuration.is_some() {
        r.ledger.requested(configuration, scenario);
    }
    stage(&mut r, "original_check");
    r.verdict = original.verdict;
    r.original_verdict = Some(original.verdict);
    r.original = Some(original.clone());
    if syntax::parse(src).is_ok_and(|ast| {
        let process = if scoped_process(options) {
            original.ledger.execution["selection"]["effective"]["process"]["value"].as_str()
        } else {
            None
        };
        ast.items.iter().any(|i| {
            let ast::Item::Process(p) = i else {
                return false;
            };
            if process.is_some_and(|name| p.name.value != name) {
                return false;
            }
            let mut sites = Vec::new();
            calls(&p.body, &mut sites);
            sites.iter().any(|c| c.recovery.is_some())
        })
    }) {
        r.verdict = Verdict::Unsupported;
        r.diagnostics.push(diag::Diagnostic {
            code: diag::U_SEM_0.into(),
            severity: diag::Severity::Error,
            message: "recovery_feature_combination".into(),
            span: None,
        });
        return r;
    }
    if !matches!(original.verdict, Verdict::Satisfied | Verdict::Violated) {
        return r;
    }
    let ast = syntax::parse(src).expect("a decidable check already parsed this source");
    // Use the same selection as both checks. In particular, a scenario must
    // never fall back to the common model; an unselected configuration stays
    // blocked by the original check, just as it does for mutant execution.
    let context = lower::lower_with_selection(&ast, configuration, scenario)
        .expect("a decidable check already lowered this selection");
    let process = scoped_process(options).then_some(context.process.name.as_str());
    let reads = read_sites(src, &ast, process);
    let guards = guard_sites(src, &ast, process);
    let authorities = authority_sites(src, &ast, process);
    if !context.process.degradations.is_empty() {
        r.verdict = Verdict::Unsupported;
        r.diagnostics.push(diag::Diagnostic {
            code: diag::U_SEM_0.into(),
            severity: diag::Severity::Error,
            message: "q6_feature_combination: degradation mutation not connected".into(),
            span: None,
        });
        return r;
    }
    let summaries = summary_sites(src, &ast, &context);
    let no_expiry_time = !authorities.is_empty() && no_time_elapse(&context, &original);
    stage(&mut r, "site_enumeration");
    let mut attempts = 0;
    // Budget order is deliberately not display order: never steal an old trial.
    for i in [0, 2, 8, 9, 1, 4] {
        let row = &mut r.mutations[i];
        let candidates = match i {
            0 | 9 => &reads,
            1 => &authorities,
            4 => &summaries,
            _ => &guards,
        };
        row.candidates = Some(if i == 1 && no_expiry_time {
            0
        } else {
            candidates.len()
        });
        if row.status == "not_selected" {
            continue;
        }
        if i == 1 && no_expiry_time {
            row.status = "not_applicable";
            row.reason = Some("no_time_elapse");
            continue;
        }
        row.status = if candidates.is_empty() {
            "not_applicable"
        } else {
            "generated"
        };
        row.reason = candidates.is_empty().then_some("no_matching_site");
        for c in candidates {
            let mut m = MutantRow {
                id: format!("{}:{}", row.kind, c.identity),
                site: c.site.clone(),
                operator: c.operator(),
                result: "not_run",
                reason: Some("mutant_limit"),
                report: None,
                assigned_changes: vec![],
                other_changes: vec![],
            };
            if !select_site(&mut m, options) {
                row.mutants.push(m);
                continue;
            }
            if let Some(edited) = budgeted_edit(c, &ast, &mut attempts, limit) {
                let report = execute(src, &edited);
                let comparison = match &c.edit {
                    Edit::Summary {
                        sources, consumer, ..
                    } => compare_summary(
                        &original,
                        &report,
                        sources,
                        &consumer.binding,
                        &consumer.capability,
                    ),
                    _ => compare(i, &original, &report),
                };
                m.result = comparison.result;
                m.reason = comparison.reason;
                m.assigned_changes = comparison.assigned;
                m.other_changes = comparison.other;
                m.report = Some(report);
            } else {
                r.run.limit_reached = true;
            }
            row.mutants.push(m);
        }
    }
    kind7(src, &ast, &mut r, &mut attempts, options, process);
    response::run(
        src,
        &ast,
        &context,
        &original,
        &mut r,
        (&mut attempts, options),
        &mut execute,
    );
    r.run.candidates_total = Some(r.mutations.iter().filter_map(|row| row.candidates).sum());
    for m in r
        .mutations
        .iter()
        .flat_map(|row| &row.mutants)
        .filter(|m| m.result == "not_selected")
    {
        r.unchecked.push(format!("mutation.site.{}", m.id));
    }
    // A run cut by the mutant limit did not generate or compare every site,
    // so those stages stay unchecked.
    if attempts > 0
        && !r.run.limit_reached
        && r.mutations.iter().all(|row| {
            row.status != "not_selected"
                && row
                    .mutants
                    .iter()
                    .all(|m| !matches!(m.result, "not_run" | "not_selected"))
        })
    {
        stage(&mut r, "single_fault_generation");
        stage(&mut r, "assigned_check_comparison");
    }
    if r.mutations[1].status != "not_selected" && all_sites_ran(&r.mutations[1]) {
        stage(&mut r, "authority_expiry");
    }
    if !r.mutations[7].mutants.is_empty() && all_sites_ran(&r.mutations[7]) {
        stage(&mut r, "quality_denominator");
    }
    r.verdict = fold(&r.mutations);
    let incomplete = r.mutations.iter().flat_map(|x| &x.mutants).any(|m| {
        matches!(
            m.result,
            "unsupported" | "inconclusive" | "invalid_mutant" | "not_run"
        )
    });
    r.run.completion = if incomplete {
        "inconclusive"
    } else {
        "complete"
    };
    r.run.reason = if r.run.limit_reached {
        Some("mutant_limit")
    } else {
        incomplete.then_some("mutant_incomplete")
    };
    // A declared scope containing no executed mutant is not a successful test.
    // Preserve actual computational cuts (including the private zero-budget census).
    if attempts == 0 && !r.run.limit_reached && r.verdict == Verdict::Satisfied {
        r.verdict = Verdict::Vacuous;
        r.run.reason = Some("no_selected_mutant");
    }
    r
}
fn budgeted_edit(
    c: &Candidate,
    ast: &ast::Model,
    attempts: &mut usize,
    limit: usize,
) -> Option<ast::Model> {
    if *attempts >= limit {
        return None;
    }
    *attempts += 1;
    Some(c.apply(ast))
}

/// Conservative applicability proof: zero declared service times alone are
/// insufficient (late arrivals, waiting and backoff can advance absolute time).
fn no_time_elapse(context: &ir::Model, original: &report::Report) -> bool {
    if original.bounds.truncated != Some(false)
        // Start-slot calendar boundaries can advance time independently of
        // service durations, including on paths missing from terminal extrema.
        || context.resource_time.is_some()
        || has_time_wait(&context.process.body)
        || context
            .response_attempts()
            .iter()
            .any(|(_, attempt)| attempt.backoff != 0)
        || context.capabilities.iter().any(|c| {
            c.duration != 0
                || c.duration_distribution
                    .as_ref()
                    .is_some_and(|d| d.entries.iter().any(|(tick, _)| *tick != 0))
        })
    {
        return false;
    }
    if let Some(cases) = &original.case_outcomes {
        return !cases.is_empty()
            && cases.iter().all(|c| {
                c.arrival == 0
                    && !c.terminals.is_empty()
                    && c.terminals
                        .iter()
                        .all(|t| t.completed_at.is_some_and(|time| time.max == 0))
            });
    }
    !original.terminals.is_empty() && original.terminals.iter().all(|t| t.elapsed.max == 0)
}

fn has_time_wait(block: &ir::Block) -> bool {
    block.iter().any(|s| match &s.kind {
        ir::Stmt::Wait(tick) => *tick > 0,
        ir::Stmt::WaitUntil(_) => true,
        ir::Stmt::If {
            then_body,
            else_body,
            ..
        } => has_time_wait(then_body) || has_time_wait(else_body),
        ir::Stmt::Response(_, attempt) => attempt.recovery.as_ref().is_some_and(has_time_wait),
        // Fork holds Calls, not nested Blocks; duration is checked above.
        ir::Stmt::Query(_, _)
        | ir::Stmt::ResolveEffect { .. }
        | ir::Stmt::StopReceiptUnknown
        | ir::Stmt::Call(_)
        | ir::Stmt::Fork { .. }
        | ir::Stmt::End(_) => false,
    })
}

/// A structural definition record, deliberately not executable IR. The only
/// permitted edit removes one terminal; no mutant ratio is ever computed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct DenominatorDefinition {
    cohort_name: String,
    numerator_definition: serde_json::Value,
    denominator_kind: String,
    denominator_property: Option<String>,
    within: Option<u64>,
    terminals: Vec<String>,
}
fn compare_definition(before: &DenominatorDefinition, after: &DenominatorDefinition) -> Comparison {
    let mut fixed = after.clone();
    fixed.terminals = before.terminals.clone();
    if &fixed != before {
        return Comparison {
            result: "invalid_mutant",
            reason: Some("static_invalid"),
            assigned: vec![],
            other: vec![],
        };
    }
    if before == after {
        return Comparison {
            result: "undetected",
            reason: Some("no_worsening"),
            assigned: vec![],
            other: vec![],
        };
    }
    let valid = before.terminals.len() == after.terminals.len() + 1
        && before
            .terminals
            .iter()
            .filter(|e| !after.terminals.contains(e))
            .count()
            == 1
        && before
            .terminals
            .iter()
            .filter(|e| after.terminals.contains(e))
            .eq(after.terminals.iter());
    if !valid {
        return Comparison {
            result: "invalid_mutant",
            reason: Some("static_invalid"),
            assigned: vec![],
            other: vec![],
        };
    }
    Comparison {
        result: "detected",
        reason: None,
        assigned: ["quality_denominator", "comparison_mismatch"]
            .into_iter()
            .map(|check| Change {
                check: check.into(),
                key: format!("cohort.{}", before.cohort_name),
                field: "denominator.terminals".into(),
                before: Payload::Strings(before.terminals.clone()),
                after: Payload::Strings(after.terminals.clone()),
                direction: "changed",
            })
            .collect(),
        other: vec![],
    }
}
fn lexical_ends(block: &ast::Block, out: &mut Vec<String>) {
    fn add(out: &mut Vec<String>, name: &str) {
        if !out.iter().any(|s| s == name) {
            out.push(name.into())
        }
    }
    for s in block {
        match &s.value {
            ast::Stmt::End(e) => add(out, &e.value),
            ast::Stmt::Fork { join, .. } => {
                if let ast::Join::Any { if_none, .. } = &join.value {
                    add(out, &if_none.value)
                }
            }
            ast::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                lexical_ends(then_body, out);
                lexical_ends(else_body, out);
            }
            _ => (),
        }
    }
}
fn kind7(
    src: &str,
    ast: &ast::Model,
    r: &mut MutationReport,
    attempts: &mut usize,
    options: &Options,
    process_scope: Option<&str>,
) {
    let limit = options.limit;
    let metrics: Vec<_> = ast
        .items
        .iter()
        .filter_map(|i| {
            if let ast::Item::CohortMetric(c) = i {
                Some(c)
            } else {
                None
            }
        })
        .collect();
    if metrics.is_empty() {
        return;
    }
    let properties: Vec<_> = ast
        .items
        .iter()
        .filter_map(|i| {
            if let ast::Item::QualityProperty(p) = i {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    let process = ast
        .items
        .iter()
        .find_map(|i| {
            if let ast::Item::Process(p) = i
                && process_scope.is_none_or(|name| p.name.value == name)
            {
                Some(p)
            } else {
                None
            }
        })
        .expect("decidable process");
    let mut process_ends = Vec::new();
    lexical_ends(&process.body, &mut process_ends);
    let row = &mut r.mutations[7];
    row.candidates = Some(0);
    let selected = row.status != "not_selected";
    // Enumerate even an excluded kind, without executing or claiming its checks.
    let mut skipped_sites = Vec::new();
    for c in metrics {
        let denominator = c.denominator.as_ref().map(|n| {
            *properties
                .iter()
                .find(|p| p.name.value == n.value)
                .expect("resolved property")
        });
        let terminals = match denominator {
            None => process_ends.clone(),
            Some(p) => match &p.target {
                ast::QualityTarget::State(_) => {
                    skipped_sites.push(SkippedSite {
                        site: None,
                        cohort: Some(c.name.value.clone()),
                        status: "not_applicable",
                        reason: "state_condition",
                    });
                    continue;
                }
                ast::QualityTarget::Ends(es) => {
                    let mut out = Vec::new();
                    for e in es {
                        if !out.contains(&e.value) {
                            out.push(e.value.clone())
                        }
                    }
                    out
                }
            },
        };
        if terminals.len() < 2 {
            skipped_sites.push(SkippedSite {
                site: None,
                cohort: Some(c.name.value.clone()),
                status: "not_applicable",
                reason: "no_matching_site",
            });
            continue;
        }
        let eligible = terminals
            .iter()
            .filter(|end| process_scope.is_none() || process_ends.contains(end))
            .count();
        if eligible == 0 {
            skipped_sites.push(SkippedSite {
                site: None,
                cohort: Some(c.name.value.clone()),
                status: "not_applicable",
                reason: "no_matching_site",
            });
            continue;
        }
        *row.candidates.as_mut().unwrap() += eligible;
        if !selected {
            continue;
        }
        let numerator = r
            .original
            .as_ref()
            .unwrap()
            .quality
            .as_ref()
            .unwrap()
            .properties
            .iter()
            .find(|p| p["name"] == c.numerator.value)
            .unwrap();
        let before = DenominatorDefinition {
            cohort_name: c.name.value.clone(),
            numerator_definition: serde_json::json!({"condition":numerator["condition"],"within":numerator["within"]}),
            denominator_kind: if denominator.is_some() {
                "property"
            } else {
                "arrivals"
            }
            .into(),
            denominator_property: denominator.map(|p| p.name.value.clone()),
            within: denominator.and_then(|p| p.within.as_ref()).map(|d| {
                d.amount.value
                    * match d.unit.value.as_str() {
                        "h" => 60,
                        "d" => 1440,
                        _ => 1,
                    }
            }),
            terminals,
        };
        for removed in &before.terminals {
            if process_scope.is_some() && !process_ends.contains(removed) {
                continue;
            }
            let mut s = site(src, c.denominator_span);
            s.cohort = Some(c.name.value.clone());
            s.removed_end = Some(removed.clone());
            if denominator.is_none() {
                s.process = Some(process.name.value.clone());
            }
            let mut m = MutantRow {
                id: format!(
                    "old_promise_denominator_removed:cohort.{}:remove:{removed}",
                    c.name.value
                ),
                site: s,
                operator: "remove_denominator_end",
                result: "not_run",
                reason: Some("mutant_limit"),
                report: None,
                assigned_changes: vec![],
                other_changes: vec![],
            };
            if !select_site(&mut m, options) {
                row.mutants.push(m);
                continue;
            }
            if *attempts < limit {
                *attempts += 1;
                let mut after = before.clone();
                after.terminals.retain(|e| e != removed);
                let comparison = compare_definition(&before, &after);
                m.result = comparison.result;
                m.reason = comparison.reason;
                m.assigned_changes = comparison.assigned;
                m.other_changes = comparison.other;
            } else {
                r.run.limit_reached = true;
            }
            row.mutants.push(m);
        }
    }
    if !selected {
        return;
    }
    row.skipped_sites = Some(skipped_sites);
    row.status = if row.mutants.is_empty() {
        "not_applicable"
    } else {
        "generated"
    };
    row.reason = if row.mutants.is_empty() {
        Some(
            if row
                .skipped_sites
                .as_ref()
                .unwrap()
                .iter()
                .all(|s| s.reason == "state_condition")
            {
                "state_condition"
            } else {
                "no_matching_site"
            },
        )
    } else {
        None
    };
    if !row.mutants.is_empty() {
        r.unchecked.push("numerator_subset_of_denominator".into());
        r.assumptions.push(Assumption {field:"mutation.kind7".into(),value:"structural-definition-comparison".into(),affects:"Kind 7 compares denominator definitions structurally; it does not re-lower, execute either engine, or run compare. Mutant ratios are neither computed nor reported; numerator inclusion is unchecked.".into()});
    }
}
fn fold(rows: &[KindRow]) -> Verdict {
    if rows
        .iter()
        .any(|r| r.status == "unsupported" || r.mutants.iter().any(|m| m.result == "unsupported"))
    {
        return Verdict::Unsupported;
    }
    let results: Vec<_> = rows
        .iter()
        .flat_map(|r| r.mutants.iter().map(|m| m.result))
        .collect();
    if results
        .iter()
        .any(|r| matches!(*r, "invalid_mutant" | "inconclusive" | "not_run"))
    {
        Verdict::Inconclusive
    } else if results.contains(&"undetected") {
        Verdict::Violated
    } else {
        Verdict::Satisfied
    }
}

#[derive(Debug, Clone)]
enum Edit {
    Authority {
        item: usize,
        existing: Option<usize>,
    },
    Summary {
        process: usize,
        path: Vec<(usize, bool)>,
        index: usize,
        consumer: ConsumerPath,
        binding: String,
        sources: BTreeSet<String>,
    },
    Read {
        actor: usize,
        cap: usize,
        item: usize,
        source: usize,
    },
    Guard {
        process: usize,
        path: Vec<(usize, bool)>,
        index: usize,
        keep_then: bool,
    },
}
#[derive(Debug, Clone)]
struct ConsumerPath {
    path: Vec<(usize, bool)>,
    index: usize,
    branch: Option<usize>,
    binding: String,
    capability: String,
}
#[derive(Debug, Clone)]
struct Candidate {
    identity: String,
    site: Site,
    edit: Edit,
}
impl Candidate {
    fn operator(&self) -> &'static str {
        match self.edit {
            Edit::Authority { .. } => "expire_authority",
            Edit::Summary { .. } => "duplicate_summary",
            Edit::Read { .. } => "remove_read",
            Edit::Guard { .. } => "bypass_guard",
        }
    }
    fn apply(&self, original: &ast::Model) -> ast::Model {
        let mut model = original.clone();
        match &self.edit {
            Edit::Authority { item, existing } => {
                let ast::Item::Authority(a) = &mut model.items[*item] else {
                    unreachable!()
                };
                let span = existing.map_or(a.span, |i| a.items[i].span);
                let expiry = ast::Spanned {
                    value: ast::AuthorityItem::Expires(ast::Duration {
                        amount: ast::Spanned { value: 0, span },
                        unit: ast::Spanned {
                            value: "min".into(),
                            span,
                        },
                        span,
                    }),
                    span,
                };
                if let Some(i) = existing {
                    a.items[*i] = expiry;
                } else {
                    a.items.push(expiry);
                }
            }
            Edit::Summary {
                process,
                path,
                index,
                consumer,
                binding,
                ..
            } => {
                let ast::Item::Process(p) = &mut model.items[*process] else {
                    unreachable!()
                };
                // Rewrite the original consumer before insertion shifts any indices.
                let stmt = &mut block_at(&mut p.body, &consumer.path)[consumer.index].value;
                let call = match stmt {
                    ast::Stmt::Call(c) => c,
                    ast::Stmt::Fork { branches, .. } => &mut branches[consumer.branch.unwrap()],
                    _ => unreachable!(),
                };
                let old = &self.site.affected_bindings[0];
                for arg in &mut call.args {
                    if arg.value == *old {
                        arg.value.clone_from(binding);
                    }
                }
                let block = block_at(&mut p.body, path);
                let mut duplicate = block[*index].clone();
                let ast::Stmt::Call(c) = &mut duplicate.value else {
                    unreachable!()
                };
                c.binding.value.clone_from(binding);
                block.insert(index + 1, duplicate);
            }
            Edit::Read {
                actor,
                cap,
                item,
                source,
            } => {
                let ast::Item::Actor(a) = &mut model.items[*actor] else {
                    unreachable!()
                };
                let ast::ActorItem::Capability(c) = &mut a.items[*cap] else {
                    unreachable!()
                };
                let ast::CapItem::Reads(reads) = &mut c.items[*item].value else {
                    unreachable!()
                };
                reads.remove(*source);
                if reads.is_empty() {
                    c.items.remove(*item);
                }
            }
            Edit::Guard {
                process,
                path,
                index,
                keep_then,
            } => {
                let ast::Item::Process(p) = &mut model.items[*process] else {
                    unreachable!()
                };
                let mut block = &mut p.body;
                for (i, then) in path {
                    let ast::Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } = &mut block[*i].value
                    else {
                        unreachable!()
                    };
                    block = if *then { then_body } else { else_body };
                }
                let ast::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } = &block[*index].value
                else {
                    unreachable!()
                };
                let kept = if *keep_then { then_body } else { else_body }.clone();
                block.splice(*index..=*index, kept);
            }
        }
        model
    }
}
fn site(src: &str, span: diag::Span) -> Site {
    let prefix = &src[..span.start];
    Site {
        cohort: None,
        removed_end: None,
        process: None,
        actor: None,
        capability: None,
        source: None,
        affected_bindings: vec![],
        statement_path: None,
        retained_branch: None,
        span,
        line: prefix.bytes().filter(|b| *b == b'\n').count() + 1,
        column: prefix.rsplit('\n').next().unwrap().chars().count() + 1,
        authority: None,
        duplicate_binding: None,
    }
}
fn calls<'a>(block: &'a ast::Block, out: &mut Vec<&'a ast::Call>) {
    for stmt in block {
        match &stmt.value {
            ast::Stmt::Call(c) => {
                out.push(c);
                if let Some(body) = &c.recovery {
                    calls(body, out);
                }
            }
            ast::Stmt::Fork { branches, .. } => out.extend(branches),
            ast::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                calls(then_body, out);
                calls(else_body, out);
            }
            ast::Stmt::End(_)
            | ast::Stmt::Wait(_)
            | ast::Stmt::WaitUntil(_)
            | ast::Stmt::ResolveEffect { .. }
            | ast::Stmt::StopReceiptUnknown => {}
        }
    }
}
fn read_sites(src: &str, model: &ast::Model, process: Option<&str>) -> Vec<Candidate> {
    let mut all_calls = vec![];
    for item in &model.items {
        if let ast::Item::Process(p) = item
            && process.is_none_or(|name| p.name.value == name)
        {
            calls(&p.body, &mut all_calls);
        }
    }
    let mut out = vec![];
    for (ai, item) in model.items.iter().enumerate() {
        if let ast::Item::Actor(a) = item {
            for (ci, item) in a.items.iter().enumerate() {
                if let ast::ActorItem::Capability(c) = item {
                    if process.is_some()
                        && !all_calls.iter().any(|call| {
                            call.actor.value == a.name.value
                                && call.capability.value == c.name.value
                        })
                    {
                        continue;
                    }
                    for (ri, item) in c.items.iter().enumerate() {
                        if let ast::CapItem::Reads(reads) = &item.value {
                            for (si, source) in reads.iter().enumerate() {
                                let mut s = site(src, source.span);
                                s.actor = Some(a.name.value.clone());
                                s.capability = Some(c.name.value.clone());
                                s.source = Some(source.value.clone());
                                s.affected_bindings = all_calls
                                    .iter()
                                    .filter(|call| {
                                        call.actor.value == a.name.value
                                            && call.capability.value == c.name.value
                                    })
                                    .map(|call| call.binding.value.clone())
                                    .collect();
                                out.push(Candidate {
                                    identity: format!(
                                        "{}.{}:reads:{}",
                                        a.name.value, c.name.value, source.value
                                    ),
                                    site: s,
                                    edit: Edit::Read {
                                        actor: ai,
                                        cap: ci,
                                        item: ri,
                                        source: si,
                                    },
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    out
}
fn block_at<'a>(mut block: &'a mut ast::Block, path: &[(usize, bool)]) -> &'a mut ast::Block {
    for (i, then) in path {
        let ast::Stmt::If {
            then_body,
            else_body,
            ..
        } = &mut block[*i].value
        else {
            unreachable!()
        };
        block = if *then { then_body } else { else_body };
    }
    block
}

fn authority_sites(src: &str, model: &ast::Model, process: Option<&str>) -> Vec<Candidate> {
    let mut named: BTreeMap<&str, BTreeSet<(&str, &str)>> = BTreeMap::new();
    let mut all_calls = vec![];
    for item in &model.items {
        match item {
            ast::Item::Contract(c) => {
                for clause in &c.clauses {
                    if let ast::ContractClause::AuthorityBasis { target, authority } = &clause.value
                    {
                        named
                            .entry(&authority.value)
                            .or_default()
                            .insert((&target.actor.value, &target.capability.value));
                    }
                }
            }
            ast::Item::Process(p) if process.is_none_or(|name| p.name.value == name) => {
                calls(&p.body, &mut all_calls)
            }
            _ => {}
        }
    }
    let mut out = vec![];
    for (item, declaration) in model.items.iter().enumerate() {
        let ast::Item::Authority(a) = declaration else {
            continue;
        };
        let Some(targets) = named.get(a.name.value.as_str()) else {
            continue;
        };
        if process.is_some()
            && !all_calls
                .iter()
                .any(|c| targets.contains(&(c.actor.value.as_str(), c.capability.value.as_str())))
        {
            continue;
        }
        let existing = a
            .items
            .iter()
            .position(|i| matches!(i.value, ast::AuthorityItem::Expires(_)));
        if existing.is_some_and(
            |i| matches!(&a.items[i].value, ast::AuthorityItem::Expires(d) if d.amount.value == 0),
        ) {
            continue;
        }
        let mut s = site(src, existing.map_or(a.span, |i| a.items[i].span));
        s.authority = Some(a.name.value.clone());
        for c in &all_calls {
            if targets.contains(&(c.actor.value.as_str(), c.capability.value.as_str()))
                && !s.affected_bindings.contains(&c.binding.value)
            {
                s.affected_bindings.push(c.binding.value.clone());
            }
        }
        out.push(Candidate {
            identity: format!("{}:expires", a.name.value),
            site: s,
            edit: Edit::Authority { item, existing },
        });
    }
    out
}

fn call_paths(block: &ast::Block, path: &mut Vec<(usize, bool)>, out: &mut Vec<ConsumerPath>) {
    for (index, s) in block.iter().enumerate() {
        let mut add = |c: &ast::Call, branch| {
            out.push(ConsumerPath {
                path: path.clone(),
                index,
                branch,
                binding: c.binding.value.clone(),
                capability: format!("{}.{}", c.actor.value, c.capability.value),
            })
        };
        match &s.value {
            ast::Stmt::Call(c) => add(c, None),
            ast::Stmt::Fork { branches, .. } => {
                for (i, c) in branches.iter().enumerate() {
                    add(c, Some(i));
                }
            }
            ast::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                path.push((index, true));
                call_paths(then_body, path, out);
                path.pop();
                path.push((index, false));
                call_paths(else_body, path, out);
                path.pop();
            }
            _ => {}
        }
    }
}

fn ast_call<'a>(block: &'a ast::Block, p: &ConsumerPath) -> &'a ast::Call {
    let mut block = block;
    for (i, then) in &p.path {
        let ast::Stmt::If {
            then_body,
            else_body,
            ..
        } = &block[*i].value
        else {
            unreachable!()
        };
        block = if *then { then_body } else { else_body };
    }
    match &block[p.index].value {
        ast::Stmt::Call(c) => c,
        ast::Stmt::Fork { branches, .. } => &branches[p.branch.unwrap()],
        _ => unreachable!(),
    }
}

fn written_variables(cap: &ir::Capability) -> BTreeSet<ir::StateVarId> {
    cap.start_writes
        .iter()
        .chain(cap.finish_writes())
        .map(|w| w.variable)
        .chain(cap.kernel_writes.iter().copied())
        .chain(
            cap.kernel
                .iter()
                .flat_map(|k| &k.rows)
                .flat_map(|r| &r.branches)
                .flat_map(|b| &b.overrides)
                .map(|w| w.0),
        )
        .collect()
}

// Actor state of either visibility is shared by the copy: a second write
// would add a change besides the reused summary (fail closed).
fn summary_shared_effect(cap: &ir::Capability, context: &ir::Model) -> bool {
    cap.commits
        || cap.history_dependent_result()
        || written_variables(cap).iter().any(|v| {
            matches!(
                context.state_variables[v.0].kind,
                ir::StateKind::ActorObserved(_) | ir::StateKind::ActorLatent(_)
            )
        })
}

fn condition_variables(c: &ir::Cond, out: &mut BTreeSet<ir::StateVarId>) {
    match c {
        ir::Cond::Contains { set, .. } => {
            out.insert(*set);
        }
        ir::Cond::IntCompare { variable, .. } => {
            out.insert(*variable);
        }
        ir::Cond::StateEq(s) => {
            out.insert(s.variable);
        }
        ir::Cond::And(cs) | ir::Cond::Or(cs) => {
            for c in cs {
                condition_variables(c, out);
            }
        }
        ir::Cond::Eq(_) => {}
    }
}

fn later_guard_variables(block: &ir::Block, after: usize, out: &mut BTreeSet<ir::StateVarId>) {
    for s in block {
        if let ir::Stmt::If {
            condition,
            then_body,
            else_body,
        } = &s.kind
        {
            if s.span.start > after {
                condition_variables(condition, out);
            }
            later_guard_variables(then_body, after, out);
            later_guard_variables(else_body, after, out);
        }
    }
}

fn reserved_names(model: &ast::Model) -> BTreeSet<String> {
    fn block_names(block: &ast::Block, out: &mut BTreeSet<String>) {
        let mut cs = vec![];
        calls(block, &mut cs);
        out.extend(cs.iter().map(|c| c.binding.value.clone()));
        for s in block {
            match &s.value {
                ast::Stmt::Fork { name, join, .. } => {
                    out.insert(name.value.clone());
                    if let ast::Join::Any { result, .. } = &join.value {
                        out.insert(result.value.clone());
                    }
                }
                ast::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    block_names(then_body, out);
                    block_names(else_body, out);
                }
                _ => {}
            }
        }
    }
    let mut out = BTreeSet::new();
    for item in &model.items {
        match item {
            ast::Item::Actor(a) => {
                out.insert(a.name.value.clone());
                for i in &a.items {
                    if let ast::ActorItem::State(v)
                    | ast::ActorItem::Latent(v)
                    | ast::ActorItem::Decides(v) = i
                    {
                        out.extend(v.names.iter().map(|n| n.value.clone()));
                    }
                }
            }
            ast::Item::Type(t) => {
                out.insert(t.name.value.clone());
            }
            ast::Item::Authority(a) => {
                out.insert(a.name.value.clone());
            }
            ast::Item::People(p) => out.extend(p.value.iter().map(|n| n.value.clone())),
            ast::Item::ObservedCase(vs) | ast::Item::LatentCase(vs) => {
                out.extend(vs.iter().flat_map(|v| &v.names).map(|n| n.value.clone()))
            }
            ast::Item::Process(p) => block_names(&p.body, &mut out),
            _ => {}
        }
    }
    out
}

fn summary_sites(src: &str, model: &ast::Model, context: &ir::Model) -> Vec<Candidate> {
    let names = reserved_names(model);
    let mut out = vec![];
    for (process, item) in model.items.iter().enumerate() {
        let ast::Item::Process(p) = item else {
            continue;
        };
        if p.name.value != context.process.name {
            continue;
        }
        let mut paths = vec![];
        call_paths(&p.body, &mut vec![], &mut paths);
        // Match agency::read_graph::read_parents: every lexical preceding
        // writer contributes to an observed read, not just the last writer.
        // Retain each writer's source snapshot, including argument/read ancestry;
        // latent kernel inputs are not evidence sources.
        let mut sources: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut written_sources: BTreeMap<ir::StateVarId, BTreeSet<String>> = BTreeMap::new();
        for path in &paths {
            let c = ast_call(&p.body, path);
            let cap = context
                .capabilities
                .iter()
                .find(|cap| {
                    context.actors[cap.actor.0].name == c.actor.value
                        && cap.name == c.capability.value
                })
                .expect("resolved capability");
            let mut set: BTreeSet<_> = cap
                .reads
                .iter()
                .map(|s| context.actors[s.0].name.clone())
                .collect();
            for arg in &c.args {
                if let Some(s) = sources.get(&arg.value) {
                    set.extend(s.iter().cloned());
                }
            }
            for variable in &cap.read_variables {
                if let Some(s) = written_sources.get(variable) {
                    set.extend(s.iter().cloned());
                }
            }
            for variable in written_variables(cap) {
                written_sources
                    .entry(variable)
                    .or_default()
                    .extend(set.iter().cloned());
            }
            sources.insert(c.binding.value.clone(), set);
        }
        for (pos, path) in paths.iter().enumerate() {
            if path.branch.is_some() {
                continue;
            }
            let c = ast_call(&p.body, path);
            let cap = context
                .capabilities
                .iter()
                .find(|cap| {
                    context.actors[cap.actor.0].name == c.actor.value
                        && cap.name == c.capability.value
                })
                .expect("resolved capability");
            if summary_shared_effect(cap, context) {
                continue;
            }
            let mut guarded = BTreeSet::new();
            later_guard_variables(&context.process.body, c.span.start, &mut guarded);
            guarded.retain(|v| {
                matches!(
                    context.state_variables[v.0].kind,
                    ir::StateKind::ObservedCase
                )
            });
            if !written_variables(cap).is_disjoint(&guarded) {
                continue;
            }
            let consumers: Vec<_> = paths[pos + 1..]
                .iter()
                .filter(|p2| {
                    ast_call(&p.body, p2)
                        .args
                        .iter()
                        .any(|a| a.value == c.binding.value)
                })
                .collect();
            let fresh = (1..)
                .map(|i| {
                    if i == 1 {
                        format!("{}_dup", c.binding.value)
                    } else {
                        format!("{}_dup{i}", c.binding.value)
                    }
                })
                .find(|n| !names.contains(n))
                .unwrap();
            let mut label = "body".to_string();
            for (i, then) in &path.path {
                label.push_str(&format!("/{i}/{}", if *then { "then" } else { "else" }));
            }
            label.push_str(&format!("/{}", path.index));
            for consumer in consumers.into_iter().skip(1) {
                let mut s = site(src, c.span);
                s.process = Some(p.name.value.clone());
                s.actor = Some(c.actor.value.clone());
                s.capability = Some(c.capability.value.clone());
                s.affected_bindings = vec![c.binding.value.clone(), consumer.binding.clone()];
                s.statement_path = Some(label.clone());
                s.duplicate_binding = Some(fresh.clone());
                out.push(Candidate {
                    identity: format!(
                        "{}:{label}:{}->{}",
                        p.name.value, c.binding.value, consumer.binding
                    ),
                    site: s,
                    edit: Edit::Summary {
                        process,
                        path: path.path.clone(),
                        index: path.index,
                        consumer: consumer.clone(),
                        binding: fresh.clone(),
                        sources: sources[&c.binding.value].clone(),
                    },
                });
            }
        }
    }
    out
}

fn has_end(block: &ast::Block) -> bool {
    block.iter().any(|s| match &s.value {
        ast::Stmt::End(_) => true,
        ast::Stmt::If {
            then_body,
            else_body,
            ..
        } => has_end(then_body) || has_end(else_body),
        _ => false,
    })
}
fn has_call(block: &ast::Block) -> bool {
    block.iter().any(|s| match &s.value {
        ast::Stmt::Call(_) | ast::Stmt::Fork { .. } => true,
        ast::Stmt::If {
            then_body,
            else_body,
            ..
        } => has_call(then_body) || has_call(else_body),
        _ => false,
    })
}
fn bindings(c: &ast::Cond, out: &mut Vec<String>) {
    match c {
        ast::Cond::Eq { binding, .. } => {
            if !out.contains(&binding.value) {
                out.push(binding.value.clone());
            }
        }
        ast::Cond::And(cs) | ast::Cond::Or(cs) => {
            for c in cs {
                bindings(&c.value, out);
            }
        }
        ast::Cond::Group(c) => bindings(&c.value, out),
        ast::Cond::Contains { .. } | ast::Cond::LocalEq { .. } | ast::Cond::Compare { .. } => {}
    }
}
fn guard_sites(src: &str, model: &ast::Model, process: Option<&str>) -> Vec<Candidate> {
    struct Walker<'a> {
        src: &'a str,
        process: usize,
        name: &'a str,
        call_bindings: Vec<String>,
        out: Vec<Candidate>,
    }
    impl Walker<'_> {
        fn visit(
            &mut self,
            block: &ast::Block,
            path: &mut Vec<(usize, bool)>,
            prefix: &str,
            parent_continuation: bool,
        ) {
            for (i, stmt) in block.iter().enumerate() {
                if let ast::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } = &stmt.value
                {
                    let continuation = parent_continuation || has_call(&block[i + 1..].to_vec());
                    let label = format!("{prefix}/{i}");
                    for (keep_then, kept, discarded) in
                        [(true, then_body, else_body), (false, else_body, then_body)]
                    {
                        if has_end(discarded) && (has_call(kept) || continuation) {
                            let branch = if keep_then { "then" } else { "else" };
                            let mut s = site(self.src, stmt.span);
                            s.process = Some(self.name.into());
                            s.statement_path = Some(label.clone());
                            s.retained_branch = Some(branch);
                            bindings(&condition.value, &mut s.affected_bindings);
                            s.affected_bindings
                                .retain(|b| self.call_bindings.contains(b));
                            self.out.push(Candidate {
                                identity: format!("{}:{label}:keep_{branch}", self.name),
                                site: s,
                                edit: Edit::Guard {
                                    process: self.process,
                                    path: path.clone(),
                                    index: i,
                                    keep_then,
                                },
                            });
                        }
                    }
                    path.push((i, true));
                    self.visit(then_body, path, &format!("{label}/then"), continuation);
                    path.pop();
                    path.push((i, false));
                    self.visit(else_body, path, &format!("{label}/else"), continuation);
                    path.pop();
                }
            }
        }
    }
    let mut out = vec![];
    for (i, item) in model.items.iter().enumerate() {
        if let ast::Item::Process(p) = item
            && process.is_none_or(|name| p.name.value == name)
        {
            let mut process_calls = vec![];
            calls(&p.body, &mut process_calls);
            let mut w = Walker {
                src,
                process: i,
                name: &p.name.value,
                call_bindings: process_calls
                    .iter()
                    .map(|c| c.binding.value.clone())
                    .collect(),
                out: vec![],
            };
            w.visit(&p.body, &mut vec![], "body", false);
            out.extend(w.out);
        }
    }
    out
}

struct Comparison {
    result: &'static str,
    reason: Option<&'static str>,
    assigned: Vec<Change>,
    other: Vec<Change>,
}
fn normalized(sources: &[String]) -> BTreeSet<String> {
    sources.iter().cloned().collect()
}

/// Conservation is assigned to this consumer only, not to equal-sized sets or
/// Actor labels. Unlike the older faults, a held result catches double credit.
fn compare_summary(
    before: &report::Report,
    after: &report::Report,
    sources: &BTreeSet<String>,
    call: &str,
    capability: &str,
) -> Comparison {
    if let Some(c) = comparison_failure(before, after, true) {
        return c;
    }
    let mut c = Comparison {
        result: "undetected",
        reason: Some("no_assigned_check"),
        assigned: vec![],
        other: vec![],
    };
    other_changes(before, after, &mut c.other);
    let b = before
        .agency
        .as_ref()
        .and_then(crate::agency::AgencyDerivation::as_derived)
        .and_then(|a| {
            a.lineage
                .iter()
                .find(|l| l.call == call && l.capability == capability)
        });
    let Some(b) = b else { return c };
    let subject = normalized(&b.subject_sources);
    let independent = normalized(&b.independent_sources);
    let union = subject.union(&independent).cloned().collect();
    if subject.is_empty() || sources.is_empty() || !sources.is_subset(&union) {
        return c;
    }
    c.result = "inconclusive";
    c.reason = Some("assigned_check_unavailable");
    let group = |r: &report::Report, subject: &BTreeSet<String>| {
        r.agency
            .as_ref()
            .and_then(crate::agency::AgencyDerivation::as_derived)
            .and_then(|a| {
                a.lineage_groups
                    .iter()
                    .find(|g| normalized(&g.subject_sources) == *subject)
            })
            .map(|g| g.judgements)
    };
    let Some(bg) = group(before, &subject) else {
        return c;
    };
    let a = after
        .agency
        .as_ref()
        .and_then(crate::agency::AgencyDerivation::as_derived)
        .and_then(|a| {
            a.lineage
                .iter()
                .find(|l| l.call == call && l.capability == capability)
        });
    let Some(a) = a else { return c };
    let new_subject = normalized(&a.subject_sources);
    let new_independent = normalized(&a.independent_sources);
    let Some(ag) = group(after, &new_subject) else {
        return c;
    };
    let old_group_after = group(after, &subject).unwrap_or(0);
    let credited = !new_independent.is_subset(&independent) || old_group_after < bg;
    let held = subject == new_subject && independent == new_independent && ag >= bg;
    let key = format!("{call}:{capability}");
    let direction = if held { "held" } else { "changed" };
    for (field, b, a, d) in [
        ("subject_sources", &subject, &new_subject, direction),
        (
            "independent_sources",
            &independent,
            &new_independent,
            if !new_independent.is_subset(&independent) {
                "improved"
            } else {
                direction
            },
        ),
    ] {
        let before = Payload::Strings(b.iter().cloned().collect());
        let after = Payload::Strings(a.iter().cloned().collect());
        if held || before != after {
            c.assigned.push(Change {
                check: "agency.lineage".into(),
                key: key.clone(),
                field: field.into(),
                before,
                after,
                direction: d,
            });
        }
    }
    let group_after = if subject == new_subject {
        ag
    } else {
        old_group_after
    };
    if held || bg != group_after {
        c.assigned.push(Change {
            check: "agency.lineage_groups".into(),
            key: serde_json::to_string(&subject).unwrap(),
            field: "judgements".into(),
            before: Payload::Integer(bg as u64),
            after: Payload::Integer(group_after as u64),
            direction: if held {
                "held"
            } else if group_after < bg {
                "improved"
            } else {
                "changed"
            },
        });
    }
    if credited {
        c.result = "undetected";
        c.reason = Some("independence_credited");
    } else if held {
        c.result = "detected";
        c.reason = None;
    }
    c.other
        .retain(|o| !c.assigned.iter().any(|a| assigned_projection(a, o)));
    c
}

fn comparison_failure(
    before: &report::Report,
    after: &report::Report,
    agency_required: bool,
) -> Option<Comparison> {
    let (result, reason) = if after.verdict == Verdict::Invalid {
        ("invalid_mutant", "static_invalid")
    } else if after.verdict == Verdict::Unsupported && after.agency.is_none() {
        ("invalid_mutant", "static_unsupported")
    } else if agency_required
        && [&before.agency, &after.agency]
            .into_iter()
            .any(|a| a.as_ref().is_some_and(|a| a.limit().is_some()))
    {
        ("unsupported", "agency_not_derived")
    } else if after.bounds.truncated.is_none() {
        ("inconclusive", "runtime_unsupported")
    } else if after.bounds.truncated == Some(true) {
        ("inconclusive", "truncated")
    } else if after.verdict == Verdict::Unsupported {
        ("inconclusive", "runtime_unsupported")
    } else if matches!(
        after.verdict,
        Verdict::Satisfied | Verdict::Violated | Verdict::Vacuous
    ) && after.bounds.truncated == Some(false)
    {
        return None;
    } else {
        // A business horizon can leave work unfinished without a computational
        // cut. Neither worsening nor static conservation completes that check.
        ("inconclusive", "assigned_check_unavailable")
    };
    Some(Comparison {
        result,
        reason: Some(reason),
        assigned: vec![],
        other: vec![],
    })
}
fn compare(kind: usize, before: &report::Report, after: &report::Report) -> Comparison {
    if let Some(c) = comparison_failure(
        before,
        after,
        checks(kind).iter().any(|c| c.starts_with("agency.")),
    ) {
        return c;
    }
    let mut assigned = vec![];
    let mut other = vec![];
    let mut applicable = false;
    let mut unavailable = false;
    let mut dynamic = false;
    // Match property identities, never positions or the overall model verdict.
    for b in &before.properties {
        let contract = b.id.starts_with("contract")
            && ![" contest_response", " excludes", " waive"]
                .iter()
                .any(|suffix| b.id.ends_with(suffix));
        let invariant = contract && b.id.ends_with(" invariant");
        let requires = contract && b.id.ends_with(" requires_event");
        let selected = match kind {
            0 => contract,
            1 => contract && b.id.ends_with(" authority_basis"),
            2 => invariant || requires,
            8 => invariant,
            _ => false,
        };
        if selected {
            applicable = true;
            dynamic = true;
        }
        let a = after.properties.iter().find(|a| a.id == b.id);
        if selected && a.is_none() {
            unavailable = true;
        }
        if let Some(a) = a {
            let target = if selected { &mut assigned } else { &mut other };
            property_change(target, &b.id, "verdict", b.verdict, a.verdict);
            if selected && a.verdict == Verdict::Inconclusive {
                unavailable = true;
            }
            for bc in b.cases.iter().flatten() {
                if let Some(ac) = a.cases.iter().flatten().find(|ac| ac.case_id == bc.case_id) {
                    property_change(
                        target,
                        &format!("{}:case:{}", b.id, bc.case_id),
                        "verdict",
                        bc.verdict,
                        ac.verdict,
                    );
                    if selected && ac.verdict == Verdict::Inconclusive {
                        unavailable = true;
                    }
                } else if selected {
                    unavailable = true;
                }
            }
        } else {
            push_change(
                &mut other,
                "properties",
                &b.id,
                "presence",
                Payload::Bool(true),
                Payload::Bool(false),
                "changed",
            );
        }
    }
    if let (Some(b), Some(a)) = (
        before
            .agency
            .as_ref()
            .and_then(crate::agency::AgencyDerivation::as_derived),
        after
            .agency
            .as_ref()
            .and_then(crate::agency::AgencyDerivation::as_derived),
    ) {
        for bl in &b.lineage {
            let al = a
                .lineage
                .iter()
                .find(|al| al.call == bl.call && al.capability == bl.capability);
            if kind == 0 {
                applicable = true;
            }
            let target = if kind == 0 { &mut assigned } else { &mut other };
            let key = format!("{}:{}", bl.call, bl.capability);
            if let Some(al) = al {
                let lost = bl.independent_sources.iter().any(|s| {
                    !al.independent_sources.contains(s) && !al.subject_sources.contains(s)
                });
                set_change(
                    target,
                    "agency.lineage",
                    &key,
                    "independent_sources",
                    &bl.independent_sources,
                    &al.independent_sources,
                    if lost { "worsened" } else { "changed" },
                );
                set_change(
                    target,
                    "agency.lineage",
                    &key,
                    "lineage_subject_changed",
                    &bl.subject_sources,
                    &al.subject_sources,
                    "changed",
                );
            } else if kind == 0 {
                unavailable = true;
            }
        }
        for br in &b.refusal {
            let ar = a
                .refusal
                .iter()
                .find(|ar| ar.call == br.call && ar.effect == br.effect);
            if kind == 8 && br.judgements.iter().any(|j| j.process().is_some()) {
                applicable = true;
            }
            let target = if kind == 8 { &mut assigned } else { &mut other };
            for bj in br
                .judgements
                .iter()
                .filter_map(crate::agency::Judgement::process)
            {
                let aj = ar.and_then(|r| {
                    r.judgements
                        .iter()
                        .filter_map(crate::agency::Judgement::process)
                        .find(|j| j.call == bj.call && j.capability == bj.capability)
                });
                let key = format!("{}:{}:{}", br.call, bj.call, bj.capability);
                if let Some(aj) = aj {
                    let rank = |s| match s {
                        crate::agency::Stops::AllPaths => 2,
                        crate::agency::Stops::Conditional => 1,
                        crate::agency::Stops::None => 0,
                    };
                    push_change(
                        target,
                        "agency.refusal",
                        &key,
                        "stops",
                        Payload::Text(stops(bj.stops).into()),
                        Payload::Text(stops(aj.stops).into()),
                        if rank(aj.stops) < rank(bj.stops) {
                            "worsened"
                        } else {
                            "improved"
                        },
                    );
                    push_change(
                        target,
                        "agency.refusal",
                        &key,
                        "bypasses",
                        Payload::Integer(bj.bypasses as u64),
                        Payload::Integer(aj.bypasses as u64),
                        if aj.stops == bj.stops && aj.bypasses > bj.bypasses {
                            "worsened"
                        } else {
                            "changed"
                        },
                    );
                } else {
                    push_change(
                        target,
                        "agency.refusal",
                        &key,
                        "lost_judgement",
                        Payload::Bool(true),
                        Payload::Bool(false),
                        if br.reachable { "worsened" } else { "changed" },
                    );
                }
            }
        }
    } else if matches!(kind, 0 | 8) {
        unavailable = true;
    }
    if kind == 9 {
        for b in before.responsibilities.iter().flatten() {
            applicable = true;
            let a = after
                .responsibilities
                .iter()
                .flatten()
                .find(|a| a.id == b.id && a.person == b.person && a.target == b.target);
            let key = format!("{}:{}:{}", b.id, b.person, b.target.value);
            if let Some(a) = a {
                if b.observation.status == "not_derived" || a.observation.status == "not_derived" {
                    unavailable = true;
                    continue;
                }
                let lost = b
                    .observation
                    .required_sources
                    .iter()
                    .any(|s| !a.observation.required_sources.contains(s));
                set_change(
                    &mut assigned,
                    "responsibilities",
                    &key,
                    "baseline-required_source_lost",
                    &b.observation.required_sources,
                    &a.observation.required_sources,
                    if lost { "worsened" } else { "changed" },
                );
                let missing = a
                    .observation
                    .missing_sources
                    .iter()
                    .any(|s| !b.observation.missing_sources.contains(s));
                set_change(
                    &mut assigned,
                    "responsibilities",
                    &key,
                    "missing_sources",
                    &b.observation.missing_sources,
                    &a.observation.missing_sources,
                    if missing { "worsened" } else { "improved" },
                );
                push_change(
                    &mut assigned,
                    "responsibilities",
                    &key,
                    "observation.status",
                    Payload::Text(b.observation.status.clone()),
                    Payload::Text(a.observation.status.clone()),
                    if b.observation.status == "present" && a.observation.status == "missing" {
                        "worsened"
                    } else {
                        "changed"
                    },
                );
                set_change(
                    &mut assigned,
                    "responsibilities",
                    &key,
                    "warnings",
                    &b.warnings,
                    &a.warnings,
                    if !b.warnings.iter().any(|w| w == "W-RESP-1")
                        && a.warnings.iter().any(|w| w == "W-RESP-1")
                    {
                        "worsened"
                    } else {
                        "changed"
                    },
                );
            } else {
                unavailable = true;
            }
        }
        for b in before.contest_paths.iter().flatten() {
            if let Some(a) = after.contest_paths.iter().flatten().find(|a| a.id == b.id) {
                applicable |= a != b;
                let key = b.id.clone();
                for (field, b, a) in [
                    (
                        "route_exists",
                        b.checks.route_exists.satisfied,
                        a.checks.route_exists.satisfied,
                    ),
                    (
                        "reversal_authority",
                        b.checks.reversal_authority.satisfied,
                        a.checks.reversal_authority.satisfied,
                    ),
                    (
                        "deadline_defined",
                        b.checks.deadline_defined.satisfied,
                        a.checks.deadline_defined.satisfied,
                    ),
                ] {
                    push_change(
                        &mut assigned,
                        "contest_paths",
                        &key,
                        field,
                        Payload::Bool(b),
                        Payload::Bool(a),
                        if b && !a { "worsened" } else { "improved" },
                    );
                }
            } else {
                unavailable = true;
            }
        }
    }
    // Remaining fields are projected to closed scalar leaves, never serialized
    // as opaque strings. Span/hash/display order are intentionally not evidence.
    other_changes(before, after, &mut other);
    other.retain(|o| !assigned.iter().any(|a| assigned_projection(a, o)));
    let (result, reason) = if dynamic && before.bounds.truncated == Some(true) {
        ("inconclusive", Some("original_truncated"))
    } else if unavailable {
        ("inconclusive", Some("assigned_check_unavailable"))
    } else if assigned.iter().any(|c| c.direction == "worsened") {
        ("detected", None)
    } else {
        (
            "undetected",
            Some(if applicable {
                "no_worsening"
            } else {
                "no_assigned_check"
            }),
        )
    };
    Comparison {
        result,
        reason,
        assigned,
        other,
    }
}
// Keep unrelated changes even when their payloads happen to equal an assigned
// change. Only the same stable subject and selected field are assigned.
fn assigned_projection(a: &Change, o: &Change) -> bool {
    if (a.check.as_str(), a.key.as_str(), a.field.as_str())
        == (o.check.as_str(), o.key.as_str(), o.field.as_str())
    {
        return true;
    }
    if o.field != "value" {
        return false;
    }
    let quoted = |s: &str| serde_json::to_string(s).expect("string serializes");
    match a.check.as_str() {
        "properties" if o.check == "properties" => {
            let (id, case) = a
                .key
                .split_once(":case:")
                .map_or((a.key.as_str(), None), |(id, case)| (id, Some(case)));
            let prefix = format!("/id={}", quoted(id));
            if !o.key.starts_with(&format!("{prefix}/")) {
                return false;
            }
            match case {
                Some(case) => o.key == format!("{prefix}/cases/case_id={case}/verdict"),
                None => o.key == format!("{prefix}/verdict"),
            }
        }
        "agency.lineage" if o.check == "agency" => {
            let Some((call, cap)) = a.key.split_once(':') else {
                return false;
            };
            let field = if a.field == "lineage_subject_changed" {
                "subject_sources"
            } else {
                &a.field
            };
            o.key
                == format!(
                    "/lineage/call={};capability={}/{field}",
                    quoted(call),
                    quoted(cap)
                )
        }
        "agency.refusal" if o.check == "agency" => {
            let parts: Vec<_> = a.key.splitn(3, ':').collect();
            if parts.len() != 3 {
                return false;
            }
            o.key
                .starts_with(&format!("/refusal/call={};", quoted(parts[0])))
                && o.key.contains(&format!(
                    "/judgements/call={};capability={}/",
                    quoted(parts[1]),
                    quoted(parts[2])
                ))
                && (a.field == "lost_judgement" || o.key.ends_with(&format!("/{}", a.field)))
        }
        "agency.lineage_groups" if o.check == "agency" => {
            let Some(subject) = o
                .key
                .strip_prefix("/lineage_groups/subject=")
                .and_then(|s| s.strip_suffix("/judgements"))
            else {
                return false;
            };
            let Ok(subject) = serde_json::from_str::<Vec<String>>(subject) else {
                return false;
            };
            let Ok(assigned) = serde_json::from_str::<Vec<String>>(&a.key) else {
                return false;
            };
            a.field == "judgements" && normalized(&subject) == normalized(&assigned)
        }
        "responsibilities" if o.check == "responsibilities" => {
            let parts: Vec<_> = a.key.splitn(3, ':').collect();
            if parts.len() != 3 {
                return false;
            }
            let field = match a.field.as_str() {
                "baseline-required_source_lost" => "observation/required_sources",
                "observation.status" => "observation/status",
                "missing_sources" => "observation/missing_sources",
                "warnings" => "warnings",
                _ => return false,
            };
            o.key == format!("/id={};person={}/{field}", parts[0], quoted(parts[1]))
        }
        "contest_paths" if o.check == "contest_paths" => {
            o.key.starts_with(&format!("/id={};", quoted(&a.key)))
                && o.key.ends_with(&format!("/checks/{}/satisfied", a.field))
        }
        _ => false,
    }
}
fn stops(s: crate::agency::Stops) -> &'static str {
    match s {
        crate::agency::Stops::AllPaths => "all_paths",
        crate::agency::Stops::Conditional => "conditional",
        crate::agency::Stops::None => "none",
    }
}
fn property_change(out: &mut Vec<Change>, key: &str, field: &str, b: Verdict, a: Verdict) {
    push_change(
        out,
        "properties",
        key,
        field,
        Payload::Text(b.as_str().into()),
        Payload::Text(a.as_str().into()),
        // A contract that was inactive (vacuous) and now fails also caught
        // the fault (ADR 0020 addendum 2).
        if matches!(b, Verdict::Satisfied | Verdict::Vacuous) && a == Verdict::Violated {
            "worsened"
        } else {
            "changed"
        },
    );
}
fn push_change(
    out: &mut Vec<Change>,
    check: &str,
    key: &str,
    field: &str,
    before: Payload,
    after: Payload,
    direction: &'static str,
) {
    if before != after {
        out.push(Change {
            check: check.into(),
            key: key.into(),
            field: field.into(),
            before,
            after,
            direction,
        });
    }
}
fn set_change(
    out: &mut Vec<Change>,
    check: &str,
    key: &str,
    field: &str,
    b: &[String],
    a: &[String],
    direction: &'static str,
) {
    let sorted = |v: &[String]| {
        let mut v = v.to_vec();
        v.sort();
        v.dedup();
        Payload::Strings(v)
    };
    push_change(out, check, key, field, sorted(b), sorted(a), direction);
}
fn other_changes(before: &report::Report, after: &report::Report, out: &mut Vec<Change>) {
    fn leaves(
        v: &serde_json::Value,
        path: &str,
        out: &mut std::collections::BTreeMap<String, Payload>,
        repeated_assumption_fields: &BTreeSet<&str>,
    ) {
        use serde_json::Value;
        match v {
            Value::Null => {
                out.insert(path.into(), Payload::Null);
            }
            Value::String(s) => {
                out.insert(path.into(), Payload::Text(s.clone()));
            }
            Value::Bool(b) => {
                out.insert(path.into(), Payload::Bool(*b));
            }
            Value::Number(n) => {
                if let Some(n) = n.as_u64() {
                    out.insert(path.into(), Payload::Integer(n));
                }
            }
            Value::Array(a) => {
                if a.iter().all(Value::is_string) {
                    let mut a: Vec<_> = a.iter().map(|x| x.as_str().unwrap().to_string()).collect();
                    a.sort();
                    a.dedup();
                    out.insert(path.into(), Payload::Strings(a));
                } else {
                    // Identity fields, not display positions, align table rows.
                    for (i, v) in a.iter().enumerate() {
                        let mut identity = Vec::new();
                        for k in [
                            "id",
                            "case_id",
                            "call",
                            "capability",
                            "effect",
                            "actor",
                            "person",
                            "end",
                            "field",
                            "code",
                            "severity",
                        ] {
                            if let Some(x) = v.get(k).filter(|x| x.is_string() || x.is_u64()) {
                                identity.push(format!("{k}={x}"));
                            }
                        }
                        if path.is_empty()
                            && let Some(field) = v.get("field").and_then(Value::as_str)
                            && repeated_assumption_fields.contains(field)
                            && let Some(value) = v.get("value")
                        {
                            identity.push(format!("value={value}"));
                        }
                        if identity.is_empty()
                            && let Some(sources) = v.get("subject_sources")
                        {
                            identity.push(format!("subject={sources}"));
                        }
                        let identity = if identity.is_empty() {
                            i.to_string()
                        } else {
                            identity.join(";")
                        };
                        let row_path = format!("{path}/{identity}");
                        if v.get("code").is_some() {
                            let count = match out.get(&row_path) {
                                Some(Payload::Integer(n)) => n + 1,
                                _ => 1,
                            };
                            out.insert(row_path, Payload::Integer(count));
                        } else {
                            leaves(v, &row_path, out, repeated_assumption_fields);
                        }
                    }
                }
            }
            Value::Object(o) => {
                for (k, v) in o {
                    if k != "span" && k != "sha256" {
                        leaves(v, &format!("{path}/{k}"), out, repeated_assumption_fields);
                    }
                }
            }
        }
    }
    let b = serde_json::to_value(before).expect("report serializes");
    let a = serde_json::to_value(after).expect("report serializes");
    // Both projections must use the same identity even for a 2 -> 1 change.
    // Single-row fields keep their historical keys, including value changes.
    let mut repeated_assumption_fields = BTreeSet::new();
    for report in [before, after] {
        let mut seen = BTreeSet::new();
        for row in &report.assumptions {
            if !seen.insert(row.field.as_str()) {
                repeated_assumption_fields.insert(row.field.as_str());
            }
        }
    }
    let no_repeated_fields = BTreeSet::new();
    for field in [
        "unchecked",
        "agency",
        "responsibilities",
        "contest_paths",
        "diagnostics",
        "terminals",
        "case_outcomes",
        "admission",
        "metrics",
        "assumptions",
        "properties",
    ] {
        let mut bs = std::collections::BTreeMap::new();
        let mut as_ = std::collections::BTreeMap::new();
        let repeated = if field == "assumptions" {
            &repeated_assumption_fields
        } else {
            &no_repeated_fields
        };
        leaves(&b[field], "", &mut bs, repeated);
        leaves(&a[field], "", &mut as_, repeated);
        let keys: std::collections::BTreeSet<_> = bs.keys().chain(as_.keys()).collect();
        for key in keys {
            push_change(
                out,
                field,
                key,
                "value",
                bs.get(key).cloned().unwrap_or(Payload::Null),
                as_.get(key).cloned().unwrap_or(Payload::Null),
                "changed",
            );
        }
    }
}

#[cfg(test)]
#[path = "../tests/support/mutation_default_private.rs"]
mod default_private;

#[cfg(test)]
mod default_tests {
    use super::*;

    #[test]
    fn private_comparison_oracle() {
        default_private::comparison_cases(|b, a, sources, call, capability| {
            let mut b = b.clone();
            let mut a = a.clone();
            crate::ledger_test_shape::dto_projection(&mut b).unwrap();
            crate::ledger_test_shape::dto_projection(&mut a).unwrap();
            let b = serde_json::from_value(b).unwrap();
            let a = serde_json::from_value(a).unwrap();
            let sources = sources.iter().map(|s| s.to_string()).collect();
            let c = compare_summary(&b, &a, &sources, call, capability);
            serde_json::json!({ "result": c.result, "reason": c.reason, "assigned_changes": c.assigned, "other_changes": c.other })
        });
    }

    #[test]
    fn private_ast_oracle() {
        default_private::ast_cases(|src, id| {
            let ast = syntax::parse(src).unwrap();
            let context = lower::lower(&ast).unwrap();
            summary_sites(src, &ast, &context)
                .into_iter()
                .find(|c| format!("summary_reused:{}", c.identity) == id)
                .unwrap()
                .apply(&ast)
        });
    }

    #[test]
    fn authority_case_disappearance_and_original_truncation() {
        let mut b = report::check(include_str!(
            "../../../fixtures/mut2-expiry-cases/model.relca"
        ));
        let mut a = b.clone();
        let property = a
            .properties
            .iter_mut()
            .find(|p| p.id.ends_with(" authority_basis"))
            .unwrap();
        property.cases.as_mut().unwrap().remove(0);
        assert_eq!(
            compare(1, &b, &a).reason,
            Some("assigned_check_unavailable")
        );
        a = b.clone();
        b.bounds.truncated = Some(true);
        assert_eq!(compare(1, &b, &a).reason, Some("original_truncated"));
    }

    #[test]
    fn independent_duplicates_and_static_original_truncation() {
        let mut b = report::check(include_str!("../../../fixtures/mut2-summary/model.relca"));
        let l = b
            .agency
            .as_mut()
            .unwrap()
            .as_derived_mut()
            .unwrap()
            .lineage
            .iter_mut()
            .find(|l| l.call == "a")
            .unwrap();
        l.independent_sources = vec!["C".into()];
        let mut a = b.clone();
        a.agency
            .as_mut()
            .unwrap()
            .as_derived_mut()
            .unwrap()
            .lineage
            .iter_mut()
            .find(|l| l.call == "a")
            .unwrap()
            .independent_sources
            .push("C".into());
        b.bounds.truncated = Some(true);
        let c = compare_summary(
            &b,
            &a,
            &BTreeSet::from(["Invoice".into()]),
            "a",
            "Approver.approve",
        );
        assert_eq!(c.result, "detected");
        assert!(c.assigned.iter().all(|c| c.direction == "held"));
    }
}

#[cfg(test)]
#[path = "evaluation_mutation_tests.rs"]
mod evaluation_mutation_tests;
#[cfg(test)]
#[path = "../tests/support/evaluation_review_private_mutation.rs"]
mod evaluation_review_private_mutation;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lower, reference};
    use std::cell::Cell;

    fn source(name: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures")
                .join(name)
                .join("model.relca"),
        )
        .unwrap()
    }
    fn fixture(name: &str) -> report::Report {
        report::check(&source(name))
    }
    #[test]
    fn policy_mutant_ast_retains_complete_policy_and_relowers_each_guard() {
        let mut src = source("mut-guard");
        let at = src.rfind('}').unwrap();
        src.insert_str(
            at,
            "policy observable fifo tie_by case_id then call_order assign first_idle\n",
        );
        let ast = syntax::parse(&src).unwrap();
        let policy: Vec<_> = ast
            .items
            .iter()
            .filter(|i| matches!(i, ast::Item::Policy(_)))
            .cloned()
            .collect();
        assert_eq!(policy.len(), 1);
        let candidates = guard_sites(&src, &ast, None);
        assert!(!candidates.is_empty(), "actually generate guard mutants");
        for candidate in candidates {
            let changed = candidate.apply(&ast);
            assert_ne!(changed, ast);
            let actual: Vec<_> = changed
                .items
                .iter()
                .filter(|i| matches!(i, ast::Item::Policy(_)))
                .cloned()
                .collect();
            assert_eq!(
                actual, policy,
                "all policy fields and source spans preserved"
            );
            let m = lower::lower(&changed).unwrap();
            assert!(m.start_policy.as_ref().unwrap().call_order);
            assert_eq!(reference::explore(&m).stop, reference::StopReason::Complete);
        }
    }
    fn structural(ast: &ast::Model) -> String {
        // Debug includes every AST field. Only source-byte spans differ between
        // independently reparsed text and source-preserving AST edits.
        let mut text = format!("{ast:?}");
        while let Some(start) = text.find("Span {") {
            let end = start + text[start..].find('}').unwrap() + 1;
            text.replace_range(start..end, "<source-span>");
        }
        text
    }
    fn hand_edit(src: &str, c: &Candidate) -> String {
        if c.operator() == "remove_read" {
            let actor = c.site.actor.as_ref().unwrap();
            let begin = src.find(&format!("actor {actor} {{")).unwrap();
            let at = begin + src[begin..].find("reads [").unwrap();
            let end = at + src[at..].find(']').unwrap() + 1;
            let kept: Vec<_> = src[at + 7..end - 1]
                .split(',')
                .filter(|s| s.trim() != c.site.source.as_ref().unwrap())
                .collect();
            let new = if kept.is_empty() {
                String::new()
            } else {
                format!("reads [{}]", kept.join(","))
            };
            format!("{}{}{}", &src[..at], new, &src[end..])
        } else {
            let b = &c.site.affected_bindings[0];
            let old = format!("if {b} == pass {{ }} else {{ end returned }}");
            assert_eq!(src.matches(&old).count(), 1);
            src.replacen(&old, "", 1)
        }
    }
    #[test]
    fn m_one_fault_all_generated_asts_independent_source_and_backends() {
        let mut count = 0;
        for name in [
            "mut-evidence",
            "mut-evidence.broken-no-independent-read",
            "mut-guard",
            "mut-guard.broken-no-contract",
            "mut-origin",
            "mut-origin.broken-route-only",
            "mut-limit",
            "mut-limit.broken-many-sites",
        ] {
            let src = source(name);
            let ast = syntax::parse(&src).unwrap();
            let original = ast.clone();
            let reads = read_sites(&src, &ast, None);
            let guards = guard_sites(&src, &ast, None);
            for c in reads
                .iter()
                .chain(&guards)
                .chain(&guards)
                .chain(&reads)
                .take(128)
            {
                let actual = c.apply(&ast);
                let expected = syntax::parse(&hand_edit(&src, c)).unwrap();
                assert_eq!(
                    structural(&actual),
                    structural(&expected),
                    "{name}:{}",
                    c.identity
                );
                assert_eq!(ast, original, "no accumulated edit");
                let ir = lower::lower(&actual).unwrap();
                let e = engine::explore(&ir);
                let mut r = reference::explore(&ir);
                assert_eq!(r.stop, reference::StopReason::Complete);
                assert!(r.outcome.diagnostics.starts_with(&ir.diagnostics));
                r.outcome.diagnostics.drain(..ir.diagnostics.len());
                assert_eq!(e, r.outcome);
                count += 1;
            }
        }
        assert_eq!(count, 286);
    }
    #[test]
    fn m_guard_keeps_nested_block_and_parent_suffix() {
        let src = source("mut-guard").replace(
            "if po == pass { } else { end returned }",
            "if po == pass { if po == fail { } else { } } else { end returned }",
        );
        let ast = syntax::parse(&src).unwrap();
        let cs = guard_sites(&src, &ast, None);
        let edited = cs[0].apply(&ast);
        let expected = syntax::parse(&src.replace(
            "if po == pass { if po == fail { } else { } } else { end returned }",
            "if po == fail { } else { }",
        ))
        .unwrap();
        assert_eq!(structural(&edited), structural(&expected));
        let dead = src.replace(
            "if po == pass { if po == fail { } else { } } else { end returned }",
            "if po == pass { end returned } else { end returned }; end returned",
        );
        let cs = guard_sites(&dead, &syntax::parse(&dead).unwrap(), None);
        assert!(cs.iter().any(|c| c.identity.ends_with("body/2:keep_then")));
        assert!(cs.iter().any(|c| c.identity.ends_with("body/2:keep_else")));
    }
    #[test]
    fn m_guard_then_else_nested_site_hand_inventory() {
        // Hand count: outer G1 has two directions (both discarded branches
        // contain End and the parent suffix contains approve/post calls).
        // Its then and else branches each have one terminal inner guard.
        // Original G2 contributes one more: 2 + 1 + 1 + 1 = 5.
        for dead_suffix in [false, true] {
            let inserted = if dead_suffix {
                "if po == pass { if po == fail { } else { end returned } } else { if po == pass { } else { end returned } }; end returned"
            } else {
                "if po == pass { if po == fail { } else { end returned } } else { if po == pass { } else { end returned } }"
            };
            let src =
                source("mut-guard").replace("if po == pass { } else { end returned }", inserted);
            let ast = syntax::parse(&src).unwrap();
            let sites = guard_sites(&src, &ast, None);
            let last = if dead_suffix {
                "Flow:body/5:keep_then"
            } else {
                "Flow:body/4:keep_then"
            };
            assert_eq!(
                sites
                    .iter()
                    .map(|c| c.identity.as_str())
                    .collect::<Vec<_>>(),
                [
                    "Flow:body/2:keep_then",
                    "Flow:body/2:keep_else",
                    "Flow:body/2/then/0:keep_then",
                    "Flow:body/2/else/0:keep_then",
                    last,
                ]
            );
            // Each outer bypass retains the inner If verbatim, and retains
            // every parent suffix statement, including text following End.
            let ast::Item::Process(original) = ast
                .items
                .iter()
                .find(|i| matches!(i, ast::Item::Process(_)))
                .unwrap()
            else {
                unreachable!()
            };
            let ast::Stmt::If {
                then_body,
                else_body,
                ..
            } = &original.body[2].value
            else {
                unreachable!()
            };
            for (c, kept) in sites[..2].iter().zip([then_body, else_body]) {
                let edited = c.apply(&ast);
                let ast::Item::Process(actual) = edited
                    .items
                    .iter()
                    .find(|i| matches!(i, ast::Item::Process(_)))
                    .unwrap()
                else {
                    unreachable!()
                };
                let expected: ast::Block = original.body[..2]
                    .iter()
                    .chain(kept)
                    .chain(&original.body[3..])
                    .cloned()
                    .collect();
                assert_eq!(actual.body, expected);
            }
        }
    }
    #[test]
    fn m_original_gate_all_six_and_limit_counters() {
        let src = source("mut-evidence");
        let original = fixture("mut-evidence");
        for verdict in [
            Verdict::Satisfied,
            Verdict::Violated,
            Verdict::Invalid,
            Verdict::Unsupported,
            Verdict::Inconclusive,
            Verdict::Vacuous,
        ] {
            let baselines = Cell::new(0);
            let executions = Cell::new(0);
            let r = mutate_using(
                &src,
                128,
                None,
                |_| {
                    baselines.set(baselines.get() + 1);
                    let mut r = original.clone();
                    r.verdict = verdict;
                    r
                },
                |src, ast| {
                    executions.set(executions.get() + 1);
                    report::check_ast_using(src, ast, engine::explore)
                },
            );
            assert_eq!(baselines.get(), 1);
            for i in [5, 7] {
                assert_eq!(r.mutations[i].status, "unsupported");
                assert_eq!(r.mutations[i].reason, KINDS[i].2);
            }
            let wire = serde_json::to_value(&r).unwrap();
            if matches!(verdict, Verdict::Satisfied | Verdict::Violated) {
                response_boundary_oracle::assert_not_applicable(&src, &wire["mutations"]);
            } else {
                response_boundary_oracle::assert_blocked(&wire["mutations"]);
            }
            if matches!(verdict, Verdict::Satisfied | Verdict::Violated) {
                assert_eq!(executions.get(), 4);
                for i in [0, 1, 2, 4, 8, 9] {
                    assert!(matches!(
                        r.mutations[i].status,
                        "generated" | "not_applicable"
                    ));
                }
            } else {
                assert_eq!(executions.get(), 0);
                assert_eq!(r.verdict, verdict);
                assert_eq!(r.run.completion, "blocked");
                for i in [0, 1, 2, 4, 8, 9] {
                    assert_eq!(r.mutations[i].status, "blocked");
                    assert_eq!(r.mutations[i].reason, Some("original_not_decidable"));
                    assert!(r.mutations[i].mutants.is_empty());
                }
            }
        }
        for (name, n) in [
            ("mut-empty", 0),
            ("mut-evidence", 4),
            ("mut-limit", 128),
            ("mut-limit.broken-many-sites", 130),
        ] {
            for limit in [0, 1, 2, 128] {
                let src = source(name);
                let calls = Cell::new(0);
                let baselines = Cell::new(0);
                let r = mutate_using(
                    &src,
                    limit,
                    None,
                    |s| {
                        baselines.set(baselines.get() + 1);
                        report::check(s)
                    },
                    |src, ast| {
                        calls.set(calls.get() + 1);
                        report::check_ast_using(src, ast, engine::explore)
                    },
                );
                assert_eq!(baselines.get(), 1);
                assert_eq!(calls.get(), n.min(limit));
                let all: Vec<_> = r.mutations.iter().flat_map(|r| &r.mutants).collect();
                assert_eq!(all.len(), n);
                assert_eq!(
                    all.iter().filter(|m| m.result == "not_run").count(),
                    n.saturating_sub(limit)
                );
                assert_eq!(r.run.limit_reached, n > limit);
                assert_eq!(
                    r.run.completion,
                    if n > limit {
                        "inconclusive"
                    } else {
                        "complete"
                    }
                );
            }
        }
        assert!(
            mutate_set("must not parse this", "unknown")
                .original
                .is_none()
        );
    }
    #[test]
    fn m_limit_exact_candidate_matrix() {
        let src = source("mut-evidence");
        let ast = syntax::parse(&src).unwrap();
        let c = read_sites(&src, &ast, None).remove(0);
        for limit in [0, 1, 2, 128] {
            for n in [0_usize, 1, 2, 3, 128, 129] {
                let mut attempts = 0;
                let mut edited = 0;
                let mut pending = 0;
                for _ in 0..n {
                    match budgeted_edit(&c, &ast, &mut attempts, limit) {
                        Some(m) => {
                            edited += 1;
                            assert_ne!(m, ast);
                        }
                        None => pending += 1,
                    }
                }
                assert_eq!(attempts, n.min(limit));
                assert_eq!(edited, attempts);
                assert_eq!(pending, n.saturating_sub(limit));
            }
        }
    }
    fn row(results: &[&'static str]) -> KindRow {
        let mut r = uncomputed("invalid_arguments", 128).mutations.remove(0);
        r.status = if results.is_empty() {
            "not_applicable"
        } else {
            "generated"
        };
        r.mutants = results
            .iter()
            .map(|&result| MutantRow {
                id: result.into(),
                site: site("", diag::Span { start: 0, end: 0 }),
                operator: "remove_read",
                result,
                reason: None,
                report: None,
                assigned_changes: vec![],
                other_changes: vec![],
            })
            .collect();
        r
    }
    #[test]
    fn r1_unsupported_mutant_does_not_need_an_unsupported_kind_to_close_fold() {
        let mut rows = uncomputed("invalid_arguments", 128).mutations;
        for r in &mut rows {
            r.status = "generated";
            r.reason = None;
            r.mutants = row(&["detected"]).mutants;
        }
        // In particular stale_definition_write (index 5) is now supported.
        assert_eq!(rows[5].status, "generated");
        rows[0].mutants[0].result = "unsupported";
        assert_eq!(fold(&rows), Verdict::Unsupported);
    }
    #[test]
    fn r1_unsupported_agency_checks_leave_the_run_incomplete() {
        let r = mutate(&crate::agency::limit_test_inputs::work());
        assert!(
            r.mutations
                .iter()
                .flat_map(|k| &k.mutants)
                .any(|m| m.result == "unsupported")
        );
        assert_eq!(r.run.completion, "inconclusive");
        assert_eq!(r.run.reason, Some("mutant_incomplete"));
    }
    #[test]
    fn m_fold_all_combinations() {
        for unsupported in [false, true] {
            for a in [
                "detected",
                "undetected",
                "invalid_mutant",
                "inconclusive",
                "not_run",
            ] {
                for b in [
                    "detected",
                    "undetected",
                    "invalid_mutant",
                    "inconclusive",
                    "not_run",
                ] {
                    let mut rows = vec![row(&[a, b])];
                    if unsupported {
                        rows.push(uncomputed("invalid_arguments", 128).mutations.remove(5));
                    }
                    let want = if unsupported {
                        Verdict::Unsupported
                    } else if [a, b]
                        .iter()
                        .any(|r| matches!(*r, "invalid_mutant" | "inconclusive" | "not_run"))
                    {
                        Verdict::Inconclusive
                    } else if [a, b].contains(&"undetected") {
                        Verdict::Violated
                    } else {
                        Verdict::Satisfied
                    };
                    assert_eq!(fold(&rows), want);
                }
            }
        }
        assert_eq!(
            fold(&[row(&[]), row(&[]), row(&[]), row(&[])]),
            Verdict::Satisfied
        );
    }
    #[test]
    fn m_contract_directions_case_keys_reordering_and_unavailable() {
        let base = fixture("mut-guard");
        for (before, after, want) in [
            (Verdict::Satisfied, Verdict::Violated, "detected"),
            (Verdict::Vacuous, Verdict::Violated, "detected"),
            (Verdict::Violated, Verdict::Violated, "undetected"),
            (Verdict::Satisfied, Verdict::Vacuous, "undetected"),
        ] {
            let mut b = base.clone();
            let mut a = base.clone();
            b.properties[1].verdict = before;
            a.properties[1].verdict = after;
            assert_eq!(compare(2, &b, &a).result, want);
        }
        let mut a = base.clone();
        a.properties.reverse();
        assert!(compare(2, &base, &a).assigned.is_empty());
        assert!(compare(2, &base, &a).other.is_empty());
        a = base.clone();
        a.properties.remove(1);
        assert_eq!(
            compare(2, &base, &a).reason,
            Some("assigned_check_unavailable")
        );
        a = base.clone();
        a.properties[0].verdict = Verdict::Violated;
        assert_eq!(compare(2, &base, &a).result, "undetected");
        assert!(!compare(2, &base, &a).other.is_empty());
        a.properties[1].verdict = Verdict::Violated;
        let both = compare(2, &base, &a);
        assert_eq!(both.result, "detected");
        assert!(
            both.other
                .iter()
                .any(|c| c.check == "properties" && c.key == base.properties[0].id),
            "equal before/after verdicts on different properties must not erase other changes"
        );
        let mut b = base.clone();
        b.properties[1].cases = Some(vec![report::CaseProperty {
            case_id: 1,
            arrival: 0,
            verdict: Verdict::Satisfied,
            probability: None,
        }]);
        a = b.clone();
        a.properties[1].cases.as_mut().unwrap()[0].verdict = Verdict::Violated;
        assert_eq!(compare(2, &b, &a).result, "detected");
        b.bounds.truncated = Some(true);
        assert_eq!(compare(2, &b, &a).reason, Some("original_truncated"));
        // A source-only comparison remains available when no dynamic selector exists.
        let mut b = fixture("mut-evidence");
        let a = b.clone();
        b.bounds.truncated = Some(true);
        assert_eq!(compare(0, &b, &a).result, "undetected");
    }
    #[test]
    fn m_truncate_and_static_runtime_u_boundaries() {
        let b = fixture("mut-guard");
        let mut a = b.clone();
        a.properties[1].verdict = Verdict::Violated;
        a.bounds.truncated = Some(true);
        assert_eq!(compare(2, &b, &a).reason, Some("truncated"));
        a = b.clone();
        a.agency = None;
        a.verdict = Verdict::Unsupported;
        a.bounds.truncated = None;
        assert_eq!(compare(2, &b, &a).reason, Some("static_unsupported"));
        a = b.clone();
        a.verdict = Verdict::Unsupported;
        a.bounds.truncated = None;
        assert_eq!(compare(2, &b, &a).reason, Some("runtime_unsupported"));
        let src = source("mut-guard");
        let ast = syntax::parse(&src).unwrap();
        let mut bad = ast.clone();
        for item in &mut bad.items {
            if let ast::Item::Actor(actor) = item {
                for item in &mut actor.items {
                    if let ast::ActorItem::Capability(cap) = item {
                        for item in &mut cap.items {
                            if let ast::CapItem::DurationFixed(d) = &mut item.value {
                                d.unit.value = "day".into();
                            }
                        }
                    }
                }
            }
        }
        let static_u =
            report::check_ast_using(&src, &bad, |_| panic!("static rejection must not execute"));
        assert!(static_u.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
        assert_eq!(compare(2, &b, &static_u).reason, Some("static_unsupported"));
        let runtime_u = report::check_ast_using(&src, &ast, |m| {
            let mut out = engine::explore(m);
            out.diagnostics.push(diag::Diagnostic {
                code: diag::U_SEM_0.into(),
                severity: diag::Severity::Error,
                message: "injected backend refusal".into(),
                span: None,
            });
            out
        });
        assert_eq!(
            compare(2, &b, &runtime_u).reason,
            Some("runtime_unsupported")
        );
        for budget in [0, 1] {
            let a = report::check_ast_using(&src, &ast, |m| engine::explore_with_budget(m, budget));
            assert_eq!(compare(2, &b, &a).reason, Some("truncated"));
        }
        let calls = Cell::new(0);
        let r = mutate_using(&src, 1, None, report::check, |_, _| {
            calls.set(calls.get() + 1);
            let mut r = b.clone();
            r.agency = None;
            r.verdict = Verdict::Invalid;
            r
        });
        assert_eq!(calls.get(), 1);
        assert_eq!(r.mutations[0].mutants[0].result, "invalid_mutant");
        assert!(
            r.mutations
                .iter()
                .flat_map(|r| &r.mutants)
                .skip(1)
                .all(|m| m.result == "not_run")
        );
    }
    #[test]
    fn m_lineage_set_identity_regroup_and_subject_only() {
        let b = fixture("mut-evidence");
        let mut a = b.clone();
        a.agency.as_mut().unwrap().as_derived_mut().unwrap().lineage[0].independent_sources =
            vec!["Replacement".into()];
        assert_eq!(
            compare(0, &b, &a).result,
            "detected",
            "same cardinality loses original source"
        );
        a = b.clone();
        a.agency
            .as_mut()
            .unwrap()
            .as_derived_mut()
            .unwrap()
            .lineage_groups
            .reverse();
        assert_eq!(compare(0, &b, &a).result, "undetected");
        let mut split = a
            .agency
            .as_ref()
            .unwrap()
            .as_derived()
            .unwrap()
            .lineage_groups[0]
            .clone();
        split.subject_sources = vec!["RegroupedSubject".into()];
        a.agency
            .as_mut()
            .unwrap()
            .as_derived_mut()
            .unwrap()
            .lineage_groups
            .push(split);
        let regrouped = compare(0, &b, &a);
        assert_eq!(regrouped.result, "undetected");
        assert!(
            !regrouped.other.is_empty(),
            "regrouping is recorded without inventing source loss"
        );
        a = b.clone();
        a.agency.as_mut().unwrap().as_derived_mut().unwrap().lineage[0].subject_sources =
            vec!["OtherSubject".into()];
        assert_eq!(compare(0, &b, &a).result, "undetected");
        a = b.clone();
        let l = &mut a.agency.as_mut().unwrap().as_derived_mut().unwrap().lineage[0];
        l.subject_sources.append(&mut l.independent_sources);
        assert_eq!(
            compare(0, &b, &a).result,
            "undetected",
            "source reclassification is not lost evidence"
        );
    }
    #[test]
    fn m_refusal_stops_bypasses_and_lost_judgement() {
        let b = fixture("mut-guard");
        for mode in 0..3 {
            let mut a = b.clone();
            let r = &mut a.agency.as_mut().unwrap().as_derived_mut().unwrap().refusal[0];
            let crate::agency::Judgement::Process(j) = &mut r.judgements[0] else {
                panic!("legacy process judgement")
            };
            match mode {
                0 => j.stops = crate::agency::Stops::None,
                1 => j.bypasses += 1,
                _ => {
                    r.judgements.remove(0);
                }
            }
            assert_eq!(compare(8, &b, &a).result, "detected");
            assert_eq!(compare(2, &b, &a).result, "undetected");
        }
    }
    #[test]
    fn m_responsibility_required_loss_missing_not_derived_warning_target() {
        let b = fixture("mut-origin");
        for mode in 0..5 {
            let mut a = b.clone();
            let r = &mut a.responsibilities.as_mut().unwrap()[0];
            match mode {
                0 => r.observation.required_sources.clear(),
                1 => r.observation.missing_sources.push("Invoice".into()),
                2 => r.observation.status = "missing".into(),
                3 => r.observation.status = "not_derived".into(),
                _ => r.warnings.push("W-RESP-1".into()),
            }
            assert_eq!(
                compare(9, &b, &a).result,
                if mode == 3 {
                    "inconclusive"
                } else {
                    "detected"
                }
            );
        }
        let mut b = b;
        let mut second = b.responsibilities.as_ref().unwrap()[0].clone();
        second.id += 1;
        second.person = "other".into();
        second.warnings.push("W-RESP-1".into());
        b.responsibilities.as_mut().unwrap().push(second);
        let mut a = b.clone();
        a.responsibilities.as_mut().unwrap()[0]
            .warnings
            .push("W-RESP-1".into());
        a.responsibilities.as_mut().unwrap()[1].warnings.clear();
        let c = compare(9, &b, &a);
        assert_eq!(c.result, "detected");
        assert!(
            c.assigned
                .iter()
                .any(|c| c.field == "warnings" && c.key.starts_with("1:"))
        );
    }
}
