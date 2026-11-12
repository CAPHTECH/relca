//! P3a test-first acceptance: comparison is not permission to lose unknowns.
#[path = "support/p3a_hand.rs"]
mod h;
#[path = "support/p3a_inputs.rs"]
mod i;
#[path = "support/response_loss_inputs.rs"]
mod q1;
use relca_core::{
    agency,
    compare::{self, Analysis},
    diff, lower, report, syntax,
};
use serde_json::{Value, json};
fn model(s: &str) -> relca_core::ir::Model {
    lower::lower(&syntax::parse(s).unwrap()).unwrap()
}
#[test]
fn r1a_unknown_terminal_dtos_are_closed_and_roundtrip_observed_quantities() {
    let v = json!({"probability":{"min":"1/100","max":"1/100"},"completed_at":null,"elapsed":null,"work":null,"result_at":null,
        "observed_until":{"min":2,"max":2},"observed_elapsed":{"min":2,"max":2},"observed_work":{"min":2,"max":2}});
    let t: compare::ExhaustiveTerminal =
        serde_json::from_value(v.clone()).expect("new observation fields are not unknown DTO keys");
    assert_eq!(json!(t), v);
    let t: diff::CaseValue = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(json!(t), v);
    let mut bad = v;
    bad["invented"] = json!(true);
    assert!(serde_json::from_value::<compare::ExhaustiveTerminal>(bad.clone()).is_err());
    assert!(serde_json::from_value::<diff::CaseValue>(bad).is_err());
}
macro_rules! comparison {
    ($test:ident,$input:literal,$cmp:literal) => {
        #[test]
        fn $test() {
            let s = i::pair($input);
            let ast = syntax::parse(&s).expect("paired hand grammar");
            for plan in ["A", "B"] {
                lower::lower_configuration(&ast, plan)
                    .expect("paired source is statically supported before comparison");
            }
            let v = if $cmp {
                json!(compare::compare(&s, &["A", "B"], Analysis::Exhaustive))
            } else {
                json!(diff::diff(&s, "A", "B"))
            };
            h::comparison(&v, $input, $cmp);
            if !$cmp {
                let r: diff::DiffReport = serde_json::from_value(v.clone()).unwrap();
                assert_eq!(json!(r), v);
            }
        }
    };
}
comparison!(r1_compare_g_resend, "g-resend", true);
comparison!(r1_diff_g_resend, "g-resend", false);
comparison!(r1_compare_modeled_resend, "modeled-resend", true);
comparison!(r1_diff_modeled_resend, "modeled-resend", false);
comparison!(r1_compare_g_unknown, "g-unknown", true);
comparison!(r1_diff_g_unknown, "g-unknown", false);
comparison!(r1_compare_g_probability, "g-probability", true);
comparison!(r1_diff_g_probability, "g-probability", false);
comparison!(r1_compare_k8_resend, "k8-g-resend", true);
comparison!(r1_diff_k8_resend, "k8-g-resend", false);
comparison!(r1_compare_k8_unknown, "k8-g-unknown", true);
comparison!(r1_diff_k8_unknown, "k8-g-unknown", false);
comparison!(r1_compare_k8_clock, "k8-g-clock", true);
comparison!(r1_diff_k8_clock, "k8-g-clock", false);

