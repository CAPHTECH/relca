//! Real CLI, all report positions, and closed metadata negative controls.
#[path = "../../relca-core/tests/common_cause_wlin2.rs"]
mod acceptance;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::process::Command;

#[test]
fn all_twenty_four_expected_schemas_and_population_are_closed() {
    assert_eq!(acceptance::NAMES, fixture_match::COMMON_CAUSE_FIXTURES);
    for name in acceptance::NAMES {
        fixture_match::validate_expected(&acceptance::expected(name))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}
#[test]
fn twenty_four_real_reports_and_exit_codes() {
    for name in acceptance::NAMES {
        let out = Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg("check")
            .arg(acceptance::root().join(name).join("model.relca"))
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        acceptance::assert_report(name, &acceptance::source(name), &v);
        fixture_match::compare_report(&acceptance::expected(name), &v).unwrap();
        let verdict: relca_core::Verdict =
            serde_json::from_value(acceptance::expected(name)["verdict"].clone()).unwrap();
        assert_eq!(
            out.status.code(),
            Some(verdict.exit_code()),
            "warning alone never fails {name}"
        );
        assert_eq!(
            String::from_utf8(out.stderr)
                .unwrap()
                .lines()
                .filter(|l| l.starts_with("W-LIN-2:"))
                .count(),
            acceptance::warning_calls(name).len()
        );
    }
}

fn hand_agency() -> Value {
    let mut a = json!({"rules":"agency-p0-1","origin":[],"lineage":[],"lineage_groups":[],"refusal":[],"unchecked":[]});
    a["common_causes"] =
        acceptance::expected("lin-shared-kernel")["agency"]["common_causes"].clone();
    a["model_uncertainties"] =
        acceptance::expected("lin-basis")["agency"]["model_uncertainties"].clone();
    a["source_descriptions"] =
        acceptance::expected("lin-tacit")["agency"]["source_descriptions"].clone();
    a
}
#[test]
fn closed_new_tables_reject_missing_unknown_null_empty_duplicate_and_wrong_types() {
    let a = hand_agency();
    fixture_match::validate_common_cause_agency(&a).unwrap();
    for pointer in [
        "/common_causes",
        "/common_causes/causes/0",
        "/common_causes/causes/0/targets/0",
        "/model_uncertainties",
        "/model_uncertainties/items/0",
        "/source_descriptions/0",
    ] {
        for key in a.pointer(pointer).unwrap().as_object().unwrap().keys() {
            let mut b = a.clone();
            b.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                fixture_match::validate_common_cause_agency(&b).is_err(),
                "missing {pointer}/{key}"
            );
        }
        let mut b = a.clone();
        b.pointer_mut(pointer).unwrap()["unknown"] = json!(1);
        assert!(
            fixture_match::validate_common_cause_agency(&b).is_err(),
            "unknown {pointer}"
        );
    }
    for pointer in [
        "/common_causes",
        "/model_uncertainties",
        "/source_descriptions",
        "/common_causes/causes",
        "/common_causes/causes/0/targets",
        "/model_uncertainties/items",
        "/model_uncertainties/items/0/targets",
        "/model_uncertainties/items/0/error_latents",
    ] {
        for value in [Value::Null, json!([]), json!(1), json!("x")] {
            let mut b = a.clone();
            *b.pointer_mut(pointer).unwrap() = value;
            assert!(
                fixture_match::validate_common_cause_agency(&b).is_err(),
                "invalid {pointer}"
            );
        }
    }
    for pointer in [
        "/common_causes/causes",
        "/common_causes/causes/0/targets",
        "/model_uncertainties/items",
        "/model_uncertainties/items/0/targets",
        "/source_descriptions",
    ] {
        let mut b = a.clone();
        let array = b.pointer_mut(pointer).unwrap().as_array_mut().unwrap();
        array.push(array[0].clone());
        assert!(
            fixture_match::validate_common_cause_agency(&b).is_err(),
            "duplicate {pointer}"
        );
    }
    let mut b = a.clone();
    b["unchecked"] = json!(["W-LIN-2"]);
    assert!(
        fixture_match::validate_common_cause_agency(&b).is_err(),
        "forbidden in Agency, retained in mutation wrapper"
    );
    for (pointer, value) in [
        ("/common_causes/rules", "new"),
        ("/common_causes/notice", "wrong"),
        ("/common_causes/scope", "all_models"),
        ("/model_uncertainties/items/0/kind", "other"),
        ("/source_descriptions/0/kind", "document"),
    ] {
        let mut b = a.clone();
        *b.pointer_mut(pointer).unwrap() = json!(value);
        assert!(
            fixture_match::validate_common_cause_agency(&b).is_err(),
            "closed literal {pointer}"
        );
    }
}

