//! Semantic seams not reduced to a root verdict: explain, compare, diff, metrics.
#[path = "support/effect_standing.rs"]
mod hand;
use relca_core::{compare, diff, explain, report};
use serde_json::{Value, json};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;

fn check(n: &str) -> Value {
    serde_json::to_value(report::check(&hand::source(n))).unwrap()
}
fn metric<'a>(v: &'a Value, table: &str, key: &str, id: &str) -> &'a Value {
    v["metrics"][table]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r[key] == id)
        .unwrap()
}
fn scalar(lo: &str, hi: &str) -> Value {
    json!({"min":lo,"max":hi,"reason":null})
}

#[test]
fn route_occupation_is_not_terminal_work_or_terminal_price() {
    for (n, ordinary, overall, c1) in [
        ("sov-metrics", 3, 4, 1),
        ("sov-metrics.broken-longer-route", 3, 5, 2),
        ("sov-concurrent", 5, 6, 3),
        ("sov-paid", 5, 6, 3),
        ("sov-paid.broken-missing-route-fee", 5, 6, 3),
    ] {
        let a = check(n);
        assert_eq!(a["verdict"], "satisfied", "{n}: {a}");
        let post = a["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["end"] == "posted")
            .unwrap();
        assert_eq!(post["work"], json!({"min":ordinary,"max":ordinary}));
        assert_eq!(
            a["metrics"]["overall"]["work"],
            scalar("2", &overall.to_string())
        );
        assert_eq!(
            metric(&a, "people", "person", "c1")["work"],
            scalar("0", &c1.to_string())
        );
        let price = &metric(&a, "terminals", "end", "posted")["cost_per_case"];
        let total = &a["metrics"]["overall"]["observed_cost_total"];
        if n.starts_with("sov-paid") {
            assert_eq!(price, &scalar("5", "5"));
            if n.contains("broken") {
                assert_eq!(total["reason"], "not_configured");
            } else {
                assert_eq!(total, &scalar("2", "6"));
            }
        } else {
            assert_eq!(price["reason"], "not_configured");
            assert_eq!(total["reason"], "not_configured");
        }
        for alt in a["contest_runtime"][0]["alternatives"].as_array().unwrap() {
            if alt["filed"] == false {
                assert_eq!(alt["route_cost"]["reason"], "never_created");
            } else if n == "sov-paid" {
                assert_eq!(alt["route_cost"], scalar("1", "1"));
            } else {
                assert_eq!(alt["route_cost"]["reason"], "not_configured");
            }
        }
    }
    for (n, occupancy) in [("sov-resource", "3"), ("sov-resource.broken-unshared", "2")] {
        let a = check(n);
        assert_eq!(
            metric(&a, "resources", "resource", "R")["occupancy_time"],
            scalar("0", occupancy)
        );
        assert_eq!(
            metric(&a, "resources", "resource", "R")["peak_units"],
            scalar("0", "1")
        );
    }
}

#[test]
fn explain_trigger_is_commit_at3_not_business_end_and_reference_replays() {
    let src = hand::source("sov-concurrent.broken-short");
    let v = serde_json::to_value(explain::explain(&src)).unwrap();
    assert_eq!(v["verdict"], "violated");
    let ce = v["explanation"]["counterexamples"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "contract Controls #1 contest_response")
        .unwrap();
    assert_eq!(ce["status"], "verified");
    assert_eq!(ce["replay"]["status"], "verified");
    let violation = &ce["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| !s["violation"].is_null())
        .expect("violation stage")["violation"];
    // The published clause witness uses trigger_step/deadline on violation;
    // step itself must carry a Commit, never a neighboring End.
    assert_eq!(violation["deadline"], 5, "{ce}");
    let index = violation["trigger_step"]
        .as_u64()
        .expect("commit trigger step");
    let step = ce["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["index"] == index)
        .unwrap();
    assert_eq!(step["time"], 3, "{step}");
    assert_eq!(step["kind"], "finish", "{step}");
    assert!(
        step["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "commit" && e["actor"] == "Ledger" && e["capability"] == "post"),
        "{step}"
    );
    let mut projection = v;
    projection.as_object_mut().unwrap().remove("explanation");
    assert_eq!(
        ledger_shape::original_report(&projection),
        ledger_shape::original_report(&check("sov-concurrent.broken-short"))
    );
}

#[test]
fn diff_effect_target_and_intent_are_static_while_compare_transports_tables() {
    for n in ["sov-diff", "sov-diff.broken-no-intent"] {
        let src = hand::source(n);
        let d = serde_json::to_value(diff::diff(&src, "Skipping", "Posting")).unwrap();
        assert_eq!(
            d["verdict"], "violated",
            "inactive side no_decision is not success: {d}"
        );
        let rows = d["differences"].as_array().unwrap();
        let effect = rows
            .iter()
            .find(|r| r["kind"] == "contest" && r["target"] == "Ledger.post.commit")
            .expect("effect contest diff row");
        assert_eq!(effect["from"], json!([]));
        assert_eq!(effect["to"].as_array().unwrap().len(), 1);
        let warning = d["diagnostics"].as_array().unwrap().iter().any(|d| {
            d["code"] == "W-DIFF-1"
                && d["message"]
                    .as_str()
                    .unwrap()
                    .contains("Ledger.post.commit")
        });
        assert_eq!(warning, n.contains("broken"));
        let c = serde_json::to_value(compare::compare(
            &src,
            &["Skipping", "Posting"],
            compare::Analysis::Exhaustive,
        ))
        .unwrap();
        for v in [&d, &c] {
            assert!(
                v["unchecked"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("contest_runtime_tables"))
            );
        }
        let plans = c["plans"].as_array().unwrap();
        assert_eq!(
            plans[0]["report"]["contest_runtime"][0]["alternatives"],
            json!([])
        );
        hand::subset(
            &hand::expected("sov-basic")["contest_runtime"],
            &plans[1]["report"]["contest_runtime"],
            n,
        );
    }
}

#[test]
fn end_open_counts_cases_once_and_keeps_same_tick_causal_order() {
    for (name, max) in [
        ("sov-basic", "1"),
        ("sov-end-open", "0"),
        ("sov-end-open.broken-zero-tail", "1"),
        ("sov-two-contests", "1"),
        ("sov-cases", "2"),
        ("sov-effect-and-end", "1"),
    ] {
        assert_eq!(
            check(name)["contest_summary"]["business_ended_open"],
            scalar("0", max),
            "{name}"
        );
    }
    // Independent normal tail1 finishes at4, same time as route response.
    // End-before-route-finish and route-finish-before-End remain distinct.
    let src = hand::replace(
        &hand::source("sov-end-open"),
        "duration fixed 10min",
        "duration fixed 1min",
    );
    let a = serde_json::to_value(report::check(&src)).unwrap();
    assert_eq!(
        a["contest_summary"]["business_ended_open"],
        scalar("0", "1")
    );
    let a = check("sov-cases.broken-short");
    let p = a["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "contract Controls #1 contest_response")
        .unwrap();
    let values: Vec<_> = p["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["case_id"].as_u64().unwrap(),
                c["verdict"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        values,
        [(1, "violated"), (2, "satisfied")],
        "second case responds exactly at its own deadline6"
    );
}

#[test]
fn label_and_grain_full_report_comparison_maps_source_bytes_independently() {
    let base = hand::source("sov-basic");
    let mut labelled = base.clone();
    let mut positions: Vec<usize> = (0..=base.len()).collect();
    // Derive the permitted source mapping from the input transformations,
    // never from differences between implementation reports.
    for (old, new, all) in [
        ("Vendor", "Supplier", true),
        ("Ledger", "Book", true),
        ("actor Book {", "actor Book { grain ledger;", false),
        ("actor Supplier {", "actor Supplier { grain party;", false),
    ] {
        let mut from = 0;
        while let Some(relative) = labelled[from..].find(old) {
            let start = from + relative;
            let end = start + old.len();
            let a = positions[start];
            let b = positions[end];
            let map: Vec<_> = if new.starts_with(old) {
                positions[start..end]
                    .iter()
                    .copied()
                    .chain(std::iter::repeat_n(b, new.len() - old.len()))
                    .collect()
            } else {
                (0..new.len())
                    .map(|i| a + (b - a) * i / new.len())
                    .collect()
            };
            labelled.replace_range(start..end, new);
            positions.splice(start..end, map);
            from = start + new.len();
            if !all {
                break;
            }
        }
    }
    assert_eq!(labelled, hand::source("sov-label"));
    fn sha(s: &str) -> String {
        use std::{
            io::Write,
            process::{Command, Stdio},
        };
        let mut c = Command::new("sha256sum")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        c.stdin.take().unwrap().write_all(s.as_bytes()).unwrap();
        let o = c.wait_with_output().unwrap();
        assert!(o.status.success());
        String::from_utf8(o.stdout)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .into()
    }
    fn normalize(v: &mut Value, map: &[usize]) {
        match v {
            Value::String(s) => *s = s.replace("Supplier", "Vendor").replace("Book", "Ledger"),
            Value::Array(a) => {
                for x in a {
                    normalize(x, map)
                }
            }
            Value::Object(o) => {
                if o.len() == 2 && o.contains_key("start") && o.contains_key("end") {
                    for k in ["start", "end"] {
                        let at = o[k].as_u64().unwrap() as usize;
                        o[k] = json!(map[at]);
                    }
                } else {
                    for x in o.values_mut() {
                        normalize(x, map)
                    }
                }
            }
            _ => {}
        }
    }
    let a = check("sov-basic");
    let mut b = check("sov-label");
    assert_eq!(a["verdict"], "satisfied");
    assert_eq!(b["verdict"], "satisfied");
    assert_eq!(a["model"]["sha256"], sha(&base));
    assert_eq!(b["model"]["sha256"], sha(&labelled));
    b["model"]["sha256"] = a["model"]["sha256"].clone();
    for (actor, grain) in [("Book", "ledger"), ("Supplier", "party")] {
        let row = b["actors"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["name"] == actor)
            .unwrap();
        assert_eq!(row["grain"], json!({"status":"declared","value":grain}));
        row["grain"] = json!({"status":"default","value":null});
    }
    normalize(&mut b, &positions);
    assert_eq!(
        a, b,
        "only independently enumerated input labels/grain/source coordinates may differ"
    );
}

#[test]
fn effect_assumptions_are_explicit_and_do_not_claim_acceptance_or_remedy() {
    let a = check("sov-basic");
    let assumptions = a["assumptions"].as_array().unwrap();
    for (field, value, affects) in [
        (
            "contest.occurrence",
            "nondeterministic_at_target_event",
            "対象の終端または commit で起票と不起票の両方を扱う。発生率は仮定しない。",
        ),
        (
            "contest.closure",
            "business_end_distinct_from_execution_close",
            "business_ended_open は業務 End の時点で異議が開いていた案件を数える。",
        ),
        (
            "contest.scope",
            "model_structure_and_runtime_not_acceptance",
            "効果の異議は commit の時点で起票し、route は process の残りの作業と同じ容量と人を取り合う。",
        ),
    ] {
        let row = assumptions.iter().find(|r| r["field"] == field).unwrap();
        assert_eq!(row["value"], value);
        assert!(row["affects"].as_str().unwrap().contains(affects), "{row}");
    }
    for word in [
        "contest_acceptance",
        "contest_remedy",
        "contest_real_world_occurrence",
        "contest_real_world_response_time",
    ] {
        assert!(
            a["agency"]["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!(word))
        );
    }
}
