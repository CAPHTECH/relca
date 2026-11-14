//! Observed event contracts only (ADR 0057); never a process replay or simulation.
mod approval;
pub mod checks;
pub mod identity;
pub mod input;
pub mod lifecycle;
pub mod monitors;
pub mod order;
mod q1_scope;

use crate::{Verdict, diag, ir, lower, report, syntax};
use checks::{Check, Tri};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Read;

#[derive(Debug, Serialize)]
pub struct ConformanceReport {
    pub ledger: crate::ledger::Ledger,
    pub uncertainty: crate::uncertainty::Uncertainty,
    pub verdict: Verdict,
    pub conformance: Tri,
    pub mode: &'static str,
    pub schema: &'static str,
    pub scope: &'static str,
    pub model: report::ModelMetadata,
    pub log: LogMetadata,
    pub configuration: Option<String>,
    pub scenario: Option<String>,
    pub cut: report::Cut,
    pub cut_context: report::CutContext,
    pub assumptions: Vec<Value>,
    pub checks: Vec<Check>,
    pub cases: Vec<Case>,
    pub summary: Summary,
    pub unmapped_actor: Vec<identity::UnmappedActor>,
    pub unmapped_operation: Vec<identity::UnmappedOperation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unchecked: Vec<String>,
    pub not_checked: Vec<NotChecked>,
    pub limits: Limits,
    pub diagnostics: Vec<LogDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_burden: Option<Value>,
}
/// Extend only conform's wire diagnostics; other commands retain the common
/// Diagnostic unchanged. Inherited model diagnostics have no invented reason.
#[derive(Debug, Serialize)]
pub struct LogDiagnostic {
    #[serde(flatten)]
    pub diagnostic: diag::Diagnostic,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
impl From<diag::Diagnostic> for LogDiagnostic {
    fn from(diagnostic: diag::Diagnostic) -> Self {
        Self {
            diagnostic,
            reason: None,
        }
    }
}
impl std::ops::Deref for LogDiagnostic {
    type Target = diag::Diagnostic;
    fn deref(&self) -> &Self::Target {
        &self.diagnostic
    }
}
#[derive(Debug, Default, Serialize)]
pub struct LogMetadata {
    pub sha256: Option<String>,
    pub lines: Option<usize>,
    pub processed_lines: usize,
    pub events: usize,
    pub clock: Option<String>,
    pub time_unit: Option<String>,
    pub window: Option<input::Window>,
}
#[derive(Debug, Default, Serialize)]
pub struct Counts {
    pub conformant: usize,
    pub violating: usize,
    pub unknown: usize,
}
impl Counts {
    fn add(&mut self, state: Tri) {
        match state {
            Tri::Conformant => self.conformant += 1,
            Tri::Violating => self.violating += 1,
            Tri::Unknown => self.unknown += 1,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct Case {
    pub case_id: String,
    pub conformance: Tri,
    pub checks: Counts,
}
#[derive(Debug, Default, Serialize)]
pub struct Summary {
    pub cases: Counts,
    pub checks: Counts,
    pub unsupported: usize,
    pub unmapped_actor: usize,
    pub unmapped_operation: usize,
}
#[derive(Debug, Serialize)]
pub struct NotChecked {
    pub item: String,
    pub reason: &'static str,
    pub model_span: Option<diag::Span>,
}
#[derive(Debug, Serialize)]
pub struct Limits {
    pub max_events: usize,
    pub max_line_bytes: usize,
    pub max_order_states: usize,
    pub event_limit: bool,
    pub line_limit: bool,
    pub order_limit: bool,
    pub truncated: bool,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_events: input::MAX_EVENTS,
            max_line_bytes: input::MAX_LINE_BYTES,
            max_order_states: input::MAX_ORDER_STATES,
            event_limit: false,
            line_limit: false,
            order_limit: false,
            truncated: false,
        }
    }
}
impl Default for ConformanceReport {
    fn default() -> Self {
        Self {
            ledger: crate::ledger::Ledger::conformance(true),
            uncertainty: crate::uncertainty::Uncertainty::not_lowered(),
            verdict: Verdict::Inconclusive,
            conformance: Tri::Unknown,
            mode: "log_conformance",
            schema: "relca-conformance/v1",
            scope: "observable_event_contracts_v1",
            model: report::ModelMetadata {
                name: None,
                sha256: None,
            },
            log: LogMetadata::default(),
            configuration: None,
            scenario: None,
            cut: report::Cut::Status(report::CutStatus::Unresolved),
            cut_context: report::CutContext::default(),
            assumptions: Vec::new(),
            checks: Vec::new(),
            cases: Vec::new(),
            summary: Summary::default(),
            unmapped_actor: Vec::new(),
            unmapped_operation: Vec::new(),
            unchecked: Vec::new(),
            not_checked: [
                "process_path",
                "fork_join_control",
                "work_conserving",
                "resource_capacity",
                "resource_waits",
            ]
            .into_iter()
            .map(|s| NotChecked {
                item: s.into(),
                reason: "outside_scope",
                model_span: None,
            })
            .collect(),
            limits: Limits::default(),
            diagnostics: Vec::new(),
            approval_burden: None,
        }
        .seal()
    }
}

#[cfg(test)]
mod ledger_default_tests {
    use super::*;
    #[test]
    fn conformance_default_ledger_is_closed_without_claiming_an_execution() {
        let (r, calls) = crate::evaluation_spy::observe(ConformanceReport::default);
        assert_eq!(calls, [0; 7]);
        assert_eq!(r.verdict, Verdict::Inconclusive);
        assert_eq!(r.ledger.execution["status"], "not_run");
        r.ledger
            .validate(&serde_json::to_value(&r).unwrap())
            .unwrap();
    }
}
fn diagnostic(
    code: &str,
    reason: &str,
    message: impl Into<String>,
    span: Option<diag::Span>,
) -> LogDiagnostic {
    LogDiagnostic {
        diagnostic: diag::Diagnostic {
            code: code.into(),
            severity: diag::severity_of(code).expect("registered conformance code"),
            message: message.into(),
            span,
        },
        reason: Some(reason.into()),
    }
}
impl ConformanceReport {
    /// Select the opt-in wire even for argument, I/O and model refusals.
    pub fn approval_scope(mut self) -> Self {
        self.schema = "relca-conformance-approval/v1";
        self.scope = "observable_event_contracts_with_approval_burden_v1";
        if self.approval_burden.is_none() {
            self.approval_burden = Some(Value::Null);
        }
        self
    }
    pub fn with_model_source(&mut self, src: &str) {
        self.model.sha256 = Some(
            Sha256::digest(src.as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        );
        self.model.name = syntax::parse(src).ok().map(|ast| ast.name.value);
    }
    pub fn failure(code: &'static str, reason: &'static str) -> Self {
        let mut r = Self::default();
        r.fail(input::Failure { code, reason });
        r.seal()
    }
    /// Reconnect CLI-only I/O refusals as well as the core entry point.
    pub fn seal(mut self) -> Self {
        if self.configuration.is_some() || self.scenario.is_some() {
            self.ledger
                .requested(self.configuration.as_deref(), self.scenario.as_deref());
        }
        for (name, n) in [
            ("max_events", self.limits.max_events),
            ("max_line_bytes", self.limits.max_line_bytes),
            ("max_order_states", self.limits.max_order_states),
        ] {
            self.ledger.execution["limits"][name] = crate::ledger::known(n);
        }
        let host = serde_json::to_value(&self).expect("conformance uncertainty context");
        self.uncertainty = crate::uncertainty::Uncertainty::conform(&host);
        let host = serde_json::to_value(&self).expect("conformance wire");
        let mut diagnostics = Vec::new();
        crate::ledger::complete(&mut self.ledger, &host, &mut self.verdict, &mut diagnostics);
        for diagnostic in diagnostics {
            if !self.diagnostics.iter().any(|d| d.code == diagnostic.code) {
                self.diagnostics.push(diagnostic.into());
            }
        }
        self
    }
    fn fail(&mut self, failure: input::Failure) {
        self.verdict = if failure.code.starts_with("U-") {
            Verdict::Unsupported
        } else {
            Verdict::Invalid
        };
        self.conformance = Tri::Unknown;
        self.checks.clear();
        self.cases.clear();
        self.summary = Summary::default();
        if self.approval_burden.is_some() {
            self.approval_burden = Some(Value::Null);
        }
        self.diagnostics.push(diagnostic(
            failure.code,
            failure.reason,
            failure.reason,
            None,
        ));
    }
    fn scope_check(&mut self, id: &str, case: Option<&str>, reason: &str, events: &[input::Event]) {
        self.checks.push(Check::new(
            id,
            case,
            Tri::Unknown,
            Some(reason),
            &[],
            events,
            None,
        ));
    }
    fn finish(&mut self, scopes: &[input::CaseScope]) {
        self.checks.sort_by(|a, b| {
            fn key(c: &Check) -> (u8, usize, &str) {
                if let Some(n) =
                    c.id.strip_prefix("independent:")
                        .unwrap_or(&c.id)
                        .strip_prefix("contract:")
                        .and_then(|s| s.parse().ok())
                {
                    (1, n, &c.id)
                } else {
                    (0, 0, &c.id)
                }
            }
            (&a.case_id, key(a)).cmp(&(&b.case_id, key(b)))
        });
        self.summary = Summary::default();
        for c in &self.checks {
            self.summary.checks.add(c.conformance);
            if c.conformance != Tri::Conformant {
                let reason = c.reason.as_deref().expect("unknown/violation reason");
                let code = if c.conformance == Tri::Violating {
                    diag::E_LOG_2
                } else if reason.starts_with("unsupported") {
                    diag::U_LOG_1
                } else {
                    match reason {
                        "time_precision" | "ambiguous_order" | "ambiguous_approval_order" => {
                            diag::W_LOG_1
                        }
                        "partial_start"
                        | "partial_end"
                        | "scope_unspecified"
                        | "no_evidence"
                        | "no_observed_trigger"
                        | "no_observed_approval"
                        | "zero_denominator" => diag::W_LOG_2,
                        "unmapped_actor" | "unmapped_operation" => diag::W_LOG_3,
                        "event_limit"
                        | "line_limit"
                        | "order_limit"
                        | "approval_spec_limit"
                        | "approval_episode_limit" => diag::W_LOG_4,
                        _ => diag::W_LOG_0,
                    }
                };
                self.diagnostics.push(diagnostic(
                    code,
                    reason,
                    format!("{}: {reason}", c.id),
                    c.model_span,
                ));
                if code == diag::U_LOG_1 {
                    self.summary.unsupported += 1;
                }
            }
        }
        for scope in scopes {
            let states: Vec<_> = self
                .checks
                .iter()
                .filter(|c| c.case_id.as_deref().is_none_or(|id| id == scope.case_id))
                .map(|c| c.conformance)
                .collect();
            let mut counts = Counts::default();
            for &s in &states {
                counts.add(s);
            }
            // Unread records may overturn Task correspondence even for a
            // positive prefix violation. Keep check counts as observations,
            // but do not certify any case from an incomplete input.
            let conformance = if self.limits.event_limit || self.limits.line_limit {
                Tri::Unknown
            } else {
                Tri::fold(states)
            };
            self.summary.cases.add(conformance);
            self.cases.push(Case {
                case_id: scope.case_id.clone(),
                conformance,
                checks: counts,
            });
        }
        self.cases.sort_by(|a, b| a.case_id.cmp(&b.case_id));
        self.summary.unmapped_actor = self.unmapped_actor.len();
        self.summary.unmapped_operation = self.unmapped_operation.len();
        if self.limits.event_limit || self.limits.line_limit {
            self.conformance = Tri::Unknown;
            self.verdict = Verdict::Inconclusive;
            return;
        }
        self.conformance = Tri::fold(self.checks.iter().map(|c| c.conformance));
        self.verdict = match self.conformance {
            Tri::Violating => Verdict::Violated,
            Tri::Unknown if self.summary.unsupported > 0 => Verdict::Unsupported,
            Tri::Unknown => Verdict::Inconclusive,
            Tri::Conformant => Verdict::Satisfied,
        };
    }
}

/// Parse/lower only. Neither the state engine nor the reference interpreter nor
/// simulation is used to certify observations as an execution of the process.
pub fn conform(
    src: &str,
    reader: impl Read,
    configuration: Option<&str>,
    scenario: Option<&str>,
) -> ConformanceReport {
    conform_raw(src, reader, configuration, scenario, None).seal()
}
/// ADR 0061's composite observed-event scope. Sidecar reading and analysis have
/// no access to any execution engine; the old entry point remains unchanged.
pub fn conform_approval(
    src: &str,
    reader: impl Read,
    mut approval_spec: impl Read,
    configuration: Option<&str>,
    scenario: Option<&str>,
) -> ConformanceReport {
    conform_raw(
        src,
        reader,
        configuration,
        scenario,
        Some(&mut approval_spec),
    )
    .seal()
}
fn conform_raw(
    src: &str,
    reader: impl Read,
    configuration: Option<&str>,
    scenario: Option<&str>,
    approval_reader: Option<&mut dyn Read>,
) -> ConformanceReport {
    let mut r = ConformanceReport {
        configuration: configuration.map(String::from),
        scenario: scenario.map(String::from),
        ..ConformanceReport::default()
    };
    if approval_reader.is_some() {
        r = r.approval_scope();
    }
    r.ledger = crate::ledger::Ledger::conformance(false);
    r.ledger.requested(configuration, scenario);
    r.model.sha256 = Some(
        Sha256::digest(src.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    );
    let ast = match syntax::parse(src) {
        Ok(ast) => ast,
        Err(ds) => {
            r.verdict = report::fold_verdict(&[], &ds);
            r.diagnostics = ds.into_iter().map(Into::into).collect();
            return r;
        }
    };
    r.model.name = Some(ast.name.value.clone());
    let q1_features = q1_scope::declarations(&ast);
    let m = match lower::lower_with_selection(&ast, configuration, scenario) {
        Ok(m) => m,
        Err(ds) => {
            r.verdict = report::fold_verdict(&[], &ds);
            r.diagnostics = ds.into_iter().map(Into::into).collect();
            for f in &q1_features {
                r.diagnostics.push(diagnostic(
                    diag::U_LOG_1,
                    "unsupported_q1_model_feature",
                    format!("{} is outside observable_event_contracts_v1", f.item),
                    Some(f.span),
                ));
                r.not_checked.push(NotChecked {
                    item: f.item.clone(),
                    reason: "unsupported_request",
                    model_span: Some(f.span),
                });
            }
            return r;
        }
    };
    r.ledger.lowered(&m);
    r.ledger.execution["bounds"] = crate::ledger::known(
        json!({"cases":m.bounds.cases,"live_tasks":m.bounds.live_tasks,"clock":m.bounds.clock}),
    );
    r.ledger.execution["limits"]["state_budget"] = crate::ledger::na("no_analysis");
    if m.has_merges() {
        r.verdict = Verdict::Unsupported;
        r.diagnostics.push(diagnostic(
            diag::U_LOG_1,
            "unsupported_merge_model_feature",
            "merge_feature_combination: merged values are not observable in event logs",
            Some(m.process.span),
        ));
        return r;
    }
    if m.has_loops() {
        r.verdict = Verdict::Unsupported;
        r.diagnostics.push(diagnostic(
            diag::U_LOG_1,
            "unsupported_loop_model_feature",
            "loop_feature_combination: repeated Task log matching is not connected",
            Some(m.process.span),
        ));
        return r;
    }
    if m.uses_work_profiles() {
        r.verdict = crate::Verdict::Unsupported;
        r.diagnostics.push(diagnostic(
            diag::U_SEM_0,
            "q6_feature_combination",
            "q6_feature_combination: work profiles are outside observable event conformance",
            None,
        ));
        return r;
    }
    if !m.process.degradations.is_empty() {
        r.verdict = Verdict::Unsupported;
        r.diagnostics.push(diagnostic(
            diag::U_SEM_0,
            "q6_feature_combination",
            "degradation conformance not connected",
            None,
        ));
        return r;
    }
    if let Some(input) = &m.case_priority {
        for item in [
            "priority_starts",
            "priority_external_readmission",
            "priority_version_resolution",
        ] {
            r.not_checked.push(NotChecked {
                item: item.into(),
                reason: "priority_order_not_in_event_contract_scope",
                model_span: Some(input.span),
            });
        }
    }
    if m.has_condition_wait() {
        r.not_checked.push(NotChecked {
            item: "condition_wait_observations".into(),
            reason: "condition_observations_not_in_log_schema",
            model_span: None,
        });
    }
    if m.resource_time.is_some() {
        r.not_checked.push(NotChecked {
            item: "calendar_start_gates_and_working_hours".into(),
            reason: "calendar_observations_not_in_log_schema",
            model_span: None,
        });
    }
    let (q1_features, q1_capabilities) = q1_scope::selected(q1_features, &m);
    r.cut = m.cut.as_ref().map_or(report::Cut::UNDECLARED, |c| {
        report::Cut::Declared(Box::new(report::declared_cut(&m, c)))
    });
    r.cut_context = report::CutContext::resolved(&m);
    r.diagnostics = m.diagnostics.iter().cloned().map(Into::into).collect();
    r.assumptions = m
        .assumptions
        .iter()
        .map(|a| {
            json!({
                "model_assumption": a,
                "status": "model_declaration_or_default_not_log_verified"
            })
        })
        .collect();
    r.assumptions.push(json!({"scope":"observable_event_contracts_v1","notice":"Observations and endpoint declarations do not certify log completeness, latent truth, Standing, future probabilities or process conformance; no model or kernel is updated."}));
    let mut log = input::Log::default();
    let observed = match approval_reader.map(approval::read).transpose() {
        Ok(v) => v,
        Err(f) => {
            r.fail(f);
            return r;
        }
    };
    r.ledger.execution["status"] = "ran".into();
    r.ledger.backends["builtin"]["status"] = "ran".into();
    let input_failure = input::read_into(reader, &mut log).err();
    r.log = LogMetadata {
        sha256: log.sha256.clone(),
        lines: log.lines,
        processed_lines: log.processed_lines,
        events: log.events.len(),
        clock: log.header.as_ref().map(|h| h.clock.clone()),
        time_unit: log.header.as_ref().map(|h| h.time_unit.clone()),
        window: log.header.as_ref().map(|h| h.window),
    };
    if let Some(f) = input_failure {
        r.fail(f);
        return r;
    }
    if let Some(observed) = &observed
        && let Err(f) = observed.validate(&m, r.model.sha256.as_deref().expect("model hash"), &log)
    {
        r.fail(f);
        return r;
    }
    if let Some(reason) = log.limit {
        r.limits.event_limit = reason == "event_limit";
        r.limits.line_limit = reason == "line_limit";
        r.limits.truncated = true;
        r.scope_check("input_limit", None, reason, &log.events);
    }
    let Some(h) = &log.header else {
        r.finish(&[]);
        return r;
    };
    for f in q1_features {
        let mut c = Check::new(
            &f.item,
            None,
            Tri::Unknown,
            Some("unsupported_q1_model_feature"),
            &[],
            &log.events,
            Some(f.span),
        );
        c.model_span = Some(f.span);
        r.checks.push(c);
        r.not_checked.push(NotChecked {
            item: f.item,
            reason: "unsupported_request",
            model_span: Some(f.span),
        });
    }
    r.assumptions.push(json!({"declared_clock":h.clock,"declared_window":h.window,"declared_endpoints":h.cases,"actor_map":h.actor_map,"executor_map":h.executor_map,"status":"declared_not_certified"}));
    let resolved = match identity::resolve(&m, h, &log.events) {
        Ok(v) => v,
        Err(f) => {
            r.fail(f);
            return r;
        }
    };
    let mut order = match order::Order::source(&log.events, log.limit.is_some()) {
        Ok(v) => v,
        Err(f) => {
            r.fail(f);
            return r;
        }
    };
    let life = lifecycle::analyze_with_q1(&m, &log.events, &resolved, &q1_capabilities);
    for c in &m.capabilities {
        for field in lifecycle::features(c, &m).outside_scope {
            r.not_checked.push(NotChecked {
                item: format!("capability:{}.{}:{field}", m.actors[c.actor.0].name, c.name),
                reason: "outside_scope",
                model_span: Some(c.span),
            });
        }
    }
    if let Err(f) = order.add_semantic(&life.edges) {
        r.fail(f);
        return r;
    }
    r.checks.extend(life.checks);
    for scope in &h.cases {
        for (id, value, reason) in [
            ("from_case_start", scope.from_case_start, "partial_start"),
            ("through_case_end", scope.through_case_end, "partial_end"),
        ] {
            if value != Some(true) {
                r.scope_check(
                    id,
                    Some(&scope.case_id),
                    if value.is_none() {
                        "scope_unspecified"
                    } else {
                        reason
                    },
                    &log.events,
                );
            }
        }
        if !log.events.iter().any(|e| e.case_id == scope.case_id) {
            r.scope_check("evidence", Some(&scope.case_id), "no_evidence", &log.events);
        }
        r.checks.extend(monitors::independent_with_q1(
            &m,
            &log.events,
            &life.tasks,
            &scope.case_id,
            &q1_capabilities,
        ));
        let ordered = monitors::ordered_with_q1(
            &m,
            &log.events,
            &life.tasks,
            &scope.case_id,
            &order,
            &q1_capabilities,
        );
        r.limits.order_limit |= ordered.truncated;
        r.limits.truncated |= ordered.truncated;
        r.checks.extend(ordered.checks);
        for (i, c) in m.contracts.iter().enumerate() {
            if !monitors::supported_contract_with_q1(&c.kind, &m, &q1_capabilities) {
                r.checks.push(Check::new(
                    format!("contract:{}", c.ordinal.unwrap_or(i + 1)),
                    Some(&scope.case_id),
                    Tri::Unknown,
                    Some("unsupported_contract"),
                    &[],
                    &log.events,
                    Some(c.span),
                ));
            }
        }
    }
    if log.events.is_empty() {
        r.scope_check("evidence", None, "no_evidence", &log.events);
    }
    for (i, e) in log.events.iter().enumerate() {
        if e.time.earliest != e.time.latest {
            r.checks.push(Check::new(
                format!("precision:{}", e.event_id),
                Some(&e.case_id),
                Tri::Unknown,
                Some("time_precision"),
                &[i],
                &log.events,
                None,
            ));
        }
        if e.input_version.is_none()
            || (e.lifecycle == input::Lifecycle::Finish && e.output_version.is_none())
        {
            r.assumptions.push(json!({"missing_version":{"event_id":e.event_id,"line":e.line,"input_version":e.input_version,"output_version":e.output_version}}));
        }
    }
    for (i, v) in m.verify.iter().enumerate() {
        let unsupported = match v.property {
            ir::Property::Contract(c) => {
                !monitors::supported_contract_with_q1(&m.contracts[c.0].kind, &m, &q1_capabilities)
            }
            ir::Property::NoLostUpdate(_)
            | ir::Property::StateInRange
            | ir::Property::RecoverableEnd
            | ir::Property::TerminatingEnd
            | ir::Property::AlmostSureEnd
            | ir::Property::Contest(_)
            | ir::Property::ProbabilityReached(_)
            | ir::Property::NoResourceOvercommit
            | ir::Property::ReachableEnd(_) => true,
        };
        if unsupported && !matches!(v.property, ir::Property::Contract(_)) {
            r.not_checked.push(NotChecked {
                item: format!("verify:{}:{:?}", i + 1, v.property),
                reason: "unsupported_request",
                model_span: Some(v.span),
            });
            r.checks.push(Check::new(
                format!("verify:{}", i + 1),
                None,
                Tri::Unknown,
                Some("unsupported_verify"),
                &[],
                &log.events,
                Some(v.span),
            ));
        }
    }
    for (i, c) in m.contracts.iter().enumerate() {
        if !monitors::supported_contract_with_q1(&c.kind, &m, &q1_capabilities) {
            if matches!(c.kind, ir::ContractKind::EvidenceBasis { .. })
                && !r.unchecked.iter().any(|s| s == "evidence_basis")
            {
                r.unchecked.push("evidence_basis".into());
            }
            r.not_checked.push(NotChecked {
                item: c.property_id(ir::ClauseId(i)),
                reason: "unsupported_request",
                model_span: Some(c.span),
            });
            if h.cases.is_empty() {
                r.checks.push(Check::new(
                    format!("contract:{}", c.ordinal.unwrap_or(i + 1)),
                    None,
                    Tri::Unknown,
                    Some("unsupported_contract"),
                    &[],
                    &log.events,
                    Some(c.span),
                ));
            }
        }
    }
    for row in &r.checks {
        if row.reason.as_deref().is_some_and(|s| {
            s.starts_with("unsupported_capability") || s.starts_with("unsupported_cancel")
        }) {
            r.not_checked.push(NotChecked {
                item: row.id.clone(),
                reason: "unsupported_semantics",
                model_span: row.model_span,
            });
        }
    }
    if let Some(reason) = log.limit {
        for row in &mut r.checks {
            if (row.id.starts_with("contract:") || row.id.starts_with("independent:contract:"))
                && row.conformance == Tri::Conformant
            {
                row.conformance = Tri::Unknown;
                row.reason = Some(reason.into());
            }
        }
    }
    if let Some(observed) = observed {
        let ctx = approval::Context {
            model: &m,
            header: h,
            events: &log.events,
            tasks: &life.tasks,
            checks: &r.checks,
        };
        let out = match observed.analyze(&ctx, &order, log.limit) {
            Ok(out) => out,
            Err(f) => {
                r.fail(f);
                return r;
            }
        };
        r.approval_burden = Some(out.value);
        r.checks.extend(out.checks);
        r.limits.order_limit |= out.order_limit;
        r.limits.truncated |= out.input_limit.is_some() || out.order_limit;
    }
    r.unmapped_actor = resolved.unmapped_actor;
    r.unmapped_operation = resolved.unmapped_operation;
    r.finish(&h.cases);
    r
}

#[cfg(test)]
#[path = "../../../relca-cli/tests/support/log_conformance_inputs.rs"]
mod acceptance_inputs;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_logs_retain_known_metadata_but_no_prefix_conclusions() {
        let c = acceptance_inputs::case("wire_missing_field");
        let r = conform(&c.model, c.log().as_bytes(), None, None);
        assert_eq!(r.verdict, Verdict::Invalid);
        assert_eq!(r.log.clock.as_deref(), Some("shared-test-clock"));
        assert_eq!(r.log.processed_lines, 2);
        assert!(r.log.sha256.is_none() && r.log.lines.is_none());
        assert!(r.checks.is_empty() && r.cases.is_empty());
        let c = acceptance_inputs::case("wire_after_unknown");
        let r = conform(&c.model, c.log().as_bytes(), None, None);
        assert_eq!(r.verdict, Verdict::Invalid);
        assert!(r.log.sha256.is_some() && r.log.lines == Some(3));
        assert!(r.checks.is_empty() && r.cases.is_empty());
    }
    #[test]
    fn ambiguous_task_correspondence_is_also_independent_of_line_order() {
        for name in [
            "wire_unknown_operation",
            "l08_same_person",
            "wire_operation_actor_mismatch",
            "l11_duplicate_finish",
            "l13_ambiguous",
        ] {
            let mut c = acceptance_inputs::case(name);
            let a = conform(&c.model, c.log().as_bytes(), None, None);
            c.records.as_array_mut().unwrap()[1..].reverse();
            let b = conform(&c.model, c.log().as_bytes(), None, None);
            assert_eq!(a.verdict, b.verdict, "{name}");
            let rows = |r: ConformanceReport| {
                r.checks
                    .into_iter()
                    .map(|c| {
                        (
                            c.id,
                            c.case_id,
                            c.conformance,
                            c.reason,
                            c.model_span,
                            c.evidence
                                .into_iter()
                                .map(|e| e.event_id)
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(rows(a), rows(b), "{name}");
        }
    }
    #[test]
    fn declared_aliases_resolve_to_the_same_actor_and_person_across_phases() {
        let mut c = acceptance_inputs::case("l08_same_person_witness");
        c.records[0]["actor_map"]
            .as_array_mut()
            .unwrap()
            .push(json!({"source_id":"alias-clerk","actor":"Clerk"}));
        c.records[0]["executor_map"]
            .as_array_mut()
            .unwrap()
            .push(json!({"source_id":"alias-p","person":"p"}));
        c.records[2]["actor_id"] = json!("alias-clerk");
        c.records[2]["executor_id"] = json!("alias-p");
        let r = conform(&c.model, c.log().as_bytes(), None, None);
        assert_eq!(r.verdict, Verdict::Violated);
        assert!(
            r.checks
                .iter()
                .any(|r| r.reason.as_deref() == Some("separation")
                    && r.conformance == Tri::Violating)
        );
        assert!(
            !r.checks
                .iter()
                .any(|r| r.reason.as_deref() == Some("task_identity_mismatch"))
        );
    }
    #[test]
    fn a_missing_result_is_not_a_missing_person_witness() {
        let mut c = acceptance_inputs::case("l08_same_person_witness");
        c.records[2]["result"] = Value::Null;
        let r = conform(&c.model, c.log().as_bytes(), None, None);
        assert_eq!(r.verdict, Verdict::Violated);
        assert!(
            r.checks
                .iter()
                .any(|r| r.reason.as_deref() == Some("missing_result"))
        );
        assert!(
            r.checks
                .iter()
                .any(|r| r.reason.as_deref() == Some("separation")
                    && r.conformance == Tri::Violating)
        );
    }
    #[test]
    fn unsupported_state_effects_do_not_hide_independent_person_or_duration_witnesses() {
        for (name, capability, reason) in [
            (
                "l08_same_person_witness",
                "enter() -> Verdict",
                "separation",
            ),
            ("l23_duration_violation", "post() -> Ack", "duration"),
        ] {
            let mut c = acceptance_inputs::case(name);
            c.model = c.model.replacen(
                "people [p,q]",
                "observed case { marker: bool = false } people [p,q]",
                1,
            );
            let key = format!("capability {capability} {{");
            c.model = c.model.replacen(
                &key,
                &format!("{key} writes marker; finish {{ marker = true }}"),
                1,
            );
            let r = conform(&c.model, c.log().as_bytes(), None, None);
            assert_eq!(r.verdict, Verdict::Violated, "{name}: {r:?}");
            assert!(r.checks.iter().any(|r| r.reason.as_deref()==Some(reason) && r.conformance==Tri::Violating));
            assert!(
                r.summary.checks.unknown > 0
                    && r.diagnostics.iter().any(|d| d.code == diag::U_LOG_1)
            );
        }
    }
    #[test]
    fn run_wide_order_limit_retains_an_independent_person_witness() {
        let mut c = acceptance_inputs::case("l13_ambiguous");
        for i in 0..11 {
            let id = format!("extra{i}");
            c.apply(&acceptance_inputs::Edit::AppendTask(vec![
                acceptance_inputs::event(&id, "Approver.approve", "start", 1, "person-q"),
                acceptance_inputs::event(&id, "Approver.approve", "finish", 2, "person-q"),
            ]));
        }
        let r = conform(&c.model, c.log().as_bytes(), None, None);
        assert_eq!(r.verdict, Verdict::Inconclusive);
        assert!(r.limits.truncated && r.limits.order_limit);
        assert!(r.diagnostics.iter().any(|d| d.code == diag::W_LOG_4));
        c.apply(&acceptance_inputs::Edit::Contract(
            "separation Clerk.enter.finish, Approver.approve.finish",
        ));
        c.apply(&acceptance_inputs::Edit::AssignTask {
            task: "a",
            to: "person-p",
        });
        let r = conform(&c.model, c.log().as_bytes(), None, None);
        assert_eq!(r.verdict, Verdict::Violated);
        assert!(r.limits.truncated && r.summary.checks.unknown > 0);
    }
    #[test]
    fn core_matches_the_frozen_hand_expectations_without_running_the_engine() {
        let names = [
            "l01_serial_ok",
            "l02_missing_start",
            "l03_missing_finish",
            "l04_time_precision",
            "l05_partial_start",
            "l05_unspecified_start",
            "l06_partial_end",
            "l07_missing_approval",
            "l07_failed_approval",
            "l08_separation_ok",
            "l08_same_person",
            "l08_distinct_person_control",
            "l08_same_person_witness",
            "l09_once_ok",
            "l09_duplicate_commit",
            "l10_distinct_cases",
            "l10_reused_task_id",
            "l11_duplicate_finish",
            "l12_excludes_violation",
            "l13_ambiguous",
            "l14_forward",
            "l14_reverse",
            "l15_unmapped_actor",
            "l16_unmapped_person",
            "l17_cancel_ok",
            "l17_request_only",
            "l17_missing_request",
            "l17_cancel_without_explicit_edge",
            "l17_finish_approval_control",
            "l17_ack_not_approval",
            "l18_forbidden_ack",
            "l19_bad_mapping",
            "l20_future_schema",
            "l21_response_unsupported",
            "l21_independent_violation",
            "l22_empty",
            "l23_duration_violation",
            "l23_event_boundary",
            "l23_event_limit",
            "l23_line_boundary",
            "l23_line_limit",
            "wire_missing_field",
            "wire_unknown_field",
            "wire_duplicate_key",
            "wire_duplicate_event_id",
            "wire_after_unknown",
            "wire_after_cycle",
            "wire_after_time_contradiction",
            "wire_reversed_lifecycle",
            "wire_outcome_domain",
            "wire_result_type",
            "wire_window",
            "wire_unknown_operation",
            "wire_cut_undeclared",
            "wire_null_version",
            "wire_equal_time_requires",
            "wire_future_unit",
            "wire_no_requests",
            "wire_explicit_task_edge",
            "wire_source_cycle",
            "wire_no_implicit_actor_match",
            "wire_no_implicit_person_match",
            "wire_operation_actor_mismatch",
        ];
        for name in names {
            let c = acceptance_inputs::case(name);
            let (r, counts) = crate::evaluation_spy::observe(|| {
                conform(&c.model, c.log().as_bytes(), None, None)
            });
            assert!(
                counts[1..].iter().all(|n| *n == 0),
                "conform must not call any execution engine"
            );
            assert_eq!(r.verdict.exit_code(), c.expected.exit, "{name}: {r:?}");
            assert_eq!(
                serde_json::to_value(r.conformance).unwrap(),
                c.expected.conformance,
                "{name}: three-valued result"
            );
            if let Some(code) = c.expected.code {
                assert!(
                    r.diagnostics.iter().any(|d| d.code == code),
                    "{name}: missing code {code}"
                );
            }
            if let Some(reason) = c.expected.reason {
                assert!(
                    r.checks.iter().any(|c| c.reason.as_deref() == Some(reason)),
                    "{name}: missing reason {reason}"
                );
            }
        }
    }
}
