//! CLI delivery and closed-wire negatives for the independent ADR0043 tables.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/agency_second_half_acceptance.rs"]
mod hand;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

fn invoke(args: &[&str]) -> (Value, Output) {
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .current_dir(hand::root())
        .output()
        .unwrap();
    let v = serde_json::from_slice(&o.stdout).unwrap();
    (v, o)
}
fn temp(source: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = hand::root().join("target/agency-second-half-inputs");
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join(format!(
        "{}-{}.relca",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&p, source).unwrap();
    p
}
fn effects(v: &Value) -> &Value {
    let vp = &v["agency"]["viewpoints"];
    assert_eq!(
        vp["rules"], "agency-viewpoints-p1-2",
        "new rule version required"
    );
    fixture_match::validate_viewpoints(vp).unwrap();
    vp.get("effects")
        .expect("ADR0043 effects missing (unimplemented)")
}

#[test]
fn every_authored_expectation_and_invocation_is_closed_before_running_cli() {
    let rows = hand::population();
    assert_eq!(rows.len(), fixture_match::AGENCY_SECOND_HALF_FIXTURES.len());
    for row in rows {
        let name = row["name"].as_str().unwrap();
        assert!(fixture_match::AGENCY_SECOND_HALF_FIXTURES.contains(&name));
        let invocation = fixture_match::Invocation::parse(
            (!row["invocation"].is_null()).then_some(&row["invocation"]),
        )
        .unwrap();
        invocation
            .validate_expected(&hand::expected(name))
            .unwrap_or_else(|e| panic!("authored {name}: {e}"));
    }
}

#[test]
fn check_explain_preserve_all_writers_not_just_latest_snapshot_value() {
    let path = temp(&hand::all_writers_source());
    for command in ["check", "explain"] {
        let (a, o) = invoke(&[command, path.to_str().unwrap()]);
        assert_eq!(o.status.code(), Some(0), "{a}");
        let c = &effects(&a)["rows"][0]["concentration"];
        assert_eq!(
            c["generation_roots"],
            json!([{"call":"old","capability":"Earlier.write"},{"call":"e","capability":"Clerk.enter"}])
        );
        assert_eq!(c["single_generation_root"], false);
    }
}

#[test]
fn all_pairs_exit_diagnostic_multiplicity_and_complete_hand_effects_together() {
    let mut failed = vec![];
    for row in hand::population() {
        let name = row["name"].as_str().unwrap();
        let p = format!("fixtures/{name}/model.relca");
        let inv = &row["invocation"];
        let args = match inv["command"].as_str() {
            Some("check") => vec![
                "check",
                p.as_str(),
                "--configuration",
                inv["configuration"].as_str().unwrap(),
            ],
            Some("diff") => vec![
                "diff",
                p.as_str(),
                "--from",
                inv["from"].as_str().unwrap(),
                "--to",
                inv["to"].as_str().unwrap(),
            ],
            _ => vec!["check", p.as_str()],
        };
        let (v, o) = invoke(&args);
        let result = std::panic::catch_unwind(|| {
            hand::assert_report(&row, &v);
            if name == "agy2-static-still-called.broken-rejected" {
                let (d, output) = invoke(&["diff", p.as_str(), "--from", "From", "--to", "To"]);
                assert_eq!(
                    output.status.code(),
                    Some(3),
                    "still-called keep intent must also fail diff: {d}"
                );
                assert_eq!(d["verdict"], "invalid");
                let diagnostics: Vec<_> = d["diagnostics"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .chain(
                        d["to"]["report"]["diagnostics"]
                            .as_array()
                            .into_iter()
                            .flatten(),
                    )
                    .collect();
                assert!(diagnostics.iter().any(|x| x["code"] == "E-DIFF-0"));
                assert!(!diagnostics.iter().any(|x| x["code"] == "W-AGY-3"));
            }
            let verdict: relca_core::Verdict =
                serde_json::from_value(hand::expected(name)["verdict"].clone()).unwrap();
            assert_eq!(o.status.code(), Some(verdict.exit_code()), "{name}");
            if inv["command"] != "diff" {
                let lines: String = v["diagnostics"]
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
                assert_eq!(
                    o.stderr,
                    lines.as_bytes(),
                    "stderr preserves exact ordered diagnostic sequence {name}"
                );
                if !v["agency"].is_null() {
                    effects(&v);
                }
            } else {
                fixture_match::validate_agency_differences(&v["agency"]["differences"]).unwrap();
            }
        });
        if result.is_err() {
            failed.push(name.to_owned());
        }
    }
    assert!(
        failed.is_empty(),
        "all independent failing inputs:\n{}",
        failed.join("\n")
    );
}

#[test]
fn check_and_configuration_free_explain_deliver_same_effects_and_fallback_is_selected_check_only() {
    let p = "fixtures/agy2-jue/model.relca";
    let (a, ao) = invoke(&["check", p]);
    let (b, bo) = invoke(&["explain", p]);
    assert_eq!(ao.status.code(), Some(0));
    assert_eq!(bo.status.code(), Some(0));
    assert_eq!(effects(&a), effects(&b));
    let (to, o) = invoke(&[
        "check",
        "fixtures/agy2-keep-fallback/model.relca",
        "--configuration",
        "To",
    ]);
    assert_eq!(o.status.code(), Some(0), "{to}");
    assert_eq!(
        to["agency"]["viewpoints"]["retention"]["capabilities"][0]["fallback"],
        "kept_by_intent"
    );
    assert_eq!(
        to["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-AGY-3")
            .count(),
        1
    );
}

#[test]
fn compare_simulate_and_diff_preserve_each_nested_report_and_top_copy() {
    let source = hand::replace(
        &hand::source("agy2-state-write-only.broken-commits"),
        "  process P",
        "  configuration Before { cut K; process P }\n  configuration After { cut K; process P }\n  process P",
    );
    let p = temp(&source);
    let p = p.to_str().unwrap();
    let (check, _) = invoke(&["check", p, "--configuration", "Before"]);
    let want = effects(&check);
    let (compare, o) = invoke(&[
        "compare",
        p,
        "--plans",
        "Before,After",
        "--mode",
        "exhaustive",
    ]);
    assert_eq!(o.status.code(), Some(0), "{compare}");
    for plan in compare["plans"].as_array().unwrap() {
        assert_eq!(effects(&plan["report"]), want);
    }
    let (diff, o) = invoke(&["diff", p, "--from", "Before", "--to", "After"]);
    assert_eq!(o.status.code(), Some(0), "{diff}");
    for side in ["from", "to"] {
        assert_eq!(effects(&diff[side]["report"]), want);
        assert_eq!(diff["agency"][side], diff[side]["report"]["agency"]);
    }
    assert_eq!(diff["agency"]["differences"]["rows"], json!([]));
    // Deterministic simulation witness, not a declaration of all-schedule safety.
    // An integer would add implicit state_in_range, whose sampled non-violation
    // correctly remains inconclusive. Keep only the deterministic reach witness.
    let simulation_source = hand::replace(
        &hand::source("agy2-state-write-only.broken-commits"),
        "state n: 0..1 = 0;",
        "",
    );
    let simulation_source = hand::replace(&simulation_source, "writes n; finish { n += 1 };", "");
    let s = hand::replace(
        &simulation_source,
        "verify safety no_resource_overcommit",
        "simulation replications 10 seed 0 confidence 0.95\n  verify reachable end.done",
    );
    let path = temp(&s);
    let (simulation, o) = invoke(&["simulate", path.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "{simulation}");
    assert_eq!(effects(&simulation), want);
}

#[test]
fn keep_annotation_plain_retire_and_diff_full_meaning_rows_are_separate() {
    for (name, kept) in [
        ("agy2-keep-fallback", true),
        ("agy2-keep-fallback.broken-plain-retire", false),
    ] {
        let p = format!("fixtures/{name}/model.relca");
        let (v, o) = invoke(&["diff", &p, "--from", "From", "--to", "To"]);
        assert_eq!(o.status.code(), Some(0), "{v}");
        let removed = v["differences"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["kind"] == "relation" && r["target"] == "Clerk.enter")
            .unwrap();
        assert_eq!(
            removed["annotations"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|a| a["kind"] == "keep_fallback"),
            kept
        );
        assert_eq!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["code"] == "W-DIFF-2")
                .count(),
            usize::from(!kept)
        );
        assert_eq!(v["agency"]["to"], v["to"]["report"]["agency"]);
    }
}

#[test]
fn mutant_new_effects_remain_other_changes_and_never_become_assigned_detection() {
    let (v, o) = invoke(&["mutate", "fixtures/agy2-jue/model.relca"]);
    assert_eq!(o.status.code(), Some(4));
    effects(&v["original"]);
    let mut n = 0;
    let mut guards = 0;
    for row in v["mutations"].as_array().unwrap() {
        for m in row["mutants"].as_array().into_iter().flatten() {
            if m["report"].is_null() {
                continue;
            }
            effects(&m["report"]);
            n += 1;
            for c in m["assigned_changes"].as_array().unwrap() {
                assert!(!c["key"].as_str().unwrap_or("").contains("effects"));
            }
            if m["operator"] == "bypass_guard" {
                guards += 1;
                assert!(
                    m["other_changes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|r| r["key"]
                            .as_str()
                            .is_some_and(|s| s.starts_with("/viewpoints/effects/"))),
                    "all typed effects changes must be carried: {m}"
                );
            }
        }
    }
    assert!(n > 0);
    assert!(guards > 0, "the guard-delta assertion must actually run");
}

#[test]
fn new_wire_rejects_missing_wrong_extra_fields_and_same_unknown_is_explicit() {
    let vp = hand::expected("agy2-jue")["agency"]["viewpoints"].clone();
    let e = vp["effects"].clone();
    fixture_match::validate_effects(&e).unwrap();
    fn walk(v: &Value, original: &Value, p: &str) {
        if let Some(obj) = v.as_object() {
            for (key, x) in obj {
                let mut b = original.clone();
                b.pointer_mut(p)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                assert!(
                    fixture_match::validate_effects(&b).is_err(),
                    "missing {p}/{key}"
                );
                let mut b = original.clone();
                b.pointer_mut(p)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert("unknown".into(), json!(true));
                assert!(fixture_match::validate_effects(&b).is_err());
                let q = format!("{p}/{key}");
                let mut b = original.clone();
                *b.pointer_mut(&q).unwrap() = Value::Null;
                assert!(
                    fixture_match::validate_effects(&b).is_err(),
                    "wrong type {q}"
                );
                walk(x, original, &q);
            }
        } else if let Some(a) = v.as_array() {
            for (i, x) in a.iter().enumerate() {
                walk(x, original, &format!("{p}/{i}"));
            }
        }
    }
    walk(&e, &e, "");
    let d = hand::expected("agy2-diff-items")["agency"]["differences"].clone();
    fixture_match::validate_agency_differences(&d).unwrap();
    for item in ["unknown.item", "normal.score"] {
        let mut b = d.clone();
        b["rows"][0]["item"] = json!(item);
        assert!(fixture_match::validate_agency_differences(&b).is_err());
    }
    let mut b = d.clone();
    let duplicate = b["rows"][0].clone();
    b["rows"].as_array_mut().unwrap().push(duplicate);
    assert!(fixture_match::validate_agency_differences(&b).is_err());
    let unknown =
        hand::expected("agy2-diff-items.broken-state-guard")["agency"]["differences"].clone();
    fixture_match::validate_agency_differences(&unknown).unwrap();
    let mut b = unknown.clone();
    b["rows"][6]["from"]["reason"] = json!("invented");
    assert!(fixture_match::validate_agency_differences(&b).is_err());
}

/// Deferred T17 (after B5, ADR 0042): Agency is derived from the IR of the
/// selected scenario, never from the scenario label. The hand expectations
/// are the deferred note's (Ordered: violated/exit 1, W-DEADLOCK-0,
/// W-UNREACH-0 twice, W-UNREACH-2, done F and both L rows not_derived
/// resource_infeasible; Loose: satisfied/exit 0, W-UNREACH-0 for big only,
/// done F derived reachable read_only b, L big 0..0 false, small 1..1 true;
/// effects empty for both) and must equal the two authored substrate
/// expectations exactly.
#[test]
fn t17_scenario_reads_selected_policy_not_scenario_label() {
    let base = hand::source("agy2-rsrc-callorder");
    let line = "  policy fifo tie_by case_id then call_order\n";
    assert_eq!(base.matches(line).count(), 1);
    let ordered_body = "policy fifo tie_by case_id then call_order";
    let loose_body = "policy fifo tie_by case_id";
    let ordered = hand::expected("agy2-scenario-callorder-substrate");
    let loose = hand::expected("agy2-scenario-callorder-substrate.broken-loose");
    // The whole F/L payloads of the two scenario-free substrates, which the
    // family t17_selected_policy_substrate checks against the same files.
    let substrate = |name: &str| {
        let (v, _) = invoke(&["check", &format!("fixtures/{name}/model.relca")]);
        let vp = &v["agency"]["viewpoints"];
        (vp["framing"].clone(), vp["retention"].clone())
    };
    let ordered_fl = substrate("agy2-scenario-callorder-substrate");
    let loose_fl = substrate("agy2-scenario-callorder-substrate.broken-loose");
    for (first, second) in [("Ordered", "Loose"), ("Loose", "Ordered")] {
        // first carries call_order, second does not: the exchange swaps names
        // over the same contents, so only the contents may matter.
        let source = hand::replace(
            &base,
            line,
            &format!(
                "  scenario {first} {{ {ordered_body} }}\n  scenario {second} {{ {loose_body} }}\n"
            ),
        );
        assert!(!source.contains(line) && source.contains("scenario Loose"));
        let path = temp(&source);
        for (name, want, fl, exit, codes) in [
            (
                first,
                &ordered,
                &ordered_fl,
                1,
                &["W-DEADLOCK-0", "W-UNREACH-0", "W-UNREACH-0", "W-UNREACH-2"][..],
            ),
            (second, &loose, &loose_fl, 0, &["W-UNREACH-0"][..]),
        ] {
            for command in ["check", "explain"] {
                let (v, o) = invoke(&[command, path.to_str().unwrap(), "--scenario", name]);
                assert_eq!(o.status.code(), Some(exit), "{command} {name}: {v}");
                hand::subset(want, &v, &format!("{command} {name}"));
                let got: Vec<_> = v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|d| d["code"].as_str().unwrap())
                    .collect();
                assert_eq!(got, codes, "{command} {name}");
                assert_eq!(effects(&v)["rows"], json!([]));
                let vp = &v["agency"]["viewpoints"];
                assert_eq!((vp["framing"].clone(), vp["retention"].clone()), *fl);
            }
        }
    }
}

