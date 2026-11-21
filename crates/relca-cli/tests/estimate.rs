//! ADR 0082. Oracle: estimate_calculations.md, not captured production output.
#[path = "support/test_tmp.rs"]
mod test_tmp;

use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

const BASE: &str = r#"model M version "0.2" {
  type T = enum { pass, fail, denied }
  people [p]
  kernel K { * -> { 0.5: pass; 0.5: fail } }
  actor A { executors [p]; capability run() -> T { duration fixed 1min; outcomes kernel K } }
  process P { r = A.run(); end done }
  cut C { include [A] }
  bounds { cases 1; live_tasks 1; clock 20min }
  verify reachable end.done
}"#;
const DURATION: &str = "duration.A.run";
const KERNEL: &str = "kernel.A.run.K.1";
const ROUND2: &str = "kernel.A.run.K.2";
const ROUND3: &str = "kernel.A.run.K.3";
const ROW: &str = "{ 0.5: pass; 0.5: fail }";

fn latent() -> String {
    BASE.replace(
        "kernel K {",
        "latent case { truth:bool=true }; kernel K given [truth] {",
    )
    .replace(
        &format!("* -> {ROW}"),
        &format!("false -> {ROW}; true -> {ROW}"),
    )
    .replace(
        "outcomes kernel K",
        "outcomes kernel K; kernel_reads latent [truth]",
    )
}

fn repeat(conditioned: bool) -> String {
    let base = BASE.replace("r = A.run(); end done", "repeat laps at_most 3 { r = A.run(); if r == fail { leave } else { wait 1min; again } } on_limit { leave } end done");
    if conditioned {
        base.replace(
            &format!("kernel K {{ * -> {ROW} }}"),
            &format!("kernel K given [laps.round] {{ 1 -> {ROW}; 2 -> {ROW}; 3 -> {ROW} }}"),
        )
    } else {
        base
    }
}

fn guarded() -> String {
    repeat(true).replace(
        "duration fixed 1min; outcomes",
        "duration fixed 1min; requires laps.round >= 2 else denied; outcomes",
    )
}

fn skipped() -> String {
    repeat(true).replace(
        "r = A.run(); if r == fail { leave } else { wait 1min; again }",
        "if laps.round == 1 { wait 1min; again } else { r = A.run(); leave }",
    )
}

fn two_sites() -> String {
    repeat(true).replace("r = A.run(); if r", "r = A.run(); s = A.run(); if r")
}

fn same_result() -> String {
    BASE.replace("0.5: fail", "0.5: pass")
}

#[test]
fn models_lower_without_using_estimate() {
    for model in [
        BASE.to_owned(),
        latent(),
        same_result(),
        repeat(true),
        repeat(false),
        guarded(),
        skipped(),
        two_sites(),
    ] {
        let ast = relca_core::syntax::parse(&model).unwrap_or_else(|e| panic!("{model}: {e:?}"));
        relca_core::lower::lower(&ast).unwrap_or_else(|e| panic!("{model}: {e:?}"));
    }
}

fn task(case: usize, t: usize, time: usize, result: &str) -> [Value; 2] {
    let make = |phase: &str, time, result: Value| {
        json!({
            "kind":"event", "event_id":format!("c{case}t{t}-{phase}"), "case_id":format!("c{case}"),
            "task_id":format!("t{t}"), "operation":"A.run", "lifecycle":phase, "cancel_phase":null,
            "time":{"earliest":time,"latest":time}, "after":[], "actor_id":"actor-a", "executor_id":"person-p",
            "input_version":null,"output_version":null,"result":result,
        })
    };
    [
        make("start", time, Value::Null),
        make("finish", time + 1, json!(result)),
    ]
}

