//! Source/IR-derived raw-output oracles. No captured values or permission table.
#![allow(dead_code)]
#[path = "conclusion_conform_jobs.rs"]
pub mod conform_jobs;
#[path = "../../../relca-core/tests/support/relation_producer_read_oracle.rs"]
mod producer;
#[path = "../../../relca-core/tests/support/read_write_review.rs"]
pub mod review;
#[path = "q7_snapshot_oracle.rs"]
pub mod sensitivity_jobs;
use relca_core::{ast, ir, lower, syntax};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const FIELDS: [&str; 2] = [review::h::FIELD, review::EXCLUDED];
pub type Surfaces = BTreeMap<String, Value>;

pub fn flag<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn bypass(block: &mut ast::Block, start: usize, keep: bool) -> bool {
    for i in 0..block.len() {
        if block[i].span.start == start
            && let ast::Stmt::If {
                then_body,
                else_body,
                ..
            } = &block[i].value
        {
            let replacement = if keep { then_body } else { else_body }.clone();
            block.splice(i..=i, replacement);
            return true;
        }
        if let ast::Stmt::If {
            then_body,
            else_body,
            ..
        } = &mut block[i].value
            && (bypass(then_body, start, keep) || bypass(else_body, start, keep))
        {
            return true;
        }
    }
    false
}

/// Projection of the declared edit onto condition/write/If relations, as in
/// the independent original census. Non-guard operators do not change them.
pub fn relation_ast(original: &ast::Model, mutant: &Value) -> ast::Model {
    let mut result = original.clone();
    match mutant["operator"]
        .as_str()
        .expect("classified mutation operator")
    {
        "bypass_guard" => {
            let start = mutant["site"]["span"]["start"].as_u64().unwrap() as usize;
            let keep = mutant["site"]["retained_branch"].as_str().unwrap();
            assert!(["then", "else"].contains(&keep));
            let mut found = false;
            for item in &mut result.items {
                if let ast::Item::Process(p) = item
                    && p.name.value == mutant["site"]["process"]
                {
                    found |= bypass(&mut p.body, start, keep == "then");
                }
            }
            assert!(found, "independent exact guard edit");
        }
        "remove_read" | "expire_authority" | "duplicate_summary" | "remove_denominator_end" => {}
        other => panic!("unclassified mutation relation: {other}"),
    }
    result
}

pub fn read_ast(original: &ast::Model, mutant: &Value) -> ast::Model {
    let mut result = relation_ast(original, mutant);
    if mutant["operator"] == "remove_read" {
        let start = mutant["site"]["span"]["start"].as_u64().unwrap() as usize;
        let mut found = false;
        for item in &mut result.items {
            if let ast::Item::Actor(a) = item {
                for item in &mut a.items {
                    if let ast::ActorItem::Capability(c) = item {
                        c.items.retain_mut(|i| {
                            if let ast::CapItem::Reads(reads) = &mut i.value {
                                let before = reads.len();
                                reads.retain(|r| r.span.start != start);
                                found |= before != reads.len();
                                return !reads.is_empty();
                            }
                            true
                        });
                    }
                }
            }
        }
        assert!(found, "independent exact read edit");
    }
    result
}

pub fn canonical_reports(
    doc: &Value,
    args: &[String],
) -> Vec<(String, Option<String>, Option<Value>)> {
    match args[0].as_str() {
        "sensitivity" => {
            assert!(sensitivity_jobs::classified_args(args));
            vec![]
        }
        "verify" => vec![],
        "conform" => {
            // Only ADR 0058's closed three observed-event jobs reach this class.
            assert!(conform_jobs::classified_args(args));
            vec![]
        }
        "check" | "simulate" | "explain" => vec![(
            String::new(),
            flag(args, "--configuration").map(str::to_owned),
            None,
        )],
        "diff" => ["from", "to"]
            .into_iter()
            .map(|side| {
                (
                    format!("/{side}/report"),
                    flag(args, &format!("--{side}")).map(str::to_owned),
                    None,
                )
            })
            .collect(),
        "compare" => {
            if doc["plans"].as_array().unwrap().is_empty()
                && matches!(doc["verdict"].as_str(), Some("invalid" | "unsupported"))
            {
                return vec![];
            } // Global front-end refusal precedes plan Reports.
            let names: Vec<_> = flag(args, "--plans").unwrap().split(',').collect();
            assert_eq!(doc["plans"].as_array().unwrap().len(), names.len());
            names
                .into_iter()
                .enumerate()
                .map(|(i, name)| {
                    assert_eq!(doc["plans"][i]["configuration"], name);
                    (format!("/plans/{i}/report"), Some(name.into()), None)
                })
                .collect()
        }
        "mutate" => {
            let mut result = vec![];
            if doc["original"].is_object() {
                result.push(("/original".into(), None, None));
            }
            for (ki, kind) in doc["mutations"].as_array().unwrap().iter().enumerate() {
                for (mi, mutant) in kind["mutants"].as_array().unwrap().iter().enumerate() {
                    if mutant["report"].is_object() {
                        result.push((
                            format!("/mutations/{ki}/mutants/{mi}/report"),
                            None,
                            Some(mutant.clone()),
                        ));
                    }
                }
            }
            result
        }
        other => panic!("unclassified CLI kind {other}"),
    }
}