#[test]
fn all_three_suppression_kinds_have_disjoint_closed_payloads() {
    for (name, field) in [
        ("lin-basis", "basis"),
        ("lin-tacit", "sources"),
        ("lin-correlation", "judgements"),
    ] {
        let mut a = hand_agency();
        a["model_uncertainties"] =
            acceptance::expected(name)["agency"]["model_uncertainties"].clone();
        fixture_match::validate_common_cause_agency(&a).unwrap();
        let row = "/model_uncertainties/items/0";
        for key in [
            "kind",
            "call",
            "capability",
            "kernel",
            "targets",
            "error_latents",
            "notice",
            field,
        ] {
            let mut bad = a.clone();
            bad.pointer_mut(row)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                fixture_match::validate_common_cause_agency(&bad).is_err(),
                "{name} missing {key}"
            );
        }
        for wrong_field in ["basis", "sources", "judgements"] {
            if wrong_field == field {
                continue;
            }
            let mut bad = a.clone();
            bad.pointer_mut(row).unwrap()[wrong_field] = if wrong_field == "basis" {
                json!("x")
            } else {
                json!(["x"])
            };
            assert!(
                fixture_match::validate_common_cause_agency(&bad).is_err(),
                "{name} mixed payload {wrong_field}"
            );
        }
        for value in [Value::Null, json!(1), json!(true)] {
            let mut bad = a.clone();
            bad.pointer_mut(row).unwrap()[field] = value;
            assert!(
                fixture_match::validate_common_cause_agency(&bad).is_err(),
                "{name} wrong payload type"
            );
        }
        if field != "basis" {
            for value in [json!([]), json!([""]), json!(["x", "x"])] {
                let mut bad = a.clone();
                bad.pointer_mut(row).unwrap()[field] = value;
                assert!(
                    fixture_match::validate_common_cause_agency(&bad).is_err(),
                    "{name} empty/duplicate detail"
                );
            }
        }
        let mut bad = a.clone();
        bad.pointer_mut(row).unwrap()["notice"] = json!("wrong");
        assert!(fixture_match::validate_common_cause_agency(&bad).is_err());
    }
}

#[test]
fn eight_real_boundary_reports_keep_exact_warning_spans_and_suppression_rows() {
    for case in acceptance::boundary_cases() {
        let (v, out) = run_source(&case.src, case.label, &["check"]);
        acceptance::assert_boundary(&case, &v);
        assert_eq!(out.status.code(), Some(0));
        fixture_match::compare_report(&json!({}), &v).unwrap();
        fixture_match::validate_common_cause_agency(&v["agency"]).unwrap();
        assert_eq!(
            String::from_utf8(out.stderr)
                .unwrap()
                .lines()
                .filter(|l| l.starts_with("W-LIN-2:"))
                .count(),
            case.calls.len()
        );
    }
}

