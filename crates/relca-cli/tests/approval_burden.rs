//! ADR 0061: fixed synthetic acceptance, intentionally RED before Q8 implementation.
//! No new core API is assumed; this suite compiles against the pre-Q8 binary.
#[path = "support/approval_inputs.rs"]
mod input;
#[path = "support/log_conformance_wire.rs"]
mod old_wire;
#[path = "support/approval_wire.rs"]
mod wire;
use serde_json::{Value, json};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn digest(bytes: &[u8]) -> String {
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .into()
}
struct Run {
    value: Value,
    exit: i32,
    err: String,
    spec_hash: String,
    records: Vec<Value>,
}
fn prepared(c: &input::Case) -> (String, String) {
    let mut log = c.log();
    if matches!(c.name, "a17_line_boundary" | "a17_line_limit") {
        let limit = if c.name == "a17_line_boundary" {
            65536
        } else {
            65537
        };
        let mut lines: Vec<_> = log.lines().map(String::from).collect();
        assert!(lines[1].len() < limit);
        let padding = limit - lines[1].len();
        lines[1].push_str(&" ".repeat(padding));
        log = lines.join("\n") + "\n";
    }
    // Input hash updates are mechanical transport only; they do not compute expectations.
    let mut spec = c.spec.clone();
    if spec["schema"] == "relca-approval-spec/v1" {
        spec["model_sha256"] = json!(digest(c.model.as_bytes()));
        spec["log_sha256"] = json!(if c.hash_mismatch {
            "0".repeat(64)
        } else {
            digest(log.as_bytes())
        });
    }
    let mut spec = c.raw_spec.clone().unwrap_or_else(|| spec.to_string());
    if matches!(c.name, "a17_spec_boundary" | "a17_spec_limit") {
        let limit = if c.name == "a17_spec_boundary" {
            1048576
        } else {
            1048577
        };
        assert!(spec.len() < limit);
        let padding = limit - spec.len();
        spec.push_str(&" ".repeat(padding));
    }
    (log, spec)
}
fn invoke(c: &input::Case, scope: &str, with_spec: bool, extra: &[&str]) -> Run {
    let dir = root().join(".tmp").join(format!(
        "q8-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let (log, spec) = prepared(c);
    let model_path = dir.join("model.relca");
    let log_path = dir.join("events.jsonl");
    let spec_path = dir.join("approval.json");
    std::fs::write(&model_path, &c.model).unwrap();
    std::fs::write(&log_path, &log).unwrap();
    std::fs::write(&spec_path, &spec).unwrap();
    let hashes = [
        digest(c.model.as_bytes()),
        digest(log.as_bytes()),
        digest(spec.as_bytes()),
    ];
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_relca"));
    cmd.current_dir(root())
        .arg("conform")
        .arg(&model_path)
        .arg("--log")
        .arg(&log_path)
        .args(["--scope", scope]);
    if with_spec {
        cmd.arg("--approval-spec").arg(&spec_path);
    }
    let out = cmd.args(extra).output().unwrap();
    for (p, hash) in [&model_path, &log_path, &spec_path]
        .into_iter()
        .zip(&hashes)
    {
        assert_eq!(digest(&std::fs::read(p).unwrap()), *hash, "input mutation");
    }
    assert!(out.stdout.ends_with(b"\n"));
    assert_eq!(out.stdout.iter().filter(|b| **b == b'\n').count(), 1);
    let value = serde_json::from_slice(&out.stdout).expect("one raw JSON result");
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(!err.contains("do-not-echo-private-sentinel"));
    Run {
        value,
        exit: out.status.code().unwrap(),
        err,
        spec_hash: hashes[2].clone(),
        records: c.records.clone(),
    }
}
fn expected(c: &input::Case, run: &Run) {
    let v = &run.value;
    let e = &c.expected;
    wire::report(v, run.exit).unwrap_or_else(|s| {
        panic!(
            "{}: {s}; actual exit={} verdict={} schema={} stderr={}",
            c.name, run.exit, v["verdict"], v["schema"], run.err
        )
    });
    assert_eq!(
        run.exit, e.exit,
        "{}: verdict {} diagnostics {}",
        c.name, v["verdict"], v["diagnostics"]
    );
    if let Some(code) = e.code {
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == code),
            "{} missing {code}",
            c.name
        );
        assert!(
            run.err.lines().any(|s| s.starts_with(&format!("{code}:"))),
            "diagnostic missing on stderr"
        );
    }
    if let Some(reason) = e.reason {
        assert!(
            wire::has_reason(v, reason),
            "{} missing reason {reason}: {}",
            c.name,
            v["diagnostics"]
        );
    }
    if e.no_metrics {
        assert!(v["approval_burden"].is_null());
        return;
    }
    let b = &v["approval_burden"];
    if !e.truncated {
        let mut wanted = std::collections::BTreeSet::new();
        for event in c.records.iter().skip(1) {
            if event["operation"] == c.spec["profile"]["approval_operation"] {
                wanted.insert((
                    event["case_id"].as_str().unwrap(),
                    event["task_id"].as_str().unwrap(),
                ));
            }
        }
        for row in c.spec["episodes"].as_array().unwrap() {
            wanted.insert((
                row["case_id"].as_str().unwrap(),
                row["approval_task_id"].as_str().unwrap(),
            ));
        }
        let actual: std::collections::BTreeSet<_> = b["episodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                (
                    row["case_id"].as_str().unwrap(),
                    row["approval_task_id"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            actual, wanted,
            "candidate union must not lose an observed or declared episode"
        );
        let wanted: std::collections::BTreeSet<_> = c.records[0]["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["case_id"].as_str().unwrap())
            .collect();
        let actual: std::collections::BTreeSet<_> = b["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["case_id"].as_str().unwrap())
            .collect();
        assert_eq!(
            actual, wanted,
            "header cases are not a selectable denominator"
        );
    }
    if !e.truncated {
        assert_eq!(b["spec"]["sha256"], run.spec_hash);
        assert_eq!(
            b["spec"]["profile"], c.spec["profile"],
            "declared classification must remain explicit"
        );
        assert_eq!(v["model"]["sha256"], digest(c.model.as_bytes()));
        assert_eq!(v["log"]["sha256"], digest(prepared(c).0.as_bytes()));
    }
    if let Some(n) = &e.numbers {
        let r = &b["rate"];
        let t = &b["reinvestigation"];
        assert_eq!(
            json!([
                r["numerator"],
                r["denominator"],
                r["value"],
                r["known_ineligible"],
                r["unknown_episodes"],
                r["no_observed_approval_cases"]
            ]),
            json!([n.n, n.d, n.rate, n.ineligible, n.unknown, n.no_approval]),
            "{} rate hand oracle",
            c.name
        );
        assert_eq!(
            json!([
                t["total"],
                t["mean"],
                t["known_episodes"],
                t["unknown_episodes"]
            ]),
            json!([n.total, n.mean, n.known_time, n.unknown_time]),
            "{} union/person-minute hand oracle",
            c.name
        );
    }
    if e.truncated {
        assert_eq!(v["limits"]["truncated"], true);
        if c.name != "a17_order_limit" {
            assert!(
                b["rate"]["numerator"].is_null()
                    && b["rate"]["denominator"].is_null()
                    && b["rate"]["value"].is_null()
            );
            assert!(
                b["reinvestigation"]["total"].is_null() && b["reinvestigation"]["mean"].is_null()
            );
        }
    }
    for episode in b["episodes"].as_array().unwrap() {
        let case = episode["case_id"].as_str().unwrap();
        let approval = episode["approval_task_id"].as_str().unwrap();
        let linked = c.spec["episodes"].as_array().and_then(|rows| {
            rows.iter()
                .find(|row| row["case_id"] == case && row["approval_task_id"] == approval)
        });
        if !e.truncated {
            let ordinal = c.spec["episodes"]
                .as_array()
                .unwrap()
                .iter()
                .position(|row| row["case_id"] == case && row["approval_task_id"] == approval);
            assert_eq!(
                episode["spec_pointer"],
                json!(ordinal.map(|i| format!("/episodes/{i}")))
            );
        }
        if matches!(
            c.name,
            "a01_baseline"
                | "a14_reverse"
                | "a14_rename"
                | "a10_case_key_control"
                | "a17_spec_boundary"
                | "a17_line_boundary"
        ) {
            assert_eq!(
                episode["rate"]["classification"],
                if case == "c2" {
                    "not_endorsed"
                } else {
                    "endorsed"
                }
            );
            assert_eq!(
                episode["reinvestigation"]["value"],
                match case {
                    "c1" => "2",
                    "c2" => "0",
                    "c3" => "5",
                    _ => panic!("unexpected case"),
                }
            );
        }
        let mut tasks = vec![approval];
        if let Some(link) = linked {
            if let Some(proposal) = link["proposal_task_id"].as_str() {
                tasks.push(proposal);
            }
            if let Some(ids) = link["reinvestigation_task_ids"].as_array() {
                tasks.extend(ids.iter().map(|id| id.as_str().unwrap()));
            }
        }
        let wanted: std::collections::BTreeSet<_> = run
            .records
            .iter()
            .skip(1)
            .filter(|event| {
                event["case_id"] == case && tasks.contains(&event["task_id"].as_str().unwrap())
            })
            .map(|event| event["event_id"].as_str().unwrap())
            .collect();
        let actual: std::collections::BTreeSet<_> = episode["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event["event_id"].as_str().unwrap())
            .collect();
        assert_eq!(
            actual, wanted,
            "{} complete positive evidence for {case}/{approval}",
            c.name
        );
        for evidence in episode["evidence"].as_array().unwrap() {
            let (line, event) = run
                .records
                .iter()
                .enumerate()
                .skip(1)
                .find(|(_, ev)| ev["event_id"] == evidence["event_id"])
                .expect("no invented evidence");
            assert_eq!(evidence["line"], line + 1);
            assert_eq!(evidence["case_id"], event["case_id"]);
            assert_eq!(evidence["task_id"], event["task_id"]);
            assert_eq!(evidence["time"], event["time"]);
            assert_eq!(evidence["after"], event["after"]);
        }
    }
    if c.name == "a15_orphan_recheck" {
        assert_eq!(
            b["unassigned_reinvestigation_tasks"],
            json!([{"case_id":"c1","task_id":"r1","event_ids":["c1-r1-finish","c1-r1-start"]}])
        );
    }
    if c.name == "a04_empty_control" {
        assert_eq!(
            b["episodes"][0]["reinvestigation"]["status"],
            "declared_empty_observation"
        );
        assert_eq!(b["episodes"][0]["reinvestigation"]["value"], "0");
    }
    if c.name == "a15_unmapped" {
        assert!(!v["unmapped_actor"].as_array().unwrap().is_empty());
    }
    if c.name == "a18_independent_violation" {
        assert!(wire::has_reason(v, "missing_reinvestigation_observation"));
    }
}
fn live(name: &'static str) {
    let c = input::case(name);
    let r = invoke(&c, "approval-burden", true, &[]);
    expected(&c, &r);
}
macro_rules! live_tests {($($name:ident),+ $(,)?)=>{$(#[test] fn $name(){live(stringify!($name));})+};}
live_tests!(
    a01_baseline,
    a01_approved_control,
    a02_ineligible,
    a02_eligible_control,
    a03_unobserved_case,
    a04_null,
    a04_empty_control,
    a05_missing_start,
    a05_missing_finish,
    a06_precision,
    a07_same_tick,
    a07_after_control,
    a08_same_person_union,
    a08_two_person_control,
    a08_alias_control,
    a09_missing_approval_start,
    a09_cancel_ack,
    a10_two_decisions,
    a10_duplicate_finish,
    a10_case_key_control,
    a11_missing_link,
    a12_version_mismatch,
    a12_missing_version,
    a13_zero_denominator,
    a14_reverse,
    a14_rename,
    a15_unmapped,
    a15_cut_outside,
    a15_partial_start,
    a15_partial_end,
    a15_endpoint_null,
    a15_header_only,
    a15_orphan_recheck,
    a16_duplicate_key,
    a16_unknown_field,
    a16_missing_field,
    a16_duplicate_episode,
    a16_double_assignment,
    a16_hash_mismatch,
    a16_future_schema,
    a16_missing_task,
    a16_multiple_profiles,
    a16_bad_result,
    a16_bad_operation,
    a17_spec_boundary,
    a17_spec_limit,
    a17_episode_boundary,
    a17_episode_limit,
    a17_event_boundary,
    a17_event_limit,
    a17_line_boundary,
    a17_line_limit,
    a17_order_limit,
    a18_independent_violation,
    a18_q1_unsupported,
    a18_defer,
    a18_cross_episode_overlap,
);

#[test]
fn green_all_positive_substrates_are_existing_scope_conformant() {
    for name in [
        "a01_baseline",
        "a01_approved_control",
        "a02_ineligible",
        "a02_eligible_control",
        "a04_empty_control",
        "a07_after_control",
        "a08_same_person_union",
        "a08_two_person_control",
        "a08_alias_control",
        "a10_two_decisions",
        "a14_reverse",
        "a14_rename",
    ] {
        let c = input::case(name);
        let r = relca_core::conformance::conform(&c.model, c.log().as_bytes(), None, None);
        assert_eq!(
            r.verdict,
            relca_core::Verdict::Satisfied,
            "{name}: {:?}",
            r.diagnostics
        );
    }
}
#[test]
fn green_all_selected_models_lower_before_testing_any_new_scope() {
    let mut sources = std::collections::BTreeSet::new();
    for &name in input::NAMES {
        sources.insert(input::case(name).model);
    }
    for source in sources {
        let ast = relca_core::syntax::parse(&source)
            .unwrap_or_else(|d| panic!("fixture syntax, not Q8 RED: {d:?}"));
        relca_core::lower::lower_with_selection(&ast, None, None)
            .unwrap_or_else(|d| panic!("fixture lower, not Q8 RED: {d:?}"));
    }
}
#[test]
fn green_order_limit_substrate_really_reaches_the_existing_run_wide_budget() {
    let c = input::case("a17_order_limit");
    let r = relca_core::conformance::conform(&c.model, c.log().as_bytes(), None, None);
    assert_eq!(r.verdict, relca_core::Verdict::Inconclusive);
    assert!(r.limits.order_limit && r.limits.truncated);
    assert!(
        r.checks
            .iter()
            .any(|c| c.reason.as_deref() == Some("order_limit"))
    );
}
#[test]
fn green_legacy_scope_matches_existing_core_with_ledger_and_no_approval_field() {
    let c = input::base("legacy");
    let r = invoke(&c, "event-contracts", false, &[]);
    old_wire::validate(&r.value, r.exit).unwrap();
    assert_eq!(r.exit, 0);
    let core = serde_json::to_value(relca_core::conformance::conform(
        &c.model,
        c.log().as_bytes(),
        None,
        None,
    ))
    .unwrap();
    assert_eq!(r.value, core);
    assert!(r.value.get("approval_burden").is_none());
    assert_eq!(
        r.value["summary"]["checks"],
        json!({"conformant":39,"violating":0,"unknown":0})
    );
    assert_eq!(
        r.value["summary"]["cases"],
        json!({"conformant":3,"violating":0,"unknown":0})
    );
    assert_eq!(old_wire::evidence_ids(&r.value).len(), 18);
    assert!(!old_wire::has_code(&r.value, "E-LOG-2"));
    assert_eq!(r.value["ledger"]["execution"]["mode"], "log_conformance");
}
#[test]
fn green_legacy_scope_rejects_spec_instead_of_silently_ignoring_it() {
    let c = input::base("legacy-spec");
    let r = invoke(&c, "event-contracts", true, &[]);
    old_wire::validate(&r.value, r.exit).unwrap();
    assert_eq!(r.exit, 3);
    assert!(wire::has_reason(&r.value, "argument"));
    assert!(r.value.get("approval_burden").is_none());
}
#[test]
fn green_legacy_scope_all_five_exits_are_fixed_without_q8_metrics() {
    for (name, exit) in [
        ("a01_baseline", 0),
        ("a18_independent_violation", 1),
        ("a05_missing_start", 2),
        ("a18_q1_unsupported", 4),
    ] {
        let c = input::case(name);
        let r = invoke(&c, "event-contracts", false, &[]);
        old_wire::validate(&r.value, r.exit).unwrap();
        assert_eq!(r.exit, exit, "{name}");
        if exit == 2 {
            assert!(old_wire::has_reason(&r.value, "missing_start"));
        }
        assert!(r.value.get("approval_burden").is_none());
    }
    let mut c = input::base("bad-model");
    c.model = "not a model".into();
    let r = invoke(&c, "event-contracts", false, &[]);
    old_wire::validate(&r.value, r.exit).unwrap();
    assert_eq!(r.exit, 3);
}
#[test]
fn green_fixed_inventory_has_distinct_names_and_correct_limit_constructions() {
    let names: std::collections::BTreeSet<_> = input::NAMES.iter().collect();
    assert_eq!(names.len(), input::NAMES.len());
    for name in ["a17_spec_boundary", "a17_spec_limit"] {
        let (log, spec) = prepared(&input::case(name));
        assert!(log.lines().next().unwrap().len() <= 65536);
        assert_eq!(
            spec.len(),
            if name == "a17_spec_boundary" {
                1048576
            } else {
                1048577
            }
        );
    }
    for name in ["a17_episode_boundary", "a17_episode_limit"] {
        let c = input::case(name);
        let (_, spec) = prepared(&c);
        assert_eq!(
            c.spec["episodes"].as_array().unwrap().len(),
            if name == "a17_episode_boundary" {
                10000
            } else {
                10001
            }
        );
        assert!(
            spec.len() <= 1048576,
            "episode limit must not hit byte limit first: {}",
            spec.len()
        );
    }
    for name in ["a17_event_boundary", "a17_event_limit"] {
        let c = input::case(name);
        let (log, _) = prepared(&c);
        assert_eq!(
            c.records.len() - 1,
            if name == "a17_event_boundary" {
                10000
            } else {
                10001
            }
        );
        assert!(log.lines().all(|s| s.len() <= 65536));
    }
}
#[test]
fn green_wire_control_and_single_faults_are_independent_of_production() {
    let valid = wire::control();
    wire::metrics(&valid).unwrap();
    for path in [
        "/rate/value",
        "/rate/denominator",
        "/reinvestigation/total",
        "/reinvestigation/known_episodes",
        "/episodes/0/rate/classification",
    ] {
        let mut bad = valid.clone();
        *bad.pointer_mut(path).unwrap() = match path {
            "/rate/value" => json!("0"),
            "/rate/denominator" => json!(2),
            "/reinvestigation/total" => json!("5"),
            "/reinvestigation/known_episodes" => json!(2),
            _ => json!("ineligible"),
        };
        assert!(
            wire::metrics(&bad).is_err(),
            "undetected single fault {path}"
        );
    }
    for key in valid.as_object().unwrap().keys() {
        let mut bad = valid.clone();
        bad.as_object_mut().unwrap().remove(key);
        assert!(wire::metrics(&bad).is_err(), "missing {key}");
    }
    let mut bad = valid.clone();
    bad["invented"] = json!(0);
    assert!(wire::metrics(&bad).is_err());
}
#[test]
fn green_unread_spec_prefix_cannot_claim_a_whole_input_hash() {
    let mut cut = wire::control();
    cut["spec"]["truncated"] = json!(true);
    cut["spec"]["sha256"] = Value::Null;
    cut["spec"]["processed_episodes"] = json!(0);
    cut["episodes"] = json!([]);
    cut["cases"] = Value::Null;
    for key in [
        "numerator",
        "denominator",
        "value",
        "known_ineligible",
        "unknown_episodes",
        "no_observed_approval_cases",
    ] {
        cut["rate"][key] = Value::Null;
    }
    cut["rate"]["reason"] = json!("truncated");
    cut["rate"]["coverage"] = json!("partial");
    for key in ["total", "mean", "known_episodes", "unknown_episodes"] {
        cut["reinvestigation"][key] = Value::Null;
    }
    cut["reinvestigation"]["reason"] = json!("truncated");
    cut["reinvestigation"]["coverage"] = json!("partial");
    wire::metrics(&cut).unwrap();
    let mut log_cut = cut.clone();
    log_cut["spec"]["truncated"] = json!(false);
    log_cut["spec"]["sha256"] = wire::control()["spec"]["sha256"].clone();
    wire::metrics(&log_cut).unwrap(); // log truncation does not imply a partial sidecar
    cut["spec"]["truncated"] = json!(false);
    assert!(wire::metrics(&cut).is_err());
}
#[test]
fn green_independent_small_order_enumerator_and_person_tick_union() {
    fn permutations(prefix: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if prefix.len() == 4 {
            out.push(prefix.clone());
            return;
        }
        for i in 0..4 {
            if !prefix.contains(&i) {
                prefix.push(i);
                permutations(prefix, out);
                prefix.pop();
            }
        }
    }
    // r.start=2, r.finish=4, a.start=4, a.finish=5. The same-tick pair
    // has no time edge. Every other strict-time/lifecycle edge is fixed.
    let mut all = Vec::new();
    permutations(&mut Vec::new(), &mut all);
    assert_eq!(all.len(), 24);
    let edges = [(0, 1), (0, 2), (0, 3), (1, 3), (2, 3)];
    let allowed: Vec<_> = all
        .iter()
        .filter(|path| {
            edges
                .iter()
                .all(|(a, b)| path.iter().position(|x| x == a) < path.iter().position(|x| x == b))
        })
        .collect();
    assert_eq!(allowed.len(), 2);
    assert_eq!(
        allowed
            .iter()
            .filter(|p| p.iter().position(|x| *x == 1) < p.iter().position(|x| *x == 2))
            .count(),
        1
    );
    let after: Vec<_> = allowed
        .into_iter()
        .filter(|p| p.iter().position(|x| *x == 1) < p.iter().position(|x| *x == 2))
        .collect();
    assert_eq!(after.len(), 1);
    let occupancy = |intervals: &[(&str, u64, u64)]| {
        let mut ticks = std::collections::BTreeSet::new();
        for &(person, start, stop) in intervals {
            for tick in start..stop {
                ticks.insert((person, tick));
            }
        }
        ticks.len()
    };
    assert_eq!(occupancy(&[("p", 2, 4), ("p", 3, 6)]), 4);
    assert_eq!(occupancy(&[("p", 2, 4), ("q", 3, 6)]), 5);
    assert_eq!(wire::fraction(&json!("7/3")).unwrap(), (7, 3));
    assert!(wire::fraction(&json!("14/6")).is_err());
}
#[test]
fn green_five_requested_broken_pairs_are_non_noop_and_have_distinct_hand_oracles() {
    let null = input::case("a04_null");
    let empty = input::case("a04_empty_control");
    assert_eq!(null.records, empty.records);
    assert_ne!(null.spec, empty.spec);
    assert_eq!(
        null.expected.numbers.as_ref().unwrap().n,
        2,
        "missing recheck is not extra endorsement"
    );
    assert_ne!(null.expected.exit, empty.expected.exit);
    let union = input::case("a08_same_person_union");
    let two = input::case("a08_two_person_control");
    assert_ne!(union.records, two.records);
    assert_eq!(union.expected.numbers.unwrap().total, Some("4"));
    assert_eq!(two.expected.numbers.unwrap().total, Some("5"));
    let missing = input::case("a03_unobserved_case");
    assert_eq!(
        missing.expected.numbers.unwrap().d,
        3,
        "unknown case is not denominator +1"
    );
    let multi = input::case("a16_multiple_profiles");
    assert!(multi.spec["profile"].is_array());
    assert_eq!(multi.expected.exit, 4);
}
#[test]
fn new_scope_missing_sidecar_is_invalid_in_the_new_schema() {
    let c = input::base("missing-sidecar");
    let r = invoke(&c, "approval-burden", false, &[]);
    wire::report(&r.value, r.exit).unwrap();
    assert_eq!(r.exit, 3);
    assert!(wire::has_reason(&r.value, "approval_spec_argument"));
}
#[test]
fn new_scope_duplicate_flag_is_invalid_not_last_value_wins() {
    let c = input::base("duplicate-flag");
    let r = invoke(&c, "approval-burden", true, &["--approval-spec", "unused"]);
    wire::report(&r.value, r.exit).unwrap();
    assert_eq!(r.exit, 3);
    assert!(wire::has_reason(&r.value, "approval_spec_argument"));
}

#[test]
fn close_input_cuts_withdraw_unobserved_counts_and_cases() {
    for name in [
        "a17_spec_limit",
        "a17_episode_limit",
        "a17_event_limit",
        "a17_line_limit",
    ] {
        let c = input::case(name);
        let r = invoke(&c, "approval-burden", true, &[]);
        assert_eq!(r.exit, 2, "{name}");
        let b = &r.value["approval_burden"];
        for path in [
            "/rate/numerator",
            "/rate/denominator",
            "/rate/known_ineligible",
            "/rate/unknown_episodes",
            "/rate/no_observed_approval_cases",
            "/reinvestigation/known_episodes",
            "/reinvestigation/unknown_episodes",
            "/cases",
        ] {
            assert!(b.pointer(path).unwrap().is_null(), "{name}: {path}");
        }
        assert_eq!(b["rate"]["reason"], "truncated");
        assert_eq!(b["reinvestigation"]["reason"], "truncated");
        assert_eq!(
            b["spec"]["truncated"],
            matches!(name, "a17_spec_limit" | "a17_episode_limit")
        );
        wire::report(&r.value, r.exit).unwrap();
    }
}

#[test]
fn close_approval_and_proposal_defer_preserve_not_configured_aggregate_reason() {
    let baseline = input::case("a01_baseline");
    let control = invoke(&baseline, "approval-burden", true, &[]);
    assert_eq!(control.exit, 0);
    assert_eq!(control.value["approval_burden"]["rate"]["value"], "2/3");
    for target in ["A", "A.decide", "P", "P.propose"] {
        let mut c = baseline.clone();
        c.model = c.model.replace(
            "cut C { include [P,A,R] }",
            &format!("cut C {{ include [P,A,R]; defer {target} reason \"unmeasured\" loses \"approval metrics\" }}"),
        );
        let r = invoke(&c, "approval-burden", true, &[]);
        assert_eq!(r.exit, 2, "{target}: {}", r.value["diagnostics"]);
        let b = &r.value["approval_burden"];
        assert_eq!(b["rate"]["reason"], "not_configured", "{target}");
        assert!(b["rate"]["value"].is_null());
        assert_eq!(b["rate"]["denominator"], 0);
        assert_eq!(b["rate"]["unknown_episodes"], 3);
        assert_eq!(b["reinvestigation"]["reason"], "not_configured");
        assert!(b["episodes"].as_array().unwrap().iter().all(|e| {
            e["rate"]["reasons"]
                .as_array()
                .unwrap()
                .contains(&json!("not_configured"))
        }));
        wire::report(&r.value, r.exit).unwrap();
    }
}

#[test]
fn close_wire_rejects_zero_filled_unobserved_census() {
    for spec_cut in [false, true] {
        let cut = wire::truncated_control(spec_cut);
        wire::metrics(&cut).unwrap();
        for path in [
            "/rate/known_ineligible",
            "/rate/unknown_episodes",
            "/rate/no_observed_approval_cases",
            "/reinvestigation/known_episodes",
            "/reinvestigation/unknown_episodes",
            "/cases",
        ] {
            let mut bad = cut.clone();
            *bad.pointer_mut(path).unwrap() = if path == "/cases" {
                json!([])
            } else {
                json!(0)
            };
            assert!(wire::metrics(&bad).is_err(), "{spec_cut}: {path}");
        }
    }
}

#[test]
fn close_ledger_rejects_mixed_schema_scope_even_with_matching_component_claims() {
    let c = input::case("a01_baseline");
    for scope in ["event-contracts", "approval-burden"] {
        let r = invoke(&c, scope, scope == "approval-burden", &[]);
        assert_eq!(r.exit, 0);
        let mut v = r.value;
        let good: relca_core::ledger::Ledger = serde_json::from_value(v["ledger"].clone()).unwrap();
        good.validate(&v).unwrap();
        v["scope"] = json!(if scope == "event-contracts" {
            "observable_event_contracts_with_approval_burden_v1"
        } else {
            "observable_event_contracts_v1"
        });
        // Reconcile the copied ledger's local claim too. Reject the pair itself,
        // not merely a stale component after a host-only edit.
        let mixed_scope = v["scope"].clone();
        for component in v["ledger"]["derivation"]["components"]
            .as_array_mut()
            .unwrap()
        {
            if component["at"] == "/scope" {
                component["version"] = mixed_scope.clone();
            }
        }
        let bad: relca_core::ledger::Ledger = serde_json::from_value(v["ledger"].clone()).unwrap();
        assert!(bad.validate(&v).is_err(), "{scope}");
    }
}
