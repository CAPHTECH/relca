//! All current snapshot jobs, raw CLI output, independently recomputed IR values.
#[path = "support/common_cause_oracle.rs"]
mod common;
#[path = "support/completion_population_oracle.rs"]
mod completion;
#[path = "support/population_oracles.rs"]
mod oracle;
#[path = "support/p2c_snapshot_oracle.rs"]
mod p2c;
#[path = "support/q4_snapshot_oracle.rs"]
mod q4;
#[path = "support/q5_snapshot_oracle.rs"]
mod q5;
#[path = "support/q6_snapshot_oracle.rs"]
mod q6;
#[path = "support/q6b_snapshot_oracle.rs"]
mod q6b;
#[path = "support/q6c_snapshot_oracle.rs"]
mod q6c;
#[path = "support/q9_snapshot_oracle.rs"]
mod q9;
#[path = "support/r2_snapshot_oracle.rs"]
mod r2;
#[path = "support/response_snapshot_oracle.rs"]
mod response;
use oracle::conform_jobs;
use serde_json::Value;
use std::{path::Path, process::Command};

fn evidence_dir(root: &Path) -> std::path::PathBuf {
    // Test evidence is build output, not an untracked workspace input. Do not
    // rely on the invoking worktree's private .git/info/exclude entries.
    root.join("target/current-population-oracles")
}

// Closed state-IR census, not values copied from golden stdout. ADR 0058's
// three observable-event and three approval jobs are separately checked and counted.
// See current-oracle-obligations.md for enumeration and update obligations.
// Q5 adds exactly three closed hand jobs, not state-IR graph Reports. All
// pre-existing census categories and their independent checks stay unchanged.
// Q6a adds exactly two independently hand-checked jobs, not legacy IR Reports.
// Q6b adds one independent degradation hand job.
// Q6c adds three hand jobs (I17 compare/diff and I18 sensitivity); all other
// source/IR categories remain bound to the pre-existing enumeration.
// Cleanup adds exactly three closed hand jobs. These do not enter the legacy
// state-IR graph oracle; its other thirteen census categories remain unchanged.
// R2's fourteen closed hand jobs are checked separately, never fed to or
// subtracted from an existing legacy Report category. CENSUS is unchanged.
// ADR 0068: two zero-time expiry mutants are now not_applicable. Their two
// Report slots and modern-edit contexts disappear; originals and all twelve
// other categories remain. This is not an oracle exclusion or weaker check.
// Round 1 adds two advancing-time fixtures: one original and one expiry
// Report each; only the two originals enter the old graph/completion oracle.
const CENSUS: [usize; 14] = [2537, 4273, 436, 64, 20, 8, 3551, 3551, 3151, 4, 4, 4, 4, 77];

#[test]
fn advancing_time_expiry_pair_adds_exactly_four_report_slots_and_two_graph_contexts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in [
        "mut3-expiry-window",
        "mut3-expiry-window.broken-late-arrival",
    ] {
        let path = format!("fixtures/{name}/model.relca");
        let source = std::fs::read_to_string(root.join(&path)).unwrap();
        let args = vec!["mutate".to_owned(), path];
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(&args)
            .current_dir(&root)
            .output()
            .unwrap();
        let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
        let proof = oracle::expected(&doc, Some(&source), &args).unwrap();
        assert_eq!((proof.reports, proof.excluded, proof.changes), (2, 0, 0));
        assert_eq!(proof.models.len(), 2);
        let model = relca_core::lower::lower(&relca_core::syntax::parse(&source).unwrap()).unwrap();
        common::check(&doc["original"], &model).unwrap();
        assert!(
            completion::check(&doc["original"], &model, false)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn zero_time_expiry_removes_only_two_mutant_slots_not_their_originals() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in [
        "mut2-expiry.broken-zero-duration",
        "mut2-expiry-zero-finish",
    ] {
        let args = vec!["mutate".to_owned(), format!("fixtures/{name}/model.relca")];
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(&args)
            .current_dir(&root)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(4));
        let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(doc["run"]["reason"].is_null());
        assert_eq!(doc["original"]["verdict"], "satisfied");
        assert_eq!(doc["mutations"][1]["status"], "not_applicable");
        assert_eq!(doc["mutations"][1]["reason"], "no_time_elapse");
        let routes = oracle::canonical_reports(&doc, &args);
        assert_eq!(
            routes.len(),
            1,
            "{name}: keep the original, generate no mutant"
        );
        assert_eq!(routes[0].0, "/original");
    }
}