fn run_source(src: &str, tag: &str, args: &[&str]) -> (Value, std::process::Output) {
    // Remains inside this worktree, not the global temporary directory. No network.
    let dir = acceptance::root()
        .parent()
        .unwrap()
        .join("target/lin-cli-inputs");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{}-{tag}.relca", std::process::id()));
    std::fs::write(&path, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(args[0])
        .arg(path)
        .args(&args[1..])
        .output()
        .unwrap();
    (serde_json::from_slice(&out.stdout).unwrap(), out)
}
fn checked_agency(v: &Value, hand: &Value) {
    let a = &v["agency"];
    fixture_match::validate_common_cause_agency(a).unwrap();
    assert!(a.is_object());
    assert!(
        !a["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("W-LIN-2"))
    );
    for key in [
        "common_causes",
        "model_uncertainties",
        "source_descriptions",
    ] {
        assert_eq!(
            a.get(key),
            hand.get(key),
            "all command paths preserve {key}"
        );
    }
}
fn warning_inventory(name: &str, src: &str, v: &Value) {
    let expected = acceptance::warning_calls(name);
    let ds: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "W-LIN-2")
        .collect();
    assert_eq!(
        ds.len(),
        expected.len(),
        "static warnings survive every report command"
    );
    for (d, call) in ds.into_iter().zip(expected) {
        assert_eq!(d["severity"], "warning");
        let start = d["span"]["start"].as_u64().unwrap() as usize;
        let end = d["span"]["end"].as_u64().unwrap() as usize;
        let hand_start = src.find(&format!("{call} =")).unwrap();
        let hand_end = hand_start + src[hand_start..].find(')').unwrap() + 1;
        assert_eq!(
            &src[start..end],
            &src[hand_start..hand_end],
            "span is a complete call, including in the duplicated selected process"
        );
    }
}
fn configured_source(src: &str) -> String {
    let start = src.find("  process Flow").unwrap();
    let end = src.find("  cut C").unwrap();
    let alt = src[start..end].replace("process Flow", "process AltFlow");
    src.replace("  bounds {",&format!("{alt}  configuration First {{ cut C; process Flow }}\n  configuration Second {{ cut C; process AltFlow }}\n  simulation replications 2 seed 17 confidence 0.5\n  bounds {{"))
}
fn closed_at_depths(inv: &fixture_match::Invocation, v: &Value, paths: &[String]) {
    inv.compare(&json!({}), v).unwrap();
    for path in paths {
        for key in [
            "common_causes",
            "model_uncertainties",
            "source_descriptions",
        ] {
            let pointer = format!("{path}/{key}");
            let Some(table) = v.pointer(&pointer) else {
                continue;
            };
            for fault in [Value::Null, json!([]), json!({"unknown":true})] {
                let mut bad = v.clone();
                *bad.pointer_mut(&pointer).unwrap() = fault;
                assert!(
                    inv.compare(&json!({}), &bad).is_err(),
                    "closed table at {pointer}"
                );
            }
            let child = if key == "common_causes" {
                format!("{pointer}/causes")
            } else if key == "model_uncertainties" {
                format!("{pointer}/items")
            } else {
                pointer.clone()
            };
            let mut bad = v.clone();
            *bad.pointer_mut(&child).unwrap() = json!([]);
            assert!(
                inv.compare(&json!({}), &bad).is_err(),
                "nonempty at {child}: {table}"
            );
        }
    }
}

#[test]
fn all_commands_keep_exact_tables_at_canonical_reports_and_mutate_keeps_wrapper_word() {
    for name in [
        "lin-basis",
        "lin-tacit",
        "lin-correlation",
        "lin-shared-kernel",
        "lin-basis.broken-no-basis",
    ] {
        let hand = acceptance::expected(name)["agency"].clone();
        let src = acceptance::source(name);
        for command in ["check", "simulate", "explain", "mutate"] {
            let src = if command == "simulate" {
                src.replace(
                    "  bounds {",
                    "  simulation replications 2 seed 17 confidence 0.5\n  bounds {",
                )
            } else {
                src.clone()
            };
            let (v, out) = run_source(&src, &format!("{name}-{command}"), &[command]);
            if command == "mutate" {
                assert_eq!(
                    out.status.code(),
                    Some(4),
                    "unavailable mutation kinds stay unsupported"
                );
                assert!(
                    v["unchecked"]
                        .as_array()
                        .unwrap()
                        .contains(&json!("W-LIN-2")),
                    "wrapper means detection comparison unavailable, not unchecked Report"
                );
                checked_agency(&v["original"], &hand);
                warning_inventory(name, &src, &v["original"]);
                fixture_match::validate_mutation_wire(&v).unwrap();
                let mut paths = vec!["/original/agency".into()];
                for (k, kind) in v["mutations"].as_array().unwrap().iter().enumerate() {
                    for (i, m) in kind["mutants"].as_array().unwrap().iter().enumerate() {
                        if m["report"]["agency"].is_object() {
                            fixture_match::validate_common_cause_agency(&m["report"]["agency"])
                                .unwrap();
                            paths.push(format!("/mutations/{k}/mutants/{i}/report/agency"));
                        }
                    }
                }
                closed_at_depths(&fixture_match::Invocation::Mutate { set: None }, &v, &paths);
            } else {
                assert_eq!(
                    out.status.code(),
                    Some(if command == "simulate" { 2 } else { 0 }),
                    "{name}/{command}: {v}"
                );
                checked_agency(&v, &hand);
                warning_inventory(name, &src, &v);
                match command {
                    "check" => {
                        fixture_match::compare_report(&json!({"verdict":"satisfied"}), &v).unwrap()
                    }
                    "simulate" => fixture_match::validate_simulation_wire(&v).unwrap(),
                    "explain" => {
                        assert!(v.get("report").is_none(), "explain root is flattened");
                        fixture_match::validate_explain_wire(&v).unwrap();
                    }
                    _ => unreachable!(),
                }
                let inv = match command {
                    "check" => fixture_match::Invocation::LegacyCheck,
                    "simulate" => fixture_match::Invocation::Simulate {
                        configuration: None,
                    },
                    "explain" => fixture_match::Invocation::Explain,
                    _ => unreachable!(),
                };
                closed_at_depths(&inv, &v, &["/agency".into()]);
            }
        }
        let configured = configured_source(&src);
        let (v, out) = run_source(
            &configured,
            &format!("{name}-diff"),
            &["diff", "--from", "First", "--to", "Second"],
        );
        assert_eq!(out.status.code(), Some(0), "{v}");
        for side in ["from", "to"] {
            checked_agency(&v[side]["report"], &hand);
            warning_inventory(name, &configured, &v[side]["report"]);
            assert_eq!(v["agency"][side], v[side]["report"]["agency"]);
        }
        closed_at_depths(
            &fixture_match::Invocation::Diff {
                from: "First".into(),
                to: "Second".into(),
            },
            &v,
            &[
                "/from/report/agency".into(),
                "/to/report/agency".into(),
                "/agency/from".into(),
                "/agency/to".into(),
            ],
        );
        for mode in ["exhaustive", "simulation"] {
            let (v, out) = run_source(
                &configured,
                &format!("{name}-compare-{mode}"),
                &["compare", "--plans", "Second,First", "--mode", mode],
            );
            assert_eq!(
                out.status.code(),
                Some(if mode == "simulation" { 2 } else { 0 }),
                "{v}"
            );
            fixture_match::validate_compare_wire(&v).unwrap();
            assert_eq!(v["plans"].as_array().unwrap().len(), 2);
            for p in v["plans"].as_array().unwrap() {
                checked_agency(&p["report"], &hand);
                warning_inventory(name, &configured, &p["report"]);
            }
            closed_at_depths(
                &fixture_match::Invocation::Compare {
                    plans: vec!["Second".into(), "First".into()],
                    mode: if mode == "simulation" {
                        fixture_match::Analysis::Simulation
                    } else {
                        fixture_match::Analysis::Exhaustive
                    },
                },
                &v,
                &[
                    "/plans/0/report/agency".into(),
                    "/plans/1/report/agency".into(),
                ],
            );
        }
    }
}

#[test]
fn selected_configuration_static_failure_and_sample_abort_do_not_launder_metadata() {
    let src=acceptance::source("lin-shared-kernel").replace("  bounds {","  process Single { solo = A.check(); end done }\n  configuration Pair { cut C; process Flow }\n  configuration Single { cut C; process Single }\n  bounds {");
    for (selection, has_cause) in [("Pair", true), ("Single", false)] {
        let (v, out) = run_source(
            &src,
            &format!("selection-{selection}"),
            &["check", "--configuration", selection],
        );
        assert_eq!(out.status.code(), Some(0), "{v}");
        assert_eq!(v["agency"].get("common_causes").is_some(), has_cause);
    }
    let bad = acceptance::source("lin-basis") + " unknown trailing input";
    for command in ["check", "simulate", "explain"] {
        let (v, out) = run_source(&bad, &format!("failure-{command}"), &[command]);
        assert_ne!(out.status.code(), Some(0));
        assert!(v["agency"].is_null(), "failed parse has no checked table");
    }
    let abort = acceptance::source("lin-basis.broken-no-basis")
        .replace("bad: bool = false", "bad: bool")
        .replace(
            "  bounds {",
            "  simulation replications 2 seed 7 confidence 0.5\n  bounds {",
        );
    let (v, out) = run_source(&abort, "sample-abort", &["simulate"]);
    assert_eq!(
        out.status.code(),
        Some(4),
        "unresolved nondeterminism, not random fill: {v}"
    );
    assert!(v["agency"].is_object());
    fixture_match::validate_simulation_wire(&v).unwrap();
    assert!(
        !v["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("W-LIN-2"))
    );
}

#[test]
fn old_syntax_command_preflight_has_no_unsupported_acceptance_construction() {
    for name in ["lin-basis", "lin-tacit", "lin-shared-kernel"] {
        let src = acceptance::source(name)
            .replace(" basis \"sample X measured independent detection\"", "")
            .replace("tacit \"取引慣行の知識\"", "tacit");
        for command in ["check", "simulate", "explain", "mutate"] {
            let s = if command == "simulate" {
                src.replace(
                    "  bounds {",
                    "  simulation replications 2 seed 17 confidence 0.5\n  bounds {",
                )
            } else {
                src.clone()
            };
            let (v, out) = run_source(&s, &format!("preflight-{name}-{command}"), &[command]);
            assert_eq!(
                out.status.code(),
                Some(match command {
                    "mutate" => 4,
                    "simulate" => 2,
                    _ => 0,
                }),
                "{v}"
            );
            assert!(
                if command == "mutate" {
                    v["original"]["agency"].is_object()
                } else {
                    v["agency"].is_object()
                },
                "{v}"
            );
        }
        let s = configured_source(&src);
        let (v, out) = run_source(
            &s,
            &format!("preflight-{name}-diff"),
            &["diff", "--from", "First", "--to", "Second"],
        );
        assert_eq!(out.status.code(), Some(0), "{v}");
        for mode in ["exhaustive", "simulation"] {
            let (v, out) = run_source(
                &s,
                &format!("preflight-{name}-{mode}"),
                &["compare", "--plans", "Second,First", "--mode", mode],
            );
            assert_eq!(
                out.status.code(),
                Some(if mode == "simulation" { 2 } else { 0 }),
                "{v}"
            );
        }
    }
    let s = acceptance::source("lin-basis.broken-no-basis")
        .replace("bad: bool = false", "bad: bool")
        .replace(
            "  bounds {",
            "  simulation replications 2 seed 7 confidence 0.5\n  bounds {",
        );
    let (v, out) = run_source(&s, "preflight-abort", &["simulate"]);
    assert_eq!(out.status.code(), Some(4), "{v}");
    assert!(v["agency"].is_object());
}
