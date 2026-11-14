//! Real CLI hand acceptances and wire fault controls for the integrated Rule route.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/rule_integration_inputs.rs"]
mod i;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
#[test]
fn rule_population_is_closed_exact_and_unknown_names_are_not_exempt() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let names: Vec<String> = serde_json::from_str(include_str!(
        "../../relca-core/tests/support/rule_names.json"
    ))
    .unwrap();
    let declared: std::collections::BTreeSet<_> = names.iter().cloned().collect();
    let actual: std::collections::BTreeSet<_> = std::fs::read_dir(root.join("fixtures"))
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| p.join("model.relca").is_file())
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("rule-"))
        .collect();
    assert_eq!(names.len(), 140);
    assert_eq!(declared.len(), 140);
    assert_eq!(declared, actual);
    for name in names {
        assert!(fixture_match::rule_actor_fixture(&name));
    }
    assert!(!fixture_match::rule_actor_fixture(
        "rule-unlisted-integration"
    ));
}
fn run(s: &str, cmd: &str, args: &[&str]) -> (Value, Output) {
    let dir = test_tmp::root().join(format!("a6int-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{}.relca", NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&path, s).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(cmd)
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    let v = serde_json::from_slice(&o.stdout).unwrap_or_else(|_| {
        panic!(
            "not JSON: {:?} {}",
            o.status,
            String::from_utf8_lossy(&o.stderr)
        )
    });
    (v, o)
}
#[test]
fn record_type_has_one_lower_error_and_exit_four_not_parser_error_or_cascade() {
    let (good, o) = run(&i::record_rule(false), "check", &[]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(good["verdict"], "satisfied");
    let (v, o) = run(&i::record_rule(true), "check", &[]);
    assert_eq!(o.status.code(), Some(4));
    fixture_match::compare_report(&json!({"verdict":"unsupported"}), &v).unwrap();
    let ds = v["diagnostics"].as_array().unwrap();
    assert_eq!(ds.len(), 1);
    assert_eq!(ds[0]["code"], "U-SEM-0");
    assert_eq!(ds[0]["message"], "rule value type is unsupported");
    assert!(ds[0]["span"].is_object());
}
#[test]
fn named_enum_pair_has_required_and_vacuous_exits() {
    for (closed, exit, verdict) in [(false, 0, "satisfied"), (true, 5, "vacuous")] {
        let (v, o) = run(&i::named_enum_rule(closed), "check", &[]);
        assert_eq!(o.status.code(), Some(exit));
        assert_eq!(v["verdict"], verdict);
        assert_eq!(
            v["terminals"].as_array().unwrap().len(),
            if closed { 1 } else { 2 }
        );
    }
}
fn inline_position(decides_first: bool) {
    let s = if decides_first {
        i::inline_rule_decides_first(false)
    } else {
        i::inline_rule(false)
    };
    let (v, o) = run(&s, "check", &[]);
    assert_eq!(o.status.code(), Some(4));
    assert_eq!(v["verdict"], "unsupported");
    let ds = v["diagnostics"].as_array().unwrap();
    assert_eq!(ds.len(), 1);
    assert_eq!(ds[0]["code"], "U-SYN-0");
    assert_eq!(ds[0]["severity"], "error");
    assert_eq!(
        ds[0]["message"],
        "unsupported syntax: inline enum position is unsupported"
    );
    assert!(ds[0]["span"].is_object());
    let start = s.find("enum { open, closed }").unwrap();
    assert_eq!(ds[0]["span"], json!({"start":start,"end":start+4}));
    assert_eq!(v["cut"], "unresolved");
}
#[test]
fn inline_enum_decides_position_is_unsupported_and_exit_four() {
    inline_position(false);
}
#[test]
fn inline_enum_decides_first_keeps_inner_message_span_and_exit_four() {
    inline_position(true);
}
#[test]
fn actual_diff_keeps_rule_u_and_exact_ten_agency_changes() {
    let (v, o) = run(
        &i::fixture("rule-diff"),
        "diff",
        &["--from", "AsIs", "--to", "ToBeB"],
    );
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(v["agency"]["differences"]["rows"], i::differences());
    fixture_match::compare_diff_report(&json!({"verdict":"violated"}), &v).unwrap();
}
fn hand_report() -> Value {
    let (mut v, o) = run(
        &i::fixture("rule-basic"),
        "check",
        &["--configuration", "AsIs"],
    );
    assert_eq!(o.status.code(), Some(0));
    v["agency"]["viewpoints"]["effects"]["rows"] = json!([i::effect(false)]);
    v
}
#[test]
fn effects_wire_positive_accepts_rule_union() {
    fixture_match::compare_report(&json!({"verdict":"satisfied"}), &hand_report()).unwrap();
}
#[test]
fn effects_wire_rejects_unknown_missing_mixed_empty_and_duplicate_rule_u() {
    let good = hand_report();
    for tag in ["unknown", "kind", "mixed", "reads", "duplicate"] {
        let mut bad = good.clone();
        bad["agency"]["viewpoints"]["effects"]["rows"][0]["u"] = if tag == "duplicate" {
            json!([i::rule_item("r1"), i::rule_item("r1")])
        } else {
            json!([i::bad_rule(tag)])
        };
        assert!(
            fixture_match::compare_report(&json!({"verdict":"satisfied"}), &bad).is_err(),
            "{tag}"
        );
    }
}
#[test]
fn difference_wire_positive_accepts_rule_cells() {
    let (mut v, _) = run(
        &i::fixture("rule-diff"),
        "diff",
        &["--from", "AsIs", "--to", "ToBeB"],
    );
    v["from"]["report"]["agency"]["viewpoints"]["effects"]["rows"] = json!([i::effect(false)]);
    v["to"]["report"]["agency"]["viewpoints"]["effects"]["rows"] = json!([i::effect(true)]);
    v["agency"]["differences"] = json!({"rules":"agency-diff-p2-1","status":"derived","rows":i::differences(),"unchecked":["fallback_path","workload_transfer","via_model_paths"]});
    fixture_match::compare_diff_report(&json!({"verdict":"violated"}), &v).unwrap();
}
#[test]
fn difference_wire_rejects_each_unknown_missing_mixed_empty_or_duplicate_rule_cell() {
    for (view, item) in [("effects", "u"), ("refusal", "judgements")] {
        let good = json!({"rules":"agency-diff-p2-1","status":"derived","rows":[{"viewpoint":view,"item":item,"effect":"Ledger.post","call":"ack","status":"changed","from":[i::rule_item("r1")],"to":[]}],"unchecked":["fallback_path","workload_transfer","via_model_paths"]});
        fixture_match::validate_agency_differences(&good).unwrap();
        for tag in ["unknown", "kind", "mixed", "reads", "duplicate"] {
            let mut bad = good.clone();
            bad["rows"][0]["from"] = if tag == "duplicate" {
                json!([i::rule_item("r1"), i::rule_item("r1")])
            } else {
                json!([i::bad_rule(tag)])
            };
            assert!(
                fixture_match::validate_agency_differences(&bad).is_err(),
                "{view}/{item}/{tag}"
            );
        }
    }
}
#[test]
fn non_rule_wire_cannot_opt_into_rule_u_without_rule_actor_metadata() {
    let (v, o) = run(&i::fixture("contract-invariant"), "check", &[]);
    assert_eq!(o.status.code(), Some(0));
    fixture_match::compare_report(&json!({"verdict":"satisfied"}), &v).unwrap();
    let mut bad = v;
    bad["agency"]["viewpoints"]["effects"]["rows"][0]["u"]
        .as_array_mut()
        .expect("real non-Rule effect row")
        .push(i::rule_item("r1"));
    assert!(
        fixture_match::compare_report(&json!({"verdict":"satisfied"}), &bad).is_err(),
        "new Rule union must not widen old non-Rule models"
    );
}
#[test]
fn rule_quality_is_present_with_fixed_rule_ratio() {
    for cmd in ["check", "simulate"] {
        for c in ["AsIs", "ToBeB"] {
            let (v, o) = run(i::QUALITY, cmd, &["--configuration", c]);
            assert_eq!(o.status.code(), Some(if cmd == "check" { 0 } else { 2 }));
            i::assert_fixed_quality(&v["quality"], c == "AsIs", cmd == "simulate");
            i::assert_quality_assumptions(&v, cmd == "simulate", true);
            if cmd == "check" {
                assert_eq!(
                    v["terminals"],
                    json!([{"end":"done","elapsed":{"min":1,"max":1},"work":{"min":1,"max":1},"result_at":null}])
                );
                fixture_match::compare_report(&json!({}), &v).unwrap();
            } else {
                assert_eq!(v["simulation"]["completed_replications"], 4);
                fixture_match::validate_simulation_wire(&v).unwrap();
            }
        }
    }
}

fn validate(v: &Value, sampled: bool) -> Result<(), String> {
    if sampled {
        fixture_match::validate_simulation_wire(v)
    } else {
        fixture_match::compare_report(&json!({}), v)
    }
}

// Historical disclosure is now an invalid injection (ADR0052).
fn with_disclosure(mut v: Value) -> Value {
    let rows = v["assumptions"].as_array_mut().unwrap();
    rows.retain(|a| a["field"] != "quality.rule");
    let at = rows
        .iter()
        .position(|a| a["field"] == "quality.scheduler" || a["field"] == "quality.estimation")
        .unwrap()
        + 1;
    rows.insert(at, i::quality_rule_assumption());
    v
}

fn assert_quality_mode_assumption_bytes(v: &Value, sampled: bool) {
    let (field, value, affects) = if sampled {
        (
            "quality.estimation",
            "ratio_of_sums_per_replication",
            "ΣN/ΣDの点推定。標本は反復単位であり案件を独立標本としない。expected_countは既知hit件数、unknown_countは未確定件数の別標本。観測結末は観測停止時点の分類であり将来の到達確率ではない。区間推定でも全スケジュールの検証でもない。",
        )
    } else {
        (
            "quality.scheduler",
            "per_case_relaxation",
            "案件別のmin/maxの和は同時の極値ではない。未確定では保守的外端点を示すが確定した期待件数とは呼ばない。計算切断はtruncated、業務打切りだけundeterminedとし、性質ごとに確定判定する。観測結末と性質の将来到達の未知を分ける。",
        )
    };
    let rows: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["field"] == field)
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0]["value"].as_str().unwrap().as_bytes(),
        value.as_bytes()
    );
    assert_eq!(
        rows[0]["affects"].as_str().unwrap().as_bytes(),
        affects.as_bytes()
    );
}

