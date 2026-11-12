//! ADR0064 Q6a real executable: independent JSON paths, stderr and exit code.
#[path = "../../relca-core/tests/support/q6a_oracle.rs"]
mod hand;
#[path = "../../relca-core/tests/support/q6a_inputs.rs"]
mod input;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn diagnostics(v: &Value, prefix: &str) -> String {
    v["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .map(|d| {
            format!(
                "{prefix}{}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            )
        })
        .collect()
}
fn invoke_source(name: &str, mode: &str, src: &str, args: &[&str]) -> (Value, i32) {
    let dir = root().join(".tmp/q6a-cli").join(format!(
        "{}-{name}-{mode}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.relca");
    std::fs::write(&path, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(mode)
        .arg(path)
        .args(args)
        .current_dir(root())
        .env("CARGO_NET_OFFLINE", "true")
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    assert_eq!(
        out.stdout.iter().filter(|c| **c == b'\n').count(),
        1,
        "one JSON line, {name}/{mode}"
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let stderr = match mode {
        "compare" => {
            let mut s = String::new();
            for p in v["plans"].as_array().unwrap() {
                s.push_str(&diagnostics(
                    &p["report"],
                    &format!("configuration {}: ", p["configuration"].as_str().unwrap()),
                ));
            }
            s.push_str(&diagnostics(&v, "compare: "));
            s
        }
        "diff" => {
            let mut s = diagnostics(&v["from"]["report"], "from: ");
            s.push_str(&diagnostics(&v["to"]["report"], "to: "));
            s.push_str(&diagnostics(&v, "diff: "));
            s
        }
        "mutate" => {
            let mut s = String::new();
            if v["original"].is_object() {
                s.push_str(&diagnostics(&v["original"], "original: "));
            }
            for row in v["mutations"].as_array().unwrap() {
                for mutant in row["mutants"].as_array().unwrap() {
                    if mutant["report"].is_object() {
                        s.push_str(&diagnostics(
                            &mutant["report"],
                            &format!("{}: ", mutant["id"].as_str().unwrap()),
                        ));
                    }
                }
            }
            s.push_str(&diagnostics(&v, "mutate: "));
            s
        }
        _ => diagnostics(&v, ""),
    };
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        stderr,
        "{name}/{mode}"
    );
    let code = out.status.code().expect("normal exit");
    assert_eq!(code, hand::exit(&v), "classification/exit: {name}/{mode}");
    (v, code)
}
fn invoke(name: &str, mode: &str) -> (Value, i32) {
    let args = if input::scenario(name).is_some() {
        vec!["--scenario", "Window"]
    } else {
        vec![]
    };
    invoke_source(name, mode, &input::source(name), &args)
}
fn check(name: &str) {
    let (v, _) = invoke(name, "check");
    hand::validate(name, &v)
        .unwrap_or_else(|e| panic!("Q6a/{name}: {e}; diagnostics={}", v["diagnostics"]));
}
macro_rules! checks {($($test:ident=>$name:literal),+$(,)?)=>{$(#[test]fn $test(){check($name);})+};}
checks! {
    a1_qualification_before_first_idle=>"qualified",a1_unpermitted_qualified_person_stays_assigned=>"forbidden",
    a1_uncontrolled_start=>"uncontrolled",
    a2_wait_for_qualified_person_shift=>"shift",a2_nonfixed_frontier=>"shift-unassigned",
    a2_empty_is_deadlock_not_fake_opening=>"empty",a3_p_performance=>"people",a3_q_performance=>"reverse",
    a3_unassigned_is_not_uniform=>"unassigned",a4_burst_count_and_rate=>"burst",a4_changed_high_kernel=>"lighter",
    a5_held_not_backlog=>"held",a6_start_band_frozen=>"later",a6_changed_arrival=>"simultaneous",
    a4_self_not_queued=>"single",a10_clock_unknown=>"clock",a10_horizon_not_truncation=>"horizon",
    a13_priority_with_qualified_calendar=>"priority",a13_reversed_priority=>"priority-reverse",
    a14_requires_refusal=>"refused",a14_requires_admission=>"admitted",a10_cut_preserves_null=>"defer",
    i16_response_none=>"known-none",i16_grain_not_skill=>"label",i5_unused_profile=>"unused"
}
#[test]
fn a12_each_static_fault_has_the_intended_diagnostic() {
    let mut errors = vec![];
    for &(name, code, reason) in input::STATIC {
        for mode in ["check", "simulate"] {
            let (v, exit) = invoke(name, mode);
            if let Err(e) = hand::diagnostic(&v, code, reason) {
                errors.push(format!("{name}/{mode}: {e}"));
            }
            if v.pointer("/ledger/execution/status") != Some(&json!("not_run")) {
                errors.push(format!("{name}/{mode}: static refusal executed a prefix"));
            }
            if exit != if code.starts_with("E-") { 3 } else { 4 } {
                errors.push(format!("{name}/{mode}: wrong exit {exit}"));
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
#[test]
fn a3_a6_a14_real_simulation_matches_the_independent_seed_bank() {
    let mut errors = vec![];
    for name in [
        "people",
        "reverse",
        "burst",
        "lighter",
        "held",
        "later",
        "simultaneous",
        "refused",
        "admitted",
        "known-none",
    ] {
        let (v, _) = invoke(name, "simulate");
        if let Err(e) = hand::validate_sim(name, &v) {
            errors.push(format!("{name}: {e}; diagnostics={}", v["diagnostics"]));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
#[test]
fn a3_simulation_refuses_unassigned_not_implicit_skill_optimization() {
    let (v, exit) = invoke("unassigned", "simulate");
    assert_eq!(exit, 4);
    hand::diagnostic(&v, "U-SEM-0", "unresolved").unwrap();
}

#[test]
fn i3_qualification_only_simulate_is_not_exhaustive_safety() {
    for name in ["qualified", "shift"] {
        let (v, exit) = invoke(name, "simulate");
        assert_eq!(
            exit, 2,
            "sampled safety is inconclusive: {}",
            v["diagnostics"]
        );
        assert_eq!(v["simulation"]["completed_replications"], 16);
        assert_eq!(v["work_profiles"], hand::table(name));
        hand::decode_table(&v["work_profiles"]).unwrap();
        for (person, work) in [("p", 0), ("q", 2)] {
            hand::subset(
                &hand::sample(&work.to_string(), work, work),
                &hand::by(&v["metrics"]["people"], "person", &json!(person)).unwrap()["work"],
                name,
            )
            .unwrap();
        }
    }
}
#[test]
fn a1_a2_explain_transports_only_independently_verified_replays() {
    for name in ["forbidden", "shift"] {
        let src = if name == "shift" {
            input::append(
                &input::source(name),
                "contract Deadline { response case.received -> one_of(end.done) by 4min after case.received otherwise unresolved }",
            )
        } else {
            input::source(name)
        };
        let (v, exit) = invoke_source(name, "explain", &src, &[]);
        assert_eq!(exit, 1, "diagnostics={}", v["diagnostics"]);
        let rows = v
            .pointer("/explanation/counterexamples")
            .and_then(Value::as_array)
            .expect("counterexamples");
        assert!(!rows.is_empty());
        for row in rows {
            assert_eq!(row["replay"]["status"], "verified");
        }
    }
}
#[test]
fn i11_wire_is_decoded_independently_then_by_the_public_decoder() {
    let (v, _) = invoke("burst", "check");
    hand::validate("burst", &v).unwrap();
    let decoded: relca_core::report::Report = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(json!(decoded)["work_profiles"], hand::table("burst"));
    for (base, label, bad) in hand::wire_faults() {
        let (mut host, _) = invoke(base, "check");
        hand::validate(base, &host).unwrap();
        host["work_profiles"] = bad;
        assert!(
            serde_json::from_value::<relca_core::report::Report>(host).is_err(),
            "{label}"
        );
    }
}
#[test]
fn i16_unconnected_compare_diff_and_mutation_fail_closed() {
    // Q6c裁定1・補足4によりcompare/diffの未接続期待を撤去した。
    // 接続後の意味はq6c_connectの独立手値で検査する。mutateは未接続のまま。
    let (v, exit) = invoke("people", "mutate");
    assert_eq!(exit, 4);
    assert!(v.to_string().contains("q6_feature_combination"), "{v}");
}
#[test]
fn green_real_cli_existing_kernel_cut_and_words_preserve_contracts() {
    let (v, exit) = invoke_source("old-kernel", "check", &input::legacy("ordinary"), &[]);
    assert_eq!(exit, 0);
    hand::quality("people", &v["quality"]).unwrap();
    assert!(v.get("work_profiles").is_none());
    let (v, exit) = invoke_source("old-clock", "check", &input::legacy("clock"), &[]);
    assert_eq!(exit, 2);
    hand::quality("clock", &v["quality"]).unwrap();
    let (v, exit) = invoke_source(
        "old-horizon",
        "check",
        &input::legacy("horizon"),
        &["--scenario", "Window"],
    );
    assert_eq!(exit, 2);
    hand::quality("horizon", &v["quality"]).unwrap();
    let (v, exit) = invoke_source("old-word", "check", &input::legacy("words"), &[]);
    assert_eq!(exit, 1, "p is first_idle and forbidden by permit[q]");
    assert!(v.get("work_profiles").is_none());
}
#[test]
fn green_independent_wire_fault_controls() {
    hand::decode_table(&hand::table("people")).unwrap();
    for (base, name, v) in hand::wire_faults() {
        hand::decode_table(&hand::table(base)).unwrap();
        assert!(hand::decode_table(&v).is_err(), "{name}");
    }
}

#[test]
fn i9_i10_actual_wire_preserves_finish_time_given_condition() {
    for (name, p, exit) in [("given-finish", "1/2", 0), ("given-later", "0", 1)] {
        let (v, code) = invoke(name, "check");
        assert_eq!(code, exit, "{name}: {}", v["diagnostics"]);
        let row = hand::by(&v["quality"]["properties"], "name", &json!("Hit")).unwrap();
        hand::subset(&hand::exact(p), &row["expected_count"], name).unwrap();
        hand::decode_table(&v["work_profiles"]).unwrap();
    }
}

#[test]
fn i16_quantile_is_time_not_skill_score() {
    for (base, slow, minutes) in [
        ("people", false, "2"),
        ("reverse", false, "2"),
        ("people", true, "3"),
    ] {
        let (v, exit) = invoke_source(base, "check", &input::quantile_source(base, slow), &[]);
        assert_eq!(exit, 0, "{}", v["diagnostics"]);
        hand::subset(&hand::quantiles(minutes), &v["quality"]["quantiles"], base).unwrap();
    }
}
