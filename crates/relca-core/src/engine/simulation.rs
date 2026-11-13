//! Sampled driver. State identity and business successor producers belong to
//! the parent engine; the random context is never stored in a State.
use super::*;
use crate::outcome::{
    QueryTruth, ReplicationOutcome, SimulationCase, SimulationCut, SimulationDraw,
    SimulationOutcome, SimulationTerminal, SimulationTerminalKind,
};
use num_bigint::BigInt;
use sha2::{Digest, Sha256};

pub(super) struct Sample {
    seed: u64,
    rep: u64,
    pub(super) case: u64,
    pub(super) draws: Vec<SimulationDraw>,
    pub(super) sites: Vec<(usize, String, u64)>,
    pub(super) kernels: Vec<(usize, usize)>,
    pub(super) provenance: Vec<(u64, String, diag::Span)>,
    pub(super) refused: bool,
    pub(super) priority_tie: bool,
}
impl Generation for Sample {
    fn take(&mut self, _: usize) -> EngineResult<()> {
        Ok(())
    }
    fn sampling(&mut self) -> Option<&mut Sample> {
        Some(self)
    }
}
impl Sample {
    pub(super) fn new(seed: u64, rep: u64) -> Self {
        Self {
            seed,
            rep,
            case: 1,
            draws: Vec::new(),
            sites: Vec::new(),
            kernels: Vec::new(),
            provenance: Vec::new(),
            refused: false,
            priority_tie: false,
        }
    }
    pub(super) fn reject<T>(&mut self) -> EngineResult<T> {
        self.refused = true;
        Err(if self.priority_tie {
            "unresolved simulation nondeterminism: priority_tie_remaining"
        } else {
            "unresolved simulation nondeterminism"
        }
        .into())
    }
    pub(super) fn choose(
        &mut self,
        case: u64,
        site: &str,
        n: usize,
        ps: &[Rational],
    ) -> EngineResult<usize> {
        let key = format!(
            "relca-sim-v1|seed={}|rep={}|case={case}|site={site}|n={n}",
            self.seed, self.rep
        );
        let digest = Sha256::digest(key.as_bytes());
        let k = u64::from_be_bytes(digest[..8].try_into().unwrap());
        let selected = categorical(k, ps)?;
        self.draws.push(SimulationDraw { key });
        Ok(selected)
    }
    pub(super) fn initial_site(m: &ir::Model, i: usize, case: u64) -> (u64, String) {
        let v = &m.state_variables[i];
        match v.kind {
            ir::StateKind::ActorLatent(a) => (
                if m.actors[a.0].is_per_case() { case } else { 0 },
                format!("init.actor.{}.{}", m.actors[a.0].name, v.name),
            ),
            _ => (case, format!("init.case.{}", v.name)),
        }
    }
    pub(super) fn task_site(m: &ir::Model, binding: ir::BindingId, kind: &str) -> String {
        if let Some(r) = m.response_attempt(binding) {
            return format!(
                "{kind}.{}.{}.attempt{}",
                m.process.name,
                m.site_name(r.command),
                r.attempt
            );
        }
        format!("{kind}.{}.{}", m.process.name, m.site_name(binding))
    }
    pub(super) fn tag(&self, index: usize) -> Option<(String, u64)> {
        self.sites
            .iter()
            .find(|(i, _, _)| *i == index)
            .map(|(_, s, c)| (s.clone(), *c))
    }
    pub(super) fn note_task(&mut self, m: &ir::Model, binding: ir::BindingId, fork: bool) {
        fn find(body: &ir::Block, binding: ir::BindingId, fork: bool) -> Option<diag::Span> {
            for stmt in body {
                match &stmt.kind {
                    ir::Stmt::Call(c) | ir::Stmt::Response(c, _) if c.binding == binding => {
                        return Some(c.span);
                    }
                    ir::Stmt::Fork { branches, .. }
                        if branches.iter().any(|c| c.binding == binding) =>
                    {
                        return Some(if fork {
                            stmt.span
                        } else {
                            branches.iter().find(|c| c.binding == binding).unwrap().span
                        });
                    }
                    ir::Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } => {
                        if let Some(span) = find(then_body, binding, fork)
                            .or_else(|| find(else_body, binding, fork))
                        {
                            return Some(span);
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        self.provenance.push((
            self.case,
            format!("{}.{}", m.process.name, m.site_name(binding)),
            m.site_contest(binding)
                .map(|id| m.contests()[id].span)
                .or_else(|| find(&m.process.body, binding, fork))
                .unwrap_or(m.process.span),
        ));
    }
    pub(super) fn note_initial(&mut self, m: &ir::Model, i: usize, case: u64) {
        let (case, site) = Self::initial_site(m, i, case);
        self.provenance
            .push((case, site, m.state_variables[i].span));
    }
    // Only adjacent equal effects may be coalesced: non-adjacent intervals
    // cannot be moved without changing the key-to-effect coupling.
    pub(super) fn unique<T: Clone + Eq>(
        &mut self,
        actions: &[Vec<(Rational, T)>],
    ) -> EngineResult<()> {
        let canonical = |xs: &Vec<(Rational, T)>| {
            let mut result: Vec<(Rational, T)> = Vec::new();
            for (p, s) in xs.iter().filter(|(p, _)| !p.is_zero()) {
                if let Some((q, t)) = result.last_mut()
                    && t == s
                {
                    q.0 += &p.0;
                } else {
                    result.push((p.clone(), s.clone()));
                }
            }
            result
        };
        if let Some(first) = actions.first() {
            let first = canonical(first);
            let tag = self.tag(0);
            for (i, other) in actions.iter().enumerate().skip(1) {
                if self.tag(i) != tag || canonical(other) != first {
                    return self.reject();
                }
            }
        }
        Ok(())
    }
    pub(super) fn legacy_unique(
        &mut self,
        states: &[State],
        evidence: &mut Exploration,
    ) -> EngineResult<()> {
        let saved = evidence.chance_groups.clone();
        let actions = probability_actions(states.to_vec(), evidence)?;
        evidence.chance_groups = saved;
        // During generation sites carry successor offsets. Remap them to the
        // action positions without touching the producer's original metadata.
        let old = self.sites.clone();
        let mut offset = 0;
        self.sites.clear();
        for (i, a) in actions.iter().enumerate() {
            if let Some((_, site, case)) = old.iter().find(|(j, _, _)| *j == offset) {
                self.sites.push((i, site.clone(), *case));
            }
            offset += a.len();
        }
        let result = self.unique(&actions);
        self.sites = old;
        result
    }
}

fn categorical(k: u64, ps: &[Rational]) -> EngineResult<usize> {
    if ps.is_empty() || ps.iter().any(|p| !p.valid()) {
        return Err("invalid categorical probabilities".into());
    }
    let sum = ps.iter().fold(Rational::zero(), |mut s, p| {
        s.0 += &p.0;
        s
    });
    if sum != Rational::one() {
        return Err("categorical probabilities do not sum to one".into());
    }
    let mut cumulative = Rational::zero();
    for (i, p) in ps.iter().enumerate() {
        cumulative.0 += &p.0;
        if BigInt::from(k) * cumulative.0.denom() < cumulative.0.numer() * (BigInt::from(1u8) << 64)
        {
            return Ok(i);
        }
    }
    Err("categorical support exhausted".into())
}

pub(super) struct QueryObservation {
    pub hit: bool,
    pub closed: bool,
    pub time: u64,
    pub arrival: u64,
    pub within: Option<u64>,
    pub cut: Option<SimulationCut>,
    pub clock: u64,
}

pub(super) fn path_warning(
    out: &mut SimulationOutcome,
    m: &ir::Model,
    numeric: bool,
    deadlock: bool,
) {
    for (yes, code, message) in [
        (
            numeric,
            diag::W_BUDGET_0,
            "probability denominator exceeded the exact arithmetic limit",
        ),
        (
            deadlock,
            diag::W_DEADLOCK_0,
            "a sampled nonterminal state has no enabled transition",
        ),
    ] {
        if yes && !out.diagnostics.iter().any(|d| d.code == code) {
            out.diagnostics.push(diag::Diagnostic {
                code: code.into(),
                severity: diag::Severity::Warning,
                message: message.into(),
                span: Some(m.process.span),
            });
        }
    }
}
impl QueryObservation {
    pub(super) fn quality_truth(&self, range_stopped: bool) -> QueryTruth {
        if self.hit {
            return QueryTruth::Hit;
        }
        if self.closed
            || self
                .within
                .is_some_and(|d| self.time >= self.arrival && self.time - self.arrival > d)
        {
            return QueryTruth::Miss;
        }
        if range_stopped {
            return QueryTruth::Unknown;
        }
        self.truth()
    }
    pub(super) fn truth(&self) -> QueryTruth {
        if self.hit {
            return QueryTruth::Hit;
        }
        if self.closed
            || self.cut != Some(SimulationCut::Numeric)
                && self.within.is_some_and(|d| {
                    self.time >= self.arrival && self.time - self.arrival > d
                        || matches!(
                            self.cut,
                            Some(SimulationCut::Clock | SimulationCut::Horizon)
                        ) && self.arrival <= self.clock
                            && d <= self.clock - self.arrival
                })
        {
            return QueryTruth::Miss;
        }
        if self.cut.is_some() {
            QueryTruth::Unknown
        } else {
            QueryTruth::Miss
        }
    }
}

pub(super) fn simulate(m: &ir::Model) -> SimulationOutcome {
    if let Some(v) = m.verify.iter().find(|v| v.property.completion()) {
        let mut out = invalid_outcome("simulation cannot check structural completion questions", m);
        out.diagnostics[0].span = Some(v.span);
        return out;
    }
    let mut result = SimulationOutcome::default();
    let Some(spec) = &m.simulation else {
        return invalid_outcome("simulation requires explicit configuration", m);
    };
    if !(1..=10000).contains(&spec.replications)
        || spec.confidence <= Rational::zero()
        || spec.confidence >= Rational::one()
        || m.verify.is_empty()
    {
        return invalid_outcome("invalid simulation configuration", m);
    }
    if m.arrival_distribution.is_some() {
        return arrivals::simulate(m);
    }
    if let Err(Halt::Unsupported(message)) = validate(m) {
        return invalid_outcome(message, m);
    }
    if !m.legacy_route() {
        return arrivals::simulate(m);
    }
    let mut program = vec![Instruction::Stop];
    let entry = compile(&m.process.body, 0, &mut program);
    result.metric_template = metrics::finish(None, m, crate::obligation::Coverage::Aborted);
    result.capability_refusals = m.has_requires().then(Vec::new);
    let mut quality_observations = Vec::new();
    for rep in 1..=spec.replications {
        result.attempted_replications = rep;
        let mut sample = Sample::new(spec.seed, rep);
        let mut s = State::initial(m, entry);
        let mut facts = Exploration::default();
        let mut hit = vec![false; m.probability_queries.len()];
        let mut quality_hits = vec![false; m.quality.as_ref().map_or(0, |q| q.properties.len())];
        let mut first_hits = vec![
            None;
            m.quality.as_ref().map_or(0, |q| if q.quantiles.is_empty() {
                0
            } else {
                q.properties.len()
            })
        ];
        let mut cut = None;
        let mut terminal = None;
        let mut deadlock = None;
        let path = (|| -> EngineResult<()> {
            // Even a deterministic initial state receives exactly one bundle.
            if s.initialized {
                emit(&mut s, Bundle::Received, m, &mut facts)?;
            } else {
                s = probability_initial(&s, m, &mut sample, &mut facts)?
                    .remove(0)
                    .1;
            }
            loop {
                if s.exceeds_live_bound(m.bounds.live_tasks) {
                    cut = Some(SimulationCut::LiveTasks);
                    break;
                }
                if s.initialized {
                    if let Some(q) = &m.quality {
                        for (i, p) in q.properties.iter().enumerate() {
                            let arrived = quality_hit(
                                p,
                                &s,
                                s.contest
                                    .as_ref()
                                    .and_then(|c| c.business.as_ref())
                                    .map(|b| (b.end, b.at)),
                                0,
                            )?;
                            quality_hits[i] |= arrived;
                            if let Some(first) = first_hits.get_mut(i) {
                                let at = match p.target {
                                    ir::QualityTarget::Ends(_) => s
                                        .contest
                                        .as_ref()
                                        .and_then(|c| c.business.as_ref())
                                        .map(|b| b.at),
                                    _ => None,
                                };
                                crate::quality::observe_first_world(
                                    std::slice::from_mut(first),
                                    &[0],
                                    s.time,
                                    true,
                                    &[(true, arrived, at)],
                                );
                            }
                        }
                    }
                    for (i, q) in m.probability_queries.iter().enumerate() {
                        hit[i] |= query_hit(
                            q,
                            &s,
                            s.contest
                                .as_ref()
                                .and_then(|c| c.business.as_ref())
                                .map(|b| (b.end, b.at)),
                            0,
                        )?;
                    }
                }
                facts.observe(&s, m)?;
                facts.contracts(&s);
                // A gate abort must not report the previous step's call sites.
                sample.provenance.clear();
                if let Some(mut xs) = duration_gate(&s, m, &mut sample, &mut facts, 4096)? {
                    s = xs.remove(0).1;
                    continue;
                }
                sample.sites.clear();
                sample.kernels.clear();
                sample.provenance.clear();
                let mut scratch = Exploration::default();
                match step_observed(s.clone(), &program, m, &mut sample, &mut scratch)? {
                    Step::Next(xs) => {
                        if !scratch.ranges.is_empty() {
                            if !xs.is_empty() {
                                return sample.reject();
                            }
                            facts.ranges = scratch.ranges;
                            break;
                        }
                        sample.legacy_unique(&xs, &mut scratch)?;
                        let old = sample.sites.clone();
                        let mut actions = probability_actions(xs, &mut scratch)?;
                        let action = actions.remove(0);
                        if let Some((_, task)) =
                            sample.kernels.iter().find(|(i, _)| *i == 0).copied()
                        {
                            s = sample_kernel(&action[0].1, task, m, &mut sample, &mut scratch)?;
                            continue;
                        }
                        let selected =
                            if let Some((_, site, case)) = old.iter().find(|(i, _, _)| *i == 0) {
                                sample.choose(
                                    *case,
                                    site,
                                    1,
                                    &action.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
                                )?
                            } else {
                                0
                            };
                        let mut next = action
                            .into_iter()
                            .nth(selected)
                            .ok_or("empty successor action")?
                            .1;
                        selected_refusals(&s, &mut next, m, &mut facts);
                        s = next;
                    }
                    Step::Terminal(e) => {
                        if let Some(q) = &m.quality {
                            for (i, p) in q.properties.iter().enumerate() {
                                let arrived = quality_hit(p, &s, Some((e, s.time)), 0)?;
                                quality_hits[i] |= arrived;
                                if let Some(first) = first_hits.get_mut(i) {
                                    crate::quality::observe_first_world(
                                        std::slice::from_mut(first),
                                        &[0],
                                        s.time,
                                        true,
                                        &[(true, arrived, Some(s.time))],
                                    );
                                }
                            }
                        }
                        for (i, q) in m.probability_queries.iter().enumerate() {
                            if matches!(q.condition, ir::ProbabilityCondition::End(_)) {
                                hit[i] |= query_hit(q, &s, Some((e, s.time)), 0)?;
                            }
                        }
                        emit(&mut s, Bundle::End(e), m, &mut facts)?;
                        close_responses(&mut s);
                        facts.contracts(&s);
                        terminal = Some(SimulationTerminal {
                            kind: SimulationTerminalKind::End,
                            end: Some(m.ends[e.0].clone()),
                        });
                        break;
                    }
                    Step::Cut => {
                        // Closing deadlines at clock is a selected-path fact,
                        // unlike candidate-generation facts.
                        for monitor in &mut s.monitors {
                            monitor.violated |=
                                monitor.pending.iter().any(|d| *d <= m.bounds.clock);
                        }
                        facts.contracts(&s);
                        cut = Some(SimulationCut::Clock);
                        break;
                    }
                    Step::Deadlock => {
                        close_responses(&mut s);
                        facts.contracts(&s);
                        deadlock = Some(s.time);
                        break;
                    }
                }
            }
            Ok(())
        })();
        if let Err(halt) = path {
            if halt == Halt::Budget && facts.duration_numeric {
                cut = Some(SimulationCut::Numeric);
            } else {
                let message = match halt {
                    Halt::Unsupported(s) => s,
                    Halt::Budget => "unexpected simulation limit",
                };
                result.diagnostics.clear();
                result.diagnostics.push(runtime_diagnostic(
                    m,
                    rep,
                    s.time,
                    sample.refused,
                    message,
                    &sample.provenance,
                ));
                result.truncated_replications = None;
                return result;
            }
        }
        let queries = m
            .probability_queries
            .iter()
            .enumerate()
            .map(|(i, q)| {
                if !facts.ranges.is_empty() && !hit[i] {
                    return QueryTruth::Unknown;
                }
                QueryObservation {
                    hit: hit[i],
                    closed: terminal.is_some() || deadlock.is_some(),
                    time: s.time,
                    arrival: 0,
                    within: q.within,
                    cut,
                    clock: m.bounds.clock,
                }
                .truth()
            })
            .collect();
        if let Some(q) = &m.quality {
            let truths: Vec<Vec<QueryTruth>> = q
                .properties
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    vec![
                        QueryObservation {
                            hit: quality_hits[i],
                            closed: terminal.is_some()
                                || deadlock.is_some()
                                || (matches!(p.target, ir::QualityTarget::Ends(_))
                                    && s.contest.as_ref().is_some_and(|c| c.business.is_some())),
                            time: s.time,
                            arrival: 0,
                            within: p.within,
                            cut,
                            clock: m.bounds.clock,
                        }
                        .quality_truth(!facts.ranges.is_empty()),
                    ]
                })
                .collect();
            let key = if let Some(t) = &terminal {
                ("end", t.end.clone())
            } else if deadlock.is_some() {
                ("deadlock", None)
            } else {
                ("unfinished", None)
            };
            #[cfg(test)]
            let key = crate::quality_impl_tests::outcome_lookup_key("engine_legacy", key);
            let Some(idx) = crate::quality::outcome_keys(m)
                .iter()
                .position(|k| *k == key)
            else {
                return invalid_outcome("quality outcome inventory mismatch", m);
            };
            quality_observations.push(crate::quality::Observation {
                arrivals: q
                    .properties
                    .iter()
                    .take(first_hits.len())
                    .enumerate()
                    .map(|(i, p)| {
                        vec![if let Some(t) = first_hits[i] {
                            crate::quality::Arrival::Hit(t)
                        } else if truths[i][0] == QueryTruth::Unknown {
                            crate::quality::Arrival::censor(
                                if cut == Some(SimulationCut::Clock) {
                                    m.bounds.clock as u128 + 1
                                } else {
                                    s.time as u128
                                },
                                p.within.map(u128::from),
                                true,
                            )
                        } else {
                            crate::quality::Arrival::Never
                        }]
                    })
                    .collect(),
                truths,
                outcomes: vec![idx],
                computational: true,
            });
        }
        path_warning(
            &mut result,
            m,
            cut == Some(SimulationCut::Numeric),
            deadlock.is_some(),
        );
        if !facts.ranges.is_empty() {
            result.range_stops.push((rep, facts.ranges.clone()));
        }
        result.replications.push(ReplicationOutcome {
            response_recovery: None,
            contests: Vec::new(),
            admission: None,
            lost_updates: facts.lost_updates.iter().copied().collect(),
            scenario_observed_until: None,
            metrics: s.metric_history.as_ref().map(|h| {
                metrics::snapshot(
                    m,
                    &[metrics::CaseObservation {
                        admitted_at: Some(0),
                        state: Some(&s),
                        arrival: 0,
                        end: terminal
                            .as_ref()
                            .and_then(|t| t.end.as_ref())
                            .and_then(|name| m.ends.iter().position(|e| e == name))
                            .map(ir::EndId),
                        rejected: false,
                        deadlocked: deadlock.is_some(),
                        not_arrived: false,
                    }],
                    &h.peak,
                    if cut == Some(SimulationCut::Clock) {
                        m.bounds.clock
                    } else {
                        s.time
                    },
                )
            }),
            replication: rep,
            draws: sample.draws,
            cases: vec![SimulationCase {
                cut_off: None,
                case_id: 1,
                arrival: 0,
                terminal,
                queries,
                violations: facts.violation,
                deadlock,
            }],
            cut,
            overcommit: facts.overcommit,
        });
        result.add_refusals(facts.refusals.into_values());
        result.completed_replications = rep;
        *result.truncated_replications.get_or_insert(0) += u64::from(cut.is_some());
    }
    result.quality = m
        .quality
        .as_ref()
        .map(|_| crate::quality::simulation(m, &quality_observations, false));
    result
}

pub(super) fn invalid_outcome(message: &str, m: &ir::Model) -> SimulationOutcome {
    SimulationOutcome {
        diagnostics: vec![diag::Diagnostic {
            code: diag::U_SEM_0.into(),
            severity: diag::Severity::Error,
            message: message.into(),
            span: Some(m.process.span),
        }],
        ..SimulationOutcome::default()
    }
}

pub(super) fn runtime_diagnostic(
    m: &ir::Model,
    rep: u64,
    time: u64,
    nondet: bool,
    message: &str,
    provenance: &[(u64, String, diag::Span)],
) -> diag::Diagnostic {
    let sites: BTreeSet<_> = provenance.iter().map(|(_, s, _)| s.as_str()).collect();
    let cases: BTreeSet<_> = provenance.iter().map(|(c, _, _)| *c).collect();
    let span = provenance
        .iter()
        .map(|(_, _, s)| *s)
        .min_by_key(|s| s.start)
        .unwrap_or(m.process.span);
    diag::Diagnostic {
        code: diag::U_SEM_0.into(),
        severity: diag::Severity::Error,
        span: Some(span),
        message: if nondet {
            let label = if message.contains("priority_tie_remaining") {
                "unresolved simulation nondeterminism: priority_tie_remaining"
            } else {
                "simulation encountered unresolved nondeterminism"
            };
            format!(
                "{label}: replication {rep}, time {time}, cases [{}], sites [{}]; at least two distinct successor actions remain",
                cases
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
                sites.into_iter().collect::<Vec<_>>().join(",")
            )
        } else {
            format!("simulation aborted: replication {rep}, time {time}: {message}")
        },
    }
}

#[cfg(test)]
mod simulation_tests {
    use super::*;
    fn fixture(name: &str) -> ir::Model {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let src = std::fs::read_to_string(root.join(name).join("model.relca")).unwrap();
        crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap()
    }
    #[test]
    fn s_engine_paths_hand_oracle_before_reference() {
        for (name, hits) in [
            ("sim-keys", vec![4, 2]),
            ("sim-hazard", vec![4]),
            ("sim-hazard.broken-short-clock", vec![2]),
            ("sim-actor-initial", vec![2]),
            ("sim-cut", vec![4, 4]),
            ("sim-cut.broken-short-clock", vec![0, 0]),
            ("sim-fifo", vec![4]),
            ("sim-rejection", vec![0]),
            ("sim-rejection.broken-positive-queue", vec![4]),
            ("sim-statistical", vec![4956, 4956, 4956]),
        ] {
            let m = fixture(name);
            let out = simulate(&m);
            assert!(out.diagnostics.is_empty(), "{name}: {:?}", out.diagnostics);
            assert_eq!(
                out.completed_replications,
                m.simulation.as_ref().unwrap().replications,
                "{name}"
            );
            for case in 0..m.bounds.cases as usize {
                for (q, h) in hits.iter().enumerate() {
                    assert_eq!(
                        out.replications
                            .iter()
                            .filter(|r| r.cases[case].queries[q] == QueryTruth::Hit)
                            .count(),
                        *h,
                        "{name}/case{case}/q{q}"
                    );
                }
            }
        }
        for name in [
            "sim-actor-initial.broken-missing-distribution",
            "sim-fifo.broken-no-policy",
        ] {
            let out = simulate(&fixture(name));
            assert_eq!(out.attempted_replications, 1);
            assert_eq!(out.completed_replications, 0);
            assert_eq!(out.diagnostics[0].code, diag::U_SEM_0);
        }
    }
    #[test]
    fn s_key_boundary_exact_cdf() {
        let p = |s| Rational::decimal(s).unwrap();
        assert_eq!(categorical(1 << 63, &[p("0.5"), p("0.5")]).unwrap(), 1);
        assert_eq!(
            categorical(
                1 << 63,
                &[p("0.50000000000000000001"), p("0.49999999999999999999")]
            )
            .unwrap(),
            0
        );
        assert_eq!(categorical(0, &[p("0"), p("1")]).unwrap(), 1);
        assert_eq!(categorical(u64::MAX, &[p("0.5"), p("0.5")]).unwrap(), 1);
    }
    #[test]
    fn s_dedup_full_state_and_factor_descriptions() {
        let m = fixture("sim-contract");
        let mut program = vec![Instruction::Stop];
        let entry = compile(&m.process.body, 0, &mut program);
        let s = State::initial(&m, entry);
        let mut sample = Sample::new(0, 1);
        let one = |s: State| vec![(Rational::one(), s)];
        sample.unique(&[one(s.clone()), one(s.clone())]).unwrap();
        let mut variants = Vec::new();
        let mut t = s.clone();
        t.monitors[0].activated = true;
        variants.push(t);
        let mut t = s.clone();
        t.pc -= 1;
        variants.push(t);
        let mut t = s.clone();
        t.initialized = !t.initialized;
        variants.push(t);
        let mut t = s.clone();
        t.work += 1;
        variants.push(t);
        for t in variants {
            assert!(Sample::new(0, 1).unique(&[one(s.clone()), one(t)]).is_err());
        }
        let mut t = s.clone();
        t.pc -= 1;
        let p = |x| Rational::decimal(x).unwrap();
        let a = vec![
            (p("0.2"), s.clone()),
            (p("0.3"), s.clone()),
            (p("0.5"), t.clone()),
        ];
        let b = vec![(p("0.5"), s.clone()), (p("0.5"), t.clone())];
        sample.sites = vec![(0, "kernel.P.a".into(), 1), (1, "kernel.P.a".into(), 1)];
        sample.unique(&[a.clone(), b.clone()]).unwrap();
        sample.sites[1].1 = "kernel.P.b".into();
        assert!(sample.unique(&[a, b.clone()]).is_err());
        sample.sites[1].1 = "kernel.P.a".into();
        assert!(
            sample
                .unique(&[b, vec![(p("0.5"), t), (p("0.5"), s)]])
                .is_err()
        );
        let m = fixture("exec-authority");
        let mut program = vec![Instruction::Stop];
        let entry = compile(&m.process.body, 0, &mut program);
        let mut s = State::initial(&m, entry);
        s.tasks.push(Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: Some(ir::PersonId(0)),
            binding: ir::BindingId(0),
            capability: ir::CapabilityId(0),
            lifecycle: Lifecycle::Running {
                finish: 1,
                ack: None,
            },
        });
        let mut t = s.clone();
        t.tasks[0].executor = Some(ir::PersonId(1));
        assert!(Sample::new(0, 1).unique(&[one(s), one(t)]).is_err());
    }
    #[test]
    fn s_cancelled_kernel_has_no_draw_on_post_ack_snapshot() {
        let m = fixture("prob-cancel");
        let mut program = vec![Instruction::Stop];
        let entry = compile(&m.process.body, 0, &mut program);
        let Instruction::Wait { calls, .. } = &program[entry] else {
            unreachable!()
        };
        let mut s = State::initial(&m, entry);
        s.time = 2;
        s.work = 3;
        s.waiting = true;
        s.adopted = Some(calls[0].binding);
        s.result_at = Some(1);
        s.results[calls[0].binding.0] = Some(ir::ValueId(0));
        s.tasks = calls
            .iter()
            .map(|call| Task {
                frozen_performance: None,
                stale: Vec::new(),
                executor: None,
                binding: call.binding,
                capability: call.capability,
                lifecycle: Lifecycle::Done,
            })
            .collect();
        let mut sample = Sample::new(0, 1);
        let mut ended = false;
        for _ in 0..20 {
            let mut e = Exploration::default();
            match step_observed(s.clone(), &program, &m, &mut sample, &mut e).unwrap() {
                Step::Next(xs) => {
                    assert_eq!(xs.len(), 1);
                    s = xs.into_iter().next().unwrap();
                }
                Step::Terminal(_) => {
                    ended = true;
                    break;
                }
                _ => panic!("post-ack snapshot should complete through Probe"),
            }
        }
        let ir::InitialValue::Fixed(initial) = m.state_variables[0].initial else {
            unreachable!()
        };
        assert!(ended);
        assert!(sample.draws.is_empty());
        assert_eq!(s.state_values[0], initial);
    }
    #[test]
    fn s_key_sites_hundred_initial_factors_are_lazy_and_order_independent() {
        let declarations = (0..100)
            .map(|i| {
                format!(
                    "x{i}:T ~ {{first:0.{},second:0.{}1}}",
                    "9".repeat(63),
                    "0".repeat(62)
                )
            })
            .collect::<Vec<_>>()
            .join(";\n");
        let src = format!(
            "model M version \"0.2\" {{type T=enum{{first,second}} latent case {{{declarations}}} process P {{end done}} bounds {{cases 1;live_tasks 1;clock 1min}} verify reachable end.done\nsimulation replications 1 seed 0 confidence 0.5\n}}"
        );
        let mut m = crate::syntax::parse(&src)
            .and_then(|a| crate::lower::lower(&a))
            .unwrap();
        let mut tuples = Vec::new();
        for reverse in [false, true] {
            if reverse {
                m.state_variables.reverse();
            }
            let s = State::initial(&m, 0);
            let mut sample = Sample::new(0, 1);
            let mut e = Exploration::default();
            let xs = probability_initial(&s, &m, &mut sample, &mut e).unwrap();
            assert_eq!(xs.len(), 1);
            assert_eq!(sample.draws.len(), 100);
            assert!(!e.duration_numeric);
            let tuple: BTreeMap<_, _> = m
                .state_variables
                .iter()
                .zip(&xs[0].1.state_values)
                .map(|(v, x)| (v.name.clone(), *x))
                .collect();
            for (i, v) in m.state_variables.iter().enumerate() {
                let key = format!(
                    "relca-sim-v1|seed=0|rep=1|case=1|site=init.case.{}|n=1",
                    v.name
                );
                let digest = Sha256::digest(key.as_bytes());
                let k = BigInt::from(u64::from_be_bytes(digest[..8].try_into().unwrap()));
                let d = BigInt::from(10u8).pow(63);
                let chosen = if &k * &d < (&d - 1u8) * (BigInt::from(1u8) << 64) {
                    ir::StateValue::Value(ir::ValueId(0))
                } else {
                    ir::StateValue::Value(ir::ValueId(1))
                };
                assert_eq!(xs[0].1.state_values[i], chosen);
                assert!(sample.draws.iter().any(|d| d.key == key));
            }
            tuples.push(tuple);
            let full = simulate(&m);
            assert_eq!(full.completed_replications, 1);
            assert_eq!(full.replications[0].draws.len(), 100);
        }
        assert_eq!(tuples[0], tuples[1]);
    }
    #[test]
    fn s_key_occurrences_two_task_hazards_order_revisit_and_final_one() {
        let mut m = fixture("sim-hazard");
        m.process.bindings.push(ir::Binding {
            repetition: None,
            name: "b".into(),
            ty: m.process.bindings[0].ty,
            span: m.process.span,
        });
        let mut s = State::initial(&m, 0);
        s.time = 1;
        s.tasks = (0..2)
            .map(|i| Task {
                frozen_performance: None,
                stale: Vec::new(),
                executor: None,
                binding: ir::BindingId(i),
                capability: ir::CapabilityId(0),
                lifecycle: Lifecycle::DistributedRunning {
                    started: 0,
                    next: 0,
                    due: false,
                    ack: None,
                },
            })
            .collect();
        let mut tuples = Vec::new();
        for reverse in [false, true] {
            let mut s = s.clone();
            if reverse {
                s.tasks.reverse();
            }
            let mut sample = Sample::new(0, 1);
            let mut e = Exploration::default();
            let mut n = duration_gate(&s, &m, &mut sample, &mut e, 4096)
                .unwrap()
                .unwrap()
                .remove(0)
                .1;
            assert_eq!(sample.draws.len(), 2);
            let tuple: BTreeMap<_, _> = n.tasks.iter().map(|t| (t.binding, t.lifecycle)).collect();
            for task in &n.tasks {
                let site = format!("duration.P.{}", m.process.bindings[task.binding.0].name);
                let digest = Sha256::digest(
                    format!("relca-sim-v1|seed=0|rep=1|case=1|site={site}|n=1").as_bytes(),
                );
                let finish = digest[0] < 128;
                assert!(
                    matches!(task.lifecycle,Lifecycle::DistributedRunning{due,next,..} if due==finish&&next==usize::from(!finish))
                );
            }
            assert!(
                duration_gate(&n, &m, &mut sample, &mut e, 4096)
                    .unwrap()
                    .is_none()
            );
            assert_eq!(sample.draws.len(), 2);
            // Only survivors reach the last probability-one candidate.
            let survivors = n
                .tasks
                .iter()
                .filter(|t| {
                    matches!(
                        t.lifecycle,
                        Lifecycle::DistributedRunning { due: false, .. }
                    )
                })
                .count();
            n.time = 3;
            let _ = duration_gate(&n, &m, &mut sample, &mut e, 4096).unwrap();
            assert_eq!(sample.draws.len(), 2 + survivors);
            for d in sample.draws.iter().skip(2) {
                assert!(d.key.ends_with("|n=2"));
            }
            tuples.push(tuple);
        }
        assert_eq!(tuples[0], tuples[1]);
    }
    #[test]
    fn s_numeric_cut_unknown_absorbing_facts_and_overcommit_snapshot() {
        let mut m = fixture("sim-hazard");
        let tiny = Rational(num_rational::BigRational::new(
            1.into(),
            (BigInt::from(1u8) << 4096usize) + 1u8,
        ));
        m.capabilities[0]
            .duration_distribution
            .as_mut()
            .unwrap()
            .entries = vec![(1, tiny.clone()), (3, Rational(Rational::one().0 - tiny.0))];
        m.probability_queries[0].within = Some(0);
        let out = simulate(&m);
        assert_eq!(out.completed_replications, 4);
        for r in &out.replications {
            assert_eq!(r.cut, Some(SimulationCut::Numeric));
            assert_eq!(r.cases[0].queries, [QueryTruth::Unknown]);
        }
        let ir::ProbabilityCondition::State(ir::Cond::StateEq(target)) =
            m.probability_queries[0].condition
        else {
            unreachable!()
        };
        m.state_variables[0].initial = ir::InitialValue::Fixed(target.value);
        let out = simulate(&m);
        for r in &out.replications {
            assert_eq!(r.cases[0].queries, [QueryTruth::Hit]);
        }
        for closed in [true, false] {
            for hit in [true, false] {
                assert_eq!(
                    QueryObservation {
                        hit,
                        closed,
                        time: 99,
                        arrival: 0,
                        within: Some(0),
                        cut: Some(SimulationCut::Numeric),
                        clock: 100
                    }
                    .truth(),
                    if hit {
                        QueryTruth::Hit
                    } else if closed {
                        QueryTruth::Miss
                    } else {
                        QueryTruth::Unknown
                    }
                );
            }
        }
        let mut m = fixture("sim-contract");
        m.actors[0].capacity = Some(0);
        let mut s = State::initial(&m, 0);
        s.tasks.push(Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: ir::BindingId(0),
            capability: ir::CapabilityId(0),
            lifecycle: Lifecycle::Running {
                finish: 1,
                ack: None,
            },
        });
        let mut facts = Exploration::default();
        facts.observe(&s, &m).unwrap();
        assert!(facts.overcommit);
        // An unselected branch with an overflowing response deadline must not
        // be applied before its categorical outcome is selected.
        let mut m = fixture("sim-keys");
        let mut s = State::initial(&m, 0);
        s.time = u64::MAX;
        let cap = ir::CapabilityId(0);
        m.contracts = vec![ir::ContractClause {
            ordinal: None,
            name: None,
            span: m.process.span,
            kind: ir::ContractKind::Response {
                trigger: ir::Trigger::Event(ir::Event {
                    capability: cap,
                    kind: ir::EventKind::Finish(Some(ir::ValueId(0))),
                }),
                targets: vec![ir::Target::End(ir::EndId(0))],
                within: 1,
            },
        }];
        s.monitors = vec![Monitor::default()];
        s.tasks = vec![Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: ir::BindingId(0),
            capability: cap,
            lifecycle: Lifecycle::Running {
                finish: u64::MAX,
                ack: None,
            },
        }];
        let mut sample = Sample::new(0, 1);
        let chosen = sample_kernel(&s, 0, &m, &mut sample, &mut Exploration::default()).unwrap();
        assert_eq!(chosen.results[0], Some(ir::ValueId(1)));
    }
    #[test]
    fn s_engine_migration_classification_preflight() {
        const LIN: [&str; 24] = [
            "lin-basis",
            "lin-basis.broken-no-basis",
            "lin-case-latent",
            "lin-case-latent.broken-actor-local",
            "lin-correlation",
            "lin-correlation.broken-unrelated-given",
            "lin-five-kernels",
            "lin-five-kernels.broken-given-only-read",
            "lin-invoice",
            "lin-invoice.broken-po-evidence",
            "lin-no-error-writer",
            "lin-no-error-writer.broken-connect-error-writer",
            "lin-numeric",
            "lin-numeric.broken-zero-detection",
            "lin-same-actor",
            "lin-same-actor.broken-read-removed",
            "lin-same-source-different-target",
            "lin-same-source-different-target.broken-use-same-target",
            "lin-shared-kernel",
            "lin-shared-kernel.broken-drop-call",
            "lin-single",
            "lin-single.broken-second-call",
            "lin-tacit",
            "lin-tacit.broken-unused-tacit",
        ];
        let test = include_str!("../../tests/simulation_differential.rs");
        let direct: Vec<_> = test
            .split("const COMPLETE_DIRECT:")
            .nth(1)
            .unwrap()
            .split("fn migration_complete")
            .next()
            .unwrap()
            .split('"')
            .enumerate()
            .filter_map(|(i, s)| (i % 2 == 1).then_some(s))
            .collect();
        assert_eq!(direct.len(), 50);
        let selected = |name: &str, c: &str| match name {
            "config-diff-add-actor"
            | "config-diff-add-actor.broken-missing-intent"
            | "config-diff-add-actor.broken-other-cut" => ["AsIs", "ToBe", "Hold"].contains(&c),
            "config-diff-parallel"
            | "config-diff-parallel.broken-intent-only"
            | "config-diff-parallel.broken-no-configuration" => c == "AsIs",
            "config-diff-retire"
            | "config-diff-retire.broken-missing-loses"
            | "config-diff-terminal-union"
            | "config-diff-unreachable"
            | "config-diff-unreachable.broken-intent-only" => ["AsIs", "ToBe"].contains(&c),
            "contract-diff" | "contract-diff.broken-zero-deadline" => {
                ["Before", "After"].contains(&c)
            }
            "exec-vacuous" => ["AsIs", "ToBeA"].contains(&c),
            "prob-invoice" | "prob-invoice.broken-ai-transfer" => c == "AsIs",
            _ => false,
        };
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut mismatches = Vec::new();
        let mut count = [0; 3];
        let (mut compare_dirs, mut old_dirs) = (0, 0);
        // D1 is not part of the frozen 148-dir simulation migration population.
        const D1: [&str; 24] = [
            "ctx-excludes",
            "ctx-excludes.broken-start",
            "ctx-nonwaivable",
            "ctx-nonwaivable.broken-waive",
            "ctx-obligation",
            "ctx-obligation.broken-excludes-target",
            "ctx-waive",
            "ctx-waive-authority",
            "ctx-waive-authority.broken-no-authority",
            "ctx-waive-names",
            "ctx-waive-names.broken-unknown-response",
            "ctx-waive.broken-after-deadline",
            "ctx-contest",
            "ctx-contest-branches",
            "ctx-contest-branches.broken-zero-capacity",
            "ctx-contest-clock",
            "ctx-contest-clock.broken-short-clock",
            "ctx-contest-metrics",
            "ctx-contest-metrics.broken-missing-route-rate",
            "ctx-contest-origin",
            "ctx-contest-origin.broken-arrival-clock",
            "ctx-contest-resource",
            "ctx-contest-resource.broken-slow-route",
            "ctx-contest.broken-late",
        ];
        let mut metrics_dirs = 0;
        for dir in std::fs::read_dir(root)
            .unwrap()
            .map(|d| d.unwrap().path())
            .filter(|p| p.is_dir())
        {
            let name = dir.file_name().unwrap().to_str().unwrap();
            let wait_names: &[&str] = include!("../../tests/support/waits_names.rs");
            if wait_names.contains(&name) {
                continue;
            }
            if super::super::record_fixture(name) || ir::rule_fixture(name) {
                continue;
            }
            if crate::contest::population::fixture(name) {
                continue;
            }
            if super::super::policy_population::fixture(name)
                || crate::scenario_population::fixture(name)
                || crate::quality_population::fixture(name)
                || crate::mutation::extension_fixture(name)
                || crate::mutation::review_fixture(name)
                || crate::evaluation_population::fixture(name)
                || crate::quantile_population::fixture(name)
                || include!("../../tests/support/acceptance_gap_names.rs").contains(&name)
                || crate::state_invariant_population::fixture(name)
                || super::super::limits_population::fixture(name)
            {
                continue;
            }
            if super::super::actor_observed_population::fixture(name) {
                continue;
            }
            if super::super::caseset_population::fixture(name)
                || super::super::conflict_population::fixture(name)
            {
                continue;
            }
            if ir::per_case_fixture(name)
                || ir::resource_fixture(name)
                || crate::agency::viewpoints_fixture(name)
                || crate::agency::second_half_fixture(name)
                || crate::integer_tests::fixture(name)
                || crate::diff::extension_population::fixture(name)
            {
                continue;
            }
            if crate::engine::authority_population::fixture(name) {
                continue;
            }
            assert!(
                !name.starts_with("lin-") || LIN.contains(&name),
                "unclassified {name}"
            );
            assert!(
                !name.starts_with("done-") || crate::completion::FIXTURES.contains(&name),
                "unclassified completion fixture {name}"
            );
            if LIN.contains(&name)
                || crate::completion::FIXTURES.contains(&name)
                || requires_fixture(name)
            {
                continue;
            }
            if crate::actor_table::fixture(name) {
                continue;
            }
            if crate::responsibility::extension_population::fixture(name) {
                continue;
            }
            if D1.contains(&name) {
                continue;
            }
            if super::super::legacy_census::explain_fixture(name) {
                continue;
            }
            if super::super::legacy_census::mutation_fixture(name) {
                continue;
            }
            if super::super::legacy_census::metrics_fixture(name) {
                metrics_dirs += 1;
                continue;
            }
            if crate::engine::legacy_census::comparison_fixture(name) {
                compare_dirs += 1;
                continue;
            }
            if SIMULATION_FIXTURES.contains(&name) {
                continue;
            }
            if super::super::legacy_census::responsibility_fixture(name) {
                continue;
            }
            old_dirs += 1;
            let src = std::fs::read_to_string(dir.join("model.relca")).unwrap();
            let at = src.rfind('}').unwrap();
            let src = format!(
                "{}\nsimulation replications 4 seed 0 confidence 0.5\n{}",
                &src[..at],
                &src[at..]
            );
            let Ok(ast) = crate::syntax::parse(&src) else {
                count[2] += 1;
                continue;
            };
            let mut configs: Vec<Option<&str>> = ast
                .items
                .iter()
                .filter_map(|i| {
                    if let crate::ast::Item::Configuration(c) = i {
                        Some(Some(c.name.value.as_str()))
                    } else {
                        None
                    }
                })
                .collect();
            if configs.is_empty() {
                configs.push(None);
            }
            for cfg in configs {
                let m = match cfg {
                    Some(c) => crate::lower::lower_configuration(&ast, c).map(|s| s.model),
                    None => crate::lower::lower(&ast),
                };
                let Ok(m) = m else {
                    count[2] += 1;
                    continue;
                };
                let mut without = m.clone();
                without.simulation = None;
                let census = |model: &ir::Model| {
                    if !model.legacy_route() {
                        return arrivals::compatibility_counts(model);
                    }
                    let mut e = Exploration::default();
                    if model.uses_probability() {
                        let (nodes, _) =
                            probability_graph(model, DEFAULT_STATE_BUDGET, &mut e).unwrap();
                        (nodes.len(), e.census.1, e.census.2)
                    } else {
                        explore_into(
                            model,
                            true,
                            DEFAULT_STATE_BUDGET,
                            std::convert::identity,
                            &mut e,
                        )
                        .unwrap();
                        e.census
                    }
                };
                let measured = census(&without);
                assert_eq!(
                    measured,
                    census(&m),
                    "{name}/{cfg:?}: config does not enter exhaustive identity"
                );
                for budget in 0..=measured.2 + 1 {
                    assert_eq!(
                        explore_with_budget(&without, budget),
                        explore_with_budget(&m, budget),
                        "{name}/{cfg:?}/budget{budget}: config does not affect generation, Room or outcome"
                    );
                }
                let expected = cfg.map_or_else(|| direct.contains(&name), |c| selected(name, c));
                let out = simulate(&m);
                let complete = out.completed_replications == 4;
                count[usize::from(!complete)] += 1;
                if complete != expected {
                    println!(
                        "MIGRATION-MISMATCH {name}/{cfg:?} expected_complete={expected} complete={} diagnostics={:?}",
                        out.completed_replications, out.diagnostics
                    );
                    mismatches.push((name.to_owned(), cfg.map(str::to_owned)));
                }
            }
        }
        println!("MIGRATION-ACTUAL {count:?}");
        assert_eq!((compare_dirs, old_dirs), (16, 148));
        assert_eq!(metrics_dirs, 24);
        assert!(mismatches.is_empty(), "{mismatches:?}");
        assert_eq!(count, [80, 80, 19]);
    }
}
