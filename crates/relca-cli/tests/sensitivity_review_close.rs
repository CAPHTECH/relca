//! Actual CLI suites, independently hand-derived A2/A3 and review negatives.
#[path = "support/sensitivity_harness.rs"]
mod h;
#[path = "../../relca-core/tests/support/sensitivity_review_inputs.rs"]
mod i;
use serde_json::{Value, json};

fn suite(points: &[(&str, &str)], simulation: bool, queries: usize) -> h::Output {
    let (d, mut m) = h::custom(points, simulation);
    m["targets"] = json!(
        (1..=queries)
            .map(|n| json!({"kind":"probability_query","id":format!("probability reached #{n}")}))
            .collect::<Vec<_>>()
    );
    h::run_custom(
        &d,
        &m,
        if simulation {
            "simulation"
        } else {
            "exhaustive"
        },
    )
}

fn complete(out: &h::Output, count: usize) {
    h::runner(&out.value);
    assert_eq!(out.exit, 0, "{}", out.stderr);
    // A2 deliberately declares no responsibilities for its two vetoes and
    // posting effect. Those warnings are not gate refusal or failed analysis.
    assert!(out.stderr.lines().all(|line| line.contains(": W-RESP-0: ")));
    assert_eq!(out.value["scope"]["planned"], count);
    assert_eq!(out.value["scope"]["evaluated"], count);
    assert_eq!(out.value["scope"]["refused"], 0);
    assert_eq!(out.value["scope"]["not_run"], 0);
    assert_eq!(out.value["scope"]["all_points_evaluated"], true);
}

#[test]
fn a2_suite_accepts_common_cause_and_ordered_miss_correlation() {
    let sources = [i::a2("independent"), i::a2("common"), i::a2("ordered")];
    let out = suite(
        &[
            ("independent", &sources[0]),
            ("common", &sources[1]),
            ("ordered", &sources[2]),
        ],
        false,
        4,
    );
    complete(&out, 3);
    for (n, wrong, joint) in [
        (0, "1/5000", "1/100"),
        (1, "1/500", "1/10"),
        (2, "1/500", "1/10"),
    ] {
        let point = &out.value["points"][n];
        for (q, p) in [(1, wrong), (2, joint), (3, "1/10"), (4, "1/10")] {
            let r = i::property(&point["report"], &format!("probability reached #{q}"));
            assert_eq!(r["probability"], json!({"min":p,"max":p}));
        }
    }
    for (n, min, max) in [
        (0, "1/5000", "1/500"),
        (1, "1/100", "1/10"),
        (2, "1/10", "1/10"),
        (3, "1/10", "1/10"),
    ] {
        assert_eq!(out.value["summaries"][n]["kind"], "finite_point_extrema");
        assert_eq!(
            out.value["summaries"][n]["value"],
            json!({"min":min,"max":max})
        );
    }
    let inputs = &out.value["points"][1]["effective_inputs"]["capabilities"];
    for operation in inputs
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kernel"].is_object())
    {
        assert_eq!(
            operation["kernel"]["given"],
            json!(["actor.latent.Cause.bad"])
        );
        assert_eq!(
            operation["kernel_reads_latent"],
            json!(["actor.latent.Cause.bad"])
        );
    }
}

#[test]
fn a2_observation_read_change_is_a_contract_mismatch_not_a_kernel_assumption() {
    let source = i::a2("independent");
    let changed = source.replacen(
        "capability check(e: Entry) -> Verdict { reads [Record];",
        "capability check(e: Entry) -> Verdict {",
        1,
    );
    assert_ne!(source, changed);
    let out = suite(
        &[("base", &source), ("changed_observation", &changed)],
        false,
        1,
    );
    h::reject(&out, 3, "E-CMP-0", "sensitivity_contract_mismatch");
    assert_eq!(out.value["scope"]["evaluated"], 0);
    h::no_executed_prefix(&out.value);
}

#[test]
fn a3_four_explicit_points_keep_expected_counts_and_label_only_control() {
    let sources = [
        i::a3(false, false),
        i::a3(true, false),
        i::a3(false, true),
        i::a3(true, true),
    ];
    let out = suite(
        &[
            ("normal", &sources[0]),
            ("busy", &sources[1]),
            ("degraded", &sources[2]),
            ("busy_degraded", &sources[3]),
        ],
        false,
        1,
    );
    complete(&out, 4);
    for (n, count, cases, rate) in [
        (0, "1/100", 1, "1/100"),
        (1, "1/50", 2, "1/100"),
        (2, "1/25", 1, "1/25"),
        (3, "2/25", 2, "1/25"),
    ] {
        i::assert_a3(&out.value["points"][n]["report"], count, cases, rate);
    }
    let labels = suite(
        &[
            ("ordinary", &sources[1]),
            ("quality_not_changed_by_busy_label", &sources[1]),
        ],
        false,
        1,
    );
    complete(&labels, 2);
    assert_eq!(
        labels.value["points"][0]["report"],
        labels.value["points"][1]["report"]
    );
}

