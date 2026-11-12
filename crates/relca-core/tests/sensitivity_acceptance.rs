//! U8 front half only. Missing E9b is a runtime RED, not a compilation failure.
#[path = "support/sensitivity_oracle.rs"]
mod o;
use relca_core::{Verdict, engine, lower, reference, report, syntax};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn check(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}
fn stages(s: &str, want: &str) {
    let m = lower::lower(&syntax::parse(s).expect("old DSL parse substrate"))
        .expect("old DSL lower substrate");
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for out in [&e, &r.outcome] {
        assert!(!out.truncated);
        let p = out
            .properties
            .iter()
            .find(|p| p.probability.is_some())
            .unwrap()
            .probability
            .as_ref()
            .unwrap();
        assert_eq!(
            (p.min.to_string(), p.max.to_string()),
            (want.into(), want.into())
        );
        assert_eq!(out.properties.last().unwrap().verdict, Verdict::Satisfied);
    }
}
#[test]
fn green_old_models_have_independent_hand_probabilities_in_both_engines() {
    for (id, src) in [("base", o::BASE), ("high", o::HIGH), ("miss", o::MISS)] {
        let h = o::hand();
        let digest = Sha256::digest(src.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(json!(digest), h["source_sha256"][id]);
        let h = &h["values"][id];
        stages(src, h["wrong"].as_str().unwrap());
        let v = check(src);
        assert_eq!(v["verdict"], "satisfied");
        assert_eq!(
            o::property(&v, o::QUERY)["probability"],
            json!({"min":h["wrong"],"max":h["wrong"]})
        );
        for (end, p) in [("accepted", &h["accepted"]), ("rejected", &h["rejected"])] {
            let t = v["terminals"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["end"] == end)
                .unwrap();
            assert_eq!(t["probability"], json!({"min":p,"max":p}));
            for field in ["elapsed", "work"] {
                assert_eq!(t[field], json!({"min":1,"max":1}));
            }
        }
    }
}
#[test]
fn green_nonmonotone_endpoints_and_middle_are_not_a_robust_oracle() {
    for (s, p) in [
        (include_str!("data/sensitivity/p0.relca"), "0"),
        (include_str!("data/sensitivity/phalf.relca"), "1/4"),
        (include_str!("data/sensitivity/p1.relca"), "0"),
    ] {
        stages(s, p);
    }
}
#[test]
fn green_no_implicit_complement_and_old_cut_exit_contract() {
    let bad = o::BASE.replace("0.20: pass; 0.80: fail", "0.20: pass");
    assert_ne!(bad, o::BASE);
    let v = check(&bad);
    assert_eq!(v["verdict"], "invalid");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-PROB-0")
    );
    assert!(v["terminals"].as_array().unwrap().is_empty());
    let cut = check(&o::BASE.replace("clock 4min", "clock 0min"));
    assert_eq!(cut["verdict"], "inconclusive");
    assert_eq!(cut["bounds"]["truncated"], true);
    assert_eq!(Verdict::Inconclusive.exit_code(), 2);
    assert_eq!(Verdict::Invalid.exit_code(), 3);
}
#[test]
fn green_single_simulation_keeps_local_cp_interval_and_verdict() {
    let v = serde_json::to_value(report::simulate(o::ZERO)).unwrap();
    o::zero_statistics(&v, 1, "1/16", "1/2", "satisfied").unwrap();
    assert_eq!(Verdict::Satisfied.exit_code(), 0);
    let two = serde_json::to_value(report::simulate(include_str!(
        "data/sensitivity/zero-two-cases.relca"
    )))
    .unwrap();
    o::zero_statistics(&two, 2, "1/32", "607705/1048576", "inconclusive").unwrap();
    let cases = o::property(&two, o::QUERY)["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 2);
    for c in cases {
        assert_eq!(c["trials"], 4);
        assert_eq!(c["hits"], 0);
    }
}
#[test]
fn green_cp_grid_oracle_and_abort_key_sequence_are_independent() {
    let bytes = include_str!("data/sensitivity/values.json").as_bytes();
    let digest = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(json!(digest), o::hand()["values_manifest_sha256"]);
    let grid = 1u128 << 20;
    let hi = 607705u128;
    assert!((grid - hi).pow(4) * 32 <= grid.pow(4));
    assert!((grid - (hi - 1)).pow(4) * 32 > grid.pow(4));
    let hi = 677849u128;
    assert!((grid - hi).pow(4) * 64 <= grid.pow(4));
    assert!((grid - (hi - 1)).pow(4) * 64 > grid.pow(4));
    for (factor, expected) in [
        (16, std::cmp::Ordering::Less),
        (32, std::cmp::Ordering::Greater),
    ] {
        assert_eq!((6561u128 * factor).cmp(&160000), expected);
    }
    let picks: Vec<_> = (1..=3)
        .map(|rep| {
            let key = format!("relca-sim-v1|seed=0|rep={rep}|case=1|site=kernel.P.a|n=1");
            let bytes = Sha256::digest(key.as_bytes());
            u64::from_be_bytes(bytes[..8].try_into().unwrap()) < (1u64 << 63)
        })
        .collect();
    assert_eq!(picks, [false, false, true]);
    let v = serde_json::to_value(report::simulate(o::ABORT)).unwrap();
    assert_eq!(v["verdict"], "unsupported");
    assert_eq!(v["simulation"]["attempted_replications"], 3);
    assert_eq!(v["simulation"]["completed_replications"], 2);
}
#[test]
fn green_five_wire_faults_are_rejected_by_independent_predicates() {
    let v = o::finite_control();
    let ids = ["base", "high", "miss"];
    o::suite(&v, &ids, [3, 0, 0], "satisfied", Some(("1/250", "1/125"))).unwrap();
    let mut cartesian = v.clone();
    cartesian["points"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"high_miss","status":"evaluated"}));
    assert!(
        o::suite(
            &cartesian,
            &ids,
            [3, 0, 0],
            "satisfied",
            Some(("1/250", "1/125"))
        )
        .is_err()
    );
    let mut robust = v.clone();
    robust["scope"]["robust_guarantee"] = json!(true);
    assert!(
        o::suite(
            &robust,
            &ids,
            [3, 0, 0],
            "satisfied",
            Some(("1/250", "1/125"))
        )
        .is_err()
    );
    let mut worst = v.clone();
    worst["summaries"][0]["kind"] = json!("worst_case");
    assert!(
        o::suite(
            &worst,
            &ids,
            [3, 0, 0],
            "satisfied",
            Some(("1/250", "1/125"))
        )
        .is_err()
    );
    let z = o::zero_control();
    o::zero_statistics(&z, 2, "1/32", "607705/1048576", "inconclusive").unwrap();
    let mut stop = z;
    stop["simulation"]["multiplicity"]["rows"] = json!(1);
    stop["simulation"]["multiplicity"]["alpha_after"] = json!("1/16");
    assert!(o::zero_statistics(&stop, 2, "1/32", "607705/1048576", "inconclusive").is_err());
    let mut pooled = o::zero_control();
    pooled["properties"][0]["cases"][0]["trials"] = json!(8);
    assert!(o::zero_statistics(&pooled, 2, "1/32", "607705/1048576", "inconclusive").is_err());
    let mut prefix = v;
    prefix["scope"]["planned"] = json!(257);
    assert!(
        o::suite(
            &prefix,
            &ids,
            [3, 0, 0],
            "satisfied",
            Some(("1/250", "1/125"))
        )
        .is_err()
    );
    // Complement is an input fault, not an artificial output fault (separate GREEN above).
}
#[test]
fn green_e9b_defer_response_and_suppression_inputs_are_already_valid() {
    let v = check(include_str!("data/sensitivity/defer.relca"));
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(v["metrics"]["overall"]["work"]["reason"], "not_configured");
    assert_eq!(v["metrics"]["overall"]["work"]["min"], Value::Null);
    let s = include_str!("../../relca-cli/tests/snapshots/inputs/response-g-unknown.relca");
    let v = check(s);
    assert_eq!(v["verdict"], "inconclusive");
    assert_eq!(v["bounds"]["truncated"], false);
    assert_eq!(
        v["case_outcomes"][0]["terminals"][0]["kind"],
        "receipt_unknown"
    );
    for src in [
        include_str!("../../../fixtures/lin-basis/model.relca"),
        include_str!("../../../fixtures/lin-tacit/model.relca"),
        include_str!("../../../fixtures/lin-correlation/model.relca"),
    ] {
        let v = check(src);
        assert_eq!(v["verdict"], "satisfied");
        assert!(
            v["agency"]["model_uncertainties"]["items"]
                .as_array()
                .is_some_and(|v| !v.is_empty())
        );
    }
}
#[test]
fn red_e9b_probability_and_deterministic_leaf_classifications() {
    o::uncertainty(
        &check(o::BASE),
        [
            ("available", None),
            ("not_applicable", Some("exhaustive_analysis")),
            ("available", None),
        ],
    )
    .unwrap();
    o::uncertainty(
        &check(include_str!("../../../fixtures/serial-time/model.relca")),
        [
            ("not_configured", Some("no_probabilistic_model")),
            ("not_applicable", Some("exhaustive_analysis")),
            ("available", None),
        ],
    )
    .unwrap();
}
#[test]
fn red_e9b_simulation_zero_observations_are_not_true_rate_zero() {
    let v = serde_json::to_value(report::simulate(o::ZERO)).unwrap();
    o::uncertainty(&v, [("available", None); 3]).unwrap();
    o::zero_statistics(&v, 1, "1/16", "1/2", "satisfied").unwrap();
    assert!(
        v["uncertainty"]["estimation"]["items"]
            .to_string()
            .contains("/"),
        "must link to n/CI evidence"
    );
}
#[test]
fn red_e9b_parse_failure_is_unavailable_not_empty_known_model() {
    o::uncertainty(
        &check("not a model"),
        [
            ("unavailable", Some("not_lowered")),
            ("not_applicable", Some("exhaustive_analysis")),
            ("unavailable", Some("not_lowered")),
        ],
    )
    .unwrap();
}
#[test]
fn red_e9b_explain_flattened_and_aggregate_children_keep_ownership() {
    let s = include_str!("../../../fixtures/config-diff-parallel/model.relca");
    let d = serde_json::to_value(relca_core::diff::diff(s, "AsIs", "ToBeP")).unwrap();
    o::references(&d, &["/from/report".into(), "/to/report".into()]).unwrap();
    let c = serde_json::to_value(relca_core::compare::compare(
        s,
        &["ToBeP", "AsIs"],
        relca_core::compare::Analysis::Exhaustive,
    ))
    .unwrap();
    o::references(&c, &["/plans/0/report".into(), "/plans/1/report".into()]).unwrap();
    let e = serde_json::to_value(relca_core::explain::explain(o::BASE)).unwrap();
    o::uncertainty(
        &e,
        [
            ("available", None),
            ("not_applicable", Some("exhaustive_analysis")),
            ("available", None),
        ],
    )
    .unwrap();
    assert!(e["explanation"].get("uncertainty").is_none());
}
#[test]
fn red_e9b_decode_rejects_missing_unknown_and_inconsistent_classification() {
    let v = check(o::BASE);
    o::uncertainty(
        &v,
        [
            ("available", None),
            ("not_applicable", Some("exhaustive_analysis")),
            ("available", None),
        ],
    )
    .unwrap();
    assert!(serde_json::from_value::<report::Report>(v.clone()).is_ok());
    for (pointer, value) in [
        ("/uncertainty/rules", json!("future")),
        ("/uncertainty/model/status", json!("certain")),
        (
            "/uncertainty/aleatory/reason",
            json!("no_probabilistic_model"),
        ),
    ] {
        let mut bad = v.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            serde_json::from_value::<report::Report>(bad).is_err(),
            "escaped {pointer}"
        );
    }
    let mut bad = v;
    bad.as_object_mut().unwrap().remove("uncertainty");
    assert!(serde_json::from_value::<report::Report>(bad).is_err());
}
#[test]
fn green_uncertainty_oracle_rejects_single_classification_and_reference_faults() {
    let mut v = json!({"assumptions":[{"field":"schedule","value":"work-conserving"}],
        "uncertainty":{"rules":"uncertainty-p2-1",
            "aleatory":{"status":"not_configured","reason":"no_probabilistic_model","items":[]},
            "estimation":{"status":"not_applicable","reason":"exhaustive_analysis","items":[]},
            "model":{"status":"available","reason":null,"items":[{"kind":"assumption","at":"/assumptions/0"}]}}});
    let expected = [
        ("not_configured", Some("no_probabilistic_model")),
        ("not_applicable", Some("exhaustive_analysis")),
        ("available", None),
    ];
    o::uncertainty(&v, expected).unwrap();
    let mut bad = v.clone();
    bad["uncertainty"]["model"]["reason"] = json!("certain");
    assert!(o::uncertainty(&bad, expected).is_err());
    let mut bad = v.clone();
    bad["uncertainty"]["model"]["items"][0]["at"] = json!("/missing");
    assert!(o::uncertainty(&bad, expected).is_err());
    v["uncertainty"]["extra"] = json!([]);
    assert!(o::uncertainty(&v, expected).is_err());
}
#[test]
fn red_e9b_model_discloses_basis_tacit_and_correlation_not_only_widths() {
    for (kind, src) in [
        (
            "basis",
            include_str!("../../../fixtures/lin-basis/model.relca"),
        ),
        (
            "tacit",
            include_str!("../../../fixtures/lin-tacit/model.relca"),
        ),
        (
            "correlation",
            include_str!("../../../fixtures/lin-correlation/model.relca"),
        ),
    ] {
        let v = check(src);
        o::uncertainty(
            &v,
            [
                ("available", None),
                ("not_applicable", Some("exhaustive_analysis")),
                ("available", None),
            ],
        )
        .unwrap();
        let evidence = o::evidence_text(&v, &v["uncertainty"]["model"]["items"]);
        assert!(
            evidence.contains(kind),
            "suppression lost {kind}: {evidence}"
        );
        assert!(
            v["agency"]["model_uncertainties"]["items"]
                .as_array()
                .is_some_and(|v| !v.is_empty())
        );
    }
}
#[test]
fn red_e9b_cut_defer_and_receipt_unknown_do_not_become_zero_or_success() {
    let v = check(include_str!("data/sensitivity/defer.relca"));
    o::uncertainty(
        &v,
        [
            ("not_configured", Some("no_probabilistic_model")),
            ("not_applicable", Some("exhaustive_analysis")),
            ("available", None),
        ],
    )
    .unwrap();
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(v["metrics"]["overall"]["work"]["reason"], "not_configured");
    assert_eq!(v["metrics"]["overall"]["work"]["min"], Value::Null);
    assert!(o::evidence_text(&v, &v["uncertainty"]["model"]["items"]).contains("defer"));
    let s = include_str!("../../relca-cli/tests/snapshots/inputs/response-g-unknown.relca");
    let v = check(s);
    o::uncertainty(
        &v,
        [
            ("not_configured", Some("no_probabilistic_model")),
            ("not_applicable", Some("exhaustive_analysis")),
            ("available", None),
        ],
    )
    .unwrap();
    assert_eq!(v["verdict"], "inconclusive");
    assert_eq!(v["bounds"]["truncated"], false);
    assert!(o::evidence_text(&v, &v["uncertainty"]["model"]["items"]).contains("response_control"));
    assert_eq!(
        v["case_outcomes"][0]["terminals"][0]["kind"],
        "receipt_unknown"
    );
}
#[test]
fn red_e9b_mutation_root_references_only_real_reports() {
    let v = serde_json::to_value(relca_core::mutation::mutate(include_str!(
        "../../../fixtures/serial-time/model.relca"
    )))
    .unwrap();
    let mut children = vec!["/original".to_owned()];
    for (k, kind) in v["mutations"].as_array().unwrap().iter().enumerate() {
        for (n, m) in kind["mutants"].as_array().into_iter().flatten().enumerate() {
            if m.get("report").is_some_and(|r| !r.is_null()) {
                children.push(format!("/mutations/{k}/mutants/{n}/report"));
            }
        }
    }
    o::references(&v, &children).unwrap();
}
