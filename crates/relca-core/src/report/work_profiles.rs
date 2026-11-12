//! Declared eligibility and selector tables, never a measured skill score.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub const RULES: &str = "work-profiles-v1";

/// Unconnected modes refuse before calling an interpreter. Unused profile
/// declarations alone do not reject an otherwise ordinary analysis.
pub(crate) fn refusal(
    src: &str,
    configuration: Option<&str>,
    scenario: Option<&str>,
) -> Option<(Report, ir::Model)> {
    let ast = syntax::parse(src).ok()?;
    if !ast.items.iter().any(|item| matches!(item,ast::Item::Actor(a) if a.items.iter().any(|item|
        matches!(item,ast::ActorItem::Capability(c) if c.items.iter().any(|item|
            matches!(item.value,ast::CapItem::QualifiedExecutors(_)|ast::CapItem::OutcomesPerformance(_))))))) {return None}
    let model = lower::lower_with_selection(&ast, configuration, scenario).ok()?;
    if !model.uses_work_profiles() {
        return None;
    }
    let report = sensitivity_preflight_failure(
        src,
        configuration,
        scenario,
        Some(&model),
        vec![diag::Diagnostic {
            code: diag::U_SEM_0.into(),
            severity: diag::Severity::Error,
            message: "q6_feature_combination: analysis mode is not connected".into(),
            span: Some(ast.span),
        }],
    );
    Some((report, model))
}

fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qualification {
    pub capability: String,
    pub actor: String,
    #[serde(deserialize_with = "nullable")]
    pub executors: Option<Vec<String>>,
    #[serde(deserialize_with = "nullable")]
    pub qualified_executors: Option<Vec<String>>,
    #[serde(deserialize_with = "nullable")]
    pub permitted: Option<Vec<String>>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    pub basis: String,
    pub evidence_status: String,
    pub load: String,
    pub snapshot: String,
    pub duration_effect: String,
    pub rows: Vec<Row>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub executor: String,
    #[serde(deserialize_with = "nullable")]
    pub lower: Option<u64>,
    #[serde(deserialize_with = "nullable")]
    pub upper: Option<u64>,
    pub kernel: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub capability: String,
    pub profile: String,
}

macro_rules! fields {
    ($m:ident) => { $m! { rules: String, qualifications: Vec<Qualification>, profiles: Vec<Profile>, bindings: Vec<Binding>, } };
}
macro_rules! table {
    ($($name:ident: $ty:ty,)*) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(try_from = "Wire")]
        pub struct WorkProfiles { $(pub $name: $ty,)* }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire { $($name: $ty,)* }
        impl TryFrom<Wire> for WorkProfiles {
            type Error = String;
            fn try_from(w: Wire) -> Result<Self, String> {
                let t = Self { $($name: w.$name,)* };
                t.validate()?;
                Ok(t)
            }
        }
    };
}
fields!(table);