#[test]
fn quality_disclosure_wire_is_closed_ordered_and_rule_only() {
    for sampled in [false, true] {
        let cmd = if sampled { "simulate" } else { "check" };
        let (v, o) = run(i::QUALITY, cmd, &["--configuration", "AsIs"]);
        assert_eq!(o.status.code(), Some(if sampled { 2 } else { 0 }));
        let mut good = v;
        good["assumptions"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["field"] != "quality.rule");
        validate(&good, sampled).unwrap();
        assert_quality_mode_assumption_bytes(&good, sampled);
        for key in ["value", "affects"] {
            let mut bad = good.clone();
            let row = bad["assumptions"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|a| a["field"] == "quality.scheduler" || a["field"] == "quality.estimation")
                .unwrap();
            row[key] = json!("different string");
            // The generic matcher only checks the type; this direct oracle
            // independently protects the exact value and explanatory bytes.
            validate(&bad, sampled).unwrap();
            assert!(
                std::panic::catch_unwind(|| assert_quality_mode_assumption_bytes(&bad, sampled))
                    .is_err(),
                "string {key}/sampled={sampled}"
            );
        }
        for fault in [
            "order",
            "missing",
            "duplicate",
            "value",
            "reason",
            "unknown",
            "obsolete_rule",
        ] {
            let mut bad = good.clone();
            let rows = bad["assumptions"].as_array_mut().unwrap();
            let at = rows
                .iter()
                .position(|a| {
                    a["field"] == "quality.scheduler" || a["field"] == "quality.estimation"
                })
                .unwrap();
            match fault {
                "order" => rows.swap(at, at - 1),
                "missing" => {
                    rows.remove(at);
                }
                "duplicate" => rows.insert(at, rows[at].clone()),
                "value" => rows[at]["value"] = json!(false),
                "reason" => rows[at]["affects"] = json!(false),
                "unknown" => rows[at]["field"] = json!("quality.unlisted"),
                "obsolete_rule" => rows.insert(at + 1, i::quality_rule_assumption()),
                _ => unreachable!(),
            }
            assert!(
                validate(&bad, sampled).is_err(),
                "{fault}/sampled={sampled}"
            );
        }
        let (v, _) = run(&i::held_quality(true, false), cmd, &["--scenario", "Day"]);
        i::assert_quality_assumptions(&v, sampled, false);
        validate(&v, sampled).unwrap();
        let bad = with_disclosure(v);
        assert!(
            validate(&bad, sampled).is_err(),
            "non-Rule disclosure injection"
        );
    }
}

