#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/quantile_integration_inputs.rs"]
mod i;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::{Value, json};
fn cli(src: &str, configuration: Option<&str>, scenario: Option<&str>, sim: bool) -> Value {
    command(
        src,
        configuration,
        scenario,
        if sim { "simulate" } else { "check" },
    )
}
fn command(src: &str, configuration: Option<&str>, scenario: Option<&str>, command: &str) -> Value {
    let dir = test_tmp::root().join(format!(
        "relca-c7int-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("model.relca");
    std::fs::write(&p, src).unwrap();
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_relca"));
    c.arg(command).arg(&p);
    if let Some(s) = configuration {
        c.args(["--configuration", s]);
    }
    if let Some(s) = scenario {
        c.args(["--scenario", s]);
    }
    let out = c.output().unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    if command != "mutate" {
        let stderr: String = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| {
                format!(
                    "{}: {}\n",
                    d["code"].as_str().unwrap(),
                    d["message"].as_str().unwrap()
                )
            })
            .collect();
        assert_eq!(out.stderr, stderr.as_bytes(), "diagnostic stderr transport");
    }
    let verdict: relca_core::Verdict = serde_json::from_value(v["verdict"].clone()).unwrap();
    assert_eq!(out.status.code(), Some(verdict.exit_code()));
    let _ = std::fs::remove_dir_all(dir);
    v
}
#[test]
fn c6_six_kinds_keep_quantiles_without_new_change_detection() {
    for &(name, kind) in i::MUTATIONS {
        for scenario in [None, Some("Day")] {
            let s = i::mutation_source(name, scenario.is_some());
            let v = command(&s, None, scenario, "mutate");
            i::assert_mutation(&v, kind);
            assert_eq!(
                i::wrapper_old(v),
                i::wrapper_old(command(&i::h::no_quantiles(&s), None, scenario, "mutate"))
            );
        }
    }
}
#[test]
fn c6_horizon_child_real_command_keeps_unknown_value_separate_from_detection() {
    i::assert_mutation_horizon(&command(
        &i::mutation_horizon(),
        None,
        Some("Day"),
        "mutate",
    ));
}
#[test]
fn horizon_sources_match_hand_oracles_in_real_commands() {
    let mut failures = vec![];
    for name in i::HORIZON {
        if std::panic::catch_unwind(|| {
            let v = i::deferred(name);
            let src = v["source"].as_str().unwrap();
            let sim = v["invocation"]["command"] == "simulate";
            let actual = cli(src, None, v["invocation"]["scenario"].as_str(), sim);
            i::assert_report(
                &actual,
                &v["expected"]["quality"]["quantiles"],
                v["expected"]["verdict"].as_str().unwrap(),
            );
            let command = if sim { "simulate" } else { "check" };
            let inv = fixture_match::Invocation::parse(Some(
                &json!({"command":command,"scenario":"Day"}),
            ))
            .unwrap();
            inv.compare(&v["expected"], &actual).unwrap();
        })
        .is_err()
        {
            failures.push(*name)
        }
    }
    assert!(failures.is_empty(), "real CLI hand failures: {failures:?}");
}
#[test]
fn structural_completion_refusal_keeps_old_assumptions_before_quantile() {
    let s = i::completion_refusal();
    let v = cli(&s, None, None, true);
    assert_eq!(v["verdict"], "unsupported");
    i::assert_aborted_quantiles(&v);
}
#[test]
fn x_unident_g_real_command() {
    let v = i::deferred("mutation-X-unident-g");
    let actual = cli(v["source"].as_str().unwrap(), None, Some("Day"), false);
    i::assert_report(
        &actual,
        &v["expected"]["quality"]["quantiles"],
        "inconclusive",
    );
}
#[test]
fn no_samples_hand_wire_places_quantiles_in_unchecked() {
    let q = i::no_samples_hand_quality();
    serde_json::from_value::<relca_core::quality::Quality>(q.clone())
        .expect("production DTO accepts complete hand no_samples");
    fixture_match::validate_quality_wire(&q, true)
        .expect("same no_samples inventory in independent matcher");
    let mut bad = q;
    bad["unchecked"]
        .as_array_mut()
        .unwrap()
        .retain(|s| s != "quantiles");
    bad["checked"]
        .as_array_mut()
        .unwrap()
        .push(json!("quantiles"));
    assert!(fixture_match::validate_quality_wire(&bad, true).is_err());
}
#[test]
fn sampled_wire_refuses_finite_all_censored_and_zero_denominator_with_censor() {
    let q = i::h::hand_quality(true);
    fixture_match::validate_quality_wire(&q, true).unwrap();
    let mut all = q.clone();
    all["quantiles"][0]["value"]["censored"] = json!(8);
    assert!(serde_json::from_value::<relca_core::quality::Quality>(all.clone()).is_err());
    assert!(
        fixture_match::validate_quality_wire(&all, true).is_err(),
        "finite conditional point with C=N"
    );
    let mut zero = q;
    zero["quantiles"][0]["value"] =
        json!({"point":null,"observations":8,"censored":1,"reason":"zero_denominator"});
    assert!(serde_json::from_value::<relca_core::quality::Quality>(zero.clone()).is_err());
    assert!(
        fixture_match::validate_quality_wire(&zero, true).is_err(),
        "positive censor cannot establish D=0"
    );
}
#[test]
fn record_atomic_held_overflow_wire_and_metrics_transport() {
    for (s, q, day) in [
        (i::record(), i::record_expected(false), None),
        (i::record_wrong(), i::record_expected(true), None),
        (i::held(), i::held_expected(), Some("Day")),
        (i::overflow(), i::overflow_expected(), Some("Day")),
        (i::atomic(true), i::atomic_expected(true), None),
        (i::atomic(false), i::atomic_expected(false), None),
    ] {
        let v = cli(&s, None, day, false);
        assert_eq!(v["quality"]["quantiles"], q);
        i::h::assumptions(&v);
        let inv = fixture_match::Invocation::parse(None).unwrap();
        inv.compare(
            &json!({"verdict":v["verdict"],"quality":{"quantiles":q}}),
            &v,
        )
        .unwrap();
        assert_eq!(
            i::h::old_projection(v),
            i::h::old_projection(cli(&i::h::no_quantiles(&s), None, day, false))
        );
    }
}
#[test]
fn rule_quantiles_remain_withdrawn_across_the_configuration_scenario_product() {
    let s = i::rule();
    // Hand-fixed full product followed by its reverse, not learned from output.
    let selections = [
        ("AsIs", "Light"),
        ("AsIs", "Later"),
        ("ToBeB", "Light"),
        ("ToBeB", "Later"),
        ("ToBeB", "Later"),
        ("ToBeB", "Light"),
        ("AsIs", "Later"),
        ("AsIs", "Light"),
    ];
    assert_eq!(
        &selections[..4],
        &[
            ("AsIs", "Light"),
            ("AsIs", "Later"),
            ("ToBeB", "Light"),
            ("ToBeB", "Later"),
        ]
    );
    assert!(selections[..4].iter().eq(selections[4..].iter().rev()));
    let mut failures = vec![];
    for sim in [false, true] {
        for &(c, day) in &selections {
            if std::panic::catch_unwind(|| {
                let v = cli(&s, Some(c), Some(day), sim);
                i::assert_rule_report(&v, sim, c == "AsIs");
            })
            .is_err()
            {
                failures.push((sim, c, day));
            }
        }
    }
    assert!(failures.is_empty(), "Rule withdrawal failures {failures:?}");
}
#[test]
fn rule_legacy_quantiles_remain_withdrawn_in_real_check_commands() {
    let s = i::rule_legacy();
    for c in ["AsIs", "ToBeB"] {
        i::assert_rule_route(&s, c, None, true);
        let v = cli(&s, Some(c), None, false);
        i::assert_rule_report(&v, false, c == "AsIs");
        fixture_match::Invocation::parse(Some(&json!({"command":"check","configuration":c})))
            .unwrap()
            .compare(
                &json!({"verdict":v["verdict"],"quality":{"quantiles":i::rule_expected(false, c == "AsIs")}}),
                &v,
            )
            .unwrap();
        assert_eq!(
            i::h::old_projection(v),
            i::h::old_projection(cli(&i::h::no_quantiles(&s), Some(c), None, false))
        );
    }
}