fn census_check(actual: &[usize; 14]) -> Result<(), String> {
    let names = [
        "jobs",
        "Reports",
        "excluded Reports",
        "K5 change blocks",
        "producer rows",
        "K5 fault pairs",
        "M1 Reports",
        "completion Reports",
        "completion warnings",
        "inline fault pairs",
        "delta faults",
        "M1 faults",
        "completion faults",
        "modern edits outside the pre-C6 graph oracle",
    ];
    for (i, name) in names.into_iter().enumerate() {
        if actual[i] != CENSUS[i] {
            return Err(format!(
                "closed IR census {name}: {} != {}; re-enumerate source/IR and document the reason before updating CENSUS",
                actual[i], CENSUS[i]
            ));
        }
    }
    Ok(())
}

fn inject(doc: &mut Value, args: &[String], fault: &str) -> bool {
    if fault == "threshold"
        && args[0] == "compare"
        && doc["evaluation"]["hard"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
    {
        doc["evaluation"]["hard"][0]["bound"] = serde_json::json!("wrong-threshold");
        return true;
    }
    let routes = oracle::canonical_reports(doc, args);
    for (path, _, mutant) in routes {
        let report = doc.pointer(&path).unwrap();
        if fault == "agency-null" && report["agency"].is_object() {
            doc.pointer_mut(&format!("{path}/agency"))
                .unwrap()
                .clone_from(&Value::Null);
            return true;
        }
        if fault == "mechanism"
            && let Some(i) = report["properties"]
                .as_array()
                .unwrap()
                .iter()
                .position(|p| p.get("notes").is_some())
        {
            doc.pointer_mut(&format!("{path}/properties/{i}/notes"))
                .unwrap()
                .clone_from(&serde_json::json!([]));
            return true;
        }
        let assumption = match fault {
            "excluded" | "offpath" => Some(oracle::review::EXCLUDED),
            "candidate" => Some(oracle::review::h::FIELD),
            _ => None,
        };
        if let Some(field) = assumption
            && let Some(i) = report["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .position(|r| r["field"] == field)
        {
            if fault == "offpath" {
                doc["outside"] = report["assumptions"][i].clone();
            } else {
                doc.pointer_mut(&format!("{path}/assumptions/{i}/value"))
                    .unwrap()
                    .clone_from(&serde_json::json!("corrupted independent count"));
            }
            return true;
        }
        if fault == "requires" && report["agency"]["unchecked"].is_array() {
            doc.pointer_mut(&format!("{path}/agency/unchecked"))
                .unwrap()
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!("requires"));
            return true;
        }
        if matches!(fault, "M1" | "analysis-offpath")
            && report["agency"].get("common_causes").is_some()
        {
            if fault == "M1" {
                doc.pointer_mut(&format!("{path}/agency/common_causes/notice"))
                    .unwrap()
                    .clone_from(&serde_json::json!("corrupted"));
            } else {
                doc["outside"] =
                    serde_json::json!({"common_causes":report["agency"]["common_causes"]});
            }
            return true;
        }
        if fault == "capacity" && report["actors"][0]["capacity"]["value"].is_u64() {
            doc.pointer_mut(&format!("{path}/actors/0/capacity/value"))
                .unwrap()
                .clone_from(&serde_json::json!(999));
            return true;
        }
        if fault == "completion"
            && let Some(i) = report["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .position(|d| {
                    d["code"]
                        .as_str()
                        .is_some_and(|c| c.starts_with("W-UNREACH-"))
                })
        {
            doc.pointer_mut(&format!("{path}/diagnostics/{i}/span/start"))
                .unwrap()
                .clone_from(&serde_json::json!(999999));
            return true;
        }
        if mutant.is_some()
            && matches!(
                fault,
                "delta" | "delta-position" | "agency-transport" | "diagnostic-transport"
            )
        {
            let parent = path.strip_suffix("/report").unwrap();
            let change_path = format!("{parent}/other_changes");
            let rows = doc.pointer(&change_path).unwrap().as_array().unwrap();
            if let Some(i) = rows.iter().position(|r| match fault {
                "delta" | "delta-position" => r["key"]
                    .as_str()
                    .is_some_and(|k| k.starts_with("/field=\"concurrency.read_write_excluded\"")),
                "agency-transport" => r["check"] == "agency",
                _ => r["check"] == "diagnostics",
            }) {
                if fault == "delta-position" {
                    let rows = doc
                        .pointer_mut(&change_path)
                        .unwrap()
                        .as_array_mut()
                        .unwrap();
                    let row = rows.remove(i);
                    rows.insert(0, row);
                } else {
                    doc.pointer_mut(&format!("{change_path}/{i}/after"))
                        .unwrap()
                        .clone_from(&serde_json::json!("corrupted"));
                }
                return true;
            }
        }
    }
    if fault == "producer"
        && args[0] == "diff"
        && let Some(i) = doc["differences"]
            .as_array()
            .unwrap()
            .iter()
            .position(|r| r["kind"] == "relation" && r["change"] == "modified")
    {
        doc["differences"][i]["to"]["producer"] = serde_json::json!("wrong.producer");
        return true;
    }
    false
}

