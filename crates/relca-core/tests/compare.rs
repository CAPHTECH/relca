//! ADR 0017 public acceptance. Oracles come from calc.md, never a runtime snapshot.
use relca_core::{
    compare::{self, Analysis},
    diff, report,
};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
fn source(name: &str) -> String {
    fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
fn run(src: &str, plans: &[&str], mode: Analysis) -> Value {
    serde_json::to_value(compare::compare(src, plans, mode)).unwrap()
}
fn exhaustive(name: &str) -> Value {
    run(&source(name), &["Aplan", "Bplan"], Analysis::Exhaustive)
}
fn partial(expected: &Value, actual: &Value) {
    match expected {
        Value::Object(o) => {
            for (k, v) in o {
                partial(
                    v,
                    actual
                        .get(k)
                        .unwrap_or_else(|| panic!("missing {k}: {actual}")),
                );
            }
        }
        Value::Array(a) => {
            let b = actual.as_array().unwrap();
            assert_eq!(a.len(), b.len(), "array inventory");
            // Raw terminal order is not the compare table order. Match identity independently.
            for (i, v) in a.iter().enumerate() {
                let found = if v.get("end").is_some() {
                    b.iter()
                        .find(|x| x["end"] == v["end"] && x.get("kind") == v.get("kind"))
                        .unwrap()
                } else if v.get("id").is_some() {
                    b.iter().find(|x| x["id"] == v["id"]).unwrap()
                } else {
                    &b[i]
                };
                partial(v, found);
            }
        }
        _ => assert_eq!(expected, actual),
    }
}
fn tables_empty(v: &Value) {
    assert_eq!(v["tables"], json!({"properties":[],"terminals":[]}));
}
fn code(v: &Value, code: &str) {
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == code),
        "{v}"
    );
}

#[test]
fn c_all_16_fixture_oracles_and_single_substitutions() {
    let mut names: Vec<_> = fs::read_dir(root())
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| {
            e.file_type().unwrap().is_dir()
                && e.file_name().to_string_lossy().starts_with("compare-")
        })
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names.len(), 16);
    let mut broken = 0;
    for name in names {
        let dir = root().join(&name);
        let src = source(&name);
        let expected: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
        let inv: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("invocation.json")).unwrap())
                .unwrap();
        let actual = if inv["command"] == "diff" {
            serde_json::to_value(diff::diff(&src, "Aplan", "Bplan")).unwrap()
        } else {
            let plans: Vec<_> = inv["plans"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_str().unwrap())
                .collect();
            run(
                &src,
                &plans,
                if inv["mode"] == "simulation" {
                    Analysis::Simulation
                } else {
                    Analysis::Exhaustive
                },
            )
        };
        partial(&expected, &actual);
        if let Some((base, _)) = name.split_once(".broken-") {
            broken += 1;
            let original = source(base);
            assert_eq!(src.lines().count(), original.lines().count());
            assert_eq!(
                src.lines()
                    .zip(original.lines())
                    .filter(|(a, b)| a != b)
                    .count(),
                1,
                "{name}: one fault"
            );
        }
    }
    assert_eq!(broken, 8);
}

