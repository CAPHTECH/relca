//! ADR0047 acceptance, hand expectations are fixed before production changes.
#[path = "support/evaluation_acceptance.rs"]
mod h;
use serde_json::Value;

#[test]
fn every_evaluation_fixture_collects_all_hand_assertion_failures() {
    let mut failures = vec![];
    for n in h::names() {
        if std::panic::catch_unwind(|| h::hand(&n, &h::run(&n, &h::source(&n)))).is_err() {
            failures.push(n);
        }
    }
    assert!(failures.is_empty(), "all ADR0047 failures: {failures:?}");
}

#[test]
fn exact_edit_witnesses_and_invocation_controls_are_not_noop_replacements() {
    let pairs: Value = serde_json::from_str(include_str!("support/evaluation_pairs.json")).unwrap();
    for p in pairs.as_array().unwrap() {
        let a = h::source(p["base"].as_str().unwrap());
        let b = h::source(p["broken"].as_str().unwrap());
        if p["invocation_only"] == true {
            assert_eq!(a, b);
            let i = h::inventory();
            assert_ne!(
                i[p["base"].as_str().unwrap()]["invocation"],
                i[p["broken"].as_str().unwrap()]["invocation"]
            );
        } else {
            let (start, end) = (
                p["start"].as_u64().unwrap() as usize,
                p["end"].as_u64().unwrap() as usize,
            );
            assert_eq!(&a[start..end], p["old"].as_str().unwrap());
            let mut changed = a.clone();
            changed.replace_range(start..end, p["new"].as_str().unwrap());
            assert_ne!(changed, a);
            assert_eq!(changed, b);
        }
    }
}

