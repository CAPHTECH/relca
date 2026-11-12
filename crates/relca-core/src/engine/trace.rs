//! Optional, driver-owned labelled graph. No execution history enters state keys.
use super::*;
use crate::explain::{self as x, *};
use std::collections::VecDeque;
#[cfg(test)]
#[path = "../../tests/support/actor_observed_private_trace.rs"]
mod actor_observed_private;
#[cfg(test)]
#[path = "../../tests/support/caseset_private_trace.rs"]
mod caseset_private;
#[cfg(test)]
#[path = "trace_policy_tests.rs"]
mod fixed_policy_tests;
pub(crate) type WitnessPath = (Vec<Stage>, Option<u64>, Vec<Option<KernelChoice>>);
// Property, case, binding (None = receipt), trigger time, deadline. These are
// path-local ledger facts attached to graph edges, never semantic state keys.
pub(super) type ResponseOrigin = (String, u64, Option<String>, u64, u64);

#[derive(Debug, Clone)]
pub(crate) struct Edge {
    pub to: usize,
    pub stage: Stage,
    pub targets: Vec<String>,
    pub cases: BTreeMap<String, u64>,
    pub choice: Option<KernelChoice>,
}
#[derive(Debug, Clone)]
pub(crate) struct Capture {
    src: String,
    line_starts: Vec<usize>,
    recorded_edges: usize,
    pub(super) spans: Vec<diag::Span>,
    calls: BTreeMap<ir::BindingId, ir::Call>,
    pub current: usize,
    pub initial: Option<Stage>,
    pub edges: Vec<Vec<Edge>>,
    aliases: Vec<usize>,
    pub failure: Option<ReplayReason>,
    pub(super) arrival_span: Option<diag::Span>,
    pub(super) case_id: u64,
    initial_targets: Vec<String>,
    pub(super) shared_resources: Option<Vec<Resource>>,
    pub(super) shared_model_resources: Option<Vec<SharedResource>>,
    pub(super) kernel_choice: Option<KernelChoice>,
    generated_choices: Vec<(KernelChoice, Stage)>,
    response_origins: BTreeMap<usize, Vec<ResponseOrigin>>,
    pub(crate) capture_limit: usize,
    state_details: BTreeMap<(usize, String), StateInvariantDetail>,
}
impl Capture {
    pub(crate) fn new(src: &str, model: &ir::Model) -> Self {
        fn walk(
            block: &ir::Block,
            spans: &mut Vec<diag::Span>,
            calls: &mut BTreeMap<ir::BindingId, ir::Call>,
        ) {
            for stmt in block.iter().rev() {
                match &stmt.kind {
                    ir::Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } => {
                        walk(then_body, spans, calls);
                        walk(else_body, spans, calls);
                    }
                    ir::Stmt::Response(call, r) => {
                        calls.insert(call.binding, call.clone());
                        if r.attempt < r.max_attempts {
                            spans.push(r.retry_span.unwrap_or(stmt.span));
                        }
                        if r.exhausted.is_some() {
                            spans.push(r.retry_span.unwrap_or(stmt.span));
                        }
                        spans.push(stmt.span);
                    }
                    ir::Stmt::Call(call) => {
                        calls.insert(call.binding, call.clone());
                    }
                    ir::Stmt::Fork { branches, .. } => {
                        for call in branches {
                            calls.insert(call.binding, call.clone());
                        }
                    }
                    ir::Stmt::End(_) | ir::Stmt::Wait(_) | ir::Stmt::WaitUntil(_) => {}
                }
                spans.push(stmt.span);
            }
        }
        let mut spans = vec![model.process.span];
        let mut calls = BTreeMap::new();
        walk(&model.process.body, &mut spans, &mut calls);
        let arrival_span = crate::syntax::parse(src).ok().and_then(|ast| {
            ast.items.into_iter().find_map(|item| match item {
                crate::ast::Item::ArrivalsFixed(a) => Some(a.span),
                _ => None,
            })
        });
        Self {
            src: src.into(),
            line_starts: Location::line_starts(src),
            recorded_edges: 0,
            spans,
            calls,
            current: 0,
            initial: None,
            edges: vec![Vec::new()],
            aliases: vec![0],
            failure: None,
            arrival_span,
            case_id: 1,
            initial_targets: Vec::new(),
            shared_resources: None,
            shared_model_resources: None,
            kernel_choice: None,
            generated_choices: Vec::new(),
            response_origins: BTreeMap::new(),
            capture_limit: 200_000,
            state_details: BTreeMap::new(),
        }
    }
    pub(crate) fn location(&self, span: diag::Span) -> Result<Location, ReplayReason> {
        Location::indexed(&self.src, &self.line_starts, span)
    }
    fn values(&self, state: &State, m: &ir::Model) -> Result<Values, ReplayReason> {
        let mut values = Values::default();
        for (var, value) in m.state_variables.iter().zip(&state.state_values) {
            let observed = var.kind.is_observed();
            let value = Value {
                name: match var.kind {
                    ir::StateKind::ActorObserved(a) | ir::StateKind::Rule(a) => {
                        format!("{}.{}", m.actors[a.0].name, var.name)
                    }
                    _ => var.name.clone(),
                },
                value: match value {
                    ir::StateValue::Set(bits) => crate::explain::DisplayValue::CaseSet(
                        (0..64)
                            .filter(|i| bits & (1u64 << i) != 0)
                            .map(|i| i + 1)
                            .collect(),
                    ),
                    ir::StateValue::Value(v) => m.types[var.base.0].members[v.0].clone().into(),
                    ir::StateValue::None => "none".into(),
                    ir::StateValue::Integer(v) => v.to_string().into(),
                },
                visibility: if observed {
                    Visibility::Observed
                } else {
                    Visibility::Latent
                },
                location: self.location(var.span)?,
            };
            if observed {
                values.observed.push(value);
            } else {
                values.latent.push(value);
            }
        }
        Ok(values)
    }
    pub(super) fn stage(
        &self,
        s: &State,
        m: &ir::Model,
        kind: Kind,
    ) -> Result<Stage, ReplayReason> {
        Ok(Stage {
            command: None,
            attempt: None,
            observed_receipt: None,
            actual_result: None,
            retry_location: None,
            shared_resources: Vec::new(),
            state_range_violation: None,
            index: 0,
            time: s.time,
            case_id: Some(self.case_id),
            binding: None,
            actor: None,
            capability: None,
            executor: None,
            result: None,
            kind,
            events: Vec::new(),
            location: self.location(m.process.span)?,
            values: self.values(s, m)?,
            violation: None,
            resource: None,
            capability_location: None,
            deadlock_detail: None,
        })
    }
    pub(super) fn initial(&mut self, s: &State, m: &ir::Model) {
        self.initial_targets = Self::targets(s, m);
        self.capture_state_details(0, s, m, self.case_id);
        match self.stage(s, m, Kind::Initial) {
            Ok(mut stage) => {
                if s.initialized {
                    stage
                        .events
                        .push(Event::Business(Self::event(&stage, Kind::Received)));
                }
                self.initial = Some(stage);
            }
            Err(reason) => self.failure = Some(reason),
        }
    }
    pub(super) fn capture_state_details(
        &mut self,
        node: usize,
        state: &State,
        m: &ir::Model,
        case: u64,
    ) {
        if !state.initialized || self.failure == Some(ReplayReason::CaptureLimit) {
            return;
        }
        fn reads(c: &ir::Cond, ids: &mut BTreeSet<ir::StateVarId>) {
            match c {
                ir::Cond::StateEq(t) => {
                    ids.insert(t.variable);
                }
                ir::Cond::IntCompare { variable, .. }
                | ir::Cond::Contains { set: variable, .. } => {
                    ids.insert(*variable);
                }
                ir::Cond::And(cs) | ir::Cond::Or(cs) => {
                    for c in cs {
                        reads(c, ids);
                    }
                }
                ir::Cond::Eq(_) => {}
            }
        }
        for (i, clause) in m.contracts.iter().enumerate() {
            let ir::ContractKind::StateInvariant {
                antecedent,
                condition: consequent,
            } = &clause.kind
            else {
                continue;
            };
            let detail = (|| {
                // The explicit case context of membership atoms (ADR0035).
                let k = usize::try_from(case.checked_sub(1).ok_or(ReplayReason::StateMismatch)?)
                    .map_err(|_| ReplayReason::StateMismatch)?;
                let a = match antecedent {
                    Some(c) => condition_at(c, &state.results, &state.state_values, k),
                    None => Ok(true),
                }
                .map_err(|_| ReplayReason::StateMismatch)?;
                let b = condition_at(consequent, &state.results, &state.state_values, k)
                    .map_err(|_| ReplayReason::StateMismatch)?;
                if !a || b {
                    return Ok(None);
                }
                let mut ids = BTreeSet::new();
                if let Some(c) = antecedent {
                    reads(c, &mut ids);
                }
                reads(consequent, &mut ids);
                let values = ids
                    .into_iter()
                    .map(|id| {
                        let var = &m.state_variables[id.0];
                        Ok(Value {
                            name: match var.kind {
                                ir::StateKind::ActorLatent(a)
                                | ir::StateKind::ActorObserved(a)
                                | ir::StateKind::Rule(a) => {
                                    format!("{}.{}", m.actors[a.0].name, var.name)
                                }
                                _ => var.name.clone(),
                            },
                            value: match state.state_values[id.0] {
                                ir::StateValue::None => "none".into(),
                                ir::StateValue::Value(v) => {
                                    m.types[var.base.0].members[v.0].clone().into()
                                }
                                ir::StateValue::Integer(n) => n.to_string().into(),
                                ir::StateValue::Set(bits) => crate::explain::DisplayValue::CaseSet(
                                    (0..64)
                                        .filter(|i| bits & (1u64 << i) != 0)
                                        .map(|i| i + 1)
                                        .collect(),
                                ),
                            },
                            visibility: if var.kind.is_observed() {
                                Visibility::Observed
                            } else {
                                Visibility::Latent
                            },
                            location: self.location(var.span)?,
                        })
                    })
                    .collect::<Result<Vec<_>, ReplayReason>>()?;
                Ok(Some(StateInvariantDetail {
                    case_id: case,
                    antecedent: a,
                    condition: b,
                    values,
                }))
            })();
            match detail {
                Ok(Some(d)) => {
                    self.state_details
                        .entry((node, clause.property_id(ir::ClauseId(i))))
                        .or_insert(d);
                }
                Ok(None) => {}
                Err(e) => {
                    self.failure.get_or_insert(e);
                }
            }
        }
    }

    fn attach_state_detail(&self, node: usize, target: &str, stage: &mut Stage) {
        if let Some(detail) = self.state_details.get(&(node, target.to_owned())) {
            stage.violation = Some(Violation {
                state_invariant: Some(detail.clone()),
                id: target.into(),
                property_location: None,
                at_step: stage.index,
                trigger_step: None,
                deadline: None,
                related_steps: Vec::new(),
            });
        }
    }
    pub(super) fn event(stage: &Stage, kind: Kind) -> BusinessEvent {
        BusinessEvent {
            kind,
            case_id: stage.case_id,
            binding: stage.binding.clone(),
            actor: stage.actor.clone(),
            capability: stage.capability.clone(),
            executor: stage.executor.clone(),
            result: stage.actual_result.clone().or_else(|| stage.result.clone()),
            location: stage.location.clone(),
            ack_at: None,
        }
    }
    fn resource(&self, s: &State, actor: ir::ActorId, m: &ir::Model) -> Resource {
        let mut resource = Resource {
            actor: m.actors[actor.0].name.clone(),
            capacity: m.actors[actor.0].capacity.unwrap_or(0),
            anonymous: m.actors[actor.0].executors.is_none(),
            holders: s
                .tasks
                .iter()
                .filter(|t| t.lifecycle.running() && m.capabilities[t.capability.0].actor == actor)
                .map(|t| Holder {
                    case_id: self.case_id,
                    binding: m.site_name(t.binding),
                    executor: t.executor.map(|p| m.people[p.0].name.clone()),
                })
                .collect(),
        };
        if let Some(resources) = &self.shared_resources
            && let Some(shared) = resources.iter().find(|r| r.actor == resource.actor)
        {
            resource.holders.extend(
                shared
                    .holders
                    .iter()
                    .filter(|h| h.case_id != self.case_id)
                    .cloned(),
            );
            resource
                .holders
                .sort_by_key(|h| (h.case_id, h.binding.clone()));
        }
        resource
    }
    pub(super) fn model_resources(
        &self,
        s: &State,
        m: &ir::Model,
    ) -> Result<Vec<SharedResource>, ReplayReason> {
        m.resources
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let mut holders = Vec::new();
                for t in s.tasks.iter().filter(|t| t.lifecycle.running()) {
                    for u in &m.capabilities[t.capability.0].uses {
                        if u.resource.0 == i {
                            holders.push(SharedHolder {
                                case_id: self.case_id,
                                binding: m.site_name(t.binding),
                                amount: u.amount,
                            });
                        }
                    }
                }
                if let Some(global) = &self.shared_model_resources {
                    holders.extend(
                        global[i]
                            .holders
                            .iter()
                            .filter(|h| h.case_id != self.case_id)
                            .cloned(),
                    );
                }
                holders.sort_by_key(|h| (h.case_id, h.binding.clone()));
                let occupied = holders
                    .iter()
                    .try_fold(0u64, |n, h| n.checked_add(h.amount))
                    .ok_or(ReplayReason::IllegalTransition)?;
                Ok(SharedResource {
                    resource: r.name.clone(),
                    capacity: r.capacity,
                    occupied,
                    holders,
                })
            })
            .collect()
    }

    /// A use without its resource row (no table, or a short one) is not
    /// displayable; the caller turns it into an unsupported explanation.
    pub(super) fn shortages(
        cap: &ir::Capability,
        rows: &[SharedResource],
    ) -> Result<Vec<ResourceShortage>, ReplayReason> {
        let mut uses: Vec<_> = cap.uses.iter().collect();
        uses.sort_by_key(|u| u.resource);
        let mut shortages = Vec::new();
        for u in uses {
            let r = rows.get(u.resource.0).ok_or(ReplayReason::StateMismatch)?;
            let available = r.capacity.saturating_sub(r.occupied);
            if u.amount > available {
                shortages.push(ResourceShortage {
                    resource: r.resource.clone(),
                    required: u.amount,
                    available,
                    missing: u.amount - available,
                });
            }
        }
        Ok(shortages)
    }

    fn task_stage(
        &self,
        s: &State,
        t: &Task,
        m: &ir::Model,
        kind: Kind,
    ) -> Result<Stage, ReplayReason> {
        let mut stage = self.stage(s, m, kind)?;
        let cap = &m.capabilities[t.capability.0];
        if matches!(
            kind,
            Kind::Queue
                | Kind::Start
                | Kind::Finish
                | Kind::CancelRequest
                | Kind::CancelAck
                | Kind::Refuse
        ) {
            stage.shared_resources = self
                .model_resources(s, m)?
                .into_iter()
                .enumerate()
                .filter(|(i, _)| cap.uses.iter().any(|u| u.resource.0 == *i))
                .map(|(_, r)| r)
                .collect();
        }
        let span = if let Some(id) = m.site_contest(t.binding) {
            m.contests()[id].span
        } else {
            self.calls
                .get(&t.binding)
                .ok_or(ReplayReason::SourceMappingFailed)?
                .span
        };
        stage.location = self.location(span)?;
        stage.binding = Some(m.site_name(t.binding));
        if let Some(r) = m.response_attempt(t.binding) {
            stage.command = Some(m.site_name(r.command));
            stage.attempt = Some(r.attempt);
            stage.retry_location = r.retry_span.map(|span| self.location(span)).transpose()?;
            stage.observed_receipt = s
                .response
                .as_ref()
                .and_then(|h| h.rows.iter().find(|row| row.binding == t.binding))
                .map(|row| row.receipt);
        }
        stage.actor = Some(m.actors[cap.actor.0].name.clone());
        stage.capability = Some(cap.name.clone());
        if kind != Kind::Queue {
            stage.executor = t.executor.map(|p| m.people[p.0].name.clone());
            stage.capability_location = Some(self.location(cap.span)?);
            stage.resource = Some(self.resource(s, cap.actor, m));
        }
        if kind == Kind::Finish || kind == Kind::Refuse {
            let result = if kind == Kind::Refuse {
                s.results.get(t.binding.0).copied().flatten().or_else(|| {
                    m.site_contest(t.binding)
                        .and_then(|_| cap.requires.as_ref().map(|r| r.otherwise))
                })
            } else if let Some(id) = m.site_contest(t.binding) {
                s.contest.as_ref().and_then(|c| c.routes[id].result)
            } else {
                s.response
                    .as_ref()
                    .and_then(|h| h.actuals.get(&t.binding).copied())
                    .or(s.results[t.binding.0])
            };
            stage.result = result.map(|v| m.types[cap.return_type.0].members[v.0].clone());
            if m.response_attempt(t.binding).is_some() && kind == Kind::Finish {
                stage.actual_result = stage.result.clone();
                if stage.observed_receipt == Some(crate::ast::Receipt::Unknown) {
                    stage.result = None;
                }
            }
            if kind == Kind::Refuse {
                stage.capability_location = Some(
                    self.location(
                        cap.requires
                            .as_ref()
                            .ok_or(ReplayReason::IllegalTransition)?
                            .span,
                    )?,
                );
            }
            if kind == Kind::Finish && m.site_contest(t.binding).is_some() {
                stage.kind = Kind::ContestResponse;
            }
        }
        stage
            .events
            .push(Event::Business(Self::event(&stage, kind)));
        if kind == Kind::Finish
            && stage
                .actual_result
                .as_ref()
                .or(stage.result.as_ref())
                .and_then(|r| {
                    m.types[cap.return_type.0]
                        .members
                        .iter()
                        .position(|v| v == r)
                })
                .is_some_and(|r| cap.commits_result(ir::ValueId(r)))
        {
            stage
                .events
                .push(Event::Business(Self::event(&stage, Kind::Commit)));
        }
        Ok(stage)
    }
    pub(super) fn classify(
        &self,
        a: &State,
        b: &State,
        m: &ir::Model,
    ) -> Result<Stage, ReplayReason> {
        if !a.initialized && b.initialized {
            let mut stage = self.stage(b, m, Kind::Initialize)?;
            stage.events.extend(
                stage
                    .values
                    .observed
                    .iter()
                    .chain(&stage.values.latent)
                    .cloned()
                    .map(Event::Value),
            );
            stage
                .events
                .push(Event::Business(Self::event(&stage, Kind::Received)));
            return Ok(stage);
        }
        if a.time != b.time {
            let mut stage = self.stage(b, m, Kind::Advance)?;
            stage.case_id = None;
            return Ok(stage);
        }
        if a.pc == b.pc
            && a.metric_history
                .as_ref()
                .and_then(|h| h.conditions.as_ref())
                != b.metric_history
                    .as_ref()
                    .and_then(|h| h.conditions.as_ref())
        {
            let mut stage = self.stage(b, m, Kind::Wait)?;
            stage.location = self.location(self.spans[a.pc])?;
            return Ok(stage);
        }
        if a.sleep_until != b.sleep_until {
            let mut stage = self.stage(
                b,
                m,
                if b.sleep_until.is_some() {
                    Kind::Wait
                } else {
                    Kind::Wake
                },
            )?;
            stage.location = self.location(self.spans[a.pc])?;
            return Ok(stage);
        }
        if let (Some(old), Some(new)) = (&a.contest, &b.contest) {
            if old.business.is_none() && new.business.is_some() {
                let mut stage = self.stage(b, m, Kind::End)?;
                stage.location = self.location(self.spans[a.pc])?;
                stage
                    .events
                    .push(Event::Business(Self::event(&stage, Kind::End)));
                return Ok(stage);
            }
            for (id, (old, new)) in old.routes.iter().zip(&new.routes).enumerate() {
                let kind = if old.filed != new.filed {
                    new.filed.map(|filed| {
                        if filed {
                            Kind::ContestFiled
                        } else {
                            Kind::ContestDeclined
                        }
                    })
                } else if old.queued != new.queued {
                    Some(Kind::ContestQueue)
                } else {
                    None
                };
                if let Some(kind) = kind {
                    return self.task_stage(
                        b,
                        &Task {
                            frozen_performance: None,
                            stale: Vec::new(),
                            binding: m.contest_site(id),
                            capability: m.contests()[id].route,
                            executor: None,
                            lifecycle: Lifecycle::Queued,
                        },
                        m,
                        kind,
                    );
                }
            }
        }
        if !a.waiting && b.waiting {
            let added: Vec<_> = b
                .tasks
                .iter()
                .filter(|t| !a.tasks.iter().any(|old| old.binding == t.binding))
                .collect();
            if added.len() == 1 {
                return self.task_stage(b, added[0], m, Kind::Queue);
            }
            let mut stage = self.stage(b, m, Kind::Queue)?;
            stage.location = self.location(self.spans[a.pc])?;
            for t in added {
                let task = self.task_stage(b, t, m, Kind::Queue)?;
                stage.events.extend(task.events);
            }
            return Ok(stage);
        }
        for (old, new) in b.tasks.iter().filter_map(|new| {
            a.tasks
                .iter()
                .find(|old| old.binding == new.binding)
                .map(|old| (old, new))
        }) {
            if old.lifecycle == Lifecycle::Queued
                && new.lifecycle == Lifecycle::Done
                && (b.results.get(new.binding.0).is_some_and(|r| r.is_some())
                    || m.site_contest(new.binding).is_some_and(|id| {
                        b.contest
                            .as_ref()
                            .is_some_and(|c| c.routes[id].violation == Some("refused"))
                    }))
            {
                return self.task_stage(b, new, m, Kind::Refuse);
            }
            if old.lifecycle == Lifecycle::Queued && new.lifecycle.running() {
                return self.task_stage(b, new, m, Kind::Start);
            }
            if old.lifecycle.running() && new.lifecycle == Lifecycle::Done {
                return self.task_stage(
                    b,
                    new,
                    m,
                    if b.results.get(new.binding.0).is_some_and(|r| r.is_some())
                        || b.response
                            .as_ref()
                            .is_some_and(|h| h.actuals.contains_key(&new.binding))
                        || m.site_contest(new.binding).is_some_and(|id| {
                            b.contest
                                .as_ref()
                                .is_some_and(|c| c.routes[id].result.is_some())
                        })
                    {
                        Kind::Finish
                    } else {
                        Kind::CancelAck
                    },
                );
            }
        }
        let kind = if a.adopted != b.adopted && b.adopted.is_some() {
            Kind::Adopt
        } else if a.waiting && !b.waiting {
            Kind::Join
        } else if a.pc != b.pc {
            Kind::Branch
        } else {
            Kind::DurationGate
        };
        let mut stage = self.stage(b, m, kind)?;
        stage.location = self.location(self.spans[a.pc])?;
        if kind == Kind::Adopt {
            fn cancels(block: &ir::Block, winner: ir::BindingId) -> bool {
                block.iter().any(|s| match &s.kind {
                    ir::Stmt::Response(_, _) => false, // Cancellation is requested only by a fork join.
                    ir::Stmt::Fork {
                        branches,
                        join:
                            ir::Join::Any {
                                remaining: ir::Remaining::CancelThenDrain,
                                ..
                            },
                        ..
                    } => branches.iter().any(|c| c.binding == winner),
                    ir::Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } => cancels(then_body, winner) || cancels(else_body, winner),
                    _ => false,
                })
            }
            if let Some(winner) = b
                .adopted
                .and_then(|binding| b.tasks.iter().find(|t| t.binding == binding))
            {
                let task = self.task_stage(b, winner, m, Kind::Adopt)?;
                stage
                    .events
                    .push(Event::Business(Self::event(&task, Kind::Adopt)));
            }
            let mut requested = Vec::new();
            for (old, new) in b.tasks.iter().filter_map(|new| {
                a.tasks
                    .iter()
                    .find(|old| old.binding == new.binding)
                    .map(|old| (old, new))
            }) {
                let ack = |t: &Task| match t.lifecycle {
                    Lifecycle::Running { ack, .. } | Lifecycle::DistributedRunning { ack, .. } => {
                        ack
                    }
                    _ => None,
                };
                let cap = &m.capabilities[new.capability.0];
                let tracks_history = cap.version_check.is_some()
                    || cap
                        .finish_writes()
                        .any(|w| m.state_variables[w.variable.0].last_write_wins);
                let refused_request = tracks_history
                    && cap.cancel == ir::Cancel::ForbiddenAfterStart
                    && old.lifecycle.running()
                    && b.adopted.is_some_and(|winner| {
                        winner != old.binding && cancels(&m.process.body, winner)
                    });
                if ack(old).is_none() && ack(new).is_some() || refused_request {
                    let task = self.task_stage(b, new, m, Kind::CancelRequest)?;
                    let mut event = Self::event(&task, Kind::CancelRequest);
                    event.ack_at = ack(new);
                    stage.events.push(Event::Business(event));
                    // Adoption and cancellation requests are one semantic edge.
                    // For a resource-using request, the stage names the first
                    // affected task while retaining every bundled event.
                    let uses = &m.capabilities[new.capability.0].uses;
                    requested.extend(uses.iter().map(|u| u.resource.0));
                    if stage.binding.is_none() && (!uses.is_empty() || tracks_history) {
                        stage.kind = Kind::CancelRequest;
                        stage.binding = task.binding;
                        stage.actor = task.actor;
                        stage.capability = task.capability;
                        stage.executor = task.executor;
                        stage.resource = task.resource;
                        stage.capability_location = task.capability_location;
                        stage.location = task.location;
                    }
                }
            }
            // The post-transition snapshot covers every resource of every
            // requested task, not only the one the stage is named after.
            if stage.kind == Kind::CancelRequest {
                stage.shared_resources = self
                    .model_resources(b, m)?
                    .into_iter()
                    .enumerate()
                    .filter(|(i, _)| requested.contains(i))
                    .map(|(_, r)| r)
                    .collect();
            }
        }
        if kind == Kind::DurationGate {
            for (old, new) in b.tasks.iter().filter_map(|new| {
                a.tasks
                    .iter()
                    .find(|old| old.binding == new.binding)
                    .map(|old| (old, new))
            }) {
                if let (
                    Lifecycle::DistributedRunning {
                        next: old_next,
                        due: old_due,
                        ..
                    },
                    Lifecycle::DistributedRunning { next, due, .. },
                ) = (old.lifecycle, new.lifecycle)
                    && (next != old_next || due != old_due)
                {
                    let task = self.task_stage(b, new, m, Kind::DurationGate)?;
                    stage.events.push(Event::Business(Self::event(
                        &task,
                        if due {
                            Kind::Finish
                        } else {
                            Kind::DurationGate
                        },
                    )));
                }
            }
        }
        Ok(stage)
    }
    pub(super) fn targets(s: &State, m: &ir::Model) -> Vec<String> {
        let mut targets: Vec<_> = s
            .monitors
            .iter()
            .zip(&m.contracts)
            .enumerate()
            .filter(|(_, (monitor, _))| monitor.violated)
            .map(|(i, (_, c))| c.property_id(ir::ClauseId(i)))
            .collect();
        let mut people = BTreeSet::new();
        targets.extend(s.lost_updates.iter().map(|id| m.loss_property_id(*id)));
        if s.occupancy(m)
            .iter()
            .zip(&m.actors)
            .any(|(n, a)| a.capacity.is_some_and(|capacity| *n > capacity))
            || s.tasks
                .iter()
                .filter(|t| t.lifecycle.running())
                .filter_map(|t| t.executor)
                .any(|p| !people.insert(p))
        {
            targets.push("safety no_resource_overcommit".into());
        }
        targets
    }
    pub(super) fn add(
        &mut self,
        stage: Result<Stage, ReplayReason>,
        targets: Vec<String>,
    ) -> usize {
        if self.failure == Some(ReplayReason::CaptureLimit) {
            return 0;
        }
        if stage.is_ok() && self.recorded_edges == self.capture_limit {
            self.failure = Some(ReplayReason::CaptureLimit);
            self.edges = vec![Vec::new()];
            self.aliases = vec![0];
            self.initial = None;
            self.initial_targets.clear();
            self.response_origins.clear();
            self.state_details.clear();
            self.generated_choices.clear();
            self.current = 0;
            return 0;
        }
        let to = self.edges.len();
        self.edges.push(Vec::new());
        self.aliases.push(to);
        match stage {
            Ok(stage) => {
                self.recorded_edges += 1;
                let cases = targets
                    .iter()
                    .map(|id| (id.clone(), self.case_id))
                    .collect();
                self.edges[self.current].push(Edge {
                    to,
                    stage,
                    targets,
                    cases,
                    choice: self.kernel_choice.clone(),
                });
            }
            Err(reason) => self.failure = Some(reason),
        }
        to
    }
    pub(super) fn transition(&mut self, a: &State, b: &State, m: &ir::Model) -> usize {
        if self.failure == Some(ReplayReason::CaptureLimit) {
            return 0;
        }
        let stage = self.classify(a, b, m);
        let node = self.add_transition(stage, Self::targets(b, m));
        self.capture_state_details(node, b, m, self.case_id);
        self.record_origins(node, Self::origins(b, m, self.case_id));
        node
    }
    pub(super) fn add_transition(
        &mut self,
        stage: Result<Stage, ReplayReason>,
        targets: Vec<String>,
    ) -> usize {
        if let Ok(stage) = &stage
            && let Some(n) = self.generated_choices.iter().position(|(_, generated)| {
                generated.case_id == stage.case_id
                    && generated.binding == stage.binding
                    && generated.result == stage.result
                    && generated.values == stage.values
            })
        {
            self.kernel_choice = Some(self.generated_choices.remove(n).0);
        }
        let node = self.add(stage, targets);
        self.kernel_choice = None;
        node
    }
    pub(super) fn begin(&mut self, node: usize) {
        if self.failure == Some(ReplayReason::CaptureLimit) {
            return;
        }
        self.current = node;
        self.generated_choices.clear();
        self.kernel_choice = None;
    }
    pub(super) fn range(
        &mut self,
        s: &State,
        task: &Task,
        m: &ir::Model,
        witness: &outcome::StateRangeViolation,
    ) {
        let stage = (|| {
            let mut stage = self.task_stage(s, task, m, Kind::RangeViolation)?;
            stage.events.clear();
            stage.result = None;
            stage.state_range_violation = Some(witness.clone());
            stage.location = self.location(witness.write_span)?;
            Ok(stage)
        })();
        self.add(stage, vec!["state_in_range".into()]);
    }
    pub(super) fn leaf(&mut self, s: &State, m: &ir::Model, kind: Kind) {
        let kind = if kind == Kind::End && s.contest.as_ref().is_some_and(|c| c.business.is_some())
        {
            Kind::ExecutionClose
        } else {
            kind
        };
        if self.failure == Some(ReplayReason::CaptureLimit) {
            return;
        }
        let mut targets = Self::targets(s, m);
        let case_id = self.case_id;
        let stage = (|| {
            let mut stage = self.stage(s, m, kind)?;
            stage.location = self.location(self.spans[s.pc])?;
            if matches!(kind, Kind::Deadlock | Kind::Deadline) {
                stage.case_id = None;
            }
            if kind == Kind::Deadline {
                stage.time = m.bounds.clock;
                stage.location = self.location(m.process.span)?;
            }
            if kind == Kind::Deadlock {
                targets.push("diagnostic:W-DEADLOCK-0".into());
                stage.location = self.location(m.process.span)?;
                let rows = self.model_resources(s, m)?;
                let waiting_tasks = s
                    .tasks
                    .iter()
                    .filter(|t| t.lifecycle == Lifecycle::Queued)
                    .map(|t| {
                        Ok(WaitingTask {
                            resource_shortages: Self::shortages(
                                &m.capabilities[t.capability.0],
                                &rows,
                            )?,
                            case_id: 1,
                            binding: m.site_name(t.binding),
                            actor: m.actors[m.capabilities[t.capability.0].actor.0]
                                .name
                                .clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, ReplayReason>>()?;
                stage.deadlock_detail = Some(DeadlockDetail {
                    shared_resources: rows
                        .into_iter()
                        .enumerate()
                        .filter(|(i, _)| {
                            s.tasks
                                .iter()
                                .filter(|t| t.lifecycle == Lifecycle::Queued)
                                .any(|t| {
                                    m.capabilities[t.capability.0]
                                        .uses
                                        .iter()
                                        .any(|u| u.resource.0 == *i)
                                })
                        })
                        .map(|(_, r)| r)
                        .collect(),
                    waiting_tasks,
                    resources: m
                        .actors
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| {
                            s.tasks
                                .iter()
                                .any(|t| m.capabilities[t.capability.0].actor.0 == *i)
                        })
                        .map(|(i, _)| self.resource(s, ir::ActorId(i), m))
                        .collect(),
                    obligations: if s.obligations.is_some() {
                        Self::debt_details(s, m, case_id)
                    } else {
                        s.monitors
                            .iter()
                            .enumerate()
                            .flat_map(|(n, monitor)| {
                                monitor.pending.iter().map(move |deadline| Obligation {
                                    case_id,
                                    id: m.contracts[n].property_id(ir::ClauseId(n)),
                                    trigger_step: 0,
                                    deadline: *deadline,
                                })
                            })
                            .collect()
                    },
                    joins: if s.waiting {
                        vec![x::Join {
                            case_id: 1,
                            bindings: s.tasks.iter().map(|t| m.site_name(t.binding)).collect(),
                        }]
                    } else {
                        Vec::new()
                    },
                });
            }
            Ok(stage)
        })();
        let node = self.add(stage, targets);
        self.record_origins(node, Self::origins(s, m, case_id));
    }
    pub(super) fn record_origins(&mut self, node: usize, origins: Vec<ResponseOrigin>) {
        if !origins.is_empty() && self.failure.is_none() {
            self.response_origins.insert(node, origins);
        }
    }
    pub(super) fn origins(s: &State, m: &ir::Model, case: u64) -> Vec<ResponseOrigin> {
        s.obligations
            .iter()
            .flatten()
            .filter(|d| {
                d.resolution
                    .as_ref()
                    .is_some_and(|r| r.status == crate::obligation::Status::Violated)
            })
            .map(|d| {
                (
                    m.contracts[d.response.0].property_id(d.response),
                    case,
                    d.site.map(|b| m.site_name(b)),
                    d.triggered_at,
                    d.deadline,
                )
            })
            .collect()
    }
    pub(super) fn debt_details(s: &State, m: &ir::Model, case_id: u64) -> Vec<Obligation> {
        s.obligations
            .iter()
            .flatten()
            .filter(|d| {
                d.resolution.as_ref().is_none_or(|r| {
                    matches!(
                        r.reason.as_deref(),
                        Some("deadlock_unresolved" | "terminal_unresolved")
                    )
                })
            })
            .map(|d| Obligation {
                case_id,
                id: m.contracts[d.response.0].property_id(d.response),
                trigger_step: 0,
                deadline: d.deadline,
            })
            .collect()
    }
    pub(crate) fn response_origin(&self, target: &str) -> Option<(u64, Option<String>, u64, u64)> {
        // Use exactly shortest's BFS ordering, including canonical aliases, so
        // the origin belongs to the selected edge, not another schedule.
        let mut seen = vec![false; self.edges.len()];
        let mut queue = VecDeque::from([0]);
        seen[0] = true;
        while let Some(node) = queue.pop_front() {
            for edge in &self.edges[node] {
                if edge.targets.iter().any(|id| id == target) {
                    return self
                        .response_origins
                        .get(&edge.to)?
                        .iter()
                        .find(|o| o.0 == target && edge.cases.get(target) == Some(&o.1))
                        .map(|o| (o.1, o.2.clone(), o.3, o.4));
                }
                let next = self.canonical(edge.to);
                if !seen[next] {
                    seen[next] = true;
                    queue.push_back(next);
                }
            }
        }
        None
    }
    pub(super) fn emitted(
        &mut self,
        s: &State,
        bundle: Bundle,
        binding: Option<ir::BindingId>,
        m: &ir::Model,
    ) {
        if self.failure == Some(ReplayReason::CaptureLimit) {
            return;
        }
        let targets = Self::targets(s, m);
        if self.initial.is_none() || (targets.is_empty() && self.kernel_choice.is_none()) {
            return;
        }
        let (cap, person, result, kind) = match bundle {
            Bundle::CancelAck(c, p) => (c, p, None, Kind::CancelAck),
            Bundle::Start(c, p) => (c, p, None, Kind::Start),
            Bundle::Finish(c, v, p) => (c, p, Some(v), Kind::Finish),
            Bundle::End(_) => {
                self.leaf(s, m, Kind::End);
                return;
            }
            _ => return,
        };
        let task = s.tasks.iter().find(|t| {
            binding.is_none_or(|binding| t.binding == binding)
                && t.capability == cap
                && t.executor == person
                && (result.is_none()
                    || s.results.get(t.binding.0).copied().flatten() == result
                    || m.site_contest(t.binding).is_some_and(|id| {
                        s.contest
                            .as_ref()
                            .is_some_and(|c| c.routes[id].result == result)
                    }))
        });
        if let Some(task) = task {
            let stage = self.task_stage(s, task, m, kind);
            if let (Some(choice), Ok(stage)) = (&self.kernel_choice, &stage) {
                self.generated_choices.push((choice.clone(), stage.clone()));
            }
            if !targets.is_empty() {
                let node = self.add(stage, targets);
                self.capture_state_details(node, s, m, self.case_id);
            }
        }
    }
    pub(crate) fn merge(&mut self, from: usize, into: usize) {
        if self.failure == Some(ReplayReason::CaptureLimit) {
            return;
        }
        self.aliases[from] = into;
    }
    fn canonical(&self, mut node: usize) -> usize {
        while self.aliases[node] != node {
            node = self.aliases[node];
        }
        node
    }
    pub(crate) fn shortest(&self, target: &str) -> Option<WitnessPath> {
        if self.initial_targets.iter().any(|id| id == target) {
            let mut initial = self.initial.clone()?;
            self.attach_state_detail(0, target, &mut initial);
            return Some((vec![initial], Some(self.case_id), vec![None]));
        }
        let mut parents: Vec<Option<(usize, usize)>> = vec![None; self.edges.len()];
        let mut seen = vec![false; self.edges.len()];
        let mut queue = VecDeque::from([0]);
        seen[0] = true;
        while let Some(node) = queue.pop_front() {
            for (ordinal, edge) in self.edges[node].iter().enumerate() {
                if edge.targets.iter().any(|id| id == target) {
                    let mut path = vec![edge.stage.clone()];
                    let mut choices = vec![edge.choice.clone()];
                    let mut at = node;
                    while let Some((parent, index)) = parents[at] {
                        path.push(self.edges[parent][index].stage.clone());
                        choices.push(self.edges[parent][index].choice.clone());
                        at = parent;
                    }
                    path.push(self.initial.clone()?);
                    choices.push(None);
                    choices.reverse();
                    path.reverse();
                    for (i, stage) in path.iter_mut().enumerate() {
                        stage.index = i;
                    }
                    self.attach_state_detail(edge.to, target, path.last_mut()?);
                    return Some((path, edge.cases.get(target).copied(), choices));
                }
                let next = self.canonical(edge.to);
                if !seen[next] {
                    seen[next] = true;
                    parents[next] = Some((node, ordinal));
                    queue.push_back(next);
                }
            }
        }
        None
    }
}

#[cfg(test)]
#[path = "response_trace_tests.rs"]
mod response_tests;

pub(crate) fn explore(src: &str, model: &ir::Model) -> (outcome::Outcome, Capture) {
    #[cfg(test)]
    crate::evaluation_spy::hit(6);
    explore_with_budget(src, model, DEFAULT_STATE_BUDGET)
}

#[cfg(test)]
#[path = "trace_rule_tests.rs"]
mod rule_tests;

#[cfg(test)]
mod percase_replay_tests {
    use super::*;

    fn selected_path(
        c: &Capture,
        target: &str,
        case: u64,
        name: &str,
        wanted: [&str; 2],
    ) -> WitnessPath {
        let mut queue = VecDeque::from([(0, vec![c.initial.clone().unwrap()], vec![None])]);
        let mut seen = HashSet::new();
        while let Some((node, path, choices)) = queue.pop_front() {
            if !seen.insert(node) {
                continue;
            }
            for edge in &c.edges[node] {
                let mut k = None;
                let mut allowed = true;
                for event in &edge.stage.events {
                    match event {
                        Event::Business(e) if e.kind == Kind::Initialize => k = e.case_id,
                        Event::Value(v) if v.name == name => {
                            if let Some(k) = k {
                                allowed &= v.value == wanted[k as usize - 1];
                            }
                        }
                        _ => {}
                    }
                }
                if !allowed {
                    continue;
                }
                let mut next = path.clone();
                next.push(edge.stage.clone());
                let mut draws = choices.clone();
                draws.push(edge.choice.clone());
                if edge.cases.get(target) == Some(&case) {
                    for (i, s) in next.iter_mut().enumerate() {
                        s.index = i;
                    }
                    let last = next.last_mut().unwrap();
                    last.violation = Some(Violation {
                        state_invariant: None,
                        id: target.into(),
                        property_location: None,
                        at_step: last.index,
                        trigger_step: None,
                        deadline: None,
                        related_steps: vec![],
                    });
                    return (next, Some(case), draws);
                }
                queue.push_back((c.canonical(edge.to), next, draws));
            }
        }
        panic!("no selected nonuniform path for case {case}")
    }

    #[test]
    fn percase_nonuniform_case1_case2_replay_and_one_case_tamper() {
        let src = include_str!("../../../../fixtures/pcase-explain/model.relca");
        let m = crate::syntax::parse(src)
            .and_then(|a| crate::lower::lower(&a))
            .unwrap();
        let (_, c) = explore(src, &m);
        assert_eq!(c.failure, None);
        let target = m.contracts[0].property_id(ir::ClauseId(0));
        for (case, values) in [(1, ["routine", "difficult"]), (2, ["difficult", "routine"])] {
            let (path, k, choices) = selected_path(&c, &target, case, "difficulty", values);
            assert_eq!(
                crate::reference::replay::replay_choices(&m, &path, &target, k, &choices),
                Ok(())
            );
            let mut bad = path.clone();
            let finish = bad
                .iter_mut()
                .find(|s| s.kind == Kind::Finish && s.case_id == Some(case))
                .unwrap();
            finish
                .values
                .latent
                .iter_mut()
                .find(|v| v.name == "difficulty")
                .unwrap()
                .value = "difficult".into();
            assert!(
                crate::reference::replay::replay_choices(&m, &bad, &target, k, &choices).is_err()
            );
        }
    }

    #[test]
    fn percase_initial_mixed_equal_names_show_only_shared_declaration() {
        let src = include_str!("../../../../fixtures/pcase-explain/model.relca")
            .replace(
                "  process Flow",
                "  actor Shared { latent difficulty: bool = true }\n  process Flow",
            )
            .replace(
                "include [Invoice,Reader]",
                "include [Invoice,Reader,Shared]",
            );
        let m = crate::syntax::parse(&src)
            .and_then(|a| crate::lower::lower(&a))
            .unwrap();
        let (_, c) = explore(&src, &m);
        let values = &c.initial.as_ref().unwrap().values.latent;
        assert_eq!(values.len(), 1);
        let shared = m
            .state_variables
            .iter()
            .find(
                |v| matches!(v.kind,ir::StateKind::ActorLatent(a) if m.actors[a.0].name=="Shared"),
            )
            .unwrap();
        assert_eq!(values[0].location.span, shared.span);
        assert_eq!(values[0].value, "true");
    }

    #[test]
    fn percase_external_ir_write_replay_preserves_other_frame_and_rejects_tamper() {
        let src = include_str!("../../../../fixtures/pcase-write-ir/model.relca")
            .replace(
                "  observed case",
                "  latent case { gate: bool ~ { true:0.5, false:0.5 } }\n  observed case",
            )
            .replace(
                "  bounds",
                "  contract C { invariant Reader.read.finish -> gate == true }\n  bounds",
            );
        let mut m = crate::syntax::parse(&src)
            .and_then(|a| crate::lower::lower(&a))
            .unwrap();
        let doc = ir::StateVarId(
            m.state_variables
                .iter()
                .position(|v| v.name == "difficulty")
                .unwrap(),
        );
        m.capabilities[0].kernel_writes.push(doc);
        for row in &mut m.capabilities[0].kernel.as_mut().unwrap().rows {
            row.branches[0]
                .overrides
                .push((doc, ir::StateValue::Value(ir::ValueId(1))));
        }
        let (out, c) = explore(&src, &m);
        assert!(
            out.diagnostics
                .iter()
                .all(|d| d.severity != diag::Severity::Error)
        );
        assert_eq!(c.failure, None);
        let target = m.contracts[0].property_id(ir::ClauseId(0));
        let (path, k, choices) = selected_path(&c, &target, 2, "gate", ["true", "false"]);
        assert!(
            path.iter()
                .any(|s| s.kind == Kind::Finish && s.case_id == Some(1))
        );
        assert_eq!(
            crate::reference::replay::replay_choices(&m, &path, &target, k, &choices),
            Ok(())
        );
        let mut bad = path.clone();
        let finish = bad
            .iter_mut()
            .find(|s| s.kind == Kind::Finish && s.case_id == Some(1))
            .unwrap();
        finish
            .values
            .latent
            .iter_mut()
            .find(|v| v.name == "difficulty")
            .unwrap()
            .value = "routine".into();
        assert!(crate::reference::replay::replay_choices(&m, &bad, &target, k, &choices).is_err());
    }

    /// The pcase-explain path selected for case 1 (routine, difficult), and the
    /// position of the first value event after case 1's Initialize event.
    fn initialize_value_event() -> (ir::Model, WitnessPath, String, usize, usize) {
        let src = include_str!("../../../../fixtures/pcase-explain/model.relca");
        let m = crate::syntax::parse(src)
            .and_then(|a| crate::lower::lower(&a))
            .unwrap();
        let (_, c) = explore(src, &m);
        let target = m.contracts[0].property_id(ir::ClauseId(0));
        let path = selected_path(&c, &target, 1, "difficulty", ["routine", "difficult"]);
        assert_eq!(
            crate::reference::replay::replay_choices(&m, &path.0, &target, path.1, &path.2),
            Ok(())
        );
        for (i, s) in path.0.iter().enumerate() {
            if let Some(pos) = s.events.iter().position(|e| {
                matches!(e, Event::Business(b) if b.kind == Kind::Initialize && b.case_id == Some(1))
            }) {
                assert!(matches!(s.events[pos + 1], Event::Value(_)));
                return (m, path, target, i, pos + 1);
            }
        }
        panic!("no initialize stage for case 1")
    }

    /// Q24: an admission initializes each local once, so a duplicated value
    /// event for the same declaration is an illegal replay.
    #[test]
    fn percase_replay_rejects_duplicated_initialize_value_event() {
        let (m, (path, k, choices), target, si, vi) = initialize_value_event();
        let mut dup = path.clone();
        let ev = dup[si].events[vi].clone();
        dup[si].events.insert(vi, ev);
        assert!(
            crate::reference::replay::replay_choices(&m, &dup, &target, k, &choices).is_err(),
            "duplicated initialize value event must not replay"
        );
    }

    /// Q25: the replayed visibility must match the declaration (observed case
    /// stays Observed, latent stays Latent); flipping it is an illegal replay.
    #[test]
    fn percase_replay_rejects_flipped_initialize_value_visibility() {
        let (m, (path, k, choices), target, si, vi) = initialize_value_event();
        let mut vis = path.clone();
        let Event::Value(v) = &mut vis[si].events[vi] else {
            unreachable!()
        };
        v.visibility = match v.visibility {
            x::Visibility::Latent => x::Visibility::Observed,
            x::Visibility::Observed => x::Visibility::Latent,
        };
        assert!(
            crate::reference::replay::replay_choices(&m, &vis, &target, k, &choices).is_err(),
            "flipped visibility must not replay"
        );
    }
}

pub(super) fn explore_with_budget(
    src: &str,
    model: &ir::Model,
    budget: usize,
) -> (outcome::Outcome, Capture) {
    let capture = Capture::new(src, model);
    explore_captured(model, budget, capture)
}

pub(crate) fn explore_captured(
    model: &ir::Model,
    budget: usize,
    capture: Capture,
) -> (outcome::Outcome, Capture) {
    if !model.legacy_route() {
        let (outcome, trace) = arrivals::explore_traced(model, budget, Some(capture));
        return (outcome, trace.expect("enabled arrivals capture"));
    }
    if model.uses_probability() && model.legacy_route() {
        let (outcome, trace) = run_probability_traced(model, budget, Some(capture));
        return (
            attach_legacy_quality(outcome, model, budget),
            trace.expect("enabled probability capture"),
        );
    }
    let mut exploration = Exploration {
        trace: Some(capture),
        ..Exploration::default()
    };
    let result = explore_into(
        model,
        true,
        budget,
        std::convert::identity,
        &mut exploration,
    );
    let capture = exploration.trace.take().expect("enabled capture");
    let outcome = match result {
        Ok(()) => exploration.outcome(model),
        Err(Halt::Budget) => {
            exploration.truncated = true;
            exploration.budget_exhausted = true;
            exploration.outcome(model)
        }
        Err(Halt::Unsupported(message)) => exploration.abort(message, model),
    };
    (attach_legacy_quality(outcome, model, budget), capture)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn d1_shared_person_start_can_race_join_cancellation() {
        let original = include_str!("../../../../fixtures/contract-cancel/model.relca");
        let prefix = original.split_once("  contract Controls {").unwrap().0;
        let src = format!(
            "{prefix}  contract Controls {{ excludes Slow.work.cancel_ack, Fast.win.start }}\n}}\n"
        )
        .replace("    end done", "    again = Fast.win(); end done")
        .replace("  type Result", "  people [p]\n  type Result")
        .replace("capacity 1", "executors [p]");
        let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
        let mut exploration = Exploration {
            trace: Some(Capture::new(&src, &m)),
            ..Exploration::default()
        };
        explore_into(
            &m,
            true,
            DEFAULT_STATE_BUDGET,
            std::convert::identity,
            &mut exploration,
        )
        .unwrap();
        let capture = exploration.trace.take().unwrap();
        let (steps, _, _) = capture.shortest("contract Controls #1 excludes").expect(
            "a running cancellation is reachable after the newly eligible start races adoption",
        );
        for s in &steps {
            println!(
                "{} time={} kind={:?} binding={:?} events={:?}",
                s.index, s.time, s.kind, s.binding, s.events
            );
        }
        let slow_start = steps
            .iter()
            .position(|s| s.kind == Kind::Start && s.binding.as_deref() == Some("slow"))
            .unwrap();
        let ack = steps
            .iter()
            .position(|s| s.kind == Kind::CancelAck && s.binding.as_deref() == Some("slow"))
            .unwrap();
        assert!(slow_start < ack);
        assert_eq!(steps[slow_start].time, 1);
        assert_eq!(steps[ack].time, 2);
    }
    #[test]
    fn capture_limit_is_exact_and_never_publishes_a_partial_graph() {
        let (src, m) = model();
        let mut capture = Capture::new(&src, &m);
        capture.capture_limit = 2;
        let s = State::initial(&m, 0);
        for _ in 0..2 {
            let stage = capture.stage(&s, &m, Kind::Advance);
            capture.add(stage, vec![]);
            assert_eq!(capture.failure, None);
        }
        assert_eq!(capture.edges.iter().map(Vec::len).sum::<usize>(), 2);
        let stage = capture.stage(&s, &m, Kind::Advance);
        capture.add(stage, vec![]);
        assert_eq!(capture.failure, Some(ReplayReason::CaptureLimit));
        assert_eq!(capture.edges.iter().map(Vec::len).sum::<usize>(), 0);
        assert_eq!(capture.recorded_edges, 2);
    }

    #[test]
    fn large_capture_limit_preserves_semantic_census() {
        let calls = (1..=8)
            .map(|n| format!("x{n}=Worker.run();"))
            .collect::<String>();
        let src = format!(
            "model CaptureBenchmark version \"0.2\" {{type T=enum{{a,b,c}} actor Worker{{capacity 8;capability run()->T{{duration fixed 1min;result nondet{{a,b,c}}}}}} process Flow{{fork work{{{calls}}}join all work;end done}} bounds{{cases 1;live_tasks 8;clock 10min}} cut Scope{{question \"capture benchmark only\";include[Worker]}} verify safety no_resource_overcommit}}"
        );
        let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
        let mut plain = Exploration::default();
        let mut traced = Exploration {
            trace: Some(Capture::new(&src, &m)),
            ..Exploration::default()
        };
        assert_eq!(
            explore_into(
                &m,
                true,
                DEFAULT_STATE_BUDGET,
                std::convert::identity,
                &mut plain
            ),
            Ok(())
        );
        assert_eq!(
            explore_into(
                &m,
                true,
                DEFAULT_STATE_BUDGET,
                std::convert::identity,
                &mut traced
            ),
            Ok(())
        );
        assert_eq!(plain.census, traced.census);
        eprintln!("large model visited/generated/max-held: {:?}", plain.census);
        assert!((50_000..150_000).contains(&plain.census.0));
        assert_eq!(
            traced.trace.take().unwrap().failure,
            Some(ReplayReason::CaptureLimit)
        );
        assert_eq!(plain.outcome(&m), traced.outcome(&m));
    }
    fn model() -> (String, ir::Model) {
        let src =
            include_str!("../../../../fixtures/explain-once.broken-repeat-result/model.relca")
                .to_string();
        let model = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
        (src, model)
    }
    #[test]
    fn bfs_uses_all_edges_after_state_merge() {
        let (src, m) = model();
        let mut c = Capture::new(&src, &m);
        let s = State::initial(&m, 0);
        c.initial(&s, &m);
        let label = c.initial.clone().unwrap();
        // DFS first reaches merge4 along0-1-2-4; the later0-3-4 must survive.
        c.edges = vec![Vec::new(); 7];
        c.aliases = (0..7).collect();
        for (from, to) in [(0, 1), (1, 2), (2, 4), (0, 3), (3, 6), (4, 5)] {
            c.edges[from].push(Edge {
                to,
                stage: label.clone(),
                targets: if to == 5 {
                    vec!["target".into()]
                } else {
                    Vec::new()
                },
                cases: BTreeMap::new(),
                choice: None,
            });
        }
        c.merge(6, 4);
        assert_eq!(c.shortest("target").unwrap().0.len() - 1, 3);
        c.edges[3].clear();
        assert_eq!(c.shortest("target").unwrap().0.len() - 1, 4);
    }
    #[test]
    fn legacy_capture_preserves_outcome_and_budget_census() {
        let (src, m) = model();
        for budget in 0..32 {
            let mut plain = Exploration::default();
            let mut traced = Exploration {
                trace: Some(Capture::new(&src, &m)),
                ..Exploration::default()
            };
            let a = explore_into(&m, true, budget, std::convert::identity, &mut plain);
            let b = explore_into(&m, true, budget, std::convert::identity, &mut traced);
            assert_eq!(a, b, "budget{budget}");
            assert_eq!(plain.census, traced.census, "budget{budget}");
            traced.trace.take();
            assert_eq!(plain.outcome(&m), traced.outcome(&m), "budget{budget}");
        }
    }
    #[test]
    fn probability_capture_preserves_small_budget_outcomes() {
        let src = include_str!("../../../../fixtures/duration-parallel/model.relca");
        let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        for budget in 0..80 {
            let mut plain = Exploration::default();
            let mut traced = Exploration {
                trace: Some(Capture::new(src, &m)),
                ..Exploration::default()
            };
            let (plain_nodes, plain_scheduler) = probability_graph(&m, budget, &mut plain).unwrap();
            let (traced_nodes, traced_scheduler) =
                probability_graph(&m, budget, &mut traced).unwrap();
            assert_eq!(
                plain_nodes.len(),
                traced_nodes.len(),
                "nodes/budget{budget}"
            );
            assert_eq!(
                plain_scheduler, traced_scheduler,
                "scheduler/budget{budget}"
            );
            assert_eq!(plain.census, traced.census, "generated/held/budget{budget}");
            assert_eq!(
                run_probability(&m, budget),
                run_probability_traced(&m, budget, Some(Capture::new(src, &m))).0,
                "budget{budget}"
            );
        }
    }
    #[test]
    fn arrivals_capture_preserves_budget_census_and_outcome() {
        let src = include_str!(
            "../../../../fixtures/arrivals-contracts.broken-short-response/model.relca"
        );
        let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        for budget in 0..80 {
            arrivals::trace_compatibility_at_budget(src, &m, budget);
            assert_eq!(
                arrivals::explore(&m, budget),
                arrivals::explore_traced(&m, budget, Some(Capture::new(src, &m))).0,
                "budget{budget}"
            );
        }
    }
    #[test]
    fn room_boundaries_only_keep_actually_emitted_witnesses() {
        let (src, m) = model();
        for budget in [0, 1] {
            let (out, c) = explore_with_budget(&src, &m, budget);
            assert!(out.truncated);
            assert!(
                c.shortest(&m.contracts[0].property_id(ir::ClauseId(0)))
                    .is_none()
            );
        }
        let mut witnessed_cut = false;
        for budget in 2..64 {
            let (out, c) = explore_with_budget(&src, &m, budget);
            let target = m.contracts[0].property_id(ir::ClauseId(0));
            let found = c.shortest(&target).is_some();
            assert_eq!(
                found,
                out.properties
                    .iter()
                    .any(|p| matches!(p.property, ir::Property::Contract(_))
                        && p.verdict == Verdict::Violated)
            );
            witnessed_cut |= found && out.truncated;
        }
        assert!(witnessed_cut);
    }
    #[test]
    fn partial_emission_survives_aborting_other_clause_in_either_order() {
        let declarations = [
            "invariant A.run.finish -> good == true",
            "response A.run.finish -> one_of(end.done) by 18446744073709551615min after A.run.finish otherwise unresolved",
        ];
        for global in [false, true] {
            for reverse in [false, true] {
                let clauses = if reverse {
                    format!("{}\n{}", declarations[1], declarations[0])
                } else {
                    declarations.join("\n")
                };
                let src = format!(
                    "model M version \"0.2\" {{type T=enum{{ok}} observed case{{good:bool=false}} actor A{{capacity 1;capability run()->T{{duration fixed 1min;result nondet{{ok}}}}}} process P{{x=A.run();end done}} {} bounds{{cases {};live_tasks 1;clock 10min}} contract C{{{clauses}}} }}",
                    if global {
                        "arrivals fixed [0min,0min]"
                    } else {
                        ""
                    },
                    if global { 2 } else { 1 }
                );
                let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
                let (out, trace) = explore(&src, &m);
                assert!(out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
                let (n, clause) = m
                    .contracts
                    .iter()
                    .enumerate()
                    .find(|(_, c)| matches!(c.kind, ir::ContractKind::Invariant { .. }))
                    .unwrap();
                let path = trace
                    .shortest(&clause.property_id(ir::ClauseId(n)))
                    .expect("emitted invariant cannot be lost on abort")
                    .0;
                assert_eq!(path.last().unwrap().kind, Kind::Finish);
                assert_eq!(path.last().unwrap().time, 1);
            }
        }
    }
    #[test]
    fn abort_keeps_nonviolating_successors_generated_before_the_bad_branch() {
        let src = "model M version \"0.2\" {type T=enum{pass,fail} observed case{good:bool=false} actor A{capacity 1;capability run()->T{duration fixed 1min;result nondet{pass,fail}}} process P{x=A.run();end done} bounds{cases 1;live_tasks 1;clock 10min} contract C{invariant A.run.finish(fail) -> good == true;response A.run.finish(fail) -> one_of(end.done) by 18446744073709551615min after A.run.finish(fail) otherwise unresolved}}";
        for global in [false, true] {
            for kernel in [false, true] {
                let mut src = src.to_string();
                if global {
                    src = src.replace(
                        "bounds{cases 1;",
                        "arrivals fixed [0min,0min] bounds{cases 2;",
                    );
                }
                if kernel {
                    src = src
                        .replace(
                            "observed case",
                            "kernel K{* -> {0.5:pass;0.5:fail}} observed case",
                        )
                        .replace("result nondet{pass,fail}", "outcomes kernel K");
                }
                let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
                let (out, trace) = explore(&src, &m);
                assert_eq!(out, super::super::explore(&m));
                assert!(out.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
                assert!(
                    trace
                        .edges
                        .iter()
                        .flatten()
                        .any(|e| e.stage.kind == Kind::Finish
                            && e.stage.result.as_deref() == Some("pass")),
                    "a generated nonviolating sibling is still an observed graph edge"
                );
            }
        }
    }
    #[test]
    fn numeric_hazard_cut_retains_prior_charged_leaves_only_in_trace() {
        let src = include_str!("../../../../fixtures/duration-parallel/model.relca");
        let mut m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let third = Rational(Rational::one().0 / num_bigint::BigInt::from(3));
        m.capabilities[0].duration_distribution = Some(ir::DurationDistribution {
            entries: vec![
                (1, Rational::decimal("0.5").unwrap()),
                (2, Rational::decimal("0.5").unwrap()),
            ],
        });
        m.capabilities[1].duration_distribution = Some(ir::DurationDistribution {
            entries: vec![
                (1, Rational(&third.0 * num_bigint::BigInt::from(2))),
                (2, third),
            ],
        });
        let mut s = State::initial(&m, 0);
        s.time = 1;
        s.tasks = (0..2)
            .map(|n| Task {
                frozen_performance: None,
                stale: Vec::new(),
                executor: None,
                binding: ir::BindingId(n),
                capability: ir::CapabilityId(n),
                lifecycle: Lifecycle::DistributedRunning {
                    started: 0,
                    next: 0,
                    due: false,
                    ack: None,
                },
            })
            .collect();
        let mut plain = Exploration::default();
        let mut traced = Exploration {
            trace: Some(Capture::new(src, &m)),
            ..Exploration::default()
        };
        assert!(matches!(
            duration_gate(&s, &m, &mut Room(4), &mut plain, 2),
            Err(Halt::Budget)
        ));
        assert!(matches!(
            duration_gate(&s, &m, &mut Room(4), &mut traced, 2),
            Err(Halt::Budget)
        ));
        assert_eq!(plain.duration_clones, traced.duration_clones);
        assert!(plain.trace_discarded.is_empty());
        assert_eq!(traced.trace_discarded.len(), 2);
        assert!(
            traced.partial_successors.is_empty(),
            "trace observations must not enter probability reduction"
        );
    }
    #[test]
    fn synthetic_safety_occupancy_is_captured_but_not_legally_replayed() {
        let (src, m) = model();
        let mut c = Capture::new(&src, &m);
        let mut s = State::initial(&m, 0);
        s.tasks = (0..2)
            .map(|_| Task {
                frozen_performance: None,
                stale: Vec::new(),
                executor: None,
                binding: ir::BindingId(0),
                capability: ir::CapabilityId(0),
                lifecycle: Lifecycle::Running {
                    finish: 1,
                    ack: None,
                },
            })
            .collect();
        c.initial(&s, &m);
        let safety = crate::explain::synthetic_safety_report(&src, &m, &c);
        let verify = m
            .verify
            .iter()
            .find(|v| v.property == ir::Property::NoResourceOvercommit)
            .unwrap();
        assert_eq!(
            safety.property_location,
            Some(Location::new(&src, verify.span).unwrap())
        );
        assert_eq!(safety.status, ReplayStatus::Unsupported);
        assert!(safety.steps.is_empty());
        let (mut path, _, _) = c.shortest("safety no_resource_overcommit").unwrap();
        path[0].violation = Some(Violation {
            state_invariant: None,
            id: "safety no_resource_overcommit".into(),
            property_location: None,
            at_step: 0,
            trigger_step: None,
            deadline: None,
            related_steps: Vec::new(),
        });
        assert!(
            crate::reference::replay::replay(&m, &path, "safety no_resource_overcommit", None)
                .is_err()
        );
    }
    #[test]
    fn graph_history_never_participates_in_semantic_state_hash_or_equality() {
        use std::hash::{Hash, Hasher};
        let (src, m) = model();
        let state = State::initial(&m, 0);
        let mut c = Capture::new(&src, &m);
        c.initial(&state, &m);
        let first = c.transition(&state, &state, &m);
        c.current = first;
        c.transition(&state, &state, &m);
        let mut keys = HashSet::new();
        let mut left = std::collections::hash_map::DefaultHasher::new();
        state.hash(&mut left);
        assert!(keys.insert(state.clone()));
        c.current = 0;
        c.transition(&state, &state, &m);
        let mut right = std::collections::hash_map::DefaultHasher::new();
        state.hash(&mut right);
        assert!(!keys.insert(state));
        assert_eq!(left.finish(), right.finish());
        assert_eq!(keys.len(), 1);
    }
    #[test]
    fn phase_graph_shortest_matches_independent_distance_relaxation() {
        for src in [
            include_str!("../../../../fixtures/arrivals-contracts.broken-short-response/model.relca").to_string(),
            include_str!("../../../../fixtures/duration-parallel/model.relca").replace("  verify safety","  contract C { invariant A.left.finish -> leftDone == false }\n  verify safety"),
            include_str!("../../../../fixtures/exec-cancel/model.relca").replace("  contract Controls {","  contract Controls { response Slow.work.start -> one_of(Slow.work.finish) by 10min after Slow.work.start otherwise unresolved"),
        ] {
            let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
            let (_,c) = explore(&src,&m);
            let mut distance = vec![usize::MAX;c.edges.len()]; distance[0] = 0;
            loop {
                let mut changed = false;
                for (from,edges) in c.edges.iter().enumerate() {
                    if distance[from] == usize::MAX {continue;}
                    for edge in edges {
                        let to = c.canonical(edge.to);
                        if distance[to] > distance[from]+1 {distance[to] = distance[from]+1;changed = true;}
                    }
                }
                if !changed {break;}
            }
            let targets: BTreeSet<_> = c.edges.iter().flatten().flat_map(|e|e.targets.iter().cloned()).collect();
            assert!(!targets.is_empty());
            for target in targets {
                let mut oracle = usize::MAX;
                for (n,edges) in c.edges.iter().enumerate() {if distance[n] != usize::MAX && edges.iter().any(|e|e.targets.contains(&target)) {oracle = oracle.min(distance[n]+1);}}
                assert_eq!(c.shortest(&target).unwrap().0.len()-1,oracle,"{target}");
            }
        }
    }
}
#[cfg(test)]
#[path = "../../tests/support/conflict_private_trace.rs"]
#[rustfmt::skip]
mod conflict_private;
