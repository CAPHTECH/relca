// ADR0053 independent acceptance wire rules. Included only by fixture_match.
// Never calls production suppression, lower, engine, or a production DTO.
#[derive(Default)]
struct DeferredWireScope {
    applied: bool,
    checked: bool,
    global: bool,
    business: bool,
    actors: std::collections::BTreeSet<String>,
    capabilities: std::collections::BTreeSet<String>,
    people: std::collections::BTreeSet<String>,
}
fn deferred_wire_scope(v: &Value) -> Result<DeferredWireScope, String> {
    let mut s = DeferredWireScope::default();
    let Some(cut) = v.get("cut").filter(|x| x.is_object()) else {
        return Ok(s);
    };
    let rows = cut["defer"].as_array().ok_or("defer inventory")?;
    s.checked = cut["checked"]
        .as_array()
        .is_some_and(|a| a.iter().any(|x| x == "deferred_metrics"));
    s.applied = !rows.is_empty()
        && v["actors"].is_array()
        && (v["metrics"].is_object() || v["quality"].is_object());
    let actors = v["actors"].as_array();
    let mut all_caps = std::collections::BTreeSet::new();
    if let Some(actors) = actors {
        for a in actors {
            if let Some(cs) = a["capability"]["value"].as_array() {
                for c in cs {
                    all_caps.insert(format!(
                        "{}.{}",
                        a["name"].as_str().ok_or("Actor name")?,
                        c.as_str().ok_or("Capability name")?
                    ));
                }
            }
        }
    }
    for e in rows {
        let target = e["target"].as_str().ok_or("defer target")?;
        if e["actor"] == true {
            s.actors.insert(target.to_owned());
            s.capabilities.extend(
                all_caps
                    .iter()
                    .filter(|c| c.split_once('.').is_some_and(|(a, _)| a == target))
                    .cloned(),
            );
        } else if all_caps.contains(target) {
            s.capabilities.insert(target.to_owned());
            s.actors
                .insert(target.split_once('.').unwrap().0.to_owned());
        } else {
            s.global = true;
        }
    }
    if let Some(actors) = actors {
        for a in actors {
            if (s.global || a["name"].as_str().is_some_and(|n| s.actors.contains(n)))
                && let Some(ids) = a["executors"]["value"]["ids"].as_array()
            {
                for id in ids {
                    s.people
                        .insert(id.as_str().ok_or("executor id")?.to_owned());
                }
            }
        }
    }
    let calls = v["metrics"]["rates"]["capabilities"].as_array();
    s.business = s.global
        || calls.is_some_and(|cs| {
            cs.iter().any(|c| {
                c["required"] == true
                    && c["capability"]
                        .as_str()
                        .is_some_and(|n| s.capabilities.contains(n))
            })
        })
        || calls.is_none()
            && v["agency"]["lineage"].as_array().is_some_and(|cs| {
                cs.iter().any(|c| {
                    c["capability"]
                        .as_str()
                        .is_some_and(|n| s.capabilities.contains(n))
                })
            })
        || calls.is_none()
            && v["agency"]["viewpoints"]["retention"]["capabilities"]
                .as_array()
                .is_some_and(|cs| {
                    cs.iter().any(|c| {
                        (c["normal"]["max"].as_u64().is_some_and(|n| n > 0)
                            || c["contest_route"] == true)
                            && c["capability"]
                                .as_str()
                                .is_some_and(|n| s.capabilities.contains(n))
                    })
                });
    Ok(s)
}
fn deferred_is_new(v: &Value) -> bool {
    v["cut"]["checked"]
        .as_array()
        .is_some_and(|a| a.iter().any(|x| x == "deferred_metrics"))
}
fn validate_deferred_quantity_positions(v: &Value) -> Result<(), String> {
    let mut s = deferred_wire_scope(v)?;
    if s.checked {
        return deferred_validate_scope(v, &s);
    }
    // Historical projectors first validate the NEW wire/cells and restore OLD
    // bytes. Those legacy controls may keep defer in unchecked, but may never
    // acquire a new not_configured outside the old missing-fee positions.
    s.applied = false;
    s.business = false;
    s.global = false;
    deferred_validate_scope(v, &s)?;
    if let Some(rs) = v["metrics"]["resources"].as_array() {
        deferred_tree(&Value::Array(rs.clone()), false, false, false)?;
    }
    Ok(())
}
fn deferred_tree(
    v: &Value,
    affected: bool,
    missing_fee: bool,
    quality: bool,
) -> Result<(), String> {
    match v {
        Value::Object(o) => {
            let scalar = o.contains_key("reason")
                && (o.contains_key("min") && o.contains_key("max")
                    || o.contains_key("lower") && o.contains_key("upper")
                    || o.contains_key("point"));
            if scalar {
                let reason = v["reason"].as_str();
                let valid = if quality {
                    reason.is_none_or(|r| {
                        [
                            "not_configured",
                            "zero_denominator",
                            "denominator_may_be_zero",
                            "undetermined",
                            "truncated",
                            "aborted",
                            "no_samples",
                            "beyond_reached_mass",
                        ]
                        .contains(&r)
                    })
                } else {
                    reason.is_none_or(|r| METRIC_REASONS.contains(&r) || r == "join_any")
                };
                if !valid {
                    return Err("unknown deferred quantity reason".into());
                }
                let prior = reason.is_some_and(|r| r != "not_configured");
                if !prior && (affected || missing_fee) && reason != Some("not_configured") {
                    return Err("deferred or missing-rate quantity leaked a number".into());
                }
                if reason == Some("not_configured") && !affected && !missing_fee {
                    return Err("not_configured outside the deferred layer".into());
                }
                if affected || reason == Some("not_configured") || !quality && reason.is_some() {
                    for key in ["min", "max", "mean", "lower", "upper", "basis", "point"] {
                        if o.get(key).is_some_and(|x| !x.is_null()) {
                            return Err("deferred null retains a numeric endpoint/basis".into());
                        }
                    }
                    if o.contains_key("mean") && v["samples"] != 0 {
                        return Err("deferred Sample has no numeric samples".into());
                    }
                }
                if prior && !affected {
                    return Ok(());
                }
            }
            for (key, x) in o {
                if scalar && !["numerator", "denominator"].contains(&key.as_str()) {
                    continue;
                }
                deferred_tree(x, affected, missing_fee, quality)?;
            }
        }
        Value::Array(a) => {
            for x in a {
                deferred_tree(x, affected, missing_fee, quality)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn deferred_missing_rate(m: &Value, section: &str, id: &str, name: &Value, rate: &str) -> bool {
    m["rates"][section]
        .as_array()
        .is_some_and(|rows| rows.iter().any(|r| r[id] == *name && r[rate].is_null()))
}

/// ADR0019 追記U2 and ADR0022 追記「指標」: terminal costs use
/// normal lexical calls only. Report Case.tasks is that closed inventory;
/// contest-only routes belong to the observed totals, not the frozen end.
fn deferred_business_missing_rate(m: &Value) -> Result<bool, String> {
    let actors = m["rates"]["actors"]
        .as_array()
        .ok_or("Actor rate inventory")?;
    let caps = m["rates"]["capabilities"]
        .as_array()
        .ok_or("Capability rate inventory")?;
    let mut missing = false;
    for case in m["cases"].as_array().ok_or("metric cases")? {
        for task in case["tasks"].as_array().ok_or("metric tasks")? {
            let capability = task["capability"].as_str().ok_or("Task Capability")?;
            let (actor, _) = capability.split_once('.').ok_or("Task Capability name")?;
            let actor_rate = actors
                .iter()
                .find(|r| r["actor"] == actor)
                .ok_or("normal Task Actor rate missing")?;
            let start_rate = caps
                .iter()
                .find(|r| r["capability"] == capability)
                .ok_or("normal Task start rate missing")?;
            missing |= actor_rate["per_minute"].is_null() || start_rate["per_start"].is_null();
        }
    }
    Ok(missing)
}
/// Strict entry used before any historical projection. Old synthetic wires are
/// accepted by the old comparator; a new marker always selects this strict seam.
#[allow(dead_code)]
pub fn validate_deferred_metrics_wire(v: &Value) -> Result<(), String> {
    let s = deferred_wire_scope(v)?;
    deferred_validate_scope(v, &s)
}
// The deferred state is independent of a prior quality reason or retained hits.
pub fn validate_deferred_hard_cell(
    report: &Value,
    cohort: &Value,
    cell: &Value,
) -> Result<(), String> {
    let scope = deferred_wire_scope(report)?;
    if scope.applied && scope.business {
        let verdict = if ["aborted", "no_samples"]
            .iter()
            .any(|r| cohort["value"]["reason"] == *r)
        {
            "unsupported"
        } else {
            "inconclusive"
        };
        if !cell["interval"].is_null()
            || !cell["basis"].is_null()
            || !cell["point"].is_null()
            || cell["verdict"] != verdict
            || cell["reason"] != cohort["value"]["reason"]
        {
            return Err("deferred hard cannot reconstruct an interval from retained trials".into());
        }
    }
    Ok(())
}
fn deferred_validate_scope(v: &Value, s: &DeferredWireScope) -> Result<(), String> {
    if s.applied != s.checked {
        return Err("deferred_metrics checked applicability".into());
    }
    if s.applied {
        let cut = &v["cut"];
        if cut["checked"].as_array().unwrap().last() != Some(&serde_json::json!("deferred_metrics"))
            || cut["checked"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|x| *x == "deferred_metrics")
                .count()
                != 1
            || cut["unchecked"]
                .as_array()
                .ok_or("cut unchecked")?
                .iter()
                .any(|x| x == "deferred_metrics")
        {
            return Err("deferred_metrics exact checked move/order".into());
        }
    }
    let Some(m) = v.get("metrics").filter(|x| x.is_object()) else {
        return deferred_quality(v, s);
    };
    let active = s.applied;
    let business = active && s.business;
    let missing_actors = m["rates"]["actors"].as_array().is_some_and(|rs| {
        rs.iter()
            .any(|r| r["required"] == true && r["per_minute"].is_null())
    });
    let missing_caps = m["rates"]["capabilities"].as_array().is_some_and(|rs| {
        rs.iter()
            .any(|r| r["required"] == true && r["per_start"].is_null())
    });
    let missing_business = deferred_business_missing_rate(m)?;
    for a in m["actors"].as_array().ok_or("metric Actor inventory")? {
        let affected =
            active && (s.global || a["actor"].as_str().is_some_and(|n| s.actors.contains(n)));
        for key in ["work", "peak_tasks", "utilization", "queue"] {
            if let Some(x) = a.get(key) {
                deferred_tree(x, affected || business, false, false)?;
            }
        }
        deferred_tree(
            &a["occupancy_cost"],
            affected || business,
            deferred_missing_rate(m, "actors", "actor", &a["actor"], "per_minute"),
            false,
        )?;
    }
    for c in m["capabilities"]
        .as_array()
        .ok_or("metric Capability inventory")?
    {
        let affected = active
            && (s.global
                || c["capability"]
                    .as_str()
                    .is_some_and(|n| s.capabilities.contains(n)));
        deferred_tree(&c["starts"], false, false, false)?;
        deferred_tree(
            &c["start_cost"],
            affected || business,
            deferred_missing_rate(
                m,
                "capabilities",
                "capability",
                &c["capability"],
                "per_start",
            ),
            false,
        )?;
    }
    for p in m["people"].as_array().ok_or("metric people inventory")? {
        let affected =
            active && (s.global || p["person"].as_str().is_some_and(|n| s.people.contains(n)));
        deferred_tree(p, affected || business, false, false)?;
    }
    // U4/root B: T withdraws ALL resource time, including resources not used
    // by D. Direct non-T uses still need the independent source-bound seam.
    if active
        && (s.global || business)
        && let Some(rs) = m.get("resources")
    {
        deferred_tree(rs, true, false, false)?;
    }
    for (key, x) in m["overall"].as_object().ok_or("metric overall")? {
        if key == "business_value" {
            continue;
        }
        let missing = match key.as_str() {
            "occupancy_cost" => missing_actors,
            "start_cost" => missing_caps,
            "observed_cost_total" | "observed_cost_per_arrival" => missing_actors || missing_caps,
            _ => false,
        };
        deferred_tree(x, key != "counts" && business, missing, false)?;
    }
    for c in m["cases"].as_array().ok_or("metric cases")? {
        for (key, x) in c.as_object().ok_or("metric case")? {
            match key.as_str() {
                "tasks" => {
                    for t in x.as_array().ok_or("metric tasks")? {
                        let direct = active
                            && (s.global
                                || t["capability"]
                                    .as_str()
                                    .is_some_and(|n| s.capabilities.contains(n)));
                        for (field, y) in t.as_object().ok_or("metric task")? {
                            let affected = match field.as_str() {
                                "work" => direct || business,
                                "wait" | "wait_causes" | "predecessor_wait" | "refusal_wait" => {
                                    business
                                }
                                _ => false,
                            };
                            deferred_tree(y, affected, false, false)?;
                        }
                    }
                }
                "terminals" => {
                    for t in x.as_array().ok_or("metric case terminals")? {
                        for (field, y) in t.as_object().ok_or("metric case terminal")? {
                            deferred_tree(
                                y,
                                ["lead_time", "cost_per_case", "breakdown"]
                                    .contains(&field.as_str())
                                    && business,
                                field == "cost_per_case" && missing_business,
                                false,
                            )?;
                        }
                    }
                }
                "observed_cost_total" => {
                    deferred_tree(x, business, missing_actors || missing_caps, false)?
                }
                _ => deferred_tree(x, key != "counts" && business, false, false)?,
            }
        }
    }
    for t in m["terminals"].as_array().ok_or("metric terminals")? {
        deferred_tree(&t["cost_per_case"], business, missing_business, false)?;
    }
    deferred_quality(v, s)
}
fn deferred_quality(v: &Value, s: &DeferredWireScope) -> Result<(), String> {
    let Some(q) = v.get("quality").filter(|x| x.is_object()) else {
        return Ok(());
    };
    let affected = s.applied && s.business;
    for p in q["properties"].as_array().ok_or("quality properties")? {
        // The widened prior-reason null form belongs only to a Report which
        // actually declares the applied Cut check, not standalone quality syntax.
        if !s.checked && v["mode"] != "simulation" {
            for c in p["cases"].as_array().ok_or("quality cases")? {
                if c["min"].is_null()
                    && c["max"].is_null()
                    && matches!(c["reason"].as_str(), Some("truncated" | "undetermined"))
                {
                    return Err("prior-reason null case requires checked deferred_metrics".into());
                }
            }
        }
        deferred_tree(&p["cases"], affected, false, true)?;
        deferred_tree(&p["expected_count"], affected, false, true)?;
        if let Some(x) = p.get("unknown_count") {
            deferred_tree(x, false, false, true)?;
        }
    }
    for c in q["cohort"].as_array().ok_or("quality cohort")? {
        deferred_tree(&c["value"], affected, false, true)?;
        if affected && !c["bounds_with_undetermined"].is_null() {
            return Err("deferred cohort has no fallback bounds".into());
        }
    }
    for c in q["outcomes_per_arrival"]
        .as_array()
        .ok_or("quality outcomes")?
    {
        deferred_tree(&c["expected_count"], affected, false, true)?;
        deferred_tree(&c["rate"], affected, false, true)?;
    }
    if let Some(rows) = q["quantiles"].as_array() {
        for r in rows {
            deferred_tree(&r["value"], affected, false, true)?;
            if affected && !r["lower_bound"].is_null() {
                return Err("deferred quantile has no lower bound".into());
            }
        }
    }
    Ok(())
}

/// Source-bound independent AST/IR seam: closes resources and quality-only B.
/// The parser is shared syntax, but neither production lower nor suppression is
/// called. All process statements are exhaustively matched, including dead arms.
#[allow(dead_code)]
pub fn validate_deferred_metrics_with_source(
    v: &Value,
    src: &str,
    selection: Option<&str>,
) -> Result<(), String> {
    use relca_core::ast;
    use std::collections::{BTreeMap, BTreeSet};
    let parsed = relca_core::syntax::parse(src)
        .map_err(|_| "source-bound deferred oracle requires parsed input")?;
    if !v["actors"].is_array() {
        return validate_deferred_metrics_wire(v);
    }
    let mut caps: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut people: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut uses: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut processes = BTreeMap::new();
    let mut configurations = BTreeMap::new();
    let mut cuts = BTreeMap::new();
    let mut has_metrics = false;
    let mut has_quality = false;
    for item in &parsed.items {
        match item {
            ast::Item::Performance(_) => {
                panic!("performance declarations are outside the existing deferred population")
            }
            ast::Item::CasePriority(_) => {
                panic!("deferred wire walker does not handle priority declarations")
            }
            ast::Item::ArrivalsDistribution(_) => {
                panic!("deferred wire walker does not handle stochastic arrival distributions")
            }
            ast::Item::Calendar(_) | ast::Item::PersonCalendar { .. } => {
                panic!("deferred wire walker does not handle calendar declarations")
            }
            ast::Item::Actor(a) => {
                let mut names = Vec::new();
                for item in &a.items {
                    match item {
                        ast::ActorItem::Calendar(_) => {
                            panic!("deferred wire walker does not handle Actor calendars")
                        }
                        ast::ActorItem::Capability(c) => {
                            let key = format!("{}.{}", a.name.value, c.name.value);
                            names.push(key.clone());
                            let resources = uses.entry(key).or_default();
                            for part in &c.items {
                                if let ast::CapItem::Uses(rs) = &part.value {
                                    resources.extend(rs.iter().map(|r| r.resource.value.clone()));
                                }
                            }
                        }
                        ast::ActorItem::Executors(ps) => {
                            people.insert(
                                a.name.value.clone(),
                                ps.value.iter().map(|p| p.value.clone()).collect(),
                            );
                        }
                        ast::ActorItem::Version(_)
                        | ast::ActorItem::Decides(_)
                        | ast::ActorItem::Instance(_)
                        | ast::ActorItem::Boundary(_)
                        | ast::ActorItem::Grain(_)
                        | ast::ActorItem::Contains(_)
                        | ast::ActorItem::Overlaps(_)
                        | ast::ActorItem::Control(_)
                        | ast::ActorItem::ViaModel(_)
                        | ast::ActorItem::ControlParty(_)
                        | ast::ActorItem::Standing { .. }
                        | ast::ActorItem::Latent(_)
                        | ast::ActorItem::State(_)
                        | ast::ActorItem::Capacity(_)
                        | ast::ActorItem::Source(..) => {}
                    }
                }
                caps.insert(a.name.value.clone(), names);
            }
            ast::Item::Process(p) => {
                processes.insert(p.name.value.clone(), &p.body);
            }
            ast::Item::Configuration(c) => {
                configurations.insert(
                    c.name.value.clone(),
                    (c.cut.value.clone(), c.process.value.clone()),
                );
            }
            ast::Item::Cut(c) => {
                cuts.insert(c.name.value.clone(), c);
            }
            ast::Item::Metrics(_) => has_metrics = true,
            ast::Item::QualityProperty(_) => has_quality = true,
            ast::Item::Compare(_)
            | ast::Item::CohortMetric(_)
            | ast::Item::QuantileMetric(_)
            | ast::Item::Resource(_)
            | ast::Item::Responsible(_)
            | ast::Item::Simulation(_)
            | ast::Item::ArrivalsFixed(_)
            | ast::Item::ArrivalsEvery(_)
            | ast::Item::Scenario(_)
            | ast::Item::Policy(_)
            | ast::Item::Queue(_)
            | ast::Item::Concurrent(_)
            | ast::Item::Kernel(_)
            | ast::Item::ObservedCase(_)
            | ast::Item::LatentCase(_)
            | ast::Item::People(_)
            | ast::Item::Permit(_)
            | ast::Item::Authority(_)
            | ast::Item::Type(_)
            | ast::Item::Bounds(_)
            | ast::Item::Verify(_)
            | ast::Item::Contract(_) => {}
        }
    }
    if has_metrics != v["metrics"].is_object() || has_quality != v["quality"].is_object() {
        return Err("source indicator declaration/report mismatch".into());
    }
    let (cut_name, process_name) = if let Some(name) = selection {
        configurations
            .get(name)
            .cloned()
            .ok_or("unregistered source configuration")?
    } else {
        if processes.len() != 1 || cuts.len() > 1 {
            return Err("source oracle needs explicit configuration selection".into());
        }
        (
            cuts.keys().next().cloned().unwrap_or_default(),
            processes.keys().next().unwrap().clone(),
        )
    };
    let mut s = DeferredWireScope {
        checked: deferred_is_new(v),
        ..DeferredWireScope::default()
    };
    let mut expected = Vec::new();
    if let Some(cut) = cuts.get(&cut_name) {
        if v["cut"]["name"] != cut_name {
            return Err("source Cut selection differs".into());
        }
        for entry in &cut.items {
            if let ast::CutItem::Defer(e) = &entry.value {
                let target = e
                    .target
                    .value
                    .iter()
                    .map(|n| n.value.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                let actor = caps.contains_key(&target);
                expected.push(serde_json::json!({"target":target,"actor":actor,"reason":e.reason.value,"loses":e.loses.as_ref().map(|x|&x.value)}));
                if let Some(cs) = caps.get(&target) {
                    s.actors.insert(target.clone());
                    s.capabilities.extend(cs.iter().cloned());
                } else if uses.contains_key(&target) {
                    s.actors
                        .insert(target.split_once('.').unwrap().0.to_owned());
                    s.capabilities.insert(target);
                } else {
                    s.global = true;
                }
            }
        }
        if v["cut"]["defer"] != serde_json::json!(expected) {
            return Err("source defer declaration/report mismatch".into());
        }
    }
    s.applied = !expected.is_empty() && (has_metrics || has_quality);
    for a in &s.actors {
        if let Some(ps) = people.get(a) {
            s.people.extend(ps.iter().cloned());
        }
    }
    if s.global {
        s.people.extend(people.values().flatten().cloned());
    }
    fn calls(b: &ast::Block, out: &mut BTreeSet<String>, ends: &mut BTreeSet<String>) {
        let add = |c: &ast::Call, out: &mut BTreeSet<String>| {
            out.insert(format!("{}.{}", c.actor.value, c.capability.value));
        };
        for stmt in b {
            match &stmt.value {
                ast::Stmt::Merge { .. } | ast::Stmt::Yield(_) => panic!("merge control is outside this oracle"),
                ast::Stmt::Repeat { .. } | ast::Stmt::Again | ast::Stmt::Leave => panic!("bounded repetition is outside this oracle"),
                ast::Stmt::ResolveEffect { .. } | ast::Stmt::StopReceiptUnknown => {
                    panic!("receipt recovery is outside this oracle")
                }
                ast::Stmt::WaitUntil(_) => {
                    panic!("deferred wire walker does not handle condition waits")
                }
                ast::Stmt::Call(c) => add(c, out),
                ast::Stmt::Fork { branches, join, .. } => {
                    for c in branches {
                        add(c, out)
                    }
                    match &join.value {
                        ast::Join::All { .. } => {}
                        ast::Join::Any { if_none, .. } => {
                            ends.insert(if_none.value.clone());
                        }
                    }
                }
                ast::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    calls(then_body, out, ends);
                    calls(else_body, out, ends);
                }
                ast::Stmt::End(e) => {
                    ends.insert(e.value.clone());
                }
                ast::Stmt::Wait(_) => {}
            }
        }
    }
    let mut b = BTreeSet::new();
    let mut ends = BTreeSet::new();
    calls(
        processes
            .get(&process_name)
            .ok_or("source process selection")?,
        &mut b,
        &mut ends,
    );
    // Synthetic contest tasks are not process statements. Resolve activation
    // from the selected process's end/commit inventory, never all declarations.
    let normal = b.clone();
    for item in &parsed.items {
        if let ast::Item::Contract(c) = item {
            for clause in &c.clauses {
                if let ast::ContractClause::Contest { target, route, .. } = &clause.value {
                    let enabled = match target {
                        ast::StandingRef::End(e) => ends.contains(&e.value),
                        ast::StandingRef::Commit(c) => {
                            normal.contains(&format!("{}.{}", c.actor.value, c.capability.value))
                        }
                    };
                    if enabled {
                        b.insert(format!("{}.{}", route.actor.value, route.capability.value));
                    }
                }
            }
        }
    }
    s.business = s.global || !s.capabilities.is_disjoint(&b);
    deferred_validate_scope(v, &s)?;
    let resources: BTreeSet<_> = s
        .capabilities
        .iter()
        .flat_map(|c| uses.get(c).into_iter().flatten().cloned())
        .collect();
    if let Some(rows) = v["metrics"]["resources"].as_array() {
        for r in rows {
            let affected = s.applied
                && (s.business
                    || s.global
                    || r["resource"]
                        .as_str()
                        .is_some_and(|n| resources.contains(n)));
            deferred_tree(r, affected, false, false)?;
        }
    }
    Ok(())
}
