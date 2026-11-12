//! Real CLI integration, fixed hand arithmetic and independent old-field pairs.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/quality_integration_inputs.rs"]
mod i;
#[path = "../../relca-core/tests/support/responsibility_defaults_oracle.rs"]
mod responsibility_oracle;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::{Value, json};
use std::{
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

fn run(src: &str, command: &str, args: &[&str]) -> (Value, i32, Vec<u8>) {
    let dir = test_tmp::root().join(format!("b6int-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{}.relca", NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&path, src).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    let v = serde_json::from_slice(&o.stdout).unwrap_or_else(|_| {
        panic!(
            "CLI must not panic: exit={:?}, stderr={}",
            o.status.code(),
            String::from_utf8_lossy(&o.stderr)
        )
    });
    (v, o.status.code().unwrap(), o.stderr)
}
fn assumptions(v: &Value, simulation: bool) {
    let rows = v["assumptions"].as_array().unwrap();
    let slots: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, a)| a["field"].as_str().unwrap().starts_with("quality."))
        .map(|(k, _)| k)
        .collect();
    assert_eq!(slots.len(), 4, "one closed block");
    assert_eq!(slots, (slots[0]..slots[0] + 4).collect::<Vec<_>>());
    let fields: Vec<_> = slots
        .iter()
        .map(|&k| rows[k]["field"].as_str().unwrap())
        .collect();
    assert_eq!(
        fields,
        vec![
            "quality.definition",
            "quality.denominator",
            "quality.form",
            if simulation {
                "quality.estimation"
            } else {
                "quality.scheduler"
            }
        ]
    );
    let scheduler = rows
        .iter()
        .position(|a| a["field"] == "probability.scheduler")
        .unwrap_or(rows.len());
    assert_eq!(
        slots[0] + 4,
        scheduler,
        "after every old scenario/Agency/simulation row, before probability.scheduler"
    );
}
fn defer(name: &str) {
    let e = i::deferred(name);
    let day = e["invocation"]["scenario"] == "Day";
    let sim = e["invocation"]["command"] == "simulate";
    let cmd = if sim { "simulate" } else { "check" };
    let args = if day { &["--scenario", "Day"][..] } else { &[] };
    let (v, exit, _) = run(e["source"].as_str().unwrap(), cmd, args);
    assert_eq!(exit, 2, "unfinished is not success");
    assert_eq!(v["verdict"], "inconclusive");
    assert_eq!(
        v["quality"], e["expected"]["quality"],
        "{name}: complete preauthored hand oracle"
    );
    assumptions(&v, sim);
    let inv =
        fixture_match::Invocation::parse((!e["invocation"].is_null()).then_some(&e["invocation"]))
            .unwrap();
    inv.compare(
        &json!({"verdict":"inconclusive","quality":e["expected"]["quality"]}),
        &v,
    )
    .unwrap();
    if sim {
        assert_eq!(
            v["simulation"]["truncated_replications"],
            if day { 0 } else { 4 }
        );
        if day {
            assert_eq!(v["simulation"]["cut_off_replications"], 4);
        } else {
            assert!(v["simulation"].get("cut_off_replications").is_none());
        }
    } else {
        assert_eq!(v["bounds"]["truncated"], !day);
    }
}
#[test]
fn t06_horizon_hand_wire_and_exit() {
    defer("qual-horizon");
}
#[test]
fn t06_clock_hand_wire_and_exit() {
    defer("qual-horizon.broken-clock");
}
#[test]
fn t10_horizon_hand_wire_and_exit() {
    defer("qual-sim-horizon");
}
#[test]
fn t10_clock_hand_wire_and_exit() {
    defer("qual-sim-horizon.broken-clock");
}