#[test]
fn evidence_stays_in_ignored_build_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert_eq!(
        evidence_dir(&root),
        root.join("target/current-population-oracles")
    );
}

#[test]
fn lowered_ir_null_agency_requires_the_specific_frontend_refusal() {
    let source = include_str!("../../../fixtures/lin-invoice/model.relca");
    let ast = relca_core::syntax::parse(source).unwrap();
    let mut model = relca_core::lower::lower(&ast).unwrap();
    model.simulation = None;
    let mut report = serde_json::json!({
        "agency":null, "mode":"simulation", "verdict":"invalid",
        "diagnostics":[{"code":"E-SIM-0", "severity":"error",
            "message":"simulation requires an explicit simulation clause"}]
    });
    common::check(&report, &model).unwrap();
    for (key, value) in [
        ("code", "U-SEM-0"),
        ("message", "unrelated refusal"),
        ("severity", "warning"),
    ] {
        let mut broken = report.clone();
        broken["diagnostics"][0][key] = serde_json::json!(value);
        assert!(common::check(&broken, &model).is_err(), "{key}");
    }
    report["mode"] = serde_json::json!("exhaustive");
    assert!(common::check(&report, &model).is_err());
    report["mode"] = serde_json::json!("simulation");
    let sampled = include_str!("../../../fixtures/sim-statistical/model.relca");
    let sampled = relca_core::syntax::parse(sampled).unwrap();
    model.simulation = relca_core::lower::lower(&sampled).unwrap().simulation;
    assert!(model.simulation.is_some());
    assert!(
        common::check(&report, &model).is_err(),
        "IR has a simulation clause"
    );
}

#[test]
fn closed_census_rejects_population_loss_and_new_graph_exclusions() {
    census_check(&CENSUS).unwrap();
    for index in 0..CENSUS.len() {
        let mut broken = CENSUS;
        broken[index] -= 1;
        assert!(census_check(&broken).is_err(), "lost census entry {index}");
        broken[index] += 2;
        assert!(census_check(&broken).is_err(), "extra census entry {index}");
        if matches!(index, 1 | 13) {
            eprintln!(
                "single census fault rejected: {}",
                census_check(&broken).unwrap_err()
            );
        }
    }
}