fn k5_surface(v: &Value) -> bool {
    v["field"]
        .as_str()
        .is_some_and(|s| s.starts_with("concurrency.read_write"))
        || v["key"]
            .as_str()
            .is_some_and(|s| s.starts_with("/field=\"concurrency.read_write"))
}

pub fn observed(v: &Value) -> Surfaces {
    fn walk(v: &Value, path: String, result: &mut Surfaces) {
        if k5_surface(v) {
            result.insert(path.clone(), v.clone());
        }
        match v {
            Value::Object(o) => {
                for (key, child) in o {
                    walk(
                        child,
                        format!("{path}/{}", key.replace('~', "~0").replace('/', "~1")),
                        result,
                    );
                }
            }
            Value::Array(a) => {
                for (i, child) in a.iter().enumerate() {
                    walk(child, format!("{path}/{i}"), result);
                }
            }
            _ => {}
        }
    }
    let mut result = BTreeMap::new();
    walk(v, String::new(), &mut result);
    result
}

pub fn compare_surfaces(doc: &Value, expected: &Surfaces) -> Result<(), String> {
    let actual = observed(doc);
    if actual == *expected {
        Ok(())
    } else {
        Err(format!(
            "K5 exact registered source-derived surfaces differ: expected {} actual {}",
            expected.len(),
            actual.len()
        ))
    }
}

pub fn assumption_changes(before: &[Value], after: &[Value]) -> Vec<Value> {
    let duplicate: BTreeSet<_> = before
        .iter()
        .chain(after)
        .filter_map(|r| {
            let field = r["field"].as_str().unwrap();
            (before.iter().filter(|r| r["field"] == field).count() > 1
                || after.iter().filter(|r| r["field"] == field).count() > 1)
                .then_some(field)
        })
        .collect();
    let flatten = |rows: &[Value]| {
        let mut out = BTreeMap::new();
        for row in rows {
            let field = row["field"].as_str().unwrap();
            let id = format!(
                "/field={}{}",
                json!(field),
                if duplicate.contains(field) {
                    format!(";value={}", row["value"])
                } else {
                    String::new()
                }
            );
            for (key, value) in row.as_object().unwrap() {
                out.insert(format!("{id}/{key}"), value.clone());
            }
        }
        out
    };
    let a = flatten(before);
    let b = flatten(after);
    let keys: BTreeSet<_> = a.keys().chain(b.keys()).cloned().collect();
    keys.into_iter().filter_map(|key| {
        let before=a.get(&key).unwrap_or(&Value::Null);let after=b.get(&key).unwrap_or(&Value::Null);
        (before!=after).then(||json!({"check":"assumptions","key":key,"field":"value","before":before,"after":after,"direction":"changed"}))
    }).collect()
}

pub fn comparison_completed(report: &Value) -> bool {
    matches!(
        report["verdict"].as_str(),
        Some("satisfied" | "violated" | "vacuous")
    ) && report["bounds"]["truncated"] == false
}

/// Independent test-side scalar/set/count transport (the retained C2/C5
/// oracle), not the production mutation comparator.
pub fn typed_changes(before: &Value, after: &Value, check: &str) -> Vec<Value> {
    fn leaves(v: &Value, path: &str, out: &mut BTreeMap<String, Value>) {
        match v {
            Value::Object(o) => {
                for (key, value) in o {
                    if key != "span" && key != "sha256" {
                        leaves(value, &format!("{path}/{key}"), out);
                    }
                }
            }
            Value::Array(a) if a.iter().all(Value::is_string) => {
                out.insert(
                    path.into(),
                    json!(
                        a.iter()
                            .map(|v| v.as_str().unwrap())
                            .collect::<BTreeSet<_>>()
                    ),
                );
            }
            Value::Array(a) => {
                for (i, value) in a.iter().enumerate() {
                    let mut id = [
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
                    ]
                    .into_iter()
                    .filter_map(|key| {
                        value
                            .get(key)
                            .filter(|v| v.is_string() || v.is_i64() || v.is_u64())
                            .map(|v| format!("{key}={v}"))
                    })
                    .collect::<Vec<_>>()
                    .join(";");
                    if id.is_empty() {
                        id = if value.get("subject_sources").is_some() {
                            format!("subject={}", value["subject_sources"])
                        } else {
                            i.to_string()
                        };
                    }
                    let path = format!("{path}/{id}");
                    if value.get("code").is_some() {
                        let old = out.get(&path).and_then(Value::as_u64).unwrap_or(0);
                        out.insert(path, json!(old + 1));
                    } else {
                        leaves(value, &path, out);
                    }
                }
            }
            _ => {
                out.insert(path.into(), v.clone());
            }
        }
    }
    let mut a = BTreeMap::new();
    let mut b = BTreeMap::new();
    leaves(before, "", &mut a);
    leaves(after, "", &mut b);
    let keys: BTreeSet<_> = a.keys().chain(b.keys()).cloned().collect();
    keys.into_iter().filter_map(|key|{
        let before=a.get(&key).unwrap_or(&Value::Null);let after=b.get(&key).unwrap_or(&Value::Null);
        (before!=after).then(||json!({"check":check,"key":key,"field":"value","before":before,"after":after,"direction":"changed"}))
    }).collect()
}