fn names(values: &[String]) -> bool {
    values.iter().all(|s| !s.is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

impl WorkProfiles {
    fn validate(&self) -> Result<(), String> {
        let require = |ok| {
            if ok {
                Ok(())
            } else {
                Err("invalid work_profiles declaration contract".to_owned())
            }
        };
        require(
            self.rules == RULES
                && self.profiles.len() <= 256
                && self.profiles.iter().map(|p| p.rows.len()).sum::<usize>() <= 4096,
        )?;
        let mut qualifications = BTreeMap::new();
        for q in &self.qualifications {
            require(
                !q.actor.is_empty()
                    && q.capability
                        .strip_prefix(&q.actor)
                        .is_some_and(|s| s.starts_with('.') && s.len() > 1)
                    && qualifications.insert(&q.capability, q).is_none(),
            )?;
            match &q.executors {
                None => require(q.qualified_executors.is_none() && q.permitted.is_none())?,
                Some(executors) => {
                    require(names(executors))?;
                    if let Some(ids) = &q.qualified_executors {
                        require(names(ids) && ids.iter().all(|p| executors.contains(p)))?;
                    }
                    if let Some(ids) = &q.permitted {
                        require(names(ids))?;
                    }
                }
            }
        }
        let mut profiles = BTreeMap::new();
        for p in &self.profiles {
            require(
                !p.name.is_empty()
                    && profiles.insert(&p.name, p).is_none()
                    && !p.basis.trim().is_empty()
                    && p.evidence_status == "analyst_declared_not_validated"
                    && p.snapshot == "start"
                    && p.duration_effect == "not_supported"
                    && matches!(p.load.as_str(), "none" | "queued_same_actor_at_start")
                    && !p.rows.is_empty(),
            )?;
            let mut ends = BTreeMap::new();
            for r in &p.rows {
                require(!r.executor.is_empty() && !r.kernel.is_empty())?;
                if p.load == "none" {
                    require(
                        r.lower.is_none() && r.upper.is_none() && !ends.contains_key(&r.executor),
                    )?;
                } else {
                    require(
                        r.lower.is_some()
                            && ends.get(&r.executor).copied().unwrap_or(Some(0)) == r.lower
                            && r.upper.is_none_or(|hi| r.lower.is_some_and(|lo| lo < hi)),
                    )?;
                }
                ends.insert(&r.executor, r.upper);
            }
            require(ends.values().all(Option::is_none))?;
        }
        let mut bound = BTreeSet::new();
        for binding in &self.bindings {
            require(bound.insert(&binding.capability))?;
            let q = qualifications
                .get(&binding.capability)
                .ok_or("work_profiles missing qualification")?;
            let p = profiles
                .get(&binding.profile)
                .ok_or("work_profiles unknown profile")?;
            let expected: BTreeSet<_> = q
                .qualified_executors
                .as_ref()
                .or(q.executors.as_ref())
                .map_or_else(
                    || ["anonymous"].into_iter().collect(),
                    |ps| ps.iter().map(String::as_str).collect(),
                );
            require(
                !expected.is_empty()
                    && expected == p.rows.iter().map(|r| r.executor.as_str()).collect(),
            )?;
        }
        Ok(())
    }

    /// Cross-check references against the evidence actually present on this wire.
    pub(super) fn validate_host(&self, host: &serde_json::Value) -> Result<(), String> {
        self.validate()?;
        let actors = host["actors"]
            .as_array()
            .ok_or("work_profiles requires resolved actors")?;
        for q in &self.qualifications {
            let actor = actors
                .iter()
                .find(|a| a["name"] == q.actor)
                .ok_or("unknown work profile actor")?;
            let recorded = actor.pointer("/executors/value/ids");
            if let Some(ids) = &q.executors {
                let actual: Vec<String> =
                    serde_json::from_value(recorded.cloned().ok_or("missing named executors")?)
                        .map_err(|e| e.to_string())?;
                if actual.iter().collect::<BTreeSet<_>>() != ids.iter().collect() {
                    return Err("work_profiles executors disagree with Actor declaration".into());
                }
            } else if recorded.is_some() {
                return Err("work_profiles anonymous Actor mismatch".into());
            }
            let name = q
                .capability
                .strip_prefix(&format!("{}.", q.actor))
                .ok_or("invalid capability")?;
            if actor
                .pointer("/capability/value")
                .and_then(serde_json::Value::as_array)
                .is_none_or(|caps| !caps.iter().any(|c| c == name))
            {
                return Err("work_profiles capability absent from Actor".into());
            }
        }
        let items = host
            .pointer("/uncertainty/aleatory/items")
            .and_then(serde_json::Value::as_array)
            .ok_or("missing aleatory evidence")?;
        for binding in &self.bindings {
            let p = self
                .profiles
                .iter()
                .find(|p| p.name == binding.profile)
                .ok_or("missing profile")?;
            let kernels: BTreeSet<_> = p.rows.iter().map(|r| &r.kernel).collect();
            for kernel in kernels {
                if items
                    .iter()
                    .filter(|r| {
                        r["kind"] == "kernel"
                            && r["capability"] == binding.capability
                            && r["name"] == *kernel
                    })
                    .count()
                    != 1
                {
                    return Err("work_profiles kernel reference does not resolve uniquely".into());
                }
            }
        }
        let refs = host
            .pointer("/uncertainty/model/items")
            .and_then(serde_json::Value::as_array)
            .ok_or("missing model uncertainty")?;
        if refs
            .iter()
            .filter(|r| r["kind"] == "model_context" && r["at"] == "/work_profiles")
            .count()
            != 1
        {
            return Err("missing work_profiles model context".into());
        }
        Ok(())
    }
}

pub(super) fn record(m: &ir::Model, src: &str) -> Result<Option<WorkProfiles>, String> {
    if m.performances.is_empty() && !m.uses_work_profiles() {
        return Ok(None);
    }
    let declarations = syntax::parse(src).map_err(|_| "work_profiles source cannot be parsed")?;
    let people = |ids: &[ir::PersonId]| {
        ids.iter()
            .map(|p| m.people.get(p.0).map(|p| p.name.clone()))
            .collect::<Option<Vec<_>>>()
            .ok_or("work_profiles references an unknown person")
    };
    let canonical = |ids: &[ir::PersonId]| {
        let mut ids = ids.to_vec();
        ids.sort();
        people(&ids)
    };
    let mut qualifications = Vec::new();
    let mut bindings = Vec::new();
    for c in &m.capabilities {
        if c.qualified_executors.is_none() && c.performance.is_none() {
            continue;
        }
        let actor = m
            .actors
            .get(c.actor.0)
            .ok_or("work_profiles references an unknown actor")?;
        let capability = format!("{}.{}", actor.name, c.name);
        // Actor IR canonically stores a set; this declaration record retains
        // source order, independently of the policy's first_idle order table.
        let executors = declarations.items.iter().find_map(|item| match item {
            ast::Item::Actor(a) if a.name.value == actor.name => {
                a.items.iter().find_map(|item| match item {
                    ast::ActorItem::Executors(ids) => Some(
                        ids.value
                            .iter()
                            .map(|p| p.value.clone())
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                })
            }
            _ => None,
        });
        if executors
            .as_ref()
            .map(|ps| ps.iter().cloned().collect::<BTreeSet<_>>())
            != actor
                .executors
                .as_deref()
                .map(people)
                .transpose()?
                .map(|ps| ps.into_iter().collect())
        {
            return Err("work_profiles source executors disagree with IR".into());
        }
        qualifications.push(Qualification {
            capability: capability.clone(),
            actor: actor.name.clone(),
            executors: executors.clone(),
            qualified_executors: c
                .qualified_executors
                .as_deref()
                .map(canonical)
                .transpose()?,
            permitted: match c.permission.as_deref() {
                Some(ids) => Some(canonical(ids)?),
                None => executors,
            },
        });
        if let Some(source) = &c.performance {
            bindings.push(Binding {
                capability,
                profile: m
                    .performances
                    .get(source.profile)
                    .ok_or("work_profiles references an unknown profile")?
                    .name
                    .clone(),
            });
        }
    }
    let mut profiles = Vec::new();
    for p in &m.performances {
        let mut declared = p.rows.clone();
        declared.sort_by_key(|r| (r.executor, r.from));
        let rows = declared
            .iter()
            .map(|r| {
                Ok(Row {
                    executor: match r.executor {
                        None => "anonymous".into(),
                        Some(id) => m
                            .people
                            .get(id.0)
                            .ok_or("work_profiles row references an unknown person")?
                            .name
                            .clone(),
                    },
                    lower: p.queued.then_some(r.from),
                    upper: if p.queued { r.until } else { None },
                    kernel: m
                        .kernels
                        .get(r.kernel)
                        .ok_or("work_profiles row references an unknown kernel")?
                        .name
                        .clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        profiles.push(Profile {
            name: p.name.clone(),
            basis: p.basis.clone(),
            evidence_status: "analyst_declared_not_validated".into(),
            load: if p.queued {
                "queued_same_actor_at_start"
            } else {
                "none"
            }
            .into(),
            snapshot: "start".into(),
            duration_effect: "not_supported".into(),
            rows,
        });
    }
    let table = WorkProfiles {
        rules: RULES.into(),
        qualifications,
        profiles,
        bindings,
    };
    table.validate()?;
    Ok(Some(table))
}

/// Recording is part of admission: a broken declaration must not be published
/// as absence or reach an interpreter with its evidence silently omitted.
pub(super) fn attach(r: &mut Report, m: &ir::Model, src: &str) -> Result<(), diag::Diagnostic> {
    match record(m, src) {
        Ok(table) => {
            r.work_profiles = table;
            Ok(())
        }
        Err(message) => Err(diag::Diagnostic {
            code: diag::E_TYPE_0.into(),
            severity: diag::Severity::Error,
            message: format!("invalid_q6_record: {message}"),
            span: None,
        }),
    }
}