#[test]
fn m8_distribution_only_change_remains_a_valid_suite() {
    let source = i::a2("independent");
    let changed = source.replace("true: 0.02, false: 0.98", "true: 0.04, false: 0.96");
    let out = suite(&[("base", &source), ("changed", &changed)], false, 1);
    complete(&out, 2);
    assert_eq!(
        i::property(&out.value["points"][1]["report"], "probability reached #1")["probability"],
        json!({"min":"1/2500","max":"1/2500"})
    );
}

#[test]
fn m8_distribution_nondet_switch_refuses_both_orders_without_executed_prefix() {
    let source = i::a2("independent");
    let nondet = source.replace("wrong: bool ~ { true: 0.02, false: 0.98 }", "wrong: bool");
    let outputs = [
        [
            ("distribution", source.as_str()),
            ("nondet", nondet.as_str()),
        ],
        [
            ("nondet", nondet.as_str()),
            ("distribution", source.as_str()),
        ],
    ]
    .map(|points| suite(&points, false, 1));
    assert_eq!(
        [outputs[0].exit, outputs[1].exit],
        [3, 3],
        "both orders must reject the changed question, not evaluate a nondet envelope"
    );
    for out in &outputs {
        h::reject(out, 3, "E-CMP-0", "sensitivity_contract_mismatch");
        assert_eq!(out.value["scope"]["evaluated"], 0);
        h::no_executed_prefix(&out.value);
    }
}

#[test]
fn m1_distribution_traffic_has_null_fixed_arrivals_and_complete_rational_support() {
    let source = i::traffic();
    let out = suite(&[("distributed", &source)], true, 1);
    complete(&out, 1);
    let traffic = &out.value["points"][0]["effective_inputs"]["traffic"];
    assert_eq!(
        traffic["distribution"]["entries"],
        json!([
            {"times":[2,2],"probability":"1/4"},{"times":[2,5],"probability":"3/4"}
        ])
    );
    assert_eq!(
        traffic["distribution"]["basis"],
        "hand assumption; not measured"
    );
    assert_eq!(
        traffic.get("arrivals"),
        Some(&Value::Null),
        "a distribution must not claim one fixed schedule"
    );
}

#[test]
fn u1_later_child_preexecution_refusal_must_not_execute_a_valid_prefix() {
    let distribution = i::with_quality(&i::traffic());
    let fixed = distribution.replace(i::ARRIVAL_DECL, "arrivals fixed [2min, 2min]");
    // Independent reachability witnesses: same-question projection accepts this
    // actual pair; fixed samples, distribution+quality is rejected before driver.
    let contract = |s: &str| {
        let m = relca_core::lower::lower(&relca_core::syntax::parse(s).unwrap()).unwrap();
        relca_core::sensitivity::contract::Contract::from_model(&m).unwrap()
    };
    assert!(
        contract(&fixed)
            .differences(&contract(&distribution))
            .is_empty()
    );
    let good = serde_json::to_value(relca_core::report::simulate(&fixed)).unwrap();
    assert_eq!(good["ledger"]["execution"]["status"], "ran");
    let bad = serde_json::to_value(relca_core::report::simulate(&distribution)).unwrap();
    assert_eq!(bad["ledger"]["execution"]["status"], "not_run");
    assert!(bad["diagnostics"].as_array().unwrap().iter().any(
        |d| d["code"] == "U-SEM-0" && d["message"] == "stochastic_arrivals_feature_combination"
    ));
    let out = suite(
        &[
            ("valid_first", &fixed),
            ("static_refusal_last", &distribution),
        ],
        true,
        1,
    );
    h::runner(&out.value);
    assert_eq!(out.exit, 4);
    assert_eq!(out.value["scope"]["planned"], 2);
    assert_eq!(out.value["scope"]["refused"], 1);
    assert_eq!(
        out.value["scope"]["evaluated"], 0,
        "static refusals must be known BEFORE executing the prefix"
    );
    assert_eq!(out.value["scope"]["not_run"], 1);
    h::no_executed_prefix(&out.value);
}