fn assigned_covers(assigned: &Value, row: &Value) -> bool {
    if assigned["check"] == row["check"]
        && assigned["key"] == row["key"]
        && assigned["field"] == row["field"]
    {
        return true;
    }
    if row["check"] != "agency" || row["field"] != "value" {
        return false;
    }
    let key = assigned["key"].as_str().unwrap();
    let field = assigned["field"].as_str().unwrap();
    let target = row["key"].as_str().unwrap();
    if assigned["check"] == "agency.lineage"
        && let Some((call, cap)) = key.split_once(':')
    {
        return target
            == format!(
                "/lineage/call={};capability={}/{}",
                json!(call),
                json!(cap),
                if field == "lineage_subject_changed" {
                    "subject_sources"
                } else {
                    field
                }
            );
    }
    if assigned["check"] == "agency.refusal" {
        let parts: Vec<_> = key.splitn(3, ':').collect();
        return parts.len() == 3
            && target.starts_with(&format!("/refusal/call={};", json!(parts[0])))
            && target.contains(&format!(
                "/judgements/call={};capability={}/",
                json!(parts[1]),
                json!(parts[2])
            ))
            && (field == "lost_judgement" || target.ends_with(&format!("/{field}")));
    }
    false
}

pub fn transport_check(doc: &Value, args: &[String]) -> Result<usize, String> {
    let mut count = 0;
    for (path, _, mutant) in canonical_reports(doc, args) {
        let Some(mutant) = mutant else { continue };
        let after = doc.pointer(&path).unwrap();
        let order = [
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
        ];
        let ranks: Result<Vec<_>, String> = mutant["other_changes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                let check = r["check"].as_str().unwrap();
                if check == "properties"
                    && matches!(r["field"].as_str(), Some("verdict" | "presence"))
                {
                    Ok(0)
                } else if check == "agency.lineage" {
                    Ok(1)
                } else if check == "agency.refusal" {
                    Ok(2)
                } else {
                    order
                        .iter()
                        .position(|s| *s == check)
                        .map(|i| 3 + i)
                        .ok_or_else(|| format!("unclassified change group {check}"))
                }
            })
            .collect();
        if !ranks?.is_sorted() {
            return Err(format!("typed change group placement at {path}"));
        }
        for (check, before, after) in [
            ("agency", &doc["original"]["agency"], &after["agency"]),
            (
                "diagnostics",
                &doc["original"]["diagnostics"],
                &after["diagnostics"],
            ),
        ] {
            let wanted: Vec<_> = if comparison_completed(doc.pointer(&path).unwrap()) {
                typed_changes(before, after, check)
                    .into_iter()
                    .filter(|row| {
                        !mutant["assigned_changes"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|a| assigned_covers(a, row))
                    })
                    .collect()
            } else {
                vec![]
            };
            let actual: Vec<_> = mutant["other_changes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["check"] == check)
                .cloned()
                .collect();
            if actual != wanted {
                return Err(format!(
                    "independent complete {check} leaf/count transport at {path}: {}!={}",
                    actual.len(),
                    wanted.len()
                ));
            }
        }
        count += 1;
    }
    Ok(count)
}

pub struct Evidence {
    pub surfaces: Surfaces,
    pub reports: usize,
    pub excluded: usize,
    pub changes: usize,
    pub models: Vec<(String, ir::Model, Option<Value>)>,
}