#[test]
fn current_population_requires_exact_independent_registered_surfaces() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut jobs: Vec<Value> = serde_json::from_slice(
        &std::fs::read(root.join("crates/relca-cli/tests/snapshots/jobs.json")).unwrap(),
    )
    .unwrap();
    r2::validate_set(&jobs).unwrap();
    let legacy: Vec<_> = jobs
        .iter()
        .filter(|j| !r2::IDS.contains(&j["id"].as_str().unwrap()))
        .cloned()
        .collect();
    conform_jobs::validate_set(&legacy).unwrap();
    q4::validate_set(&jobs);
    q5::validate_set(&jobs);
    q6::validate_set(&jobs);
    q9::validate_set(&jobs);
    q6b::validate_set(&jobs);
    q6c::validate_set(&jobs);
    p2c::validate_set(&jobs);
    let canonical = jobs.len();
    let mut seen: std::collections::BTreeSet<Vec<String>> = jobs
        .iter()
        .map(|j| serde_json::from_value(j["args"].clone()).unwrap())
        .collect();
    let mut inspected = std::collections::BTreeSet::new();
    let mut added = vec![];
    for job in jobs.clone() {
        let args: Vec<String> = serde_json::from_value(job["args"].clone()).unwrap();
        if args.len() < 2
            || args[0] == "verify"
            || args[0] == "conform"
            || args[0] == "sensitivity"
            || q4::name(job["id"].as_str().unwrap()).is_some()
            || q5::name(job["id"].as_str().unwrap()).is_some()
            || q6::IDS.contains(&job["id"].as_str().unwrap())
            || q6b::IDS.contains(&job["id"].as_str().unwrap())
            || q6c::IDS.contains(&job["id"].as_str().unwrap())
            || p2c::IDS.contains(&job["id"].as_str().unwrap())
            || r2::IDS.contains(&job["id"].as_str().unwrap())
        {
            continue;
        }
        let scenario = oracle::flag(&args, "--scenario");
        if !inspected.insert((args[1].clone(), scenario.map(str::to_string))) {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(root.join(&args[1])) else {
            continue;
        };
        let Ok(ast) = relca_core::syntax::parse(&source) else {
            continue;
        };
        let Ok(model) = relca_core::lower::lower_with_selection(&ast, None, scenario) else {
            continue;
        };
        if !oracle::excluded_if_requires_mutation(&model) {
            continue;
        }
        let mut extra = vec!["mutate".to_string(), args[1].clone()];
        if let Some(scenario) = scenario {
            extra.extend(["--scenario".into(), scenario.into()]);
        }
        if seen.insert(extra.clone()) {
            added.push(serde_json::json!({"id":format!("semantic-excluded-delta-{}",added.len()),"args":extra,
                "reason":"Independent IR contains an excluded If read and a writer; mutate is required to check its delta."}));
        }
    }
    let evidence = evidence_dir(&root);
    std::fs::create_dir_all(&evidence).unwrap();
    std::fs::write(
        evidence.join("current-oracle-semantic-inputs.json"),
        serde_json::to_vec_pretty(&added).unwrap(),
    )
    .unwrap();
    jobs.extend(added);
    let selected = std::env::var("RELCA_POPULATION_JOB").ok();
    let fault = std::env::var("RELCA_POPULATION_FAULT").ok();
    if let Some(name) = &selected {
        jobs.retain(|j| j["id"] == *name);
        assert!(!jobs.is_empty(), "unknown calibration job");
    }
    let delta_evidence = std::sync::Mutex::new(Vec::<Value>::new());
    let response_reports = std::sync::atomic::AtomicUsize::new(0);
    let conform_evidence = std::sync::Mutex::new(Vec::<String>::new());
    let q4_evidence = std::sync::Mutex::new(Vec::<String>::new());
    let q7_evidence = std::sync::Mutex::new(Vec::<String>::new());
    let r2_evidence = std::sync::Mutex::new(Vec::<String>::new());
    let totals = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|worker| {
                let jobs = &jobs;
                let root = &root;
                let fault=&fault;
                 let delta_evidence=&delta_evidence;
                 let response_reports=&response_reports;
                  let conform_evidence=&conform_evidence;
                   let q4_evidence=&q4_evidence;
                    let q7_evidence=&q7_evidence;
                    let r2_evidence=&r2_evidence;
                scope.spawn(move || {
                    let mut counts = [0usize; 14];
                    for job in jobs.iter().skip(worker).step_by(4) {
                        let args: Vec<String> =
                            serde_json::from_value(job["args"].clone()).unwrap();
                        let output = Command::new(env!("CARGO_BIN_EXE_relca"))
                            .args(&args)
                            .current_dir(root)
                            .output()
                            .unwrap();
                        assert!(output.status.code().is_some_and(|c| (0..=5).contains(&c)));
                           let mut doc: Value = serde_json::from_slice(&output.stdout).unwrap();
                           if r2::IDS.contains(&job["id"].as_str().unwrap()) {
                               assert!(fault.is_none(),"legacy graph faults are not recovery hand jobs");
                               r2::check(job["id"].as_str().unwrap(),&args,&doc,output.status.code().unwrap()).unwrap_or_else(|e|panic!("{}: {e}",job["id"]));
                               r2_evidence.lock().unwrap().push(job["id"].as_str().unwrap().to_owned());
                               counts[0]+=1;
                               continue;
                           }
                          if p2c::IDS.contains(&job["id"].as_str().unwrap()) {
                              assert!(fault.is_none(), "legacy graph faults do not apply to cleanup hand jobs");
                              p2c::check(job["id"].as_str().unwrap(), &args, &doc, output.status.code().unwrap(), &output.stderr);
                              counts[0] += 1;
                              continue;
                          }
                          if q6c::IDS.contains(&job["id"].as_str().unwrap()) {
                              assert!(fault.is_none(), "legacy state-IR faults are not Q6c hand jobs");
                              q6c::check(job["id"].as_str().unwrap(), &args, &doc, output.status.code().unwrap(), &output.stderr);
                              counts[0] += 1;
                              continue;
                          }
                         if args[0] == "sensitivity" {
                             assert!(fault.is_none(), "old graph faults do not apply to sensitivity suites");
                             oracle::sensitivity_jobs::check(job["id"].as_str().unwrap(), &args, &doc, output.status.code().unwrap(), &output.stderr);
                             q7_evidence.lock().unwrap().push(job["id"].as_str().unwrap().to_owned());
                             counts[0] += 1;
                             continue;
                         }
                        if q5::name(job["id"].as_str().unwrap()).is_some() {
                            assert!(fault.is_none(), "old graph faults do not apply to Q5 hand jobs");
                            q5::check(job["id"].as_str().unwrap(), &args, &doc, output.status.code().unwrap(), &output.stderr);
                            counts[0] += 1;
                            continue;
                        }
                         if q6::IDS.contains(&job["id"].as_str().unwrap()) {
                             assert!(fault.is_none(), "legacy state-IR faults are not Q6a hand jobs");
                             let source = std::fs::read_to_string(root.join(&args[1])).unwrap();
                             q6::check(job["id"].as_str().unwrap(), &args, &doc, output.status.code().unwrap(), &output.stderr, &source);
                             counts[0] += 1;
                             continue;
                         }
                         if q6b::IDS.contains(&job["id"].as_str().unwrap()) {
                             assert!(fault.is_none(), "legacy state-IR faults are not Q6b hand jobs");
                             let source = std::fs::read_to_string(root.join(&args[1])).unwrap();
                             q6b::check(job["id"].as_str().unwrap(), &args, &doc, output.status.code().unwrap(), &output.stderr, &source);
                             counts[0] += 1;
                             continue;
                         }
                        if q9::IDS.contains(&job["id"].as_str().unwrap()) {
                            assert!(fault.is_none(), "legacy state-IR injected faults are not Q9 hand jobs");
                            let source = std::fs::read_to_string(root.join(&args[1])).unwrap();
                            q9::check(job["id"].as_str().unwrap(), &args, &doc, output.status.code().unwrap(), &output.stderr, &source);
                            counts[0] += 1;
                            continue;
                        }
                        if q4::name(job["id"].as_str().unwrap()).is_some() {
                            assert!(fault.is_none(), "old state-IR faults do not apply to Q4 time hand jobs");
                            q4::check(job["id"].as_str().unwrap(), &args, &doc, output.status.code().unwrap(), &output.stderr);
                            q4_evidence.lock().unwrap().push(job["id"].as_str().unwrap().to_owned());
                            counts[0] += 1;
                            continue;
                        }
                        if args[0] == "conform" {
                            conform_jobs::check_output(job["id"].as_str().unwrap(), &args, &doc, output.status.code().unwrap()).unwrap();
                            assert!(oracle::canonical_reports(&doc,&args).is_empty());
                            assert!(fault.is_none(), "state-IR faults do not apply to observed-event jobs");
                            conform_evidence.lock().unwrap().push(job["id"].as_str().unwrap().to_owned());
                            counts[0] += 1;
                            continue;
                        }
                        if let Some(fault)=fault {assert!(inject(&mut doc,&args,fault),"fault not applicable");eprintln!("fault applied population {fault}");}
                        let source = args
                            .get(1)
                            .and_then(|p| std::fs::read_to_string(root.join(p)).ok());
                        let proof = oracle::expected(&doc, source.as_deref(), &args)
                            .unwrap_or_else(|e| panic!("{}: {e}", job["id"]));
                        counts[0] += 1;
                        counts[1] += proof.reports;
                        counts[2] += proof.excluded;
                        counts[3] += proof.changes;
                        for (path,row) in &proof.surfaces {
                            if path.contains("/other_changes/") {
                                delta_evidence.lock().unwrap().push(serde_json::json!({"job":job["id"],"args":args,"path":path,"row":row}));
                            }
                        }
                        oracle::transport_check(&doc, &args)
                            .unwrap_or_else(|e| panic!("{}: {e}", job["id"]));
                        oracle::registered_analysis_check(&doc,&args).unwrap_or_else(|e|panic!("{}: {e}",job["id"]));
                        if let Some(source) = source.as_deref() {
                            oracle::evaluation_definition_check(&doc,source,&args).unwrap_or_else(|e|panic!("{}: {e}",job["id"]));
                            counts[4] += oracle::producer_check(&doc, source, &args)
                                .unwrap_or_else(|e| panic!("{}: {e}", job["id"]));
                            let ast = relca_core::syntax::parse(source).ok();
                            // doc and argv are immutable here. Reuse this lookup
                            // instead of cloning every mutant Report per model.
                            let canonical = oracle::canonical_reports(&doc, &args);
                            for (path, _, mutant) in &proof.models {
                                // These are the edit families present in the
                                // original independent common/completion gates.
                                // New C6/denominator operators retain the full
                                // K5 oracle above; they are not silently treated
                                // as old source edits for other analyses.
                                if mutant.as_ref().is_some_and(|m| {
                                    !matches!(
                                        m["operator"].as_str(),
                                        Some("remove_read" | "bypass_guard")
                                    )
                                }) {
                                    counts[13] += 1;
                                    continue;
                                }
                                let selected = &canonical
                                    .iter()
                                    .find(|(p, _, _)| p == path)
                                    .unwrap()
                                    .1;
                                let edited = mutant.as_ref().map_or_else(
                                    || ast.as_ref().unwrap().clone(),
                                    |m| oracle::read_ast(ast.as_ref().unwrap(), m),
                                );
                                let report = doc.pointer(path).unwrap();
                                let Ok(model) = relca_core::lower::lower_with_selection(
                                    &edited,
                                    selected.as_deref(),
                                    oracle::flag(&args, "--scenario"),
                                ) else {
                                    assert!(
                                        report["agency"].is_null(),
                                        "static refusal must not publish Agency"
                                    );
                                    continue;
                                };
                                oracle::declaration_check(report, &edited, &model)
                                    .unwrap_or_else(|e| panic!("{} {path}: {e}", job["id"]));
                                oracle::mechanism_notes_check(report,&model).unwrap_or_else(|e|panic!("{} {path}: {e}",job["id"]));
                                 oracle::agency_identity_check(report,&model).unwrap_or_else(|e|panic!("{} {path}: {e}",job["id"]));
                                 if !model.response_attempts().is_empty() {
                                     assert!(path.is_empty() && mutant.is_none(), "unregistered response Report route");
                                     response::check(job, &args, source, report, output.status.code());
                                     response_reports.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                     continue;
                                 }
                                common::check(report, &model)
                                    .unwrap_or_else(|e| panic!("{} {path}: {e}", job["id"]));
                                counts[6] += 1;
                                let simulation = args[0] == "simulate"
                                    || args[0] == "compare"
                                        && oracle::flag(&args, "--mode") == Some("simulation");
                                let warnings = completion::check(report, &model, simulation)
                                    .unwrap_or_else(|e| panic!("{} {path}: {e}", job["id"]));
                                counts[7] += 1;
                                counts[8] += warnings.len();
                                if counts[11] == 0
                                    && report["agency"].get("common_causes").is_some()
                                {
                                    let mut broken = report.clone();
                                    broken["agency"]["common_causes"]["notice"] =
                                        serde_json::json!("corrupted");
                                    assert!(common::check(&broken, &model).is_err());
                                    counts[11] += 1;
                                }
                                if counts[12] == 0 && !warnings.is_empty() {
                                    let mut broken = report.clone();
                                    let index = broken["diagnostics"]
                                        .as_array()
                                        .unwrap()
                                        .iter()
                                        .position(|d| {
                                            d["code"]
                                                .as_str()
                                                .is_some_and(|c| c.starts_with("W-UNREACH-"))
                                        })
                                        .unwrap();
                                    broken["diagnostics"][index]["span"]["start"] =
                                        serde_json::json!(999999);
                                    assert!(
                                        completion::check(&broken, &model, simulation).is_err()
                                    );
                                    counts[12] += 1;
                                }
                            }
                        }
                        // One genuine raw payload fault and one off-path occurrence
                        // traverse exactly the same production-facing comparator.
                        if (counts[5] == 0 || job["id"].as_str().unwrap().starts_with("excluded-"))
                            && let Some((path, row)) = proof
                                .surfaces
                                .iter()
                                .find(|(_, r)| r["field"] == oracle::review::EXCLUDED)
                        {
                            let mut broken = doc.clone();
                            broken.pointer_mut(path).unwrap()["value"] =
                                serde_json::json!("single_case=99; case_local=99");
                            assert!(oracle::compare_surfaces(&broken, &proof.surfaces).is_err());
                            let mut broken = doc.clone();
                            broken["outside"] = row.clone();
                            assert!(oracle::compare_surfaces(&broken, &proof.surfaces).is_err());
                            counts[5] += 1;
                            if job["id"].as_str().unwrap().starts_with("excluded-") {
                                counts[9] += 1;
                            }
                        }
                        if counts[10] == 0
                            && let Some((path, _)) = proof
                                .surfaces
                                .iter()
                                .find(|(p, _)| p.contains("/other_changes/"))
                        {
                            let mut broken = doc.clone();
                            broken.pointer_mut(path).unwrap()["before"] =
                                serde_json::json!("corrupted");
                            assert!(oracle::compare_surfaces(&broken, &proof.surfaces).is_err());
                            counts[10] += 1;
                        }
                    }
                    counts
                })
            })
            .collect();
        let mut total = [0; 14];
        for worker in workers {
            for (a, b) in total.iter_mut().zip(worker.join().unwrap()) {
                *a += b;
            }
        }
        std::fs::write(
            evidence.join("current-oracle-deltas.json"),
            serde_json::to_vec_pretty(&*delta_evidence.lock().unwrap()).unwrap(),
        )
        .unwrap();
        total
    });
    assert_eq!(totals[0], jobs.len());
    if selected.is_some() {
        assert!(fault.is_none(), "injected fault escaped all active oracles");
        eprintln!("selected calibration input positive GREEN");
        return;
    }
    let mut conform_checked = conform_evidence.into_inner().unwrap();
    let mut q4_checked = q4_evidence.into_inner().unwrap();
    q4_checked.sort();
    let mut expected_q4 = q4::IDS.map(str::to_string).to_vec();
    expected_q4.sort();
    assert_eq!(q4_checked, expected_q4);
    conform_checked.sort();
    let mut expected_conform = conform_jobs::IDS.map(str::to_string).to_vec();
    expected_conform.sort();
    assert_eq!(conform_checked, expected_conform);
    std::fs::write(
        evidence.join("conform-oracle-jobs.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"jobs":conform_checked,"count":conform_jobs::IDS.len(),"reason":conform_jobs::REASON}),
        )
        .unwrap(),
    )
    .unwrap();
    eprintln!(
        "canonical snapshot jobs={canonical}; source/IR-derived semantic-only mutate jobs={}",
        jobs.len() - canonical
    );
    eprintln!(
        "current population jobs={} state-IR Reports={} excluded={} change_blocks={} producer_rows={} fault_pairs={}",
        totals[0], totals[1], totals[2], totals[3], totals[4], totals[5]
    );
    eprintln!(
        "M1 Reports={} completion Reports={} completion warning sites={} inline faults={} delta faults={} M1 faults={} completion faults={} modern edit families outside pre-C6 graph oracle={}",
        totals[6], totals[7], totals[8], totals[9], totals[10], totals[11], totals[12], totals[13]
    );
    let mut state_ir_totals = totals;
    let mut r2_checked = r2_evidence.into_inner().unwrap();
    r2_checked.sort();
    let mut expected_r2 = r2::IDS.map(str::to_owned).to_vec();
    expected_r2.sort();
    assert_eq!(r2_checked, expected_r2);
    std::fs::write(
        evidence.join("r2-oracle-jobs.json"),
        serde_json::to_vec_pretty(&r2_checked).unwrap(),
    )
    .unwrap();
    state_ir_totals[0] -= r2::IDS.len();
    eprintln!("separate R2 recovery hand jobs=14; all 14 legacy census categories unchanged");
    let mut q7_checked = q7_evidence.into_inner().unwrap();
    q7_checked.sort();
    let mut expected_q7 = oracle::sensitivity_jobs::IDS.map(str::to_owned).to_vec();
    expected_q7.sort();
    assert_eq!(q7_checked, expected_q7);
    state_ir_totals[0] -= oracle::sensitivity_jobs::IDS.len();
    eprintln!("separate Q9 rates hand jobs=2; legacy state-IR Report categories unchanged");
    eprintln!("separate Q6a work profile hand jobs=2; other 13 census categories unchanged");
    eprintln!("separate Q6b degradation hand jobs=1; other 13 census categories unchanged");
    eprintln!(
        "separate Q6c compare/diff/sensitivity hand jobs=3; other 13 census categories unchanged"
    );
    eprintln!("separate Q7 sensitivity hand jobs=2; legacy state-IR census unchanged");
    state_ir_totals[0] -= conform_jobs::IDS.len();
    state_ir_totals[0] -= q4::IDS.len();
    eprintln!("separate Q4 time hand jobs=3; legacy lexical census unchanged");
    eprintln!(
        "separate observed-event wire+hand jobs={}; {}",
        conform_jobs::IDS.len(),
        conform_jobs::REASON
    );
    census_check(&state_ir_totals).unwrap();
    assert_eq!(
        response_reports.load(std::sync::atomic::Ordering::Relaxed),
        3,
        "three new Response contexts require their source-bound hand oracle; legacy graph contexts must not shrink"
    );
}
