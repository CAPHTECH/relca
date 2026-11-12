//! CLI delivery of the independent ADR0034 O/F/P/L hand tables.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/agency_viewpoints_acceptance.rs"]
mod hand;
#[path = "support/agency_changes.rs"]
mod projection;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

#[test]
fn actor_capacity_wire_reasons_are_row_local_and_framing_is_the_remaining_union() {
    // Hand table: Checker selects both ends. Withdraw only returned: done's
    // selector still contributes Checker. Withdraw both: the union is empty.
    let mut v = expected("agy-f-select");
    v["framing"]["ends"][1] = json!({
        "end":"returned", "status":"not_derived", "reason":"actor_capacity_infeasible"
    });
    v["framing"]["complete"] = json!(false);
    v["retention"]["capabilities"][0] = json!({
        "capability":"Checker.check", "status":"not_derived",
        "reason":"actor_capacity_infeasible", "contest_route":false,
        "basis":"unknown", "fallback":"unchecked", "join_any_cancellation":false
    });
    fixture_match::validate_viewpoints(&v).unwrap();
    let mut bad = v.clone();
    bad["framing"]["actors"] = json!([]);
    assert!(
        fixture_match::validate_viewpoints(&bad)
            .unwrap_err()
            .contains("Actor union")
    );
    let mut bad = v.clone();
    bad["framing"]["complete"] = json!(true);
    assert!(fixture_match::validate_viewpoints(&bad).is_err());
    v["framing"]["ends"][0] = json!({
        "end":"done", "status":"not_derived", "reason":"actor_capacity_infeasible"
    });
    v["framing"]["actors"] = json!([]);
    fixture_match::validate_viewpoints(&v).unwrap();
    let mut bad = v.clone();
    bad["framing"]["actors"] = json!(["Checker"]);
    assert!(
        fixture_match::validate_viewpoints(&bad)
            .unwrap_err()
            .contains("Actor union")
    );
    for block in ["observation", "framing", "retention", "proposal"] {
        let mut bad = v.clone();
        bad[block] = json!({"status":"not_derived", "reason":"actor_capacity_infeasible"});
        assert!(fixture_match::validate_viewpoints(&bad).is_err(), "{block}");
    }
    let mut bad = v.clone();
    bad["framing"]["ends"][0]["reachable"] = json!(false);
    assert!(fixture_match::validate_viewpoints(&bad).is_err());
    let mut bad = v;
    bad["retention"]["capabilities"][0]["normal"] = json!({"min":0,"max":0});
    assert!(fixture_match::validate_viewpoints(&bad).is_err());
}

#[test]
fn actor_capacity_reason_is_rejected_for_positive_capacity_reports() {
    for cmd in ["check", "simulate"] {
        let source = hand::source("agy-f-select").replace(
            "verify safety no_resource_overcommit",
            "simulation replications 2 seed 0 confidence 0.5\n verify safety no_resource_overcommit",
        );
        let path = temp_source(&source);
        let (r, _) = invoke(&[cmd, path.to_str().unwrap()]);
        let validate = |v: &Value| {
            if cmd == "check" {
                fixture_match::compare_report(&json!({}), v)
            } else {
                fixture_match::validate_simulation_wire(v)
            }
        };
        validate(&r).unwrap();
        for block in ["framing", "retention"] {
            let mut bad = r.clone();
            let vp = &mut bad["agency"]["viewpoints"];
            if block == "framing" {
                vp["framing"]["ends"] = json!([
                    {"end":"done","status":"not_derived","reason":"actor_capacity_infeasible"},
                    {"end":"returned","status":"not_derived","reason":"actor_capacity_infeasible"}
                ]);
                vp["framing"]["actors"] = json!([]);
                vp["framing"]["complete"] = json!(false);
            } else {
                vp["retention"]["capabilities"][0] = json!({
                    "capability":"Checker.check","status":"not_derived",
                    "reason":"actor_capacity_infeasible","contest_route":false,
                    "basis":"unknown","fallback":"unchecked","join_any_cancellation":false
                });
            }
            fixture_match::validate_viewpoints(&bad["agency"]["viewpoints"]).unwrap();
            assert!(
                validate(&bad)
                    .unwrap_err()
                    .contains("requires an Actor with capacity 0"),
                "{cmd} {block}"
            );
        }
    }
}