#[test]
fn r1_one_response_side_keeps_unknown_and_not_reported_distinct_from_zero() {
    let s = i::one_sided();
    // End inventory mismatch is still a comparison error: retry's exhausted
    // destination is a declared End, even though it is never reached here.
    let v = json!(compare::compare(&s, &["A", "B"], Analysis::Exhaustive));
    h::verdict(&v, "invalid");
    assert!(v["mismatch"].is_object());
    let v = json!(diff::diff(&s, "A", "B"));
    h::verdict(&v, "inconclusive");
    let u = h::row(
        &v["case_outcomes"][0]["terminals"],
        "kind",
        "receipt_unknown",
    );
    h::observed(&u["from"], 2, 2);
    assert_eq!(u.get("to"), Some(&Value::Null));
}
#[test]
fn r1_one_response_side_with_matching_ends_is_comparable_and_other_side_is_inapplicable() {
    let v = json!(compare::compare(
        &i::one_sided_matching_ends(),
        &["A", "B"],
        Analysis::Exhaustive
    ));
    h::verdict(&v, "inconclusive");
    let u = h::row(&v["tables"]["terminals"], "kind", "receipt_unknown");
    assert_eq!(u["values"][0]["status"], "reported");
    h::observed(&u["values"][0]["value"], 2, 2);
    assert_eq!(u["values"][1]["status"], "not_applicable");
    assert_eq!(u["values"][1].get("value"), Some(&Value::Null));
}

#[test]
fn r1_no_wait_table_is_invented_in_compare_or_diff() {
    let s = i::metrics(&i::pair("g-resend"), true);
    let c = json!(compare::compare(&s, &["A", "B"], Analysis::Exhaustive));
    let d = json!(diff::diff(&s, "A", "B"));
    h::comparison(&c, "g-resend", true);
    h::comparison(&d, "g-resend", false);
    for v in [&c["tables"], &d] {
        assert!(v.get("waiting").is_none());
        assert!(v.get("breakdown").is_none());
        assert!(h::rows(&v["metrics"]).iter().all(|r| {
            !r["key"]["metric"].as_str().unwrap().contains("breakdown")
                && !r["key"]["metric"].as_str().unwrap().contains("wait_causes")
        }));
    }
}

#[test]
fn r1_metrics_quality_keep_arrival_denominator_and_unknown_partition_in_both_tables() {
    let s = i::metrics(
        &i::append(
            &i::pair("g-unknown"),
            "property case.Registered = reached(registered == true)\nmetric cohort.RegisteredPerArrival = expected(count(case.Registered)) / arrivals",
        ),
        false,
    );
    for cmp in [true, false] {
        let v = if cmp {
            json!(compare::compare(&s, &["A", "B"], Analysis::Exhaustive))
        } else {
            json!(diff::diff(&s, "A", "B"))
        };
        h::verdict(&v, "inconclusive");
        let table = if cmp {
            &v["tables"]["metrics"]
        } else {
            &v["metrics"]
        };
        for (metric, n, count) in [
            ("counts.received", "1", true),
            ("counts.completed", "0", true),
            ("counts.unfinished", "1", true),
            ("counts.receipt_unknown_count", "1", true),
            ("work", "2", false),
            ("observed_cost_total", "6", false),
        ] {
            let rs: Vec<_> = h::rows(table)
                .iter()
                .filter(|r| r["key"]["scope"] == "overall" && r["key"]["metric"] == metric)
                .collect();
            assert_eq!(rs.len(), 1, "{metric}");
            let cells = if cmp {
                h::rows(&rs[0]["values"]).iter().collect::<Vec<_>>()
            } else {
                vec![&rs[0]["from"], &rs[0]["to"]]
            };
            for cell in cells {
                assert_eq!(cell["status"], "reported");
                assert_eq!(
                    if count {
                        &cell["value"]["stats"]
                    } else {
                        &cell["value"]
                    },
                    &h::scalar(n)
                );
            }
        }
        let table = if cmp {
            &v["tables"]["quality"]
        } else {
            &v["quality"]
        };
        for subject in ["receipt_unknown", "unfinished"] {
            let rs: Vec<_> = h::rows(table)
                .iter()
                .filter(|r| {
                    r["key"]["scope"] == "outcome"
                        && r["key"]["subject"] == subject
                        && r["key"]["field"] == "rate"
                })
                .collect();
            assert_eq!(rs.len(), 1);
            let cells = if cmp {
                h::rows(&rs[0]["values"]).iter().collect::<Vec<_>>()
            } else {
                vec![&rs[0]["from"], &rs[0]["to"]]
            };
            for c in cells {
                assert_eq!(c["status"], "reported");
                assert_eq!(c["value"]["lower"], "1");
                assert_eq!(c["value"]["upper"], "1");
            }
        }
        for child in h::children(&v, cmp) {
            let registered = h::row(&child["quality"]["cohort"], "name", "RegisteredPerArrival");
            assert_eq!(registered["denominator"]["count"], 1);
            assert_eq!(registered["value"]["lower"], "1");
        }
    }
}