pub fn expected(doc: &Value, source: Option<&str>, args: &[String]) -> Result<Evidence, String> {
    let ast = source.and_then(|s| syntax::parse(s).ok());
    let mut proof = Evidence {
        surfaces: BTreeMap::new(),
        reports: 0,
        excluded: 0,
        changes: 0,
        models: vec![],
    };
    let mut rows_by_report = BTreeMap::<String, Vec<Value>>::new();
    for (path, configuration, mutant) in canonical_reports(doc, args) {
        let report = doc
            .pointer(&path)
            .ok_or_else(|| format!("missing canonical Report {path}"))?;
        let rows = report["assumptions"]
            .as_array()
            .ok_or("Report assumptions absent")?;
        proof.reports += 1;
        let edited = ast.as_ref().map(|ast| {
            mutant
                .as_ref()
                .map_or_else(|| ast.clone(), |m| relation_ast(ast, m))
        });
        let model = edited.as_ref().and_then(|ast| {
            lower::lower_with_selection(ast, configuration.as_deref(), flag(args, "--scenario"))
                .ok()
        });
        let mut wanted = vec![];
        // Explicit front-end/static refusals may publish no lowered assumptions.
        // They still authorize no occurrence elsewhere in the raw document.
        if !(rows.is_empty() && report["bounds"]["cases"].is_null())
            && let Some(model) = &model
        {
            wanted = review::h::rows(model);
            let extra = review::disclosure(review::excluded_counts(model));
            proof.excluded += extra.len();
            wanted.extend(extra);
            let anchor = review::h::concurrency_anchor(report);
            if rows.get(anchor..anchor + wanted.len()) != Some(wanted.as_slice()) {
                return Err(format!(
                    "exact IR candidate/excluded payload and anchor {path}"
                ));
            }
            for (i, row) in wanted.iter().enumerate() {
                proof
                    .surfaces
                    .insert(format!("{path}/assumptions/{}", anchor + i), row.clone());
            }
        }
        rows_by_report.insert(path.clone(), wanted);
        // ADR 0026: supported requires is absent at every actual Agency slot.
        if report["agency"]["unchecked"]
            .as_array()
            .is_some_and(|a| a.iter().any(|w| w == "requires"))
        {
            return Err(format!("requires remained at canonical Agency {path}"));
        }
        if let Some(model) = model {
            proof.models.push((path, model, mutant));
        }
    }
    for (path, _, mutant) in canonical_reports(doc, args) {
        if mutant.is_none() {
            continue;
        }
        let parent = path.strip_suffix("/report").unwrap();
        let changes = doc
            .pointer(&format!("{parent}/other_changes"))
            .and_then(Value::as_array)
            .ok_or("mutant change table absent")?;
        let before = rows_by_report
            .get("/original")
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let after = rows_by_report.get(&path).map(Vec::as_slice).unwrap_or(&[]);
        let report = doc.pointer(&path).unwrap();
        let wanted = if !comparison_completed(report) {
            vec![] // A pre-execution/static refusal has no executed Report delta.
        } else {
            assumption_changes(before, after)
        };
        let actual: Vec<_> = changes.iter().filter(|r| k5_surface(r)).cloned().collect();
        if actual != wanted {
            return Err(format!("IR-derived other_changes payload {parent}"));
        }
        let indices: Vec<_> = changes
            .iter()
            .enumerate()
            .filter(|(_, r)| r["check"] == "assumptions")
            .map(|(i, _)| i)
            .collect();
        if indices.windows(2).any(|w| w[1] != w[0] + 1) {
            return Err("split generic assumptions block".into());
        }
        let keys: Vec<_> = indices
            .iter()
            .map(|i| changes[*i]["key"].as_str().unwrap())
            .collect();
        if keys.windows(2).any(|w| w[0] >= w[1]) {
            return Err("unordered/duplicate assumption change keys".into());
        }
        for (i, row) in changes.iter().enumerate().filter(|(_, r)| k5_surface(r)) {
            proof
                .surfaces
                .insert(format!("{parent}/other_changes/{i}"), row.clone());
        }
        proof.changes += usize::from(!wanted.is_empty());
    }
    compare_surfaces(doc, &proof.surfaces)?;
    Ok(proof)
}

