//! ADR 0018 static metadata. None of these types enter a runtime state key.
use crate::{agency::Stops, diag, ir};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
#[path = "../tests/support/responsibility_extension_population.rs"]
pub(crate) mod extension_population;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Metadata {
    pub parties: Vec<ir::ActorId>,
    pub standings: Vec<Standing>,
    pub duties: Vec<Duty>,
    pub contests: Vec<crate::contest::Contest>,
    pub responsibilities: Vec<Responsibility>,
    pub contest_paths: Vec<crate::contest::ContestPath>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    pub actor: ir::ActorId,
    pub target: crate::contest::ContestTarget,
    pub basis: String,
    pub span: diag::Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Duty {
    pub person: ir::PersonId,
    pub target: Target,
    pub within: Option<ir::Tick>,
    pub span: diag::Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Capability(ir::CapabilityId),
    Condition(ir::Cond),
    Formula(Formula),
    Maintenance(ir::ActorId),
}

/// Responsibility-only analytical formula, never evaluated by runtime control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Formula {
    State(ir::Cond),
    Bool(ir::StateMatch),
    Not(Box<Formula>),
    And(Vec<Formula>),
    Or(Vec<Formula>),
}

pub(crate) fn same_target(
    types: &[ir::EnumType],
    variables: &[ir::StateVariable],
    a: &Target,
    b: &Target,
) -> bool {
    if a == b {
        return true;
    }
    if !matches!(a, Target::Formula(_)) && !matches!(b, Target::Formula(_)) {
        return false;
    }
    fn negate(types: &[ir::EnumType], variables: &[ir::StateVariable], c: Formula) -> Formula {
        match c {
            Formula::Not(c) => *c,
            Formula::And(cs) => Formula::Or(
                cs.into_iter()
                    .map(|c| negate(types, variables, c))
                    .collect(),
            ),
            Formula::Or(cs) => Formula::And(
                cs.into_iter()
                    .map(|c| negate(types, variables, c))
                    .collect(),
            ),
            Formula::State(ir::Cond::StateEq(x))
                if !variables[x.variable.0].optional
                    && types[variables[x.variable.0].base.0].members.len() == 2 =>
            {
                let ir::StateValue::Value(value) = x.value else {
                    unreachable!()
                };
                Formula::State(ir::Cond::StateEq(ir::StateMatch {
                    variable: x.variable,
                    value: ir::StateValue::Value(ir::ValueId(1 - value.0)),
                }))
            }
            c => Formula::Not(Box::new(c)),
        }
    }
    fn normalized(types: &[ir::EnumType], variables: &[ir::StateVariable], f: &Formula) -> Formula {
        match f {
            Formula::Bool(x) => Formula::State(ir::Cond::StateEq(*x)),
            Formula::State(ir::Cond::And(cs)) => Formula::And(
                cs.iter()
                    .map(|c| normalized(types, variables, &Formula::State(c.clone())))
                    .collect(),
            ),
            Formula::State(ir::Cond::Or(cs)) => Formula::Or(
                cs.iter()
                    .map(|c| normalized(types, variables, &Formula::State(c.clone())))
                    .collect(),
            ),
            Formula::Not(c) => {
                let c = normalized(types, variables, c);
                negate(types, variables, c)
            }
            Formula::And(cs) => {
                Formula::And(cs.iter().map(|c| normalized(types, variables, c)).collect())
            }
            Formula::Or(cs) => {
                Formula::Or(cs.iter().map(|c| normalized(types, variables, c)).collect())
            }
            _ => f.clone(),
        }
    }
    let formula = |t: &Target| match t {
        Target::Formula(f) => Some(normalized(types, variables, f)),
        Target::Condition(c) => Some(normalized(types, variables, &Formula::State(c.clone()))),
        _ => None,
    };
    match (formula(a), formula(b)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Responsibility {
    pub id: usize,
    pub rules: String,
    pub person: String,
    pub target: TargetDescription,
    pub actors: Vec<String>,
    pub observation: Observation,
    pub refusal: Refusal,
    pub time_frame: TimeFrame,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetDescription {
    pub kind: String,
    pub value: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub status: String,
    pub required_sources: Vec<String>,
    pub direct: Vec<String>,
    pub indirect: Vec<String>,
    pub missing_sources: Vec<String>,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Refusal {
    pub status: String,
    pub effects: Vec<Effect>,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub capability: String,
    pub call: String,
    pub status: String,
    pub judgements: Vec<Judgement>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Judgement {
    pub actor: String,
    pub capability: String,
    pub call: String,
    pub stops: Stops,
    pub bypasses: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeFrame {
    pub status: String,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub details: Option<TimeDetails>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeDetails {
    pub within: ir::Tick,
    pub required_duration: Option<ir::Tick>,
    pub basis: String,
    pub scope: String,
    pub reason: Option<String>,
    pub paths: Vec<TimePath>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimePath {
    pub kind: String,
    pub source: Option<String>,
    pub judgement_call: Option<String>,
    pub effect_call: Option<String>,
    pub calls: Vec<String>,
    pub duration_upper: ir::Tick,
}

impl TimeFrame {
    fn unchecked(within: Option<ir::Tick>, reason: &str) -> Self {
        Self {
            status: "unchecked".into(),
            details: within.map(|within| TimeDetails {
                within,
                required_duration: None,
                basis: "static_capability_path_upper".into(),
                scope: "static_responsibility_reachability".into(),
                reason: Some(reason.into()),
                paths: vec![],
            }),
        }
    }
}

pub(crate) fn invalid() -> diag::Diagnostic {
    diag::Diagnostic {
        code: diag::U_SEM_0.into(),
        severity: diag::Severity::Error,
        message: "invalid responsibility/contest static IR".into(),
        span: None,
    }
}
pub(crate) fn warning(code: &str, message: String, span: diag::Span) -> diag::Diagnostic {
    diag::Diagnostic {
        code: code.into(),
        severity: diag::Severity::Warning,
        message,
        span: Some(span),
    }
}
pub(crate) fn cap_name(m: &ir::Model, id: ir::CapabilityId) -> String {
    let c = &m.capabilities[id.0];
    format!("{}.{}", m.actors[c.actor.0].name, c.name)
}
pub(crate) fn included(m: &ir::Model, a: ir::ActorId) -> bool {
    m.cut.as_ref().is_none_or(|c| c.include.contains(&a))
}

#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub depth: usize,
    pub work: usize,
    pub cells: usize,
}
const DEFAULT: Limits = Limits {
    depth: 128,
    work: 1_000_000,
    cells: 1_000_000,
};
fn row_limits(_index: usize) -> Limits {
    #[cfg(test)]
    if let Some(limit) =
        TEST_LIMITS.with(|v| v.borrow().as_ref().and_then(|v| v.get(_index)).copied())
    {
        return limit;
    }
    DEFAULT
}
#[cfg(test)]
thread_local! {static TEST_LIMITS: std::cell::RefCell<Option<Vec<Limits>>> = const {std::cell::RefCell::new(None)};}
struct Budget {
    limits: Limits,
    work: usize,
    cells: usize,
}
type Limited<T> = Result<T, ()>;
impl Budget {
    fn add(counter: &mut usize, n: usize, max: usize) -> Limited<()> {
        *counter = counter.checked_add(n).filter(|v| *v <= max).ok_or(())?;
        Ok(())
    }
    fn charge(&mut self, work: usize, cells: usize) -> Limited<()> {
        Self::add(&mut self.work, work, self.limits.work)?;
        Self::add(&mut self.cells, cells, self.limits.cells)
    }
    fn depth(&mut self, n: usize) -> Limited<()> {
        if n > self.limits.depth {
            Err(())
        } else {
            self.charge(1, 0)
        }
    }
}

pub(crate) fn calls(m: &ir::Model) -> BTreeMap<ir::BindingId, &ir::Call> {
    fn visit<'a>(b: &'a ir::Block, out: &mut BTreeMap<ir::BindingId, &'a ir::Call>) {
        for s in b {
            match &s.kind {
                ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => {
                    out.insert(c.binding, c);
                }
                ir::Stmt::Fork { branches, .. } => {
                    for c in branches {
                        out.insert(c.binding, c);
                    }
                }
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    visit(then_body, out);
                    visit(else_body, out);
                }
                _ => {}
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(&m.process.body, &mut out);
    out
}
fn support(
    c: &ir::Cond,
    out: &mut BTreeSet<ir::StateVarId>,
    budget: &mut Budget,
    depth: usize,
) -> Limited<()> {
    budget.depth(depth)?;
    match c {
        ir::Cond::IntCompare { variable, .. } | ir::Cond::Contains { set: variable, .. } => {
            budget.charge(1, 1)?;
            out.insert(*variable);
        }
        ir::Cond::StateEq(x) => {
            budget.charge(1, 1)?;
            out.insert(x.variable);
        }
        ir::Cond::And(cs) | ir::Cond::Or(cs) => {
            for c in cs {
                support(c, out, budget, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn formula_support(
    f: &Formula,
    out: &mut BTreeSet<ir::StateVarId>,
    budget: &mut Budget,
    depth: usize,
) -> Limited<()> {
    if let Formula::State(c) = f {
        return support(c, out, budget, depth);
    }
    budget.depth(depth)?;
    match f {
        Formula::Bool(x) => {
            budget.charge(1, 1)?;
            out.insert(x.variable);
        }
        Formula::Not(c) => formula_support(c, out, budget, depth + 1)?,
        Formula::And(cs) | Formula::Or(cs) => {
            for c in cs {
                formula_support(c, out, budget, depth + 1)?;
            }
        }
        Formula::State(_) => unreachable!(),
    }
    Ok(())
}
pub(crate) fn decision_support(
    c: &ir::Cond,
    m: &ir::Model,
    cs: &BTreeMap<ir::BindingId, &ir::Call>,
    out: &mut BTreeSet<ir::CapabilityId>,
) {
    match c {
        ir::Cond::IntCompare { variable, .. } | ir::Cond::Contains { set: variable, .. } => {
            for c in cs.values() {
                if writes(&m.capabilities[c.capability.0], *variable) {
                    out.insert(c.capability);
                }
            }
        }
        ir::Cond::Eq(x) => {
            if let Some(c) = cs.get(&x.binding) {
                out.insert(c.capability);
            }
        }
        ir::Cond::StateEq(x) => {
            for c in cs.values() {
                if writes(&m.capabilities[c.capability.0], x.variable) {
                    out.insert(c.capability);
                }
            }
        }
        ir::Cond::And(parts) | ir::Cond::Or(parts) => {
            for c in parts {
                decision_support(c, m, cs, out);
            }
        }
    }
}
fn writes(c: &ir::Capability, v: ir::StateVarId) -> bool {
    c.finish_writes().any(|w| w.variable == v)
        || c.start_writes.iter().any(|w| {
            w.variable == v
                && matches!(
                    w.source,
                    ir::WriteSource::Executor | ir::WriteSource::InsertCase { .. }
                )
        })
        || c.kernel_writes.contains(&v)
        || c.structural_kernel().is_some_and(|k| {
            k.rows
                .iter()
                .flat_map(|r| &r.branches)
                .any(|b| b.overrides.iter().any(|(id, _)| *id == v))
        })
}

fn sources(
    m: &ir::Model,
    cs: &BTreeMap<ir::BindingId, &ir::Call>,
    b: ir::BindingId,
    seen: &mut BTreeSet<ir::BindingId>,
    out: &mut BTreeSet<ir::ActorId>,
    depth: usize,
    budget: &mut Budget,
) -> Limited<()> {
    budget.depth(depth)?;
    if seen.contains(&b) {
        return Ok(());
    }
    budget.charge(1, 1)?;
    seen.insert(b);
    let c = cs.get(&b).ok_or(())?;
    for a in &m.capabilities[c.capability.0].reads {
        budget.charge(1, 1)?;
        out.insert(*a);
    }
    for a in &c.args {
        if *a == ir::CURRENT_CASE {
            continue;
        }
        sources(m, cs, *a, seen, out, depth + 1, budget)?;
    }
    Ok(())
}
fn ancestors(
    cs: &BTreeMap<ir::BindingId, &ir::Call>,
    b: ir::BindingId,
    out: &mut BTreeSet<ir::BindingId>,
    depth: usize,
    budget: &mut Budget,
) -> Limited<()> {
    budget.depth(depth)?;
    if out.contains(&b) {
        return Ok(());
    }
    budget.charge(1, 1)?;
    out.insert(b);
    for a in &cs.get(&b).ok_or(())?.args {
        if *a == ir::CURRENT_CASE {
            continue;
        }
        ancestors(cs, *a, out, depth + 1, budget)?;
    }
    Ok(())
}
fn evaluate(
    c: &ir::Cond,
    values: &BTreeMap<ir::StateVarId, ir::StateValue>,
    budget: &mut Budget,
    depth: usize,
) -> Limited<bool> {
    budget.depth(depth)?;
    match c {
        ir::Cond::IntCompare {
            variable,
            operator,
            value,
        } => {
            let Some(ir::StateValue::Integer(n)) = values.get(variable) else {
                return Err(());
            };
            let n = i128::from(*n);
            Ok(match operator {
                ir::IntComparison::Eq => n == *value,
                ir::IntComparison::Ne => n != *value,
                ir::IntComparison::Lt => n < *value,
                ir::IntComparison::Le => n <= *value,
                ir::IntComparison::Gt => n > *value,
                ir::IntComparison::Ge => n >= *value,
            })
        }
        ir::Cond::StateEq(x) => Ok(values.get(&x.variable) == Some(&x.value)),
        ir::Cond::And(cs) => {
            for c in cs {
                if !evaluate(c, values, budget, depth + 1)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        ir::Cond::Or(cs) => {
            for c in cs {
                if evaluate(c, values, budget, depth + 1)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Err(()),
    }
}
// Abstract finite truth-preserving pre-states, not runtime reachability. A kernel
// result and its overrides are always kept together; zero mass is not a branch.
fn breaks_truth(before: bool, after: bool) -> bool {
    before && !after
}
fn may_break(
    m: &ir::Model,
    c: &ir::Capability,
    condition: &Formula,
    vars: &[ir::StateVarId],
    budget: &mut Budget,
) -> Limited<bool> {
    if vars
        .iter()
        .any(|v| m.state_variables[v.0].case_set_size.is_some())
        || c.atomic.is_some()
        || c.finish_writes().chain(&c.start_writes).any(|w| {
            matches!(
                w.source,
                ir::WriteSource::Executor | ir::WriteSource::InsertCase { .. }
            )
        })
    {
        return Err(());
    }
    fn enumerate(
        m: &ir::Model,
        c: &ir::Capability,
        condition: &Formula,
        vars: &[ir::StateVarId],
        values: &mut BTreeMap<ir::StateVarId, ir::StateValue>,
        i: usize,
        budget: &mut Budget,
    ) -> Limited<bool> {
        budget.depth(i)?;
        if i < vars.len() {
            let id = vars[i];
            let v = &m.state_variables[id.0];
            for value in v
                .domain(&m.types)
                .chain(v.optional.then_some(ir::StateValue::None))
            {
                budget.charge(1, 1)?;
                values.insert(id, value);
                if enumerate(m, c, condition, vars, values, i + 1, budget)? {
                    return Ok(true);
                }
            }
            return Ok(false);
        }
        let before = evaluate_formula(condition, values, budget, 0)?;
        if !before {
            return Ok(false);
        }
        let branch = |result: ir::ValueId,
                      overrides: &[(ir::StateVarId, ir::StateValue)],
                      budget: &mut Budget|
         -> Limited<bool> {
            budget.charge(values.len(), values.len())?;
            budget.charge(c.writes.len().checked_add(overrides.len()).ok_or(())?, 0)?;
            let mut after = values.clone();
            for w in &c.writes {
                if matches!(w.source, ir::WriteSource::Integer { .. })
                    && !values.contains_key(&w.variable)
                {
                    return Err(());
                }
                if let Some(slot) = after.get_mut(&w.variable) {
                    *slot = match w.source {
                        ir::WriteSource::Executor | ir::WriteSource::InsertCase { .. } => {
                            return Err(());
                        }
                        ir::WriteSource::Integer {
                            operator, operand, ..
                        } => {
                            let Some(ir::StateValue::Integer(n)) = values.get(&w.variable) else {
                                return Err(());
                            };
                            let n = i128::from(*n);
                            let attempted = match operator {
                                ir::AssignmentOp::Assign => operand,
                                ir::AssignmentOp::Add => n + operand,
                                ir::AssignmentOp::Subtract => n - operand,
                            };
                            let upper = m.state_variables[w.variable.0].integer_max.ok_or(())?;
                            if attempted < 0 || attempted > i128::from(upper) {
                                return Err(());
                            }
                            ir::StateValue::Integer(attempted as u64)
                        }
                        ir::WriteSource::Result => ir::StateValue::Value(result),
                        ir::WriteSource::Constant(v) => v,
                    };
                }
            }
            for (id, v) in overrides {
                if after.contains_key(id) {
                    after.insert(*id, *v);
                }
            }
            Ok(breaks_truth(
                before,
                evaluate_formula(condition, &after, budget, 0)?,
            ))
        };
        if c.kernel.is_some() || c.performance.is_some() {
            for k in c.result_kernels() {
                for r in &k.rows {
                    budget.charge(k.given.len().checked_add(1).ok_or(())?, 0)?;
                    // Rows select latent conditions at finish; only compatible
                    // supported pre-values constrain this abstract state.
                    if k.given
                        .iter()
                        .zip(&r.values)
                        .any(|(id, v)| values.get(id).is_some_and(|x| x != v))
                    {
                        continue;
                    }
                    for b in &r.branches {
                        budget.charge(1, 0)?;
                        if !b.probability.is_zero() && branch(b.result, &b.overrides, budget)? {
                            return Ok(true);
                        }
                    }
                }
            }
        } else {
            for r in &c.results {
                if branch(*r, &[], budget)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    enumerate(m, c, condition, vars, &mut BTreeMap::new(), 0, budget)
}

fn evaluate_formula(
    f: &Formula,
    values: &BTreeMap<ir::StateVarId, ir::StateValue>,
    budget: &mut Budget,
    depth: usize,
) -> Limited<bool> {
    if let Formula::State(c) = f {
        return evaluate(c, values, budget, depth);
    }
    budget.depth(depth)?;
    match f {
        Formula::Bool(x) => Ok(values.get(&x.variable) == Some(&x.value)),
        Formula::Not(c) => Ok(!evaluate_formula(c, values, budget, depth + 1)?),
        Formula::And(cs) => {
            for c in cs {
                if !evaluate_formula(c, values, budget, depth + 1)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Formula::Or(cs) => {
            for c in cs {
                if evaluate_formula(c, values, budget, depth + 1)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Formula::State(_) => unreachable!(),
    }
}
fn formula_name(m: &ir::Model, f: &Formula) -> String {
    match f {
        Formula::State(c) => condition_name(m, c),
        Formula::Bool(x) => m.state_variables[x.variable.0].name.clone(),
        Formula::Not(c) => format!("!({})", formula_name(m, c)),
        // Lowering represents implication as Or(Not(A), B). Preserve its
        // accepted source form rather than printing unsupported explicit Not.
        Formula::Or(cs) if matches!(cs.as_slice(), [Formula::Not(_), _]) => {
            let Formula::Not(a) = &cs[0] else {
                unreachable!()
            };
            format!("({} -> {})", formula_name(m, a), formula_name(m, &cs[1]))
        }
        Formula::And(cs) | Formula::Or(cs) => format!(
            "({})",
            cs.iter()
                .map(|c| formula_name(m, c))
                .collect::<Vec<_>>()
                .join(if matches!(f, Formula::And(_)) {
                    " && "
                } else {
                    " || "
                })
        ),
    }
}

fn empty_row(m: &ir::Model, d: &Duty, id: usize) -> Responsibility {
    let target = match &d.target {
        Target::Capability(c) => TargetDescription {
            kind: "capability".into(),
            value: cap_name(m, *c),
        },
        Target::Condition(c) => TargetDescription {
            kind: "state_condition".into(),
            value: condition_name(m, c),
        },
        Target::Formula(f) => TargetDescription {
            kind: "state_condition".into(),
            value: formula_name(m, f),
        },
        Target::Maintenance(a) => TargetDescription {
            kind: "maintenance".into(),
            value: format!("{}.maintenance", m.actors[a.0].name),
        },
    };
    Responsibility {
        id,
        rules: if d.within.is_some()
            || matches!(d.target, Target::Formula(_) | Target::Maintenance(_))
        {
            "responsibility-p1-2"
        } else {
            "responsibility-p1-1"
        }
        .into(),
        person: m.people[d.person.0].name.clone(),
        target,
        actors: m
            .actors
            .iter()
            .filter(|a| a.executors.as_ref().is_some_and(|p| p.contains(&d.person)))
            .map(|a| a.name.clone())
            .collect(),
        observation: Observation {
            status: "not_derived".into(),
            required_sources: vec![],
            direct: vec![],
            indirect: vec![],
            missing_sources: vec![],
            reason: Some("derivation_limit".into()),
        },
        refusal: Refusal {
            status: "not_derived".into(),
            effects: vec![],
            reason: Some("derivation_limit".into()),
        },
        time_frame: TimeFrame::unchecked(d.within, "derivation_limit"),
        warnings: vec![],
    }
}
fn condition_name(m: &ir::Model, c: &ir::Cond) -> String {
    enum Fragment<'a> {
        Condition(&'a ir::Cond),
        Text(&'static str),
    }
    let mut pending = vec![Fragment::Condition(c)];
    let mut out = String::new();
    while let Some(fragment) = pending.pop() {
        let c = match fragment {
            Fragment::Text(t) => {
                out.push_str(t);
                continue;
            }
            Fragment::Condition(c) => c,
        };
        match c {
            ir::Cond::Contains { set, .. } => {
                let v = &m.state_variables[set.0];
                let owner = v
                    .kind
                    .actor()
                    .map(|a| m.actors[a.0].name.as_str())
                    .unwrap_or("case");
                out.push_str(&format!("case.id in {owner}.{}", v.name));
            }
            ir::Cond::IntCompare {
                variable,
                operator,
                value,
            } => out.push_str(&format!(
                "{} {} {}",
                match m.state_variables[variable.0].kind.actor() {
                    Some(a) => format!(
                        "{}.{}",
                        m.actors[a.0].name, m.state_variables[variable.0].name
                    ),
                    None => m.state_variables[variable.0].name.clone(),
                },
                match operator {
                    ir::IntComparison::Eq => "==",
                    ir::IntComparison::Ne => "!=",
                    ir::IntComparison::Lt => "<",
                    ir::IntComparison::Le => "<=",
                    ir::IntComparison::Gt => ">",
                    ir::IntComparison::Ge => ">=",
                },
                value
            )),
            ir::Cond::StateEq(x) => {
                let v = &m.state_variables[x.variable.0];
                let name = match v.kind {
                    ir::StateKind::ActorLatent(a)
                    | ir::StateKind::ActorObserved(a)
                    | ir::StateKind::Rule(a) => {
                        format!("{}.{}", m.actors[a.0].name, v.name)
                    }
                    _ => v.name.clone(),
                };
                let value = match x.value {
                    ir::StateValue::Set(bits) => format!("{bits}"),
                    ir::StateValue::None => "none".into(),
                    ir::StateValue::Integer(x) => x.to_string(),
                    ir::StateValue::Value(x) => m.types[v.base.0].members[x.0].clone(),
                };
                out.push_str(&format!("{name} == {value}"));
            }
            ir::Cond::And(cs) | ir::Cond::Or(cs) => {
                pending.push(Fragment::Text(")"));
                for (i, part) in cs.iter().enumerate().rev() {
                    pending.push(Fragment::Condition(part));
                    if i > 0 {
                        pending.push(Fragment::Text(if matches!(c, ir::Cond::And(_)) {
                            " && "
                        } else {
                            " || "
                        }));
                    }
                }
                pending.push(Fragment::Text("("));
            }
            _ => out.push_str("invalid"),
        }
    }
    out
}
fn derive_row(m: &ir::Model, d: &Duty, id: usize, limits: Limits) -> Responsibility {
    let mut row = empty_row(m, d, id);
    let mut budget = Budget {
        limits,
        work: 0,
        cells: 0,
    };
    if derive_row_into(m, d, &mut row, &mut budget).is_err() {
        empty_row(m, d, id)
    } else {
        row.time_frame = derive_time(m, d, &row, time_limits(id - 1));
        row
    }
}
fn derive_row_into(
    m: &ir::Model,
    d: &Duty,
    row: &mut Responsibility,
    budget: &mut Budget,
) -> Limited<()> {
    budget.charge(m.process.bindings.len(), m.process.bindings.len())?;
    budget.charge(m.actors.len(), m.actors.len().checked_mul(2).ok_or(())?)?;
    let graph_cells = crate::contest::control_cells(m).ok_or(())?;
    budget.charge(graph_cells, graph_cells)?;
    let condition_cells = crate::contest::condition_control_cells(m).ok_or(())?;
    budget.charge(condition_cells, condition_cells)?;
    let cs = calls(m);
    let control = crate::contest::control_dependencies(m);
    let mut seeds = BTreeSet::new();
    let mut vars = BTreeSet::new();
    let formula = match &d.target {
        Target::Condition(c) => Some(Formula::State(c.clone())),
        Target::Formula(f) => Some(f.clone()),
        _ => None,
    };
    match &d.target {
        Target::Capability(id) => {
            for c in cs.values() {
                budget.charge(1, 1)?;
                if c.capability == *id {
                    seeds.insert(c.binding);
                }
            }
        }
        Target::Condition(_) | Target::Formula(_) => {
            formula_support(formula.as_ref().ok_or(())?, &mut vars, budget, 0)?;
            for call in cs.values() {
                budget.charge(1, 1)?;
                if vars
                    .iter()
                    .any(|v| writes(&m.capabilities[call.capability.0], *v))
                {
                    seeds.insert(call.binding);
                }
            }
        }
        Target::Maintenance(a) => {
            for c in cs.values() {
                budget.charge(1, 1)?;
                if m.capabilities[c.capability.0].actor == *a {
                    seeds.insert(c.binding);
                }
            }
        }
    }
    let mut required = BTreeSet::new();
    if let Target::Maintenance(a) = &d.target {
        for c in &m.capabilities {
            budget.charge(1, 0)?;
            if c.actor == *a {
                for source in &c.reads {
                    budget.charge(1, 1)?;
                    required.insert(*source);
                }
            }
        }
    }
    for b in &seeds {
        sources(m, &cs, *b, &mut BTreeSet::new(), &mut required, 0, budget)?;
    }
    for v in &vars {
        if let Some(a) = m.state_variables[v.0].kind.actor()
            && m.actors[a.0].source.is_some()
        {
            required.insert(a);
        }
    }
    row.observation.required_sources = required
        .iter()
        .map(|a| m.actors[a.0].name.clone())
        .collect();
    let mut direct = BTreeSet::new();
    let mut indirect = BTreeSet::new();
    for o in &m.agency.origin {
        budget.charge(row.actors.len().checked_add(1).ok_or(())?, 0)?;
        if row.actors.contains(&o.actor) {
            let cells = o.direct.len().checked_add(o.indirect.len()).ok_or(())?;
            budget.charge(cells, cells)?;
            direct.extend(o.direct.iter().cloned());
            indirect.extend(o.indirect.iter().cloned());
        }
    }
    // Source declaration order, not alphabetic order, matches Agency.
    row.observation.direct = m
        .actors
        .iter()
        .filter(|a| direct.contains(&a.name))
        .map(|a| a.name.clone())
        .collect();
    row.observation.indirect = m
        .actors
        .iter()
        .filter(|a| indirect.contains(&a.name))
        .map(|a| a.name.clone())
        .collect();
    row.observation.missing_sources = row
        .observation
        .required_sources
        .iter()
        .filter(|n| !direct.contains(*n))
        .cloned()
        .collect();
    let unused =
        matches!(d.target, Target::Capability(_) | Target::Maintenance(_)) && seeds.is_empty();
    row.observation.reason = if unused {
        Some("target_not_called".into())
    } else if required.is_empty() {
        Some("no_source_identified".into())
    } else {
        None
    };
    row.observation.status = if row.observation.reason.is_some() {
        "not_derived"
    } else if row.observation.missing_sources.is_empty() {
        "present"
    } else {
        "missing"
    }
    .into();
    if row.observation.status == "missing" {
        row.warnings.push(diag::W_RESP_1.into());
    }
    let mut noncommit = false;
    let mut unknown = false;
    let mut condition_unknown = false;
    for call in cs.values() {
        let cap = &m.capabilities[call.capability.0];
        let mut ancestry = BTreeSet::new();
        ancestors(&cs, call.binding, &mut ancestry, 0, budget)?;
        let mut candidate = !ancestry.is_disjoint(&seeds);
        candidate |= control
            .calls
            .get(&call.binding)
            .is_some_and(|guards| seeds.iter().any(|b| guards.contains(&cs[b].capability)));
        let condition = control.condition_calls.get(&call.binding);
        if let Some(producers) = condition {
            for producer in cs.values().filter(|c| producers.contains(&c.capability)) {
                let mut inputs = BTreeSet::new();
                ancestors(&cs, producer.binding, &mut inputs, 0, budget)?;
                candidate |= !inputs.is_disjoint(&seeds);
            }
        }
        // Refusal carries guard judgement candidates as well as argument ancestry.
        let refusal = m
            .agency
            .refusal
            .iter()
            .find(|r| r.call == m.process.bindings[call.binding.0].name);
        if let Some(r) = refusal {
            candidate |= r
                .judgements
                .iter()
                .filter_map(crate::agency::Judgement::process)
                .any(|j| seeds.iter().any(|b| m.process.bindings[b.0].name == j.call));
        }
        if let Some(c) = &formula
            && vars.iter().any(|v| writes(cap, *v))
        {
            if vars
                .iter()
                .any(|v| m.state_variables[v.0].case_set_size.is_some())
                || cap.atomic.is_some()
                || cap.finish_writes().chain(&cap.start_writes).any(|w| {
                    matches!(
                        w.source,
                        ir::WriteSource::Executor | ir::WriteSource::InsertCase { .. }
                    )
                })
            {
                candidate = true;
                unknown = true;
            } else {
                candidate =
                    may_break(m, cap, c, &vars.iter().copied().collect::<Vec<_>>(), budget)?;
            }
            if candidate && !cap.commits && !unknown {
                noncommit = true;
            }
        }
        if let Target::Maintenance(a) = &d.target {
            candidate = cap.actor == *a;
            if candidate
                && !cap.commits
                && m.state_variables
                    .iter()
                    .enumerate()
                    .any(|(v, _)| writes(cap, ir::StateVarId(v)))
            {
                noncommit = true;
            }
        }
        if !candidate || !cap.commits {
            continue;
        }
        condition_unknown |= condition.is_some();
        let mut effect = Effect {
            capability: cap_name(m, call.capability),
            call: m.process.bindings[call.binding.0].name.clone(),
            status: "not_derived".into(),
            judgements: vec![],
        };
        if let Some(r) = refusal {
            for j in r
                .judgements
                .iter()
                .filter_map(crate::agency::Judgement::process)
            {
                let actor = j.capability.split('.').next().unwrap_or("");
                if row.actors.iter().any(|a| a == actor) {
                    effect.judgements.push(Judgement {
                        actor: actor.into(),
                        capability: j.capability.clone(),
                        call: j.call.clone(),
                        stops: j.stops,
                        bypasses: j.bypasses,
                    });
                }
            }
            effect.status = if !r.reachable {
                "not_applicable"
            } else if effect.judgements.iter().any(|j| j.stops == Stops::AllPaths) {
                "all_paths"
            } else if effect
                .judgements
                .iter()
                .any(|j| j.stops == Stops::Conditional)
            {
                "conditional"
            } else {
                "none"
            }
            .into();
        } else {
            unknown = true;
        }
        row.refusal.effects.push(effect);
    }
    if condition_unknown {
        // A conditional release can stop the continuation. Do not mistake its
        // unmodeled decision support for either a missing or a certified veto.
        row.refusal.effects.clear();
    }
    let effects = &row.refusal.effects;
    row.refusal.reason = if unused {
        Some("target_not_called".into())
    } else if condition_unknown {
        Some("condition_wait_control".into())
    } else if noncommit {
        Some("noncommit_state_write".into())
    } else if unknown {
        Some("state_guard".into())
    } else if effects.is_empty() || effects.iter().all(|e| e.status == "not_applicable") {
        Some("no_reachable_effect".into())
    } else {
        None
    };
    row.refusal.status = if row.refusal.reason.is_some() {
        "not_derived"
    } else if effects.iter().any(|e| e.status == "none") {
        "missing"
    } else if effects.iter().any(|e| e.status == "conditional") {
        "conditional"
    } else {
        "present"
    }
    .into();
    if matches!(row.refusal.status.as_str(), "missing" | "conditional") {
        row.warnings.push(diag::W_RESP_2.into());
    }
    Ok(())
}

#[cfg(test)]
thread_local! {static TEST_TIME_LIMITS: std::cell::RefCell<Option<Limits>> = const {std::cell::RefCell::new(None)};}
fn time_limits(_index: usize) -> Limits {
    #[cfg(test)]
    if let Some(limits) = TEST_TIME_LIMITS.with(|v| *v.borrow()) {
        return limits;
    }
    DEFAULT
}

fn derive_time(m: &ir::Model, d: &Duty, row: &Responsibility, limits: Limits) -> TimeFrame {
    let Some(within) = d.within else {
        return TimeFrame::unchecked(None, "");
    };
    let reason = match (row.observation.status.as_str(), row.refusal.status.as_str()) {
        ("missing", _) => Some("observation_missing"),
        ("not_derived", _) => Some("observation_not_derived"),
        (_, "missing") => Some("refusal_missing"),
        (_, "conditional") => Some("refusal_conditional"),
        (_, "not_derived") => Some("refusal_not_derived"),
        _ => None,
    };
    if let Some(reason) = reason {
        return TimeFrame::unchecked(Some(within), reason);
    }
    // These rows are structural witnesses, not a proof that a scheduled start
    // or an observation-dependent release is feasible before the duty deadline.
    if m.resource_time.is_some() {
        return TimeFrame::unchecked(Some(within), "calendar_time_feasibility_not_derived");
    }
    if m.has_condition_wait() {
        return TimeFrame::unchecked(Some(within), "condition_wait_feasibility_not_derived");
    }
    let mut budget = Budget {
        limits,
        work: 0,
        cells: 0,
    };
    let path = time_witness(m, row, &mut budget);
    match path {
        Err(reason) => TimeFrame::unchecked(Some(within), reason),
        Ok(path) => TimeFrame {
            status: if path.duration_upper <= within {
                "satisfied"
            } else {
                "violated"
            }
            .into(),
            details: Some(TimeDetails {
                within,
                required_duration: Some(path.duration_upper),
                basis: "static_capability_path_upper".into(),
                scope: "static_responsibility_reachability".into(),
                reason: None,
                paths: vec![path],
            }),
        },
    }
}

type TimeResult<T> = Result<T, &'static str>;
fn time_charge(budget: &mut Budget, work: usize, cells: usize) -> TimeResult<()> {
    budget.charge(work, cells).map_err(|_| "derivation_limit")
}
fn duration_upper(c: &ir::Capability, budget: &mut Budget) -> TimeResult<ir::Tick> {
    if let Some(d) = &c.duration_distribution {
        time_charge(budget, d.entries.len(), 0)?;
        d.entries
            .iter()
            .filter(|(_, p)| !p.is_zero())
            .map(|(t, _)| *t)
            .max()
            .ok_or("duration_unknown")
    } else {
        Ok(c.duration)
    }
}

/// Memoized longest paths on call identities, not Capability identities. A
/// judgement and the effect itself both contribute, but unrelated predecessors
/// and mutually exclusive branches are never concatenated.
struct TimeGraph<'a> {
    m: &'a ir::Model,
    cs: BTreeMap<ir::BindingId, &'a ir::Call>,
    predecessors: BTreeMap<ir::BindingId, BTreeSet<ir::BindingId>>,
}
type WeightedPath = (ir::Tick, Vec<ir::BindingId>);
impl TimeGraph<'_> {
    fn longest(
        &self,
        from: ir::BindingId,
        to: ir::BindingId,
        memo: &mut BTreeMap<ir::BindingId, Option<WeightedPath>>,
        active: &mut BTreeSet<ir::BindingId>,
        budget: &mut Budget,
        depth: usize,
    ) -> TimeResult<Option<WeightedPath>> {
        budget.depth(depth).map_err(|_| "derivation_limit")?;
        if let Some(path) = memo.get(&to) {
            return Ok(path.clone());
        }
        if !active.insert(to) {
            return Err("path_not_reconstructed");
        }
        time_charge(budget, 1, 1)?;
        let call = self.cs.get(&to).ok_or("path_not_reconstructed")?;
        let mut best = if from == to { Some((0, vec![])) } else { None };
        if from != to {
            for p in self.predecessors.get(&to).ok_or("path_not_reconstructed")? {
                time_charge(budget, 1, 0)?;
                if let Some(path) = self.longest(from, *p, memo, active, budget, depth + 1)?
                    && best.as_ref().is_none_or(|old: &WeightedPath| {
                        path.0 > old.0 || (path.0 == old.0 && path.1 < old.1)
                    })
                {
                    best = Some(path);
                }
            }
        }
        if let Some((sum, path)) = &mut best {
            *sum = sum
                .checked_add(duration_upper(
                    &self.m.capabilities[call.capability.0],
                    budget,
                )?)
                .ok_or("duration_overflow")?;
            time_charge(budget, path.len() + 1, path.len() + 1)?;
            path.push(to);
        }
        active.remove(&to);
        memo.insert(to, best.clone());
        Ok(best)
    }
}
fn time_witness(m: &ir::Model, row: &Responsibility, budget: &mut Budget) -> TimeResult<TimePath> {
    time_charge(budget, m.process.bindings.len(), m.process.bindings.len())?;
    let cs = calls(m);
    let mut predecessors = BTreeMap::new();
    for (id, c) in &cs {
        time_charge(budget, c.args.len() + 1, c.args.len() + 1)?;
        predecessors.insert(
            *id,
            c.args
                .iter()
                .copied()
                .filter(|a| *a != ir::CURRENT_CASE)
                .collect::<BTreeSet<_>>(),
        );
    }
    // Intermediate calls need guard edges too, even when they have no result
    // arguments and are not themselves commits effects in Agency's table.
    // Keep the exact guard binding identity: matching only CapabilityId would
    // conflate two separate judgements of the same Capability.
    for (to, guards) in time_guard_edges(&m.process.body, budget)? {
        time_charge(budget, guards.len(), guards.len())?;
        predecessors
            .get_mut(&to)
            .ok_or("path_not_reconstructed")?
            .extend(guards);
    }
    let named: BTreeMap<_, _> = cs
        .keys()
        .map(|id| (m.process.bindings[id.0].name.as_str(), *id))
        .collect();
    // Consume only guard relations certified by Agency's existing refusal
    // analysis. Argument dependencies above retain every intermediate call.
    for effect in &m.agency.refusal {
        if let Some(to) = named.get(effect.call.as_str()) {
            for judgement in effect
                .judgements
                .iter()
                .filter_map(crate::agency::Judgement::process)
            {
                time_charge(budget, 1, 1)?;
                if let Some(from) = named.get(judgement.call.as_str())
                    && from != to
                    && judgement.stops != Stops::None
                {
                    predecessors
                        .get_mut(to)
                        .ok_or("path_not_reconstructed")?
                        .insert(*from);
                }
            }
        }
    }
    let graph = TimeGraph {
        m,
        cs,
        predecessors,
    };
    let mut best: Option<TimePath> = None;
    // Strictly greater preserves deterministic O-before-U / declaration-order
    // ties. Observation iteration is call-first, then source declaration order.
    for (id, c) in &graph.cs {
        let cap = &m.capabilities[c.capability.0];
        if !row.actors.contains(&m.actors[cap.actor.0].name) {
            continue;
        }
        for a in &m.actors {
            time_charge(budget, 1, 0)?;
            if row.observation.required_sources.contains(&a.name)
                && cap.reads.iter().any(|id| m.actors[id.0].name == a.name)
            {
                let duration_upper = duration_upper(cap, budget)?;
                let path = TimePath {
                    kind: "observation".into(),
                    source: Some(a.name.clone()),
                    judgement_call: None,
                    effect_call: None,
                    calls: vec![m.process.bindings[id.0].name.clone()],
                    duration_upper,
                };
                if best
                    .as_ref()
                    .is_none_or(|p| path.duration_upper > p.duration_upper)
                {
                    best = Some(path);
                }
            }
        }
    }
    // Origin may mention uncalled declarations. Such evidence has no selected
    // process path and cannot silently become a zero-duration observation.
    for source in &row.observation.required_sources {
        if !graph.cs.values().any(|c| {
            let cap = &m.capabilities[c.capability.0];
            row.actors.contains(&m.actors[cap.actor.0].name)
                && cap.reads.iter().any(|a| m.actors[a.0].name == *source)
        }) {
            return Err("no_reachability_path");
        }
    }
    // Judgement declaration order is the stable tie breaker, independent of
    // effect iteration; per-start memoization avoids enumerating paths.
    for (from, call) in &graph.cs {
        let name = &m.process.bindings[from.0].name;
        if !row
            .actors
            .contains(&m.actors[m.capabilities[call.capability.0].actor.0].name)
        {
            continue;
        }
        let mut memo = BTreeMap::new();
        for effect in &row.refusal.effects {
            if effect.status == "not_applicable" {
                continue;
            }
            if !effect
                .judgements
                .iter()
                .any(|j| j.call == *name && j.stops != Stops::None)
            {
                continue;
            }
            let to = *named
                .get(effect.call.as_str())
                .ok_or("path_not_reconstructed")?;
            let (duration_upper, ids) = graph
                .longest(*from, to, &mut memo, &mut BTreeSet::new(), budget, 0)?
                .ok_or("path_not_reconstructed")?;
            let path = TimePath {
                kind: "refusal".into(),
                source: None,
                judgement_call: Some(name.clone()),
                effect_call: Some(effect.call.clone()),
                calls: ids
                    .iter()
                    .map(|id| m.process.bindings[id.0].name.clone())
                    .collect(),
                duration_upper,
            };
            if best
                .as_ref()
                .is_none_or(|p| path.duration_upper > p.duration_upper)
            {
                best = Some(path);
            }
        }
    }
    best.ok_or("no_reachability_path")
}

/// The same conservative lexical continuation envelope as the existing
/// responsibility control evidence, retaining BindingId instead of collapsing
/// it to CapabilityId. This does not derive any new refusal or runtime guard.
fn time_guard_edges(
    body: &ir::Block,
    budget: &mut Budget,
) -> TimeResult<BTreeMap<ir::BindingId, BTreeSet<ir::BindingId>>> {
    type Guards = BTreeSet<ir::BindingId>;
    type Edges = BTreeMap<ir::BindingId, Guards>;
    fn support(
        c: &ir::Cond,
        out: &mut Guards,
        budget: &mut Budget,
        depth: usize,
    ) -> TimeResult<()> {
        budget.depth(depth).map_err(|_| "derivation_limit")?;
        match c {
            ir::Cond::Eq(x) => {
                time_charge(budget, 1, 1)?;
                out.insert(x.binding);
            }
            ir::Cond::And(cs) | ir::Cond::Or(cs) => {
                for c in cs {
                    support(c, out, budget, depth + 1)?;
                }
            }
            // Existing Agency does not certify state-guard refusal. Never
            // invent a result judgement identity for an observed state read.
            ir::Cond::Contains { .. } | ir::Cond::StateEq(_) | ir::Cond::IntCompare { .. } => {}
        }
        Ok(())
    }
    fn walk(
        body: &ir::Block,
        mut guards: Guards,
        edges: &mut Edges,
        budget: &mut Budget,
        depth: usize,
    ) -> TimeResult<(Option<Guards>, bool)> {
        budget.depth(depth).map_err(|_| "derivation_limit")?;
        let mut alive = true;
        let mut may_end = false;
        for statement in body {
            time_charge(budget, 1, 0)?;
            match &statement.kind {
                ir::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    let mut condition_guards = Guards::new();
                    support(condition, &mut condition_guards, budget, 0)?;
                    time_charge(budget, guards.len(), guards.len())?;
                    let mut next = guards.clone();
                    next.extend(condition_guards.iter().copied());
                    time_charge(budget, next.len(), next.len())?;
                    let (yes, yes_end) = walk(then_body, next.clone(), edges, budget, depth + 1)?;
                    let (no, no_end) = walk(else_body, next, edges, budget, depth + 1)?;
                    may_end |= yes_end || no_end;
                    match (yes, no) {
                        (Some(y), Some(n)) if yes_end || no_end => {
                            time_charge(budget, y.len() + n.len(), y.len() + n.len())?;
                            guards.extend(y.union(&n).copied());
                        }
                        (Some(y), Some(n)) => {
                            time_charge(budget, y.len() + n.len(), y.len() + n.len())?;
                            guards.extend(
                                y.intersection(&n)
                                    .filter(|b| !condition_guards.contains(b))
                                    .copied(),
                            );
                        }
                        (Some(y), None) | (None, Some(y)) => {
                            guards = y;
                        }
                        (None, None) => {
                            alive = false;
                        }
                    }
                }
                ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => {
                    time_charge(budget, guards.len() + 1, guards.len() + 1)?;
                    edges.insert(c.binding, guards.clone());
                }
                ir::Stmt::Fork { branches, join, .. } => {
                    for c in branches {
                        time_charge(budget, guards.len() + 1, guards.len() + 1)?;
                        edges.insert(c.binding, guards.clone());
                    }
                    if matches!(join, ir::Join::Any { .. }) {
                        may_end = true;
                        time_charge(budget, branches.len(), branches.len())?;
                        guards.extend(branches.iter().map(|c| c.binding));
                    }
                }
                ir::Stmt::End(_) => {
                    may_end = true;
                    alive = false;
                }
                // ADR 0026 wait: no Capability, no guard, and within counts
                // static Capability durations only (ADR 0028 裁定 U3).
                ir::Stmt::Wait(_) => {}
                ir::Stmt::WaitUntil(_) => return Err("condition_wait_control"),
            }
        }
        Ok((alive.then_some(guards), may_end))
    }
    let mut edges = Edges::new();
    walk(body, Guards::new(), &mut edges, budget, 0)?;
    Ok(edges)
}

pub(crate) fn derive(m: &mut ir::Model) -> Result<(), diag::Diagnostic> {
    if m.people.is_empty()
        && !m
            .agency
            .unchecked
            .iter()
            .any(|s| s == "responsibility_people_undeclared")
    {
        m.agency
            .unchecked
            .push("responsibility_people_undeclared".into());
    }
    if let Some(view) = &m.agency.viewpoints
        && let Some(crate::agency::effects::Effects::NotDerived { reason, .. }) = &view.effects
    {
        // ADR 0054: legacy candidates remain known, but cannot make a failed
        // U/E derivation a complete default-responsibility candidate census.
        let reason = serde_json::to_value(reason).map_err(|_| invalid())?;
        let reason = reason.as_str().ok_or_else(invalid)?;
        let disclosure = format!(
            "responsibility_default_candidates: process {}: U/E not_derived ({reason})",
            m.process.name
        );
        if !m.agency.unchecked.contains(&disclosure) {
            m.agency.unchecked.push(disclosure);
        }
    }
    let Some(meta) = m.responsibility.as_ref() else {
        m.diagnostics.extend(undeclared_responsibility(m));
        return Ok(());
    };
    validate_declarations(m, meta)?;
    let rows = meta
        .duties
        .iter()
        .enumerate()
        .map(|(i, d)| derive_row(m, d, i + 1, row_limits(i)))
        .collect::<Vec<_>>();
    let paths = meta
        .contests
        .iter()
        .map(|c| crate::contest::derive(m, c))
        .collect::<Vec<_>>();
    let mut warnings = crate::contest::missing_warnings(m, meta);
    for (row, d) in rows.iter().zip(&meta.duties) {
        for code in &row.warnings {
            warnings.push(warning(
                code,
                format!(
                    "{} {}: {}",
                    row.person,
                    row.target.value,
                    if code == diag::W_RESP_1 {
                        "責務に対応する観測が無い"
                    } else if row.refusal.status == "conditional" {
                        "責務に対応する拒否に迂回がある"
                    } else {
                        "責務に対応する拒否が効果へ届かない"
                    }
                ),
                d.span,
            ));
        }
    }
    warnings.extend(undeclared_responsibility(m));
    let unchecked = &mut m.agency.unchecked;
    if !rows.is_empty() {
        unchecked.retain(|v| v != "W-RESP-1");
        if rows.iter().any(|r| r.time_frame.status == "unchecked") {
            unchecked.push("responsibility_time_frame".into());
        }
        unchecked.push("responsibility_assignment_scope".into());
        for (word, present) in [
            (
                "responsibility_observation",
                rows.iter().any(|r| r.observation.status == "not_derived"),
            ),
            (
                "responsibility_refusal",
                rows.iter().any(|r| r.refusal.status == "not_derived"),
            ),
            (
                "responsibility_noncommit_writes",
                rows.iter()
                    .any(|r| r.refusal.reason.as_deref() == Some("noncommit_state_write")),
            ),
            (
                "responsibility_derivation_limit",
                rows.iter()
                    .any(|r| r.observation.reason.as_deref() == Some("derivation_limit")),
            ),
        ] {
            if present {
                unchecked.push(word.into());
            }
        }
    }
    if !paths.is_empty() {
        unchecked.retain(|v| v != "contest_path");
        for word in [
            "contest_route_handoff",
            "contest_decision_coverage",
            "contest_real_world_occurrence",
            "contest_real_world_response_time",
            "contest_acceptance",
            "contest_remedy",
        ] {
            unchecked.push(word.into());
        }
    }
    let meta = m.responsibility.as_mut().unwrap();
    meta.responsibilities = rows;
    meta.contest_paths = paths;
    m.diagnostics.extend(warnings);
    Ok(())
}

fn undeclared_responsibility(m: &ir::Model) -> Vec<diag::Diagnostic> {
    let mut warnings = Vec::new();
    let mut actors = BTreeSet::new();
    for r in &m.agency.refusal {
        if r.reachable {
            actors.insert(r.effect.split('.').next().unwrap_or(""));
            for j in r
                .judgements
                .iter()
                .filter_map(crate::agency::Judgement::process)
            {
                if j.stops != Stops::None {
                    actors.insert(j.capability.split('.').next().unwrap_or(""));
                }
            }
        }
    }
    if let Some(view) = &m.agency.viewpoints
        && let Some(crate::agency::effects::Effects::Derived { rows, .. }) = &view.effects
    {
        for row in rows {
            // ADR 0054: known U/E carriers do not depend on people or legacy
            // refusal reachability. E remains known even when process U is not.
            actors.extend(row.e.actors.iter().map(String::as_str));
            if let Some(u) = &row.u {
                for r in u {
                    match r {
                        crate::agency::effects::Refuser::Process(j) => {
                            actors.extend(j.carrier.actors.iter().map(String::as_str));
                        }
                        crate::agency::effects::Refuser::Rule(r) => {
                            actors.insert(r.actor.as_str());
                        }
                    }
                }
            }
            for r in &row.state_refusers {
                actors.insert(r.actor.as_str());
            }
        }
    }
    for a in &m.actors {
        if actors.contains(a.name.as_str())
            && !a.executors.as_ref().is_some_and(|ps| {
                m.responsibility
                    .as_ref()
                    .is_some_and(|meta| meta.duties.iter().any(|d| ps.contains(&d.person)))
            })
        {
            warnings.push(warning(
                diag::W_RESP_0,
                format!("{} has no declared responsibility", a.name),
                a.span,
            ));
        }
    }
    warnings
}

fn valid_condition(m: &ir::Model, c: &ir::Cond) -> bool {
    let mut pending = vec![c];
    while let Some(c) = pending.pop() {
        match c {
            ir::Cond::Contains { set, element } => {
                if *element != ir::Element::CurrentCase
                    || !m.state_variables.get(set.0).is_some_and(|v| {
                        v.case_set_size.is_some() && v.kind.actor().is_none_or(|a| included(m, a))
                    })
                {
                    return false;
                }
            }
            ir::Cond::IntCompare { variable, .. } => {
                if !m.state_variables.get(variable.0).is_some_and(|v| {
                    v.integer_max.is_some() && v.kind.actor().is_none_or(|a| included(m, a))
                }) {
                    return false;
                }
            }
            ir::Cond::StateEq(x) => {
                if !m.state_variables.get(x.variable.0).is_some_and(|v| {
                    v.kind.actor().is_none_or(|a| included(m, a))
                        && m.types.get(v.base.0).is_some_and(|t| match x.value {
                            ir::StateValue::None => v.optional,
                            ir::StateValue::Value(x) => x.0 < t.members.len(),
                            ir::StateValue::Integer(_) | ir::StateValue::Set(_) => false,
                        })
                }) {
                    return false;
                }
            }
            ir::Cond::And(cs) | ir::Cond::Or(cs) => {
                if cs.len() < 2 {
                    return false;
                }
                pending.extend(cs);
            }
            _ => return false,
        }
    }
    true
}
fn validate_declarations(m: &ir::Model, meta: &Metadata) -> Result<(), diag::Diagnostic> {
    if !crate::contest::rates_declarations_valid(m) {
        return Err(invalid());
    }
    if !meta.contests.is_empty() && !crate::contest::control_within_limits(m, 1_000_000) {
        return Err(invalid());
    }
    let mut parties = BTreeSet::new();
    for a in &meta.parties {
        if a.0 >= m.actors.len() || !parties.insert(*a) {
            return Err(invalid());
        }
    }
    let mut standings = BTreeSet::new();
    for s in &meta.standings {
        if !parties.contains(&s.actor)
            || !s.target.valid(m)
            || !standings.insert((s.actor, s.target))
        {
            return Err(invalid());
        }
    }
    for d in &meta.duties {
        if !match &d.target {
            Target::Capability(c) => c.0 < m.capabilities.len(),
            Target::Condition(c) => valid_condition(m, c),
            Target::Formula(f) => valid_formula(m, f),
            Target::Maintenance(a) => {
                a.0 < m.actors.len()
                    && !m
                        .capabilities
                        .iter()
                        .any(|c| c.actor == *a && c.name == "maintenance")
            }
        } {
            return Err(invalid());
        }
    }
    for (i, d) in meta.duties.iter().enumerate() {
        if d.person.0 >= m.people.len()
            || meta.duties[..i].iter().any(|x| {
                x.person == d.person
                    && same_target(&m.types, &m.state_variables, &x.target, &d.target)
            })
        {
            return Err(invalid());
        }
    }
    let mut ordinals = BTreeSet::new();
    for (i, c) in m.contracts.iter().enumerate() {
        if let ir::ContractKind::ContestResponse { contest } = c.kind {
            let Some(declaration) = meta.contests.get(contest) else {
                return Err(invalid());
            };
            if c.ordinal != Some(declaration.ordinal)
                || c.name != declaration.name
                || c.span != declaration.span
            {
                return Err(invalid());
            }
            continue;
        }
        if c.ordinal == Some(0) || !ordinals.insert(c.ordinal.unwrap_or(i + 1)) {
            return Err(invalid());
        }
    }
    for (i, c) in meta.contests.iter().enumerate() {
        let runtime: Vec<_> = m.contracts.iter().enumerate().filter(|(_, clause)|
            matches!(clause.kind, ir::ContractKind::ContestResponse { contest } if contest == i)).collect();
        if runtime.len() != 1
            || !m.verify.windows(2).any(|pair| {
                pair[0].property == ir::Property::Contest(i)
                    && pair[1].property == ir::Property::Contract(ir::ClauseId(runtime[0].0))
            })
        {
            return Err(invalid());
        }
        if c.ordinal == 0
            || !ordinals.insert(c.ordinal)
            || c.route.0 >= m.capabilities.len()
            || !standings.contains(&(c.party, c.target))
            || c.reversible_by.is_empty()
            || c.reversible_by.iter().any(|p| p.0 >= m.people.len())
            || c.reversible_by.iter().collect::<BTreeSet<_>>().len() != c.reversible_by.len()
            || m.verify
                .iter()
                .filter(|v| v.property == ir::Property::Contest(i))
                .count()
                != 1
        {
            return Err(invalid());
        }
    }
    if !crate::contest::effect_sites_valid(m) {
        return Err(invalid());
    }
    if m.verify
        .iter()
        .any(|v| matches!(v.property,ir::Property::Contest(i) if i>=meta.contests.len()))
    {
        return Err(invalid());
    }
    if ordinals.iter().copied().ne(1..=ordinals.len()) {
        return Err(invalid());
    }
    Ok(())
}
fn valid_formula(m: &ir::Model, f: &Formula) -> bool {
    let mut pending = vec![f];
    while let Some(f) = pending.pop() {
        match f {
            Formula::State(c) => {
                if !valid_condition(m, c) {
                    return false;
                }
            }
            Formula::Bool(x) => {
                if !m.state_variables.get(x.variable.0).is_some_and(|v| {
                    v.kind == ir::StateKind::ObservedCase
                        && !v.optional
                        && m.types.get(v.base.0).is_some_and(|t| {
                            t.name == "bool"
                                && t.members == ["true", "false"]
                                && x.value == ir::StateValue::Value(ir::ValueId(0))
                        })
                }) {
                    return false;
                }
            }
            Formula::Not(c) => pending.push(c),
            Formula::And(cs) | Formula::Or(cs) => {
                if cs.len() < 2 {
                    return false;
                }
                pending.extend(cs);
            }
        }
    }
    true
}
/// Public IR is untrusted. Recompute static certificates, never trust a bool.
pub(crate) fn validate(m: &ir::Model) -> Result<(), diag::Diagnostic> {
    let Some(meta) = &m.responsibility else {
        return if m
            .verify
            .iter()
            .any(|v| matches!(v.property, ir::Property::Contest(_)))
        {
            Err(invalid())
        } else {
            Ok(())
        };
    };
    validate_declarations(m, meta)?;
    // Re-derive Agency for the certificate, rather than trusting editable origin
    // and refusal fields on public IR. Kernel/multi-case unchecked extensions
    // affect no static evidence and are intentionally not compared here.
    if let Some(diagnostic) = crate::contest::condition_wait_refusal(m) {
        return Err(diagnostic);
    }
    let (agency, _) = crate::agency::derive(m)?;
    if agency.origin != m.agency.origin || agency.refusal != m.agency.refusal {
        return Err(invalid());
    }
    if meta.contest_paths
        != meta
            .contests
            .iter()
            .map(|c| crate::contest::derive(m, c))
            .collect::<Vec<_>>()
        || meta.responsibilities
            != meta
                .duties
                .iter()
                .enumerate()
                .map(|(i, d)| derive_row(m, d, i + 1, row_limits(i)))
                .collect::<Vec<_>>()
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn q4_contest_validation_cell_boundary_is_independent_of_wait() {
        use super::*;
        let source = include_str!("../../../fixtures/resp-contest/model.relca");
        let model = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
        for (bindings, accepted) in [(936, true), (937, false)] {
            for wait in [false, true] {
                let mut m = model.clone();
                // Reservation-only probe, not a certificate for the extra IR slots.
                m.process
                    .bindings
                    .resize(bindings, m.process.bindings[0].clone());
                if wait {
                    m.process.body.insert(
                        2,
                        ir::Statement {
                            kind: ir::Stmt::WaitUntil(ir::Cond::Eq(ir::ResultMatch {
                                binding: ir::BindingId(1),
                                value: ir::ValueId(0),
                            })),
                            span: m.process.body[1].span,
                        },
                    );
                }
                let result = validate_declarations(&m, m.responsibility.as_ref().unwrap());
                assert_eq!(
                    result.is_ok(),
                    accepted,
                    "n={bindings}, wait={wait}: {result:?}"
                );
                if let Err(d) = result {
                    assert_eq!(d.code, diag::U_SEM_0);
                    assert_eq!(d.message, "invalid responsibility/contest static IR");
                }
            }
        }
    }

    #[test]
    fn integer_truth_effects_use_numeric_domain_and_limit_range_stops() {
        use super::*;
        let source = include_str!("../../../fixtures/int-upper/model.relca");
        let mut m = crate::integer_tests::model(source);
        let condition = Formula::State(ir::Cond::IntCompare {
            variable: ir::StateVarId(0),
            operator: ir::IntComparison::Lt,
            value: 2,
        });
        let budget = || Budget {
            limits: DEFAULT,
            work: 0,
            cells: 0,
        };
        assert_eq!(
            may_break(
                &m,
                &m.capabilities[0],
                &condition,
                &[ir::StateVarId(0)],
                &mut budget()
            ),
            Ok(true)
        );
        let outside = Formula::State(ir::Cond::IntCompare {
            variable: ir::StateVarId(0),
            operator: ir::IntComparison::Ge,
            value: 0,
        });
        assert!(
            may_break(
                &m,
                &m.capabilities[0],
                &outside,
                &[ir::StateVarId(0)],
                &mut budget()
            )
            .is_err()
        );
        m.state_variables[0].integer_max = Some(u64::MAX);
        m.capabilities[0].writes.clear();
        let mut limited = Budget {
            limits: Limits {
                work: 20,
                cells: 20,
                ..DEFAULT
            },
            work: 0,
            cells: 0,
        };
        assert!(
            may_break(
                &m,
                &m.capabilities[0],
                &outside,
                &[ir::StateVarId(0)],
                &mut limited
            )
            .is_err()
        );
        assert!(limited.work <= 21);
    }
    use super::*;
    const WITHIN: &str = include_str!("../../../fixtures/resp2-within/model.relca");

    #[test]
    fn r2_time_budget_exact_work_cells_depth_and_preserved_ou() {
        let m = model(WITHIN);
        let d = &m.responsibility.as_ref().unwrap().duties[0];
        let row = &m.responsibility.as_ref().unwrap().responsibilities[0];
        let original_out = crate::engine::explore(&m);
        let mut budget = Budget {
            limits: DEFAULT,
            work: 0,
            cells: 0,
        };
        assert_eq!(
            time_witness(&m, row, &mut budget).unwrap().duration_upper,
            5
        );
        let minimum_depth = (0..=128)
            .find(|depth| {
                derive_time(
                    &m,
                    d,
                    row,
                    Limits {
                        depth: *depth,
                        ..DEFAULT
                    },
                ) == row.time_frame
            })
            .unwrap();
        assert!(minimum_depth > 0);
        for (limits, complete) in [
            (
                Limits {
                    work: budget.work,
                    cells: budget.cells,
                    depth: minimum_depth,
                },
                true,
            ),
            (
                Limits {
                    work: budget.work - 1,
                    cells: budget.cells,
                    depth: minimum_depth,
                },
                false,
            ),
            (
                Limits {
                    work: budget.work,
                    cells: budget.cells - 1,
                    depth: minimum_depth,
                },
                false,
            ),
            (
                Limits {
                    work: budget.work,
                    cells: budget.cells,
                    depth: minimum_depth - 1,
                },
                false,
            ),
        ] {
            struct TimeReset;
            impl Drop for TimeReset {
                fn drop(&mut self) {
                    TEST_TIME_LIMITS.with(|v| *v.borrow_mut() = None);
                }
            }
            TEST_TIME_LIMITS.with(|v| *v.borrow_mut() = Some(limits));
            let _reset = TimeReset;
            let changed = model(WITHIN);
            let changed_row = &changed.responsibility.as_ref().unwrap().responsibilities[0];
            assert_eq!(changed_row.observation, row.observation);
            assert_eq!(changed_row.refusal, row.refusal);
            assert_eq!(changed_row.warnings, row.warnings);
            if complete {
                assert_eq!(changed_row, row);
            } else {
                assert_eq!(
                    changed_row.time_frame,
                    TimeFrame::unchecked(d.within, "derivation_limit")
                );
            }
            assert_eq!(crate::engine::explore(&changed), original_out);
            assert_eq!(
                crate::reference::explore(&changed).stop,
                crate::reference::StopReason::Complete
            );
        }
    }

    #[test]
    fn r2_false_to_same_false_is_not_a_break_and_atomic_poststate_is_evaluated_once() {
        let m = model(include_str!("../../../fixtures/resp2-atomic/model.relca"));
        let Target::Formula(formula) = &m.responsibility.as_ref().unwrap().duties[0].target else {
            panic!()
        };
        let r = ir::StateVarId(
            m.state_variables
                .iter()
                .position(|v| v.name == "r")
                .unwrap(),
        );
        let ok = ir::StateVarId(
            m.state_variables
                .iter()
                .position(|v| v.name == "ok")
                .unwrap(),
        );
        let value = |b: bool| ir::StateValue::Value(ir::ValueId(usize::from(!b)));
        let bad = BTreeMap::from([(r, value(true)), (ok, value(false))]);
        let mut budget = Budget {
            limits: DEFAULT,
            work: 0,
            cells: 0,
        };
        let before = evaluate_formula(formula, &bad, &mut budget, 0).unwrap();
        let after = evaluate_formula(formula, &bad, &mut budget, 0).unwrap();
        assert!(!before && !after);
        assert!(
            !breaks_truth(before, after),
            "false-to-false is not a new breaking write"
        );
        let cap = m.capabilities.iter().find(|c| c.name == "put").unwrap();
        assert!(!may_break(&m, cap, formula, &[r, ok], &mut budget).unwrap());
    }

    #[test]
    fn r2_public_formula_maintenance_within_and_duplicate_ir_fail_closed() {
        let good = model(WITHIN);
        let atom = ir::StateMatch {
            variable: ir::StateVarId(0),
            value: ir::StateValue::Value(ir::ValueId(0)),
        };
        for target in [
            Target::Maintenance(ir::ActorId(usize::MAX)),
            Target::Formula(Formula::Bool(ir::StateMatch {
                variable: ir::StateVarId(usize::MAX),
                value: atom.value,
            })),
            Target::Formula(Formula::Bool(ir::StateMatch {
                variable: atom.variable,
                value: ir::StateValue::None,
            })),
            Target::Formula(Formula::State(ir::Cond::StateEq(ir::StateMatch {
                variable: atom.variable,
                value: ir::StateValue::Value(ir::ValueId(usize::MAX)),
            }))),
            Target::Formula(Formula::And(vec![])),
            Target::Formula(Formula::Or(vec![])),
        ] {
            let mut bad = good.clone();
            bad.responsibility.as_mut().unwrap().duties[0].target = target;
            rejected(&bad);
        }
        let mut bad = good.clone();
        bad.responsibility.as_mut().unwrap().duties[0].within = Some(4);
        rejected(&bad);
        let mut bad = good.clone();
        let mut duty = bad.responsibility.as_ref().unwrap().duties[0].clone();
        duty.within = Some(99);
        bad.responsibility.as_mut().unwrap().duties.push(duty);
        rejected(&bad);
        for state in [
            "latent case { r: bool = false }",
            "observed case { r: optional<bool> }",
            "observed case { r: Flag = off }",
        ] {
            let source = include_str!("../../../fixtures/resp2-bool/model.relca")
                .replace("observed case { r: bool = false }", state)
                .replace("for (r)", "for (r == true)");
            let source = if state.contains("Flag") {
                source.replace("for (r == true)", "for (r == off)")
            } else {
                source
            };
            let mut bad = model(&source);
            bad.responsibility.as_mut().unwrap().duties[0].target =
                Target::Formula(Formula::Bool(atom));
            assert!(validate_declarations(&bad, bad.responsibility.as_ref().unwrap()).is_err());
            rejected(&bad);
        }
        let duplicate = include_str!("../../../fixtures/resp2-direction/model.relca").replace(
            "  process Flow",
            "  responsible p for (r == false || ok == true)\n  process Flow",
        );
        assert!(
            crate::report::check(&duplicate)
                .diagnostics
                .iter()
                .any(|d| d.code == diag::U_SEM_0)
        );
        let nested = duplicate.replace("(r -> ok)", "((r && ok) -> ok)").replace(
            "(r == false || ok == true)",
            "((r == false || ok == false) || ok == true)",
        );
        assert!(
            crate::report::check(&nested)
                .diagnostics
                .iter()
                .any(|d| d.code == diag::U_SEM_0)
        );
    }

    #[test]
    fn r2_time_unknown_overflow_missing_path_and_unknown_priority() {
        let good = model(WITHIN);
        let d = &good.responsibility.as_ref().unwrap().duties[0];
        let row = &good.responsibility.as_ref().unwrap().responsibilities[0];
        let mut overflow = good.clone();
        overflow
            .capabilities
            .iter_mut()
            .find(|c| c.name == "check")
            .unwrap()
            .duration = u64::MAX;
        assert_eq!(
            derive_time(&overflow, d, row, DEFAULT),
            TimeFrame::unchecked(d.within, "duration_overflow")
        );
        let mut unknown = overflow.clone();
        unknown
            .capabilities
            .iter_mut()
            .find(|c| c.name == "put")
            .unwrap()
            .duration_distribution = Some(ir::DurationDistribution { entries: vec![] });
        assert_eq!(
            derive_time(&unknown, d, row, DEFAULT),
            TimeFrame::unchecked(d.within, "duration_unknown")
        );
        let mut missing = row.clone();
        missing.refusal.effects[0].call = "not-a-call".into();
        assert_eq!(
            derive_time(&good, d, &missing, DEFAULT),
            TimeFrame::unchecked(d.within, "path_not_reconstructed")
        );
        let mut no_path = good.clone();
        no_path
            .capabilities
            .iter_mut()
            .find(|c| c.name == "check")
            .unwrap()
            .reads
            .clear();
        assert_eq!(
            derive_time(&no_path, d, row, DEFAULT),
            TimeFrame::unchecked(d.within, "no_reachability_path")
        );
    }

    #[test]
    fn r2_formula_cut_reads_are_checked_inside_implication_and_public_ir() {
        let source = WITHIN.replace("  actor Record { source record }", "  actor Outside { latent truth: bool = false }\n  actor Record { source record }")
            .replace("include [Record,Reader,Judge,Effect]", "include [Record,Reader,Judge,Effect]; exclude Outside reason \"outside\" loses \"truth\"");
        let good = model(&source);
        let id = ir::StateVarId(
            good.state_variables
                .iter()
                .position(|v| v.name == "truth")
                .unwrap(),
        );
        let mut bad = good.clone();
        bad.responsibility.as_mut().unwrap().duties[0].target = Target::Formula(Formula::Not(
            Box::new(Formula::State(ir::Cond::StateEq(ir::StateMatch {
                variable: id,
                value: ir::StateValue::Value(ir::ValueId(0)),
            }))),
        ));
        assert!(!valid_formula(
            &bad,
            match &bad.responsibility.as_ref().unwrap().duties[0].target {
                Target::Formula(f) => f,
                _ => unreachable!(),
            }
        ));
        rejected(&bad);
        let source = source.replace(
            "for Judge.check within 5min",
            "for (r -> Outside.truth == true)",
        );
        let report = crate::report::check(&source);
        assert_eq!(report.verdict, crate::Verdict::Invalid);
        assert!(report.diagnostics.iter().any(|d| d.code == diag::E_CUT_1));
    }

    #[test]
    fn r2_implication_and_expansion_preserve_legacy_downstream_effect_candidates() {
        let src = include_str!("../../../fixtures/resp2-direction/model.relca")
            .replace("  actor Record", "  actor Last { capacity 1; capability commit(v:Ack) -> Ack { commits; duration fixed 1min; result nondet { committed } } }\n  actor Record")
            .replace("include [Record,Reader,Guard,SetR]", "include [Last,Record,Reader,Guard,SetR]")
            .replace("out = SetR.put(j); end done", "out = SetR.put(j); last = Last.commit(out); end done");
        let expanded = src.replace("(r -> ok)", "(r == false || ok == true)");
        let actual = crate::report::check(&src);
        let old = crate::report::check(&expanded);
        assert_eq!(actual.verdict, crate::Verdict::Satisfied);
        assert_eq!(
            actual.responsibilities.as_ref().unwrap()[0].refusal,
            old.responsibilities.as_ref().unwrap()[0].refusal
        );
    }

    #[test]
    fn r2_guard_only_intermediate_call_keeps_its_duration_and_exact_binding() {
        let source = include_str!("../../../fixtures/resp2-relay/model.relca")
            .replace("forward(v:Verdict)", "forward()")
            .replace("Relay.forward(j)", "Relay.forward()");
        // Direct observation=2. The successful guard makes the argument-free
        // relay depend on j: 2+4+3=9, not the direct j->out shortcut's 5.
        for source in [
            source.clone(),
            source.replace(
                "j = Judge.check(e);",
                "unrelated = Judge.check(e); j = Judge.check(e);",
            ),
        ] {
            let m = model(&source);
            let row = &m.responsibility.as_ref().unwrap().responsibilities[0];
            assert_eq!(row.time_frame.status, "satisfied");
            let time = row.time_frame.details.as_ref().unwrap();
            assert_eq!(time.required_duration, Some(9));
            assert_eq!(time.paths[0].calls, ["j", "relay", "out"]);
            let short = model(&source.replace("within 9min", "within 8min"));
            assert_eq!(
                short.responsibility.as_ref().unwrap().responsibilities[0]
                    .time_frame
                    .status,
                "violated"
            );
        }
    }
    #[test]
    fn r_unknown_sources_do_not_hide_a_known_refusal_gap() {
        // O cannot be derived without reads, but the known commits effect still has no stop.
        let src = include_str!("../../../fixtures/resp-missing-duty/model.relca")
            .replace("{ reads [Record]; commits;", "{ commits;");
        let r = crate::report::check(&src);
        let row = &r.responsibilities.as_ref().unwrap()[0];
        assert_eq!(row.observation.status, "not_derived");
        assert_eq!(row.refusal.status, "missing");
        assert!(r.diagnostics.iter().any(|d| d.code == diag::W_RESP_2));
    }
    fn model(source: &str) -> ir::Model {
        crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap()
    }
    const ORIGIN: &str = include_str!("../../../fixtures/resp-origin/model.relca");
    const CONTEST: &str = include_str!("../../../fixtures/resp-contest/model.relca");
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            TEST_LIMITS.with(|v| *v.borrow_mut() = None);
        }
    }
    fn use_limits(limits: Vec<Limits>) -> Reset {
        TEST_LIMITS.with(|v| *v.borrow_mut() = Some(limits));
        Reset
    }

    #[test]
    fn r_soft_limit_exact_work_cells_depth_overflow_and_no_partial_evidence() {
        let m = model(ORIGIN);
        let d = &m.responsibility.as_ref().unwrap().duties[0];
        let expected = derive_row(&m, d, 1, DEFAULT);
        let mut budget = Budget {
            limits: DEFAULT,
            work: 0,
            cells: 0,
        };
        derive_row_into(&m, d, &mut empty_row(&m, d, 1), &mut budget).unwrap();
        for (work, cells) in [
            (budget.work, budget.cells),
            (budget.work - 1, budget.cells),
            (budget.work, budget.cells - 1),
            (0, 0),
        ] {
            let row = derive_row(
                &m,
                d,
                1,
                Limits {
                    depth: 128,
                    work,
                    cells,
                },
            );
            if work == budget.work && cells == budget.cells {
                assert_eq!(row, expected);
            } else {
                assert_eq!(row, empty_row(&m, d, 1));
                assert!(row.warnings.is_empty());
                assert!(row.observation.required_sources.is_empty());
                assert!(row.refusal.effects.is_empty());
            }
        }
        let minimum = (0..=128)
            .find(|depth| {
                derive_row(
                    &m,
                    d,
                    1,
                    Limits {
                        depth: *depth,
                        ..DEFAULT
                    },
                ) == expected
            })
            .unwrap();
        assert!(minimum > 0);
        assert_eq!(
            derive_row(
                &m,
                d,
                1,
                Limits {
                    depth: minimum - 1,
                    ..DEFAULT
                }
            ),
            empty_row(&m, d, 1)
        );
        let mut counter = usize::MAX;
        assert!(Budget::add(&mut counter, 1, usize::MAX).is_err());
        assert_eq!(counter, usize::MAX);
    }

    #[test]
    fn r_soft_limit_is_per_row_order_independent_and_not_a_verdict() {
        let source = ORIGIN.replace(
            "responsible a1 for Approver.approve",
            "responsible a1 for Approver.approve\nresponsible a1 for Ledger.post",
        );
        let control = crate::report::check(&source);
        let normal = model(&source);
        let normal_out = crate::engine::explore(&normal);
        let reset = use_limits(vec![Limits { work: 0, ..DEFAULT }, DEFAULT]);
        let limited = model(&source);
        let meta = limited.responsibility.as_ref().unwrap();
        assert_eq!(
            meta.responsibilities[0],
            empty_row(&limited, &meta.duties[0], 1)
        );
        assert_eq!(
            meta.responsibilities[1],
            normal.responsibility.as_ref().unwrap().responsibilities[1]
        );
        assert_eq!(crate::engine::explore(&limited), normal_out);
        let actual = crate::report::check(&source);
        assert_eq!(actual.verdict, control.verdict);
        assert_eq!(actual.verdict.exit_code(), 0);
        assert_eq!(actual.terminals, control.terminals);
        assert_eq!(actual.properties, control.properties);
        assert!(
            actual
                .agency
                .as_ref()
                .unwrap()
                .unchecked
                .contains(&"responsibility_derivation_limit".into())
        );
        drop(reset);
        let reversed = source.replace(
            "responsible a1 for Approver.approve\nresponsible a1 for Ledger.post",
            "responsible a1 for Ledger.post\nresponsible a1 for Approver.approve",
        );
        let _reset = use_limits(vec![DEFAULT, Limits { work: 0, ..DEFAULT }]);
        let reverse = model(&reversed);
        let rows = &reverse.responsibility.as_ref().unwrap().responsibilities;
        let mut same = rows[0].clone();
        same.id = 2;
        assert_eq!(same, meta.responsibilities[1]);
        assert_eq!(
            rows[1].observation.reason.as_deref(),
            Some("derivation_limit")
        );
        // A contest coexisting with a limited duty remains a static satisfied row.
        let source = CONTEST.replace(
            "process Flow",
            "responsible a1 for Approver.approve\nprocess Flow",
        );
        TEST_LIMITS.with(|v| *v.borrow_mut() = Some(vec![Limits { work: 0, ..DEFAULT }]));
        let report = crate::report::check(&source);
        assert_eq!(
            report.responsibilities.as_ref().unwrap()[0]
                .observation
                .reason
                .as_deref(),
            Some("derivation_limit")
        );
        assert_eq!(
            report.contest_paths.as_ref().unwrap()[0].verdict,
            crate::Verdict::Satisfied
        );
    }

    #[test]
    fn r_agency_hard_limit_is_not_the_responsibility_soft_limit() {
        let guards = "if x == pass {} else {};".repeat(14);
        let source = format!(
            r#"model M version "0.2" {{
            type T=enum{{pass,fail}}
            people[p]
            actor A {{executors[p];capability a()->T{{duration fixed 1min;result nondet{{pass,fail}}}}}}
            actor E {{capacity 1;capability e()->T{{duration fixed 1min;result nondet{{pass}};commits}}}}
            responsible p for A.a
            process P{{x=A.a();{guards} y=E.e();end done}}
            bounds{{cases 1;live_tasks 2;clock 10min}}
            verify safety no_resource_overcommit
        }}"#
        );
        let _reset = use_limits(vec![Limits { work: 0, ..DEFAULT }]);
        let report = crate::report::check(&source);
        assert_eq!(report.verdict, crate::Verdict::Unsupported);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == diag::U_SEM_0 && d.message.contains("Agency"))
        );
        assert!(report.agency.is_none());
        assert!(report.responsibilities.is_none());
        assert!(report.contest_paths.is_none());
        assert!(report.properties.is_empty());
    }

    #[test]
    fn r_deep_responsibility_condition_soft_limits_without_rejecting_valid_ir() {
        let mut m = model(include_str!("../../../fixtures/resp-state/model.relca"));
        let Target::Condition(atom) = m.responsibility.as_ref().unwrap().duties[0].target.clone()
        else {
            panic!()
        };
        let mut condition = atom.clone();
        for _ in 0..130 {
            condition = ir::Cond::And(vec![atom.clone(), condition]);
        }
        m.responsibility.as_mut().unwrap().duties[0].target = Target::Condition(condition);
        derive(&mut m).unwrap();
        assert_eq!(
            m.responsibility.as_ref().unwrap().responsibilities[0]
                .observation
                .reason
                .as_deref(),
            Some("derivation_limit")
        );
        assert!(
            !crate::engine::explore(&m)
                .diagnostics
                .iter()
                .any(|d| d.severity == diag::Severity::Error)
        );
        assert_eq!(
            crate::reference::explore(&m).stop,
            crate::reference::StopReason::Complete
        );
    }

    fn rejected(m: &ir::Model) {
        let outcome = std::panic::catch_unwind(|| crate::engine::explore(m))
            .expect("validator must not panic");
        assert!(outcome.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
        let reference = std::panic::catch_unwind(|| crate::reference::explore(m))
            .expect("reference validator must not panic");
        assert_ne!(reference.stop, crate::reference::StopReason::Complete);
    }
    #[test]
    fn r_public_ir_new_indices_ordinals_and_certificates_fail_closed() {
        let good = model(&CONTEST.replace(
            "process Flow",
            "responsible a1 for Approver.approve\nprocess Flow",
        ));
        let mut cases = Vec::new();
        let mut bad = good.clone();
        bad.responsibility.as_mut().unwrap().duties[0].person = ir::PersonId(999);
        cases.push(bad);
        let mut bad = good.clone();
        bad.responsibility.as_mut().unwrap().duties[0].target =
            Target::Capability(ir::CapabilityId(999));
        cases.push(bad);
        let mut bad = good.clone();
        bad.responsibility.as_mut().unwrap().duties[0].target =
            Target::Condition(ir::Cond::StateEq(ir::StateMatch {
                variable: ir::StateVarId(999),
                value: ir::StateValue::None,
            }));
        cases.push(bad);
        let mut bad = good.clone();
        bad.responsibility.as_mut().unwrap().parties[0] = ir::ActorId(999);
        cases.push(bad);
        let mut bad = good.clone();
        bad.responsibility.as_mut().unwrap().parties.clear();
        cases.push(bad);
        let mut bad = good.clone();
        bad.responsibility.as_mut().unwrap().standings[0].target =
            crate::contest::ContestTarget::End(ir::EndId(999));
        cases.push(bad);
        let mut bad = good.clone();
        bad.responsibility.as_mut().unwrap().contests[0].route = ir::CapabilityId(999);
        cases.push(bad);
        let mut bad = good.clone();
        bad.responsibility.as_mut().unwrap().contests[0].reversible_by[0] = ir::PersonId(999);
        cases.push(bad);
        let mut bad = good.clone();
        bad.responsibility.as_mut().unwrap().contests[0].ordinal = 0;
        cases.push(bad);
        let mut bad = good.clone();
        bad.responsibility.as_mut().unwrap().contest_paths[0]
            .checks
            .route_exists
            .satisfied = false;
        cases.push(bad);
        let mut bad = good.clone();
        bad.responsibility.as_mut().unwrap().responsibilities[0]
            .observation
            .status = "present".into();
        cases.push(bad);
        let mut bad = good.clone();
        bad.verify.push(ir::Verify {
            property: ir::Property::Contest(999),
            span: bad.process.span,
        });
        cases.push(bad);
        for bad in cases {
            rejected(&bad);
        }
        let mixed = model(&CONTEST.replace(
            "contest Vendor",
            "permitted Approver.approve;\ncontest Vendor",
        ));
        let mut bad = mixed.clone();
        bad.contracts[0].ordinal = Some(2);
        rejected(&bad);
        let mut legacy = model(include_str!("../../../fixtures/exec-authority/model.relca"));
        assert!(legacy.responsibility.is_none());
        let before = crate::engine::explore(&legacy);
        legacy.responsibility = Some(Metadata::default());
        assert_eq!(crate::engine::explore(&legacy), before);
    }
}