#[test]
fn c_quality_block_follows_scenario_and_agency_rows_and_matcher_accepts() {
    // The known complete run has exact Wrong/arrival=1/50. Rename only the
    // scenario and use H30 (all cases finish by11); old rows remain unchanged.
    let e = i::deferred("qual-horizon");
    let s = e["source"]
        .as_str()
        .unwrap()
        .replace("horizon 14min", "horizon 30min");
    let (v, exit, _) = run(&s, "check", &["--scenario", "Day"]);
    assert_eq!(exit, 0);
    assert_eq!(
        i::ratio(&v["quality"], "WrongPerArrival")["value"],
        i::exact("1/50")
    );
    assumptions(&v, false);
    fixture_match::Invocation::parse(Some(&json!({"command":"check","scenario":"Day"})))
        .unwrap()
        .compare(&json!({"verdict":"satisfied"}), &v)
        .unwrap();
    let old_source = i::strip_quality(&s);
    let (old, old_exit, old_err) = run(&old_source, "check", &["--scenario", "Day"]);
    assert_eq!(exit, old_exit);
    // ADR 0054: stripping quality does not strip the commit's responsibility.
    // Require exact source/Day-IR rows and stderr instead of a blanket empty log.
    let ast = relca_core::syntax::parse(&old_source).unwrap();
    let model = relca_core::lower::lower_with_selection(&ast, None, Some("Day")).unwrap();
    responsibility_oracle::assert_report(&model, &old);
    let warnings = responsibility_oracle::expected(&model);
    assert!(!warnings.is_empty());
    assert_eq!(old["diagnostics"], json!(warnings));
    let stderr = warnings
        .iter()
        .map(|d| format!("{}: {}\n", d.code, d.message))
        .collect::<String>();
    let matches_stderr = |actual: &[u8]| actual == stderr.as_bytes();
    assert!(matches_stderr(&old_err));
    assert!(
        !matches_stderr(b""),
        "missing responsibility stderr rejected"
    );
    let mut extra = old_err.clone();
    extra.extend_from_slice(b"W-RESP-0: Unknown has no declared responsibility\n");
    assert!(
        !matches_stderr(&extra),
        "spurious responsibility stderr rejected"
    );
    assert_eq!(i::old_projection(v), i::old_projection(old));
}

#[test]
fn c_matcher_rejects_quality_block_before_scenario_or_after_scheduler() {
    let s = i::append(
        &i::scenario(&i::fixture("qual-contention.broken-no-policy"), 30),
        "verify probability reached registered == true >= 0",
    );
    let (v, exit, _) = run(&s, "check", &["--scenario", "Day"]);
    assert_eq!(exit, 0);
    assert!(
        v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["field"] == "probability.scheduler"),
        "must actually cross a scheduler row"
    );
    let inv = fixture_match::Invocation::parse(Some(&json!({"command":"check","scenario":"Day"})))
        .unwrap();
    let mut wrong = v.clone();
    let rows = wrong["assumptions"].as_array_mut().unwrap();
    let quality: Vec<_> = rows
        .iter()
        .filter(|a| a["field"].as_str().unwrap().starts_with("quality."))
        .cloned()
        .collect();
    rows.retain(|a| !a["field"].as_str().unwrap().starts_with("quality."));
    rows.splice(0..0, quality.clone());
    assert!(
        inv.compare(&json!({"verdict":"satisfied"}), &wrong)
            .is_err()
    );
    let mut wrong = v;
    let rows = wrong["assumptions"].as_array_mut().unwrap();
    rows.retain(|a| !a["field"].as_str().unwrap().starts_with("quality."));
    rows.extend(quality);
    assert!(
        inv.compare(&json!({"verdict":"satisfied"}), &wrong)
            .is_err()
    );
}

#[test]
fn scenario_simulation_refusal_preserves_old_fields_and_four_quality_rows() {
    let s = i::scenario(&i::fixture("qual-base"), 30);
    let (v, exit, err) = run(&s, "simulate", &["--scenario", "Day"]);
    let (old, old_exit, old_err) = run(&i::strip_quality(&s), "simulate", &["--scenario", "Day"]);
    assert_eq!(exit, 3);
    assert_eq!(exit, old_exit);
    assert_eq!(err, old_err);
    assert_eq!(i::old_projection(v.clone()), i::old_projection(old));
    assert_eq!(v["quality"]["cohort"][0]["value"]["reason"], "aborted");
    assumptions(&v, true);
}

