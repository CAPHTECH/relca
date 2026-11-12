//! Intent verification is a projection over already computed differences. It
//! never manufactures a difference, a duty, or an execution monitor.
use super::static_projection::{Shape, capname};
use super::{Change, DiffReport, Difference, Kind};
use crate::{Verdict, ast, diag, ir, lower, responsibility as resp};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Annotation {
    KeepFallback,
    Preserve {
        selector: String,
        global: bool,
        properties: Vec<String>,
    },
    Override {
        subject_kind: String,
        subject: String,
        verdict: Verdict,
        effects: Vec<EffectWitness>,
        reason: Option<String>,
        unchecked: Vec<String>,
    },
    Maintain {
        person: String,
        links: Vec<Link>,
        reason: Option<String>,
        unlinked_reason: Option<String>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectWitness {
    pub effect: String,
    pub call: String,
    pub judgements: Vec<String>,
    pub stops: String,
    pub bypasses: Option<usize>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub actor: String,
    pub from_duties: Vec<DutyRef>,
    pub to_duties: Vec<DutyRef>,
    pub status: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DutyRef {
    pub id: usize,
    pub person: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preservation {
    pub properties: Vec<PreservedProperty>,
    pub summary: PreservationSummary,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreservedProperty {
    pub id: String,
    pub from: Verdict,
    pub to: Verdict,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreservationSummary {
    pub satisfied: usize,
    pub vacuous: usize,
    pub violated: usize,
    pub inconclusive: usize,
    pub unsupported: usize,
    pub invalid: usize,
}

fn selected(selector: &ast::PropertySelector, m: &ir::Model) -> BTreeSet<String> {
    match selector {
        ast::PropertySelector::Exact(id) => [id.value.clone()].into_iter().collect(),
        ast::PropertySelector::Contract(name) => m
            .contracts
            .iter()
            .enumerate()
            .filter(|(_, c)| c.name.as_deref() == Some(&name.value))
            .map(|(i, c)| c.property_id(ir::ClauseId(i)))
            .chain(
                m.contests()
                    .iter()
                    .filter(|c| c.name.as_deref() == Some(&name.value))
                    .map(|c| c.property_id()),
            )
            .collect(),
    }
}
pub(super) fn preservation(result: &mut DiffReport, to: &lower::SelectedConfiguration) {
    let mut ids = BTreeSet::new();
    let mut present = false;
    for i in &to.intent {
        if let ast::Intent::Preserve { selector, .. } = i {
            present = true;
            ids.extend(selected(selector, &to.model));
        }
    }
    if !present {
        return;
    }
    let mut summary = PreservationSummary::default();
    let mut properties = Vec::new();
    for p in &result.properties {
        if ids.contains(&p.id) {
            match p.to {
                Verdict::Satisfied => summary.satisfied += 1,
                Verdict::Vacuous => summary.vacuous += 1,
                Verdict::Violated => summary.violated += 1,
                Verdict::Inconclusive => summary.inconclusive += 1,
                Verdict::Unsupported => summary.unsupported += 1,
                Verdict::Invalid => summary.invalid += 1,
            }
            properties.push(PreservedProperty {
                id: p.id.clone(),
                from: p.from,
                to: p.to,
            });
        }
    }
    result.preservation = Some(Preservation {
        properties,
        summary,
    });
}
fn matches(target: &ast::DiffTarget, d: &Difference) -> bool {
    target.kind == d.kind && target.name() == d.target
}

pub(super) fn apply(
    result: &mut DiffReport,
    from: &lower::SelectedConfiguration,
    to: &lower::SelectedConfiguration,
) {
    let mut covered = vec![false; result.differences.len()];
    for i in &to.intent {
        let rule_target = match i {
            ast::Intent::Claim { target, .. }
            | ast::Intent::Override { target, .. }
            | ast::Intent::Maintain { target, .. } => Some(target),
            ast::Intent::Preserve { on, .. } => on.as_ref(),
        };
        if rule_target.is_some_and(|t| {
            t.kind == Kind::Rule && !result.differences.iter().any(|d| matches(t, d))
        }) && rule_target.is_some_and(|t| {
            to.model.actors.iter().any(|a| {
                a.name == t.name()
                    && result.diagnostics.iter().any(|d| {
                        d.span == Some(a.span)
                            && d.code == diag::E_CON_0
                            && d.message == "rule version change has no declared amendment"
                    })
            })
        }) {
            continue;
        }
        let (target, span) = match i {
            ast::Intent::Claim { target, span, .. }
            | ast::Intent::Override { target, span, .. }
            | ast::Intent::Maintain { target, span, .. } => (Some(target), *span),
            ast::Intent::Preserve { on, span, .. } => (on.as_ref(), *span),
        };
        let index = target.and_then(|t| {
            result.differences.iter().position(|d| {
                matches(t, d)
                    && match i {
                        ast::Intent::Claim {
                            action: ast::IntentAction::Add,
                            ..
                        } => d.change == Change::Added,
                        ast::Intent::Claim {
                            action: ast::IntentAction::Retire,
                            ..
                        } => d.change == Change::Removed,
                        _ => true,
                    }
            })
        });
        if let Some(t) = target {
            if let Some(index) = index {
                covered[index] = true;
            } else {
                result.diagnostic(
                    diag::E_DIFF_0,
                    format!("intent for {} has no matching computed change", t.name()),
                    Some(span),
                );
            }
        }
        match i {
            ast::Intent::Claim {
                action,
                target,
                reason,
                loses,
                span,
                keep_fallback,
            } => {
                if let Some(index) = index {
                    let d = &mut result.differences[index];
                    d.reason = reason.as_ref().map(|r| r.value.clone());
                    d.loses = loses.as_ref().map(|s| s.value.clone());
                    if *keep_fallback {
                        d.annotations.push(Annotation::KeepFallback);
                    }
                }
                if *action == ast::IntentAction::Retire && loses.is_none() && !keep_fallback {
                    result.diagnostic(
                        diag::W_DIFF_2,
                        format!("retire {} does not record loses", target.name()),
                        Some(*span),
                    );
                }
            }
            ast::Intent::Preserve { selector, on, .. } => {
                let ids = selected(selector, &to.model);
                let properties: Vec<_> = result
                    .to
                    .report
                    .properties
                    .iter()
                    .filter(|p| ids.contains(&p.id))
                    .map(|p| p.id.clone())
                    .collect();
                for (n, d) in result.differences.iter_mut().enumerate() {
                    if on.is_none() || Some(n) == index {
                        d.annotations.push(Annotation::Preserve {
                            selector: selector.value().into(),
                            global: on.is_none(),
                            properties: properties.clone(),
                        });
                    }
                }
            }
            ast::Intent::Override {
                subject, reason, ..
            } => {
                if let Some(index) = index {
                    let a = override_check(
                        &result.differences[index],
                        &to.model,
                        &subject.value,
                        reason.as_ref().map(|r| r.value.clone()),
                    );
                    result.differences[index].annotations.push(a);
                }
            }
            ast::Intent::Maintain { person, reason, .. } => {
                if let Some(index) = index {
                    let a = maintain(
                        &result.differences[index],
                        &from.model,
                        &to.model,
                        &person.value,
                        reason.as_ref().map(|r| r.value.clone()),
                    );
                    result.differences[index].annotations.push(a);
                }
            }
        }
    }
    for d in &mut result.differences {
        d.annotations.sort_by_key(|a| match a {
            Annotation::Preserve {
                selector, global, ..
            } => (0, !*global, selector.clone()),
            Annotation::Override { .. } => (1, false, String::new()),
            Annotation::Maintain { .. } => (2, false, String::new()),
            Annotation::KeepFallback => (3, false, String::new()),
        });
        if d.reason.is_none() {
            d.reason = d.annotations.iter().find_map(|a| match a {
                Annotation::Override { reason, .. } | Annotation::Maintain { reason, .. } => {
                    reason.clone()
                }
                _ => None,
            });
        }
    }
    // Preserve legacy diagnostic order and wording; new diagnostics are a suffix.
    let missing: Vec<_> = result
        .differences
        .iter()
        .zip(covered)
        .filter(|(_, c)| !c)
        .map(|(d, _)| (d.kind, d.target.clone()))
        .collect();
    for (kind, target) in missing {
        let message = if matches!(kind, Kind::Actor | Kind::Relation) {
            format!("computed change for {target} has no intent")
        } else {
            format!("computed change for {} {target} has no intent", kind.name())
        };
        result.diagnostic(diag::W_DIFF_1, message, None);
    }
    let unannotated: Vec<_> = result
        .differences
        .iter()
        .filter(|d| d.annotations.is_empty() && d.loses.is_none())
        .map(|d| (d.kind, d.target.clone()))
        .collect();
    for (kind, target) in unannotated {
        result.diagnostic(diag::W_DIFF_3,format!("computed change for {} {target} has no preserve, override, maintain or loses annotation",kind.name()),None);
    }
}

fn related(d: &Difference, m: &ir::Model) -> BTreeSet<ir::CapabilityId> {
    let shape = Shape::new(m);
    let active = shape.active(m);
    match d.kind {
        Kind::Rule | Kind::Qualification | Kind::Performance | Kind::Degradation => BTreeSet::new(),
        Kind::Relation | Kind::Authority | Kind::Time => active
            .into_iter()
            .filter(|c| capname(m, *c) == d.target)
            .collect(),
        Kind::Actor | Kind::Resource => active
            .into_iter()
            .filter(|c| m.actors[m.capabilities[c.0].actor.0].name == d.target)
            .collect(),
        Kind::Visibility => active
            .into_iter()
            .filter(|c| {
                m.capabilities[c.0]
                    .reads
                    .iter()
                    .any(|a| m.actors[a.0].name == d.target)
            })
            .collect(),
        Kind::Meaning | Kind::Contest => {
            let mut caps = BTreeSet::new();
            if let Some(end) = m.ends.iter().position(|e| format!("end.{e}") == d.target)
                && shape.ends.contains(&ir::EndId(end))
            {
                if let Some(cs) = crate::contest::control_dependencies(m)
                    .terminals
                    .get(&ir::EndId(end))
                {
                    caps.extend(cs);
                }
                if d.kind == Kind::Contest {
                    caps.extend(
                        m.contests()
                            .iter()
                            .filter(|c| c.target.end() == Some(ir::EndId(end)))
                            .map(|c| c.route),
                    );
                }
            }
            if d.kind == Kind::Contest {
                let control = crate::contest::control_dependencies(m);
                for c in m.contests().iter().filter(|c| {
                    c.target.effect() && c.target.name(m) == d.target && c.target.active(m)
                }) {
                    caps.extend(crate::contest::candidates(m, c.target, &control));
                    caps.insert(c.route);
                }
            }
            caps
        }
    }
}
fn duties(m: &ir::Model, actor: &str) -> Vec<DutyRef> {
    m.responsibility
        .iter()
        .flat_map(|meta| meta.duties.iter().enumerate())
        .filter_map(|(i, d)| {
            if let resp::Target::Maintenance(a) = d.target
                && m.actors[a.0].name == actor
            {
                Some(DutyRef {
                    id: i + 1,
                    person: m.people[d.person.0].name.clone(),
                })
            } else {
                None
            }
        })
        .collect()
}
fn maintain(
    d: &Difference,
    from: &ir::Model,
    to: &ir::Model,
    person: &str,
    reason: Option<String>,
) -> Annotation {
    let mut actors: BTreeSet<_> = related(d, to)
        .into_iter()
        .map(|c| to.actors[to.capabilities[c.0].actor.0].name.clone())
        .chain(
            related(d, from)
                .into_iter()
                .map(|c| from.actors[from.capabilities[c.0].actor.0].name.clone()),
        )
        .collect();
    // A visibility target is the source being maintained, not the consuming
    // Actors whose reads happen to have changed.
    if d.kind == Kind::Visibility {
        actors.clear();
    }
    if matches!(
        d.kind,
        Kind::Actor | Kind::Resource | Kind::Visibility | Kind::Rule
    ) {
        actors.insert(d.target.clone());
    }
    let links: Vec<_> = actors
        .into_iter()
        .map(|actor| {
            let from_duties = duties(from, &actor);
            let to_duties = duties(to, &actor);
            let status = if to_duties.iter().any(|d| d.person == person) {
                "linked"
            } else if to_duties.is_empty() {
                "unlinked"
            } else {
                "conflict"
            }
            .into();
            Link {
                actor,
                from_duties,
                to_duties,
                status,
            }
        })
        .collect();
    Annotation::Maintain {
        person: person.into(),
        unlinked_reason: links.is_empty().then(|| "no_actor_derived".into()),
        links,
        reason,
    }
}

/// Static derivation bound for the override U check (control cells and the
/// argument-ancestry walk). Reaching it is unknown, never success or failure.
const OVERRIDE_LIMIT: usize = 1_000_000;

fn override_check(
    d: &Difference,
    m: &ir::Model,
    subject: &str,
    reason: Option<String>,
) -> Annotation {
    override_check_within(d, m, subject, reason, OVERRIDE_LIMIT, OVERRIDE_LIMIT)
}

/// An effect for the U check: a commit or any state write.
fn effect_like(cap: &ir::Capability) -> bool {
    cap.commits
        || !cap.writes.is_empty()
        || !cap.start_writes.is_empty()
        || !cap.kernel_writes.is_empty()
}
fn unknown_witness(m: &ir::Model, call: &ir::Call) -> EffectWitness {
    EffectWitness {
        effect: capname(m, call.capability),
        call: m.process.bindings[call.binding.0].name.clone(),
        judgements: vec![],
        stops: "unknown".into(),
        bypasses: None,
    }
}
/// Whether a person may execute a judgement: an executor of its Actor and
/// within its Permission (permit union, else the executors). An anonymous
/// Actor cannot show membership either way unless a permit excludes the person.
#[derive(PartialEq)]
enum Qualification {
    Yes,
    No,
    Unknown,
}
fn qualification(m: &ir::Model, cap: &ir::Capability, p: ir::PersonId) -> Qualification {
    match &m.actors[cap.actor.0].executors {
        Some(executors) => {
            let permission = cap.permission.as_ref().unwrap_or(executors);
            if executors.contains(&p) && permission.contains(&p) {
                Qualification::Yes
            } else {
                Qualification::No
            }
        }
        None if cap.permission.as_ref().is_some_and(|ps| !ps.contains(&p)) => Qualification::No,
        None => Qualification::Unknown,
    }
}

fn override_check_within(
    d: &Difference,
    m: &ir::Model,
    subject: &str,
    reason: Option<String>,
    cell_limit: usize,
    work_limit: usize,
) -> Annotation {
    let person = m
        .people
        .iter()
        .position(|p| p.name == subject)
        .map(ir::PersonId);
    let kind = if person.is_some() { "person" } else { "actor" };
    let unchecked = vec!["override_authority_validity".into()];
    let shape = Shape::new(m);
    let caps = related(d, m);
    let seeds: BTreeSet<_> = shape
        .calls
        .values()
        .filter(|c| caps.contains(&c.capability))
        .map(|c| c.binding)
        .collect();
    let calls: Vec<_> = shape.calls.values().collect();
    let mut unknown = false;
    let mut failed = false;
    let mut effects = Vec::new();
    if m.agency.as_derived().is_none() || !crate::contest::control_within_limits(m, cell_limit) {
        // Candidates cannot be derived: every effect-like call is unknown.
        unknown = true;
        effects.extend(
            calls
                .iter()
                .filter(|c| effect_like(&m.capabilities[c.capability.0]))
                .map(|c| unknown_witness(m, c)),
        );
    } else {
        let control = crate::contest::control_dependencies(m);
        let state_unknown = m
            .agency
            .as_derived()
            .is_none_or(|a| a.unchecked.iter().any(|u| u == "state_guards"));
        let mut work = 0usize;
        for (index, call) in calls.iter().enumerate() {
            let mut ancestry = BTreeSet::new();
            let mut pending = vec![call.binding];
            while let Some(b) = pending.pop() {
                work += 1;
                if work > work_limit {
                    break;
                }
                if ancestry.insert(b)
                    && let Some(c) = shape.calls.get(&b)
                {
                    pending.extend(c.args.iter().copied().filter(|a| *a != ir::CURRENT_CASE));
                }
            }
            if work > work_limit {
                // This and every later call are undecided: effect-like ones
                // keep an unknown witness instead of disappearing.
                unknown = true;
                effects.extend(
                    calls[index..]
                        .iter()
                        .filter(|c| effect_like(&m.capabilities[c.capability.0]))
                        .map(|c| unknown_witness(m, c)),
                );
                break;
            }
            let refusal = m
                .agency
                .as_derived()
                .into_iter()
                .flat_map(|a| &a.refusal)
                .find(|r| r.call == m.process.bindings[call.binding.0].name);
            let candidate = !ancestry.is_disjoint(&seeds)
                || control
                    .calls
                    .get(&call.binding)
                    .is_some_and(|cs| cs.iter().any(|c| caps.contains(c)))
                || refusal.is_some_and(|r| {
                    r.judgements
                        .iter()
                        .filter_map(crate::agency::Judgement::process)
                        .any(|j| seeds.iter().any(|b| m.process.bindings[b.0].name == j.call))
                });
            if !candidate {
                continue;
            }
            let cap = &m.capabilities[call.capability.0];
            if !cap.commits {
                if effect_like(cap) {
                    unknown = true;
                    effects.push(unknown_witness(m, call));
                }
                continue;
            }
            let mut witness = unknown_witness(m, call);
            if let Some(r) = refusal
                && r.reachable
                && !state_unknown
            {
                let judged = |j: &crate::agency::ProcessJudgement| {
                    let Some(c) = shape
                        .calls
                        .values()
                        .find(|c| m.process.bindings[c.binding.0].name == j.call)
                    else {
                        return Qualification::No;
                    };
                    let cap = &m.capabilities[c.capability.0];
                    if let Some(p) = person {
                        qualification(m, cap, p)
                    } else if m.actors[cap.actor.0].name == subject {
                        Qualification::Yes
                    } else {
                        Qualification::No
                    }
                };
                let stops_all = |j: &crate::agency::ProcessJudgement| {
                    j.stops == crate::agency::Stops::AllPaths && j.bypasses == 0
                };
                let qualified: Vec<_> = r
                    .judgements
                    .iter()
                    .filter_map(crate::agency::Judgement::process)
                    .filter(|j| judged(j) == Qualification::Yes)
                    .collect();
                // A judgement that would stop the effect but whose executors
                // are anonymous cannot be credited or refuted for a person.
                let undetermined = r
                    .judgements
                    .iter()
                    .filter_map(crate::agency::Judgement::process)
                    .filter(|j| judged(j) == Qualification::Unknown)
                    .any(stops_all);
                if qualified.iter().any(|j| stops_all(j)) {
                    witness.judgements = qualified
                        .iter()
                        .map(|j| j.call.clone())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect();
                    witness.stops = "all_paths".into();
                    witness.bypasses = Some(0);
                } else if undetermined {
                    unknown = true;
                } else {
                    failed = true;
                    witness.judgements = qualified
                        .iter()
                        .map(|j| j.call.clone())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect();
                    witness.stops = if qualified
                        .iter()
                        .any(|j| j.stops == crate::agency::Stops::Conditional)
                    {
                        "conditional"
                    } else {
                        "none"
                    }
                    .into();
                    // With no qualified judgement there is no derived bypass
                    // counter. Lack of U is a violation, not a fabricated count.
                    witness.bypasses = qualified.iter().map(|j| j.bypasses).min();
                }
            } else {
                unknown = true;
            }
            effects.push(witness);
        }
    }
    if effects.is_empty() && d.change != Change::Removed {
        // No effect could be found for a difference that still exists: the
        // target's own calls are the unknown witnesses.
        unknown = true;
        effects.extend(
            calls
                .iter()
                .filter(|c| seeds.contains(&c.binding))
                .map(|c| unknown_witness(m, c)),
        );
    }
    effects.sort_by(|a, b| (&a.effect, &a.call).cmp(&(&b.effect, &b.call)));
    let verdict = if failed {
        Verdict::Violated
    } else if unknown {
        Verdict::Unsupported
    } else if effects.is_empty() {
        Verdict::Vacuous
    } else {
        Verdict::Satisfied
    };
    Annotation::Override {
        subject_kind: kind.into(),
        subject: subject.into(),
        verdict,
        effects,
        reason,
        unchecked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r1_unavailable_agency_cannot_make_a_removed_override_vacuous() {
        let mut sources = vec![crate::agency::limit_test_inputs::work()];
        // 1500*1499/2 read-graph comparisons exceed its 1M work ceiling.
        // No writes or commits: there are genuinely zero effect-like calls.
        sources.push(format!("model NoEffects version \"0.2\" {{ type V=enum{{pass}} observed case {{x:V=pass}} actor A{{capacity 1; capability read()->V{{reads[x];duration fixed 1min;result nondet{{pass}}}}}} process Flow{{{} end done}} cut C{{include[A]}} bounds{{cases 1;live_tasks 2;clock 2000min}} verify safety no_resource_overcommit }}", (0..1500).map(|i| format!("a{i}=A.read();")).collect::<String>()));
        for source in sources {
            let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
            assert!(m.agency.limit().is_some());
            let d = Difference {
                kind: Kind::Relation,
                target: "Retired.old".into(),
                change: Change::Removed,
                reason: None,
                loses: None,
                from: None,
                to: None,
                annotations: vec![],
            };
            let Annotation::Override {
                verdict, effects, ..
            } = override_check_within(&d, &m, "nobody", None, usize::MAX, usize::MAX)
            else {
                panic!()
            };
            assert_eq!(verdict, Verdict::Unsupported);
            let expected = crate::responsibility::calls(&m)
                .values()
                .filter(|c| effect_like(&m.capabilities[c.capability.0]))
                .count();
            assert_eq!(effects.len(), expected);
            assert!(
                effects
                    .iter()
                    .all(|e| e.stops == "unknown" && e.bypasses.is_none())
            );
        }
    }

    #[test]
    fn q4_override_cell_boundary_does_not_count_condition_walk_twice() {
        let src = "model Limits version \"0.2\" { type V = enum { pass } people [p] actor A { executors [p]; capability a() -> V { duration fixed 1min; result nondet { pass } } } actor B { capacity 1; capability b() -> V { duration fixed 1min; result nondet { pass } } } actor P { capacity 1; capability post(v:V) -> V { commits; duration fixed 1min; result nondet { pass } } } process Flow { x = A.a(); z = B.b(); y = P.post(z); end done } cut C { include [A,B,P] } bounds { cases 1; live_tasks 2; clock 20min } verify safety no_resource_overcommit }";
        let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let difference = Difference {
            kind: Kind::Relation,
            target: "B.b".into(),
            change: Change::Added,
            reason: None,
            loses: None,
            from: None,
            to: None,
            annotations: vec![],
        };
        for wait in [false, true] {
            let mut m = m.clone();
            if wait {
                let span = m.process.body[0].span;
                m.process.body.push(ir::Statement {
                    kind: ir::Stmt::WaitUntil(ir::Cond::Eq(ir::ResultMatch {
                        binding: ir::BindingId(0),
                        value: ir::ValueId(0),
                    })),
                    span,
                });
            }
            for (limit, expected) in [(531, Verdict::Unsupported), (532, Verdict::Violated)] {
                let Annotation::Override {
                    verdict, effects, ..
                } = override_check_within(&difference, &m, "p", None, limit, usize::MAX)
                else {
                    panic!()
                };
                assert_eq!(verdict, expected, "wait={wait}, limit={limit}");
                assert_eq!(effects.len(), 1);
                assert_eq!(
                    effects[0].stops,
                    if limit == 531 { "unknown" } else { "none" }
                );
            }
        }
    }

    // Boundary review (ADR 0031): the argument-ancestry work bound is not
    // reachable through a public model before the Agency bound, so both limits
    // are lowered here. Hand: the target B.b (call z) is not an effect; the
    // only effect-like call is y = P.post(z). Without limits y has no judgement
    // at all -> none, violated.
    #[test]
    fn override_limits_keep_unknown_witnesses_for_undecided_effects() {
        let src = "model Limits version \"0.2\" {\n  type V = enum { pass }\n  people [p]\n  actor A { executors [p]; capability a() -> V { duration fixed 1min; result nondet { pass } } }\n  actor B { capacity 1; capability b() -> V { duration fixed 1min; result nondet { pass } } }\n  actor P { capacity 1; capability post(v:V) -> V { commits; duration fixed 1min; result nondet { pass } } }\n  process Before { x = A.a(); end done }\n  process After { x = A.a(); z = B.b(); y = P.post(z); end done }\n  cut C { include [A,B,P] }\n  configuration From { cut C; process Before }\n  configuration To { cut C; process After }\n  bounds { cases 1; live_tasks 2; clock 20min }\n  verify safety no_resource_overcommit\n}\n";
        let result = super::super::diff(src, "From", "To");
        let d = result
            .differences
            .iter()
            .find(|d| d.kind == Kind::Relation && d.target == "B.b")
            .unwrap();
        let m = crate::report::check_selected(src, "To").1.unwrap().model;
        let x = EffectWitness {
            effect: "P.post".into(),
            call: "y".into(),
            judgements: vec![],
            stops: "unknown".into(),
            bypasses: None,
        };
        for (cells, work) in [(0, usize::MAX), (usize::MAX, 2)] {
            let Annotation::Override {
                verdict, effects, ..
            } = override_check_within(d, &m, "p", None, cells, work)
            else {
                panic!()
            };
            assert_eq!(verdict, Verdict::Unsupported, "{cells}/{work}");
            assert_eq!(effects, std::slice::from_ref(&x), "{cells}/{work}");
        }
        let Annotation::Override {
            verdict, effects, ..
        } = override_check_within(d, &m, "p", None, usize::MAX, usize::MAX)
        else {
            panic!()
        };
        assert_eq!(verdict, Verdict::Violated);
        assert_eq!(effects.len(), 1);
        assert_eq!(
            (effects[0].call.as_str(), effects[0].stops.as_str()),
            ("y", "none")
        );
    }

    #[test]
    fn visibility_maintenance_links_source_not_its_readers() {
        let src = include_str!("../../../../fixtures/diff2-visibility/model.relca")
            .replace("  type V", "  people [s]\n  type V")
            .replace(
                "process After }",
                "process After; intent { maintain visibility S by s } }",
            );
        let result = super::super::diff(&src, "From", "To");
        assert_eq!(result.verdict, Verdict::Satisfied);
        let row = result
            .differences
            .iter()
            .find(|d| d.kind == Kind::Visibility)
            .unwrap();
        let Annotation::Maintain { links, .. } = &row.annotations[0] else {
            panic!("maintenance annotation")
        };
        assert_eq!(
            links.iter().map(|l| l.actor.as_str()).collect::<Vec<_>>(),
            ["S"]
        );
        assert_eq!(links[0].status, "unlinked");
    }

    #[test]
    fn namespace_errors_do_not_reclassify_legacy_add_or_retire() {
        let src = include_str!("../../../../fixtures/diff2-override/model.relca");
        for (intent, code) in [
            ("change actor p reason \"x\"", diag::E_CON_0),
            ("add p reason \"x\"", diag::E_NAME_0),
            ("retire p reason \"x\"", diag::E_NAME_0),
            ("maintain actor P by P", diag::E_CON_0),
            ("override relation P.post by missing", diag::E_NAME_0),
            ("change reason \"x\"", diag::E_SYN_0),
            ("change unknown P reason \"x\"", diag::U_SYN_0),
        ] {
            let report = crate::report::check_configuration(
                &src.replace("override relation P.post by p", intent),
                "From",
            );
            assert!(
                report.diagnostics.iter().any(|d| d.code == code),
                "{intent}: {:?}",
                report.diagnostics
            );
        }
    }

    #[test]
    fn unknown_witnesses_and_person_without_qualification_do_not_invent_counts() {
        let src = include_str!("../../../../fixtures/diff2-override/model.relca");
        let missing_person = super::super::diff(&src.replace("by p }", "by q }"), "From", "To");
        let row = missing_person
            .differences
            .iter()
            .find(|d| d.kind == Kind::Relation)
            .unwrap();
        let Annotation::Override {
            verdict,
            effects,
            unchecked,
            ..
        } = &row.annotations[0]
        else {
            panic!()
        };
        assert_eq!(*verdict, Verdict::Violated);
        assert_eq!(effects[0].bypasses, None);
        assert!(effects[0].judgements.is_empty());
        assert_eq!(unchecked, &["override_authority_validity"]);
        let noncommit = src
            .replace(
                "  type V",
                "  observed case { flag: bool = false }\n  type V",
            )
            .replace(
                "commits; duration",
                "writes flag; finish { flag = true }; duration",
            );
        let unknown = super::super::diff(&noncommit, "From", "To");
        let row = unknown
            .differences
            .iter()
            .find(|d| d.kind == Kind::Relation)
            .unwrap();
        let Annotation::Override {
            verdict,
            effects,
            unchecked,
            ..
        } = &row.annotations[0]
        else {
            panic!()
        };
        assert_eq!(*verdict, Verdict::Unsupported);
        assert_eq!(effects[0].stops, "unknown");
        assert_eq!(effects[0].bypasses, None);
        assert!(effects[0].judgements.is_empty());
        assert_eq!(unchecked, &["override_authority_validity"]);
    }
}
