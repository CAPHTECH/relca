//! ADR 0009 static Agency subset. No runtime state, scheduling, or probabilities
//! enter this analysis. Limits are implementation limits, never business cuts.

use crate::{diag, ir};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

mod common_cause;
pub(crate) mod degradation;
pub use degradation::Degradation;
mod differences;
pub(crate) use differences::differences_for_models;
pub(crate) mod effects;
mod read_graph;
pub(crate) mod response;
pub use differences::{
    ComparisonSide, DifferenceReason, DifferenceStatus, Differences, differences,
};
#[cfg(test)]
#[path = "../tests/support/agency_second_half_budget.rs"]
mod second_half_budget;
#[cfg(test)]
#[path = "agency/second_half_tests.rs"]
mod second_half_tests;
pub use common_cause::{CommonCauses, ModelUncertainties, SourceDescription};
pub(crate) use read_graph::{concurrent_read, lexical_has_capability};
mod viewpoints;
pub use viewpoints::Viewpoints;
#[cfg(test)]
pub(crate) use viewpoints::fixture as viewpoints_fixture;
#[cfg(test)]
pub(crate) fn second_half_fixture(name: &str) -> bool {
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    NAMES
        .get_or_init(|| {
            let names: Vec<String> = serde_json::from_str(include_str!(
                "../tests/support/agency_second_half_names.json"
            ))
            .unwrap();
            assert_eq!(names.len(), 82);
            names
        })
        .iter()
        .any(|known| known == name)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Agency {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degradation: Option<Degradation>,
    pub rules: String,
    pub origin: Vec<Origin>,
    pub lineage: Vec<Lineage>,
    pub lineage_groups: Vec<LineageGroup>,
    pub refusal: Vec<Refusal>,
    pub unchecked: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub common_causes: Option<CommonCauses>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_uncertainties: Option<ModelUncertainties>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_descriptions: Option<Vec<SourceDescription>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewpoints: Option<Viewpoints>,
}

impl Agency {
    /// Empty metadata for direct execution-only IR builders, not evidence that
    /// lowering or static derivation has run.
    pub fn empty() -> Self {
        Self {
            degradation: None,
            rules: "agency-p0-1".into(),
            origin: Vec::new(),
            lineage: Vec::new(),
            lineage_groups: Vec::new(),
            refusal: Vec::new(),
            common_causes: None,
            model_uncertainties: None,
            source_descriptions: None,
            viewpoints: None,
            unchecked: [
                "W-RESP-1",
                "contest_path",
                "fallback_path",
                "executors",
                "via_model",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    pub actor: String,
    pub direct: Vec<String>,
    pub indirect: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lineage {
    pub call: String,
    pub capability: String,
    pub subject_sources: Vec<String>,
    pub independent_sources: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineageGroup {
    pub subject_sources: Vec<String>,
    pub judgements: usize,
    pub actors: usize,
    pub independent_sources: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Refusal {
    pub effect: String,
    pub call: String,
    pub reachable: bool,
    #[serde(deserialize_with = "decode_judgements")]
    pub judgements: Vec<Judgement>,
}
fn decode_judgements<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Vec<Judgement>, D::Error> {
    let rows = Vec::<Judgement>::deserialize(d)?;
    let mut rules = BTreeSet::new();
    for row in &rows {
        if let Judgement::Rule(r) = row
            && !rules.insert(&r.actor)
        {
            return Err(serde::de::Error::custom("duplicate Rule judgement"));
        }
    }
    Ok(rows)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Judgement {
    Process(ProcessJudgement),
    Rule(RuleJudgement),
}
impl Judgement {
    pub fn process(&self) -> Option<&ProcessJudgement> {
        if let Self::Process(p) = self {
            Some(p)
        } else {
            None
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RuleJudgementWire")]
pub struct RuleJudgement {
    pub kind: RuleKind,
    pub actor: String,
    pub version: String,
    pub reads: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleJudgementWire {
    kind: RuleKind,
    actor: String,
    version: String,
    reads: Vec<String>,
}
impl TryFrom<RuleJudgementWire> for RuleJudgement {
    type Error = &'static str;
    fn try_from(r: RuleJudgementWire) -> std::result::Result<Self, Self::Error> {
        let mut seen = BTreeSet::new();
        if r.actor.is_empty()
            || r.version.is_empty()
            || r.reads.is_empty()
            || r.reads.iter().any(|s| {
                !s.starts_with(&format!("{}.", r.actor))
                    || s.len() <= r.actor.len() + 1
                    || !seen.insert(s)
            })
        {
            return Err("invalid Rule judgement");
        }
        Ok(Self {
            kind: r.kind,
            actor: r.actor,
            version: r.version,
            reads: r.reads,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleKind {
    #[serde(rename = "rule")]
    Rule,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessJudgement {
    pub call: String,
    pub capability: String,
    pub stops: Stops,
    pub bypasses: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stops {
    AllPaths,
    Conditional,
    None,
}

#[cfg(test)]
mod rule_wire_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn rule_union_roundtrip_is_closed_and_has_no_process_fields() {
        let rule = json!({"kind":"rule","actor":"R","version":"r1","reads":["R.x"]});
        let parsed: Judgement = serde_json::from_value(rule.clone()).unwrap();
        assert!(parsed.process().is_none());
        assert_eq!(serde_json::to_value(parsed).unwrap(), rule);
        for key in [
            "call",
            "capability",
            "stops",
            "bypasses",
            "people",
            "unknown",
        ] {
            let mut bad = rule.clone();
            bad[key] = json!("forged");
            assert!(serde_json::from_value::<Judgement>(bad).is_err(), "{key}");
        }
        for key in ["kind", "actor", "version", "reads"] {
            let mut bad = rule.clone();
            bad.as_object_mut().unwrap().remove(key);
            assert!(
                serde_json::from_value::<Judgement>(bad).is_err(),
                "missing {key}"
            );
        }
        for reads in [json!([]), json!(["R.x", "R.x"]), json!(["Other.x"])] {
            let mut bad = rule.clone();
            bad["reads"] = reads;
            assert!(serde_json::from_value::<Judgement>(bad).is_err());
        }
        let duplicate =
            json!({"effect":"A.post","call":"a","reachable":true,"judgements":[rule,rule]});
        assert!(serde_json::from_value::<Refusal>(duplicate).is_err());
        let old = json!({"call":"a","capability":"A.judge","stops":"all_paths","bypasses":0});
        let process: Judgement = serde_json::from_value(old.clone()).unwrap();
        assert_eq!(serde_json::to_value(process).unwrap(), old);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViaModelPaths {
    pub rules: String,
    pub basis: String,
    pub scope: String,
    pub truncated: bool,
    pub path_count_lower_bound: u64,
    pub paths: Vec<ViaPath>,
    pub unchecked: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViaPath {
    pub actor: String,
    pub model: String,
    pub start_call: String,
    pub calls: Vec<ViaCall>,
    pub target: ViaTarget,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViaCall {
    pub call: String,
    pub capability: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ViaTarget {
    E {
        call: String,
    },
    U {
        call: String,
        effect_call: String,
        stops: Stops,
    },
}

#[derive(Clone, Copy)]
struct ViaLimits {
    paths: usize,
    work: usize,
    cells: usize,
    depth: usize,
}
const VIA_LIMITS: ViaLimits = ViaLimits {
    paths: 4096,
    work: 1_000_000,
    cells: 1_000_000,
    depth: 128,
};

#[cfg(test)]
mod via_limit_tests {
    use super::*;
    #[test]
    fn independent_path_work_cell_and_depth_budgets_have_exact_boundaries() {
        let src = include_str!("../../../fixtures/act-via/model.relca");
        let m = crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let full = via_model_paths(&m).unwrap();
        assert_eq!(full.paths.len(), 2);
        assert!(!full.truncated);
        for category in ["paths", "work", "cells", "depth"] {
            let with = |n| {
                let mut limits = VIA_LIMITS;
                match category {
                    "paths" => limits.paths = n,
                    "work" => limits.work = n,
                    "cells" => limits.cells = n,
                    _ => limits.depth = n,
                }
                via_with_limits(&m, limits).unwrap()
            };
            let k = (1..2048)
                .find(|n| !with(*n).truncated)
                .expect("small exact budget");
            let exact = with(k);
            let short = with(k - 1);
            assert_eq!(exact, full, "{category} at k={k}");
            assert!(short.truncated, "{category} at k-1");
            let expected = match category {
                "paths" => "path_enumeration_limit",
                "work" => "path_work_limit",
                "cells" => "path_cell_limit",
                _ => "path_depth_limit",
            };
            assert!(
                short.unchecked.iter().any(|s| s == expected),
                "{category}: {short:?}"
            );
            assert!(short.path_count_lower_bound >= short.paths.len() as u64);
        }
        let no_via = crate::lower::lower(
            &crate::syntax::parse(&src.replace("via model \"doc@v3\"; ", "")).unwrap(),
        )
        .unwrap();
        assert!(
            via_with_limits(
                &no_via,
                ViaLimits {
                    paths: 0,
                    work: 0,
                    cells: 0,
                    depth: 0
                }
            )
            .is_none()
        );
    }
}

pub(crate) fn via_model_paths(m: &ir::Model) -> Option<ViaModelPaths> {
    via_with_limits(m, VIA_LIMITS)
}

fn via_with_limits(m: &ir::Model, limits: ViaLimits) -> Option<ViaModelPaths> {
    if !m.actors.iter().any(|a| a.metadata.via_model.is_some()) {
        return None;
    }
    let mut result = ViaModelPaths {
        rules: "via-model-read-paths-1".into(),
        basis: "追跡であって、AI を介した変化の認定ではない".into(),
        scope: "selected_process_lexical_reads".into(),
        truncated: false,
        path_count_lower_bound: 0,
        paths: Vec::new(),
        unchecked: [
            "transformation_recognition",
            "state_guard_control_paths",
            "contest_paths",
            "fallback_paths",
        ]
        .map(String::from)
        .to_vec(),
    };
    fn truncated(r: &mut ViaModelPaths, why: &str) {
        r.truncated = true;
        if !r.unchecked.iter().any(|s| s == why) {
            r.unchecked.push(why.into());
        }
    }
    let n = m.process.bindings.len();
    let Some(mut cells) = n.checked_mul(4).filter(|n| *n <= limits.cells) else {
        truncated(&mut result, "path_cell_limit");
        return Some(result);
    };
    let mut budget = Budget {
        limits: DEFAULT,
        work: 0,
        cells: 0,
        contexts: 0,
        paths: 0,
    };
    let mut indexed = vec![None; n];
    if collect(&m.process.body, &mut indexed, m, 0, &mut budget).is_err() {
        truncated(&mut result, "path_work_limit");
        return Some(result);
    }
    let Some(calls) = indexed.into_iter().collect::<Option<Vec<_>>>() else {
        truncated(&mut result, "path_work_limit");
        return Some(result);
    };
    let mut edges = vec![Vec::new(); n];
    let mut targets = vec![Vec::new(); n];
    let extra = match read_graph::read_parents(m, &calls) {
        Ok(p) => p,
        Err(_) => {
            truncated(&mut result, "path_work_limit");
            return Some(result);
        }
    };
    if extra.iter().any(|p| !p.is_empty()) {
        result.rules = "via-model-read-paths-reads-1".into();
    }
    let mut work = budget.work;
    for c in &calls {
        for arg in c.args.iter().chain(&extra[c.binding.0]) {
            if *arg == ir::CURRENT_CASE {
                continue;
            }
            if cells >= limits.cells {
                truncated(&mut result, "path_cell_limit");
                return Some(result);
            }
            cells += 1;
            edges[arg.0].push(c.binding.0);
        }
        let name = &m.process.bindings[c.binding.0].name;
        if m.capabilities[c.capability.0].commits {
            targets[c.binding.0].push(ViaTarget::E { call: name.clone() });
        }
        for effect in &m.agency.refusal {
            for j in effect.judgements.iter().filter_map(Judgement::process) {
                work += 1;
                if work > limits.work {
                    truncated(&mut result, "path_work_limit");
                    return Some(result);
                }
                if j.call == *name && j.stops != Stops::None {
                    if cells >= limits.cells {
                        truncated(&mut result, "path_cell_limit");
                        return Some(result);
                    }
                    cells += 1;
                    targets[c.binding.0].push(ViaTarget::U {
                        call: name.clone(),
                        effect_call: effect.call.clone(),
                        stops: j.stops,
                    });
                }
            }
        }
    }
    for next in &mut edges {
        next.sort_unstable();
        next.dedup();
    }
    struct Walker<'a> {
        m: &'a ir::Model,
        calls: &'a [&'a ir::Call],
        edges: &'a [Vec<usize>],
        targets: &'a [Vec<ViaTarget>],
        limits: ViaLimits,
        work: usize,
        cells: usize,
        result: &'a mut ViaModelPaths,
    }
    impl Walker<'_> {
        fn walk(&mut self, start: usize, node: usize, path: &mut Vec<usize>) -> bool {
            if self.work >= self.limits.work {
                truncated(self.result, "path_work_limit");
                return false;
            }
            self.work += 1;
            if path.len() >= self.limits.depth {
                truncated(self.result, "path_depth_limit");
                return false;
            }
            // The lexical read graph is a DAG, already validated by Agency.
            path.push(node);
            for target in &self.targets[node] {
                self.result.path_count_lower_bound += 1;
                if self.result.paths.len() >= self.limits.paths {
                    truncated(self.result, "path_enumeration_limit");
                    path.pop();
                    return false;
                }
                let Some(cells) = self
                    .cells
                    .checked_add(path.len() + 1)
                    .filter(|n| *n <= self.limits.cells)
                else {
                    truncated(self.result, "path_cell_limit");
                    path.pop();
                    return false;
                };
                self.cells = cells;
                let actor =
                    &self.m.actors[self.m.capabilities[self.calls[start].capability.0].actor.0];
                self.result.paths.push(ViaPath {
                    actor: actor.name.clone(),
                    model: actor.metadata.via_model.clone().expect("via start"),
                    start_call: self.m.process.bindings[start].name.clone(),
                    calls: path
                        .iter()
                        .map(|i| {
                            let cap = &self.m.capabilities[self.calls[*i].capability.0];
                            ViaCall {
                                call: self.m.process.bindings[*i].name.clone(),
                                capability: format!(
                                    "{}.{}",
                                    self.m.actors[cap.actor.0].name, cap.name
                                ),
                            }
                        })
                        .collect(),
                    target: target.clone(),
                });
            }
            for next in &self.edges[node] {
                if !self.walk(start, *next, path) {
                    path.pop();
                    return false;
                }
            }
            path.pop();
            true
        }
    }
    let mut walker = Walker {
        m,
        calls: &calls,
        edges: &edges,
        targets: &targets,
        limits,
        work,
        cells,
        result: &mut result,
    };
    for (i, c) in calls.iter().enumerate() {
        if m.actors[m.capabilities[c.capability.0].actor.0]
            .metadata
            .via_model
            .is_some()
            && !walker.walk(i, i, &mut Vec::new())
        {
            break;
        }
    }
    Some(result)
}

#[derive(Clone, Copy)]
struct Limits {
    depth: usize,
    paths: usize,
    contexts: usize,
    cells: usize,
    work: usize,
}
const DEFAULT: Limits = Limits {
    depth: 128,
    paths: 4096,
    contexts: 65_536,
    cells: 1_000_000,
    work: 1_000_000,
};
type Result<T> = std::result::Result<T, diag::Diagnostic>;
fn invalid() -> diag::Diagnostic {
    diag::Diagnostic {
        code: diag::U_SEM_0.into(),
        severity: diag::Severity::Error,
        message: "Agency derivation limit exceeded or invalid static IR".into(),
        span: None,
    }
}
struct Budget {
    limits: Limits,
    work: usize,
    cells: usize,
    contexts: usize,
    paths: usize,
}
impl Budget {
    fn add(counter: &mut usize, n: usize, max: usize) -> Result<()> {
        *counter = counter
            .checked_add(n)
            .filter(|v| *v <= max)
            .ok_or_else(invalid)?;
        Ok(())
    }
    fn work(&mut self) -> Result<()> {
        self.steps(1)
    }
    fn steps(&mut self, n: usize) -> Result<()> {
        Self::add(&mut self.work, n, self.limits.work)
    }
    fn cells(&mut self, n: usize) -> Result<()> {
        Self::add(&mut self.cells, n, self.limits.cells)
    }
    fn path(&mut self) -> Result<()> {
        Self::add(&mut self.paths, 1, self.limits.paths)
    }
    fn insert<T: Ord>(&mut self, set: &mut BTreeSet<T>, value: T) -> Result<()> {
        self.work()?;
        if !set.contains(&value) {
            self.cells(1)?;
            set.insert(value);
        }
        Ok(())
    }
    fn copy(&mut self, cells: usize) -> Result<()> {
        self.steps(cells)?;
        self.cells(cells)
    }
}

#[derive(Clone)]
enum Guard<'a> {
    Symbolic(Symbolic, bool),
    Domain(ir::BindingId, Vec<ir::ValueId>),
    If(&'a ir::Cond, bool),
    Any(&'a [ir::ResultMatch]),
}

#[derive(Clone, Default)]
struct Path<'a> {
    rule_sources: BTreeSet<ir::StateVarId>,
    // Set by call_paths just before an effect is recorded: the effect's own
    // requires reads a Rule and the selected Rule values make it true on every
    // assignment of this path before the guard (ADR 0044 ruling M1). A Rule
    // read elsewhere on the path never sets it.
    requires_fixed_by_rule: bool,
    store: BTreeMap<ir::StateVarId, Provenance>,
    guards: Vec<Guard<'a>>,
    defined: BTreeSet<ir::BindingId>,
    // None is the legacy mode: no new allocation or legacy-budget charge.
    trace: Option<viewpoints::Trace>,
}
type Values = BTreeMap<ir::BindingId, ir::ValueId>;

#[derive(Clone)]
enum Provenance {
    Constant(ir::StateValue),
    Result(ir::BindingId),
    Unknown,
}
#[derive(Clone)]
enum Symbolic {
    Constant(bool),
    Eq(ir::ResultMatch),
    And(Vec<Symbolic>),
    Or(Vec<Symbolic>),
    Unknown,
}
impl Symbolic {
    fn unknown(&self) -> bool {
        match self {
            Self::Unknown => true,
            Self::And(cs) | Self::Or(cs) => cs.iter().any(Self::unknown),
            _ => false,
        }
    }
    fn support(&self, out: &mut BTreeSet<ir::BindingId>, b: &mut Budget) -> Result<()> {
        b.work()?;
        match self {
            Self::Eq(x) => b.insert(out, x.binding)?,
            Self::And(cs) | Self::Or(cs) => {
                for c in cs {
                    c.support(out, b)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn eval(&self, values: &Values, b: &mut Budget) -> Result<bool> {
        b.work()?;
        Ok(match self {
            Self::Constant(v) => *v,
            Self::Eq(x) => *values.get(&x.binding).ok_or_else(invalid)? == x.value,
            Self::And(cs) => {
                let mut v = true;
                for c in cs {
                    v &= c.eval(values, b)?;
                }
                v
            }
            Self::Or(cs) => {
                let mut v = false;
                for c in cs {
                    v |= c.eval(values, b)?;
                }
                v
            }
            Self::Unknown => return Err(invalid()),
        })
    }
}
fn symbolic(c: &ir::Cond, p: &Path, m: &ir::Model, b: &mut Budget) -> Result<Symbolic> {
    b.work()?;
    Ok(match c {
        ir::Cond::Contains { .. } | ir::Cond::IntCompare { .. } => Symbolic::Unknown,
        ir::Cond::StateEq(x) => {
            if matches!(
                m.state_variables[x.variable.0].kind,
                ir::StateKind::ActorObserved(_)
            ) {
                return Ok(Symbolic::Unknown);
            }
            let origin = p.store.get(&x.variable).cloned().unwrap_or_else(|| {
                match m.state_variables[x.variable.0].initial {
                    ir::InitialValue::Fixed(v) => Provenance::Constant(v),
                    _ => Provenance::Unknown,
                }
            });
            match origin {
                Provenance::Constant(v) => Symbolic::Constant(v == x.value),
                Provenance::Result(binding) => match x.value {
                    ir::StateValue::Value(value) => {
                        Symbolic::Eq(ir::ResultMatch { binding, value })
                    }
                    ir::StateValue::None | ir::StateValue::Integer(_) | ir::StateValue::Set(_) => {
                        Symbolic::Constant(false)
                    }
                },
                Provenance::Unknown => Symbolic::Unknown,
            }
        }
        ir::Cond::Eq(_) if unknown_atomic_result(c, m) => Symbolic::Unknown,
        ir::Cond::Eq(x) => Symbolic::Eq(*x),
        ir::Cond::And(cs) => Symbolic::And(
            cs.iter()
                .map(|c| symbolic(c, p, m, b))
                .collect::<Result<_>>()?,
        ),
        ir::Cond::Or(cs) => Symbolic::Or(
            cs.iter()
                .map(|c| symbolic(c, p, m, b))
                .collect::<Result<_>>()?,
        ),
    })
}

fn apply_writers(p: &mut Path, c: &ir::Call, m: &ir::Model, b: &mut Budget) -> Result<()> {
    let cap = &m.capabilities[c.capability.0];
    b.copy(
        cap.start_writes.len()
            + cap.writes.len()
            + cap.kernel_writes.len()
            + cap.atomic.as_ref().map_or(0, |a| a.writes.len()),
    )?;
    for w in cap.start_writes.iter().chain(&cap.writes) {
        let origin = match w.source {
            ir::WriteSource::Executor | ir::WriteSource::InsertCase { .. } => Provenance::Unknown,
            ir::WriteSource::Integer { .. } => Provenance::Unknown,
            ir::WriteSource::Constant(v) => Provenance::Constant(v),
            ir::WriteSource::Result => Provenance::Result(c.binding),
        };
        p.store.insert(w.variable, origin);
    }
    if let Some(a) = &cap.atomic {
        for w in &a.writes {
            p.store.insert(w.variable, Provenance::Unknown);
        }
    }
    if cap.version_check.is_some() {
        for w in &cap.writes {
            p.store.insert(w.variable, Provenance::Unknown);
        }
    }
    if let Some(k) = cap.structural_kernel() {
        for (v, _) in k
            .rows
            .iter()
            .flat_map(|r| &r.branches)
            .flat_map(|b| &b.overrides)
        {
            p.store.insert(*v, Provenance::Unknown);
        }
    }
    Ok(())
}

// A Rule in a process guard must not unlock requires' case-writer
// substitution for neighbouring mutable state leaves.
fn process_rule_symbolic(
    c: &ir::Cond,
    p: &Path,
    m: &ir::Model,
    b: &mut Budget,
) -> Result<Symbolic> {
    b.work()?;
    match c {
        ir::Cond::StateEq(x)
            if !matches!(m.state_variables[x.variable.0].kind, ir::StateKind::Rule(_)) =>
        {
            Ok(Symbolic::Unknown)
        }
        ir::Cond::And(cs) => Ok(Symbolic::And(
            cs.iter()
                .map(|c| process_rule_symbolic(c, p, m, b))
                .collect::<Result<_>>()?,
        )),
        ir::Cond::Or(cs) => Ok(Symbolic::Or(
            cs.iter()
                .map(|c| process_rule_symbolic(c, p, m, b))
                .collect::<Result<_>>()?,
        )),
        _ => symbolic(c, p, m, b),
    }
}

#[cfg(test)]
mod caseset_budget_test {
    use super::*;
    #[test]
    fn caseset_atomic_unknown_store_is_charged_before_insertion() {
        let m = crate::lower::lower(
            &crate::syntax::parse(include_str!("../../../fixtures/aset-dup/model.relca")).unwrap(),
        )
        .unwrap();
        let c = find_call(&m.process.body, ir::BindingId(0)).unwrap();
        let mut p = Path::default();
        let mut b = Budget {
            limits: Limits {
                cells: 0,
                ..DEFAULT
            },
            work: 0,
            cells: 0,
            contexts: 0,
            paths: 0,
        };
        assert!(apply_writers(&mut p, c, &m, &mut b).is_err());
        assert!(p.store.is_empty());
    }
}

fn call_paths<'a>(
    c: &ir::Call,
    incoming: Vec<Path<'a>>,
    effects: &mut BTreeMap<ir::BindingId, Vec<Path<'a>>>,
    m: &ir::Model,
    b: &mut Budget,
) -> Result<Vec<Path<'a>>> {
    let cap = &m.capabilities[c.capability.0];
    let mut out = Vec::new();
    for mut p in incoming {
        if let Some(req) = &cap.requires {
            if p.trace.is_some() {
                viewpoints::condition_depth(&req.condition, b, 0)?;
            }
            let guard = symbolic(&req.condition, &p, m, b)?;
            let fixed = record_rule_constraint(&req.condition, &guard, true, &mut p, m, b)?;
            b.path()?;
            b.copy(
                p.defined.len()
                    + p.guards.len()
                    + p.store.len()
                    + cap.results.len()
                    + usize::from(cap.version_check.is_some())
                    + 3,
            )?;
            let unresolved_control = p.trace.is_some() && viewpoints::uncertain(&p);
            if let Some(trace) = &mut p.trace {
                let mut support = BTreeSet::new();
                guard.support(&mut support, b)?;
                // A later requires may be symbolically expressible while its
                // result still depends on an earlier unknown start/refusal.
                // Do not fix that dependent Domain as an independent L context.
                if unresolved_control
                    || guard.unknown()
                    || !support.is_disjoint(&trace.unknown_results)
                {
                    b.insert(&mut trace.unknown_results, c.binding)?;
                }
                b.copy(trace.cells()?)?;
            }
            if p.trace.is_some() {
                viewpoints::guard_storage(&p, b)?;
                viewpoints::symbolic_storage(&guard, b)?;
            }
            let mut refused = p.clone();
            refused.guards.push(Guard::Symbolic(guard.clone(), false));
            refused
                .guards
                .push(Guard::Domain(c.binding, vec![req.otherwise]));
            b.insert(&mut refused.defined, c.binding)?;
            out.push(refused);
            p.guards.push(Guard::Symbolic(guard, true));
            p.requires_fixed_by_rule = fixed;
            record_effect(c, std::slice::from_ref(&p), effects, m, b)?;
            p.guards
                .push(Guard::Domain(c.binding, cap.finish_results()));
        } else {
            p.requires_fixed_by_rule = false;
            record_effect(c, std::slice::from_ref(&p), effects, m, b)?;
        }
        b.insert(&mut p.defined, c.binding)?;
        if let Some(trace) = &mut p.trace {
            b.insert(&mut trace.started, c.binding)?;
        }
        apply_writers(&mut p, c, m, b)?;
        out.push(p);
    }
    Ok(out)
}

fn has_state(c: &ir::Cond) -> bool {
    match c {
        ir::Cond::Contains { .. } | ir::Cond::StateEq(_) | ir::Cond::IntCompare { .. } => true,
        ir::Cond::Eq(_) => false,
        ir::Cond::And(cs) | ir::Cond::Or(cs) => cs.iter().any(has_state),
    }
}

fn find_call(b: &ir::Block, id: ir::BindingId) -> Option<&ir::Call> {
    for s in b {
        match &s.kind {
            ir::Stmt::Call(c) | ir::Stmt::Response(c, _) if c.binding == id => return Some(c),
            ir::Stmt::Response(_, r) if r.command == id => return Some(&r.command_call),
            ir::Stmt::Fork { branches, .. } => {
                if let Some(c) = branches.iter().find(|c| c.binding == id) {
                    return Some(c);
                }
            }
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                if let Some(c) = find_call(then_body, id).or_else(|| find_call(else_body, id)) {
                    return Some(c);
                }
            }
            _ => {}
        }
    }
    None
}

fn unknown_atomic_result(c: &ir::Cond, m: &ir::Model) -> bool {
    match c {
        ir::Cond::Eq(x) => find_call(&m.process.body, x.binding).is_some_and(|c| {
            m.capabilities
                .get(c.capability.0)
                .is_some_and(|c| c.history_dependent_result())
        }),
        ir::Cond::And(cs) | ir::Cond::Or(cs) => cs.iter().any(|c| unknown_atomic_result(c, m)),
        _ => false,
    }
}

fn mutable_state(c: &ir::Cond, m: &ir::Model) -> bool {
    match c {
        ir::Cond::StateEq(x) => {
            !matches!(m.state_variables[x.variable.0].kind, ir::StateKind::Rule(_))
        }
        ir::Cond::And(cs) | ir::Cond::Or(cs) => cs.iter().any(|c| mutable_state(c, m)),
        _ => has_state(c),
    }
}
fn rule_variables(c: &ir::Cond, m: &ir::Model, out: &mut BTreeSet<ir::StateVarId>) {
    match c {
        ir::Cond::StateEq(x)
            if matches!(m.state_variables[x.variable.0].kind, ir::StateKind::Rule(_)) =>
        {
            out.insert(x.variable);
        }
        ir::Cond::And(cs) | ir::Cond::Or(cs) => {
            for c in cs {
                rule_variables(c, m, out);
            }
        }
        _ => {}
    }
}
pub(crate) fn reads_rule(c: &ir::Cond, m: &ir::Model) -> bool {
    let mut reads = BTreeSet::new();
    rule_variables(c, m, &mut reads);
    !reads.is_empty()
}
/// Records the Rule reads of `c` as a constraint source when some assignment
/// of the path before this guard makes the guard differ from `truth`. Returns
/// true only when `c` reads a Rule and no such assignment exists, i.e. the
/// selected Rule values make the guard a tautology on this path. An unknown
/// guard or path is never reported as a tautology.
fn record_rule_constraint(
    c: &ir::Cond,
    guard: &Symbolic,
    truth: bool,
    p: &mut Path,
    m: &ir::Model,
    b: &mut Budget,
) -> Result<bool> {
    let mut reads = BTreeSet::new();
    rule_variables(c, m, &mut reads);
    if reads.is_empty() {
        return Ok(false);
    }
    if guard.unknown()
        || p.guards.iter().any(|g| {
            matches!(g, Guard::Symbolic(s, _) if s.unknown())
                || matches!(g, Guard::If(c, _) if has_state(c))
        })
    {
        return Ok(false);
    }
    let mut support = path_support(p, m, b)?;
    guard.support(&mut support, b)?;
    b.cells(m.process.bindings.len())?;
    let mut indexed = vec![None; m.process.bindings.len()];
    collect(&m.process.body, &mut indexed, m, 0, b)?;
    let calls: Vec<_> = indexed
        .into_iter()
        .collect::<Option<_>>()
        .ok_or_else(invalid)?;
    if assignments(
        &support.into_iter().collect::<Vec<_>>(),
        &Values::new(),
        &calls,
        m,
        b,
        |values, b| Ok(satisfied(p, values, b)? && guard.eval(values, b)? != truth),
    )? {
        for v in reads {
            b.insert(&mut p.rule_sources, v)?;
        }
        return Ok(false);
    }
    Ok(true)
}

fn block_has_state_guard(b: &ir::Block, m: &ir::Model) -> bool {
    b.iter().any(|s| match &s.kind {
        ir::Stmt::Response(_, _) => false, // A receipt is not a mutable-state If.
        ir::Stmt::If {
            condition,
            then_body,
            else_body,
        } => {
            mutable_state(condition, m)
                || block_has_state_guard(then_body, m)
                || block_has_state_guard(else_body, m)
        }
        _ => false,
    })
}

fn condition_support(
    cond: &ir::Cond,
    out: &mut BTreeSet<ir::BindingId>,
    model: &ir::Model,
    depth: usize,
    budget: &mut Budget,
) -> Result<()> {
    if depth > budget.limits.depth {
        return Err(invalid());
    }
    budget.work()?;
    match cond {
        ir::Cond::Contains { set, .. } => {
            if !model
                .state_variables
                .get(set.0)
                .is_some_and(|v| v.case_set_size.is_some())
            {
                return Err(invalid());
            }
        }
        ir::Cond::IntCompare { variable, .. } => {
            if !model
                .state_variables
                .get(variable.0)
                .is_some_and(|v| v.kind.is_observed() && v.integer_max.is_some())
            {
                return Err(invalid());
            }
        }
        ir::Cond::StateEq(m) => {
            let v = model
                .state_variables
                .get(m.variable.0)
                .ok_or_else(invalid)?;
            let t = model.types.get(v.base.0).ok_or_else(invalid)?;
            if v.kind.is_latent()
                || !match m.value {
                    ir::StateValue::None => v.optional,
                    ir::StateValue::Value(x) => x.0 < t.members.len(),
                    ir::StateValue::Integer(_) | ir::StateValue::Set(_) => false,
                }
            {
                return Err(invalid());
            }
        }
        ir::Cond::Eq(m) => {
            let b = model
                .process
                .bindings
                .get(m.binding.0)
                .ok_or_else(invalid)?;
            let ty = model.types.get(b.ty.0).ok_or_else(invalid)?;
            if m.value.0 >= ty.members.len() {
                return Err(invalid());
            }
            budget.insert(out, m.binding)?;
        }
        ir::Cond::And(parts) | ir::Cond::Or(parts) => {
            if parts.len() < 2 {
                return Err(invalid());
            }
            for part in parts {
                condition_support(part, out, model, depth + 1, budget)?;
            }
        }
    }
    Ok(())
}

fn collect<'a>(
    block: &'a ir::Block,
    calls: &mut [Option<&'a ir::Call>],
    model: &ir::Model,
    depth: usize,
    budget: &mut Budget,
) -> Result<()> {
    if depth > budget.limits.depth {
        return Err(invalid());
    }
    for stmt in block {
        budget.work()?;
        let mut register = |c: &'a ir::Call| -> Result<()> {
            budget.work()?;
            let slot = calls.get_mut(c.binding.0).ok_or_else(invalid)?;
            if slot.replace(c).is_some() {
                return Err(invalid());
            }
            Ok(())
        };
        match &stmt.kind {
            ir::Stmt::Wait(_) | ir::Stmt::WaitUntil(_) => {}
            ir::Stmt::Call(c) => register(c)?,
            ir::Stmt::Response(c, metadata) => {
                register(c)?;
                if metadata.attempt == 1 {
                    register(&metadata.command_call)?;
                }
            }
            ir::Stmt::Fork { id, branches, join } => {
                if branches.is_empty() || id.0 >= model.process.forks.len() {
                    return Err(invalid());
                }
                for c in branches {
                    register(c)?;
                }
                if let ir::Join::Any { matches, .. } = join {
                    if matches.len() != branches.len() {
                        return Err(invalid());
                    }
                    budget.copy(branches.len())?;
                    let branch_ids: BTreeSet<_> = branches.iter().map(|c| c.binding).collect();
                    let mut seen = BTreeSet::new();
                    for m in matches {
                        budget.work()?;
                        if !branch_ids.contains(&m.binding) || seen.contains(&m.binding) {
                            return Err(invalid());
                        }
                        budget.insert(&mut seen, m.binding)?;
                        condition_support(
                            &ir::Cond::Eq(*m),
                            &mut BTreeSet::new(),
                            model,
                            0,
                            budget,
                        )?;
                    }
                }
            }
            ir::Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                condition_support(condition, &mut BTreeSet::new(), model, 0, budget)?;
                collect(then_body, calls, model, depth + 1, budget)?;
                collect(else_body, calls, model, depth + 1, budget)?;
            }
            ir::Stmt::End(id) => {
                if id.0 >= model.ends.len() {
                    return Err(invalid());
                }
            }
        }
    }
    Ok(())
}

// Preserve unsimplified support and lexical definitions, including on impossible
// branches. End cuts the continuation; where guards only the fork's suffix.
fn paths<'a>(
    block: &'a ir::Block,
    mut incoming: Vec<Path<'a>>,
    effects: &mut BTreeMap<ir::BindingId, Vec<Path<'a>>>,
    model: &ir::Model,
    budget: &mut Budget,
    mut recorder: Option<&mut viewpoints::Recorder<'a>>,
    depth: usize,
) -> Result<Vec<Path<'a>>> {
    if recorder.is_some() && depth >= budget.limits.depth {
        return Err(invalid());
    }
    for stmt in block {
        budget.work()?;
        match &stmt.kind {
            ir::Stmt::Wait(_) | ir::Stmt::WaitUntil(_) => {}
            ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => {
                if let Some(r) = recorder.as_deref_mut()
                    && viewpoints::precise_resources(model)
                    && viewpoints::infeasible_call(model, c)?
                {
                    for p in incoming.drain(..) {
                        r.blocked(p, budget)?;
                    }
                    continue;
                }
                if model.has_requires()
                    || model
                        .capabilities
                        .iter()
                        .any(|c| c.history_dependent_result())
                {
                    incoming = call_paths(c, incoming, effects, model, budget)?;
                    if let ir::Stmt::Response(_, metadata) = &stmt.kind {
                        for p in &mut incoming {
                            budget.insert(&mut p.defined, metadata.command)?;
                        }
                    }
                    continue;
                }
                record_effect(c, &incoming, effects, model, budget)?;
                for p in &mut incoming {
                    budget.insert(&mut p.defined, c.binding)?;
                    if let ir::Stmt::Response(_, metadata) = &stmt.kind {
                        budget.insert(&mut p.defined, metadata.command)?;
                    }
                    if let Some(trace) = &mut p.trace {
                        budget.insert(&mut trace.started, c.binding)?;
                    }
                }
            }
            ir::Stmt::Fork { branches, join, .. } => {
                if let Some(r) = recorder.as_deref_mut()
                    && viewpoints::precise_resources(model)
                    && branches
                        .iter()
                        .any(|c| viewpoints::infeasible_call(model, c).unwrap_or(true))
                {
                    incoming = r.infeasible_fork(
                        incoming,
                        branches,
                        join,
                        model,
                        budget,
                        &BTreeSet::new(),
                    )?;
                    continue;
                }
                if let Some(r) = recorder.as_deref_mut() {
                    let zero = r.precise_actor_fork(&incoming, branches, join, model, budget)?;
                    if !zero.is_empty() {
                        incoming =
                            r.infeasible_fork(incoming, branches, join, model, budget, &zero)?;
                        continue;
                    }
                }
                if model.has_requires()
                    || model
                        .capabilities
                        .iter()
                        .any(|c| c.history_dependent_result())
                {
                    let written: BTreeSet<_> = branches
                        .iter()
                        .flat_map(|c| {
                            let cap = &model.capabilities[c.capability.0];
                            cap.start_writes
                                .iter()
                                .chain(cap.finish_writes())
                                .map(|w| w.variable)
                                .chain(cap.kernel_writes.iter().copied())
                        })
                        .collect();
                    for p in &mut incoming {
                        budget.copy(written.len())?;
                        for v in &written {
                            p.store.insert(*v, Provenance::Unknown);
                        }
                    }
                    for c in branches {
                        incoming = call_paths(c, incoming, effects, model, budget)?;
                        // A sibling's write is concurrent, not a serial writer
                        // for the next sibling's requires (ADR 0026 裁定 U1).
                        for p in &mut incoming {
                            budget.copy(written.len())?;
                            for v in &written {
                                p.store.insert(*v, Provenance::Unknown);
                            }
                        }
                    }
                    for p in &mut incoming {
                        for v in &written {
                            p.store.insert(*v, Provenance::Unknown);
                        }
                        if recorder.is_none()
                            && let ir::Join::Any { matches, .. } = join
                        {
                            p.guards.push(
                                if matches
                                    .iter()
                                    .any(|x| unknown_atomic_result(&ir::Cond::Eq(*x), model))
                                {
                                    Guard::Symbolic(Symbolic::Unknown, true)
                                } else {
                                    Guard::Any(matches)
                                },
                            );
                        }
                    }
                    if let Some(r) = recorder.as_deref_mut() {
                        r.join(&mut incoming, branches, join, model, budget)?;
                    }
                    continue;
                }
                for c in branches {
                    record_effect(c, &incoming, effects, model, budget)?;
                }
                for p in &mut incoming {
                    for c in branches {
                        budget.insert(&mut p.defined, c.binding)?;
                        if let Some(trace) = &mut p.trace {
                            budget.insert(&mut trace.started, c.binding)?;
                        }
                    }
                }
                if let Some(r) = recorder.as_deref_mut() {
                    r.join(&mut incoming, branches, join, model, budget)?;
                    continue;
                }
                if let ir::Join::Any { matches, .. } = join {
                    for p in &mut incoming {
                        budget.copy(1)?;
                        p.guards.push(Guard::Any(matches));
                    }
                }
            }
            ir::Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                if recorder.is_none()
                    && has_state(condition)
                    && then_body.is_empty()
                    && else_body.is_empty()
                {
                    continue;
                }
                let mut yes = Vec::new();
                let mut no = Vec::new();
                for p in incoming {
                    budget.path()?;
                    budget.copy(p.defined.len())?;
                    budget.copy(p.guards.len())?;
                    budget.copy(2)?;
                    if let Some(trace) = &p.trace {
                        budget.copy(trace.cells()?)?;
                        budget.copy(p.store.len())?;
                        viewpoints::guard_storage(&p, budget)?;
                    }
                    let mut y = p.clone();
                    let rule_guard = if reads_rule(condition, model) {
                        Some(process_rule_symbolic(condition, &p, model, budget)?)
                    } else {
                        None
                    };
                    if let Some(g) = &rule_guard {
                        record_rule_constraint(condition, g, true, &mut y, model, budget)?;
                    }
                    y.guards.push(if let Some(g) = &rule_guard {
                        Guard::Symbolic(g.clone(), true)
                    } else if unknown_atomic_result(condition, model) {
                        Guard::Symbolic(Symbolic::Unknown, true)
                    } else {
                        Guard::If(condition, true)
                    });
                    yes.push(y);
                    let mut n = p;
                    if let Some(g) = &rule_guard {
                        record_rule_constraint(condition, g, false, &mut n, model, budget)?;
                    }
                    n.guards.push(if let Some(g) = &rule_guard {
                        Guard::Symbolic(g.clone(), false)
                    } else if unknown_atomic_result(condition, model) {
                        Guard::Symbolic(Symbolic::Unknown, false)
                    } else {
                        Guard::If(condition, false)
                    });
                    no.push(n);
                }
                incoming = paths(
                    then_body,
                    yes,
                    effects,
                    model,
                    budget,
                    recorder.as_deref_mut(),
                    depth + 1,
                )?;
                incoming.extend(paths(
                    else_body,
                    no,
                    effects,
                    model,
                    budget,
                    recorder.as_deref_mut(),
                    depth + 1,
                )?);
            }
            ir::Stmt::End(end) => {
                if let Some(r) = recorder.as_deref_mut() {
                    for p in incoming.drain(..) {
                        r.sink(Some(*end), p, budget)?;
                    }
                } else {
                    incoming.clear();
                }
            }
        }
    }
    Ok(incoming)
}

fn record_effect<'a>(
    c: &ir::Call,
    incoming: &[Path<'a>],
    effects: &mut BTreeMap<ir::BindingId, Vec<Path<'a>>>,
    model: &ir::Model,
    budget: &mut Budget,
) -> Result<()> {
    if model.capabilities[c.capability.0].commits {
        for p in incoming {
            if let Some(trace) = &p.trace {
                budget.copy(
                    trace
                        .cells()?
                        .checked_add(p.store.len())
                        .ok_or_else(invalid)?,
                )?;
                viewpoints::guard_storage(p, budget)?;
            }
            budget.copy(p.defined.len())?;
            budget.copy(p.guards.len())?;
            budget.copy(1)?;
        }
        effects
            .entry(c.binding)
            .or_default()
            .extend_from_slice(incoming);
    }
    Ok(())
}

fn eval(cond: &ir::Cond, values: &Values, budget: &mut Budget) -> Result<bool> {
    budget.work()?;
    match cond {
        ir::Cond::Contains { .. } | ir::Cond::StateEq(_) | ir::Cond::IntCompare { .. } => {
            Err(invalid())
        }
        ir::Cond::Eq(m) => Ok(*values.get(&m.binding).ok_or_else(invalid)? == m.value),
        ir::Cond::And(parts) => {
            for p in parts {
                if !eval(p, values, budget)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        ir::Cond::Or(parts) => {
            for p in parts {
                if eval(p, values, budget)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}
fn satisfied(path: &Path, values: &Values, budget: &mut Budget) -> Result<bool> {
    for guard in &path.guards {
        let truth = match guard {
            Guard::Symbolic(c, truth) => c.eval(values, budget)? == *truth,
            Guard::Domain(id, domain) => domain.contains(values.get(id).ok_or_else(invalid)?),
            Guard::If(cond, truth) => eval(cond, values, budget)? == *truth,
            Guard::Any(matches) => {
                let mut found = false;
                for m in *matches {
                    budget.work()?;
                    if *values.get(&m.binding).ok_or_else(invalid)? == m.value {
                        found = true;
                        break;
                    }
                }
                found
            }
        };
        if !truth {
            return Ok(false);
        }
    }
    Ok(true)
}

fn path_support(
    path: &Path,
    model: &ir::Model,
    budget: &mut Budget,
) -> Result<BTreeSet<ir::BindingId>> {
    let mut support = BTreeSet::new();
    for guard in &path.guards {
        match guard {
            Guard::Symbolic(c, _) => c.support(&mut support, budget)?,
            Guard::Domain(id, _) => budget.insert(&mut support, *id)?,
            Guard::If(cond, _) => condition_support(cond, &mut support, model, 0, budget)?,
            Guard::Any(matches) => {
                for m in *matches {
                    budget.insert(&mut support, m.binding)?;
                }
            }
        }
    }
    Ok(support)
}

// Mixed-radix iterator; no Cartesian product allocation or recursive stack
// proportional to the number of bindings. Every visit is charged.
fn assignments(
    vars: &[ir::BindingId],
    fixed: &Values,
    calls: &[&ir::Call],
    model: &ir::Model,
    budget: &mut Budget,
    mut visit: impl FnMut(&Values, &mut Budget) -> Result<bool>,
) -> Result<bool> {
    budget.copy(vars.len())?;
    budget.copy(vars.len())?;
    budget.copy(fixed.len())?;
    let vars: Vec<_> = vars
        .iter()
        .copied()
        .filter(|v| !fixed.contains_key(v))
        .collect();
    let mut indices = vec![0; vars.len()];
    let mut values = fixed.clone();
    for b in &vars {
        budget.copy(1)?;
        values.insert(*b, model.capabilities[calls[b.0].capability.0].results[0]);
    }
    loop {
        budget.work()?;
        if visit(&values, budget)? {
            return Ok(true);
        }
        let mut i = 0;
        loop {
            if i == vars.len() {
                return Ok(false);
            }
            budget.work()?;
            let b = vars[i];
            let cap = &model.capabilities[calls[b.0].capability.0];
            let mut domain = cap.possible_results();
            if let Some(r) = &cap.requires
                && !domain.contains(&r.otherwise)
            {
                domain.push(r.otherwise);
            }
            indices[i] += 1;
            if indices[i] < domain.len() {
                values.insert(b, domain[indices[i]]);
                break;
            }
            indices[i] = 0;
            values.insert(b, domain[0]);
            i += 1;
        }
    }
}

/// Derive the static subset once, after name/type/availability resolution.
/// Invalid graph indices, cycles, nesting, or computational limits fail closed
/// with U-SEM-0; no partial summary is returned. This is not runtime exploration.
pub fn derive(model: &ir::Model) -> Result<(Agency, Vec<diag::Diagnostic>)> {
    let _timer = crate::perf::span("agency.derive");
    crate::actor_table::validate(model).map_err(|_| invalid())?;
    derive_with_limits(model, DEFAULT)
}

struct Group {
    subject: BTreeSet<ir::ActorId>,
    actors: BTreeSet<ir::ActorId>,
    independent: BTreeSet<ir::ActorId>,
    judgements: usize,
}

fn derive_with_limits(
    model: &ir::Model,
    limits: Limits,
) -> Result<(Agency, Vec<diag::Diagnostic>)> {
    derive_with_new_limits(model, limits, [DEFAULT; 3], DEFAULT)
}

fn derive_with_new_limits(
    model: &ir::Model,
    limits: Limits,
    viewpoint_limits: [Limits; 3],
    effects_limits: Limits,
) -> Result<(Agency, Vec<diag::Diagnostic>)> {
    let mut budget = Budget {
        limits,
        work: 0,
        cells: 0,
        contexts: 0,
        paths: 0,
    };
    budget.path()?;
    let n = model.process.bindings.len();
    budget.steps(model.actors.len())?;
    let sources_count = model.actors.iter().filter(|a| a.source.is_some()).count();
    budget.cells(n.checked_mul(sources_count).ok_or_else(invalid)?)?;
    // Bound the per-binding graph tables before allocating them, separately
    // from their variable-sized memo cells below.
    budget.cells(n.checked_mul(8).ok_or_else(invalid)?)?;
    let mut indexed = vec![None; n];
    collect(&model.process.body, &mut indexed, model, 0, &mut budget)?;
    let calls: Vec<_> = indexed
        .into_iter()
        .collect::<Option<_>>()
        .ok_or_else(invalid)?;
    let mut dependents = vec![Vec::new(); n];
    let mut degree = vec![0usize; n];
    let extra = read_graph::read_parents(model, &calls)?;
    for c in &calls {
        budget.work()?;
        let cap = model.capabilities.get(c.capability.0).ok_or_else(invalid)?;
        let binding = &model.process.bindings[c.binding.0];
        let ty = model.types.get(cap.return_type.0).ok_or_else(invalid)?;
        if cap.actor.0 >= model.actors.len()
            || cap.results.is_empty()
            || binding.ty != cap.return_type
            || cap.params.len() != c.args.len()
        {
            return Err(invalid());
        }
        for value in &cap.possible_results() {
            budget.work()?;
            if value.0 >= ty.members.len() {
                return Err(invalid());
            }
        }
        for source in &cap.reads {
            budget.work()?;
            if model
                .actors
                .get(source.0)
                .is_none_or(|a| a.source.is_none())
            {
                return Err(invalid());
            }
        }
        for (arg, param) in c.args.iter().zip(&cap.params) {
            budget.work()?;
            if *arg == ir::CURRENT_CASE || param.ty == ir::CASE_ID_TYPE {
                if *arg != ir::CURRENT_CASE || param.ty != ir::CASE_ID_TYPE {
                    return Err(invalid());
                }
                continue;
            }
            let b = model.process.bindings.get(arg.0).ok_or_else(invalid)?;
            if b.ty != param.ty {
                return Err(invalid());
            }
            budget.cells(1)?;
            dependents[arg.0].push(c.binding);
            degree[c.binding.0] = degree[c.binding.0].checked_add(1).ok_or_else(invalid)?;
        }
        for arg in &extra[c.binding.0] {
            budget.copy(1)?;
            dependents[arg.0].push(c.binding);
            degree[c.binding.0] = degree[c.binding.0].checked_add(1).ok_or_else(invalid)?;
        }
    }
    let mut source_sets = vec![BTreeSet::new(); n];
    let mut ancestors = vec![BTreeSet::new(); n];
    let mut subjects = vec![BTreeSet::new(); n];
    let mut ready: VecDeque<_> = degree
        .iter()
        .enumerate()
        .filter_map(|(i, d)| (*d == 0).then_some(i))
        .collect();
    let mut visited = 0;
    while let Some(i) = ready.pop_front() {
        budget.work()?;
        visited += 1;
        let c = calls[i];
        for arg in c.args.iter().chain(&extra[i]) {
            if *arg == ir::CURRENT_CASE {
                continue;
            }
            budget.work()?;
            for s in &source_sets[arg.0] {
                budget.insert(&mut subjects[i], *s)?;
            }
            budget.copy(ancestors[arg.0].len())?;
            let inherited = ancestors[arg.0].clone();
            for a in inherited {
                budget.insert(&mut ancestors[i], a)?;
            }
            budget.insert(&mut ancestors[i], *arg)?;
        }
        budget.copy(subjects[i].len())?;
        source_sets[i] = subjects[i].clone();
        for source in &model.capabilities[c.capability.0].reads {
            budget.insert(&mut source_sets[i], *source)?;
        }
        for next in &dependents[i] {
            budget.work()?;
            degree[next.0] -= 1;
            if degree[next.0] == 0 {
                ready.push_back(next.0);
            }
        }
    }
    if visited != n {
        return Err(invalid());
    }
    let names = |set: &BTreeSet<ir::ActorId>| {
        set.iter()
            .map(|s| model.actors[s.0].name.clone())
            .collect::<Vec<_>>()
    };
    let cap_name = |c: &ir::Call| {
        let cap = &model.capabilities[c.capability.0];
        format!("{}.{}", model.actors[cap.actor.0].name, cap.name)
    };
    let mut agency = Agency::empty();
    if !model.process.degradations.is_empty() {
        agency.degradation = Some(degradation::derive(model, 1_000_000));
    }
    if block_has_state_guard(&model.process.body, model) {
        agency.rules = "agency-p1-state-1".into();
        agency.unchecked.push("state_guards".into());
    }
    if model.has_requires()
        || model
            .capabilities
            .iter()
            .any(|c| c.history_dependent_result())
        || model.has_wait()
        || calls.iter().any(|c| {
            let cap = &model.capabilities[c.capability.0];
            !cap.start_writes.is_empty() || cap.cancel == ir::Cancel::Immediate
        })
    {
        agency.rules = "agency-p1-requires-1".into();
    }
    fn used_rules(b: &ir::Block, m: &ir::Model) -> bool {
        b.iter().any(|s| match &s.kind {
            ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => m.capabilities[c.capability.0]
                .requires
                .as_ref()
                .is_some_and(|r| reads_rule(&r.condition, m)),
            ir::Stmt::Fork { branches, .. } => branches.iter().any(|c| {
                m.capabilities[c.capability.0]
                    .requires
                    .as_ref()
                    .is_some_and(|r| reads_rule(&r.condition, m))
            }),
            ir::Stmt::If {
                condition,
                then_body,
                else_body,
            } => reads_rule(condition, m) || used_rules(then_body, m) || used_rules(else_body, m),
            _ => false,
        })
    }
    if used_rules(&model.process.body, model) {
        agency.rules = "agency-p1-rule-1".into();
    }
    budget.cells(model.actors.len().checked_mul(2).ok_or_else(invalid)?)?;
    let mut direct = vec![BTreeSet::new(); model.actors.len()];
    let mut indirect = direct.clone();
    for c in &calls {
        let cap = &model.capabilities[c.capability.0];
        for source in &cap.reads {
            budget.insert(&mut direct[cap.actor.0], *source)?;
        }
        for source in &subjects[c.binding.0] {
            budget.insert(&mut indirect[cap.actor.0], *source)?;
        }
    }
    agency.origin = model
        .actors
        .iter()
        .enumerate()
        .map(|(i, a)| Origin {
            actor: a.name.clone(),
            direct: names(&direct[i]),
            indirect: names(&indirect[i]),
        })
        .collect();
    let mut effect_paths = BTreeMap::new();
    // Models without effects need no path expansion.
    if calls
        .iter()
        .any(|c| model.capabilities[c.capability.0].commits)
    {
        paths(
            &model.process.body,
            vec![Path::default()],
            &mut effect_paths,
            model,
            &mut budget,
            None,
            0,
        )?;
    }
    let mut all_j: BTreeSet<ir::BindingId> = BTreeSet::new();
    let mut warnings = Vec::new();
    for effect in calls
        .iter()
        .filter(|c| model.capabilities[c.capability.0].commits)
    {
        if model.capabilities[effect.capability.0].history_dependent_result() {
            if !agency.unchecked.iter().any(|s| s == "state_guards") {
                agency.unchecked.push("state_guards".into());
            }
            continue;
        }
        let ps = effect_paths
            .get(&effect.binding)
            .map(Vec::as_slice)
            .unwrap_or_default();
        if ps.iter().any(|p| {
            p.guards.iter().any(|g| {
                matches!(g, Guard::If(c, _) if has_state(c))
                    || matches!(g, Guard::Symbolic(c, _) if c.unknown())
            })
        }) {
            if !agency.unchecked.iter().any(|x| x == "state_guards") {
                agency.unchecked.push("state_guards".into());
            }
            continue;
        }
        let mut support = BTreeSet::new();
        let mut reachable_paths = Vec::new();
        let mut path_supports = Vec::new();
        for p in ps {
            let local = path_support(p, model, &mut budget)?;
            if !local.is_subset(&p.defined) {
                return Err(invalid());
            }
            let vars: Vec<_> = local.iter().copied().collect();
            if !assignments(
                &vars,
                &Values::new(),
                &calls,
                model,
                &mut budget,
                |values, b| satisfied(p, values, b),
            )? {
                continue;
            }
            // Only satisfiable paths contribute guard-based J candidates and
            // context variables. Static argument ancestry is kept separately.
            for b in &local {
                budget.insert(&mut support, *b)?;
            }
            budget.cells(1)?;
            reachable_paths.push(p);
            path_supports.push(local);
        }
        let mut candidates = BTreeSet::new();
        for b in ancestors[effect.binding.0].union(&support) {
            budget.work()?;
            if calls[b.0].args.iter().any(|a| *a != ir::CURRENT_CASE) || !extra[b.0].is_empty() {
                budget.insert(&mut candidates, *b)?;
                budget.insert(&mut all_j, *b)?;
            }
        }
        let mut row = Refusal {
            effect: cap_name(effect),
            call: model.process.bindings[effect.binding.0].name.clone(),
            reachable: !reachable_paths.is_empty(),
            judgements: Vec::new(),
        };
        if row.reachable {
            for j in candidates {
                let mut contexts = BTreeSet::<Values>::new();
                for p in &reachable_paths {
                    budget.copy(support.len())?;
                    let vars: Vec<_> = support.intersection(&p.defined).copied().collect();
                    assignments(
                        &vars,
                        &Values::new(),
                        &calls,
                        model,
                        &mut budget,
                        |values, b| {
                            if satisfied(p, values, b)? {
                                b.copy(values.len())?;
                                let c: Values = values
                                    .iter()
                                    .filter(|(id, _)| **id != j)
                                    .map(|(id, v)| (*id, *v))
                                    .collect();
                                if !contexts.contains(&c) {
                                    Budget::add(&mut b.contexts, 1, b.limits.contexts)?;
                                    contexts.insert(c);
                                }
                            }
                            Ok(false)
                        },
                    )?;
                }
                let mut stopping = 0;
                let mut bypasses = 0;
                for context in &contexts {
                    let mut may = false;
                    let mut cannot = false;
                    for value in &model.capabilities[calls[j.0].capability.0].finish_results() {
                        let mut reaches = false;
                        for (p, local) in reachable_paths.iter().zip(&path_supports) {
                            budget.work()?;
                            // Missing bindings on an alternative path are not
                            // ghost constraints. New bindings are existential in
                            // may_reach, hence universal for the stopping claim.
                            budget.copy(context.len())?;
                            let mut fixed: Values = context
                                .iter()
                                .filter(|(id, _)| p.defined.contains(id))
                                .map(|(id, v)| (*id, *v))
                                .collect();
                            budget.copy(1)?;
                            fixed.insert(j, *value);
                            budget.copy(local.len())?;
                            let vars: Vec<_> = local.iter().copied().collect();
                            if assignments(
                                &vars,
                                &fixed,
                                &calls,
                                model,
                                &mut budget,
                                |values, b| satisfied(p, values, b),
                            )? {
                                reaches = true;
                                break;
                            }
                        }
                        may |= reaches;
                        cannot |= !reaches;
                    }
                    if may {
                        if cannot {
                            stopping += 1;
                        } else {
                            bypasses += 1;
                        }
                    }
                }
                let stops = if stopping == 0 {
                    Stops::None
                } else if bypasses == 0 {
                    Stops::AllPaths
                } else {
                    Stops::Conditional
                };
                if stops != Stops::AllPaths {
                    warnings.push(diag::Diagnostic {
                        code: if stops == Stops::None {
                            diag::W_AGY_0
                        } else {
                            diag::W_AGY_1
                        }
                        .into(),
                        severity: diag::Severity::Warning,
                        message: format!(
                            "{} judgement {} {} (bypasses: {bypasses})",
                            row.effect,
                            model.process.bindings[j.0].name,
                            if stops == Stops::None {
                                "does not stop the effect"
                            } else {
                                "has a bypass"
                            }
                        ),
                        span: Some(calls[j.0].span),
                    });
                }
                row.judgements.push(Judgement::Process(ProcessJudgement {
                    call: model.process.bindings[j.0].name.clone(),
                    capability: cap_name(calls[j.0]),
                    stops,
                    bypasses,
                }));
            }
        }
        for (id, actor) in model.actors.iter().enumerate() {
            if let Some(rule) = &actor.metadata.rule {
                let reads: Vec<_> = rule
                    .values
                    .iter()
                    .filter(|v| ps.iter().any(|p| p.rule_sources.contains(&v.variable)))
                    .map(|v| {
                        format!(
                            "{}.{}",
                            actor.name, model.state_variables[v.variable.0].name
                        )
                    })
                    .collect();
                if !reads.is_empty() {
                    row.judgements.push(Judgement::Rule(RuleJudgement {
                        kind: RuleKind::Rule,
                        actor: model.actors[id].name.clone(),
                        version: rule.effective_version.clone(),
                        reads,
                    }));
                }
            }
        }
        // ADR 0044 ruling M1: omit only a derived-empty row whose every path
        // reaches the effect under a requires that reads a Rule and that the
        // selected Rule values make a tautology. A Rule read only by a path
        // `if`, or a requires that ignores the Rule, keeps the row.
        if !row.judgements.is_empty()
            || ps.is_empty()
            || !ps.iter().all(|p| p.requires_fixed_by_rule)
        {
            agency.refusal.push(row);
        }
    }
    let mut groups: Vec<Group> = Vec::new();
    for j in all_j {
        let cap = &model.capabilities[calls[j.0].capability.0];
        let mut independent = BTreeSet::new();
        for s in &cap.reads {
            budget.work()?;
            if !subjects[j.0].contains(s) {
                budget.insert(&mut independent, *s)?;
            }
        }
        agency.lineage.push(Lineage {
            call: model.process.bindings[j.0].name.clone(),
            capability: cap_name(calls[j.0]),
            subject_sources: names(&subjects[j.0]),
            independent_sources: names(&independent),
        });
        let mut matching = None;
        for (index, group) in groups.iter().enumerate() {
            budget.work()?;
            budget.steps(subjects[j.0].len())?;
            if group.subject == subjects[j.0] {
                matching = Some(index);
                break;
            }
        }
        if let Some(index) = matching {
            let group = &mut groups[index];
            budget.insert(&mut group.actors, cap.actor)?;
            for s in independent {
                budget.insert(&mut group.independent, s)?;
            }
            group.judgements += 1;
        } else {
            budget.copy(subjects[j.0].len())?;
            budget.copy(1)?;
            groups.push(Group {
                subject: subjects[j.0].clone(),
                actors: BTreeSet::from([cap.actor]),
                independent,
                judgements: 1,
            });
        }
    }
    agency.lineage_groups = groups
        .into_iter()
        .map(|group| LineageGroup {
            subject_sources: names(&group.subject),
            judgements: group.judgements,
            actors: group.actors.len(),
            independent_sources: names(&group.independent),
        })
        .collect();
    if calls
        .iter()
        .any(|c| model.capabilities[c.capability.0].performance.is_some())
    {
        // The representative given/write contract is sufficient for structural
        // provenance, not for per-profile basis/correlation suppression.
        agency.unchecked.extend(
            [
                "performance_control",
                "common_cause(performance_control)",
                "model_uncertainties(performance_control)",
            ]
            .map(String::from),
        );
    } else {
        common_cause::derive(
            model,
            &calls,
            &ancestors,
            &subjects,
            &mut agency,
            &mut warnings,
        )?;
    }
    if extra.iter().any(|p| !p.is_empty()) {
        agency.rules = "agency-p1-reads-1".into();
    }
    let (viewpoints, new_warnings) =
        viewpoints::with_limits_and_warnings(model, &agency, viewpoint_limits, effects_limits);
    if matches!(viewpoints.effects, Some(effects::Effects::Derived { .. })) {
        agency.unchecked.retain(|word| word != "executors");
    }
    warnings.extend(new_warnings);
    if let viewpoints::Retention::Derived { capabilities, .. } = &viewpoints.retention {
        for id in &model.fallback_capabilities {
            if matches!(
                capabilities.get(id.0),
                Some(viewpoints::Exercise::Derived {
                    normal: viewpoints::Count { min: 0, max: 0 },
                    ..
                })
            ) {
                let cap = &model.capabilities[id.0];
                warnings.push(diag::Diagnostic { code:diag::W_AGY_3.into(),severity:diag::Severity::Warning,
                    message:format!("{}.{} は縮退のために保たれるが、通常経路での行使は 0 回/件。保持の根拠は unknown、依存する縮退経路はモデル外",model.actors[cap.actor.0].name,cap.name),span:Some(cap.span) });
            }
        }
    }
    agency.viewpoints = Some(viewpoints);
    Ok((agency, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> ir::Model {
        let source = include_str!("../../../fixtures/agency-origin/model.relca");
        crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap()
    }

    #[test]
    fn injected_limits_accept_k_and_reject_k_plus_one_units_of_required_work() {
        let model = model();
        let expected = derive(&model).unwrap();
        // Find each exact boundary without assuming an incidental work-counter
        // implementation. All other limits remain fixed at DEFAULT.
        for field in ["depth", "paths", "contexts", "cells", "work"] {
            let set = |limit| {
                let mut limits = DEFAULT;
                match field {
                    "depth" => limits.depth = limit,
                    "paths" => limits.paths = limit,
                    "contexts" => limits.contexts = limit,
                    "cells" => limits.cells = limit,
                    "work" => limits.work = limit,
                    _ => unreachable!(),
                }
                limits
            };
            let mut low = 0;
            let mut high = match field {
                "depth" => DEFAULT.depth,
                "paths" => DEFAULT.paths,
                "contexts" => DEFAULT.contexts,
                "cells" => DEFAULT.cells,
                _ => DEFAULT.work,
            };
            while low < high {
                let mid = low + (high - low) / 2;
                if derive_with_limits(&model, set(mid)).is_ok() {
                    high = mid;
                } else {
                    low = mid + 1;
                }
            }
            assert!(low > 0, "{field} must be charged");
            assert_eq!(
                derive_with_limits(&model, set(low)).unwrap(),
                expected,
                "{field}"
            );
            let error = derive_with_limits(&model, set(low - 1)).unwrap_err();
            assert_eq!(error.code, diag::U_SEM_0, "{field}");
            assert_eq!(error.severity, diag::Severity::Error);
        }
    }

    #[test]
    fn counters_reject_overflow_instead_of_wrapping() {
        let mut counter = usize::MAX;
        assert!(Budget::add(&mut counter, 1, usize::MAX).is_err());
    }

    #[test]
    fn guard_after_end_does_not_supply_judgement_or_make_refusal_reachable() {
        let mut model = model();
        let ir::Stmt::Call(mut effect) = model.process.body[3].kind.clone() else {
            panic!()
        };
        model.capabilities[effect.capability.0].params.clear();
        effect.args.clear();
        let span = effect.span;
        // Keep both producer calls but put the no-argument effect inside an
        // enclosing guard, after control has already ended.
        model.process.body.truncate(2);
        model.process.body.push(ir::Statement {
            span,
            kind: ir::Stmt::End(ir::EndId(0)),
        });
        model.process.body.push(ir::Statement {
            span,
            kind: ir::Stmt::If {
                condition: ir::Cond::Eq(ir::ResultMatch {
                    binding: ir::BindingId(1),
                    value: ir::ValueId(0),
                }),
                then_body: vec![
                    ir::Statement {
                        span,
                        kind: ir::Stmt::End(ir::EndId(0)),
                    },
                    ir::Statement {
                        span,
                        kind: ir::Stmt::Call(effect),
                    },
                ],
                else_body: Vec::new(),
            },
        });
        let (agency, warnings) = derive(&model).unwrap();
        assert!(agency.lineage.is_empty());
        assert!(!agency.refusal[0].reachable);
        assert!(agency.refusal[0].judgements.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn invalid_indices_cycles_and_deep_hand_ir_are_errors_not_panics() {
        let base = model();
        let mut bad = base.clone();
        let bindings = bad.process.bindings.len();
        let ir::Stmt::Call(call) = &mut bad.process.body[1].kind else {
            panic!()
        };
        // Out of range but not the CurrentCase sentinel (usize::MAX).
        call.args[0] = ir::BindingId(usize::MAX - 1);
        assert_ne!(call.args[0], ir::CURRENT_CASE);
        assert!(call.args[0].0 >= bindings);
        assert_eq!(derive(&bad).unwrap_err().code, diag::U_SEM_0);

        let mut bad = base.clone();
        let ir::Stmt::Call(call) = &mut bad.process.body[1].kind else {
            panic!()
        };
        call.capability = ir::CapabilityId(usize::MAX);
        assert_eq!(derive(&bad).unwrap_err().code, diag::U_SEM_0);

        let mut bad = base.clone();
        let ir::Stmt::Call(call) = &mut bad.process.body[1].kind else {
            panic!()
        };
        let cap = &mut bad.capabilities[call.capability.0];
        cap.params[0].ty = cap.return_type;
        call.args[0] = call.binding;
        assert_eq!(derive(&bad).unwrap_err().code, diag::U_SEM_0);

        let mut bad = base;
        for _ in 0..=DEFAULT.depth {
            bad.process.body = vec![ir::Statement {
                span: bad.process.span,
                kind: ir::Stmt::If {
                    condition: ir::Cond::Eq(ir::ResultMatch {
                        binding: ir::BindingId(0),
                        value: ir::ValueId(0),
                    }),
                    then_body: bad.process.body,
                    else_body: Vec::new(),
                },
            }];
        }
        assert_eq!(derive(&bad).unwrap_err().code, diag::U_SEM_0);
    }
}
#[cfg(test)]
mod response_close_tests;