/// Deferred B4 (after ADR 0039) through the CLI: the same hand input as the
/// core target; the overflow End never enters F and renaming it keeps F, L
/// and effects exactly.
#[test]
fn overflow_end_never_becomes_framing_classification() {
    let source = r#"model Overflow version "0.2" {
  type T = enum { ok }
  actor A { capacity 1; capability run() -> T { duration fixed 2min; result nondet { ok } } }
  process Flow { r = A.run(); end done }
  arrivals fixed [0min, 0min]
  policy fifo tie_by case_id
  concurrent cases 1 on_overflow end spill
  queue capacity 1 on_overflow end spill
  cut K { include [A] }
  bounds { cases 2; live_tasks 1; clock 20min }
  verify safety no_resource_overcommit
}
"#;
    let mut views = vec![];
    for s in [source.to_owned(), source.replace("end spill", "end other")] {
        let path = temp(&s);
        let (v, o) = invoke(&["check", path.to_str().unwrap()]);
        assert_eq!(o.status.code(), Some(0), "{v}");
        assert!(o.stderr.is_empty());
        let vp = &v["agency"]["viewpoints"];
        let ends: Vec<_> = vp["framing"]["ends"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["end"].clone())
            .collect();
        assert_eq!(ends, [json!("done")]);
        assert_eq!(effects(&v)["rows"], json!([]));
        assert_eq!(
            vp["retention"]["capabilities"][0]["normal"],
            json!({"min":1,"max":1})
        );
        views.push((vp["framing"].clone(), vp["retention"].clone()));
    }
    assert_eq!(views[0], views[1]);
}