#[test]
fn actor_capacity_diff_reason_is_not_a_blanket_unknown_reason() {
    let table = |row: Value| {
        json!({
            "rules":"agency-diff-p2-1","status":"derived","rows":[row],
            "unchecked":["fallback_path","workload_transfer","via_model_paths"]
        })
    };
    let unknown = json!({"status":"not_derived","reason":"actor_capacity_infeasible"});
    for (viewpoint, item, key) in [
        ("framing", "reachable", json!({"end":"done"})),
        ("framing", "selectors", json!({"end":"done"})),
        ("framing", "read_only", json!({"end":"done"})),
        ("framing", "actors", json!({})),
        ("retention", "normal", json!({"capability":"A.x"})),
        ("retention", "fallback", json!({"capability":"A.x"})),
        ("retention", "contest_route", json!({"capability":"A.x"})),
        (
            "retention",
            "join_any_cancellation",
            json!({"capability":"A.x"}),
        ),
    ] {
        let mut row = json!({"viewpoint":viewpoint,"item":item,"status":"not_comparable",
            "from":unknown,"to":{"status":"derived","reason":null}});
        row.as_object_mut()
            .unwrap()
            .extend(key.as_object().unwrap().clone());
        let good = table(row);
        fixture_match::validate_agency_differences(&good).unwrap();
        let mut bad = good;
        bad["rows"][0]["from"]["status"] = json!("unchecked");
        assert!(fixture_match::validate_agency_differences(&bad).is_err());
    }
    for (vp, item, key) in [
        ("origin", "direct", json!({"actor":"Doc"})),
        ("observation", "normal.direct", json!({"person":"p"})),
        ("effects", "e", json!({"effect":"A.x","call":"x"})),
        ("refusal", "judgements", json!({"effect":"A.x","call":"x"})),
        ("lineage_groups", "actors", json!({"subject_sources":[]})),
    ] {
        let mut row = json!({"viewpoint":vp,"item":item,"status":"not_comparable",
            "from":{"status":"not_derived","reason":"state_guards"},
            "to":{"status":"derived","reason":null}});
        row.as_object_mut()
            .unwrap()
            .extend(key.as_object().unwrap().clone());
        let mut bad = table(row);
        fixture_match::validate_agency_differences(&bad).unwrap();
        bad["rows"][0]["from"]["reason"] = json!("actor_capacity_infeasible");
        assert!(
            fixture_match::validate_agency_differences(&bad).is_err(),
            "{vp}"
        );
    }
}