#[test]
fn r1_root_fold_preserves_a_violated_child_and_an_unsupported_child() {
    let s = i::pair("modeled-resend");
    let at = s.find("process After").unwrap();
    let s = format!(
        "{}process After {{ ack = Other.choose(); if ack == committed {{end posted}} else {{end duplicate}}; if ack == committed {{end failed}} else {{}} }}\nconfiguration A {{cut C;process Before}}\nconfiguration B {{cut C;process After}}\n}}",
        &s[..at]
    );
    let s=i::append(&s,"actor Other { capacity 1; capability choose() -> Ack { duration fixed 1min; result nondet {committed,rejected_duplicate} } }\nsimulation replications 4 seed 7 confidence 0.5").replace("include [Ledger]","include [Ledger,Other]");
    let v = json!(compare::compare(&s, &["A", "B"], Analysis::Simulation));
    h::verdict(&v["plans"][0]["report"], "violated");
    h::verdict(&v["plans"][1]["report"], "unsupported");
    assert!(
        h::rows(&v["plans"][1]["report"]["diagnostics"])
            .iter()
            .any(|d| d["code"] == "U-SEM-0")
    );
    h::verdict(&v, "violated");
}

#[test]
fn r1_static_response_changes_are_original_command_meaning_not_fake_actors() {
    for kind in ["loss", "backoff", "attempts", "condition", "exhausted"] {
        let v = json!(diff::diff(&i::control_change(kind), "A", "B"));
        assert_ne!(v["verdict"], "unsupported", "{kind}: {}", v["diagnostics"]);
        assert_ne!(v["verdict"], "invalid", "{kind}: {}", v["diagnostics"]);
        let rs = h::rows(&v["differences"]);
        assert!(!rs.iter().any(|r| r["kind"] == "actor"));
        let r = h::row(&v["differences"], "target", "response.ack");
        assert_eq!(r["kind"], "meaning");
        assert_eq!(r["change"], "modified");
        assert_ne!(r["from"], r["to"]);
        assert_eq!(r["from"]["capability"], "Ledger.post");
        assert_eq!(r["from"]["idempotency"], "key");
        assert_eq!(r["from"].get("effect_scope"), Some(&Value::Null));
        assert_eq!(r["from"]["arguments"], json!(["case.id"]));
        assert_eq!(
            r["from"]["loss"],
            json!({"kind":"sequence","receipts":["unknown","delivered"]})
        );
        assert_eq!(
            r["from"]["retry"],
            json!({"max_attempts":2,"when":{"receipt_unknown":true,"results":[]},"backoff":0,"exhausted":"failed"})
        );
        let mut expected = r["from"].clone();
        match kind {
            "loss" => {
                expected["loss"] = json!({"kind":"sequence","receipts":["unknown","unknown"]})
            }
            "backoff" => expected["retry"]["backoff"] = json!(3),
            "attempts" => {
                expected["loss"] = json!({"kind":"sequence","receipts":["unknown"]});
                expected["retry"]["max_attempts"] = json!(1);
            }
            "condition" => {
                expected["retry"]["when"] =
                    json!({"receipt_unknown":false,"results":["rejected_duplicate"]})
            }
            "exhausted" => expected["retry"]["exhausted"] = json!("posted"),
            _ => unreachable!(),
        }
        assert_eq!(r["to"], expected, "exact one control change: {kind}");
        let round: diff::DiffReport = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(json!(round), v);
        let mut bad = r.clone();
        bad["from"]["extra"] = json!(true);
        assert!(serde_json::from_value::<diff::Difference>(bad).is_err());
    }
}