/// Deferred C4 (after ADR 0040) through the CLI: the hand input of the core
/// target; E has only Ledger.post/ack, the route commits call stays out of E
/// and is the contest route in L, and route reads change only O's contest
/// block.
#[test]
fn effect_standing_route_is_not_normal_effect_row() {
    let base = hand::source("sov-basic");
    let base = hand::replace(
        &base,
        "  actor Vendor",
        "  actor Doc { source document }\n  actor Vendor",
    );
    let base = hand::replace(
        &base,
        "include [Approver, Ledger, Clerk, Vendor]",
        "include [Approver, Ledger, Clerk, Doc, Vendor]",
    );
    let mut blocks = vec![];
    for (reads, contest) in [
        (
            "",
            json!({"actors":[],"people":[],"unattributed_actors":[],"routes":[]}),
        ),
        (
            "reads [Doc]; ",
            json!({"actors":["Clerk"],"people":["c1"],"unattributed_actors":[],"routes":["Clerk.recheck"]}),
        ),
    ] {
        let s = hand::replace(
            &base,
            "capability recheck() -> Verdict { duration fixed 1min;",
            &format!("capability recheck() -> Verdict {{ {reads}commits; duration fixed 1min;"),
        );
        let path = temp(&s);
        let (v, o) = invoke(&["check", path.to_str().unwrap()]);
        assert_eq!(o.status.code(), Some(0), "{v}");
        let rows = effects(&v)["rows"].as_array().unwrap().clone();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (rows[0]["effect"].clone(), rows[0]["call"].clone()),
            (json!("Ledger.post"), json!("ack"))
        );
        let a = &v["agency"];
        assert!(
            a["viewpoints"]["retention"]["capabilities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["capability"] == "Clerk.recheck" && c["contest_route"] == true)
        );
        // O's contest block follows the route's reads (ADR 0040, kept by C5);
        // the legacy origin counts process calls only and stays empty.
        let doc = &a["viewpoints"]["observation"]["sources"][0];
        assert_eq!(doc["actor"], "Doc");
        assert_eq!(doc["contest"], contest);
        let clerk = a["origin"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["actor"] == "Clerk")
            .unwrap();
        assert_eq!(clerk["direct"], json!([]));
        blocks.push(effects(&v).clone());
    }
    assert_eq!(blocks[0], blocks[1]);
}
