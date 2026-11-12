//! ADR0047 independent hand oracle transport. No production evaluator used as oracle.
#![allow(dead_code)]
use relca_core::{compare, mutation, report};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use std::{
    io::Write,
    process::{Command, Stdio},
};

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn inventory() -> Value {
    serde_json::from_str(include_str!("evaluation_inventory.json")).unwrap()
}
pub fn names() -> Vec<String> {
    serde_json::from_str(include_str!("evaluation_names.json")).unwrap()
}
pub fn source(n: &str) -> String {
    fs::read_to_string(root().join("fixtures").join(n).join("model.relca")).unwrap()
}
pub fn expected(n: &str) -> Value {
    serde_json::from_slice(
        &fs::read(root().join("fixtures").join(n).join("expected.json")).unwrap(),
    )
    .unwrap()
}
pub fn partial(want: &Value, got: &Value, path: &str) {
    match want {
        Value::Object(o) => {
            for (k, v) in o {
                partial(
                    v,
                    got.get(k)
                        .unwrap_or_else(|| panic!("missing {path}/{k}: {got}")),
                    &format!("{path}/{k}"),
                );
            }
        }
        Value::Array(a) => {
            let b = got
                .as_array()
                .unwrap_or_else(|| panic!("not array {path}: {got}"));
            assert_eq!(a.len(), b.len(), "{path} inventory");
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                partial(x, y, &format!("{path}/{i}"));
            }
        }
        _ => assert_eq!(want, got, "{path}"),
    }
}
pub fn run(n: &str, s: &str) -> Value {
    let iv = &inventory()[n]["invocation"];
    if iv["command"] == "mutate" {
        return serde_json::to_value(mutation::mutate(s)).unwrap();
    }
    let ns: Vec<_> = iv["plans"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap())
        .collect();
    serde_json::to_value(compare::compare_with_scenario(
        s,
        &ns,
        if iv["mode"] == "simulation" {
            compare::Analysis::Simulation
        } else {
            compare::Analysis::Exhaustive
        },
        iv["scenario"].as_str(),
    ))
    .unwrap()
}
pub fn replace(s: &str, a: &str, b: &str) -> String {
    let changed = s.replacen(a, b, 1);
    assert_ne!(changed, s, "replacement witness {a}");
    changed
}
pub fn without_declaration(s: &str) -> String {
    s.split_inclusive('\n')
        .map(|l| {
            if l.trim_start().starts_with("compare [") {
                l.chars()
                    .map(|c| if c == '\n' { '\n' } else { ' ' })
                    .collect()
            } else {
                l.to_owned()
            }
        })
        .collect()
}
pub fn standalone(s: &str, n: &str, sampled: bool, scenario: Option<&str>) -> Value {
    if sampled {
        serde_json::to_value(report::simulate_with_selection(s, Some(n), scenario)).unwrap()
    } else {
        serde_json::to_value(report::check_with_selection(s, Some(n), scenario)).unwrap()
    }
}
pub fn hand(n: &str, actual: &Value) {
    let info = &inventory()[n];
    let want = expected(n);
    assert_eq!(actual["verdict"], want["verdict"], "{n}: root verdict");
    if let Some(code) = info["code"].as_str() {
        let mut ds: Vec<_> = actual["diagnostics"].as_array().unwrap().iter().collect();
        for p in actual["plans"].as_array().into_iter().flatten() {
            ds.extend(p["report"]["diagnostics"].as_array().into_iter().flatten());
        }
        assert!(
            ds.iter().any(|d| d["code"] == code),
            "{n}: missing {code}: {ds:?}"
        );
        if let Some(message) = info["message_contains"].as_str() {
            assert!(
                ds.iter().any(|d| d["code"] == code
                    && d["message"]
                        .as_str()
                        .unwrap()
                        .to_lowercase()
                        .contains(&message.to_lowercase())),
                "{n}:specific diagnostic {code}: {message};got {ds:?}"
            );
        }
        assert!(
            actual.get("evaluation").is_none(),
            "{n}: no partial evaluation"
        );
        assert!(
            actual["tables"].get("agency").is_none(),
            "{n}: no partial Agency"
        );
        if n == "cmp7-under.broken-cut" {
            assert!(actual["mismatch"].is_null());
        }
        return;
    }
    if info["mutation"] == true {
        mutation_hand(n, actual);
        return;
    }
    // A full evaluation oracle, including all arrays, modes and nullability.
    assert_eq!(
        actual["evaluation"], want["evaluation"],
        "{n}: complete hand evaluation"
    );
    let ev = &actual["evaluation"];
    let ns = ev["declared"]["plans"].as_array().unwrap();
    let pairs = actual["tables"]["agency"]
        .as_array()
        .expect("tables.agency is opt-in, not omitted");
    assert_eq!(pairs.len(), ns.len() * (ns.len() - 1) / 2);
    let mut k = 0;
    for i in 0..ns.len() {
        for j in i + 1..ns.len() {
            assert_eq!(pairs[k]["from"], ns[i]);
            assert_eq!(pairs[k]["to"], ns[j]);
            k += 1;
        }
    }
    if n.starts_with("cmp7-responsibility") {
        let rows = pairs[0]["differences"]["rows"].as_array().unwrap();
        let observations: Vec<_> = rows
            .iter()
            .filter(|r| r["viewpoint"] == "responsibility" && r["item"] == "observation")
            .collect();
        if n.contains("broken-") {
            assert!(observations.is_empty());
        } else {
            assert_eq!(observations.len(), 1);
            let r = observations[0];
            assert_eq!(r["person"], "a1");
            assert_eq!(r["status"], "changed");
            assert_eq!(
                r["from"],
                json!({"status":"missing","reason":null,"required_sources":["Doc"],"direct":[],"indirect":["Doc"],"missing_sources":["Doc"]})
            );
            assert_eq!(
                r["to"],
                json!({"status":"present","reason":null,"required_sources":["Doc"],"direct":["Doc"],"indirect":["Doc"],"missing_sources":[]})
            );
        }
        assert!(rows.iter().any(|r| r["viewpoint"] == "responsibility"
            && r["item"] == "time_frame"
            && r["status"] == "not_comparable"));
    }
    let checked = actual["checked"].as_array().unwrap();
    let unchecked = actual["unchecked"].as_array().unwrap();
    for word in ["hard_filter", "admissibility"] {
        assert!(checked.contains(&json!(word)), "{n}:{word}");
        assert!(!unchecked.contains(&json!(word)), "{n}:{word}");
    }
    if actual["analysis"] == "simulation" {
        assert!(checked.contains(&json!("hard_cross_plan_bonferroni")));
        assert!(unchecked.contains(&json!("pareto")));
    } else {
        assert!(checked.contains(&json!("pareto_sure_dominance")));
        assert!(!unchecked.contains(&json!("pareto")));
    }
    for word in [
        "ranking",
        "weighted_score",
        "difference_intervals",
        "cross_plan_confidence",
    ] {
        assert!(unchecked.contains(&json!(word)));
    }
    let aa = actual["assumptions"].as_array().unwrap();
    let fields: Vec<_> = aa
        .iter()
        .filter_map(|a| a["field"].as_str())
        .filter(|s| s.starts_with("compare."))
        .collect();
    assert_eq!(
        fields,
        if actual["analysis"] == "simulation" {
            vec![
                "compare.purpose",
                "compare.common_random",
                "compare.confidence",
                "compare.hard",
                "compare.pareto",
            ]
        } else {
            vec!["compare.purpose", "compare.hard", "compare.pareto"]
        }
    );
    assert_eq!(
        aa.iter().find(|a| a["field"] == "compare.purpose").unwrap()["value"],
        "declared-evaluation"
    );
    let affects = aa.iter().find(|a| a["field"] == "compare.pareto").unwrap()["affects"]
        .as_str()
        .unwrap();
    if n == "eval-shared-source" {
        for name in [
            "metrics.overall.work",
            "metrics.overall.observed_cost_total",
        ] {
            assert!(affects.contains(name));
        }
    } else if n == "eval-shared-source.broken-one-objective" {
        assert!(!affects.contains("metrics.overall.observed_cost_total"));
    }
    if n == "cmp7-shared-cohort" {
        assert!(
            affects.contains("cohort.WrongPerArrival")
                && affects.contains("cohort.WrongPerRegistered")
        );
    } else if n == "cmp7-shared-cohort.broken-one-objective" {
        assert!(!affects.contains("cohort.WrongPerRegistered"));
    }
    for p in actual["plans"].as_array().unwrap() {
        assert!(
            p["report"]["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .all(|a| !a["field"].as_str().unwrap().starts_with("compare."))
        );
    }
}
pub fn mutation_hand(n: &str, v: &Value) {
    let row = &v["mutations"][7];
    assert_eq!(row["kind"], "old_promise_denominator_removed");
    if let Some(oracle) = inventory()[n].get("kind7_oracle") {
        partial(oracle, row, "kind7");
        let generated = !oracle["mutants"].as_array().unwrap().is_empty();
        assert_eq!(
            v["checked"]
                .as_array()
                .unwrap()
                .contains(&json!("quality_denominator")),
            generated
        );
        assert_eq!(
            v["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("quality_denominator")),
            !generated
        );
        assert_eq!(
            v["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("numerator_subset_of_denominator")),
            generated
        );
        assert_eq!(
            v["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["field"] == "mutation.kind7"),
            generated
        );
        for m in row["mutants"].as_array().unwrap() {
            denominator_location(n, m["site"]["cohort"].as_str().unwrap(), &m["site"]);
        }
        return;
    }
    if n.ends_with("no-cohort") {
        assert_eq!(row["status"], "unsupported");
        assert_eq!(row["reason"], "quality_denominator_unavailable");
        assert!(row.get("skipped_sites").is_none());
        assert!(
            !v["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["field"] == "mutation.kind7")
        );
        return;
    }
    assert_eq!(row["status"], "generated");
    assert_eq!(row["reason"], Value::Null);
    assert_eq!(
        row["skipped_sites"],
        json!([{"cohort":"WrongPerRegistered","status":"not_applicable","reason":"state_condition"}])
    );
    let ms = row["mutants"].as_array().unwrap();
    assert_eq!(ms.len(), 4);
    for (m, (cohort, removed, before, after)) in ms.iter().zip([
        (
            "WrongPerArrival",
            "returned",
            vec!["returned", "posted"],
            vec!["posted"],
        ),
        (
            "WrongPerArrival",
            "posted",
            vec!["returned", "posted"],
            vec!["returned"],
        ),
        (
            "WrongPerResolved",
            "posted",
            vec!["posted", "returned"],
            vec!["returned"],
        ),
        (
            "WrongPerResolved",
            "returned",
            vec!["posted", "returned"],
            vec!["posted"],
        ),
    ]) {
        assert_eq!(
            m["id"],
            format!("old_promise_denominator_removed:cohort.{cohort}:remove:{removed}")
        );
        assert_eq!(m["operator"], "remove_denominator_end");
        assert_eq!(m["site"]["cohort"], cohort);
        assert_eq!(m["site"]["removed_end"], removed);
        denominator_location(n, cohort, &m["site"]);
        assert_eq!(
            m["site"]["process"],
            if cohort == "WrongPerArrival" {
                json!("Flow")
            } else {
                Value::Null
            }
        );
        assert_eq!(m["result"], "detected");
        assert!(m["reason"].is_null());
        assert!(m["report"].is_null(), "never execute kind7");
        assert_eq!(m["other_changes"], json!([]));
        assert_eq!(m["assigned_changes"], json!(["quality_denominator", "comparison_mismatch"].into_iter().map(|c|
            json!({"check":c,"key":format!("cohort.{cohort}"),"field":"denominator.terminals","before":before,"after":after,"direction":"changed"})).collect::<Vec<_>>()));
    }
    assert!(
        v["checked"]
            .as_array()
            .unwrap()
            .contains(&json!("quality_denominator"))
    );
    assert!(
        !v["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("quality_denominator"))
    );
    assert!(
        v["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("numerator_subset_of_denominator"))
    );
    assert!(
        v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["field"] == "mutation.kind7")
    );
}
pub fn denominator_location(n: &str, cohort: &str, site: &Value) {
    let s = source(n);
    let prefix = format!("metric cohort.{cohort} =");
    let start = s.find(&prefix).unwrap();
    let line = &s[start..s[start..].find('\n').map_or(s.len(), |i| start + i)];
    let begin = start + line.find(" / ").unwrap() + 3;
    let end = start + line.trim_end().len();
    assert_eq!(
        site["span"],
        json!({"start":begin,"end":end}),
        "{n}/{cohort}:denominator-only span"
    );
    assert_eq!(
        site["line"],
        json!(s[..begin].bytes().filter(|c| *c == b'\n').count() + 1)
    );
    assert_eq!(
        site["column"],
        json!(begin - s[..begin].rfind('\n').map_or(0, |p| p + 1) + 1)
    );
}

pub fn label_byte_projection(base: &[u8], label: &[u8]) {
    // T12: compare two current executions using the hand name bijection and
    // source-derived unused-Capability spans. Align metric keys only; never
    // permute declared plans, evaluation, admissibility, Pareto or Agency rows.
    let payload = json!({"base":String::from_utf8(base.to_vec()).unwrap(),"label":String::from_utf8(label.to_vec()).unwrap(),
        "base_source":source("eval-contention"),"label_source":source("eval-label")});
    let mut child = Command::new("python3")
        .args([
            "-B",
            "crates/relca-cli/tests/evaluation_label_projection.py",
            "--verify",
            "--self-test",
        ])
        .current_dir(root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&payload).unwrap())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "T12 byte projection:{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
