//! I03 basic finite-IR same-question gate. Unsupported families fail closed;
//! this is not an IR validity certificate or an analysis result.
mod analysis;
#[cfg(test)]
mod extension_tests;
pub(super) mod names;
mod obligations;
use super::input::{FailureKind, InputError, Target};
use crate::{diag, ir};
use names::Names;
use serde_json::{Value, json};
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, InputError>;
fn canonical_names(rows: impl IntoIterator<Item = Result<String>>) -> Result<Vec<String>> {
    let mut rows = rows.into_iter().collect::<Result<Vec<_>>>()?;
    rows.sort();
    Ok(rows)
}
fn unsupported(detail: &str) -> InputError {
    InputError::new(
        FailureKind::FeatureUnsupported,
        format!("contract projection: {detail}"),
    )
}
fn named(rows: impl IntoIterator<Item = Result<(String, Value)>>) -> Result<Value> {
    let mut map = BTreeMap::new();
    for row in rows {
        let (name, value) = row?;
        if name.is_empty() || map.insert(name, value).is_some() {
            return Err(unsupported("empty or duplicate declaration name"));
        }
    }
    Ok(json!(map))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contract {
    value: Value,
}
impl Contract {
    /// Values permitted to vary (kernel tables, latent initial values, operation
    /// duration, finite traffic/horizon, computational bounds) are not contracts.
    /// Positive-mass result support is not mistaken for a return-type change.
    pub fn from_model(m: &ir::Model) -> Result<Self> {
        supported(m)?;
        let n = Names(m);
        let process = n.block(&m.process.body, 0)?;
        let types = named(m.types.iter().map(|t| {
            let mut members = t.members.clone();
            members.sort();
            Ok((
                t.name.clone(),
                if t.name == "CaseId" {
                    json!({"builtin":"CaseId"})
                } else {
                     json!({"members":members,"record":t.record.as_ref().map(|r|named(r.fields.iter().map(|f|Ok((f.name.clone(),json!(n.ty(f.ty)?)))))).transpose()?})
                },
            ))
        }))?;
        let state = named(m.state_variables.iter().enumerate().map(|(i,v)| {
            let id = ir::StateVarId(i);
            let initial = if v.kind.is_latent() {
                if matches!(v.initial,ir::InitialValue::Nondet) {json!(["nondet"])} else {Value::Null}
            } else {
                match v.initial {
                    ir::InitialValue::Fixed(x) => json!(["fixed",n.state_value(id,x)?]),
                    ir::InitialValue::Nondet => json!(["nondet"]),
                    _ => return Err(unsupported("observed initial distribution")),
                }
            };
            let ty=if let Some(max)=v.integer_max {json!({"integer_max":max})} else if v.case_set_size.is_some() {json!({"set":"CaseId"})} else {json!(n.ty(v.base)?)};
            Ok((n.state(id)?,json!({"type":ty,"optional":v.optional,"initial":initial,"last_write_wins":v.last_write_wins})))
        }))?;
        // Process references have been checked above; metadata does not execute.
        let actors = named(crate::actor_table::project(m).into_iter().enumerate().map(
            |(i, a)| {
                let name = a.name.clone();
                let mut value = json!(a);
                // Grain is a display label, never an operational contract or a
                // command to infer a different stochastic model (AGENTS.md).
                value.as_object_mut().unwrap().remove("grain");
                for field in [
                    "capability",
                    "contains",
                    "overlaps",
                    "latent",
                    "state",
                    "executors",
                ] {
                    if let Some(rows) = value[field]["value"].as_array_mut() {
                        rows.sort_by_key(Value::to_string);
                    }
                }
                value["source_description"] = json!(m.actors[i].metadata.source_description);
                Ok((name, value))
            },
        ))?;
        let capabilities = named(m.capabilities.iter().enumerate().map(|(i,c)| {
            let requires = match &c.requires {
                None => Value::Null,
                Some(r) => json!({"condition":n.condition(&r.condition,0)?,"otherwise":n.value(c.return_type,r.otherwise)?}),
            };
            let results = if c.kernel.is_some() || c.performance.is_some() { Value::Null } else {
                json!(c.results.iter().map(|v| n.value(c.return_type,*v)).collect::<Result<Vec<_>>>()?)
            };
            let cancel = match c.cancel {
                ir::Cancel::Immediate => json!(["immediate"]), ir::Cancel::Unspecified => json!(["unspecified"]),
                ir::Cancel::ForbiddenAfterStart => json!(["forbidden_after_start"]),
                ir::Cancel::Cooperative { ack_within } => json!(["cooperative",ack_within]),
            };
            let uses = named(c.uses.iter().map(|u| Ok((n.resource(u.resource)?,json!(u.amount)))))?;
            let atomic=c.atomic.as_ref().map(|a|->Result<Value>{Ok(json!({"condition":n.condition(&a.condition,0)?,"commit_when":a.commit_when,
                "commit":n.value(c.return_type,a.commit)?,"reject":n.value(c.return_type,a.reject)?,"writes":n.writes(&a.writes)?}))}).transpose()?;
            let version=c.version_check.as_ref().map(|v|->Result<Value>{Ok(json!({"variables":canonical_names(v.variables.iter().map(|v|n.state(*v)))?,
                "reject":n.value(c.return_type,v.reject)?,"resolve":v.resolve.map(|r|match r {ir::ResolutionTie::Nondet=>"nondet",ir::ResolutionTie::CaseId=>"case_id"})}))}).transpose()?;
            Ok((n.capability(ir::CapabilityId(i))?,json!({"return_type":n.ty(c.return_type)?,"requires":requires,
                "probabilistic":c.kernel.is_some() || c.performance.is_some(),"results":results,"commits":c.commits,"cancel":cancel,
                "qualified_executors":c.qualified_executors.as_deref().map(|ps|n.people(ps)).transpose()?,
                "performance":c.performance.as_ref().map(|p|m.performances.get(p.profile).map(|p|p.name.clone()).ok_or_else(||unsupported("performance reference"))).transpose()?,
                "reads":canonical_names(c.reads.iter().map(|a|n.actor(*a)))?,
                "read_variables":canonical_names(c.read_variables.iter().map(|v|n.state(*v)))?,
                "kernel_writes":canonical_names(c.kernel_writes.iter().map(|v|n.state(*v)))?,
                "start_writes":n.writes(&c.start_writes)?,"finish_writes":n.writes(&c.writes)?,"uses":uses,"atomic":atomic,"version_check":version,
                "parameters":c.params.iter().map(|p|Ok(json!([p.name,n.ty(p.ty)?]))).collect::<Result<Vec<_>>>()?,
                "permission":c.permission.as_ref().map(|ps|n.people(ps)).transpose()?})))
        }))?;
        let queries = named(m.probability_queries.iter().enumerate().map(|(i,q)| {
            let condition = match &q.condition {
                ir::ProbabilityCondition::State(c) => json!(["state",n.condition(c,0)?]),
                ir::ProbabilityCondition::End(e) => json!(["end",n.end(*e)?]),
            };
            let comparison = match q.comparison { ir::Comparison::Le => "<=", ir::Comparison::Ge => ">=" };
            Ok((format!("probability reached #{}",i+1),json!({"condition":condition,"within":q.within,"comparison":comparison,"bound":q.bound})))
        }))?;
        let verify = m
            .verify
            .iter()
            .map(|v| n.verify_name(v.property))
            .collect::<Result<Vec<_>>>()?;
        let policy = match &m.start_policy {
            None => Value::Null,
            Some(p) => {
                let classes = p
                    .classes
                    .as_ref()
                    .map(|c| -> Result<Value> {
                        Ok(json!({
                    "policy":canonical_names(c.policy.iter().map(|a|n.actor(*a)))?,
                    "environment":canonical_names(c.environment.iter().map(|a|n.actor(*a)))?,
                    "party":canonical_names(c.party.iter().map(|a|n.actor(*a)))?}))
                    })
                    .transpose()?;
                json!({"priority":p.priority.map(names::queue_tie),"observable":p.observable,"call_order":p.call_order,"assign_first_idle":p.assign_first_idle,"classes":classes,
                    "executor_order":named(p.executor_order.iter().enumerate().map(|(i,ps)|Ok((n.actor(ir::ActorId(i))?,json!(ps.as_ref().map(|ps|ps.iter().map(|p|n.person(*p)).collect::<Result<Vec<_>>>()).transpose()?)))))?})
            }
        };
        Ok(Self {
            value: json!({"model_name":m.name,"language":m.version,"types":types,"state":state,"actors":actors,"capabilities":capabilities,
            "process":{"name":m.process.name,"bindings":named(m.process.bindings.iter().filter(|b| !b.name.starts_with("@response:")).map(|b|Ok((b.name.clone(),json!(n.ty(b.ty)?)))))?,"body":process},
            "queries":queries,"verify":verify,"policy":policy,"cut":n.cut()?,
            "resources":named(m.resources.iter().map(|r|Ok((r.name.clone(),json!(r.capacity)))))?,
            "admission":{"queue":n.admission(m.admission_queue.as_ref())?,"concurrent":n.admission(m.concurrent_cases.as_ref())?,"overflow_end":m.overflow_end.map(|e|n.end(e)).transpose()?},
            "simulation":m.simulation.as_ref().map(|s|json!({"replications":s.replications,"seed":s.seed,"confidence":s.confidence})),
            "priority_policy":crate::report::priority::record(m),
             "performances":named(m.performances.iter().map(|p| {
                 let mut rows=p.rows.iter().map(|r| -> Result<Value> {Ok(json!({
                     "executor":r.executor.map(|id|n.person(id)).transpose()?,
                     "lower":p.queued.then_some(r.from),"upper":if p.queued {r.until} else {None},
                     "kernel":m.kernels.get(r.kernel).ok_or_else(||unsupported("performance kernel reference"))?.name
                 }))}).collect::<Result<Vec<_>>>()?;
                 rows.sort_by_key(Value::to_string);
                 Ok((p.name.clone(),json!({"load":p.queued,"rows":rows})))
             }))?,
              "quality":n.quality()?,"metrics":n.metrics()?,"comparison":n.comparison()?,"contracts":n.contracts()?,"authorities":n.authorities()?,"responsibility":n.responsibilities()?,
            "people":named(m.people.iter().map(|p|Ok((p.name.clone(),Value::Null))))?}),
        })
    }
    pub fn differences(&self, other: &Self) -> Vec<String> {
        self.value
            .as_object()
            .unwrap()
            .iter()
            .filter(|(key, v)| other.value.get(*key) != Some(*v))
            .map(|(key, _)| format!("/{key}"))
            .collect()
    }
    pub fn validate_targets(&self, targets: &[Target]) -> Result<()> {
        if targets.is_empty() {
            return Err(InputError::new(
                FailureKind::TargetInvalid,
                "no probability targets",
            ));
        }
        for t in targets {
            if t.kind != "probability_query"
                || self.value["queries"].get(&t.id).is_none()
                || !self.value["verify"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(t.id))
            {
                return Err(InputError::new(
                    FailureKind::TargetInvalid,
                    format!("unresolved probability target {:?}", t.id),
                ));
            }
        }
        Ok(())
    }
}

fn supported(m: &ir::Model) -> Result<()> {
    if m.has_merges() {
        return Err(unsupported(
            "merge_feature_combination: merge sensitivity is not connected",
        ));
    }
    if crate::evidence_basis::present(m) {
        return Err(unsupported("evidence_basis_unsupported_command"));
    }
    if m.has_loops() {
        return Err(unsupported(
            "loop_feature_combination: repeat sensitivity is not connected",
        ));
    }
    if !m.process.degradations.is_empty() {
        return Err(unsupported(
            "q6_feature_combination: degradation sensitivity not connected",
        ));
    }
    // A new Model field must be consciously classified here, not silently left
    // out of the gate. Derived reports/assumptions are not declaration inputs.
    let ir::Model {
        performances: _,
        case_priority: _,
        arrival_distribution: _,
        resource_time: _,
        concurrency_resolutions: _,
        fallback_capabilities: _,
        quality: _,
        comparison: _,
        resources: _,
        metrics: _,
        responsibility: _,
        simulation: _,
        arrivals: _,
        arrival_every: _,
        scenario: _,
        horizon: _,
        start_policy: _,
        admission_queue: _,
        concurrent_cases: _,
        overflow_end: _,
        kernels: _,
        initial_distributions: _,
        probability_queries: _,
        state_variables: _,
        people: _,
        authorities: _,
        contracts: _,
        agency: _,
        name: _,
        version: _,
        types: _,
        actors: _,
        capabilities: _,
        process: _,
        ends: _,
        bounds: _,
        verify: _,
        assumptions: _,
        diagnostics: _,
        cut: _,
    } = m;
    for c in &m.capabilities {
        let ir::Capability {
            receipt_query: _,
            qualified_executors: _,
            performance: _,
            version_check: _,
            read_variables: _,
            atomic: _,
            uses: _,
            requires: _,
            start_writes: _,
            kernel: _,
            kernel_reads_latent: _,
            kernel_writes: _,
            writes: _,
            permission: _,
            params: _,
            reads: _,
            commits: _,
            name: _,
            actor: _,
            return_type: _,
            duration: _,
            duration_distribution: _,
            results: _,
            cancel: _,
            span: _,
        } = c;
    }
    for a in &m.actors {
        let ir::Actor {
            metadata,
            executors: _,
            source: _,
            name: _,
            capacity: _,
            span: _,
        } = a;
        let ir::ActorMetadata {
            rule: _,
            instance: _,
            source_description: _,
            boundary: _,
            grain: _,
            contains: _,
            overlaps: _,
            control: _,
            via_model: _,
            capacity_declared: _,
        } = metadata;
    }
    for v in &m.state_variables {
        let ir::StateVariable {
            last_write_wins: _,
            case_set_size: _,
            integer_max: _,
            name: _,
            kind: _,
            base: _,
            optional: _,
            initial: _,
            span: _,
        } = v;
    }
    for q in &m.probability_queries {
        let ir::ProbabilityQuery {
            condition: _,
            within: _,
            comparison: _,
            bound: _,
        } = q;
    }
    for v in &m.verify {
        let ir::Verify {
            property: _,
            span: _,
        } = v;
    }
    if m.resource_time.is_some()
        || !m.concurrency_resolutions.is_empty()
        || !m.fallback_capabilities.is_empty()
    {
        return Err(unsupported(
            "calendars/shared-resolution families not yet projected",
        ));
    }
    if !m.contests().is_empty() {
        return Err(unsupported("contest sensitivity not projected"));
    }
    if m.diagnostics
        .iter()
        .any(|d| d.severity == diag::Severity::Error)
    {
        return Err(unsupported("model contains static errors"));
    }
    crate::actor_table::validate(m).map_err(|e| unsupported(e.message))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lower, syntax};
    const BASE: &str = include_str!("../../tests/data/sensitivity/base.relca");
    fn contract(s: &str) -> Contract {
        Contract::from_model(&lower::lower(&syntax::parse(s).unwrap()).unwrap()).unwrap()
    }
    #[test]
    fn declared_parameter_changes_do_not_change_the_question() {
        let base = contract(BASE);
        for s in [
            include_str!("../../tests/data/sensitivity/high.relca").to_owned(),
            include_str!("../../tests/data/sensitivity/miss.relca").to_owned(),
            BASE.replace("duration fixed 1min", "duration fixed 2min"),
            BASE.replace("clock 4min", "clock 0min"),
        ] {
            assert!(base.differences(&contract(&s)).is_empty());
        }
        let a = contract(include_str!("../../tests/data/sensitivity/zero.relca"));
        let b = contract(include_str!(
            "../../tests/data/sensitivity/zero-two-cases.relca"
        ));
        assert!(a.differences(&b).is_empty());
    }
    #[test]
    fn threshold_cut_control_and_capacity_changes_cannot_pass() {
        let base = contract(BASE);
        for (s, at) in [
            (BASE.replace("<= 0.008", "<= 0.01"), "/queries"),
            (
                BASE.replace(
                    "cut C { include [Check] }",
                    "cut C { question \"changed\"; include [Check] }",
                ),
                "/cut",
            ),
            (BASE.replace("if v == pass", "if v == fail"), "/process"),
            (BASE.replace("capacity 1", "capacity 2"), "/actors"),
        ] {
            assert!(
                base.differences(&contract(&s)).contains(&at.to_owned()),
                "missed {at}"
            );
        }
    }
    #[test]
    fn positive_mass_support_is_not_a_declared_result_domain() {
        let a = contract(include_str!("../../tests/data/sensitivity/p0.relca"));
        for source in [
            include_str!("../../tests/data/sensitivity/phalf.relca"),
            include_str!("../../tests/data/sensitivity/p1.relca"),
        ] {
            assert!(a.differences(&contract(source)).is_empty());
        }
    }
    #[test]
    fn targets_must_be_requested_and_unprojected_features_are_unsupported() {
        let c = contract(BASE);
        c.validate_targets(&[Target {
            kind: "probability_query".into(),
            id: "probability reached #1".into(),
        }])
        .unwrap();
        assert_eq!(
            c.validate_targets(&[Target {
                kind: "probability_query".into(),
                id: "probability reached #99".into()
            }])
            .unwrap_err()
            .kind,
            FailureKind::TargetInvalid
        );
        let source = include_str!("../../tests/data/sensitivity/defer.relca");
        let model = lower::lower(&syntax::parse(source).unwrap()).unwrap();
        let base = Contract::from_model(&model).unwrap();
        assert!(base.value["quality"]["cohorts"].get("Rate").is_some());
        assert_eq!(base.value["metrics"]["actor_rates"]["Work"], 10);
        for (from, to, key) in [
            ("/ arrivals", "/ expected(count(case.Done))", "/quality"),
            ("per_minute 10", "per_minute 11", "/metrics"),
        ] {
            let changed = contract(&source.replace(from, to));
            assert!(base.differences(&changed).contains(&key.to_owned()));
        }
        let mut projected = model.clone();
        projected.responsibility = Some(Default::default());
        let projected = Contract::from_model(&projected).unwrap();
        assert_eq!(projected.value["responsibility"]["duties"], json!([]));
        // P3a connects responsibility, but calendars remain outside this gate.
        let calendar = source.replace("actor Work { capacity 1", "calendar All through 5min on_close continue { [0min,5min) capacity 1 }\n  actor Work { capacity 1; calendar All");
        let unprojected = lower::lower(&syntax::parse(&calendar).unwrap()).unwrap();
        assert!(unprojected.resource_time.is_some());
        let error = Contract::from_model(&unprojected).unwrap_err();
        assert_eq!(error.kind, FailureKind::FeatureUnsupported);
        assert_eq!(error.reason(), "sensitivity_feature_combination");
    }
    #[test]
    fn table_index_changes_do_not_change_named_operands() {
        let source = include_str!("../../tests/data/sensitivity/abort.relca");
        let a = "actor A { capacity 1; capability c() -> T { duration fixed 1min; outcomes kernel K; writes y } }";
        let b = "actor B { capacity 1; capability c() -> T { duration fixed 1min; result nondet { first, second } } }";
        assert_eq!(source.matches(a).count(), 1);
        assert_eq!(source.matches(b).count(), 1);
        let changed = source
            .replace(a, "@A@")
            .replace(b, a)
            .replace("@A@", b)
            .replace(
                "latent case { z:T ~ { second:0.5, first:0.5 } }\n  observed case { y:T = second }",
                "observed case { y:T = second }\n  latent case { z:T ~ { second:0.5, first:0.5 } }",
            );
        let old = lower::lower(&syntax::parse(source).unwrap()).unwrap();
        let new = lower::lower(&syntax::parse(&changed).unwrap()).unwrap();
        assert_ne!(old.actors[0].name, new.actors[0].name);
        assert_ne!(old.state_variables[0].name, new.state_variables[0].name);
        assert!(
            Contract::from_model(&old)
                .unwrap()
                .differences(&Contract::from_model(&new).unwrap())
                .is_empty()
        );
    }
    #[test]
    fn observed_initial_facts_are_not_latent_parameter_changes() {
        let source = include_str!("../../tests/data/sensitivity/zero.relca");
        let old = "observed case { hit: bool = false }";
        assert_eq!(source.matches(old).count(), 1);
        let changed = source.replace(old, "observed case { hit: bool = true }");
        assert!(
            contract(source)
                .differences(&contract(&changed))
                .contains(&"/state".to_owned())
        );
    }
}