fn records(per_case: &[usize], window: usize) -> Vec<Value> {
    let cases: Vec<_> = (0..per_case.len())
        .map(|case| {
            json!({
                "case_id":format!("c{case}"),"from_case_start":true,"through_case_end":true
            })
        })
        .collect();
    let mut log = vec![json!({
        "kind":"header","schema":"relca-log/v1","clock":"observed","time_unit":"min",
        "window":{"from":0,"to":window},"cases":cases,
        "actor_map":[{"source_id":"actor-a","actor":"A"}],"executor_map":[{"source_id":"person-p","person":"p"}],
    })];
    for (case, &n) in per_case.iter().enumerate() {
        for t in 0..n {
            let result = if (case + t) % 2 == 0 { "pass" } else { "fail" };
            log.extend(task(case, t, 2 * t, result));
        }
    }
    log
}

fn baseline() -> Vec<Value> {
    records(&[1, 1, 1, 1], 1)
}
fn loop_log() -> Vec<Value> {
    records(&[1, 2, 3, 3], 7)
}

fn digest(path: &Path) -> String {
    let out = Command::new("sha256sum").arg(path).output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

fn invoke(
    model: &str,
    records: &[Value],
    slots: &[&str],
    confidence: &str,
    edit: impl FnOnce(&mut Value),
    truncate: bool,
) -> (Value, i32) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = test_tmp::root().join(format!(
        "e2-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).unwrap();
    let model_path = dir.join("model.relca");
    let log_path = dir.join("events.jsonl");
    let spec_path = dir.join("estimate-spec.json");
    fs::write(&model_path, model).unwrap();
    let lines: Vec<_> = records.iter().map(Value::to_string).collect();
    let mut log = lines.join("\n");
    if truncate {
        log.push_str(&" ".repeat(65537 - lines.last().unwrap().len()));
    }
    log.push('\n');
    fs::write(&log_path, &log).unwrap();
    let wanted_model = digest(&model_path);
    let wanted_log = digest(&log_path);
    let mut spec = json!({
        "schema":"relca-estimate-spec/v1", "model_sha256":wanted_model, "log_sha256":wanted_log,
        "confidence":confidence, "min_samples":1, "max_support":4, "decimals":2, "slots":slots,
    });
    edit(&mut spec);
    let spec_bytes = spec.to_string();
    fs::write(&spec_path, &spec_bytes).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("estimate")
        .arg(&model_path)
        .arg("--log")
        .arg(&log_path)
        .arg("--spec")
        .arg(&spec_path)
        .output()
        .unwrap();
    assert_eq!(fs::read_to_string(&model_path).unwrap(), model);
    assert_eq!(fs::read_to_string(&log_path).unwrap(), log);
    assert_eq!(fs::read_to_string(&spec_path).unwrap(), spec_bytes);
    assert!(
        !o.stderr.starts_with(b"usage: relca <"),
        "ENTRYPOINT_MISSING: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(o.stdout.split(|&b| b == b'\n').count(), 2);
    assert_eq!(o.stdout.last(), Some(&b'\n'));
    let wire: Value = serde_json::from_slice(&o.stdout).expect("one estimate JSON object");
    assert_eq!(wire["schema"], "relca-estimate/v1");
    if wire["model"]["sha256"].is_string() {
        assert_eq!(wire["model"]["sha256"], wanted_model);
    }
    if wire["log"]["sha256"].is_string() {
        assert_eq!(wire["log"]["sha256"], wanted_log);
    }
    (wire, o.status.code().expect("normal exit"))
}

fn ordinary(model: &str, log: &[Value], edit: impl FnOnce(&mut Value)) -> (Value, i32) {
    invoke(model, log, &[KERNEL, DURATION], "1079/2048", edit, false)
}
fn looping(model: &str, log: &[Value], conditioned: bool) -> (Value, i32) {
    let slots: &[&str] = if conditioned {
        &[KERNEL, ROUND2, ROUND3]
    } else {
        &[KERNEL]
    };
    invoke(model, log, slots, "55/1024", |_| {}, false)
}

fn row<'a>(w: &'a Value, id: &str) -> &'a Value {
    w["slots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["slot"]["id"] == id)
        .unwrap_or_else(|| panic!("missing {id}: {w}"))
}
fn verdict(w: &Value, exit: i32, v: &str, expected_exit: i32) {
    assert_eq!((&w["verdict"], exit), (&json!(v), expected_exit), "{w}");
}
fn fields(w: &Value, expected: Value) {
    for (key, val) in expected.as_object().unwrap() {
        assert_eq!(w.get(key), Some(val), "field {key}: {w}");
    }
}
fn mass(p: &str, f: &str, pd: &str, fd: &str) -> Value {
    json!([{"result":"pass","exact":p,"dsl":pd},{"result":"fail","exact":f,"dsl":fd}])
}
fn counts(p: u32, f: u32) -> Value {
    json!([{"result":"pass","hits":p},{"result":"fail","hits":f}])
}
fn interval(a: (&str, &str), b: (&str, &str)) -> Value {
    json!([{"lower":a.0,"upper":a.1},{"lower":b.0,"upper":b.1}])
}
fn equal_four() -> Value {
    interval(("1/8", "7/8"), ("1/8", "7/8"))
}
fn single_duration(n: u32, lower: &str) -> Value {
    json!({"n":n,"counts":[{"minutes":1,"hits":n}],"interval":[{"minutes":1,"lower":lower,"upper":"1"}]})
}

#[test]
fn e01_exact_baseline() {
    let (w, exit) = ordinary(BASE, &baseline(), |_| {});
    verdict(&w, exit, "satisfied", 0);
    fields(
        &w["multiplicity"],
        json!({"m":3,"confidence":"1079/2048","alpha":"323/4096"}),
    );
    let k = row(&w, KERNEL);
    fields(
        k,
        json!({"status":"derived","reason":null,"coverage":"complete","n":4,"unknown":0,"censored":0,
        "counts":counts(2,2),"interval":equal_four(),"candidate":mass("1/2","1/2","0.50","0.50"),
        "rounding_error_bound":"1/100","declared_vs_candidate":{"matches":true,"within_interval":true},
        "sensitivity_points":{"lower":["1/8","7/8"],"candidate":["1/2","1/2"],"upper":["7/8","1/8"]}}),
    );
    fields(
        &k["provenance"],
        json!({"log_sha256":w["log"]["sha256"],"window":{"from":0,"to":1},"case_count":4,"event_lines":{"from":2,"to":9}}),
    );
    let d = row(&w, DURATION);
    fields(d, single_duration(4, "555661/1048576"));
    fields(
        d,
        json!({"status":"derived","candidate":[{"minutes":1,"exact":"1","dsl":"1.00"}],
        "duration_summary":{"completed_only":false,"min":"1","q1":"1","median":"1","q3":"1","max":"1","mean":"1",
            "mean_interval":null,"mean_interval_reason":"no_statistical_bound","fixed_representative":"not_selected"},
        "sensitivity_points":{"lower":"1","candidate":"1","upper":"1"}}),
    );
}

#[test]
fn e02_insufficient_and_e03_unmapped() {
    let (w, exit) = ordinary(BASE, &baseline(), |s| s["min_samples"] = json!(5));
    verdict(&w, exit, "inconclusive", 2);
    for id in [KERNEL, DURATION] {
        fields(
            row(&w, id),
            json!({"status":"withheld","reason":"insufficient_samples","n":4,"candidate":null}),
        );
    }
    assert_eq!(row(&w, KERNEL)["interval"], equal_four());
    fields(row(&w, DURATION), single_duration(4, "555661/1048576"));

    let mut log = baseline();
    for e in &mut log[1..3] {
        e["actor_id"] = json!("other");
    }
    let (w, exit) = ordinary(BASE, &log, |_| {});
    verdict(&w, exit, "inconclusive", 2);
    assert_eq!(
        w["unmapped_actor"],
        json!([{"source_actor_id":"other","source_executor_id":"person-p","mapped_actor":null,
        "reason":"no_actor_mapping","case_ids":["c0"],"event_ids":["c0t0-finish","c0t0-start"]}])
    );
    fields(
        row(&w, KERNEL),
        json!({"n":3,"unknown":0,"counts":counts(1,2),"coverage":"partial",
        "candidate":mass("1/3","2/3","0.33","0.67"),
        "interval":interval(("885/32768","867875/1048576"),("180701/1048576","31883/32768"))}),
    );
    fields(row(&w, DURATION), single_duration(3, "56207/131072"));
}

#[test]
fn e04_latent_e05_zero_branch_e08_indistinguishable() {
    let (w, exit) = ordinary(&latent(), &baseline(), |_| {});
    verdict(&w, exit, "unsupported", 4);
    fields(
        row(&w, KERNEL),
        json!({"status":"not_derivable","reason":"latent_condition","n":null,"counts":null,
        "candidate":null,"interval":null,"marginal_counts":[{"result":"fail","hits":2},{"result":"pass","hits":2}]}),
    );
    let mut log = baseline();
    log[4]["result"] = json!("pass");
    let (w, exit) = ordinary(BASE, &log, |_| {});
    verdict(&w, exit, "satisfied", 0);
    assert_eq!(
        row(&w, KERNEL)["candidate"],
        mass("3/4", "1/4", "0.75", "0.25")
    );
    log[8]["result"] = json!("pass");
    let (w, exit) = ordinary(BASE, &log, |_| {});
    verdict(&w, exit, "inconclusive", 2);
    fields(
        row(&w, KERNEL),
        json!({"reason":"zero_count_branch","n":4,"counts":counts(4,0),"candidate":null,
        "interval":interval(("555661/1048576","1"),("0","492915/1048576"))}),
    );
    let (w, exit) = ordinary(&same_result(), &baseline(), |_| {});
    verdict(&w, exit, "unsupported", 4);
    fields(
        row(&w, KERNEL),
        json!({"status":"not_derivable","reason":"branches_not_distinguishable","candidate":null,"interval":null}),
    );
}

#[test]
fn e06_censored_e07_time_precision() {
    for (reason, censored) in [("censored", 1), ("time_precision", 0)] {
        let mut log = baseline();
        if censored == 1 {
            log.remove(2);
        } else {
            log[1]["time"]["latest"] = json!(1);
        }
        let (w, exit) = ordinary(BASE, &log, |_| {});
        verdict(&w, exit, "inconclusive", 2);
        fields(
            row(&w, KERNEL),
            json!({"n":4,"unknown":1,"censored":censored,"coverage":"partial","counts":counts(1,2),
            "interval":interval(("21313/1048576","7/8"),("1/8","1027263/1048576")),"candidate":mass("1/3","2/3","0.33","0.67")}),
        );
        let d = row(&w, DURATION);
        fields(
            d,
            json!({"status":"withheld","reason":reason,"n":4,"unknown":1,"censored":censored,"candidate":null,"counts":[{"minutes":1,"hits":3}]}),
        );
        fields(
            &d["duration_summary"],
            json!({"completed_only":true,"mean":"1"}),
        );
    }
}

#[test]
fn r01_conditioned_and_unpooled() {
    let (w, exit) = looping(&repeat(true), &loop_log(), true);
    verdict(&w, exit, "satisfied", 0);
    fields(&w["multiplicity"], json!({"m":6,"alpha":"323/4096"}));
    let hand = [
        json!({"n":4,"counts":counts(2,2),"candidate":mass("1/2","1/2","0.50","0.50"),"interval":equal_four()}),
        json!({"n":3,"counts":counts(2,1),"candidate":mass("2/3","1/3","0.67","0.33"),
            "interval":interval(("180701/1048576","31883/32768"),("885/32768","867875/1048576"))}),
        json!({"n":2,"counts":counts(1,1),"candidate":mass("1/2","1/2","0.50","0.50"),
            "interval":interval(("2637/65536","62899/65536"),("2637/65536","62899/65536"))}),
    ];
    for (id, want) in [KERNEL, ROUND2, ROUND3].into_iter().zip(&hand) {
        fields(row(&w, id), want.clone());
    }
    let (w, exit) = looping(&repeat(false), &loop_log(), false);
    verdict(&w, exit, "unsupported", 4);
    let r = row(&w, KERNEL);
    fields(
        r,
        json!({"status":"not_derivable","reason":"round_pooling_not_supported","candidate":null,"interval":null}),
    );
    let rounds = r["rounds"].as_array().unwrap();
    assert_eq!(rounds.len(), 3);
    for (index, (round, want)) in rounds.iter().zip(hand).enumerate() {
        assert_eq!(round["round"], index + 1);
        fields(round, want);
    }
}

#[test]
fn r02_ambiguous_order_and_r03_over_limit() {
    for reason in ["round_order_ambiguous", "rounds_exceed_model_limit"] {
        let mut log = loop_log();
        if reason == "round_order_ambiguous" {
            log[15]["time"] = json!({"earliest":0,"latest":0});
        } else {
            log.extend(task(3, 3, 6, "pass"));
        }
        let (w, exit) = looping(&repeat(true), &log, true);
        verdict(&w, exit, "inconclusive", 2);
        for (id, n) in [(KERNEL, 4), (ROUND2, 3), (ROUND3, 2)] {
            fields(
                row(&w, id),
                json!({"reason":reason,"coverage":"partial","n":n,"unknown":1}),
            );
        }
    }
}

#[test]
fn r04_two_sites_r05_multiple_and_r06_r07_unidentifiable() {
    for (model, reason) in [
        (two_sites(), "ambiguous_call_site"),
        (skipped(), "round_not_identifiable"),
        (guarded(), "round_not_identifiable"),
    ] {
        let (w, exit) = looping(&model, &loop_log(), true);
        verdict(&w, exit, "unsupported", 4);
        fields(
            row(&w, KERNEL),
            json!({"status":"not_derivable","reason":reason,"candidate":null,"interval":null,"rounds":null,
            "occurrences_per_case":[{"observations":1,"cases":1},{"observations":2,"cases":1},{"observations":3,"cases":2}]}),
        );
    }
    let mut log = baseline();
    log.extend(task(0, 1, 0, "pass"));
    let (w, exit) = ordinary(BASE, &log, |_| {});
    verdict(&w, exit, "inconclusive", 2);
    fields(
        row(&w, KERNEL),
        json!({"status":"withheld","reason":"multiple_observations_per_case","candidate":null}),
    );
}

#[test]
fn i01_input_and_truncation() {
    for (fault, expected) in [
        ("missing", "spec_bad_field"),
        ("unknown", "spec_bad_field"),
        ("hash", "spec_hash_mismatch"),
    ] {
        let (w, exit) = ordinary(BASE, &baseline(), |s| match fault {
            "missing" => {
                s.as_object_mut().unwrap().remove("confidence");
            }
            "unknown" => s["default_please"] = json!(true),
            "hash" => s["log_sha256"] = json!("0".repeat(64)),
            _ => unreachable!(),
        });
        verdict(&w, exit, "invalid", 3);
        fields(
            &w,
            json!({"reason":expected,"slots":[],"assumption_records":[]}),
        );
    }
    let (w, exit) = invoke(
        BASE,
        &baseline(),
        &[KERNEL, DURATION],
        "1079/2048",
        |_| {},
        true,
    );
    verdict(&w, exit, "inconclusive", 2);
    fields(
        &w,
        json!({"reason":"truncated","truncated":true,"assumption_records":[]}),
    );
    assert_eq!(w["log"]["sha256"], Value::Null);
    for id in [KERNEL, DURATION] {
        fields(
            row(&w, id),
            json!({"status":"withheld","reason":"truncated","n":null,"unknown":null,"censored":null,
            "counts":null,"candidate":null,"interval":null,"provenance":null,"sensitivity_points":null,"duration_summary":null}),
        );
    }
}

#[test]
fn revise3_integer_precision_cannot_zero_an_observed_branch() {
    let (w, exit) = ordinary(BASE, &baseline(), |s| s["decimals"] = json!(0));
    let k = row(&w, KERNEL);
    fields(
        k,
        json!({"reason":"dsl_precision_insufficient","status":"withheld","coverage":"partial",
        "candidate":[{"result":"pass","exact":"1/2","dsl":null},{"result":"fail","exact":"1/2","dsl":null}],
        "interval":equal_four(),"rounding_error_bound":null}),
    );
    assert_eq!(
        row(&w, DURATION)["candidate"],
        json!([{"minutes":1,"exact":"1","dsl":"1"}])
    );
    verdict(&w, exit, "inconclusive", 2);
}

#[test]
fn revise3_decimal_precision_preserves_rare_result_and_duration() {
    let mut log = records(&[1; 30], 2);
    for finish in log[2..].iter_mut().step_by(2) {
        finish["result"] = json!("pass");
    }
    log[60]["result"] = json!("fail");
    log[60]["time"] = json!({"earliest":2,"latest":2});
    let (w, exit) = ordinary(BASE, &log, |s| s["decimals"] = json!(1));
    for (id, candidate) in [
        (
            KERNEL,
            json!([{"result":"pass","exact":"29/30","dsl":null},{"result":"fail","exact":"1/30","dsl":null}]),
        ),
        (
            DURATION,
            json!([{"minutes":1,"exact":"29/30","dsl":null},{"minutes":2,"exact":"1/30","dsl":null}]),
        ),
    ] {
        fields(
            row(&w, id),
            json!({"reason":"dsl_precision_insufficient","status":"withheld","coverage":"partial",
            "n":30,"unknown":0,"censored":0,"candidate":candidate,"rounding_error_bound":null}),
        );
    }
    verdict(&w, exit, "inconclusive", 2);
}

#[test]
fn revise3_minimum_uses_known_cases_not_all_starts() {
    let mut log = baseline();
    log.remove(8);
    log.remove(6);
    let (w, exit) = ordinary(BASE, &log, |s| s["min_samples"] = json!(4));
    verdict(&w, exit, "inconclusive", 2);
    for id in [KERNEL, DURATION] {
        fields(
            row(&w, id),
            json!({"reason":"insufficient_samples","status":"withheld","coverage":"partial",
            "n":4,"unknown":2,"censored":2,"candidate":null}),
        );
    }
    fields(
        row(&w, KERNEL),
        json!({"counts":counts(1,1),
        "interval":interval(("21313/1048576","1027263/1048576"),("21313/1048576","1027263/1048576"))}),
    );
    let (w, exit) = ordinary(BASE, &log, |s| s["min_samples"] = json!(2));
    verdict(&w, exit, "inconclusive", 2);
    assert_eq!(
        row(&w, KERNEL)["candidate"],
        mass("1/2", "1/2", "0.50", "0.50")
    );
    assert_eq!(row(&w, DURATION)["reason"], "censored");
}

#[test]
fn revise3_rounds_are_identified_with_overlapping_arrivals() {
    let model = repeat(true).replace(
        "bounds { cases 1; live_tasks 1",
        "arrivals fixed [0min,0min]; bounds { cases 2; live_tasks 2",
    );
    let ast = relca_core::syntax::parse(&model).unwrap();
    let lowered = relca_core::lower::lower(&ast).unwrap();
    assert_eq!(
        lowered.bounds.cases, 2,
        "the estimator receives the pre-execution IR"
    );
    assert!(lowered.resource_time.is_none());
    let (w, exit) = looping(&model, &loop_log(), true);
    fields(
        row(&w, ROUND2),
        json!({"status":"derived","n":3,"unknown":0,"counts":counts(2,1),
        "candidate":mass("2/3","1/3","0.67","0.33")}),
    );
    verdict(&w, exit, "satisfied", 0);
}

#[test]
fn revise3_no_argument_usage_lists_estimate() {
    let out = Command::new(env!("CARGO_BIN_EXE_relca")).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(out.stderr, b"usage: relca <check|diff|verify|simulate|compare|explain|mutate|sensitivity|conform|estimate> ...\n");
}