pub fn producer_check(doc: &Value, source: &str, args: &[String]) -> Result<usize, String> {
    if args[0] != "diff" {
        return Ok(0);
    }
    for side in ["from", "to"] {
        if doc["agency"][side] != doc[side]["report"]["agency"] {
            return Err("diff Agency alias must transport the entire Report Agency".into());
        }
    }
    let Ok(ast) = syntax::parse(source) else {
        return Ok(0);
    };
    let (Ok(a), Ok(b)) = (
        lower::lower_configuration_with_scenario(
            &ast,
            flag(args, "--from").unwrap(),
            flag(args, "--scenario"),
        ),
        lower::lower_configuration_with_scenario(
            &ast,
            flag(args, "--to").unwrap(),
            flag(args, "--scenario"),
        ),
    ) else {
        return Ok(0);
    };
    let mismatch = a.model.cut.as_ref().map(|c| (&c.name, &c.horizon))
        != b.model.cut.as_ref().map(|c| (&c.name, &c.horizon));
    let left = producer::inventory(&a.model);
    let right = producer::inventory(&b.model);
    let wanted: Vec<_> = if mismatch {
        vec![]
    } else {
        left.iter()
            .filter_map(|(name, from)| {
                right
                    .get(name)
                    .filter(|to| *to != from)
                    .map(|to| json!({"target":name,"from":from,"to":to}))
            })
            .collect()
    };
    let actual: Vec<_> = doc["differences"]
        .as_array()
        .ok_or("diff rows absent")?
        .iter()
        .filter(|r| r["kind"] == "relation" && r["change"] == "modified")
        .map(
            |r| json!({"target":r["target"],"from":r["from"]["producer"],"to":r["to"]["producer"]}),
        )
        .collect();
    if actual != wanted {
        return Err("independent producer row inventory/payload/order".into());
    }
    for row in &wanted {
        let name = row["target"].as_str().unwrap();
        let claim = b.intent.iter().any(|i| match i {
            ast::Intent::Claim {
                action: ast::IntentAction::Change,
                target,
                ..
            } => target.kind == ast::DiffKind::Relation && target.name() == name,
            ast::Intent::Preserve {
                on: Some(target), ..
            } => target.kind == ast::DiffKind::Relation && target.name() == name,
            _ => false,
        });
        let annotation = b.intent.iter().any(|i| match i {
            ast::Intent::Claim { target, loses, .. } => {
                target.kind == ast::DiffKind::Relation && target.name() == name && loses.is_some()
            }
            ast::Intent::Preserve { on, .. } => on
                .as_ref()
                .is_none_or(|t| t.kind == ast::DiffKind::Relation && t.name() == name),
            _ => false,
        });
        for (code, message, count) in [
            (
                "W-DIFF-1",
                format!("computed change for {name} has no intent"),
                usize::from(!claim),
            ),
            (
                "W-DIFF-3",
                format!(
                    "computed change for relation {name} has no preserve, override, maintain or loses annotation"
                ),
                usize::from(!annotation),
            ),
        ] {
            let actual = doc["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["code"] == code && d["message"] == message)
                .count();
            if actual != count {
                return Err(format!(
                    "independent producer warning {name} {code}: {actual}!={count}"
                ));
            }
        }
    }
    Ok(wanted.len())
}

pub fn excluded_if_requires_mutation(model: &ir::Model) -> bool {
    model.state_variables.iter().enumerate().any(|(i, v)| {
        let eligible = match v.kind {
            ir::StateKind::ObservedCase => true,
            ir::StateKind::ActorObserved(a) => {
                model.bounds.cases == 1 && !model.actors[a.0].is_per_case()
            }
            _ => false,
        };
        let id = ir::StateVarId(i);
        let mut readers = vec![];
        review::h::if_readers(&model.process.body, id, &model.process.name, &mut readers);
        eligible
            && !readers.is_empty()
            && model.capabilities.iter().any(|c| {
                c.start_writes
                    .iter()
                    .chain(&c.writes)
                    .chain(c.atomic.iter().flat_map(|a| &a.writes))
                    .any(|w| w.variable == id)
            })
    })
}

pub fn registered_analysis_check(doc: &Value, args: &[String]) -> Result<(), String> {
    const TABLES: [&str; 3] = [
        "common_causes",
        "model_uncertainties",
        "source_descriptions",
    ];
    let mut allowed = BTreeSet::new();
    for (path, _, _) in canonical_reports(doc, args) {
        let report = doc.pointer(&path).unwrap();
        for field in TABLES {
            if report["agency"].get(field).is_some() {
                allowed.insert(format!("{path}/agency/{field}"));
            }
        }
        for (i, d) in report["diagnostics"].as_array().unwrap().iter().enumerate() {
            if d["code"]
                .as_str()
                .is_some_and(|c| c == "W-LIN-2" || c.starts_with("W-UNREACH-"))
            {
                allowed.insert(format!("{path}/diagnostics/{i}"));
            }
        }
    }
    if args[0] == "diff" {
        for side in ["from", "to"] {
            for field in TABLES {
                if doc["agency"][side].get(field).is_some() {
                    allowed.insert(format!("/agency/{side}/{field}"));
                }
            }
        }
    }
    fn collect(v: &Value, path: &str, out: &mut BTreeSet<String>) {
        if v["code"]
            .as_str()
            .is_some_and(|c| c == "W-LIN-2" || c.starts_with("W-UNREACH-"))
        {
            out.insert(path.into());
        }
        match v {
            Value::Object(o) => {
                for (key, value) in o {
                    let p = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                    if [
                        "common_causes",
                        "model_uncertainties",
                        "source_descriptions",
                    ]
                    .contains(&key.as_str())
                    {
                        out.insert(p.clone());
                    }
                    collect(value, &p, out);
                }
            }
            Value::Array(a) => {
                for (i, value) in a.iter().enumerate() {
                    collect(value, &format!("{path}/{i}"), out);
                }
            }
            _ => {}
        }
    }
    let mut actual = BTreeSet::new();
    collect(doc, "", &mut actual);
    if actual == allowed {
        Ok(())
    } else {
        Err("common/completion analysis outside canonical registered locations".into())
    }
}

/// Protect declaration keys/thresholds/directions/row order independently of
/// computed quality cells. The former K3 gate stored these in old-output
/// witnesses; the current predicate derives them directly from typed source IR.
pub fn evaluation_definition_check(
    doc: &Value,
    source: &str,
    args: &[String],
) -> Result<usize, String> {
    if args[0] != "compare" || doc.get("evaluation").is_none() {
        return Ok(0);
    }
    let ast = syntax::parse(source).map_err(|_| "evaluation without parsed source")?;
    let names: Vec<_> = flag(args, "--plans").unwrap().split(',').collect();
    let model = names
        .iter()
        .find_map(|name| {
            lower::lower_with_selection(&ast, Some(name), flag(args, "--scenario")).ok()
        })
        .ok_or("evaluation without selected typed source")?;
    let spec = model.comparison.as_ref().ok_or("undeclared evaluation")?;
    let quality = model.quality.as_ref();
    let hard = doc["evaluation"]["hard"]
        .as_array()
        .ok_or("hard table absent")?;
    if hard.len() != spec.hard.len() {
        return Err("independent hard declaration inventory".into());
    }
    for (row, decl) in hard.iter().zip(&spec.hard) {
        let name = &quality.ok_or("hard quality declaration absent")?.metrics[decl.cohort].name;
        let bound = decl.bound.to_string();
        let op = decl.op.text();
        if row["constraint"] != format!("cohort.{name} {op} {bound}")
            || row["cohort"] != *name
            || row["bound"] != bound
            || row["op"] != op
        {
            return Err("independent hard declaration/threshold".into());
        }
        let plans: Vec<_> = row["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["configuration"].as_str().unwrap())
            .collect();
        if plans != names {
            return Err("independent hard plan order".into());
        }
    }
    let objectives = doc["evaluation"]["objectives"]
        .as_array()
        .ok_or("objective table absent")?;
    if objectives.len() != spec.objectives.len() {
        return Err("independent objective declaration inventory".into());
    }
    for (row, decl) in objectives.iter().zip(&spec.objectives) {
        let direction = match decl.direction {
            ir::Direction::Minimize => "minimize",
            ir::Direction::Maximize => "maximize",
        };
        let target = match decl.target {
            ir::ObjectiveRef::Cohort(i) => format!(
                "cohort.{}",
                quality
                    .ok_or("objective quality declaration absent")?
                    .metrics[i]
                    .name
            ),
            ir::ObjectiveRef::Quantile(i) => format!(
                "quantile.{}",
                quality
                    .ok_or("objective quantile declaration absent")?
                    .quantiles[i]
                    .name
            ),
            ir::ObjectiveRef::Overall(v) => format!("metrics.overall.{}", v.text()),
        };
        if row["objective"] != format!("{direction} {target}") || row["direction"] != direction {
            return Err("independent objective definition/direction/order".into());
        }
        let plans: Vec<_> = row["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["configuration"].as_str().unwrap())
            .collect();
        if plans != names {
            return Err("independent objective plan order".into());
        }
    }
    Ok(hard.len() + objectives.len())
}

/// The old zero-location/source censuses also checked genuine declaration
/// invariants. Recompute those on current selected IR rather than freezing a
/// historical population. Pure lexical classifiers have separate hand controls.
pub fn declaration_check(
    report: &Value,
    ast: &ast::Model,
    model: &ir::Model,
) -> Result<(), String> {
    let actors: Vec<_> = ast
        .items
        .iter()
        .filter_map(|i| {
            if let ast::Item::Actor(a) = i {
                Some(a)
            } else {
                None
            }
        })
        .collect();
    if !actors.iter().any(|a| {
        a.items
            .iter()
            .any(|i| matches!(i, ast::ActorItem::State(_)))
    }) && model
        .state_variables
        .iter()
        .any(|v| matches!(v.kind, ir::StateKind::ActorObserved(_)))
    {
        return Err("undeclared Actor-observed state appeared in selected IR".into());
    }
    if !actors.iter().any(|a| {
        a.items
            .iter()
            .any(|i| matches!(i, ast::ActorItem::Instance(_)))
    }) && model.actors.iter().any(|a| a.is_per_case())
    {
        return Err("undeclared per-case instance appeared in IR".into());
    }
    if !actors.iter().any(|a| {
        a.items
            .iter()
            .any(|i| matches!(i, ast::ActorItem::Decides(_)))
    }) && model.actors.iter().any(|a| a.metadata.rule.is_some())
    {
        return Err("Actor label invented Rule semantics".into());
    }
    if !ast
        .items
        .iter()
        .any(|i| matches!(i, ast::Item::Resource(_)))
        && !model.resources.is_empty()
    {
        return Err("undeclared explicit resource appeared in IR".into());
    }
    if !ast
        .items
        .iter()
        .any(|i| matches!(i, ast::Item::Scenario(_)))
        && model.scenario.is_some()
    {
        return Err("undeclared scenario appeared in IR".into());
    }
    let authorities: Vec<_> = ast
        .items
        .iter()
        .filter_map(|i| {
            if let ast::Item::Authority(a) = i {
                Some(a)
            } else {
                None
            }
        })
        .collect();
    let classic = authorities.iter().all(|a| {
        a.items.iter().all(|i| {
            matches!(
                i.value,
                ast::AuthorityItem::Basis(_)
                    | ast::AuthorityItem::Grants { .. }
                    | ast::AuthorityItem::IssuedBy(_)
                    | ast::AuthorityItem::NotBy(_)
            )
        })
    });
    if classic {
        if model.authorities.len() != authorities.len() {
            return Err("declaration/IR Authority inventory".into());
        }
        for a in &model.authorities {
            let r = &a.restrictions;
            if r.scope_span.is_some()
                || r.expires.is_some()
                || r.effective_expires.is_some()
                || r.revocable_by.is_some()
                || r.delegated_from.is_some()
            {
                return Err("unconfigured Authority restriction appeared in IR".into());
            }
        }
        if report["assumptions"].as_array().unwrap().iter().any(|r| {
            r["field"]
                .as_str()
                .is_some_and(|f| f.starts_with("authority.") && f.ends_with(".revocable_by"))
        }) {
            return Err("unconfigured revocation disclosure".into());
        }
    }
    if let Some(table) = report["actors"].as_array() {
        if table.len() != model.actors.len() {
            return Err("Actor table inventory".into());
        }
        for (row, a) in table.iter().zip(&model.actors) {
            if row["name"] != a.name {
                return Err("Actor table declaration order".into());
            }
            let wanted = if !a.metadata.capacity_declared {
                a.executors.as_ref().map(|p| p.len() as u64).or(a.capacity)
            } else {
                a.capacity
            };
            if row["capacity"]["value"] != json!(wanted) {
                return Err("independent executor/capacity transport".into());
            }
            if let Some(pool) = &a.executors {
                let names: Vec<_> = pool
                    .iter()
                    .map(|p| model.people[p.0].name.clone())
                    .collect();
                if row["executors"]["value"] != json!({"kind":"named","ids":names}) {
                    return Err("independent executor identity transport".into());
                }
            }
        }
    } else if report["bounds"]["cases"].is_u64() {
        return Err("runtime lost Actor table".into());
    }
    Ok(())
}

pub fn agency_identity_check(report: &Value, model: &ir::Model) -> Result<(), String> {
    let agency = &report["agency"];
    let Some(v) = agency.get("viewpoints") else {
        return Ok(());
    };
    let actors: BTreeSet<_> = model.actors.iter().map(|a| a.name.as_str()).collect();
    let sources: BTreeSet<_> = model
        .actors
        .iter()
        .filter(|a| a.source.is_some())
        .map(|a| a.name.as_str())
        .collect();
    let people: BTreeSet<_> = model.people.iter().map(|p| p.name.as_str()).collect();
    let caps: BTreeSet<_> = model
        .capabilities
        .iter()
        .map(|c| format!("{}.{}", model.actors[c.actor.0].name, c.name))
        .collect();
    fn calls(block: &ir::Block, m: &ir::Model, result: &mut Vec<(String, String, bool)>) {
        let call = |c: &ir::Call| {
            let cap = &m.capabilities[c.capability.0];
            (
                m.process.bindings[c.binding.0].name.clone(),
                format!("{}.{}", m.actors[cap.actor.0].name, cap.name),
                cap.commits,
            )
        };
        for s in block {
            match &s.kind {
                ir::Stmt::LoopControl(_) => panic!("bounded repetition is outside this oracle"),
                ir::Stmt::Query(_, _)
                | ir::Stmt::ResolveEffect { .. }
                | ir::Stmt::StopReceiptUnknown => panic!("receipt recovery is outside this oracle"),
                ir::Stmt::WaitUntil(_) => {
                    panic!("population call oracle does not handle condition waits")
                }
                ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => result.push(call(c)),
                ir::Stmt::Fork { branches, .. } => result.extend(branches.iter().map(call)),
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    calls(then_body, m, result);
                    calls(else_body, m, result);
                }
                ir::Stmt::Wait(_) | ir::Stmt::End(_) => {}
            }
        }
    }
    let mut lexical = vec![];
    calls(&model.process.body, model, &mut lexical);
    let call_pairs: BTreeSet<_> = lexical
        .iter()
        .map(|(c, k, _)| (c.as_str(), k.as_str()))
        .collect();
    fn strings(v: &Value, known: &BTreeSet<&str>) -> bool {
        v.as_array().is_none_or(|a| {
            a.iter()
                .all(|s| s.as_str().is_some_and(|s| known.contains(s)))
        })
    }
    for source in v["observation"]["sources"].as_array().into_iter().flatten() {
        if !source["actor"]
            .as_str()
            .is_some_and(|s| sources.contains(s))
        {
            return Err("independent O source identity".into());
        }
        for route in ["normal", "contest"] {
            if !strings(&source[route]["actors"], &actors)
                || !strings(&source[route]["unattributed_actors"], &actors)
                || !strings(&source[route]["people"], &people)
            {
                return Err("independent O carrier identities".into());
            }
        }
        if source["contest"]["routes"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|c| !c.as_str().is_some_and(|c| caps.contains(c)))
        {
            return Err("independent contest capability identity".into());
        }
    }
    for person in v["observation"]["people"].as_array().into_iter().flatten() {
        if !person["person"]
            .as_str()
            .is_some_and(|s| people.contains(s))
        {
            return Err("independent O executor identity".into());
        }
        for (route, field) in [
            ("normal", "direct"),
            ("normal", "indirect"),
            ("contest", "direct"),
        ] {
            if !strings(&person[route][field], &sources) {
                return Err("independent executor source identities".into());
            }
        }
    }
    for end in v["framing"]["ends"].as_array().into_iter().flatten() {
        if !end["end"]
            .as_str()
            .is_some_and(|s| model.ends.iter().any(|e| e == s))
        {
            return Err("independent framing end identity".into());
        }
        for field in ["selectors", "read_only"] {
            for call in end[field].as_array().into_iter().flatten() {
                if !call_pairs.contains(&(
                    call["call"].as_str().unwrap(),
                    call["capability"].as_str().unwrap(),
                )) {
                    return Err("independent framing call/capability identity".into());
                }
            }
        }
    }
    for row in v["retention"]["capabilities"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if !row["capability"].as_str().is_some_and(|s| caps.contains(s)) {
            return Err("independent retained capability identity".into());
        }
    }
    if v["effects"]["status"] == "derived" {
        let expected: Vec<_> = lexical
            .iter()
            .filter(|(_, _, commit)| *commit)
            .map(|(call, cap, _)| json!({"call":call,"effect":cap}))
            .collect();
        let actual: Vec<_> = v["effects"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| json!({"call":r["call"],"effect":r["effect"]}))
            .collect();
        if actual != expected {
            return Err("independent lexical effect-call inventory".into());
        }
        for row in v["effects"]["rows"].as_array().unwrap() {
            for carrier in std::iter::once(&row["e"])
                .chain(row["j"].as_array().into_iter().flatten())
                .chain(row["u"].as_array().into_iter().flatten())
            {
                if !strings(&carrier["actors"], &actors)
                    || !strings(&carrier["unattributed_actors"], &actors)
                    || !strings(&carrier["people"], &people)
                    || !strings(&carrier["permitted"], &people)
                {
                    return Err("independent E/J/U carrier identities".into());
                }
            }
        }
    }
    Ok(())
}

pub fn mechanism_notes_check(report: &Value, model: &ir::Model) -> Result<(), String> {
    for (i, clause) in model.contracts.iter().enumerate() {
        let ir::ContractKind::AtMostOnce { event } = clause.kind else {
            continue;
        };
        let id = clause.property_id(ir::ClauseId(i));
        let cap = &model.capabilities[event.capability.0];
        let modeled = cap.atomic.as_ref().is_some_and(|a| {
            let ir::Cond::Contains {
                set,
                element: ir::Element::Parameter(parameter),
            } = a.condition
            else {
                return false;
            };
            event.kind == ir::EventKind::Commit
                && cap.commits
                && !a.commit_when
                && cap
                    .params
                    .get(parameter)
                    .is_some_and(|p| p.ty == ir::CASE_ID_TYPE)
                && model.state_variables[set.0].case_set_size.is_some()
                && model.state_variables[set.0].kind == ir::StateKind::ActorObserved(cap.actor)
                && a.writes.iter().any(|w| {
                    w.variable == set && w.source == ir::WriteSource::InsertCase { parameter }
                })
        });
        for property in report["properties"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["id"] == id)
        {
            let expected = (!modeled).then(|| json!(["mechanism_unmodeled"]));
            if property.get("notes") != expected.as_ref() {
                return Err("independent at-most-once modeling note".into());
            }
        }
    }
    Ok(())
}