#[test]
fn c_nested_reports_equal_standalone_before_and_after_in_requested_order() {
    for (name, plans, mode) in [
        (
            "compare-legacy",
            vec!["ToBeP", "AsIs"],
            Analysis::Exhaustive,
        ),
        (
            "compare-arrivals",
            vec!["Bplan", "Aplan"],
            Analysis::Exhaustive,
        ),
        (
            "compare-random",
            vec!["Bplan", "Aplan"],
            Analysis::Simulation,
        ),
    ] {
        let src = source(name);
        let standalone = |p| {
            if name == "compare-random" {
                serde_json::to_value(report::simulate_configuration(&src, p)).unwrap()
            } else {
                serde_json::to_value(report::check_configuration(&src, p)).unwrap()
            }
        };
        let before: Vec<_> = plans.iter().map(|p| standalone(p)).collect();
        let v = run(&src, &plans, mode);
        for (i, p) in plans.iter().enumerate() {
            assert_eq!(v["plans"][i]["configuration"], *p);
            assert_eq!(v["plans"][i]["report"], before[i]);
            assert_eq!(standalone(p), before[i], "compare did not leak RNG/state");
        }
        for row in v["tables"]["properties"]
            .as_array()
            .unwrap()
            .iter()
            .chain(v["tables"]["terminals"].as_array().unwrap())
        {
            assert_eq!(
                row["values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|c| c["configuration"].as_str().unwrap())
                    .collect::<Vec<_>>(),
                plans
            );
        }
        assert!(
            v["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("terminal_meaning"))
        );
    }
}

fn terminal<'a>(v: &'a Value, case: u64, end: Option<&str>) -> &'a Value {
    v["tables"]["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["case_id"] == case && r["end"] == json!(end))
        .unwrap()
}

#[test]
fn c_legacy_exhaustive_order_and_values() {
    for (name, parallel, serial, work) in [
        ("compare-legacy", 5, 8, 8),
        ("compare-legacy.broken-longer-task", 6, 9, 9),
    ] {
        let v = run(&source(name), &["ToBeP", "AsIs"], Analysis::Exhaustive);
        assert_eq!(v["verdict"], "satisfied");
        assert!(v.get("common_random").is_none());
        for (i, elapsed) in [parallel, serial].into_iter().enumerate() {
            assert_eq!(
                terminal(&v, 1, Some("done"))["values"][i]["value"],
                json!({"completed_at":{"min":elapsed,"max":elapsed},"elapsed":{"min":elapsed,"max":elapsed},"work":{"min":work,"max":work},"result_at":null,"probability":null})
            );
            assert_eq!(v["plans"][i]["report"].as_object().unwrap().len(), 15);
            assert!(v["plans"][i]["report"].get("uncertainty").is_some());
        }
        assert_eq!(
            v["tables"]["terminals"].as_array().unwrap().len(),
            2,
            "done plus potential rejected"
        );
        assert!(
            terminal(&v, 1, None)["values"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["status"] == "not_reported" && c["value"].is_null())
        );
        let safety = v["tables"]["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "safety no_resource_overcommit")
            .unwrap();
        assert!(safety["case_id"].is_null() && safety["arrival"].is_null());
    }
}

#[test]
fn c_arrivals_exhaustive_and_diff_case_rows() {
    let v = exhaustive("compare-arrivals");
    assert_eq!(
        terminal(&v, 2, Some("done"))["values"][0]["value"]["elapsed"],
        json!({"min":2,"max":6})
    );
    assert_eq!(
        terminal(&v, 2, Some("done"))["values"][1]["value"]["elapsed"],
        json!({"min":2,"max":2})
    );
    let p = v["tables"]["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "probability reached #1" && r["case_id"] == 2)
        .unwrap();
    assert_eq!(
        p["values"][0]["probability"],
        json!({"min":"1/4","max":"1/4"})
    );
    assert_eq!(p["values"][1]["probability"], json!({"min":"1","max":"1"}));
    let d =
        serde_json::to_value(diff::diff(&source("compare-arrivals"), "Aplan", "Bplan")).unwrap();
    assert_eq!(d["terminals"], json!([]));
    assert_eq!(
        d["case_outcomes"][1]["terminals"][0]["from"]["elapsed"],
        json!({"min":2,"max":6})
    );
    assert_eq!(
        d["properties"][1]["cases"][1]["from"]["probability"],
        json!({"min":"1/4","max":"1/4"})
    );
}

#[test]
fn c_simulation_common_keys_counts_and_scope() {
    let v = run(
        &source("compare-random"),
        &["Aplan", "Bplan"],
        Analysis::Simulation,
    );
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(
        terminal(&v, 1, Some("yes"))["values"][0]["value"],
        json!({"count":2})
    );
    assert_eq!(
        terminal(&v, 1, Some("yes"))["values"][1]["value"],
        json!({"count":0})
    );
    assert_eq!(
        terminal(&v, 1, Some("yes"))["values"][1]["status"],
        "reported"
    );
    assert_eq!(v["common_random"]["confidence_scope"], "within-plan-only");
    assert_eq!(v["common_random"]["shared"][1]["case_ids"], json!([0]));
    for plan in v["plans"].as_array().unwrap() {
        let cs = &plan["report"]["case_outcomes"][0];
        let total: u64 = cs["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["count"].as_u64().unwrap())
            .sum();
        assert_eq!(
            total
                + cs["unfinished_count"].as_u64().unwrap()
                + cs["deadlocked_count"].as_u64().unwrap(),
            4
        );
    }
}

#[test]
fn c_alias_modes_and_cut_precedence() {
    let src = source("compare-random.broken-alias-process");
    let v = run(&src, &["Aplan", "Bplan"], Analysis::Simulation);
    code(&v, "U-SEM-0");
    tables_empty(&v);
    assert!(v["mismatch"].is_null());
    let message = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "U-SEM-0")
        .unwrap()["message"]
        .as_str()
        .unwrap();
    for name in ["Aplan", "Bplan", "P"] {
        assert!(message.contains(name));
    }
    for p in ["Aplan", "Bplan"] {
        assert_eq!(
            report::check_configuration(&src, p).verdict,
            relca_core::Verdict::Satisfied
        );
        assert_eq!(
            report::simulate_configuration(&src, p).verdict,
            relca_core::Verdict::Satisfied
        );
    }
    assert_eq!(
        run(&src, &["Aplan", "Bplan"], Analysis::Exhaustive)["verdict"],
        "satisfied"
    );
    assert_eq!(
        diff::diff(&src, "Aplan", "Bplan").verdict,
        relca_core::Verdict::Satisfied
    );
    let src = src.replace(
        "configuration Bplan { cut C",
        "cut D { include [A] }\n  configuration Bplan { cut D",
    );
    let v = run(&src, &["Aplan", "Bplan"], Analysis::Simulation);
    code(&v, "E-CMP-0");
    assert!(
        !v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SEM-0")
    );
}

#[test]
fn c_terminal_inventory_is_process_local_includes_dead_suffix_and_all_pairs() {
    let v = exhaustive("compare-terminal.broken-other-end");
    assert_eq!(
        v["mismatch"]["pairs"][0]["terminals"],
        json!({"left_only":["done"],"right_only":["other"]})
    );
    let src = source("compare-terminal");
    for q in [
        "end done; end ghost",
        "if a == ok { end done } else { end ghost }",
    ] {
        let changed = src.replace(
            "process Q { a = A.run(); end done }",
            &format!("process Q {{ a = A.run(); {q} }}"),
        );
        let v = run(&changed, &["Aplan", "Bplan"], Analysis::Exhaustive);
        assert_eq!(
            v["mismatch"]["pairs"][0]["terminals"]["right_only"],
            json!(["ghost"])
        );
    }
    let three = src.replace(
        "  bounds",
        "  process R { end third }\n  configuration Cplan { cut C; process R }\n  bounds",
    );
    let v = run(&three, &["Bplan", "Cplan", "Aplan"], Analysis::Exhaustive);
    let pairs: Vec<_> = v["mismatch"]["pairs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["left"].clone(), p["right"].clone()))
        .collect();
    assert_eq!(
        pairs,
        vec![
            (json!("Bplan"), json!("Cplan")),
            (json!("Cplan"), json!("Aplan"))
        ]
    );
    let d = serde_json::to_value(diff::diff(
        &source("compare-terminal.broken-other-end"),
        "Aplan",
        "Bplan",
    ))
    .unwrap();
    assert!(d["mismatch"].is_null());
    assert_eq!(d["terminals"].as_array().unwrap().len(), 2);
}

#[test]
fn c_missing_simulation_static_side_and_malformed_api_never_publish_tables() {
    let src = source("compare-terminal");
    let v = run(&src, &["Aplan", "Bplan"], Analysis::Simulation);
    assert_eq!(v["verdict"], "invalid");
    tables_empty(&v);
    assert!(v["mismatch"].is_null());
    for p in v["plans"].as_array().unwrap() {
        code(&p["report"], "E-SIM-0");
    }
    let v = run(&src, &["Aplan", "Unknown"], Analysis::Exhaustive);
    assert_eq!(v["plans"][0]["report"]["verdict"], "satisfied");
    code(&v["plans"][1]["report"], "E-NAME-0");
    tables_empty(&v);
    for plans in [
        vec![],
        vec!["Aplan"],
        vec!["Aplan", "Aplan"],
        vec!["", "Bplan"],
    ] {
        let v = run(&src, &plans, Analysis::Exhaustive);
        code(&v, "E-SYN-0");
        tables_empty(&v);
        assert_eq!(v["verdict"], "invalid");
    }
}

#[test]
fn c_reject_time_null_mass_and_case_ids() {
    for name in ["compare-reject", "compare-reject.broken-zero-queue"] {
        let v = exhaustive(name);
        assert_eq!(v["tables"]["terminals"].as_array().unwrap().len(), 6);
        let rejected = terminal(&v, 3, None);
        assert_eq!(rejected["values"][0]["value"]["completed_at"], Value::Null);
        assert_eq!(
            rejected["values"][0]["value"]["probability"],
            json!({"min":"1","max":"1"})
        );
        assert_eq!(
            terminal(&v, 3, Some("done"))["values"][0]["status"],
            "not_reported"
        );
    }
}

#[test]
fn c_public_runtime_abort_cut_and_vacuous_fold_without_partial_terminal_tables() {
    let base=source("compare-random").replace("; writes y","")
        .replace("process Q { a = A.c(); if a == first { end yes } else { end no } }",
            "process Q { fork f { a = A.c(); b = A.c() }; join all f; if a == first { end yes } else { end no } }")
        .replace("live_tasks 1","live_tasks 2");
    for (s, expected) in [
        (base.clone(), "unsupported"),
        (
            base.replace("y == first <= 1", "y == first >= 1"),
            "violated",
        ),
    ] {
        let v = run(&s, &["Aplan", "Bplan"], Analysis::Simulation);
        assert_eq!(v["verdict"], expected);
        assert_eq!(
            v["plans"][1]["report"]["simulation"]["status"],
            "unsupported"
        );
        assert_eq!(v["tables"]["terminals"], json!([]));
        assert!(
            v["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("terminal_tables"))
        );
        assert_eq!(v["tables"]["properties"].as_array().unwrap().len(), 3);
        for p in v["tables"]["properties"].as_array().unwrap() {
            assert_eq!(p["values"][1]["verdict"], "unsupported");
            assert!(
                p["values"][1]["hits"].is_null()
                    && p["values"][1]["trials"].is_null()
                    && p["values"][1]["interval"].is_null()
            );
        }
    }
    let cut=source("compare-terminal").replace("  process P", "  actor Slow { capacity 1; capability run() -> T { duration fixed 4min; result nondet { ok } } }\n  process P")
        .replace("include [A]","include [A,Slow]").replace("process P { a = A.run()","process P { a = Slow.run()");
    let v = run(&cut, &["Aplan", "Bplan"], Analysis::Exhaustive);
    assert_eq!(v["verdict"], "inconclusive");
    assert_eq!(v["plans"][0]["report"]["bounds"]["truncated"], true);
    assert_eq!(
        terminal(&v, 1, Some("done"))["values"][0]["status"],
        "not_reported"
    );
    assert!(terminal(&v, 1, Some("done"))["values"][0]["value"].is_null());
    assert_eq!(
        terminal(&v, 1, Some("done"))["values"][1]["status"],
        "reported"
    );
    let v = run(
        &source("contract-diff"),
        &["Before", "After"],
        Analysis::Exhaustive,
    );
    assert_eq!(v["verdict"], "vacuous");
}

#[test]
fn c_if_none_is_an_inventory_reference_even_when_unreachable() {
    let src = source("any-drain");
    let start = src.find("  process Check").unwrap();
    let end = src.find("  bounds").unwrap();
    let p = src[start..end].replace("process Check", "process P");
    let q = p.replace("process P", "process Q");
    let p = p.replace("if none -> no_match", "if none -> ghost");
    let src = format!(
        "{}{}{}  process Names {{ end ghost }}\n cut C {{ include [Office] }}\n configuration Aplan {{ cut C; process P }}\n configuration Bplan {{ cut C; process Q }}\n{}",
        &src[..start],
        p,
        q,
        &src[end..]
    );
    let v = run(&src, &["Aplan", "Bplan"], Analysis::Exhaustive);
    assert_eq!(
        v["mismatch"]["pairs"][0]["terminals"]["left_only"],
        json!(["ghost"])
    );
}

#[test]
fn c_cut_mismatch_all_six_typed_fields_not_just_names() {
    let src=source("compare-cut.broken-other-cut").replace("  process P","  actor Archive {}\n  process P")
        .replace("cut C { include [A] }","cut C { question \"old\"; include [A,Archive]; defer A.run.rates reason \"old\" loses \"rates\"; externality [Vendor.old]; reentry [review cut quarterly by cfo] }")
        .replace("cut D { include [A] }","cut D { question \"new\"; include [A]; exclude Archive reason \"outside\" loses \"archive\"; defer A.run.rates reason \"new\" loses \"rates\"; externality [Vendor.new]; reentry [review cut annually by cfo] }");
    let v = run(&src, &["Aplan", "Bplan"], Analysis::Exhaustive);
    assert_eq!(v["verdict"], "invalid");
    let ds = v["mismatch"]["pairs"][0]["cut"]["differences"]
        .as_array()
        .unwrap();
    let fields: std::collections::BTreeSet<_> =
        ds.iter().map(|d| d["field"].as_str().unwrap()).collect();
    assert_eq!(
        fields,
        std::collections::BTreeSet::from([
            "question",
            "include",
            "exclude",
            "defer",
            "externality",
            "reentry"
        ])
    );
    for d in ds {
        assert!(d.get("left").is_some() && d.get("right").is_some());
        assert_ne!(d["left"], d["right"]);
    }
    tables_empty(&v);
}

#[test]
fn c_comparison_assumptions_are_root_only_and_do_not_claim_joint_confidence() {
    let purpose = json!({"field":"compare.purpose","value":"side-by-side-only","affects":"plans are listed in requested order without ranking, a baseline, hard filtering, Pareto selection or a combined score"});
    let common = json!({"field":"compare.common_random","value":"initial-only","affects":"case initial keys and actor-local initial keys with case ID 0 are shared when the corresponding draws occur; rejected or unarrived cases do not draw; kernel and duration sites are separated by distinct process names; independence of distinct SHA-256 keys is a modeling assumption"});
    let confidence = json!({"field":"compare.confidence","value":"within-plan-only","affects":"Bonferroni correction applies to probability-query case rows within each plan; no simultaneous confidence across plans or interval for a difference is claimed"});
    let old = run(
        &source("compare-legacy"),
        &["ToBeP", "AsIs"],
        Analysis::Exhaustive,
    );
    assert_eq!(old["assumptions"], json!([purpose]));
    let sim = run(
        &source("compare-random"),
        &["Aplan", "Bplan"],
        Analysis::Simulation,
    );
    assert_eq!(sim["assumptions"], json!([purpose, common, confidence]));
    for v in [&old, &sim] {
        for p in v["plans"].as_array().unwrap() {
            assert!(
                !p["report"]["assumptions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|a| a["field"].as_str().unwrap().starts_with("compare."))
            );
        }
    }
}

#[test]
fn c_all_cases_future_clock_cut_keeps_rectangular_not_reported_rows() {
    let src = source("compare-terminal")
        .replace("cases 1", "cases 2")
        .replace("  bounds", "  arrivals fixed [5min,6min]\n  bounds");
    let v = run(&src, &["Aplan", "Bplan"], Analysis::Exhaustive);
    assert_eq!(v["verdict"], "inconclusive");
    assert_eq!(v["tables"]["terminals"].as_array().unwrap().len(), 4);
    for row in v["tables"]["terminals"].as_array().unwrap() {
        for cell in row["values"].as_array().unwrap() {
            assert_eq!(cell["status"], "not_reported");
            assert!(cell["value"].is_null());
        }
    }
    let d = serde_json::to_value(diff::diff(&src, "Aplan", "Bplan")).unwrap();
    assert_eq!(d["verdict"], "inconclusive");
    assert_eq!(d["case_outcomes"].as_array().unwrap().len(), 2);
    for c in d["case_outcomes"].as_array().unwrap() {
        assert_eq!(c["terminals"], json!([]));
    }
}

#[test]
fn c_terminal_declaration_order_is_not_a_comparison_condition() {
    let src = source("compare-random").replace(
        "process Q { a = A.c(); if a == first { end yes } else { end no } }",
        "process Q { a = A.c(); if a == second { end no } else { end yes } }",
    );
    let v = run(&src, &["Bplan", "Aplan"], Analysis::Exhaustive);
    assert_eq!(v["verdict"], "satisfied");
    assert!(v["mismatch"].is_null());
    assert_eq!(v["tables"]["terminals"][0]["end"], "no");
    assert_eq!(v["tables"]["terminals"][1]["end"], "yes");
}