fn agency_diff(a: &report::Report, b: &report::Report) -> Value {
    fn side(r: &report::Report) -> agency::ComparisonSide<'_> {
        agency::ComparisonSide {
            agency: r
                .agency
                .as_ref()
                .and_then(agency::AgencyDerivation::as_derived),
            responsibilities: r.responsibilities.as_deref(),
        }
    }
    json!(agency::differences(side(a), side(b)))
}
#[test]
fn r1b_partial_agency_transports_response_reasons_both_directions_and_equal_unknowns() {
    let a = report::check(&i::agency(true));
    let b = report::check(&i::agency(false));
    h::verdict(&json!(a), "satisfied");
    h::verdict(&json!(b), "satisfied");
    let raw = json!(a);
    let lineages = h::rows(&raw["agency"]["lineage"]);
    assert_eq!(
        lineages.len(),
        2,
        "two real attempts, no extra judgement alias"
    );
    for j in lineages {
        assert_ne!(j["call"], "v");
        assert_eq!(j["subject_sources"], json!(["Original"]));
        assert_eq!(j["independent_sources"], json!(["Evidence"]));
    }
    assert_eq!(raw["agency"]["lineage_groups"][0]["judgements"], 2);
    assert_eq!(
        raw["agency"]["lineage_groups"][0]["independent_sources"],
        json!(["Evidence"])
    );
    let origin = h::row(&raw["agency"]["origin"], "actor", "Check");
    assert_eq!(origin["direct"], json!(["Evidence"]));
    assert_eq!(origin["indirect"], json!(["Original"]));
    for (from, to) in [(&a, &b), (&b, &a), (&a, &a)] {
        let v = agency_diff(from, to);
        assert_eq!(v["status"], "derived", "not a whole-table fallback: {v}");
        let rs = h::rows(&v["rows"]);
        if !std::ptr::eq(from, to) {
            let l = rs
                .iter()
                .find(|r| {
                    r["viewpoint"] == "retention"
                        && r["item"] == "normal"
                        && r["capability"] == "Check.run"
                })
                .expect("known real Start count difference");
            assert_eq!(l["status"], "changed");
            let (x, y) = if std::ptr::eq(from, &a) {
                (2, 1)
            } else {
                (1, 2)
            };
            assert_eq!(l["from"], json!({"min":1,"max":x}));
            assert_eq!(l["to"], json!({"min":1,"max":y}));
            let j = rs
                .iter()
                .find(|r| {
                    r["viewpoint"] == "lineage_groups"
                        && r["item"] == "judgements"
                        && r["subject_sources"] == json!(["Original"])
                })
                .expect("known real attempt J difference");
            assert_eq!(j["status"], "changed");
            assert_eq!(j["from"], x);
            assert_eq!(j["to"], y);
        }
        for (view, item) in [
            ("framing", "actors"),
            ("effects", "u"),
            ("effects", "concentration.single_actor"),
            ("responsibility", "refusal"),
        ] {
            let found: Vec<_> = rs
                .iter()
                .filter(|r| r["viewpoint"] == view && r["item"] == item)
                .collect();
            assert!(!found.is_empty(), "{view}/{item} missing: {v}");
            for r in found {
                assert_eq!(r["status"], "not_comparable");
                assert!(r.to_string().contains("response_control"));
            }
        }
        // O/J/E/L may be equal (no row), but must not be thrown away as unknown.
        for r in rs.iter().filter(|r| {
            matches!(
                r["viewpoint"].as_str(),
                Some("origin" | "lineage_groups" | "retention")
            ) || r["viewpoint"] == "effects" && r["item"] == "e"
        }) {
            assert_ne!(r["status"], "not_comparable", "known O/J/E/L: {r}");
        }
        serde_json::from_value::<agency::Differences>(v).unwrap();
    }
    let v = json!(a);
    for code in ["W-AGY-0", "W-AGY-2", "W-LIN-2", "W-RESP-2"] {
        let r = h::row(
            &v["assumptions"],
            "field",
            &format!("agency.diagnostics.{code}"),
        );
        assert_eq!(r["value"], "unchecked(response_control)");
    }
    assert!(
        h::rows(&v["agency"]["lineage"])
            .iter()
            .all(|r| r["call"] != "ack")
    );
}