#[test]
fn quality_day_real_cli_horizon_and_drain_have_exact_exits_and_withdrawal() {
    let mut reports = Vec::new();
    for drain in [false, true] {
        for sampled in [false, true] {
            for rule in [true, false] {
                let cmd = if sampled { "simulate" } else { "check" };
                let (v, o) = run(&i::held_quality(drain, rule), cmd, &["--scenario", "Day"]);
                assert_eq!(o.status.code(), Some(if sampled || !drain { 2 } else { 0 }));
                assert!(
                    !v["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|d| d["code"] == "U-SEM-0")
                );
                {
                    let q = &v["quality"];
                    assert_eq!(q["cohort"][0]["value"]["reason"], Value::Null);
                    assert_eq!(
                        q["cohort"][0]["value"][if sampled { "point" } else { "lower" }],
                        "1"
                    );
                }
                if sampled {
                    assert_eq!(v["simulation"]["completed_replications"], 4);
                } else {
                    let cases = v["case_outcomes"].as_array().unwrap();
                    assert_eq!(cases.len(), 3);
                    for (k, c) in cases.iter().enumerate() {
                        let t = &c["terminals"][0];
                        assert_eq!(t["kind"], if drain { "end" } else { "cut_off" });
                        if drain {
                            let at = 5 * (k + 1);
                            assert_eq!(t["completed_at"], json!({"min":at,"max":at}));
                        }
                    }
                }
                reports.push((v, sampled, rule));
            }
        }
    }
    for (v, sampled, rule) in reports {
        i::assert_quality_assumptions(&v, sampled, rule);
        validate(&v, sampled).unwrap();
        if rule {
            // The rule tail follows every existing row in both wires: an old row moved
            // after R.rule is refused (the matcher must not accept a reordered tail).
            for field in ["scenario.aggregation", "concurrent.cases", "R.boundary"] {
                let mut bad = v.clone();
                let rows = bad["assumptions"].as_array_mut().unwrap();
                let row = rows.remove(rows.iter().position(|r| r["field"] == field).unwrap());
                let at = rows.iter().position(|r| r["field"] == "R.rule").unwrap();
                rows.insert(at + 1, row);
                assert_ne!(bad, v);
                assert!(
                    validate(&bad, sampled).is_err(),
                    "{field} sampled {sampled}"
                );
            }
        }
    }
}

#[test]
fn quality_execution_cli_keeps_hand_exits_done_time_work_and_four_replications() {
    for cmd in ["check", "simulate"] {
        for c in ["AsIs", "ToBeB"] {
            let (v, o) = run(i::QUALITY, cmd, &["--configuration", c]);
            let (old, baseline) = run(&i::strip_quality(i::QUALITY), cmd, &["--configuration", c]);
            assert_eq!(o.status.code(), Some(if cmd == "check" { 0 } else { 2 }));
            assert_eq!(baseline.status.code(), o.status.code());
            assert_eq!(v["diagnostics"], old["diagnostics"]);
            assert!(
                !v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "U-SEM-0")
            );
            i::assert_fixed_quality(&v["quality"], c == "AsIs", cmd == "simulate");
            if cmd == "check" {
                assert_eq!(
                    v["terminals"],
                    json!([{"end":"done","elapsed":{"min":1,"max":1},"work":{"min":1,"max":1},"result_at":null}])
                );
                assert_eq!(v["terminals"], old["terminals"]);
                assert_eq!(v["properties"], old["properties"]);
            } else {
                assert_eq!(v["simulation"]["completed_replications"], 4);
                assert_eq!(v["simulation"], old["simulation"]);
                assert_eq!(v["case_outcomes"], old["case_outcomes"]);
            }
        }
    }
}

#[test]
fn rule_effects_u_production_dto_roundtrip_and_corrupt_arm_control() {
    let good = hand_report();
    // Unlike the historical matcher projection this directly checks the real
    // production Report, then requires the same normal wire through the matcher.
    let dto: relca_core::report::Report = serde_json::from_value(good.clone()).unwrap();
    assert_eq!(json!(dto), good);
    fixture_match::compare_report(&json!({}), &good).unwrap();
    let mut bad = good;
    bad["agency"]["viewpoints"]["effects"]["rows"][0]["u"][0]["version"] = json!("");
    assert!(serde_json::from_value::<relca_core::report::Report>(bad.clone()).is_err());
    assert!(fixture_match::compare_report(&json!({}), &bad).is_err());
}