#[test]
fn old_language_substrates_have_only_the_predicted_verify_verdicts_and_quality() {
    let inventory = h::inventory();
    let mut failures = vec![];
    for n in h::names() {
        if inventory[&n]["code"].is_string() || inventory[&n]["mutation"] == true {
            continue;
        }
        let result = std::panic::catch_unwind(|| {
            let s = h::source(&n);
            let plain = h::without_declaration(&s);
            assert_ne!(plain, s);
            let v = h::run(&n, &plain);
            let e = h::expected(&n);
            let expected = e["evaluation"]["plans"].as_array().unwrap();
            for (p, want) in v["plans"].as_array().unwrap().iter().zip(expected) {
                assert_eq!(p["configuration"], want["configuration"], "{n}");
                assert_eq!(
                    p["report"]["verdict"], want["verify_verdict"],
                    "{n}: unrelated substrate refusal: {}",
                    p["report"]["diagnostics"]
                );
            }
            for hard in e["evaluation"]["hard"].as_array().unwrap() {
                for (j, p) in v["plans"].as_array().unwrap().iter().enumerate() {
                    let q = p["report"]["quality"]["cohort"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|q| q["name"] == hard["cohort"])
                        .unwrap();
                    let want = &hard["values"][j];
                    if inventory[&n]["invocation"]["mode"] == "simulation" {
                        if n.starts_with("cmp7-nondyadic") {
                            let property = p["report"]["quality"]["properties"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .find(|p| p["name"] == "Wrong")
                                .unwrap();
                            assert_eq!(
                                property["cases"],
                                serde_json::json!([{"case_id":1,"arrival":0,"hits":2,"unknown":0,"trials":4}]),
                                "spec-derived fair SHA256 draws"
                            );
                        }
                        assert_eq!(
                            q["value"]["point"], want["point"],
                            "{n}:source point hand oracle"
                        );
                        let source_reason = if q["value"]["reason"] == want["reason"] {
                            Value::Null
                        } else {
                            q["value"]["reason"].clone()
                        };
                        assert_eq!(
                            source_reason, want["source_reason"],
                            "{n}:source reason hand oracle"
                        );
                    } else {
                        let raw = if q["value"]["lower"].is_null() {
                            &q["bounds_with_undetermined"]
                        } else {
                            &q["value"]
                        };
                        let interval = if raw.is_null() {
                            Value::Null
                        } else {
                            serde_json::json!({"lower":raw["lower"],"upper":raw["upper"]})
                        };
                        assert_eq!(interval, want["interval"], "{n}:B6 hand interval");
                        assert_eq!(q["value"]["reason"], want["reason"], "{n}:B6 hand reason");
                    }
                }
            }
            for objective in e["evaluation"]["objectives"].as_array().unwrap() {
                let target = objective["objective"]
                    .as_str()
                    .unwrap()
                    .split_once(' ')
                    .unwrap()
                    .1;
                for (j, p) in v["plans"].as_array().unwrap().iter().enumerate() {
                    let want = &objective["values"][j];
                    if let Some(field) = target.strip_prefix("metrics.overall.") {
                        let raw = &p["report"]["metrics"]["overall"][field];
                        if p["report"].get("metrics").is_none() {
                            assert!(want["lower"].is_null() && want["upper"].is_null());
                            assert_eq!(want["reason"], "not_configured");
                            continue;
                        }
                        if inventory[&n]["invocation"]["mode"] == "simulation" {
                            assert_eq!(raw, &want["sample"], "{n}:existing Sample hand oracle");
                        } else {
                            assert_eq!(
                                raw["min"], want["lower"],
                                "{n}:objective lower hand oracle"
                            );
                            assert_eq!(
                                raw["max"], want["upper"],
                                "{n}:objective upper hand oracle"
                            );
                        }
                        assert_eq!(raw["reason"], want["reason"], "{n}:objective reason");
                    } else {
                        let name = target.strip_prefix("cohort.").unwrap();
                        let raw = &p["report"]["quality"]["cohort"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|q| q["name"] == name)
                            .unwrap()["value"];
                        for field in ["lower", "upper", "basis", "reason"] {
                            assert_eq!(
                                raw[field], want[field],
                                "{n}:cohort objective {field} hand oracle"
                            );
                        }
                    }
                }
            }
        });
        if result.is_err() {
            failures.push(n);
        }
    }
    assert!(
        failures.is_empty(),
        "unrelated substrate failures: {failures:?}"
    );
}

#[test]
fn declaration_does_not_mutate_nested_reports_rng_or_verify_confidence() {
    let inventory = h::inventory();
    let mut failures = vec![];
    for n in h::names() {
        if inventory[&n]["code"].is_string() || inventory[&n]["mutation"] == true {
            continue;
        }
        let result = std::panic::catch_unwind(|| {
            let s = h::source(&n);
            let v = h::run(&n, &s);
            assert!(
                v.get("evaluation").is_some(),
                "exercise real declaration before projection"
            );
            let iv = &inventory[&n]["invocation"];
            for p in v["plans"].as_array().unwrap() {
                let name = p["configuration"].as_str().unwrap();
                let alone = h::standalone(
                    &s,
                    name,
                    iv["mode"] == "simulation",
                    iv["scenario"].as_str(),
                );
                assert_eq!(
                    p["report"], alone,
                    "{n}/{name}: complete report,not merely intervals"
                );
            }
        });
        if result.is_err() {
            failures.push(n);
        }
    }
    assert!(
        failures.is_empty(),
        "nested report isolation failures: {failures:?}"
    );
}

#[test]
fn tables_agency_transport_keeps_both_sides_responsibilities_and_declared_direction() {
    use relca_core::agency::{ComparisonSide, differences};
    for n in ["eval-agency", "eval-agency.broken-swapped"] {
        let v = h::run(n, &h::source(n));
        let p = v["plans"].as_array().unwrap();
        // JSON is transported through the existing public C5 type,not re-derived here.
        let a: relca_core::report::Report = serde_json::from_value(p[0]["report"].clone()).unwrap();
        let b: relca_core::report::Report = serde_json::from_value(p[1]["report"].clone()).unwrap();
        let want = serde_json::to_value(differences(
            ComparisonSide {
                agency: a.agency.as_ref(),
                responsibilities: a.responsibilities.as_deref(),
            },
            ComparisonSide {
                agency: b.agency.as_ref(),
                responsibilities: b.responsibilities.as_deref(),
            },
        ))
        .unwrap();
        assert_eq!(want["status"], "derived");
        assert_eq!(
            want["rows"],
            serde_json::json!([]),
            "independent M call inventory"
        );
        assert_eq!(v["tables"]["agency"][0]["differences"], want);
    }
}

#[test]
fn t12_full_bytes_after_bijective_names_and_only_old_metric_key_permutation() {
    let a = h::run("eval-contention", &h::source("eval-contention"));
    let b = h::run("eval-label", &h::source("eval-label"));
    h::hand("eval-contention", &a);
    h::hand("eval-label", &b);
    h::label_byte_projection(
        &serde_json::to_vec(&a).unwrap(),
        &serde_json::to_vec(&b).unwrap(),
    );
}

#[test]
fn u16_other_commands_only_check_names_and_types_not_hard_under_or_objectives() {
    use relca_core::{diff, explain, mutation, report};
    let mut failures = vec![];
    for command in [
        "check",
        "simulate",
        "outside-under",
        "diff",
        "explain",
        "mutate",
    ] {
        let result = std::panic::catch_unwind(|| {
            let name = match command {
                "simulate" => "eval-sim",
                "outside-under" => "cmp7-under",
                "diff" | "explain" | "mutate" => "eval-agency",
                _ => "eval-hard",
            };
            let source = h::source(name);
            let plain = h::without_declaration(&source);
            assert_ne!(plain, source);
            let run = |s: &str| -> Value {
                match command {
                    "check" => {
                        serde_json::to_value(report::check_configuration(s, "Fast")).unwrap()
                    }
                    "simulate" => {
                        serde_json::to_value(report::simulate_configuration(s, "Bad")).unwrap()
                    }
                    "outside-under" => serde_json::to_value(report::check_with_selection(
                        s,
                        Some("Check"),
                        Some("Outside"),
                    ))
                    .unwrap(),
                    "diff" => serde_json::to_value(diff::diff(s, "AsIs", "ToBeP")).unwrap(),
                    "explain" => serde_json::to_value(explain::explain(s)).unwrap(),
                    "mutate" => serde_json::to_value(mutation::mutate(s)).unwrap(),
                    _ => unreachable!(),
                }
            };
            let mut with = run(&source);
            let mut without = run(&plain);
            let paths: Vec<_> = match command {
                "diff" => vec!["/from/report/model/sha256", "/to/report/model/sha256"],
                // ExplainReport flattens its report, so explain shares the root pointers.
                "mutate" => vec!["/original/model/sha256"],
                _ => vec!["/model/sha256"],
            };
            let verdict_path = "/verdict";
            let verdict = if ["explain", "mutate"].contains(&command) {
                "invalid"
            } else {
                "satisfied"
            };
            assert_eq!(
                without.pointer(verdict_path).unwrap(),
                verdict,
                "{command}:old control"
            );
            assert_eq!(
                with.pointer(verdict_path).unwrap(),
                verdict,
                "{command}:no compare verdict leak"
            );
            for p in paths {
                assert_ne!(
                    with.pointer(p),
                    without.pointer(p),
                    "only actual source hashes differ"
                );
                *with.pointer_mut(p).unwrap() = Value::Null;
                *without.pointer_mut(p).unwrap() = Value::Null;
            }
            // Only source hashes are projected;declaration is appended and
            // blanked with equal-length whitespace,so old diagnostics keep spans.
            assert_eq!(
                serde_json::to_vec(&with).unwrap(),
                serde_json::to_vec(&without).unwrap(),
                "{command}:every other old byte/field"
            );
        });
        if result.is_err() {
            failures.push(command);
        }
    }
    assert!(
        failures.is_empty(),
        "U16 command isolation failures:{failures:?}"
    );
}
