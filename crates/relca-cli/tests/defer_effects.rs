//! ADR 0077 acceptance, intentionally RED before implementation.
//! Expectations were fixed from the ADR and defer_effects_calculations.md.
#[allow(dead_code)]
#[path = "support/approval_inputs.rs"]
mod approval_input;
#[allow(dead_code)]
#[path = "support/log_conformance_inputs.rs"]
mod conformance_input;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "support/defer_effects_inputs.rs"]
mod h;
#[path = "support/test_tmp.rs"]
mod test_tmp;

use serde_json::{Value, json};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
fn each<T: std::fmt::Debug>(cases: impl IntoIterator<Item = T>, mut check: impl FnMut(T)) {
    let mut failures = Vec::new();
    for case in cases {
        let label = format!("{case:?}");
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(case))).is_err() {
            failures.push(label);
        }
    }
    assert!(failures.is_empty(), "failed matrix cells: {failures:?}");
}
struct Run {
    v: Value,
    err: String,
    exit: i32,
}
fn write(name: &str, text: &str) -> PathBuf {
    let dir = test_tmp::root().join(format!("adr0077-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{}-{name}", NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&path, text).unwrap();
    path
}
fn run(src: &str, command: &str, extra: &[&str]) -> Run {
    let path = write("model.relca", src);
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(path)
        .args(extra)
        .output()
        .unwrap();
    Run {
        v: serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout))),
        err: String::from_utf8(out.stderr).unwrap(),
        exit: out.status.code().unwrap(),
    }
}
fn row<'a>(rows: &'a Value, key: &str, name: &str) -> &'a Value {
    let rows: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r[key] == name)
        .collect();
    assert_eq!(rows.len(), 1, "{key}={name}");
    rows[0]
}
fn scalar(n: &str, sim: bool) -> Value {
    if sim {
        json!({"mean":n,"min":n,"max":n,"samples":3,"reason":null})
    } else {
        json!({"min":n,"max":n,"reason":null})
    }
}
fn absent(sim: bool) -> Value {
    if sim {
        json!({"mean":null,"min":null,"max":null,"samples":0,"reason":"not_configured"})
    } else {
        json!({"min":null,"max":null,"reason":"not_configured"})
    }
}
fn effects(report: &Value, expected: Option<&Value>) {
    match expected {
        Some(e) => {
            assert_eq!(report["cut"]["defer_effects"], *e, "ADR0077 defer_effects");
            let entries = e["entries"].as_array().unwrap();
            assert_eq!(
                report["cut"]["defer"].as_array().unwrap().len(),
                entries.len()
            );
            for (i, entry) in entries.iter().enumerate() {
                assert_eq!(entry["target"], report["cut"]["defer"][i]["target"]);
            }
            assert_eq!(
                report["cut"]["checked"].as_array().unwrap().last(),
                Some(&json!("deferred_metrics"))
            );
            assert!(
                !report["cut"]["unchecked"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("deferred_metrics"))
            );
        }
        None => {
            assert!(
                report["cut"].get("defer_effects").is_none(),
                "inapplicable field must be absent, not null"
            );
            assert!(
                !report["cut"]["checked"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("deferred_metrics"))
            );
            assert!(
                report["cut"]["unchecked"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("deferred_metrics"))
            );
        }
    }
}
fn warning_rows(report: &Value, src: &str, targets: &[&str]) {
    let actual: Vec<_> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "W-CUT-2")
        .cloned()
        .collect();
    let mut offset = 0;
    let expected: Vec<_> = targets.iter().map(|target| {
        let needle = format!("defer {target} reason");
        let start = offset + src[offset..].find(&needle).expect("target occurrence") + "defer ".len();
        offset = start + target.len();
        json!({"code":"W-CUT-2","severity":"warning","message":h::warning(target),"span":{"start":start,"end":offset}})
    }).collect();
    assert_eq!(actual, expected, "W-CUT-2 code/message/target byte span");
    assert!(
        !report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "W-CUT-3")
    );
}
fn stderr_warnings(err: &str, prefixed: &[(&str, &[&str])]) {
    let expected: Vec<_> = prefixed
        .iter()
        .flat_map(|(prefix, targets)| {
            targets
                .iter()
                .map(move |t| format!("{prefix}W-CUT-2: {}", h::warning(t)))
        })
        .collect();
    let actual: Vec<_> = err
        .lines()
        .filter(|line| line.contains("W-CUT-2:"))
        .collect();
    assert_eq!(
        actual, expected,
        "stderr W-CUT-2 exact lines/prefix/multiplicity"
    );
    assert!(!err.contains("W-CUT-3:"));
}
fn k_targets(k: u8) -> &'static [&'static str] {
    match k {
        1 | 7 => &["Ghost"],
        2 => &["A.nosuch"],
        _ => &[],
    }
}
fn legacy_bytes(id: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest: Value = serde_json::from_str(include_str!("snapshots/jobs.json")).unwrap();
    let job = row(&manifest, "id", id);
    let args: Vec<_> = job["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    let live = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .current_dir(&root)
        .output()
        .unwrap();
    let saved = Command::new("gzip")
        .arg("-dc")
        .arg(root.join(format!("crates/relca-cli/tests/snapshots/{id}.snap.gz")))
        .output()
        .unwrap();
    assert!(saved.status.success());
    let at = saved.stdout.iter().position(|b| *b == b'\n').unwrap();
    let lengths: Vec<usize> = std::str::from_utf8(&saved.stdout[..at])
        .unwrap()
        .split_whitespace()
        .map(|s| s.parse().unwrap())
        .collect();
    assert_eq!(lengths.len(), 3);
    let data = &saved.stdout[at + 1..];
    assert_eq!(data.len(), lengths.iter().sum::<usize>());
    assert_eq!(live.stdout, data[..lengths[0]], "legacy stdout {id}");
    assert_eq!(
        live.stderr,
        data[lengths[0]..lengths[0] + lengths[1]],
        "legacy stderr {id}"
    );
    assert_eq!(
        format!("{}\n", live.status.code().unwrap()).as_bytes(),
        &data[lengths[0] + lengths[1]..],
        "legacy exit {id}"
    );
}
#[test]
fn legacy_no_defer_stdout_stderr_exit_are_byte_identical() {
    legacy_bytes("fixture-met-serial");
}
#[test]
fn legacy_no_indicators_stdout_stderr_exit_are_byte_identical() {
    legacy_bytes("fixture-cut-declared");
}
fn values(v: &Value, k: u8, sim: bool) {
    assert_eq!(v["verdict"], "satisfied");
    if k == 6 {
        assert!(v.get("metrics").is_none());
        assert!(v.get("quality").is_none());
        return;
    }
    let withdrawn = matches!(k, 1 | 2 | 3 | 5 | 7);
    let m = &v["metrics"];
    assert_eq!(
        m["overall"]["work"],
        if withdrawn {
            absent(sim)
        } else {
            scalar(if k == 4 { "3" } else { "5" }, sim)
        }
    );
    assert_eq!(
        m["overall"]["observed_cost_total"],
        if withdrawn {
            absent(sim)
        } else {
            scalar(if k == 4 { "31" } else { "73" }, sim)
        }
    );
    assert_eq!(
        row(&m["actors"], "actor", "A")["work"],
        if withdrawn {
            absent(sim)
        } else {
            scalar("3", sim)
        }
    );
    assert_eq!(
        row(&m["people"], "person", "q")["work"],
        if k == 0 {
            scalar("2", sim)
        } else {
            absent(sim)
        }
    );
    assert_eq!(
        row(&m["resources"], "resource", "Desk")["occupancy_time"],
        if k == 0 {
            scalar("2", sim)
        } else {
            absent(sim)
        }
    );
    if k == 4 || k == 7 {
        assert!(
            !m["actors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["actor"] == "R")
        );
        assert!(
            !m["capabilities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["capability"] == "R.y")
        );
    }
    assert_eq!(
        row(&m["cases"][0]["tasks"], "binding", "a")["starts"],
        scalar("1", sim)
    );
    assert_eq!(m["overall"]["counts"]["offered"]["stats"], scalar("1", sim));
    let q = &v["quality"];
    let rate = &q["cohort"][0]["value"];
    let p95 = &q["quantiles"][0]["value"];
    if withdrawn {
        if sim {
            assert!(rate["point"].is_null());
            assert!(p95["point"].is_null());
            assert_eq!(q["properties"][0]["cases"][0]["trials"], 3);
            assert!(q["properties"][0]["cases"][0].get("reason").is_none());
        } else {
            let n = json!({"lower":null,"upper":null,"basis":null,"reason":"not_configured"});
            assert_eq!(*rate, n);
            assert_eq!(*p95, n);
            assert!(q["properties"][0]["cases"][0]["min"].is_null());
            assert_eq!(q["properties"][0]["cases"][0]["reason"], "not_configured");
        }
        assert_eq!(rate["reason"], "not_configured");
        assert_eq!(p95["reason"], "not_configured");
        assert!(q["quantiles"][0]["lower_bound"].is_null());
        assert_eq!(
            q["properties"][0]["expected_count"]["reason"],
            "not_configured"
        );
    } else {
        let field = if sim { "point" } else { "lower" };
        assert_eq!(rate[field], "1");
        assert_eq!(rate["reason"], Value::Null);
        assert_eq!(p95[field], if k == 4 { "3" } else { "5" });
        assert_eq!(p95["reason"], Value::Null);
    }
    fixture_match::validate_deferred_metrics_wire(v).unwrap();
}

macro_rules! inline_case {
    ($name:ident, $k:literal) => {
        #[test]
        fn $name() {
            let src = h::source($k);
            let r = run(&src, "check", &[]);
            assert_eq!(r.exit, 0);
            values(&r.v, $k, false);
            warning_rows(&r.v, &src, k_targets($k));
            stderr_warnings(&r.err, &[("", k_targets($k))]);
            effects(&r.v, h::expected($k).as_ref());
        }
    };
}
inline_case!(k0_legacy_no_defer, 0);
inline_case!(k1_opaque, 1);
inline_case!(k2_missing_capability_hint, 2);
inline_case!(k3_called_capability, 3);
inline_case!(k4_uncalled_scope, 4);
inline_case!(k5_actor_capabilities_name_sorted, 5);
inline_case!(k6_legacy_no_indicators, 6);
inline_case!(k7_per_entry_not_union, 7);

#[test]
fn v1_withdrawal_and_reason_are_unchanged_in_all_three_commands() {
    for k in [0, 1, 2, 3, 4, 5, 6, 7] {
        for command in ["check", "explain", "simulate"] {
            let r = run(&h::source(k), command, &[]);
            assert_eq!(r.exit, 0);
            values(&r.v, k, command == "simulate");
        }
    }
}

#[test]
fn e1_legacy_inputs_omit_the_field_and_warning_in_every_report_wrapper() {
    for k in [0, 6] {
        let src = h::source(k);
        for command in ["check", "explain", "simulate", "mutate"] {
            let r = run(&src, command, &[]);
            let v = if command == "mutate" {
                &r.v["original"]
            } else {
                &r.v
            };
            effects(v, None);
            warning_rows(v, &src, &[]);
            stderr_warnings(&r.err, &[]);
            assert_eq!(
                v["cut"]["defer"],
                if k == 0 {
                    json!([])
                } else {
                    json!([{"target":"Ghost","actor":false,"reason":"later","loses":"load"}])
                }
            );
            // B2 must not add resolution/effect fields to the declaration rows.
            assert_eq!(v["cut"].as_object().unwrap().len(), 9);
        }
        let i = src.rfind('}').unwrap();
        let src = format!(
            "{}\nconfiguration Left {{ cut C; process P }}\nconfiguration Right {{ cut C; process P }}\n}}",
            &src[..i]
        );
        for mode in ["exhaustive", "simulation"] {
            let r = run(&src, "compare", &["--plans", "Left,Right", "--mode", mode]);
            assert_eq!(r.v["plans"].as_array().unwrap().len(), 2);
            for p in r.v["plans"].as_array().unwrap() {
                effects(&p["report"], None);
                warning_rows(&p["report"], &src, &[]);
            }
            stderr_warnings(&r.err, &[]);
        }
        let r = run(&src, "diff", &["--from", "Left", "--to", "Right"]);
        for side in ["from", "to"] {
            effects(&r.v[side]["report"], None);
            warning_rows(&r.v[side]["report"], &src, &[]);
        }
        stderr_warnings(&r.err, &[]);
    }
}
fn single_command(command: &str) {
    let src = h::source(1);
    let r = run(&src, command, &[]);
    assert_eq!(r.exit, if command == "mutate" { 4 } else { 0 });
    let v = if command == "mutate" {
        &r.v["original"]
    } else {
        &r.v
    };
    warning_rows(v, &src, &["Ghost"]);
    stderr_warnings(
        &r.err,
        &[(
            if command == "mutate" {
                "original: "
            } else {
                ""
            },
            &["Ghost"],
        )],
    );
    effects(v, h::expected(1).as_ref());
    if command == "simulate" {
        let typed: relca_core::report::SimulationReport =
            serde_json::from_value(v.clone()).unwrap();
        assert_eq!(json!(typed), *v);
    } else if command != "explain" {
        let typed: relca_core::report::Report = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(json!(typed), *v);
    }
}
macro_rules! command_case {
    ($name:ident, $command:literal) => {
        #[test]
        fn $name() {
            single_command($command);
        }
    };
}
command_case!(c1_check, "check");
command_case!(c1_explain, "explain");
command_case!(c1_simulate, "simulate");
command_case!(c1_mutate_original, "mutate");
#[test]
fn e1_metrics_only_and_quality_only_are_both_eligible() {
    let src = h::source(1);
    let quality_only = src
        .lines()
        .filter(|s| !s.starts_with("metrics {"))
        .collect::<Vec<_>>()
        .join("\n");
    let metrics_only = src
        .lines()
        .filter(|s| !s.starts_with("property ") && !s.starts_with("metric "))
        .collect::<Vec<_>>()
        .join("\n");
    each(
        [&quality_only, &metrics_only]
            .into_iter()
            .flat_map(|s| ["check", "explain", "simulate"].map(move |command| (s, command))),
        |(s, command)| {
            let r = run(s, command, &[]);
            assert_eq!(r.exit, 0);
            warning_rows(&r.v, s, &["Ghost"]);
            stderr_warnings(&r.err, &[("", &["Ghost"])]);
            effects(&r.v, h::expected(1).as_ref());
        },
    );
}
fn selected_cut(plan: &str) {
    let src = h::selected_cuts();
    each(["check", "explain", "simulate"], |command| {
        let r = run(&src, command, &["--configuration", plan]);
        assert_eq!(r.exit, 0);
        let targets: &[&str] = if plan == "Clean" { &[] } else { &["Ghost"] };
        warning_rows(&r.v, &src, targets);
        stderr_warnings(&r.err, &[("", targets)]);
        effects(
            &r.v,
            h::expected(if plan == "Clean" { 0 } else { 1 }).as_ref(),
        );
    });
}
#[test]
fn u2_selected_clean_cut_does_not_warn_for_other_cut() {
    selected_cut("Clean");
}
#[test]
fn u2_selected_opaque_cut_warns() {
    selected_cut("Hidden");
}
#[test]
fn u1_repeated_opaque_targets_keep_entry_and_span_multiplicity() {
    let src = h::source(1).replace(
        "defer Ghost reason",
        "defer Ghost reason \"最初\" loses \"load\";\ndefer Ghost reason",
    );
    let r = run(&src, "check", &[]);
    assert_eq!(r.exit, 0);
    warning_rows(&r.v, &src, &["Ghost", "Ghost"]);
    stderr_warnings(&r.err, &[("", &["Ghost", "Ghost"])]);
    effects(
        &r.v,
        Some(&h::effects(vec![h::opaque("Ghost"), h::opaque("Ghost")])),
    );
}
#[test]
fn u3_empty_actor_has_none() {
    let empty = h::source(0)
        .replace("cut C {", "actor Empty { source document }\ncut C {")
        .replace(
            "include [A,R];",
            "include [A,R,Empty]; defer Empty reason \"later\" loses \"load\";",
        );
    let r = run(&empty, "check", &[]);
    assert_eq!(r.exit, 0);
    warning_rows(&r.v, &empty, &[]);
    effects(
        &r.v,
        Some(&h::effects(vec![
            json!({"target":"Empty","resolution":"actor","capabilities":[],"withdraws":"none","because":[],"scope":null}),
        ])),
    );
}
#[test]
fn u3_dead_branch_is_lexical_call() {
    let dead = h::source(3).replace(
        "r = R.y(); end done",
        "if a == fail { r = R.y(); end other } else { end done }",
    );
    let r = run(&dead, "check", &[]);
    assert_eq!(r.exit, 0);
    warning_rows(&r.v, &dead, &[]);
    effects(&r.v, h::expected(3).as_ref());
    assert_eq!(r.v["metrics"]["overall"]["work"], absent(false));
}

fn invoice_values(v: &Value, variant: &str, plan: &str) {
    assert_eq!(
        v["verdict"],
        if plan == "ToBeA" {
            "violated"
        } else {
            "satisfied"
        }
    );
    let withdrawn =
        variant == "P0" || variant == "Pb" || variant == "Pc" || variant == "Pd" && plan == "ToBeA";
    let cohort = &v["quality"]["cohort"][0]["value"];
    if withdrawn {
        assert_eq!(
            *cohort,
            json!({"lower":null,"upper":null,"basis":null,"reason":"not_configured"})
        );
        assert_eq!(v["metrics"]["overall"]["named_people_work"], absent(false));
        for q in v["quality"]["quantiles"].as_array().unwrap() {
            assert_eq!(
                q["value"],
                json!({"lower":null,"upper":null,"basis":null,"reason":"not_configured"})
            );
            assert!(q["lower_bound"].is_null());
        }
    } else {
        let (wrong, p95, min, max) = match plan {
            "AsIs" => ("93/125000", "17", "14", "17"),
            "ToBeP" => ("93/125000", "14", "14", "17"),
            "ToBeA" => ("231/125000", "9", "8", "11"),
            _ => unreachable!(),
        };
        assert_eq!(
            *cohort,
            json!({"lower":wrong,"upper":wrong,"basis":"exact","reason":null})
        );
        for q in v["quality"]["quantiles"].as_array().unwrap() {
            assert_eq!(
                q["value"],
                json!({"lower":p95,"upper":p95,"basis":"exact","reason":null})
            );
        }
        assert_eq!(
            v["metrics"]["overall"]["named_people_work"],
            json!({"min":min,"max":max,"reason":null})
        );
        // Missing rates are not made free when defer is removed.
        assert_eq!(
            v["metrics"]["overall"]["observed_cost_total"],
            absent(false)
        );
    }
}
fn invoice_targets(variant: &str) -> &'static [&'static str] {
    match variant {
        "P0" => &["Resubmission", "Vendor.contest_rate"],
        "Pb" => &["Resubmission"],
        _ => &[],
    }
}
macro_rules! invoice_case {
    ($name:ident, $variant:literal, $plan:literal) => {
        #[test]
        fn $name() {
            let src = h::invoice($variant);
            let r = run(&src, "check", &["--configuration", $plan]);
            assert_eq!(r.exit, if $plan == "ToBeA" { 1 } else { 0 });
            invoice_values(&r.v, $variant, $plan);
            warning_rows(&r.v, &src, invoice_targets($variant));
            stderr_warnings(&r.err, &[("", invoice_targets($variant))]);
            effects(&r.v, h::invoice_expected($variant, $plan).as_ref());
        }
    };
}
invoice_case!(p0_asis, "P0", "AsIs");
invoice_case!(p0_tobep, "P0", "ToBeP");
invoice_case!(p0_tobea, "P0", "ToBeA");
invoice_case!(pa_asis, "Pa", "AsIs");
invoice_case!(pa_tobep, "Pa", "ToBeP");
invoice_case!(pa_tobea, "Pa", "ToBeA");
invoice_case!(pb_asis, "Pb", "AsIs");
invoice_case!(pb_tobep, "Pb", "ToBeP");
invoice_case!(pb_tobea, "Pb", "ToBeA");
invoice_case!(pc_asis, "Pc", "AsIs");
invoice_case!(pc_tobep, "Pc", "ToBeP");
invoice_case!(pc_tobea, "Pc", "ToBeA");
invoice_case!(pd_asis, "Pd", "AsIs");
invoice_case!(pd_tobep, "Pd", "ToBeP");
invoice_case!(pd_tobea, "Pd", "ToBeA");

#[test]
fn c1_invoice_compare_six_lines() {
    let src = h::invoice("P0");
    let r = run(
        &src,
        "compare",
        &["--plans", "AsIs,ToBeP,ToBeA", "--mode", "exhaustive"],
    );
    assert_eq!(r.exit, 1); // ToBeA's probability violation outranks the inconclusive quality hard.
    stderr_warnings(
        &r.err,
        &[
            ("configuration AsIs: ", invoice_targets("P0")),
            ("configuration ToBeP: ", invoice_targets("P0")),
            ("configuration ToBeA: ", invoice_targets("P0")),
        ],
    );
    for (i, plan) in ["AsIs", "ToBeP", "ToBeA"].iter().enumerate() {
        let v = &r.v["plans"][i]["report"];
        warning_rows(v, &src, invoice_targets("P0"));
        effects(v, h::invoice_expected("P0", plan).as_ref());
    }
}
#[test]
fn c1_invoice_diff_four_lines() {
    let src = h::invoice("P0");
    let r = run(&src, "diff", &["--from", "AsIs", "--to", "ToBeA"]);
    assert_eq!(r.exit, 1);
    stderr_warnings(
        &r.err,
        &[
            ("from: ", invoice_targets("P0")),
            ("to: ", invoice_targets("P0")),
        ],
    );
    for side in ["from", "to"] {
        let v = &r.v[side]["report"];
        warning_rows(v, &src, invoice_targets("P0"));
        effects(v, h::invoice_expected("P0", "AsIs").as_ref());
    }
}
fn comparison_values(v: &Value, opaque: bool) {
    for i in 0..2 {
        let withdrawn = opaque || i == 0;
        let c = &v["evaluation"]["hard"][0]["values"][i];
        assert_eq!(
            c["verdict"],
            if withdrawn {
                "inconclusive"
            } else {
                "satisfied"
            }
        );
        assert_eq!(
            c["reason"],
            if withdrawn {
                json!("not_configured")
            } else {
                Value::Null
            }
        );
        if withdrawn {
            for key in ["interval", "point", "basis"] {
                assert!(c[key].is_null(), "{key}: {c}");
            }
        }
        for o in v["evaluation"]["objectives"].as_array().unwrap() {
            let c = &o["values"][i];
            assert_eq!(
                c["reason"],
                if withdrawn {
                    json!("not_configured")
                } else {
                    Value::Null
                }
            );
            if withdrawn {
                for key in ["lower", "upper", "point", "basis"] {
                    assert!(c[key].is_null(), "{key}: {c}");
                }
            }
        }
    }
}
#[test]
fn v1_evaluation_does_not_restore_withdrawn_hard_or_objectives() {
    for opaque in [false, true] {
        for mode in ["exhaustive", "simulation"] {
            let r = run(
                &h::comparison(opaque),
                "compare",
                &["--plans", "Included,Unrelated", "--mode", mode],
            );
            assert_eq!(r.exit, 2);
            comparison_values(&r.v, opaque);
        }
    }
}
#[test]
fn c1_compare_modes_transport_per_plan_effects() {
    each(
        [false, true]
            .into_iter()
            .flat_map(|opaque| ["exhaustive", "simulation"].map(move |mode| (opaque, mode))),
        |(opaque, mode)| {
            let src = h::comparison(opaque);
            let r = run(
                &src,
                "compare",
                &["--plans", "Included,Unrelated", "--mode", mode],
            );
            assert_eq!(r.exit, 2);
            let targets: &[&str] = if opaque { &["Ghost"] } else { &[] };
            stderr_warnings(
                &r.err,
                &[
                    ("configuration Included: ", targets),
                    ("configuration Unrelated: ", targets),
                ],
            );
            each(0..2, |i| {
                let v = &r.v["plans"][i]["report"];
                warning_rows(v, &src, targets);
                effects(
                    v,
                    h::expected(if opaque {
                        1
                    } else if i == 0 {
                        3
                    } else {
                        4
                    })
                    .as_ref(),
                );
            });
        },
    );
}

#[test]
fn wire_declared_cut_round_trip_accepts_all_effect_fields_and_remains_closed() {
    let mut cut = json!({"name":"C","question":null,"include":["A","R"],"exclude":[],"defer":[{"target":"Ghost","actor":false,"reason":"later","loses":"load"},{"target":"R.y","actor":false,"reason":"later","loses":"load"}],"externality":[],"reentry":[],"checked":["actor_membership","excluded_call","missing_loses","deferred_metrics"],"unchecked":["party_contest_path","retired_capability","comparison_mismatch"],"defer_effects":h::expected(7).unwrap()});
    let typed: relca_core::report::DeclaredCut =
        serde_json::from_value(cut.clone()).expect("new DeclaredCut field must deserialize");
    assert_eq!(json!(typed), cut);
    cut["unknown"] = json!(true);
    assert!(serde_json::from_value::<relca_core::report::DeclaredCut>(cut).is_err());
}
#[test]
fn wire_shared_validator_rejects_wrong_effects_not_just_withdrawn_values() {
    let src = h::source(7);
    let mut good = run(&src, "check", &[]).v;
    // New fields are hand-assembled, not learned from the running binary.
    good["cut"]["defer_effects"] = h::expected(7).unwrap();
    fixture_match::validate_deferred_metrics_wire(&good).unwrap();
    for (pointer, value) in [
        ("/cut/defer_effects/process", json!(false)),
        ("/cut/defer_effects/entries/0/resolution", json!("actor")),
        ("/cut/defer_effects/entries/0/target", json!("Other")),
        ("/cut/defer_effects/entries/0/because", json!([])),
        ("/cut/defer_effects/entries/1/withdraws", json!("process")),
        ("/cut/defer_effects/entries/1/capabilities", json!([])),
        ("/cut/defer_effects/entries/1/scope/actors", json!([])),
        ("/cut/defer_effects/entries/1/scope/people", json!([])),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            fixture_match::validate_deferred_metrics_wire(&bad).is_err(),
            "shared validator must reject {pointer}"
        );
    }
    let mut bad = good.clone();
    bad["cut"].as_object_mut().unwrap().remove("defer_effects");
    assert!(
        fixture_match::validate_deferred_metrics_wire(&bad).is_err(),
        "eligible wire must carry effects"
    );
    let typed: relca_core::report::Report = serde_json::from_value(good.clone()).unwrap();
    assert_eq!(json!(typed), good);
}
#[test]
fn diag_vocabulary_registers_w_cut_2_as_warning() {
    assert!(relca_core::diag::CODES.contains(&"W-CUT-2"));
    assert_eq!(
        relca_core::diag::severity_of("W-CUT-2"),
        Some(relca_core::diag::Severity::Warning)
    );
}

#[test]
fn c1_sensitivity_point_reports_in_both_modes() {
    let src = h::source(1).replace(
        "verify reachable end.done",
        "verify probability reached end.done >= 0.9",
    );
    let model = write("point.relca", &src);
    each(["exhaustive", "simulation"], |mode| {
        let manifest = json!({"schema":"relca-sensitivity/v1","scope":{"kind":"enumerated_models","rationale":"one fixed hand point"},"points":[{"id":"opaque","model":model.file_name().unwrap().to_str().unwrap(),"configuration":null,"scenario":null}],"targets":[{"kind":"probability_query","id":"probability reached #1"}],"confidence":if mode == "simulation" {json!("1/2")} else {Value::Null}});
        let r = run(&manifest.to_string(), "sensitivity", &["--mode", mode]);
        assert_eq!(r.v["points"][0]["status"], "evaluated", "{}", r.err);
        let v = &r.v["points"][0]["report"];
        assert_eq!(v["cut"]["name"], "C");
        warning_rows(v, &src, &["Ghost"]);
        stderr_warnings(&r.err, &[("point opaque: ", &["Ghost"])]);
        effects(v, h::expected(1).as_ref());
    });
}

#[test]
fn c1_mutation_reports_with_a_real_generated_mutant() {
    let src = h::source(1)
        .replace("actor A {", "actor Doc { source document }\nactor A {")
        .replace(
            "capability x() -> V {",
            "capability x() -> V { reads [Doc];",
        )
        .replace("include [A,R]", "include [A,R,Doc]");
    let r = run(
        &src,
        "mutate",
        &["--kinds", "evidence_missing", "--mutant-limit", "1"],
    );
    let mutants: Vec<_> = r.v["mutations"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|k| k["mutants"].as_array().unwrap())
        .filter(|m| m["report"].is_object())
        .collect();
    assert_eq!(mutants.len(), 1, "a real mutant report is required");
    let prefix = format!("{}: ", mutants[0]["id"].as_str().unwrap());
    stderr_warnings(&r.err, &[("original: ", &["Ghost"]), (&prefix, &["Ghost"])]);
    for v in [&r.v["original"], &mutants[0]["report"]] {
        warning_rows(v, &src, &["Ghost"]);
        effects(v, h::expected(1).as_ref());
        assert_eq!(v["metrics"]["overall"]["work"], absent(false));
    }
}

fn no_new_conform_fields(v: &Value) {
    match v {
        Value::Object(o) => {
            assert!(
                !o.contains_key("defer_effects"),
                "conform must not acquire analysis effects"
            );
            assert_ne!(o.get("code"), Some(&json!("W-CUT-2")));
            for child in o.values() {
                no_new_conform_fields(child);
            }
        }
        Value::Array(a) => {
            for child in a {
                no_new_conform_fields(child);
            }
        }
        _ => {}
    }
}
fn digest(text: &str) -> String {
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
    let o = child.wait_with_output().unwrap();
    assert!(o.status.success());
    String::from_utf8(o.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .into()
}
#[test]
fn c1_conform_event_contracts_excludes_effects_and_warning_even_with_indicators() {
    let mut c = conformance_input::case("l01_serial_ok");
    c.model = c.model.replace(
        "include [Clerk,Approver,Poster]",
        "include [Clerk,Approver,Poster]; defer Ghost reason \"later\" loses \"load\"",
    );
    c.model = c.model.replace("bounds {", "metrics { window 30000min; currency \"JPY\" }\nproperty case.Done = reached(end.done)\nbounds {");
    let log = write("events.jsonl", &c.log());
    let r = run(
        &c.model,
        "conform",
        &["--log", log.to_str().unwrap(), "--scope", "event-contracts"],
    );
    assert_eq!(r.exit, 0, "{}", r.err);
    assert_eq!(r.v["conformance"], "conformant");
    no_new_conform_fields(&r.v);
    stderr_warnings(&r.err, &[]);
}
#[test]
fn c1_conform_approval_burden_excludes_effects_and_warning_even_with_indicators() {
    let mut c = approval_input::base("adr0077");
    c.model = c.model.replace(
        "include [P,A,R]",
        "include [P,A,R]; defer Ghost reason \"later\" loses \"load\"",
    );
    c.model = c.model.replace("bounds {", "metrics { window 30000min; currency \"JPY\" }\nproperty case.Done = reached(end.done)\nbounds {");
    let log_text = c.log();
    c.spec["model_sha256"] = json!(digest(&c.model));
    c.spec["log_sha256"] = json!(digest(&log_text));
    let log = write("events.jsonl", &log_text);
    let spec = write("approval.json", &c.spec.to_string());
    let r = run(
        &c.model,
        "conform",
        &[
            "--log",
            log.to_str().unwrap(),
            "--scope",
            "approval-burden",
            "--approval-spec",
            spec.to_str().unwrap(),
        ],
    );
    assert_eq!(r.exit, 0, "{}", r.err);
    no_new_conform_fields(&r.v);
    stderr_warnings(&r.err, &[]);
    assert_eq!(r.v["approval_burden"]["rate"]["value"], "2/3");
}

#[test]
fn five_fault_pairs_are_rejected_by_the_hand_oracles() {
    // (1) Even a resolved capability which withdraws the process must not warn.
    let src = h::source(3);
    let mut good = run(&src, "check", &[]).v;
    good["cut"]["defer_effects"] = h::expected(3).unwrap();
    warning_rows(&good, &src, &[]);
    let mut bad = good.clone();
    bad["diagnostics"].as_array_mut().unwrap().push(
        json!({"code":"W-CUT-2","severity":"warning","message":h::warning("R.y"),"span":null}),
    );
    assert!(
        std::panic::catch_unwind(|| warning_rows(&bad, &src, &[])).is_err(),
        "fault 1"
    );
    // (2) A union-wide process flag must not be copied to every entry.
    let src = h::source(7);
    let mut good = run(&src, "check", &[]).v;
    good["cut"]["defer_effects"] = h::expected(7).unwrap();
    effects(&good, h::expected(7).as_ref());
    let mut bad = good.clone();
    bad["cut"]["defer_effects"]["entries"][1]["withdraws"] = json!("process");
    assert!(
        std::panic::catch_unwind(|| effects(&bad, h::expected(7).as_ref())).is_err(),
        "fault 2"
    );
    // (3) The process flag must agree with hard/objective withdrawal, not just metrics.
    let good = run(
        &h::comparison(true),
        "compare",
        &["--plans", "Included,Unrelated", "--mode", "exhaustive"],
    )
    .v;
    comparison_values(&good, true);
    let mut bad = good.clone();
    bad["evaluation"]["hard"][0]["values"][0]["interval"] = json!({"lower":"1","upper":"1"});
    assert!(
        std::panic::catch_unwind(|| comparison_values(&bad, true)).is_err(),
        "fault 3 hard"
    );
    let mut bad = good;
    bad["evaluation"]["objectives"][0]["values"][0]["lower"] = json!("5");
    assert!(
        std::panic::catch_unwind(|| comparison_values(&bad, true)).is_err(),
        "fault 3 objective"
    );
    // (4) V1 keeps the exact reason vocabulary.
    let good = run(&h::source(1), "check", &[]).v;
    values(&good, 1, false);
    let mut bad = good;
    bad["metrics"]["overall"]["work"]["reason"] = json!("deferred");
    assert!(
        std::panic::catch_unwind(|| values(&bad, 1, false)).is_err(),
        "fault 4"
    );
    // (5) An opaque defer in the other Cut must not leak a warning.
    let src = h::selected_cuts();
    let good = run(&src, "check", &["--configuration", "Clean"]).v;
    warning_rows(&good, &src, &[]);
    let mut bad = good;
    bad["diagnostics"].as_array_mut().unwrap().push(
        json!({"code":"W-CUT-2","severity":"warning","message":h::warning("Ghost"),"span":null}),
    );
    assert!(
        std::panic::catch_unwind(|| warning_rows(&bad, &src, &[])).is_err(),
        "fault 5"
    );
}