macro_rules! waits {
    ($test:ident,$path:literal,$sample:literal) => {
        #[test]
        fn $test() {
            let s = i::wait($path);
            let v = if $sample {
                json!(report::simulate(&s))
            } else {
                json!(report::check(&s))
            };
            h::waiting(&v, $path, $sample);
            let m = model(&s);
            if $sample {
                let reference = relca_core::reference::simulate(&m);
                assert_eq!(reference.stop, relca_core::reference::StopReason::Complete);
                for out in [relca_core::engine::simulate(&m), reference.outcome] {
                    assert_eq!(out.completed_replications, 4);
                    for rep in out.replications {
                        let metrics = json!(rep.metrics.unwrap());
                        assert_eq!(
                            metrics["overall"]["work"],
                            h::scalar(if $path == "D" { "1" } else { "2" })
                        );
                        assert_eq!(
                            metrics["overall"]["counts"]["received"]["stats"],
                            h::scalar("1")
                        );
                        if $path != "UU" {
                            let t = h::row(
                                &metrics["cases"][0]["terminals"],
                                "end",
                                if $path == "D" { "posted" } else { "duplicate" },
                            );
                            assert_eq!(
                                t["breakdown"]["in_service"],
                                h::scalar(if $path == "D" { "1" } else { "2" })
                            );
                            assert_eq!(
                                t["breakdown"]["intentional"],
                                h::scalar(if $path == "D" { "0" } else { "3" })
                            );
                        }
                    }
                }
            } else {
                let reference = relca_core::reference::explore(&m);
                assert_eq!(reference.stop, relca_core::reference::StopReason::Complete);
                for metrics in [
                    json!(relca_core::engine::explore(&m).metrics),
                    json!(reference.outcome.metrics),
                ] {
                    let mut v = v.clone();
                    v["metrics"] = metrics;
                    h::waiting(&v, $path, false);
                }
            }
        }
    };
}
waits!(r1c_check_d, "D", false);
waits!(r1c_check_ud, "UD", false);
waits!(r1c_check_uu, "UU", false);
waits!(r1c_simulate_d, "D", true);
waits!(r1c_simulate_ud, "UD", true);
waits!(r1c_simulate_uu, "UU", true);

#[test]
fn r1_contest_and_quantile_crossings_remain_reasoned_refusals() {
    let contest=i::append(&q1::source("g-resend"),r#"people [p]
      actor Party { control party; standing over [end.posted] basis "assumed standing" }
      actor Reply { executors [p]; capability run() -> Ack { duration fixed 1min; result nondet { committed } } }
      contract X { contest Party on end.posted route Reply.run reversible_by [p] respond_by 2min }"#).replace("include [Ledger]","include [Ledger,Party,Reply]");
    let quantile = i::append(
        &q1::source("g-resend"),
        "property case.Done = reached(end.posted || end.duplicate)\nmetric quantile.Done = quantile(0.5, case.Done)",
    );
    for s in [contest, quantile] {
        let v = json!(report::check(&s));
        h::verdict(&v, "unsupported");
        assert!(
            h::rows(&v["diagnostics"])
                .iter()
                .any(|d| d["code"] == "U-SEM-0"
                    && d["message"]
                        .as_str()
                        .unwrap()
                        .contains("retry_feature_combination")),
            "{v}"
        );
    }
}