#[test]
fn explain_horizon_keeps_quality_and_verified_trace_not_panic() {
    let s = i::HELD.replace(
        "verify probability reached hit == true >= 1",
        "contract K { invariant hit == true }",
    );
    let (v, exit, _) = run(&s, "explain", &["--scenario", "Day"]);
    assert_eq!(
        exit, 1,
        "receipt0 already violates invariant, quality cannot improve it"
    );
    assert_eq!(v["verdict"], "violated");
    i::unknown(&v["quality"], "Rate", "undetermined", "0", "1");
    assumptions(&v, false);
    let text = serde_json::to_string(&v["explanation"]).unwrap();
    assert!(
        text.contains("verified"),
        "must replay a real counterexample: {text}"
    );
}

#[test]
fn record_field_condition_uses_finite_state_canonical_form() {
    let (v, exit, _) = run(i::RECORD, "check", &[]);
    assert_eq!(exit, 0);
    assert_eq!(
        i::property(&v["quality"], "Pass")["condition"],
        "entry == Verdict { confidence = pass }"
    );
    assert_eq!(
        i::ratio(&v["quality"], "PassRate")["value"],
        i::exact("23/25")
    );
}

#[test]
fn d08_lost_update_violation_stays_sticky_while_quality_is_truncated() {
    let s = i::fixture("cf-lww.broken-overlap")
        .replace("clock 10min", "clock 2min")
        .replace("r = Ledger.claim()", "r = Ledger.claim(); wait 5min");
    // last_write_wins itself installs no_lost_update Ledger.holder; there is
    // no separate verify safety no_lost_update syntax.
    let s = i::append(
        &s,
        "property case.Done = reached(end.claimed)\nmetric cohort.Rate = expected(count(case.Done)) / arrivals",
    );
    let (v, exit, err) = run(&s, "check", &[]);
    let (old, old_exit, old_err) = run(&i::strip_quality(&s), "check", &[]);
    assert_eq!(exit, 1);
    assert_eq!(exit, old_exit);
    assert_eq!(err, old_err);
    assert_eq!(v["verdict"], "violated");
    i::unknown(&v["quality"], "Rate", "truncated", "0", "1");
    assert_eq!(i::old_projection(v), i::old_projection(old));
}

fn agency_transport(command: &str, args: &[&str]) {
    let s = i::append(
        &i::fixture("agy2-diff-items"),
        "property case.Resolved = reached(end.posted || end.returned)\nmetric cohort.Rate = expected(count(case.Resolved)) / arrivals",
    );
    let (v, exit, err) = run(&s, command, args);
    let (old, old_exit, old_err) = run(&i::strip_quality(&s), command, args);
    assert_eq!(exit, old_exit);
    assert_eq!(exit, 0);
    assert_eq!(err, old_err);
    assert_eq!(
        i::old_projection(v.clone()),
        i::old_projection(old),
        "complete old Agency/viewpoints/effects/differences and consumer projection"
    );
    let reports: Vec<&Value> = if command == "diff" {
        vec![&v["from"]["report"], &v["to"]["report"]]
    } else if command == "compare" {
        v["plans"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| &p["report"])
            .collect()
    } else {
        vec![&v]
    };
    for report in reports {
        assert_eq!(
            i::property(&report["quality"], "Resolved")["expected_count"],
            i::exact("1")
        );
        assert_eq!(i::ratio(&report["quality"], "Rate")["value"], i::exact("1"));
        assumptions(report, false);
    }
    if command == "compare" || command == "diff" {
        let table = if command == "compare" {
            &v["tables"]["quality"]
        } else {
            &v["quality"]
        };
        assert_eq!(
            table[0]["key"],
            json!({"scope":"property","subject":"Resolved","end":null,"field":"expected_count"})
        );
        assert!(table[0].get("difference").is_none());
        assert!(table[0].get("rank").is_none());
    }
}
#[test]
fn d09_check_agency_full_fields_unchanged() {
    agency_transport("check", &["--configuration", "To"]);
}
#[test]
fn d09_compare_consumer_keeps_agency_and_quality_separate() {
    agency_transport("compare", &["--plans", "From,To", "--mode", "exhaustive"]);
}
#[test]
fn d09_diff_consumer_keeps_agency_and_quality_separate() {
    agency_transport("diff", &["--from", "From", "--to", "To"]);
}