#[test]
fn actor_capacity_reason_in_diff_copies_and_comparison_pairs_requires_zero_capacity() {
    let source = hand::source("agy-f-select").replace(
        "process Flow {",
        "configuration Before { cut K; process Flow }\n configuration After { cut K; process Flow }\n compare [Before, After] cut K\n process Flow {",
    );
    let path = temp_source(&source);
    let path = path.to_str().unwrap();
    let (diff, _) = invoke(&["diff", path, "--from", "Before", "--to", "After"]);
    fixture_match::compare_diff_report(&json!({}), &diff).unwrap();
    for side in ["from", "to"] {
        let mut bad = diff.clone();
        bad["agency"][side]["viewpoints"]["framing"]["ends"] = json!([
            {"end":"done","status":"not_derived","reason":"actor_capacity_infeasible"},
            {"end":"returned","status":"not_derived","reason":"actor_capacity_infeasible"}
        ]);
        bad["agency"][side]["viewpoints"]["framing"]["actors"] = json!([]);
        bad["agency"][side]["viewpoints"]["framing"]["complete"] = json!(false);
        fixture_match::validate_viewpoints(&bad["agency"][side]["viewpoints"]).unwrap();
        assert!(
            fixture_match::compare_diff_report(&json!({}), &bad)
                .unwrap_err()
                .contains("requires an Actor with capacity 0")
        );
    }
    let rows = json!([{"viewpoint":"framing","item":"actors","status":"not_comparable",
        "from":{"status":"not_derived","reason":"actor_capacity_infeasible"},
        "to":{"status":"derived","reason":null}}]);
    let mut bad = diff;
    bad["agency"]["differences"]["rows"] = rows.clone();
    fixture_match::validate_agency_differences(&bad["agency"]["differences"]).unwrap();
    assert!(
        fixture_match::compare_diff_report(&json!({}), &bad)
            .unwrap_err()
            .contains("requires an Actor with capacity 0")
    );
    let (mut comparison, _) = invoke(&[
        "compare",
        path,
        "--plans",
        "Before,After",
        "--mode",
        "exhaustive",
    ]);
    fixture_match::validate_compare_wire(&comparison).unwrap();
    comparison["tables"]["agency"][0]["differences"]["rows"] = rows;
    assert!(
        fixture_match::validate_compare_wire(&comparison)
            .unwrap_err()
            .contains("requires an Actor with capacity 0")
    );
    // Same DSL, one capacity edit: zero is a legitimate precondition for both
    // per-side rows and their not_comparable differences, not a schema error.
    let zero_path = temp_source(&source.replace("capacity 1", "capacity 0"));
    let zero_path = zero_path.to_str().unwrap();
    let (zero_diff, _) = invoke(&["diff", zero_path, "--from", "Before", "--to", "After"]);
    fixture_match::compare_diff_report(&json!({}), &zero_diff).unwrap();
    let rows = zero_diff["agency"]["differences"]["rows"]
        .as_array()
        .unwrap();
    assert!(
        rows.iter()
            .any(|r| r["from"]["reason"] == "actor_capacity_infeasible")
    );
    let (zero_compare, _) = invoke(&[
        "compare",
        zero_path,
        "--plans",
        "Before,After",
        "--mode",
        "exhaustive",
    ]);
    fixture_match::validate_compare_wire(&zero_compare).unwrap();
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn invoke(args: &[&str]) -> (Value, Output) {
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .current_dir(root())
        .output()
        .unwrap();
    let v = serde_json::from_slice(&o.stdout).unwrap();
    (v, o)
}
fn temp_source(source: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = root().join("target/agency-viewpoints-inputs");
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join(format!(
        "{}-{}.relca",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&p, source).unwrap();
    p
}
fn expected(name: &str) -> Value {
    hand::expected(name)["agency"]["viewpoints"].clone()
}
fn viewpoints(r: &Value) -> Value {
    assert!(r["agency"].is_object(), "preflight must lower: {r}");
    let v = r["agency"]
        .get("viewpoints")
        .expect("ADR0034 viewpoints missing (unimplemented)");
    fixture_match::validate_viewpoints(v).unwrap();
    hand::legacy_viewpoints(v)
}
#[test]
fn every_pair_checks_exit_stderr_full_ordered_viewpoints_and_diagnostics() {
    for n in hand::population()["names"].as_array().unwrap() {
        let name = n.as_str().unwrap();
        let p = format!("fixtures/{name}/model.relca");
        let args = if name.starts_with("agy-invoice") {
            vec!["check", p.as_str(), "--configuration", "AsIs"]
        } else {
            vec!["check", p.as_str()]
        };
        let (v, o) = invoke(&args);
        hand::assert_report(name, &v);
        let verdict: relca_core::Verdict =
            serde_json::from_value(hand::expected(name)["verdict"].clone()).unwrap();
        assert_eq!(o.status.code(), Some(verdict.exit_code()));
        let want: String = v["diagnostics"]
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
        assert_eq!(o.stderr, want.as_bytes(), "stderr {name}");
        if !v["agency"].is_null() {
            assert_eq!(viewpoints(&v), expected(name));
        }
    }
}
#[test]
fn check_and_configuration_free_explain_have_the_same_new_table() {
    for name in [
        "agy-o-people",
        "agy-f-or",
        "agy-l-state-local",
        "agy-o-contest",
    ] {
        let p = format!("fixtures/{name}/model.relca");
        for cmd in ["check", "explain"] {
            let (v, o) = invoke(&[cmd, &p]);
            assert_eq!(o.status.code(), Some(0));
            assert_eq!(viewpoints(&v), expected(name));
        }
    }
}
#[test]
fn simulate_witness_has_viewpoints_but_missing_simulation_has_null_agency() {
    let (bad, o) = invoke(&["simulate", "fixtures/agy-o-people/model.relca"]);
    assert_eq!(o.status.code(), Some(3));
    assert!(bad["agency"].is_null());
    assert!(
        bad["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-SIM-0")
    );
    let s = hand::source("agy-p-name");
    let s = hand::replace(
        &s,
        "verify safety no_resource_overcommit",
        "simulation replications 10 seed 0 confidence 0.95\n verify reachable end.done",
    );
    let p = temp_source(&s);
    let (v, o) = invoke(&["simulate", p.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "{v}");
    assert_eq!(v["properties"][0]["cases"][0]["basis"], "witness");
    assert_eq!(viewpoints(&v), expected("agy-p-name"));
}

#[test]
fn compare_and_diff_include_every_report_and_top_agency_copy() {
    let s = hand::source("agy-p-name");
    let s = hand::replace(
        &s,
        "process Flow {",
        "configuration Before { cut K; process Flow }\n configuration After { cut K; process Flow }\n process Flow {",
    );
    let p = temp_source(&s);
    let p = p.to_str().unwrap();
    let want = expected("agy-p-name");
    let (v, o) = invoke(&[
        "compare",
        p,
        "--plans",
        "Before,After",
        "--mode",
        "exhaustive",
    ]);
    assert_eq!(o.status.code(), Some(0), "{v}");
    let (diff, diff_output) = invoke(&["diff", p, "--from", "Before", "--to", "After"]);
    assert_eq!(diff_output.status.code(), Some(0), "{diff}");
    for r in v["plans"].as_array().unwrap() {
        assert_eq!(viewpoints(&r["report"]), want);
    }
    for side in ["from", "to"] {
        assert_eq!(viewpoints(&diff[side]["report"]), want);
        assert_eq!(
            hand::legacy_viewpoints(&diff["agency"][side]["viewpoints"]),
            want
        );
    }
    assert_eq!(diff["differences"], json!([]));
}

#[test]
fn invoice_asis_and_tobea_match_independent_q4_oracle_without_explain_configuration() {
    let s = hand::source("agy-invoice");
    for (configuration, name) in [
        ("AsIs", "agy-invoice"),
        ("ToBeA", "agy-invoice.broken-ai-transfer"),
    ] {
        let a = serde_json::to_value(relca_core::report::check_configuration(&s, configuration))
            .unwrap();
        assert_eq!(viewpoints(&a), expected(name));
    }
}

#[test]
fn mutate_reports_every_viewpoint_delta_only_as_other_changes_not_detection() {
    let (v, o) = invoke(&["mutate", "fixtures/agy-o-contest/model.relca"]);
    assert_eq!(o.status.code(), Some(4));
    assert_eq!(v["verdict"], "unsupported");
    assert_eq!(viewpoints(&v["original"]), expected("agy-o-contest"));
    let mut routes = 0;
    let mut reports = 0;
    for row in v["mutations"].as_array().unwrap() {
        for m in row["mutants"].as_array().into_iter().flatten() {
            if !m["report"].is_null() {
                reports += 1;
                viewpoints(&m["report"]);
                let want = projection::changes(
                    &v["original"]["agency"]["viewpoints"],
                    &m["report"]["agency"]["viewpoints"],
                );
                let actual: Vec<_> = m["other_changes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|r| {
                        r["check"] == "agency"
                            && r["key"]
                                .as_str()
                                .is_some_and(|k| k.starts_with("/viewpoints/"))
                    })
                    .cloned()
                    .collect();
                assert_eq!(actual, want, "all typed leaf deltas at {}", m["id"]);
            }
            assert!(
                !m["assigned_changes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["key"].as_str().is_some_and(|k| k.contains("viewpoints")))
            );
            if m["site"]["actor"] == "Clerk" && m["site"]["capability"] == "recheck" {
                routes += 1;
                assert_eq!(m["operator"], "remove_read");
                assert_eq!(m["result"], "undetected");
                assert_eq!(m["assigned_changes"], json!([]));
                assert_eq!(
                    m["reason"],
                    if row["kind"] == "evidence_missing" {
                        json!("no_worsening")
                    } else {
                        json!("no_assigned_check")
                    }
                );
                assert_eq!(
                    viewpoints(&m["report"]),
                    expected("agy-o-contest.broken-route-no-read")
                );
            }
        }
    }
    assert_eq!(routes, 2);
    assert!(reports > 2);
}

#[test]
fn closed_schema_rejects_each_missing_extra_and_wrong_type_field_in_hand_tables() {
    fn mutate(v: &Value, root: &Value, p: &str) {
        match v {
            Value::Object(o) => {
                for (k, x) in o {
                    let mut bad = root.clone();
                    bad.pointer_mut(p)
                        .unwrap()
                        .as_object_mut()
                        .unwrap()
                        .remove(k);
                    assert!(
                        fixture_match::validate_viewpoints(&bad).is_err(),
                        "missing {p}/{k}"
                    );
                    let mut bad = root.clone();
                    bad.pointer_mut(p)
                        .unwrap()
                        .as_object_mut()
                        .unwrap()
                        .insert("unknown_field".into(), json!(true));
                    assert!(fixture_match::validate_viewpoints(&bad).is_err());
                    let child = format!("{p}/{k}");
                    let mut bad = root.clone();
                    *bad.pointer_mut(&child).unwrap() = Value::Null;
                    assert!(
                        fixture_match::validate_viewpoints(&bad).is_err(),
                        "null {child}"
                    );
                    mutate(x, root, &child);
                }
            }
            Value::Array(a) => {
                for (i, x) in a.iter().enumerate() {
                    mutate(x, root, &format!("{p}/{i}"));
                }
            }
            _ => {}
        }
    }
    for name in [
        "agy-o-people",
        "agy-f-state",
        "agy-l-state-local",
        "agy-invoice",
    ] {
        let v = expected(name);
        fixture_match::validate_viewpoints(&v).unwrap();
        mutate(&v, &v, "");
    }
}

#[test]
fn soft_budget_wire_drops_entire_block_payload_and_never_uses_empty_derived_arrays() {
    let full = expected("agy-soft-budget");
    for (block, payload) in [
        ("observation", "sources"),
        ("framing", "ends"),
        ("retention", "capabilities"),
    ] {
        let mut v = full.clone();
        v[block] = match block {
            "observation" => {
                json!({"status":"not_derived","reason":"derivation_limit","fallback":"unchecked"})
            }
            "framing" => {
                json!({"status":"not_derived","reason":"derivation_limit","complete":false,"unchecked":["type_domain_authority","practical_framing"]})
            }
            "retention" => {
                json!({"status":"not_derived","reason":"derivation_limit","notice":"行使回数から不要とは判定しない","unchecked":["fallback_dependency","retention_basis","actual_skill"]})
            }
            _ => unreachable!(),
        };
        fixture_match::validate_viewpoints(&v).unwrap();
        assert_eq!(v["proposal"], full["proposal"]);
        let mut bad = v.clone();
        bad[block][payload] = json!([]);
        assert!(
            fixture_match::validate_viewpoints(&bad).is_err(),
            "not_derived cannot masquerade as an empty derived collection"
        );
        let mut bad = v.clone();
        bad[block]["reason"] = json!("state_guards");
        assert!(fixture_match::validate_viewpoints(&bad).is_err());
        if block == "framing" {
            v[block]["complete"] = json!(true);
            assert!(fixture_match::validate_viewpoints(&v).is_err());
        }
    }
}

#[test]
fn resource_infeasible_is_a_closed_row_reason_but_never_a_block_reason() {
    // F end rows and L capability rows add
    // resource_infeasible; block-level reasons stay derivation_limit only.
    let (r, _) = invoke(&["check", "fixtures/rsrc-over-demand/model.relca"]);
    let v = viewpoints(&r).clone();
    let mut row = v.clone();
    row["framing"]["ends"][0] =
        json!({"status":"not_derived","end":"done","reason":"resource_infeasible"});
    row["framing"]["complete"] = json!(false);
    row["retention"]["capabilities"][0] = json!({"status":"not_derived","capability":"A.x","reason":"resource_infeasible","contest_route":false,"basis":"unknown","fallback":"unchecked","join_any_cancellation":false});
    fixture_match::validate_viewpoints(&row).unwrap();
    for (path, word) in [
        ("/framing/ends/0/reason", "resource_infeasibility"),
        ("/retention/capabilities/0/reason", "resource_shortage"),
    ] {
        let mut bad = row.clone();
        *bad.pointer_mut(path).unwrap() = json!(word);
        assert!(fixture_match::validate_viewpoints(&bad).is_err(), "{path}");
    }
    for block in ["framing", "retention"] {
        let mut bad = v.clone();
        let payload = if block == "framing" {
            "ends"
        } else {
            "capabilities"
        };
        bad[block].as_object_mut().unwrap().remove(payload);
        bad[block]["status"] = json!("not_derived");
        bad[block]["reason"] = json!("resource_infeasible");
        if block == "framing" {
            bad[block]["complete"] = json!(false);
            bad[block].as_object_mut().unwrap().remove("actors");
        }
        assert!(fixture_match::validate_viewpoints(&bad).is_err(), "{block}");
    }
}
